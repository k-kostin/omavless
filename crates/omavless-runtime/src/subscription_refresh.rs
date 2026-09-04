// SPDX-License-Identifier: MIT

//! Offline orchestration seams for native subscription refresh.
//!
//! The refresh-all path in this module is deliberately not registered in IPC
//! or the live owner. Its store traits represent two distinct, short serialized
//! leases. Transport is called only after the snapshot lease returns and before
//! the commit lease is acquired, so slow network I/O can never be hidden inside
//! store mutation.

use crate::cutover::{CutoverError, CutoverPaths, MigrationLock};
use crate::subscription_mutation::{
    SubscriptionMutationCommitError, SubscriptionRefreshCommit, commit_subscription_refresh,
    commit_subscription_refresh_batch, snapshot_subscription_refresh,
    snapshot_subscription_refresh_batch,
};
pub use crate::subscription_transport::SubscriptionTransportError;
use omavless_domain::private_store::{
    IncomingSubscriptionProfile, MAX_PRIVATE_STORE_BYTES, SubscriptionRefreshBatchEntries,
    SubscriptionRefreshBatchSnapshot, SubscriptionRefreshCounts, SubscriptionRefreshSnapshot,
};
use omavless_domain::subscription_feed::{
    DecodedSubscriptionFeed, PrivateSubscriptionBody, SubscriptionFeedError,
    decode_subscription_feed,
};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionRefreshError {
    Store(SubscriptionMutationCommitError),
    Transport(SubscriptionTransportError),
    Feed(SubscriptionFeedError),
    AggregateTooLarge,
}

impl SubscriptionRefreshError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Store(SubscriptionMutationCommitError::UnsafeStore) => "unsafe_store",
            Self::Store(SubscriptionMutationCommitError::StoreIo) => "store_io",
            Self::Store(SubscriptionMutationCommitError::Busy) => "busy",
            Self::Store(SubscriptionMutationCommitError::UnsafeLock) => "unsafe_lock",
            Self::Store(SubscriptionMutationCommitError::Mutation(error)) => error.code(),
            Self::Transport(error) => error.code(),
            Self::Feed(error) => error.code(),
            Self::AggregateTooLarge => "subscription_refresh_all_too_large",
        }
    }
}

impl fmt::Display for SubscriptionRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
            Self::Feed(error) => error.fmt(formatter),
            Self::AggregateTooLarge => {
                formatter.write_str("Combined subscription refresh data is too large")
            }
        }
    }
}

impl std::error::Error for SubscriptionRefreshError {}

/// Two independently scoped store operations. Implementations must release
/// their serialization lease before returning from either method.
pub trait SubscriptionRefreshStore {
    fn snapshot(
        &mut self,
        subscription_id: &str,
    ) -> Result<SubscriptionRefreshSnapshot, SubscriptionMutationCommitError>;

    fn commit(
        &mut self,
        snapshot: SubscriptionRefreshSnapshot,
        entries: Vec<IncomingSubscriptionProfile>,
        updated_at: u64,
        skipped: usize,
    ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>;
}

/// Two short all-subscription store leases. Implementations must release the
/// first lease before any transport runs and acquire a fresh lease only for
/// the single final commit.
pub trait SubscriptionRefreshBatchStore {
    fn snapshot_batch(
        &mut self,
    ) -> Result<SubscriptionRefreshBatchSnapshot, SubscriptionMutationCommitError>;

    fn commit_batch(
        &mut self,
        snapshot: SubscriptionRefreshBatchSnapshot,
        updates: Vec<SubscriptionRefreshBatchEntries>,
        updated_at: u64,
    ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>;
}

/// Fixed-path existing private-store adapter. Each method independently takes
/// and releases the shared Python/Rust mutation lock. It performs no network,
/// IPC or lifecycle work.
pub struct ExistingPrivateSubscriptionStore {
    path: PathBuf,
    uid: u32,
    cutover_paths: CutoverPaths,
}

impl ExistingPrivateSubscriptionStore {
    #[must_use]
    pub fn new(path: &Path, uid: u32, cutover_paths: CutoverPaths) -> Self {
        Self {
            path: path.to_owned(),
            uid,
            cutover_paths,
        }
    }

