// SPDX-License-Identifier: MIT
//! One subordinate writer. Neither its mailbox nor its cache is a lifecycle
//! mutex. Dropping this handle never waits for controller/filesystem I/O.
use super::{State, file};
use crate::traffic::TrafficCounters;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, mpsc};

#[derive(Clone, Copy)]
pub(crate) enum Control {
    Configure(bool),
    Reset,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Busy,
    Conflict,
    Unavailable,
    Unknown,
}

enum Command {
    Tick,
    Control {
        expected: u64,
        operation: Option<(String, crate::mutation::MutationDigest)>,
        action: Control,
        utc: u64,
        reply: mpsc::SyncSender<Result<Value, Error>>,
    },
}

struct Receipt {
    operation: String,
    digest: crate::mutation::MutationDigest,
    outcome: Option<Result<Value, Error>>,
}
type Receipts = Arc<Mutex<std::collections::VecDeque<Receipt>>>;

pub(crate) struct Worker {
    sender: mpsc::SyncSender<Command>,
    receipts: Receipts,
    cache: Arc<Mutex<Option<State>>>,
    next_tick: Mutex<std::time::Instant>,
    finished: Arc<std::sync::atomic::AtomicBool>,
}

impl Worker {
    #[cfg(test)]
    pub(crate) fn start(
        store: file::Store,
        collect: impl Fn() -> Option<TrafficCounters> + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::start_with(store, collect, || {})
    }

    /// Construction itself performs no filesystem/controller I/O. The fixed
    /// parent comes only from the registered owner, never a client path.
    pub(crate) fn spawn(
        parent: std::path::PathBuf,
        uid: u32,
        collect: impl Fn() -> Option<TrafficCounters> + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::open_with(move || file::Store::new(&parent, uid), collect, || {})
    }

    #[cfg(test)]
    pub(crate) fn start_with(
        store: file::Store,
        collect: impl Fn() -> Option<TrafficCounters> + Send + 'static,
        before_commit: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::open_with(move || Ok(store), collect, before_commit)
    }

    fn open_with(
        open: impl FnOnce() -> Result<file::Store, file::Error> + Send + 'static,
        collect: impl Fn() -> Option<TrafficCounters> + Send + 'static,
        before_commit: impl Fn() + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cache = Arc::new(Mutex::new(None));
        let published = Arc::clone(&cache);
        let receipts: Receipts = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let retained = Arc::clone(&receipts);
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ended = Arc::clone(&finished);
        std::thread::Builder::new()
            .name("omavless-stats".into())
            .spawn(move || {
                struct Finished(Arc<std::sync::atomic::AtomicBool>);
                impl Drop for Finished {
                    fn drop(&mut self) {
                        self.0.store(true, std::sync::atomic::Ordering::Release);
                    }
                }
                let _finished = Finished(ended);
                let Ok(store) = open() else {
                    return;
                };
                let Ok(_writer) = store.lock_writer() else {
                    return;
                };
                let Ok(mut snapshot) = store.read() else {
                    return;
                };
                let mut recovered = snapshot.state().clone();
                if recovered.new_runtime().is_err() {
                    return;
                }
                if &recovered != snapshot.state() {
                    before_commit();
                    let Ok(next) = store.replace(&snapshot, &recovered) else {
                        return;
                    };
                    snapshot = next;
                }
                if let Ok(mut view) = published.lock() {
                    *view = Some(snapshot.state().clone());
                }
                while let Ok(command) = receiver.recv() {
                    let mut operation = None;
                    let mut candidate = snapshot.state().clone();
                    let reply = match command {
                        Command::Tick => {
                            if !candidate.enabled || candidate.overflowed {
                                continue;
                            }
                            let changed = match collect() {
                                Some(sample) => candidate.observe(&sample),
                                None => candidate.unavailable(),
                            };
                            if changed.is_err() {
                                break;
                            }
                            None
                        }
                        Command::Control {
                            expected,
                            operation: supplied,
                            action,
                            utc,
                            reply,
                        } => {
                            operation = supplied;
                            if candidate.control_revision != expected {
                                remember(&retained, operation.take(), Err(Error::Conflict));
                                let _ = reply.send(Err(Error::Conflict));
                                continue;
                            }
                            let change = match action {
                                Control::Configure(enabled) => candidate.configure(enabled, utc),
                                Control::Reset => candidate.reset(utc),
                            };
                            if change.is_err() {
                                remember(&retained, operation.take(), Err(Error::Unavailable));
                                let _ = reply.send(Err(Error::Unavailable));
                                continue;
                            }
                            Some(reply)
                        }
                    };
                    if &candidate != snapshot.state() {
                        before_commit();
                        match store.replace(&snapshot, &candidate) {
                            Ok(next) => snapshot = next,
                            Err(_) => {
                                if let Ok(mut view) = published.lock() {
                                    *view = None;
                                }
                                remember(&retained, operation, Err(Error::Unknown));
                                if let Some(reply) = reply {
                                    let _ = reply.send(Err(Error::Unknown));
                                }
                                // Never resend a possibly completed write. A new runtime
                                // reads actual durable bytes under a new exclusive lock.
                                return;
                            }
                        }
                    }
                    if let Ok(mut view) = published.lock() {
                        *view = Some(snapshot.state().clone());
                    }
                    if let Some(reply) = reply {
                        let result = Ok(snapshot.state().projection());
                        remember(&retained, operation, result.clone());
                        let _ = reply.send(result);
                    }
                }
            })?;
        Ok(Self {
            sender,
            receipts,
            cache,
            next_tick: Mutex::new(std::time::Instant::now()),
            finished,
        })
    }

