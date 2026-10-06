use super::*;
use omavless_netguard::{
    policy::Policy,
    transaction::{self, Effect, Marker, Observation, Table},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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
impl ProtectedHost for Host {
    type Admission = MockAdmission;
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

#[test]
fn borrowed_same_executor_checks_original_fence_and_never_compensates() {
    for fail_at in 1..=32 {
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
            origin: Some(&mut fence),
        };
        let result = candidate
            .connect_full("fixture")
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
