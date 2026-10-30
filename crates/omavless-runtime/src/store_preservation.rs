// SPDX-License-Identifier: MIT

//! Explicit preservation of the fixed original store, before native startup.
//! No profile parser, runtime owner, repair, store write or daemon IPC.

use crate::backup_destination_candidate::{PublishError, publish_original};
use crate::backup_source_candidate::{OriginalStore, open_owned_directory, open_private_directory};
use crate::cutover::{
    CutoverPaths, MigrationLock, OwnershipMarker, OwnershipPhase, read_marker_existing,
};
use nix::unistd::Uid;
use serde::Deserialize;
use std::fs::{self, File, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub const MAX_INPUT_BYTES: usize = 8192;

#[derive(Debug, PartialEq, Eq)]
pub enum PreservationError {
    InvalidInput,
    Refused,
    DestinationExists,
    Unknown,
}

impl std::fmt::Display for PreservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Original preservation requires a destination and explicit unencrypted-copy consent through bounded stdin",
            Self::Refused => "Original preservation refused: source, destination or operation state is unavailable, unsafe or changed",
            Self::DestinationExists => "Original preservation refused: destination already exists; it was not replaced",
            Self::Unknown => "Original preservation outcome is unknown; a copy may exist. Inspect the chosen destination; do not repeat or delete it automatically",
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    destination: PathBuf,
    confirm_unencrypted: bool,
}

fn same_directory(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino() && a.uid() == b.uid() && a.mode() == b.mode()
}

fn same_optional_file(a: Option<&Metadata>, b: Option<&Metadata>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            same_directory(a, b)
                && a.gid() == b.gid()
                && a.nlink() == b.nlink()
                && a.len() == b.len()
                && a.mtime() == b.mtime()
                && a.mtime_nsec() == b.mtime_nsec()
                && a.ctime() == b.ctime()
                && a.ctime_nsec() == b.ctime_nsec()
        }
        _ => false,
    }
}

fn marker_metadata(paths: &CutoverPaths, uid: u32) -> Result<Option<Metadata>, PreservationError> {
    match fs::symlink_metadata(&paths.ownership_marker) {
        Ok(m)
            if m.is_file()
                && !m.file_type().is_symlink()
                && m.uid() == uid
                && m.mode() & 0o7777 == 0o600
                && m.nlink() == 1 =>
        {
            Ok(Some(m))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        _ => Err(PreservationError::Refused),
    }
}

// Pin the existing state parent even in preactivation Legacy, when its private
// omavless child may be absent. Missing state BASE is refused, never created.
// Hold directory FDs so inode identity cannot be recycled during this operation.
struct StateFence {
    parent: File,
    state: Option<File>,
    marker: OwnershipMarker,
    marker_metadata: Option<Metadata>,
}

impl StateFence {
    fn read(
        paths: &CutoverPaths,
        uid: u32,
        lock: &MigrationLock,
    ) -> Result<Self, PreservationError> {
        let refused = |_| PreservationError::Refused;
        if !lock.authorizes(paths, uid) {
            return Err(PreservationError::Refused);
        }
        let parent = open_owned_directory(
            paths
                .state_directory
                .parent()
                .ok_or(PreservationError::Refused)?,
            uid,
        )
        .map_err(refused)?;
        let state = match fs::symlink_metadata(&paths.state_directory) {
            Ok(_) => Some(open_private_directory(&paths.state_directory, uid).map_err(refused)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            _ => return Err(PreservationError::Refused),
        };
        let before = marker_metadata(paths, uid)?;
        let marker = read_marker_existing(paths, uid).map_err(|_| PreservationError::Refused)?;
        if !matches!(
            marker.phase(),
            OwnershipPhase::Legacy | OwnershipPhase::Rust
        ) || crate::pending_private_transaction::pending_at(&paths.state_directory)
            || !same_optional_file(before.as_ref(), marker_metadata(paths, uid)?.as_ref())
            || !lock.authorizes(paths, uid)
        {
            return Err(PreservationError::Refused);
        }
        Ok(Self {
            parent,
            state,
            marker,
            marker_metadata: before,
        })
    }

    fn verify(&self, paths: &CutoverPaths, uid: u32, lock: &MigrationLock) -> bool {
        let Ok(current) = Self::read(paths, uid, lock) else {
            return false;
        };
        let directories_match = |a: &File, b: &File| match (a.metadata(), b.metadata()) {
            (Ok(a), Ok(b)) => same_directory(&a, &b),
            _ => false,
        };
        directories_match(&self.parent, &current.parent)
            && match (&self.state, &current.state) {
                (None, None) => true,
                (Some(a), Some(b)) => directories_match(a, b),
                _ => false,
            }
            && self.marker == current.marker
            && same_optional_file(
                self.marker_metadata.as_ref(),
                current.marker_metadata.as_ref(),
            )
    }
}

/// Destination is private stdin data. The source is always the established
/// current-user location; this is not a generic file read/export interface.
pub fn preserve_current_original(input: &str) -> Result<(), PreservationError> {
    if input.len() > MAX_INPUT_BYTES || !input.trim_start().starts_with('{') {
        return Err(PreservationError::InvalidInput);
    }
    let input: Input = serde_json::from_str(input).map_err(|_| PreservationError::InvalidInput)?;
    if !input.confirm_unencrypted
        || !input.destination.is_absolute()
        || input.destination.as_os_str().len() > 4096
    {
        return Err(PreservationError::InvalidInput);
    }
    let uid = Uid::current().as_raw();
    let home = std::env::var_os("OMAVLESS_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or(PreservationError::Refused)?;
    let config = home.join(".config/omavless");
    let paths = CutoverPaths::current(uid).map_err(|_| PreservationError::Refused)?;
    // Never publish into the managed config/state/runtime child itself.
    if input.destination.starts_with(&config)
        || input.destination.starts_with(&paths.state_directory)
        || input.destination.starts_with(
            crate::RuntimePaths::current()
                .map_err(|_| PreservationError::Refused)?
                .directory,
        )
        || input.destination == paths.operation_lock
    {
        return Err(PreservationError::Refused);
    }
    open_private_directory(&paths.runtime_base, uid).map_err(|_| PreservationError::Refused)?;
    let lock = MigrationLock::acquire(&paths, uid).map_err(|_| PreservationError::Refused)?;
    let fence = StateFence::read(&paths, uid, &lock)?;
    let original = OriginalStore::capture(&config, uid).map_err(|_| PreservationError::Refused)?;
    publish_original(Path::new(&input.destination), uid, &original, || {
        fence.verify(&paths, uid, &lock)
    })
    .map_err(|e| match e {
        PublishError::Exists => PreservationError::DestinationExists,
        PublishError::Ambiguous => PreservationError::Unknown,
        _ => PreservationError::Refused,
    })
}
