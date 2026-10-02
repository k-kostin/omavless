// SPDX-License-Identifier: MIT

//! Inactive v4 persistence prerequisite. No daemon, IPC, CLI, bootstrap or
//! migration caller uses this module. It accepts an already-v4 private store
//! only; it cannot upgrade a production v1-v3 store to unreadable v4 bytes.
//! Owner revision/replay and active-profile lifecycle admission remain required
//! before a future registered integration can use this transaction.

use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::private_store_transaction::{
    PreparedPrivateStoreWrite, PreparedWrite, PrivateStoreWriteError, prepare_private_store_write,
};
use omavless_domain::private_store::{
    CandidatePrivateStore, PrivateStoreError, parse_candidate_private_store,
};
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateStoreWriteError {
    OwnershipUnavailable,
    Write(PrivateStoreWriteError),
}

impl fmt::Display for CandidateStoreWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnershipUnavailable => {
                formatter.write_str("Candidate store ownership is unavailable")
            }
            Self::Write(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for CandidateStoreWriteError {}

/// Exact private original/candidate pair plus pinned owner generation. No
/// formatting, cloning, serialization or credential accessors are provided.
pub struct PreparedCandidateStoreWrite {
    prepared: PreparedPrivateStoreWrite,
    paths: CutoverPaths,
    uid: u32,
    generation: u64,
}

fn authorize(
    lock: &MigrationLock,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
) -> Result<(), CandidateStoreWriteError> {
    if !lock.authorizes(paths, uid) {
        return Err(CandidateStoreWriteError::Write(
            PrivateStoreWriteError::LockMismatch,
        ));
    }
    for (path, private) in [
        (paths.state_directory.as_path(), true),
        (
            paths
                .state_directory
                .parent()
                .ok_or(CandidateStoreWriteError::OwnershipUnavailable)?,
            false,
        ),
    ] {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| CandidateStoreWriteError::OwnershipUnavailable)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || (private && metadata.permissions().mode() & 0o077 != 0)
        {
            return Err(CandidateStoreWriteError::OwnershipUnavailable);
        }
    }
    let marker = read_marker_existing(paths, uid)
        .map_err(|_| CandidateStoreWriteError::OwnershipUnavailable)?;
    if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
        return Err(CandidateStoreWriteError::OwnershipUnavailable);
    }
    Ok(())
}

impl PreparedCandidateStoreWrite {
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.prepared.changed()
    }

    pub fn commit_locked(
        &self,
        lock: &MigrationLock,
    ) -> Result<PreparedWrite, CandidateStoreWriteError> {
        authorize(lock, &self.paths, self.uid, self.generation)?;
        self.prepared
            .commit_locked(lock, &self.paths)
            .map_err(CandidateStoreWriteError::Write)
    }

    /// Compensation preserves byte-exact original data and refuses unrelated
    /// bytes or revoked ownership. It cannot downgrade or drop WG records.
    pub fn restore_locked(
        &self,
        lock: &MigrationLock,
    ) -> Result<PreparedWrite, CandidateStoreWriteError> {
        authorize(lock, &self.paths, self.uid, self.generation)?;
        self.prepared
            .restore_locked(lock, &self.paths)
            .map_err(CandidateStoreWriteError::Write)
    }
}

