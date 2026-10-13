// SPDX-License-Identifier: MIT
//! File/mock controls. Synthetic inputs are NOT admitted core/MRS artifacts;
//! fake zero is completion-model evidence, never namespace/engine validation.
use super::*;
use std::cell::Cell;
use std::collections::VecDeque;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::process::ExitStatusExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

struct Inputs {
    dropped: Rc<Cell<usize>>,
    resources: Vec<(String, Vec<u8>)>,
    panic_core: Rc<Cell<bool>>,
    anchor: Rc<Cell<Option<i32>>>,
    panic_anchor: bool,
    panic_file_anchor: bool,
}
impl Inputs {
    fn new(dropped: Rc<Cell<usize>>) -> Self {
        Self {
            dropped,
            panic_core: Rc::new(Cell::new(false)),
            anchor: Rc::new(Cell::new(None)),
            panic_anchor: false,
            panic_file_anchor: false,
            resources: vec![(
                "synthetic.mrs".into(),
                b"SYNTHETIC_NON_MRS_PAYLOAD".to_vec(),
            )],
        }
    }
}
impl CapturedInputs for Inputs {
    fn core(&self) -> &[u8] {
        assert!(!self.panic_core.get(), "synthetic proof panic");
        b"SYNTHETIC_NONEXECUTABLE_CORE"
    }
    fn config(&self) -> &[u8] {
        b"SYNTHETIC_PRIVATE_CONFIG_CANARY"
    }
    fn resources(&self) -> &[(String, Vec<u8>)] {
        &self.resources
    }
    fn after_staged_anchor(&self, scratch: &Scratch) {
        self.anchor.set(Some(scratch.files.first().map_or_else(
            || scratch.directory.as_raw_fd(),
            |(_, held)| held.as_raw_fd(),
        )));
        assert!(!self.panic_anchor, "synthetic staging panic");
        assert!(
            !self.panic_file_anchor || scratch.files.is_empty(),
            "synthetic file-anchor panic"
        );
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
    fn stage(&self) -> Armed<Inputs, FakeChild> {
        stage_owned(Inputs::new(Rc::new(Cell::new(0))), &self.parent, self.uid).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.parent).unwrap();
    }
}
#[derive(Clone)]
struct FakeClock(Rc<Cell<Duration>>, Rc<Cell<usize>>, Rc<Cell<Option<usize>>>);
impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.1.set(self.1.get() + 1);
        assert!(self.2.get() != Some(self.1.get()), "synthetic clock panic");
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
    panic_wait: bool,
    panic_kill: bool,
    proof_panic_on_wait: Option<Rc<Cell<bool>>>,
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
            panic_wait: false,
            panic_kill: false,
            proof_panic_on_wait: None,
        }
    }
}
impl OriginalChild for FakeChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.waits.set(self.waits.get() + 1);
        assert!(!self.panic_wait, "synthetic wait panic");
        if let Some(flag) = &self.proof_panic_on_wait {
            flag.set(true);
        }
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
        assert!(!self.panic_kill, "synthetic kill panic");
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
    FakeClock(
        Rc::new(Cell::new(Duration::ZERO)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(None)),
    )
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
    let input_drop = scope.input.dropped.clone();
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
    let child_drop = child.dropped.clone();
    let completed = supervise(scope, timer, |_| Ok(child)).ok().unwrap();
    finish_model(completed).unwrap();
    assert_eq!(input_drop.get(), 1);
    assert_eq!(child_drop.get(), 1);
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
    let mut owner = Armed::new(Inputs::new(input_drop.clone()), fixture.uid);
    owner.partial = Some(PartialScratch {
        path: fixture.parent.clone(),
        directory: None,
        rules: None,
    });
    owner.partial.as_mut().unwrap().directory = Some(open_directory(&fixture.parent).unwrap());
    let fd = owner
        .partial
        .as_ref()
        .unwrap()
        .directory
        .as_ref()
        .unwrap()
        .as_raw_fd();
    let child = FakeChild::new(clock(), []);
    let child_drop = child.dropped.clone();
    owner.child = Some(child);
    let failure = retain(Refusal::Staging, owner);
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
    let failure = retain::<_, FakeChild>(Refusal::Cleanup, scope);
    drop(failure);
    assert!(path.exists());
    assert!(fs::metadata(format!("/proc/self/fd/{fd}")).is_ok());
    assert!(path.join("ruleset/foreign").exists());
}

