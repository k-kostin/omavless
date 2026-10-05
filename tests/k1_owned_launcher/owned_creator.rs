//! Included only in the exported kernel_observer module.
//! The actual shared LocalReadSession is the sole socket owner. No send here.
use super::*;
use crate::launch_acquisition::CreatorOwner;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::os::fd::{AsFd, BorrowedFd};
use std::rc::Rc;
use std::thread::{self, ThreadId};

pub(crate) struct ActualCreator {
    session: ManuallyDrop<LocalReadSession>,
    deadline: Instant,
    thread: ThreadId,
    sealed: bool,
    _same_thread: PhantomData<Rc<()>>,
}

impl CreatorOwner for ActualCreator {
    fn creator_socket(&self) -> BorrowedFd<'_> {
        self.session.socket.as_fd()
    }
}

impl ActualCreator {
    pub(crate) fn seal(&mut self) { self.sealed=true; self.session.poisoned=true; }
    // Before LaunchLife exists, preserve the shared observer's exact local
    // identity predicates but fence EACH original leaf, not the whole helper.
    fn local_identity(file: &File, deadline: Instant) -> Result<(u64, u64)> {
        require(Instant::now() < deadline)?;
        let filesystem = fstatfs(file);
        require(Instant::now() < deadline)?;
        require(filesystem.map_err(|_| REFUSE)?.filesystem_type() == NSFS_MAGIC)?;
        require(Instant::now() < deadline)?;
        let metadata = file.metadata();
        require(Instant::now() < deadline)?;
        let metadata = metadata.map_err(|_| REFUSE)?;
        require(Instant::now() < deadline)?;
        let label = std::fs::read_link(format!("/proc/thread-self/fd/{}", file.as_raw_fd()));
        require(Instant::now() < deadline)?;
        let label = label.map_err(|_| REFUSE)?;
        require(metadata.ino() != 0 && label.to_str() == Some(&format!("net:[{}]", metadata.ino())))?;
        Ok((metadata.dev(), metadata.ino()))
    }

    /// Fixed real opener; no supplied socket/namespace/epoch or alternate sender.
    /// Every successfully returned original is retained before late classification.
    pub(super) fn open_before(deadline: Instant) -> Result<Self> {
        require(Instant::now() < deadline)?;
        let namespace = File::open("/proc/thread-self/ns/net")
            .map(ManuallyDrop::new).map_err(|_| REFUSE)?;
        require(Instant::now() < deadline)?;
        let identity = Self::local_identity(&namespace, deadline);
        require(Instant::now() < deadline)?;
        let identity = identity?;
        let socket = socket(AddressFamily::Netlink, SockType::Raw,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            SockProtocol::NetlinkNetFilter).map(ManuallyDrop::new).map_err(|_| REFUSE)?;
        require(Instant::now() < deadline)?;
        let result = bind(socket.as_raw_fd(), &NetlinkAddr::new(0, 0));
        require(Instant::now() < deadline)?;
        result.map_err(|_| REFUSE)?;
        let local = getsockname::<NetlinkAddr>(socket.as_raw_fd());
        require(Instant::now() < deadline)?;
        let local = local.map_err(|_| REFUSE)?;
        require(local.pid() != 0 && local.groups() == 0)?;
        let session = LocalReadSession {
            namespace: ManuallyDrop::into_inner(namespace), identity,
            socket: ManuallyDrop::into_inner(socket), local,
            next_sequence: 1, poisoned: false, last_generation: None,
            launch: None,
        };
        let owner = Self { session: ManuallyDrop::new(session), deadline,
            thread: thread::current().id(), sealed: false, _same_thread: PhantomData };
        // The historical session.check nests namespace_file/identity calls.
        // Keep their predicates, but retain new originals before every late
        // classification and do not enter another leaf after expiration.
        require(Self::local_identity(&owner.session.namespace, deadline)? == identity)?;
        require(Instant::now() < deadline)?;
        let proc_ns = File::open("/proc/thread-self/ns").map(ManuallyDrop::new);
        require(Instant::now() < deadline)?;
        let proc_ns = proc_ns.map_err(|_| REFUSE)?;
        let filesystem = fstatfs(proc_ns.as_fd());
        require(Instant::now() < deadline)?;
        require(filesystem.map_err(|_| REFUSE)?.filesystem_type() == PROC_SUPER_MAGIC)?;
        require(Instant::now() < deadline)?;
        let current = File::open("/proc/thread-self/ns/net").map(ManuallyDrop::new);
        require(Instant::now() < deadline)?;
        let current = current.map_err(|_| REFUSE)?;
        require(Self::local_identity(&current, deadline)? == identity)?;
        require(Instant::now() < deadline)?;
        let actual = getsockname::<NetlinkAddr>(owner.session.socket.as_raw_fd());
        require(Instant::now() < deadline)?;
        require(actual.map_err(|_| REFUSE)? == owner.session.local)?;
        Ok(owner)
    }

    pub(crate) fn attach_launch(&mut self, life: Rc<crate::launch_acquisition::owned_launcher::LaunchLife>) -> Result<()> {
        require(self.session.launch.is_none() && !self.sealed && Instant::now() < self.deadline)?;
        self.session.launch = Some(life);
        self.session.check(self.deadline)
    }

    /// Reuse the actual #641 exclusive lease, including table/generation and
    /// original deadline. It remains alive through acquisition post-verification.
    pub(crate) fn borrow_inventory(&mut self) -> Result<ActualInventory<'_>> {
        require(!self.sealed)?;
        self.sealed = true;
        require(thread::current().id() == self.thread && Instant::now() < self.deadline)?;
        let lease = self.session.borrow_policy_inventory_before(self.deadline)?;
        Ok(ActualInventory { lease, creator_sealed: &mut self.sealed, completed: false })
    }
}

/// No constructor, mutable session accessor, clone, transfer or effect port.
/// An unfinished scope poisons the original session without any I/O or close.
pub(crate) struct ActualInventory<'a> {
    lease: LocalInventoryLease<'a>,
    creator_sealed: &'a mut bool,
    completed: bool,
}
impl ActualInventory<'_> {
    pub(crate) fn recheck(&mut self) -> Result<()> { self.lease.recheck() }
    pub(crate) fn socket(&self) -> BorrowedFd<'_> { self.lease.session.socket.as_fd() }
    pub(crate) fn complete(mut self) -> LocalPolicyInventory {
        let observed = self.lease.observed();
        self.completed = true;
        *self.creator_sealed = false;
        observed
    }
}
impl Drop for ActualInventory<'_> {
    fn drop(&mut self) {
        if !self.completed { self.lease.session.poisoned = true; }
    }
}

/// Available only to the sibling acquisition extension inside the same crate.
pub(crate) fn open_actual(deadline: Instant) -> Result<ActualCreator> {
    ActualCreator::open_before(deadline)
}
