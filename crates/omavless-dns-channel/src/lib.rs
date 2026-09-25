// SPDX-License-Identifier: MIT
//! Fixed Linux SOCK_SEQPACKET DNS lease channel; no broker or DNS effects.
//!
//! Acquire carries one OwnedFd for a SEPARATE trusted TUN admission layer.
//! Channel credentials and a received descriptor do not establish TUN ownership.
//! EOF/timeout is failure, never DNS-cleanup success. No production consumers.

#![cfg(target_os = "linux")]

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net::{
        self, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrUnix, SocketFlags,
        SocketType, UCred, sockopt,
    },
};
use std::{
    fmt, fs,
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const FRAME_BYTES: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(10);
const MAGIC: [u8; 4] = *b"OVDN";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Idle,
    InvalidFrame,
    InvalidDescriptor,
    PeerRejected,
    Unavailable,
    Timeout,
    ChannelLost,
    InvalidState,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Idle => "DNS channel is idle; no new request or effect is pending",
            Self::InvalidFrame => "DNS channel frame is invalid",
            Self::InvalidDescriptor => "DNS channel descriptor transfer is invalid",
            Self::PeerRejected => "DNS channel peer is not authorized",
            Self::Unavailable => "DNS channel is unavailable",
            Self::Timeout => "DNS channel deadline expired; outcome is unknown",
            Self::ChannelLost => "DNS channel closed; cleanup is not confirmed",
            Self::InvalidState => "DNS channel transition is invalid",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Response {
    Applying = 1,
    Ready = 2,
    Releasing = 3,
    Released = 4,
    Rejected = 5,
    RecoveryRequired = 6,
}
impl Response {
    fn decode(bytes: [u8; FRAME_BYTES]) -> Result<Self, Error> {
        match decode(bytes, 2)? {
            1 => Ok(Self::Applying),
            2 => Ok(Self::Ready),
            3 => Ok(Self::Releasing),
            4 => Ok(Self::Released),
            5 => Ok(Self::Rejected),
            6 => Ok(Self::RecoveryRequired),
            _ => Err(Error::InvalidFrame),
        }
    }
}

fn frame(kind: u8, code: u8) -> [u8; FRAME_BYTES] {
    [MAGIC[0], MAGIC[1], MAGIC[2], MAGIC[3], 1, kind, code, 0]
}
fn decode(bytes: [u8; FRAME_BYTES], kind: u8) -> Result<u8, Error> {
    if bytes[..4] != MAGIC || bytes[4] != 1 || bytes[5] != kind || bytes[7] != 0 {
        Err(Error::InvalidFrame)
    } else {
        Ok(bytes[6])
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Acquiring,
    Applying,
    Ready,
    ReleaseRequested,
    Releasing,
    Terminal,
}

struct Endpoint {
    fd: OwnedFd,
    peer: UCred,
    timeout: Duration,
}
impl Endpoint {
    fn new(fd: OwnedFd, expected_uid: u32) -> Result<Self, Error> {
        if sockopt::socket_domain(&fd).map_err(|_| Error::Unavailable)? != AddressFamily::UNIX
            || sockopt::socket_type(&fd).map_err(|_| Error::Unavailable)? != SocketType::SEQPACKET
        {
            return Err(Error::PeerRejected);
        }
        let peer = sockopt::socket_peercred(&fd).map_err(|_| Error::PeerRejected)?;
        if peer.uid.as_raw() != expected_uid {
            return Err(Error::PeerRejected);
        }
        sockopt::set_socket_passcred(&fd, true).map_err(|_| Error::Unavailable)?;
        Ok(Self {
            fd,
            peer,
            timeout: TIMEOUT,
        })
    }

    fn send(&self, bytes: &[u8], descriptor: Option<BorrowedFd<'_>>) -> Result<(), Error> {
        let deadline = Instant::now() + self.timeout;
        let fds: Vec<_> = descriptor.into_iter().collect();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(&fds)) {
            return Err(Error::InvalidDescriptor);
        }
        loop {
            wait(&self.fd, PollFlags::OUT, deadline)?;
            match net::sendmsg(
                &self.fd,
                &[IoSlice::new(bytes)],
                &mut control,
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            ) {
                Ok(n) if n == bytes.len() => return Ok(()),
                Ok(_) => return Err(Error::ChannelLost),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(_) => return Err(Error::ChannelLost),
            }
        }
    }

    fn receive(&self, want_fd: bool) -> Result<([u8; FRAME_BYTES], Option<OwnedFd>), Error> {
        let deadline = Instant::now() + self.timeout;
        loop {
            wait(&self.fd, PollFlags::IN, deadline)?;
            // Exact cmsg space matters: rustix skips unknown ancillary kinds.
            // Required kernel credentials + rights fill this capacity; an extra
            // header must cause CTRUNC or displace a required record. No raw FD
            // ownership conversion or unsafe uninitialized-buffer inspection.
            #[repr(align(8))]
            struct Control([MaybeUninit<u8>; rustix::cmsg_space!(ScmRights(1), ScmCredentials(1))]);
            let mut space = Control(
                [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1), ScmCredentials(1))],
            );
            let len = if want_fd {
                space.0.len()
            } else {
                rustix::cmsg_space!(ScmCredentials(1))
            };
            let mut control = RecvAncillaryBuffer::new(&mut space.0[..len]);
            let mut bytes = [0; FRAME_BYTES];
            let result = match net::recvmsg(
                &self.fd,
                &mut [IoSliceMut::new(&mut bytes)],
                &mut control,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(result) => result,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(_) => return Err(Error::ChannelLost),
            };
            // Every delivered FD is owned by control and closes on early return.
            if result.bytes == 0 {
                return Err(Error::ChannelLost);
            }
            if result.bytes != FRAME_BYTES
                || result
                    .flags
                    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            {
                return Err(Error::InvalidFrame);
            }
            let mut descriptor = None;
            let mut credential_seen = false;
            let mut rights_seen = false;
            let mut invalid = None;
            // Always exhaust this drain before returning. rustix 1.1.5 tracks
            // partial drains by unaligned cmsg_len; re-draining after a short
            // read can misalign the next header. Exhaustion avoids that path
            // and closes every surplus descriptor before error publication.
            for message in control.drain() {
                match message {
                    RecvAncillaryMessage::ScmRights(mut rights) if want_fd && !rights_seen => {
                        rights_seen = true;
                        descriptor = rights.next();
                        if descriptor.is_none() || rights.next().is_some() {
                            invalid = Some(Error::InvalidDescriptor);
                        }
                    }
                    RecvAncillaryMessage::ScmCredentials(credential) if !credential_seen => {
                        credential_seen = true;
                        if credential != self.peer {
                            invalid = Some(Error::PeerRejected);
                        }
                    }
                    _ => invalid = Some(Error::InvalidDescriptor),
                }
            }
            if let Some(error) = invalid {
                return Err(error);
            }
            if !credential_seen || rights_seen != want_fd {
                return Err(Error::InvalidDescriptor);
            }
            return Ok((bytes, descriptor));
        }
    }
}

