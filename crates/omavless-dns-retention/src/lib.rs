// SPDX-License-Identifier: MIT
//! Uninstalled descriptor-store wire boundary, not a host DNS broker.
//!
//! There is no environment/socket discovery or installed host-service consumer.
//! A trusted constructor accepts parts from the root broker's fixed admission;
//! tests supply a private notify socket and independent mock manager.
//! Metadata alone does NOT distinguish different open TUN descriptions. Exact
//! retention additionally requires an empty exclusive single-slot store and one
//! serialized insertion by the authenticated service main process.
#![cfg(target_os = "linux")]

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl, fstat, major, minor},
    net::{self, SendAncillaryBuffer, SendAncillaryMessage, SendFlags},
    pipe::{PipeFlags, pipe_with},
};
use serde::de::DeserializeOwned;
use std::{
    fmt,
    future::Future,
    io::IoSlice,
    mem::MaybeUninit,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    time::{Duration, Instant},
};
use zbus::{Message, blocking::Connection, zvariant::OwnedValue};

const SERVICE_PATH: &str = "/org/freedesktop/systemd1/unit/omavless_2ddns_2dbroker_2eservice";
const SERVICE_INTERFACE: &str = "org.freedesktop.systemd1.Service";
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const FD_NAME: &str = "omavless-tun-lease";
const STORE: &[u8] = b"FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0";
const REMOVE: &[u8] = b"FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease";
const MAX_REPLY: usize = 16 * 1024;

// Installed systemd1.Service.xml and systemd v261 dbus-service.c:
// a(suuutuusu): name,mode,dev-major,dev-minor,inode,rdev-major,rdev-minor,path,flags.
type Entry = (String, u32, u32, u32, u64, u32, u32, String, u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidState,
    InvalidDescriptor,
    ManagerUnavailable,
    InvalidReply,
    PolicyMismatch,
    ExistingStore,
    RetentionUnconfirmed,
    OutcomeUnknown,
    RecoveryRequired,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidState => "Descriptor retention transition is invalid",
            Self::InvalidDescriptor => "Descriptor retention requires a valid object",
            Self::ManagerUnavailable => "Descriptor manager is unavailable",
            Self::InvalidReply => "Descriptor manager response is invalid",
            Self::PolicyMismatch => "Descriptor retention policy is not established",
            Self::ExistingStore => "Existing retained state requires recovery",
            Self::RetentionUnconfirmed => "Descriptor retention is not confirmed",
            Self::OutcomeUnknown => "Descriptor store operation outcome is unknown",
            Self::RecoveryRequired => "Descriptor quarantine requires recovery",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Fresh,
    Retained,
    Quarantined,
    Released,
}

struct Identity {
    mode: u32,
    device: (u32, u32),
    inode: u64,
    rdevice: (u32, u32),
    flags: u32,
}
impl Identity {
    fn capture(fd: &OwnedFd) -> Result<Self, Error> {
        let stat = fstat(fd).map_err(|_| Error::InvalidDescriptor)?;
        let flags = fcntl_getfl(fd).map_err(|_| Error::InvalidDescriptor)?;
        Ok(Self {
            mode: stat.st_mode,
            device: (major(stat.st_dev), minor(stat.st_dev)),
            inode: stat.st_ino,
            rdevice: (major(stat.st_rdev), minor(stat.st_rdev)),
            // systemd masks RAW_O_LARGEFILE from its F_GETFL response too.
            flags: flags.bits() & !OFlags::LARGEFILE.bits(),
        })
    }
    fn matches(&self, entry: &Entry) -> bool {
        entry.0 == FD_NAME
            && entry.1 == self.mode
            && (entry.2, entry.3) == self.device
            && entry.4 == self.inode
            && (entry.5, entry.6) == self.rdevice
            && entry.8 == self.flags
            && entry.7.len() <= 4096
            && !entry.7.chars().any(char::is_control)
    }
}

