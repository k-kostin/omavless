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
    /// Fixed real opener; no supplied socket/namespace/epoch or alternate sender.
    /// Every successfully returned original is retained before late classification.
    pub(super) fn open_before(deadline: Instant) -> Result<Self> {
        require(Instant::now() < deadline)?;
        let namespace = File::open("/proc/thread-self/ns/net")
            .map(ManuallyDrop::new).map_err(|_| REFUSE)?;
        require(Instant::now() < deadline)?;
        let identity = namespace_identity(&namespace);
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
        owner.session.check(deadline)?;
        Ok(owner)
    }

    pub(crate) fn attach_launch(&mut self, life: Rc<crate::launch_acquisition::owned_launcher::LaunchLife>) -> Result<()> {
        require(self.session.launch.is_none() && !self.sealed && Instant::now() < self.deadline)?;
        self.session.launch = Some(life);
        self.session.check(self.deadline)
    }

    /// Existing complete table/chain/rule/set/object/flowtable readback, unchanged.
    /// No result from this method is Canonical, OwnedVerified or an EffectPort.
    pub(crate) fn inventory(&mut self) -> Result<LocalPolicyInventory> {
        require(!self.sealed)?;
        self.sealed = true;
        require(thread::current().id() == self.thread && Instant::now() < self.deadline)?;
        let result = self.session.inspect_policy_inventory_before(self.deadline);
        if result.is_err() { self.session.poisoned = true; }
        let (inventory, _, _) = result?;
        require(Instant::now() < self.deadline)?;
        self.sealed = false;
        Ok(inventory)
    }
}

/// Available only to the sibling acquisition extension inside the same crate.
pub(crate) fn open_actual(deadline: Instant) -> Result<ActualCreator> {
    ActualCreator::open_before(deadline)
}