#[test]
fn staging_unwind_retains_input_and_first_directory_anchor() {
    for file_anchor in [false, true] {
        let fixture = Fixture::new();
        let dropped = Rc::new(Cell::new(0));
        let mut input = Inputs::new(dropped.clone());
        input.panic_anchor = !file_anchor;
        input.panic_file_anchor = file_anchor;
        let anchor = input.anchor.clone();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _ = stage_owned::<_, FakeChild>(input, &fixture.parent, fixture.uid);
        }));
        assert!(outcome.is_err());
        assert_eq!(dropped.get(), 0);
        let fd_path = PathBuf::from(format!("/proc/self/fd/{}", anchor.get().unwrap()));
        if file_anchor {
            assert!(fs::read(fd_path).unwrap() == b"SYNTHETIC_NONEXECUTABLE_CORE");
        } else {
            assert!(fs::metadata(&fd_path).unwrap().is_dir());
            assert_eq!(fs::read_dir(fd_path).unwrap().count(), 1); // retained ruleset
        }
    }
}

#[test]
fn launch_child_wait_clock_proof_and_cleanup_unwind_never_drop_owned_resources() {
    for phase in [
        "launch",
        "wait",
        "clock",
        "kill",
        "pre-proof",
        "post-proof",
        "cleanup-proof",
    ] {
        let fixture = Fixture::new();
        let dropped = Rc::new(Cell::new(0));
        let input = Inputs::new(dropped.clone());
        let proof_panic = input.panic_core.clone();
        let owner = stage_owned::<_, FakeChild>(input, &fixture.parent, fixture.uid).unwrap();
        let fd = owner.scratch.as_ref().unwrap().files[0].1.as_raw_fd();
        let timer = clock();
        let child_drop = Rc::new(Cell::new(0));
        if phase == "clock" {
            timer.2.set(Some(2));
        } // immediately after launch return
        if phase == "pre-proof" {
            proof_panic.set(true);
        }
        // No child exists in an owner until launch returns. For launch/pre-proof
        // panics don't construct a callee-owned fake child whose Drop is outside
        // that guarantee: these rows assert input/FD retention only.
        let child = if phase == "launch" || phase == "pre-proof" {
            None
        } else {
            let mut child = FakeChild::new(timer.clone(), [Observation::Code(0)]);
            child.dropped = child_drop.clone();
            child.panic_wait = phase == "wait";
            child.panic_kill = phase == "kill";
            if phase == "post-proof" {
                child.proof_panic_on_wait = Some(proof_panic.clone());
            }
            Some(child)
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let completed = supervise(owner, timer.clone(), |_| {
                assert!(phase != "launch", "synthetic launch panic");
                if phase == "kill" {
                    timer.0.set(EXECUTION_BUDGET);
                }
                Ok(child.unwrap())
            })
            .ok()
            .unwrap();
            if phase == "cleanup-proof" {
                proof_panic.set(true);
                let _ = finish_model(completed);
            }
        }));
        assert!(outcome.is_err());
        assert_eq!(dropped.get(), 0);
        if phase != "launch" && phase != "pre-proof" {
            assert_eq!(child_drop.get(), 0);
        }
        // Reopen the exact original inode through the retained FD, not its
        // possibly replaceable former pathname; no process/core was executed.
        assert!(
            fs::read(format!("/proc/self/fd/{fd}")).unwrap() == b"SYNTHETIC_NONEXECUTABLE_CORE"
        );
    }
}

#[test]
fn consumer_drop_of_staged_or_completed_owner_remains_armed_until_proved_cleanup() {
    for completed in [false, true] {
        let fixture = Fixture::new();
        let owner = fixture.stage();
        let input_drop = owner.input.dropped.clone();
        let fd = owner.scratch.as_ref().unwrap().files[0].1.as_raw_fd();
        if completed {
            let timer = clock();
            let child = FakeChild::new(timer.clone(), [Observation::Code(0)]);
            let child_drop = child.dropped.clone();
            let completion = supervise(owner, timer, |_| Ok(child)).ok().unwrap();
            drop(completion);
            assert_eq!(child_drop.get(), 0);
        } else {
            drop(owner);
        }
        assert_eq!(input_drop.get(), 0);
        assert!(
            fs::read(format!("/proc/self/fd/{fd}")).unwrap() == b"SYNTHETIC_NONEXECUTABLE_CORE"
        );
    }
}
