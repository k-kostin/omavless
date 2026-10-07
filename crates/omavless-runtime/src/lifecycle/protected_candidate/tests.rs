use super::*;
use omavless_netguard::{
    policy::Policy,
    transaction::{self, Effect, Marker, Observation, Table},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[cfg(feature = "netguard-normal-lifecycle")]
mod normal_controls {
    use super::*;
    use crate::lifecycle::protected_candidate::normal::NormalSession;

    fn original(f: &Fixture, cut: Option<&'static str>) -> (LifecycleExecutor<Host>, NormalSession<Port>) {
        let mut candidate = f.candidate(cut, None);
        let (executor, port) = candidate.owned.take().unwrap();
        let Executor::Owned(executor) = executor else {
            panic!("fixed inert fixture owns its executor");
        };
        (executor, NormalSession::for_test(port))
    }

    #[test]
    fn normal_separate_operations_keep_same_port_and_never_run_traffic_interval() {
        let f = Fixture::new(4, Some(8));
        let (mut executor, mut session) = original(&f, None);
        assert!(session.is_fresh());
        let connected = session.connect(&mut executor, "fixture", &mut || Ok(())).unwrap();
        assert_eq!(connected.generation, 9);
        assert!(session.is_armed() && session.retention_required());
        assert_eq!(executor.actual(), ActualState::Connected);
        let closed = session.disconnect(&mut executor, &mut || Ok(())).unwrap();
        assert_eq!(closed.generation, 10);
        assert!(session.is_closed() && !session.retention_required());
        assert_eq!(executor.actual(), ActualState::Disconnected);
        assert_eq!(f.root.borrow().marker, Marker::Closed(9));
        assert!(!f.log.borrow().contains(&"interval_begin"));
        assert!(!f.log.borrow().contains(&"interval_complete"));
        assert!(f.log.borrow().contains(&"interval_recheck"));
        drop(session);
        drop(executor);
        assert_eq!(f.drops.get(), 2);
    }

    #[test]
    fn normal_armed_second_connect_never_resends_or_retargets() {
        let f = Fixture::new(0, None);
        let (mut executor, mut session) = original(&f, None);
        session.connect(&mut executor, "fixture", &mut || Ok(())).unwrap();
        let before = f.log.borrow().clone();
        assert!(session.connect(&mut executor, "another", &mut || Ok(())).is_err());
        assert_eq!(*f.log.borrow(), before);
        assert!(session.is_armed());
        drop(session);
        assert_eq!(f.drops.get(), 0);
        // Model the required outer registered-owner custody, not a promise
        // that forgetting a borrowed reference retains the actual executor.
        std::mem::forget(executor);
        assert_eq!(f.drops.get(), 0);
    }

    #[test]
    fn normal_live_binding_refusal_never_stops_or_disarms_and_slot_stays_sealed() {
        let f = Fixture::new(0, None);
        let (mut executor, mut session) = original(&f, Some("interval_recheck"));
        session.connect(&mut executor, "fixture", &mut || Ok(())).unwrap();
        assert!(session.disconnect(&mut executor, &mut || Ok(())).is_err());
        assert!(session.retention_required() && !session.is_armed() && !session.is_closed());
        assert!(!f.log.borrow().contains(&"stop"));
        assert!(!f.log.borrow().contains(&"disarm"));
        let before = f.log.borrow().clone();
        assert!(session.disconnect(&mut executor, &mut || Ok(())).is_err());
        assert_eq!(*f.log.borrow(), before);
        drop(session);
        std::mem::forget(executor);
        assert_eq!(f.drops.get(), 0);
    }

    #[test]
    fn normal_origin_unwind_latches_before_original_port_is_borrowed() {
        let f = Fixture::new(0, None);
        let (mut executor, mut session) = original(&f, None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.connect(&mut executor, "fixture", &mut || panic!("fixed original cut"))
        }));
        assert!(result.is_err() && session.retention_required());
        assert!(f.log.borrow().is_empty());
        drop(session);
        std::mem::forget(executor);
        assert_eq!(f.drops.get(), 0);
    }
}

