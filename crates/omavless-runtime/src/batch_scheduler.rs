// SPDX-License-Identifier: MIT

//! One supervised batch per runtime instance. Network work never owns the
//! dispatcher mutex. Unary peers only admit, poll or cancel bounded records.

use super::*;
use crate::native_coordinator::{NativeBatchTicket, NativeSubscriptionBatch};
use crate::provider_refresh::{
    PROVIDER_DISCOVERY_TIMEOUT, ProviderRefreshStep, RuleProviderTransport,
    UnixRuleProviderTransport,
};
use crate::subscription_batch_work::BatchWorkStep;
use crate::subscription_batch_work::{BatchWorkError, BudgetedSubscriptionTransport};
use crate::subscription_schedule_attempt::{
    AttemptError, AttemptSnapshot, AttemptTicket, begin_attempt_for_batch,
    check_attempt_before_step, finish_attempt_with_receipt, preflight_attempt,
};
use crate::subscription_schedule_plan::OwnerFence;
use std::sync::atomic::AtomicU64;

type ScheduledResult<T> = std::result::Result<T, ScheduledOnceError>;

pub(super) const METHODS: &[&str] = &[
    "subscriptions.refresh_all",
    "subscriptions.probe",
    "subscriptions.probe_results",
    "profiles.probe",
    "profiles.probe_results",
    "routing.refresh_providers",
    "operations.get",
    "operations.cancel",
];

pub(super) enum BatchWork {
    Probe {
        job: native_coordinator::NativeSubscriptionProbe,
        lease: auxiliary_core::AuxiliaryLease,
        core: PathBuf,
        scratch: PathBuf,
    },
    Subscription {
        job: NativeSubscriptionBatch,
        transport: SharedSubscriptionTransport,
        record_ids: RecordIdGenerator,
    },
    Provider {
        job: native_coordinator::NativeProviderRefresh,
        transport: UnixRuleProviderTransport,
    },
}
impl BatchWork {
    fn ticket(&self) -> NativeBatchTicket {
        match self {
            Self::Subscription { job, .. } => job.supervisor_ticket(),
            Self::Provider { job, .. } => job.supervisor_ticket(),
            Self::Probe { job, .. } => job.supervisor_ticket(),
        }
    }
}

