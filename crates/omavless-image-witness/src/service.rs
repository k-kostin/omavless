// SPDX-License-Identifier: MIT
//! A single development epoch/channel, not production service activation.
use crate::{
    Error, Result,
    channel::Endpoint,
    class::Class,
    kernel,
    protocol::{self, Kind, Sequence},
};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{self, OFlags, XattrFlags},
    net::{self, AddressFamily, SocketAddrUnix, SocketFlags, SocketType},
};
use std::{
    fs::File,
    os::{
        fd::{AsFd, OwnedFd},
        unix::fs::{FileTypeExt, MetadataExt},
    },
    time::{Duration, Instant},
};

const REQUEST_BUDGET: Duration = Duration::from_secs(2);
const IDLE_BUDGET: Duration = Duration::from_secs(30);
fn until(caller: Instant) -> Instant {
    caller.min(Instant::now() + REQUEST_BUDGET)
}
fn protected(path: &str) -> Result<()> {
    let m = std::fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if !m.is_dir() || m.uid() != 0 || m.mode() & 0o7022 != 0 {
        return Err(Error::Refused);
    }
    Ok(())
}
fn node(class: Class, mode: u32) -> Result<(u64, u64)> {
    for p in ["/", "/run", class.directory()] {
        protected(p)?;
    }
    let m = std::fs::symlink_metadata(class.socket()).map_err(|_| Error::Unavailable)?;
    if !m.file_type().is_socket() || m.uid() != 0 || m.nlink() != 1 || m.mode() & 0o7777 != mode {
        return Err(Error::Refused);
    }
    Ok((m.dev(), m.ino()))
}
fn acl(uid: u32) -> [u8; 44] {
    let mut output = [0; 44];
    output[..4].copy_from_slice(&2_u32.to_le_bytes());
    for (n, (tag, perm, id)) in [
        (1_u16, 6_u16, u32::MAX),
        (2, 6, uid),
        (4, 0, u32::MAX),
        (16, 6, u32::MAX),
        (32, 0, u32::MAX),
    ]
    .into_iter()
    .enumerate()
    {
        let i = 4 + n * 8;
        output[i..i + 2].copy_from_slice(&tag.to_le_bytes());
        output[i + 2..i + 4].copy_from_slice(&perm.to_le_bytes());
        output[i + 4..i + 8].copy_from_slice(&id.to_le_bytes());
    }
    output
}
fn access(class: Class, uid: u32, id: (u64, u64)) -> Result<()> {
    if uid == 0 || uid == u32::MAX || node(class, 0o660)? != id {
        return Err(Error::Refused);
    }
    let mut actual = [0; 44];
    let n = fs::lgetxattr(class.socket(), "system.posix_acl_access", &mut actual[..])
        .map_err(|_| Error::Unavailable)?;
    if n != 44 || actual != acl(uid) || node(class, 0o660)? != id {
        return Err(Error::Refused);
    }
    Ok(())
}
fn privilege() -> Result<()> {
    if rustix::process::getuid().as_raw() != 0 || rustix::process::geteuid().as_raw() != 0 {
        return Err(Error::Refused);
    }
    use std::io::Read;
    let mut raw = Vec::new();
    File::open("/proc/self/status")
        .map_err(|_| Error::Unavailable)?
        .take((kernel_status_limit() + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| Error::Unavailable)?;
    if raw.len() > kernel_status_limit() {
        return Err(Error::Refused);
    }
    let text = std::str::from_utf8(&raw).map_err(|_| Error::Refused)?;
    for field in ["CapPrm:", "CapEff:"] {
        let values: Vec<_> = text.lines().filter_map(|s| s.strip_prefix(field)).collect();
        if values.len() != 1 || values[0].trim() != "0000000000080000" {
            return Err(Error::Refused);
        }
    }
    Ok(())
}
const fn kernel_status_limit() -> usize {
    16384
}
fn listener_wait(fd: &OwnedFd, until: Instant) -> Result<()> {
    let duration = until
        .checked_duration_since(Instant::now())
        .ok_or(Error::Expired)?;
    let mut row = [PollFd::new(fd, PollFlags::IN)];
    let count = poll(
        &mut row,
        Some(&Timespec {
            tv_sec: duration.as_secs() as i64,
            tv_nsec: duration.subsec_nanos() as i64,
        }),
    )
    .map_err(|_| Error::Unavailable)?;
    kernel::tick(until)?;
    if count == 0 || !row[0].revents().intersects(PollFlags::IN) {
        return Err(Error::Expired);
    }
    Ok(())
}
/// Explicit root-selected development service only. Does not mkdir, enroll,
/// install a unit, gain caps, remove a socket or restart after any refusal.
pub fn serve_development() -> Result<()> {
    serve(Class::Tests)
}
/// Separate fixed installed-runtime development class. No enrollment/install,
/// unit activation, arbitrary client path or product effect permission.
pub fn serve_development_runtime() -> Result<()> {
    serve(Class::InstalledRuntime)
}
/// Default-off product-class provider. It does not enroll, install or remove
/// anything. Only the fixed held root record chooses its one nonroot UID.
#[cfg(feature = "product-epochs")]
pub fn serve_product_epochs() -> Result<()> {
    serve(Class::Product)
}

#[cfg(feature = "product-epochs")]
const MAX_EPOCHS: usize = 128;
#[cfg(feature = "product-epochs")]
struct EpochHistory {
    // Never evict prior positive terminal sequences to admit another channel.
    finished: [Option<u32>; MAX_EPOCHS],
    count: usize,
    active: bool,
    poisoned: bool,
}
#[cfg(feature = "product-epochs")]
impl EpochHistory {
    fn new() -> Self {
        Self {
            finished: [None; MAX_EPOCHS],
            count: 0,
            active: false,
            poisoned: false,
        }
    }
    fn idle(&self) -> Result<()> {
        if self.poisoned || self.active || self.count == MAX_EPOCHS {
            Err(Error::Refused)
        } else {
            Ok(())
        }
    }
    fn reserve(&mut self) -> Result<()> {
        self.idle()?;
        self.active = true;
        Ok(())
    }
    fn complete(&mut self, terminal: Result<u32>) -> Result<()> {
        // Consume before inspecting a returned result. Every accepted-context
        // error is sticky, including a late send/metadata/expiry failure.
        let active = self.active;
        self.active = false;
        self.poisoned = true;
        let sequence = terminal?;
        if !active || self.count == MAX_EPOCHS || sequence == 0 {
            return Err(Error::Refused);
        }
        self.finished[self.count] = Some(sequence);
        self.count += 1;
        self.poisoned = false;
        Ok(())
    }
}

#[cfg(feature = "product-epochs")]
fn product_idle(
    fd: &OwnedFd,
    roots: &kernel::Roots,
    class: Class,
    id: (u64, u64),
    history: &EpochHistory,
) -> Result<()> {
    loop {
        // No Binding, Session, capability or request exists during these
        // bounded polls. Root/source drift refuses rather than recapturing.
        history.idle()?;
        let deadline = Instant::now() + REQUEST_BUDGET;
        roots.check(deadline)?;
        access(class, roots.uid, id)?;
        let mut row = [PollFd::new(fd, PollFlags::IN)];
        let count = poll(
            &mut row,
            Some(&Timespec {
                tv_sec: 1,
                tv_nsec: 0,
            }),
        )
        .map_err(|_| Error::Unavailable)?;
        roots.check(deadline)?;
        access(class, roots.uid, id)?;
        kernel::tick(deadline)?;
        if idle_ready(history, count, row[0].revents())? {
            return Ok(());
        }
    }
}
#[cfg(feature = "product-epochs")]
fn idle_ready(history: &EpochHistory, count: usize, flags: PollFlags) -> Result<bool> {
    history.idle()?;
    if count == 0 && flags.is_empty() {
        Ok(false)
    } else if count == 1 && flags == PollFlags::IN {
        Ok(true)
    } else {
        Err(Error::Refused)
    }
}
fn serve(class: Class) -> Result<()> {
    privilege()?;
    // Read-only observed resources may be dropped on error; no effect/Bundle
    // custody or hidden-library allocation guarantee is being asserted.
    rustix::process::setrlimit(
        rustix::process::Resource::Nofile,
        rustix::process::Rlimit {
            current: Some(64),
            maximum: Some(64),
        },
    )
    .map_err(|_| Error::Unavailable)?;
    let roots = kernel::Roots::capture(class, Instant::now() + Duration::from_secs(5))?;
    for p in ["/", "/run", class.directory()] {
        protected(p)?;
    }
    match std::fs::symlink_metadata(class.socket()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        _ => return Err(Error::Refused),
    }
    let fd = net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .map_err(|_| Error::Unavailable)?;
    let address = SocketAddrUnix::new(class.socket()).map_err(|_| Error::Refused)?;
    // Create with owner-only initial access, then exact named-UID ACL under
    // root-only writable ancestors (socket descriptor itself is sockfs).
    rustix::process::umask(fs::Mode::RWXG | fs::Mode::RWXO);
    net::bind(&fd, &address).map_err(|_| Error::Unavailable)?;
    let id = node(class, 0o700)?;
    std::fs::set_permissions(
        class.socket(),
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .map_err(|_| Error::Unavailable)?;
    if node(class, 0o600)? != id {
        return Err(Error::Refused);
    }
    fs::lsetxattr(
        class.socket(),
        "system.posix_acl_access",
        &acl(roots.uid),
        XattrFlags::empty(),
    )
    .map_err(|_| Error::Unavailable)?;
    access(class, roots.uid, id)?;
    net::listen(&fd, 1).map_err(|_| Error::Unavailable)?;
    eprintln!("image_witness_listener_ready");
    #[cfg(feature = "product-epochs")]
    if class.product() {
        let mut history = EpochHistory::new();
        loop {
            product_idle(&fd, &roots, class, id, &history)?;
            // Capacity consumed before accept/peer pidfd/child acquisition.
            history.reserve()?;
            // All original per-channel descriptors are dropped at this
            // function's return, BEFORE completion makes admission idle.
            let terminal = run_session(&fd, &roots, class, id);
            history.complete(terminal)?;
        }
    }
    run_session(&fd, &roots, class, id).map(|_| ())
}

fn run_session(fd: &OwnedFd, roots: &kernel::Roots, class: Class, id: (u64, u64)) -> Result<u32> {
    listener_wait(fd, Instant::now() + IDLE_BUDGET)?;
    let accepted = net::accept_with(fd, SocketFlags::CLOEXEC | SocketFlags::NONBLOCK)
        .map_err(|_| Error::Unavailable)?;
    access(class, roots.uid, id)?;
    let deadline = Instant::now() + REQUEST_BUDGET;
    let endpoint = Endpoint::new(accepted, roots.uid, deadline)?;
    let (raw, child) = endpoint.receive(true, deadline)?;
    let (kind, seq) = protocol::decode(raw)?;
    let mut sequence = Sequence::new();
    sequence.consume(kind, seq, child.is_some())?;
    let peer = endpoint.original_peer(deadline)?;
    eprintln!("image_witness_before_child_bind");
    let mut binding =
        match kernel::Binding::bind(roots, peer, child.ok_or(Error::Refused)?, deadline) {
            Ok(binding) => binding,
            Err(error) => {
                eprintln!("image_witness_child_bind_refused");
                return Err(error);
            }
        };
    eprintln!("image_witness_original_binding_verified");
    endpoint.check(deadline)?;
    endpoint.send(&protocol::frame(Kind::Ack, seq), None, deadline)?;
    loop {
        // Idle wait is separately finite. This never renews an in-flight
        // request: all acquisition/send continuations share its absolute2s.
        listener_wait(&endpoint.fd, Instant::now() + IDLE_BUDGET)?;
        let deadline = Instant::now() + REQUEST_BUDGET;
        let (raw, rights) = endpoint.receive(false, deadline)?;
        let (kind, seq) = protocol::decode(raw)?;
        sequence.consume(kind, seq, rights.is_some())?;
        access(class, roots.uid, id)?;
        endpoint.check(deadline)?;
        match kind {
            Kind::Observe => {
                let image = binding.observe(deadline)?;
                endpoint.check(deadline)?;
                endpoint.send(
                    &protocol::frame(Kind::Image, seq),
                    Some(image.as_fd()),
                    deadline,
                )?;
                eprintln!("image_witness_current_image_sent");
            }
            Kind::Finish => {
                if class.product() {
                    // Same original roots/rows/current exe at final positive
                    // retirement; never turn old facts into a new grant.
                    drop(binding.observe(deadline)?);
                    access(class, roots.uid, id)?;
                    endpoint.check(deadline)?;
                }
                endpoint.send(&protocol::frame(Kind::Ack, seq), None, deadline)?;
                if class.product() {
                    kernel::tick(deadline)?;
                }
                eprintln!("image_witness_positive_finished");
                return Ok(seq);
            }
            _ => {
                sequence.poison();
                return Err(Error::Refused);
            }
        }
    }
}

#[cfg(all(test, feature = "product-epochs"))]
mod epoch_tests {
    use super::*;

    #[test]
    fn idle_slices_exist_only_without_an_accepted_context() {
        let mut history = EpochHistory::new();
        for _ in 0..100 {
            assert!(!idle_ready(&history, 0, PollFlags::empty()).unwrap());
        }
        assert!(idle_ready(&history, 1, PollFlags::IN).unwrap());
        history.reserve().unwrap();
        assert!(idle_ready(&history, 0, PollFlags::empty()).is_err());
        assert!(idle_ready(&history, 1, PollFlags::IN).is_err());
        assert!(history.reserve().is_err());
        // An accepted channel uses the original finite idle/request budgets.
        assert_eq!(IDLE_BUDGET, Duration::from_secs(30));
        assert_eq!(REQUEST_BUDGET, Duration::from_secs(2));
        history.complete(Err(Error::Expired)).unwrap_err();
        assert!(history.reserve().is_err());
    }

    #[test]
    fn two_positive_terminals_and_capacity_have_no_eviction_or_parallel_admission() {
        let mut history = EpochHistory::new();
        for n in 0..MAX_EPOCHS {
            history.reserve().unwrap();
            assert!(history.reserve().is_err());
            history.complete(Ok(n as u32 + 1)).unwrap();
            assert_eq!(history.finished[0], Some(1));
            assert_eq!(history.finished[n], Some(n as u32 + 1));
        }
        assert_eq!(history.count, MAX_EPOCHS);
        assert!(history.idle().is_err());
        assert!(history.reserve().is_err());
        assert_eq!(history.finished[0], Some(1));
    }

    #[test]
    fn every_accepted_error_blocks_next_admission_without_repair() {
        for error in [
            Error::Expired,
            Error::Refused,
            Error::Unavailable,
            Error::ChannelLost,
        ] {
            let mut history = EpochHistory::new();
            history.reserve().unwrap();
            assert_eq!(history.complete(Err(error)), Err(error));
            assert!(history.idle().is_err());
            assert!(history.reserve().is_err());
            // Even a late success cannot convert that original failure.
            assert!(history.complete(Ok(1)).is_err());
            assert!(history.reserve().is_err());
        }
    }

    #[test]
    fn invalid_terminal_and_bad_poll_never_create_idle_permission() {
        let mut history = EpochHistory::new();
        assert!(history.complete(Ok(1)).is_err());
        assert!(history.idle().is_err());
        for flags in [
            PollFlags::ERR,
            PollFlags::HUP,
            PollFlags::NVAL,
            PollFlags::IN | PollFlags::HUP,
        ] {
            assert!(idle_ready(&EpochHistory::new(), 1, flags).is_err());
        }
        assert!(idle_ready(&EpochHistory::new(), 2, PollFlags::IN).is_err());
    }
}

/// Noncloneable original channel/child binding. No automatic reconnect or
/// retry; every error permanently poisons this context. FDs are evidence only.
pub struct Client {
    endpoint: Endpoint,
    child: OwnedFd,
    node: (u64, u64),
    next: u32,
    terminal: bool,
    class: Class,
    uid: u32,
}
impl Client {
    pub fn bind_original(child: OwnedFd, caller: Instant) -> Result<Self> {
        Self::bind_class(child, caller, Class::Tests)
    }
    /// Bind only the distinct root-enrolled /usr/bin/omavless development class.
    /// The child is still the original kernel pidfd, never a scalar PID/path.
    pub fn bind_original_runtime(child: OwnedFd, caller: Instant) -> Result<Self> {
        Self::bind_class(child, caller, Class::InstalledRuntime)
    }
    #[cfg(feature = "product-epochs")]
    pub fn bind_original_product(child: OwnedFd, caller: Instant) -> Result<Self> {
        Self::bind_class(child, caller, Class::Product)
    }
    fn bind_class(child: OwnedFd, caller: Instant, class: Class) -> Result<Self> {
        let deadline = until(caller);
        kernel::tick(deadline)?;
        let uid = rustix::process::getuid().as_raw();
        if !class.admits_uid(uid) || rustix::process::geteuid().as_raw() != uid {
            return Err(Error::Refused);
        }
        let id = node(class, 0o660)?;
        access(class, uid, id)?;
        let fd = net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|_| Error::Unavailable)?;
        net::connect(
            &fd,
            &SocketAddrUnix::new(class.socket()).map_err(|_| Error::Refused)?,
        )
        .map_err(|_| Error::ChannelLost)?;
        let endpoint = Endpoint::new(fd, 0, deadline)?;
        endpoint.check(deadline)?;
        endpoint.send(
            &protocol::frame(Kind::Bind, 0),
            Some(child.as_fd()),
            deadline,
        )?;
        let (raw, rights) = endpoint.receive(false, deadline)?;
        if protocol::decode(raw)? != (Kind::Ack, 0) || rights.is_some() {
            return Err(Error::Refused);
        }
        endpoint.check(deadline)?;
        access(class, uid, id)?;
        kernel::tick(deadline)?;
        kernel::alive(&child, deadline)?;
        Ok(Self {
            endpoint,
            child,
            node: id,
            next: 1,
            terminal: false,
            class,
            uid,
        })
    }
    pub fn observe(&mut self, caller: Instant) -> Result<File> {
        if self.terminal {
            return Err(Error::Refused);
        }
        self.terminal = true;
        let deadline = until(caller);
        let result = (|| {
            if self.next > protocol::MAX_OBSERVATIONS {
                return Err(Error::Refused);
            }
            kernel::alive(&self.child, deadline)?;
            access(self.class, self.uid, self.node)?;
            self.endpoint.check(deadline)?;
            self.endpoint
                .send(&protocol::frame(Kind::Observe, self.next), None, deadline)?;
            let (raw, fd) = self.endpoint.receive(true, deadline)?;
            if protocol::decode(raw)? != (Kind::Image, self.next) {
                return Err(Error::Refused);
            }
            let fd = fd.ok_or(Error::Refused)?;
            let flags = fs::fcntl_getfl(&fd).map_err(|_| Error::Unavailable)?;
            if flags.contains(OFlags::PATH) || flags & OFlags::ACCMODE != OFlags::RDONLY {
                return Err(Error::Refused);
            }
            let file = File::from(fd);
            let m = file.metadata().map_err(|_| Error::Unavailable)?;
            if !m.is_file() || m.uid() != 0 || m.nlink() != 1 || m.mode() & 0o7777 != 0o755 {
                return Err(Error::Refused);
            }
            self.endpoint.check(deadline)?;
            access(self.class, self.uid, self.node)?;
            kernel::tick(deadline)?;
            kernel::alive(&self.child, deadline)?;
            self.next = self.next.checked_add(1).ok_or(Error::Refused)?;
            Ok(file)
        })();
        if result.is_ok() {
            self.terminal = false
        }
        result
    }
    /// Terminal read-only session completion, not current-image or effect proof.
    pub fn finish(&mut self, caller: Instant) -> Result<()> {
        if self.terminal {
            return Err(Error::Refused);
        }
        self.terminal = true;
        let deadline = until(caller);
        self.endpoint.check(deadline)?;
        access(self.class, self.uid, self.node)?;
        self.endpoint
            .send(&protocol::frame(Kind::Finish, self.next), None, deadline)?;
        let (raw, fd) = self.endpoint.receive(false, deadline)?;
        if protocol::decode(raw)? != (Kind::Ack, self.next) || fd.is_some() {
            return Err(Error::Refused);
        }
        // Helper may exit after this ACK. No live authority follows from it.
        kernel::tick(deadline)
    }
}