struct Host {
    log: Rc<RefCell<Vec<&'static str>>>,
    drops: Rc<Cell<u8>>,
    paths: DesiredPaths,
    uid: u32,
    running: bool,
    cut: Option<&'static str>,
    unsupported: bool,
}
impl Drop for Host {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl Host {
    fn step(&self, name: &'static str) -> Result<(), HostStepError> {
        self.log.borrow_mut().push(name);
        if self.cut == Some(name) {
            Err(HostStepError::Prepare)
        } else {
            Ok(())
        }
    }
}
impl sealed::Sealed for Host {}
struct MockAdmission {
    generation: u64,
}
struct MockInterval(Rc<Cell<u8>>);
impl Drop for MockInterval {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
impl ProtectedHost for Host {
    type Admission = MockAdmission;
    type Interval = MockInterval;
    fn begin_interval(&mut self, _: &DesiredState) -> Result<MockInterval, HostStepError> {
        self.step("interval_begin")?;
        Ok(MockInterval(self.drops.clone()))
    }
    fn complete_interval(&mut self, _: &mut MockInterval) -> Result<(), HostStepError> {
        if self.cut == Some("interval_panic") {
            panic!("fixed inert interval cut");
        }
        self.step("interval_complete")
    }
    fn recheck_interval(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        if self.log.borrow().contains(&"interval_complete") {
            self.step("interval_postcheck")?;
        }
        self.step("interval_recheck")
    }
    fn prepare_admitted(&mut self, desired: &DesiredState) -> Result<MockAdmission, HostStepError> {
        self.prepare(desired)?;
        self.step("validation_reaped")?;
        self.step("coverage")?;
        Ok(MockAdmission {
            generation: desired.generation,
        })
    }
    fn recheck_admission(&self, admission: &MockAdmission) -> Result<(), HostStepError> {
        // The reserved state is durable and still disconnected at this cut.
        if self.cut != Some("reserve_write") {
            let state = read_desired(&self.paths, self.uid).unwrap();
            assert_eq!(state.generation, admission.generation);
            assert!(!state.connected);
        }
        self.step("admit")
    }
    fn start_admitted(&mut self, admission: MockAdmission) -> Result<(), HostStepError> {
        assert_eq!(
            read_desired(&self.paths, self.uid).unwrap().generation,
            admission.generation
        );
        self.start_prepared()
    }
    fn commit_protected(&mut self) -> Result<(), HostStepError> {
        self.commit_prepared()
    }
    fn discard_protected(&mut self) -> Result<(), HostStepError> {
        self.discard_prepared()
    }
}

#[test]
fn interval_failure_retains_armed_owner_without_stop_or_disarm() {
    for cut in [
        "interval_begin",
        "interval_complete",
        "interval_recheck",
        "interval_postcheck",
        "interval_panic",
    ] {
        let f = Fixture::new(0, None);
        let mut candidate = f.candidate(Some(cut), None);
        candidate.connect_full("fixture").unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            candidate.observe_interval()
        }));
        assert!(result.is_err() || result.unwrap().is_err());
        assert!(!f.log.borrow().contains(&"stop"));
        assert!(!f.log.borrow().contains(&"disarm"));
        assert_eq!(f.root.borrow().marker, Marker::Armed(1));
        drop(candidate);
        assert_eq!(f.drops.get(), 0);
    }
}
#[test]
fn positive_interval_is_one_use_then_existing_explicit_close() {
    let f = Fixture::new(0, None);
    let mut candidate = f.candidate(None, None);
    candidate.connect_full("fixture").unwrap();
    candidate.observe_interval().unwrap();
    assert_eq!(candidate.phase, Phase::Armed(1));
    candidate.disconnect().unwrap();
    let log = f.log.borrow();
    assert!(
        log.iter().position(|s| *s == "interval_complete").unwrap()
            < log.iter().position(|s| *s == "stop").unwrap()
    );
    drop(log);
    drop(candidate);
    assert_eq!(f.drops.get(), 3);
}
#[test]
fn second_interval_never_spawns_or_disconnects() {
    let f = Fixture::new(0, None);
    let mut candidate = f.candidate(None, None);
    candidate.connect_full("fixture").unwrap();
    candidate.observe_interval().unwrap();
    let before = f.log.borrow().clone();
    assert!(candidate.observe_interval().is_err());
    assert_eq!(*f.log.borrow(), before);
    drop(candidate);
    assert_eq!(f.drops.get(), 0);
}
impl LifecycleHost for Host {
    fn protected_preflight(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        self.step("preflight")?;
        if self.unsupported {
            Err(HostStepError::Prepare)
        } else {
            Ok(())
        }
    }
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.step(if self.running { "verify" } else { "empty" })?;
        Ok(OwnedObservation {
            service_active: self.running,
            controller_ready: self.running,
            core_count: u8::from(self.running),
            tun_count: u8::from(self.running),
            active_profile_matches: self.running,
        })
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        self.step("prepare")?;
        if self.cut == Some("reserve_write") {
            // Owned synthetic file only: force the next ordinary writer refusal.
            std::fs::remove_file(&self.paths.file).unwrap();
            std::fs::create_dir(&self.paths.file).unwrap();
        }
        Ok(())
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        let saved = read_desired(&self.paths, self.uid).unwrap();
        assert!(saved.connected);
        self.step("start")?;
        self.running = true;
        Ok(())
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        self.step("commit")
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        assert!(!read_desired(&self.paths, self.uid).unwrap().connected);
        self.step("stop")?;
        self.running = false;
        Ok(())
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        self.step("discard")
    }
}

