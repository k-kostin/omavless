// SPDX-License-Identifier: MIT
//! Diagnostic continuity only: no trusted-process, session or write authority.
use crate::{Error, local_bus};
use gio::glib::{self, prelude::ToVariant};
use gio::prelude::*;
use nix::fcntl::{FcntlArg, FdFlag, OFlag, fcntl, open, openat};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::sys::socket::{UnixAddr, getpeername, getsockopt, sockopt::PeerPidfd};
use nix::sys::stat::Mode;
use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

#[derive(PartialEq, Eq)]
struct Inode(u64, u64);
fn inode(file: &File) -> Result<Inode, Error> {
    let m = file.metadata().map_err(|_| Error::IdentityUnverified)?;
    Ok(Inode(m.dev(), m.ino()))
}

// Deliberately private: no arbitrary host path or configurable owner interface.
fn child_directory(parent: &File, name: &str, uid: u32) -> Result<File, Error> {
    let file = File::from(
        openat(parent, name, local_bus::directory_flags(), Mode::empty())
            .map_err(|_| Error::IdentityUnverified)?,
    );
    local_bus::validate_directory(&file, uid, false)?;
    Ok(file)
}
fn socket_file(parent: &File, name: &str, uid: u32) -> Result<File, Error> {
    let file = File::from(
        openat(
            parent,
            name,
            OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Error::IdentityUnverified)?,
    );
    let m = file.metadata().map_err(|_| Error::IdentityUnverified)?;
    if !m.file_type().is_socket() || m.uid() != uid || m.nlink() != 1 {
        return Err(Error::IdentityUnverified);
    }
    Ok(file)
}

struct Endpoints {
    directories: Vec<File>,
    system_bus: File,
    private: File,
}
impl Endpoints {
    fn open(uid: u32) -> Result<Self, Error> {
        let root = File::from(
            open(Path::new("/"), local_bus::directory_flags(), Mode::empty())
                .map_err(|_| Error::IdentityUnverified)?,
        );
        local_bus::validate_directory(&root, 0, false)?;
        let run = child_directory(&root, "run", 0)?;
        let dbus = child_directory(&run, "dbus", 0)?;
        let system_bus = socket_file(&dbus, "system_bus_socket", 0)?;
        let runtime = local_bus::fixed_directory(uid)?;
        let systemd = child_directory(&runtime, "systemd", uid)?;
        let private = socket_file(&systemd, "private", uid)?;
        Ok(Self {
            directories: vec![root, run, dbus, runtime, systemd],
            system_bus,
            private,
        })
    }
    fn identities(&self) -> Result<Vec<Inode>, Error> {
        self.directories
            .iter()
            .chain([&self.system_bus, &self.private])
            .map(inode)
            .collect()
    }
}

fn scalar_pid(reply: &glib::Variant) -> Result<u32, Error> {
    if reply.type_().as_str() != "(v)" || reply.size() > 32 {
        return Err(Error::IdentityUnverified);
    }
    let (value,) = reply
        .get::<(glib::Variant,)>()
        .ok_or(Error::IdentityUnverified)?;
    let pid = value.get::<u32>().ok_or(Error::IdentityUnverified)?;
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(Error::IdentityUnverified);
    }
    Ok(pid)
}

#[derive(PartialEq, Eq)]
struct Lifetime {
    pid: u32,
    start: u64,
}

// The kernel handle pins the connected socket's recorded peer process, not
// necessarily the process that later writes on an inherited listener.
struct UnverifiedManagerPin(OwnedFd);
impl UnverifiedManagerPin {
    fn capture(socket: &impl AsFd) -> Result<Self, Error> {
        getpeername::<UnixAddr>(socket.as_fd().as_raw_fd())
            .map_err(|_| Error::IdentityUnverified)?;
        let fd = getsockopt(socket, PeerPidfd).map_err(|_| Error::IdentityUnverified)?;
        let flags = fcntl(&fd, FcntlArg::F_GETFD).map_err(|_| Error::IdentityUnverified)?;
        if flags & FdFlag::FD_CLOEXEC.bits() == 0 {
            return Err(Error::IdentityUnverified);
        }
        let pin = Self(fd);
        pin.require_alive_now()?;
        Ok(pin)
    }

    fn require_alive_now(&self) -> Result<(), Error> {
        let mut fds = [PollFd::new(self.0.as_fd(), PollFlags::POLLIN)];
        match poll(&mut fds, PollTimeout::ZERO).map_err(|_| Error::IdentityUnverified)? {
            0 if fds[0].revents().is_some_and(|events| events.is_empty()) => Ok(()),
            _ => Err(Error::IdentityUnverified),
        }
    }
}

