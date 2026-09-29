// SPDX-License-Identifier: MIT

//! Opt-in AUTH response sender observation. Never a write or broker permit.
//!
//! Linux SO_PEERCRED can identify a listener creator rather than the process
//! writing a socket-activated response. SO_PASSCRED observes the latter. This
//! probe sends only AUTH EXTERNAL, then closes; no BEGIN, Hello, method call,
//! manager/environment read, or GSettings access occurs.

use crate::{Error, local_bus};
use gio::glib;
use gio::prelude::*;
use std::os::unix::fs::MetadataExt;
use std::time::{Duration, Instant};

const MAX_AUTH_FRAME: usize = 512;
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// Fixed non-sensitive observations, deliberately insufficient for admission.
pub struct AuthSenderProbe {
    listener_matches_sender: bool,
}

impl AuthSenderProbe {
    pub fn listener_matches_sender(&self) -> bool {
        self.listener_matches_sender
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Sender {
    pid: u32,
    uid: u32,
}

// No Debug, formatting, or serialization of private kernel/bus identities.
struct AuthFrame {
    bytes: Vec<u8>,
    sender: Option<Sender>,
}

impl AuthFrame {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_AUTH_FRAME),
            sender: None,
        }
    }

    fn append(&mut self, bytes: &[u8], sender: Sender, uid: u32) -> Result<bool, Error> {
        if bytes.is_empty()
            || sender.pid == 0
            || sender.uid != uid
            || self.sender.is_some_and(|previous| previous != sender)
            || self.bytes.len() + bytes.len() > MAX_AUTH_FRAME
        {
            return Err(Error::IdentityUnverified);
        }
        self.sender = Some(sender);
        self.bytes.extend_from_slice(bytes);
        if let Some(end) = self.bytes.windows(2).position(|v| v == b"\r\n") {
            if end + 2 != self.bytes.len()
                || end != 35
                || !self.bytes.starts_with(b"OK ")
                || !self.bytes[3..end].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(Error::IdentityUnverified);
            }
            return Ok(true);
        }
        if self.bytes.len() == MAX_AUTH_FRAME {
            return Err(Error::IdentityUnverified);
        }
        Ok(false)
    }
}

pub fn probe_auth_sender_read_only() -> Result<AuthSenderProbe, Error> {
    let deadline = Instant::now() + PROBE_DEADLINE;
    let uid = nix::unistd::geteuid().as_raw();
    if uid == 0 || uid != nix::unistd::getuid().as_raw() {
        return Err(Error::IdentityUnverified);
    }
    let runtime = format!("/run/user/{uid}");
    local_bus::validate_environment(
        &runtime,
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").as_deref(),
    )?;
    let directory = local_bus::fixed_directory(uid)?;
    let endpoint = local_bus::pin_endpoint(&directory, uid)?;
    let socket = local_bus::connect_endpoint(&endpoint)?;
    let result = (|| {
        let peer = socket
            .credentials()
            .map_err(|_| Error::IdentityUnverified)?;
        if peer.unix_user().map_err(|_| Error::IdentityUnverified)? != uid {
            return Err(Error::IdentityUnverified);
        }
        // Do not require the listener creator to still exist or be the broker.
        let listener_pid = peer.unix_pid().map_err(|_| Error::IdentityUnverified)?;
        let sender = observe_auth(&socket, uid, deadline)?;
        let first_start = local_bus::process_start(sender.pid, uid)?;
        let current_directory = local_bus::fixed_directory(uid)?;
        let current_endpoint = local_bus::pin_endpoint(&current_directory, uid)?;
        for (a, b) in [
            (&directory, &current_directory),
            (&endpoint, &current_endpoint),
        ] {
            let a = a.metadata().map_err(|_| Error::IdentityUnverified)?;
            let b = b.metadata().map_err(|_| Error::IdentityUnverified)?;
            if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
                return Err(Error::OwnerChanged);
            }
        }
        if local_bus::process_start(sender.pid, uid)? != first_start || Instant::now() >= deadline {
            return Err(Error::OwnerChanged);
        }
        Ok(AuthSenderProbe {
            listener_matches_sender: i64::from(listener_pid) == i64::from(sender.pid),
        })
    })();
    let _ = socket.close();
    result
}

