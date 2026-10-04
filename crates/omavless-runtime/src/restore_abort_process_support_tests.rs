// SPDX-License-Identifier: MIT
//! Test-only raw owned-child provenance; uncertainty never permits cleanup.
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid, waitpid};
use nix::unistd::Pid;
use std::cell::Cell;
use std::process::{Child, ChildStdout};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub(super) type Quarantine = Rc<Cell<bool>>;
type Result<T> = std::result::Result<T, &'static str>;

trait Calls {
    fn observe(&mut self, pid: Pid) -> nix::Result<WaitStatus>;
    fn reap(&mut self, pid: Pid) -> nix::Result<WaitStatus>;
    fn signal(&mut self, pid: Pid) -> nix::Result<()>;
}
struct Raw;
impl Calls for Raw {
    fn observe(&mut self, pid: Pid) -> nix::Result<WaitStatus> {
        waitid(
            Id::Pid(pid),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
        )
    }
    fn reap(&mut self, pid: Pid) -> nix::Result<WaitStatus> {
        waitpid(pid, Some(WaitPidFlag::WNOHANG))
    }
    fn signal(&mut self, pid: Pid) -> nix::Result<()> {
        kill(pid, Signal::SIGKILL)
    }
}

struct State {
    pid: Pid,
    quarantine: Quarantine,
    terminal: Option<WaitStatus>,
    reaped: bool,
}
impl State {
    fn allowed(&self) -> Result<()> {
        if self.quarantine.get() || self.reaped {
            Err("owned state unavailable")
        } else {
            Ok(())
        }
    }
    fn unknown<T>(&self) -> Result<T> {
        self.quarantine.set(true);
        Err("owned state uncertain; no retry, signal, reap or cleanup")
    }
    fn exact_terminal(&self, status: WaitStatus) -> bool {
        matches!(status, WaitStatus::Exited(pid, _) | WaitStatus::Signaled(pid, _, _) if pid == self.pid)
    }
    fn observe(&mut self, calls: &mut impl Calls) -> Result<WaitStatus> {
        self.allowed()?;
        let observed = match calls.observe(self.pid) {
            Ok(value) => value,
            Err(_) => return self.unknown(),
        };
        if observed != WaitStatus::StillAlive && !self.exact_terminal(observed) {
            return self.unknown();
        }
        if self.terminal.is_some_and(|prior| prior != observed) {
            return self.unknown();
        }
        if observed != WaitStatus::StillAlive {
            self.terminal = Some(observed);
        }
        Ok(observed)
    }
    fn kill_live(&mut self, calls: &mut impl Calls) -> Result<()> {
        // WNOWAIT retains this exclusively owned unreaped child as PID anchor.
        if self.observe(calls)? != WaitStatus::StillAlive {
            return Err("child already exited; no signal");
        }
        if calls.signal(self.pid).is_err() {
            return self.unknown();
        }
        Ok(())
    }
    fn reap(&mut self, expected: WaitStatus, calls: &mut impl Calls) -> Result<WaitStatus> {
        self.allowed()?;
        if !self.exact_terminal(expected) || self.terminal != Some(expected) {
            return self.unknown();
        }
        match calls.reap(self.pid) {
            Ok(actual) if actual == expected => {
                self.reaped = true;
                Ok(actual)
            }
            _ => self.unknown(),
        }
    }
}

