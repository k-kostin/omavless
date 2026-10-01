// SPDX-License-Identifier: MIT

//! Inactive, read-only inspection of a future private restore decision pair.
//! No production writer or recovery executor exists. A complete observation
//! is still only a point-in-time fact, never permission to replace live files.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::desired::MAX_DESIRED_STATE_BYTES;
use crate::restore_decision_candidate::{DecisionChain, RECORD_BYTES};
use crate::restore_staging_candidate::{InspectError, inspect_stage_identity};
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use zeroize::Zeroizing;

const INTENT_MEMBER: &str = "restore-decision.intent";
const TERMINAL_MEMBER: &str = "restore-decision.terminal";
const DESIRED_MEMBER: &str = "desired.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JournalError {
    Admission,
    Missing,
    UnsafeOrChanged,
    Invalid,
    Stage(InspectError),
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

/// Missing and unsafe are distinct. Every present member is a single-link
/// private regular file, read through a pinned directory without following a
/// symlink, then reopened to detect a concurrent replacement.
fn read_optional(
    directory: &File,
    name: &str,
    uid: u32,
    limit: usize,
    exact_length: bool,
) -> Result<Option<Zeroizing<Vec<u8>>>, JournalError> {
    let fd = match openat(
        directory,
        Path::new(name),
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::ENOENT) => return Ok(None),
        Err(_) => return Err(JournalError::UnsafeOrChanged),
    };
    let mut file = File::from(fd);
    let before = file.metadata().map_err(|_| JournalError::UnsafeOrChanged)?;
    if !before.is_file()
        || before.uid() != uid
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > limit as u64
        || (exact_length && before.len() != limit as u64)
    {
        return Err(JournalError::UnsafeOrChanged);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| JournalError::UnsafeOrChanged)?;
    if bytes.len() as u64 != before.len() {
        return Err(JournalError::UnsafeOrChanged);
    }
    let after = file.metadata().map_err(|_| JournalError::UnsafeOrChanged)?;
    let reopened = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| JournalError::UnsafeOrChanged)?,
    );
    if !same_file(&before, &after)
        || !same_file(
            &before,
            &reopened
                .metadata()
                .map_err(|_| JournalError::UnsafeOrChanged)?,
        )
    {
        return Err(JournalError::UnsafeOrChanged);
    }
    Ok(Some(bytes))
}

/// Inspect only. Even a matching terminal decision cannot authorize cleanup
/// or commit without later fresh live-pair and host checks under the lease.
#[allow(dead_code)]
pub(crate) fn inspect_decision_journal(
    paths: &CutoverPaths,
    uid: u32,
    lock: &MigrationLock,
) -> Result<DecisionChain, JournalError> {
    if !lock.authorizes(paths, uid) {
        return Err(JournalError::Admission);
    }
    let directory = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| JournalError::UnsafeOrChanged)?;
    let directory_before = directory
        .metadata()
        .map_err(|_| JournalError::UnsafeOrChanged)?;
    let intent = read_optional(&directory, INTENT_MEMBER, uid, RECORD_BYTES, true)?;
    let terminal = read_optional(&directory, TERMINAL_MEMBER, uid, RECORD_BYTES, true)?;
    let Some(intent) = intent else {
        return Err(if terminal.is_some() {
            JournalError::Invalid
        } else {
            JournalError::Missing
        });
    };
    let chain = DecisionChain::decode(&intent, terminal.as_ref().map(|bytes| bytes.as_slice()))
        .map_err(|_| JournalError::Invalid)?;
    let marker = read_marker_existing(paths, uid).map_err(|_| JournalError::Admission)?;
    if marker.phase() != OwnershipPhase::Rust {
        return Err(JournalError::Admission);
    }
    let stage = inspect_stage_identity(&paths.state_directory, uid).map_err(JournalError::Stage)?;
    let desired = read_optional(
        &directory,
        DESIRED_MEMBER,
        uid,
        MAX_DESIRED_STATE_BYTES as usize,
        false,
    )?;
    if !chain.active().matches_current_bindings(
        marker.generation(),
        desired.as_ref().map(|bytes| bytes.as_slice()),
        &stage,
    ) {
        return Err(JournalError::Invalid);
    }
    // A snapshot can become stale immediately after return. These final
    // checks only reject replacements detected during this read pass.
    if !same_file(
        &directory_before,
        &open_private_directory(&paths.state_directory, uid)
            .map_err(|_| JournalError::UnsafeOrChanged)?
            .metadata()
            .map_err(|_| JournalError::UnsafeOrChanged)?,
    ) || read_marker_existing(paths, uid).map_err(|_| JournalError::Admission)? != marker
        || inspect_stage_identity(&paths.state_directory, uid)
            .map_err(JournalError::Stage)?
            .digest()
            != stage.digest()
        || read_optional(&directory, INTENT_MEMBER, uid, RECORD_BYTES, true)?
            .as_ref()
            .map(|bytes| bytes.as_slice())
            != Some(intent.as_slice())
        || read_optional(&directory, TERMINAL_MEMBER, uid, RECORD_BYTES, true)? != terminal
        || read_optional(
            &directory,
            DESIRED_MEMBER,
            uid,
            MAX_DESIRED_STATE_BYTES as usize,
            false,
        )? != desired
    {
        return Err(JournalError::UnsafeOrChanged);
    }
    Ok(chain)
}
