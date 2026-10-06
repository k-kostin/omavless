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
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    Automatic {
        job: native_coordinator::AutomaticSubscriptionBatch,
        clock: developer_subscription_schedule::Clock,
        transport: SharedSubscriptionTransport,
        record_ids: RecordIdGenerator,
    },
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
            #[cfg(any(test, feature = "developer-subscription-schedule"))]
            Self::Automatic { job, .. } => job.supervisor_ticket(),
            Self::Subscription { job, .. } => job.supervisor_ticket(),
            Self::Provider { job, .. } => job.supervisor_ticket(),
            Self::Probe { job, .. } => job.supervisor_ticket(),
        }
    }
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    fn automatic(&self) -> bool {
        #[cfg(any(test, feature = "developer-subscription-schedule"))]
        if matches!(self, Self::Automatic { .. }) {
            return true;
        }
        false
    }
}

#[derive(Default)]
pub(super) struct BatchScheduler {
    worker: Mutex<Option<WorkerHandle>>,
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    drained: Mutex<Option<DrainedAutomaticAttempt>>,
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    context: Arc<()>,
    stopping: Arc<AtomicBool>,
    #[cfg(test)]
    pub(super) fail_next_spawn: AtomicBool,
}

struct WorkerHandle {
    handle: thread::JoinHandle<()>,
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    automatic: Option<NativeBatchTicket>,
}

/// Private, non-cloneable original-supervisor drain. No PID, instance string,
/// caller token or is_finished observation can construct this proof.
#[cfg(any(test, feature = "developer-subscription-schedule"))]
pub(crate) struct DrainedAutomaticAttempt {
    ticket: NativeBatchTicket,
    context: Arc<()>,
    dispatcher: std::sync::Weak<Mutex<RuntimeDispatcher>>,
}
#[cfg(any(test, feature = "developer-subscription-schedule"))]
impl DrainedAutomaticAttempt {
    pub(crate) fn ticket(&self) -> &NativeBatchTicket {
        &self.ticket
    }
}

// Runs even when a worker unwinds. Does not format or retain a panic payload.
// The exact ticket cannot terminate a later operation with a reused owner.
struct Supervisor {
    dispatcher: Arc<Mutex<RuntimeDispatcher>>,
    ticket: Option<NativeBatchTicket>,
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    automatic: bool,
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Some(ticket) = self.ticket.take()
            && let Ok(mut dispatcher) = self.dispatcher.lock()
            && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
        {
            #[cfg(any(test, feature = "developer-subscription-schedule"))]
            if self.automatic {
                owner.automatic_lost(ticket);
                return;
            }
            owner.batch_abort(ticket);
        }
    }
}

impl BatchScheduler {
    #[cfg(test)]
    pub(super) fn invalidate_original_drain_for_test(&self, dispatcher: bool) {
        let mut slot = self.drained.lock().unwrap();
        let proof = slot.as_mut().expect("actual original drain");
        if dispatcher {
            proof.dispatcher = std::sync::Weak::new();
        } else {
            proof.context = Arc::new(());
        }
    }
    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    fn retain_drain(&self, ticket: NativeBatchTicket, dispatcher: &Arc<Mutex<RuntimeDispatcher>>) {
        let eligible=dispatcher.lock().ok().is_some_and(|mut owner| {
            matches!(&mut *owner,RuntimeDispatcher::Native(owner) if owner.automatic_interrupted(&ticket))
        });
        if eligible
            && let Ok(mut slot) = self.drained.lock()
            && slot.is_none()
        {
            *slot = Some(DrainedAutomaticAttempt {
                ticket,
                context: Arc::clone(&self.context),
                dispatcher: Arc::downgrade(dispatcher),
            });
        }
    }

