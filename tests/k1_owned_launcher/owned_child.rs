//! Fixed safe-library spawn, with every pipe owned before the next leaf.
//! No std Command fallback, caller PID, arbitrary argv, descriptor or child API.
use super::{protocol, require, retain_after, EffectError, ERROR};
use nix::fcntl::OFlag;
use nix::spawn::{posix_spawn,PosixSpawnAttr,PosixSpawnFileActions};
use nix::unistd::{pipe2,Pid};
use std::cell::{Cell,RefCell};
use std::ffi::{CStr,CString};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::marker::PhantomData;
use std::rc::Rc;
use std::os::fd::AsRawFd;
use std::time::Instant;
use super::child_executable::{Executable,CHILD};
type Result<T>=std::result::Result<T,EffectError>;
fn gate(end:Instant)->Result<()> {require(Instant::now()<end)}

pub(super) struct OwnedChild {
    pid: Pid,
    pub(super) input: ManuallyDrop<RefCell<File>>,
    pub(super) output: ManuallyDrop<RefCell<File>>,
    child_ends: ManuallyDrop<RefCell<[Option<File>;2]>>,
    ready_attempted: Cell<bool>,
    released: Cell<bool>,
    _same_thread: PhantomData<Rc<()>>,
}
impl OwnedChild {
    pub(super) fn spawn_fixed(executable:&Executable,end:Instant)->Result<Self> {
        gate(end)?;
        let first=retain_after(pipe2(OFlag::O_CLOEXEC|OFlag::O_NONBLOCK)
            .map(|(r,w)|(File::from(r),File::from(w))),||gate(end).is_ok()).map_err(|_|ERROR)?;
        let (child_read,parent_write)=ManuallyDrop::into_inner(first);
        let child_read=ManuallyDrop::new(child_read);
        let parent_write=ManuallyDrop::new(parent_write);
        gate(end)?;
        let second=retain_after(pipe2(OFlag::O_CLOEXEC|OFlag::O_NONBLOCK)
            .map(|(r,w)|(File::from(r),File::from(w))),||gate(end).is_ok()).map_err(|_|ERROR)?;
        let (parent_read,child_write)=ManuallyDrop::into_inner(second);
        let parent_read=ManuallyDrop::new(parent_read);
        let child_write=ManuallyDrop::new(child_write);
        // Do not allow any source descriptor to alias the stdio destinations.
        for fd in [child_read.as_raw_fd(),parent_write.as_raw_fd(),parent_read.as_raw_fd(),child_write.as_raw_fd()] {
            require(fd>=3)?;
        }
        gate(end)?;
        let attr=retain_after(PosixSpawnAttr::init(),||gate(end).is_ok()).map_err(|_|ERROR)?;
        gate(end)?;
        let actions=retain_after(PosixSpawnFileActions::init(),||gate(end).is_ok()).map_err(|_|ERROR)?;
        let mut actions=actions;
        // Child stderr shares the bounded PRIVATE protocol pipe; no extra path
        // or stream is opened. Any unexpected diagnostic bytes refuse parsing.
        for (source,target) in [(child_read.as_raw_fd(),0),(child_write.as_raw_fd(),1),(child_write.as_raw_fd(),2)] {
            gate(end)?;let result=actions.add_dup2(source,target);gate(end)?;result.map_err(|_|ERROR)?;
        }
        let program=CString::new(executable.exec_path()).map_err(|_|ERROR)?;
        let argv0=CString::new(CHILD).map_err(|_|ERROR)?;
        let argument=CString::new("--fixed-owned-child-no-policy").map_err(|_|ERROR)?;
        let environment:[&CStr;0]=[];
        executable.recheck(end)?;gate(end)?;
        // This single safe-library call returns only its actual newly spawned
        // PID. libc internals are synchronous/noncancellable, not leaf-fenced.
        let pid=retain_after(posix_spawn(program.as_c_str(),&actions,&attr,
            &[argv0.as_c_str(),argument.as_c_str()],&environment),||gate(end).is_ok()).map_err(|_|ERROR)?;
        require(pid.as_raw()>1)?;
        Ok(Self {pid:*pid,input:ManuallyDrop::new(RefCell::new(ManuallyDrop::into_inner(parent_write))),
            output:ManuallyDrop::new(RefCell::new(ManuallyDrop::into_inner(parent_read))),
            child_ends:ManuallyDrop::new(RefCell::new([Some(ManuallyDrop::into_inner(child_read)),Some(ManuallyDrop::into_inner(child_write))])),
            ready_attempted:Cell::new(false),released:Cell::new(false),_same_thread:PhantomData})
    }
    pub(super) fn pid(&self)->Pid {self.pid}
    pub(super) fn ready(&self,end:Instant)->Result<()> {
        require(!self.ready_attempted.replace(true))?;
        protocol::read_frame(&mut *self.output.borrow_mut(),protocol::READY,
            &mut ||gate(end).map_err(|_|protocol::Refused)).map_err(|_|ERROR)
    }
    // Called once only AFTER READY and full original live-image/acquisition
    // verification. Release only the parent's copies of the child's pipe ends;
    // otherwise a retained parent writer would make genuine output EOF impossible.
    // The child and parent's actual I/O originals remain held on any late close.
    pub(super) fn complete_handoff(&self,end:Instant)->Result<()> {
        require(self.ready_attempted.get() && !self.released.replace(true))?;
        for index in 0..2 {
            gate(end)?;
            let original=self.child_ends.borrow_mut()[index].take().ok_or(ERROR)?;
            drop(original);
            gate(end)?;
        }
        Ok(())
    }
}
