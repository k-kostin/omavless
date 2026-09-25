//! Fixed root-service admission; no listener, DNS operation or installation.
use rustix::{
    fs::{self, Mode, OFlags},
    net::{self, AddressFamily, SocketAddrUnix, SocketFlags, SocketType},
};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    cell::Cell,
    fmt,
    fs::File,
    future::Future,
    io::Read,
    os::fd::OwnedFd,
    path::Path,
    time::{Duration, Instant},
};
use zbus::{Message, blocking::Connection, zvariant::OwnedValue};

const ENROLLMENT: &str = "/etc/omavless-dns/enrollment.json";
const SYSTEM_BUS: &str = "/run/dbus/system_bus_socket";
const BUS_ADDRESS: &str = "unix:path=/run/dbus/system_bus_socket";
const NOTIFY: &str = "/run/systemd/notify";
const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const MANAGER: &str = "org.freedesktop.systemd1";
const RESOLVED: &str = "org.freedesktop.resolve1";
const UNIT: &str = "/org/freedesktop/systemd1/unit/omavless_2ddns_2dbroker_2eservice";
const SERVICE: &str = "org.freedesktop.systemd1.Service";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_REPLY: usize = 16 * 1024;
const MAX_ENROLLMENT: u64 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Refused,
    InvalidEnrollment,
    BusUnavailable,
    InvalidReply,
    ServiceMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Refused => "DNS broker host admission was refused.",
            Self::InvalidEnrollment => "DNS broker enrollment is invalid.",
            Self::BusUnavailable => "DNS broker service authority is unavailable.",
            Self::InvalidReply => "DNS broker authority response is invalid.",
            Self::ServiceMismatch => "DNS broker service policy is not established.",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrollment {
    schema: u8,
    uid: u32,
    policy: String,
}

/// Authenticated fixed-unit context. It does not establish empty FD store,
/// journal safety, TUN identity or DNS readiness. Constructors never mutate DNS.
pub struct RootContext {
    connection: Connection,
    manager_owner: String,
    resolved_owner: String,
    enrolled_uid: u32,
    notify: OwnedFd,
    timeout: Duration,
    deadline: Cell<Option<Instant>>,
}
impl fmt::Debug for RootContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootContext { .. }")
    }
}

impl RootContext {
    #[cfg(test)]
    pub(crate) fn fixture_context(connection: Connection, notify: OwnedFd) -> Result<Self, Error> {
        Self::authenticate(
            connection,
            notify,
            1000,
            rustix::process::geteuid().as_raw(),
            TIMEOUT,
        )
    }

    /// Only the current main process of the fixed root-owned service can pass.
    /// No caller-selected path, bus address, unit, account, or notification value.
    pub fn admit() -> Result<Self, Error> {
        require_root(rustix::process::geteuid().as_raw())?;
        for path in [
            "/etc",
            "/etc/omavless-dns",
            "/run",
            "/run/dbus",
            "/run/systemd",
        ] {
            protected_directory(Path::new(path), 0)?;
        }
        let uid = read_enrollment(Path::new(ENROLLMENT), 0)?;
        protected_socket(Path::new(SYSTEM_BUS), 0)?;
        protected_socket(Path::new(NOTIFY), 0)?;
        // Literal transport, not system()/session() or ambient DBUS_* values.
        let builder = zbus::connection::Builder::address(BUS_ADDRESS)
            .map_err(|_| Error::BusUnavailable)?
            .max_queued(8);
        let connection = complete(TIMEOUT, builder.build()).map(Connection::from)?;
        let notify = connect_notify(Path::new(NOTIFY))?;
        Self::authenticate(connection, notify, uid, 0, TIMEOUT)
    }

