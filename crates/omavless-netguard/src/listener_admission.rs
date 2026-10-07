//! Inactive path-entry admission for an already bound, fixed-path listener.
//! The future trusted service must prove it created the supplied descriptor
//! before publishing group access: local_addr alone cannot bind a descriptor
//! to the current filesystem entry after an unlink/rebind. This module does not
//! create the directory, resolve the package group, bind, or start a service.

use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixListener;
use std::path::Path;

const DIR: &str = "omavless-netguard";
const LEAF: &str = "control.sock";
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

#[cfg(feature = "netguard-cold-bootstrap")]
type Retained<T> = std::mem::ManuallyDrop<T>;
#[cfg(not(feature = "netguard-cold-bootstrap"))]
type Retained<T> = T;
fn retain<T>(value: T) -> Retained<T> {
    #[cfg(feature = "netguard-cold-bootstrap")]
    {
        std::mem::ManuallyDrop::new(value)
    }
    #[cfg(not(feature = "netguard-cold-bootstrap"))]
    {
        value
    }
}

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
    listener: Retained<UnixListener>,
    parent: Option<Retained<File>>,
    directory: Option<Retained<File>>,
    leaf: Option<Retained<File>>,
    parent_owner: (u32, u32),
    owner_uid: u32,
    group: u32,
}

impl AdmittedListener {
    #[cfg(feature = "netguard-cold-bootstrap")]
    pub(crate) fn open_published_pinned_retaining(
        listener: UnixListener,
        parent: File,
        directory: File,
        leaf: File,
        expected_path: &Path,
        owner: (u32, u32),
        group: u32,
    ) -> Result<Self> {
        let admitted = std::mem::ManuallyDrop::new(Self {
            listener: retain(listener),
            parent: Some(retain(parent)),
            directory: Some(retain(directory)),
            leaf: Some(retain(leaf)),
            parent_owner: owner,
            owner_uid: owner.0,
            group,
        });
        ancestor(admitted.parent.as_ref().ok_or(REFUSE)?, owner)?;
        if admitted
            .listener
            .local_addr()
            .map_err(|_| REFUSE)?
            .as_pathname()
            != Some(expected_path)
        {
            return Err(REFUSE);
        }
        admitted.validate()?;
        Ok(std::mem::ManuallyDrop::into_inner(admitted))
    }
    /// Accept only the descriptors pinned by the private-bind publisher.
    /// Reopening the path here would allow an already replaced socket and
    /// directory to be treated as the publisher's own entry.
    pub(crate) fn open_published_pinned(
        listener: UnixListener,
        parent: File,
        directory: File,
        leaf: File,
        expected_path: &Path,
        owner: (u32, u32),
        group: u32,
    ) -> Result<Self> {
        ancestor(&parent, owner)?;
        if listener.local_addr().map_err(|_| REFUSE)?.as_pathname() != Some(expected_path) {
            return Err(REFUSE);
        }
        let admitted = Self {
            listener: retain(listener),
            parent: Some(retain(parent)),
            directory: Some(retain(directory)),
            leaf: Some(retain(leaf)),
            parent_owner: owner,
            owner_uid: owner.0,
            group,
        };
        admitted.validate()?;
        Ok(admitted)
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
            listener: retain(listener),
            parent: Some(retain(parent)),
            directory: Some(retain(directory)),
            leaf: Some(retain(leaf)),
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
            File::from(openat(parent.as_fd(), DIR, DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        check_directory(&current_directory, self.owner_uid, self.group)?;
        same(directory_fd, &current_directory)?;
        let current_leaf = File::from(
            openat(directory_fd.as_fd(), LEAF, SOCKET_PATH, Mode::empty()).map_err(|_| REFUSE)?,
        );
        check_socket_path(&current_leaf, self.owner_uid, self.group)?;
        same(leaf_fd, &current_leaf)
    }

    pub(crate) fn listener(&self) -> &UnixListener {
        &self.listener
    }

    #[cfg(test)]
    pub(crate) fn unchecked_for_test(listener: UnixListener) -> Self {
        Self {
            listener: retain(listener),
            parent: None,
            directory: None,
            leaf: None,
            parent_owner: (0, 0),
            owner_uid: 0,
            group: 0,
        }
    }
}
