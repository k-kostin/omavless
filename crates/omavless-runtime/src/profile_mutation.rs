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
    pub(crate) fn planned_output(&self) -> &[u8] {
        self.prepared.planned_output()
    }
    #[cfg(test)]
    pub(crate) fn research_matches_candidate(&self, bytes: &[u8]) -> bool {
        self.prepared.research_matches_candidate(bytes)
    }
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

pub(crate) fn prepare_subscription_schedule(
    store_path: &Path,
    uid: u32,
    id: &str,
    enabled: bool,
    now: u64,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        omavless_domain::private_store::apply_subscription_schedule(input, id, enabled, now)
    })?;
    Ok(PreparedProfileMutation { prepared })
}

/// Validate and render a complete replacement without changing the store.
pub fn prepare_profile_mutation(
    store_path: &Path,
    uid: u32,
    mutation: ProfileMutation,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        if let ProfileMutation::Replace { new_input, .. } = &mutation
            && !crate::profile_import_protocol::admitted_profile_input(new_input)
        {
            return Err(omavless_domain::private_store::PrivateStoreError::TypedCredentialRequired);
        }
        let result = apply_profile_mutation(input, mutation)?;
        Ok((result.payload().to_vec(), result.changed))
    })?;
    Ok(PreparedProfileMutation { prepared })
}

pub(crate) fn prepare_profile_import(
    store_path: &Path,
    uid: u32,
    profile_id: &str,
    name: &str,
    profile_input: &str,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        if !crate::profile_import_protocol::admitted_profile_input(profile_input) {
            return Err(omavless_domain::private_store::PrivateStoreError::TypedCredentialRequired);
        }
        let result = omavless_domain::private_store::apply_profile_import(
            input,
            profile_id,
            name,
            profile_input,
        )?;
        Ok((result.payload().to_vec(), result.changed))
    })?;
    Ok(PreparedProfileMutation { prepared })
}

/// Prepare a local snapshot as one existing guarded store transaction. This
/// does not register an IPC method, fetch a subscription, or change VPN state.
/// The future serialized owner supplies record IDs and confirmation fencing.
pub fn prepare_local_bundle(
    store_path: &Path,
    uid: u32,
    input: &str,
    next_record_id: impl FnMut() -> String,
) -> Result<
    (
        PreparedProfileMutation,
        omavless_domain::private_store::LocalBundleCounts,
    ),
    ProfileMutationCommitError,
> {
    prepare_local_bundle_inner(store_path, uid, input, next_record_id, None)
}

pub(crate) fn prepare_confirmed_local_bundle(
    store_path: &Path,
    uid: u32,
    input: &str,
    next_record_id: impl FnMut() -> String,
    expected: &str,
) -> Result<
    (
        PreparedProfileMutation,
        omavless_domain::private_store::LocalBundleCounts,
    ),
    ProfileMutationCommitError,
> {
    prepare_local_bundle_inner(store_path, uid, input, next_record_id, Some(expected))
}

fn prepare_local_bundle_inner(
    store_path: &Path,
    uid: u32,
    input: &str,
    next_record_id: impl FnMut() -> String,
    expected: Option<&str>,
) -> Result<
    (
        PreparedProfileMutation,
        omavless_domain::private_store::LocalBundleCounts,
    ),
    ProfileMutationCommitError,
> {
    let mut counts = None;
    let mut mismatch = false;
    let prepared = prepare_private_store_write(store_path, uid, |original| {
        if expected.is_some_and(|value| {
            crate::profile_bundle_protocol::fingerprint(original, input) != value
        }) {
            mismatch = true;
            return Err(omavless_domain::private_store::PrivateStoreError::InvalidShape);
        }
        let plan = omavless_domain::private_store::parse_private_store(original)?
            .plan_local_bundle(input)?;
        counts = Some(plan.counts());
        let result = plan.into_mutation(next_record_id)?;
        Ok((result.payload().to_vec(), result.changed))
    })
    .map_err(|error| {
        if mismatch {
            ProfileMutationCommitError::StoreChanged
        } else {
            error
        }
    })?;
    Ok((
        PreparedProfileMutation { prepared },
        counts.expect("successful preparation invokes planner"),
    ))
}

pub(crate) fn prepare_startup_preferences(
    store_path: &Path,
    uid: u32,
    preferences: &omavless_domain::private_store::StartupPreferences,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        if preferences.enabled {
            let store = omavless_domain::private_store::parse_private_store(input)?;
            let selected = store.resolve_startup_selection(preferences)?;
            store.managed_connection_compatibility(&selected)?;
        }
        omavless_domain::private_store::apply_startup_preferences(input, preferences)
    })?;
    Ok(PreparedProfileMutation { prepared })
}

pub(crate) fn prepare_onboarding_completion(
    store_path: &Path,
    uid: u32,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(
        store_path,
        uid,
        omavless_domain::private_store::complete_onboarding,
    )?;
    Ok(PreparedProfileMutation { prepared })
}

