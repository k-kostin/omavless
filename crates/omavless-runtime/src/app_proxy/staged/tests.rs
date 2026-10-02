// SPDX-License-Identifier: MIT
use super::*;

fn owner() -> Owner {
    Owner {
        instance: [7; 16],
        generation: 4,
    }
}

pub(crate) fn states(mode: &str, present: bool, default: &str) -> (State, State) {
    let (mut original, intended) = fields::tests::states();
    let Value::Desktop(mut before) = original.value(MODE) else {
        unreachable!()
    };
    before.default = DesktopValue::String(default.into());
    before.effective = DesktopValue::String(if present { mode } else { default }.into());
    before.user = if present {
        Override::Present(before.effective.clone())
    } else {
        Override::Absent
    };
    original = original.with(MODE, Value::Desktop(before.clone())).unwrap();
    let Value::Desktop(mut after) = intended.value(MODE) else {
        unreachable!()
    };
    after.default = before.default;
    let intended = intended.with(MODE, Value::Desktop(after)).unwrap();
    let pair = intended.encode().unwrap();
    let manager = original.encode().unwrap();
    let intended = State::decode(&[pair[0].clone(), manager[1].clone()]).unwrap();
    (original, intended)
}

pub(crate) fn cases() -> impl Iterator<Item = (State, State)> {
    [
        ("none", false, "none"),
        ("none", true, "none"),
        ("manual", true, "none"),
        ("auto", true, "none"),
        ("manual", false, "manual"),
        ("auto", false, "auto"),
    ]
    .into_iter()
    .map(|(mode, present, default)| states(mode, present, default))
}

fn act(planner: &mut Planner, current: &mut State) -> Option<Step> {
    let effect = planner.begin_next(owner(), current).unwrap()?;
    if matches!(effect.step, Step::ApplyControl(_) | Step::RestoreControl(_)) {
        assert_eq!(current.value(MODE), planner.mode_value(ModeSide::Quiescent));
    }
    if effect.step == Step::RestoreMode {
        for key in controls() {
            assert_eq!(
                current.value(Field::Desktop(key)),
                planner.original.value(Field::Desktop(key))
            );
        }
    }
    *current = current
        .with(effect.change.field, effect.change.replacement)
        .unwrap();
    planner.confirm(owner(), current).unwrap();
    Some(effect.step)
}

fn restore(mut planner: Planner, mut current: State, original: &State) {
    planner.begin_restore(owner(), &current).unwrap();
    while act(&mut planner, &mut current).is_some() {}
    assert_eq!(planner.stage(), Stage::Released);
    assert_eq!(&current, original);
}

#[test]
fn six_layered_baselines_round_trip_with_four_distinct_mode_operations() {
    for (original, intended) in cases() {
        let mut current = original.clone();
        let mut planner = Planner::prepare(owner(), original.clone(), intended.clone()).unwrap();
        let mut steps = Vec::new();
        while let Some(step) = act(&mut planner, &mut current) {
            steps.push(step);
        }
        assert_eq!(current, intended);
        assert_eq!(planner.stage(), Stage::Active);
        assert_eq!(steps.last(), Some(&Step::Enable));
        assert_eq!(
            steps
                .iter()
                .filter(|step| matches!(step, Step::ApplyControl(_)))
                .count(),
            CONTROL_COUNT
        );
        planner.begin_restore(owner(), &current).unwrap();
        assert_eq!(act(&mut planner, &mut current), Some(Step::QuiesceRestore));
        while let Some(step) = act(&mut planner, &mut current) {
            steps.push(step);
        }
        assert_eq!(current, original);
        assert_eq!(planner.stage(), Stage::Released);
        assert_ne!(Step::QuiesceApply, Step::QuiesceRestore);
        assert_ne!(Step::Enable, Step::RestoreMode);
    }
}

