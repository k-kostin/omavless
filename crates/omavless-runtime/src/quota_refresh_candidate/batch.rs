// SPDX-License-Identifier: MIT

//! Test-only all-member feed/usage composition. No worker or production caller.

use super::*;
use omavless_domain::private_store::{
    MAX_PRIVATE_STORE_BYTES, SubscriptionRefreshBatchEntries, SubscriptionRefreshBatchSnapshot,
    apply_subscription_refresh_batch, prepare_subscription_refresh_batch,
};

struct BatchSnapshot {
    refresh: SubscriptionRefreshBatchSnapshot,
    members: Vec<(String, String)>,
}

impl BatchSnapshot {
    fn capture(input: &str) -> Result<Self, Error> {
        // Empty ID is intentional: validate raw JSON even for an empty batch.
        read_provider_usage(input, "").map_err(Error::Usage)?;
        let refresh = prepare_subscription_refresh_batch(input).map_err(Error::Store)?;
        let root: serde_json::Value =
            serde_json::from_str(input).map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
        let members = root["subscriptions"]
            .as_array()
            .ok_or(Error::Store(PrivateStoreError::InvalidShape))?
            .iter()
            .map(|record| {
                Ok((
                    record["id"]
                        .as_str()
                        .ok_or(Error::Store(PrivateStoreError::InvalidShape))?
                        .to_owned(),
                    record["url"]
                        .as_str()
                        .ok_or(Error::Store(PrivateStoreError::InvalidShape))?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self { refresh, members })
    }
}

// No formatting, cloning or serialization of private payloads.
struct BatchCandidate {
    payload: Vec<u8>,
    counts: SubscriptionRefreshCounts,
    changed: bool,
}

fn compose_batch(
    latest: &str,
    snapshot: BatchSnapshot,
    fetched: Vec<FetchedSubscription>,
    now: impl FnOnce() -> u64,
    observed_seconds: u64,
    retention: Retention,
    next_id: &mut impl FnMut() -> String,
) -> Result<BatchCandidate, Error> {
    read_provider_usage(latest, "").map_err(Error::Usage)?;
    if fetched.len() != snapshot.refresh.len() {
        return Err(Error::Store(PrivateStoreError::SubscriptionChanged));
    }
    // The batch planner still verifies membership for empty snapshots; an
    // intervening add is not a successful empty no-op. No clock or ID is read.
    if snapshot.refresh.is_empty() {
        let (_, counts) = apply_subscription_refresh_batch(latest, snapshot.refresh, vec![], 0)
            .map_err(Error::Store)?;
        return Ok(BatchCandidate {
            payload: latest.as_bytes().to_vec(),
            counts,
            changed: false,
        });
    }
    let mut clean: serde_json::Value =
        serde_json::from_str(latest).map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
    for record in clean["subscriptions"]
        .as_array_mut()
        .ok_or(Error::Store(PrivateStoreError::InvalidShape))?
    {
        record
            .as_object_mut()
            .ok_or(Error::Store(PrivateStoreError::InvalidShape))?
            .remove("providerUsageV1");
    }
    let mut updates = Vec::with_capacity(fetched.len());
    let mut usages = Vec::with_capacity(fetched.len());
    let mut retained_bytes = 0_usize;
    for fetched in fetched {
        let feed = decode_subscription_feed(fetched.body).map_err(Error::Feed)?;
        retained_bytes = retained_bytes
            .checked_add(feed.private_payload_bytes())
            .filter(|bytes| *bytes <= MAX_PRIVATE_STORE_BYTES)
            .ok_or(Error::Store(PrivateStoreError::TooLarge))?;
        let skipped = feed.counts().skipped;
        updates.push(SubscriptionRefreshBatchEntries {
            entries: feed.into_private_entries(&mut *next_id),
            skipped,
        });
        usages.push(match retention {
            Retention::Discard => None,
            Retention::ModelPrivatePersistence => fetched.usage,
        });
    }
    let (feed_candidate, counts) =
        apply_subscription_refresh_batch(&clean.to_string(), snapshot.refresh, updates, now())
            .map_err(Error::Store)?;
    let feed_bytes = feed_candidate.payload().to_vec();
    let root: serde_json::Value = serde_json::from_slice(&feed_bytes)
        .map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
    let mut payload = feed_bytes.clone();
    for ((id, url), usage) in snapshot.members.iter().zip(usages) {
        let token = root["subscriptions"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["id"] == *id))
            .and_then(|record| record["updatedAt"].as_u64())
            .ok_or(Error::Usage(UsageStoreError::SubscriptionChanged))?;
        let text = std::str::from_utf8(&payload)
            .map_err(|_| Error::Usage(UsageStoreError::InvalidStore))?;
        let bind =
            |usage| bind_usage_to_refresh_candidate(text, id, url, token, observed_seconds, usage);
        match bind(usage) {
            Ok(candidate) => payload = candidate.payload().to_vec(),
            Err(UsageStoreError::InvalidObservation) => {
                payload = bind(None).map_err(Error::Usage)?.payload().to_vec()
            }
            // Deterministic whole-batch fallback: no provider gets priority
            // because its metadata happened to be composed first.
            Err(UsageStoreError::TooLarge) => {
                return Ok(BatchCandidate {
                    payload: feed_bytes,
                    counts,
                    changed: true,
                });
            }
            Err(error) => return Err(Error::Usage(error)),
        }
    }
    Ok(BatchCandidate {
        payload,
        counts,
        changed: true,
    })
}

const SECOND: &str = "20000000-0000-4000-8000-000000000002";

fn batch_store() -> String {
    let mut root: serde_json::Value = serde_json::from_str(&store()).unwrap();
    root["subscriptions"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": SECOND, "name":"Second", "url":"https://example.invalid/second", "updatedAt":50
        }));
    root.to_string()
}

