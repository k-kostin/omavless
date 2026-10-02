// SPDX-License-Identifier: MIT
use super::tests::{ordinary_edit, prepared};
use super::*;
use crate::login_activation::epoch_candidate::CurrentEpochProof;
use crate::restore_successor_publication_candidate::tests::Fixture;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::{
    fs::{MetadataExt, PermissionsExt},
    process::ExitStatusExt,
};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const EPOCH: &str = "11111111111111111111111111111111";
const NEXT: &str = "22222222222222222222222222222222";
const WORKER: &str = "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::epoch_tests::epoch_resync_crash_worker";

struct OffHost {
    before_observe: Box<dyn FnMut()>,
    occupied: bool,
}
impl crate::lifecycle::LifecycleHost for OffHost {
    fn observe(
        &mut self,
        _: &crate::desired::DesiredState,
    ) -> Result<crate::desired::OwnedObservation, crate::lifecycle::HostStepError> {
        (self.before_observe)();
        Ok(crate::desired::OwnedObservation {
            service_active: self.occupied,
            controller_ready: self.occupied,
            core_count: u8::from(self.occupied),
            tun_count: u8::from(self.occupied),
            active_profile_matches: true,
        })
    }
    fn prepare(
        &mut self,
        _: &crate::desired::DesiredState,
    ) -> Result<(), crate::lifecycle::HostStepError> {
        panic!("research must not prepare")
    }
    fn start_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
        panic!("research must not start")
    }
    fn commit_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
        panic!("research must not commit")
    }
    fn stop_owned(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
        panic!("research must not stop")
    }
    fn discard_prepared(&mut self) -> Result<(), crate::lifecycle::HostStepError> {
        panic!("research must not discard")
    }
}

fn desired_paths(f: &Fixture) -> crate::desired::DesiredPaths {
    crate::desired::DesiredPaths {
        directory: f.paths.state_directory.clone(),
        file: f.paths.state_directory.join("desired.json"),
    }
}

#[test]
fn real_owner_off_research_reconciles_nochange_and_keeps_normal_entry_fenced() {
    for commit in [false, true] {
        let (f, lock) = prepared(commit);
        ordinary_edit(&f);
        receipt(&f);
        let store = f.config.join(LIVE[0]);
        // Establish an already-normalized ordinary pointer before proof capture.
        // The research entry itself has no authority to change this pointer.
        crate::private_store_transaction::prepare_pointer_mutation(
            &store,
            f.uid,
            omavless_domain::private_store::CompatibilityPointerTarget::Disconnected {
                prune_missing: true,
            },
        )
        .unwrap()
        .commit_locked(&lock, &f.paths)
        .unwrap();
        let before = fs::read(&store).unwrap();
        let identity = fs::metadata(&store).unwrap();
        let original = Snapshot::read_policy(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            LivePolicy::ValidCurrentOff,
        )
        .unwrap();
        let retained = witness(&f, &lock)
            .research(proof(&f, &lock), || true)
            .unwrap();
        let state_before = fs::metadata(&f.paths.state_directory).unwrap();
        let owner = crate::production_owner::ProductionNativeOwner::initialize_off_research(
            OffHost {
                before_observe: Box::new(|| {}),
                occupied: false,
            },
            desired_paths(&f),
            &store,
            f.paths.clone(),
            f.uid,
            (&lock, retained),
        )
        .unwrap();
        assert_eq!(owner.actual(), crate::lifecycle::ActualState::Disconnected);
        assert!(!owner.startup_outcome().changed);
        assert!(!owner.login_ready());
        let state_after = fs::metadata(&f.paths.state_directory).unwrap();
        assert_eq!(
            (state_before.ctime(), state_before.ctime_nsec()),
            (state_after.ctime(), state_after.ctime_nsec()),
            "research must not chmod the state directory"
        );
        assert_eq!(fs::read(&store).unwrap(), before);
        assert!(same_member(&identity, &fs::metadata(&store).unwrap()));
        assert!(
            original.same(
                &Snapshot::read_policy(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    LivePolicy::ValidCurrentOff
                )
                .unwrap()
            )
        );
        assert_fenced(&f, &lock);
        drop(owner);
        drop(lock);
        assert!(
            crate::production_owner::ProductionNativeOwner::initialize(
                OffHost {
                    before_observe: Box::new(|| panic!(
                        "ordinary pending startup must not observe"
                    )),
                    occupied: false
                },
                desired_paths(&f),
                &store,
                f.paths.clone(),
                f.uid
            )
            .is_err()
        );
    }
}

