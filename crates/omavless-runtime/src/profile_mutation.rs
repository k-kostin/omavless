// SPDX-License-Identifier: MIT

//! Atomic private-store commit boundary for native profile mutations.
//!
//! This module is not registered with IPC. A future owner dispatch must hold
//! the migration lock and serialized mutation slot before calling it, and must
//! coordinate active-profile rename/delete with the lifecycle transaction.

use omavless_domain::private_store::{PrivateStoreError, ProfileMutation, apply_profile_mutation};
use omavless_store::{StoreIoError, atomic_replace_private, read_private_utf8};
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMutationCommitError {
    UnsafeStore,
    StoreIo,
    Mutation(PrivateStoreError),
}

impl fmt::Display for ProfileMutationCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeStore => formatter.write_str("Private profile store path is unsafe"),
            Self::StoreIo => formatter.write_str("Private profile store update failed"),
            Self::Mutation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProfileMutationCommitError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileMutationCommit {
    pub changed: bool,
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

fn map_io(_error: StoreIoError) -> ProfileMutationCommitError {
    ProfileMutationCommitError::StoreIo
}

/// Apply and atomically replace one existing `profiles.json`.
///
/// There is no path, shell, service, controller or command input exposed to a
/// client. The replacement is fully rendered and bounded before the first
/// filesystem mutation.
pub fn commit_profile_mutation(
    store_path: &Path,
    uid: u32,
    mutation: ProfileMutation,
) -> Result<ProfileMutationCommit, ProfileMutationCommitError> {
    if !store_path.is_absolute()
        || store_path.file_name().and_then(|name| name.to_str()) != Some("profiles.json")
        || !private_parent(store_path, uid)
        || !private_store_file(store_path, uid)
    {
        return Err(ProfileMutationCommitError::UnsafeStore);
    }
    let input = read_private_utf8(store_path, uid).map_err(map_io)?;
    let result =
        apply_profile_mutation(&input, mutation).map_err(ProfileMutationCommitError::Mutation)?;
    let changed = result.changed;
    atomic_replace_private(store_path, result.payload(), uid).map_err(map_io)?;
    Ok(ProfileMutationCommit { changed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";

    fn root(label: &str) -> (std::path::PathBuf, u32) {
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
            "activeId": "",
            "lastId": PROFILE,
            "profiles": [{
                "id": PROFILE,
                "name": "Private",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Private",
                "protocol": "vless",
                "favorite": false,
            }],
            "subscriptions": [],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true,
        })
    }

    fn write_store(path: &Path) {
        fs::write(path, serde_json::to_vec(&store()).unwrap()).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn successful_commit_is_atomic_private_and_preserves_credentials() {
        let (root, uid) = root("success");
        let path = root.join("profiles.json");
        write_store(&path);
        let result = commit_profile_mutation(
            &path,
            uid,
            ProfileMutation::Favorite {
                profile_id: PROFILE.to_owned(),
                enabled: true,
            },
        )
        .unwrap();
        assert!(result.changed);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let written: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["profiles"][0]["favorite"], true);
        assert!(
            written["profiles"][0]["uri"]
                .as_str()
                .unwrap()
                .contains("11111111")
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejected_mutation_does_not_change_private_file() {
        let (root, uid) = root("rejected");
        let path = root.join("profiles.json");
        write_store(&path);
        let before = fs::read(&path).unwrap();
        let error = commit_profile_mutation(
            &path,
            uid,
            ProfileMutation::Rename {
                profile_id: PROFILE.to_owned(),
                new_name: "x".repeat(81),
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            ProfileMutationCommitError::Mutation(PrivateStoreError::InvalidName)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn symlink_and_nonprivate_parent_fail_without_touching_destination() {
        let (root, uid) = root("unsafe");
        let destination = root.join("destination");
        fs::write(&destination, b"private.example/password").unwrap();
        let path = root.join("profiles.json");
        symlink(&destination, &path).unwrap();
        assert_eq!(
            commit_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned(),
                },
            ),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"private.example/password");
        fs::remove_file(&path).unwrap();
        write_store(&path);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            commit_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned(),
                },
            ),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            commit_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: PROFILE.to_owned(),
                },
            ),
            Err(ProfileMutationCommitError::UnsafeStore)
        );
        let public = format!("{}", ProfileMutationCommitError::StoreIo);
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
        fs::remove_dir_all(root).unwrap();
    }
}
