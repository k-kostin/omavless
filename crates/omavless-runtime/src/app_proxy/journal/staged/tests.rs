// SPDX-License-Identifier: MIT
use super::*;
use crate::app_proxy::journal::storage::{Checkpoint, RECORD, STAGING};
use crate::app_proxy::{
    codec::DesktopKey,
    fields::Field,
    staged::tests::{cases, states},
};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "s1-stage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join(DIRECTORY))
            .unwrap();
        Self(path)
    }
    fn record(&self) -> PathBuf {
        self.0.join(DIRECTORY).join(RECORD)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn binding() -> Binding {
    Binding {
        owner_instance: [8; 16],
        owner_generation: 4,
        boot: [9; 16],
        session: [10; 16],
        uid: nix::unistd::geteuid().as_raw(),
    }
}

fn mode_sites() -> Vec<(Step, Planner)> {
    let (original, intended) = states("manual", true, "none");
    let mut planner = Planner::prepare(binding().owner(), original.clone(), intended).unwrap();
    let mut current = original;
    let mut sites = Vec::new();
    let mut restoring = false;
    loop {
        let before = planner.clone();
        let Some(effect) = planner.begin_next(binding().owner(), &current).unwrap() else {
            if !restoring {
                restoring = true;
                planner.begin_restore(binding().owner(), &current).unwrap();
                continue;
            }
            break;
        };
        if matches!(
            effect.step,
            Step::QuiesceApply | Step::Enable | Step::QuiesceRestore | Step::RestoreMode
        ) {
            sites.push((effect.step, before));
        }
        current = current
            .with(effect.change.field, effect.change.replacement)
            .unwrap();
        planner.confirm(binding().owner(), &current).unwrap();
    }
    assert_eq!(sites.len(), 4);
    sites
}

#[test]
fn every_apply_and_restore_intent_round_trips_and_reopens_without_authority() {
    for (original, intended) in cases() {
        let temp = Temp::new();
        let mut journal =
            StagedJournal::create(&temp.0, binding(), original.clone(), intended.clone()).unwrap();
        let mut observed = original.clone();
        let mut applying = true;
        loop {
            let Some(effect) = journal.begin_next(binding(), &observed).unwrap() else {
                if applying {
                    assert_eq!(journal.stage(), Stage::Active);
                    applying = false;
                    journal.begin_restore(binding(), &observed).unwrap();
                    continue;
                }
                break;
            };
            let before = fs::read(temp.record()).unwrap();
            for (candidate, relation) in [
                (observed.clone(), Relationship::RecordedBefore),
                (
                    observed
                        .with(effect.change.field, effect.change.replacement.clone())
                        .unwrap(),
                    Relationship::PendingAfter,
                ),
            ] {
                let review = journal.recovery_review(binding(), &candidate).unwrap();
                assert_eq!(review.pending, Some(effect.step));
                assert_eq!(review.relationship, relation);
                assert_eq!(review.decision, Decision::RetainUnsettledEvidence);
                assert_eq!(fs::read(temp.record()).unwrap(), before);
            }
            let (recorded, decoded) = decode(&before).unwrap();
            assert!(recorded == binding());
            assert_eq!(encode(binding(), &decoded).unwrap(), before);
            observed = observed
                .with(effect.change.field, effect.change.replacement)
                .unwrap();
            journal.confirm(binding(), &observed).unwrap();
        }
        assert_eq!(observed, original);
        assert_eq!(journal.stage(), Stage::Released);
        assert_eq!(
            journal
                .recovery_review(binding(), &observed)
                .unwrap()
                .decision,
            Decision::RetainReleasedTombstone
        );
        assert_eq!(
            fs::metadata(temp.record()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(journal);
        let mut recovered = StagedJournal::open(&temp.0, binding()).unwrap();
        assert!(matches!(
            recovered.begin_next(binding(), &observed),
            Err(Error::RecoveryRequired)
        ));
    }
}

#[test]
fn every_partial_prefix_reopens_only_for_explicit_model_compensation() {
    let (original, intended) = states("auto", true, "none");
    let mut planner = Planner::prepare(binding().owner(), original.clone(), intended).unwrap();
    let mut current = original.clone();
    loop {
        for written in [false, true] {
            let temp = Temp::new();
            let mut pending = planner.clone();
            let effect = pending.begin_next(binding().owner(), &current).unwrap();
            let persisted = encode(binding(), &pending).unwrap();
            let storage = Storage::acquire(&temp.0.join(DIRECTORY)).unwrap();
            storage.replace(None, &persisted).unwrap();
            drop(storage);
            let mut journal = StagedJournal::open(&temp.0, binding()).unwrap();
            let mut observed = if written {
                effect
                    .as_ref()
                    .map(|effect| {
                        current
                            .with(effect.change.field, effect.change.replacement.clone())
                            .unwrap()
                    })
                    .unwrap_or(current.clone())
            } else {
                current.clone()
            };
            assert!(matches!(
                journal.begin_next(binding(), &observed),
                Err(Error::RecoveryRequired)
            ));
            // Pure model only; real tests separately require retained-origin drain.
            journal.begin_restore(binding(), &observed).unwrap();
            while let Some(effect) = journal.begin_next(binding(), &observed).unwrap() {
                observed = observed
                    .with(effect.change.field, effect.change.replacement)
                    .unwrap();
                journal.confirm(binding(), &observed).unwrap();
            }
            assert_eq!(observed, original);
        }
        let Some(effect) = planner.begin_next(binding().owner(), &current).unwrap() else {
            break;
        };
        current = current
            .with(effect.change.field, effect.change.replacement)
            .unwrap();
        planner.confirm(binding().owner(), &current).unwrap();
    }
}

#[test]
fn strict_stages_side_prefixes_and_distinct_format_are_not_reinterpreted() {
    let (original, intended) = states("manual", true, "none");
    let planner = Planner::prepare(binding().owner(), original.clone(), intended.clone()).unwrap();
    let bytes = encode(binding(), &planner).unwrap();
    let initial: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for (name, value) in [
        ("version", serde_json::json!(3)),
        ("mode", serde_json::json!("intended")),
        ("stage", serde_json::json!("enabling")),
        ("controls", serde_json::json!([true])),
        (
            "pending",
            serde_json::json!({"kind":"apply_control","key":"mode"}),
        ),
        (
            "pending",
            serde_json::json!({"kind":"quiesce_apply","key":"mode"}),
        ),
        ("pending", serde_json::json!({"kind":"restore_mode"})),
    ] {
        let mut bad = initial.clone();
        bad[name] = value;
        assert!(matches!(
            decode(&serde_json::to_vec(&bad).unwrap()),
            Err(Error::Invalid)
        ));
    }
    let text = String::from_utf8(bytes).unwrap();
    assert!(matches!(
        decode(
            text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1)
                .as_bytes()
        ),
        Err(Error::Invalid)
    ));
    let temp = Temp::new();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(temp.0.join("app-proxy-fields"))
        .unwrap();
    let legacy =
        super::super::fields::FieldJournal::create(&temp.0, binding(), original, intended).unwrap();
    assert!(matches!(
        StagedJournal::open(&temp.0, binding()),
        Err(Error::Missing)
    ));
    assert!(!temp.record().exists());
    drop(legacy);
}

#[test]
fn fsync_failures_poison_before_any_effect_and_drift_preserves_every_byte() {
    for point in [
        Checkpoint::Created,
        Checkpoint::Written,
        Checkpoint::FileSynced,
        Checkpoint::Renamed,
        Checkpoint::DirectorySynced,
    ] {
        let temp = Temp::new();
        let (original, intended) = states("manual", true, "none");
        let mut journal =
            StagedJournal::create(&temp.0, binding(), original.clone(), intended).unwrap();
        journal.storage.fail_at.set(Some(point));
        assert!(journal.begin_next(binding(), &original).is_err());
        assert!(matches!(
            journal.begin_next(binding(), &original),
            Err(Error::RecoveryRequired)
        ));
        drop(journal);
        if matches!(
            point,
            Checkpoint::Created | Checkpoint::Written | Checkpoint::FileSynced
        ) {
            assert!(matches!(
                StagedJournal::open(&temp.0, binding()),
                Err(Error::Interrupted)
            ));
        } else {
            let mut journal = StagedJournal::open(&temp.0, binding()).unwrap();
            assert!(matches!(
                journal.begin_next(binding(), &original),
                Err(Error::RecoveryRequired)
            ));
        }
    }
    let temp = Temp::new();
    let (original, intended) = states("auto", true, "none");
    let mut journal =
        StagedJournal::create(&temp.0, binding(), original.clone(), intended.clone()).unwrap();
    let before = fs::read(temp.record()).unwrap();
    let foreign = original
        .with(
            Field::Desktop(DesktopKey::HttpHost),
            intended.value(Field::Desktop(DesktopKey::HttpHost)),
        )
        .unwrap();
    assert_eq!(
        journal
            .recovery_review(binding(), &foreign)
            .unwrap()
            .decision,
        Decision::PreserveForeignEdits
    );
    assert!(journal.begin_restore(binding(), &foreign).is_err());
    assert_eq!(fs::read(temp.record()).unwrap(), before);
    fs::write(temp.record(), b"foreign private bytes").unwrap();
    assert_eq!(
        journal.recovery_review(binding(), &original),
        Err(Error::ForeignChange)
    );
}

#[test]
fn crash_worker() {
    let Some(path) = std::env::var_os("OMAVLESS_TEST_STAGE_CRASH") else {
        return;
    };
    let point = match std::env::var("OMAVLESS_TEST_STAGE_POINT").unwrap().as_str() {
        "created" => Checkpoint::Created,
        "written" => Checkpoint::Written,
        "synced" => Checkpoint::FileSynced,
        "renamed" => Checkpoint::Renamed,
        "directory" => Checkpoint::DirectorySynced,
        _ => panic!("fixed checkpoint"),
    };
    let journal = StagedJournal::open(Path::new(&path), binding()).unwrap();
    // Test-only storage publication, not an executor or bypass of recovered
    // admission. No host operation can occur in this process.
    let observed = journal.planner.expected().unwrap();
    let mut candidate = journal.planner.clone();
    assert!(
        candidate
            .begin_next(binding().owner(), &observed)
            .unwrap()
            .is_some()
    );
    let payload = encode(binding(), &candidate).unwrap();
    journal.storage.crash_at.set(Some(point));
    let _ = journal.storage.replace(Some(&journal.persisted), &payload);
    panic!("expected fixture exit");
}

#[test]
fn actual_process_exit_at_all_storage_boundaries_remains_fenced() {
    use std::process::{Command, Stdio};
    for (step, planner) in mode_sites() {
        for point in ["created", "written", "synced", "renamed", "directory"] {
            let temp = Temp::new();
            let observed = planner.expected().unwrap();
            let storage = Storage::acquire(&temp.0.join(DIRECTORY)).unwrap();
            storage
                .replace(None, &encode(binding(), &planner).unwrap())
                .unwrap();
            drop(storage);
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "app_proxy::journal::staged::tests::crash_worker"])
                .env("OMAVLESS_TEST_STAGE_CRASH", &temp.0)
                .env("OMAVLESS_TEST_STAGE_POINT", point)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(71));
            if temp.0.join(DIRECTORY).join(STAGING).exists() {
                assert!(matches!(
                    StagedJournal::open(&temp.0, binding()),
                    Err(Error::Interrupted)
                ));
            } else {
                let mut journal = StagedJournal::open(&temp.0, binding()).unwrap();
                assert_eq!(journal.planner.pending(), Some(step));
                assert_eq!(
                    journal
                        .recovery_review(binding(), &observed)
                        .unwrap()
                        .decision,
                    Decision::RetainUnsettledEvidence
                );
                assert!(matches!(
                    journal.begin_next(binding(), &observed),
                    Err(Error::RecoveryRequired)
                ));
            }
        }
    }
}
