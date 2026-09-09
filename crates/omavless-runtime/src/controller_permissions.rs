// SPDX-License-Identifier: MIT
//! Startup-only normalization of the authenticated owned core socket.
//! Never chmod a caller-selected pathname after checking it: the O_PATH
//! descriptor pins the inode, including if its directory entry is replaced.

use nix::fcntl::{OFlag, open, openat};
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use nix::sys::stat::Mode;
use std::fs::{self, File, Metadata};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

fn parent_valid(value: &Metadata, uid: u32) -> bool {
    value.is_dir() && value.uid() == uid && value.mode() & 0o7777 == 0o700
}

pub(crate) fn secure_owned(path: &Path, pid: u32, uid: u32) -> bool {
    secure_before(path, pid, uid, || {}).is_some()
}

fn secure_before(path: &Path, pid: u32, uid: u32, before_chmod: impl FnOnce()) -> Option<()> {
    if !path.is_absolute() || pid == 0 {
        return None;
    }
    let parent = path.parent()?;
    let parent_fd = File::from(
        open(
            parent,
            OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .ok()?,
    );
    let directory = parent_fd.metadata().ok()?;
    if !parent_valid(&directory, uid) {
        return None;
    }
    let held = File::from(
        openat(
            &parent_fd,
            Path::new(path.file_name()?),
            OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .ok()?,
    );
    let original = held.metadata().ok()?;
    if !original.file_type().is_socket()
        || original.uid() != uid
        || !matches!(original.mode() & 0o7777, 0o600 | 0o666)
    {
        return None;
    }
    let paths_match = || {
        fs::symlink_metadata(parent)
            .is_ok_and(|now| same(&now, &directory) && parent_valid(&now, uid))
            && fs::symlink_metadata(path).is_ok_and(|now| {
                same(&now, &original) && now.file_type().is_socket() && now.uid() == uid
            })
    };
    if !paths_match() {
        return None;
    }
    let connection = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        None,
    )
    .ok()?;
    connect(connection.as_raw_fd(), &UnixAddr::new(path).ok()?).ok()?;
    let peer = getsockopt(&connection, PeerCredentials).ok()?;
    if peer.uid() != uid || u32::try_from(peer.pid()).ok()? != pid || !paths_match() {
        return None;
    }
    before_chmod();
    // O_PATH cannot use fchmod. Linux procfs follows this fixed, still-held
    // descriptor to its inode; there is deliberately no pathname fallback.
    fs::set_permissions(
        format!("/proc/self/fd/{}", held.as_raw_fd()),
        fs::Permissions::from_mode(0o600),
    )
    .ok()?;
    let after = held.metadata().ok()?;
    if !same(&after, &original) || after.mode() & 0o7777 != 0o600 || !paths_match() {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    fn fixture() -> (std::path::PathBuf, std::path::PathBuf, UnixListener) {
        let root = crate::test_temp::directory("controller-mode").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("mihomo.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        (root, path, listener)
    }

    #[test]
    fn authenticated_socket_becomes_private_and_is_idempotent() {
        let (root, path, _listener) = fixture();
        for _ in 0..2 {
            assert!(secure_owned(
                &path,
                std::process::id(),
                nix::unistd::Uid::current().as_raw()
            ));
            assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_peer_or_owner_never_changes_mode() {
        let (root, path, _listener) = fixture();
        let uid = nix::unistd::Uid::current().as_raw();
        assert!(!secure_owned(
            &path,
            std::process::id().saturating_add(1),
            uid
        ));
        assert!(!secure_owned(
            &path,
            std::process::id(),
            uid.wrapping_add(1)
        ));
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o666);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_parent_socket_link_and_regular_file_are_refused() {
        let (root, path, _listener) = fixture();
        let uid = nix::unistd::Uid::current().as_raw();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!secure_owned(&path, std::process::id(), uid));
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let link = root.join("link");
        symlink(&path, &link).unwrap();
        assert!(!secure_owned(&link, std::process::id(), uid));
        let file = root.join("file");
        fs::write(&file, b"synthetic").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(!secure_owned(&file, std::process::id(), uid));
        assert_eq!(fs::metadata(&file).unwrap().mode() & 0o7777, 0o666);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o666);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacement_before_chmod_is_not_modified_and_admission_fails() {
        let (root, path, _listener) = fixture();
        let old = root.join("held-socket");
        assert!(
            secure_before(
                &path,
                std::process::id(),
                nix::unistd::Uid::current().as_raw(),
                || {
                    fs::rename(&path, &old).unwrap();
                    fs::write(&path, b"replacement").unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
                }
            )
            .is_none()
        );
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o666);
        assert_eq!(fs::metadata(&old).unwrap().mode() & 0o7777, 0o600);
        fs::remove_dir_all(root).unwrap();
    }
}
