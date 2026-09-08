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
    "routing.refresh_providers",
    "operations.get",
    "operations.cancel",
];

pub(super) enum BatchWork {
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
        }
    }
}

#[derive(Default)]
pub(super) struct BatchScheduler {
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    stopping: Arc<AtomicBool>,
    #[cfg(test)]
    pub(super) fail_next_spawn: AtomicBool,
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
        let handle = if let Ok(mut worker) = self.worker.lock() {
            if let Ok(mut dispatcher) = dispatcher.lock()
                && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
            {
                owner.batch_stop();
            }
            worker.take()
        } else {
            None
        };
        // Do not hold admission while the bounded provider request drains.
        // An already accepted peer must receive daemon_restarting promptly,
        // rather than waiting as long as the 25-second provider deadline.
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
                    BatchWork::Subscription { job, .. } => owner.batch_finish(job),
                    BatchWork::Provider { job, transport } => {
                        owner.provider_finish(job, &transport)
                    }
                }
                supervisor.ticket = None;
                return;
            }
        }
    }
}