fn responses() -> Vec<FetchedSubscription> {
    let mut second = fetched(true);
    second.body = omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
        URI.replace("192.0.2.1", "192.0.2.2").into_bytes(),
    )
    .unwrap();
    second.usage.as_mut().unwrap().upload_bytes = 55;
    vec![fetched(true), second]
}

fn build(
    latest: &str,
    captured: &str,
    fetched: Vec<FetchedSubscription>,
    retention: Retention,
) -> Result<BatchCandidate, Error> {
    let mut counter = 0;
    compose_batch(
        latest,
        BatchSnapshot::capture(captured)?,
        fetched,
        || 20,
        100,
        retention,
        &mut || {
            counter += 1;
            format!("10000000-0000-4000-8000-{counter:012}")
        },
    )
}

fn contents(candidate: &BatchCandidate) -> &str {
    std::str::from_utf8(&candidate.payload).unwrap()
}

#[test]
fn every_feed_and_distinct_claim_share_the_actual_batch_token() {
    let input = batch_store();
    let candidate = build(
        &input,
        &input,
        responses(),
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    assert_eq!(candidate.counts.added, 2);
    let root: serde_json::Value = serde_json::from_slice(&candidate.payload).unwrap();
    for record in root["subscriptions"].as_array().unwrap() {
        assert_eq!(record["updatedAt"], 51);
        assert_eq!(record["providerUsageV1"]["refreshToken"], 51);
    }
    for (id, expected) in [(ID, 12), (SECOND, 55)] {
        assert_eq!(
            read_provider_usage(contents(&candidate), id)
                .unwrap()
                .unwrap()
                .usage()
                .upload_bytes,
            expected
        );
    }
    let profiles = root["profiles"].as_array().unwrap();
    assert_ne!(profiles[0]["id"], profiles[1]["id"]);
}

#[test]
fn missing_invalid_and_default_discard_clear_old_claims_without_losing_feeds() {
    let input = batch_store();
    let old = build(
        &input,
        &input,
        responses(),
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    for invalid in [false, true] {
        let mut fetched = responses();
        if invalid {
            fetched[1].usage.as_mut().unwrap().expiry_unix_seconds = Some(u64::MAX);
        } else {
            fetched[1].usage = None;
        }
        let new = build(
            contents(&old),
            contents(&old),
            fetched,
            Retention::ModelPrivatePersistence,
        )
        .unwrap();
        assert_eq!(new.counts.total, 2);
        assert!(read_provider_usage(contents(&new), ID).unwrap().is_some());
        assert!(
            read_provider_usage(contents(&new), SECOND)
                .unwrap()
                .is_none()
        );
        assert!(
            !serde_json::from_slice::<serde_json::Value>(&new.payload).unwrap()["subscriptions"][1]
                .as_object()
                .unwrap()
                .contains_key("providerUsageV1")
        );
    }
    let discarded = build(
        contents(&old),
        contents(&old),
        responses(),
        Retention::default(),
    )
    .unwrap();
    assert!(!contents(&discarded).contains("providerUsageV1"));
}

#[test]
fn any_stale_member_or_incomplete_feed_set_refuses_the_whole_candidate() {
    let input = batch_store();
    for change in ["url", "token", "delete", "add", "reorder"] {
        let mut root: serde_json::Value = serde_json::from_str(&input).unwrap();
        match change {
            "url" => root["subscriptions"][1]["url"] = "https://example.invalid/replaced".into(),
            "token" => root["subscriptions"][1]["updatedAt"] = 51.into(),
            "delete" => {
                root["subscriptions"].as_array_mut().unwrap().pop();
            }
            "add" => {
                let mut third = root["subscriptions"][1].clone();
                third["id"] = "20000000-0000-4000-8000-000000000003".into();
                root["subscriptions"].as_array_mut().unwrap().push(third);
            }
            _ => root["subscriptions"].as_array_mut().unwrap().reverse(),
        }
        assert!(build(&root.to_string(), &input, responses(), Retention::Discard).is_err());
    }
    for extra in [false, true] {
        let mut feeds = responses();
        if extra {
            feeds.push(fetched(true));
        } else {
            feeds.pop();
        }
        assert!(build(&input, &input, feeds, Retention::Discard).is_err());
    }
    let mut feeds = responses();
    feeds[1].body = omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
        b"invalid-feed".to_vec(),
    )
    .unwrap();
    assert!(build(&input, &input, feeds, Retention::Discard).is_err());
    let duplicate = input.replacen("\"version\":3", "\"version\":3,\"version\":3", 1);
    assert!(BatchSnapshot::capture(&duplicate).is_err());
    assert!(build(&duplicate, &input, responses(), Retention::Discard).is_err());
}

#[test]
fn rename_and_unrelated_latest_changes_survive() {
    let input = batch_store();
    let mut latest: serde_json::Value = serde_json::from_str(&input).unwrap();
    latest["subscriptions"][1]["name"] = "Renamed".into();
    latest["extension"]["new"] = true.into();
    let result = build(
        &latest.to_string(),
        &input,
        responses(),
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&result.payload).unwrap();
    assert_eq!(root["subscriptions"][1]["name"], "Renamed");
    assert_eq!(root["extension"]["new"], true);
}

#[test]
fn aggregate_metadata_growth_falls_back_to_the_complete_feed_only_candidate() {
    let mut root: serde_json::Value = serde_json::from_str(&batch_store()).unwrap();
    root["padding"] = "".into();
    let base = root.to_string();
    let plain = build(&base, &base, responses(), Retention::Discard).unwrap();
    let with = build(
        &base,
        &base,
        responses(),
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    let growth = with.payload.len() - plain.payload.len();
    // Enough room for one claim, but not all: the entire metadata set drops.
    root["padding"] = "x"
        .repeat(MAX_PRIVATE_STORE_BYTES - plain.payload.len() - growth * 3 / 4)
        .into();
    let input = root.to_string();
    let result = build(
        &input,
        &input,
        responses(),
        Retention::ModelPrivatePersistence,
    )
    .unwrap();
    let expected = build(&input, &input, responses(), Retention::Discard).unwrap();
    assert!(result.payload == expected.payload);
    assert_eq!(result.counts.total, 2);
    assert!(!contents(&result).contains("providerUsageV1"));
}

#[test]
fn empty_batch_preserves_exact_bytes_without_clock_ids_or_write_intent() {
    let input = "\n {\"version\":3,\"profiles\":[],\"subscriptions\":[]}\n";
    let result = compose_batch(
        input,
        BatchSnapshot::capture(input).unwrap(),
        vec![],
        || panic!("clock called"),
        0,
        Retention::Discard,
        &mut || panic!("ID called"),
    )
    .unwrap();
    assert!(!result.changed);
    assert!(result.payload == input.as_bytes());
    assert!(
        compose_batch(
            &batch_store(),
            BatchSnapshot::capture(input).unwrap(),
            vec![],
            || panic!("clock called"),
            0,
            Retention::Discard,
            &mut || panic!("ID called")
        )
        .is_err()
    );
}

mod transaction_tests {
    use super::*;
    use crate::cutover::{CutoverPaths, MigrationLock};
    use crate::private_store_transaction::{PreparedWrite, prepare_private_store_write};
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    #[test]
    fn one_real_publication_restores_every_claim_and_refuses_unknown_bytes() {
        let root = crate::test_temp::directory("quota-batch").unwrap();
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
        let input = batch_store();
        let prior = build(
            &input,
            &input,
            responses(),
            Retention::ModelPrivatePersistence,
        )
        .unwrap();
        let original = format!("\n  {}\n", contents(&prior));
        fs::write(&path, &original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let prepared = prepare_private_store_write(&path, uid, |latest| {
            let candidate = build(latest, &original, responses(), Retention::Discard)
                .map_err(|_| PrivateStoreError::InvalidShape)?;
            Ok((candidate.payload, candidate.changed))
        })
        .unwrap();
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert!(
            !fs::read_to_string(&path)
                .unwrap()
                .contains("providerUsageV1")
        );
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert!(prepared.commit_locked(&lock, &paths).is_err());
        // Model downstream rejection after publication, then exact restoration.
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert!(fs::read(&path).unwrap() == original.as_bytes());
        assert!(prepared.verify_outcome_locked(&lock, &paths, true).is_ok());
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        fs::write(&path, b"unknown concurrent bytes").unwrap();
        assert!(prepared.restore_locked(&lock, &paths).is_err());
        assert!(fs::read(&path).unwrap() == b"unknown concurrent bytes");
        drop(lock);
        fs::remove_dir_all(root).unwrap();
    }
}
