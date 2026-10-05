// SPDX-License-Identifier: MIT
//! Inactive read-only production-boundary adapter. No owner, login freshness,
//! durability or mutation authority is returned; normal startup stays fenced.
use super::*;
use crate::cutover::{MAX_OWNERSHIP_MARKER_BYTES, OWNERSHIP_MARKER_NAME};
use crate::desired::{MAX_DESIRED_STATE_BYTES, read_desired_snapshot};
use crate::restore_cleanup_candidate::read_optional;
use crate::restore_executor_candidate::successor::rotation::final_review::{
    FinalPhase, review_final_closure,
};
use crate::restore_staging_candidate::{same_directory, same_member};
use omavless_domain::private_backup::OpenedBackup;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use zeroize::Zeroizing;

#[path = "restore_preauth_status_candidate.rs"]
mod preauth;
#[path = "restore_final_recovery_candidate.rs"]
mod recovery;

const REFUSE: ProductionOwnerError = ProductionOwnerError::ManualRecoveryRequired;
type Member = Option<(Zeroizing<Vec<u8>>, Metadata)>;
struct Boundary {
    directories: [Metadata; 2],
    members: [Member; 3],
    operation_lock: Metadata,
}

impl Boundary {
    fn read(paths: &CutoverPaths, uid: u32) -> Result<Self, ProductionOwnerError> {
        let operation_lock =
            std::fs::symlink_metadata(&paths.operation_lock).map_err(|_| REFUSE)?;
        if !operation_lock.is_file()
            || operation_lock.uid() != uid
            || operation_lock.mode() & 0o7777 != 0o600
            || operation_lock.nlink() != 1
        {
            return Err(REFUSE);
        }
        let state =
            crate::backup_source_candidate::open_private_directory(&paths.state_directory, uid)
                .map_err(|_| REFUSE)?;
        let runtime =
            crate::backup_source_candidate::open_private_directory(&paths.runtime_base, uid)
                .map_err(|_| REFUSE)?;
        Ok(Self {
            operation_lock,
            directories: [
                state.metadata().map_err(|_| REFUSE)?,
                runtime.metadata().map_err(|_| REFUSE)?,
            ],
            members: [
                read_optional(
                    &state,
                    OWNERSHIP_MARKER_NAME,
                    uid,
                    MAX_OWNERSHIP_MARKER_BYTES as usize,
                )
                .map_err(|_| REFUSE)?,
                read_optional(
                    &state,
                    "desired.json",
                    uid,
                    MAX_DESIRED_STATE_BYTES as usize,
                )
                .map_err(|_| REFUSE)?,
                read_optional(&runtime, "omavless-login.receipt", uid, 1024).map_err(|_| REFUSE)?,
            ],
        })
    }
    fn same(&self, other: &Self) -> bool {
        same_member(&self.operation_lock, &other.operation_lock)
            && self
                .directories
                .iter()
                .zip(&other.directories)
                .all(|(a, b)| same_directory(a, b))
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|(a, b)| match (a, b) {
                    (None, None) => true,
                    (Some((a, am)), Some((b, bm))) => a == b && same_member(am, bm),
                    _ => false,
                })
    }
}

impl<H: LifecycleHost> ProductionNativeOwner<H> {
    #[allow(dead_code)]
    pub(crate) fn review_final_restore_startup(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
        backup: &OpenedBackup,
    ) -> Result<FinalPhase, ProductionOwnerError> {
        Self::validate_review_paths(&desired_paths, store_path, &paths, uid)?;
        let lock_identity = std::fs::symlink_metadata(&paths.operation_lock)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        let lock = MigrationLock::acquire_existing(&paths, uid).map_err(lock_error)?;
        Self::review_final_restore_locked(
            host,
            desired_paths,
            store_path,
            paths,
            uid,
            backup,
            lock,
            lock_identity,
        )
    }