struct Port {
    log: Rc<RefCell<Vec<&'static str>>>,
    drops: Rc<Cell<u8>>,
    root: Rc<RefCell<Observation>>,
    paths: DesiredPaths,
    uid: u32,
    cut: Option<&'static str>,
}
impl Drop for Port {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl ProtectionPort for Port {
    fn exchange(&mut self, request: Request) -> Result<Response, ()> {
        let name = match request {
            Request::Status {} => "status",
            Request::Arm { .. } => "arm",
            Request::Disarm { .. } => "disarm",
        };
        self.log.borrow_mut().push(name);
        if name == "status" && self.log.borrow().contains(&"disarm") {
            if self.cut == Some("final_status_unknown") {
                return Err(());
            }
            if self.cut == Some("final_status_wrong") {
                return Ok(Response::Status {
                    policy_version: POLICY_VERSION,
                    protection: Protection::Disarmed {
                        closed_generation: None,
                    },
                    health: Health::Verified,
                });
            }
        }
        if let Request::Arm { generation, .. } = request {
            let saved = read_desired(&self.paths, self.uid).unwrap();
            assert!(!saved.connected && saved.generation == generation);
        }
        if let Request::Disarm { .. } = request {
            assert!(!read_desired(&self.paths, self.uid).unwrap().connected);
            let log = self.log.borrow();
            assert_eq!(log[log.len() - 2], "empty");
        }
        if self.cut == Some("status_unknown") && name == "status" {
            return Err(());
        }
        let mut state = self.root.borrow_mut();
        let mut tx = transaction::plan(request, *state).map_err(|_| ())?;
        while let Some(effect) = tx.next_effect() {
            match effect {
                Effect::CreateTableAtomic(p) | Effect::ReplaceOwnedTableAtomic(p) => {
                    state.table = Table::OwnedVerified(p)
                }
                Effect::PersistArmedDurably(n) => state.marker = Marker::Armed(n),
                Effect::PersistClosedDurably(n) => state.marker = Marker::Closed(n),
                Effect::DeleteOwnedTableAtomic => state.table = Table::Absent,
                Effect::VerifyTable(_) | Effect::VerifyTableAbsent => {}
            }
            tx.acknowledge(effect, true).map_err(|_| ())?;
        }
        let mut response = tx.response().map_err(|_| ())?;
        if name == "status" && self.cut == Some("status_manual") {
            response = Response::Status {
                policy_version: POLICY_VERSION,
                protection: Protection::Disarmed {
                    closed_generation: Some(7),
                },
                health: Health::ManualRecoveryRequired,
            };
        }
        if name == "arm" && self.cut == Some("connected_write") {
            // The reservation was read above BEFORE Arm; now force its later
            // connected replacement to refuse, not magically restore it.
            std::fs::remove_file(&self.paths.file).unwrap();
            std::fs::create_dir(&self.paths.file).unwrap();
        }
        if self.cut == Some("arm_unknown") && name == "arm"
            || self.cut == Some("disarm_unknown") && name == "disarm"
        {
            return Err(());
        }
        if self.cut == Some("arm_panic") && name == "arm" {
            panic!("fixed inert cut");
        }
        if name == "arm" {
            response = match self.cut {
                Some("wrong_generation") => Response::Status {
                    policy_version: POLICY_VERSION,
                    protection: Protection::Armed { generation: 0 },
                    health: Health::Verified,
                },
                Some("wrong_policy") => Response::Status {
                    policy_version: 9,
                    protection: Protection::Armed { generation: 8 },
                    health: Health::Verified,
                },
                Some("manual_health") => Response::Status {
                    policy_version: POLICY_VERSION,
                    protection: Protection::Armed { generation: 8 },
                    health: Health::ManualRecoveryRequired,
                },
                Some("error_reply") => Response::Error {
                    code: omavless_netguard::protocol::ErrorCode::ManualRecoveryRequired,
                },
                _ => response,
            };
        }
        if self.cut == Some("wrong_closed") && name == "disarm" {
            response = Response::Status {
                policy_version: POLICY_VERSION,
                protection: Protection::Disarmed {
                    closed_generation: None,
                },
                health: Health::Verified,
            };
        }
        Ok(response)
    }
}
struct Fixture {
    _tmp: tempfile::TempDir,
    paths: DesiredPaths,
    uid: u32,
    log: Rc<RefCell<Vec<&'static str>>>,
    drops: Rc<Cell<u8>>,
    root: Rc<RefCell<Observation>>,
}

#[cfg(feature = "netguard-native-scenario")]
#[test]
fn diagnostic_origin_calls_distinguish_reserve_arm_write_and_start_without_extra_checks() {
    use crate::protected_native_diagnostic::{self as diagnostic, Cut, Site};
    diagnostic::mark(Cut::NotEntered);
    let f = Fixture::new(0, None);
    let mut initial = f.candidate(None, None);
    let owned = initial.owned.take();
    let calls = RefCell::new(Vec::new());
    let mut origin = || {
        diagnostic::enter_origin();
        calls.borrow_mut().push(diagnostic::origin());
        Ok(())
    };
    // Same initial invocation as coordinator, then actual candidate call sites.
    origin().unwrap();
    let mut candidate = ProtectedCandidate {
        owned,
        phase: Phase::Fresh,
        admission: None,
        interval: None,
        origin: Some(&mut origin),
    };
    candidate.connect_full("fixture").unwrap();
    assert_eq!(
        *calls.borrow(),
        vec![
            (Site::Initial, 1),
            (Site::Local, 2),
            (Site::Local, 3),
            (Site::Local, 4),
            (Site::Status, 5),
            (Site::Preparation, 6),
            (Site::Local, 7),
            (Site::Arm, 8),
            (Site::Local, 9),
            (Site::Local, 10),
            (Site::Local, 11),
            (Site::Local, 12),
        ]
    );
    candidate.observe_interval().unwrap();
    candidate.disconnect().unwrap();
    assert!(
        calls
            .borrow()
            .iter()
            .any(|(site, _)| *site == Site::IntervalBefore)
    );
    assert!(
        calls
            .borrow()
            .iter()
            .any(|(site, _)| *site == Site::IntervalAfter)
    );
    assert!(calls.borrow().iter().any(|(site, _)| *site == Site::Disarm));
    drop(candidate);
    assert_eq!(f.drops.get(), 3);
}

#[cfg(feature = "netguard-native-scenario")]
#[test]
fn successful_origin_local18_can_precede_retained_interval_postcheck_refusal() {
    use crate::protected_native_diagnostic::{
        self as d, ControllerPhase, Cut, Endpoint, ReadinessPhase, Site,
    };
    d::mark(Cut::NotEntered);
    let f = Fixture::new(0, None);
    let mut initial = f.candidate(Some("interval_postcheck"), None);
    let owned = initial.owned.take();
    let calls = RefCell::new(Vec::new());
    let mut origin = || {
        d::enter_origin();
        d::mark(Cut::OriginLogin);
        // Models a successful checker observation, NOT a genuine receipt or
        // authority. The later mock host refusal is a separate operation.
        let _login = crate::login_transaction::diagnostic::begin();
        calls.borrow_mut().push(d::origin());
        Ok(())
    };
    origin().unwrap();
    let mut candidate = ProtectedCandidate {
        owned,
        phase: Phase::Fresh,
        admission: None,
        interval: None,
        origin: Some(&mut origin),
    };
    candidate.connect_full("fixture").unwrap();
    // A prior readonly observation remains stale through unrelated fences.
    {
        let _scope = d::readiness_scope();
        d::readiness_mark(Endpoint::Proxies, ReadinessPhase::FinalDeadline);
        d::controller_mark(ControllerPhase::Exchange);
    }
    assert!(candidate.observe_interval().is_err());
    assert_eq!(d::origin(), (Site::Local, 18));
    assert_eq!(d::last(), Cut::OriginLogin);
    assert_eq!(
        crate::login_transaction::diagnostic::last(),
        (
            crate::login_transaction::diagnostic::Reason::NoFailure,
            crate::login_transaction::diagnostic::Io::None
        )
    );
    assert_eq!(
        d::readiness(),
        (
            Endpoint::Proxies,
            ReadinessPhase::FinalDeadline,
            ControllerPhase::Exchange
        )
    );
    assert_eq!(
        &calls.borrow()[12..],
        &[
            (Site::Local, 13),
            (Site::Local, 14),
            (Site::Local, 15),
            (Site::Local, 16),
            (Site::IntervalBefore, 17),
            (Site::Local, 18)
        ]
    );
    let log = f.log.borrow();
    assert!(log.contains(&"interval_complete"));
    assert!(log.contains(&"interval_postcheck"));
    assert!(!log.contains(&"stop") && !log.contains(&"disarm"));
    drop(log);
    assert_eq!(candidate.phase, Phase::Poisoned);
    assert_eq!(f.root.borrow().marker, Marker::Armed(1));
    // Mock host refusal has no native subguard; the caller records the last
    // desired-equality check, demonstrating why native reached hooks matter.
    assert_eq!(
        d::post(),
        (d::PostGuard::DesiredEquality, d::PostRefusal::NotRecorded)
    );
    drop(candidate);
    assert_eq!(f.drops.get(), 0);
}

#[cfg(feature = "netguard-native-scenario")]
#[test]
fn diagnostic_distinguishes_origin_read_empty_eligibility_and_status_cuts() {
    use crate::protected_native_diagnostic::{Cut, last, mark};

    let f = Fixture::new(0, None);
    let mut initial = f.candidate(None, None);
    let (Executor::Owned(executor), port) = initial.owned.take().unwrap() else {
        panic!("fixed owned fixture");
    };
    let mut origin = || {
        mark(Cut::OriginEnvelope);
        Err(LifecycleError::ManualRecoveryRequired)
    };
    let mut candidate = ProtectedCandidate {
        owned: Some((Executor::Owned(executor), port)),
        phase: Phase::Fresh,
        admission: None,
        interval: None,
        origin: Some(&mut origin),
    };
    assert!(candidate.connect_full("fixture").is_err());
    assert_eq!(last(), Cut::OriginEnvelope);
    assert!(f.log.borrow().is_empty());
    assert_eq!(candidate.phase, Phase::Poisoned);
    drop(candidate);
    assert_eq!(f.drops.get(), 0);

    let f = Fixture::new(0, None);
    let mut candidate = f.candidate(None, None);
    // A missing Desired file legitimately reads as the default Off intent.
    // Malformed bytes, not absence, exercise the actual read-refusal boundary.
    std::fs::write(&f.paths.file, b"{").unwrap();
    assert!(candidate.connect_full("fixture").is_err());
    assert_eq!(last(), Cut::DesiredRead);
    assert!(f.log.borrow().is_empty());
    drop(candidate);
    assert_eq!(f.drops.get(), 0);

    for (host_cut, port_cut, expected) in [
        (Some("empty"), None, Cut::EmptyObservation),
        (Some("preflight"), None, Cut::ProtectedEligibility),
        (None, Some("status_manual"), Cut::StatusInterpretation),
    ] {
        let f = Fixture::new(0, None);
        let mut candidate = f.candidate(host_cut, port_cut);
        assert!(candidate.connect_full("fixture").is_err());
        assert_eq!(last(), expected);
        assert!(!f.log.borrow().contains(&"prepare"));
        assert!(!f.log.borrow().contains(&"arm"));
        assert_eq!(candidate.phase, Phase::Poisoned);
        drop(candidate);
        assert_eq!(f.drops.get(), 0);
    }
}

#[test]
fn borrowed_same_executor_checks_original_fence_and_never_compensates() {
    for fail_at in 1..=48 {
        let f = Fixture::new(0, None);
        let mut initial = f.candidate(None, None);
        let (Executor::Owned(executor), port) = initial.owned.take().unwrap() else {
            panic!()
        };
        // Models the outer guard's retention, never a second executor.
        let mut retained = std::mem::ManuallyDrop::new(executor);
        let calls = Cell::new(0);
        let mut fence = || {
            let n = calls.get() + 1;
            calls.set(n);
            if n == fail_at {
                Err(LifecycleError::ManualRecoveryRequired)
            } else {
                Ok(())
            }
        };
        let mut candidate = ProtectedCandidate {
            owned: Some((Executor::Borrowed(&mut retained), port)),
            phase: Phase::Fresh,
            admission: None,
            interval: None,
            origin: Some(&mut fence),
        };
        let result = candidate
            .connect_full("fixture")
            .and_then(|_| candidate.observe_interval())
            .and_then(|_| candidate.disconnect());
        if calls.get() == fail_at {
            assert!(result.is_err());
            assert_eq!(candidate.phase, Phase::Poisoned);
            let before = f.log.borrow().clone();
            assert!(candidate.disconnect().is_err());
            assert_eq!(*f.log.borrow(), before);
        }
        drop(candidate);
        assert_eq!(
            retained.actual(),
            if result.is_err() {
                ActualState::ManualRecoveryRequired
            } else {
                ActualState::Disconnected
            }
        );
        // Outer guard, not the borrowed reference, retains this original Host.
    }
}
impl Fixture {
    fn new(desired_generation: u64, floor: Option<u64>) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let paths = DesiredPaths::below(tmp.path());
        let uid = nix::unistd::getuid().as_raw();
        write_desired(
            &paths,
            uid,
            &DesiredState {
                generation: desired_generation,
                ..DesiredState::default()
            },
        )
        .unwrap();
        Self {
            _tmp: tmp,
            paths,
            uid,
            log: Rc::default(),
            drops: Rc::default(),
            root: Rc::new(RefCell::new(Observation {
                marker: floor.map_or(Marker::Missing, Marker::Closed),
                table: Table::Absent,
            })),
        }
    }
    fn candidate(
        &self,
        host_cut: Option<&'static str>,
        port_cut: Option<&'static str>,
    ) -> ProtectedCandidate<'static, Host, Port> {
        ProtectedCandidate::new(
            LifecycleExecutor::new(
                Host {
                    log: self.log.clone(),
                    drops: self.drops.clone(),
                    paths: self.paths.clone(),
                    uid: self.uid,
                    running: false,
                    cut: host_cut,
                    unsupported: false,
                },
                self.paths.clone(),
                self.uid,
            ),
            Port {
                log: self.log.clone(),
                drops: self.drops.clone(),
                root: self.root.clone(),
                paths: self.paths.clone(),
                uid: self.uid,
                cut: port_cut,
            },
        )
    }
    fn desired(&self) -> DesiredState {
        read_desired(&self.paths, self.uid).unwrap()
    }
}

