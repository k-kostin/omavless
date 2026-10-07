// SPDX-License-Identifier: MIT
//! Inactive private one-shot ticket recovery boundary. No owner is constructed.
use super::*;
use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::resync_disposition;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TicketRecoveryResult {
    ResynchronizedStillFenced,
}

// This non-Copy session never escapes the operation. The low-level resync pins
// ticket/C1/live/config before its first callback and keeps the original evidence
// across all effects; the session adds semantic owner/Off/login/host checks.
struct TicketSession<'a, H> {
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
}

impl<'a, H: LifecycleHost> TicketSession<'a, H> {
    fn resync(self) -> Result<TicketRecoveryResult, ProductionOwnerError> {
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
        } = self;
        // No observation after the inner evidence is dropped: its final gate
        // checks sources and ticket again after this callback before returning.
        resync_disposition(
            &config,
            &paths,
            uid,
            marker.generation(),
            &lock,
            backup,
            || {
                boundary_stable(
                    &paths,
                    &desired_paths,
                    uid,
                    &marker,
                    &desired,
                    &lock,
                    &boundary,
                ) && empty(&mut host, &desired)
                    && boundary_stable(
                        &paths,
                        &desired_paths,
                        uid,
                        &marker,
                        &desired,
                        &lock,
                        &boundary,
                    )
            },
        )
        .map_err(|_| REFUSE)?;
        Ok(TicketRecoveryResult::ResynchronizedStillFenced)
    }
}