    /// Inactive lost-name diagnostic. Only an inert fixed lock may be created.
    /// The existing runtime parent must already be private; no state directory,
    /// owner, desired, login, live pair or restore evidence is repaired/retired.
    #[allow(dead_code)]
    pub(crate) fn review_lost_lease_final_restore_startup(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
        backup: &OpenedBackup,
    ) -> Result<FinalPhase, ProductionOwnerError> {
        Self::validate_review_paths(&desired_paths, store_path, &paths, uid)?;
        let marker = read_marker_existing(&paths, uid).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust
            || read_desired_snapshot(&desired_paths, uid)
                .map_err(|_| REFUSE)?
                .connected
        {
            return Err(REFUSE);
        }
        // Preliminary existence is never authority: the complete authenticated
        // reader repeats all phase/output/owner proofs under the newly held lease.
        let state =
            crate::backup_source_candidate::open_private_directory(&paths.state_directory, uid)
                .map_err(|_| REFUSE)?;
        let closure = read_optional(
            &state,
            crate::restore_closure_model::CLOSURE_MEMBER,
            uid,
            crate::restore_closure_model::RECORD_BYTES,
        )
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
        crate::restore_closure_model::ClosureRecord::decode(&closure.0).map_err(|_| REFUSE)?;
        let lock = MigrationLock::acquire_absent(&paths, uid).map_err(lock_error)?;
        let lock_identity = std::fs::symlink_metadata(&paths.operation_lock).map_err(|_| REFUSE)?;
        // No drop/reacquire window: this exact new flock spans both observations.
        Self::review_final_restore_locked(
            host,
            desired_paths,
            store_path,
            paths,
            uid,
            backup,
            lock,
            lock_identity,
        )
    }

