// SPDX-License-Identifier: MIT

//! One supervised batch per runtime instance. Network work never owns the
//! dispatcher mutex. Unary peers only admit, poll or cancel bounded records.

use super::*;
use crate::native_coordinator::{NativeBatchTicket, NativeSubscriptionBatch};
use crate::subscription_batch_work::BatchWorkStep;

pub(super) const METHODS: &[&str] = &[
    "subscriptions.refresh_all",
    "operations.get",
    "operations.cancel",
];

pub(super) struct BatchWork {
    pub job: NativeSubscriptionBatch,
    pub transport: SharedSubscriptionTransport,
    pub record_ids: RecordIdGenerator,
}

#[derive(Default)]
pub(super) struct BatchScheduler {
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    stopping: Arc<AtomicBool>,
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
    pub(super) fn dispatch(
        &self,
        request: &Value,
        instance: &str,
        dispatcher: &Arc<Mutex<RuntimeDispatcher>>,
        pool: &remote_fetch::RemoteFetchPool,
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
        let (projection, work) = match owner.batch_control(request, instance) {
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
                ticket: Some(work.job.supervisor_ticket()),
            };
            let stopping = Arc::clone(&self.stopping);
            let pool = pool.clone();
            match thread::Builder::new()
                .name("omavless-batch".to_owned())
                .spawn(move || {
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
        if let Ok(mut worker) = self.worker.lock() {
            if let Ok(mut dispatcher) = dispatcher.lock()
                && let RuntimeDispatcher::Native(owner) = &mut *dispatcher
            {
                owner.batch_stop();
            }
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
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
            if !owner.batch_progress(&work.job) {
                return;
            }
        }
        let step = work
            .job
            .step(&work.transport, pool, &mut || work.record_ids.next());
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
                owner.batch_finish(work.job);
                supervisor.ticket = None;
                return;
            }
        }
    }
}