fn wait(
    socket: &gio::Socket,
    condition: glib::IOCondition,
    deadline: Instant,
) -> Result<(), Error> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|v| !v.is_zero())
        .ok_or(Error::ManagerUnavailable)?;
    socket
        .condition_timed_wait(
            condition,
            i64::try_from(remaining.as_micros()).map_err(|_| Error::ManagerUnavailable)?,
            None::<&gio::Cancellable>,
        )
        .map_err(|_| Error::ManagerUnavailable)
}

fn observe_auth(socket: &gio::Socket, uid: u32, deadline: Instant) -> Result<Sender, Error> {
    socket
        .set_option(nix::libc::SOL_SOCKET, nix::libc::SO_PASSCRED, 1)
        .map_err(|_| Error::IdentityUnverified)?;
    socket.set_blocking(false);
    let mut request = b"\0AUTH EXTERNAL ".to_vec();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in uid.to_string().bytes() {
        request.push(HEX[(byte >> 4) as usize]);
        request.push(HEX[(byte & 15) as usize]);
    }
    request.extend_from_slice(b"\r\n");
    let mut sent = 0;
    while sent < request.len() {
        wait(socket, glib::IOCondition::OUT, deadline)?;
        match socket.send(&request[sent..], None::<&gio::Cancellable>) {
            Ok(0) => return Err(Error::ManagerUnavailable),
            Ok(count) => sent += count,
            Err(error) if error.matches(gio::IOErrorEnum::WouldBlock) => (),
            Err(_) => return Err(Error::ManagerUnavailable),
        }
    }

    let mut frame = AuthFrame::new();
    loop {
        wait(socket, glib::IOCondition::IN, deadline)?;
        let mut bytes = [0; MAX_AUTH_FRAME + 1];
        let mut messages = gio::SocketControlMessages::new();
        let received = socket.receive_message(
            None,
            &mut [gio::InputVector::new(&mut bytes)],
            Some(&mut messages),
            nix::libc::MSG_CMSG_CLOEXEC,
            None::<&gio::Cancellable>,
        );
        let (count, flags) = match received {
            Ok(value) => value,
            Err(error) if error.matches(gio::IOErrorEnum::WouldBlock) => continue,
            Err(_) => return Err(Error::ManagerUnavailable),
        };
        let sender = parse_ancillary(&messages, flags)?;
        if frame.append(&bytes[..count], sender, uid)? {
            if Instant::now() >= deadline {
                return Err(Error::ManagerUnavailable);
            }
            return frame.sender.ok_or(Error::IdentityUnverified);
        }
    }
}

fn parse_ancillary(messages: &[gio::SocketControlMessage], flags: i32) -> Result<Sender, Error> {
    // GIO owns received descriptors (including refused UnixFDMessage values)
    // and closes them when the message list is dropped. Never leak SCM_RIGHTS
    // received alongside malicious or truncated authentication bytes.
    if flags & (nix::libc::MSG_CTRUNC | nix::libc::MSG_TRUNC) != 0 || messages.len() != 1 {
        return Err(Error::IdentityUnverified);
    }
    let message = messages[0]
        .downcast_ref::<gio::UnixCredentialsMessage>()
        .ok_or(Error::IdentityUnverified)?;
    let credentials = message.credentials();
    let pid = u32::try_from(
        credentials
            .unix_pid()
            .map_err(|_| Error::IdentityUnverified)?,
    )
    .map_err(|_| Error::IdentityUnverified)?;
    let uid = credentials
        .unix_user()
        .map_err(|_| Error::IdentityUnverified)?;
    Ok(Sender { pid, uid })
}

#[cfg(test)]
mod tests;