/// Validate the whole v4 source and whole transformed candidate while holding
/// the exact migration lease. Existing private-file and atomic replacement
/// policy are reused. Preparation never publishes bytes. A semantic no-op
/// preserves original formatting rather than normalizing/replacing the file.
pub fn prepare_candidate_store_write<F>(
    store_path: &Path,
    uid: u32,
    lock: &MigrationLock,
    paths: &CutoverPaths,
    generation: u64,
    transform: F,
) -> Result<PreparedCandidateStoreWrite, CandidateStoreWriteError>
where
    F: FnOnce(CandidatePrivateStore) -> Result<(CandidatePrivateStore, bool), PrivateStoreError>,
{
    authorize(lock, paths, uid, generation)?;
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        let source = parse_candidate_private_store(input)?;
        let (candidate, changed) = transform(source)?;
        let bytes = candidate.into_private_bytes()?;
        Ok((bytes, changed))
    })
    .map_err(CandidateStoreWriteError::Write)?;
    authorize(lock, paths, uid, generation)?;
    Ok(PreparedCandidateStoreWrite {
        prepared,
        paths: paths.clone(),
        uid,
        generation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use omavless_domain::private_store::{CandidateProfileInput, parse_private_store};
    use omavless_profile::wireguard::parse_wireguard_config;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::PathBuf;

    const ID: &str = "00000000-0000-0000-0000-000000000001";

    fn fixture() -> (PathBuf, PathBuf, CutoverPaths, u32, MigrationLock) {
        let root = crate::test_temp::directory("p4-write").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        let paths = CutoverPaths::below(&root, &root, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        fs::create_dir(&paths.state_directory).unwrap();
        fs::set_permissions(&paths.state_directory, fs::Permissions::from_mode(0o700)).unwrap();
        set_marker(&paths, "rust", 2);
        let native = format!(
            "[Interface]\nPrivateKey = {}\nAddress = 10.8.0.2/32\n[Peer]\nPublicKey = {}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.1:51820\n",
            STANDARD.encode([7_u8; 32]),
            STANDARD.encode([9_u8; 32])
        );
        let candidate = parse_candidate_private_store(r#"{"version":4,"profiles":[]}"#)
            .unwrap()
            .with_profile(
                ID,
                "WG",
                CandidateProfileInput::WireGuard(parse_wireguard_config(&native).unwrap()),
            )
            .unwrap();
        let store = root.join("profiles.json");
        fs::write(&store, candidate.into_private_bytes().unwrap()).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
        (root, store, paths, uid, lock)
    }

    fn set_marker(paths: &CutoverPaths, phase: &str, generation: u64) {
        fs::write(
            &paths.ownership_marker,
            json!({"schemaVersion":1,"phase":phase,"generation":generation}).to_string(),
        )
        .unwrap();
        fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn candidate_private_publication_and_compensation_use_exact_existing_writer() {
        let (root, store, paths, uid, lock) = fixture();
        let before = fs::read(&store).unwrap();
        let prepared = prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |candidate| {
            candidate.rename_standalone(ID, "Renamed")
        })
        .unwrap();
        assert!(
            fs::read(&store).unwrap() == before,
            "preparation cannot publish"
        );
        assert_eq!(prepared.commit_locked(&lock), Ok(PreparedWrite::Changed));
        assert_eq!(
            fs::metadata(&store).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let after = fs::read_to_string(&store).unwrap();
        assert!(parse_candidate_private_store(&after).is_ok());
        assert!(
            parse_private_store(&after).is_err(),
            "production reader remains v4-refusing"
        );
        assert_eq!(prepared.restore_locked(&lock), Ok(PreparedWrite::Changed));
        assert!(fs::read(&store).unwrap() == before);
        assert_eq!(prepared.restore_locked(&lock), Ok(PreparedWrite::NoChange));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_write_noop_and_concurrent_edits_do_not_replace_bytes() {
        let (root, store, paths, uid, lock) = fixture();
        let before = fs::read(&store).unwrap();
        let inode = fs::metadata(&store).unwrap().ino();
        let noop = prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |candidate| {
            candidate.rename_standalone(ID, "WG")
        })
        .unwrap();
        assert!(!noop.changed());
        assert_eq!(noop.commit_locked(&lock), Ok(PreparedWrite::NoChange));
        assert_eq!(fs::metadata(&store).unwrap().ino(), inode);
        assert!(fs::read(&store).unwrap() == before);
        let prepared = prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |candidate| {
            candidate.set_favorite(ID, true)
        })
        .unwrap();
        fs::write(&store, b"unrelated\n").unwrap();
        for result in [
            prepared.commit_locked(&lock),
            prepared.restore_locked(&lock),
        ] {
            assert_eq!(
                result,
                Err(CandidateStoreWriteError::Write(
                    PrivateStoreWriteError::StoreChanged
                ))
            );
        }
        assert!(fs::read(&store).unwrap() == b"unrelated\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_write_requires_same_lock_and_durable_owner_at_every_boundary() {
        let (root, store, paths, uid, lock) = fixture();
        let prepared = prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |candidate| {
            candidate.set_favorite(ID, true)
        })
        .unwrap();
        let before = fs::read(&store).unwrap();
        let wrong_root = root.join("wrong");
        fs::create_dir(&wrong_root).unwrap();
        fs::set_permissions(&wrong_root, fs::Permissions::from_mode(0o700)).unwrap();
        let wrong_paths = CutoverPaths::below(&wrong_root, &wrong_root, uid);
        let wrong_lock = MigrationLock::acquire(&wrong_paths, uid).unwrap();
        assert_eq!(
            prepared.commit_locked(&wrong_lock),
            Err(CandidateStoreWriteError::Write(
                PrivateStoreWriteError::LockMismatch
            ))
        );
        for (phase, generation) in [
            ("legacy", 2),
            ("cutoverPreparing", 2),
            ("rollbackPreparing", 2),
            ("rust", 4),
        ] {
            set_marker(&paths, phase, generation);
            assert_eq!(
                prepared.commit_locked(&lock),
                Err(CandidateStoreWriteError::OwnershipUnavailable)
            );
            assert_eq!(
                prepared.restore_locked(&lock),
                Err(CandidateStoreWriteError::OwnershipUnavailable)
            );
            assert!(
                prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                    "transform after revocation"
                ))
                .is_err()
            );
        }
        assert!(fs::read(&store).unwrap() == before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_writer_refuses_legacy_corrupt_and_unsafe_store_before_publication() {
        let (root, store, paths, uid, lock) = fixture();
        for bytes in [
            br#"{"version":3,"profiles":[]}"#.as_slice(),
            b"broken",
            br#"{"version":4,"version":4,"profiles":[]}"#,
        ] {
            fs::write(&store, bytes).unwrap();
            assert!(
                prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                    "invalid source admitted"
                ))
                .is_err()
            );
            assert!(fs::read(&store).unwrap() == bytes);
        }
        fs::set_permissions(&store, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                "unsafe store admitted"
            ))
            .is_err()
        );
        fs::remove_file(&store).unwrap();
        symlink(&paths.ownership_marker, &store).unwrap();
        assert!(
            prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                "symlink admitted"
            ))
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_writer_refuses_unsafe_owner_and_does_not_create_missing_state() {
        let (root, store, paths, uid, lock) = fixture();
        let before = fs::read(&store).unwrap();
        for bytes in [
            b"broken".as_slice(),
            br#"{"schemaVersion":1,"generation":2,"phase":"rust","phase":"rust"}"#,
        ] {
            fs::write(&paths.ownership_marker, bytes).unwrap();
            assert!(
                prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                    "invalid owner admitted"
                ))
                .is_err()
            );
        }
        set_marker(&paths, "rust", 2);
        fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                "unsafe owner admitted"
            ))
            .is_err()
        );
        fs::remove_file(&paths.ownership_marker).unwrap();
        symlink(&store, &paths.ownership_marker).unwrap();
        assert!(
            prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                "symlink owner admitted"
            ))
            .is_err()
        );
        fs::remove_file(&paths.ownership_marker).unwrap();
        fs::remove_dir(&paths.state_directory).unwrap();
        assert!(
            prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |_| panic!(
                "missing owner admitted"
            ))
            .is_err()
        );
        assert!(!paths.state_directory.exists());
        assert!(fs::read(&store).unwrap() == before);
        fs::remove_dir_all(root).unwrap();
    }
}
