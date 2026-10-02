// SPDX-License-Identifier: MIT
use super::*;
use crate::app_proxy::{codec::DesktopKey, fields::tests::mode_state};

const ORDER: Order = Order::ModeLastOriginalNone;

#[test]
fn ordered_intent_reopens_at_every_prefix_and_each_restore_intent() {
    let (original, intended) = states();
    let mode = Field::Desktop(DesktopKey::Mode);
    let sequence: Vec<_> = ORDER.fields().collect();
    for confirmed in 0..=FIELD_COUNT {
        for pending_written in [false, true] {
            let temp = Temp::new();
            let mut journal = FieldJournal::create_ordered(
                &temp.0,
                binding(),
                original.clone(),
                intended.clone(),
                ORDER,
            )
            .unwrap();
            let mut observed = original.clone();
            for expected in sequence.iter().take(confirmed) {
                let effect = journal.begin_next(binding(), &observed).unwrap().unwrap();
                assert_eq!(effect.field, *expected);
                observed = observed.with(effect.field, effect.replacement).unwrap();
                journal.confirm(binding(), &observed).unwrap();
            }
            if confirmed < FIELD_COUNT {
                let effect = journal.begin_next(binding(), &observed).unwrap().unwrap();
                if pending_written {
                    observed = observed.with(effect.field, effect.replacement).unwrap();
                }
            }
            let bytes = fs::read(temp.record()).unwrap();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(document["version"], 3);
            assert_eq!(document["order"], "mode_last_original_none");
            drop(journal);
            let mut journal = FieldJournal::open(&temp.0, binding()).unwrap();
            assert_eq!(journal.order(), ORDER);
            assert!(matches!(
                journal.begin_next(binding(), &observed),
                Err(Error::RecoveryRequired)
            ));
            assert_eq!(fs::read(temp.record()).unwrap(), bytes);
            journal.begin_restore(binding(), &observed).unwrap();
            let active = observed.value(mode) != original.value(mode);
            let mut count = 0;
            while let Some(effect) = journal.begin_next(binding(), &observed).unwrap() {
                if count == 0 && active {
                    assert_eq!(effect.field, mode);
                }
                // Restore pending mode's stable index is zero but order rank
                // is last. Decode must validate rank, not enum index.
                drop(journal);
                journal = FieldJournal::open(&temp.0, binding()).unwrap();
                assert_eq!(journal.order(), ORDER);
                observed = observed.with(effect.field, effect.replacement).unwrap();
                // Reopening models storage only, not real request settlement;
                // fixtures separately establish retained same-port drain.
                journal.begin_restore(binding(), &observed).unwrap();
                count += 1;
                assert_eq!(observed.value(mode), original.value(mode));
            }
            assert_eq!(observed, original);
            assert_eq!(journal.phase(), Phase::Released);
        }
    }
}

#[test]
fn v2_bytes_and_mode_first_semantics_remain_unchanged() {
    let temp = Temp::new();
    let (original, intended) = states();
    let mut journal = FieldJournal::create(&temp.0, binding(), original.clone(), intended).unwrap();
    let bytes = fs::read(temp.record()).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["version"], 2);
    assert!(document.get("order").is_none());
    assert_eq!(encode(binding(), &journal.planner).unwrap(), bytes);
    assert_eq!(
        journal
            .begin_next(binding(), &original)
            .unwrap()
            .unwrap()
            .field,
        Field::Desktop(DesktopKey::Mode)
    );
    let pending = fs::read(temp.record()).unwrap();
    drop(journal);
    let journal = FieldJournal::open(&temp.0, binding()).unwrap();
    assert_eq!(journal.order(), Order::LegacyModeFirst);
    assert_eq!(fs::read(temp.record()).unwrap(), pending);
}

