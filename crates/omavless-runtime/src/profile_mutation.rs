// SPDX-License-Identifier: MIT

//! Prepared, compare-before-replace profile-store transactions.
//!
//! This module is not registered with IPC. A future owner must hold the
//! migration lock and serialized mutation slot while a prepared value exists.

use crate::cutover::{CutoverPaths, MigrationLock};
use crate::private_store_transaction::{PreparedPrivateStoreWrite, prepare_private_store_write};
use omavless_domain::private_store::{ProfileMutation, apply_profile_mutation};
use std::path::Path;

pub use crate::private_store_transaction::{PreparedWrite, PrivateStoreWriteError};
pub type ProfileMutationCommitError = PrivateStoreWriteError;

/// One credential-bearing original/candidate pair. It intentionally cannot be
/// formatted, cloned or serialized. `restore` accepts only the exact original
/// or exact candidate bytes, never an unrelated same-user edit.
pub struct PreparedProfileMutation {
    prepared: PreparedPrivateStoreWrite,
}

impl PreparedProfileMutation {
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.prepared.changed()
    }

    /// Commit only while the store still has the exact bytes used to prepare
    /// this candidate. A semantic reparse is deliberately insufficient.
    pub fn commit_locked(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, ProfileMutationCommitError> {
        self.prepared.commit_locked(lock, paths)
    }

    /// Restore the exact original after a failed active-profile transition.
    /// Already-restored bytes are accepted as an idempotent no-op. Anything
    /// else is ambiguous and fails closed.
    pub fn restore_locked(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, ProfileMutationCommitError> {
        self.prepared.restore_locked(lock, paths)
    }
}

/// Validate and render a complete replacement without changing the store.
pub fn prepare_profile_mutation(
    store_path: &Path,
    uid: u32,
    mutation: ProfileMutation,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        let result = apply_profile_mutation(input, mutation)?;
        Ok((result.payload().to_vec(), result.changed))
    })?;
    Ok(PreparedProfileMutation { prepared })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_domain::private_store::PrivateStoreError;
    use serde_json::{Value, json};
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";

    fn root(label: &str) -> (PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-profile-mutation-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn store() -> Value {
        json!({
            "version": 3,
            "activeId": PROFILE,
            "lastId": PROFILE,
            "profiles": [{
                "id": PROFILE,
                "name": "Private",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Private",
                "protocol": "vless",
                "favorite": false
            }],
            "subscriptions": [],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": true, "target": "profile", "profileId": PROFILE, "mode": "rule"},
            "onboardingComplete": true
        })
    }

    fn write_store(path: &Path) {
        fs::write(path, serde_json::to_vec(&store()).unwrap()).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn locked(root: &Path, uid: u32) -> (CutoverPaths, MigrationLock) {
        let paths = CutoverPaths::below(root, root, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        (paths, lock)
    }

    #[test]
    fn prepare_is_read_only_and_commit_restore_are_exact_private_replacements() {
        let (root, uid) = root("prepared");
        let path = root.join("profiles.json");
        write_store(&path);
        let before = fs::read(&path).unwrap();
        let prepared = prepare_profile_mutation(
            &path,
            uid,
            ProfileMutation::Rename {
                profile_id: PROFILE.to_owned(),
                new_name: "Renamed".to_owned(),
            },
        )
        .unwrap();
        let (paths, lock) = locked(&root, uid);
        assert!(prepared.changed());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_ne!(fs::read(&path).unwrap(), before);
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Ok(PreparedWrite::NoChange)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_change_never_rewrites_and_external_edits_fail_guarded_compare_and_restore() {
        let (root, uid) = root("cas");
        let path = root.join("profiles.json");
        write_store(&path);
        let no_change = prepare_profile_mutation(
            &path,
            uid,
            ProfileMutation::Favorite {
                profile_id: PROFILE.to_owned(),
                enabled: false,
            },
        )
        .unwrap();
        let mut external = store();
        external["onboardingComplete"] = json!(false);
        fs::write(&path, serde_json::to_vec(&external).unwrap()).unwrap();
        let (paths, lock) = locked(&root, uid);
        assert_eq!(
            no_change.commit_locked(&lock, &paths),
            Err(ProfileMutationCommitError::StoreChanged)
        );
        drop(lock);
        write_store(&path);

        let prepared = prepare_profile_mutation(
            &path,
            uid,
            ProfileMutation::Favorite {
                profile_id: PROFILE.to_owned(),
                enabled: true,
            },
        )
        .unwrap();
        let mut external = store();
        external["onboardingComplete"] = json!(false);
        fs::write(&path, serde_json::to_vec(&external).unwrap()).unwrap();
        let (paths, lock) = locked(&root, uid);
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Err(ProfileMutationCommitError::StoreChanged)
        );
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Err(ProfileMutationCommitError::StoreChanged)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["onboardingComplete"],
            false
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepared_transaction_rechecks_symlink_and_mode_safety() {
        let (root, uid) = root("prepared-safety");
        let path = root.join("profiles.json");
        write_store(&path);
        let prepared = prepare_profile_mutation(
            &path,
            uid,
            ProfileMutation::Favorite {
                profile_id: PROFILE.to_owned(),
                enabled: true,
            },
        )
        .unwrap();
        let (paths, lock) = locked(&root, uid);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let destination = root.join("destination");
        fs::write(&destination, b"private.example/password").unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&destination, &path).unwrap();
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"private.example/password");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejected_mutation_and_unsafe_paths_are_unchanged_and_private() {
        let (root, uid) = root("unsafe");
        let path = root.join("profiles.json");
        write_store(&path);
        let before = fs::read(&path).unwrap();
        assert_eq!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Rename {
                    profile_id: PROFILE.to_owned(),
                    new_name: "x".repeat(81)
                }
            )
            .err()
            .unwrap(),
            ProfileMutationCommitError::Mutation(PrivateStoreError::InvalidName)
        );
        assert_eq!(fs::read(&path).unwrap(), before);

        let destination = root.join("destination");
        fs::write(&destination, b"private.example/password").unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&destination, &path).unwrap();
        let (paths, lock) = locked(&root, uid);
        assert_eq!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned()
                }
            )
            .and_then(|prepared| prepared.commit_locked(&lock, &paths)),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"private.example/password");
        fs::remove_file(&path).unwrap();
        write_store(&path);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned()
                }
            )
            .err()
            .unwrap(),
            ProfileMutationCommitError::UnsafeStore
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned()
                }
            )
            .err()
            .unwrap(),
            ProfileMutationCommitError::UnsafeStore
        );
        for error in [
            ProfileMutationCommitError::StoreIo,
            ProfileMutationCommitError::StoreChanged,
        ] {
            let public = format!("{error:?} {error}");
            assert!(!public.contains("private.example"));
            assert!(!public.contains("password"));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
