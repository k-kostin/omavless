//! Fixed Linux namespace queries plus private inherited-descriptor ingress.
//!
//! Descriptor identity is not proof of a canonical host or trusted launch.
use crate::{errno::Errno, Result};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

/// Duplicate an inherited descriptor into a newly owned CLOEXEC descriptor.
/// The source is also marked CLOEXEC, but is NEVER adopted, closed or replaced.
/// `minimum` must exceed `source` so the new descriptor cannot use its slot.
///
/// This is a private bootstrap ingress, not authentication or a namespace
/// query. A concurrently closed/replaced source may change the object that
/// the kernel duplicates; callers must separately enforce original slot
/// custody and validate the returned object. No unsafe borrowed/owned wrapper
/// is constructed from the caller-supplied source number. Failed operations
/// are not retried and do not roll back an already set source CLOEXEC flag.
pub fn duplicate_inherited_cloexec(source: RawFd, minimum: RawFd) -> Result<OwnedFd> {
    duplicate_inherited_with(source, minimum, |fd, command, value| {
        // SAFETY: fixed scalar fcntl commands, no userspace pointer argument.
        // Any invalid source/minimum is handled by the kernel as an error.
        Errno::result(unsafe { libc::fcntl(fd, command, value) })
    })
}

fn duplicate_inherited_with(
    source: RawFd,
    minimum: RawFd,
    mut call: impl FnMut(RawFd, libc::c_int, libc::c_int) -> Result<libc::c_int>,
) -> Result<OwnedFd> {
    if source < 0 || minimum <= source {
        return Err(Errno::EINVAL);
    }
    let flags = call(source, libc::F_GETFD, 0)?;
    if flags < 0 || flags & !libc::FD_CLOEXEC != 0 {
        return Err(Errno::EINVAL);
    }
    if call(source, libc::F_SETFD, flags | libc::FD_CLOEXEC)? != 0 {
        return Err(Errno::EINVAL);
    }
    let duplicated = call(source, libc::F_DUPFD_CLOEXEC, minimum)?;
    if duplicated < minimum {
        return Err(Errno::EINVAL);
    }
    // SAFETY: only the fresh successful F_DUPFD_CLOEXEC result is adopted.
    // The kernel allocates an unused descriptor owned by this call, distinct
    // from source. The source number is never wrapped in OwnedFd/BorrowedFd.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicated) })
}

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
    fn inherited_ingress_invalid_arguments_call_nothing() {
        for (source, minimum) in [(-1, 6), (3, 3), (4, 3)] {
            assert_eq!(
                duplicate_inherited_with(source, minimum, |_, _, _| panic!("no call")).unwrap_err(),
                Errno::EINVAL
            );
        }
    }

    #[test]
    fn inherited_ingress_errors_are_one_attempt_and_never_own_source() {
        for error in [Errno::EBADF, Errno::EINTR, Errno::EPERM] {
            for cut in 0..3 {
                let mut calls = 0;
                let result = duplicate_inherited_with(3, 6, |fd, command, value| {
                    assert_eq!(fd, 3);
                    assert_eq!((command, value), [(libc::F_GETFD, 0),
                        (libc::F_SETFD, libc::FD_CLOEXEC), (libc::F_DUPFD_CLOEXEC, 6)][calls]);
                    let here = calls; calls += 1;
                    if here == cut { Err(error) } else { Ok(0) }
                });
                assert_eq!(result.unwrap_err(), error);
                assert_eq!(calls, cut + 1);
            }
        }
    }

    #[test]
    fn inherited_ingress_only_new_successful_descriptor_becomes_owned() {
        use std::os::fd::IntoRawFd;
        // Ordinary file ownership supplies the mock kernel return; no fcntl,
        // namespace ioctl, socket or inherited descriptor is exercised.
        let file = File::open("/dev/null").unwrap();
        let expected = file.as_raw_fd();
        let raw = file.into_raw_fd();
        let mut calls = 0;
        let duplicated = duplicate_inherited_with(0, 1, |fd, command, value| {
            assert_eq!(fd, 0);
            assert_eq!((command, value), [(libc::F_GETFD, 0),
                (libc::F_SETFD, libc::FD_CLOEXEC), (libc::F_DUPFD_CLOEXEC, 1)][calls]);
            calls += 1;
            Ok(if calls == 3 { raw } else { 0 })
        }).unwrap();
        assert_eq!(duplicated.as_raw_fd(), expected);
        assert_eq!(calls, 3);
    }

    #[test]
    fn inherited_ingress_invalid_flags_status_and_dup_result_refuse() {
        for values in [vec![-1], vec![2], vec![0, 1], vec![0, 0, 5]] {
            let mut calls = 0;
            let result = duplicate_inherited_with(3, 6, |_, _, _| {
                let result = values[calls]; calls += 1; Ok(result)
            });
            assert_eq!(result.unwrap_err(), Errno::EINVAL);
            assert_eq!(calls, values.len());
        }
    }

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
