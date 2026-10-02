// SPDX-License-Identifier: MIT

//! Inactive v4 persistence prerequisite. No daemon, IPC, CLI, bootstrap or
//! migration caller uses this module. It accepts an already-v4 private store
//! only; it cannot upgrade a production v1-v3 store to unreadable v4 bytes.
//! Owner revision/replay and active-profile lifecycle admission remain required
//! before a future registered integration can use this transaction.

use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::private_store_transaction::{
    PreparedPrivateStoreWrite, PreparedWrite, PrivateStoreWriteError, prepare_private_store_write,
    validate_store_path,
};
use omavless_domain::private_store::{
    CandidatePrivateStore, CandidateProfileEditInput, CandidateProfileExport, PrivateStoreError,
    parse_candidate_private_store,
};
use omavless_store::read_private_utf8;
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

fn read_candidate_locked<T>(
    store_path: &Path,
    uid: u32,
    lock: &MigrationLock,
    paths: &CutoverPaths,
    generation: u64,
    project: impl FnOnce(&CandidatePrivateStore) -> Result<T, PrivateStoreError>,
) -> Result<T, CandidateStoreWriteError> {
    authorize(lock, paths, uid, generation)?;
    validate_store_path(store_path, uid).map_err(CandidateStoreWriteError::Write)?;
    let source = read_private_utf8(store_path, uid)
        .map_err(|_| CandidateStoreWriteError::Write(PrivateStoreWriteError::StoreIo))?;
    let candidate = parse_candidate_private_store(&source).map_err(|error| {
        CandidateStoreWriteError::Write(PrivateStoreWriteError::Mutation(error))
    })?;
    let result = project(&candidate).map_err(|error| {
        CandidateStoreWriteError::Write(PrivateStoreWriteError::Mutation(error))
    })?;
    authorize(lock, paths, uid, generation)?;
    validate_store_path(store_path, uid).map_err(CandidateStoreWriteError::Write)?;
    let current = read_private_utf8(store_path, uid)
        .map_err(|_| CandidateStoreWriteError::Write(PrivateStoreWriteError::StoreIo))?;
    if current != source {
        return Err(CandidateStoreWriteError::Write(
            PrivateStoreWriteError::StoreChanged,
        ));
    }
    Ok(result)
}

/// Inactive deliberate private native export from an already-v4 file. No
/// registered client can call this; eventual same-user IPC authentication,
/// explicit action and bounded credential framing remain required.
pub fn read_candidate_native_export_locked(
    store_path: &Path,
    uid: u32,
    lock: &MigrationLock,
    paths: &CutoverPaths,
    generation: u64,
    profile_id: &str,
) -> Result<CandidateProfileExport, CandidateStoreWriteError> {
    read_candidate_locked(store_path, uid, lock, paths, generation, |candidate| {
        candidate.export_private_native_credential(profile_id)
    })
}

