//! Fixed managed-TUN descriptor admission, not a DNS writer or privileged API.
//!
//! Holding a validated original single-queue FD protects against ordinary close
//! and reuse. It does not defend against administrators/CAP_NET_ADMIN, establish
//! a pristine resolved baseline, authenticate peers, or make D-Bus writes atomic.
#![deny(unsafe_code)]

use std::fmt;
use std::os::fd::OwnedFd;

#[cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
mod sys;

const DEVICE: &str = "Meta";
const FORBIDDEN_FLAGS: u16 = 0x0100 | 0x0200 | 0x0400 | 0x0800;

/// Credential-free fixed classifications; kernel text and descriptor values are
/// deliberately never included in public errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedPlatform,
    InvalidDescriptor,
    InvalidTun,
    NamespaceMismatch,
    Changed,
    KernelUnavailable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedPlatform => "TUN admission is unavailable on this platform.",
            Self::InvalidDescriptor => "A managed TUN descriptor is required.",
            Self::InvalidTun => "The supplied TUN does not meet managed-link policy.",
            Self::NamespaceMismatch => "The TUN network namespace does not match.",
            Self::Changed => "The managed TUN identity changed.",
            Self::KernelUnavailable => "The kernel TUN identity check is unavailable.",
        })
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Eq, PartialEq)]
struct NamespaceId {
    device: u64,
    inode: u64,
}

struct InterfaceInfo {
    name: [u8; 16],
    flags: u16,
}

fn validate_info(info: &InterfaceInfo) -> Result<(), Error> {
    let end = info
        .name
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::InvalidTun)?;
    if &info.name[..end] != DEVICE.as_bytes()
        || info.flags & 0x000f != 1
        || info.flags & FORBIDDEN_FLAGS != 0
    {
        return Err(Error::InvalidTun);
    }
    // TUNGETIFF's NOFILTER bit aliases NO_PI: do not infer packet framing here.
    Ok(())
}

trait Kernel {
    fn current_namespace(&self) -> Result<OwnedFd, Error>;
    fn namespace_id(&self, fd: &OwnedFd) -> Result<NamespaceId, Error>;
    fn device(&self, fd: &OwnedFd) -> Result<(), Error>;
    fn info(&self, fd: &OwnedFd) -> Result<InterfaceInfo, Error>;
    fn device_namespace(&self, fd: &OwnedFd) -> Result<OwnedFd, Error>;
    fn query_socket(&self) -> Result<OwnedFd, Error>;
    fn index(&self, socket: &OwnedFd) -> Result<u32, Error>;
}

/// Retains the received object and its original namespace. No raw descriptor,
/// namespace, interface name or caller-selected target is exposed.
pub struct HeldTun {
    descriptor: OwnedFd,
    namespace: OwnedFd,
    query_socket: OwnedFd,
    index: u32,
}

impl fmt::Debug for HeldTun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HeldTun { .. }")
    }
}

impl HeldTun {
    /// Consumes a received descriptor. Refusal closes it; success retains it.
    /// The caller must provide authenticated ancillary transport separately.
    pub fn admit(descriptor: OwnedFd) -> Result<Self, Error> {
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ))]
        {
            Self::admit_with(descriptor, &sys::Linux)
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86_64")
        )))]
        {
            drop(descriptor);
            Err(Error::UnsupportedPlatform)
        }
    }

    fn admit_with(descriptor: OwnedFd, kernel: &impl Kernel) -> Result<Self, Error> {
        // Gate device type/major/minor BEFORE issuing any driver ioctl.
        kernel.device(&descriptor)?;
        let namespace = kernel.current_namespace()?;
        validate_info(&kernel.info(&descriptor)?)?;
        let device_namespace = kernel.device_namespace(&descriptor)?;
        if kernel.namespace_id(&device_namespace)? != kernel.namespace_id(&namespace)? {
            return Err(Error::NamespaceMismatch);
        }
        drop(device_namespace);
        let query_socket = kernel.query_socket()?;
        let index = kernel.index(&query_socket)?;
        if index == 0 || index > i32::MAX as u32 {
            return Err(Error::InvalidTun);
        }
        let held = Self {
            descriptor,
            namespace,
            query_socket,
            index,
        };
        held.recheck_with(kernel)?;
        Ok(held)
    }

    /// Revalidates current thread namespace, actual held object, and fixed-name
    /// lookup before a later effect. This is a check, not an atomic D-Bus lease.
    pub fn recheck(&self) -> Result<(), Error> {
        #[cfg(all(
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86_64")
        ))]
        {
            self.recheck_with(&sys::Linux)
        }
        #[cfg(not(all(
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86_64")
        )))]
        Err(Error::UnsupportedPlatform)
    }

    fn recheck_with(&self, kernel: &impl Kernel) -> Result<(), Error> {
        kernel.device(&self.descriptor)?;
        let current = kernel.current_namespace()?;
        let expected = kernel.namespace_id(&self.namespace)?;
        if kernel.namespace_id(&current)? != expected {
            return Err(Error::NamespaceMismatch);
        }
        validate_info(&kernel.info(&self.descriptor)?)?;
        let actual = kernel.device_namespace(&self.descriptor)?;
        if kernel.namespace_id(&actual)? != expected {
            return Err(Error::NamespaceMismatch);
        }
        if kernel.index(&self.query_socket)? != self.index {
            return Err(Error::Changed);
        }
        Ok(())
    }

    /// Internal broker target derived from the admitted held object. Call
    /// recheck immediately before and after effects; never cache past this lease.
    pub fn interface_index(&self) -> u32 {
        self.index
    }
}

#[cfg(test)]
mod tests;
