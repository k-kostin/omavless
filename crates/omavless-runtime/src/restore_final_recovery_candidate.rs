// SPDX-License-Identifier: MIT
//! Inactive one-shot C1-only resynchronization, always returning StillFenced.
//! No normal owner, portable phase authority, unlink, rewrite or activation.
use super::*;
use crate::cutover::OwnershipMarker;
use crate::desired::DesiredState;
use crate::restore_executor_candidate::successor::rotation::final_review::{
    FinalEvidence, capture_final_closure,
    handoff_retirement::last_receipt::resync_completed_successor,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClosureRecoveryResult {
    ResynchronizedStillFenced,
}

struct Session<'a, H> {
    host: H,
    desired_paths: DesiredPaths,
    paths: CutoverPaths,
    config: PathBuf,
    uid: u32,
    marker: OwnershipMarker,
    desired: DesiredState,
    lock: MigrationLock,
    boundary: Boundary,
    backup: &'a OpenedBackup,
    evidence: FinalEvidence<'a>,
}

#[allow(clippy::too_many_arguments)]
fn boundary_stable(
    paths: &CutoverPaths,
    desired_paths: &DesiredPaths,
    uid: u32,
    marker: &OwnershipMarker,
    desired: &DesiredState,
    lock: &MigrationLock,
    boundary: &Boundary,
) -> bool {
    lock.authorizes(paths, uid)
        && read_marker_existing(paths, uid).ok().as_ref() == Some(marker)
        && read_desired_snapshot(desired_paths, uid).ok().as_ref() == Some(desired)
        && check_login_receipt_without_private_fence(paths, uid, lock, Some(marker.generation()))
            .is_ok()
        && Boundary::read(paths, uid).is_ok_and(|now| boundary.same(&now))
}
fn empty(host: &mut impl LifecycleHost, desired: &DesiredState) -> bool {
    host.fresh_observation(desired).is_ok_and(|o| {
        !o.owned_core_running
            && o.owned_auxiliary_mihomo_count == 0
            && o.managed_tun_count == 0
            && !o.owned_controller_config_verified
            && !o.desired_profile_matches_owned
    })
}

impl<'a, H: LifecycleHost> Session<'a, H> {
    fn admit(
        mut host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
        backup: &'a OpenedBackup,
    ) -> Result<Self, ProductionOwnerError> {
        ProductionNativeOwner::<H>::validate_review_paths(&desired_paths, store_path, &paths, uid)?;
        let lock = MigrationLock::acquire_existing(&paths, uid).map_err(lock_error)?;
        let marker = read_marker_existing(&paths, uid).map_err(|_| REFUSE)?;
        let desired = read_desired_snapshot(&desired_paths, uid).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust || desired.connected {
            return Err(REFUSE);
        }
        let config = store_path.parent().ok_or(REFUSE)?.to_path_buf();
        let boundary = Boundary::read(&paths, uid)?;
        let stable = || {
            boundary_stable(
                &paths,
                &desired_paths,
                uid,
                &marker,
                &desired,
                &lock,
                &boundary,
            )
        };
        let evidence = capture_final_closure(
            &config,
            &paths,
            uid,
            marker.generation(),
            &lock,
            backup,
            || stable() && empty(&mut host, &desired) && stable(),
        )
        .map_err(|_| REFUSE)?;
        if evidence.phase() != FinalPhase::HandoffAbsentReceiptAbsent || !stable() {
            return Err(REFUSE);
        }
        Ok(Self {
            host,
            desired_paths,
            paths,
            config,
            uid,
            marker,
            desired,
            lock,
            boundary,
            backup,
            evidence,
        })
    }

