// SPDX-License-Identifier: MIT

//! Owner-side composition of the inactive refresh-all protocol and worker.
//! The caller serializes this owner, but runs `NativeSubscriptionBatch::step`
//! outside that lock. No scheduler, thread, IPC registration or cutover here.

use super::*;
use crate::long_operation::{
    CommitFence, DEFAULT_COMPLETED_OPERATION_LIMIT, LongOperationError, LongOperationRegistry,
    LongOperationToken, StartOutcome,
};
use crate::long_operation_protocol::LongOperationState;
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
use crate::subscription_schedule_plan::RefreshSchedule;
use crate::subscription_schedule_preference::read_locked as read_schedule_preference_locked;
use omavless_domain::private_store::{
    SubscriptionRefreshBatchEntries, SubscriptionRefreshBatchSnapshot,
};

pub(super) struct BatchOwnerState {
    pub(super) instance: String,
    pub(super) registry: LongOperationRegistry,
    pub(super) active: Option<(LongOperationToken, ActiveCancellation)>,
    pub(super) stopped: bool,
}

pub(super) enum ActiveCancellation {
    Subscription(BatchCancellation),
    Provider(crate::provider_refresh::ProviderRefreshCancellation),
    Probe(super::probe::ProbeCancellation),
}
impl ActiveCancellation {
    pub(super) fn request(&self) {
        match self {
            Self::Subscription(flag) => flag.request(),
            Self::Provider(flag) => flag.request(),
            Self::Probe(flag) => flag.request(),
        }
    }
}

/// Private supervisor capability: retain before spawning the worker so a
/// spawn failure or panic can be terminalized without the worker payload.
#[derive(Clone)]
pub struct NativeBatchTicket {
    pub(super) instance: String,
    pub(super) token: LongOperationToken,
    pub(super) subscription_base_revision: Option<u64>,
}

impl NativeBatchTicket {
    pub(crate) fn instance(&self) -> &str {
        &self.instance
    }

    pub(crate) fn sequence(&self) -> u64 {
        self.token.sequence()
    }

    pub(crate) fn base_revision(&self) -> Option<u64> {
        self.subscription_base_revision
    }