    fn validate_review_paths(
        desired_paths: &DesiredPaths,
        store_path: &Path,
        paths: &CutoverPaths,
        uid: u32,
    ) -> Result<(), ProductionOwnerError> {
        if desired_paths.directory != paths.state_directory
            || desired_paths.file != paths.state_directory.join("desired.json")
            || paths.ownership_marker != paths.state_directory.join(OWNERSHIP_MARKER_NAME)
            || paths.operation_lock != paths.runtime_base.join(format!("omavless.{uid}.lock"))
        {
            return Err(REFUSE);
        }
        store_path
            .parent()
            .filter(|_| store_path.file_name().is_some_and(|n| n == "profiles.json"))
            .ok_or(REFUSE)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn review_final_restore_locked(
        mut host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
        backup: &OpenedBackup,
        lock: MigrationLock,
        lock_identity: Metadata,
    ) -> Result<FinalPhase, ProductionOwnerError> {
        let config = store_path.parent().ok_or(REFUSE)?;
        if !lock.authorizes(&paths, uid) {
            return Err(REFUSE);
        }
        let marker = read_marker_existing(&paths, uid)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        if marker.phase() != OwnershipPhase::Rust {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        let desired = read_desired_snapshot(&desired_paths, uid).map_err(|_| REFUSE)?;
        if desired.connected {
            return Err(REFUSE);
        }
        let boundary = Boundary::read(&paths, uid)?;
        if !same_member(&lock_identity, &boundary.operation_lock) {
            return Err(REFUSE);
        }
        let stable = || {
            read_marker_existing(&paths, uid).ok().as_ref() == Some(&marker)
                && read_desired_snapshot(&desired_paths, uid).ok().as_ref() == Some(&desired)
                && check_login_receipt_without_private_fence(
                    &paths,
                    uid,
                    &lock,
                    Some(marker.generation()),
                )
                .is_ok()
                && Boundary::read(&paths, uid).is_ok_and(|current| boundary.same(&current))
        };
        // The reader's exact two-pass snapshot spans the second observation;
        // comparing only its returned phase would miss same-byte replacement.
        let phase = review_final_closure(
            config,
            &paths,
            uid,
            marker.generation(),
            &lock,
            backup,
            || {
                stable()
                    && host.fresh_observation(&desired).is_ok_and(|o| {
                        !o.owned_core_running
                            && o.owned_auxiliary_mihomo_count == 0
                            && o.managed_tun_count == 0
                            && !o.owned_controller_config_verified
                            && !o.desired_profile_matches_owned
                    })
                    && stable()
            },
        )
        .map_err(|_| REFUSE)?;
        if !stable() {
            return Err(REFUSE);
        }
        Ok(phase)
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
pub(super) mod tests {
    use super::*;
    use crate::desired::{DesiredState, OwnedObservation};
    use crate::lifecycle::{HostStepError, NativeLocalObservation};
    use crate::restore_executor_candidate::successor::{rotation::final_review, tests::second};
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::{
        cell::Cell,
        fs,
        os::unix::fs::{PermissionsExt, symlink},
        rc::Rc,
    };

    pub(super) struct Host {
        pub(super) calls: Rc<Cell<usize>>,
        pub(super) action: Box<dyn FnMut(usize)>,
        pub(super) owned: bool,
        pub(super) owned_after_observe: Option<usize>,
    }
    impl LifecycleHost for Host {
        fn fresh_observation(
            &mut self,
            _: &DesiredState,
        ) -> Result<NativeLocalObservation, HostStepError> {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            (self.action)(n);
            Ok(NativeLocalObservation {
                owned_core_running: self.owned
                    || self.owned_after_observe.is_some_and(|at| n >= at),
                visible_mihomo_count: 3,
                owned_auxiliary_mihomo_count: 0,
                visible_tun_count: 2,
                managed_tun_count: 0,
                owned_controller_config_verified: false,
                desired_profile_matches_owned: false,
            })
        }
        fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            panic!("no reconcile");
        }
        fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
            panic!("no effect");
        }
        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no effect");
        }
        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no effect");
        }
        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            panic!("no effect");
        }
        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no effect");
        }
    }
    pub(super) fn host() -> Host {
        Host {
            calls: Rc::new(Cell::new(0)),
            action: Box::new(|_| {}),
            owned: false,
            owned_after_observe: None,
        }
    }
    pub(super) fn desired(f: &Fixture) -> DesiredPaths {
        DesiredPaths {
            directory: f.paths.state_directory.clone(),
            file: f.paths.state_directory.join("desired.json"),
        }
    }
    fn review(
        f: &Fixture,
        host: Host,
        archive: &OpenedBackup,
    ) -> Result<FinalPhase, ProductionOwnerError> {
        ProductionNativeOwner::review_final_restore_startup(
            host,
            desired(f),
            &f.config.join("profiles.json"),
            f.paths.clone(),
            f.uid,
            archive,
        )
    }
    pub(super) fn ready(commit: bool, phase: usize) -> Fixture {
        let (f, lock) = final_review::tests::ready(commit);
        if phase > 0 {
            final_review::handoff_retirement::retire_successor_handoff(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                || true,
            )
            .unwrap();
        }
        if phase > 1 {
            final_review::handoff_retirement::last_receipt::retire_last_successor_receipt(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                || true,
            )
            .unwrap();
        }
        drop(lock);
        f
    }
    pub(super) fn private(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn lost_review(
        f: &Fixture,
        host: Host,
        archive: &OpenedBackup,
    ) -> Result<FinalPhase, ProductionOwnerError> {
        ProductionNativeOwner::review_lost_lease_final_restore_startup(
            host,
            desired(f),
            &f.config.join("profiles.json"),
            f.paths.clone(),
            f.uid,
            archive,
        )
    }

    pub(super) fn source_snapshot(f: &Fixture) -> Vec<(std::path::PathBuf, Vec<u8>, Metadata)> {
        let mut values = Vec::new();
        for dir in [&f.paths.state_directory, &f.config] {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                values.push((
                    path.clone(),
                    fs::read(&path).unwrap(),
                    fs::symlink_metadata(path).unwrap(),
                ));
            }
        }
        values.sort_by(|a, b| a.0.cmp(&b.0));
        values
    }
    pub(super) fn unchanged(f: &Fixture, before: &[(std::path::PathBuf, Vec<u8>, Metadata)]) {
        let now = source_snapshot(f);
        assert_eq!(before.len(), now.len());
        for (a, b) in before.iter().zip(&now) {
            assert_eq!(a.0, b.0);
            assert_eq!(a.1, b.1);
            assert!(same_member(&a.2, &b.2));
        }
    }

    #[test]
    fn lost_lease_review_creates_only_lock_and_keeps_every_phase_fenced() {
        for commit in [false, true] {
            for (phase, expected) in [
                FinalPhase::BeforeHandoffRetirement,
                FinalPhase::HandoffAbsentReceiptPresent,
                FinalPhase::HandoffAbsentReceiptAbsent,
            ]
            .into_iter()
            .enumerate()
            {
                let f = ready(commit, phase);
                let before = source_snapshot(&f);
                fs::remove_file(&f.paths.operation_lock).unwrap();
                let h = host();
                let calls = h.calls.clone();
                assert_eq!(lost_review(&f, h, second()), Ok(expected));
                assert_eq!(calls.get(), 2);
                assert_eq!(
                    fs::metadata(&f.paths.operation_lock).unwrap().mode() & 0o7777,
                    0o600
                );
                unchanged(&f, &before);
                assert!(lost_review(&f, host(), second()).is_err());
                assert_eq!(review(&f, host(), second()), Ok(expected));
                assert!(matches!(
                    ProductionNativeOwner::initialize(
                        host(),
                        desired(&f),
                        &f.config.join("profiles.json"),
                        f.paths.clone(),
                        f.uid
                    ),
                    Err(ProductionOwnerError::ManualRecoveryRequired)
                ));
            }
        }
    }

    #[test]
    fn lost_lease_review_requires_existing_parent_and_stales_old_held_inode() {
        let f = ready(false, 2);
        let old = MigrationLock::acquire_existing(&f.paths, f.uid).unwrap();
        let before = source_snapshot(&f);
        let saved = f.root.join("runtime-saved");
        fs::rename(&f.paths.runtime_base, &saved).unwrap();
        assert!(lost_review(&f, host(), second()).is_err());
        assert!(!f.paths.runtime_base.exists());
        // Fixture-only simulation of a user manager recreating volatile space;
        // no reboot/epoch or old-process-death proof is inferred from this.
        fs::create_dir(&f.paths.runtime_base).unwrap();
        fs::set_permissions(&f.paths.runtime_base, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            lost_review(&f, host(), second()),
            Ok(FinalPhase::HandoffAbsentReceiptAbsent)
        );
        assert!(!old.authorizes(&f.paths, f.uid));
        unchanged(&f, &before);
    }

    #[test]
    fn lost_lease_review_failure_retains_inert_lock_without_repair_or_authority() {
        for wrong_archive in [false, true] {
            let f = ready(false, 2);
            fs::remove_file(&f.paths.operation_lock).unwrap();
            let before = source_snapshot(&f);
            let mut h = host();
            h.owned = !wrong_archive;
            assert_eq!(
                lost_review(&f, h, if wrong_archive { backup() } else { second() }),
                Err(REFUSE)
            );
            assert!(f.paths.operation_lock.is_file());
            unchanged(&f, &before);
            assert!(lost_review(&f, host(), second()).is_err());
            assert_eq!(
                review(&f, host(), second()),
                Ok(FinalPhase::HandoffAbsentReceiptAbsent)
            );
        }
        let f = ready(true, 2);
        let before = source_snapshot(&f);
        fs::set_permissions(&f.paths.operation_lock, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(lost_review(&f, host(), second()).is_err());
        assert_eq!(
            fs::metadata(&f.paths.operation_lock).unwrap().mode() & 0o7777,
            0o644
        );
        unchanged(&f, &before);
    }

    #[test]
    fn lost_lease_review_late_substitution_refuses_under_same_new_flock() {
        let f = ready(true, 2);
        fs::remove_file(&f.paths.operation_lock).unwrap();
        let paths = f.paths.clone();
        let uid = f.uid;
        let mut h = host();
        h.action = Box::new(move |n| {
            assert!(matches!(
                MigrationLock::acquire_existing(&paths, uid),
                Err(crate::cutover::CutoverError::Busy)
            ));
            if n == 2 {
                let c = paths
                    .state_directory
                    .join(crate::restore_closure_model::CLOSURE_MEMBER);
                let bytes = fs::read(&c).unwrap();
                let replacement = c.with_extension("replacement");
                private(&replacement, &bytes);
                fs::rename(replacement, c).unwrap();
            }
        });
        assert_eq!(lost_review(&f, h, second()), Err(REFUSE));
    }

    const LOST_WORKER: &str =
        "production_owner::final_restore_review::tests::lost_lease_crash_worker";
    #[test]
    #[ignore = "internal synthetic lost-lease process-death worker"]
    fn lost_lease_crash_worker() {
        let f = Fixture::reopen(
            std::env::var_os("OMAVLESS_T4_LOST_LEASE_ROOT")
                .unwrap()
                .into(),
        );
        let mut h = host();
        h.action = Box::new(|n| {
            if n == 1 {
                nix::sys::signal::kill(nix::unistd::Pid::this(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
                panic!("SIGKILL returned");
            }
        });
        let _ = lost_review(&f, h, second());
        panic!("worker did not reach post-create observation");
    }
    #[test]
    fn lost_lease_review_process_death_reenters_existing_only_without_source_changes() {
        use std::os::unix::process::ExitStatusExt;
        for commit in [false, true] {
            let f = ready(commit, 2);
            fs::remove_file(&f.paths.operation_lock).unwrap();
            let before = source_snapshot(&f);
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", LOST_WORKER, "--ignored"])
                .env("OMAVLESS_T4_LOST_LEASE_ROOT", &f.root)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            assert!(f.paths.operation_lock.is_file());
            unchanged(&f, &before);
            assert!(lost_review(&f, host(), second()).is_err());
            assert_eq!(
                review(&f, host(), second()),
                Ok(FinalPhase::HandoffAbsentReceiptAbsent)
            );
            unchanged(&f, &before);
        }
    }
    #[test]
    fn final_startup_review_is_read_only_diagnostic_for_all_phases() {
        for commit in [false, true] {
            for (phase, expected) in [
                FinalPhase::BeforeHandoffRetirement,
                FinalPhase::HandoffAbsentReceiptPresent,
                FinalPhase::HandoffAbsentReceiptAbsent,
            ]
            .into_iter()
            .enumerate()
            {
                let f = ready(commit, phase);
                for receipt in [false, true] {
                    if receipt {
                        private(&f.paths.runtime_base.join("omavless-login.receipt"), br#"{"schemaVersion":1,"epochHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ownershipGeneration":2,"phase":"consumed"}"#);
                    }
                    let h = host();
                    let count = h.calls.clone();
                    let before = Boundary::read(&f.paths, f.uid).unwrap();
                    assert_eq!(review(&f, h, second()), Ok(expected));
                    assert_eq!(count.get(), 2);
                    assert!(before.same(&Boundary::read(&f.paths, f.uid).unwrap()));
                    assert!(matches!(
                        ProductionNativeOwner::initialize(
                            host(),
                            desired(&f),
                            &f.config.join("profiles.json"),
                            f.paths.clone(),
                            f.uid
                        ),
                        Err(ProductionOwnerError::ManualRecoveryRequired)
                    ));
                }
            }
        }
    }
    #[test]
    fn final_startup_review_refuses_late_same_byte_member_swaps() {
        for name in [
            "ownership.json",
            "restore-closure.complete",
            "restore-finalization.pending",
            "restore-successor.pending",
            "omavless-login.receipt",
            "profiles.json",
            "route-template.yaml",
            "operation-lock",
        ] {
            let f = ready(true, 0);
            private(&f.paths.runtime_base.join("omavless-login.receipt"), br#"{"schemaVersion":1,"epochHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ownershipGeneration":2,"phase":"consumed"}"#);
            let path = if name == "operation-lock" {
                f.paths.operation_lock.clone()
            } else if name == "omavless-login.receipt" {
                f.paths.runtime_base.join(name)
            } else if name == "profiles.json" || name == "route-template.yaml" {
                f.config.join(name)
            } else {
                f.paths.state_directory.join(name)
            };
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == 2 {
                    let bytes = fs::read(&path).unwrap();
                    let replacement = path.with_extension("replacement");
                    private(&replacement, &bytes);
                    fs::rename(replacement, &path).unwrap();
                }
            });
            assert_eq!(review(&f, h, second()), Err(REFUSE), "{name}");
        }
    }
    #[test]
    fn final_startup_review_refuses_missing_lock_archive_owned_and_foreign_paths() {
        let f = ready(false, 2);
        assert_eq!(review(&f, host(), backup()), Err(REFUSE));
        let mut h = host();
        h.owned = true;
        assert_eq!(review(&f, h, second()), Err(REFUSE));
        let mut wrong = desired(&f);
        wrong.file = f.root.join("foreign-desired");
        assert_eq!(
            ProductionNativeOwner::review_final_restore_startup(
                host(),
                wrong,
                &f.config.join("profiles.json"),
                f.paths.clone(),
                f.uid,
                second()
            ),
            Err(REFUSE)
        );
        fs::remove_file(&f.paths.operation_lock).unwrap();
        assert_eq!(
            review(&f, host(), second()),
            Err(ProductionOwnerError::OwnershipUnavailable)
        );
        assert!(!f.paths.operation_lock.exists());
    }
    #[test]
    fn final_startup_review_refuses_late_fences_and_desired_or_login_changes() {
        for name in [
            "restore-closure.next",
            "restore-pair.pending",
            "restore-decision.intent",
            "routing-preset.pending.json",
            "desired.json",
            "omavless-login.receipt",
        ] {
            let f = ready(true, 2);
            let path = if name == "omavless-login.receipt" {
                f.paths.runtime_base.join(name)
            } else {
                f.paths.state_directory.join(name)
            };
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == 2 {
                    symlink("missing-synthetic", &path).unwrap();
                }
            });
            assert_eq!(review(&f, h, second()), Err(REFUSE), "{name}");
        }
    }

    #[test]
    fn final_startup_boundary_pins_desired_and_runtime_state_directories() {
        let f = ready(true, 2);
        let desired_path = f.paths.state_directory.join("desired.json");
        let bytes = serde_json::to_vec(&DesiredState::default()).unwrap();
        private(&desired_path, &bytes);
        let before = Boundary::read(&f.paths, f.uid).unwrap();
        let replacement = desired_path.with_extension("replacement");
        private(&replacement, &bytes);
        fs::rename(replacement, &desired_path).unwrap();
        assert!(!before.same(&Boundary::read(&f.paths, f.uid).unwrap()));
        // Connected desired is rejected before any host observation, even if
        // the archive/terminal proof would independently refuse these bytes.
        let connected = DesiredState {
            connected: true,
            profile_id: "synthetic".into(),
            ..DesiredState::default()
        };
        private(&desired_path, &serde_json::to_vec(&connected).unwrap());
        let h = host();
        let calls = h.calls.clone();
        assert_eq!(review(&f, h, second()), Err(REFUSE));
        assert_eq!(calls.get(), 0);
        for runtime in [false, true] {
            let f = ready(true, 2);
            let path = if runtime {
                f.paths.runtime_base.clone()
            } else {
                f.paths.state_directory.clone()
            };
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == 2 {
                    let saved = path.with_extension("saved");
                    fs::rename(&path, &saved).unwrap();
                    fs::create_dir(&path).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                    for entry in fs::read_dir(&saved).unwrap() {
                        let entry = entry.unwrap();
                        fs::rename(entry.path(), path.join(entry.file_name())).unwrap();
                    }
                }
            });
            assert_eq!(review(&f, h, second()), Err(REFUSE));
        }
    }
}
