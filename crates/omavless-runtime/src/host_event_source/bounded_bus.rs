// SPDX-License-Identifier: MIT
//! Safe admission wrapper for the pinned zbus Unix socket. The upstream
//! receive_message allocator sees the final fixed header only after its full
//! advertised size is checked here. Auth reads cannot overread binary frames.

use super::*;
use async_io::Async;
use std::fmt;
use std::io;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, MutexGuard};
use zbus::connection::socket::{ReadHalf, Socket, Split, WriteHalf};

const MAX_AUTH_BYTES: usize = 1024;
const MAX_AUTH_LINE: usize = 128;

pub(super) struct Framing {
    binary: bool,
    auth_line: [u8; MAX_AUTH_LINE],
    auth_line_len: usize,
    auth_received: usize,
    auth_in_line_len: usize,
    header: [u8; 16],
    header_len: usize,
    body_remaining: usize,
    received: u64,
    retired: u64,
    bytes_left: usize,
    lost: Option<Lost>,
}
impl Framing {
    pub(super) fn new() -> Self {
        Self {
            binary: false,
            auth_line: [0; MAX_AUTH_LINE],
            auth_line_len: 0,
            auth_received: 0,
            auth_in_line_len: 0,
            header: [0; 16],
            header_len: 0,
            body_remaining: 0,
            received: 0,
            retired: 0,
            bytes_left: MAX_STARTUP_BYTES,
            lost: None,
        }
    }
    fn refuse<T>(&mut self, lost: Lost) -> Result<T, Lost> {
        self.lost.get_or_insert(lost);
        Err(self.lost.unwrap())
    }
    pub(super) fn loss(&self) -> Result<(), Lost> {
        self.lost.map_or(Ok(()), Err)
    }
    fn read_limit(&self, offered: usize) -> usize {
        offered.min(if !self.binary {
            1
        } else if self.header_len < 16 {
            16 - self.header_len
        } else {
            self.body_remaining
        })
    }
    pub(super) fn written(&mut self, bytes: &[u8]) -> Result<(), Lost> {
        self.loss()?;
        if self.binary {
            return Ok(());
        }
        self.auth_received = match self.auth_received.checked_add(bytes.len()) {
            Some(n) => n,
            None => return self.refuse(Lost::Overflow),
        };
        if self.auth_received > MAX_AUTH_BYTES || bytes.len() > self.bytes_left {
            return self.refuse(Lost::Overflow);
        }
        self.bytes_left -= bytes.len();
        for &byte in bytes {
            if self.auth_line_len == MAX_AUTH_LINE {
                return self.refuse(Lost::Overflow);
            }
            self.auth_line[self.auth_line_len] = byte;
            self.auth_line_len += 1;
            if byte == b'\n' {
                if &self.auth_line[..self.auth_line_len] == b"BEGIN\r\n" {
                    self.binary = true;
                }
                self.auth_line_len = 0;
            }
        }
        Ok(())
    }
    pub(super) fn received_bytes(&mut self, bytes: &[u8]) -> Result<(), Lost> {
        self.loss()?;
        if bytes.is_empty() {
            return self.refuse(Lost::Unavailable);
        }
        if !self.binary {
            self.auth_received = match self.auth_received.checked_add(bytes.len()) {
                Some(n) => n,
                None => return self.refuse(Lost::Overflow),
            };
            if self.auth_received > MAX_AUTH_BYTES || bytes.len() > self.bytes_left {
                return self.refuse(Lost::Overflow);
            }
            self.bytes_left -= bytes.len();
            for &byte in bytes {
                self.auth_in_line_len += 1;
                if self.auth_in_line_len > MAX_AUTH_LINE {
                    return self.refuse(Lost::Overflow);
                }
                if byte == b'\n' {
                    self.auth_in_line_len = 0;
                }
            }
            return Ok(());
        }
        if bytes.len() > self.read_limit(bytes.len()) {
            return self.refuse(Lost::InvalidFrame);
        }
        if self.header_len < 16 {
            self.header[self.header_len..self.header_len + bytes.len()].copy_from_slice(bytes);
            self.header_len += bytes.len();
            if self.header_len < 16 {
                return Ok(());
            }
            let h = &self.header;
            if !matches!(h[0], b'l' | b'B')
                || !(1..=4).contains(&h[1])
                || h[3] != 1
                || h[2] & !7 != 0
            {
                return self.refuse(Lost::InvalidFrame);
            }
            let number = |offset| {
                let b: [u8; 4] = h[offset..offset + 4].try_into().unwrap();
                if h[0] == b'l' {
                    u32::from_le_bytes(b)
                } else {
                    u32::from_be_bytes(b)
                }
            };
            let fields = number(12) as usize;
            let body = number(4) as usize;
            if number(8) == 0 {
                return self.refuse(Lost::InvalidFrame);
            }
            let Some(header) = 16usize.checked_add(fields) else {
                return self.refuse(Lost::Overflow);
            };
            let Some(total) = header
                .checked_add((8 - header % 8) % 8)
                .and_then(|n| n.checked_add(body))
            else {
                return self.refuse(Lost::Overflow);
            };
            if total > MAX_WIRE_BYTES || self.received - self.retired >= MAX_QUEUED_FRAMES as u64 {
                return self.refuse(Lost::Overflow);
            }
            if total > self.bytes_left {
                return self.refuse(Lost::Overflow);
            }
            self.bytes_left -= total;
            self.body_remaining = total - 16;
        } else {
            self.body_remaining -= bytes.len();
        }
        if self.body_remaining == 0 {
            self.header_len = 0;
            self.received = match self.received.checked_add(1) {
                Some(n) => n,
                None => return self.refuse(Lost::Overflow),
            };
        }
        Ok(())
    }
    pub(super) fn reset_after_drain(&mut self, bytes: usize) -> Result<(), Lost> {
        self.loss()?;
        if self.partial() || self.received != self.retired {
            return self.refuse(Lost::InvalidFrame);
        }
        self.bytes_left = bytes;
        Ok(())
    }
    pub(super) fn partial(&self) -> bool {
        self.header_len != 0
    }
    pub(super) fn pending(&self) -> bool {
        self.partial() || self.received != self.retired
    }
    pub(super) fn retire_one(&mut self) -> Result<(), Lost> {
        self.loss()?;
        if self.retired == self.received {
            return self.refuse(Lost::InvalidFrame);
        }
        self.retired += 1;
        Ok(())
    }
    pub(super) fn before_subscription(&mut self) -> Result<(), Lost> {
        self.loss()?;
        if self.partial() {
            return self.refuse(Lost::InvalidFrame);
        }
        self.retired = self.received;
        Ok(()) // Hello only; no matches installed yet.
    }
}

