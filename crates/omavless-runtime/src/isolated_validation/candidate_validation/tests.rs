// SPDX-License-Identifier: MIT
//! File/mock controls. Synthetic inputs are NOT admitted core/MRS artifacts;
//! fake zero is completion-model evidence, never namespace/engine validation.
use super::*;
use std::cell::Cell;
use std::collections::VecDeque;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::process::ExitStatusExt;
use std::rc::Rc;

struct Inputs {
    dropped: Rc<Cell<usize>>,
    resources: Vec<(String, Vec<u8>)>,
}
impl Inputs {
    fn new(dropped: Rc<Cell<usize>>) -> Self {
        Self {
            dropped,
            resources: vec![(
                "synthetic.mrs".into(),
                b"SYNTHETIC_NON_MRS_PAYLOAD".to_vec(),
            )],
        }
    }
}
impl CapturedInputs for Inputs {
    fn core(&self) -> &[u8] {
        b"SYNTHETIC_NONEXECUTABLE_CORE"
    }
    fn config(&self) -> &[u8] {
        b"SYNTHETIC_PRIVATE_CONFIG_CANARY"
    }
    fn resources(&self) -> &[(String, Vec<u8>)] {
        &self.resources
    }
}
impl Drop for Inputs {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}
struct Fixture {
    parent: PathBuf,
    uid: u32,
}
impl Fixture {
    fn new() -> Self {
        Self {
            parent: crate::test_temp::directory("p4-stage").unwrap(),
            uid: nix::unistd::Uid::current().as_raw(),
        }
    }
    fn stage(&self) -> Scope<Inputs> {
        stage_owned(Inputs::new(Rc::new(Cell::new(0))), &self.parent, self.uid).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.parent).unwrap();
    }
}
#[derive(Clone)]
struct FakeClock(Rc<Cell<Duration>>);
impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.0.get()
    }
    fn pause(&mut self) {
        self.0.set(self.0.get() + POLL);
    }
}
enum Observation {
    Pending,
    Code(i32),
    Signal(i32),
    Error,
}
struct FakeChild {
    clock: FakeClock,
    wait_cost: Duration,
    observations: VecDeque<Observation>,
    waits: Rc<Cell<usize>>,
    kills: Rc<Cell<usize>>,
    dropped: Rc<Cell<usize>>,
    kill_error: bool,
}
impl FakeChild {
    fn new(clock: FakeClock, observations: impl IntoIterator<Item = Observation>) -> Self {
        Self {
            clock,
            wait_cost: Duration::ZERO,
            observations: observations.into_iter().collect(),
            waits: Rc::new(Cell::new(0)),
            kills: Rc::new(Cell::new(0)),
            dropped: Rc::new(Cell::new(0)),
            kill_error: false,
        }
    }
}
impl OriginalChild for FakeChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.waits.set(self.waits.get() + 1);
        self.clock.0.set(self.clock.0.get() + self.wait_cost);
        match self
            .observations
            .pop_front()
            .unwrap_or(Observation::Pending)
        {
            Observation::Pending => Ok(None),
            Observation::Code(code) => Ok(Some(ExitStatus::from_raw(code << 8))),
            Observation::Signal(signal) => Ok(Some(ExitStatus::from_raw(signal))),
            Observation::Error => Err(io::Error::other("synthetic wait refusal")),
        }
    }
    fn kill(&mut self) -> io::Result<()> {
        self.kills.set(self.kills.get() + 1);
        if self.kill_error {
            Err(io::Error::other("synthetic kill refusal"))
        } else {
            Ok(())
        }
    }
}
impl Drop for FakeChild {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}
fn clock() -> FakeClock {
    FakeClock(Rc::new(Cell::new(Duration::ZERO)))
}

#[test]
fn candidate_argv_has_only_pid1_prefix_delta_and_no_launch_in_this_control() {
    let path = Path::new("/synthetic-fixed-staging");
    let old = super::super::sandbox_command(path);
    let new = candidate_command(path);
    assert_eq!(new.get_program(), old.get_program());
    let mut args = new.get_args().map(OsString::from).collect::<Vec<_>>();
    let index = args.iter().position(|arg| arg == "--as-pid-1").unwrap();
    assert!(index < args.iter().position(|arg| arg == "/input/core").unwrap());
    assert_eq!(args.iter().filter(|arg| *arg == "--as-pid-1").count(), 1);
    args.remove(index);
    assert_eq!(args, old.get_args().map(OsString::from).collect::<Vec<_>>());
    assert_eq!(new.get_envs().count(), 0);
    // Source builder retains env_clear and all-null stdio; neither command runs.
}