#[derive(Default)]
struct WorkerSlot {
    handle: Option<thread::JoinHandle<()>>,
    scheduled: Option<NativeBatchTicket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleOffError {
    DispatcherUnavailable,
    AdmissionUnavailable,
    Owner(native_coordinator::NativeOwnerError),
    Preference(crate::subscription_schedule_preference::PreferenceError),
    CancellationUncertain(native_coordinator::NativeOwnerError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScheduleOffResult {
    pub(crate) preference: crate::subscription_schedule_preference::PreferenceSnapshot,
    pub(crate) cancellation_requested: bool,
}

#[derive(Default)]
pub(super) struct BatchScheduler {
    worker: Mutex<WorkerSlot>,
    scheduled_inhibited: AtomicBool,
    stopping: Arc<AtomicBool>,
    scheduled_sequence: AtomicU64,
    #[cfg(test)]
    pub(super) fail_next_spawn: AtomicBool,
    #[cfg(test)]
    pub(super) panic_after_scheduled_finish: AtomicBool,
    #[cfg(test)]
    pub(super) fail_next_off_after_publish: AtomicBool,
}

#[allow(dead_code)] // inactive until a separately reviewed supervised worker exists
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledOnceError {
    Stopping,
    AdmissionUncertain,
    WorkerBusy,
    CounterExhausted,
    Protocol,
    Attempt(AttemptError),
    Owner(native_coordinator::NativeOwnerError),
    Work(BatchWorkError),
    StepNotAuthorized,
    DispatcherUnavailable,
    SpawnFailed,
    AbortUncertain,
    CompletionUncertain,
}

/// Inactive, one-shot composition seam. No timer, IPC, socket or production
/// caller exists. Dropping it is NOT proof of failure: its durable Started
/// entry conservatively blocks future automatic attempts until reviewed.
#[must_use = "finish or abort under the serialized owner; a lost attempt remains uncertain"]
#[allow(dead_code)] // inactive composition seam, exercised by synthetic tests
pub(crate) struct ScheduledBatchAttempt {
    job: Option<NativeSubscriptionBatch>,
    batch_ticket: NativeBatchTicket,
    operation_id: String,
    journal_ticket: AttemptTicket,
    pool: remote_fetch::RemoteFetchPool,
    paths: cutover::CutoverPaths,
    uid: u32,
    generation: u64,
    step_authorized: bool,
}

#[allow(dead_code)] // no live scheduler/IPC registration in this checkpoint
impl ScheduledBatchAttempt {
    /// Owner-side cancellation is keyed to this private scheduler operation,
    /// never to a client-provided ID. The registry decides whether it still
    /// precedes the commit fence.
    pub(crate) fn cancel<H: lifecycle::LifecycleHost>(
        &self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
    ) -> ScheduledResult<bool> {
        let request = make_request(
            "scheduled-cancel",
            "operations.cancel",
            json!({
                "instanceId": self.batch_ticket.instance(),
                "operationId": self.operation_id,
            }),
        )
        .map_err(|_| ScheduledOnceError::Protocol)?;
        owner
            .cancel_subscription_batch(&request)
            .map_err(ScheduledOnceError::Owner)
    }

    /// The caller performs this check under the serialized owner, then drops
    /// that lock before calling step. The shared pool is supplied by the same
    /// runtime instance as manual fetches; a new pool would break the limit.
    pub(crate) fn progress<H: lifecycle::LifecycleHost>(
        &mut self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
    ) -> ScheduledResult<()> {
        self.step_authorized = false;
        let (paths, uid, generation) = owner
            .scheduled_journal_scope()
            .map_err(ScheduledOnceError::Owner)?;
        if generation != self.generation || uid != self.uid || paths != self.paths {
            return Err(ScheduledOnceError::Attempt(AttemptError::StaleOwner));
        }
        check_attempt_before_step(
            &self.paths,
            self.uid,
            &self.journal_ticket,
            OwnerFence {
                expected_generation: self.generation,
                current_generation: generation,
                expected_revision: self
                    .batch_ticket
                    .base_revision()
                    .ok_or(ScheduledOnceError::CompletionUncertain)?,
                current_revision: owner.revision(),
            },
        )
        .map_err(ScheduledOnceError::Attempt)?;
        owner
            .publish_subscription_batch_progress(
                self.job
                    .as_ref()
                    .ok_or(ScheduledOnceError::CompletionUncertain)?,
            )
            .map_err(ScheduledOnceError::Owner)?;
        self.step_authorized = true;
        Ok(())
    }

    pub(crate) fn step<T, G>(
        &mut self,
        transport: &T,
        next_record_id: &mut G,
    ) -> ScheduledResult<BatchWorkStep>
    where
        T: BudgetedSubscriptionTransport,
        G: FnMut() -> String,
    {
        if !std::mem::take(&mut self.step_authorized) {
            return Err(ScheduledOnceError::StepNotAuthorized);
        }
        self.job
            .as_mut()
            .ok_or(ScheduledOnceError::CompletionUncertain)?
            .step(transport, &self.pool, next_record_id)
            .map_err(ScheduledOnceError::Work)
    }

    /// The owner may return an error after a proved terminal failure or
    /// cancellation. Only its exact typed terminal receipt settles the
    /// journal; an absent receipt leaves Started uncertain.
    pub(crate) fn finish<H: lifecycle::LifecycleHost>(
        mut self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
        now_millis: u64,
        now_secs: u64,
    ) -> ScheduledResult<AttemptSnapshot> {
        let job = self
            .job
            .take()
            .ok_or(ScheduledOnceError::CompletionUncertain)?;
        let _completion = owner.complete_scheduled_subscription_batch(
            job,
            self.generation,
            self.journal_ticket.preference_revision(),
            || now_millis,
        );
        self.settle(owner, now_secs)
    }

    /// For failed worker launch before any fetch. A failed abort cannot be
    /// converted into a guessed result or automatic retry.
    pub(crate) fn abort<H: lifecycle::LifecycleHost>(
        self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
        now_secs: u64,
    ) -> ScheduledResult<AttemptSnapshot> {
        owner
            .abort_subscription_batch(self.batch_ticket.clone())
            .map_err(|_| ScheduledOnceError::AbortUncertain)?;
        self.settle(owner, now_secs)
    }

    /// A shutdown may have already minted the exact failure receipt while
    /// revoking the active worker. Consume that receipt instead of treating a
    /// second abort's NotFound as proof of an uncertain batch. If no terminal
    /// receipt exists, abort only this ticket; any failure stays Started.
    pub(crate) fn abort_or_settle<H: lifecycle::LifecycleHost>(
        self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
        now_secs: u64,
    ) -> ScheduledResult<AttemptSnapshot> {
        if owner.subscription_batch_receipt(&self.batch_ticket).is_ok() {
            self.settle(owner, now_secs)
        } else {
            self.abort(owner, now_secs)
        }
    }

    fn settle<H: lifecycle::LifecycleHost>(
        self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
        now_secs: u64,
    ) -> ScheduledResult<AttemptSnapshot> {
        let receipt = owner
            .subscription_batch_receipt(&self.batch_ticket)
            .map_err(|_| ScheduledOnceError::CompletionUncertain)?;
        let revision = owner.revision();
        finish_attempt_with_receipt(
            &self.paths,
            self.uid,
            self.journal_ticket,
            receipt,
            now_secs,
            OwnerFence {
                expected_generation: self.generation,
                current_generation: self.generation,
                expected_revision: revision,
                current_revision: revision,
            },
        )
        .map_err(ScheduledOnceError::Attempt)
    }
}

// Runs even when a worker unwinds. Does not format or retain a panic payload.
// The exact ticket cannot terminate a later operation with a reused owner.
struct Supervisor {
    dispatcher: Arc<Mutex<RuntimeDispatcher>>,
    ticket: Option<NativeBatchTicket>,
}

/// The inactive scheduled worker owns its attempt until an exact owner
/// completion consumes it. On spawn failure, pre-commit panic or shutdown,
/// this guard runs only after any dispatcher guard has been dropped. If it
/// cannot obtain/settle an exact receipt, durable Started remains blocked.
struct ScheduledSupervisor {
    dispatcher: Arc<Mutex<RuntimeDispatcher>>,
    attempt: Option<ScheduledBatchAttempt>,
    finished_at_secs: u64,
}

impl Drop for ScheduledSupervisor {
    fn drop(&mut self) {
        let Some(attempt) = self.attempt.take() else {
            return;
        };
        if let Ok(mut dispatcher) = self.dispatcher.lock()
            && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
        {
            let _ = owner.scheduled_abort_or_settle(attempt, self.finished_at_secs);
        }
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Some(ticket) = self.ticket.take()
            && let Ok(mut dispatcher) = self.dispatcher.lock()
            && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
        {
            owner.batch_abort(ticket);
        }
    }
}

/// Reservation must be acquired before the serialized owner, matching manual
/// dispatch lock order. It is released before any transport step.
#[allow(dead_code)]
pub(crate) struct ScheduledAdmissionGuard<'a> {
    scheduler: &'a BatchScheduler,
    _worker: std::sync::MutexGuard<'a, WorkerSlot>,
}

#[allow(dead_code)] // inactive one-shot seam, constructed by synthetic tests
pub(crate) struct ScheduledOnceInputs<'a> {
    pub(crate) dispatcher: &'a Arc<Mutex<RuntimeDispatcher>>,
    pub(crate) pool: &'a remote_fetch::RemoteFetchPool,
    pub(crate) instance: &'a str,
    pub(crate) started_at_secs: u64,
    pub(crate) finished_at_secs: u64,
    pub(crate) finished_at_millis: u64,
}

impl BatchScheduler {
    #[cfg(test)]
    pub(super) fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(super) fn worker_finished(&self) -> bool {
        self.worker.lock().is_ok_and(|worker| {
            worker
                .handle
                .as_ref()
                .is_none_or(thread::JoinHandle::is_finished)
        })
    }