#[test]
fn every_apply_prefix_and_unknown_outcome_can_only_compensate_via_quiescence() {
    for (original, intended) in cases() {
        let mut prefix = Planner::prepare(owner(), original.clone(), intended).unwrap();
        let mut current = original.clone();
        loop {
            restore(prefix.clone(), current.clone(), &original);
            let mut pending = prefix.clone();
            if let Some(effect) = pending.begin_next(owner(), &current).unwrap() {
                for written in [false, true] {
                    let observed = if written {
                        current
                            .with(effect.change.field, effect.change.replacement.clone())
                            .unwrap()
                    } else {
                        current.clone()
                    };
                    assert_eq!(pending.pending(), Some(effect.step));
                    restore(pending.clone(), observed, &original);
                }
            }
            if act(&mut prefix, &mut current).is_none() {
                break;
            }
        }
    }
}

#[test]
fn every_restore_prefix_and_unknown_outcome_preserves_mode_last_restoration() {
    for (original, intended) in cases() {
        let mut prefix = Planner::prepare(owner(), original.clone(), intended).unwrap();
        let mut current = original.clone();
        while act(&mut prefix, &mut current).is_some() {}
        prefix.begin_restore(owner(), &current).unwrap();
        loop {
            if prefix.stage() != Stage::Released {
                restore(prefix.clone(), current.clone(), &original);
            }
            let mut pending = prefix.clone();
            if let Some(effect) = pending.begin_next(owner(), &current).unwrap() {
                for written in [false, true] {
                    let observed = if written {
                        current
                            .with(effect.change.field, effect.change.replacement.clone())
                            .unwrap()
                    } else {
                        current.clone()
                    };
                    restore(pending.clone(), observed, &original);
                }
            }
            if act(&mut prefix, &mut current).is_none() {
                break;
            }
        }
    }
}

#[test]
fn foreign_mode_reset_defaults_locks_manager_and_stale_owner_refuse() {
    let (original, intended) = states("manual", true, "none");
    let mut planner = Planner::prepare(owner(), original.clone(), intended.clone()).unwrap();
    let quiescent = original
        .with(MODE, planner.mode_value(ModeSide::Quiescent))
        .unwrap();
    // An unattempted external none is not an owned quiescence transition.
    assert_eq!(
        planner.begin_restore(owner(), &quiescent),
        Err(Error::ForeignChange)
    );
    assert_eq!(
        planner.begin_next(
            Owner {
                generation: 5,
                ..owner()
            },
            &original
        ),
        Err(Error::StaleOwner)
    );
    let mut current = original.clone();
    act(&mut planner, &mut current).unwrap();
    let changed = Field::Desktop(DesktopKey::AutoconfigUrl);
    act(&mut planner, &mut current).unwrap();
    let external_reset = current.with(changed, original.value(changed)).unwrap();
    assert_eq!(
        planner.begin_restore(owner(), &external_reset),
        Err(Error::ForeignChange)
    );
    let Value::Desktop(mut mode) = current.value(MODE) else {
        unreachable!()
    };
    mode.default = DesktopValue::String("auto".into());
    assert_eq!(
        planner.begin_restore(owner(), &current.with(MODE, Value::Desktop(mode)).unwrap()),
        Err(Error::ForeignChange)
    );
    let Value::Desktop(mut mode) = intended.value(MODE) else {
        unreachable!()
    };
    mode.writable = false;
    assert!(matches!(
        Planner::prepare(
            owner(),
            original.clone(),
            intended.with(MODE, Value::Desktop(mode)).unwrap()
        ),
        Err(Error::InvalidSnapshot)
    ));
    let (base, manager_changed) = fields::tests::states();
    assert!(matches!(
        Planner::prepare(owner(), base, manager_changed),
        Err(Error::UnsupportedOrder)
    ));
    assert_eq!(
        planner.begin_next(owner(), &current).unwrap().unwrap().step,
        Step::ApplyControl(DesktopKey::IgnoreHosts)
    );
    assert!(!format!("{planner:?}").contains("synthetic"));
}