#[test]
fn natural_status_floor_orders_reserve_arm_connected_core_and_explicit_disarm() {
    for (desired, floor, expected) in [(3, Some(7), 8), (9, Some(7), 10), (3, None, 4)] {
        let f = Fixture::new(desired, floor);
        let mut c = f.candidate(None, None);
        assert_eq!(
            c.connect_full("public-fixture").unwrap().generation,
            expected
        );
        assert_eq!(f.root.borrow().marker, Marker::Armed(expected));
        assert!(f.desired().connected);
        assert_eq!(c.disconnect().unwrap().actual, ActualState::Disconnected);
        assert_eq!(f.root.borrow().marker, Marker::Closed(expected));
        assert_eq!(f.desired().generation, expected + 1);
        assert!(!f.desired().connected);
        assert_eq!(
            *f.log.borrow(),
            vec![
                "empty",
                "preflight",
                "status",
                "prepare",
                "validation_reaped",
                "coverage",
                "admit",
                "arm",
                "start",
                "verify",
                "commit",
                "stop",
                "discard",
                "empty",
                "disarm",
                "status"
            ]
        );
        drop(c);
        assert_eq!(f.drops.get(), 2);
    }
}
#[test]
fn final_status_is_distinct_and_unknown_never_retries_or_releases() {
    for cut in ["final_status_unknown", "final_status_wrong"] {
        let f = Fixture::new(0, None);
        let mut c = f.candidate(None, Some(cut));
        c.connect_full("fixture").unwrap();
        assert!(c.disconnect().is_err());
        assert_eq!(c.phase, Phase::Poisoned);
        assert_eq!(f.root.borrow().marker, Marker::Closed(1));
        let before = f.log.borrow().clone();
        assert!(c.disconnect().is_err());
        assert_eq!(*f.log.borrow(), before);
        drop(c);
        assert_eq!(f.drops.get(), 0);
    }
}
#[test]
fn initial_non_disarmed_or_uncertain_status_never_arms_or_starts() {
    for cut in [None, Some("status_unknown"), Some("status_manual")] {
        let f = Fixture::new(3, Some(7));
        if cut.is_none() {
            *f.root.borrow_mut() = Observation {
                marker: Marker::Armed(7),
                table: Table::OwnedVerified(Policy::FullVpn),
            };
        }
        let mut c = f.candidate(None, cut);
        assert!(c.connect_full("public-fixture").is_err());
        assert!(!f.log.borrow().contains(&"arm"));
        assert!(!f.log.borrow().contains(&"prepare"));
        let before = f.log.borrow().len();
        assert!(c.connect_full("public-fixture").is_err());
        assert_eq!(before, f.log.borrow().len());
    }
}
#[test]
fn exhaustion_and_unsupported_preflight_do_not_reserve_arm_or_start() {
    for floor in [
        Some(MAX_GENERATION - 1),
        Some(MAX_GENERATION),
        Some(u64::MAX),
    ] {
        let f = Fixture::new(3, floor);
        let mut c = f.candidate(None, None);
        assert!(c.connect_full("public-fixture").is_err());
        assert_eq!(f.desired().generation, 3);
        assert!(!f.log.borrow().contains(&"arm"));
    }
    let f = Fixture::new(3, Some(7));
    let mut c = f.candidate(None, None);
    c.owned.as_mut().unwrap().0.host.unsupported = true;
    assert!(c.connect_full("public-fixture").is_err());
    assert_eq!(*f.log.borrow(), vec!["empty", "preflight"]);
}
#[test]
fn uncertain_pre_arm_validation_never_discards_or_retries() {
    let f = Fixture::new(3, Some(7));
    let mut c = f.candidate(Some("prepare"), None);
    assert_eq!(
        c.connect_full("public-fixture"),
        Err(LifecycleError::ManualRecoveryRequired)
    );
    assert_eq!(
        *f.log.borrow(),
        vec!["empty", "preflight", "status", "prepare"]
    );
    assert_eq!(f.desired().generation, 3);
    assert_eq!(c.phase, Phase::Poisoned);
    let before = f.log.borrow().len();
    assert!(c.connect_full("public-fixture").is_err());
    assert_eq!(before, f.log.borrow().len());
    drop(c);
    assert_eq!(f.drops.get(), 0);
}