impl<H: LifecycleHost> ProductionNativeOwner<H> {
    /// Private candidate only. Future caller must supply trusted product-derived
    /// paths and freshly authenticated payload; path shape validation and a
    /// borrowed OpenedBackup prove neither provenance nor authentication time.
    /// Consumed login generation is not proof of the current boot/login epoch.
    #[allow(dead_code)]
    pub(crate) fn resync_ticket_restore_startup(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
        backup: &OpenedBackup,
    ) -> Result<TicketRecoveryResult, ProductionOwnerError> {
        Self::validate_review_paths(&desired_paths, store_path, &paths, uid)?;
        let lock = MigrationLock::acquire_existing(&paths, uid).map_err(lock_error)?;
        let marker = read_marker_existing(&paths, uid).map_err(|_| REFUSE)?;
        let desired = read_desired_snapshot(&desired_paths, uid).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust || desired.connected {
            return Err(REFUSE);
        }
        let boundary = Boundary::read(&paths, uid)?;
        if !boundary_stable(
            &paths,
            &desired_paths,
            uid,
            &marker,
            &desired,
            &lock,
            &boundary,
        ) {
            return Err(REFUSE);
        }
        TicketSession {
            host,
            desired_paths,
            config: store_path.parent().ok_or(REFUSE)?.to_path_buf(),
            paths,
            uid,
            marker,
            desired,
            lock,
            boundary,
            backup,
        }
        .resync()
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::super::tests::{
        Host, desired, host, private, ready, source_snapshot, unchanged,
    };
    use super::*;
    use crate::desired::OwnedObservation;
    use crate::lifecycle::{HostStepError, NativeLocalObservation};
    use crate::restore_disposition_ticket_model::TICKET_MEMBER;
    use crate::restore_executor_candidate::successor::rotation::final_review::disposition::publish_disposition;
    use crate::restore_executor_candidate::successor::tests::second;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::{
        fs,
        os::unix::process::ExitStatusExt,
        process::{Command, Stdio},
    };

    fn published(commit: bool) -> Fixture {
        let f = ready(commit, 2);
        let lock = MigrationLock::acquire_existing(&f.paths, f.uid).unwrap();
        publish_disposition(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        drop(lock);
        f
    }
    fn recover(
        f: &Fixture,
        h: Host,
        archive: &OpenedBackup,
    ) -> Result<TicketRecoveryResult, ProductionOwnerError> {
        ProductionNativeOwner::resync_ticket_restore_startup(
            h,
            desired(f),
            &f.config.join("profiles.json"),
            f.paths.clone(),
            f.uid,
            archive,
        )
    }
    fn replace(path: &Path) {
        let raw = fs::read(path).unwrap();
        let replacement = path.with_extension("replacement");
        private(&replacement, &raw);
        fs::rename(replacement, path).unwrap();
    }
    struct NonEmptyHost(Host, usize);
    impl LifecycleHost for NonEmptyHost {
        fn fresh_observation(
            &mut self,
            desired: &DesiredState,
        ) -> Result<NativeLocalObservation, HostStepError> {
            let mut observation = self.0.fresh_observation(desired)?;
            match self.1 {
                0 => observation.owned_auxiliary_mihomo_count = 1,
                1 => observation.managed_tun_count = 1,
                2 => observation.owned_controller_config_verified = true,
                _ => observation.desired_profile_matches_owned = true,
            }
            Ok(observation)
        }
        fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            panic!("no reconcile")
        }
        fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
            panic!("no effect")
        }
        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no effect")
        }
        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no effect")
        }
        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            panic!("no effect")
        }
        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            panic!("no effect")
        }
    }
    #[test]
    fn ticket_boundary_complete_owned_host_observation_is_required() {
        let f = published(true);
        let before = source_snapshot(&f);
        for kind in 0..4 {
            assert_eq!(
                ProductionNativeOwner::resync_ticket_restore_startup(
                    NonEmptyHost(host(), kind),
                    desired(&f),
                    &f.config.join("profiles.json"),
                    f.paths.clone(),
                    f.uid,
                    second(),
                ),
                Err(REFUSE)
            );
            unchanged(&f, &before);
        }
    }
    #[test]
    fn ticket_boundary_commit_abort_keeps_sources_and_continuous_lease() {
        for commit in [false, true] {
            let f = published(commit);
            let before = source_snapshot(&f);
            let paths = f.paths.clone();
            let uid = f.uid;
            let mut h = host();
            let calls = h.calls.clone();
            h.action = Box::new(move |_| {
                assert!(matches!(
                    MigrationLock::acquire_existing(&paths, uid),
                    Err(crate::cutover::CutoverError::Busy)
                ))
            });
            assert_eq!(
                recover(&f, h, second()),
                Ok(TicketRecoveryResult::ResynchronizedStillFenced)
            );
            assert!(calls.get() > 10);
            unchanged(&f, &before);
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
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
    fn ticket_boundary_wrong_archive_missing_lock_and_unsafe_semantics_refuse() {
        for commit in [false, true] {
            let f = published(commit);
            let before = source_snapshot(&f);
            assert_eq!(recover(&f, host(), backup()), Err(REFUSE));
            let mut h = host();
            h.owned = true;
            assert_eq!(recover(&f, h, second()), Err(REFUSE));
            unchanged(&f, &before);
            fs::remove_file(&f.paths.operation_lock).unwrap();
            assert!(recover(&f, host(), second()).is_err());
            assert!(!f.paths.operation_lock.exists());
        }
        for kind in ["desired", "login", "generation", "uid"] {
            let f = published(true);
            match kind {
                "desired" => {
                    let path = f.paths.state_directory.join("desired.json");
                    let value = DesiredState {
                        connected: true,
                        ..DesiredState::default()
                    };
                    private(&path, &serde_json::to_vec(&value).unwrap());
                }
                "login" => private(
                    &f.paths.runtime_base.join("omavless-login.receipt"),
                    b"invalid synthetic receipt",
                ),
                "generation" => {
                    let mut value: serde_json::Value =
                        serde_json::from_slice(&fs::read(&f.paths.ownership_marker).unwrap())
                            .unwrap();
                    value["generation"] = 3.into();
                    private(
                        &f.paths.ownership_marker,
                        &serde_json::to_vec(&value).unwrap(),
                    );
                }
                _ => (),
            }
            if kind == "uid" {
                assert!(
                    ProductionNativeOwner::resync_ticket_restore_startup(
                        host(),
                        desired(&f),
                        &f.config.join("profiles.json"),
                        f.paths.clone(),
                        f.uid + 1,
                        second()
                    )
                    .is_err()
                );
            } else {
                assert!(recover(&f, host(), second()).is_err(), "{kind}");
            }
        }
    }
    #[test]
    fn ticket_boundary_first_and_every_host_callback_preserve_original_sources() {
        let f = published(true);
        let h = host();
        let count = h.calls.clone();
        recover(&f, h, second()).unwrap();
        let total = count.get();
        // Every callback may observe late owned-host state or a foreign fence;
        // these are read-only observations, never lifecycle effects.
        for stop in 1..=total {
            for kind in ["host", "next", "lock"] {
                let f = published(true);
                let paths = f.paths.clone();
                let mut h = host();
                if kind == "host" {
                    h.owned_after_observe = Some(stop);
                } else {
                    h.action = Box::new(move |n| {
                        if n == stop {
                            if kind == "lock" {
                                replace(&paths.operation_lock);
                            } else {
                                private(
                                    &paths.state_directory.join("restore-closure.next"),
                                    b"late synthetic fence",
                                );
                            }
                        }
                    });
                }
                assert_eq!(recover(&f, h, second()), Err(REFUSE), "{stop} {kind}");
            }
        }
        for stop in [1, 2, total] {
            for kind in [
                "ticket", "closure", "live", "template", "owner", "desired", "login", "runtime",
                "config",
            ] {
                let f = published(false);
                let paths = f.paths.clone();
                let config = f.config.clone();
                let login = paths.runtime_base.join("omavless-login.receipt");
                private(&login, br#"{"schemaVersion":1,"epochHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ownershipGeneration":2,"phase":"consumed"}"#);
                let mut h = host();
                h.action = Box::new(move |n| {
                    if n == stop {
                        if kind == "runtime" || kind == "config" {
                            let path = if kind == "runtime" {
                                &paths.runtime_base
                            } else {
                                &config
                            };
                            let displaced = path.with_extension("displaced");
                            fs::rename(path, &displaced).unwrap();
                            fs::create_dir(path).unwrap();
                            use std::os::unix::fs::PermissionsExt;
                            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
                            for entry in fs::read_dir(&displaced).unwrap() {
                                let entry = entry.unwrap();
                                fs::rename(entry.path(), path.join(entry.file_name())).unwrap();
                            }
                        } else {
                            let path = match kind {
                                "ticket" => paths.state_directory.join(TICKET_MEMBER),
                                "closure" => paths.state_directory.join("restore-closure.complete"),
                                "live" => config.join("profiles.json"),
                                "template" => config.join("route-template.yaml"),
                                "owner" => paths.ownership_marker.clone(),
                                "desired" => paths.state_directory.join("desired.json"),
                                _ => login.clone(),
                            };
                            if kind == "desired" && !path.exists() {
                                // This fixture records absent desired state. Its
                                // appearance must poison the original absence too.
                                private(
                                    &path,
                                    &serde_json::to_vec(&DesiredState::default()).unwrap(),
                                );
                            } else {
                                replace(&path);
                            }
                        }
                    }
                });
                assert_eq!(recover(&f, h, second()), Err(REFUSE), "{stop} {kind}");
            }
        }
    }
    const WORKER: &str = "production_owner::final_restore_review::recovery::ticket::tests::ticket_boundary_crash_worker";
    #[test]
    #[ignore = "internal synthetic ticket boundary process-death worker"]
    fn ticket_boundary_crash_worker() {
        let f = Fixture::reopen(
            std::env::var_os("OMAVLESS_SYNTHETIC_TICKET_BOUNDARY_ROOT")
                .unwrap()
                .into(),
        );
        let stop: usize = std::env::var("OMAVLESS_SYNTHETIC_TICKET_BOUNDARY_STOP")
            .unwrap()
            .parse()
            .unwrap();
        let mut h = host();
        h.action = Box::new(move |n| {
            if n == stop {
                nix::sys::signal::kill(nix::unistd::Pid::this(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
        });
        let _ = recover(&f, h, second());
        panic!("worker did not reach checkpoint");
    }
    #[test]
    fn ticket_boundary_actual_deaths_reenter_without_unfencing() {
        for commit in [false, true] {
            let f = published(commit);
            let before = source_snapshot(&f);
            let h = host();
            let count = h.calls.clone();
            recover(&f, h, second()).unwrap();
            for stop in [1, 3, count.get() / 2, count.get()] {
                let status = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", WORKER, "--ignored"])
                    .env("OMAVLESS_SYNTHETIC_TICKET_BOUNDARY_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_TICKET_BOUNDARY_STOP", stop.to_string())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap();
                assert_eq!(status.signal(), Some(9), "{stop}");
                unchanged(&f, &before);
                assert_eq!(
                    recover(&f, host(), second()),
                    Ok(TicketRecoveryResult::ResynchronizedStillFenced)
                );
                unchanged(&f, &before);
            }
        }
    }
}
