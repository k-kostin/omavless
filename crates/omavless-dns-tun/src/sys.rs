//! Audited two-ioctl Linux leaf; no caller-selected opcode or pointer API.
use super::{DEVICE, Error, InterfaceInfo, Kernel, NamespaceId};
use rustix::fs::{FileType, fstat, major, minor};
use rustix::net::{AddressFamily, SocketFlags, SocketType};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

pub(super) struct Linux;

impl Kernel for Linux {
    fn current_namespace(&self) -> Result<OwnedFd, Error> {
        File::open("/proc/thread-self/ns/net")
            .map(Into::into)
            .map_err(|_| Error::KernelUnavailable)
    }

    fn namespace_id(&self, fd: &OwnedFd) -> Result<NamespaceId, Error> {
        let metadata = fstat(fd).map_err(|_| Error::KernelUnavailable)?;
        Ok(NamespaceId {
            device: metadata.st_dev,
            inode: metadata.st_ino,
        })
    }

    fn device(&self, fd: &OwnedFd) -> Result<(), Error> {
        let metadata = fstat(fd).map_err(|_| Error::InvalidDescriptor)?;
        if !is_tun_device(metadata.st_mode, metadata.st_rdev) {
            return Err(Error::InvalidDescriptor);
        }
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::CLOEXEC)
            .map_err(|_| Error::KernelUnavailable)
    }

    fn info(&self, fd: &OwnedFd) -> Result<InterfaceInfo, Error> {
        tun_info(fd)
    }

    fn device_namespace(&self, fd: &OwnedFd) -> Result<OwnedFd, Error> {
        device_netns(fd)
    }

    fn query_socket(&self) -> Result<OwnedFd, Error> {
        rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::DGRAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .map_err(|_| Error::KernelUnavailable)
    }

    fn index(&self, socket: &OwnedFd) -> Result<u32, Error> {
        rustix::net::netdevice::name_to_index(socket, DEVICE).map_err(|_| Error::Changed)
    }
}

fn is_tun_device(mode: u32, device: u64) -> bool {
    FileType::from_raw_mode(mode) == FileType::CharacterDevice
        && major(device) == 10
        && minor(device) == 200
}

// Linux native 64-bit ifreq is 40 bytes, with 8-byte alignment. TUNGETIFF writes
// the whole object despite its historical _IOR encoding naming unsigned int.
#[repr(C, align(8))]
struct IfReq([u8; 40]);

#[allow(unsafe_code)]
fn tun_info(fd: &OwnedFd) -> Result<InterfaceInfo, Error> {
    Linux.device(fd)?;
    let mut request = IfReq([0; 40]);
    // SAFETY: module is compiled only for native Linux aarch64/x86_64. The
    // caller gated a held character descriptor's dev_t=(10,200). Fixed
    // TUNGETIFF writes at most one correctly aligned 40-byte ifreq. Storage is
    // initialized, lives throughout the synchronous call and has no aliases.
    let result = unsafe {
        libc::ioctl(
            fd.as_raw_fd(),
            0x8004_54d2 as libc::c_ulong,
            request.0.as_mut_ptr(),
        )
    };
    if result != 0 {
        return Err(
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EBADFD) {
                Error::InvalidTun
            } else {
                Error::KernelUnavailable
            },
        );
    }
    let mut name = [0; 16];
    name.copy_from_slice(&request.0[..16]);
    Ok(InterfaceInfo {
        name,
        flags: u16::from_ne_bytes([request.0[16], request.0[17]]),
    })
}

#[allow(unsafe_code)]
fn device_netns(fd: &OwnedFd) -> Result<OwnedFd, Error> {
    Linux.device(fd)?;
    // SAFETY: fixed no-argument TUNGETDEVNETNS on the already gated held TUN
    // descriptor. The kernel returns either -1 or a new process-owned FD; no
    // caller memory is read/written. No generic ioctl interface is exposed.
    let result = unsafe { libc::ioctl(fd.as_raw_fd(), 0x54e3 as libc::c_ulong) };
    if result < 0 {
        return Err(Error::KernelUnavailable);
    }
    // SAFETY: successful TUNGETDEVNETNS returns a fresh descriptor whose sole
    // ownership transfers here exactly once. Drop closes it on every path.
    let namespace = unsafe { OwnedFd::from_raw_fd(result) };
    rustix::io::fcntl_setfd(&namespace, rustix::io::FdFlags::CLOEXEC)
        .map_err(|_| Error::KernelUnavailable)?;
    Ok(namespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_ifreq_layout_matches_supported_linux_abis() {
        assert_eq!(std::mem::size_of::<IfReq>(), 40);
        assert_eq!(std::mem::align_of::<IfReq>(), 8);
    }

    #[test]
    fn device_number_gate_rejects_colliding_driver_ioctls() {
        let tun = rustix::fs::makedev(10, 200);
        assert!(is_tun_device(libc::S_IFCHR, tun));
        for mode in [libc::S_IFREG, libc::S_IFSOCK, libc::S_IFBLK, libc::S_IFIFO] {
            assert!(!is_tun_device(mode, tun));
        }
        assert!(!is_tun_device(libc::S_IFCHR, rustix::fs::makedev(1, 3)));
        assert!(!is_tun_device(libc::S_IFCHR, rustix::fs::makedev(10, 201)));
    }
}
