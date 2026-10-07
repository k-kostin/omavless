// SPDX-License-Identifier: MIT

//! Inactive, encrypted-only destination publisher. There is no product caller,
//! path-bearing IPC, file picker or restore authority.

use crate::backup_source_candidate::SealedBackup;
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, open, openat};
use nix::sys::stat::Mode;
use nix::unistd::linkat;
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use zeroize::Zeroizing;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PublishError {
    InvalidDestination,
    Exists,
    Unavailable,
    // The exclusive link may already exist. Never retry or claim no effect.
    Ambiguous,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadError {
    UnsafeSource,
    Changed,
    Unreadable,
}

/// Only bounded local counts are permitted to cross a future preview boundary.
/// A preview is not a reservation: restore must reread and revalidate.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BackupPreview {
    pub(crate) profiles: usize,
    pub(crate) subscriptions: usize,
}

fn stable_directory(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
}

fn stable_member(left: &Metadata, right: &Metadata) -> bool {
    stable_directory(left, right)
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

// Pin every ancestor: an intermediate symlink must never redirect a private
// backup to an attacker-controlled tree. Root-owned and same-user ancestors
// cannot be writable by other users; the destination parent is stricter.
fn open_parent(parent: &Path, uid: u32) -> Result<File, PublishError> {
    if !parent.is_absolute() {
        return Err(PublishError::InvalidDestination);
    }
    let mut directory = File::from(
        open(
            Path::new("/"),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| PublishError::InvalidDestination)?,
    );
    for component in parent.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                let metadata = directory
                    .metadata()
                    .map_err(|_| PublishError::InvalidDestination)?;
                if !metadata.is_dir()
                    || (metadata.uid() != 0 && metadata.uid() != uid)
                    || metadata.mode() & 0o6022 != 0
                {
                    return Err(PublishError::InvalidDestination);
                }
                directory = File::from(
                    openat(
                        &directory,
                        Path::new(name),
                        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| PublishError::InvalidDestination)?,
                );
            }
            _ => return Err(PublishError::InvalidDestination),
        }
    }
    let metadata = directory
        .metadata()
        .map_err(|_| PublishError::InvalidDestination)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(PublishError::InvalidDestination);
    }
    Ok(directory)
}

/// Write the already-authenticated ciphertext to an unnamed inode, sync it,
/// then create the final name exclusively. No plaintext or named temporary file
/// is ever placed at the destination. A failed post-link sync is ambiguous.
/// This is only one primitive; a future owner still must choose and authorize
/// the destination and explain uncertain results to the user.
#[allow(dead_code)]
pub(crate) fn publish_new(
    destination: &Path,
    uid: u32,
    sealed: &SealedBackup,
) -> Result<(), PublishError> {
    publish_new_with_hook(destination, uid, sealed, || {})
}

/// Open only one private, single-link regular ciphertext file. Authentication
/// finishes before an `OpenedBackup` can exist. This is deliberately not a
/// restore admission or IPC file-read method.
#[allow(dead_code)]
pub(crate) fn open_existing(
    source: &Path,
    uid: u32,
    passphrase: &[u8],
) -> Result<omavless_domain::private_backup::OpenedBackup, ReadError> {
    open_existing_with_hook(source, uid, passphrase, || {})
}

fn open_existing_with_hook(
    source: &Path,
    uid: u32,
    passphrase: &[u8],
    after_read: impl FnOnce(),
) -> Result<omavless_domain::private_backup::OpenedBackup, ReadError> {
    if !source.is_absolute() {
        return Err(ReadError::UnsafeSource);
    }
    let parent_path = source.parent().ok_or(ReadError::UnsafeSource)?;
    let name = source.file_name().ok_or(ReadError::UnsafeSource)?;
    let directory = open_parent(parent_path, uid).map_err(|_| ReadError::UnsafeSource)?;
    let parent_before = directory.metadata().map_err(|_| ReadError::UnsafeSource)?;
    let mut file = File::from(
        openat(
            &directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ReadError::UnsafeSource)?,
    );
    let before = file.metadata().map_err(|_| ReadError::UnsafeSource)?;
    let limit = omavless_domain::private_backup::MAX_BACKUP_BYTES;
    if !before.is_file()
        || before.uid() != uid
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > limit as u64
    {
        return Err(ReadError::UnsafeSource);
    }
    let mut ciphertext = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut ciphertext)
        .map_err(|_| ReadError::Changed)?;
    after_read(); // Synthetic race hook; product caller is a no-op.
    let current = File::from(
        openat(
            &directory,
            Path::new(name),
            OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ReadError::Changed)?,
    );
    let parent_after = open_parent(parent_path, uid).map_err(|_| ReadError::Changed)?;
    if ciphertext.len() as u64 != before.len()
        || ciphertext.len() > limit
        || !stable_member(&before, &file.metadata().map_err(|_| ReadError::Changed)?)
        || !stable_member(
            &before,
            &current.metadata().map_err(|_| ReadError::Changed)?,
        )
        || !stable_directory(
            &parent_before,
            &directory.metadata().map_err(|_| ReadError::Changed)?,
        )
        || !stable_directory(
            &parent_before,
            &parent_after.metadata().map_err(|_| ReadError::Changed)?,
        )
    {
        return Err(ReadError::Changed);
    }
    omavless_domain::private_backup::open(&ciphertext, passphrase)
        .map_err(|_| ReadError::Unreadable)
}

