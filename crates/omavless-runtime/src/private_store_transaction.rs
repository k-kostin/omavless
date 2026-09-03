// SPDX-License-Identifier: MIT

//! Shared exact-byte private-store transaction primitive.
//!
//! Credential-bearing original/candidate bytes live only in this type. Every
//! commit/restore requires the same shared migration lock that excludes the
//! legacy Python owner; byte comparison is a second fail-closed guard.

use crate::cutover::{CutoverPaths, MigrationLock};
use omavless_domain::private_store::{
    CompatibilityPointerTarget, PrivateStoreError, apply_compatibility_pointer_update,
};
use omavless_store::{StoreIoError, atomic_replace_private, read_private_utf8};
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateStoreWriteError {
    UnsafeStore,
    StoreChanged,
    StoreIo,
    Mutation(PrivateStoreError),
    LockMismatch,
}

impl fmt::Display for PrivateStoreWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeStore => formatter.write_str("Private profile store path is unsafe"),
            Self::StoreChanged => formatter.write_str("Private profile store changed concurrently"),
            Self::StoreIo => formatter.write_str("Private profile store update failed"),
            Self::Mutation(error) => error.fmt(formatter),
            Self::LockMismatch => formatter.write_str("Private profile store lock is invalid"),
        }
    }
}

impl std::error::Error for PrivateStoreWriteError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedWrite {
    NoChange,
    Changed,
}

/// One credential-bearing original/candidate pair. It intentionally cannot be
/// formatted, cloned or serialized.
pub(crate) struct PreparedPrivateStoreWrite {
    store_path: PathBuf,
    uid: u32,
    original: Vec<u8>,
    candidate: Vec<u8>,
    changed: bool,
}

/// Prepared compatibility-pointer update plus public bounded prune count.
pub(crate) struct PreparedPointerMutation {
    prepared: PreparedPrivateStoreWrite,
    pub(crate) pruned: usize,
}

