// SPDX-License-Identifier: MIT
//! Only recvmsg sockaddr metadata attests kernel origin. Header pid/sequence
//! are payload and neither authenticate nor detect notification gaps.
use super::*;
use nix::errno::Errno;
use nix::sys::socket::{
    AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType, bind, recvmsg,
    setsockopt, socket, sockopt::RcvBuf,
};
use std::io::IoSliceMut;
use std::os::fd::{AsRawFd, OwnedFd};

const GROUPS: u32 = 1 | 0x10 | 0x40 | 0x100 | 0x400;

pub(super) struct KernelMetadata {
    sender_port: u32,
    truncated: bool,
}
pub(super) fn datagram(bytes: &[u8], metadata: KernelMetadata) -> Result<usize, Lost> {
    if metadata.sender_port != 0 {
        return Err(Lost::Authentication);
    }
    if metadata.truncated || bytes.len() > MAX_WIRE_BYTES {
        return Err(Lost::Overflow);
    }
    if bytes.is_empty() {
        return Err(Lost::InvalidFrame);
    }
    let mut offset = 0;
    let mut events = 0;
    let mut frames = 0;
    while offset < bytes.len() {
        if bytes.len() - offset < 16 {
            return Err(Lost::InvalidFrame);
        }
        let header = &bytes[offset..offset + 16];
        let length = u32::from_ne_bytes(header[..4].try_into().unwrap()) as usize;
        let kind = u16::from_ne_bytes(header[4..6].try_into().unwrap());
        if length < 16 || length > bytes.len() - offset {
            return Err(Lost::InvalidFrame);
        }
        frames += 1;
        if frames > MAX_QUEUED_FRAMES {
            return Err(Lost::Overflow);
        }
        let body = &bytes[offset + 16..offset + length];
        let prefix = match kind {
            1 if body.is_empty() => None,
            2..=4 => return Err(Lost::Overflow), // ERROR/DONE/OVERRUN: never requested dumps.
            16 | 17 => Some(16),
            20 | 21 => Some(8),
            24 | 25 => Some(12),
            _ => None,
        };
        if let Some(prefix) = prefix {
            if body.len() < prefix {
                return Err(Lost::InvalidFrame);
            }
            let mut attr = prefix;
            while attr < body.len() {
                if body.len() - attr < 4 {
                    return Err(Lost::InvalidFrame);
                }
                let len = u16::from_ne_bytes(body[attr..attr + 2].try_into().unwrap()) as usize;
                if len < 4 || len > body.len() - attr {
                    return Err(Lost::InvalidFrame);
                }
                let aligned = len.checked_add(3).ok_or(Lost::Overflow)? & !3;
                if len == body.len() - attr {
                    attr = body.len();
                } else {
                    attr = attr.checked_add(aligned).ok_or(Lost::Overflow)?;
                }
                if attr > body.len() {
                    return Err(Lost::InvalidFrame);
                }
            }
            events += 1;
        }
        if length == bytes.len() - offset {
            offset = bytes.len();
        } else {
            let aligned = length.checked_add(3).ok_or(Lost::Overflow)? & !3;
            offset = offset.checked_add(aligned).ok_or(Lost::Overflow)?;
            if offset > bytes.len() {
                return Err(Lost::InvalidFrame);
            }
        }
    }
    Ok(usize::from(events != 0)) // one hint, after validating EVERY header/attr.
}

pub(super) struct RouteSocket {
    fd: OwnedFd,
}
impl RouteSocket {
    pub(super) fn system() -> Result<Self, Lost> {
        let fd = socket(
            AddressFamily::Netlink,
            SockType::Raw,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            SockProtocol::NetlinkRoute,
        )
        .map_err(|_| Lost::Unavailable)?;
        setsockopt(&fd, RcvBuf, &(64usize * 1024)).map_err(|_| Lost::Unavailable)?;
        bind(fd.as_raw_fd(), &NetlinkAddr::new(0, GROUPS)).map_err(|_| Lost::Unavailable)?;
        Ok(Self { fd })
    }
    pub(super) fn readable(&self) -> Result<bool, Lost> {
        let mut data = [0u8; 1];
        let mut iov = [IoSliceMut::new(&mut data)];
        match recvmsg::<NetlinkAddr>(
            self.fd.as_raw_fd(),
            &mut iov,
            None,
            MsgFlags::MSG_PEEK | MsgFlags::MSG_DONTWAIT,
        ) {
            Ok(_) => Ok(true),
            Err(Errno::EAGAIN) => Ok(false),
            Err(Errno::ENOBUFS) => Err(Lost::Overflow),
            Err(_) => Err(Lost::Unavailable),
        }
    }
    pub(super) fn poll(&self, deadline: Instant) -> Result<Vec<Event>, Lost> {
        let mut events = Vec::new();
        let mut total = 0usize;
        for _ in 0..MAX_QUEUED_FRAMES {
            if Instant::now() >= deadline {
                return Err(Lost::Deadline);
            }
            let mut data = [0u8; MAX_WIRE_BYTES];
            let mut iov = [IoSliceMut::new(&mut data)];
            let result =
                recvmsg::<NetlinkAddr>(self.fd.as_raw_fd(), &mut iov, None, MsgFlags::MSG_DONTWAIT);
            let message = match result {
                Err(Errno::EAGAIN) => break,
                Err(Errno::ENOBUFS) => return Err(Lost::Overflow),
                Err(_) => return Err(Lost::Unavailable),
                Ok(msg) => msg,
            };
            if message
                .flags
                .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC)
                || message.bytes > MAX_WIRE_BYTES
            {
                return Err(Lost::Overflow);
            }
            let metadata = KernelMetadata {
                sender_port: message.address.ok_or(Lost::Authentication)?.pid(),
                truncated: false,
            };
            let length = message.bytes;
            total = total.checked_add(length).ok_or(Lost::Overflow)?;
            if total > MAX_POLL_BYTES / 2 {
                return Err(Lost::Overflow);
            }
            let count = datagram(&data[..length], metadata)?;
            if count != 0 && events.is_empty() {
                events.push(Event::NetworkChanged);
            }
        }
        if self.readable()? {
            return Err(Lost::Overflow);
        }
        Ok(events)
    }
}

#[cfg(test)]
pub(super) fn synthetic(bytes: &[u8], sender_port: u32, truncated: bool) -> Result<usize, Lost> {
    datagram(
        bytes,
        KernelMetadata {
            sender_port,
            truncated,
        },
    )
}