    // Private fixture seam only; production calls it after fixed filesystem
    // checks and literal system-bus connection. This is not a public discovery API.
    fn authenticate(
        connection: Connection,
        notify: OwnedFd,
        enrolled_uid: u32,
        manager_uid: u32,
        timeout: Duration,
    ) -> Result<Self, Error> {
        if enrolled_uid == 0 || enrolled_uid == u32::MAX {
            return Err(Error::InvalidEnrollment);
        }
        let manager_owner = bus_owner(&connection, MANAGER, timeout)?;
        let uid: u32 = daemon_call(
            &connection,
            "GetConnectionUnixUser",
            &manager_owner,
            timeout,
        )?;
        if uid != manager_uid {
            return Err(Error::ServiceMismatch);
        }
        // resolve1 runs as systemd-resolve, not root. Trust the root-administered
        // system bus's restrictive name-ownership policy, then pin its unique owner.
        let resolved_owner = bus_owner(&connection, RESOLVED, timeout)?;
        let context = Self {
            connection,
            manager_owner,
            resolved_owner,
            enrolled_uid,
            notify,
            timeout,
            deadline: Cell::new(None),
        };
        context.recheck()?;
        Ok(context)
    }

    pub fn recheck(&self) -> Result<(), Error> {
        if bus_owner_until(&self.connection, MANAGER, self.call_deadline()?)? != self.manager_owner
            || bus_owner_until(&self.connection, RESOLVED, self.call_deadline()?)?
                != self.resolved_owner
        {
            return Err(Error::ServiceMismatch);
        }
        let pid: u32 = self.property("MainPID")?;
        let access: String = self.property("NotifyAccess")?;
        let capacity: u32 = self.property("FileDescriptorStoreMax")?;
        let preserve: String = self.property("FileDescriptorStorePreserve")?;
        let directory_preserve: String = self.property("RuntimeDirectoryPreserve")?;
        if pid != std::process::id()
            || access != "main"
            || capacity != 1
            || preserve != "yes"
            || directory_preserve != "yes"
        {
            return Err(Error::ServiceMismatch);
        }
        Ok(())
    }

    fn property<T: TryFrom<OwnedValue>>(&self, name: &'static str) -> Result<T, Error> {
        let reply = complete_until(
            self.call_deadline()?,
            self.connection.inner().call_method(
                Some(self.manager_owner.as_str()),
                UNIT,
                Some(PROPERTIES),
                "Get",
                &(SERVICE, name),
            ),
        )?;
        sender(&reply, &self.manager_owner)?;
        T::try_from(bounded::<OwnedValue>(&reply)?).map_err(|_| Error::InvalidReply)
    }

    /// Trusted operation budget, not a deadline for interactive authorization.
    pub fn set_deadline(&self, deadline: Instant) -> Result<(), Error> {
        if deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(30) {
            return Err(Error::Refused);
        }
        self.deadline.set(Some(deadline));
        Ok(())
    }

    fn call_deadline(&self) -> Result<Instant, Error> {
        let now = Instant::now();
        let deadline = self
            .deadline
            .get()
            .unwrap_or(now + self.timeout)
            .min(now + self.timeout);
        if deadline <= now {
            Err(Error::BusUnavailable)
        } else {
            Ok(deadline)
        }
    }

    // Trusted composition only, not fields exposed through the client protocol.
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
    pub(crate) fn manager_owner(&self) -> &str {
        &self.manager_owner
    }
    pub(crate) fn resolved_owner(&self) -> &str {
        &self.resolved_owner
    }
    pub(crate) fn enrolled_uid(&self) -> u32 {
        self.enrolled_uid
    }
    pub(crate) fn duplicate_notify(&self) -> Result<OwnedFd, Error> {
        self.notify.try_clone().map_err(|_| Error::BusUnavailable)
    }

    pub(crate) fn notify_ready(&self) -> Result<(), Error> {
        self.recheck()?;
        self.call_deadline()?;
        let message = b"READY=1";
        match net::send(
            &self.notify,
            message,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
        ) {
            Ok(length) if length == message.len() => Ok(()),
            _ => Err(Error::BusUnavailable),
        }
    }
}

fn require_root(uid: u32) -> Result<(), Error> {
    if uid == 0 {
        Ok(())
    } else {
        Err(Error::Refused)
    }
}

