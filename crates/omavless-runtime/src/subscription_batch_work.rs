// SPDX-License-Identifier: MIT

//! Inactive incremental preparation of one atomic refresh-all batch.
//!
//! Each step runs at most one bounded provider fetch, outside owner/store
//! locks, using the runtime's shared admission pool. This module never writes
//! a store, starts a thread, registers IPC, or advances the owner's revision.
//! The eventual executor must still admit start/replay under the owner lock
//! and fence cancellation, ownership, revision and commit together there.

use crate::remote_fetch::RemoteFetchPool;
use crate::subscription_refresh::SubscriptionRefreshError;
use crate::subscription_transport::{
    HttpsSubscriptionTransport, SUBSCRIPTION_TIMEOUT, SubscriptionTransportError,
};
use omavless_domain::private_store::{
    MAX_PRIVATE_STORE_BYTES, SubscriptionRefreshBatchEntries, SubscriptionRefreshBatchSnapshot,
};
use omavless_domain::subscription_feed::{PrivateSubscriptionBody, decode_subscription_feed};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const REFRESH_ALL_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// The budget is trusted runtime input. Implementations must bound the whole
/// fetch, including redirects and the body, by this remaining time.
pub trait BudgetedSubscriptionTransport {
    fn fetch_with_budget(
        &self,
        url: &str,
        budget: Duration,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError>;
}

impl BudgetedSubscriptionTransport for HttpsSubscriptionTransport {
    fn fetch_with_budget(
        &self,
        url: &str,
        budget: Duration,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        HttpsSubscriptionTransport::fetch_with_budget(self, url, budget)
    }
}

/// Cooperative worker notification, not the authoritative commit fence.
/// The future operation registry decides whether cancellation is accepted.
#[derive(Clone, Default)]
pub struct BatchCancellation(Arc<AtomicBool>);

impl BatchCancellation {
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchWorkError {
    Cancelled,
    Deadline,
    Preparation(SubscriptionRefreshError),
    InvalidState,
}

impl fmt::Display for BatchWorkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "Subscription batch was cancelled",
            Self::Deadline => "Subscription batch deadline was exceeded",
            Self::Preparation(_) => "Subscription batch preparation failed",
            Self::InvalidState => "Subscription batch state is invalid",
        })
    }
}

impl std::error::Error for BatchWorkError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchWorkStep {
    /// No permit was available. Retry through a bounded scheduler, without
    /// holding the owner lock; the original whole-job deadline still applies.
    Busy,
    Advanced,
    Ready,
}

/// Private resumable preparation state. Intentionally not Debug/serializable.
pub struct SubscriptionBatchWork {
    snapshot: Option<SubscriptionRefreshBatchSnapshot>,
    updates: Vec<SubscriptionRefreshBatchEntries>,
    retained_bytes: usize,
    completed: usize,
    total: usize,
    deadline: Instant,
    cancellation: BatchCancellation,
}

/// A complete, still-private batch, not permission to commit it.
pub struct PreparedSubscriptionBatch {
    snapshot: SubscriptionRefreshBatchSnapshot,
    updates: Vec<SubscriptionRefreshBatchEntries>,
    deadline: Instant,
    cancellation: BatchCancellation,
}

fn check_worker(
    cancellation: &BatchCancellation,
    deadline: Instant,
    now: Instant,
) -> Result<Duration, BatchWorkError> {
    // An accepted cancellation wins even if the in-flight fetch also failed
    // or expired before it could observe the notification.
    if cancellation.requested() {
        return Err(BatchWorkError::Cancelled);
    }
    deadline
        .checked_duration_since(now)
        .filter(|remaining| !remaining.is_zero())
        .ok_or(BatchWorkError::Deadline)
}

impl SubscriptionBatchWork {
    #[must_use]
    pub fn new(
        snapshot: SubscriptionRefreshBatchSnapshot,
        cancellation: BatchCancellation,
    ) -> Self {
        Self {
            total: snapshot.len(),
            snapshot: Some(snapshot),
            updates: Vec::new(),
            retained_bytes: 0,
            completed: 0,
            deadline: Instant::now() + REFRESH_ALL_DEADLINE,
            cancellation,
        }
    }

