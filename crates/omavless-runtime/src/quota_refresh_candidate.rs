// SPDX-License-Identifier: MIT

//! Executable design for ONE feed/usage candidate. Compiled only by tests.
//! This has no production entry point, network call or IPC. The nested
//! transaction tests use the existing atomic writer on synthetic temp stores.

mod batch;
mod transaction;

use crate::subscription_transport::FetchedSubscription;
use omavless_domain::private_store::{
    PrivateStoreError, SubscriptionRefreshCounts, SubscriptionRefreshSnapshot,
    apply_subscription_refresh, prepare_subscription_refresh,
};
use omavless_domain::subscription_feed::{SubscriptionFeedError, decode_subscription_feed};
use omavless_domain::subscription_usage_store::{
    PrivateUsageCandidate, UsageStoreError, bind_usage_to_refresh_candidate, read_provider_usage,
};

#[derive(Clone, Copy, Default)]
enum Retention {
    /// Safe default while old-runtime physical-erasure policy is unresolved.
    #[default]
    Discard,
    /// Synthetic branch for testing a proposed future private-store policy.
    /// This variant is not authorization or a production capability.
    ModelPrivatePersistence,
}

struct Snapshot {
    id: String,
    refresh: SubscriptionRefreshSnapshot,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Store(PrivateStoreError),
    Usage(UsageStoreError),
    Feed(SubscriptionFeedError),
}

impl Snapshot {
    fn capture(input: &str, id: &str) -> Result<Self, Error> {
        // Run strict raw-JSON admission BEFORE the legacy parser can collapse
        // duplicate members. Any optional claim is intentionally discarded.
        read_provider_usage(input, id).map_err(Error::Usage)?;
        Ok(Self {
            id: id.to_owned(),
            refresh: prepare_subscription_refresh(input, id).map_err(Error::Store)?,
        })
    }
}

/// Private result has no formatting traits. A future owner must prepare this
/// under its existing lease and pass only this complete payload to its existing
/// compensated commit. The intermediate feed candidate never escapes here.
struct Candidate {
    payload: PrivateUsageCandidate,
    counts: SubscriptionRefreshCounts,
}

fn compose(
    latest: &str,
    snapshot: Snapshot,
    fetched: FetchedSubscription,
    now_millis: u64,
    observed_seconds: u64,
    retention: Retention,
) -> Result<Candidate, Error> {
    read_provider_usage(latest, &snapshot.id).map_err(Error::Usage)?;
    let url = snapshot.refresh.private_url().to_owned();
    // Clear the target's old claim in memory BEFORE feed normalization. It
    // must not consume the size budget of an otherwise acceptable feed.
    // The caller's original bytes remain the compare/rollback baseline.
    let mut clean: serde_json::Value =
        serde_json::from_str(latest).map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
    if let Some(record) = clean["subscriptions"]
        .as_array_mut()
        .and_then(|items| items.iter_mut().find(|item| item["id"] == snapshot.id))
        .and_then(serde_json::Value::as_object_mut)
    {
        record.remove("providerUsageV1");
    }
    let feed = decode_subscription_feed(fetched.body).map_err(Error::Feed)?;
    let skipped = feed.counts().skipped;
    let mut next_id = 0;
    let entries = feed.into_private_entries(&mut || {
        next_id += 1;
        format!("10000000-0000-4000-8000-{next_id:012}")
    });
    // IDs above are deliberately synthetic, another reason this module is
    // test-only. Production must use the existing trusted owner ID generator.
    let (feed_candidate, counts) = apply_subscription_refresh(
        &clean.to_string(),
        snapshot.refresh,
        entries,
        now_millis,
        skipped,
    )
    .map_err(Error::Store)?;
    let text = std::str::from_utf8(feed_candidate.payload())
        .map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
    // Use the actual monotonic result, not wall time (which can move back).
    let document: serde_json::Value =
        serde_json::from_str(text).map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
    let token = document["subscriptions"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["id"] == snapshot.id))
        .and_then(|item| item["updatedAt"].as_u64())
        .ok_or(Error::Usage(UsageStoreError::SubscriptionChanged))?;
    let usage = match retention {
        Retention::Discard => None,
        Retention::ModelPrivatePersistence => fetched.usage,
    };
    let bind = |usage| {
        bind_usage_to_refresh_candidate(text, &snapshot.id, &url, token, observed_seconds, usage)
    };
    let payload = match bind(usage) {
        Ok(payload) => payload,
        // Optional metadata never rejects a usable feed due to clock or
        // metadata growth. Clearing also erases a previously retained claim.
        Err(UsageStoreError::InvalidObservation | UsageStoreError::TooLarge) => {
            bind(None).map_err(Error::Usage)?
        }
        Err(error) => return Err(Error::Usage(error)),
    };
    Ok(Candidate { payload, counts })
}