pub(super) struct OwnedProcess {
    child: Option<Child>, // Rust Child has no implicit signal/reap destructor.
    state: State,
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if !self.state.reaped {
            self.state.quarantine.set(true);
            // Preserve the handle/pipes even during unwind; no hidden retry.
            if let Some(child) = self.child.take() {
                std::mem::forget(child);
            }
        }
    }
}
impl OwnedProcess {
    pub(super) fn new(child: Child, quarantine: &Quarantine) -> Self {
        assert!(!quarantine.get());
        let pid = Pid::from_raw(i32::try_from(child.id()).unwrap());
        Self {
            child: Some(child),
            state: State {
                pid,
                quarantine: quarantine.clone(),
                terminal: None,
                reaped: false,
            },
        }
    }
    pub(super) fn stdout(&mut self) -> ChildStdout {
        self.child.as_mut().unwrap().stdout.take().unwrap()
    }
    pub(super) fn stdin(&mut self) -> std::process::ChildStdin {
        self.child.as_mut().unwrap().stdin.take().unwrap()
    }
    pub(super) fn kill_live(&mut self) -> Result<()> {
        self.state.kill_live(&mut Raw)
    }
    pub(super) fn finish(&mut self) -> Result<WaitStatus> {
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            let seen = self.state.observe(&mut Raw)?;
            if seen != WaitStatus::StillAlive {
                return self.state.reap(seen, &mut Raw);
            }
            if Instant::now() >= deadline {
                return self.state.unknown();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        observe: nix::Result<WaitStatus>,
        reap: nix::Result<WaitStatus>,
        signal: nix::Result<()>,
        counts: [usize; 3],
    }
    impl Calls for Fake {
        fn observe(&mut self, _: Pid) -> nix::Result<WaitStatus> {
            self.counts[0] += 1;
            self.observe
        }
        fn reap(&mut self, _: Pid) -> nix::Result<WaitStatus> {
            self.counts[1] += 1;
            self.reap
        }
        fn signal(&mut self, _: Pid) -> nix::Result<()> {
            self.counts[2] += 1;
            self.signal
        }
    }
    fn state() -> State {
        State {
            pid: Pid::from_raw(42),
            quarantine: Rc::new(Cell::new(false)),
            terminal: None,
            reaped: false,
        }
    }
    fn fake() -> Fake {
        Fake {
            observe: Ok(WaitStatus::StillAlive),
            reap: Ok(WaitStatus::Exited(Pid::from_raw(42), 0)),
            signal: Ok(()),
            counts: [0; 3],
        }
    }
    fn no_retry(state: &mut State, fake: &mut Fake) {
        let counts = fake.counts;
        assert!(state.observe(fake).is_err());
        assert!(state.kill_live(fake).is_err());
        assert!(
            state
                .reap(WaitStatus::Exited(Pid::from_raw(42), 0), fake)
                .is_err()
        );
        assert_eq!(counts, fake.counts);
        assert!(state.quarantine.get());
    }
    #[test]
    fn first_unknown_observation_permanently_blocks_every_followup_call() {
        for outcome in [
            Err(Errno::ECHILD),
            Err(Errno::EINTR),
            Err(Errno::EIO),
            Ok(WaitStatus::Exited(Pid::from_raw(41), 0)),
            Ok(WaitStatus::Stopped(Pid::from_raw(42), Signal::SIGSTOP)),
        ] {
            let mut s = state();
            let mut f = fake();
            f.observe = outcome;
            assert!(s.kill_live(&mut f).is_err());
            assert_eq!(f.counts, [1, 0, 0]);
            no_retry(&mut s, &mut f);
        }
    }
    #[test]
    fn reap_unknown_or_mismatch_never_becomes_success_or_retry() {
        let expected = WaitStatus::Exited(Pid::from_raw(42), 7);
        for outcome in [
            Err(Errno::ECHILD),
            Err(Errno::EINTR),
            Ok(WaitStatus::StillAlive),
            Ok(WaitStatus::Exited(Pid::from_raw(41), 7)),
            Ok(WaitStatus::Exited(Pid::from_raw(42), 0)),
        ] {
            let mut s = state();
            let mut f = fake();
            f.observe = Ok(expected);
            f.reap = outcome;
            assert_eq!(s.observe(&mut f), Ok(expected));
            assert!(s.reap(expected, &mut f).is_err());
            assert_eq!(f.counts, [1, 1, 0]);
            no_retry(&mut s, &mut f);
        }
    }
    #[test]
    fn timeout_quarantine_is_shared_by_other_owned_children_and_stays_permanent() {
        let first = state();
        let mut second = state();
        second.quarantine = first.quarantine.clone();
        assert!(first.unknown::<()>().is_err());
        let mut calls = fake();
        no_retry(&mut second, &mut calls);
        assert_eq!(calls.counts, [0; 3]);
    }

    #[test]
    fn exact_terminal_is_reaped_once_without_signal_and_signal_error_quarantines() {
        let mut s = state();
        let mut f = fake();
        f.observe = Ok(WaitStatus::Exited(Pid::from_raw(42), 0));
        assert!(s.kill_live(&mut f).is_err());
        assert_eq!(f.counts, [1, 0, 0]);
        assert_eq!(
            s.reap(f.observe.unwrap(), &mut f),
            Ok(WaitStatus::Exited(Pid::from_raw(42), 0))
        );
        assert!(s.observe(&mut f).is_err());
        assert_eq!(f.counts, [1, 1, 0]);
        let mut s = state();
        let mut f = fake();
        f.signal = Err(Errno::ESRCH);
        assert!(s.kill_live(&mut f).is_err());
        assert_eq!(f.counts, [1, 0, 1]);
        no_retry(&mut s, &mut f);
    }
}