    /// Reserve manual/scheduled admission before taking the owner lock. An
    /// active manual worker or shutdown refuses immediately; a completed
    /// handle may be joined later by the normal dispatch path.
    #[allow(dead_code)]
    pub(crate) fn reserve_scheduled(&self) -> ScheduledResult<ScheduledAdmissionGuard<'_>> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| ScheduledOnceError::WorkerBusy)?;
        if self.stopping.load(Ordering::Acquire) {
            return Err(ScheduledOnceError::Stopping);
        }
        if self.scheduled_inhibited.load(Ordering::Acquire) {
            return Err(ScheduledOnceError::AdmissionUncertain);
        }
        if worker
            .handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
        {
            return Err(ScheduledOnceError::WorkerBusy);
        }
        Ok(ScheduledAdmissionGuard {
            scheduler: self,
            _worker: worker,
        })
    }

    /// Inactive one-shot worker composition. There is deliberately no daemon,
    /// timer, IPC or CLI caller. The transport is injected only by synthetic
    /// tests at this checkpoint. Admission uses the same worker slot and lock
    /// order as manual batches; ownership is released before spawn so a failed
    /// spawn cannot deadlock while dropping its supervisor.
    #[allow(dead_code)]
    pub(crate) fn start_scheduled_once<T, G>(
        &self,
        inputs: ScheduledOnceInputs<'_>,
        transport: T,
        mut next_record_id: G,
    ) -> ScheduledResult<()>
    where
        T: BudgetedSubscriptionTransport + Send + 'static,
        G: FnMut() -> String + Send + 'static,
    {
        let mut reservation = self.reserve_scheduled()?;
        if let Some(previous) = reservation._worker.handle.take() {
            let _ = previous.join();
        }
        reservation._worker.scheduled = None;
        let attempt = {
            let mut dispatcher_guard = inputs
                .dispatcher
                .lock()
                .map_err(|_| ScheduledOnceError::DispatcherUnavailable)?;
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher_guard else {
                return Err(ScheduledOnceError::DispatcherUnavailable);
            };
            owner.scheduled_admit(
                &reservation,
                inputs.pool,
                inputs.instance,
                inputs.started_at_secs,
            )?
        };
        let scheduled = attempt.batch_ticket.clone();
        let supervisor = ScheduledSupervisor {
            dispatcher: Arc::clone(inputs.dispatcher),
            attempt: Some(attempt),
            finished_at_secs: inputs.finished_at_secs,
        };
        let stopping = Arc::clone(&self.stopping);
        #[cfg(test)]
        let panic_after_finish = self
            .panic_after_scheduled_finish
            .swap(false, Ordering::AcqRel);
        #[cfg(not(test))]
        let panic_after_finish = false;
        let handle = self
            .spawn(move || {
                run_scheduled_once(
                    supervisor,
                    &stopping,
                    &transport,
                    &mut next_record_id,
                    inputs.finished_at_millis,
                    panic_after_finish,
                );
            })
            .map_err(|_| ScheduledOnceError::SpawnFailed)?;
        reservation._worker.handle = Some(handle);
        reservation._worker.scheduled = Some(scheduled);
        Ok(())
    }