const ID: &str = "20000000-0000-4000-8000-000000000001";
const URL: &str = "https://example.invalid/synthetic";
const URI: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic";

fn store() -> String {
    serde_json::json!({"version":3,"profiles":[],"subscriptions":[{
        "id":ID,"name":"Synthetic","url":URL,"updatedAt":10
    }],"extension":{"keep":true}})
    .to_string()
}

fn fetched(with_usage: bool) -> FetchedSubscription {
    FetchedSubscription {
        body: omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
            URI.as_bytes().to_vec(),
        )
        .unwrap(),
        usage: with_usage.then_some(omavless_domain::subscription_metadata::SubscriptionUsage {
            upload_bytes: 12,
            download_bytes: 34,
            total_bytes: 100,
            expiry_unix_seconds: None,
        }),
    }
}

fn text(candidate: &Candidate) -> &str {
    std::str::from_utf8(candidate.payload.payload()).unwrap()
}

#[test]
fn one_candidate_contains_feed_and_usage_with_actual_monotonic_token() {
    let original = store();
    let candidate = compose(
        &original,
        Snapshot::capture(&original, ID).unwrap(),
        fetched(true),
        1,
        100,
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    assert_eq!(candidate.counts.added, 1);
    let document: serde_json::Value = serde_json::from_str(text(&candidate)).unwrap();
    assert_eq!(document["subscriptions"][0]["updatedAt"], 11);
    assert_eq!(
        document["subscriptions"][0]["providerUsageV1"]["refreshToken"],
        11
    );
    assert_eq!(document["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(document["extension"]["keep"], true);
    assert!(read_provider_usage(text(&candidate), ID).unwrap().is_some());
    assert_eq!(original, store());
}

#[test]
fn stale_url_token_or_deleted_subscription_never_returns_partial_payload() {
    for field in ["url", "updatedAt", "deleted"] {
        let original = store();
        let mut changed: serde_json::Value = serde_json::from_str(&original).unwrap();
        match field {
            "url" => changed["subscriptions"][0]["url"] = "https://other.invalid/feed".into(),
            "updatedAt" => changed["subscriptions"][0]["updatedAt"] = 11.into(),
            _ => changed["subscriptions"] = serde_json::json!([]),
        }
        assert!(matches!(
            compose(
                &changed.to_string(),
                Snapshot::capture(&original, ID).unwrap(),
                fetched(true),
                100,
                100,
                Retention::ModelPrivatePersistence
            ),
            Err(Error::Store(PrivateStoreError::SubscriptionChanged))
        ));
    }
}

#[test]
fn absent_invalid_or_disallowed_usage_clears_only_after_successful_feed() {
    let original = store();
    let prior = compose(
        &original,
        Snapshot::capture(&original, ID).unwrap(),
        fetched(true),
        20,
        100,
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    for (with_usage, observed, retention) in [
        (false, 100, Retention::ModelPrivatePersistence),
        (true, 0, Retention::ModelPrivatePersistence),
        (true, 100, Retention::default()),
    ] {
        let prior_text = text(&prior);
        let next = compose(
            prior_text,
            Snapshot::capture(prior_text, ID).unwrap(),
            fetched(with_usage),
            30,
            observed,
            retention,
        )
        .unwrap();
        assert_eq!(next.counts.total, 1);
        assert!(!text(&next).contains("providerUsageV1"));
        assert!(read_provider_usage(prior_text, ID).unwrap().is_some());
    }
    let mut invalid = fetched(true);
    invalid.body = omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
        b"synthetic-invalid-feed".to_vec(),
    )
    .unwrap();
    assert!(matches!(
        compose(
            text(&prior),
            Snapshot::capture(text(&prior), ID).unwrap(),
            invalid,
            30,
            100,
            Retention::Discard
        ),
        Err(Error::Feed(_))
    ));
    assert!(read_provider_usage(text(&prior), ID).unwrap().is_some());
}

#[test]
fn latest_rename_and_unrelated_changes_survive_but_duplicate_members_refuse() {
    let original = store();
    let mut latest: serde_json::Value = serde_json::from_str(&original).unwrap();
    latest["subscriptions"][0]["name"] = "Renamed".into();
    latest["extension"]["other"] = true.into();
    let candidate = compose(
        &latest.to_string(),
        Snapshot::capture(&original, ID).unwrap(),
        fetched(true),
        20,
        100,
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    let result: serde_json::Value = serde_json::from_str(text(&candidate)).unwrap();
    assert_eq!(result["subscriptions"][0]["name"], "Renamed");
    assert_eq!(result["extension"]["other"], true);
    let duplicate = original.replacen("\"version\":3", "\"version\":3,\"version\":3", 1);
    assert_ne!(duplicate, original);
    assert!(matches!(
        Snapshot::capture(&duplicate, ID),
        Err(Error::Usage(UsageStoreError::InvalidStore))
    ));
    assert!(matches!(
        compose(
            &duplicate,
            Snapshot::capture(&original, ID).unwrap(),
            fetched(true),
            20,
            100,
            Retention::ModelPrivatePersistence
        ),
        Err(Error::Usage(UsageStoreError::InvalidStore))
    ));
}

#[test]
fn old_runtime_retention_is_real_and_discard_physically_clears_next_candidate() {
    let original = store();
    let prior = compose(
        &original,
        Snapshot::capture(&original, ID).unwrap(),
        fetched(true),
        20,
        100,
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    // The existing body-only refresh models an old runtime preserving unknown
    // JSON extensions. Binding suppresses stale claims but does not erase bytes.
    let (old_refresh, _) = apply_subscription_refresh(
        text(&prior),
        prepare_subscription_refresh(text(&prior), ID).unwrap(),
        vec![],
        30,
        0,
    )
    .unwrap();
    let retained = std::str::from_utf8(old_refresh.payload()).unwrap();
    assert!(retained.contains("providerUsageV1"));
    assert!(read_provider_usage(retained, ID).unwrap().is_none());
    let next = compose(
        retained,
        Snapshot::capture(retained, ID).unwrap(),
        fetched(true),
        40,
        100,
        Retention::default(),
    )
    .unwrap();
    assert!(!text(&next).contains("providerUsageV1"));
}

#[test]
fn optional_metadata_growth_cannot_reject_a_feed_at_store_capacity() {
    use omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES;
    let mut document: serde_json::Value = serde_json::from_str(&store()).unwrap();
    document["padding"] = "".into();
    let base = document.to_string();
    let baseline = compose(
        &base,
        Snapshot::capture(&base, ID).unwrap(),
        fetched(false),
        20,
        100,
        Retention::Discard,
    )
    .unwrap();
    document["padding"] = "x"
        .repeat(MAX_PRIVATE_STORE_BYTES - text(&baseline).len() - 16)
        .into();
    let original = document.to_string();
    let candidate = compose(
        &original,
        Snapshot::capture(&original, ID).unwrap(),
        fetched(true),
        20,
        100,
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    assert_eq!(candidate.counts.total, 1);
    assert!(text(&candidate).len() <= MAX_PRIVATE_STORE_BYTES);
    assert!(!text(&candidate).contains("providerUsageV1"));
}
