// SPDX-License-Identifier: MIT

//! Prepared, compare-before-replace profile-store transactions.
//!
//! This module is not registered with IPC. A future owner must hold the
//! migration lock and serialized mutation slot while a prepared value exists.

use omavless_domain::private_store::{PrivateStoreError, ProfileMutation, apply_profile_mutation};
use omavless_store::{StoreIoError, atomic_replace_private, read_private_utf8};
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMutationCommitError {
    UnsafeStore,
    StoreChanged,
    StoreIo,
    Mutation(PrivateStoreError),
}

impl fmt::Display for ProfileMutationCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeStore => formatter.write_str("Private profile store path is unsafe"),
            Self::StoreChanged => formatter.write_str("Private profile store changed concurrently"),
            Self::StoreIo => formatter.write_str("Private profile store update failed"),
            Self::Mutation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProfileMutationCommitError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedWrite {
    NoChange,
    Changed,
}

/// One credential-bearing original/candidate pair. It intentionally cannot be
/// formatted, cloned or serialized. `restore` accepts only the exact original
/// or exact candidate bytes, never an unrelated same-user edit.
pub struct PreparedProfileMutation {
    store_path: PathBuf,
    uid: u32,
    original: Vec<u8>,
    candidate: Vec<u8>,
    changed: bool,
}

impl PreparedProfileMutation {
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    fn current_bytes(&self) -> Result<Vec<u8>, ProfileMutationCommitError> {
        validate_store_path(&self.store_path, self.uid)?;
        read_private_utf8(&self.store_path, self.uid)
            .map(String::into_bytes)
            .map_err(map_io)
    }

    fn replace_and_verify(&self, payload: &[u8]) -> Result<(), ProfileMutationCommitError> {
        atomic_replace_private(&self.store_path, payload, self.uid).map_err(map_io)?;
        if self.current_bytes()?.as_slice() != payload {
            return Err(ProfileMutationCommitError::StoreChanged);
        }
        Ok(())
    }

    /// Commit only while the store still has the exact bytes used to prepare
    /// this candidate. A semantic reparse is deliberately insufficient.
    pub fn commit(&self) -> Result<PreparedWrite, ProfileMutationCommitError> {
        if self.current_bytes()? != self.original {
            return Err(ProfileMutationCommitError::StoreChanged);
        }
        if !self.changed {
            return Ok(PreparedWrite::NoChange);
        }
        self.replace_and_verify(&self.candidate)?;
        Ok(PreparedWrite::Changed)
    }

    /// Restore the exact original after a failed active-profile transition.
    /// Already-restored bytes are accepted as an idempotent no-op. Anything
    /// else is ambiguous and fails closed.
    pub fn restore(&self) -> Result<PreparedWrite, ProfileMutationCommitError> {
        let current = self.current_bytes()?;
        if current == self.original {
            return Ok(PreparedWrite::NoChange);
        }
        if current != self.candidate {
            return Err(ProfileMutationCommitError::StoreChanged);
        }
        self.replace_and_verify(&self.original)?;
        Ok(PreparedWrite::Changed)
    }
}

fn private_parent(path: &Path, uid: u32) -> bool {
    path.parent().is_some_and(|parent| {
        fs::symlink_metadata(parent).is_ok_and(|metadata| {
            !metadata.file_type().is_symlink()
                && metadata.is_dir()
                && metadata.uid() == uid
                && metadata.permissions().mode() & 0o077 == 0
        })
    })
}

fn private_store_file(path: &Path, uid: u32) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.uid() == uid
            && metadata.permissions().mode() & 0o077 == 0
    })
}

fn validate_store_path(path: &Path, uid: u32) -> Result<(), ProfileMutationCommitError> {
    if path.is_absolute()
        && path.file_name().and_then(|name| name.to_str()) == Some("profiles.json")
        && private_parent(path, uid)
        && private_store_file(path, uid)
    {
        Ok(())
    } else {
        Err(ProfileMutationCommitError::UnsafeStore)
    }
}

fn map_io(_error: StoreIoError) -> ProfileMutationCommitError {
    ProfileMutationCommitError::StoreIo
}

/// Validate and render a complete replacement without changing the store.
pub fn prepare_profile_mutation(
    store_path: &Path,
    uid: u32,
    mutation: ProfileMutation,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    validate_store_path(store_path, uid)?;
    let input = read_private_utf8(store_path, uid).map_err(map_io)?;
    let result =
        apply_profile_mutation(&input, mutation).map_err(ProfileMutationCommitError::Mutation)?;
    Ok(PreparedProfileMutation {
        store_path: store_path.to_path_buf(),
        uid,
        original: input.into_bytes(),
        candidate: result.payload().to_vec(),
        changed: result.changed,
    })
}

/// Compatibility helper for the existing offline store-only boundary.
pub fn commit_profile_mutation(
    store_path: &Path,
    uid: u32,
    mutation: ProfileMutation,
) -> Result<PreparedWrite, ProfileMutationCommitError> {
    prepare_profile_mutation(store_path, uid, mutation)?.commit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::os::unix::fs::symlink;
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
        assert!(prepared.changed());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(prepared.commit(), Ok(PreparedWrite::Changed));
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_ne!(fs::read(&path).unwrap(), before);
        assert_eq!(prepared.restore(), Ok(PreparedWrite::Changed));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(prepared.restore(), Ok(PreparedWrite::NoChange));
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
        assert_eq!(
            no_change.commit(),
            Err(ProfileMutationCommitError::StoreChanged)
        );
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
        assert_eq!(
            prepared.commit(),
            Err(ProfileMutationCommitError::StoreChanged)
        );
        assert_eq!(
            prepared.restore(),
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
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            prepared.commit(),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let destination = root.join("destination");
        fs::write(&destination, b"private.example/password").unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&destination, &path).unwrap();
        assert_eq!(
            prepared.commit(),
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
        assert_eq!(
            commit_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned()
                }
            ),
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
