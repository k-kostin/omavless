// SPDX-License-Identifier: MIT
use super::*;
use crate::app_proxy::codec::{DesktopValue, EnvironmentValue, Override};

fn owner() -> Owner {
    Owner {
        instance: [7; 16],
        generation: 3,
    }
}

pub(crate) fn states() -> (State, State) {
    let mut original_desktop = Vec::new();
    let mut target_desktop = Vec::new();
    for key in DesktopKey::ALL {
        let default = match key.schema_key_type().2 {
            "s" if key == DesktopKey::Mode => DesktopValue::String("none".into()),
            "s" => DesktopValue::String(String::new()),
            "b" => DesktopValue::Bool(false),
            "i" => DesktopValue::Int(0),
            "as" => DesktopValue::Strings(Vec::new()),
            _ => unreachable!(),
        };
        let new = match key.schema_key_type().2 {
            "s" if key == DesktopKey::Mode => DesktopValue::String("manual".into()),
            "s" => DesktopValue::String("synthetic".into()),
            "b" => DesktopValue::Bool(true),
            "i" => DesktopValue::Int(7890),
            "as" => DesktopValue::Strings(vec!["localhost".into()]),
            _ => unreachable!(),
        };
        original_desktop.push(DesktopEntry {
            key,
            effective: default.clone(),
            default: default.clone(),
            user: Override::Absent,
            writable: true,
        });
        target_desktop.push(DesktopEntry {
            key,
            effective: new.clone(),
            default,
            user: Override::Present(new),
            writable: true,
        });
    }
    let original_manager = EnvironmentKey::ALL
        .into_iter()
        .map(|key| EnvironmentEntry {
            key,
            value: EnvironmentValue::Absent,
        })
        .collect();
    let target_manager = EnvironmentKey::ALL
        .into_iter()
        .map(|key| EnvironmentEntry {
            key,
            value: EnvironmentValue::Present("synthetic".into()),
        })
        .collect();
    (
        State::new(
            DesktopSnapshot::capture(original_desktop).unwrap(),
            EnvironmentSnapshot::capture(original_manager).unwrap(),
        ),
        State::new(
            DesktopSnapshot::capture(target_desktop).unwrap(),
            EnvironmentSnapshot::capture(target_manager).unwrap(),
        ),
    )
}

fn apply_and_confirm(planner: &mut Planner, current: &mut State) -> bool {
    let Some(effect) = planner.begin_next(owner(), current).unwrap() else {
        return false;
    };
    assert_eq!(current.value(effect.field), effect.expected);
    *current = current.with(effect.field, effect.replacement).unwrap();
    planner.confirm(owner(), current).unwrap();
    true
}

#[test]
fn all_26_fields_restore_in_reverse_after_each_partial_apply_and_unknown_write() {
    let (original, intended) = states();
    assert_eq!(Field::all().count(), FIELD_COUNT);
    for confirmed in 0..=FIELD_COUNT {
        for uncertain_written in [false, true] {
            let mut current = original.clone();
            let mut planner =
                Planner::prepare(owner(), original.clone(), intended.clone()).unwrap();
            for _ in 0..confirmed {
                assert!(apply_and_confirm(&mut planner, &mut current));
            }
            if confirmed < FIELD_COUNT {
                let effect = planner.begin_next(owner(), &current).unwrap().unwrap();
                assert_eq!(
                    planner.begin_next(owner(), &current),
                    Err(Error::PendingEffect)
                );
                if uncertain_written {
                    current = current.with(effect.field, effect.replacement).unwrap();
                }
            } else {
                assert!(!apply_and_confirm(&mut planner, &mut current));
                assert_eq!(planner.phase(), Phase::Active);
            }
            planner.begin_restore(owner(), &current).unwrap();
            let mut restored = Vec::new();
            while let Some(effect) = planner.begin_next(owner(), &current).unwrap() {
                restored.push(effect.field.index());
                current = current.with(effect.field, effect.replacement).unwrap();
                planner.confirm(owner(), &current).unwrap();
            }
            assert!(restored.windows(2).all(|pair| pair[0] > pair[1]));
            assert_eq!(planner.phase(), Phase::Released);
            assert_eq!(current, original);
        }
    }
}

#[test]
fn foreign_values_unattempted_fields_and_stale_owner_refuse() {
    let (original, intended) = states();
    let mut planner = Planner::prepare(owner(), original.clone(), intended).unwrap();
    assert_eq!(
        planner.begin_next(
            Owner {
                generation: 4,
                ..owner()
            },
            &original
        ),
        Err(Error::StaleOwner)
    );
    let effect = planner.begin_next(owner(), &original).unwrap().unwrap();
    // A third value on an attempted field is not our uncertain write.
    let mut third = match effect.replacement {
        Value::Desktop(entry) => entry,
        _ => unreachable!(),
    };
    third.user = Override::Present(DesktopValue::String("auto".into()));
    third.effective = DesktopValue::String("auto".into());
    let current = original.with(effect.field, Value::Desktop(third)).unwrap();
    assert_eq!(
        planner.begin_restore(owner(), &current),
        Err(Error::ForeignChange)
    );
    // A matching target on a field whose intent was never recorded also refuses.
    let not_attempted = Field::Desktop(DesktopKey::HttpHost);
    let current = original
        .with(not_attempted, planner.intended.value(not_attempted))
        .unwrap();
    assert_eq!(
        planner.begin_restore(owner(), &current),
        Err(Error::ForeignChange)
    );
}

#[test]
fn changed_default_lock_or_inconsistent_target_refuses() {
    let (original, intended) = states();
    let field = Field::Desktop(DesktopKey::Mode);
    let Value::Desktop(mut changed) = original.value(field) else {
        unreachable!()
    };
    changed.default = DesktopValue::String("manual".into());
    let changed_default = original.with(field, Value::Desktop(changed)).unwrap();
    assert!(matches!(
        Planner::prepare(owner(), original.clone(), changed_default),
        Err(Error::InvalidSnapshot)
    ));
    let Value::Desktop(mut changed) = intended.value(field) else {
        unreachable!()
    };
    changed.writable = false;
    let locked = intended.with(field, Value::Desktop(changed)).unwrap();
    assert!(matches!(
        Planner::prepare(owner(), original.clone(), locked),
        Err(Error::InvalidSnapshot)
    ));
    let mut planner = Planner::prepare(owner(), original.clone(), intended).unwrap();
    let effect = planner.begin_next(owner(), &original).unwrap().unwrap();
    let Value::Desktop(mut changed) = effect.replacement else {
        unreachable!()
    };
    changed.default = DesktopValue::String("manual".into());
    let current = original.with(field, Value::Desktop(changed)).unwrap();
    assert_eq!(
        planner.begin_restore(owner(), &current),
        Err(Error::ForeignChange)
    );
}

#[test]
fn field_state_codec_and_debug_keep_absent_distinct_from_empty() {
    let (original, intended) = states();
    assert_eq!(
        State::decode(&original.encode().unwrap()).unwrap(),
        original
    );
    assert_eq!(
        State::decode(&intended.encode().unwrap()).unwrap(),
        intended
    );
    let field = Field::UserManager(EnvironmentKey::Http);
    let Value::UserManager(mut entry) = original.value(field) else {
        unreachable!()
    };
    entry.value = EnvironmentValue::Present(String::new());
    let empty = original.with(field, Value::UserManager(entry)).unwrap();
    assert_ne!(empty, original);
    assert!(!format!("{empty:?}").contains("http_proxy"));
    assert!(!format!("{:?}", empty.value(field)).contains("http_proxy"));
}