    #[must_use]
    pub fn progress(&self) -> (usize, usize) {
        (self.completed, self.total)
    }

    pub fn step<T, G>(
        &mut self,
        transport: &T,
        pool: &RemoteFetchPool,
        next_record_id: &mut G,
    ) -> Result<BatchWorkStep, BatchWorkError>
    where
        T: BudgetedSubscriptionTransport,
        G: FnMut() -> String,
    {
        self.step_with_clock(transport, pool, next_record_id, Instant::now)
    }

    fn step_with_clock<T, G, C>(
        &mut self,
        transport: &T,
        pool: &RemoteFetchPool,
        next_record_id: &mut G,
        mut clock: C,
    ) -> Result<BatchWorkStep, BatchWorkError>
    where
        T: BudgetedSubscriptionTransport,
        G: FnMut() -> String,
        C: FnMut() -> Instant,
    {
        let result = self.step_inner(transport, pool, next_record_id, &mut clock);
        if result.is_err() {
            // Failures are terminal for this private preparation; no caller
            // can resume it and accidentally commit an incomplete prefix.
            self.snapshot = None;
            self.updates.clear();
            self.retained_bytes = 0;
        }
        result
    }

    fn step_inner<T, G, C>(
        &mut self,
        transport: &T,
        pool: &RemoteFetchPool,
        next_record_id: &mut G,
        clock: &mut C,
    ) -> Result<BatchWorkStep, BatchWorkError>
    where
        T: BudgetedSubscriptionTransport,
        G: FnMut() -> String,
        C: FnMut() -> Instant,
    {
        let snapshot = self.snapshot.as_ref().ok_or(BatchWorkError::InvalidState)?;
        check_worker(&self.cancellation, self.deadline, clock())?;
        if self.completed == self.total {
            return Ok(BatchWorkStep::Ready);
        }
        let Some(permit) = pool.try_acquire() else {
            return Ok(BatchWorkStep::Busy);
        };
        let budget =
            check_worker(&self.cancellation, self.deadline, clock())?.min(SUBSCRIPTION_TIMEOUT);
        let url = snapshot
            .private_urls()
            .nth(self.completed)
            .ok_or(BatchWorkError::InvalidState)?;
        let fetched = transport.fetch_with_budget(url, budget);
        drop(permit);
        check_worker(&self.cancellation, self.deadline, clock())?;
        let body = fetched.map_err(|error| {
            BatchWorkError::Preparation(SubscriptionRefreshError::Transport(error))
        })?;
        let feed = decode_subscription_feed(body)
            .map_err(|error| BatchWorkError::Preparation(SubscriptionRefreshError::Feed(error)))?;
        let retained_bytes = self
            .retained_bytes
            .checked_add(feed.private_payload_bytes())
            .filter(|bytes| *bytes <= MAX_PRIVATE_STORE_BYTES)
            .ok_or(BatchWorkError::Preparation(
                SubscriptionRefreshError::AggregateTooLarge,
            ))?;
        check_worker(&self.cancellation, self.deadline, clock())?;
        let skipped = feed.counts().skipped;
        let entries = feed.into_private_entries(next_record_id);
        check_worker(&self.cancellation, self.deadline, clock())?;
        self.updates
            .push(SubscriptionRefreshBatchEntries { entries, skipped });
        self.retained_bytes = retained_bytes;
        self.completed += 1;
        Ok(if self.completed == self.total {
            BatchWorkStep::Ready
        } else {
            BatchWorkStep::Advanced
        })
    }

    pub fn into_prepared(self) -> Result<PreparedSubscriptionBatch, BatchWorkError> {
        check_worker(&self.cancellation, self.deadline, Instant::now())?;
        if self.updates.len() != self.total {
            return Err(BatchWorkError::InvalidState);
        }
        Ok(PreparedSubscriptionBatch {
            snapshot: self.snapshot.ok_or(BatchWorkError::InvalidState)?,
            updates: self.updates,
            deadline: self.deadline,
            cancellation: self.cancellation,
        })
    }
}

