// SPDX-License-Identifier: MIT
//! Inactive pre-authentication diagnostic. A syntactically valid closure is
//! NOT proof of Commit/Abort, archive compatibility, completion or readiness.
//! No sync, writer, secret input, IPC caller or normal owner is introduced.
use super::*;
use crate::backup_source_candidate::open_private_directory;
use crate::restore_closure_model::{
    CLOSURE_MEMBER, ClosureRecord, NEXT_CLOSURE_MEMBER, RECORD_BYTES,
};
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_retirement_candidate::RECEIPT_MEMBER;
use crate::restore_staging_candidate::PENDING_DIRECTORY;
use crate::restore_successor_handoff_model::SUCCESSOR_MEMBER;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use std::fs::File;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreauthClosureStatus {
    NeedsAuthenticatedArchiveStillFenced,
}

const STATE_TRANSIENTS: [&str; 8] = [
    crate::restore_disposition_ticket_model::TICKET_MEMBER,
    NEXT_CLOSURE_MEMBER,
    SUCCESSOR_MEMBER,
    RECEIPT_MEMBER,
    PENDING_DIRECTORY,
    "restore-decision.intent",
    "restore-decision.terminal",
    "routing-preset.pending.json",
];

fn absent(directory: &File, name: &str) -> Result<(), ProductionOwnerError> {
    match openat(
        directory,
        Path::new(name),
        OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Err(nix::errno::Errno::ENOENT) => Ok(()),
        _ => Err(REFUSE),
    }
}

fn no_transients(state: &File, config: &File) -> Result<(), ProductionOwnerError> {
    for name in STATE_TRANSIENTS {
        absent(state, name)?;
    }
    for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
        absent(config, name)?;
    }
    Ok(())
}

struct Snapshot {
    directories: [Metadata; 2],
    members: [(Zeroizing<Vec<u8>>, Metadata); 3],
}

