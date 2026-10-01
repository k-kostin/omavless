// SPDX-License-Identifier: MIT
//! Test-only supervision of known synthetic workers/effects. It does not
//! discover or quiesce host processes, foreign writers, GIO or kernel work.

use super::*;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Default)]
struct Progress {
    closed: bool,
    outstanding: usize,
    unknown: bool,
}

#[derive(Clone, Default)]
struct Gate(Arc<Mutex<Progress>>);

struct EffectTicket {
    gate: Gate,
    settled: bool,
}

impl Gate {
    fn begin(&self) -> Result<EffectTicket, ()> {
        let mut progress = self.0.lock().map_err(|_| ())?;
        if progress.closed || progress.unknown {
            return Err(());
        }
        progress.outstanding = progress.outstanding.checked_add(1).ok_or(())?;
        Ok(EffectTicket {
            gate: self.clone(),
            settled: false,
        })
    }
}

impl EffectTicket {
    fn settle(mut self) {
        // Only the synthetic effect executor calls this after known completion.
        // This is not evidence about an arbitrary external/host operation.
        self.settled = true;
    }
}

impl Drop for EffectTicket {
    fn drop(&mut self) {
        if let Ok(mut progress) = self.gate.0.lock() {
            progress.outstanding -= 1;
            if !self.settled {
                progress.unknown = true;
            }
        }
    }
}

#[derive(Default)]
struct Workers {
    gate: Gate,
    workers: Vec<JoinHandle<()>>,
    joined: bool,
}

impl Workers {
    fn spawn(&mut self, work: impl FnOnce() + Send + 'static) -> Result<(), ()> {
        if self.gate.0.lock().map_err(|_| ())?.closed {
            return Err(());
        }
        self.workers.push(thread::spawn(work));
        Ok(())
    }

    fn close_admission(&mut self) {
        self.gate.0.lock().unwrap().closed = true;
    }

    fn join(&mut self) {
        self.close_admission();
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                self.gate.0.lock().unwrap().unknown = true;
            }
        }
        self.joined = true;
    }

    fn receipt(&self, model: &Model, successor: Binding) -> Option<Predecessor> {
        let progress = self.gate.0.lock().ok()?;
        if !progress.closed || !self.joined || progress.outstanding != 0 || progress.unknown {
            return None;
        }
        // The real coordinator would need its own trusted scope and a held
        // owner/storage lease. This model supplies neither host proof.
        Some(model.receipt(successor))
    }
}

fn receive<T>(channel: &mpsc::Receiver<T>) -> T {
    channel
        .recv_timeout(Duration::from_secs(3))
        .expect("synthetic worker deadline")
}

#[test]
fn closed_admission_does_not_settle_already_admitted_effects() {
    let temp = Temp::new();
    let (planner, current, original) = partial(4, false);
    let field = planner.pending.unwrap();
    let replacement = planner.intended.value(field);
    let mut model = Model::create(&temp, planner);
    let current = Arc::new(Mutex::new(current));
    let worker_current = current.clone();
    let mut workers = Workers::default();
    let ticket = workers.gate.begin().unwrap();
    let (release, wait) = mpsc::channel();
    let (ready, started) = mpsc::channel();
    workers
        .spawn(move || {
            ready.send(()).unwrap();
            receive(&wait);
            assert!(ticket.gate.0.lock().unwrap().closed);
            let mut state = worker_current.lock().unwrap();
            *state = state.with(field, replacement).unwrap();
            drop(state);
            ticket.settle();
        })
        .unwrap();
    receive(&started);
    workers.close_admission();
    assert!(workers.gate.begin().is_err());
    assert!(workers.spawn(|| {}).is_err());
    assert!(workers.receipt(&model, successor()).is_none());
    release.send(()).unwrap();
    workers.join();
    let receipt = workers.receipt(&model, successor()).unwrap();
    model
        .transfer(successor(), model.scope.facts(), receipt)
        .unwrap();
    let mut observed = current.lock().unwrap().clone();
    model.restore(successor(), &mut observed).unwrap();
    assert_eq!(observed, original);
}

#[test]
fn joined_parent_does_not_hide_a_delayed_callback() {
    let temp = Temp::new();
    let (planner, _, _) = partial(4, true);
    let model = Model::create(&temp, planner);
    let mut workers = Workers::default();
    let ticket = workers.gate.begin().unwrap();
    let (release, wait) = mpsc::channel();
    let (callback_sender, callback_receiver) = mpsc::channel();
    workers
        .spawn(move || {
            // Model an effect completion living beyond its initiating worker.
            let callback = thread::spawn(move || {
                receive(&wait);
                ticket.settle();
            });
            callback_sender.send(callback).unwrap();
        })
        .unwrap();
    let callback = receive(&callback_receiver);
    workers.join();
    assert!(workers.joined);
    assert!(workers.receipt(&model, successor()).is_none());
    release.send(()).unwrap();
    callback.join().unwrap();
    assert!(workers.receipt(&model, successor()).is_some());
}

#[test]
fn abandoned_outcome_and_worker_panic_permanently_refuse_receipts() {
    for panic_worker in [false, true] {
        let temp = Temp::new();
        let (planner, _, _) = partial(4, true);
        let model = Model::create(&temp, planner);
        let mut workers = Workers::default();
        let ticket = workers.gate.begin().unwrap();
        workers
            .spawn(move || {
                if panic_worker {
                    ticket.settle();
                    panic!("synthetic worker failure after effect");
                }
                drop(ticket); // No known outcome; zero live tickets is insufficient.
            })
            .unwrap();
        workers.join();
        assert_eq!(workers.gate.0.lock().unwrap().outstanding, 0);
        assert!(workers.receipt(&model, successor()).is_none());
        workers.join();
        assert!(workers.receipt(&model, successor()).is_none());
    }
}

#[test]
fn completed_workers_cannot_mint_a_receipt_before_join_or_revive_a_stale_record() {
    let temp = Temp::new();
    let (planner, _, _) = partial(4, true);
    let mut model = Model::create(&temp, planner);
    let mut workers = Workers::default();
    let ticket = workers.gate.begin().unwrap();
    let (done, finished) = mpsc::channel();
    workers
        .spawn(move || {
            ticket.settle();
            done.send(()).unwrap();
        })
        .unwrap();
    receive(&finished);
    workers.close_admission();
    assert_eq!(workers.gate.0.lock().unwrap().outstanding, 0);
    assert!(workers.receipt(&model, successor()).is_none());
    workers.join();
    let receipt = workers.receipt(&model, successor()).unwrap();
    // A durable same-binding revision after quiescence invalidates the receipt,
    // even though every known worker has exited and the settings still match.
    model
        .publish(binding(), model.journal.planner.clone())
        .unwrap();
    let before = fs::read(temp.record()).unwrap();
    assert!(
        model
            .transfer(successor(), model.scope.facts(), receipt)
            .is_err()
    );
    assert_eq!(fs::read(temp.record()).unwrap(), before);
    let current = workers.receipt(&model, successor()).unwrap();
    model
        .transfer(successor(), model.scope.facts(), current)
        .unwrap();
}
