//! External-only prototype constructor. No CanonicalCreator implementation.
//! Installed origin/package acceptance remains a separate unavailable boundary.
use super::*;
use crate::kernel_observer::owned_creator::{ActualCreator, open_actual};
use crate::kernel_observer::LocalPolicyInventory;
use nix::sys::nsfs::{namespace_id, namespace_type, NamespaceType};
use nix::sys::socket::{getsockopt, sockopt::NetnsCookie};
use nix::sys::wait::{waitid, Id, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::cell::{Cell, RefCell};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
#[path = "child_protocol.rs"]
mod protocol;
#[path = "child_executable.rs"]
mod child_executable;
#[path = "retained_return.rs"]
mod retained_return;
use retained_return::retain_after;
use child_executable::{Executable, CHILD};

const CHILD_ARG: &str = "--fixed-owned-child-no-policy";
const ERROR: EffectError = EffectError::UnavailableOrUncertain;

fn require(value: bool) -> Result<(), EffectError> {
    if value { Ok(()) } else { Err(ERROR) }
}
fn identity(s: &std::fs::Metadata) -> [u64; 11] {
    [s.dev(),s.ino(),u64::from(s.mode()),u64::from(s.uid()),u64::from(s.gid()),
     s.nlink(),s.size(),s.mtime() as u64,s.mtime_nsec() as u64,
     s.ctime() as u64,s.ctime_nsec() as u64]
}

/// Shared ONLY by the exact acquired creator and its actual session checks.
/// No supplied child, supplied descriptor, PID lookup adoption or replacement API.
pub(crate) struct LaunchLife {
    child: ManuallyDrop<Child>,
    executable: Executable,
    input: ManuallyDrop<RefCell<ChildStdin>>,
    output: ManuallyDrop<RefCell<ChildStdout>>,
    anchor: Rc<File>,
    thread_namespace: Rc<File>,
    initial_id: u64,
    thread: ThreadId,
    deadline: Instant,
    checks: Cell<u16>,
    sealed: Cell<bool>,
}

impl LaunchLife {
    pub(crate) fn gate(&self) -> Result<(), EffectError> {
        if self.sealed.get() || self.budget().is_err() {
            self.sealed.set(true);
            return Err(ERROR);
        }
        Ok(())
    }
    pub(crate) fn session_check(&self, namespace: BorrowedFd<'_>, socket: BorrowedFd<'_>) -> Result<(), EffectError> {
        let result = (|| {
            self.gate()?;
            self.sample(namespace)?;
            self.check(socket)
        })();
        if result.is_err() { self.sealed.set(true); }
        result
    }
    fn budget(&self) -> Result<(), EffectError> {
        require(thread::current().id()==self.thread && Instant::now()<self.deadline)
    }
    fn pipe_gate(&self) -> protocol::Result<()> { self.budget().map_err(|_|protocol::Refused) }
    fn leaf<T>(&self, f: impl FnOnce() -> Result<T, EffectError>) -> Result<T, EffectError> {
        self.budget()?;let value=f();self.budget()?;value
    }
    fn sample(&self, file: BorrowedFd<'_>) -> Result<u64, EffectError> {
        require(self.leaf(|| namespace_type(file).map_err(|_|ERROR))?==NamespaceType::Network)?;
        let id=self.leaf(|| namespace_id(file).map_err(|_|ERROR))?;
        require(id!=0 && id==self.initial_id)?;Ok(id)
    }
    /// The socket argument is borrowed directly from LocalReadSession's private
    /// original field, including each existing complete-inventory check.
    pub(crate) fn check(&self, socket: BorrowedFd<'_>) -> Result<(), EffectError> {
        require(!self.sealed.replace(true))?;
        self.budget()?;
        let count=self.checks.get().checked_add(1).ok_or(ERROR)?;
        require(count<=128)?;self.checks.set(count);
        let pid=i32::try_from(self.child.id()).map_err(|_|ERROR)?;
        require(pid>0)?;
        let status=self.leaf(|| waitid(Id::Pid(Pid::from_raw(pid)),
            WaitPidFlag::WEXITED|WaitPidFlag::WNOHANG|WaitPidFlag::WNOWAIT).map_err(|_|ERROR))?;
        require(status==WaitStatus::StillAlive)?;
        protocol::idle(&mut *self.output.borrow_mut(), &mut ||self.pipe_gate()).map_err(|_|ERROR)?;
        self.sample(self.anchor.as_ref().as_fd())?;
        self.sample(self.thread_namespace.as_ref().as_fd())?;
        self.budget()?;
        let current=File::open("/proc/thread-self/ns/net").map(ManuallyDrop::new);
        self.budget()?;let current=current.map_err(|_|ERROR)?;
        self.sample(current.as_fd())?;
        require(self.leaf(|| getsockopt(&socket,NetnsCookie).map_err(|_|ERROR))?==self.initial_id)?;
        // Only the PID from the still-owned unreaped child is used; never PID1
        // or a client-supplied PID. A finished/reused child cannot pass WNOWAIT.
        self.budget()?;
        let image=File::open(format!("/proc/{pid}/exe")).map(ManuallyDrop::new);
        self.budget()?;let image=image.map_err(|_|ERROR)?;
        require(identity(&self.leaf(||image.metadata().map_err(|_|ERROR))?)==self.executable.metadata)?;
        self.executable.recheck(self.deadline)?;
        self.budget()?;self.sealed.set(false);Ok(())
    }
    // Only Prototype::finish reaches here AFTER acquisition and actual creator
    // have been permanently sealed. It cannot re-enable readback or effects.
    fn finish(&self) -> Result<(),EffectError> {
        require(!self.sealed.replace(true))?;
        protocol::write_frame(&mut *self.input.borrow_mut(),protocol::FINISH,&mut ||self.pipe_gate()).map_err(|_|ERROR)?;
        protocol::read_frame(&mut *self.output.borrow_mut(),protocol::DONE,&mut ||self.pipe_gate()).map_err(|_|ERROR)?;
        let pid=Pid::from_raw(i32::try_from(self.child.id()).map_err(|_|ERROR)?);
        loop {
            self.budget()?;
            let status=waitid(Id::Pid(pid),WaitPidFlag::WEXITED|WaitPidFlag::WNOHANG|WaitPidFlag::WNOWAIT);
            self.budget()?;
            match status.map_err(|_|ERROR)? {
                WaitStatus::StillAlive => std::hint::spin_loop(),
                WaitStatus::Exited(actual,0) if actual==pid => break,
                _ => return Err(ERROR),
            }
        }
        protocol::eof(&mut *self.output.borrow_mut(),&mut ||self.pipe_gate()).map_err(|_|ERROR)?;
        self.executable.recheck(self.deadline)?;
        self.budget()?;
        let status=nix::sys::wait::waitpid(pid,Some(WaitPidFlag::WNOHANG));
        self.budget()?;
        require(status.map_err(|_|ERROR)?==WaitStatus::Exited(pid,0))
    }
}

struct Verify(Rc<LaunchLife>);
impl OriginalVerifier for Verify {
    fn recheck(&mut self, originals: LaunchBorrow<'_>) -> Result<(), EffectError> {
        self.0.sample(originals.anchor)?;
        self.0.sample(originals.thread_namespace)?;
        self.0.check(originals.creator_socket)
    }
}

/// Returns only an untrusted observation. No conversion to canonical epoch,
/// receipt, listener authority or mutation port exists for ActualCreator.
pub(crate) struct Prototype {
    acquired: AcquiredCreator<ActualCreator>,
    life: Rc<LaunchLife>,
}

impl Prototype {
    /// Fixed private prototype entry. Root UID is a scope prerequisite, NOT
    /// proof of installed launcher origin. Real invocation needs ROOT review.
    pub(crate) fn open_fixed() -> Result<Self, EffectError> {
        let deadline=Instant::now().checked_add(Duration::from_secs(5)).ok_or(ERROR)?;
        let gate=||require(Instant::now()<deadline);
        gate()?;require(nix::unistd::getresuid().map_err(|_|ERROR)?.effective.is_root())?;gate()?;
        // Unpinned child artifacts refuse before any namespace/socket/child.
        let executable=Executable::admit(deadline)?;gate()?;
        let anchor=File::open("/proc/thread-self/ns/net").map(ManuallyDrop::new);gate()?;
        let anchor=anchor.map_err(|_|ERROR)?;
        let thread_namespace=File::open("/proc/thread-self/ns/net").map(ManuallyDrop::new);gate()?;
        let thread_namespace=thread_namespace.map_err(|_|ERROR)?;
        let initial_id=namespace_id(anchor.as_fd()).map_err(|_|ERROR)?;gate()?;
        require(initial_id!=0)?;gate()?;
        let kind=namespace_type(anchor.as_fd());gate()?;
        require(kind.map_err(|_|ERROR)?==NamespaceType::Network)?;
        // The socket is created BEFORE the exact child. There is no second
        // matching socket in Originals, verifier or the child process.
        let creator=open_actual(deadline).map(ManuallyDrop::new);gate()?;
        let mut creator=creator.map_err(|_|ERROR)?;
        executable.recheck(deadline)?;gate()?;
        // Child stdin stays owned and open. The fixed child performs no policy
        // operation; no borrowed netlink FD is transferred across exec.
        let child=retain_after(Command::new(executable.exec_path()).arg0(CHILD).arg(CHILD_ARG).env_clear()
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn(),||gate().is_ok());
        let mut child=child.map_err(|_|ERROR)?;
        let input=ManuallyDrop::new(child.stdin.take().ok_or(ERROR)?);
        let output=ManuallyDrop::new(child.stdout.take().ok_or(ERROR)?);
        for fd in [input.as_fd(),output.as_fd()] {
            gate()?;let flags=nix::fcntl::fcntl(fd,nix::fcntl::FcntlArg::F_GETFL);gate()?;
            let flags=nix::fcntl::OFlag::from_bits(flags.map_err(|_|ERROR)?).ok_or(ERROR)?;
            gate()?;let result=nix::fcntl::fcntl(fd,nix::fcntl::FcntlArg::F_SETFL(flags|nix::fcntl::OFlag::O_NONBLOCK));gate()?;
            result.map_err(|_|ERROR)?;
        }
        let mut output=output;
        protocol::read_frame(&mut *output,protocol::READY,&mut ||gate().map_err(|_|protocol::Refused)).map_err(|_|ERROR)?;
        // No image assertion is made before READY from the original pipe.
        let anchor=Rc::new(ManuallyDrop::into_inner(anchor));
        let thread_namespace=Rc::new(ManuallyDrop::into_inner(thread_namespace));
        let life=ManuallyDrop::new(Rc::new(LaunchLife {child,executable,
            input:ManuallyDrop::new(RefCell::new(ManuallyDrop::into_inner(input))),
            output:ManuallyDrop::new(RefCell::new(ManuallyDrop::into_inner(output))),
            anchor:anchor.clone(),thread_namespace:thread_namespace.clone(),initial_id,
            thread:thread::current().id(),deadline,checks:Cell::new(0),sealed:Cell::new(false)}));
        creator.attach_launch(Rc::clone(&life)).map_err(|_|ERROR)?;gate()?;
        let mut acquired=AcquiredCreator {retained:ManuallyDrop::new(Retained {
            originals:Originals {anchor,thread_namespace,owner_thread:thread::current().id(),
                verifier:Box::new(Verify(Rc::clone(&life)))},creator:ManuallyDrop::into_inner(creator)}),
            sealed:false,_same_thread:PhantomData};
        acquired.with_lease(|_|Ok(()))?;
        Ok(Self {acquired,life:Rc::clone(&life)})
    }
    pub(crate) fn inventory(&mut self) -> Result<LocalPolicyInventory,EffectError> {
        self.acquired.with_lease(|creator|creator.inventory().map_err(|_|ERROR))
    }
    pub(crate) fn finish(mut self) -> Result<(),EffectError> {
        self.acquired.with_lease(|_|Ok(()))?;
        self.acquired.sealed=true;
        self.acquired.retained.creator.seal();
        self.life.finish()
    }
}
