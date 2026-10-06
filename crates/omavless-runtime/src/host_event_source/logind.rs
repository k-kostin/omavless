// SPDX-License-Identifier: MIT
use super::bounded_bus::{BoundedSocket, Monitor};
use super::*;
use async_io::Async;
use futures_lite::StreamExt;
use nix::sys::socket::{MsgFlags, getsockopt, recv, sockopt::PeerCredentials};
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use zbus::{
    Connection, Message, MessageStream,
    connection::{AuthMechanism, Builder},
    message::Type,
    zvariant::OwnedValue,
};

const SYSTEM_SOCKET: &str = "/run/dbus/system_bus_socket";
const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const LOGIN: &str = "org.freedesktop.login1";
const LOGIN_PATH: &str = "/org/freedesktop/login1";
const LOGIN_INTERFACE: &str = "org.freedesktop.login1.Manager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const OWNER_MATCH: &str = "type='signal',sender='org.freedesktop.DBus',path='/org/freedesktop/DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.freedesktop.login1'";
const SLEEP_MATCH: &str = "type='signal',path='/org/freedesktop/login1',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'";

pub(super) struct Logind {
    connection: Connection,
    messages: MessageStream,
    peek: UnixStream,
    monitor: Monitor,
    owner: String,
    uid: u32,
    pid: u32,
    _bus_peer_uid: u32,
    _bus_peer_pid: i32,
    sleeping: bool,
    lost: Option<Lost>,
}

async fn drive<T>(
    connection: &Connection,
    work: impl std::future::Future<Output = Result<T, Lost>>,
) -> Result<T, Lost> {
    futures_lite::future::or(work, async {
        loop {
            connection.executor().tick().await;
        }
    })
    .await
}