/// Trusted caller must establish authenticated root-manager admission. No
/// Drop notification: uncertainty never releases
/// the manager's reference automatically. Local proof remains held on errors.
pub struct Retention {
    connection: Connection,
    manager_owner: String,
    notify: OwnedFd,
    timeout: Duration,
    deadline: Option<Instant>,
    state: State,
    proof: Option<OwnedFd>,
    identity: Option<Identity>,
}
impl fmt::Debug for Retention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Retention { .. }")
    }
}

impl Retention {
    /// Trusted in-process composition only, NOT caller-controlled IPC fields.
    /// The root broker must first authenticate its fixed enrollment/service,
    /// system-bus owner and protected notification socket. This constructor
    /// checks wire shape/service policy; it cannot authenticate those paths.
    /// No host/environment discovery or DNS effect occurs here.
    pub fn from_admitted_parts(
        connection: Connection,
        manager_owner: &str,
        notify: OwnedFd,
    ) -> Result<Self, Error> {
        Self::construct(connection, manager_owner, notify, None)
    }

    /// Include constructor policy checks in the caller's absolute operation budget.
    pub fn from_admitted_parts_with_deadline(
        connection: Connection,
        manager_owner: &str,
        notify: OwnedFd,
        deadline: Instant,
    ) -> Result<Self, Error> {
        Self::construct(connection, manager_owner, notify, Some(deadline))
    }

    fn construct(
        connection: Connection,
        manager_owner: &str,
        notify: OwnedFd,
        deadline: Option<Instant>,
    ) -> Result<Self, Error> {
        if manager_owner.len() > 255
            || zbus::names::UniqueName::try_from(manager_owner).is_err()
            || net::sockopt::socket_domain(&notify).map_err(|_| Error::PolicyMismatch)?
                != net::AddressFamily::UNIX
            || net::sockopt::socket_type(&notify).map_err(|_| Error::PolicyMismatch)?
                != net::SocketType::DGRAM
        {
            return Err(Error::PolicyMismatch);
        }
        rustix::io::fcntl_setfd(&notify, rustix::io::FdFlags::CLOEXEC)
            .map_err(|_| Error::PolicyMismatch)?;
        let mut retention = Self {
            connection,
            manager_owner: manager_owner.to_owned(),
            notify,
            timeout: Duration::from_secs(2),
            deadline: None,
            state: State::Fresh,
            proof: None,
            identity: None,
        };
        if let Some(deadline) = deadline {
            retention.set_deadline(deadline)?;
        }
        retention.policy()?;
        Ok(retention)
    }

    /// Trusted budget for a complete operation, not an interactive polkit timeout.
    /// Updating a budget never clears quarantine or authorizes an uncertain removal.
    pub fn set_deadline(&mut self, deadline: Instant) -> Result<(), Error> {
        if deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(30) {
            return Err(Error::PolicyMismatch);
        }
        self.deadline = Some(deadline);
        Ok(())
    }

    fn call_deadline(&self) -> Result<Instant, Error> {
        let now = Instant::now();
        let deadline = self
            .deadline
            .unwrap_or(now + self.timeout)
            .min(now + self.timeout);
        if deadline <= now {
            Err(Error::OutcomeUnknown)
        } else {
            Ok(deadline)
        }
    }

    /// One insertion only. An error keeps the local object and forbids retries or
    /// automatic removal. Caller must not begin DNS effects without Ok.
    pub fn retain(&mut self, proof: OwnedFd) -> Result<(), Error> {
        if self.state != State::Fresh {
            return Err(Error::InvalidState);
        }
        self.state = State::Quarantined;
        self.proof = Some(proof);
        self.identity = Some(Identity::capture(
            self.proof.as_ref().ok_or(Error::InvalidState)?,
        )?);
        self.policy()?;
        if self.count()? != 0 || !self.dump()?.is_empty() {
            return Err(Error::ExistingStore);
        }
        self.notification(STORE, self.proof.as_ref().map(AsFd::as_fd))?;
        self.barrier()?;
        self.verify_object()?;
        self.state = State::Retained;
        Ok(())
    }