#[test]
fn version_order_mismatch_and_nonprefix_mode_intent_are_rejected() {
    let temp = Temp::new();
    let (original, intended) = states();
    let journal =
        FieldJournal::create_ordered(&temp.0, binding(), original, intended, ORDER).unwrap();
    let initial: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.record()).unwrap()).unwrap();
    drop(journal);
    for case in 0..7 {
        let mut changed = initial.clone();
        match case {
            0 => {
                changed.as_object_mut().unwrap().remove("order");
            }
            1 => changed["version"] = serde_json::json!(2),
            2 => changed["order"] = serde_json::json!(null),
            3 => changed["order"] = serde_json::json!("legacy_mode_first"),
            4 => {
                changed["version"] = serde_json::json!(2);
                changed["order"] = serde_json::json!(null);
            }
            5 => {
                changed["attempted"][0] = serde_json::json!(true);
                changed["pending"] = serde_json::json!(0);
            }
            6 => changed["expected"][0] = serde_json::json!("intended"),
            _ => unreachable!(),
        }
        fs::write(temp.record(), serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(matches!(
            FieldJournal::open(&temp.0, binding()),
            Err(Error::Invalid)
        ));
    }
    // Named order fields must not be duplicate-last-wins.
    let bytes = serde_json::to_vec(&initial).unwrap();
    let mut duplicate = b"{\"order\":\"mode_last_original_none\",".to_vec();
    duplicate.extend_from_slice(&bytes[1..]);
    fs::write(temp.record(), duplicate).unwrap();
    assert!(matches!(
        FieldJournal::open(&temp.0, binding()),
        Err(Error::Invalid)
    ));
}

#[test]
fn unsupported_baseline_creates_no_record_and_foreign_mode_keeps_ordered_intent() {
    let (original, intended) = states();
    for prior in ["manual", "auto"] {
        let temp = Temp::new();
        assert!(matches!(
            FieldJournal::create_ordered(
                &temp.0,
                binding(),
                mode_state(&original, prior, true),
                intended.clone(),
                ORDER,
            ),
            Err(Error::Planner(crate::app_proxy::Error::UnsupportedOrder))
        ));
        assert!(!temp.record().exists());
    }
    let temp = Temp::new();
    let mut journal =
        FieldJournal::create_ordered(&temp.0, binding(), original.clone(), intended, ORDER)
            .unwrap();
    let effect = journal.begin_next(binding(), &original).unwrap().unwrap();
    assert_ne!(effect.field, Field::Desktop(DesktopKey::Mode));
    let bytes = fs::read(temp.record()).unwrap();
    let foreign = mode_state(&original, "manual", true);
    assert_eq!(
        journal
            .recovery_review(binding(), &foreign)
            .unwrap()
            .decision,
        RecoveryDecision::PreserveForeignEdits
    );
    assert_eq!(
        journal.begin_restore(binding(), &foreign),
        Err(Error::Planner(crate::app_proxy::Error::ForeignChange))
    );
    assert_eq!(fs::read(temp.record()).unwrap(), bytes);
}

#[test]
fn restoring_endpoint_intent_while_mode_is_still_enabled_is_invalid() {
    let (original, intended) = states();
    let mut planner =
        Planner::prepare_ordered(binding().owner(), original.clone(), intended.clone(), ORDER)
            .unwrap();
    let mut observed = original;
    while let Some(effect) = planner.begin_next(binding().owner(), &observed).unwrap() {
        observed = observed.with(effect.field, effect.replacement).unwrap();
        planner.confirm(binding().owner(), &observed).unwrap();
    }
    planner.begin_restore(binding().owner(), &observed).unwrap();
    let mode = planner
        .begin_next(binding().owner(), &observed)
        .unwrap()
        .unwrap();
    assert_eq!(mode.field, Field::Desktop(DesktopKey::Mode));
    let valid = encode(binding(), &planner).unwrap();
    assert!(decode(&valid).is_ok());
    let mut corrupt: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    corrupt["pending"] = serde_json::json!(Field::Desktop(DesktopKey::SocksPort).index());
    assert!(matches!(
        decode(&serde_json::to_vec(&corrupt).unwrap()),
        Err(Error::Invalid)
    ));
}
