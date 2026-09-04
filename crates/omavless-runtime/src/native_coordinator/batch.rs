// SPDX-License-Identifier: MIT

//! Owner-side composition of the inactive refresh-all protocol and worker.
//! The caller serializes this owner, but runs `NativeSubscriptionBatch::step`
//! outside that lock. No scheduler, thread, IPC registration or cutover here.

use super::*;
use crate::long_operation::{
    CommitFence, DEFAULT_COMPLETED_OPERATION_LIMIT, LongOperationError, LongOperationRegistry,
    LongOperationToken, StartOutcome,
};
use crate::long_operation_protocol::{
    parse_operation_cancel, parse_operation_get, parse_refresh_all_start,
};
use crate::mutation::MutationDigest;
use crate::remote_fetch::RemoteFetchPool;
use crate::subscription_batch_work::{
    BatchCancellation, BatchWorkError, BatchWorkStep, BudgetedSubscriptionTransport,
    SubscriptionBatchWork,
};
use crate::subscription_mutation::{
    commit_subscription_refresh_batch, snapshot_subscription_refresh_batch,
};

pub(super) struct BatchOwnerState {
    instance: String,
    registry: LongOperationRegistry,
    active: Option<(LongOperationToken, BatchCancellation)>,
    stopped: bool,
}

/// Private supervisor capability: retain before spawning the worker so a
/// spawn failure or panic can be terminalized without the worker payload.
#[derive(Clone)]
pub struct NativeBatchTicket {
    instance: String,
    token: LongOperationToken,
}

/// Private, non-cloneable worker capability minted by one owner instance.
/// A caller returns it to complete; dropping it is not completion. Retain its
/// ticket to abort lost work. Runtime shutdown revokes all outstanding work.
pub struct NativeSubscriptionBatch {
    instance: String,
    token: LongOperationToken,
    base_revision: u64,
    work: SubscriptionBatchWork,
    failure: Option<BatchWorkError>,
}

