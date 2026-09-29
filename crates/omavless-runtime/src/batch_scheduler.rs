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
    finish_attempt_with_receipt, preflight_attempt,
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
pub(super) struct BatchScheduler {
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    stopping: Arc<AtomicBool>,
    scheduled_sequence: AtomicU64,
    #[cfg(test)]
    pub(super) fail_next_spawn: AtomicBool,
}

#[allow(dead_code)] // inactive until a separately reviewed supervised worker exists
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledOnceError {
    Stopping,
    WorkerBusy,
    CounterExhausted,
    Protocol,
    Attempt(AttemptError),
    Owner(native_coordinator::NativeOwnerError),
    Work(BatchWorkError),
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
        &self,
        owner: &mut native_coordinator::OfflineNativeCoordinator<H>,
    ) -> ScheduledResult<()> {
        owner
            .publish_subscription_batch_progress(
                self.job
                    .as_ref()
                    .ok_or(ScheduledOnceError::CompletionUncertain)?,
            )
            .map_err(ScheduledOnceError::Owner)
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
        let _completion = owner.complete_subscription_batch(job, || now_millis);
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
    _worker: std::sync::MutexGuard<'a, Option<thread::JoinHandle<()>>>,
}

impl BatchScheduler {
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
        if worker.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return Err(ScheduledOnceError::WorkerBusy);
        }
        Ok(ScheduledAdmissionGuard {
            scheduler: self,
            _worker: worker,
        })
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
            if let Some(previous) = worker.take() {
                let _ = previous.join();
            }
            let supervisor = Supervisor {
                dispatcher: Arc::clone(dispatcher),
                ticket: Some(work.ticket()),
            };
            let stopping = Arc::clone(&self.stopping);
            let pool = pool.clone();
            match self.spawn(move || {
                run(work, supervisor, &stopping, &pool);
            }) {
                Ok(handle) => *worker = Some(handle),
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
            worker.take()
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
