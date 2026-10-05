// SPDX-License-Identifier: MIT

//! Inactive fixed-member restore staging. This writes only to a new private
//! pending directory, never to the live store/template. A surviving directory
//! is a refusal/recovery signal, not permission to finish or retry a restore.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::{Mode, mkdirat};
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use zeroize::Zeroizing;

pub(crate) const PENDING_DIRECTORY: &str = "restore-pair.pending";
pub(crate) const READY_MEMBER: &str = "ready.bin";
pub(crate) const READY_MAGIC: &[u8; 8] = b"OVRPAIR1";
pub(crate) const READY_BYTES: usize = 8 + 4 * 4 + 4 * 32;

/// Existence, inaccessible metadata and unexpected entry types all block a
/// second restore attempt. There is deliberately no automatic deletion.
pub(crate) fn staging_pending_at(directory: &Path) -> bool {
    !matches!(
        std::fs::symlink_metadata(directory.join(PENDING_DIRECTORY)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}
pub(crate) const MEMBERS: [&str; 4] = [
    "old-profiles.json",
    "old-route-template.yaml",
    "new-profiles.json",
    "new-route-template.yaml",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageError {
    InvalidPair,
    UnsafeState,
    AlreadyPending,
    /// The private pending directory may exist. Never retry or remove it
    /// automatically after a write/metadata/sync failure.
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Directory,
    Member(usize),
    Ready,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectError {
    MissingOrIncomplete,
    UnsafeOrChanged,
}

/// A point-in-time digest of a fully inspected v1 stage. This is not a
/// transaction identity or authority to replace either live file.
#[derive(Clone, Copy)]
pub(crate) struct StageIdentity([u8; 32]);

impl StageIdentity {
    pub(crate) const fn digest(&self) -> [u8; 32] {
        self.0
    }
}

#[allow(dead_code)]
pub(crate) fn inspect_stage_identity(
    state_directory: &Path,
    uid: u32,
) -> Result<StageIdentity, InspectError> {
    let ready = inspect_ready(state_directory, uid)?;
    Ok(StageIdentity(Sha256::digest(ready).into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LivePairClass {
    Old,
    New,
    Identical,
    Mixed,
    Diverged,
}

/// Both values come from one inspected stage and live-pair pass under the
/// matching migration lease. A detached enum must not authorize recovery.
pub(crate) struct VerifiedLivePair {
    stage: StageIdentity,
    class: LivePairClass,
}

/// Exact staged bytes retained only in zeroizing memory. This does not grant
/// restore authority: callers must also prove the owner and host admission.
pub(crate) struct VerifiedStage {
    identity: StageIdentity,
    members: [Zeroizing<Vec<u8>>; 4],
}

impl VerifiedStage {
    pub(crate) const fn identity(&self) -> &StageIdentity {
        &self.identity
    }

    pub(crate) fn old_store(&self) -> &[u8] {
        &self.members[0]
    }

    pub(crate) fn old_template(&self) -> &[u8] {
        &self.members[1]
    }

    pub(crate) fn new_store(&self) -> &[u8] {
        &self.members[2]
    }

    pub(crate) fn new_template(&self) -> &[u8] {
        &self.members[3]
    }
}

impl VerifiedLivePair {
    pub(crate) const fn stage(&self) -> &StageIdentity {
        &self.stage
    }

    pub(crate) const fn class(&self) -> LivePairClass {
        self.class
    }

    #[cfg(test)]
    pub(crate) const fn synthetic(stage: StageIdentity, class: LivePairClass) -> Self {
        Self { stage, class }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassifyError {
    Admission,
    Stage(InspectError),
    UnsafeLive,
}

fn exact_directory(metadata: &Metadata, uid: u32) -> bool {
    metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o700
}

pub(crate) fn same_directory(before: &Metadata, after: &Metadata) -> bool {
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.mode() == after.mode()
}

pub(crate) fn same_member(before: &Metadata, after: &Metadata) -> bool {
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.mode() == after.mode()
        && before.nlink() == after.nlink()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

fn ready_bytes(members: [&[u8]; 4]) -> [u8; READY_BYTES] {
    let mut ready = [0_u8; READY_BYTES];
    ready[..8].copy_from_slice(READY_MAGIC);
    for (index, bytes) in members.iter().enumerate() {
        ready[8 + index * 4..12 + index * 4].copy_from_slice(&(bytes.len() as u32).to_be_bytes());
        ready[24 + index * 32..56 + index * 32].copy_from_slice(&Sha256::digest(bytes));
    }
    ready
}

/// Compute the exact identity that a future stage would publish, without
/// writing it. A successor handoff can bind this plan before stage creation;
/// it must still inspect and compare the actual stage after publication.
pub(crate) fn planned_stage_identity(members: [&[u8]; 4]) -> Result<StageIdentity, StageError> {
    for (index, bytes) in members.iter().enumerate() {
        let limit = if index % 2 == 0 {
            MAX_PRIVATE_STORE_BYTES
        } else {
            MAX_TEMPLATE_BYTES
        };
        if bytes.is_empty() || bytes.len() > limit {
            return Err(StageError::InvalidPair);
        }
    }
    Ok(StageIdentity(Sha256::digest(ready_bytes(members)).into()))
}

fn matches_member(bytes: &[u8], ready: &[u8; READY_BYTES], index: usize) -> bool {
    ready[8 + index * 4..12 + index * 4] == (bytes.len() as u32).to_be_bytes()
        && ready[24 + index * 32..56 + index * 32] == Sha256::digest(bytes)[..]
}

fn class_from_matches(
    old_store: bool,
    old_template: bool,
    new_store: bool,
    new_template: bool,
) -> LivePairClass {
    match (old_store && old_template, new_store && new_template) {
        (true, true) => LivePairClass::Identical,
        (true, false) => LivePairClass::Old,
        (false, true) => LivePairClass::New,
        (false, false) if (old_store || new_store) && (old_template || new_template) => {
            LivePairClass::Mixed
        }
        (false, false) => LivePairClass::Diverged,
    }
}

pub(crate) fn read_member(
    directory: &File,
    name: &str,
    uid: u32,
    limit: usize,
) -> Result<Zeroizing<Vec<u8>>, InspectError> {
    let mut file = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| InspectError::MissingOrIncomplete)?,
    );
    let before = file.metadata().map_err(|_| InspectError::UnsafeOrChanged)?;
    if !before.is_file()
        || before.uid() != uid
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > limit as u64
    {
        return Err(InspectError::UnsafeOrChanged);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| InspectError::UnsafeOrChanged)?;
    if bytes.len() as u64 != before.len() {
        return Err(InspectError::UnsafeOrChanged);
    }
    let after = file.metadata().map_err(|_| InspectError::UnsafeOrChanged)?;
    let reopened = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| InspectError::UnsafeOrChanged)?,
    );
    if !same_member(&before, &after)
        || !same_member(
            &before,
            &reopened
                .metadata()
                .map_err(|_| InspectError::UnsafeOrChanged)?,
        )
    {
        return Err(InspectError::UnsafeOrChanged);
    }
    Ok(bytes)
}

/// Reopen all four staged members after the complete-stage inspection, then
/// repeat that inspection and the directory identity check. A stale snapshot
/// is never sufficient for a later live-file effect.
pub(crate) fn read_staged_pair(
    state_directory: &Path,
    uid: u32,
) -> Result<VerifiedStage, InspectError> {
    let ready = inspect_ready(state_directory, uid)?;
    let parent =
        open_private_directory(state_directory, uid).map_err(|_| InspectError::UnsafeOrChanged)?;
    let parent_before = parent
        .metadata()
        .map_err(|_| InspectError::UnsafeOrChanged)?;
    let directory = File::from(
        openat(
            &parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| InspectError::MissingOrIncomplete)?,
    );
    let before = directory
        .metadata()
        .map_err(|_| InspectError::UnsafeOrChanged)?;
    if !exact_directory(&before, uid) {
        return Err(InspectError::UnsafeOrChanged);
    }
    let members = [
        read_member(&directory, MEMBERS[0], uid, MAX_PRIVATE_STORE_BYTES)?,
        read_member(&directory, MEMBERS[1], uid, MAX_TEMPLATE_BYTES)?,
        read_member(&directory, MEMBERS[2], uid, MAX_PRIVATE_STORE_BYTES)?,
        read_member(&directory, MEMBERS[3], uid, MAX_TEMPLATE_BYTES)?,
    ];
    if members
        .iter()
        .enumerate()
        .any(|(index, bytes)| !matches_member(bytes, &ready, index))
        || !same_directory(
            &before,
            &File::from(
                openat(
                    &parent,
                    Path::new(PENDING_DIRECTORY),
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| InspectError::UnsafeOrChanged)?,
            )
            .metadata()
            .map_err(|_| InspectError::UnsafeOrChanged)?,
        )
        || !same_directory(
            &parent_before,
            &open_private_directory(state_directory, uid)
                .map_err(|_| InspectError::UnsafeOrChanged)?
                .metadata()
                .map_err(|_| InspectError::UnsafeOrChanged)?,
        )
        || inspect_ready(state_directory, uid)? != ready
    {
        return Err(InspectError::UnsafeOrChanged);
    }
    Ok(VerifiedStage {
        identity: StageIdentity(Sha256::digest(ready).into()),
        members,
    })
}

/// Read-only integrity inspection. A ready marker is a checksum of the exact
/// staged bytes, not proof of owner admission or permission to commit them.
#[allow(dead_code)]
pub(crate) fn inspect_staged_pair(state_directory: &Path, uid: u32) -> Result<(), InspectError> {
    inspect_ready(state_directory, uid).map(|_| ())
}

fn inspect_ready(state_directory: &Path, uid: u32) -> Result<[u8; READY_BYTES], InspectError> {
    let parent =
        open_private_directory(state_directory, uid).map_err(|_| InspectError::UnsafeOrChanged)?;
    let parent_before = parent
        .metadata()
        .map_err(|_| InspectError::UnsafeOrChanged)?;
    let directory = File::from(
        openat(
            &parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| InspectError::MissingOrIncomplete)?,
    );
    let directory_before = directory
        .metadata()
        .map_err(|_| InspectError::UnsafeOrChanged)?;
    if !exact_directory(&directory_before, uid) {
        return Err(InspectError::UnsafeOrChanged);
    }
    let ready = read_member(&directory, READY_MEMBER, uid, READY_BYTES)?;
    if ready.len() != READY_BYTES || &ready[..8] != READY_MAGIC {
        return Err(InspectError::MissingOrIncomplete);
    }
    let mut signature = [0_u8; READY_BYTES];
    signature.copy_from_slice(&ready);
    for (index, name) in MEMBERS.iter().enumerate() {
        let limit = if index % 2 == 0 {
            MAX_PRIVATE_STORE_BYTES
        } else {
            MAX_TEMPLATE_BYTES
        };
        let bytes = read_member(&directory, name, uid, limit)?;
        if !matches_member(&bytes, &signature, index) {
            return Err(InspectError::UnsafeOrChanged);
        }
    }
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| InspectError::UnsafeOrChanged)?;
    exact_stage_catalogue(
        entries.map(|entry| entry.map(|value| value.file_name())),
        || Ok(()),
    )?;
    let current = File::from(
        openat(
            &parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| InspectError::UnsafeOrChanged)?,
    );
    if !same_directory(
        &directory_before,
        &current
            .metadata()
            .map_err(|_| InspectError::UnsafeOrChanged)?,
    ) || !same_directory(
        &parent_before,
        &open_private_directory(state_directory, uid)
            .map_err(|_| InspectError::UnsafeOrChanged)?
            .metadata()
            .map_err(|_| InspectError::UnsafeOrChanged)?,
    ) {
        return Err(InspectError::UnsafeOrChanged);
    }
    Ok(signature)
}

/// The same exact five-member equality, without collecting a foreign directory
/// before checking its size. A positive catalogue consumes the complete iterator;
/// an unknown/duplicate/sixth entry refuses before downstream work. Actor callers
/// must retain the actual iterator and pass their sampled pre/post deadline gate.
fn exact_stage_catalogue<E>(
    mut entries: impl Iterator<Item = Result<std::ffi::OsString, E>>,
    mut gate: impl FnMut() -> Result<(), InspectError>,
) -> Result<(), InspectError> {
    let expected = [MEMBERS[0], MEMBERS[1], MEMBERS[2], MEMBERS[3], READY_MEMBER];
    let mut seen = [false; 5];
    let mut count = 0;
    loop {
        gate()?;
        let entry = entries.next();
        gate()?;
        let Some(entry) = entry else {
            break;
        };
        if count == 5 {
            return Err(InspectError::UnsafeOrChanged);
        }
        count += 1;
        let name = entry.map_err(|_| InspectError::UnsafeOrChanged)?;
        let Some(index) = expected.iter().position(|expected| name == *expected) else {
            return Err(InspectError::UnsafeOrChanged);
        };
        if seen[index] {
            return Err(InspectError::UnsafeOrChanged);
        }
        seen[index] = true;
    }
    if count != 5 || seen != [true; 5] {
        return Err(InspectError::UnsafeOrChanged);
    }
    Ok(())
}

/// Point-in-time classification only; never a restore/rollback authority.
/// The matching owner lease excludes cooperating writers, but a foreign
/// same-user writer and later mutation remain outside this observation.
#[allow(dead_code)]
pub(crate) fn classify_live_pair(
    config_directory: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<LivePairClass, ClassifyError> {
    classify_live_pair_bound(config_directory, paths, uid, generation, lock)
        .map(|pair| pair.class())
}

#[allow(dead_code)]
pub(crate) fn classify_live_pair_bound(
    config_directory: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<VerifiedLivePair, ClassifyError> {
    if !lock.authorizes(paths, uid)
        || !read_marker_existing(paths, uid).is_ok_and(|marker| {
            marker.phase() == OwnershipPhase::Rust && marker.generation() == generation
        })
    {
        return Err(ClassifyError::Admission);
    }
    let ready = inspect_ready(&paths.state_directory, uid).map_err(ClassifyError::Stage)?;
    let config =
        open_private_directory(config_directory, uid).map_err(|_| ClassifyError::UnsafeLive)?;
    let before = config.metadata().map_err(|_| ClassifyError::UnsafeLive)?;
    let store = read_member(&config, "profiles.json", uid, MAX_PRIVATE_STORE_BYTES)
        .map_err(|_| ClassifyError::UnsafeLive)?;
    let template = read_member(&config, "route-template.yaml", uid, MAX_TEMPLATE_BYTES)
        .map_err(|_| ClassifyError::UnsafeLive)?;
    let after =
        open_private_directory(config_directory, uid).map_err(|_| ClassifyError::UnsafeLive)?;
    if !same_directory(
        &before,
        &after.metadata().map_err(|_| ClassifyError::UnsafeLive)?,
    ) {
        return Err(ClassifyError::UnsafeLive);
    }
    if !read_marker_existing(paths, uid).is_ok_and(|marker| {
        marker.phase() == OwnershipPhase::Rust && marker.generation() == generation
    }) {
        return Err(ClassifyError::Admission);
    }
    if inspect_ready(&paths.state_directory, uid).map_err(ClassifyError::Stage)? != ready {
        return Err(ClassifyError::Stage(InspectError::UnsafeOrChanged));
    }
    let old_store = matches_member(&store, &ready, 0);
    let old_template = matches_member(&template, &ready, 1);
    let new_store = matches_member(&store, &ready, 2);
    let new_template = matches_member(&template, &ready, 3);
    Ok(VerifiedLivePair {
        stage: StageIdentity(Sha256::digest(ready).into()),
        class: class_from_matches(old_store, old_template, new_store, new_template),
    })
}

fn write_member(directory: &File, name: &str, uid: u32, bytes: &[u8]) -> Result<File, StageError> {
    let mut file = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| StageError::Ambiguous)?,
    );
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|_| StageError::Ambiguous)?;
    let metadata = file.metadata().map_err(|_| StageError::Ambiguous)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(StageError::Ambiguous);
    }
    file.write_all(bytes).map_err(|_| StageError::Ambiguous)?;
    file.sync_all().map_err(|_| StageError::Ambiguous)?;
    if file.metadata().map_err(|_| StageError::Ambiguous)?.len() != bytes.len() as u64 {
        return Err(StageError::Ambiguous);
    }
    Ok(file)
}

/// Non-cloneable evidence of this writer's exclusive creations, not a general
/// pending-state exception. Original descriptors survive the final callback.
pub(crate) struct CreatedStage {
    parent: File,
    parent_before: Metadata,
    directory: File,
    directory_before: Metadata,
    members: Vec<(&'static str, File, Metadata)>,
    planned: StageIdentity,
    complete: bool,
}

impl CreatedStage {
    pub(crate) fn recheck(&self, state: &Path, uid: u32) -> Result<(), StageError> {
        let refuse = StageError::Ambiguous;
        let parent = open_private_directory(state, uid).map_err(|_| refuse)?;
        let directory = File::from(
            openat(
                &parent,
                Path::new(PENDING_DIRECTORY),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| refuse)?,
        );
        for actual in [&self.parent, &parent] {
            if !same_directory(&self.parent_before, &actual.metadata().map_err(|_| refuse)?) {
                return Err(refuse);
            }
        }
        for actual in [&self.directory, &directory] {
            if !same_directory(
                &self.directory_before,
                &actual.metadata().map_err(|_| refuse)?,
            ) {
                return Err(refuse);
            }
        }
        for (name, held, before) in &self.members {
            let current = File::from(
                openat(
                    &directory,
                    Path::new(name),
                    OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| refuse)?,
            );
            if !same_member(before, &held.metadata().map_err(|_| refuse)?)
                || !same_member(before, &current.metadata().map_err(|_| refuse)?)
            {
                return Err(refuse);
            }
        }
        let mut count = 0;
        for entry in std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| refuse)?
        {
            let entry = entry.map_err(|_| refuse)?;
            count += 1;
            if count > 5
                || !self
                    .members
                    .iter()
                    .any(|(name, _, _)| entry.file_name() == *name)
            {
                return Err(refuse);
            }
        }
        if count != self.members.len()
            || (self.complete
                && inspect_stage_identity(state, uid)
                    .map_err(|_| refuse)?
                    .digest()
                    != self.planned.digest())
        {
            return Err(refuse);
        }
        Ok(())
    }
}

pub(crate) fn stage_owned_checked(
    state: &Path,
    uid: u32,
    members: [&[u8]; 4],
    mut check: impl FnMut(&CreatedStage) -> bool,
) -> Result<CreatedStage, StageError> {
    stage_owned_with_hook(state, uid, members, |created, _| check(created))
}

/// Preserve exact old and authenticated new bytes under five fixed names.
/// This primitive has no product caller and does not validate restore
/// admission. The owner must hold its exclusive lease, recheck generation and
/// source facts, and later implement durable commit/rollback separately.
#[allow(dead_code)]
pub(crate) fn stage_private_pair(
    state_directory: &Path,
    uid: u32,
    old_store: &[u8],
    old_template: &[u8],
    new_store: &[u8],
    new_template: &[u8],
) -> Result<(), StageError> {
    stage_with_hook(
        state_directory,
        uid,
        [old_store, old_template, new_store, new_template],
        |_| true,
    )
}

/// Internal composition seam: after each fixed publication boundary the
/// enclosing owner can recheck its independent lease/host/record bindings.
pub(crate) fn stage_private_pair_checked(
    state_directory: &Path,
    uid: u32,
    members: [&[u8]; 4],
    mut check: impl FnMut() -> bool,
) -> Result<(), StageError> {
    stage_with_hook(state_directory, uid, members, |_| check())
}

fn stage_with_hook(
    state_directory: &Path,
    uid: u32,
    members: [&[u8]; 4],
    mut proceed: impl FnMut(Step) -> bool,
) -> Result<(), StageError> {
    stage_owned_with_hook(state_directory, uid, members, |_, step| proceed(step)).map(drop)
}

fn stage_owned_with_hook(
    state_directory: &Path,
    uid: u32,
    members: [&[u8]; 4],
    mut proceed: impl FnMut(&CreatedStage, Step) -> bool,
) -> Result<CreatedStage, StageError> {
    let planned = planned_stage_identity(members)?;
    if crate::restore_disposition_ticket_model::pending_at(state_directory)
        || crate::restore_disposition_complete_model::pending_at(state_directory)
    {
        return Err(StageError::AlreadyPending);
    }
    let parent =
        open_private_directory(state_directory, uid).map_err(|_| StageError::UnsafeState)?;
    let parent_before = parent.metadata().map_err(|_| StageError::UnsafeState)?;
    match mkdirat(&parent, PENDING_DIRECTORY, Mode::S_IRWXU) {
        Ok(()) => {}
        Err(Errno::EEXIST) => return Err(StageError::AlreadyPending),
        // An I/O failure does not prove the directory was never created.
        // Keep the fixed name fenced until an independent recovery decision.
        Err(_) => return Err(StageError::Ambiguous),
    }
    // Once the fixed name exists, every failure is ambiguous. Keep the
    // directory, even if it is partial, so nothing retries over it blindly.
    parent.sync_all().map_err(|_| StageError::Ambiguous)?;
    let directory = File::from(
        openat(
            &parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| StageError::Ambiguous)?,
    );
    let directory_before = directory.metadata().map_err(|_| StageError::Ambiguous)?;
    if !exact_directory(&directory_before, uid) {
        return Err(StageError::Ambiguous);
    }
    let mut created = CreatedStage {
        parent,
        parent_before,
        directory,
        directory_before,
        members: Vec::with_capacity(5),
        planned,
        complete: false,
    };
    if !proceed(&created, Step::Directory)
        || crate::restore_disposition_ticket_model::pending_at(state_directory)
        || crate::restore_disposition_complete_model::pending_at(state_directory)
    {
        return Err(StageError::Ambiguous);
    }
    created.recheck(state_directory, uid)?;
    for (index, bytes) in members.iter().enumerate() {
        let file = write_member(&created.directory, MEMBERS[index], uid, bytes)?;
        let metadata = file.metadata().map_err(|_| StageError::Ambiguous)?;
        created.members.push((MEMBERS[index], file, metadata));
        if !proceed(&created, Step::Member(index))
            || crate::restore_disposition_ticket_model::pending_at(state_directory)
            || crate::restore_disposition_complete_model::pending_at(state_directory)
        {
            return Err(StageError::Ambiguous);
        }
        created.recheck(state_directory, uid)?;
    }
    created
        .directory
        .sync_all()
        .map_err(|_| StageError::Ambiguous)?;
    let ready = ready_bytes(members);
    let file = write_member(&created.directory, READY_MEMBER, uid, &ready)?;
    let metadata = file.metadata().map_err(|_| StageError::Ambiguous)?;
    created.members.push((READY_MEMBER, file, metadata));
    created.complete = true;
    if !proceed(&created, Step::Ready)
        || crate::restore_disposition_ticket_model::pending_at(state_directory)
        || crate::restore_disposition_complete_model::pending_at(state_directory)
    {
        return Err(StageError::Ambiguous);
    }
    created.recheck(state_directory, uid)?;
    created
        .directory
        .sync_all()
        .map_err(|_| StageError::Ambiguous)?;
    if !proceed(&created, Step::Complete)
        || crate::restore_disposition_ticket_model::pending_at(state_directory)
        || crate::restore_disposition_complete_model::pending_at(state_directory)
    {
        return Err(StageError::Ambiguous);
    }
    let current = File::from(
        openat(
            &created.parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| StageError::Ambiguous)?,
    );
    if !same_directory(
        &created.directory_before,
        &current.metadata().map_err(|_| StageError::Ambiguous)?,
    ) || !same_directory(
        &created.parent_before,
        &open_private_directory(state_directory, uid)
            .map_err(|_| StageError::Ambiguous)?
            .metadata()
            .map_err(|_| StageError::Ambiguous)?,
    ) {
        return Err(StageError::Ambiguous);
    }
    created
        .parent
        .sync_all()
        .map_err(|_| StageError::Ambiguous)?;
    inspect_staged_pair(state_directory, uid).map_err(|_| StageError::Ambiguous)?;
    created.recheck(state_directory, uid)?;
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn root() -> (std::path::PathBuf, u32) {
        let home = std::env::var_os("HOME").expect("restore-stage test needs home");
        let root = crate::test_temp::directory_under(Path::new(&home), "restore-stage").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    const PAIR: [&[u8]; 4] = [b"old store", b"old template", b"new store", b"new template"];

    fn catalogue_names() -> Vec<std::ffi::OsString> {
        MEMBERS
            .iter()
            .copied()
            .chain([READY_MEMBER])
            .map(std::ffi::OsString::from)
            .collect()
    }

    #[test]
    fn finite_catalogue_accepts_every_whole_order_not_just_a_matching_prefix() {
        fn permutations(names: &mut [std::ffi::OsString], offset: usize, visits: &mut usize) {
            if offset == names.len() {
                let mut nexts = 0;
                let mut entries = names
                    .iter()
                    .cloned()
                    .map(Ok::<_, ()>)
                    .inspect(|_| nexts += 1);
                assert_eq!(exact_stage_catalogue(&mut entries, || Ok(())), Ok(()));
                assert_eq!(nexts, 5);
                *visits += 1;
                return;
            }
            for index in offset..names.len() {
                names.swap(offset, index);
                permutations(names, offset + 1, visits);
                names.swap(offset, index);
            }
        }
        let mut visits = 0;
        permutations(&mut catalogue_names(), 0, &mut visits);
        assert_eq!(visits, 120);
    }

    #[test]
    fn finite_catalogue_rejects_missing_foreign_duplicate_and_entry_error() {
        let expected = catalogue_names();
        for cut in 0..5 {
            assert!(
                exact_stage_catalogue(expected[..cut].iter().cloned().map(Ok::<_, ()>), || Ok(()))
                    .is_err()
            );
        }
        for index in 0..5 {
            let mut changed = expected.clone();
            changed[index] = "foreign".into();
            assert!(
                exact_stage_catalogue(changed.into_iter().map(Ok::<_, ()>), || Ok(())).is_err()
            );
            let mut duplicated = expected.clone();
            duplicated[index] = expected[(index + 1) % 5].clone();
            assert!(
                exact_stage_catalogue(duplicated.into_iter().map(Ok::<_, ()>), || Ok(())).is_err()
            );
            let mut failed = expected.iter().cloned().map(Ok).collect::<Vec<_>>();
            failed[index] = Err(());
            assert!(exact_stage_catalogue(failed.into_iter(), || Ok(())).is_err());
        }
    }

    #[test]
    fn finite_catalogue_samples_every_next_including_eof_and_stops_at_first_failure() {
        use std::cell::Cell;
        // Five actual entries plus the required terminal next, two guards each.
        for cut in 0_usize..12 {
            let checks = Cell::new(0);
            let nexts = Cell::new(0);
            let mut names = catalogue_names().into_iter();
            let entries = std::iter::from_fn(|| {
                nexts.set(nexts.get() + 1);
                names.next().map(Ok::<_, ()>)
            });
            assert!(
                exact_stage_catalogue(entries, || {
                    let check = checks.get();
                    checks.set(check + 1);
                    if check == cut {
                        Err(InspectError::UnsafeOrChanged)
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            assert_eq!(checks.get(), cut + 1);
            assert_eq!(nexts.get(), cut.div_ceil(2));
        }
        let nexts = Cell::new(0);
        let mut expected = catalogue_names().into_iter();
        let entries = std::iter::from_fn(|| {
            nexts.set(nexts.get() + 1);
            if nexts.get() <= 5 {
                expected.next().map(Ok::<_, ()>)
            } else {
                Some(Ok("foreign".into()))
            }
        });
        assert!(exact_stage_catalogue(entries, || Ok(())).is_err());
        assert_eq!(nexts.get(), 6); // no unbounded seventh next or downstream work
    }

    #[test]
    fn disposition_records_block_direct_stage_initial_and_callback_prefix() {
        for (member, late) in [
            (
                crate::restore_disposition_ticket_model::TICKET_MEMBER,
                false,
            ),
            (crate::restore_disposition_ticket_model::TICKET_MEMBER, true),
            (
                crate::restore_disposition_complete_model::COMPLETE_MEMBER,
                false,
            ),
            (
                crate::restore_disposition_complete_model::COMPLETE_MEMBER,
                true,
            ),
        ] {
            let (root, uid) = root();
            let ticket = root.join(member);
            if !late {
                fs::write(&ticket, b"foreign").unwrap();
            }
            let result = stage_with_hook(&root, uid, PAIR, |step| {
                if late && matches!(step, Step::Directory) {
                    fs::write(&ticket, b"late").unwrap();
                }
                true
            });
            assert!(result.is_err());
            assert!(!root.join(PENDING_DIRECTORY).join(MEMBERS[0]).exists());
            if late {
                fs::remove_dir(root.join(PENDING_DIRECTORY)).unwrap();
            }
            fs::remove_file(ticket).unwrap();
            fs::remove_dir(root).unwrap();
        }
    }

    #[test]
    fn classification_distinguishes_identical_mixed_and_diverged_pairs() {
        assert_eq!(
            class_from_matches(true, true, true, true),
            LivePairClass::Identical
        );
        assert_eq!(
            class_from_matches(true, true, false, false),
            LivePairClass::Old
        );
        assert_eq!(
            class_from_matches(false, false, true, true),
            LivePairClass::New
        );
        assert_eq!(
            class_from_matches(true, false, false, true),
            LivePairClass::Mixed
        );
        assert_eq!(
            class_from_matches(false, false, false, false),
            LivePairClass::Diverged
        );
        assert_eq!(
            class_from_matches(true, false, false, false),
            LivePairClass::Diverged
        );
    }

    #[test]
    fn stages_only_fixed_private_members_and_refuses_second_attempt() {
        let (root, uid) = root();
        let planned = planned_stage_identity(PAIR).unwrap();
        assert_eq!(
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
            Ok(())
        );
        let staged = root.join(PENDING_DIRECTORY);
        assert_eq!(
            fs::symlink_metadata(&staged).unwrap().mode() & 0o7777,
            0o700
        );
        let mut entries = fs::read_dir(&staged)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        entries.sort();
        let mut expected = MEMBERS.map(std::ffi::OsString::from).to_vec();
        expected.push(READY_MEMBER.into());
        expected.sort();
        assert_eq!(entries, expected);
        for (name, bytes) in MEMBERS.iter().zip(PAIR) {
            let file = staged.join(name);
            let metadata = fs::symlink_metadata(&file).unwrap();
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
            assert!(fs::read(file).unwrap() == bytes);
        }
        assert_eq!(inspect_staged_pair(&root, uid), Ok(()));
        assert_eq!(
            inspect_stage_identity(&root, uid).unwrap().digest(),
            planned.digest()
        );
        assert_eq!(
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
            Err(StageError::AlreadyPending)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_staging_is_ambiguous_and_never_auto_removed() {
        for stop in [
            Step::Directory,
            Step::Member(0),
            Step::Member(2),
            Step::Ready,
            Step::Complete,
        ] {
            let (root, uid) = root();
            assert_eq!(
                stage_with_hook(&root, uid, PAIR, |step| step != stop),
                Err(StageError::Ambiguous)
            );
            assert!(root.join(PENDING_DIRECTORY).is_dir());
            if !matches!(stop, Step::Ready | Step::Complete) {
                assert_ne!(inspect_staged_pair(&root, uid), Ok(()));
            }
            assert_eq!(
                stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
                Err(StageError::AlreadyPending)
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn unsafe_or_existing_targets_and_invalid_sizes_have_no_new_members() {
        let (root, uid) = root();
        assert_eq!(
            stage_private_pair(&root, uid, b"", PAIR[1], PAIR[2], PAIR[3]),
            Err(StageError::InvalidPair)
        );
        assert_eq!(
            stage_private_pair(
                &root,
                uid.wrapping_add(1),
                PAIR[0],
                PAIR[1],
                PAIR[2],
                PAIR[3]
            ),
            Err(StageError::UnsafeState)
        );
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        symlink(&target, root.join(PENDING_DIRECTORY)).unwrap();
        assert_eq!(
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
            Err(StageError::AlreadyPending)
        );
        assert!(fs::read_dir(target).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspector_refuses_tampering_missing_members_and_unexpected_entries() {
        for mutation in 0..5 {
            let (root, uid) = root();
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]).unwrap();
            let staged = root.join(PENDING_DIRECTORY);
            match mutation {
                0 => fs::write(staged.join(MEMBERS[0]), b"new store").unwrap(),
                1 => fs::write(staged.join(READY_MEMBER), b"invalid marker").unwrap(),
                2 => fs::remove_file(staged.join(MEMBERS[1])).unwrap(),
                3 => fs::write(staged.join("unexpected.txt"), b"extra").unwrap(),
                _ => {
                    let member = staged.join(MEMBERS[2]);
                    fs::hard_link(&member, staged.join("other-link")).unwrap();
                }
            }
            assert_ne!(inspect_staged_pair(&root, uid), Ok(()));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn inspector_refuses_public_and_symlinked_members() {
        for replace_with_symlink in [false, true] {
            let (root, uid) = root();
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]).unwrap();
            let staged = root.join(PENDING_DIRECTORY);
            let member = staged.join(MEMBERS[3]);
            fs::remove_file(&member).unwrap();
            if replace_with_symlink {
                symlink(staged.join(MEMBERS[1]), member).unwrap();
            } else {
                fs::write(&member, PAIR[3]).unwrap();
                fs::set_permissions(&member, fs::Permissions::from_mode(0o644)).unwrap();
            }
            assert_ne!(inspect_staged_pair(&root, uid), Ok(()));
            fs::remove_dir_all(root).unwrap();
        }
    }
}