impl NativeSubscriptionBatch {
    #[must_use]
    pub fn supervisor_ticket(&self) -> NativeBatchTicket {
        NativeBatchTicket {
            instance: self.instance.clone(),
            token: self.token,
        }
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
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.work.step(transport, pool, next_record_id);
        if let Err(error) = result {
            self.failure = Some(error);
        }
        result
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Bind once to the actual runtime instance, never a request-provided ID.
    /// Live registration remains absent; production must use the gated owner.
    pub fn initialize_batch_operations(&mut self, instance: &str) -> Result<(), NativeOwnerError> {
        if self.batch.is_some() {
            return Err(NativeOwnerError::Invariant);
        }
        self.batch = Some(BatchOwnerState {
            instance: instance.to_owned(),
            registry: LongOperationRegistry::new(instance, DEFAULT_COMPLETED_OPERATION_LIMIT)
                .map_err(NativeOwnerError::LongOperation)?,
            active: None,
            stopped: false,
        });
        Ok(())
    }

    pub(super) fn check_batch_operation_id(
        &self,
        operation_id: Option<&str>,
    ) -> Result<(), NativeOwnerError> {
        if operation_id.is_some_and(|id| {
            self.batch
                .as_ref()
                .is_some_and(|state| state.registry.has_operation_id(id))
        }) {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict,
            ));
        }
        Ok(())
    }

    fn batch_lock(&self) -> Result<MigrationLock, NativeOwnerError> {
        let lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                _ => NativeOwnerError::OwnershipUnavailable,
            })?;
        if self.required_ownership.is_some_and(|fence| {
            fence.phase != OwnershipPhase::Rust
                || !self
                    .transaction
                    .ownership_matches(fence.phase, fence.generation)
        }) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if self.transaction.blocked() {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        Ok(lock)
    }

    /// None is an exact retry: poll the retained projection, never fetch again.
    pub fn start_subscription_batch(
        &mut self,
        request: &Value,
    ) -> Result<Option<NativeSubscriptionBatch>, NativeOwnerError> {
        let request = parse_refresh_all_start(request)?;
        let _lock = self.batch_lock()?;
        let revision = self.revision();
        let ordinary_id = self
            .coordinator
            .operation_id_in_use(request.operation_id())?;
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.stopped {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        // Replay precedes store access and stale-revision rejection.
        if state.registry.has_operation_id(request.operation_id()) {
            state
                .registry
                .start(
                    request.instance_id(),
                    request.operation_id(),
                    request.digest(),
                    request.expected_revision(),
                    revision,
                    0,
                    ordinary_id,
                )
                .map_err(NativeOwnerError::LongOperation)?;
            return Ok(None);
        }
        if request.instance_id() != state.instance {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::InstanceMismatch,
            ));
        }
        if ordinary_id {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::OperationConflict,
            ));
        }
        if request
            .expected_revision()
            .is_some_and(|expected| expected != revision)
        {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::RevisionConflict,
            ));
        }
        if state.active.is_some() {
            return Err(NativeOwnerError::LongOperation(LongOperationError::Busy));
        }
        let snapshot = snapshot_subscription_refresh_batch(
            self.transaction.store_path(),
            self.transaction.uid(),
        )
        .map_err(|error| NativeOwnerError::Subscription(subscription_store_error(error)))?;
        let started = state
            .registry
            .start(
                request.instance_id(),
                request.operation_id(),
                request.digest(),
                request.expected_revision(),
                revision,
                snapshot.len(),
                false,
            )
            .map_err(NativeOwnerError::LongOperation)?;
        let StartOutcome::Started(token) = started else {
            return Err(NativeOwnerError::Invariant);
        };
        state
            .registry
            .begin(token, revision)
            .map_err(NativeOwnerError::LongOperation)?;
        let cancellation = BatchCancellation::default();
        state.active = Some((token, cancellation.clone()));
        Ok(Some(NativeSubscriptionBatch {
            instance: state.instance.clone(),
            token,
            base_revision: revision,
            work: SubscriptionBatchWork::new(snapshot, cancellation),
            failure: None,
        }))
    }

    pub fn subscription_batch_status(&self, request: &Value) -> Result<Value, NativeOwnerError> {
        let request = parse_operation_get(request)?;
        let state = self
            .batch
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        state
            .registry
            .projection(request.instance_id(), request.operation_id())
            .map_err(NativeOwnerError::LongOperation)?
            .result_value()
            .map_err(NativeOwnerError::Protocol)
    }

    pub fn cancel_subscription_batch(&mut self, request: &Value) -> Result<bool, NativeOwnerError> {
        let request = parse_operation_cancel(request)?;
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let cancelled = state
            .registry
            .request_cancel(request.instance_id(), request.operation_id())
            .map_err(NativeOwnerError::LongOperation)?;
        if cancelled.accepted {
            let (token, flag) = state.active.as_ref().ok_or(NativeOwnerError::Invariant)?;
            if *token != cancelled.token {
                return Err(NativeOwnerError::Invariant);
            }
            flag.request();
        }
        Ok(cancelled.accepted)
    }

    /// Publish only counters; no provider identity or prepared payload escapes.
    pub fn publish_subscription_batch_progress(
        &mut self,
        job: &NativeSubscriptionBatch,
    ) -> Result<(), NativeOwnerError> {
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        Self::check_batch_handle(state, job)?;
        state
            .registry
            .advance(job.token, job.work.progress().0)
            .map_err(NativeOwnerError::LongOperation)
    }

    fn check_batch_handle(
        state: &BatchOwnerState,
        job: &NativeSubscriptionBatch,
    ) -> Result<(), NativeOwnerError> {
        if state.stopped
            || state.instance != job.instance
            || state.active.as_ref().map(|entry| entry.0) != Some(job.token)
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        Ok(())
    }

    /// Recover from spawn failure or worker panic without stopping the owner.
    /// An old supervisor cannot revoke a successor operation using its ticket.
    pub fn abort_subscription_batch(
        &mut self,
        ticket: NativeBatchTicket,
    ) -> Result<(), NativeOwnerError> {
        let revision = self.revision();
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.instance != ticket.instance
            || state.active.as_ref().map(|entry| entry.0) != Some(ticket.token)
        {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::NotFound,
            ));
        }
        let (token, flag) = state.active.take().ok_or(NativeOwnerError::Invariant)?;
        flag.request();
        state
            .registry
            .finish_failure(token, revision, StableErrorCode::InternalError)
            .map_err(NativeOwnerError::LongOperation)?;
        Ok(())
    }

    /// Revoke outstanding work before shutdown/ownership withdrawal. A later
    /// completion cannot write, even if its fetch returns successfully.
    pub fn stop_batch_operations(&mut self) -> Result<(), NativeOwnerError> {
        let revision = self.revision();
        let Some(state) = self.batch.as_mut() else {
            return Ok(());
        };
        state.stopped = true;
        if let Some((token, cancellation)) = state.active.take() {
            cancellation.request();
            state
                .registry
                .finish_failure(token, revision, StableErrorCode::DaemonRestarting)
                .map_err(NativeOwnerError::LongOperation)?;
        }
        Ok(())
    }

    /// Consume a finished or failed worker under the serialized owner. This
    /// takes the migration lock again, rechecks ownership/revision and closes
    /// cancellation before the single existing atomic store transaction.
    pub fn complete_subscription_batch<N: FnOnce() -> u64>(
        &mut self,
        job: NativeSubscriptionBatch,
        now_millis: N,
    ) -> Result<(), NativeOwnerError> {
        let mut state = self
            .batch
            .take()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let result = (|| {
            Self::check_batch_handle(&state, &job)?;
            let token = job.token;
            let completed = job.work.progress().0;
            state
                .registry
                .advance(token, completed)
                .map_err(NativeOwnerError::LongOperation)?;
            let outcome = self.commit_subscription_batch_work(&mut state, job, now_millis);
            state.active = None;
            match outcome {
                Ok(true) => state
                    .registry
                    .finish_success(token, self.revision())
                    .map_err(NativeOwnerError::LongOperation),
                Ok(false) => Ok(()), // the registry already recorded cancellation
                Err(error) => {
                    state
                        .registry
                        .finish_failure(token, self.revision(), error.stable_code())
                        .map_err(NativeOwnerError::LongOperation)?;
                    Err(error)
                }
            }
        })();
        self.batch = Some(state);
        result
    }

    fn commit_subscription_batch_work<N: FnOnce() -> u64>(
        &mut self,
        state: &mut BatchOwnerState,
        job: NativeSubscriptionBatch,
        now_millis: N,
    ) -> Result<bool, NativeOwnerError> {
        if let Some(error) = job.failure {
            return Err(batch_work_error(error));
        }
        let prepared = job.work.into_prepared().map_err(batch_work_error)?;
        let _lock = self.batch_lock()?;
        if self.revision() != job.base_revision {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict,
            ));
        }
        let (snapshot, updates) = prepared.into_parts().map_err(batch_work_error)?;
        if state
            .registry
            .fence_commit(job.token, self.revision())
            .map_err(NativeOwnerError::LongOperation)?
            == CommitFence::Cancelled
        {
            return Ok(false);
        }
        if snapshot.is_empty() {
            return Ok(true);
        }
        let request = MutationRequest::new(
            MutationKind::Other,
            None,
            Some(job.base_revision),
            MutationDigest::from_semantic_bytes(b"native-refresh-all-commit-v1"),
        )?;
        let SubmitOutcome::Queued { token } = self.coordinator.submit(request)? else {
            return Err(NativeOwnerError::Invariant);
        };
        match self.coordinator.begin_next()? {
            BeginOutcome::Started(active) if active.token == token => {}
            BeginOutcome::Rejected { outcome, .. } => {
                return Err(NativeOwnerError::Coordinator(
                    if outcome.error == Some(StableErrorCode::Conflict) {
                        CoordinatorError::RevisionConflict
                    } else {
                        CoordinatorError::RevisionExhausted
                    },
                ));
            }
            _ => return Err(NativeOwnerError::Invariant),
        }
        let result = commit_subscription_refresh_batch(
            self.transaction.store_path(),
            self.transaction.uid(),
            snapshot,
            updates,
            now_millis(),
        )
        .map_err(|error| NativeOwnerError::Subscription(subscription_store_error(error)));
        self.coordinator.finish(
            token,
            match result {
                Ok(_) => MutationResult::Success,
                Err(error) => MutationResult::Failure(error.stable_code()),
            },
        )?;
        result.map(|_| true)
    }
}

fn batch_work_error(error: BatchWorkError) -> NativeOwnerError {
    NativeOwnerError::Subscription(match error {
        BatchWorkError::Cancelled => SubscriptionTransactionError::Conflict,
        BatchWorkError::Deadline | BatchWorkError::Preparation(_) => {
            SubscriptionTransactionError::Transport
        }
        BatchWorkError::InvalidState => SubscriptionTransactionError::InvalidArgument,
    })
}
