//! Fixed safe-library spawn, with every pipe owned before the next leaf.
//! No std Command fallback, caller PID, arbitrary argv, descriptor or child API.
use super::{protocol, require, EffectError, ERROR};
use nix::fcntl::OFlag;
use nix::spawn::{posix_spawn,PosixSpawnAttr,PosixSpawnFileActions};
use nix::unistd::{pipe2,Pid};
use std::cell::RefCell;
use std::ffi::{CStr,CString};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::marker::PhantomData;
use std::rc::Rc;
use std::os::fd::AsRawFd;
use std::time::Instant;
use super::child_executable::{Executable,CHILD};
#[path = "handoff.rs"]
mod handoff;
#[path = "spawn_sequence.rs"]
mod sequence;
type Result<T>=std::result::Result<T,EffectError>;
fn gate(end:Instant)->Result<()> {require(Instant::now()<end)}

struct SpawnBackend<'a> {executable:&'a Executable,end:Instant,program:CString,argv0:CString,argument:CString}
impl sequence::Backend for SpawnBackend<'_> {
    type File=File;type Attr=PosixSpawnAttr;type Actions=PosixSpawnFileActions;type Pid=Pid;
    fn gate(&mut self)->std::result::Result<(),sequence::Refused> {gate(self.end).map_err(|_|sequence::Refused)}
    fn pipe(&mut self)->std::result::Result<(File,File),sequence::Refused> {
        pipe2(OFlag::O_CLOEXEC|OFlag::O_NONBLOCK).map(|(r,w)|(File::from(r),File::from(w))).map_err(|_|sequence::Refused)
    }
    fn descriptor(file:&File)->i32 {file.as_raw_fd()}
    fn attr(&mut self)->std::result::Result<PosixSpawnAttr,sequence::Refused> {PosixSpawnAttr::init().map_err(|_|sequence::Refused)}
    fn actions(&mut self)->std::result::Result<PosixSpawnFileActions,sequence::Refused> {PosixSpawnFileActions::init().map_err(|_|sequence::Refused)}
    fn dup(&mut self,actions:&mut PosixSpawnFileActions,source:i32,target:i32)->std::result::Result<(),sequence::Refused> {
        actions.add_dup2(source,target).map_err(|_|sequence::Refused)
    }
    fn recheck(&mut self)->std::result::Result<(),sequence::Refused> {self.executable.recheck(self.end).map_err(|_|sequence::Refused)}
    fn spawn(&mut self,actions:&PosixSpawnFileActions,attr:&PosixSpawnAttr)->std::result::Result<Pid,sequence::Refused> {
        let environment:[&CStr;0]=[];
        // One safe call only: no std Command fallback or hidden application wait.
        // Synchronous libc internals are not cancellable or per-leaf controlled.
        posix_spawn(self.program.as_c_str(),actions,attr,
            &[self.argv0.as_c_str(),self.argument.as_c_str()],&environment).map_err(|_|sequence::Refused)
    }
    fn valid_pid(pid:Pid)->bool {pid.as_raw()>1}
}

pub(super) struct OwnedChild {
    pid: Pid,
    pub(super) input: ManuallyDrop<RefCell<File>>,
    pub(super) output: ManuallyDrop<RefCell<File>>,
    child_ends: handoff::Handoff<File>,
    ready: protocol::ReadyState,
    _same_thread: PhantomData<Rc<()>>,
}
impl OwnedChild {
    pub(super) fn spawn_fixed(executable:&Executable,end:Instant)->Result<Self> {
        gate(end)?;
        let program=CString::new(executable.exec_path()).map_err(|_|ERROR)?;
        let argv0=CString::new(CHILD).map_err(|_|ERROR)?;
        let argument=CString::new("--fixed-owned-child-no-policy").map_err(|_|ERROR)?;
        let sequence::Spawned{pid,child_read,parent_write,parent_read,child_write}=
            sequence::construct(&mut SpawnBackend{executable,end,program,argv0,argument}).map_err(|_|ERROR)?;
        Ok(Self {pid,input:ManuallyDrop::new(RefCell::new(ManuallyDrop::into_inner(parent_write))),
            output:ManuallyDrop::new(RefCell::new(ManuallyDrop::into_inner(parent_read))),
            child_ends:handoff::Handoff::new([ManuallyDrop::into_inner(child_read),ManuallyDrop::into_inner(child_write)]),
            ready:protocol::ReadyState::new(),_same_thread:PhantomData})
    }
    pub(super) fn pid(&self)->Pid {self.pid}
    pub(super) fn ready(&self,end:Instant)->Result<()> {
        self.ready.receive(&mut *self.output.borrow_mut(),
            &mut ||gate(end).map_err(|_|protocol::Refused)).map_err(|_|ERROR)
    }
    // Called once only AFTER READY and full original live-image/acquisition
    // verification. Release only the parent's copies of the child's pipe ends;
    // otherwise a retained parent writer would make genuine output EOF impossible.
    // The child and parent's actual I/O originals remain held on any late close.
    pub(super) fn complete_handoff(&self,end:Instant)->Result<()> {
        self.child_ends.release(self.ready.complete(),
            ||gate(end).map_err(|_|handoff::Refused),
            nix::unistd::close)
            .map_err(|_|ERROR)
    }
}
