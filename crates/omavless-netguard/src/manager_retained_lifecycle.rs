//! Developer-only fixed retained-reference coordinator.
//! The existing FixtureCreator remains the sole kernel writer.
use std::time::{Duration, Instant};

#[path = "manager_retained_lifecycle_adapter.rs"]
mod adapter;

const MAX_POLLS: usize = 450;
const PHASE_BUDGET: Duration = Duration::from_secs(45);
type Result<T> = std::result::Result<T, ()>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Job {
    id: u32,
    path: String,
}

impl Job {
    fn from_path(path: &str) -> Result<Self> {
        let suffix = path
            .strip_prefix("/org/freedesktop/systemd1/job/")
            .ok_or(())?;
        if suffix.is_empty()
            || suffix.len() > 10
            || suffix.starts_with('0')
            || !suffix.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(());
        }
        let id = suffix.parse::<u32>().map_err(|_| ())?;
        Ok(Self {
            id,
            path: path.to_owned(),
        })
    }
}

// These private values must be produced by strict typed phase validation in the
// future fixed adapter, never directly by a bus reply or a receipt boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartObservation {
    Pending,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopObservation {
    Pending,
    Completed,
}

trait FixedLifecycle {
    // Includes owner/version pin, retained Ref, whole configured admission and
    // original unit/ELF/link rechecks. Ref is NOT a configuration lock.
    fn admit_retaining_reference(&mut self) -> Result<()>;
    fn start_once(&mut self) -> Result<Job>;
    // A valid pending observation is progress, not an uncertain RPC result.
    // Any missing, malformed, failed or incoherent observation returns Err.
    fn observe_start(&mut self, job: &Job) -> Result<StartObservation>;
    // Must read original native/state FDs: exactly two effects, full inventory,
    // second socket refusal, Closed/Retired and absent, plus empty own cgroup.
    fn prove_native_completion(&mut self) -> Result<()>;
    fn stop_once(&mut self) -> Result<Job>;
    fn observe_stop(&mut self, job: &Job) -> Result<StopObservation>;
    // Final same-owner/config/original-FD/native evidence recheck BEFORE Unref.
    fn prove_stopped(&mut self) -> Result<()>;
    fn unref_once(&mut self) -> Result<()>;
    fn publish_terminal_receipt(&mut self) -> Result<()>;
    fn now(&mut self) -> Instant;
    fn pause_between_known_pending(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Fresh,
    Sealed,
    Complete,
}

struct Coordinator<P> {
    port: P,
    state: State,
}

impl<P: FixedLifecycle> Coordinator<P> {
    fn new(port: P) -> Self {
        Self {
            port,
            state: State::Fresh,
        }
    }

    fn once(&mut self) -> Result<()> {
        if self.state != State::Fresh {
            return Err(());
        }
        // Seal BEFORE the first callback. Error or panic cannot make this
        // coordinator reusable; no Drop implementation compensates or unrefs.
        // The real entry must leak the coordinator/connection BEFORE calling.
        self.state = State::Sealed;
        self.port.admit_retaining_reference()?;
        let start = self.port.start_once()?;
        self.wait_start(&start)?;
        self.port.prove_native_completion()?;
        let stop = self.port.stop_once()?;
        self.wait_stop(&stop)?;
        self.port.prove_stopped()?;
        self.port.unref_once()?;
        self.port.publish_terminal_receipt()?;
        self.state = State::Complete;
        Ok(())
    }

    fn wait_start(&mut self, job: &Job) -> Result<()> {
        let begun = self.port.now();
        for _ in 0..MAX_POLLS {
            if self.port.now().checked_duration_since(begun).ok_or(())? >= PHASE_BUDGET {
                return Err(());
            }
            let observed = self.port.observe_start(job)?;
            if self.port.now().checked_duration_since(begun).ok_or(())? >= PHASE_BUDGET {
                return Err(());
            }
            if observed == StartObservation::Completed {
                return Ok(());
            }
            self.port.pause_between_known_pending();
        }
        Err(())
    }

    fn wait_stop(&mut self, job: &Job) -> Result<()> {
        let begun = self.port.now();
        for _ in 0..MAX_POLLS {
            if self.port.now().checked_duration_since(begun).ok_or(())? >= PHASE_BUDGET {
                return Err(());
            }
            let observed = self.port.observe_stop(job)?;
            if self.port.now().checked_duration_since(begun).ok_or(())? >= PHASE_BUDGET {
                return Err(());
            }
            if observed == StopObservation::Completed {
                return Ok(());
            }
            self.port.pause_between_known_pending();
        }
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 9] = [
        "admit-ref",
        "start",
        "observe-start",
        "native-proof",
        "stop",
        "observe-stop",
        "stopped-proof",
        "unref",
        "publish",
    ];

