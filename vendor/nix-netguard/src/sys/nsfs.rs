//! Fixed, read-only Linux namespace descriptor queries.
//!
//! Descriptor identity is not proof of a canonical host or trusted launch.
use crate::{errno::Errno, Result};
use std::os::fd::{AsRawFd, BorrowedFd};

/// Kernel namespace kinds understood by this API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespaceType {
    /// Network namespace.
    Network,
    /// User namespace.
    User,
    /// PID namespace.
    Pid,
    /// Mount namespace.
    Mount,
    /// UTS namespace.
    Uts,
    /// IPC namespace.
    Ipc,
    /// Cgroup namespace.
    Cgroup,
    /// Time namespace.
    Time,
}

/// Query `NS_GET_NSTYPE` on a borrowed namespace descriptor.
/// Unknown future types return `EOPNOTSUPP`; syscall errors are preserved.
///
/// A descriptor cannot be closed by moving its owner while this borrow is used.
/// ```compile_fail,E0505
/// use std::{fs::File, os::fd::AsFd};
/// let owner = File::open("/dev/null").unwrap();
/// let borrowed = owner.as_fd();
/// drop(owner);
/// let _ = nix::sys::nsfs::namespace_type(borrowed);
/// ```
pub fn namespace_type(fd: BorrowedFd<'_>) -> Result<NamespaceType> {
    // SAFETY: this fixed no-argument ioctl neither reads nor writes userspace
    // memory. The live descriptor is borrowed for the complete call.
    namespace_type_result(Errno::result(unsafe {
        libc::ioctl(fd.as_raw_fd(), libc::NS_GET_NSTYPE)
    }))
}

fn namespace_type_result(result: Result<libc::c_int>) -> Result<NamespaceType> {
    match result? {
        libc::CLONE_NEWNET => Ok(NamespaceType::Network),
        libc::CLONE_NEWUSER => Ok(NamespaceType::User),
        libc::CLONE_NEWPID => Ok(NamespaceType::Pid),
        libc::CLONE_NEWNS => Ok(NamespaceType::Mount),
        libc::CLONE_NEWUTS => Ok(NamespaceType::Uts),
        libc::CLONE_NEWIPC => Ok(NamespaceType::Ipc),
        libc::CLONE_NEWCGROUP => Ok(NamespaceType::Cgroup),
        libc::CLONE_NEWTIME => Ok(NamespaceType::Time),
        _ => Err(Errno::EOPNOTSUPP),
    }
}

/// Query the 64-bit namespace identifier using `NS_GET_ID`.
/// This never substitutes a procfs inode or a namespace-relative netlink ID.
pub fn namespace_id(fd: BorrowedFd<'_>) -> Result<u64> {
    namespace_id_with(fd, |fd, value| {
        // SAFETY: fixed read-only operation; the initialized, aligned exclusive
        // output and the original borrowed descriptor stay live for this call.
        Errno::result(unsafe {
            libc::ioctl(fd.as_raw_fd(), libc::NS_GET_ID, value as *mut u64)
        })
    })
}

// Private seam: tests drive the SAME initialization and validation used above,
// without invoking ioctl, opening a namespace, or changing any host state.
fn namespace_id_with(
    fd: BorrowedFd<'_>,
    call: impl FnOnce(BorrowedFd<'_>, &mut u64) -> Result<libc::c_int>,
) -> Result<u64> {
    let mut value = 0_u64;
    let result = call(fd, &mut value)?;
    if result != 0 || value == 0 {
        return Err(Errno::EINVAL);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, os::fd::AsFd};

    #[test]
    fn namespace_api_type_results_preserve_errors_and_reject_unknown() {
        for error in [Errno::ENOTTY, Errno::EBADF, Errno::EPERM, Errno::EINTR] {
            assert_eq!(namespace_type_result(Err(error)), Err(error));
        }
        for value in
            [0, -2, 1, i32::MAX, libc::CLONE_NEWNET | libc::CLONE_NEWUSER]
        {
            assert_eq!(
                namespace_type_result(Ok(value)),
                Err(Errno::EOPNOTSUPP)
            );
        }
        for (raw, kind) in [
            (libc::CLONE_NEWNET, NamespaceType::Network),
            (libc::CLONE_NEWUSER, NamespaceType::User),
            (libc::CLONE_NEWPID, NamespaceType::Pid),
            (libc::CLONE_NEWNS, NamespaceType::Mount),
            (libc::CLONE_NEWUTS, NamespaceType::Uts),
            (libc::CLONE_NEWIPC, NamespaceType::Ipc),
            (libc::CLONE_NEWCGROUP, NamespaceType::Cgroup),
            (libc::CLONE_NEWTIME, NamespaceType::Time),
        ] {
            assert_eq!(namespace_type_result(Ok(raw)), Ok(kind));
        }
    }

    #[test]
    fn namespace_api_id_single_call_initialized_output_and_error_precedence() {
        let owner = std::io::stdin(); // Borrow only; no read, open, or syscall.
        for error in [Errno::ENOTTY, Errno::EBADF, Errno::EPERM, Errno::EINTR] {
            assert_eq!(
                namespace_id_with(owner.as_fd(), |fd, output| {
                    assert_eq!(fd.as_raw_fd(), owner.as_raw_fd());
                    assert_eq!(*output, 0);
                    *output = 42; // Even a partly written error is not success.
                    Err(error)
                }),
                Err(error)
            );
        }
        for status in [-2, -1, 1, i32::MAX] {
            assert_eq!(
                namespace_id_with(owner.as_fd(), |_, output| {
                    *output = 42;
                    Ok(status)
                }),
                Err(Errno::EINVAL)
            );
        }
        assert_eq!(
            namespace_id_with(owner.as_fd(), |_, _| Ok(0)),
            Err(Errno::EINVAL)
        );
        for value in [1, u64::MAX] {
            assert_eq!(
                namespace_id_with(owner.as_fd(), |_, output| {
                    *output = value;
                    Ok(0)
                }),
                Ok(value)
            );
        }
    }

    #[test]
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn namespace_api_linux_generic_ioctl_abi() {
        assert_eq!(std::mem::size_of::<u64>(), 8);
        assert_eq!(std::mem::align_of::<u64>(), 8);
        assert_eq!(libc::NS_GET_NSTYPE as u64, 0xb703);
        assert_eq!(libc::NS_GET_ID as u64, 0x8008b70d);
        assert_eq!(crate::request_code_read!(0xb7, 13, 8) as u64, 0x8008b70d);
    }

    #[test]
    fn ordinary_files_are_not_namespace_descriptors() {
        let file = File::open("/dev/null").unwrap();
        assert_eq!(namespace_type(file.as_fd()), Err(Errno::ENOTTY));
        assert_eq!(namespace_id(file.as_fd()), Err(Errno::ENOTTY));
    }

    #[test]
    fn current_network_and_pid_kinds_differ() {
        let net = File::open("/proc/thread-self/ns/net").unwrap();
        let pid = File::open("/proc/thread-self/ns/pid").unwrap();
        assert_eq!(namespace_type(net.as_fd()), Ok(NamespaceType::Network));
        assert_eq!(namespace_type(pid.as_fd()), Ok(NamespaceType::Pid));
        // Old kernels are unsupported, not a fabricated identity.
        match namespace_id(net.as_fd()) {
            Ok(id) => assert_ne!(id, 0),
            Err(error) => assert_eq!(error, Errno::ENOTTY),
        }
    }
}