/// Inactive standalone editor seed under the same complete-file and exact
/// owner checks as native export. Managed records cannot be edited.
pub fn read_candidate_edit_input_locked(
    store_path: &Path,
    uid: u32,
    lock: &MigrationLock,
    paths: &CutoverPaths,
    generation: u64,
    profile_id: &str,
) -> Result<CandidateProfileEditInput, CandidateStoreWriteError> {
    read_candidate_locked(store_path, uid, lock, paths, generation, |candidate| {
        candidate.private_edit_input(profile_id)
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
    use std::io::Write as _;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
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
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&store)
            .unwrap();
        output
            .write_all(&candidate.into_private_bytes().unwrap())
            .unwrap();
        drop(output);
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

    #[test]
    fn candidate_private_editor_roundtrip_commits_and_restores_one_complete_file() {
        let (root, store, paths, uid, lock) = fixture();
        let uri_id = "00000000-0000-0000-0000-000000000002";
        let source = fs::read_to_string(&store).unwrap();
        let candidate = parse_candidate_private_store(&source).unwrap().with_profile(uri_id, "URI", CandidateProfileInput::Uri("vless://11111111-1111-4111-8111-111111111111@192.0.2.3:443?security=none&type=tcp".into())).unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
        document["activeId"] = ID.into();
        document["lastId"] = ID.into();
        document["startup"] =
            json!({"enabled":true,"target":"profile","profileId":ID,"mode":"rule"});
        document["extension"] = json!({"preserve":true});
        fs::write(&store, document.to_string()).unwrap();
        let before = fs::read(&store).unwrap();
        let inode = fs::metadata(&store).unwrap().ino();
        let editor = read_candidate_edit_input_locked(&store, uid, &lock, &paths, 2, ID).unwrap();
        assert!(editor.private_name() == "WG");
        let export =
            read_candidate_native_export_locked(&store, uid, &lock, &paths, 2, ID).unwrap();
        assert!(
            export.expose_private_bytes() == editor.private_credential().expose_private_bytes()
        );
        assert_eq!(fs::metadata(&store).unwrap().ino(), inode);
        assert!(fs::read(&store).unwrap() == before);
        let text = std::str::from_utf8(export.expose_private_bytes())
            .unwrap()
            .replace("10.8.0.2/32", "10.8.0.3/32");
        let replacement = parse_wireguard_config(&text).unwrap();
        let prepared = prepare_candidate_store_write(&store, uid, &lock, &paths, 2, |candidate| {
            candidate.replace_standalone(
                ID,
                "Edited WG",
                CandidateProfileInput::WireGuard(replacement),
            )
        })
        .unwrap();
        assert_eq!(prepared.commit_locked(&lock), Ok(PreparedWrite::Changed));
        let after: serde_json::Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        assert!(after["profiles"][1] == document["profiles"][1]);
        for field in ["activeId", "lastId", "startup", "extension"] {
            assert!(after[field] == document[field]);
        }
        let edited = read_candidate_edit_input_locked(&store, uid, &lock, &paths, 2, ID).unwrap();
        assert!(edited.private_name() == "Edited WG");
        let restored = parse_wireguard_config(
            std::str::from_utf8(edited.private_credential().expose_private_bytes()).unwrap(),
        )
        .unwrap();
        assert!(
            restored.subscription_identity()
                != parse_wireguard_config(
                    std::str::from_utf8(export.expose_private_bytes()).unwrap()
                )
                .unwrap()
                .subscription_identity()
        );
        assert_eq!(prepared.restore_locked(&lock), Ok(PreparedWrite::Changed));
        assert!(fs::read(&store).unwrap() == before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_private_reads_refuse_whole_store_errors_and_revoked_ownership() {
        let (root, store, paths, uid, lock) = fixture();
        let original = fs::read(&store).unwrap();
        for phase in ["legacy", "cutoverPreparing", "rollbackPreparing"] {
            set_marker(&paths, phase, 2);
            assert!(matches!(
                read_candidate_native_export_locked(&store, uid, &lock, &paths, 2, ID),
                Err(CandidateStoreWriteError::OwnershipUnavailable)
            ));
            assert!(matches!(
                read_candidate_edit_input_locked(&store, uid, &lock, &paths, 2, ID),
                Err(CandidateStoreWriteError::OwnershipUnavailable)
            ));
        }
        set_marker(&paths, "rust", 4);
        assert!(read_candidate_edit_input_locked(&store, uid, &lock, &paths, 2, ID).is_err());
        set_marker(&paths, "rust", 2);
        let mut invalid: serde_json::Value = serde_json::from_slice(&original).unwrap();
        invalid["profiles"].as_array_mut().unwrap().push(json!({"id":"00000000-0000-0000-0000-000000000002","name":"Bad unrelated URI","uri":"invalid-private-input","protocol":"vless"}));
        fs::write(&store, invalid.to_string()).unwrap();
        assert!(read_candidate_edit_input_locked(&store, uid, &lock, &paths, 2, ID).is_err());
        assert!(read_candidate_native_export_locked(&store, uid, &lock, &paths, 2, ID).is_err());
        fs::write(&store, &original).unwrap();
        assert!(
            read_candidate_native_export_locked(
                &store,
                uid,
                &lock,
                &paths,
                2,
                "00000000-0000-0000-0000-000000000099"
            )
            .is_err()
        );
        fs::set_permissions(&store, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_candidate_edit_input_locked(&store, uid, &lock, &paths, 2, ID).is_err());
        assert!(fs::read(&store).unwrap() == original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn candidate_private_read_discards_projection_after_unexpected_file_or_owner_change() {
        let (root, store, paths, uid, lock) = fixture();
        let original = fs::read(&store).unwrap();
        let result = read_candidate_locked(&store, uid, &lock, &paths, 2, |candidate| {
            let export = candidate.export_private_native_credential(ID)?;
            fs::write(&store, b"unrelated\n").unwrap();
            Ok(export)
        });
        assert!(matches!(
            result,
            Err(CandidateStoreWriteError::Write(
                PrivateStoreWriteError::StoreChanged
            ))
        ));
        assert!(fs::read(&store).unwrap() == b"unrelated\n");
        fs::write(&store, &original).unwrap();
        let result = read_candidate_locked(&store, uid, &lock, &paths, 2, |candidate| {
            let export = candidate.export_private_native_credential(ID)?;
            set_marker(&paths, "rust", 4);
            Ok(export)
        });
        assert!(matches!(
            result,
            Err(CandidateStoreWriteError::OwnershipUnavailable)
        ));
        assert!(fs::read(&store).unwrap() == original);
        fs::remove_dir_all(root).unwrap();
    }
}
