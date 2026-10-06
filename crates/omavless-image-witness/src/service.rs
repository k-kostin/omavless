// SPDX-License-Identifier: MIT
//! A single development epoch/channel, not production service activation.
use crate::{
    Error, Result,
    channel::Endpoint,
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

const SOCKET: &str = "/run/omavless-image/control.sock";
const DIRECTORY: &str = "/run/omavless-image";
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
fn node(mode: u32) -> Result<(u64, u64)> {
    for p in ["/", "/run", DIRECTORY] {
        protected(p)?;
    }
    let m = std::fs::symlink_metadata(SOCKET).map_err(|_| Error::Unavailable)?;
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
fn access(uid: u32, id: (u64, u64)) -> Result<()> {
    if uid == 0 || uid == u32::MAX || node(0o660)? != id {
        return Err(Error::Refused);
    }
    let mut actual = [0; 44];
    let n = fs::lgetxattr(SOCKET, "system.posix_acl_access", &mut actual[..])
        .map_err(|_| Error::Unavailable)?;
    if n != 44 || actual != acl(uid) || node(0o660)? != id {
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
    let roots = kernel::Roots::capture(Instant::now() + Duration::from_secs(5))?;
    for p in ["/", "/run", DIRECTORY] {
        protected(p)?;
    }
    match std::fs::symlink_metadata(SOCKET) {
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
    let address = SocketAddrUnix::new(SOCKET).map_err(|_| Error::Refused)?;
    // Create with owner-only initial access, then exact named-UID ACL under
    // root-only writable ancestors (socket descriptor itself is sockfs).
    rustix::process::umask(fs::Mode::RWXG | fs::Mode::RWXO);
    net::bind(&fd, &address).map_err(|_| Error::Unavailable)?;
    let id = node(0o700)?;
    std::fs::set_permissions(SOCKET, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .map_err(|_| Error::Unavailable)?;
    if node(0o600)? != id {
        return Err(Error::Refused);
    }
    fs::lsetxattr(
        SOCKET,
        "system.posix_acl_access",
        &acl(roots.uid),
        XattrFlags::empty(),
    )
    .map_err(|_| Error::Unavailable)?;
    access(roots.uid, id)?;
    net::listen(&fd, 1).map_err(|_| Error::Unavailable)?;
    eprintln!("image_witness_listener_ready");
    listener_wait(&fd, Instant::now() + IDLE_BUDGET)?;
    let accepted = net::accept_with(&fd, SocketFlags::CLOEXEC | SocketFlags::NONBLOCK)
        .map_err(|_| Error::Unavailable)?;
    access(roots.uid, id)?;
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
        access(1000, id)?;
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
                endpoint.send(&protocol::frame(Kind::Ack, seq), None, deadline)?;
                eprintln!("image_witness_positive_finished");
                return Ok(());
            }
            _ => {
                sequence.poison();
                return Err(Error::Refused);
            }
        }
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
}
impl Client {
    pub fn bind_original(child: OwnedFd, caller: Instant) -> Result<Self> {
        let deadline = until(caller);
        kernel::tick(deadline)?;
        let uid = rustix::process::getuid().as_raw();
        if uid != 1000 || rustix::process::geteuid().as_raw() != uid {
            return Err(Error::Refused);
        }
        let id = node(0o660)?;
        access(uid, id)?;
        let fd = net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|_| Error::Unavailable)?;
        net::connect(
            &fd,
            &SocketAddrUnix::new(SOCKET).map_err(|_| Error::Refused)?,
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
        access(uid, id)?;
        kernel::tick(deadline)?;
        kernel::alive(&child, deadline)?;
        Ok(Self {
            endpoint,
            child,
            node: id,
            next: 1,
            terminal: false,
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
            access(1000, self.node)?;
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
            access(1000, self.node)?;
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
        access(1000, self.node)?;
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