    #[cfg(test)]
    pub(crate) fn completion(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.finished)
    }

    #[cfg(test)]
    pub(crate) fn settle(self) {
        let finished = Arc::clone(&self.finished);
        drop(self);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !finished.load(std::sync::atomic::Ordering::Acquire) {
            assert!(
                std::time::Instant::now() < deadline,
                "original statistics worker did not settle"
            );
            std::thread::yield_now();
        }
    }

    pub(crate) fn view(&self) -> Value {
        let result = self
            .cache
            .try_lock()
            .ok()
            .and_then(|view| view.as_ref().map(State::projection));
        if !self.finished.load(std::sync::atomic::Ordering::Acquire)
            && let Some(result) = result
        {
            return result;
        }
        json!({"schemaVersion":1,"availability":"unavailable","scope":"private_recorded_tun_bytes"})
    }

    /// One coalesced slot; never wait or queue a backlog of missed ticks.
    pub(crate) fn reconcile_due(&self) {
        let Ok(mut next) = self.next_tick.try_lock() else {
            return;
        };
        let now = std::time::Instant::now();
        if now < *next {
            return;
        }
        *next = now + std::time::Duration::from_secs(10);
        // Disabled/default collection is a memory-only no-op.
        if self
            .cache
            .try_lock()
            .is_ok_and(|state| state.as_ref().is_some_and(|s| s.enabled))
        {
            self.tick();
        }
    }

    pub(crate) fn tick(&self) {
        let _ = self.sender.try_send(Command::Tick);
    }

    /// Off/revision admission is performed by the existing native owner before
    /// enqueue. Caller waits for the reply only AFTER dropping that owner lock.
    /// Effect-free retained lookup precedes all NEW-operation Off/revision
    /// preconditions. Pending/unknown remains unknown, never resend-eligible.
    pub(crate) fn lookup(
        &self,
        request: &super::protocol::Request,
    ) -> Result<Option<Result<Value, Error>>, Error> {
        let receipts = self.receipts.try_lock().map_err(|_| Error::Busy)?;
        let Some(receipt) = receipts.iter().find(|r| r.operation == request.operation) else {
            return Ok(None);
        };
        if receipt.digest != request.digest {
            return Err(Error::Conflict);
        }
        receipt.outcome.clone().map(Some).ok_or(Error::Unknown)
    }

    pub(crate) fn submit(
        &self,
        request: &super::protocol::Request,
        utc: u64,
    ) -> Result<mpsc::Receiver<Result<Value, Error>>, Error> {
        self.enqueue(
            request.stats_revision,
            Some((request.operation.clone(), request.digest)),
            request.action,
            utc,
        )
    }

    #[cfg(test)]
    pub(crate) fn control(
        &self,
        expected: u64,
        action: Control,
        utc: u64,
    ) -> Result<mpsc::Receiver<Result<Value, Error>>, Error> {
        self.enqueue(expected, None, action, utc)
    }

    fn enqueue(
        &self,
        expected: u64,
        operation: Option<(String, crate::mutation::MutationDigest)>,
        action: Control,
        utc: u64,
    ) -> Result<mpsc::Receiver<Result<Value, Error>>, Error> {
        let (reply, receive) = mpsc::sync_channel(1);
        let mut receipts = self.receipts.lock().map_err(|_| Error::Unavailable)?;
        if let Some((id, digest)) = &operation {
            if let Some(receipt) = receipts.iter().find(|r| &r.operation == id) {
                if &receipt.digest != digest {
                    return Err(Error::Conflict);
                }
                let outcome = receipt.outcome.clone().ok_or(Error::Unknown)?;
                let _ = reply.send(outcome);
                return Ok(receive);
            }
            if receipts.len() == crate::mutation::DEFAULT_RESULT_CACHE_LIMIT {
                let Some(oldest) = receipts.iter().position(|r| {
                    r.outcome
                        .as_ref()
                        .is_some_and(|outcome| !matches!(outcome, Err(Error::Unknown)))
                }) else {
                    return Err(Error::Busy);
                };
                receipts.remove(oldest);
            }
            receipts.push_back(Receipt {
                operation: id.clone(),
                digest: *digest,
                outcome: None,
            });
        }
        let original = operation.as_ref().map(|(id, _)| id.clone());
        if let Err(error) = self.sender.try_send(Command::Control {
            expected,
            operation,
            action,
            utc,
            reply,
        }) {
            if let Some(original) = original {
                receipts.retain(|r| r.operation != original);
            }
            return Err(match error {
                mpsc::TrySendError::Full(_) => Error::Busy,
                mpsc::TrySendError::Disconnected(_) => Error::Unavailable,
            });
        }
        Ok(receive)
    }
}