impl PreparedSubscriptionBatch {
    /// Call only inside the future final owner transaction, after its atomic
    /// registry cancellation fence and ownership/revision checks. This final
    /// worker check cannot replace that fence. The existing batch store commit
    /// must also revalidate every snapshot member and write at most once.
    pub fn into_parts(
        self,
    ) -> Result<
        (
            SubscriptionRefreshBatchSnapshot,
            Vec<SubscriptionRefreshBatchEntries>,
        ),
        BatchWorkError,
    > {
        check_worker(&self.cancellation, self.deadline, Instant::now())?;
        Ok((self.snapshot, self.updates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_fetch::MAX_CONCURRENT_REMOTE_FETCHES;
    use crate::subscription_mutation::{
        SubscriptionMutationCommitError, SubscriptionRefreshCommit,
    };
    use crate::subscription_refresh::{
        SubscriptionRefreshBatchStore, refresh_subscriptions_offline,
    };
    use omavless_domain::private_store::{
        apply_subscription_refresh_batch, prepare_subscription_refresh_batch,
    };
    use serde_json::json;
    use std::cell::{Cell, RefCell};

    const URI: &str =
        "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example";

    fn input(count: usize) -> String {
        let subscriptions: Vec<_> = (0..count)
            .map(|index| {
                json!({
                    "id": format!("10000000-0000-4000-8000-{index:012}"),
                    "name": "Synthetic", "url": format!("https://provider.invalid/{index}"),
                    "updatedAt": 1,
                })
            })
            .collect();
        json!({
            "version": 3, "activeId": "", "lastId": "", "profiles": [],
            "subscriptions": subscriptions, "routingPreset": "custom", "customRules": [],
            "rulesUpdatedAt": 0, "startupConfigured": true, "onboardingComplete": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
        })
        .to_string()
    }

    fn ids() -> impl FnMut() -> String {
        let mut id = 0;
        move || {
            id += 1;
            format!("20000000-0000-4000-8000-{id:012}")
        }
    }

    #[derive(Default)]
    struct Transport {
        calls: Cell<usize>,
        budgets: RefCell<Vec<Duration>>,
        cancel: Option<BatchCancellation>,
        fail_at: Option<usize>,
    }

    impl BudgetedSubscriptionTransport for Transport {
        fn fetch_with_budget(
            &self,
            _url: &str,
            budget: Duration,
        ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            self.budgets.borrow_mut().push(budget);
            if let Some(cancel) = &self.cancel {
                cancel.request();
            }
            if self.fail_at == Some(call) {
                return Err(SubscriptionTransportError::HttpStatus);
            }
            Ok(PrivateSubscriptionBody::from_bytes(URI.as_bytes().to_vec()).unwrap())
        }
    }

    fn work(count: usize, cancel: &BatchCancellation) -> SubscriptionBatchWork {
        SubscriptionBatchWork::new(
            prepare_subscription_refresh_batch(&input(count)).unwrap(),
            cancel.clone(),
        )
    }

    struct ReferenceStore(String);

    impl SubscriptionRefreshBatchStore for ReferenceStore {
        fn snapshot_batch(
            &mut self,
        ) -> Result<SubscriptionRefreshBatchSnapshot, SubscriptionMutationCommitError> {
            prepare_subscription_refresh_batch(&self.0)
                .map_err(SubscriptionMutationCommitError::Mutation)
        }

        fn commit_batch(
            &mut self,
            snapshot: SubscriptionRefreshBatchSnapshot,
            updates: Vec<SubscriptionRefreshBatchEntries>,
            now: u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError> {
            let (store, counts) = apply_subscription_refresh_batch(&self.0, snapshot, updates, now)
                .map_err(SubscriptionMutationCommitError::Mutation)?;
            self.0 = String::from_utf8(store.payload().to_vec()).unwrap();
            Ok(SubscriptionRefreshCommit { counts })
        }
    }

    #[test]
    fn incremental_preparation_matches_the_accepted_atomic_reference() {
        let original = input(2);
        let mut reference = ReferenceStore(original.clone());
        refresh_subscriptions_offline(
            &mut reference,
            |_| Ok(PrivateSubscriptionBody::from_bytes(URI.as_bytes().to_vec()).unwrap()),
            ids(),
            || 9,
        )
        .unwrap();
        let cancel = BatchCancellation::default();
        let mut job = work(2, &cancel);
        let transport = Transport::default();
        let pool = RemoteFetchPool::default();
        let mut ids = ids();
        assert_eq!(
            job.step(&transport, &pool, &mut ids),
            Ok(BatchWorkStep::Advanced)
        );
        assert_eq!(job.progress(), (1, 2));
        assert_eq!(
            job.step(&transport, &pool, &mut ids),
            Ok(BatchWorkStep::Ready)
        );
        assert_eq!(job.progress(), (2, 2));
        assert_eq!(
            job.step(&transport, &pool, &mut ids),
            Ok(BatchWorkStep::Ready)
        );
        assert_eq!(transport.calls.get(), 2);
        let (snapshot, updates) = job.into_prepared().unwrap().into_parts().unwrap();
        let (prepared, _) =
            apply_subscription_refresh_batch(&original, snapshot, updates, 9).unwrap();
        assert_eq!(prepared.payload(), reference.0.as_bytes());
    }

    #[test]
    fn saturated_pool_preserves_progress_and_fetches_nothing() {
        let pool = RemoteFetchPool::default();
        let mut permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
            .map(|_| pool.try_acquire().unwrap())
            .collect();
        let mut job = work(1, &BatchCancellation::default());
        let transport = Transport::default();
        assert_eq!(
            job.step(&transport, &pool, &mut ids()),
            Ok(BatchWorkStep::Busy)
        );
        assert_eq!(job.progress(), (0, 1));
        assert_eq!(transport.calls.get(), 0);
        permits.pop();
        assert_eq!(
            job.step(&transport, &pool, &mut ids()),
            Ok(BatchWorkStep::Ready)
        );
        assert!(pool.try_acquire().is_some());
    }

    #[test]
    fn queued_cancellation_prevents_the_first_fetch() {
        let cancel = BatchCancellation::default();
        let mut job = work(2, &cancel);
        cancel.request();
        let transport = Transport::default();
        assert_eq!(
            job.step(&transport, &RemoteFetchPool::default(), &mut ids()),
            Err(BatchWorkError::Cancelled)
        );
        assert_eq!(transport.calls.get(), 0);
        assert!(job.into_prepared().is_err());
    }

    #[test]
    fn cancellation_wins_over_an_inflight_provider_failure() {
        let cancel = BatchCancellation::default();
        let mut job = work(2, &cancel);
        let transport = Transport {
            cancel: Some(cancel),
            fail_at: Some(1),
            ..Transport::default()
        };
        let pool = RemoteFetchPool::default();
        assert_eq!(
            job.step(&transport, &pool, &mut ids()),
            Err(BatchWorkError::Cancelled)
        );
        assert_eq!(transport.calls.get(), 1);
        assert!(job.into_prepared().is_err());
        let permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
            .map(|_| pool.try_acquire().unwrap())
            .collect();
        assert_eq!(permits.len(), MAX_CONCURRENT_REMOTE_FETCHES);
    }

    #[test]
    fn provider_failure_discards_the_entire_prepared_prefix() {
        let mut job = work(2, &BatchCancellation::default());
        let transport = Transport {
            fail_at: Some(2),
            ..Transport::default()
        };
        let pool = RemoteFetchPool::default();
        let mut ids = ids();
        assert_eq!(
            job.step(&transport, &pool, &mut ids),
            Ok(BatchWorkStep::Advanced)
        );
        assert!(matches!(
            job.step(&transport, &pool, &mut ids),
            Err(BatchWorkError::Preparation(_))
        ));
        assert_eq!(job.progress(), (1, 2));
        assert!(job.updates.is_empty());
        assert_eq!(
            job.step(&transport, &pool, &mut ids),
            Err(BatchWorkError::InvalidState)
        );
        assert_eq!(transport.calls.get(), 2);
        assert!(job.into_prepared().is_err());
    }

    #[test]
    fn deadline_bounds_queue_time_fetch_budget_and_late_success() {
        let pool = RemoteFetchPool::default();
        let mut job = work(1, &BatchCancellation::default());
        let transport = Transport::default();
        let deadline = job.deadline;
        assert_eq!(
            job.step_with_clock(&transport, &pool, &mut ids(), || deadline),
            Err(BatchWorkError::Deadline)
        );
        assert_eq!(transport.calls.get(), 0);

        let mut job = work(1, &BatchCancellation::default());
        let deadline = job.deadline;
        let early = deadline - Duration::from_millis(12);
        let mut clock = [early, early, deadline].into_iter();
        assert_eq!(
            job.step_with_clock(&transport, &pool, &mut ids(), || clock.next().unwrap()),
            Err(BatchWorkError::Deadline)
        );
        assert_eq!(&*transport.budgets.borrow(), &[Duration::from_millis(12)]);
        assert!(job.into_prepared().is_err());
    }

    #[test]
    fn cancellation_is_rechecked_after_ready_before_handoff() {
        let cancel = BatchCancellation::default();
        let mut job = work(1, &cancel);
        job.step(
            &Transport::default(),
            &RemoteFetchPool::default(),
            &mut ids(),
        )
        .unwrap();
        let ready = job.into_prepared().unwrap();
        cancel.request();
        assert!(matches!(ready.into_parts(), Err(BatchWorkError::Cancelled)));
    }

    #[test]
    fn completed_work_still_requires_unchanged_store_members_and_a_live_deadline() {
        let original = input(2);
        let mut job = work(2, &BatchCancellation::default());
        let pool = RemoteFetchPool::default();
        let transport = Transport::default();
        let mut ids = ids();
        job.step(&transport, &pool, &mut ids).unwrap();
        job.step(&transport, &pool, &mut ids).unwrap();
        let (snapshot, updates) = job.into_prepared().unwrap().into_parts().unwrap();
        let mut changed: serde_json::Value = serde_json::from_str(&original).unwrap();
        changed["subscriptions"][1]["url"] = json!("https://replacement.invalid/private");
        assert!(apply_subscription_refresh_batch(&changed.to_string(), snapshot, updates, 9).is_err());

        let mut empty = work(0, &BatchCancellation::default()).into_prepared().unwrap();
        empty.deadline = Instant::now();
        assert!(matches!(empty.into_parts(), Err(BatchWorkError::Deadline)));
    }

    #[test]
    fn empty_batch_needs_no_fetch_permit_or_record_id() {
        let mut job = work(0, &BatchCancellation::default());
        let transport = Transport::default();
        let pool = RemoteFetchPool::default();
        let _permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
            .map(|_| pool.try_acquire().unwrap())
            .collect();
        assert_eq!(
            job.step(&transport, &pool, &mut || panic!(
                "empty batch needs no IDs"
            )),
            Ok(BatchWorkStep::Ready)
        );
        assert_eq!(transport.calls.get(), 0);
        let (snapshot, updates) = job.into_prepared().unwrap().into_parts().unwrap();
        assert!(snapshot.is_empty());
        assert!(updates.is_empty());
    }

    #[test]
    fn aggregate_overflow_is_terminal_and_has_no_private_error_output() {
        let mut job = work(1, &BatchCancellation::default());
        job.retained_bytes = MAX_PRIVATE_STORE_BYTES;
        let error = job
            .step(
                &Transport::default(),
                &RemoteFetchPool::default(),
                &mut ids(),
            )
            .unwrap_err();
        assert_eq!(
            error,
            BatchWorkError::Preparation(SubscriptionRefreshError::AggregateTooLarge)
        );
        assert!(job.into_prepared().is_err());
        let output = format!("{error:?} {error}");
        for private in ["provider.invalid", "192.0.2.1", "11111111-1111", URI] {
            assert!(!output.contains(private));
        }
    }
}