    struct Fake {
        calls: Vec<&'static str>,
        fail: Option<usize>,
        panic: Option<usize>,
        pending_start: usize,
        pending_stop: usize,
        time: Instant,
        tick: Duration,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                fail: None,
                panic: None,
                pending_start: 0,
                pending_stop: 0,
                time: Instant::now(),
                tick: Duration::ZERO,
            }
        }
        fn call(&mut self, label: &'static str) -> Result<()> {
            let index = self.calls.len();
            self.calls.push(label);
            assert_ne!(self.panic, Some(index), "synthetic callback panic");
            if self.fail == Some(index) {
                Err(())
            } else {
                Ok(())
            }
        }
    }

    impl FixedLifecycle for Fake {
        fn admit_retaining_reference(&mut self) -> Result<()> {
            self.call("admit-ref")
        }
        fn start_once(&mut self) -> Result<Job> {
            self.call("start")?;
            Job::from_path("/org/freedesktop/systemd1/job/17")
        }
        fn observe_start(&mut self, job: &Job) -> Result<StartObservation> {
            assert_eq!(job, &Job::from_path("/org/freedesktop/systemd1/job/17")?);
            self.call("observe-start")?;
            if self.pending_start > 0 {
                self.pending_start -= 1;
                Ok(StartObservation::Pending)
            } else {
                Ok(StartObservation::Completed)
            }
        }
        fn prove_native_completion(&mut self) -> Result<()> {
            self.call("native-proof")
        }
        fn stop_once(&mut self) -> Result<Job> {
            self.call("stop")?;
            Job::from_path("/org/freedesktop/systemd1/job/23")
        }
        fn observe_stop(&mut self, job: &Job) -> Result<StopObservation> {
            assert_eq!(job, &Job::from_path("/org/freedesktop/systemd1/job/23")?);
            self.call("observe-stop")?;
            if self.pending_stop > 0 {
                self.pending_stop -= 1;
                Ok(StopObservation::Pending)
            } else {
                Ok(StopObservation::Completed)
            }
        }
        fn prove_stopped(&mut self) -> Result<()> {
            self.call("stopped-proof")
        }
        fn unref_once(&mut self) -> Result<()> {
            self.call("unref")
        }
        fn publish_terminal_receipt(&mut self) -> Result<()> {
            self.call("publish")
        }
        fn now(&mut self) -> Instant {
            self.time
        }
        fn pause_between_known_pending(&mut self) {
            self.time += self.tick;
        }
    }

    #[test]
    fn success_requires_native_proof_before_stop_and_stopped_proof_before_unref() {
        let mut coordinator = Coordinator::new(Fake::new());
        assert_eq!(coordinator.once(), Ok(()));
        assert_eq!(coordinator.port.calls, ORDER);
        assert_eq!(coordinator.state, State::Complete);
        assert_eq!(coordinator.once(), Err(()));
        assert_eq!(coordinator.port.calls, ORDER);
    }

    #[test]
    fn every_callback_failure_permanently_stops_before_any_later_action() {
        for failed in 0..ORDER.len() {
            let mut port = Fake::new();
            port.fail = Some(failed);
            let mut coordinator = Coordinator::new(port);
            assert_eq!(coordinator.once(), Err(()));
            assert_eq!(coordinator.state, State::Sealed);
            assert_eq!(coordinator.port.calls, ORDER[..=failed]);
            assert_eq!(coordinator.once(), Err(()));
            assert_eq!(coordinator.port.calls, ORDER[..=failed]);
        }
    }

    #[test]
    fn every_callback_panic_is_already_sealed_without_compensating_actions() {
        for panicked in 0..ORDER.len() {
            let mut port = Fake::new();
            port.panic = Some(panicked);
            let mut coordinator = Coordinator::new(port);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| coordinator.once()))
                    .is_err()
            );
            assert_eq!(coordinator.state, State::Sealed);
            assert_eq!(coordinator.once(), Err(()));
            assert_eq!(coordinator.port.calls, ORDER[..=panicked]);
        }
    }

    #[test]
    fn bounded_known_pending_is_not_unknown_and_never_restarts_a_job() {
        let mut port = Fake::new();
        port.pending_start = 2;
        port.pending_stop = 2;
        let mut coordinator = Coordinator::new(port);
        assert_eq!(coordinator.once(), Ok(()));
        for label in ["start", "stop", "unref"] {
            assert_eq!(
                coordinator
                    .port
                    .calls
                    .iter()
                    .filter(|v| **v == label)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn poll_count_and_elapsed_deadlines_are_terminal_without_stop_or_unref() {
        for stop_phase in [false, true] {
            for tick in [Duration::ZERO, PHASE_BUDGET] {
                let mut port = Fake::new();
                port.tick = tick;
                if stop_phase {
                    port.pending_stop = MAX_POLLS + 1;
                } else {
                    port.pending_start = MAX_POLLS + 1;
                }
                let mut coordinator = Coordinator::new(port);
                assert_eq!(coordinator.once(), Err(()));
                let calls = coordinator.port.calls.clone();
                assert!(!calls.contains(&"unref"));
                assert_eq!(calls.contains(&"stop"), stop_phase);
                assert_eq!(coordinator.once(), Err(()));
                assert_eq!(coordinator.port.calls, calls);
            }
        }
    }

    #[test]
    fn job_path_has_one_canonical_nonzero_u32_id_no_alias_or_extra_component() {
        for bad in [
            "/",
            "/org/freedesktop/systemd1/job/0",
            "/org/freedesktop/systemd1/job/01",
            "/org/freedesktop/systemd1/job/+1",
            "/org/freedesktop/systemd1/job/1/x",
            "/org/freedesktop/systemd1/job/4294967296",
            "/org/freedesktop/systemd1/job/1\n",
        ] {
            assert_eq!(Job::from_path(bad), Err(()));
        }
        assert_eq!(
            Job::from_path("/org/freedesktop/systemd1/job/4294967295")
                .unwrap()
                .id,
            u32::MAX
        );
    }
}