    /// Inactive, synthetic-only Off entry point. Cancellation is cooperative;
    /// success confirms durable Off, never completion of an in-flight request.
    #[allow(dead_code)]
    pub(crate) fn disable_scheduled(
        &self,
        dispatcher: &Arc<Mutex<RuntimeDispatcher>>,
        expected_revision: u64,
    ) -> std::result::Result<ScheduleOffResult, ScheduleOffError> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| ScheduleOffError::AdmissionUnavailable)?;
        let mut dispatcher = dispatcher
            .lock()
            .map_err(|_| ScheduleOffError::DispatcherUnavailable)?;
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return Err(ScheduleOffError::DispatcherUnavailable);
        };
        let result = owner.scheduled_disable(
            expected_revision,
            worker.scheduled.as_ref(),
            #[cfg(test)]
            self.fail_next_off_after_publish
                .swap(false, Ordering::AcqRel),
        );
        if matches!(
            result,
            Err(ScheduleOffError::Preference(
                crate::subscription_schedule_preference::PreferenceError::WriteUncertain
            )) | Err(ScheduleOffError::CancellationUncertain(_))
        ) {
            self.scheduled_inhibited.store(true, Ordering::Release);
        }
        result
    }
}

fn run_scheduled_once<T, G>(
    mut supervisor: ScheduledSupervisor,
    stopping: &AtomicBool,
    transport: &T,
    next_record_id: &mut G,
    finished_at_millis: u64,
    panic_after_finish: bool,
) where
    T: BudgetedSubscriptionTransport,
    G: FnMut() -> String,
{
    loop {
        if stopping.load(Ordering::Acquire) {
            return;
        }
        {
            let Ok(mut dispatcher) = supervisor.dispatcher.lock() else {
                return;
            };
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                return;
            };
            let Some(attempt) = supervisor.attempt.as_mut() else {
                return;
            };
            if owner.scheduled_progress(attempt).is_err() {
                return;
            }
        }
        if stopping.load(Ordering::Acquire) {
            return;
        }
        let Some(attempt) = supervisor.attempt.as_mut() else {
            return;
        };
        let step = attempt.step(transport, next_record_id);
        match step {
            Ok(BatchWorkStep::Busy) => thread::sleep(Duration::from_millis(20)),
            Ok(BatchWorkStep::Advanced) => {}
            Ok(BatchWorkStep::Ready) | Err(_) => {
                let Ok(mut dispatcher) = supervisor.dispatcher.lock() else {
                    return;
                };
                let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                    return;
                };
                if stopping.load(Ordering::Acquire) {
                    return;
                }
                let Some(attempt) = supervisor.attempt.take() else {
                    return;
                };
                // The exact terminal receipt and journal settlement happen
                // while this one serialized owner is held. If either is
                // uncertain, Started remains a durable no-retry blocker.
                let _ = owner.scheduled_finish(
                    attempt,
                    finished_at_millis,
                    supervisor.finished_at_secs,
                );
                drop(dispatcher);
                if panic_after_finish {
                    panic!("synthetic scheduled post-completion failure");
                }
                return;
            }
        }
    }
}