fn wait(fd: &OwnedFd, events: PollFlags, deadline: Instant) -> Result<(), Error> {
    loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::Timeout)?;
        let timeout = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: left.subsec_nanos() as i64,
        };
        let mut fds = [PollFd::new(fd, events)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => return Err(Error::Timeout),
            Ok(_) if fds[0].revents().intersects(events) => return Ok(()),
            Ok(_) => return Err(Error::ChannelLost),
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Error::Unavailable),
        }
    }
}

/// Root-broker client. A root peer is required before any descriptor is sent.
pub struct Client {
    channel: Endpoint,
    state: State,
}
impl Client {
    pub fn connect(path: &Path) -> Result<Self, Error> {
        Self::connect_peer(path, 0)
    }

    fn connect_peer(path: &Path, uid: u32) -> Result<Self, Error> {
        if !path.is_absolute() {
            return Err(Error::Unavailable);
        }
        let fd = net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|_| Error::Unavailable)?;
        sockopt::set_socket_passcred(&fd, true).map_err(|_| Error::Unavailable)?;
        let address = SocketAddrUnix::new(path).map_err(|_| Error::Unavailable)?;
        // Linux local nonblocking connect either finishes or refuses; never
        // retry an ambiguous connection attempt or inherit an ambient socket.
        net::connect(&fd, &address).map_err(|_| Error::Unavailable)?;
        Ok(Self {
            channel: Endpoint::new(fd, uid)?,
            state: State::Idle,
        })
    }

    pub fn acquire(&mut self, proof: BorrowedFd<'_>) -> Result<(), Error> {
        if self.state != State::Idle {
            return Err(Error::InvalidState);
        }
        self.state = State::Acquiring;
        if let Err(error) = self.channel.send(&frame(1, 1), Some(proof)) {
            self.state = State::Terminal;
            return Err(error);
        }
        Ok(())
    }

    pub fn release(&mut self) -> Result<(), Error> {
        if self.state != State::Ready {
            return Err(Error::InvalidState);
        }
        self.state = State::ReleaseRequested;
        if let Err(error) = self.channel.send(&frame(1, 2), None) {
            self.state = State::Terminal;
            return Err(error);
        }
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Response, Error> {
        if matches!(self.state, State::Idle | State::Terminal) {
            return Err(Error::InvalidState);
        }
        let result = self
            .channel
            .receive(false)
            .and_then(|(bytes, _)| Response::decode(bytes));
        let response = match result {
            Ok(response) => response,
            Err(Error::Timeout) if self.state == State::Ready => return Err(Error::Idle),
            Err(error) => {
                self.state = State::Terminal;
                return Err(error);
            }
        };
        self.state = match (self.state, response) {
            (State::Acquiring, Response::Applying) => State::Applying,
            (State::Applying, Response::Ready) => State::Ready,
            (State::ReleaseRequested, Response::Releasing) => State::Releasing,
            (State::Releasing, Response::Released) => State::Terminal,
            (State::Acquiring | State::Applying, Response::Rejected)
            | (
                State::Acquiring
                | State::Applying
                | State::Ready
                | State::ReleaseRequested
                | State::Releasing,
                Response::RecoveryRequired,
            ) => State::Terminal,
            _ => {
                self.state = State::Terminal;
                return Err(Error::InvalidState);
            }
        };
        Ok(response)
    }
}