pub(crate) fn prepare_custom_rule_mutation(
    store_path: &Path,
    uid: u32,
    mutation: omavless_domain::private_store::CustomRuleMutation,
    generated_id: &str,
) -> Result<PreparedProfileMutation, ProfileMutationCommitError> {
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        let result = omavless_domain::private_store::apply_custom_rule_mutation(
            input,
            mutation,
            generated_id,
        )?;
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

    #[test]
    fn ordinary_native_import_and_replace_prepare_without_any_store_bytes_change() {
        let (root, uid) = root("wg-inactive");
        let path = root.join("profiles.json");
        write_store(&path);
        let before = fs::read(&path).unwrap();
        let native = "[Interface]\nPrivateKey = BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\nAddress = 10.8.0.2/32\n[Peer]\nPublicKey = CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\n";
        assert!(
            prepare_profile_import(
                &path,
                uid,
                "00000000-0000-4000-8000-000000000002",
                "Native",
                native
            )
            .is_ok()
        );
        assert!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Replace {
                    profile_id: PROFILE.into(),
                    new_name: "Native".into(),
                    new_input: native.into()
                }
            )
            .is_ok()
        );
        assert!(fs::read(&path).unwrap() == before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_native_rows_cannot_arm_startup_but_data_and_off_cleanup_remain_possible() {
        let (root, uid) = root("wg-existing-off");
        let path = root.join("profiles.json");
        let id = "00000000-0000-4000-8000-000000000002";
        let native = "[Interface]\nPrivateKey = BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\nAddress = 10.8.0.2/32\n[Peer]\nPublicKey = CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\n";
        let input = omavless_domain::private_store::apply_profile_import(
            &store().to_string(),
            id,
            "Native",
            &native.replace("10.8.0.2/32", "10.8.0.2/32, fd00::2/128"),
        )
        .unwrap();
        fs::write(&path, input.payload()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let preferences = omavless_domain::private_store::StartupPreferences {
            enabled: true,
            target: "profile".into(),
            profile_id: id.into(),
            mode: "global".into(),
        };
        assert!(prepare_startup_preferences(&path, uid, &preferences).is_err());
        assert!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Replace {
                    profile_id: id.into(),
                    new_name: "URI".into(),
                    new_input: "trojan://synthetic@192.0.2.3:443".into()
                }
            )
            .is_ok()
        );
        let off = omavless_domain::private_store::StartupPreferences {
            enabled: false,
            ..preferences
        };
        assert!(prepare_startup_preferences(&path, uid, &off).is_ok());
        assert!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Favorite {
                    profile_id: id.into(),
                    enabled: true
                }
            )
            .is_ok()
        );
        assert!(
            prepare_profile_mutation(
                &path,
                uid,
                ProfileMutation::Delete {
                    profile_id: id.into()
                }
            )
            .is_ok()
        );
        assert!(fs::read(&path).unwrap() == input.payload());
        fs::remove_dir_all(root).unwrap();
    }

    fn locked(root: &Path, uid: u32) -> (CutoverPaths, MigrationLock) {
        let paths = CutoverPaths::below(root, root, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        (paths, lock)
    }

    #[test]
    fn local_bundle_prepares_without_writes_commits_once_and_restores_exactly() {
        let (root, uid) = root("bundle");
        let path = root.join("profiles.json");
        write_store(&path);
        let before = fs::read(&path).unwrap();
        let links = (2..=3).map(|i|format!("vless://11111111-1111-4111-8111-111111111111@192.0.2.{i}:443?security=none&type=tcp#Node {i}"))
            .collect::<Vec<_>>().join("\n");
        let mut id = 1;
        let (prepared, counts) = prepare_local_bundle(&path, uid, &links, || {
            id += 1;
            format!("00000000-0000-4000-8000-{id:012}")
        })
        .unwrap();
        assert_eq!(counts.accepted, 2);
        assert_eq!(fs::read(&path).unwrap(), before);
        let (paths, lock) = locked(&root, uid);
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        let after: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["profiles"].as_array().unwrap().len(), 3);
        assert_eq!(after["activeId"], PROFILE);
        assert_eq!(after["startup"], store()["startup"]);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_bundle_refuses_stale_store_and_invalid_ids_without_partial_write() {
        let (root, uid) = root("bundle-stale");
        let path = root.join("profiles.json");
        write_store(&path);
        let before = fs::read(&path).unwrap();
        let link =
            "vless://11111111-1111-4111-8111-111111111111@192.0.2.2:443?security=none&type=tcp#New";
        assert!(prepare_local_bundle(&path, uid, link, || PROFILE.into()).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        let (prepared, _) = prepare_local_bundle(&path, uid, link, || {
            "00000000-0000-4000-8000-000000000002".into()
        })
        .unwrap();
        let mut external = store();
        external["onboardingComplete"] = json!(false);
        let external = serde_json::to_vec(&external).unwrap();
        fs::write(&path, &external).unwrap();
        let (paths, lock) = locked(&root, uid);
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Err(ProfileMutationCommitError::StoreChanged)
        );
        assert_eq!(fs::read(&path).unwrap(), external);
        fs::remove_dir_all(root).unwrap();
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