#[test]
fn exact_owned_bytes_stage_and_model_completion_cleanup_without_core_execution() {
    let fixture = Fixture::new();
    let scope = fixture.stage();
    let path = scope.scratch.as_ref().unwrap().path.clone();
    let scratch = scope.scratch.as_ref().unwrap();
    assert!(scope.prove().is_ok());
    assert_eq!(scratch.directory.metadata().unwrap().mode() & 0o7777, 0o700);
    for ((path, held), bytes) in scratch
        .files
        .iter()
        .zip([scope.input.core(), scope.input.config()])
    {
        assert!(fs::read(path).unwrap() == bytes);
        assert_eq!(held.metadata().unwrap().nlink(), 1);
    }
    let timer = clock();
    let child = FakeChild::new(timer.clone(), [Observation::Code(0)]);
    let completed = supervise(scope, timer, |_| Ok(child)).ok().unwrap();
    finish_model(completed).unwrap();
    assert!(!path.exists());
}

#[test]
fn proof_refuses_bytes_length_mode_links_membership_and_expected_owner_drift() {
    for fault in [
        "bytes",
        "length",
        "mode",
        "link",
        "member",
        "uid",
        "directory-mode",
        "replacement",
    ] {
        let fixture = Fixture::new();
        let mut scope = fixture.stage();
        let scratch = scope.scratch.as_ref().unwrap();
        let core = scratch.path.join("core");
        match fault {
            "bytes" | "length" => {
                fs::set_permissions(&core, fs::Permissions::from_mode(0o600)).unwrap();
                let bytes = if fault == "bytes" {
                    vec![b'!'; scope.input.core().len()]
                } else {
                    vec![b'!']
                };
                fs::write(&core, bytes).unwrap();
                fs::set_permissions(&core, fs::Permissions::from_mode(0o500)).unwrap();
            }
            "mode" => fs::set_permissions(&core, fs::Permissions::from_mode(0o700)).unwrap(),
            "link" => fs::hard_link(&core, fixture.parent.join("extra-link")).unwrap(),
            "member" => fs::write(scratch.path.join("ruleset/foreign"), b"foreign").unwrap(),
            "uid" => scope.uid = scope.uid.wrapping_add(1),
            "directory-mode" => {
                fs::set_permissions(&scratch.path, fs::Permissions::from_mode(0o755)).unwrap()
            }
            "replacement" => {
                fs::rename(&core, fixture.parent.join("original-core")).unwrap();
                symlink(fixture.parent.join("original-core"), &core).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(scope.prove(), Err(Refusal::TreeProof));
    }
}

#[test]
fn drop_retained_failure_preserves_only_child_input_and_partial_fd_anchors() {
    let fixture = Fixture::new();
    let input_drop = Rc::new(Cell::new(0));
    let held = open_directory(&fixture.parent).unwrap();
    let fd = held.as_raw_fd();
    let partial = PartialScratch {
        path: fixture.parent.clone(),
        directory: Some(held),
        rules: None,
    };
    let scope = Scope {
        input: Inputs::new(input_drop.clone()),
        uid: fixture.uid,
        partial: Some(partial),
        scratch: None,
    };
    let child = FakeChild::new(clock(), []);
    let child_drop = child.dropped.clone();
    let failure = retain(Refusal::Staging, scope, Some(child));
    assert!(!format!("{failure:?}").contains("CANARY"));
    drop(failure);
    assert_eq!(input_drop.get(), 0);
    assert_eq!(child_drop.get(), 0);
    assert!(fs::metadata(format!("/proc/self/fd/{fd}")).is_ok());
}

#[test]
fn late_zero_never_completes_or_kills_already_reaped_original() {
    let fixture = Fixture::new();
    let timer = clock();
    let mut child = FakeChild::new(timer.clone(), [Observation::Code(0)]);
    child.wait_cost = EXECUTION_BUDGET;
    let kills = child.kills.clone();
    let failure = supervise(fixture.stage(), timer, |_| Ok(child))
        .err()
        .unwrap();
    assert_eq!(failure.reason, Refusal::LateCompletion);
    assert_eq!(kills.get(), 0);
    assert!(
        failure
            .retained
            .scope
            .scratch
            .as_ref()
            .unwrap()
            .path
            .exists()
    );
}

#[test]
fn expired_before_launch_never_invokes_launcher_and_slow_spawn_is_charged() {
    let fixture = Fixture::new();
    let timer = clock();
    timer.0.set(EXECUTION_BUDGET);
    let calls = Cell::new(0);
    let failure = supervise(fixture.stage(), timer.clone(), |_| {
        calls.set(calls.get() + 1);
        Ok(FakeChild::new(timer, []))
    })
    .err()
    .unwrap();
    assert_eq!(failure.reason, Refusal::Timeout);
    assert_eq!(calls.get(), 0);

    let timer = clock();
    let child = FakeChild::new(timer.clone(), [Observation::Code(0)]);
    let kills = child.kills.clone();
    let failure = supervise(fixture.stage(), timer.clone(), |_| {
        timer.0.set(EXECUTION_BUDGET);
        Ok(child)
    })
    .err()
    .unwrap();
    assert_eq!(failure.reason, Refusal::Timeout);
    assert_eq!(kills.get(), 1);
}

#[test]
fn timeout_grace_is_finite_and_reaped_launcher_zero_still_retains_scope() {
    for reaped in [false, true] {
        let fixture = Fixture::new();
        let timer = clock();
        let observations = if reaped {
            vec![Observation::Code(0)]
        } else {
            vec![]
        };
        let child = FakeChild::new(timer.clone(), observations);
        let kills = child.kills.clone();
        let failure = supervise(fixture.stage(), timer.clone(), |_| {
            timer.0.set(EXECUTION_BUDGET);
            Ok(child)
        })
        .err()
        .unwrap();
        assert_eq!(failure.reason, Refusal::Timeout);
        assert_eq!(kills.get(), 1);
        assert!(timer.0.get() <= EXECUTION_BUDGET + LAUNCHER_GRACE);
        assert!(
            failure
                .retained
                .scope
                .scratch
                .as_ref()
                .unwrap()
                .path
                .exists()
        );
        assert!(failure.retained.child.is_some());
    }
}

#[test]
fn spawn_wait_kill_and_nonzero_refusals_have_no_cleanup_or_followon_retry() {
    let fixture = Fixture::new();
    let timer = clock();
    let failure = supervise::<_, FakeChild, _>(fixture.stage(), timer, |_| {
        Err(io::Error::other("synthetic spawn refusal"))
    })
    .err()
    .unwrap();
    assert_eq!(failure.reason, Refusal::Spawn);
    for code in [
        Observation::Error,
        Observation::Code(1),
        Observation::Signal(9),
    ] {
        let timer = clock();
        let child = FakeChild::new(timer.clone(), [code]);
        let kills = child.kills.clone();
        let failure = supervise(fixture.stage(), timer, |_| Ok(child))
            .err()
            .unwrap();
        assert_eq!(failure.reason, Refusal::Wait);
        assert_eq!(kills.get(), 0);
        assert!(
            failure
                .retained
                .scope
                .scratch
                .as_ref()
                .unwrap()
                .path
                .exists()
        );
    }
    let timer = clock();
    let mut child = FakeChild::new(timer.clone(), []);
    child.kill_error = true;
    let waits = child.waits.clone();
    let failure = supervise(fixture.stage(), timer.clone(), |_| {
        timer.0.set(EXECUTION_BUDGET);
        Ok(child)
    })
    .err()
    .unwrap();
    assert_eq!(failure.reason, Refusal::Kill);
    assert_eq!(waits.get(), 0);
}

#[test]
fn postcompletion_tree_drift_refuses_and_partial_cleanup_retains_remaining_anchors() {
    let fixture = Fixture::new();
    let scope = fixture.stage();
    let path = scope.scratch.as_ref().unwrap().path.clone();
    let timer = clock();
    let child = FakeChild::new(timer.clone(), [Observation::Code(0)]);
    let failure = supervise(scope, timer, |_| {
        fs::write(path.join("foreign"), b"foreign").unwrap();
        Ok(child)
    })
    .err()
    .unwrap();
    assert_eq!(failure.reason, Refusal::TreeProof);

    // Deterministic component fault AFTER the last proof, modeling a cleanup
    // race. This synthetic fixture owns no process/namespace or live core.
    let mut scope = fixture.stage();
    scope.prove().unwrap();
    let scratch = scope.scratch.as_mut().unwrap();
    let path = scratch.path.clone();
    fs::write(path.join("ruleset/foreign"), b"foreign").unwrap();
    assert!(scratch.cleanup().is_err());
    let fd = scratch.directory.as_raw_fd();
    let failure = retain::<_, FakeChild>(Refusal::Cleanup, scope, None);
    drop(failure);
    assert!(path.exists());
    assert!(fs::metadata(format!("/proc/self/fd/{fd}")).is_ok());
    assert!(path.join("ruleset/foreign").exists());
}
