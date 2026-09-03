// SPDX-License-Identifier: MIT

//! Atomic private-store commit boundary for native subscription mutations.
//!
//! This module is deliberately absent from live IPC dispatch. Remote fetch and
//! profile parsing complete before this fixed store path is opened; a future
//! owner binding must also hold the migration lock and serialized mutation
//! slot before calling it.

use omavless_domain::private_store::{
    PrivateStoreError, SubscriptionMutation, SubscriptionMutationContext,
    SubscriptionMutationCounts, apply_subscription_mutation,
};
use omavless_store::{StoreIoError, atomic_replace_private, read_private_utf8};
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionMutationCommitError {
    UnsafeStore,
    StoreIo,
    Mutation(PrivateStoreError),
}

impl fmt::Display for SubscriptionMutationCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeStore => formatter.write_str("Private profile store path is unsafe"),
            Self::StoreIo => formatter.write_str("Private profile store update failed"),
            Self::Mutation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SubscriptionMutationCommitError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionMutationCommit {
    pub changed: bool,
    pub counts: SubscriptionMutationCounts,
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

fn map_io(_error: StoreIoError) -> SubscriptionMutationCommitError {
    SubscriptionMutationCommitError::StoreIo
}

/// Apply and atomically replace one existing `profiles.json`.
///
/// The replacement payload is completely rendered and bounded before the
/// first write. No client-selected path, shell, process or network operation
/// is accepted by this boundary. `context` must come from the canonical
/// owner's trusted host observation, never from request parameters. A first
/// installation must initialize the empty private store before calling this
/// existing-store mutation boundary.
pub fn commit_subscription_mutation(
    store_path: &Path,
    uid: u32,
    mutation: SubscriptionMutation,
    context: SubscriptionMutationContext,
) -> Result<SubscriptionMutationCommit, SubscriptionMutationCommitError> {
    if !store_path.is_absolute()
        || store_path.file_name().and_then(|name| name.to_str()) != Some("profiles.json")
        || !private_parent(store_path, uid)
        || !private_store_file(store_path, uid)
    {
        return Err(SubscriptionMutationCommitError::UnsafeStore);
    }
    let input = read_private_utf8(store_path, uid).map_err(map_io)?;
    let result = apply_subscription_mutation(&input, mutation, context)
        .map_err(SubscriptionMutationCommitError::Mutation)?;
    let commit = SubscriptionMutationCommit {
        changed: result.changed,
        counts: result.counts,
    };
    atomic_replace_private(store_path, result.payload(), uid).map_err(map_io)?;
    Ok(commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_domain::private_store::IncomingSubscriptionProfile;
    use serde_json::{Value, json};
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";
    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    const PRIVATE_URI: &str =
        "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Private";

    fn root(label: &str) -> (std::path::PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-subscription-mutation-{label}-{}-{nonce}",
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
            "lastId": "",
            "profiles": [],
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

    fn add() -> SubscriptionMutation {
        SubscriptionMutation::Add {
            subscription_id: SUBSCRIPTION.to_owned(),
            name: "Source".to_owned(),
            url: "https://provider.invalid/private-token".to_owned(),
            entries: vec![IncomingSubscriptionProfile {
                uri: PRIVATE_URI.to_owned(),
                new_id: PROFILE.to_owned(),
            }],
            updated_at: 1,
        }
    }

    #[test]
    fn successful_commit_is_private_atomic_and_complete() {
        let (root, uid) = root("success");
        let path = root.join("profiles.json");
        write_store(&path);
        let result =
            commit_subscription_mutation(&path, uid, add(), SubscriptionMutationContext::default())
                .unwrap();
        assert!(result.changed);
        assert_eq!(result.counts.added, 1);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let written: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["profiles"].as_array().unwrap().len(), 1);
        assert_eq!(written["subscriptions"].as_array().unwrap().len(), 1);
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
    fn rejected_mutation_and_unsafe_paths_never_change_destination() {
        let (root, uid) = root("rejected");
        let path = root.join("profiles.json");
        assert_eq!(
            commit_subscription_mutation(&path, uid, add(), SubscriptionMutationContext::default()),
            Err(SubscriptionMutationCommitError::UnsafeStore)
        );
        assert!(!path.exists());
        write_store(&path);
        let before = fs::read(&path).unwrap();
        let duplicate = SubscriptionMutation::Add {
            subscription_id: SUBSCRIPTION.to_owned(),
            name: "x".repeat(81),
            url: "https://provider.invalid/private-token".to_owned(),
            entries: Vec::new(),
            updated_at: 1,
        };
        assert_eq!(
            commit_subscription_mutation(
                &path,
                uid,
                duplicate,
                SubscriptionMutationContext::default()
            ),
            Err(SubscriptionMutationCommitError::Mutation(
                PrivateStoreError::InvalidName
            ))
        );
        assert_eq!(fs::read(&path).unwrap(), before);

        let destination = root.join("destination");
        fs::write(&destination, b"private.example/password").unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&destination, &path).unwrap();
        assert_eq!(
            commit_subscription_mutation(&path, uid, add(), SubscriptionMutationContext::default()),
            Err(SubscriptionMutationCommitError::UnsafeStore)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"private.example/password");
        fs::remove_file(&path).unwrap();
        write_store(&path);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            commit_subscription_mutation(&path, uid, add(), SubscriptionMutationContext::default()),
            Err(SubscriptionMutationCommitError::UnsafeStore)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            commit_subscription_mutation(&path, uid, add(), SubscriptionMutationContext::default()),
            Err(SubscriptionMutationCommitError::UnsafeStore)
        );
        let public = format!("{}", SubscriptionMutationCommitError::StoreIo);
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
        fs::remove_dir_all(root).unwrap();
    }
}
