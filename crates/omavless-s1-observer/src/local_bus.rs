// SPDX-License-Identifier: MIT

//! Read-only continuity checks, not broker, session, or write authority.

use crate::Error;
use gio::glib::{self, prelude::ToVariant};
use gio::prelude::*;
use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const MANAGER: &str = "org.freedesktop.systemd1";

pub(super) fn call(
    bus: &gio::DBusConnection,
    destination: &str,
    path: &str,
    interface: &str,
    method: &str,
    params: &glib::Variant,
) -> Result<glib::Variant, Error> {
    bus.call_sync(
        Some(destination),
        path,
        interface,
        method,
        Some(params),
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        1500,
        None::<&gio::Cancellable>,
    )
    .map_err(|_| Error::ManagerUnavailable)
}

fn driver_call(
    bus: &gio::DBusConnection,
    method: &str,
    params: &glib::Variant,
) -> Result<glib::Variant, Error> {
    call(bus, DBUS, DBUS_PATH, DBUS, method, params)
}

#[derive(PartialEq, Eq)]
struct Identity {
    bus_id: String,
    owner: String,
    manager_pid: u32,
    manager_start: u64,
    peer_pid: u32,
    peer_start: u64,
}

// Deliberately no Debug or public serialization of process/bus identifiers.
pub(super) struct LocalBus {
    pub(super) connection: gio::DBusConnection,
    socket: gio::Socket,
    endpoint: File,
    directory: File,
    uid: u32,
    identity: Identity,
}

impl LocalBus {
    pub(super) fn connect() -> Result<Self, Error> {
        let uid = nix::unistd::geteuid().as_raw();
        if uid == 0 || uid != nix::unistd::getuid().as_raw() {
            return Err(Error::IdentityUnverified);
        }
        let runtime = format!("/run/user/{uid}");
        validate_environment(
            &runtime,
            std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").as_deref(),
        )?;
        let directory = fixed_directory(uid)?;
        let endpoint = pin_endpoint(&directory, uid)?;
        let socket = connect_endpoint(&endpoint)?;
        // Validate the kernel peer before sending a D-Bus authentication byte.
        peer_identity(&socket, uid)?;
        let connection = authenticate(&socket)?;
        let identity = capture(&connection, &socket, uid)?;
        let result = Self {
            connection,
            socket,
            endpoint,
            directory,
            uid,
            identity,
        };
        result.revalidate()?;
        Ok(result)
    }

    pub(super) fn manager_owner(&self) -> &str {
        &self.identity.owner
    }

    pub(super) fn revalidate(&self) -> Result<(), Error> {
        validate_directory(&self.directory, self.uid, true)?;
        let directory = fixed_directory(self.uid)?;
        let a = directory
            .metadata()
            .map_err(|_| Error::IdentityUnverified)?;
        let b = self
            .directory
            .metadata()
            .map_err(|_| Error::IdentityUnverified)?;
        if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
            return Err(Error::OwnerChanged);
        }
        let current = pin_endpoint(&directory, self.uid)?;
        let a = current.metadata().map_err(|_| Error::IdentityUnverified)?;
        let b = self
            .endpoint
            .metadata()
            .map_err(|_| Error::IdentityUnverified)?;
        if (a.dev(), a.ino()) != (b.dev(), b.ino())
            || capture(&self.connection, &self.socket, self.uid)? != self.identity
        {
            return Err(Error::OwnerChanged);
        }
        Ok(())
    }
}

fn authenticate(socket: &gio::Socket) -> Result<gio::DBusConnection, Error> {
    let stream = socket.connection_factory_create_connection();
    let cancel = gio::Cancellable::new();
    let timer_cancel = cancel.clone();
    let (done, wait) = std::sync::mpsc::channel();
    let timer = std::thread::spawn(move || {
        if wait
            .recv_timeout(std::time::Duration::from_secs(3))
            .is_err()
        {
            timer_cancel.cancel();
        }
    });
    let observer = gio::DBusAuthObserver::new();
    observer.connect_allow_mechanism(|_, mechanism| mechanism == "EXTERNAL");
    let connection = gio::DBusConnection::new_sync(
        &stream,
        None,
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
            | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        Some(&observer),
        Some(&cancel),
    );
    let _ = done.send(());
    let _ = timer.join();
    let connection = connection.map_err(|_| Error::ManagerUnavailable)?;
    connection.set_exit_on_close(false);
    Ok(connection)
}

