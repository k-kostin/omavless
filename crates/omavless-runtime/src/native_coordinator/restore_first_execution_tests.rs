// SPDX-License-Identifier: MIT
use super::*;
use crate::desired::{DesiredState, OwnedObservation, write_desired};
use crate::lifecycle::{HostStepError, NativeLocalObservation};
use crate::mutation::MutationDigest;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::Arc;

const PASSWORD: &[u8] = b"synthetic fixture passphrase";
const PORTABLE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;

struct OffHost {
    idle: bool,
    observations: usize,
    auxiliary: Arc<crate::auxiliary_core::AuxiliarySlot>,
}
impl LifecycleHost for OffHost {
    fn auxiliary_slot(&self) -> Option<Arc<crate::auxiliary_core::AuxiliarySlot>> {
        Some(self.auxiliary.clone())
    }
    fn fresh_observation(
        &mut self,
        _: &DesiredState,
    ) -> Result<NativeLocalObservation, HostStepError> {
        self.observations += 1;
        Ok(NativeLocalObservation {
            owned_core_running: !self.idle,
            visible_mihomo_count: 0,
            owned_auxiliary_mihomo_count: 0,
            visible_tun_count: 0,
            managed_tun_count: 0,
            owned_controller_config_verified: false,
            desired_profile_matches_owned: false,
        })
    }
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        panic!("no lifecycle effect")
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
}

