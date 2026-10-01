//! Inactive path-entry admission for an already bound, fixed-path listener.
//! The future trusted service must prove it created the supplied descriptor
//! before publishing group access: local_addr alone cannot bind a descriptor
//! to the current filesystem entry after an unlink/rebind. This module does not
//! create the directory, resolve the package group, bind, or start a service.

use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use std::fs::File;
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
pub(crate) enum ListenerError {
    UnsafeOrUnavailable,
}

const REFUSE: ListenerError = ListenerError::UnsafeOrUnavailable;
type Result<T> = std::result::Result<T, ListenerError>;

fn ancestor(file: &File, owner: (u32, u32)) -> Result<()> {
    let meta = file.metadata().map_err(|_| REFUSE)?;
    if !meta.is_dir() || (meta.uid(), meta.gid()) != owner || meta.mode() & 0o022 != 0 {
        return Err(REFUSE);
    }
    Ok(())
}

fn check_directory(file: &File, owner_uid: u32, group: u32) -> Result<()> {
    let meta = file.metadata().map_err(|_| REFUSE)?;
    if !meta.is_dir()
        || (meta.uid(), meta.gid()) != (owner_uid, group)
        || meta.mode() & 0o7777 != 0o750
    {
        return Err(REFUSE);
    }
    Ok(())
}

fn check_socket_path(file: &File, owner_uid: u32, group: u32) -> Result<()> {
    let meta = file.metadata().map_err(|_| REFUSE)?;
    if !meta.file_type().is_socket()
        || meta.nlink() != 1
        || (meta.uid(), meta.gid()) != (owner_uid, group)
        || meta.mode() & 0o7777 != 0o660
    {
        return Err(REFUSE);
    }
    Ok(())
}

fn same(a: &File, b: &File) -> Result<()> {
    let a = a.metadata().map_err(|_| REFUSE)?;
    let b = b.metadata().map_err(|_| REFUSE)?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Err(REFUSE);
    }
    Ok(())
}

/// The observed path entry is pinned and rechecked. A trusted root service must
/// separately prove that it bound the supplied descriptor while the directory
/// was not yet accessible to the group. The caller also supplies the package
/// group's numeric GID; group-name resolution is a later gate. Replacement of
/// the observed entry never becomes a new authority for this instance.
pub(crate) struct AdmittedListener {
    listener: UnixListener,
    parent: Option<File>,
    directory: Option<File>,
    leaf: Option<File>,
    parent_owner: (u32, u32),
    owner_uid: u32,
    group: u32,
}

impl AdmittedListener {
    pub(crate) fn open_fixed(listener: UnixListener, package_gid: u32) -> Result<Self> {
        let root = File::from(open("/", DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        ancestor(&root, (0, 0))?;
        let run = File::from(openat(&root, "run", DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        ancestor(&run, (0, 0))?;
        Self::admit(listener, run, Path::new(FIXED_PATH), (0, 0), package_gid)
    }

    #[cfg(test)]
    pub(crate) fn open_test_parent(
        listener: UnixListener,
        parent: File,
        expected_path: &Path,
        owner: (u32, u32),
        group: u32,
    ) -> Result<Self> {
        ancestor(&parent, owner)?;
        Self::admit(listener, parent, expected_path, owner, group)
    }

    fn admit(
        listener: UnixListener,
        parent: File,
        expected_path: &Path,
        parent_owner: (u32, u32),
        group: u32,
    ) -> Result<Self> {
        if listener.local_addr().map_err(|_| REFUSE)?.as_pathname() != Some(expected_path) {
            return Err(REFUSE);
        }
        let directory =
            File::from(openat(&parent, DIR, DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        check_directory(&directory, parent_owner.0, group)?;
        let leaf =
            File::from(openat(&directory, LEAF, SOCKET_PATH, Mode::empty()).map_err(|_| REFUSE)?);
        check_socket_path(&leaf, parent_owner.0, group)?;
        let admitted = Self {
            listener,
            parent: Some(parent),
            directory: Some(directory),
            leaf: Some(leaf),
            parent_owner,
            owner_uid: parent_owner.0,
            group,
        };
        admitted.validate()?;
        Ok(admitted)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let (Some(parent), Some(directory_fd), Some(leaf_fd)) =
            (&self.parent, &self.directory, &self.leaf)
        else {
            // Only unit tests can construct an unadmitted listener.
            return Ok(());
        };
        ancestor(parent, self.parent_owner)?;
        check_directory(directory_fd, self.owner_uid, self.group)?;
        check_socket_path(leaf_fd, self.owner_uid, self.group)?;
        let current_directory =
            File::from(openat(parent, DIR, DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        check_directory(&current_directory, self.owner_uid, self.group)?;
        same(directory_fd, &current_directory)?;
        let current_leaf =
            File::from(openat(directory_fd, LEAF, SOCKET_PATH, Mode::empty()).map_err(|_| REFUSE)?);
        check_socket_path(&current_leaf, self.owner_uid, self.group)?;
        same(leaf_fd, &current_leaf)
    }

    pub(crate) fn listener(&self) -> &UnixListener {
        &self.listener
    }

    #[cfg(test)]
    pub(crate) fn unchecked_for_test(listener: UnixListener) -> Self {
        Self {
            listener,
            parent: None,
            directory: None,
            leaf: None,
            parent_owner: (0, 0),
            owner_uid: 0,
            group: 0,
        }
    }
}
