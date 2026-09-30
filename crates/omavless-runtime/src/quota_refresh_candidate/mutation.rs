// SPDX-License-Identifier: MIT

//! Test-only subscription URL mutation and quota erasure composition. No
//! production mutation path invokes this adapter.

use super::*;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::private_store_transaction::{PreparedWrite, prepare_private_store_write};
use omavless_domain::private_store::{
    SubscriptionMutation, SubscriptionMutationContext, apply_subscription_mutation,
};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

const REPLACEMENT_URL: &str = "https://replacement.invalid/synthetic";

fn prior_claim() -> String {
    let original = store();
    let candidate = compose(
        &original,
        Snapshot::capture(&original, ID).unwrap(),
        fetched(true),
        10,
        100,
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    text(&candidate).to_owned()
}

/// Model the already supported body-only URL edit. It preserves unknown
/// subscription fields; that is useful compatibility behavior but is not a
/// physical-erasure guarantee for future private quota data.
fn body_only_edit(input: &str, url: &str, token: u64) -> Vec<u8> {
    apply_subscription_mutation(
        input,
        SubscriptionMutation::Update {
            subscription_id: ID.to_owned(),
            name: "Synthetic".to_owned(),
            url: url.to_owned(),
            entries: vec![],
            updated_at: token,
        },
        SubscriptionMutationContext::default(),
    )
    .unwrap()
    .payload()
    .to_vec()
}

/// Future owner composition: use the accepted complete mutation candidate,
/// then remove the old claim before its ONE atomic private-store publication.
/// This is deliberately test-only until downgrade and owner gates are decided.
fn edit_with_erasure(input: &str, url: &str, token: u64) -> Vec<u8> {
    let updated = body_only_edit(input, url, token);
    let updated = std::str::from_utf8(&updated).unwrap();
    bind_usage_to_refresh_candidate(updated, ID, url, token, 0, None)
        .unwrap()
        .payload()
        .to_vec()
}

fn delete_subscription(input: &str, active_service: bool) -> Result<Vec<u8>, PrivateStoreError> {
    apply_subscription_mutation(
        input,
        SubscriptionMutation::Delete {
            subscription_id: ID.to_owned(),
        },
        SubscriptionMutationContext { active_service },
    )
    .map(|mutation| mutation.payload().to_vec())
}

#[test]
fn old_body_only_url_roundtrip_can_reexpose_stale_account_claim() {
    let prior = prior_claim();
    let token =
        serde_json::from_str::<serde_json::Value>(&prior).unwrap()["subscriptions"][0]["updatedAt"]
            .as_u64()
            .unwrap();
    let replaced = body_only_edit(&prior, REPLACEMENT_URL, token);
    let replaced = std::str::from_utf8(&replaced).unwrap();
    assert!(replaced.contains("providerUsageV1"));
    assert!(read_provider_usage(replaced, ID).unwrap().is_none());
    // A same-millisecond update (or clock rollback) can reuse the token.
    // Digest binding hides the claim temporarily, but cannot revoke its bytes.
    let returned = body_only_edit(replaced, URL, token);
    let returned = std::str::from_utf8(&returned).unwrap();
    assert!(read_provider_usage(returned, ID).unwrap().is_some());
}

#[test]
fn modeled_url_edit_physically_erases_claim_even_after_roundtrip() {
    let prior = prior_claim();
    let token =
        serde_json::from_str::<serde_json::Value>(&prior).unwrap()["subscriptions"][0]["updatedAt"]
            .as_u64()
            .unwrap();
    let replaced = edit_with_erasure(&prior, REPLACEMENT_URL, token);
    let replaced = std::str::from_utf8(&replaced).unwrap();
    assert!(!replaced.contains("providerUsageV1"));
    let returned = body_only_edit(replaced, URL, token);
    let returned = std::str::from_utf8(&returned).unwrap();
    assert!(read_provider_usage(returned, ID).unwrap().is_none());
    assert!(!returned.contains("providerUsageV1"));
}

#[test]
fn modeled_url_edit_uses_one_atomic_publication_and_exact_rollback() {
    let root = crate::test_temp::directory("quota-url-mutation").unwrap();
    let runtime = root.join("runtime");
    let state = root.join("state");
    for directory in [&root, &runtime, &state] {
        fs::create_dir_all(directory).unwrap();
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let uid = fs::metadata(&root).unwrap().uid();
    let paths = CutoverPaths::below(&runtime, &state, uid);
    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    let path = root.join("profiles.json");
    let original = format!("\n  {}\n", prior_claim());
    fs::write(&path, &original).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let token = serde_json::from_str::<serde_json::Value>(&original).unwrap()["subscriptions"][0]
        ["updatedAt"]
        .as_u64()
        .unwrap();
    let prepared = prepare_private_store_write(&path, uid, |latest| {
        Ok((edit_with_erasure(latest, REPLACEMENT_URL, token), true))
    })
    .unwrap();
    assert_eq!(
        prepared.commit_locked(&lock, &paths),
        Ok(PreparedWrite::Changed)
    );
    let published = fs::read_to_string(&path).unwrap();
    assert!(!published.contains("providerUsageV1"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&published).unwrap()["subscriptions"][0]["url"],
        REPLACEMENT_URL
    );
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    // Model downstream rejection: restore the original bytes, not a normalized
    // or already-scrubbed baseline. This is not a live owner completion callback.
    assert_eq!(
        prepared.restore_locked(&lock, &paths),
        Ok(PreparedWrite::Changed)
    );
    assert!(fs::read(&path).unwrap() == original.as_bytes());
    drop(lock);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_subscription_erases_claim_and_readding_same_identity_cannot_revive_it() {
    let prior = prior_claim();
    let token =
        serde_json::from_str::<serde_json::Value>(&prior).unwrap()["subscriptions"][0]["updatedAt"]
            .as_u64()
            .unwrap();
    let deleted = delete_subscription(&prior, false).unwrap();
    let deleted = std::str::from_utf8(&deleted).unwrap();
    assert!(!deleted.contains("providerUsageV1"));
    assert!(read_provider_usage(deleted, ID).unwrap().is_none());
    let document: serde_json::Value = serde_json::from_str(deleted).unwrap();
    assert_eq!(document["subscriptions"].as_array().unwrap().len(), 0);
    assert_eq!(document["profiles"].as_array().unwrap().len(), 0);
    assert_eq!(document["extension"]["keep"], true);

    let readded = apply_subscription_mutation(
        deleted,
        SubscriptionMutation::Add {
            subscription_id: ID.to_owned(),
            name: "Synthetic".to_owned(),
            url: URL.to_owned(),
            entries: vec![],
            updated_at: token,
        },
        SubscriptionMutationContext::default(),
    )
    .unwrap();
    let readded = std::str::from_utf8(readded.payload()).unwrap();
    assert!(!readded.contains("providerUsageV1"));
    assert!(read_provider_usage(readded, ID).unwrap().is_none());
}

#[test]
fn active_subscription_delete_refuses_before_private_claim_publication() {
    let mut value: serde_json::Value = serde_json::from_str(&prior_claim()).unwrap();
    value["activeId"] = value["profiles"][0]["id"].clone();
    let prior = value.to_string();
    assert!(read_provider_usage(&prior, ID).unwrap().is_some());
    assert!(matches!(
        delete_subscription(&prior, true),
        Err(PrivateStoreError::ActiveSubscription)
    ));
    assert!(read_provider_usage(&prior, ID).unwrap().is_some());
}

#[test]
fn deleting_subscription_uses_one_atomic_publication_and_exact_rollback() {
    let root = crate::test_temp::directory("quota-delete-mutation").unwrap();
    let runtime = root.join("runtime");
    let state = root.join("state");
    for directory in [&root, &runtime, &state] {
        fs::create_dir_all(directory).unwrap();
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let uid = fs::metadata(&root).unwrap().uid();
    let paths = CutoverPaths::below(&runtime, &state, uid);
    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    let path = root.join("profiles.json");
    let original = format!("\n  {}\n", prior_claim());
    fs::write(&path, &original).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let prepared = prepare_private_store_write(&path, uid, |latest| {
        Ok((delete_subscription(latest, false)?, true))
    })
    .unwrap();
    assert_eq!(
        prepared.commit_locked(&lock, &paths),
        Ok(PreparedWrite::Changed)
    );
    let published = fs::read_to_string(&path).unwrap();
    assert!(!published.contains("providerUsageV1"));
    assert!(read_provider_usage(&published, ID).unwrap().is_none());
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(
        prepared.restore_locked(&lock, &paths),
        Ok(PreparedWrite::Changed)
    );
    assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
    drop(lock);
    fs::remove_dir_all(root).unwrap();
}