#[derive(Clone)]
pub(super) struct Monitor(Arc<Mutex<Framing>>);
impl Monitor {
    pub(super) fn state(&self) -> Result<MutexGuard<'_, Framing>, Lost> {
        self.0.lock().map_err(|_| Lost::Unavailable) // Never recover a poisoned inner.
    }
    fn io_state(&self) -> io::Result<MutexGuard<'_, Framing>> {
        self.state().map_err(io_loss)
    }
    pub(super) fn terminal(&self, lost: Lost) {
        if let Ok(mut s) = self.state() {
            s.lost.get_or_insert(lost);
        }
    }
}
fn io_loss(_: Lost) -> io::Error {
    io::Error::other("event source unavailable")
}

pub(super) struct BoundedSocket {
    stream: Arc<Async<UnixStream>>,
    monitor: Monitor,
}
pub(super) struct Reader {
    stream: Arc<Async<UnixStream>>,
    monitor: Monitor,
}
pub(super) struct Writer {
    stream: Arc<Async<UnixStream>>,
    monitor: Monitor,
}
impl BoundedSocket {
    pub(super) fn new(stream: Async<UnixStream>) -> (Self, Monitor) {
        let monitor = Monitor(Arc::new(Mutex::new(Framing::new())));
        (
            Self {
                stream: Arc::new(stream),
                monitor: monitor.clone(),
            },
            monitor,
        )
    }
}
impl Socket for BoundedSocket {
    type ReadHalf = Reader;
    type WriteHalf = Writer;
    fn split(self) -> Split<Reader, Writer> {
        Split::new(
            Reader {
                stream: Arc::clone(&self.stream),
                monitor: self.monitor.clone(),
            },
            Writer {
                stream: self.stream,
                monitor: self.monitor,
            },
        )
    }
}
impl fmt::Debug for Reader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundedEventReader")
    }
}
impl fmt::Debug for Writer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundedEventWriter")
    }
}
#[async_trait::async_trait]
impl ReadHalf for Reader {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        let limit = {
            let s = self.monitor.io_state()?;
            s.loss().map_err(io_loss)?;
            s.read_limit(buf.len())
        };
        if limit == 0 {
            return Err(io_loss(Lost::InvalidFrame));
        }
        let result = ReadHalf::recvmsg(&mut self.stream, &mut buf[..limit]).await;
        let (n, fds) = result.map_err(|_| {
            self.monitor.terminal(Lost::Unavailable);
            io_loss(Lost::Unavailable)
        })?;
        if !fds.is_empty() {
            drop(fds); // Every delivered OwnedFd is refused/closed, even during auth.
            self.monitor.terminal(Lost::UnexpectedDescriptors);
            return Err(io_loss(Lost::UnexpectedDescriptors));
        }
        self.monitor
            .io_state()?
            .received_bytes(&buf[..n])
            .map_err(io_loss)?;
        Ok((n, Vec::new()))
    }
    async fn peer_credentials(&mut self) -> io::Result<zbus::fdo::ConnectionCredentials> {
        ReadHalf::peer_credentials(&mut self.stream).await
    }
}
#[async_trait::async_trait]
impl WriteHalf for Writer {
    async fn sendmsg(&mut self, buf: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<usize> {
        if !fds.is_empty() {
            self.monitor.terminal(Lost::UnexpectedDescriptors);
            return Err(io_loss(Lost::UnexpectedDescriptors));
        }
        self.monitor.io_state()?.loss().map_err(io_loss)?;
        let n = WriteHalf::sendmsg(&mut self.stream, buf, fds)
            .await
            .map_err(|_| {
                self.monitor.terminal(Lost::Unavailable);
                io_loss(Lost::Unavailable)
            })?;
        self.monitor
            .io_state()?
            .written(&buf[..n])
            .map_err(io_loss)?;
        Ok(n)
    }
    async fn close(&mut self) -> io::Result<()> {
        self.monitor.terminal(Lost::Unavailable);
        self.stream.get_ref().shutdown(std::net::Shutdown::Both)
    }
    async fn peer_credentials(&mut self) -> io::Result<zbus::fdo::ConnectionCredentials> {
        ReadHalf::peer_credentials(&mut self.stream).await
    }
}