impl ScheduledAdmissionGuard<'_> {
    /// Inactive synthetic admission seam. The caller now holds the serialized
    /// owner after this guard, preserving manual dispatch lock order. No
    /// transport is called here. A future integration still needs supervised
    /// worker registration and interrupted-attempt UI.
    #[allow(dead_code)]
    pub(crate) fn admit_scheduled_once<H: lifecycle::LifecycleHost>(
        &self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
        pool: &remote_fetch::RemoteFetchPool,
        instance: &str,
        now_secs: u64,
    ) -> ScheduledResult<ScheduledBatchAttempt> {
        if self.scheduler.stopping.load(Ordering::Acquire) {
            return Err(ScheduledOnceError::Stopping);
        }
        let (paths, uid, generation) = owner
            .scheduled_journal_scope()
            .map_err(ScheduledOnceError::Owner)?;
        let revision = owner.revision();
        let fence = OwnerFence {
            expected_generation: generation,
            current_generation: generation,
            expected_revision: revision,
            current_revision: revision,
        };
        let preference_revision = preflight_attempt(&paths, uid, instance, now_secs, fence)
            .map_err(ScheduledOnceError::Attempt)?;
        let old_sequence = self
            .scheduler
            .scheduled_sequence
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| ScheduledOnceError::CounterExhausted)?;
        let operation_id = format!("scheduled-refresh-{:016x}", old_sequence + 1);
        let request = make_request(
            "scheduled-refresh",
            "subscriptions.refresh_all",
            json!({
                "instanceId": instance,
                "operationId": operation_id,
                "expectedRevision": revision,
            }),
        )
        .map_err(|_| ScheduledOnceError::Protocol)?;
        let job = owner
            .start_subscription_batch(&request)
            .map_err(ScheduledOnceError::Owner)?
            .ok_or(ScheduledOnceError::Protocol)?;
        let batch_ticket = job.supervisor_ticket();
        let journal_ticket = match begin_attempt_for_batch(
            &paths,
            uid,
            &batch_ticket,
            preference_revision,
            now_secs,
            fence,
        ) {
            Ok(ticket) => ticket,
            Err(error) => {
                owner
                    .abort_subscription_batch(batch_ticket)
                    .map_err(|_| ScheduledOnceError::AbortUncertain)?;
                return Err(ScheduledOnceError::Attempt(error));
            }
        };
        Ok(ScheduledBatchAttempt {
            job: Some(job),
            batch_ticket,
            operation_id,
            journal_ticket,
            pool: pool.clone(),
            paths,
            uid,
            generation,
            step_authorized: false,
        })
    }
}