    fn join_worker(&self, worker: WorkerHandle, dispatcher: &Arc<Mutex<RuntimeDispatcher>>) {
        let _ = worker.handle.join();
        #[cfg(any(test, feature = "developer-subscription-schedule"))]
        if let Some(ticket) = worker.automatic {
            self.retain_drain(ticket, dispatcher);
        }
        #[cfg(not(any(test, feature = "developer-subscription-schedule")))]
        let _ = dispatcher;
    }
    fn start_work(
        &self,
        work: BatchWork,
        worker: &mut Option<WorkerHandle>,
        dispatcher: &Arc<Mutex<RuntimeDispatcher>>,
        pool: &remote_fetch::RemoteFetchPool,
    ) -> io::Result<()> {
        if let Some(previous) = worker.take() {
            self.join_worker(previous, dispatcher);
        }
        #[cfg(any(test, feature = "developer-subscription-schedule"))]
        let automatic = work.automatic().then(|| work.ticket());
        let supervisor = Supervisor {
            dispatcher: Arc::clone(dispatcher),
            ticket: Some(work.ticket()),
            #[cfg(any(test, feature = "developer-subscription-schedule"))]
            automatic: work.automatic(),
        };
        let stopping = Arc::clone(&self.stopping);
        let pool = pool.clone();
        match self.spawn(move || run(work, supervisor, &stopping, &pool)) {
            Ok(handle) => {
                *worker = Some(WorkerHandle {
                    handle,
                    #[cfg(any(test, feature = "developer-subscription-schedule"))]
                    automatic,
                })
            }
            Err(error) => {
                // The captured runnable closure/supervisor have been dropped
                // by spawn before this returned refusal; no detached work exists.
                #[cfg(any(test, feature = "developer-subscription-schedule"))]
                if let Some(ticket) = automatic {
                    self.retain_drain(ticket, dispatcher);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    pub(super) fn wake_automatic(
        &self,
        instance: &str,
        dispatcher: &Arc<Mutex<RuntimeDispatcher>>,
        pool: &remote_fetch::RemoteFetchPool,
        clock: developer_subscription_schedule::Clock,
    ) -> std::result::Result<(), native_coordinator::AutomaticRefreshError> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| native_coordinator::AutomaticRefreshError::WorkerLost)?;
        let mut guard = dispatcher
            .lock()
            .map_err(|_| native_coordinator::AutomaticRefreshError::WorkerLost)?;
        let RuntimeDispatcher::Native(owner) = &mut *guard else {
            return Err(developer_subscription_schedule::unavailable());
        };
        if self.stopping.load(Ordering::Acquire) {
            return Err(developer_subscription_schedule::unavailable());
        }
        let work = owner.automatic_start(instance, clock)?;
        drop(guard);
        if let Some(work) = work {
            self.start_work(work, &mut worker, dispatcher, pool)
                .map_err(|_| native_coordinator::AutomaticRefreshError::WorkerLost)?;
        }
        Ok(())
    }
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
            match self.start_work(work, &mut worker, dispatcher, pool) {
                Ok(()) => {}
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
            self.join_worker(worker, dispatcher);
        }
    }

    #[cfg(any(test, feature = "developer-subscription-schedule"))]
    pub(super) fn acknowledge_automatic(
        &self,
        request: &Value,
        instance: &str,
        dispatcher: &Arc<Mutex<RuntimeDispatcher>>,
        clock: &developer_subscription_schedule::Clock,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let parsed = developer_subscription_schedule::parse_acknowledgement(request, instance);
        let (sequence, preference_revision, revision) = match parsed {
            Ok(fields) => fields,
            Err(code) => return error_response(id, 0, code, false, None),
        };
        let Ok(mut worker) = self.worker.lock() else {
            return error_response(id, 0, StableErrorCode::ManualRecoveryRequired, false, None);
        };
        if worker
            .as_ref()
            .is_some_and(|worker| !worker.handle.is_finished())
        {
            return error_response(id, revision, StableErrorCode::Busy, true, None);
        }
        if let Some(previous) = worker.take() {
            self.join_worker(previous, dispatcher);
        }
        let Ok(mut drained) = self.drained.lock() else {
            return error_response(
                id,
                revision,
                StableErrorCode::ManualRecoveryRequired,
                false,
                None,
            );
        };
        let Some(proof) = drained.as_ref() else {
            return error_response(
                id,
                revision,
                StableErrorCode::ManualRecoveryRequired,
                false,
                None,
            );
        };
        if !Arc::ptr_eq(&proof.context, &self.context)
            || proof
                .dispatcher
                .upgrade()
                .is_none_or(|original| !Arc::ptr_eq(&original, dispatcher))
        {
            return error_response(
                id,
                revision,
                StableErrorCode::ManualRecoveryRequired,
                false,
                None,
            );
        }
        let Ok(mut owner) = dispatcher.lock() else {
            return error_response(
                id,
                revision,
                StableErrorCode::ManualRecoveryRequired,
                false,
                None,
            );
        };
        let RuntimeDispatcher::Native(owner) = &mut *owner else {
            return error_response(
                id,
                revision,
                StableErrorCode::CapabilityUnavailable,
                false,
                None,
            );
        };
        match owner.automatic_acknowledge(proof, sequence, preference_revision, revision, clock()) {
            Ok(()) => {
                drained.take();
                success_response(
                    id,
                    owner.revision(),
                    json!({"schemaVersion":1,"acknowledgedUncertain":true,"enabled":false}),
                )
            }
            Err(error) => error_response(
                id,
                owner.revision(),
                developer_subscription_schedule::code(error),
                false,
                None,
            ),
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
            #[cfg(any(test, feature = "developer-subscription-schedule"))]
            if let BatchWork::Automatic { job, clock, .. } = work {
                if let Ok(mut dispatcher) = supervisor.dispatcher.lock()
                    && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
                {
                    let _ = owner.automatic_finish(job, clock());
                    supervisor.ticket = None;
                }
                return;
            }
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
                #[cfg(any(test, feature = "developer-subscription-schedule"))]
                BatchWork::Automatic { job, .. } => owner.automatic_progress(job),
                BatchWork::Subscription { job, .. } => owner.batch_progress(job),
                BatchWork::Provider { job, .. } => owner.provider_progress(job),
                BatchWork::Probe { .. } => unreachable!("probe dispatched before loop"),
            };
            if !valid {
                #[cfg(any(test, feature = "developer-subscription-schedule"))]
                if let BatchWork::Automatic { job, clock, .. } = work {
                    let _ = owner.automatic_finish(job, clock());
                    supervisor.ticket = None;
                }
                return;
            }
        }
        let step = match &mut work {
            #[cfg(any(test, feature = "developer-subscription-schedule"))]
            BatchWork::Automatic {
                job,
                transport,
                record_ids,
                ..
            } => job
                .step(transport, pool, &mut || record_ids.next())
                .map_err(|_| ()),
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
                    #[cfg(any(test, feature = "developer-subscription-schedule"))]
                    BatchWork::Automatic { job, clock, .. } => {
                        let _ = owner.automatic_finish(job, clock());
                    }
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