    /// Consuming self makes this a one-shot operation, not a reusable recovery
    /// owner. Every gate rechecks the original evidence; a different valid C1
    /// cannot silently replace the admission snapshot before the first fsync.
    fn resync(self) -> Result<ClosureRecoveryResult, ProductionOwnerError> {
        let Self {
            mut host,
            desired_paths,
            paths,
            config,
            uid,
            marker,
            desired,
            lock,
            boundary,
            backup,
            evidence,
        } = self;
        let mut gate = || {
            boundary_stable(
                &paths,
                &desired_paths,
                uid,
                &marker,
                &desired,
                &lock,
                &boundary,
            ) && evidence.matches_current(&config, &paths, uid, marker.generation(), &lock)
                && empty(&mut host, &desired)
                && boundary_stable(
                    &paths,
                    &desired_paths,
                    uid,
                    &marker,
                    &desired,
                    &lock,
                    &boundary,
                )
                && evidence.matches_current(&config, &paths, uid, marker.generation(), &lock)
        };
        resync_completed_successor(
            &config,
            &paths,
            uid,
            marker.generation(),
            &lock,
            backup,
            &mut gate,
        )
        .map_err(|_| REFUSE)?;
        if !gate() {
            return Err(REFUSE);
        }
        Ok(ClosureRecoveryResult::ResynchronizedStillFenced)
    }
}