/// Owns a caller-selected LOCAL setup path, not a path received over IPC.
/// Provisioning/enrollment and the final root-owned package path are separate.
/// Socket defaults to 0600; a future package must arrange narrow enrolled-user
/// access explicitly, without making its parent writable by the enrolled UID.
pub struct Listener {
    fd: OwnedFd,
    path: PathBuf,
    identity: (u64, u64),
    enrolled_uid: u32,
}
impl Listener {
    pub fn bind(path: &Path, enrolled_uid: u32) -> Result<Self, Error> {
        let parent = path.parent().ok_or(Error::Unavailable)?;
        let metadata = fs::symlink_metadata(parent).map_err(|_| Error::Unavailable)?;
        if !path.is_absolute()
            || !metadata.is_dir()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o022 != 0
            || fs::canonicalize(parent).map_err(|_| Error::Unavailable)? != parent
        {
            return Err(Error::Unavailable);
        }
        let fd = net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|_| Error::Unavailable)?;
        sockopt::set_socket_passcred(&fd, true).map_err(|_| Error::Unavailable)?;
        net::bind(
            &fd,
            &SocketAddrUnix::new(path).map_err(|_| Error::Unavailable)?,
        )
        .map_err(|_| Error::Unavailable)?;
        let metadata = fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
        let listener = Self {
            fd,
            path: path.to_owned(),
            identity: (metadata.dev(), metadata.ino()),
            enrolled_uid,
        };
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::Unavailable)?;
        net::listen(&listener.fd, 4).map_err(|_| Error::Unavailable)?;
        Ok(listener)
    }

    pub fn accept(&self) -> Result<Session, Error> {
        wait(&self.fd, PollFlags::IN, Instant::now() + TIMEOUT)?;
        let fd = net::accept_with(&self.fd, SocketFlags::CLOEXEC | SocketFlags::NONBLOCK)
            .map_err(|_| Error::Unavailable)?;
        Ok(Session {
            channel: Endpoint::new(fd, self.enrolled_uid)?,
            state: State::Idle,
            proof: None,
        })
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// One accepted connection binds at most one lease/proof FD. Drop closes the
/// descriptor but DOES NOT prove DNS cleanup. Broker must retain this object
/// after I/O loss while reconciling unknown/in-flight DNS effects.
pub struct Session {
    channel: Endpoint,
    state: State,
    proof: Option<OwnedFd>,
}
impl Session {
    pub fn receive_acquire(&mut self) -> Result<BorrowedFd<'_>, Error> {
        if self.state != State::Idle {
            return Err(Error::InvalidState);
        }
        self.state = State::Terminal;
        let (bytes, proof) = self.channel.receive(true)?;
        if decode(bytes, 1)? != 1 {
            return Err(Error::InvalidFrame);
        }
        self.proof = proof;
        self.state = State::Acquiring;
        self.proof
            .as_ref()
            .map(AsFd::as_fd)
            .ok_or(Error::InvalidDescriptor)
    }

    pub fn proof(&self) -> Option<BorrowedFd<'_>> {
        self.proof.as_ref().map(AsFd::as_fd)
    }

    pub fn receive_release(&mut self) -> Result<(), Error> {
        if self.state != State::Ready {
            return Err(Error::InvalidState);
        }
        let result = self.channel.receive(false);
        if matches!(result, Err(Error::Timeout)) {
            return Err(Error::Idle);
        }
        self.state = State::Terminal;
        let (bytes, _) = result?;
        if decode(bytes, 1)? != 2 {
            return Err(Error::InvalidFrame);
        }
        self.state = State::ReleaseRequested;
        Ok(())
    }

    /// Trusted broker result, never client-provided DNS success. Applying/Ready
    /// require kernel admission and DNS readback outside this transport crate.
    pub fn reply(&mut self, response: Response) -> Result<(), Error> {
        let next = match (self.state, response) {
            (State::Acquiring, Response::Applying) => State::Applying,
            (State::Applying, Response::Ready) => State::Ready,
            (State::ReleaseRequested, Response::Releasing) => State::Releasing,
            (State::Releasing, Response::Released) => State::Terminal,
            (State::Acquiring | State::Applying, Response::Rejected)
            | (
                State::Acquiring
                | State::Applying
                | State::Ready
                | State::ReleaseRequested
                | State::Releasing,
                Response::RecoveryRequired,
            ) => State::Terminal,
            _ => return Err(Error::InvalidState),
        };
        self.state = State::Terminal;
        self.channel.send(&frame(2, response as u8), None)?;
        self.state = next;
        // Keep proof even after Released/Rejected: explicit handler lifetime
        // owns final teardown. I/O failure cannot accidentally release it.
        Ok(())
    }
}

#[cfg(test)]
mod tests;