trait Facts {
    fn main_pid(&mut self) -> Result<u32, Error>;
    fn lifetime(&mut self, pid: u32) -> Result<Lifetime, Error>;
    fn private_peer(&mut self) -> Result<(u32, u32), Error>;
    fn peer_alive_now(&mut self) -> Result<(), Error>;
    fn revalidate_endpoints(&mut self) -> Result<(), Error>;
}
fn observe(facts: &mut impl Facts, uid: u32) -> Result<(), Error> {
    let first = facts.main_pid()?;
    if first == 0 || first > i32::MAX as u32 {
        return Err(Error::IdentityUnverified);
    }
    let lifetime = facts.lifetime(first)?;
    if lifetime.pid != first || lifetime.start == 0 {
        return Err(Error::IdentityUnverified);
    }
    let (peer, peer_uid) = facts.private_peer()?;
    if peer != first || peer_uid != uid {
        return Err(Error::IdentityUnverified);
    }
    facts.peer_alive_now()?;
    facts.revalidate_endpoints()?;
    if facts.main_pid()? != first || facts.lifetime(first)? != lifetime {
        return Err(Error::OwnerChanged);
    }
    facts.revalidate_endpoints()?;
    facts.peer_alive_now()
}

struct Host {
    uid: u32,
    endpoints: Endpoints,
    connection: gio::DBusConnection,
    private_socket: Option<gio::Socket>,
    private_peer_pin: Option<UnverifiedManagerPin>,
}
impl Facts for Host {
    fn main_pid(&mut self) -> Result<u32, Error> {
        let path = format!(
            "/org/freedesktop/systemd1/unit/user_40{}_2eservice",
            self.uid
        );
        let reply = local_bus::call(
            &self.connection,
            "org.freedesktop.systemd1",
            &path,
            "org.freedesktop.DBus.Properties",
            "Get",
            &("org.freedesktop.systemd1.Service", "MainPID").to_variant(),
        )?;
        scalar_pid(&reply)
    }
    fn lifetime(&mut self, pid: u32) -> Result<Lifetime, Error> {
        // This detects observed restart/reuse, not pidfd authority or ABA.
        Ok(Lifetime {
            pid,
            start: local_bus::process_start(pid, self.uid)?,
        })
    }
    fn private_peer(&mut self) -> Result<(u32, u32), Error> {
        let socket = local_bus::connect_endpoint(&self.endpoints.private)?;
        let pin = UnverifiedManagerPin::capture(&socket)?;
        let credentials = socket
            .credentials()
            .map_err(|_| Error::IdentityUnverified)?;
        let pid = u32::try_from(
            credentials
                .unix_pid()
                .map_err(|_| Error::IdentityUnverified)?,
        )
        .map_err(|_| Error::IdentityUnverified)?;
        let uid = credentials
            .unix_user()
            .map_err(|_| Error::IdentityUnverified)?;
        // Keep the connection alive; send no AUTH, Hello or method on it.
        self.private_peer_pin = Some(pin);
        self.private_socket = Some(socket);
        Ok((pid, uid))
    }
    fn peer_alive_now(&mut self) -> Result<(), Error> {
        self.private_peer_pin
            .as_ref()
            .ok_or(Error::IdentityUnverified)?
            .require_alive_now()
    }
    fn revalidate_endpoints(&mut self) -> Result<(), Error> {
        let fresh = Endpoints::open(self.uid)?;
        if fresh.identities()? != self.endpoints.identities()? {
            return Err(Error::OwnerChanged);
        }
        Ok(())
    }
}

/// No settings/environment/activation read. Success is observational, never a permit.
pub fn probe_manager_continuity_read_only() -> Result<(), Error> {
    let uid = nix::unistd::geteuid().as_raw();
    if uid == 0 || uid != nix::unistd::getuid().as_raw() {
        return Err(Error::IdentityUnverified);
    }
    validate_ambient(
        std::env::var_os("DBUS_SYSTEM_BUS_ADDRESS").as_deref(),
        std::env::var_os("G_DBUS_DEBUG").as_deref(),
    )?;
    let endpoints = Endpoints::open(uid)?;
    let socket = local_bus::connect_endpoint(&endpoints.system_bus)?;
    let connection = local_bus::authenticate(&socket)?;
    let mut host = Host {
        uid,
        endpoints,
        connection,
        private_socket: None,
        private_peer_pin: None,
    };
    let result = observe(&mut host, uid);
    // No raw GIO error or identity is exposed by the check binary.
    let _ = socket.close();
    result
}

fn validate_ambient(
    address: Option<&std::ffi::OsStr>,
    debug: Option<&std::ffi::OsStr>,
) -> Result<(), Error> {
    // Refuse GIO wire-debug output before constructing a connection: scalar
    // and authentication identifiers must not leak through library logging.
    if debug.is_some() || address.is_some_and(|v| v != "unix:path=/run/dbus/system_bus_socket") {
        return Err(Error::IdentityUnverified);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
