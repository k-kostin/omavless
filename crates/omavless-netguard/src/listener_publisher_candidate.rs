//! Inactive first-publication candidate for the fixed K1 listener.
//! The socket is bound while its new directory is private (0700), and group
//! access is published only after ownership, mode and inode checks. It has no
//! production caller. Restart/old-socket retirement, package-group lookup,
//! namespace provenance and installed service behavior remain separate gates.

use crate::listener_admission::AdmittedListener;
use crate::package_group_candidate::PackageGroup;
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, open, openat};
use nix::sys::socket::{
    AddressFamily, Backlog, SockFlag, SockType, UnixAddr, bind, listen, socket,
};
use nix::sys::stat::{FchmodatFlags, Mode, fchmod, fchmodat, mkdirat};
use nix::unistd::{Gid, fchown, fchownat};
use std::fs::File;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixListener;
use std::path::Path;

const DIR: &str = "omavless-netguard";
const LEAF: &str = "control.sock";
const FIXED_PATH: &str = "/run/omavless-netguard/control.sock";
const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const SOCKET_PATH: OFlag = OFlag::O_PATH
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublishError {
    UnsafeOrExisting,
    Ambiguous,
}

type Result<T> = std::result::Result<T, PublishError>;

fn same_entry(a: &File, b: &File) -> Result<()> {
    let a = a.metadata().map_err(|_| PublishError::Ambiguous)?;
    let b = b.metadata().map_err(|_| PublishError::Ambiguous)?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Err(PublishError::Ambiguous);
    }
    Ok(())
}

fn parent_is_safe(parent: &File, owner: (u32, u32)) -> Result<()> {
    let meta = parent
        .metadata()
        .map_err(|_| PublishError::UnsafeOrExisting)?;
    if !meta.is_dir() || (meta.uid(), meta.gid()) != owner || meta.mode() & 0o022 != 0 {
        return Err(PublishError::UnsafeOrExisting);
    }
    Ok(())
}

fn private_directory(directory: &File, owner: (u32, u32)) -> Result<()> {
    let meta = directory.metadata().map_err(|_| PublishError::Ambiguous)?;
    if !meta.is_dir() || (meta.uid(), meta.gid()) != owner || meta.mode() & 0o7777 != 0o700 {
        return Err(PublishError::Ambiguous);
    }
    Ok(())
}

fn socket_entry(directory: &File, owner_uid: u32) -> Result<File> {
    let entry = File::from(
        openat(directory, LEAF, SOCKET_PATH, Mode::empty()).map_err(|_| PublishError::Ambiguous)?,
    );
    let meta = entry.metadata().map_err(|_| PublishError::Ambiguous)?;
    if !meta.file_type().is_socket() || meta.nlink() != 1 || meta.uid() != owner_uid {
        return Err(PublishError::Ambiguous);
    }
    Ok(entry)
}

fn still_named(parent: &File, directory: &File, entry: &File, owner_uid: u32) -> Result<()> {
    let current_directory = File::from(
        openat(parent, DIR, DIRECTORY, Mode::empty()).map_err(|_| PublishError::Ambiguous)?,
    );
    same_entry(directory, &current_directory)?;
    let current_entry = socket_entry(directory, owner_uid)?;
    same_entry(entry, &current_entry)
}

fn bind_private(path: &Path) -> Result<UnixListener> {
    let fd: OwnedFd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None,
    )
    .map_err(|_| PublishError::Ambiguous)?;
    let address = UnixAddr::new(path).map_err(|_| PublishError::Ambiguous)?;
    bind(fd.as_raw_fd(), &address).map_err(|_| PublishError::Ambiguous)?;
    listen(&fd, Backlog::new(8).map_err(|_| PublishError::Ambiguous)?)
        .map_err(|_| PublishError::Ambiguous)?;
    Ok(UnixListener::from(fd))
}

/// Create a *new* first-publication listener only. The GID comes from a
/// pinned, unique local package-group entry. Nothing calls this in the
/// shipped service or product runtime. An existing directory is never adopted
/// or removed, even if it appears to contain an old socket.
pub(crate) fn publish_fixed_first() -> Result<AdmittedListener> {
    let group = PackageGroup::open_fixed().map_err(|_| PublishError::UnsafeOrExisting)?;
    group
        .validate()
        .map_err(|_| PublishError::UnsafeOrExisting)?;
    let root = File::from(
        open("/", DIRECTORY, Mode::empty()).map_err(|_| PublishError::UnsafeOrExisting)?,
    );
    parent_is_safe(&root, (0, 0))?;
    let run = File::from(
        openat(&root, "run", DIRECTORY, Mode::empty())
            .map_err(|_| PublishError::UnsafeOrExisting)?,
    );
    parent_is_safe(&run, (0, 0))?;
    publish_under(
        run,
        Path::new(FIXED_PATH),
        (0, 0),
        group.gid(),
        || group.validate().is_ok(),
        || group.validate().is_ok(),
    )
}