    /// Startup/admission check; missing journal alone never proves this.
    /// No notification or descriptor removal. A nonempty store is quarantined.
    pub fn verify_fresh(&mut self) -> Result<(), Error> {
        if self.state != State::Fresh {
            return Err(Error::RecoveryRequired);
        }
        let result = (|| {
            self.policy()?;
            if self.count()? != 0 || !self.dump()?.is_empty() {
                return Err(Error::ExistingStore);
            }
            Ok(())
        })();
        if result.is_err() {
            self.state = State::Quarantined;
        }
        result
    }

    /// Recheck is not recovery and cannot reset a quarantined transaction.
    pub fn verify(&mut self) -> Result<(), Error> {
        if self.state != State::Retained {
            return Err(Error::RecoveryRequired);
        }
        if let Err(error) = self.verify_object() {
            self.state = State::Quarantined;
            return Err(error);
        }
        Ok(())
    }

    /// Called by the future trusted transaction owner for any unknown DNS write.
    /// No automatic release or fresh-readback promotion is available afterward.
    pub fn quarantine(&mut self) {
        self.state = State::Quarantined;
    }

    /// Fixed wire removal, NOT proof that DNS is clean. Future trusted caller
    /// must have independently proven a safe transaction boundary BEFORE this
    /// call. Deliberately unavailable after any uncertainty; no force option.
    pub fn release_after_proven_boundary(&mut self) -> Result<(), Error> {
        if self.state != State::Retained {
            return Err(Error::RecoveryRequired);
        }
        self.state = State::Quarantined;
        self.verify_object()?;
        self.notification(REMOVE, None)?;
        self.barrier()?;
        self.policy()?;
        if self.count()? != 0 || !self.dump()?.is_empty() {
            return Err(Error::OutcomeUnknown);
        }
        self.state = State::Released;
        self.proof = None;
        self.identity = None;
        Ok(())
    }

