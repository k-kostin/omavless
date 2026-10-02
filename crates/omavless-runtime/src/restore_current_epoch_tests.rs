// SPDX-License-Identifier: MIT
use super::tests::{ordinary_edit, prepared};
use super::*;
use crate::login_activation::epoch_candidate::CurrentEpochProof;
use crate::restore_successor_publication_candidate::tests::Fixture;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const EPOCH: &str = "11111111111111111111111111111111";
const NEXT: &str = "22222222222222222222222222222222";
const WORKER: &str = "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::epoch_tests::epoch_resync_crash_worker";

fn receipt(f: &Fixture) {
    let hash = format!(
        "{:x}",
        Sha256::digest([b"omavless-login-epoch-v1\0".as_slice(), EPOCH.as_bytes()].concat())
    );
    let path = f.paths.runtime_base.join("omavless-login.receipt");
    fs::write(&path, serde_json::json!({"schemaVersion":1,"epochHash":hash,"ownershipGeneration":2,"phase":"consumed"}).to_string()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn proof<'a>(f: &'a Fixture, lock: &'a MigrationLock) -> CurrentEpochProof<'a> {
    CurrentEpochProof::synthetic(&f.paths, f.uid, 2, lock, |_| Ok(EPOCH.into())).unwrap()
}
fn witness<'a>(f: &'a Fixture, lock: &'a MigrationLock) -> RetainedCurrentOff<'a> {
    RetainedCurrentOff::capture(&f.config, &f.paths, f.uid, 2, lock).unwrap()
}
fn assert_fenced(f: &Fixture, lock: &MigrationLock) {
    assert!(crate::pending_private_transaction::pending_at(
        &f.paths.state_directory
    ));
    assert!(
        crate::login_transaction::check_current_receipt(&f.paths, f.uid, lock, 2, EPOCH).is_err()
    );
    assert!(
        crate::login_transaction::check_startup_receipt(&f.paths, f.uid, lock, Some(2)).is_err()
    );
}

#[test]
fn one_shot_epoch_resync_commit_abort_after_edits_preserves_every_source() {
    for commit in [false, true] {
        let (f, lock) = prepared(commit);
        ordinary_edit(&f);
        receipt(&f);
        let original = Snapshot::read_policy(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            LivePolicy::ValidCurrentOff,
        )
        .unwrap();
        for _ in 0..2 {
            assert_eq!(
                witness(&f, &lock).consume(proof(&f, &lock), || true, |_| true),
                Ok(HistoricalResyncResult::ResynchronizedStillFenced)
            );
            let after = Snapshot::read_policy(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                LivePolicy::ValidCurrentOff,
            )
            .unwrap();
            assert!(original.same(&after));
            assert_fenced(&f, &lock);
        }
    }
}

#[test]
fn retained_admission_precedes_external_epoch_queries_and_cannot_rebase_sources() {
    for member in ["desired.json", COMPLETE_MEMBER, CLOSURE_MEMBER] {
        let (f, lock) = prepared(true);
        ordinary_edit(&f);
        receipt(&f);
        let retained = witness(&f, &lock);
        let path = f.paths.state_directory.join(member);
        let mut first = true;
        let proof = CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
            if first {
                first = false;
                let raw = fs::read(&path).unwrap();
                fs::rename(&path, path.with_extension("old-epoch-test")).unwrap();
                fs::write(&path, raw).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            Ok(EPOCH.into())
        })
        .unwrap();
        let mut hooks = 0;
        assert!(
            retained
                .consume(
                    proof,
                    || true,
                    |_| {
                        hooks += 1;
                        true
                    }
                )
                .is_err()
        );
        assert_eq!(hooks, 0);
        assert_fenced(&f, &lock);
    }
}

#[test]
fn epoch_receipt_and_host_drift_after_every_sync_remain_ambiguous() {
    let (f, lock) = prepared(true);
    ordinary_edit(&f);
    receipt(&f);
    let mut points = Vec::new();
    witness(&f, &lock)
        .consume(
            proof(&f, &lock),
            || true,
            |p| {
                points.push(p);
                true
            },
        )
        .unwrap();
    for selected in points {
        for kind in 0..3 {
            let (f, lock) = prepared(true);
            ordinary_edit(&f);
            receipt(&f);
            let changed = Arc::new(AtomicBool::new(false));
            let epoch_changed = Arc::clone(&changed);
            let proof = CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
                Ok(if kind == 0 && epoch_changed.load(Ordering::SeqCst) {
                    NEXT
                } else {
                    EPOCH
                }
                .into())
            })
            .unwrap();
            let path = f.paths.runtime_base.join("omavless-login.receipt");
            let result = witness(&f, &lock).consume(
                proof,
                || kind != 2 || !changed.load(Ordering::SeqCst),
                |p| {
                    if p == selected {
                        changed.store(true, Ordering::SeqCst);
                        if kind == 1 {
                            let raw = fs::read(&path).unwrap();
                            fs::rename(&path, path.with_extension("old-epoch-test")).unwrap();
                            fs::write(&path, raw).unwrap();
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                        }
                    }
                    true
                },
            );
            assert_eq!(
                result,
                Err(ExecutionError::Ambiguous),
                "{selected:?},kind={kind}"
            );
            assert_fenced(&f, &lock);
        }
    }
}

#[test]
fn epoch_resync_sigkill_at_every_checkpoint_requires_new_proof() {
    let (f, lock) = prepared(true);
    ordinary_edit(&f);
    receipt(&f);
    let mut count = 0;
    witness(&f, &lock)
        .consume(
            proof(&f, &lock),
            || true,
            |_| {
                count += 1;
                true
            },
        )
        .unwrap();
    for commit in [false, true] {
        for point in 0..count {
            let (f, lock) = prepared(commit);
            ordinary_edit(&f);
            receipt(&f);
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", WORKER])
                .env("OMAVLESS_SYNTHETIC_EPOCH_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_EPOCH_POINT", point.to_string())
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(9), "point={point}");
            let lock = f.lock();
            assert_eq!(
                witness(&f, &lock).consume(proof(&f, &lock), || true, |_| true),
                Ok(HistoricalResyncResult::ResynchronizedStillFenced)
            );
            assert_fenced(&f, &lock);
        }
    }
}

#[test]
#[ignore = "internal isolated epoch resync crash worker"]
fn epoch_resync_crash_worker() {
    let root = PathBuf::from(std::env::var_os("OMAVLESS_SYNTHETIC_EPOCH_ROOT").unwrap());
    let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_EPOCH_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let f = std::mem::ManuallyDrop::new(Fixture::reopen(root));
    let lock = f.lock();
    let mut index = 0;
    let _ = witness(&f, &lock).consume(
        proof(&f, &lock),
        || true,
        |_| {
            if index == selected {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            index += 1;
            true
        },
    );
    panic!("expected isolated crash");
}