impl Snapshot {
    fn read(
        config: &Path,
        paths: &CutoverPaths,
        uid: u32,
        generation: u64,
        boundary: &Boundary,
    ) -> Result<Self, ProductionOwnerError> {
        let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
        let config = open_private_directory(config, uid).map_err(|_| REFUSE)?;
        no_transients(&state, &config)?;
        let closure = read_optional(&state, CLOSURE_MEMBER, uid, RECORD_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let record = ClosureRecord::decode(&closure.0).map_err(|_| REFUSE)?;
        if !record.receipt().terminal().matches_owner_desired(
            generation,
            boundary.members[1]
                .as_ref()
                .map(|(bytes, _)| bytes.as_slice()),
        ) {
            return Err(REFUSE);
        }
        // Only bounded private identity snapshots, NOT archive/output matching.
        let store = read_optional(&config, "profiles.json", uid, MAX_PRIVATE_STORE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let template = read_optional(&config, "route-template.yaml", uid, MAX_TEMPLATE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        no_transients(&state, &config)?;
        Ok(Self {
            directories: [
                state.metadata().map_err(|_| REFUSE)?,
                config.metadata().map_err(|_| REFUSE)?,
            ],
            members: [closure, store, template],
        })
    }
    fn same(&self, other: &Self) -> bool {
        self.directories
            .iter()
            .zip(&other.directories)
            .all(|(a, b)| same_directory(a, b))
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|((a, am), (b, bm))| a == b && same_member(am, bm))
    }
}

impl<H: LifecycleHost> ProductionNativeOwner<H> {
    #[allow(dead_code)]
    pub(crate) fn review_preauth_closure_status(
        mut host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        paths: CutoverPaths,
        uid: u32,
    ) -> Result<PreauthClosureStatus, ProductionOwnerError> {
        Self::validate_review_paths(&desired_paths, store_path, &paths, uid)?;
        let lock = MigrationLock::acquire_existing(&paths, uid).map_err(lock_error)?;
        let marker = read_marker_existing(&paths, uid).map_err(|_| REFUSE)?;
        let desired = read_desired_snapshot(&desired_paths, uid).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust || desired.connected {
            return Err(REFUSE);
        }
        let boundary = Boundary::read(&paths, uid)?;
        let stable = || {
            lock.authorizes(&paths, uid)
                && read_marker_existing(&paths, uid).ok().as_ref() == Some(&marker)
                && read_desired_snapshot(&desired_paths, uid).ok().as_ref() == Some(&desired)
                && check_login_receipt_without_private_fence(
                    &paths,
                    uid,
                    &lock,
                    Some(marker.generation()),
                )
                .is_ok()
                && Boundary::read(&paths, uid).is_ok_and(|now| boundary.same(&now))
        };
        let mut gate = || {
            stable()
                && host.fresh_observation(&desired).is_ok_and(|o| {
                    !o.owned_core_running
                        && o.owned_auxiliary_mihomo_count == 0
                        && o.managed_tun_count == 0
                        && !o.owned_controller_config_verified
                        && !o.desired_profile_matches_owned
                })
                && stable()
        };
        if !gate() {
            return Err(REFUSE);
        }
        let config = store_path.parent().ok_or(REFUSE)?;
        let before = Snapshot::read(config, &paths, uid, marker.generation(), &boundary)?;
        if !gate() {
            return Err(REFUSE);
        }
        let after = Snapshot::read(config, &paths, uid, marker.generation(), &boundary)?;
        if !before.same(&after) || !stable() {
            return Err(REFUSE);
        }
        Ok(PreauthClosureStatus::NeedsAuthenticatedArchiveStillFenced)
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::tests::{Host, desired, host, private, ready, source_snapshot, unchanged};
    use super::*;
    use crate::restore_successor_publication_candidate::tests::Fixture;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn review(f: &Fixture, h: Host) -> Result<PreauthClosureStatus, ProductionOwnerError> {
        ProductionNativeOwner::review_preauth_closure_status(
            h,
            desired(f),
            &f.config.join("profiles.json"),
            f.paths.clone(),
            f.uid,
        )
    }
    #[test]
    fn preauth_status_commit_abort_is_only_archive_required_and_read_only() {
        for commit in [false, true] {
            let f = ready(commit, 2);
            let before = source_snapshot(&f);
            let h = host();
            let calls = h.calls.clone();
            assert_eq!(
                review(&f, h),
                Ok(PreauthClosureStatus::NeedsAuthenticatedArchiveStillFenced)
            );
            assert_eq!(calls.get(), 2);
            unchanged(&f, &before);
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
            // Pre-authentication deliberately does not equate C1 with live or
            // authenticate an archive: even changed bytes give no phase proof.
            private(
                &f.config.join("profiles.json"),
                b"synthetic unauthenticated output",
            );
            assert_eq!(
                review(&f, host()),
                Ok(PreauthClosureStatus::NeedsAuthenticatedArchiveStillFenced)
            );
        }
    }
    #[test]
    fn preauth_status_missing_lock_or_closure_and_unsafe_closure_never_repairs() {
        for variant in 0..6 {
            let f = ready(true, 2);
            let c = f.paths.state_directory.join(CLOSURE_MEMBER);
            match variant {
                0 => fs::remove_file(&f.paths.operation_lock).unwrap(),
                1 => fs::remove_file(&c).unwrap(),
                2 => private(&c, b"torn"),
                3 => {
                    fs::remove_file(&c).unwrap();
                    symlink("missing-synthetic", &c).unwrap();
                }
                4 => fs::set_permissions(&c, fs::Permissions::from_mode(0o644)).unwrap(),
                _ => fs::hard_link(&c, f.paths.state_directory.join("synthetic-alias")).unwrap(),
            }
            assert!(review(&f, host()).is_err());
            if variant == 0 {
                assert!(!f.paths.operation_lock.exists());
            }
        }
    }
    #[test]
    fn preauth_status_all_transients_initial_and_late_refuse() {
        for late in [false, true] {
            for (state, name) in STATE_TRANSIENTS
                .into_iter()
                .map(|n| (true, n))
                .chain(NEW_SLOT.into_iter().chain(OLD_SLOT).map(|n| (false, n)))
            {
                let f = ready(true, 2);
                let path = if state {
                    f.paths.state_directory.join(name)
                } else {
                    f.config.join(name)
                };
                let mut h = host();
                if late {
                    h.action = Box::new(move |n| {
                        if n == 2 {
                            private(&path, b"late synthetic fence");
                        }
                    });
                } else {
                    symlink("missing-synthetic", path).unwrap();
                }
                assert!(review(&f, h).is_err(), "late={late} {name}");
            }
        }
    }
    #[test]
    fn preauth_status_late_directories_login_and_owned_host_refuse() {
        for which in 0..3 {
            let f = ready(true, 2);
            let path = [
                f.config.clone(),
                f.paths.state_directory.clone(),
                f.paths.runtime_base.clone(),
            ][which]
                .clone();
            let saved = path.with_extension("synthetic-saved");
            let restore_path = path.clone();
            let restore_saved = saved.clone();
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == 2 {
                    fs::rename(&path, &saved).unwrap();
                    fs::create_dir(&path).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                }
            });
            assert!(review(&f, h).is_err());
            fs::remove_dir(&restore_path).unwrap();
            fs::rename(restore_saved, restore_path).unwrap();
        }
        for existing in [false, true] {
            let f = ready(true, 2);
            let path = f.paths.runtime_base.join("omavless-login.receipt");
            let raw = br#"{"schemaVersion":1,"epochHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ownershipGeneration":2,"phase":"consumed"}"#;
            if existing {
                private(&path, raw);
            }
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == 2 {
                    let replacement = path.with_extension("synthetic-swap");
                    private(&replacement, raw);
                    fs::rename(replacement, &path).unwrap();
                }
            });
            assert!(review(&f, h).is_err());
        }
        let f = ready(true, 2);
        let mut h = host();
        h.owned_after_observe = Some(2);
        assert!(review(&f, h).is_err());
    }

    #[test]
    fn preauth_status_source_and_boundary_same_byte_swaps_and_owned_host_refuse() {
        for index in 0..6 {
            let f = ready(true, 2);
            let path = [
                f.paths.state_directory.join(CLOSURE_MEMBER),
                f.config.join("profiles.json"),
                f.config.join("route-template.yaml"),
                f.paths.ownership_marker.clone(),
                f.paths.operation_lock.clone(),
                f.paths.state_directory.join("desired.json"),
            ][index]
                .clone();
            let mut h = host();
            h.action = Box::new(move |n| {
                if n == 2 {
                    let bytes = fs::read(&path).unwrap_or_else(|_| br#"{"schemaVersion":1,"generation":1,"connected":false,"profileId":null,"routingMode":"global"}"#.to_vec());
                    let replacement = path.with_extension("synthetic-swap");
                    private(&replacement, &bytes);
                    fs::rename(replacement, &path).unwrap();
                }
            });
            assert!(review(&f, h).is_err());
        }
        let f = ready(true, 2);
        let mut h = host();
        h.owned = true;
        assert!(review(&f, h).is_err());
    }
}