impl PreparedPointerMutation {
    pub fn commit_locked(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, PrivateStoreWriteError> {
        self.prepared.commit_locked(lock, paths)
    }

    pub fn restore_locked(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, PrivateStoreWriteError> {
        self.prepared.restore_locked(lock, paths)
    }
}

impl PreparedPrivateStoreWrite {
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    fn authorize(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<(), PrivateStoreWriteError> {
        if lock.authorizes(paths, self.uid) {
            Ok(())
        } else {
            Err(PrivateStoreWriteError::LockMismatch)
        }
    }

    fn current_bytes(&self) -> Result<Vec<u8>, PrivateStoreWriteError> {
        validate_store_path(&self.store_path, self.uid)?;
        read_private_utf8(&self.store_path, self.uid)
            .map(String::into_bytes)
            .map_err(map_io)
    }

    fn replace_and_verify(&self, payload: &[u8]) -> Result<(), PrivateStoreWriteError> {
        atomic_replace_private(&self.store_path, payload, self.uid).map_err(map_io)?;
        if self.current_bytes()?.as_slice() != payload {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        Ok(())
    }

    pub fn commit_locked(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, PrivateStoreWriteError> {
        self.authorize(lock, paths)?;
        if self.current_bytes()? != self.original {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        if !self.changed {
            return Ok(PreparedWrite::NoChange);
        }
        self.replace_and_verify(&self.candidate)?;
        Ok(PreparedWrite::Changed)
    }

    pub fn restore_locked(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, PrivateStoreWriteError> {
        self.authorize(lock, paths)?;
        let current = self.current_bytes()?;
        if current == self.original {
            return Ok(PreparedWrite::NoChange);
        }
        if current != self.candidate {
            return Err(PrivateStoreWriteError::StoreChanged);
        }
        self.replace_and_verify(&self.original)?;
        Ok(PreparedWrite::Changed)
    }
}

fn private_parent(path: &Path, uid: u32) -> bool {
    path.parent().is_some_and(|parent| {
        fs::symlink_metadata(parent).is_ok_and(|metadata| {
            !metadata.file_type().is_symlink()
                && metadata.is_dir()
                && metadata.uid() == uid
                && metadata.permissions().mode() & 0o077 == 0
        })
    })
}

fn private_store_file(path: &Path, uid: u32) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.uid() == uid
            && metadata.permissions().mode() & 0o077 == 0
    })
}

fn validate_store_path(path: &Path, uid: u32) -> Result<(), PrivateStoreWriteError> {
    if path.is_absolute()
        && path.file_name().and_then(|name| name.to_str()) == Some("profiles.json")
        && private_parent(path, uid)
        && private_store_file(path, uid)
    {
        Ok(())
    } else {
        Err(PrivateStoreWriteError::UnsafeStore)
    }
}

fn map_io(_error: StoreIoError) -> PrivateStoreWriteError {
    PrivateStoreWriteError::StoreIo
}

/// Read and validate one exact store snapshot, then prepare a complete
/// credential-bearing replacement without changing the filesystem.
pub(crate) fn prepare_private_store_write<F>(
    store_path: &Path,
    uid: u32,
    transform: F,
) -> Result<PreparedPrivateStoreWrite, PrivateStoreWriteError>
where
    F: FnOnce(&str) -> Result<(Vec<u8>, bool), PrivateStoreError>,
{
    validate_store_path(store_path, uid)?;
    let input = read_private_utf8(store_path, uid).map_err(map_io)?;
    let (candidate, changed) = transform(&input).map_err(PrivateStoreWriteError::Mutation)?;
    Ok(PreparedPrivateStoreWrite {
        store_path: store_path.to_path_buf(),
        uid,
        original: input.into_bytes(),
        candidate,
        changed,
    })
}

pub(crate) fn prepare_pointer_mutation(
    store_path: &Path,
    uid: u32,
    target: CompatibilityPointerTarget,
) -> Result<PreparedPointerMutation, PrivateStoreWriteError> {
    let mut pruned = 0;
    let prepared = prepare_private_store_write(store_path, uid, |input| {
        let mutation = apply_compatibility_pointer_update(input, target)?;
        pruned = mutation.pruned;
        Ok((mutation.payload().to_vec(), mutation.changed))
    })?;
    Ok(PreparedPointerMutation { prepared, pruned })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(label: &str) -> (PathBuf, PathBuf, CutoverPaths, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-private-write-{label}-{}-{nonce}",
            std::process::id()
        ));
        let config = root.join("config");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for path in [&config, &runtime, &state] {
            fs::create_dir_all(path).unwrap();
        }
        for path in [&root, &config, &runtime, &state] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let store = config.join("profiles.json");
        fs::write(&store, b"original\n").unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
        let paths = CutoverPaths::below(&runtime, &state, uid);
        (root, store, paths, uid)
    }

    #[test]
    fn exact_write_requires_the_matching_owner_lock_and_restores_idempotently() {
        let (root, store, paths, uid) = fixture("matching-lock");
        let prepared =
            prepare_private_store_write(&store, uid, |_| Ok((b"candidate\n".to_vec(), true)))
                .unwrap();
        let other_runtime = root.join("other-runtime");
        fs::create_dir(&other_runtime).unwrap();
        fs::set_permissions(&other_runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let wrong_paths = CutoverPaths::below(&other_runtime, &root.join("state"), uid);
        let wrong_lock = MigrationLock::acquire(&wrong_paths, uid).unwrap();
        assert_eq!(
            prepared.commit_locked(&wrong_lock, &paths),
            Err(PrivateStoreWriteError::LockMismatch)
        );
        assert_eq!(fs::read(&store).unwrap(), b"original\n");
        drop(wrong_lock);

        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        assert_eq!(
            prepared.commit_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert_eq!(fs::read(&store).unwrap(), b"candidate\n");
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Ok(PreparedWrite::Changed)
        );
        assert_eq!(
            prepared.restore_locked(&lock, &paths),
            Ok(PreparedWrite::NoChange)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