    fn policy(&self) -> Result<(), Error> {
        let capacity: u32 = self.property("FileDescriptorStoreMax")?;
        let preserve: String = self.property("FileDescriptorStorePreserve")?;
        let access: String = self.property("NotifyAccess")?;
        let main_pid: u32 = self.property("MainPID")?;
        if capacity != 1
            || preserve != "yes"
            || access != "main"
            || main_pid != rustix::process::getpid().as_raw_nonzero().get() as u32
        {
            return Err(Error::PolicyMismatch);
        }
        Ok(())
    }
    fn count(&self) -> Result<u32, Error> {
        let count: u32 = self.property("NFileDescriptorStore")?;
        if count > 1 {
            return Err(Error::InvalidReply);
        }
        Ok(count)
    }
    fn dump(&self) -> Result<Vec<Entry>, Error> {
        let reply = complete(
            self.call_deadline()?,
            self.connection.inner().call_method(
                Some(self.manager_owner.as_str()),
                SERVICE_PATH,
                Some(SERVICE_INTERFACE),
                "DumpFileDescriptorStore",
                &(),
            ),
        )
        .map_err(|_| Error::ManagerUnavailable)?;
        self.verify_sender(&reply)?;
        let entries: Vec<Entry> = bounded(&reply)?;
        if entries.len() > 1 {
            return Err(Error::InvalidReply);
        }
        Ok(entries)
    }
    fn property<T: TryFrom<OwnedValue>>(&self, name: &'static str) -> Result<T, Error> {
        let reply = complete(
            self.call_deadline()?,
            self.connection.inner().call_method(
                Some(self.manager_owner.as_str()),
                SERVICE_PATH,
                Some(PROPERTIES_INTERFACE),
                "Get",
                &(SERVICE_INTERFACE, name),
            ),
        )
        .map_err(|_| Error::ManagerUnavailable)?;
        self.verify_sender(&reply)?;
        T::try_from(bounded::<OwnedValue>(&reply)?).map_err(|_| Error::InvalidReply)
    }
    fn verify_sender(&self, reply: &Message) -> Result<(), Error> {
        if reply.header().sender().map(|name| name.as_str()) != Some(self.manager_owner.as_str()) {
            return Err(Error::ManagerUnavailable);
        }
        Ok(())
    }
    fn verify_object(&self) -> Result<(), Error> {
        self.policy()?;
        if self.count()? != 1 {
            return Err(Error::RetentionUnconfirmed);
        }
        let entries = self.dump()?;
        let identity = self.identity.as_ref().ok_or(Error::InvalidState)?;
        if entries.len() != 1 || !identity.matches(&entries[0]) {
            return Err(Error::RetentionUnconfirmed);
        }
        // Pin local proof too. Flags can change on a shared open description.
        let actual = Identity::capture(self.proof.as_ref().ok_or(Error::InvalidState)?)?;
        if !actual.matches(&entries[0]) {
            return Err(Error::RetentionUnconfirmed);
        }
        Ok(())
    }
    fn notification(&self, message: &[u8], fd: Option<BorrowedFd<'_>>) -> Result<(), Error> {
        let deadline = self.call_deadline()?;
        let fds: Vec<_> = fd.into_iter().collect();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(&fds)) {
            return Err(Error::InvalidDescriptor);
        }
        loop {
            wait(&self.notify, PollFlags::OUT, deadline)?;
            if Instant::now() >= deadline {
                return Err(Error::OutcomeUnknown);
            }
            match net::sendmsg(
                &self.notify,
                &[IoSlice::new(message)],
                &mut ancillary,
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            ) {
                Ok(length) if length == message.len() => return Ok(()),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                _ => return Err(Error::OutcomeUnknown),
            }
        }
    }
    fn barrier(&self) -> Result<(), Error> {
        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
            .map_err(|_| Error::OutcomeUnknown)?;
        self.notification(b"BARRIER=1", Some(writer.as_fd()))?;
        drop(writer);
        let deadline = self.call_deadline()?;
        loop {
            wait(&reader, PollFlags::IN | PollFlags::HUP, deadline)?;
            let mut byte = [0; 1];
            match rustix::io::read(&reader, &mut byte) {
                Ok(0) => return Ok(()),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                _ => return Err(Error::OutcomeUnknown),
            }
        }
    }
}

fn wait(fd: &OwnedFd, interest: PollFlags, deadline: Instant) -> Result<(), Error> {
    loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::OutcomeUnknown)?;
        let timeout = Timespec {
            tv_sec: left.as_secs() as i64,
            tv_nsec: left.subsec_nanos() as i64,
        };
        let mut fds = [PollFd::new(fd, interest)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(n) if n > 0 && fds[0].revents().intersects(interest) => return Ok(()),
            Err(rustix::io::Errno::INTR) => continue,
            _ => return Err(Error::OutcomeUnknown),
        }
    }
}

fn bounded<T: DeserializeOwned + zbus::zvariant::Type>(message: &Message) -> Result<T, Error> {
    // Acceptance bound, not upstream zbus's 128-MiB wire-allocation bound.
    if message.data().len() > MAX_REPLY || !message.data().fds().is_empty() {
        return Err(Error::InvalidReply);
    }
    message
        .body()
        .deserialize()
        .map_err(|_| Error::InvalidReply)
}
fn complete<T>(deadline: Instant, call: impl Future<Output = zbus::Result<T>>) -> Result<T, ()> {
    if Instant::now() >= deadline {
        return Err(());
    }
    futures_lite::future::block_on(futures_lite::future::race(
        async { call.await.map_err(|_| ()) },
        async {
            async_io::Timer::at(deadline).await;
            Err(())
        },
    ))
}

#[cfg(test)]
mod tests;
