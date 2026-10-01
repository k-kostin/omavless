// SPDX-License-Identifier: MIT
use super::*;
use crate::app_proxy::fields::tests::states;
use crate::app_proxy::journal::storage::{Checkpoint, RECORD};
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

mod transfer;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omavless-s1-field-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join(FIELD_DIRECTORY))
            .unwrap();
        Self(path)
    }

    fn record(&self) -> PathBuf {
        self.0.join(FIELD_DIRECTORY).join(RECORD)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binding() -> Binding {
    Binding {
        owner_instance: [11; 16],
        owner_generation: 5,
        boot: [12; 16],
        session: [13; 16],
        uid: nix::unistd::geteuid().as_raw(),
    }
}

#[test]
fn durable_partial_apply_reopens_only_for_reverse_compensation_at_every_field() {
    let (original, intended) = states();
    for confirmed in 0..=FIELD_COUNT {
        for uncertain_written in [false, true] {
            let temp = Temp::new();
            let mut journal =
                FieldJournal::create(&temp.0, binding(), original.clone(), intended.clone())
                    .unwrap();
            let mut current = original.clone();
            for _ in 0..confirmed {
                let effect = journal.begin_next(binding(), &current).unwrap().unwrap();
                current = current.with(effect.field, effect.replacement).unwrap();
                journal.confirm(binding(), &current).unwrap();
            }
            if confirmed < FIELD_COUNT {
                let effect = journal.begin_next(binding(), &current).unwrap().unwrap();
                if uncertain_written {
                    current = current.with(effect.field, effect.replacement).unwrap();
                }
            } else {
                assert!(journal.begin_next(binding(), &current).unwrap().is_none());
                assert_eq!(journal.phase(), Phase::Active);
            }
            drop(journal);
            let mut recovered = FieldJournal::open(&temp.0, binding()).unwrap();
            assert!(matches!(
                recovered.begin_next(binding(), &current),
                Err(Error::RecoveryRequired)
            ));
            recovered.begin_restore(binding(), &current).unwrap();
            let mut indices = Vec::new();
            while let Some(effect) = recovered.begin_next(binding(), &current).unwrap() {
                indices.push(effect.field.index());
                current = current.with(effect.field, effect.replacement).unwrap();
                recovered.confirm(binding(), &current).unwrap();
            }
            assert!(indices.windows(2).all(|pair| pair[0] > pair[1]));
            assert_eq!(recovered.phase(), Phase::Released);
            assert_eq!(current, original);
            assert_eq!(
                fs::metadata(temp.record()).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

#[test]
fn mismatch_and_foreign_state_preserve_the_durable_record() {
    let (original, intended) = states();
    let temp = Temp::new();
    let mut journal =
        FieldJournal::create(&temp.0, binding(), original.clone(), intended.clone()).unwrap();
    let effect = journal.begin_next(binding(), &original).unwrap().unwrap();
    let unchanged = fs::read(temp.record()).unwrap();
    drop(journal);
    let mut wrong = binding();
    wrong.owner_generation += 1;
    assert!(matches!(
        FieldJournal::open(&temp.0, wrong),
        Err(Error::BindingMismatch)
    ));
    let mut recovered = FieldJournal::open(&temp.0, binding()).unwrap();
    let future = original
        .with(
            Field::Desktop(crate::app_proxy::codec::DesktopKey::HttpHost),
            intended.value(Field::Desktop(
                crate::app_proxy::codec::DesktopKey::HttpHost,
            )),
        )
        .unwrap();
    assert_eq!(
        recovered.begin_restore(binding(), &future),
        Err(Error::Planner(crate::app_proxy::Error::ForeignChange))
    );
    assert_eq!(fs::read(temp.record()).unwrap(), unchanged);
    // The attempted field may still be original; the saved intent remains.
    assert_ne!(effect.replacement, original.value(effect.field));
    recovered.begin_restore(binding(), &original).unwrap();
}

#[test]
fn fsync_failure_or_interrupted_staging_never_emits_an_effect() {
    let (original, intended) = states();
    let temp = Temp::new();
    let mut journal = FieldJournal::create(&temp.0, binding(), original.clone(), intended).unwrap();
    journal.storage.fail_at.set(Some(Checkpoint::FileSynced));
    assert!(journal.begin_next(binding(), &original).is_err());
    assert!(matches!(
        journal.begin_next(binding(), &original),
        Err(Error::RecoveryRequired)
    ));
    drop(journal);
    assert!(matches!(
        FieldJournal::open(&temp.0, binding()),
        Err(Error::Interrupted)
    ));
}

#[test]
fn wrong_record_version_or_noncanonical_state_cannot_reopen() {
    let (original, intended) = states();
    let temp = Temp::new();
    let journal = FieldJournal::create(&temp.0, binding(), original, intended).unwrap();
    assert!(!format!("{journal:?}").contains("synthetic"));
    drop(journal);
    let path = temp.record();
    let initial = fs::read(&path).unwrap();
    let mut record: serde_json::Value = serde_json::from_slice(&initial).unwrap();
    record["version"] = serde_json::json!(1);
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(matches!(
        FieldJournal::open(&temp.0, binding()),
        Err(Error::Invalid)
    ));
    record["version"] = serde_json::json!(2);
    record["attempted"][FIELD_COUNT - 1] = serde_json::json!(true);
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(matches!(
        FieldJournal::open(&temp.0, binding()),
        Err(Error::Invalid)
    ));
}

#[test]
fn fixed_child_never_reuses_v1_record_or_follows_a_child_symlink() {
    let (original, intended) = states();
    let temp = Temp::new();
    let v1 = temp.0.join(RECORD);
    fs::write(&v1, b"untouched-v1-sentinel").unwrap();
    fs::set_permissions(&v1, fs::Permissions::from_mode(0o600)).unwrap();
    let journal =
        FieldJournal::create(&temp.0, binding(), original.clone(), intended.clone()).unwrap();
    assert_eq!(fs::read(&v1).unwrap(), b"untouched-v1-sentinel");
    assert!(temp.record().exists());
    drop(journal);

    let temp = Temp::new();
    fs::remove_dir(temp.0.join(FIELD_DIRECTORY)).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(temp.0.join("other"))
        .unwrap();
    symlink("other", temp.0.join(FIELD_DIRECTORY)).unwrap();
    assert!(matches!(
        FieldJournal::create(&temp.0, binding(), original, intended),
        Err(Error::UnsafePath)
    ));
}