struct Fixture {
    root: PathBuf,
    backup: PathBuf,
    store: PathBuf,
    owner: OfflineNativeCoordinator<OffHost>,
}
fn private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let root =
            crate::test_temp::directory_under(Path::new(&std::env::var_os("HOME").unwrap()), "fre")
                .unwrap();
        let runtime = root.join("runtime");
        let config = root.join("config");
        let state = root.join("state");
        let desired = DesiredPaths::below(&state);
        for dir in [&root, &runtime, &config, &state, &desired.directory] {
            fs::create_dir_all(dir).unwrap();
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = nix::unistd::Uid::current().as_raw();
        let paths = crate::cutover::CutoverPaths::below(&runtime, &state, uid);
        private(
            &paths.ownership_marker,
            br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
        );
        write_desired(&desired, uid, &DesiredState::default()).unwrap();
        let store = config.join("profiles.json");
        let mut old: serde_json::Value = serde_json::from_slice(PORTABLE).unwrap();
        old["onboardingComplete"] = true.into();
        private(&store, &serde_json::to_vec(&old).unwrap());
        private(
            &config.join("route-template.yaml"),
            b"synthetic exact old template\n",
        );
        let backup = root.join("synthetic.ovb");
        private(
            &backup,
            &omavless_domain::private_backup::seal(
                PORTABLE,
                include_bytes!("../../../../templates/default.yaml"),
                PASSWORD,
            )
            .unwrap(),
        );
        let owner = OfflineNativeCoordinator::new_ownership_gated(
            OffHost {
                idle: true,
                observations: 0,
                auxiliary: Arc::default(),
            },
            desired,
            &store,
            paths,
            uid,
            2,
        );
        Self {
            root,
            backup,
            store,
            owner,
        }
    }
    fn state(&self) -> PathBuf {
        self.owner
            .transaction
            .cutover_paths()
            .state_directory
            .clone()
    }
    fn stage(&self) -> PathBuf {
        self.state().join("restore-pair.pending")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn assert_fenced(f: &mut Fixture) {
    assert!(f.owner.transaction.independently_blocked());
    assert!(crate::pending_private_transaction::pending(
        f.owner.transaction.desired_paths()
    ));
    assert!(f.owner.restore_readiness_candidate().is_err());
    let paths = f.owner.transaction.cutover_paths().clone();
    let lock = MigrationLock::acquire_existing(&paths, f.owner.uid()).unwrap();
    assert!(
        crate::login_transaction::check_startup_receipt(&paths, f.owner.uid(), &lock, Some(2))
            .is_err()
    );
}

#[test]
fn first_restore_owner_executes_actual_pair_under_one_lease_and_retains_fence() {
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    let desired = fs::read(&f.owner.transaction.desired_paths().file).unwrap();
    let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
    let expected = opened.restore_store_off().unwrap();
    let mut stages = 0;
    let mut execution = 0;
    let result = f
        .owner
        .execute_first_restore_body(&f.backup, PASSWORD, |owner, point| {
            assert!(
                MigrationLock::acquire_existing(owner.transaction.cutover_paths(), owner.uid())
                    .is_err()
            );
            if point == Checkpoint::Staging {
                stages += 1;
            } else {
                execution += 1;
            }
            true
        });
    assert_eq!(result, Ok(FirstOutcome::CommittedStillFenced));
    assert_eq!(stages, 7);
    assert!(execution > 2);
    assert_ne!(fs::read(&f.store).unwrap(), old);
    assert_eq!(fs::read(&f.store).unwrap(), expected.as_slice());
    assert_eq!(
        fs::read(f.store.with_file_name("route-template.yaml")).unwrap(),
        opened.template()
    );
    assert_eq!(
        fs::read(&f.owner.transaction.desired_paths().file).unwrap(),
        desired
    );
    assert_eq!(fs::read(f.stage().join("old-profiles.json")).unwrap(), old);
    assert!(f.state().join("restore-decision.terminal").is_file());
    assert_fenced(&mut f);
}

#[test]
fn first_restore_owner_original_stage_swap_in_last_callback_refuses_before_intent() {
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    let member = f.stage().join("old-profiles.json");
    let mut count = 0;
    let result = f
        .owner
        .execute_first_restore_body(&f.backup, PASSWORD, |_, point| {
            assert!(point == Checkpoint::Staging);
            count += 1;
            if count == 7 {
                let before = fs::metadata(&member).unwrap().ino();
                let bytes = fs::read(&member).unwrap();
                private(&member.with_extension("swap"), &bytes);
                fs::rename(member.with_extension("swap"), &member).unwrap();
                assert_ne!(fs::metadata(&member).unwrap().ino(), before);
            }
            true
        });
    assert_eq!(count, 7);
    assert_eq!(fs::read(&f.store).unwrap(), old);
    assert!(!f.state().join("restore-decision.intent").exists());
    assert_eq!(result, Err(FirstError::StillFenced));
    assert_fenced(&mut f);
}

#[test]
fn first_restore_owner_late_execution_gates_refuse_without_automatic_rollback() {
    for fault in 0..8 {
        let mut f = Fixture::new();
        let state = f.state();
        let store = f.store.clone();
        let old = fs::read(&store).unwrap();
        let mut injected = false;
        let result = f
            .owner
            .execute_first_restore_body(&f.backup, PASSWORD, |owner, point| {
                if !injected
                    && point == Checkpoint::Execution
                    && state.join("restore-decision.intent").exists()
                    && (fault != 6 || fs::read(&store).unwrap() != old)
                {
                    injected = true;
                    match fault {
                        0 => private(
                            &state.join("ownership.json"),
                            br#"{"schemaVersion":1,"generation":4,"phase":"rust"}"#,
                        ),
                        1 => write_desired(
                            owner.transaction.desired_paths(),
                            owner.uid(),
                            &DesiredState {
                                generation: 1,
                                ..DesiredState::default()
                            },
                        )
                        .unwrap(),
                        2 | 6 => private(
                            &state.join(crate::restore_closure_model::CLOSURE_MEMBER),
                            b"late foreign fence",
                        ),
                        3 => owner.transaction.host_mut().idle = false,
                        4 => {
                            owner
                                .coordinator
                                .submit(
                                    MutationRequest::new(
                                        MutationKind::Other,
                                        Some("late"),
                                        Some(0),
                                        MutationDigest::from_semantic_bytes(b"late"),
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                        }
                        5 => {
                            owner
                                .coordinator
                                .submit(
                                    MutationRequest::new(
                                        MutationKind::Other,
                                        Some("drift"),
                                        Some(0),
                                        MutationDigest::from_semantic_bytes(b"drift"),
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                            let BeginOutcome::Started(active) =
                                owner.coordinator.begin_next().unwrap()
                            else {
                                panic!("expected mutation")
                            };
                            owner
                                .coordinator
                                .finish(active.token, MutationResult::Success)
                                .unwrap();
                        }
                        7 => owner
                            .initialize_batch_operations("different-instance")
                            .unwrap(),
                        _ => unreachable!(),
                    }
                }
                true
            });
        assert!(injected, "late boundary reached");
        assert_eq!(result, Err(FirstError::StillFenced));
        if fault == 6 {
            assert_ne!(fs::read(&store).unwrap(), old);
        } else {
            assert_eq!(fs::read(&store).unwrap(), old);
        }
        assert!(!state.join("restore-decision.terminal").exists());
        assert_fenced(&mut f);
    }
}

#[test]
fn first_restore_owner_wrong_archive_and_existing_fence_have_no_stage_effect() {
    for pending in [false, true] {
        let mut f = Fixture::new();
        if pending {
            private(
                &f.state().join(crate::restore_closure_model::CLOSURE_MEMBER),
                b"history",
            );
        }
        let old = fs::read(&f.store).unwrap();
        let result = f.owner.execute_first_restore(
            &f.backup,
            if pending {
                PASSWORD
            } else {
                b"wrong synthetic password"
            },
        );
        assert!(result.is_err());
        assert!(!f.stage().exists());
        assert_eq!(fs::read(&f.store).unwrap(), old);
    }
}

#[test]
fn first_restore_owner_every_unrelated_fence_refuses_late_actual_execution() {
    for name in [
        "restore-finalization.pending",
        crate::restore_closure_model::CLOSURE_MEMBER,
        crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
        crate::restore_disposition_ticket_model::TICKET_MEMBER,
        crate::restore_disposition_complete_model::COMPLETE_MEMBER,
        crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        "routing-preset.pending.json",
    ] {
        let mut f = Fixture::new();
        let state = f.state();
        let before = fs::read(&f.store).unwrap();
        let mut injected = false;
        let result = f
            .owner
            .execute_first_restore_body(&f.backup, PASSWORD, |_, point| {
                if !injected
                    && point == Checkpoint::Execution
                    && state.join("restore-decision.intent").exists()
                {
                    private(&state.join(name), b"unrelated retained evidence");
                    injected = true;
                }
                true
            });
        assert!(injected);
        assert_eq!(fs::read(&f.store).unwrap(), before);
        assert_eq!(
            fs::read(state.join(name)).unwrap(),
            b"unrelated retained evidence"
        );
        assert!(!state.join("restore-decision.terminal").exists());
        assert_eq!(result, Err(FirstError::StillFenced));
        assert_fenced(&mut f);
    }
}

#[test]
fn first_restore_owner_preexisting_journal_and_slots_never_create_stage() {
    for (config, name) in [
        (false, "restore-decision.intent"),
        (false, "restore-decision.terminal"),
    ]
    .into_iter()
    .chain(
        crate::restore_executor_candidate::NEW_SLOT
            .into_iter()
            .chain(crate::restore_executor_candidate::OLD_SLOT)
            .map(|name| (true, name)),
    ) {
        let mut f = Fixture::new();
        let member = if config {
            f.store.parent().unwrap().join(name)
        } else {
            f.state().join(name)
        };
        private(&member, b"preexisting evidence");
        let before = fs::read(&f.store).unwrap();
        assert!(f.owner.execute_first_restore(&f.backup, PASSWORD).is_err());
        assert!(!f.stage().exists());
        assert_eq!(fs::read(&f.store).unwrap(), before);
        assert_eq!(fs::read(member).unwrap(), b"preexisting evidence");
    }
}

#[test]
fn first_restore_owner_existing_batch_instance_is_pinned_not_only_revision() {
    for replace in [false, true] {
        let mut f = Fixture::new();
        f.owner
            .initialize_batch_operations("original-instance")
            .unwrap();
        let state = f.state();
        let mut changed = false;
        let result = f
            .owner
            .execute_first_restore_body(&f.backup, PASSWORD, |owner, point| {
                if !changed
                    && point == Checkpoint::Execution
                    && state.join("restore-decision.intent").exists()
                {
                    if replace {
                        owner.batch.as_mut().unwrap().instance = "replacement-instance".to_owned();
                    } else {
                        owner.batch = None;
                    }
                    changed = true;
                }
                true
            });
        assert!(changed);
        assert_eq!(f.owner.coordinator.revision(), 0);
        assert_eq!(result, Err(FirstError::StillFenced));
        assert_fenced(&mut f);
    }
}