#[test]
fn real_owner_off_research_refuses_source_drift_and_owned_stop_before_effects() {
    for drift in [false, true] {
        let (f, lock) = prepared(true);
        ordinary_edit(&f);
        receipt(&f);
        let store = f.config.join(LIVE[0]);
        let before = fs::read(&store).unwrap();
        let retained = witness(&f, &lock)
            .research(proof(&f, &lock), || true)
            .unwrap();
        let receipt_path = f.paths.runtime_base.join("omavless-login.receipt");
        let host = OffHost {
            occupied: !drift,
            before_observe: Box::new(move || {
                if drift {
                    fs::remove_file(&receipt_path).unwrap();
                }
            }),
        };
        assert!(
            crate::production_owner::ProductionNativeOwner::initialize_off_research(
                host,
                desired_paths(&f),
                &store,
                f.paths.clone(),
                f.uid,
                (&lock, retained)
            )
            .is_err()
        );
        assert_eq!(fs::read(&store).unwrap(), before);
        assert_fenced(&f, &lock);
    }
}

#[test]
fn real_owner_off_research_refuses_pointer_repair_and_redirected_paths() {
    for redirected in [false, true] {
        let (f, lock) = prepared(false);
        ordinary_edit(&f);
        receipt(&f);
        let store = f.config.join(LIVE[0]);
        let mut data: serde_json::Value =
            serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        data["activeId"] = data["profiles"][0]["id"].clone();
        fs::write(&store, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(
            crate::private_store_transaction::prepare_pointer_mutation(
                &store,
                f.uid,
                omavless_domain::private_store::CompatibilityPointerTarget::Disconnected {
                    prune_missing: true
                }
            )
            .unwrap()
            .changed()
        );
        let before = fs::read(&store).unwrap();
        let retained = witness(&f, &lock)
            .research(proof(&f, &lock), || true)
            .unwrap();
        let mut desired = desired_paths(&f);
        if redirected {
            desired.file = f.config.join("other-desired.json");
        }
        assert!(
            crate::production_owner::ProductionNativeOwner::initialize_off_research(
                OffHost {
                    before_observe: Box::new(move || {
                        assert!(!redirected, "redirected paths must fail before observation");
                    }),
                    occupied: false
                },
                desired,
                &store,
                f.paths.clone(),
                f.uid,
                (&lock, retained)
            )
            .is_err()
        );
        assert_eq!(fs::read(&store).unwrap(), before);
        assert_fenced(&f, &lock);
    }
}

#[test]
fn real_owner_off_research_stale_manager_and_receipt_refuse_before_observation() {
    for missing in [false, true] {
        let (f, lock) = prepared(true);
        ordinary_edit(&f);
        receipt(&f);
        let current = Arc::new(AtomicBool::new(true));
        let source = current.clone();
        let p = CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
            Ok(if source.load(Ordering::SeqCst) {
                EPOCH
            } else {
                NEXT
            }
            .into())
        })
        .unwrap();
        let retained = witness(&f, &lock).research(p, || true).unwrap();
        if missing {
            fs::remove_file(f.paths.runtime_base.join("omavless-login.receipt")).unwrap();
        } else {
            current.store(false, Ordering::SeqCst);
        }
        let store = f.config.join(LIVE[0]);
        let before = fs::read(&store).unwrap();
        assert!(
            crate::production_owner::ProductionNativeOwner::initialize_off_research(
                OffHost {
                    before_observe: Box::new(|| panic!("stale proof must not observe")),
                    occupied: false
                },
                desired_paths(&f),
                &store,
                f.paths.clone(),
                f.uid,
                (&lock, retained)
            )
            .is_err()
        );
        assert_eq!(fs::read(&store).unwrap(), before);
        assert_fenced(&f, &lock);
    }
}

#[test]
fn real_owner_off_research_missing_original_state_never_recreates_directory() {
    let (f, lock) = prepared(true);
    ordinary_edit(&f);
    receipt(&f);
    let retained = witness(&f, &lock)
        .research(proof(&f, &lock), || true)
        .unwrap();
    let original = f.paths.state_directory.clone();
    fs::rename(&original, original.with_extension("retained-original")).unwrap();
    assert!(
        crate::production_owner::ProductionNativeOwner::initialize_off_research(
            OffHost {
                before_observe: Box::new(|| panic!("missing source must not observe")),
                occupied: false
            },
            desired_paths(&f),
            &f.config.join(LIVE[0]),
            f.paths.clone(),
            f.uid,
            (&lock, retained)
        )
        .is_err()
    );
    assert_eq!(
        fs::symlink_metadata(&original).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
}

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
