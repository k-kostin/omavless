// SPDX-License-Identifier: MIT
//! Separate protocol; inherits only the reviewed ancillary discipline, not DNS authority.
use crate::{Error, Result, kernel, protocol};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net::{
        self, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketType, UCred, sockopt,
    },
};
use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    time::Instant,
};

pub(crate) struct Endpoint {
    pub fd: OwnedFd,
    pub peer: UCred,
    peer_pidfd: OwnedFd,
}
impl Endpoint {
    pub(crate) fn new(fd: OwnedFd, uid: u32, until: Instant) -> Result<Self> {
        kernel::tick(until)?;
        if sockopt::socket_domain(&fd).map_err(|_| Error::Unavailable)? != AddressFamily::UNIX
            || sockopt::socket_type(&fd).map_err(|_| Error::Unavailable)? != SocketType::SEQPACKET
        {
            return Err(Error::Refused);
        }
        let peer = sockopt::socket_peercred(&fd).map_err(|_| Error::Unavailable)?;
        if peer.uid.as_raw() != uid || peer.pid.as_raw_nonzero().get() <= 0 {
            return Err(Error::Refused);
        }
        let peer_pidfd = nix::sys::socket::getsockopt(&fd, nix::sys::socket::sockopt::PeerPidfd)
            .map_err(|_| Error::Unavailable)?;
        if kernel::kernel_pid(&peer_pidfd, until)? != peer.pid.as_raw_nonzero().get() as u32 {
            return Err(Error::Refused);
        }
        sockopt::set_socket_passcred(&fd, true).map_err(|_| Error::Unavailable)?;
        kernel::tick(until)?;
        Ok(Self {
            fd,
            peer,
            peer_pidfd,
        })
    }
    pub(crate) fn original_peer(&self, until: Instant) -> Result<OwnedFd> {
        self.check(until)?;
        rustix::io::fcntl_dupfd_cloexec(&self.peer_pidfd, 0).map_err(|_| Error::Unavailable)
    }
    pub(crate) fn check(&self, until: Instant) -> Result<()> {
        kernel::alive(&self.peer_pidfd, until)?;
        if sockopt::socket_peercred(&self.fd).map_err(|_| Error::Unavailable)? != self.peer
            || kernel::kernel_pid(&self.peer_pidfd, until)?
                != self.peer.pid.as_raw_nonzero().get() as u32
        {
            return Err(Error::Refused);
        }
        kernel::tick(until)
    }
    pub(crate) fn send(
        &self,
        frame: &[u8; protocol::BYTES],
        fd: Option<BorrowedFd<'_>>,
        until: Instant,
    ) -> Result<()> {
        kernel::tick(until)?;
        let rights: Vec<_> = fd.into_iter().collect();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !rights.is_empty() && !control.push(SendAncillaryMessage::ScmRights(&rights)) {
            return Err(Error::Refused);
        }
        wait(&self.fd, PollFlags::OUT, until)?;
        // An uncertain send is terminal, even though this interface is read-only.
        let n = net::sendmsg(
            &self.fd,
            &[IoSlice::new(frame)],
            &mut control,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )
        .map_err(|_| Error::ChannelLost)?;
        if n != frame.len() {
            return Err(Error::ChannelLost);
        }
        kernel::tick(until)
    }
    pub(crate) fn receive(
        &self,
        want_fd: bool,
        until: Instant,
    ) -> Result<([u8; protocol::BYTES], Option<OwnedFd>)> {
        loop {
            wait(&self.fd, PollFlags::IN, until)?;
            #[repr(align(8))]
            struct Control([MaybeUninit<u8>; rustix::cmsg_space!(ScmRights(1), ScmCredentials(1))]);
            let mut bytes = Control(
                [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1), ScmCredentials(1))],
            );
            let length = if want_fd {
                bytes.0.len()
            } else {
                rustix::cmsg_space!(ScmCredentials(1))
            };
            let mut control = RecvAncillaryBuffer::new(&mut bytes.0[..length]);
            let mut frame = [0; protocol::BYTES];
            let result = match net::recvmsg(
                &self.fd,
                &mut [IoSliceMut::new(&mut frame)],
                &mut control,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(r) => r,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(_) => return Err(Error::ChannelLost),
            };
            // Always exhaust every reported right before any failure publication.
            let mut descriptor = None;
            let mut rights_seen = false;
            let mut credential_seen = false;
            let mut bad = false;
            for message in control.drain() {
                match message {
                    RecvAncillaryMessage::ScmRights(rights) if want_fd && !rights_seen => {
                        rights_seen = true;
                        let mut count = 0;
                        for fd in rights {
                            if count == 0 {
                                descriptor = Some(fd)
                            } else {
                                drop(fd)
                            }
                            count += 1;
                        }
                        if count != 1 {
                            bad = true
                        }
                    }
                    RecvAncillaryMessage::ScmRights(rights) => {
                        // Exhaust even wholly unexpected records, never rely
                        // on a partial-rights iterator destructor alignment.
                        for fd in rights {
                            drop(fd)
                        }
                        bad = true;
                    }
                    RecvAncillaryMessage::ScmCredentials(c) if !credential_seen => {
                        credential_seen = true;
                        if c != self.peer {
                            bad = true
                        }
                    }
                    _ => bad = true,
                }
            }
            kernel::tick(until)?;
            if result.bytes == 0 {
                return Err(Error::ChannelLost);
            }
            if bad
                || result.bytes != protocol::BYTES
                || result
                    .flags
                    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                || !credential_seen
                || rights_seen != want_fd
            {
                return Err(Error::Refused);
            }
            return Ok((frame, descriptor));
        }
    }
}
fn wait(fd: impl AsFd, events: PollFlags, until: Instant) -> Result<()> {
    loop {
        let left = until
            .checked_duration_since(Instant::now())
            .ok_or(Error::Expired)?;
        let time = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: left.subsec_nanos() as i64,
        };
        let mut row = [PollFd::new(&fd, events)];
        match poll(&mut row, Some(&time)) {
            Ok(0) => return Err(Error::Expired),
            Ok(_) => {
                kernel::tick(until)?;
                if row[0].revents().intersects(events) {
                    return Ok(());
                }
                return Err(Error::ChannelLost);
            }
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Error::ChannelLost),
        }
    }
}