fn remember(
    receipts: &Receipts,
    operation: Option<(String, crate::mutation::MutationDigest)>,
    result: Result<Value, Error>,
) {
    if let Some((id, digest)) = operation
        && let Ok(mut receipts) = receipts.lock()
        && let Some(receipt) = receipts
            .iter_mut()
            .find(|r| r.operation == id && r.digest == digest)
    {
        receipt.outcome = Some(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn fixture() -> (tempfile::TempDir, file::Store) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let store = file::Store::new(dir.path(), nix::unistd::getuid().as_raw()).unwrap();
        (dir, store)
    }
    fn control(
        worker: &Worker,
        revision: u64,
        action: Control,
        utc: u64,
    ) -> mpsc::Receiver<Result<Value, Error>> {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            match worker.control(revision, action, utc) {
                Ok(reply) => return reply,
                Err(Error::Busy) if Instant::now() < until => std::thread::yield_now(),
                Err(error) => panic!("control admission: {error:?}"),
            }
        }
    }
    fn ready(worker: &Worker) {
        let until = Instant::now() + Duration::from_secs(2);
        while worker.view()["availability"] == "unavailable" {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
    }
    #[test]
    fn checkpoint_exhaustion_preserves_bytes_and_terminal_worker_is_unavailable_without_restart() {
        let (dir, store) = fixture();
        let mut state = State::default();
        state.configure(true, 100).unwrap();
        state.unavailable().unwrap();
        let mut value: Value = serde_json::from_slice(&state.encode().unwrap()).unwrap();
        value["checkpointSequence"] = u64::MAX.into();
        let state = State::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        store.replace(&store.read().unwrap(), &state).unwrap();
        let before = std::fs::read(dir.path().join(file::NAME)).unwrap();
        let worker = Worker::start(store, || {
            Some(TrafficCounters {
                identity: "a".repeat(64),
                rx_bytes: 1,
                tx_bytes: 1,
                sampled_at_ms: 1,
            })
        })
        .unwrap();
        ready(&worker);
        worker.tick();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !worker.finished.load(std::sync::atomic::Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(worker.view()["availability"], "unavailable");
        assert!(matches!(
            worker.control(1, Control::Reset, 200),
            Err(Error::Unavailable)
        ));
        assert_eq!(std::fs::read(dir.path().join(file::NAME)).unwrap(), before);
        worker.settle();
    }

    #[test]
    fn disabled_worker_does_not_collect_or_create_default_and_controls_are_revision_fenced() {
        let (dir, store) = fixture();
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = Arc::clone(&reads);
        let worker = Worker::start(store, move || {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            None
        })
        .unwrap();
        ready(&worker);
        worker.tick();
        // A later same-writer command is the barrier for the prior tick.
        let reply = control(&worker, 0, Control::Configure(false), 100);
        assert!(reply.recv_timeout(Duration::from_secs(2)).unwrap().is_ok());
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(
            worker
                .control(0, Control::Configure(true), 100)
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            worker
                .control(0, Control::Reset, 200)
                .unwrap()
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            Err(Error::Conflict)
        );
        worker.settle();
    }
    #[test]
    fn blocked_checkpoint_does_not_hold_owner_or_cache_and_disable_ack_follows_old_write() {
        let (_dir, store) = fixture();
        let owner = Arc::new(Mutex::new(()));
        let captured_owner = Arc::clone(&owner);
        let (entered, at_write) = mpsc::sync_channel(1);
        let (release, blocked) = mpsc::sync_channel(1);
        let once = std::sync::atomic::AtomicBool::new(false);
        let worker = Worker::start_with(
            store,
            move || {
                let _capture = captured_owner.try_lock().unwrap();
                None
            },
            move || {
                if !once.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    entered.send(()).unwrap();
                    blocked.recv_timeout(Duration::from_secs(2)).unwrap();
                }
            },
        )
        .unwrap();
        ready(&worker);
        let enabling = worker.control(0, Control::Configure(true), 100).unwrap();
        at_write.recv_timeout(Duration::from_secs(2)).unwrap();
        let start = Instant::now();
        let lifecycle = owner
            .try_lock()
            .expect("statistics write must not own lifecycle");
        assert_eq!(worker.view()["enabled"], false);
        drop(lifecycle);
        assert!(start.elapsed() < Duration::from_millis(100));
        let disabling = worker.control(1, Control::Configure(false), 100).unwrap();
        assert!(disabling.try_recv().is_err());
        release.send(()).unwrap();
        assert!(
            enabling
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        assert_eq!(
            disabling
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap()["enabled"],
            false
        );
        worker.tick();
        assert!(
            control(&worker, 2, Control::Configure(false), 100)
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        worker.settle();
    }
}