    fn lease(&self) -> Result<MigrationLock, SubscriptionMutationCommitError> {
        MigrationLock::acquire(&self.cutover_paths, self.uid).map_err(|error| match error {
            CutoverError::Busy => SubscriptionMutationCommitError::Busy,
            CutoverError::UnsafeRuntimeDirectory => SubscriptionMutationCommitError::UnsafeLock,
            _ => SubscriptionMutationCommitError::StoreIo,
        })
    }
}

impl SubscriptionRefreshStore for ExistingPrivateSubscriptionStore {
    fn snapshot(
        &mut self,
        subscription_id: &str,
    ) -> Result<SubscriptionRefreshSnapshot, SubscriptionMutationCommitError> {
        let _lease = self.lease()?;
        snapshot_subscription_refresh(&self.path, self.uid, subscription_id)
    }

    fn commit(
        &mut self,
        snapshot: SubscriptionRefreshSnapshot,
        entries: Vec<IncomingSubscriptionProfile>,
        updated_at: u64,
        skipped: usize,
    ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError> {
        let _lease = self.lease()?;
        commit_subscription_refresh(&self.path, self.uid, snapshot, entries, updated_at, skipped)
    }
}

impl SubscriptionRefreshBatchStore for ExistingPrivateSubscriptionStore {
    fn snapshot_batch(
        &mut self,
    ) -> Result<SubscriptionRefreshBatchSnapshot, SubscriptionMutationCommitError> {
        let _lease = self.lease()?;
        snapshot_subscription_refresh_batch(&self.path, self.uid)
    }