impl BatchScheduler {
    fn spawn(&self, work: impl FnOnce() + Send + 'static) -> io::Result<thread::JoinHandle<()>> {
        #[cfg(test)]
        if self.fail_next_spawn.swap(false, Ordering::AcqRel) {
            return Err(io::Error::other("synthetic spawn refusal"));
        }
        thread::Builder::new()
            .name("omavless-batch".to_owned())
            .spawn(work)
    }

    pub(super) fn dispatch(
        &self,
        request: &Value,
        instance: &str,
        dispatcher: &Arc<Mutex<RuntimeDispatcher>>,
        pool: &remote_fetch::RemoteFetchPool,
        directory: &Path,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        // The worker never acquires this mutex. It serializes admission against
        // shutdown and bounds retained thread handles independently of peers.
        let Ok(mut worker) = self.worker.lock() else {
            return error_response(id, 0, StableErrorCode::InternalError, false, None);
        };
        let Ok(mut owner_guard) = dispatcher.lock() else {
            return error_response(id, 0, StableErrorCode::InternalError, false, None);
        };
        let RuntimeDispatcher::Native(owner) = &mut *owner_guard else {
            return error_response(id, 0, StableErrorCode::UnknownMethod, false, None);
        };
        if self.stopping.load(Ordering::Acquire) {
            return error_response(
                id,
                owner.revision(),
                StableErrorCode::DaemonRestarting,
                true,
                None,
            );
        }
        let (projection, work) = if request["method"] == "routing.refresh_providers" {
            let admission = match owner.provider_preflight(request, instance) {
                Ok(admission) => admission,
                Err(error) => {
                    return error_response(
                        id,
                        owner.revision(),
                        error.stable_code(),
                        error.stable_code() == StableErrorCode::Busy,
                        None,
                    );
                }
            };
            match admission {
                native_coordinator::ProviderRefreshAdmission::Replay(projection) => {
                    (projection, None)
                }
                native_coordinator::ProviderRefreshAdmission::Discover(snapshot) => {
                    let Some(permit) = pool.try_acquire() else {
                        return error_response(
                            id,
                            owner.revision(),
                            StableErrorCode::Busy,
                            true,
                            None,
                        );
                    };
                    drop(owner_guard);
                    // Discovery reserves no registry/job slot. Release scheduler
                    // admission too, so slow starts cannot queue poll/cancel or
                    // runtime shutdown behind controller I/O. The four-work
                    // pool bounds concurrent discoveries; final admission races
                    // are resolved atomically after reacquiring both locks.
                    drop(worker);
                    let transport =
                        UnixRuleProviderTransport::new(directory, Uid::current().as_raw());
                    let targets = transport.discover(PROVIDER_DISCOVERY_TIMEOUT);
                    drop(permit);
                    worker = match self.worker.lock() {
                        Ok(guard) => guard,
                        Err(_) => {
                            return error_response(
                                id,
                                0,
                                StableErrorCode::InternalError,
                                false,
                                None,
                            );
                        }
                    };
                    owner_guard = match dispatcher.lock() {
                        Ok(guard) => guard,
                        Err(_) => {
                            return error_response(
                                id,
                                0,
                                StableErrorCode::InternalError,
                                false,
                                None,
                            );
                        }
                    };
                    let RuntimeDispatcher::Native(owner) = &mut *owner_guard else {
                        return error_response(
                            id,
                            0,
                            StableErrorCode::CapabilityUnavailable,
                            false,
                            None,
                        );
                    };
                    if self.stopping.load(Ordering::Acquire) {
                        return error_response(
                            id,
                            owner.revision(),
                            StableErrorCode::DaemonRestarting,
                            true,
                            None,
                        );
                    }
                    let targets = match targets {
                        Ok(targets) => targets,
                        Err(error) => {
                            return error_response(
                                id,
                                owner.revision(),
                                native_coordinator::NativeOwnerError::Provider(error).stable_code(),
                                false,
                                None,
                            );
                        }
                    };
                    let job = match owner.provider_start(request, snapshot, targets) {
                        Ok(job) => job,
                        Err(error) => {
                            return error_response(
                                id,
                                owner.revision(),
                                error.stable_code(),
                                false,
                                None,
                            );
                        }
                    };
                    let lookup = make_request(
                        "provider-projection",
                        "operations.get",
                        json!({"instanceId": request["params"]["instanceId"], "operationId": request["params"]["operationId"]}),
                    )?;
                    let (projection, _) = match owner.batch_control(&lookup, instance) {
                        Ok(value) => value,
                        Err(error) => {
                            return error_response(
                                id,
                                owner.revision(),
                                error.stable_code(),
                                false,
                                None,
                            );
                        }
                    };
                    (
                        projection,
                        job.map(|job| BatchWork::Provider { job, transport }),
                    )
                }
            }
        } else {
            match owner.batch_control(request, instance) {
                Ok(result) => result,
                Err(error) => {
                    return error_response(
                        id,
                        owner.revision(),
                        error.stable_code(),
                        error.stable_code() == StableErrorCode::Busy,
                        None,
                    );
                }
            }
        };
        let RuntimeDispatcher::Native(owner) = &mut *owner_guard else {
            return error_response(id, 0, StableErrorCode::CapabilityUnavailable, false, None);
        };
        let revision = owner.revision();
        drop(owner_guard);
        if let Some(work) = work {
            // A new job can only be admitted after its predecessor terminalized
            // under the owner mutex. The predecessor has no remaining I/O.
            if let Some(previous) = worker.handle.take() {
                let _ = previous.join();
            }
            worker.scheduled = None;
            let supervisor = Supervisor {
                dispatcher: Arc::clone(dispatcher),
                ticket: Some(work.ticket()),
            };
            let stopping = Arc::clone(&self.stopping);
            let pool = pool.clone();
            match self.spawn(move || {
                run(work, supervisor, &stopping, &pool);
            }) {
                Ok(handle) => worker.handle = Some(handle),
                // Failed spawn drops the captured supervisor, terminalizing the
                // admitted operation without a detached/lost private payload.
                Err(_) => {
                    return error_response(
                        id,
                        revision,
                        StableErrorCode::InternalError,
                        false,
                        None,
                    );
                }
            }
        }
        success_response(id, revision, projection)
    }