#[test]
fn admission_recheck_is_after_reservation_but_before_any_arm() {
    let f = Fixture::new(3, Some(7));
    let mut c = f.candidate(Some("admit"), None);
    assert_eq!(
        c.connect_full("public-fixture"),
        Err(LifecycleError::ManualRecoveryRequired)
    );
    assert_eq!(f.desired().generation, 8);
    assert!(!f.desired().connected);
    assert_eq!(
        *f.log.borrow(),
        vec![
            "empty",
            "preflight",
            "status",
            "prepare",
            "validation_reaped",
            "coverage",
            "admit"
        ]
    );
    assert_eq!(f.root.borrow().marker, Marker::Closed(7));
    drop(c);
    assert_eq!(f.drops.get(), 0);
}

#[test]
fn validation_and_coverage_cuts_do_not_reserve_or_arm() {
    for cut in ["validation_reaped", "coverage"] {
        let f = Fixture::new(3, Some(7));
        let mut c = f.candidate(Some(cut), None);
        assert_eq!(
            c.connect_full("public-fixture"),
            Err(LifecycleError::ManualRecoveryRequired)
        );
        assert_eq!(f.desired().generation, 3);
        assert!(!f.log.borrow().contains(&"arm"));
        assert!(!f.log.borrow().contains(&"discard"));
        drop(c);
        assert_eq!(f.drops.get(), 0);
    }
}
#[test]
fn every_post_arm_core_cut_keeps_protection_and_has_no_compensation() {
    for cut in ["start", "verify", "commit"] {
        let f = Fixture::new(3, Some(7));
        let mut c = f.candidate(Some(cut), None);
        assert_eq!(
            c.connect_full("public-fixture"),
            Err(LifecycleError::ManualRecoveryRequired)
        );
        assert_eq!(f.root.borrow().marker, Marker::Armed(8));
        assert!(f.desired().connected);
        assert!(!f.log.borrow().contains(&"disarm"));
        assert!(!f.log.borrow().contains(&"stop"));
        assert!(!f.log.borrow().contains(&"discard"));
        let before = f.log.borrow().len();
        assert!(c.disconnect().is_err());
        assert_eq!(before, f.log.borrow().len());
        drop(c);
        assert_eq!(f.drops.get(), 0);
    }
}
#[test]
fn uncertain_late_error_and_mismatched_arm_reply_poison_original_graph() {
    for cut in [
        "arm_unknown",
        "wrong_generation",
        "wrong_policy",
        "manual_health",
        "error_reply",
    ] {
        let f = Fixture::new(3, Some(7));
        let mut c = f.candidate(None, Some(cut));
        assert!(c.connect_full("public-fixture").is_err());
        assert_eq!(f.root.borrow().marker, Marker::Armed(8));
        assert_eq!(f.desired().generation, 8);
        assert!(!f.desired().connected);
        assert!(!f.log.borrow().contains(&"start"));
        assert!(!f.log.borrow().contains(&"disarm"));
        drop(c);
        assert_eq!(f.drops.get(), 0);
        // A later independently assembled test owner sees real armed Status,
        // not a fresh/null floor and cannot recycle the reserved generation.
        let mut successor = f.candidate(None, None);
        assert!(successor.connect_full("public-fixture").is_err());
        assert_eq!(f.desired().generation, 8);
    }
}
#[test]
fn panic_after_effect_entry_retains_both_original_owners() {
    let f = Fixture::new(3, Some(7));
    let mut c = f.candidate(None, Some("arm_panic"));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || c.connect_full("public-fixture")
        ))
        .is_err()
    );
    assert_eq!(c.phase, Phase::InFlight);
    drop(c);
    assert_eq!(f.drops.get(), 0);
}
#[test]
fn failed_stop_or_cleanup_never_disarms_or_falls_back() {
    for cut in ["stop", "discard", "empty"] {
        let f = Fixture::new(3, Some(7));
        let mut c = f.candidate(None, None);
        c.connect_full("public-fixture").unwrap();
        c.owned.as_mut().unwrap().0.host.cut = Some(cut);
        assert!(c.disconnect().is_err());
        assert!(!f.desired().connected);
        assert_eq!(f.root.borrow().marker, Marker::Armed(8));
        assert!(!f.log.borrow().contains(&"disarm"));
        drop(c);
        assert_eq!(f.drops.get(), 0);
    }
}
#[test]
fn lost_disarm_or_wrong_floor_never_claims_disconnected_success() {
    for cut in ["disarm_unknown", "wrong_closed"] {
        let f = Fixture::new(3, Some(7));
        let mut c = f.candidate(None, None);
        c.connect_full("public-fixture").unwrap();
        c.owned.as_mut().unwrap().1.cut = Some(cut);
        assert_eq!(c.disconnect(), Err(LifecycleError::ManualRecoveryRequired));
        assert_eq!(f.root.borrow().marker, Marker::Closed(8));
        let before = f.log.borrow().len();
        assert!(c.disconnect().is_err());
        assert_eq!(before, f.log.borrow().len());
        drop(c);
        assert_eq!(f.drops.get(), 0);
    }
}