    #[cfg(test)]
    pub(crate) fn synthetic(instance: &str, sequence: u64, base_revision: u64) -> Self {
        Self {
            instance: instance.to_owned(),
            token: LongOperationToken::synthetic(sequence),
            subscription_base_revision: Some(base_revision),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBatchOutcome {
    Committed,
    Empty,
    Cancelled,
    Failed,
    /// An atomic store replacement may have happened before a sync error.
    /// Never infer failure or retry from this result.
    Uncertain,
}

/// Minted only by the serialized owner after a terminal registry transition.
/// A receipt is specific to one daemon instance and one batch token; it
/// contains no provider identity, response, URL or profile data.
pub struct NativeBatchCompletionReceipt {
    instance: String,
    sequence: u64,
    base_revision: u64,
    completed_revision: u64,
    outcome: NativeBatchOutcome,
}

impl NativeBatchCompletionReceipt {
    pub(crate) fn instance(&self) -> &str {
        &self.instance
    }

    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) fn base_revision(&self) -> u64 {
        self.base_revision
    }

    pub(crate) fn completed_revision(&self) -> u64 {
        self.completed_revision
    }

    pub fn outcome(&self) -> NativeBatchOutcome {
        self.outcome
    }

    #[cfg(test)]
    pub(crate) fn synthetic(
        ticket: &NativeBatchTicket,
        completed_revision: u64,
        outcome: NativeBatchOutcome,
    ) -> Self {
        Self {
            instance: ticket.instance.clone(),
            sequence: ticket.sequence(),
            base_revision: ticket
                .base_revision()
                .expect("synthetic subscription ticket"),
            completed_revision,
            outcome,
        }
    }
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
            subscription_base_revision: Some(self.base_revision),
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
    /// Inactive Off transaction. The caller holds scheduler admission and the
    /// dispatcher; this lease spans preference publication and exact cancel.
    pub(crate) fn disable_subscription_schedule(
        &mut self,
        expected_revision: u64,
        scheduled: Option<&NativeBatchTicket>,
    ) -> Result<crate::batch_scheduler::ScheduleOffResult, crate::batch_scheduler::ScheduleOffError>
    {
        self.disable_subscription_schedule_with(
            expected_revision,
            scheduled,
            crate::subscription_schedule_preference::set_preference_locked,
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_subscription_schedule_uncertain(
        &mut self,
        expected_revision: u64,
        scheduled: Option<&NativeBatchTicket>,
    ) -> Result<crate::batch_scheduler::ScheduleOffResult, crate::batch_scheduler::ScheduleOffError>
    {
        self.disable_subscription_schedule_with(
            expected_revision,
            scheduled,
            |paths, uid, generation, revision, schedule| {
                crate::subscription_schedule_preference::set_preference_locked(
                    paths, uid, generation, revision, schedule,
                )?;
                Err(crate::subscription_schedule_preference::PreferenceError::WriteUncertain)
            },
        )
    }

    fn disable_subscription_schedule_with<F>(
        &mut self,
        expected_revision: u64,
        scheduled: Option<&NativeBatchTicket>,
        write: F,
    ) -> Result<crate::batch_scheduler::ScheduleOffResult, crate::batch_scheduler::ScheduleOffError>
    where
        F: FnOnce(
            &crate::cutover::CutoverPaths,
            u32,
            u64,
            u64,
            RefreshSchedule,
        ) -> Result<
            crate::subscription_schedule_preference::PreferenceSnapshot,
            crate::subscription_schedule_preference::PreferenceError,
        >,
    {
        use crate::batch_scheduler::{ScheduleOffError, ScheduleOffResult};
        use crate::subscription_schedule_preference::PreferenceError;
        let _lock = self.batch_lock().map_err(ScheduleOffError::Owner)?;
        let generation = self
            .required_ownership
            .ok_or(ScheduleOffError::Owner(
                NativeOwnerError::OwnershipUnavailable,
            ))?
            .generation;
        let result = write(
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            generation,
            expected_revision,
            RefreshSchedule::Off,
        );
        // Only a confirmed publication or an uncertain write permits touching
        // the captured worker. Stale/unsafe requests have no cancellation effect.
        if result
            .as_ref()
            .is_err_and(|error| *error != PreferenceError::WriteUncertain)
        {
            return Err(ScheduleOffError::Preference(result.unwrap_err()));
        }
        let cancellation = scheduled
            .map(|ticket| self.cancel_exact_subscription_ticket(ticket))
            .transpose();
        let preference = result.map_err(ScheduleOffError::Preference)?;
        let cancellation_requested = cancellation
            .map_err(ScheduleOffError::CancellationUncertain)?
            .unwrap_or(false);
        Ok(ScheduleOffResult {
            preference,
            cancellation_requested,
        })
    }

    pub(super) fn cancel_exact_subscription_ticket(
        &mut self,
        ticket: &NativeBatchTicket,
    ) -> Result<bool, NativeOwnerError> {
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.instance != ticket.instance || ticket.base_revision().is_none() {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let base_revision = ticket.base_revision().ok_or(NativeOwnerError::Invariant)?;
        if let Some((token, flag)) = state.active.as_ref() {
            if *token != ticket.token || !matches!(flag, ActiveCancellation::Subscription(_)) {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let accepted = state
                .registry
                .request_cancel_token(ticket.token, base_revision)
                .map_err(NativeOwnerError::LongOperation)?;
            if accepted {
                flag.request();
            }
            Ok(accepted)
        } else {
            state
                .registry
                .request_cancel_token(ticket.token, base_revision)
                .map_err(NativeOwnerError::LongOperation)
        }
    }

    /// Read an exact terminal batch result under the owner lock. The caller
    /// must keep the ticket from admission; an arbitrary status projection or
    /// a successful `Result<()>` is not proof that a store commit occurred.
    pub fn subscription_batch_receipt(
        &self,
        ticket: &NativeBatchTicket,
    ) -> Result<NativeBatchCompletionReceipt, NativeOwnerError> {
        let state = self
            .batch
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.instance != ticket.instance {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::NotFound,
            ));
        }
        let (terminal, completed_revision, total, error) =
            state.registry.terminal_by_token(ticket.token).ok_or(
                NativeOwnerError::LongOperation(LongOperationError::NotFound),
            )?;
        let base_revision = ticket
            .base_revision()
            .ok_or(NativeOwnerError::LongOperation(
                LongOperationError::NotFound,
            ))?;
        if completed_revision < base_revision {
            return Err(NativeOwnerError::Invariant);
        }
        let outcome = match terminal {
            LongOperationState::Succeeded if total == 0 => NativeBatchOutcome::Empty,
            LongOperationState::Succeeded => NativeBatchOutcome::Committed,
            LongOperationState::Cancelled => NativeBatchOutcome::Cancelled,
            LongOperationState::Failed
                if error == Some(StableErrorCode::ManualRecoveryRequired) =>
            {
                NativeBatchOutcome::Uncertain
            }
            LongOperationState::Failed => NativeBatchOutcome::Failed,
            _ => return Err(NativeOwnerError::Invariant),
        };
        Ok(NativeBatchCompletionReceipt {
            instance: ticket.instance.clone(),
            sequence: ticket.sequence(),
            base_revision,
            completed_revision,
            outcome,
        })
    }

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

    pub(super) fn batch_lock(&self) -> Result<MigrationLock, NativeOwnerError> {
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
        if self.transaction.blocked()
            || crate::routing_preset::pending(self.transaction.desired_paths())
        {
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
        state.active = Some((
            token,
            ActiveCancellation::Subscription(cancellation.clone()),
        ));
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
        self.complete_subscription_batch_with_store(
            job,
            now_millis,
            commit_subscription_refresh_batch,
        )
    }

    /// The automatic batch is allowed to commit only if its exact preference
    /// remains enabled while the same migration lock protects the store write.
    /// Off/preference changes after a fetch produce no store replacement.
    pub fn complete_scheduled_subscription_batch<N: FnOnce() -> u64>(
        &mut self,
        job: NativeSubscriptionBatch,
        expected_generation: u64,
        expected_preference_revision: u64,
        now_millis: N,
    ) -> Result<(), NativeOwnerError> {
        self.complete_subscription_batch_with_store_and_schedule(
            job,
            now_millis,
            commit_subscription_refresh_batch,
            Some((expected_generation, expected_preference_revision)),
        )
    }

    // Fixed production store function above; the seam permits deterministic
    // post-rename failures in tests without a client-selected writer or path.
    pub(super) fn complete_subscription_batch_with_store<N, F>(
        &mut self,
        job: NativeSubscriptionBatch,
        now_millis: N,
        commit: F,
    ) -> Result<(), NativeOwnerError>
    where
        N: FnOnce() -> u64,
        F: FnOnce(
            &Path,
            u32,
            SubscriptionRefreshBatchSnapshot,
            Vec<SubscriptionRefreshBatchEntries>,
            u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>,
    {
        self.complete_subscription_batch_with_store_and_schedule(job, now_millis, commit, None)
    }

    fn complete_subscription_batch_with_store_and_schedule<N, F>(
        &mut self,
        job: NativeSubscriptionBatch,
        now_millis: N,
        commit: F,
        schedule: Option<(u64, u64)>,
    ) -> Result<(), NativeOwnerError>
    where
        N: FnOnce() -> u64,
        F: FnOnce(
            &Path,
            u32,
            SubscriptionRefreshBatchSnapshot,
            Vec<SubscriptionRefreshBatchEntries>,
            u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>,
    {
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
            let outcome =
                self.commit_subscription_batch_work(&mut state, job, now_millis, commit, schedule);
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

    fn commit_subscription_batch_work<N, F>(
        &mut self,
        state: &mut BatchOwnerState,
        job: NativeSubscriptionBatch,
        now_millis: N,
        commit: F,
        schedule: Option<(u64, u64)>,
    ) -> Result<bool, NativeOwnerError>
    where
        N: FnOnce() -> u64,
        F: FnOnce(
            &Path,
            u32,
            SubscriptionRefreshBatchSnapshot,
            Vec<SubscriptionRefreshBatchEntries>,
            u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>,
    {
        if let Some(error) = job.failure {
            return Err(batch_work_error(error));
        }
        let prepared = job.work.into_prepared().map_err(batch_work_error)?;
        let _lock = self.batch_lock()?;
        if let Some((generation, preference_revision)) = schedule {
            if self.required_ownership.map(|fence| fence.generation) != Some(generation) {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            // batch_lock already holds the migration lock and proved this
            // exact committed owner. Do not reacquire it here.
            let preference = read_schedule_preference_locked(
                self.transaction.cutover_paths(),
                self.transaction.uid(),
                generation,
            )
            .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
            if preference.owner_generation != generation {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            if preference.revision != preference_revision
                || preference.schedule == RefreshSchedule::Off
            {
                return Err(NativeOwnerError::Coordinator(
                    CoordinatorError::RevisionConflict,
                ));
            }
        }
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
        let result = commit(
            self.transaction.store_path(),
            self.transaction.uid(),
            snapshot,
            updates,
            now_millis(),
        )
        .map_err(|error| {
            if error == SubscriptionMutationCommitError::StoreIo {
                // Atomic replacement can fail after rename (permissions or
                // directory fsync). Never assert no change, retry or overwrite
                // unknown current bytes; block the shared native owner.
                self.transaction.block();
                NativeOwnerError::ManualRecoveryRequired
            } else {
                NativeOwnerError::Subscription(subscription_store_error(error))
            }
        });
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