impl<H: LifecycleHost> ProductionNativeOwner<H> {
    #[allow(dead_code)]
    pub(crate) fn resync_final_restore_startup(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
        backup: &OpenedBackup,
    ) -> Result<ClosureRecoveryResult, ProductionOwnerError> {
        Session::admit(host, desired_paths, store_path, paths, uid, backup)?.resync()
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::tests::{Host, desired, host, private, ready, source_snapshot, unchanged};
    use super::*;
    use crate::restore_executor_candidate::successor::tests::second;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::{
        fs,
        os::unix::process::ExitStatusExt,
        process::{Command, Stdio},
    };

    fn admit(
        f: &Fixture,
        h: Host,
        archive: &'static OpenedBackup,
    ) -> Result<Session<'static, Host>, ProductionOwnerError> {
        Session::admit(
            h,
            desired(f),
            &f.config.join("profiles.json"),
            f.paths.clone(),
            f.uid,
            archive,
        )
    }
    #[test]
    fn one_shot_closure_resync_commit_abort_keeps_every_source_and_startup_fence() {
        for commit in [false, true] {
            let f = ready(commit, 2);
            let before = source_snapshot(&f);
            let h = host();
            let count = h.calls.clone();
            let session = admit(&f, h, second()).unwrap();
            assert_eq!(count.get(), 2);
            assert!(matches!(
                MigrationLock::acquire_existing(&f.paths, f.uid),
                Err(crate::cutover::CutoverError::Busy)
            ));
            assert_eq!(
                session.resync(),
                Ok(ClosureRecoveryResult::ResynchronizedStillFenced)
            );
            // 2 admission observations, then guarded C1/live/config/state sync,
            // plus the final wrapper gate; no normal lifecycle method is used.
            assert_eq!(count.get(), 28);
            unchanged(&f, &before);
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
    #[test]
    fn one_shot_closure_resync_refuses_unretired_fences_and_wrong_archive() {
        for commit in [false, true] {
            for phase in [0, 1] {
                let f = ready(commit, phase);
                let before = source_snapshot(&f);
                assert!(admit(&f, host(), second()).is_err());
                unchanged(&f, &before);
            }
        }
        for commit in [false, true] {
            let f = ready(commit, 2);
            let before = source_snapshot(&f);
            assert!(admit(&f, host(), backup()).is_err());
            let mut h = host();
            h.owned = true;
            assert!(admit(&f, h, second()).is_err());
            unchanged(&f, &before);
        }
    }
    #[test]
    fn one_shot_closure_resync_admission_gap_rejects_same_byte_sources_and_lock() {
        for name in [
            "restore-closure.complete",
            "profiles.json",
            "route-template.yaml",
            "ownership.json",
            "operation-lock",
        ] {
            let f = ready(true, 2);
            let session = admit(&f, host(), second()).unwrap();
            let path = match name {
                "profiles.json" | "route-template.yaml" => f.config.join(name),
                "operation-lock" => f.paths.operation_lock.clone(),
                _ => f.paths.state_directory.join(name),
            };
            let raw = fs::read(&path).unwrap();
            let replacement = path.with_extension("replacement");
            private(&replacement, &raw);
            fs::rename(replacement, &path).unwrap();
            assert_eq!(session.resync(), Err(REFUSE), "{name}");
        }
    }
    #[test]
    fn one_shot_closure_resync_late_owned_host_and_consumed_login_swap_refuse() {
        for observation in [3, 8, 12, 16, 20, 24, 28] {
            let f = ready(true, 2);
            let before = source_snapshot(&f);
            let mut h = host();
            h.owned_after_observe = Some(observation);
            assert_eq!(admit(&f, h, second()).unwrap().resync(), Err(REFUSE));
            unchanged(&f, &before);
        }
        for observation in [3, 28] {
            let f = ready(false, 2);
            let path = f.paths.runtime_base.join("omavless-login.receipt");
            private(&path,br#"{"schemaVersion":1,"epochHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ownershipGeneration":2,"phase":"consumed"}"#);
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == observation {
                    let raw = fs::read(&path).unwrap();
                    let replacement = path.with_extension("replacement");
                    private(&replacement, &raw);
                    fs::rename(replacement, &path).unwrap();
                }
            });
            assert_eq!(admit(&f, h, second()).unwrap().resync(), Err(REFUSE));
        }
    }

    #[test]
    fn one_shot_closure_resync_every_sync_boundary_rechecks_original_evidence() {
        // Observation8 follows C1 fsync,12/16 follow live-file fsync,
        // 20/24 follow config/state directory fsync,28 is wrapper return gate.
        for observation in [3, 8, 12, 16, 20, 24, 28] {
            for kind in ["closure", "next", "owner", "desired", "login", "lock"] {
                let f = ready(true, 2);
                let paths = f.paths.clone();
                let uid = f.uid;
                let mut h = host();
                h.action = Box::new(move |n| {
                    assert!(matches!(
                        MigrationLock::acquire_existing(&paths, uid),
                        Err(crate::cutover::CutoverError::Busy)
                    ));
                    if n == observation {
                        let path = match kind {
                            "closure" => paths.state_directory.join("restore-closure.complete"),
                            "next" => paths.state_directory.join("restore-closure.next"),
                            "owner" => paths.ownership_marker.clone(),
                            "desired" => paths.state_directory.join("desired.json"),
                            "login" => paths.runtime_base.join("omavless-login.receipt"),
                            _ => paths.operation_lock.clone(),
                        };
                        if path.exists() {
                            let raw = fs::read(&path).unwrap();
                            let replacement = path.with_extension("replacement");
                            private(&replacement, &raw);
                            fs::rename(replacement, path).unwrap();
                        } else {
                            private(&path, b"late synthetic evidence");
                        }
                    }
                });
                assert_eq!(
                    admit(&f, h, second()).unwrap().resync(),
                    Err(REFUSE),
                    "{observation} {kind}"
                );
            }
        }
    }

    const WORKER: &str =
        "production_owner::final_restore_review::recovery::tests::one_shot_resync_crash_worker";
    #[test]
    #[ignore = "internal synthetic one-shot resync process-death worker"]
    fn one_shot_resync_crash_worker() {
        let f = Fixture::reopen(std::env::var_os("OMAVLESS_T4_SESSION_ROOT").unwrap().into());
        let stop: usize = std::env::var("OMAVLESS_T4_SESSION_STOP")
            .unwrap()
            .parse()
            .unwrap();
        let mut h = host();
        h.action = Box::new(move |n| {
            if n == stop {
                nix::sys::signal::kill(nix::unistd::Pid::this(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
                panic!("SIGKILL returned");
            }
        });
        let _ = admit(&f, h, second()).unwrap().resync();
        panic!("worker did not reach checkpoint");
    }
    #[test]
    fn one_shot_closure_resync_actual_deaths_reenter_fresh_without_source_changes() {
        for commit in [false, true] {
            let f = ready(commit, 2);
            let before = source_snapshot(&f);
            for stop in [3, 8, 12, 16, 20, 24, 28] {
                let status = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", WORKER, "--ignored"])
                    .env("OMAVLESS_T4_SESSION_ROOT", &f.root)
                    .env("OMAVLESS_T4_SESSION_STOP", stop.to_string())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9), "stop={stop}");
                unchanged(&f, &before);
                assert_eq!(
                    admit(&f, host(), second()).unwrap().resync(),
                    Ok(ClosureRecoveryResult::ResynchronizedStillFenced)
                );
                unchanged(&f, &before);
            }
        }
    }
}