#[test]
fn failed_desired_writes_cannot_arm_before_reservation_or_compensate_after_arm() {
    let f = Fixture::new(3, Some(7));
    let mut c = f.candidate(Some("reserve_write"), None);
    assert_eq!(
        c.connect_full("public-fixture"),
        Err(LifecycleError::ManualRecoveryRequired)
    );
    assert_eq!(f.root.borrow().marker, Marker::Closed(7));
    assert!(!f.log.borrow().contains(&"arm"));
    assert!(f.log.borrow().contains(&"discard")); // known NO Arm
    assert_eq!(c.phase, Phase::Poisoned);
    let before = f.log.borrow().len();
    assert!(c.connect_full("public-fixture").is_err());
    assert_eq!(before, f.log.borrow().len());

    let f = Fixture::new(3, Some(7));
    let mut c = f.candidate(None, Some("connected_write"));
    assert_eq!(
        c.connect_full("public-fixture"),
        Err(LifecycleError::ManualRecoveryRequired)
    );
    assert_eq!(f.root.borrow().marker, Marker::Armed(8));
    for forbidden in ["start", "stop", "discard", "disarm"] {
        assert!(!f.log.borrow().contains(&forbidden));
    }
    drop(c);
    assert_eq!(f.drops.get(), 0);
}

#[test]
fn connected_restart_is_not_adopted_or_recovered_by_the_fresh_candidate() {
    let f = Fixture::new(8, Some(7));
    write_desired(
        &f.paths,
        f.uid,
        &DesiredState {
            generation: 8,
            connected: true,
            profile_id: "public-fixture".into(),
            mode: RoutingMode::Global,
            ..DesiredState::default()
        },
    )
    .unwrap();
    let mut c = f.candidate(None, None);
    assert_eq!(
        c.connect_full("public-fixture"),
        Err(LifecycleError::ManualRecoveryRequired)
    );
    assert!(f.log.borrow().is_empty());
    assert!(f.desired().connected);
    drop(c);
    assert_eq!(f.drops.get(), 0);
}