fn protected_directory(path: &Path, owner: u32) -> Result<(), Error> {
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| Error::Refused)?;
    let stat = fs::fstat(&fd).map_err(|_| Error::Refused)?;
    if stat.st_uid != owner || stat.st_mode & 0o022 != 0 {
        return Err(Error::Refused);
    }
    Ok(())
}
fn protected_socket(path: &Path, owner: u32) -> Result<(), Error> {
    let stat =
        fs::statat(fs::CWD, path, fs::AtFlags::SYMLINK_NOFOLLOW).map_err(|_| Error::Refused)?;
    if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::Socket || stat.st_uid != owner {
        return Err(Error::Refused);
    }
    Ok(())
}
fn read_enrollment(path: &Path, owner: u32) -> Result<u32, Error> {
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| Error::InvalidEnrollment)?;
    let stat = fs::fstat(&fd).map_err(|_| Error::InvalidEnrollment)?;
    if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::RegularFile
        || stat.st_uid != owner
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
        || stat.st_size < 1
        || stat.st_size as u64 > MAX_ENROLLMENT
    {
        return Err(Error::InvalidEnrollment);
    }
    let mut bytes = Vec::new();
    File::from(fd)
        .take(MAX_ENROLLMENT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidEnrollment)?;
    if bytes.len() as u64 > MAX_ENROLLMENT {
        return Err(Error::InvalidEnrollment);
    }
    let value: Enrollment = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidEnrollment)?;
    if value.schema != 1
        || value.uid == 0
        || value.uid == u32::MAX
        || value.policy != "meta-ipv4-v1"
    {
        return Err(Error::InvalidEnrollment);
    }
    Ok(value.uid)
}
fn connect_notify(path: &Path) -> Result<OwnedFd, Error> {
    let fd = net::socket_with(
        AddressFamily::UNIX,
        SocketType::DGRAM,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .map_err(|_| Error::BusUnavailable)?;
    net::connect(
        &fd,
        &SocketAddrUnix::new(path).map_err(|_| Error::BusUnavailable)?,
    )
    .map_err(|_| Error::BusUnavailable)?;
    Ok(fd)
}
fn bus_owner(
    connection: &Connection,
    name: &'static str,
    timeout: Duration,
) -> Result<String, Error> {
    bus_owner_until(connection, name, Instant::now() + timeout)
}

fn bus_owner_until(
    connection: &Connection,
    name: &str,
    deadline: Instant,
) -> Result<String, Error> {
    let reply = complete_until(
        deadline,
        connection
            .inner()
            .call_method(Some(DBUS), DBUS_PATH, Some(DBUS), "GetNameOwner", &(name,)),
    )?;
    sender(&reply, DBUS)?;
    let owner: String = bounded(&reply)?;
    if owner.len() > 255 || zbus::names::UniqueName::try_from(owner.as_str()).is_err() {
        return Err(Error::InvalidReply);
    }
    Ok(owner)
}
fn daemon_call<T: DeserializeOwned + zbus::zvariant::Type>(
    connection: &Connection,
    method: &'static str,
    value: &str,
    timeout: Duration,
) -> Result<T, Error> {
    let reply = complete(
        timeout,
        connection
            .inner()
            .call_method(Some(DBUS), DBUS_PATH, Some(DBUS), method, &(value,)),
    )?;
    sender(&reply, DBUS)?;
    bounded(&reply)
}
fn sender(message: &Message, expected: &str) -> Result<(), Error> {
    if message.header().sender().map(|name| name.as_str()) != Some(expected) {
        return Err(Error::InvalidReply);
    }
    Ok(())
}
fn bounded<T: DeserializeOwned + zbus::zvariant::Type>(message: &Message) -> Result<T, Error> {
    if message.data().len() > MAX_REPLY || !message.data().fds().is_empty() {
        return Err(Error::InvalidReply);
    }
    message
        .body()
        .deserialize()
        .map_err(|_| Error::InvalidReply)
}
fn complete<T>(timeout: Duration, call: impl Future<Output = zbus::Result<T>>) -> Result<T, Error> {
    complete_until(Instant::now() + timeout, call)
}

fn complete_until<T>(
    deadline: Instant,
    call: impl Future<Output = zbus::Result<T>>,
) -> Result<T, Error> {
    if Instant::now() >= deadline {
        return Err(Error::BusUnavailable);
    }
    futures_lite::future::block_on(futures_lite::future::race(
        async { call.await.map_err(|_| Error::BusUnavailable) },
        async {
            async_io::Timer::at(deadline).await;
            Err(Error::BusUnavailable)
        },
    ))
}

#[cfg(test)]
mod tests;