fn fixed_directory(uid: u32) -> Result<File, Error> {
    let mut directory = File::from(
        open(Path::new("/"), directory_flags(), Mode::empty())
            .map_err(|_| Error::IdentityUnverified)?,
    );
    for name in ["run", "user"] {
        validate_directory(&directory, 0, false)?;
        directory = File::from(
            openat(&directory, name, directory_flags(), Mode::empty())
                .map_err(|_| Error::IdentityUnverified)?,
        );
    }
    validate_directory(&directory, 0, false)?;
    directory = File::from(
        openat(
            &directory,
            uid.to_string().as_str(),
            directory_flags(),
            Mode::empty(),
        )
        .map_err(|_| Error::IdentityUnverified)?,
    );
    validate_directory(&directory, uid, true)?;
    Ok(directory)
}

fn directory_flags() -> OFlag {
    OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC
}

fn validate_directory(directory: &File, uid: u32, private: bool) -> Result<(), Error> {
    let meta = directory
        .metadata()
        .map_err(|_| Error::IdentityUnverified)?;
    if !meta.is_dir()
        || meta.uid() != uid
        || meta.mode() & 0o022 != 0
        || (private && meta.mode() & 0o777 != 0o700)
    {
        return Err(Error::IdentityUnverified);
    }
    Ok(())
}

fn validate_environment(
    runtime: &str,
    xdg: Option<&std::ffi::OsStr>,
    address: Option<&std::ffi::OsStr>,
) -> Result<(), Error> {
    // dconf may use GIO's session connection separately. Refuse any ambient
    // redirection there too; never repair or rewrite the user's environment.
    let expected = format!("unix:path={runtime}/bus");
    if xdg != Some(std::ffi::OsStr::new(runtime))
        || address.is_some_and(|v| v != std::ffi::OsStr::new(&expected))
    {
        return Err(Error::IdentityUnverified);
    }
    Ok(())
}

fn pin_endpoint(directory: &File, uid: u32) -> Result<File, Error> {
    let endpoint = File::from(
        openat(
            directory,
            "bus",
            OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Error::ManagerUnavailable)?,
    );
    let meta = endpoint.metadata().map_err(|_| Error::IdentityUnverified)?;
    if !meta.file_type().is_socket() || meta.uid() != uid || meta.nlink() != 1 {
        return Err(Error::IdentityUnverified);
    }
    Ok(endpoint)
}

fn connect_endpoint(endpoint: &File) -> Result<gio::Socket, Error> {
    let socket = gio::Socket::new(
        gio::SocketFamily::Unix,
        gio::SocketType::Stream,
        gio::SocketProtocol::Default,
    )
    .map_err(|_| Error::ManagerUnavailable)?;
    socket.set_timeout(2);
    // Linux procfs magic-link resolution follows the held socket inode, not a
    // replacement of /run/user/<uid>/bus after the O_PATH open.
    let path = format!("/proc/self/fd/{}", endpoint.as_raw_fd());
    let address = gio::UnixSocketAddress::new(Path::new(&path));
    SocketExt::connect(&socket, &address, None::<&gio::Cancellable>)
        .map_err(|_| Error::ManagerUnavailable)?;
    Ok(socket)
}

fn peer_identity(socket: &gio::Socket, uid: u32) -> Result<(u32, u64), Error> {
    let credentials = socket
        .credentials()
        .map_err(|_| Error::IdentityUnverified)?;
    if credentials
        .unix_user()
        .map_err(|_| Error::IdentityUnverified)?
        != uid
    {
        return Err(Error::IdentityUnverified);
    }
    let pid = u32::try_from(
        credentials
            .unix_pid()
            .map_err(|_| Error::IdentityUnverified)?,
    )
    .map_err(|_| Error::IdentityUnverified)?;
    Ok((pid, process_start(pid, uid)?))
}