impl Logind {
    /// Compiled fixed host constructor; no test or normal factory invokes it.
    pub(super) async fn system(deadline: Instant) -> Result<Self, Lost> {
        Self::connect(Path::new(SYSTEM_SOCKET), 0, 0, deadline).await
    }
    #[cfg(test)]
    pub(super) async fn fixture(path: &Path, deadline: Instant) -> Result<Self, Lost> {
        let uid = nix::unistd::Uid::current().as_raw();
        Self::connect(path, uid, uid, deadline).await
    }
    async fn connect(
        path: &Path,
        directory_uid: u32,
        login_uid: u32,
        deadline: Instant,
    ) -> Result<Self, Lost> {
        let parent = fs::symlink_metadata(path.parent().ok_or(Lost::Authentication)?)
            .map_err(|_| Lost::Unavailable)?;
        let socket = fs::symlink_metadata(path).map_err(|_| Lost::Unavailable)?;
        if parent.file_type().is_symlink()
            || !parent.is_dir()
            || parent.uid() != directory_uid
            || parent.mode() & 0o022 != 0
            || socket.file_type().is_symlink()
            || !socket.file_type().is_socket()
        {
            return Err(Lost::Authentication);
        }
        let stream = within(deadline, async {
            Async::<UnixStream>::connect(path)
                .await
                .map_err(|_| Lost::Unavailable)
        })
        .await?;
        let peer =
            getsockopt(stream.get_ref(), PeerCredentials).map_err(|_| Lost::Authentication)?;
        // Trusted immutable directory selects the fixed endpoint. Socket-
        // activated inode/creator/process credentials may legitimately differ;
        // record the actual connected credentials without guessing PID reuse.
        if peer.pid() <= 0 {
            return Err(Lost::Authentication);
        }
        let peek = stream
            .get_ref()
            .try_clone()
            .map_err(|_| Lost::Unavailable)?;
        let (bounded, monitor) = BoundedSocket::new(stream);
        let connection = within(deadline, async {
            Builder::socket(bounded)
                .auth_mechanism(AuthMechanism::External)
                .internal_executor(false)
                .max_queued(MAX_QUEUED_FRAMES)
                .build()
                .await
                .map_err(|_| Lost::Unavailable)
        })
        .await?;
        monitor.state()?.before_subscription()?;
        // Generic receiver exists BEFORE AddMatch and all owner discovery.
        // Do not use for_match_rule's bus-before-local-channel insertion gap.
        let messages = MessageStream::from(&connection);
        let mut source = Self {
            connection,
            messages,
            peek,
            monitor,
            owner: String::new(),
            uid: login_uid,
            pid: 0,
            _bus_peer_uid: peer.uid(),
            _bus_peer_pid: peer.pid(),
            sleeping: false,
            lost: None,
        };
        let initialized = source.initialize(deadline).await;
        if let Err(error) = initialized {
            source.terminal(error);
            return Err(error);
        }
        Ok(source)
    }
    async fn call<B: serde::Serialize + zbus::zvariant::DynamicType>(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        body: &B,
        deadline: Instant,
    ) -> Result<Message, Lost> {
        let reply = within(
            deadline,
            drive(&self.connection, async {
                self.connection
                    .call_method(Some(destination), path, Some(interface), member, body)
                    .await
                    .map_err(|_| Lost::Unavailable)
            }),
        )
        .await?;
        if reply.header().sender().map(|s| s.as_str()) != Some(destination) {
            return Err(Lost::Authentication);
        }
        Ok(reply)
    }
    async fn owner_now(&self, deadline: Instant) -> Result<String, Lost> {
        let reply = self
            .call(DBUS, DBUS_PATH, DBUS, "GetNameOwner", &(LOGIN,), deadline)
            .await?;
        let name: String = reply.body().deserialize().map_err(|_| Lost::InvalidFrame)?;
        if !name.starts_with(':') || name.len() > 255 {
            return Err(Lost::Authentication);
        }
        Ok(name)
    }
    async fn credentials(&self, owner: &str, deadline: Instant) -> Result<(u32, u32), Lost> {
        let uid = self
            .call(
                DBUS,
                DBUS_PATH,
                DBUS,
                "GetConnectionUnixUser",
                &(owner,),
                deadline,
            )
            .await?
            .body()
            .deserialize::<u32>()
            .map_err(|_| Lost::InvalidFrame)?;
        let pid = self
            .call(
                DBUS,
                DBUS_PATH,
                DBUS,
                "GetConnectionUnixProcessID",
                &(owner,),
                deadline,
            )
            .await?
            .body()
            .deserialize::<u32>()
            .map_err(|_| Lost::InvalidFrame)?;
        if uid != self.uid || pid == 0 {
            return Err(Lost::Authentication);
        }
        Ok((uid, pid))
    }
    async fn initialize(&mut self, deadline: Instant) -> Result<(), Lost> {
        for rule in [OWNER_MATCH, SLEEP_MATCH] {
            self.call(DBUS, DBUS_PATH, DBUS, "AddMatch", &(rule,), deadline)
                .await?;
        }
        self.owner = self.owner_now(deadline).await?;
        let (_, pid) = self.credentials(&self.owner, deadline).await?;
        self.pid = pid;
        let reply = self
            .call(
                &self.owner,
                LOGIN_PATH,
                PROPERTIES,
                "Get",
                &(LOGIN_INTERFACE, "PreparingForSleep"),
                deadline,
            )
            .await?;
        let value: OwnedValue = reply.body().deserialize().map_err(|_| Lost::InvalidFrame)?;
        self.sleeping = bool::try_from(value).map_err(|_| Lost::InvalidFrame)?;
        self.verify_owner(deadline).await?;
        self.drain(deadline, true).await?;
        self.monitor
            .state()?
            .reset_after_drain(MAX_POLL_BYTES / 2)?;
        Ok(())
    }
    pub(super) fn sleeping(&self) -> bool {
        self.sleeping
    }
    fn check(&self) -> Result<(), Lost> {
        if let Some(error) = self.lost {
            return Err(error);
        }
        self.monitor.state()?.loss()?;
        if self.connection.is_closed() {
            return Err(Lost::Unavailable);
        }
        Ok(())
    }
    pub(super) fn terminal(&mut self, error: Lost) {
        self.lost.get_or_insert(error);
        self.monitor.terminal(error);
        let _ = self.peek.shutdown(std::net::Shutdown::Both);
    }
    pub(super) fn readable(&self) -> Result<bool, Lost> {
        self.check()?;
        if self.monitor.state()?.pending() {
            return Ok(true);
        }
        let mut byte = [0; 1];
        match recv(
            self.peek.as_raw_fd(),
            &mut byte,
            MsgFlags::MSG_PEEK | MsgFlags::MSG_DONTWAIT,
        ) {
            Ok(0) => Err(Lost::Unavailable),
            Ok(_) => Ok(true),
            Err(nix::errno::Errno::EAGAIN) => Ok(false),
            Err(_) => Err(Lost::Unavailable),
        }
    }
    async fn verify_owner(&self, deadline: Instant) -> Result<(), Lost> {
        if self.owner_now(deadline).await? != self.owner {
            return Err(Lost::OwnerChanged);
        }
        if self.credentials(&self.owner, deadline).await? != (self.uid, self.pid) {
            return Err(Lost::OwnerChanged);
        }
        self.check()
    }
    fn classify(&mut self, msg: &Message, initializing: bool) -> Result<Option<Event>, Lost> {
        let h = msg.header();
        if msg.message_type() != Type::Signal {
            return Ok(None);
        }
        let path = h.path().map(|p| p.as_str());
        let iface = h.interface().map(|i| i.as_str());
        let member = h.member().map(|m| m.as_str());
        if path == Some(DBUS_PATH) && iface == Some(DBUS) && member == Some("NameOwnerChanged") {
            if h.sender().map(|s| s.as_str()) != Some(DBUS) {
                return Err(Lost::Authentication);
            }
            let (name, old, new): (String, String, String) =
                msg.body().deserialize().map_err(|_| Lost::InvalidFrame)?;
            if name == LOGIN && old != new {
                return Err(if initializing {
                    Lost::InitializationRace
                } else {
                    Lost::OwnerChanged
                });
            }
            return Ok(None);
        }
        if path == Some(LOGIN_PATH)
            && iface == Some(LOGIN_INTERFACE)
            && member == Some("PrepareForSleep")
        {
            if h.sender().map(|s| s.as_str()) != Some(self.owner.as_str()) {
                return Err(Lost::Authentication);
            }
            let sleeping: bool = msg.body().deserialize().map_err(|_| Lost::InvalidFrame)?;
            if initializing {
                return Err(Lost::InitializationRace);
            }
            self.sleeping = sleeping;
            return Ok(Some(if sleeping {
                Event::Suspend
            } else {
                Event::Resume
            }));
        }
        Ok(None) // bounded accounting still includes every irrelevant frame.
    }
    async fn drain(&mut self, deadline: Instant, initializing: bool) -> Result<Vec<Event>, Lost> {
        let mut events = Vec::new();
        for _ in 0..MAX_QUEUED_FRAMES {
            self.check()?;
            let ready = futures_lite::future::poll_once(self.messages.next()).await;
            let message = match ready {
                Some(message) => message
                    .ok_or(Lost::Unavailable)?
                    .map_err(|_| Lost::Unavailable)?,
                None => {
                    // Drive ready original reader tasks, never a detached worker.
                    if futures_lite::future::poll_once(self.connection.executor().tick())
                        .await
                        .is_some()
                    {
                        continue;
                    }
                    if !self.readable()? {
                        break;
                    }
                    within(
                        deadline,
                        drive(&self.connection, async {
                            self.messages
                                .next()
                                .await
                                .ok_or(Lost::Unavailable)?
                                .map_err(|_| Lost::Unavailable)
                        }),
                    )
                    .await?
                }
            };
            self.monitor.state()?.retire_one()?;
            if let Some(event) = self.classify(&message, initializing)? {
                events.push(event);
            }
            if events.len() > MAX_HINTS {
                return Err(Lost::Overflow);
            }
            if Instant::now() >= deadline {
                return Err(Lost::Deadline);
            }
        }
        if self.readable()? {
            return Err(Lost::Overflow);
        }
        Ok(events)
    }
    pub(super) async fn poll(&mut self, deadline: Instant) -> Result<Vec<Event>, Lost> {
        let result = self.drain(deadline, false).await;
        match result {
            Ok(events) => {
                self.monitor
                    .state()?
                    .reset_after_drain(MAX_POLL_BYTES / 2)?;
                Ok(events)
            }
            Err(error) => {
                self.terminal(error);
                Err(error)
            }
        }
    }
    pub(super) async fn quiescent(&mut self, deadline: Instant) -> Result<Vec<Event>, Lost> {
        self.check()?;
        self.verify_owner(deadline).await?;
        let events = self.drain(deadline, false).await?;
        self.monitor
            .state()?
            .reset_after_drain(MAX_POLL_BYTES / 2)?;
        Ok(events) // caller retains them; an event is not source loss or quiescence.
    }
}
impl Drop for Logind {
    fn drop(&mut self) {
        let _ = self.peek.shutdown(std::net::Shutdown::Both);
    }
}