#[allow(dead_code)]
pub(crate) fn preview_existing(
    source: &Path,
    uid: u32,
    passphrase: &[u8],
) -> Result<BackupPreview, ReadError> {
    let opened = open_existing(source, uid, passphrase)?;
    Ok(BackupPreview {
        profiles: opened.profile_count(),
        subscriptions: opened.subscription_count(),
    })
}

fn publish_new_with_hook(
    destination: &Path,
    uid: u32,
    sealed: &SealedBackup,
    after_link: impl FnOnce(),
) -> Result<(), PublishError> {
    if !destination.is_absolute() {
        return Err(PublishError::InvalidDestination);
    }
    let parent_path = destination
        .parent()
        .ok_or(PublishError::InvalidDestination)?;
    let name = destination
        .file_name()
        .ok_or(PublishError::InvalidDestination)?;
    let directory = open_parent(parent_path, uid)?;
    let before = directory
        .metadata()
        .map_err(|_| PublishError::InvalidDestination)?;

    // Unsupported O_TMPFILE is a clean refusal. There is no insecure named
    // temporary fallback, which could survive a crash as an exposed artifact.
    let mut file = File::from(
        openat(
            &directory,
            Path::new("."),
            OFlag::O_TMPFILE | OFlag::O_RDWR | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| PublishError::Unavailable)?,
    );
    let metadata = file.metadata().map_err(|_| PublishError::Unavailable)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 0
    {
        return Err(PublishError::Unavailable);
    }
    file.write_all(sealed.bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| PublishError::Unavailable)?;
    let after = file.metadata().map_err(|_| PublishError::Unavailable)?;
    if after.len() != sealed.bytes().len() as u64
        || after.uid() != uid
        || after.mode() & 0o7777 != 0o600
        || after.nlink() != 0
        || !stable_directory(
            &before,
            &directory
                .metadata()
                .map_err(|_| PublishError::Unavailable)?,
        )
    {
        return Err(PublishError::Unavailable);
    }
    match linkat(
        &file,
        Path::new(""),
        &directory,
        Path::new(name),
        AtFlags::AT_EMPTY_PATH,
    ) {
        Ok(()) => {}
        Err(Errno::EEXIST) => return Err(PublishError::Exists),
        Err(_) => return Err(PublishError::Unavailable),
    }
    after_link(); // Synthetic crash/race hook; product caller is a no-op.
    let current_parent = open_parent(parent_path, uid).map_err(|_| PublishError::Ambiguous)?;
    let current = current_parent
        .metadata()
        .map_err(|_| PublishError::Ambiguous)?;
    if !stable_directory(&before, &current) {
        return Err(PublishError::Ambiguous);
    }
    directory.sync_all().map_err(|_| PublishError::Ambiguous)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn fixture() -> (std::path::PathBuf, u32, SealedBackup) {
        // The product path policy intentionally rejects /tmp's writable
        // ancestor. Use an ordinary same-user home path even when CI sets
        // TMPDIR=/tmp, without weakening production admission.
        let home = std::env::var_os("HOME").expect("backup test needs a home directory");
        let root =
            crate::test_temp::directory_under(Path::new(&home), "backup-destination").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        let store = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
        let bytes = omavless_domain::private_backup::seal(
            store,
            include_bytes!("../../../templates/default.yaml"),
            b"synthetic passphrase only",
        )
        .unwrap();
        (root, uid, SealedBackup::synthetic(bytes))
    }

    #[test]
    fn publishes_only_ciphertext_once_without_overwriting() {
        let (root, uid, sealed) = fixture();
        let destination = root.join("private.ovb");
        assert_eq!(publish_new(&destination, uid, &sealed), Ok(()));
        let metadata = fs::symlink_metadata(&destination).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(fs::read(&destination).unwrap(), sealed.bytes());
        assert_eq!(
            publish_new(&destination, uid, &sealed),
            Err(PublishError::Exists)
        );
        assert_eq!(fs::read(&destination).unwrap(), sealed.bytes());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_unsafe_parents_and_existing_symlinks_without_writes() {
        let (root, uid, sealed) = fixture();
        let destination = root.join("private.ovb");
        assert_eq!(
            publish_new(&destination, uid.wrapping_add(1), &sealed),
            Err(PublishError::InvalidDestination)
        );
        let other = root.join("other");
        fs::create_dir(&other).unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            publish_new(&other.join("private.ovb"), uid, &sealed),
            Err(PublishError::InvalidDestination)
        );
        let link = root.join("linked");
        symlink(&other, &link).unwrap();
        assert_eq!(
            publish_new(&link.join("private.ovb"), uid, &sealed),
            Err(PublishError::InvalidDestination)
        );
        symlink(root.join("missing"), &destination).unwrap();
        assert_eq!(
            publish_new(&destination, uid, &sealed),
            Err(PublishError::Exists)
        );
        assert!(
            fs::symlink_metadata(destination)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(fs::read_dir(other).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn post_link_directory_replacement_is_ambiguous_and_keeps_encrypted_file() {
        let (root, uid, sealed) = fixture();
        let destination = root.join("private.ovb");
        let displaced = root.with_extension("displaced");
        let result = publish_new_with_hook(&destination, uid, &sealed, || {
            fs::rename(&root, &displaced).unwrap();
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        });
        assert_eq!(result, Err(PublishError::Ambiguous));
        assert!(!destination.exists());
        assert_eq!(
            fs::read(displaced.join("private.ovb")).unwrap(),
            sealed.bytes()
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(displaced).unwrap();
    }

    #[test]
    fn bounded_private_open_and_counts_only_preview_require_authentication() {
        let (root, uid, sealed) = fixture();
        let destination = root.join("private.ovb");
        publish_new(&destination, uid, &sealed).unwrap();
        let opened = open_existing(&destination, uid, b"synthetic passphrase only").unwrap();
        assert_eq!(opened.profile_count(), 0);
        assert_eq!(opened.subscription_count(), 0);
        assert_eq!(
            preview_existing(&destination, uid, b"synthetic passphrase only"),
            Ok(BackupPreview {
                profiles: 0,
                subscriptions: 0
            })
        );
        assert!(matches!(
            open_existing(&destination, uid, b"incorrect passphrase"),
            Err(ReadError::Unreadable)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_open_refuses_links_public_modes_and_changed_bytes() {
        let (root, uid, sealed) = fixture();
        let destination = root.join("private.ovb");
        publish_new(&destination, uid, &sealed).unwrap();
        let linked = root.join("linked.ovb");
        symlink(&destination, &linked).unwrap();
        assert!(matches!(
            open_existing(&linked, uid, b"synthetic passphrase only"),
            Err(ReadError::UnsafeSource)
        ));
        fs::remove_file(&linked).unwrap();
        fs::hard_link(&destination, &linked).unwrap();
        assert!(matches!(
            open_existing(&destination, uid, b"synthetic passphrase only"),
            Err(ReadError::UnsafeSource)
        ));
        fs::remove_file(&linked).unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            open_existing(&destination, uid, b"synthetic passphrase only"),
            Err(ReadError::UnsafeSource)
        ));
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).unwrap();
        let mut changed = fs::read(&destination).unwrap();
        *changed.last_mut().unwrap() ^= 1;
        fs::write(&destination, changed).unwrap();
        assert!(matches!(
            open_existing(&destination, uid, b"synthetic passphrase only"),
            Err(ReadError::Unreadable)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_open_refuses_changed_file_and_parent_before_decryption() {
        for replace_parent in [false, true] {
            let (root, uid, sealed) = fixture();
            let destination = root.join("private.ovb");
            publish_new(&destination, uid, &sealed).unwrap();
            let displaced = root.with_extension("read-displaced");
            let result =
                open_existing_with_hook(&destination, uid, b"synthetic passphrase only", || {
                    if replace_parent {
                        fs::rename(&root, &displaced).unwrap();
                        fs::create_dir(&root).unwrap();
                        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
                    } else {
                        let mut changed = fs::read(&destination).unwrap();
                        *changed.last_mut().unwrap() ^= 1;
                        fs::write(&destination, changed).unwrap();
                    }
                });
            assert!(matches!(result, Err(ReadError::Changed)));
            fs::remove_dir_all(root).unwrap();
            if replace_parent {
                fs::remove_dir_all(displaced).unwrap();
            }
        }
    }
}