#[cfg(test)]
pub(crate) fn publish_test_parent(
    parent: File,
    path: &Path,
    owner: (u32, u32),
    group: u32,
    before_access: impl FnOnce() -> bool,
) -> Result<AdmittedListener> {
    publish_under(parent, path, owner, group, before_access, || true)
}

#[cfg(test)]
pub(crate) fn publish_test_with_group(
    parent: File,
    path: &Path,
    owner: (u32, u32),
    group: &PackageGroup,
    before_access: impl FnOnce(),
    before_publish: impl FnOnce(),
) -> Result<AdmittedListener> {
    group
        .validate()
        .map_err(|_| PublishError::UnsafeOrExisting)?;
    publish_under(
        parent,
        path,
        owner,
        group.gid(),
        || {
            before_access();
            group.validate().is_ok()
        },
        || {
            before_publish();
            group.validate().is_ok()
        },
    )
}

fn publish_under(
    parent: File,
    path: &Path,
    owner: (u32, u32),
    group: u32,
    before_access: impl FnOnce() -> bool,
    before_publish: impl FnOnce() -> bool,
) -> Result<AdmittedListener> {
    parent_is_safe(&parent, owner)?;
    if path.file_name().and_then(|name| name.to_str()) != Some(LEAF)
        || path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != Some(DIR)
    {
        return Err(PublishError::UnsafeOrExisting);
    }
    let named_parent = File::from(
        open(
            path.parent()
                .and_then(Path::parent)
                .ok_or(PublishError::UnsafeOrExisting)?,
            DIRECTORY,
            Mode::empty(),
        )
        .map_err(|_| PublishError::UnsafeOrExisting)?,
    );
    same_entry(&parent, &named_parent)?;

    match mkdirat(&parent, DIR, Mode::from_bits_truncate(0o700)) {
        Ok(()) => (),
        Err(Errno::EEXIST) => return Err(PublishError::UnsafeOrExisting),
        Err(_) => return Err(PublishError::Ambiguous),
    }
    // From here an uncertain failure retains private artifacts for explicit
    // recovery. Unlinking could erase a replacement created by another owner.
    let directory = File::from(
        openat(&parent, DIR, DIRECTORY, Mode::empty()).map_err(|_| PublishError::Ambiguous)?,
    );
    private_directory(&directory, owner)?;
    let listener = bind_private(path)?;
    let entry = socket_entry(&directory, owner.0)?;
    still_named(&parent, &directory, &entry, owner.0)?;
    private_directory(&directory, owner)?;
    if !before_access() {
        return Err(PublishError::Ambiguous);
    }

    // Prepare the socket while the directory is inaccessible to its group.
    still_named(&parent, &directory, &entry, owner.0)?;
    fchownat(
        &directory,
        LEAF,
        None,
        Some(Gid::from_raw(group)),
        AtFlags::AT_SYMLINK_NOFOLLOW,
    )
    .map_err(|_| PublishError::Ambiguous)?;
    still_named(&parent, &directory, &entry, owner.0)?;
    fchmodat(
        &directory,
        LEAF,
        Mode::from_bits_truncate(0o660),
        FchmodatFlags::NoFollowSymlink,
    )
    .map_err(|_| PublishError::Ambiguous)?;
    let after = socket_entry(&directory, owner.0)?;
    same_entry(&entry, &after)?;
    let meta = after.metadata().map_err(|_| PublishError::Ambiguous)?;
    if meta.gid() != group || meta.mode() & 0o7777 != 0o660 {
        return Err(PublishError::Ambiguous);
    }
    still_named(&parent, &directory, &entry, owner.0)?;
    fchown(&directory, None, Some(Gid::from_raw(group))).map_err(|_| PublishError::Ambiguous)?;
    let meta = directory.metadata().map_err(|_| PublishError::Ambiguous)?;
    if (meta.uid(), meta.gid()) != (owner.0, group) || meta.mode() & 0o7777 != 0o700 {
        return Err(PublishError::Ambiguous);
    }
    let admission_directory = directory.try_clone().map_err(|_| PublishError::Ambiguous)?;
    let admission_entry = entry.try_clone().map_err(|_| PublishError::Ambiguous)?;
    if !before_publish() {
        return Err(PublishError::Ambiguous);
    }
    // Last step opens group traversal. A failed final admission attempts to
    // close the *pinned original* again, never a freshly resolved pathname.
    fchmod(&directory, Mode::from_bits_truncate(0o750)).map_err(|_| PublishError::Ambiguous)?;
    let admitted = AdmittedListener::open_published_pinned(
        listener,
        parent,
        admission_directory,
        admission_entry,
        path,
        owner,
        group,
    );
    admitted.map_err(|_| {
        let _ = fchmod(&directory, Mode::from_bits_truncate(0o700));
        PublishError::Ambiguous
    })
}
