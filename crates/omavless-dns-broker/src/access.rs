//! Fixed socket access, not an arbitrary chmod/ACL endpoint.
//!
//! Filesystem AF_UNIX socket nodes and socket descriptors have different inodes.
//! Therefore use no-follow pathname xattrs under root-only writable ancestors,
//! not fsetxattr on the listener's sockfs descriptor. Root/CAP administrators are
//! outside the ordinary enrolled-user threat model. No unsafe code is required.
use crate::admission::RootContext;
use rustix::fs::{XattrFlags, lgetxattr, lsetxattr};
use std::{fmt, fs, os::unix::fs::MetadataExt, path::Path};

const SOCKET: &str = "/run/omavless-dns/control.sock";
const ACL: &str = "system.posix_acl_access";
const ACL_LEN: usize = 44;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Error;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DNS broker socket access could not be established.")
    }
}
impl std::error::Error for Error {}

/// Identity is captured before granting access and rechecked before accepting.
/// Never removes a socket or broadens access on failure.
pub(crate) struct SocketAccess {
    identity: (u64, u64),
    uid: u32,
}
impl SocketAccess {
    pub(crate) fn grant(context: &RootContext) -> Result<Self, Error> {
        fixed_ancestors()?;
        grant(Path::new(SOCKET), 0, context.enrolled_uid())
    }

    pub(crate) fn recheck(&self) -> Result<(), Error> {
        fixed_ancestors()?;
        verify(Path::new(SOCKET), 0, self.uid, self.identity)
    }
}

fn fixed_ancestors() -> Result<(), Error> {
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(Error);
    }
    for directory in ["/", "/run", "/run/omavless-dns"] {
        protected_parent(Path::new(directory), 0)?;
    }
    Ok(())
}

fn protected_parent(path: &Path, owner: u32) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error)?;
    if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 {
        return Err(Error);
    }
    Ok(())
}

fn socket(path: &Path, owner: u32, mode: u32) -> Result<(u64, u64), Error> {
    use std::os::unix::fs::FileTypeExt;
    protected_parent(path.parent().ok_or(Error)?, owner)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| Error)?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != owner
        || metadata.nlink() != 1
        || metadata.mode() & 0o7777 != mode
    {
        return Err(Error);
    }
    Ok((metadata.dev(), metadata.ino()))
}

fn acl(uid: u32) -> Result<[u8; ACL_LEN], Error> {
    if uid == 0 || uid == u32::MAX {
        return Err(Error);
    }
    // Linux UAPI posix_acl_xattr.h: version 2, sorted tag/perm/id entries.
    // owner rw; enrolled user rw; owning group none; mask rw; other none.
    let entries: [(u16, u16, u32); 5] = [
        (0x01, 6, u32::MAX),
        (0x02, 6, uid),
        (0x04, 0, u32::MAX),
        (0x10, 6, u32::MAX),
        (0x20, 0, u32::MAX),
    ];
    let mut output = [0_u8; ACL_LEN];
    output[..4].copy_from_slice(&2_u32.to_le_bytes());
    for (position, (tag, permissions, id)) in entries.into_iter().enumerate() {
        let offset = 4 + position * 8;
        output[offset..offset + 2].copy_from_slice(&tag.to_le_bytes());
        output[offset + 2..offset + 4].copy_from_slice(&permissions.to_le_bytes());
        output[offset + 4..offset + 8].copy_from_slice(&id.to_le_bytes());
    }
    Ok(output)
}

// Fixture-only path/owner seam; all production callers use the fixed wrapper.
fn grant(path: &Path, owner: u32, uid: u32) -> Result<SocketAccess, Error> {
    let expected = acl(uid)?;
    let identity = socket(path, owner, 0o600)?;
    lsetxattr(path, ACL, &expected, XattrFlags::empty()).map_err(|_| Error)?;
    verify(path, owner, uid, identity)?;
    Ok(SocketAccess { identity, uid })
}

fn verify(path: &Path, owner: u32, uid: u32, identity: (u64, u64)) -> Result<(), Error> {
    if socket(path, owner, 0o660)? != identity {
        return Err(Error);
    }
    let expected = acl(uid)?;
    let mut actual = [0_u8; ACL_LEN];
    let length = lgetxattr(path, ACL, &mut actual[..]).map_err(|_| Error)?;
    if length != expected.len() || actual != expected || socket(path, owner, 0o660)? != identity {
        return Err(Error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    use tempfile::TempDir;

    fn fixture() -> (TempDir, UnixListener, u32) {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("control.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        (directory, listener, rustix::process::geteuid().as_raw())
    }

    #[test]
    fn real_socket_exact_named_uid_acl_and_readback() {
        let (directory, _listener, owner) = fixture();
        let path = directory.path().join("control.sock");
        let admitted = grant(&path, owner, 42424).unwrap();
        verify(&path, owner, admitted.uid, admitted.identity).unwrap();
        let bytes = acl(42424).unwrap();
        assert_eq!(&bytes[16..20], &42424_u32.to_le_bytes());
        assert_eq!(&bytes[22..24], &[0, 0]); // owning group has no rights
        assert_eq!(&bytes[38..40], &[0, 0]); // other has no rights
    }

    #[test]
    fn invalid_enrollment_does_not_change_initial_mode() {
        let (directory, _listener, owner) = fixture();
        let path = directory.path().join("control.sock");
        for uid in [0, u32::MAX] {
            assert!(grant(&path, owner, uid).is_err());
            assert!(socket(&path, owner, 0o600).is_ok());
        }
    }

    #[test]
    fn wrong_owner_or_permissive_initial_mode_is_refused() {
        let (directory, _listener, owner) = fixture();
        let path = directory.path().join("control.sock");
        assert!(grant(&path, owner + 1, 42424).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(grant(&path, owner, 42424).is_err());
    }

    #[test]
    fn ordinary_file_symlink_and_hardlinked_socket_are_refused() {
        let (directory, _listener, owner) = fixture();
        let path = directory.path().join("control.sock");
        let file = directory.path().join("file");
        fs::write(&file, b"fixture").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(grant(&file, owner, 42424).is_err());
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(grant(&link, owner, 42424).is_err());
        fs::hard_link(&path, directory.path().join("hardlink")).unwrap();
        assert!(grant(&path, owner, 42424).is_err());
    }

    #[test]
    fn writable_or_symlink_parent_is_refused() {
        let (directory, _listener, owner) = fixture();
        let path = directory.path().join("control.sock");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o720)).unwrap();
        assert!(grant(&path, owner, 42424).is_err());
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let other = tempfile::tempdir().unwrap();
        let link = other.path().join("parent");
        std::os::unix::fs::symlink(directory.path(), &link).unwrap();
        assert!(grant(&link.join("control.sock"), owner, 42424).is_err());
    }

    #[test]
    fn replaced_inode_or_modified_acl_is_refused() {
        let (directory, _listener, owner) = fixture();
        let path = directory.path().join("control.sock");
        let admitted = grant(&path, owner, 42424).unwrap();
        lsetxattr(&path, ACL, &acl(42425).unwrap(), XattrFlags::empty()).unwrap();
        assert!(verify(&path, owner, 42424, admitted.identity).is_err());
        fs::remove_file(&path).unwrap();
        let _replacement = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        grant(&path, owner, 42424).unwrap();
        assert!(verify(&path, owner, 42424, admitted.identity).is_err());
    }

    #[test]
    fn errors_are_fixed_and_bounded() {
        assert_eq!(
            Error.to_string(),
            "DNS broker socket access could not be established."
        );
    }
}
