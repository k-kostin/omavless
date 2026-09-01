// SPDX-License-Identifier: MIT

//! Bounded, same-user, symlink-refusing private file replacement.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const MAX_STORE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreIoError {
    UnsafePath,
    WrongOwner,
    TooLarge,
    InvalidUtf8,
    Io,
}
impl fmt::Display for StoreIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafePath => "Private store path is unsafe",
            Self::WrongOwner => "Private store ownership is unsafe",
            Self::TooLarge => "Private store is too large",
            Self::InvalidUtf8 => "Private store is not valid UTF-8",
            Self::Io => "Private store operation failed",
        })
    }
}
impl std::error::Error for StoreIoError {}

fn safe_parent(path: &Path, expected_uid: u32) -> Result<PathBuf, StoreIoError> {
    let parent = path.parent().ok_or(StoreIoError::UnsafePath)?;
    let metadata = fs::symlink_metadata(parent).map_err(|_| StoreIoError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StoreIoError::UnsafePath);
    }
    if metadata.uid() != expected_uid {
        return Err(StoreIoError::WrongOwner);
    }
    Ok(parent.to_path_buf())
}

pub fn read_private_utf8(path: &Path, expected_uid: u32) -> Result<String, StoreIoError> {
    safe_parent(path, expected_uid)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| StoreIoError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StoreIoError::UnsafePath);
    }
    if metadata.uid() != expected_uid {
        return Err(StoreIoError::WrongOwner);
    }
    if metadata.len() > MAX_STORE_BYTES as u64 {
        return Err(StoreIoError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .and_then(|file| {
            file.take((MAX_STORE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| StoreIoError::Io)?;
    if bytes.len() > MAX_STORE_BYTES {
        return Err(StoreIoError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| StoreIoError::InvalidUtf8)
}

pub fn atomic_replace_private(
    path: &Path,
    payload: &[u8],
    expected_uid: u32,
) -> Result<(), StoreIoError> {
    if payload.len() > MAX_STORE_BYTES {
        return Err(StoreIoError::TooLarge);
    }
    let parent = safe_parent(path, expected_uid)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(StoreIoError::UnsafePath);
        }
        if metadata.uid() != expected_uid {
            return Err(StoreIoError::WrongOwner);
        }
    }
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(StoreIoError::UnsafePath)?;
    let mut created = None;
    for nonce in 0..128_u32 {
        let candidate = parent.join(format!(".{name}.{}.{}", std::process::id(), nonce));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&candidate)
        {
            Ok(file) => {
                created = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(StoreIoError::Io),
        }
    }
    let (temporary, mut file) = created.ok_or(StoreIoError::Io)?;
    let result = (|| {
        file.write_all(payload).map_err(|_| StoreIoError::Io)?;
        file.sync_all().map_err(|_| StoreIoError::Io)?;
        drop(file);
        fs::rename(&temporary, path).map_err(|_| StoreIoError::Io)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| StoreIoError::Io)?;
        File::open(&parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| StoreIoError::Io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root() -> (PathBuf, u32) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("omavless-store-{}-{unique}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    #[test]
    fn replacement_is_private_durable_and_bounded() {
        let (root, uid) = root();
        let path = root.join("profiles.json");
        atomic_replace_private(&path, b"{\"version\":3}\n", uid).unwrap();
        assert_eq!(read_private_utf8(&path, uid).unwrap(), "{\"version\":3}\n");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            atomic_replace_private(&path, &vec![0; MAX_STORE_BYTES + 1], uid),
            Err(StoreIoError::TooLarge)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn symlink_target_fails_closed_without_touching_destination() {
        let (root, uid) = root();
        let destination = root.join("destination");
        fs::write(&destination, b"unchanged").unwrap();
        let path = root.join("profiles.json");
        symlink(&destination, &path).unwrap();
        assert_eq!(
            atomic_replace_private(&path, b"private", uid),
            Err(StoreIoError::UnsafePath)
        );
        assert_eq!(fs::read(destination).unwrap(), b"unchanged");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn errors_never_include_paths_or_payloads() {
        let error =
            read_private_utf8(Path::new("/private.example/password"), u32::MAX).unwrap_err();
        assert!(!error.to_string().contains("private.example"));
        assert!(!error.to_string().contains("password"));
    }
}