fn process_start(pid: u32, uid: u32) -> Result<u64, Error> {
    if pid == 0 {
        return Err(Error::IdentityUnverified);
    }
    let file = File::open(format!("/proc/{pid}/stat")).map_err(|_| Error::IdentityUnverified)?;
    if file
        .metadata()
        .map_err(|_| Error::IdentityUnverified)?
        .uid()
        != uid
    {
        return Err(Error::IdentityUnverified);
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::IdentityUnverified)?;
    parse_start(&bytes, pid)
}

fn parse_start(bytes: &[u8], pid: u32) -> Result<u64, Error> {
    if bytes.len() > 4096 {
        return Err(Error::IdentityUnverified);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::IdentityUnverified)?;
    let (_, fields) = text.rsplit_once(") ").ok_or(Error::IdentityUnverified)?;
    if !text.starts_with(&format!("{pid} (")) {
        return Err(Error::IdentityUnverified);
    }
    let start = fields
        .split_whitespace()
        .nth(19)
        .ok_or(Error::IdentityUnverified)?
        .parse::<u64>()
        .map_err(|_| Error::IdentityUnverified)?;
    if start == 0 {
        return Err(Error::IdentityUnverified);
    }
    Ok(start)
}

fn capture(bus: &gio::DBusConnection, socket: &gio::Socket, uid: u32) -> Result<Identity, Error> {
    let (peer_pid, peer_start) = peer_identity(socket, uid)?;
    let id_reply = driver_call(bus, "GetId", &().to_variant())?;
    if id_reply.size() > 128 {
        return Err(Error::IdentityUnverified);
    }
    let (bus_id,) = id_reply
        .get::<(String,)>()
        .ok_or(Error::IdentityUnverified)?;
    if bus_id.len() != 32 || !bus_id.bytes().all(|v| v.is_ascii_hexdigit()) {
        return Err(Error::IdentityUnverified);
    }
    let owner_reply = driver_call(bus, "GetNameOwner", &(MANAGER,).to_variant())?;
    if owner_reply.size() > 512 {
        return Err(Error::IdentityUnverified);
    }
    let (owner,) = owner_reply
        .get::<(String,)>()
        .ok_or(Error::IdentityUnverified)?;
    if !owner.starts_with(':') || owner.len() > 255 || !gio::dbus_is_unique_name(&owner) {
        return Err(Error::IdentityUnverified);
    }
    let credentials = driver_call(bus, "GetConnectionCredentials", &(&owner,).to_variant())?;
    let manager_pid = parse_credentials(&credentials, uid)?;
    let manager_start = process_start(manager_pid, uid)?;
    Ok(Identity {
        bus_id,
        owner,
        manager_pid,
        manager_start,
        peer_pid,
        peer_start,
    })
}

fn parse_credentials(reply: &glib::Variant, uid: u32) -> Result<u32, Error> {
    if reply.type_().as_str() != "(a{sv})" || reply.size() > 4096 {
        return Err(Error::IdentityUnverified);
    }
    let dictionary = reply.child_value(0);
    if dictionary.n_children() > 32 {
        return Err(Error::IdentityUnverified);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut observed_uid = None;
    let mut pid = None;
    for entry in dictionary.iter() {
        let name = entry
            .child_value(0)
            .get::<String>()
            .ok_or(Error::IdentityUnverified)?;
        if !seen.insert(name.clone()) {
            return Err(Error::IdentityUnverified);
        }
        let value = entry
            .child_value(1)
            .get::<glib::Variant>()
            .ok_or(Error::IdentityUnverified)?;
        match name.as_str() {
            "UnixUserID" => {
                observed_uid = Some(value.get::<u32>().ok_or(Error::IdentityUnverified)?)
            }
            "ProcessID" => pid = Some(value.get::<u32>().ok_or(Error::IdentityUnverified)?),
            _ => (),
        }
    }
    if observed_uid != Some(uid) || pid.is_none_or(|v| v == 0) {
        return Err(Error::IdentityUnverified);
    }
    pid.ok_or(Error::IdentityUnverified)
}

#[cfg(test)]
mod tests;