    pub(super) fn stop(&self, dispatcher: &Arc<Mutex<RuntimeDispatcher>>) {
        self.stopping.store(true, Ordering::Release);
        // Same lock order as admission. The worker only takes dispatcher.
        let mut auxiliary = None;
        let handle = if let Ok(mut worker) = self.worker.lock() {
            if let Ok(mut dispatcher) = dispatcher.lock()
                && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
            {
                owner.batch_stop();
                auxiliary = owner.auxiliary_slot();
            }
            worker.scheduled = None;
            worker.handle.take()
        } else {
            None
        };
        // Do not hold admission while the bounded provider request drains.
        // An already accepted peer must receive daemon_restarting promptly,
        // rather than waiting as long as the 25-second provider deadline.
        let _auxiliary_guard = auxiliary.map(|slot| slot.quiesce());
        if let Some(worker) = handle {
            let _ = worker.join();
        }
    }
}

fn run(
    mut work: BatchWork,
    mut supervisor: Supervisor,
    stopping: &AtomicBool,
    pool: &remote_fetch::RemoteFetchPool,
) {
    if let BatchWork::Probe {
        job,
        lease,
        core,
        scratch,
    } = work
    {
        run_probe(job, lease, core, scratch, supervisor, stopping, pool);
        return;
    }
    loop {
        if stopping.load(Ordering::Acquire) {
            return;
        }
        {
            let Ok(mut dispatcher) = supervisor.dispatcher.lock() else {
                return;
            };
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                return;
            };
            let valid = match &work {
                BatchWork::Subscription { job, .. } => owner.batch_progress(job),
                BatchWork::Provider { job, .. } => owner.provider_progress(job),
                BatchWork::Probe { .. } => unreachable!("probe dispatched before loop"),
            };
            if !valid {
                return;
            }
        }
        let step = match &mut work {
            BatchWork::Subscription {
                job,
                transport,
                record_ids,
            } => job
                .step(transport, pool, &mut || record_ids.next())
                .map_err(|_| ()),
            BatchWork::Probe { .. } => unreachable!("probe dispatched before loop"),
            BatchWork::Provider { job, transport } => job
                .step(transport, pool)
                .map(|step| match step {
                    ProviderRefreshStep::Busy => BatchWorkStep::Busy,
                    ProviderRefreshStep::Advanced => BatchWorkStep::Advanced,
                    ProviderRefreshStep::Ready => BatchWorkStep::Ready,
                })
                .map_err(|_| ()),
        };
        match step {
            Ok(BatchWorkStep::Busy) => thread::sleep(Duration::from_millis(20)),
            Ok(BatchWorkStep::Advanced) => {}
            Ok(BatchWorkStep::Ready) | Err(_) => {
                let Ok(mut dispatcher) = supervisor.dispatcher.lock() else {
                    return;
                };
                let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                    return;
                };
                if stopping.load(Ordering::Acquire) {
                    owner.batch_stop();
                }
                match work {
                    BatchWork::Subscription { job, .. } => {
                        if let Err(error) = owner.batch_finish(job) {
                            // The exact operation remains queryable through the
                            // owner registry. No automatic journal/timer is
                            // registered yet; never invent a success here.
                            eprintln!(
                                "OmaVLESS batch terminal receipt unavailable: {:?}",
                                error.stable_code()
                            );
                        }
                    }
                    BatchWork::Provider { job, transport } => {
                        owner.provider_finish(job, &transport)
                    }
                    BatchWork::Probe { .. } => unreachable!("probe dispatched before loop"),
                }
                supervisor.ticket = None;
                return;
            }
        }
    }
}