    fn commit_batch(
        &mut self,
        snapshot: SubscriptionRefreshBatchSnapshot,
        updates: Vec<SubscriptionRefreshBatchEntries>,
        updated_at: u64,
    ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError> {
        let _lease = self.lease()?;
        commit_subscription_refresh_batch(&self.path, self.uid, snapshot, updates, updated_at)
    }
}

/// Execute one refresh with fully injected transport, ID and time sources.
/// The function has no real HTTP client and is unreachable from live dispatch.
pub fn refresh_subscription_offline<S, T, G, N>(
    store: &mut S,
    subscription_id: &str,
    mut transport: T,
    mut next_record_id: G,
    now_millis: N,
) -> Result<SubscriptionRefreshCommit, SubscriptionRefreshError>
where
    S: SubscriptionRefreshStore,
    T: FnMut(&str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError>,
    G: FnMut() -> String,
    N: FnOnce() -> u64,
{
    let snapshot = store
        .snapshot(subscription_id)
        .map_err(SubscriptionRefreshError::Store)?;
    let body = transport(snapshot.private_url()).map_err(SubscriptionRefreshError::Transport)?;
    let feed: DecodedSubscriptionFeed =
        decode_subscription_feed(body).map_err(SubscriptionRefreshError::Feed)?;
    let skipped = feed.counts().skipped;
    let entries = feed.into_private_entries(&mut next_record_id);
    store
        .commit(snapshot, entries, now_millis(), skipped)
        .map_err(SubscriptionRefreshError::Store)
}

fn add_batch_private_bytes(current: usize, next: usize) -> Result<usize, SubscriptionRefreshError> {
    current
        .checked_add(next)
        .filter(|size| *size <= MAX_PRIVATE_STORE_BYTES)
        .ok_or(SubscriptionRefreshError::AggregateTooLarge)
}

/// Fetch and decode every captured subscription, then commit all results or
/// none. This remains an offline/injected coordinator: it has no socket or CLI
/// registration and does not solve the future long-operation scheduling
/// contract. Only one feed body is retained at a time; decoded private URI
/// data across the batch is capped at the private-store size limit.
pub fn refresh_subscriptions_offline<S, T, G, N>(
    store: &mut S,
    mut transport: T,
    mut next_record_id: G,
    now_millis: N,
) -> Result<SubscriptionRefreshCommit, SubscriptionRefreshError>
where
    S: SubscriptionRefreshBatchStore,
    T: FnMut(&str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError>,
    G: FnMut() -> String,
    N: FnOnce() -> u64,
{
    let snapshot = store
        .snapshot_batch()
        .map_err(SubscriptionRefreshError::Store)?;
    if snapshot.is_empty() {
        return Ok(SubscriptionRefreshCommit {
            counts: SubscriptionRefreshCounts {
                added: 0,
                removed: 0,
                stale: 0,
                total: 0,
                skipped: 0,
            },
        });
    }

    let mut retained_private_bytes = 0usize;
    let mut updates = Vec::with_capacity(snapshot.len());
    for private_url in snapshot.private_urls() {
        let body = transport(private_url).map_err(SubscriptionRefreshError::Transport)?;
        let feed = decode_subscription_feed(body).map_err(SubscriptionRefreshError::Feed)?;
        retained_private_bytes =
            add_batch_private_bytes(retained_private_bytes, feed.private_payload_bytes())?;
        let skipped = feed.counts().skipped;
        updates.push(SubscriptionRefreshBatchEntries {
            entries: feed.into_private_entries(&mut next_record_id),
            skipped,
        });
    }
    store
        .commit_batch(snapshot, updates, now_millis())
        .map_err(SubscriptionRefreshError::Store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_domain::private_store::{
        PrivateStoreError, SubscriptionRefreshCounts, apply_subscription_refresh_batch,
        prepare_subscription_refresh, prepare_subscription_refresh_batch,
    };
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";
    const SECOND_SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000002";
    const URI: &str =
        "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example";
    const SECOND_URI: &str = "vless://22222222-2222-4222-8222-222222222222@198.51.100.2:443?security=none&type=tcp#Second";

    fn store_text() -> String {
        serde_json::json!({
            "version": 3, "activeId": "", "lastId": "", "profiles": [],
            "subscriptions": [{"id": SUBSCRIPTION, "name": "Source", "url": "https://provider.invalid/private", "updatedAt": 1}],
            "routingPreset": "custom", "customRules": [], "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true,
        })
        .to_string()
    }

    fn batch_store_text() -> String {
        serde_json::json!({
            "version": 3, "activeId": "", "lastId": "", "profiles": [],
            "subscriptions": [
                {"id": SUBSCRIPTION, "name": "Source", "url": "https://provider.invalid/private", "updatedAt": 1},
                {"id": SECOND_SUBSCRIPTION, "name": "Second", "url": "https://second.invalid/private", "updatedAt": 3}
            ],
            "routingPreset": "custom", "customRules": [], "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true,
        })
        .to_string()
    }

    struct MockStore {
        input: String,
        in_lease: Rc<Cell<bool>>,
        calls: Vec<&'static str>,
    }

    impl SubscriptionRefreshStore for MockStore {
        fn snapshot(
            &mut self,
            subscription_id: &str,
        ) -> Result<SubscriptionRefreshSnapshot, SubscriptionMutationCommitError> {
            assert!(!self.in_lease.replace(true));
            self.calls.push("snapshot");
            let result = prepare_subscription_refresh(&self.input, subscription_id)
                .map_err(SubscriptionMutationCommitError::Mutation);
            self.in_lease.set(false);
            result
        }

        fn commit(
            &mut self,
            snapshot: SubscriptionRefreshSnapshot,
            entries: Vec<IncomingSubscriptionProfile>,
            updated_at: u64,
            skipped: usize,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError> {
            assert!(!self.in_lease.replace(true));
            self.calls.push("commit");
            let result = omavless_domain::private_store::apply_subscription_refresh(
                &self.input,
                snapshot,
                entries,
                updated_at,
                skipped,
            )
            .map_err(SubscriptionMutationCommitError::Mutation);
            self.in_lease.set(false);
            let (result, counts) = result?;
            self.input = std::str::from_utf8(result.payload()).unwrap().to_owned();
            Ok(SubscriptionRefreshCommit { counts })
        }
    }

    impl SubscriptionRefreshBatchStore for MockStore {
        fn snapshot_batch(
            &mut self,
        ) -> Result<SubscriptionRefreshBatchSnapshot, SubscriptionMutationCommitError> {
            assert!(!self.in_lease.replace(true));
            self.calls.push("snapshot_batch");
            let result = prepare_subscription_refresh_batch(&self.input)
                .map_err(SubscriptionMutationCommitError::Mutation);
            self.in_lease.set(false);
            result
        }

        fn commit_batch(
            &mut self,
            snapshot: SubscriptionRefreshBatchSnapshot,
            updates: Vec<SubscriptionRefreshBatchEntries>,
            updated_at: u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError> {
            assert!(!self.in_lease.replace(true));
            self.calls.push("commit_batch");
            let result =
                apply_subscription_refresh_batch(&self.input, snapshot, updates, updated_at)
                    .map_err(SubscriptionMutationCommitError::Mutation);
            self.in_lease.set(false);
            let (result, counts) = result?;
            self.input = std::str::from_utf8(result.payload()).unwrap().to_owned();
            Ok(SubscriptionRefreshCommit { counts })
        }
    }

    #[test]
    fn transport_is_between_leases_and_commit_occurs_once() {
        let in_lease = Rc::new(Cell::new(false));
        let mut store = MockStore {
            input: store_text(),
            in_lease: Rc::clone(&in_lease),
            calls: Vec::new(),
        };
        let result = refresh_subscription_offline(
            &mut store,
            SUBSCRIPTION,
            |_| {
                assert!(!in_lease.get());
                Ok(PrivateSubscriptionBody::from_bytes(URI.as_bytes().to_vec()).unwrap())
            },
            || "00000000-0000-4000-8000-000000000001".to_owned(),
            || 9,
        )
        .unwrap();
        assert_eq!(store.calls, ["snapshot", "commit"]);
        assert_eq!(
            result.counts,
            SubscriptionRefreshCounts {
                added: 1,
                removed: 0,
                stale: 0,
                total: 1,
                skipped: 0,
            }
        );
    }

    #[test]
    fn transport_or_feed_failure_never_commits_or_echoes_private_input() {
        for transport_failure in [true, false] {
            let in_lease = Rc::new(Cell::new(false));
            let mut store = MockStore {
                input: store_text(),
                in_lease,
                calls: Vec::new(),
            };
            let result = refresh_subscription_offline(
                &mut store,
                SUBSCRIPTION,
                |_| {
                    if transport_failure {
                        Err(SubscriptionTransportError::Unavailable)
                    } else {
                        Ok(PrivateSubscriptionBody::from_bytes(
                            b"private-provider-password".to_vec(),
                        )
                        .unwrap())
                    }
                },
                || unreachable!(),
                || 9,
            );
            let error = result.unwrap_err();
            assert_eq!(store.calls, ["snapshot"]);
            let public = format!("{error:?} {error}");
            assert!(!public.contains("private-provider"));
            assert!(!public.contains("password"));
        }
    }

    #[test]
    fn refresh_all_fetches_in_order_outside_leases_and_commits_once() {
        let in_lease = Rc::new(Cell::new(false));
        let mut store = MockStore {
            input: batch_store_text(),
            in_lease: Rc::clone(&in_lease),
            calls: Vec::new(),
        };
        let fetches = Cell::new(0usize);
        let ids = Cell::new(0usize);
        let result = refresh_subscriptions_offline(
            &mut store,
            |_| {
                assert!(!in_lease.get());
                let index = fetches.get();
                fetches.set(index + 1);
                let uri = if index == 0 { URI } else { SECOND_URI };
                Ok(PrivateSubscriptionBody::from_bytes(uri.as_bytes().to_vec()).unwrap())
            },
            || {
                let index = ids.get() + 1;
                ids.set(index);
                format!("00000000-0000-4000-8000-{index:012}")
            },
            || 9,
        )
        .unwrap();
        assert_eq!(store.calls, ["snapshot_batch", "commit_batch"]);
        assert_eq!(fetches.get(), 2);
        assert_eq!(result.counts.added, 2);
        assert_eq!(result.counts.total, 2);
        let written: serde_json::Value = serde_json::from_str(&store.input).unwrap();
        assert_eq!(written["subscriptions"][0]["updatedAt"], 9);
        assert_eq!(written["subscriptions"][1]["updatedAt"], 9);
    }

    #[test]
    fn refresh_all_failure_and_empty_set_never_commit() {
        let in_lease = Rc::new(Cell::new(false));
        let mut store = MockStore {
            input: batch_store_text(),
            in_lease: Rc::clone(&in_lease),
            calls: Vec::new(),
        };
        let fetches = Cell::new(0usize);
        let error = refresh_subscriptions_offline(
            &mut store,
            |_| {
                assert!(!in_lease.get());
                let next = fetches.get() + 1;
                fetches.set(next);
                if next == 2 {
                    Err(SubscriptionTransportError::Unavailable)
                } else {
                    Ok(PrivateSubscriptionBody::from_bytes(URI.as_bytes().to_vec()).unwrap())
                }
            },
            || "00000000-0000-4000-8000-000000000001".to_owned(),
            || panic!("time must not be read after failure"),
        )
        .unwrap_err();
        assert_eq!(error.code(), "subscription_transport_unavailable");
        assert_eq!(store.calls, ["snapshot_batch"]);

        let mut empty: serde_json::Value = serde_json::from_str(&store_text()).unwrap();
        empty["subscriptions"] = serde_json::json!([]);
        let mut store = MockStore {
            input: empty.to_string(),
            in_lease,
            calls: Vec::new(),
        };
        let result = refresh_subscriptions_offline(
            &mut store,
            |_| panic!("empty refresh must not fetch"),
            || panic!("empty refresh must not allocate IDs"),
            || panic!("empty refresh must not read time"),
        )
        .unwrap();
        assert_eq!(result.counts.total, 0);
        assert_eq!(store.calls, ["snapshot_batch"]);
    }

    #[test]
    fn refresh_all_aggregate_private_memory_bound_is_checked() {
        assert_eq!(add_batch_private_bytes(1, 2).unwrap(), 3);
        assert_eq!(
            add_batch_private_bytes(MAX_PRIVATE_STORE_BYTES, 1).unwrap_err(),
            SubscriptionRefreshError::AggregateTooLarge
        );
        assert_eq!(
            add_batch_private_bytes(usize::MAX, 1).unwrap_err(),
            SubscriptionRefreshError::AggregateTooLarge
        );
        let public = format!("{}", SubscriptionRefreshError::AggregateTooLarge);
        assert!(!public.contains("url"));
        assert!(!public.contains("password"));
    }

    #[test]
    fn stale_snapshot_maps_to_one_safe_conflict() {
        let snapshot = prepare_subscription_refresh(&store_text(), SUBSCRIPTION).unwrap();
        let mut changed: serde_json::Value = serde_json::from_str(&store_text()).unwrap();
        changed["subscriptions"][0]["updatedAt"] = serde_json::Value::from(2);
        let error = omavless_domain::private_store::apply_subscription_refresh(
            &changed.to_string(),
            snapshot,
            Vec::new(),
            9,
            0,
        )
        .err()
        .unwrap();
        assert_eq!(error, PrivateStoreError::SubscriptionChanged);
    }

    #[test]
    fn fixed_store_adapter_releases_shared_lock_while_transport_runs() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-refresh-lock-{}-{nonce}",
            std::process::id()
        ));
        let runtime = root.join("runtime");
        let state = root.join("state");
        let config = root.join("config");
        for directory in [&root, &runtime, &state, &config] {
            fs::create_dir(directory).unwrap();
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let store_path = config.join("profiles.json");
        fs::write(&store_path, store_text()).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let paths = CutoverPaths::below(&runtime, &state, uid);
        let transport_paths = paths.clone();
        let mut store = ExistingPrivateSubscriptionStore::new(&store_path, uid, paths);
        let result = refresh_subscription_offline(
            &mut store,
            SUBSCRIPTION,
            |_| {
                // Acquiring the same lock here proves the snapshot lease was
                // released before transport. Commit then acquires it again.
                let lease = MigrationLock::acquire(&transport_paths, uid).unwrap();
                drop(lease);
                Ok(PrivateSubscriptionBody::from_bytes(URI.as_bytes().to_vec()).unwrap())
            },
            || "00000000-0000-4000-8000-000000000001".to_owned(),
            || 9,
        )
        .unwrap();
        assert_eq!(result.counts.added, 1);
        assert_eq!(
            fs::metadata(&store_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }
}
