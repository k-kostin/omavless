// SPDX-License-Identifier: MIT

//! Inactive, encrypted-only destination publisher. There is no product caller,
//! path-bearing IPC, file picker or restore authority.

use crate::backup_source_candidate::SealedBackup;
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, open, openat};
use nix::sys::stat::Mode;
use nix::unistd::linkat;
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PublishError {
    InvalidDestination,
    Exists,
    Unavailable,
    // The exclusive link may already exist. Never retry or claim no effect.
    Ambiguous,
}

fn stable_directory(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
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
}