fn run_probe(
    job: native_coordinator::NativeSubscriptionProbe,
    lease: auxiliary_core::AuxiliaryLease,
    core: PathBuf,
    scratch: PathBuf,
    mut supervisor: Supervisor,
    stopping: &AtomicBool,
    pool: &remote_fetch::RemoteFetchPool,
) {
    // This local guard is dropped BEFORE Supervisor on unwind. Child cleanup
    // never happens under the dispatcher mutex, including a panicking worker.
    let lease = lease;
    let cancellation = job.cancellation();
    let valid = || {
        if stopping.load(Ordering::Acquire) || cancellation.requested() || lease.cancelled() {
            return false;
        }
        let Ok(mut dispatcher) = supervisor.dispatcher.lock() else {
            return false;
        };
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return false;
        };
        let valid = owner.probe_progress(&job, 0);
        if !valid {
            cancellation.request();
        }
        valid
    };
    let result =
        crate::subscription_probe_work::execute(&job, &lease, &core, &scratch, pool, &valid);
    // A result, including cancellation, is not terminal until the owned child
    // has been reaped. A failed cleanup poisons the lifecycle owner.
    let result = if lease.finish().is_err() {
        Err(StableErrorCode::ManualRecoveryRequired)
    } else {
        result
    };
    drop(lease);
    let Ok(mut dispatcher) = supervisor.dispatcher.lock() else {
        return;
    };
    let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
        return;
    };
    owner.probe_finish(job, result);
    supervisor.ticket = None;
}
