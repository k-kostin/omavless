// SPDX-License-Identifier: MIT

//! Inactive fixed-artifact retirement and final receipt closure. The pending
//! receipt may be retired only after a separate durable completion record is
//! bound; that record remains a startup/mutation fence. No product caller.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::restore_closure_model::{CLOSURE_MEMBER, ClosureRecord, RECORD_BYTES as CLOSURE_BYTES};
use crate::restore_decision_candidate::{DecisionRecord, RECORD_BYTES};
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_journal_candidate::read_desired_for_decision;
use crate::restore_retirement_candidate::{
    RECEIPT_MEMBER, RetirementReceipt, durable_retirement_receipt,
};
use crate::restore_staging_candidate::{
    MEMBERS, PENDING_DIRECTORY, READY_BYTES, READY_MAGIC, READY_MEMBER, same_directory, same_member,
};
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use nix::unistd::{UnlinkatFlags, unlinkat};
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use zeroize::Zeroizing;

const TERMINAL: &str = "restore-decision.terminal";
const INTENT: &str = "restore-decision.intent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupError {
    Admission,
    ManualRecovery,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupResult {
    RetiredStillFenced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalizeError {
    Admission,
    ManualRecovery,
    Ambiguous,
}

impl From<CleanupError> for FinalizeError {
    fn from(error: CleanupError) -> Self {
        match error {
            CleanupError::Admission => Self::Admission,
            CleanupError::ManualRecovery => Self::ManualRecovery,
            CleanupError::Ambiguous => Self::Ambiguous,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalizeResult {
    ReceiptRetiredStillFenced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosurePublicationResult {
    PublishedStillFenced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClosurePublishCheckpoint {
    Created,
    Written,
    FileSynced,
    DirectorySynced,
    Reopened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalizeCheckpoint {
    Unlinked,
    Synced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    StageMember(usize),
    Ready,
    StageDirectory,
    Terminal,
    Intent,
    Done,
}

impl Step {
    fn index(self) -> usize {
        match self {
            Self::StageMember(index) => index,
            Self::Ready => 4,
            Self::StageDirectory => 5,
            Self::Terminal => 6,
            Self::Intent => 7,
            Self::Done => 8,
        }
    }

    fn name(self) -> Option<&'static str> {
        match self {
            Self::StageMember(index) => Some(MEMBERS[index]),
            Self::Ready => Some(READY_MEMBER),
            Self::StageDirectory => Some(PENDING_DIRECTORY),
            Self::Terminal => Some(TERMINAL),
            Self::Intent => Some(INTENT),
            Self::Done => None,
        }
    }

    fn in_stage(self) -> bool {
        matches!(self, Self::StageMember(_) | Self::Ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookPoint {
    Unlinked(Step),
    Synced(Step),
}

struct Observed {
    step: Step,
    target: Option<Metadata>,
    stage_parent: Option<Metadata>,
}

pub(crate) type PrivateMember = (Zeroizing<Vec<u8>>, Metadata);

fn exact_directory(metadata: &Metadata, uid: u32) -> bool {
    metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o700
}

fn replacement_slot_pending(config: &Path) -> bool {
    NEW_SLOT.into_iter().chain(OLD_SLOT).any(|name| {
        !matches!(
            std::fs::symlink_metadata(config.join(name)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        )
    })
}

pub(crate) fn read_optional(
    directory: &File,
    name: &str,
    uid: u32,
    limit: usize,
) -> Result<Option<PrivateMember>, CleanupError> {
    let mut file = match openat(
        directory,
        Path::new(name),
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::ENOENT) => return Ok(None),
        Err(_) => return Err(CleanupError::ManualRecovery),
    };
    let before = file.metadata().map_err(|_| CleanupError::ManualRecovery)?;
    if !before.is_file()
        || before.uid() != uid
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > limit as u64
    {
        return Err(CleanupError::ManualRecovery);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CleanupError::ManualRecovery)?;
    let reopened = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| CleanupError::ManualRecovery)?,
    );
    if bytes.len() as u64 != before.len()
        || !same_member(
            &before,
            &file.metadata().map_err(|_| CleanupError::ManualRecovery)?,
        )
        || !same_member(
            &before,
            &reopened
                .metadata()
                .map_err(|_| CleanupError::ManualRecovery)?,
        )
    {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(Some((bytes, before)))
}

fn names(directory: &File) -> Result<Vec<String>, CleanupError> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| CleanupError::ManualRecovery)?
    {
        let entry = item.map_err(|_| CleanupError::ManualRecovery)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| CleanupError::ManualRecovery)?;
        if !MEMBERS.contains(&name.as_str()) && name != READY_MEMBER {
            return Err(CleanupError::ManualRecovery);
        }
        entries.push(name);
        if entries.len() > 5 {
            return Err(CleanupError::ManualRecovery);
        }
    }
    entries.sort();
    Ok(entries)
}

fn open_stage(state: &File, uid: u32) -> Result<Option<(File, Metadata)>, CleanupError> {
    let stage = match openat(
        state,
        Path::new(PENDING_DIRECTORY),
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::ENOENT) => return Ok(None),
        Err(_) => return Err(CleanupError::ManualRecovery),
    };
    let metadata = stage.metadata().map_err(|_| CleanupError::ManualRecovery)?;
    if !exact_directory(&metadata, uid) {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(Some((stage, metadata)))
}

fn stage_step(
    state: &File,
    uid: u32,
    receipt: &RetirementReceipt,
    before_read: impl FnOnce(&File) -> Result<(), CleanupError>,
) -> Result<Option<Observed>, CleanupError> {
    let Some((stage, identity)) = open_stage(state, uid)? else {
        return Ok(None);
    };
    before_read(&stage)?;
    let before = names(&stage)?;
    let ready = read_optional(&stage, READY_MEMBER, uid, READY_BYTES)?;
    let observed = if let Some((ready, ready_identity)) = ready {
        if ready.len() != READY_BYTES
            || &ready[..8] != READY_MAGIC
            || !receipt.terminal().matches_stage_ready(&ready)
        {
            return Err(CleanupError::ManualRecovery);
        }
        let mut first = None;
        let mut target = None;
        for (index, member) in MEMBERS.iter().enumerate() {
            let limit = if index % 2 == 0 {
                MAX_PRIVATE_STORE_BYTES
            } else {
                MAX_TEMPLATE_BYTES
            };
            match read_optional(&stage, member, uid, limit)? {
                Some((bytes, metadata)) => {
                    let length = u32::from_be_bytes(
                        ready[8 + index * 4..12 + index * 4]
                            .try_into()
                            .map_err(|_| CleanupError::ManualRecovery)?,
                    );
                    let digest = &ready[24 + index * 32..56 + index * 32];
                    if length as usize != bytes.len() || *digest != Sha256::digest(&bytes)[..] {
                        return Err(CleanupError::ManualRecovery);
                    }
                    if first.is_none() {
                        first = Some(index);
                        target = Some(metadata);
                    }
                }
                None if first.is_some() => return Err(CleanupError::ManualRecovery),
                None => {}
            }
        }
        match first {
            Some(index) => Observed {
                step: Step::StageMember(index),
                target,
                stage_parent: Some(identity.clone()),
            },
            None => Observed {
                step: Step::Ready,
                target: Some(ready_identity),
                stage_parent: Some(identity.clone()),
            },
        }
    } else if before.is_empty() {
        Observed {
            step: Step::StageDirectory,
            target: Some(identity.clone()),
            stage_parent: Some(identity.clone()),
        }
    } else {
        return Err(CleanupError::ManualRecovery);
    };
    if names(&stage)? != before
        || !same_directory(
            &identity,
            &stage.metadata().map_err(|_| CleanupError::ManualRecovery)?,
        )
        || !same_directory(
            &identity,
            &open_stage(state, uid)?
                .ok_or(CleanupError::ManualRecovery)?
                .1,
        )
    {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(Some(observed))
}

fn inspect_inventory(
    paths: &CutoverPaths,
    state: &File,
    uid: u32,
    receipt: &RetirementReceipt,
) -> Result<Observed, CleanupError> {
    inventory_sync(state)?;
    inspect_inventory_with_stage_check(paths, state, uid, receipt, inventory_sync)
}

fn inventory_sync(file: &File) -> Result<(), CleanupError> {
    #[cfg(test)]
    FORBID_INVENTORY_SYNC
        .with(|flag| assert!(!flag.get(), "read-only inventory attempted synchronization"));
    file.sync_all().map_err(|_| CleanupError::Ambiguous)
}

#[cfg(test)]
thread_local! { static FORBID_INVENTORY_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[cfg(test)]
pub(crate) fn without_inventory_sync<T>(read: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FORBID_INVENTORY_SYNC.with(|flag| flag.set(false));
        }
    }
    FORBID_INVENTORY_SYNC.with(|flag| assert!(!flag.replace(true)));
    let _reset = Reset;
    read()
}

/// Strict prefix classification only; no sync or filesystem effects.
pub(crate) fn inspect_cleanup_prefix(
    paths: &CutoverPaths,
    state: &File,
    uid: u32,
    receipt: &RetirementReceipt,
) -> Result<Step, CleanupError> {
    Ok(inspect_inventory_with_stage_check(paths, state, uid, receipt, |_| Ok(()))?.step)
}

fn inspect_inventory_with_stage_check(
    paths: &CutoverPaths,
    state: &File,
    uid: u32,
    receipt: &RetirementReceipt,
    before_stage_read: impl FnOnce(&File) -> Result<(), CleanupError>,
) -> Result<Observed, CleanupError> {
    let parent = state.metadata().map_err(|_| CleanupError::ManualRecovery)?;
    let staged = stage_step(state, uid, receipt, before_stage_read)?;
    let terminal = read_optional(state, TERMINAL, uid, RECORD_BYTES)?;
    let intent = read_optional(state, INTENT, uid, RECORD_BYTES)?;
    if let Some((bytes, _)) = &terminal
        && bytes.as_slice() != receipt.terminal().encode()
    {
        return Err(CleanupError::ManualRecovery);
    }
    if let Some((bytes, _)) = &intent {
        let record = DecisionRecord::decode(bytes).map_err(|_| CleanupError::ManualRecovery)?;
        if !record.is_intent_of(receipt.terminal()) {
            return Err(CleanupError::ManualRecovery);
        }
    }
    let observed = match (staged, terminal, intent) {
        (Some(stage), Some(_), Some(_)) => stage,
        (None, Some((_, metadata)), Some(_)) => Observed {
            step: Step::Terminal,
            target: Some(metadata),
            stage_parent: None,
        },
        (None, None, Some((_, metadata))) => Observed {
            step: Step::Intent,
            target: Some(metadata),
            stage_parent: None,
        },
        (None, None, None) => Observed {
            step: Step::Done,
            target: None,
            stage_parent: None,
        },
        _ => return Err(CleanupError::ManualRecovery),
    };
    if !same_directory(
        &parent,
        &open_private_directory(&paths.state_directory, uid)
            .map_err(|_| CleanupError::ManualRecovery)?
            .metadata()
            .map_err(|_| CleanupError::ManualRecovery)?,
    ) {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(observed)
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    state: File,
    state_identity: Metadata,
    receipt_identity: Option<Metadata>,
}

impl<'a> Context<'a> {
    fn new(
        config: &'a Path,
        paths: &'a CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &'a MigrationLock,
    ) -> Result<Self, CleanupError> {
        let state = open_private_directory(&paths.state_directory, uid)
            .map_err(|_| CleanupError::ManualRecovery)?;
        let state_identity = state.metadata().map_err(|_| CleanupError::ManualRecovery)?;
        Ok(Self {
            config,
            paths,
            uid,
            generation,
            lock,
            state,
            state_identity,
            receipt_identity: None,
        })
    }

    fn check_bindings(&mut self) -> Result<Observed, CleanupError> {
        if !self.lock.authorizes(self.paths, self.uid) {
            return Err(CleanupError::Admission);
        }
        let (receipt, identity) = durable_retirement_receipt(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
        )
        .map_err(|_| CleanupError::ManualRecovery)?;
        if replacement_slot_pending(self.config) {
            return Err(CleanupError::ManualRecovery);
        }
        if self
            .receipt_identity
            .as_ref()
            .is_some_and(|before| !same_member(before, &identity))
            || !same_directory(
                &self.state_identity,
                &self
                    .state
                    .metadata()
                    .map_err(|_| CleanupError::ManualRecovery)?,
            )
        {
            return Err(CleanupError::ManualRecovery);
        }
        let observed = inspect_inventory(self.paths, &self.state, self.uid, &receipt)?;
        self.receipt_identity = Some(identity);
        Ok(observed)
    }

    fn check(&mut self, gate: &mut impl FnMut() -> bool) -> Result<Observed, CleanupError> {
        if !gate() {
            return Err(CleanupError::Admission);
        }
        let observed = self.check_bindings()?;
        if !gate() {
            return Err(CleanupError::Admission);
        }
        let after_gate = self.check_bindings()?;
        let same_target = match (&observed.target, &after_gate.target) {
            (Some(before), Some(after)) => same_member(before, after),
            (None, None) => true,
            _ => false,
        };
        let same_stage = match (&observed.stage_parent, &after_gate.stage_parent) {
            (Some(before), Some(after)) => same_directory(before, after),
            (None, None) => true,
            _ => false,
        };
        if after_gate.step != observed.step || !same_target || !same_stage {
            return Err(CleanupError::ManualRecovery);
        }
        Ok(after_gate)
    }
}

/// Remove only the eight fixed artifacts in order, never the receipt. Any
/// hole, extra entry, drift or failed synchronization preserves its fence.
#[allow(dead_code)]
pub(crate) fn retire_fixed_restore_artifacts(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<CleanupResult, CleanupError> {
    retire_with_hook(config, paths, uid, generation, lock, gate, |_| true)
}

fn absent(directory: &File, name: &str) -> Result<bool, FinalizeError> {
    match openat(
        directory,
        Path::new(name),
        OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Err(Errno::ENOENT) => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(FinalizeError::ManualRecovery),
    }
}

fn read_completion_member(
    state: &File,
    uid: u32,
) -> Result<(ClosureRecord, Metadata), FinalizeError> {
    let (raw, identity) = read_optional(state, CLOSURE_MEMBER, uid, CLOSURE_BYTES)
        .map_err(FinalizeError::from)?
        .ok_or(FinalizeError::ManualRecovery)?;
    let record = ClosureRecord::decode(&raw).map_err(|_| FinalizeError::ManualRecovery)?;
    Ok((record, identity))
}

/// Read-only candidate usable after the pending receipt is gone. It checks
/// both fixed live members and exact owner/desired bindings independently;
/// it does not grant normal-owner startup or permit removing the record.
pub(crate) fn inspect_completion_record(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<ClosureRecord, FinalizeError> {
    if !lock.authorizes(paths, uid) {
        return Err(FinalizeError::Admission);
    }
    let state = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let state_identity = state
        .metadata()
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let config_dir =
        open_private_directory(config, uid).map_err(|_| FinalizeError::ManualRecovery)?;
    let config_identity = config_dir
        .metadata()
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let (record, identity) = read_completion_member(&state, uid)?;
    let marker = read_marker_existing(paths, uid).map_err(|_| FinalizeError::ManualRecovery)?;
    let desired =
        read_desired_for_decision(paths, uid, lock).map_err(|_| FinalizeError::ManualRecovery)?;
    if marker.phase() != OwnershipPhase::Rust
        || marker.generation() != generation
        || !record
            .receipt()
            .terminal()
            .matches_owner_desired(generation, desired.as_ref().map(|value| value.as_slice()))
        || record.receipt().matches_live(config, uid) != Ok(true)
        || !fixed_artifacts_absent(&state, &config_dir)?
    {
        return Err(FinalizeError::ManualRecovery);
    }
    let (again, after) = read_completion_member(&state, uid)?;
    if !same_member(&identity, &after)
        || again.encode() != record.encode()
        || read_marker_existing(paths, uid).ok() != Some(marker)
        || read_desired_for_decision(paths, uid, lock).ok() != Some(desired)
        || record.receipt().matches_live(config, uid) != Ok(true)
        || !fixed_artifacts_absent(&state, &config_dir)?
        || !same_directory(
            &state_identity,
            &open_private_directory(&paths.state_directory, uid)
                .map_err(|_| FinalizeError::ManualRecovery)?
                .metadata()
                .map_err(|_| FinalizeError::ManualRecovery)?,
        )
        || !same_directory(
            &config_identity,
            &open_private_directory(config, uid)
                .map_err(|_| FinalizeError::ManualRecovery)?
                .metadata()
                .map_err(|_| FinalizeError::ManualRecovery)?,
        )
    {
        return Err(FinalizeError::ManualRecovery);
    }
    Ok(record)
}

/// A visible record is not sufficient to authorize removing the older
/// receipt: the member and its directory name must first be synchronized and
/// independently rebound. This also handles a retry after interrupted
/// completion publication, without overwriting the existing record.
fn durable_completion_record(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<(ClosureRecord, Metadata), FinalizeError> {
    let before = inspect_completion_record(config, paths, uid, generation, lock)?;
    let state = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let (_, identity) = read_completion_member(&state, uid)?;
    let file = File::from(
        openat(
            &state,
            Path::new(CLOSURE_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| FinalizeError::ManualRecovery)?,
    );
    if !same_member(
        &identity,
        &file.metadata().map_err(|_| FinalizeError::ManualRecovery)?,
    ) {
        return Err(FinalizeError::ManualRecovery);
    }
    file.sync_all().map_err(|_| FinalizeError::Ambiguous)?;
    state.sync_all().map_err(|_| FinalizeError::Ambiguous)?;
    let after = inspect_completion_record(config, paths, uid, generation, lock)?;
    let (_, rebound) = read_completion_member(&state, uid)?;
    if before.encode() != after.encode() || !same_member(&identity, &rebound) {
        return Err(FinalizeError::ManualRecovery);
    }
    Ok((after, rebound))
}

/// Inactive first half of last-fence closure. It creates no normal-owner
/// authority and leaves the pending receipt intact. Any uncertainty after
/// exclusive creation retains both names as startup fences.
#[allow(dead_code)]
pub(crate) fn publish_completion_record(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<ClosurePublicationResult, FinalizeError> {
    publish_completion_with_hook(config, paths, uid, generation, lock, gate, |_| true)
}

fn publish_completion_with_hook<G: FnMut() -> bool, H: FnMut(ClosurePublishCheckpoint) -> bool>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: G,
    mut hook: H,
) -> Result<ClosurePublicationResult, FinalizeError> {
    let mut context =
        Context::new(config, paths, uid, generation, lock).map_err(FinalizeError::from)?;
    let config_dir =
        open_private_directory(config, uid).map_err(|_| FinalizeError::ManualRecovery)?;
    let observed = context.check(&mut gate).map_err(FinalizeError::from)?;
    if observed.step != Step::Done
        || !fixed_artifacts_absent(&context.state, &config_dir)?
        || !absent(&context.state, CLOSURE_MEMBER)?
    {
        return Err(FinalizeError::ManualRecovery);
    }
    let (pending, pending_identity) =
        durable_retirement_receipt(config, paths, uid, generation, lock)
            .map_err(|_| FinalizeError::ManualRecovery)?;
    if context
        .receipt_identity
        .as_ref()
        .is_none_or(|prior| !same_member(prior, &pending_identity))
    {
        return Err(FinalizeError::ManualRecovery);
    }
    let record = ClosureRecord::from_verified_receipt(&pending)
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let again = context.check(&mut gate).map_err(FinalizeError::from)?;
    if again.step != Step::Done
        || !fixed_artifacts_absent(&context.state, &config_dir)?
        || !absent(&context.state, CLOSURE_MEMBER)?
    {
        return Err(FinalizeError::ManualRecovery);
    }
    let mut file = File::from(
        openat(
            &context.state,
            Path::new(CLOSURE_MEMBER),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| FinalizeError::Ambiguous)?,
    );
    let created = file.metadata().map_err(|_| FinalizeError::Ambiguous)?;
    if !created.is_file()
        || created.uid() != uid
        || created.mode() & 0o7777 != 0o600
        || created.nlink() != 1
        || !hook(ClosurePublishCheckpoint::Created)
    {
        return Err(FinalizeError::Ambiguous);
    }
    file.write_all(&record.encode())
        .map_err(|_| FinalizeError::Ambiguous)?;
    if !hook(ClosurePublishCheckpoint::Written) {
        return Err(FinalizeError::Ambiguous);
    }
    file.sync_all().map_err(|_| FinalizeError::Ambiguous)?;
    if !hook(ClosurePublishCheckpoint::FileSynced) {
        return Err(FinalizeError::Ambiguous);
    }
    context
        .state
        .sync_all()
        .map_err(|_| FinalizeError::Ambiguous)?;
    if !hook(ClosurePublishCheckpoint::DirectorySynced) {
        return Err(FinalizeError::Ambiguous);
    }
    let durable_identity = file.metadata().map_err(|_| FinalizeError::Ambiguous)?;
    let (reopened, identity) =
        read_completion_member(&context.state, uid).map_err(|_| FinalizeError::Ambiguous)?;
    if created.dev() != durable_identity.dev()
        || created.ino() != durable_identity.ino()
        || !same_member(&durable_identity, &identity)
        || !same_member(
            &durable_identity,
            &file.metadata().map_err(|_| FinalizeError::Ambiguous)?,
        )
        || !reopened.matches_pending(&pending)
        || !hook(ClosurePublishCheckpoint::Reopened)
    {
        return Err(FinalizeError::Ambiguous);
    }
    let checked = context
        .check(&mut gate)
        .map_err(|_| FinalizeError::Ambiguous)?;
    let (pending_again, pending_after) =
        durable_retirement_receipt(config, paths, uid, generation, lock)
            .map_err(|_| FinalizeError::Ambiguous)?;
    let (completion_after, completion_identity) =
        read_completion_member(&context.state, uid).map_err(|_| FinalizeError::Ambiguous)?;
    if checked.step != Step::Done
        || !fixed_artifacts_absent(&context.state, &config_dir)
            .map_err(|_| FinalizeError::Ambiguous)?
        || !same_member(&pending_identity, &pending_after)
        || !reopened.matches_pending(&pending_again)
        || completion_after.encode() != reopened.encode()
        || !same_member(&identity, &completion_identity)
        || !same_directory(
            &context.state_identity,
            &open_private_directory(&paths.state_directory, uid)
                .map_err(|_| FinalizeError::Ambiguous)?
                .metadata()
                .map_err(|_| FinalizeError::Ambiguous)?,
        )
    {
        return Err(FinalizeError::Ambiguous);
    }
    Ok(ClosurePublicationResult::PublishedStillFenced)
}

fn fixed_artifacts_absent(state: &File, config: &File) -> Result<bool, FinalizeError> {
    for name in [PENDING_DIRECTORY, TERMINAL, INTENT] {
        if !absent(state, name)? {
            return Ok(false);
        }
    }
    for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
        if !absent(config, name)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Inactive last step after all fixed artifacts are retired and a completion
/// record is durable. This unlinks only the older pending receipt, never the
/// retained completion fence. A crash before directory sync can leave either
/// or both records; neither state admits the ordinary owner.
#[allow(dead_code)]
pub(crate) fn finalize_fenced_restore(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<FinalizeResult, FinalizeError> {
    finalize_with_hook(config, paths, uid, generation, lock, gate, |_| true)
}

fn finalize_with_hook<G: FnMut() -> bool, H: FnMut(FinalizeCheckpoint) -> bool>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: G,
    mut hook: H,
) -> Result<FinalizeResult, FinalizeError> {
    let mut context =
        Context::new(config, paths, uid, generation, lock).map_err(FinalizeError::from)?;
    let config_dir =
        open_private_directory(config, uid).map_err(|_| FinalizeError::ManualRecovery)?;
    let config_identity = config_dir
        .metadata()
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let observed = context.check(&mut gate).map_err(FinalizeError::from)?;
    if observed.step != Step::Done || !fixed_artifacts_absent(&context.state, &config_dir)? {
        return Err(FinalizeError::ManualRecovery);
    }
    let (receipt, identity) = durable_retirement_receipt(config, paths, uid, generation, lock)
        .map_err(|_| FinalizeError::ManualRecovery)?;
    let (completion, completion_identity) =
        durable_completion_record(config, paths, uid, generation, lock)?;
    if context
        .receipt_identity
        .as_ref()
        .is_none_or(|prior| !same_member(prior, &identity))
        || !completion.matches_pending(&receipt)
    {
        return Err(FinalizeError::ManualRecovery);
    }
    // Repeat owner/host and filesystem admission immediately before the only
    // effect. A stale first check must not become permission to remove the
    // final startup fence.
    let again = context.check(&mut gate).map_err(FinalizeError::from)?;
    let (checked_completion, completion_before) =
        durable_completion_record(config, paths, uid, generation, lock)?;
    if again.step != Step::Done
        || !fixed_artifacts_absent(&context.state, &config_dir)?
        || !same_member(&completion_identity, &completion_before)
        || !checked_completion.matches_pending(&receipt)
    {
        return Err(FinalizeError::ManualRecovery);
    }
    let entry = File::from(
        openat(
            &context.state,
            Path::new(RECEIPT_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| FinalizeError::ManualRecovery)?,
    );
    if !same_member(
        &identity,
        &entry
            .metadata()
            .map_err(|_| FinalizeError::ManualRecovery)?,
    ) || !same_directory(
        &config_identity,
        &open_private_directory(config, uid)
            .map_err(|_| FinalizeError::ManualRecovery)?
            .metadata()
            .map_err(|_| FinalizeError::ManualRecovery)?,
    ) {
        return Err(FinalizeError::ManualRecovery);
    }
    if !gate() {
        return Err(FinalizeError::Admission);
    }
    let final_bindings = context.check_bindings().map_err(FinalizeError::from)?;
    let (completion_last, completion_last_identity) = read_completion_member(&context.state, uid)?;
    if final_bindings.step != Step::Done
        || !fixed_artifacts_absent(&context.state, &config_dir)?
        || !same_member(&completion_identity, &completion_last_identity)
        || !completion_last.matches_pending(&receipt)
        || !same_member(
            &identity,
            &entry
                .metadata()
                .map_err(|_| FinalizeError::ManualRecovery)?,
        )
    {
        return Err(FinalizeError::ManualRecovery);
    }
    unlinkat(
        &context.state,
        Path::new(RECEIPT_MEMBER),
        UnlinkatFlags::NoRemoveDir,
    )
    .map_err(|_| FinalizeError::Ambiguous)?;
    if !hook(FinalizeCheckpoint::Unlinked) {
        return Err(FinalizeError::Ambiguous);
    }
    context
        .state
        .sync_all()
        .map_err(|_| FinalizeError::Ambiguous)?;
    if !hook(FinalizeCheckpoint::Synced) {
        return Err(FinalizeError::Ambiguous);
    }
    let marker = read_marker_existing(paths, uid).map_err(|_| FinalizeError::Ambiguous)?;
    let desired =
        read_desired_for_decision(paths, uid, lock).map_err(|_| FinalizeError::Ambiguous)?;
    let completed = inspect_completion_record(config, paths, uid, generation, lock)
        .map_err(|_| FinalizeError::Ambiguous)?;
    let (_, completed_identity) =
        read_completion_member(&context.state, uid).map_err(|_| FinalizeError::Ambiguous)?;
    if !lock.authorizes(paths, uid)
        || marker.phase() != OwnershipPhase::Rust
        || marker.generation() != generation
        || !receipt
            .terminal()
            .matches_owner_desired(generation, desired.as_ref().map(|value| value.as_slice()))
        || receipt.matches_live(config, uid) != Ok(true)
        || !completed.matches_pending(&receipt)
        || !same_member(&completion_identity, &completed_identity)
        || !same_directory(
            &context.state_identity,
            &open_private_directory(&paths.state_directory, uid)
                .map_err(|_| FinalizeError::Ambiguous)?
                .metadata()
                .map_err(|_| FinalizeError::Ambiguous)?,
        )
        || !same_directory(
            &config_identity,
            &open_private_directory(config, uid)
                .map_err(|_| FinalizeError::Ambiguous)?
                .metadata()
                .map_err(|_| FinalizeError::Ambiguous)?,
        )
        || !absent(&context.state, RECEIPT_MEMBER).map_err(|_| FinalizeError::Ambiguous)?
        || !fixed_artifacts_absent(&context.state, &config_dir)
            .map_err(|_| FinalizeError::Ambiguous)?
    {
        return Err(FinalizeError::Ambiguous);
    }
    Ok(FinalizeResult::ReceiptRetiredStillFenced)
}

fn retire_with_hook<G: FnMut() -> bool, H: FnMut(HookPoint) -> bool>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: G,
    mut hook: H,
) -> Result<CleanupResult, CleanupError> {
    let mut context = Context::new(config, paths, uid, generation, lock)?;
    for _ in 0..=8 {
        let observed = context.check(&mut gate)?;
        let step = observed.step;
        if step == Step::Done {
            return Ok(CleanupResult::RetiredStillFenced);
        }
        let name = step.name().ok_or(CleanupError::ManualRecovery)?;
        // check() performs its final filesystem pass after the last gate
        // callback, so that callback cannot make a stale unlink decision.
        let stage = if step.in_stage() {
            Some(
                open_stage(&context.state, uid)?
                    .ok_or(CleanupError::ManualRecovery)?
                    .0,
            )
        } else {
            None
        };
        if let Some(stage) = &stage
            && !same_directory(
                observed
                    .stage_parent
                    .as_ref()
                    .ok_or(CleanupError::ManualRecovery)?,
                &stage.metadata().map_err(|_| CleanupError::ManualRecovery)?,
            )
        {
            return Err(CleanupError::ManualRecovery);
        }
        let parent = stage.as_ref().unwrap_or(&context.state);
        let flags = OFlag::O_RDONLY
            | OFlag::O_NONBLOCK
            | OFlag::O_NOFOLLOW
            | OFlag::O_CLOEXEC
            | if step == Step::StageDirectory {
                OFlag::O_DIRECTORY
            } else {
                OFlag::empty()
            };
        let target = File::from(
            openat(parent, Path::new(name), flags, Mode::empty())
                .map_err(|_| CleanupError::ManualRecovery)?,
        );
        if !same_member(
            observed
                .target
                .as_ref()
                .ok_or(CleanupError::ManualRecovery)?,
            &target
                .metadata()
                .map_err(|_| CleanupError::ManualRecovery)?,
        ) {
            return Err(CleanupError::ManualRecovery);
        }
        let current = File::from(
            openat(parent, Path::new(name), flags, Mode::empty())
                .map_err(|_| CleanupError::ManualRecovery)?,
        );
        if !same_member(
            observed
                .target
                .as_ref()
                .ok_or(CleanupError::ManualRecovery)?,
            &current
                .metadata()
                .map_err(|_| CleanupError::ManualRecovery)?,
        ) {
            return Err(CleanupError::ManualRecovery);
        }
        unlinkat(
            parent,
            Path::new(name),
            if step == Step::StageDirectory {
                UnlinkatFlags::RemoveDir
            } else {
                UnlinkatFlags::NoRemoveDir
            },
        )
        .map_err(|_| CleanupError::Ambiguous)?;
        if !hook(HookPoint::Unlinked(step)) {
            return Err(CleanupError::Ambiguous);
        }
        parent.sync_all().map_err(|_| CleanupError::Ambiguous)?;
        if !hook(HookPoint::Synced(step)) {
            return Err(CleanupError::Ambiguous);
        }
        let after = context
            .check(&mut gate)
            .map_err(|_| CleanupError::Ambiguous)?;
        if after.step.index() != step.index() + 1 {
            return Err(CleanupError::Ambiguous);
        }
    }
    Err(CleanupError::Ambiguous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::DesiredState;
    use crate::restore_decision_candidate::TerminalChoice;
    use crate::restore_executor_candidate::{PendingOutcome, execute_staged_pair};
    use crate::restore_retirement_candidate::{
        RECEIPT_MEMBER, inspect_retirement_receipt, publish_retirement_receipt,
    };
    use crate::restore_staging_candidate::{read_staged_pair, stage_private_pair};
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Command;

    const OLD_STORE: &[u8] = b"synthetic old store\n";
    const OLD_TEMPLATE: &[u8] = b"synthetic old template\n";
    const NEW_STORE: &[u8] = b"synthetic new store\n";
    const NEW_TEMPLATE: &[u8] = b"synthetic new template\n";

    struct Fixture {
        root: PathBuf,
        config: PathBuf,
        paths: CutoverPaths,
        uid: u32,
    }

    impl Fixture {
        fn new() -> Self {
            Self::new_pair(NEW_STORE, NEW_TEMPLATE)
        }

        fn new_pair(new_store: &[u8], new_template: &[u8]) -> Self {
            let home = std::env::var_os("HOME").expect("test needs home");
            let root =
                crate::test_temp::directory_under(Path::new(&home), "restore-cleanup").unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let config = root.join("config");
            let runtime = root.join("runtime");
            let state = root.join("state");
            for directory in [&config, &runtime, &state] {
                fs::create_dir(directory).unwrap();
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            let paths = CutoverPaths::below(&runtime, &state, uid);
            fs::create_dir(&paths.state_directory).unwrap();
            fs::set_permissions(&paths.state_directory, fs::Permissions::from_mode(0o700)).unwrap();
            Self::member(
                &paths.ownership_marker,
                br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
            );
            Self::member(
                &paths.state_directory.join("desired.json"),
                &serde_json::to_vec(&DesiredState::default()).unwrap(),
            );
            Self::member(&config.join("profiles.json"), OLD_STORE);
            Self::member(&config.join("route-template.yaml"), OLD_TEMPLATE);
            stage_private_pair(
                &paths.state_directory,
                uid,
                OLD_STORE,
                OLD_TEMPLATE,
                new_store,
                new_template,
            )
            .unwrap();
            Self {
                root,
                config,
                paths,
                uid,
            }
        }

        fn member(path: &Path, bytes: &[u8]) {
            fs::write(path, bytes).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        fn reopen(root: PathBuf) -> Self {
            let uid = fs::metadata(&root).unwrap().uid();
            let paths = CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid);
            Self {
                config: root.join("config"),
                root,
                paths,
                uid,
            }
        }

        fn lock(&self) -> MigrationLock {
            MigrationLock::acquire(&self.paths, self.uid).unwrap()
        }

        fn committed(&self, lock: &MigrationLock) {
            assert_eq!(
                execute_staged_pair(
                    &self.config,
                    &self.paths,
                    self.uid,
                    2,
                    lock,
                    [45; 16],
                    || true,
                ),
                Ok(PendingOutcome::Committed)
            );
            assert_eq!(
                publish_retirement_receipt(&self.config, &self.paths, self.uid, 2, lock, || true,),
                Ok(PendingOutcome::Committed)
            );
        }

        fn completed(&self, lock: &MigrationLock) {
            assert_eq!(
                publish_completion_record(&self.config, &self.paths, self.uid, 2, lock, || true,),
                Ok(ClosurePublicationResult::PublishedStillFenced)
            );
        }

        fn assert_fenced_live(&self, lock: &MigrationLock) {
            assert_eq!(
                fs::read(self.config.join("profiles.json")).unwrap(),
                NEW_STORE
            );
            assert_eq!(
                fs::read(self.config.join("route-template.yaml")).unwrap(),
                NEW_TEMPLATE
            );
            assert!(self.paths.state_directory.join(RECEIPT_MEMBER).exists());
            assert!(crate::pending_private_transaction::pending_at(
                &self.paths.state_directory
            ));
            assert!(
                inspect_retirement_receipt(&self.config, &self.paths, self.uid, 2, lock).is_ok()
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn exact_cleanup_leaves_only_independent_receipt_and_is_idempotent() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        for _ in 0..2 {
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                ),
                Ok(CleanupResult::RetiredStillFenced)
            );
            fixture.assert_fenced_live(&lock);
        }
        assert!(
            !fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .exists()
        );
        assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        assert!(!fixture.paths.state_directory.join(INTENT).exists());
    }

    #[test]
    fn completion_publication_requires_retirement_and_leaves_both_fences() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        assert_eq!(
            publish_completion_record(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            ),
            Err(FinalizeError::ManualRecovery)
        );
        assert!(!fixture.paths.state_directory.join(CLOSURE_MEMBER).exists());
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        assert_eq!(
            publish_completion_record(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            ),
            Ok(ClosurePublicationResult::PublishedStillFenced)
        );
        let state = open_private_directory(&fixture.paths.state_directory, fixture.uid).unwrap();
        let (closure, _) = read_completion_member(&state, fixture.uid).unwrap();
        let pending =
            inspect_retirement_receipt(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                .unwrap();
        assert!(closure.matches_pending(&pending));
        assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
        assert_eq!(
            publish_completion_record(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            ),
            Err(FinalizeError::ManualRecovery)
        );
    }

    #[test]
    fn completion_publication_crashes_never_erase_pending_fence() {
        for phase in ["created", "written", "file", "directory", "reopened"] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            retire_fixed_restore_artifacts(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            )
            .unwrap();
            drop(lock);
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("restore_cleanup_candidate::tests::completion_publication_crash_worker")
                .env("OMAVLESS_COMPLETION_TEST_ROOT", &fixture.root)
                .env("OMAVLESS_COMPLETION_TEST_PHASE", phase)
                .status()
                .unwrap();
            assert_eq!(child.signal(), Some(9), "{phase}");
            let lock = fixture.lock();
            fixture.assert_fenced_live(&lock);
            assert!(crate::pending_private_transaction::pending_at(
                &fixture.paths.state_directory
            ));
            let completed =
                inspect_completion_record(&fixture.config, &fixture.paths, fixture.uid, 2, &lock);
            if matches!(phase, "directory" | "reopened") {
                assert!(completed.is_ok(), "{phase}");
            }
            if let Ok(record) = completed {
                let pending = inspect_retirement_receipt(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                )
                .unwrap();
                assert!(record.matches_pending(&pending));
            }
        }
    }

    #[test]
    fn completion_change_after_reopen_cannot_report_durable_publication() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        let completion = fixture.paths.state_directory.join(CLOSURE_MEMBER);
        assert_eq!(
            publish_completion_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
                |point| {
                    if point == ClosurePublishCheckpoint::Reopened {
                        fs::write(&completion, b"torn synthetic completion").unwrap();
                    }
                    true
                },
            ),
            Err(FinalizeError::Ambiguous)
        );
        fixture.assert_fenced_live(&lock);
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }

    #[test]
    fn completion_publication_crash_worker() {
        let Some(root) = std::env::var_os("OMAVLESS_COMPLETION_TEST_ROOT") else {
            return;
        };
        let fixture = Fixture::reopen(PathBuf::from(root));
        let lock = fixture.lock();
        let phase = std::env::var("OMAVLESS_COMPLETION_TEST_PHASE").unwrap();
        let _ = publish_completion_with_hook(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
            |point| {
                let selected = match point {
                    ClosurePublishCheckpoint::Created => phase == "created",
                    ClosurePublishCheckpoint::Written => phase == "written",
                    ClosurePublishCheckpoint::FileSynced => phase == "file",
                    ClosurePublishCheckpoint::DirectorySynced => phase == "directory",
                    ClosurePublishCheckpoint::Reopened => phase == "reopened",
                };
                if selected {
                    nix::sys::signal::kill(
                        nix::unistd::Pid::this(),
                        nix::sys::signal::Signal::SIGKILL,
                    )
                    .unwrap();
                }
                true
            },
        );
        panic!("expected abrupt completion publication worker termination");
    }

    #[test]
    fn final_closure_requires_complete_cleanup_and_preserves_live_pair() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        let receipt_path = fixture.paths.state_directory.join(RECEIPT_MEMBER);
        let receipt_before = fs::read(&receipt_path).unwrap();
        assert_eq!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Err(FinalizeError::ManualRecovery)
        );
        assert_eq!(fs::read(&receipt_path).unwrap(), receipt_before);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        fixture.assert_fenced_live(&lock);
        fixture.completed(&lock);
        assert_eq!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || false
            ),
            Err(FinalizeError::Admission)
        );
        assert_eq!(fs::read(&receipt_path).unwrap(), receipt_before);
        assert_eq!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Ok(FinalizeResult::ReceiptRetiredStillFenced)
        );
        assert!(!receipt_path.exists());
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
        assert_eq!(
            fs::read(fixture.config.join("profiles.json")).unwrap(),
            NEW_STORE
        );
        assert_eq!(
            fs::read(fixture.config.join("route-template.yaml")).unwrap(),
            NEW_TEMPLATE
        );
        assert_eq!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Err(FinalizeError::ManualRecovery)
        );
    }

    #[test]
    fn final_closure_refuses_surviving_slot_and_changed_live_pair() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        fixture.completed(&lock);
        let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
        let original = fs::read(&receipt).unwrap();
        let slot = fixture.config.join(NEW_SLOT[0]);
        Fixture::member(&slot, NEW_STORE);
        assert!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            )
            .is_err()
        );
        assert_eq!(fs::read(&receipt).unwrap(), original);
        fs::remove_file(slot).unwrap();
        symlink("missing-synthetic-slot", fixture.config.join(NEW_SLOT[0])).unwrap();
        assert!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            )
            .is_err()
        );
        assert_eq!(fs::read(&receipt).unwrap(), original);
        fs::remove_file(fixture.config.join(NEW_SLOT[0])).unwrap();
        Fixture::member(
            &fixture.config.join("profiles.json"),
            b"different synthetic store\n",
        );
        assert!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            )
            .is_err()
        );
        assert_eq!(fs::read(&receipt).unwrap(), original);
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }

    #[test]
    fn final_closure_refuses_absent_torn_unsafe_or_foreign_completion() {
        for case in ["absent", "torn", "symlink", "hardlink", "foreign"] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            retire_fixed_restore_artifacts(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            )
            .unwrap();
            let completion = fixture.paths.state_directory.join(CLOSURE_MEMBER);
            if case != "absent" {
                fixture.completed(&lock);
            }
            match case {
                "torn" => fs::write(&completion, b"torn synthetic completion").unwrap(),
                "symlink" => {
                    fs::remove_file(&completion).unwrap();
                    symlink("missing-synthetic-completion", &completion).unwrap();
                }
                "hardlink" => {
                    fs::hard_link(&completion, fixture.root.join("synthetic-alias")).unwrap();
                }
                "foreign" => {
                    let other = Fixture::new_pair(b"different synthetic store", NEW_TEMPLATE);
                    let other_lock = other.lock();
                    other.committed(&other_lock);
                    retire_fixed_restore_artifacts(
                        &other.config,
                        &other.paths,
                        other.uid,
                        2,
                        &other_lock,
                        || true,
                    )
                    .unwrap();
                    other.completed(&other_lock);
                    fs::write(
                        &completion,
                        fs::read(other.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                    )
                    .unwrap();
                }
                _ => {}
            }
            let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
            let before = fs::read(&receipt).unwrap();
            assert!(
                finalize_fenced_restore(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                )
                .is_err(),
                "{case}"
            );
            assert_eq!(fs::read(&receipt).unwrap(), before, "{case}");
            assert!(crate::pending_private_transaction::pending_at(
                &fixture.paths.state_directory
            ));
        }
    }

    #[test]
    fn final_closure_rechecks_the_owner_gate_before_unlink() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        fixture.completed(&lock);
        let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
        let original = fs::read(&receipt).unwrap();
        let mut checks = 0;
        let result = finalize_fenced_restore(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || {
                checks += 1;
                checks < 3
            },
        );
        assert_eq!(result, Err(FinalizeError::Admission));
        assert!(checks >= 3);
        assert_eq!(fs::read(&receipt).unwrap(), original);
        fixture.assert_fenced_live(&lock);
    }

    #[test]
    fn partial_completion_write_keeps_pending_and_refuses_final_unlink() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        let completion = fixture.paths.state_directory.join(CLOSURE_MEMBER);
        let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
        let pending = fs::read(&receipt).unwrap();
        assert_eq!(
            publish_completion_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
                |point| {
                    if point == ClosurePublishCheckpoint::Created {
                        fs::write(&completion, vec![0x41; CLOSURE_BYTES / 2]).unwrap();
                        return false;
                    }
                    true
                },
            ),
            Err(FinalizeError::Ambiguous)
        );
        assert_eq!(fs::read(&receipt).unwrap(), pending);
        assert!(
            inspect_completion_record(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                .is_err()
        );
        assert!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            )
            .is_err()
        );
        fixture.assert_fenced_live(&lock);
    }

    #[test]
    fn last_gate_desired_replacement_keeps_final_fence() {
        let baseline = Fixture::new();
        let lock = baseline.lock();
        baseline.committed(&lock);
        retire_fixed_restore_artifacts(
            &baseline.config,
            &baseline.paths,
            baseline.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        baseline.completed(&lock);
        let mut total = 0;
        assert_eq!(
            finalize_fenced_restore(
                &baseline.config,
                &baseline.paths,
                baseline.uid,
                2,
                &lock,
                || {
                    total += 1;
                    true
                }
            ),
            Ok(FinalizeResult::ReceiptRetiredStillFenced)
        );
        assert!(total >= 4);

        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        fixture.completed(&lock);
        let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
        let original = fs::read(&receipt).unwrap();
        let desired = fixture.paths.state_directory.join("desired.json");
        let mut count = 0;
        assert!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || {
                    count += 1;
                    if count == total {
                        fs::write(&desired, b"different synthetic desired\n").unwrap();
                    }
                    true
                }
            )
            .is_err()
        );
        assert_eq!(count, total);
        assert_eq!(fs::read(&receipt).unwrap(), original);
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }

    #[test]
    fn last_gate_completion_replacement_keeps_pending_receipt() {
        let baseline = Fixture::new();
        let lock = baseline.lock();
        baseline.committed(&lock);
        retire_fixed_restore_artifacts(
            &baseline.config,
            &baseline.paths,
            baseline.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        baseline.completed(&lock);
        let mut total = 0;
        assert_eq!(
            finalize_fenced_restore(
                &baseline.config,
                &baseline.paths,
                baseline.uid,
                2,
                &lock,
                || {
                    total += 1;
                    true
                }
            ),
            Ok(FinalizeResult::ReceiptRetiredStillFenced)
        );

        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        retire_fixed_restore_artifacts(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
        )
        .unwrap();
        fixture.completed(&lock);
        let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
        let original = fs::read(&receipt).unwrap();
        let completion = fixture.paths.state_directory.join(CLOSURE_MEMBER);
        let mut count = 0;
        assert!(
            finalize_fenced_restore(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || {
                    count += 1;
                    if count == total {
                        fs::write(&completion, b"torn synthetic completion").unwrap();
                    }
                    true
                }
            )
            .is_err()
        );
        assert_eq!(count, total);
        assert_eq!(fs::read(&receipt).unwrap(), original);
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }

    #[test]
    fn abrupt_worker_loss_during_last_fence_closure_never_leaves_incomplete_cleanup_unfenced() {
        for phase in ["unlinked", "synced"] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            retire_fixed_restore_artifacts(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            )
            .unwrap();
            fixture.completed(&lock);
            let expected_receipt = RetirementReceipt::decode(
                &fs::read(fixture.paths.state_directory.join(RECEIPT_MEMBER)).unwrap(),
            )
            .unwrap();
            drop(lock);
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("restore_cleanup_candidate::tests::finalize_crash_worker")
                .env("OMAVLESS_FINALIZE_TEST_ROOT", &fixture.root)
                .env("OMAVLESS_FINALIZE_TEST_PHASE", phase)
                .status()
                .unwrap();
            assert_eq!(child.signal(), Some(9), "{phase}");
            let lock = fixture.lock();
            for name in [PENDING_DIRECTORY, TERMINAL, INTENT] {
                assert!(!fixture.paths.state_directory.join(name).exists());
            }
            assert!(
                NEW_SLOT
                    .into_iter()
                    .chain(OLD_SLOT)
                    .all(|name| !fixture.config.join(name).exists())
            );
            assert_eq!(
                fs::read(fixture.config.join("profiles.json")).unwrap(),
                NEW_STORE
            );
            assert_eq!(
                fs::read(fixture.config.join("route-template.yaml")).unwrap(),
                NEW_TEMPLATE
            );
            let receipt = fixture.paths.state_directory.join(RECEIPT_MEMBER);
            let completed =
                inspect_completion_record(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                    .unwrap();
            assert!(completed.matches_pending(&expected_receipt), "{phase}");
            if receipt.exists() {
                fixture.assert_fenced_live(&lock);
            } else {
                assert!(crate::pending_private_transaction::pending_at(
                    &fixture.paths.state_directory
                ));
            }
        }
    }

    #[test]
    fn finalize_crash_worker() {
        let Some(root) = std::env::var_os("OMAVLESS_FINALIZE_TEST_ROOT") else {
            return;
        };
        let fixture = Fixture::reopen(PathBuf::from(root));
        let lock = fixture.lock();
        let phase = std::env::var("OMAVLESS_FINALIZE_TEST_PHASE").unwrap();
        let _ = finalize_with_hook(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
            |point| {
                let selected = matches!(point, FinalizeCheckpoint::Unlinked) && phase == "unlinked"
                    || matches!(point, FinalizeCheckpoint::Synced) && phase == "synced";
                if selected {
                    nix::sys::signal::kill(
                        nix::unistd::Pid::this(),
                        nix::sys::signal::Signal::SIGKILL,
                    )
                    .unwrap();
                }
                true
            },
        );
        panic!("expected abrupt finalization worker termination");
    }

    #[test]
    fn aborted_terminal_and_identical_pair_can_retire_without_touching_live() {
        let aborted = Fixture::new();
        let lock = aborted.lock();
        let stage = read_staged_pair(&aborted.paths.state_directory, aborted.uid).unwrap();
        let desired = fs::read(aborted.paths.state_directory.join("desired.json")).unwrap();
        let intent = DecisionRecord::intent(2, Some(&desired), stage.identity(), [46; 16]).unwrap();
        Fixture::member(
            &aborted.paths.state_directory.join(INTENT),
            &intent.encode(),
        );
        Fixture::member(
            &aborted.paths.state_directory.join(TERMINAL),
            &intent.terminal(TerminalChoice::Abort).unwrap().encode(),
        );
        assert_eq!(
            publish_retirement_receipt(
                &aborted.config,
                &aborted.paths,
                aborted.uid,
                2,
                &lock,
                || true,
            ),
            Ok(PendingOutcome::Aborted)
        );
        assert_eq!(
            retire_fixed_restore_artifacts(
                &aborted.config,
                &aborted.paths,
                aborted.uid,
                2,
                &lock,
                || true,
            ),
            Ok(CleanupResult::RetiredStillFenced)
        );
        assert_eq!(
            fs::read(aborted.config.join("profiles.json")).unwrap(),
            OLD_STORE
        );
        assert_eq!(
            fs::read(aborted.config.join("route-template.yaml")).unwrap(),
            OLD_TEMPLATE
        );
        assert!(
            inspect_retirement_receipt(&aborted.config, &aborted.paths, aborted.uid, 2, &lock)
                .is_ok()
        );

        let identical = Fixture::new_pair(OLD_STORE, OLD_TEMPLATE);
        let lock = identical.lock();
        identical.committed(&lock);
        assert_eq!(
            retire_fixed_restore_artifacts(
                &identical.config,
                &identical.paths,
                identical.uid,
                2,
                &lock,
                || true,
            ),
            Ok(CleanupResult::RetiredStillFenced)
        );
        assert_eq!(
            fs::read(identical.config.join("profiles.json")).unwrap(),
            OLD_STORE
        );
        assert!(
            inspect_retirement_receipt(
                &identical.config,
                &identical.paths,
                identical.uid,
                2,
                &lock,
            )
            .is_ok()
        );
    }

    #[test]
    fn every_post_unlink_and_post_sync_interruption_resumes_from_one_prefix() {
        for index in 0..8 {
            for after_sync in [false, true] {
                let fixture = Fixture::new();
                let lock = fixture.lock();
                fixture.committed(&lock);
                let stopped = retire_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                    |point| {
                        !matches!(point, HookPoint::Unlinked(step) if !after_sync && step.index() == index)
                            && !matches!(point, HookPoint::Synced(step) if after_sync && step.index() == index)
                    },
                );
                assert_eq!(
                    stopped,
                    Err(CleanupError::Ambiguous),
                    "{index} {after_sync}"
                );
                fixture.assert_fenced_live(&lock);
                drop(lock);
                let lock = fixture.lock();
                assert_eq!(
                    retire_fixed_restore_artifacts(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(CleanupResult::RetiredStillFenced),
                    "{index} {after_sync}"
                );
                fixture.assert_fenced_live(&lock);
            }
        }
    }

    #[test]
    fn abrupt_process_loss_after_every_unlink_and_sync_reopens_to_fenced_retirement() {
        for index in 0..8 {
            for phase in ["unlinked", "synced"] {
                let fixture = Fixture::new();
                let lock = fixture.lock();
                fixture.committed(&lock);
                drop(lock);
                let child = Command::new(std::env::current_exe().unwrap())
                    .arg("--exact")
                    .arg("restore_cleanup_candidate::tests::crash_worker")
                    .env("OMAVLESS_CLEANUP_TEST_ROOT", &fixture.root)
                    .env("OMAVLESS_CLEANUP_TEST_INDEX", index.to_string())
                    .env("OMAVLESS_CLEANUP_TEST_PHASE", phase)
                    .status()
                    .unwrap();
                assert_eq!(child.signal(), Some(9), "{index} {phase}");
                let lock = fixture.lock();
                fixture.assert_fenced_live(&lock);
                assert_eq!(
                    retire_fixed_restore_artifacts(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(CleanupResult::RetiredStillFenced),
                    "{index} {phase}"
                );
                fixture.assert_fenced_live(&lock);
            }
        }
    }

    #[test]
    fn crash_worker() {
        let Some(root) = std::env::var_os("OMAVLESS_CLEANUP_TEST_ROOT") else {
            return;
        };
        let fixture = Fixture::reopen(PathBuf::from(root));
        let lock = fixture.lock();
        let index: usize = std::env::var("OMAVLESS_CLEANUP_TEST_INDEX")
            .unwrap()
            .parse()
            .unwrap();
        let phase = std::env::var("OMAVLESS_CLEANUP_TEST_PHASE").unwrap();
        let _ = retire_with_hook(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
            |point| {
                let selected = match point {
                    HookPoint::Unlinked(step) => phase == "unlinked" && step.index() == index,
                    HookPoint::Synced(step) => phase == "synced" && step.index() == index,
                };
                if selected {
                    nix::sys::signal::kill(
                        nix::unistd::Pid::this(),
                        nix::sys::signal::Signal::SIGKILL,
                    )
                    .unwrap();
                }
                true
            },
        );
        panic!("expected abrupt cleanup worker termination");
    }

    #[test]
    fn holes_unexpected_entries_and_bad_journal_refuse_without_further_deletion() {
        for change in 0..4 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let stage = fixture.paths.state_directory.join(PENDING_DIRECTORY);
            match change {
                0 => fs::remove_file(stage.join(MEMBERS[2])).unwrap(),
                1 => fs::remove_file(stage.join(READY_MEMBER)).unwrap(),
                2 => Fixture::member(&stage.join("unexpected"), b"other"),
                _ => Fixture::member(
                    &fixture.paths.state_directory.join(TERMINAL),
                    b"bad journal",
                ),
            }
            let old_first = fs::metadata(stage.join(MEMBERS[0])).unwrap().ino();
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert_eq!(
                fs::metadata(stage.join(MEMBERS[0])).unwrap().ino(),
                old_first
            );
            assert!(crate::pending_private_transaction::pending_at(
                &fixture.paths.state_directory
            ));
        }
    }

    #[test]
    fn receipt_owner_host_and_unsafe_target_refuse_before_unlink() {
        for change in 0..7 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            let inode = fs::metadata(&first).unwrap().ino();
            match change {
                0 => Fixture::member(&fixture.paths.state_directory.join(RECEIPT_MEMBER), b"torn"),
                1 => Fixture::member(
                    &fixture.paths.ownership_marker,
                    br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                ),
                2 => {
                    assert_eq!(
                        retire_fixed_restore_artifacts(
                            &fixture.config,
                            &fixture.paths,
                            fixture.uid,
                            2,
                            &lock,
                            || false,
                        ),
                        Err(CleanupError::Admission)
                    );
                    assert_eq!(fs::metadata(&first).unwrap().ino(), inode);
                    continue;
                }
                _ => match change {
                    3 => {
                        fs::remove_file(&first).unwrap();
                        symlink("new-profiles.json", &first).unwrap();
                    }
                    4 => fs::hard_link(&first, fixture.root.join("linked-copy")).unwrap(),
                    5 => fs::set_permissions(&first, fs::Permissions::from_mode(0o644)).unwrap(),
                    _ => Fixture::member(&first, b"changed staged bytes\n"),
                },
            }
            assert!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                )
                .is_err()
            );
            if change != 3 {
                assert_eq!(fs::metadata(&first).unwrap().ino(), inode);
            }
        }
    }

    #[test]
    fn final_gate_target_replacement_cannot_delete_foreign_or_replaced_member() {
        for same_bytes in [false, true] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            // Keep the unlinked inode alive. Otherwise the filesystem may
            // legitimately reuse its number for the replacement immediately.
            let held_original = File::open(&first).unwrap();
            let old_inode = held_original.metadata().unwrap().ino();
            let mut calls = 0;
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if calls == 2 {
                            fs::remove_file(&first).unwrap();
                            Fixture::member(
                                &first,
                                if same_bytes {
                                    OLD_STORE
                                } else {
                                    b"foreign synthetic bytes\n"
                                },
                            );
                        }
                        true
                    },
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert!(first.exists());
            assert_eq!(held_original.metadata().unwrap().ino(), old_inode);
            assert_ne!(fs::metadata(&first).unwrap().ino(), old_inode);
            assert_eq!(
                fs::read(&first).unwrap(),
                if same_bytes {
                    OLD_STORE
                } else {
                    b"foreign synthetic bytes\n"
                }
            );
            assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        }
    }

    #[test]
    fn final_gate_live_or_desired_drift_refuses_before_first_unlink() {
        for desired_drift in [false, true] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            let before = fs::metadata(&first).unwrap().ino();
            let mut calls = 0;
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if calls == 2 {
                            if desired_drift {
                                Fixture::member(
                                    &fixture.paths.state_directory.join("desired.json"),
                                    &serde_json::to_vec(&DesiredState {
                                        generation: 1,
                                        ..DesiredState::default()
                                    })
                                    .unwrap(),
                                );
                            } else {
                                Fixture::member(
                                    &fixture.config.join("profiles.json"),
                                    b"foreign live bytes\n",
                                );
                            }
                        }
                        true
                    },
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert_eq!(fs::metadata(&first).unwrap().ino(), before);
            assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        }
    }

    #[test]
    fn surviving_or_new_replacement_slot_preserves_complete_stage() {
        for introduced_at_gate in [false, true] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            let before = fs::metadata(&first).unwrap().ino();
            let slot = fixture.config.join(NEW_SLOT[0]);
            if !introduced_at_gate {
                Fixture::member(&slot, NEW_STORE);
            }
            let mut calls = 0;
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if introduced_at_gate && calls == 2 {
                            Fixture::member(&slot, NEW_STORE);
                        }
                        true
                    },
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert_eq!(fs::metadata(&first).unwrap().ino(), before);
            assert!(slot.exists());
            assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        }
    }

    #[test]
    fn final_gate_receipt_loss_cannot_remove_last_journal_fence() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        assert_eq!(
            retire_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
                |point| point != HookPoint::Synced(Step::Terminal),
            ),
            Err(CleanupError::Ambiguous)
        );
        assert!(fixture.paths.state_directory.join(INTENT).exists());
        assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        let mut calls = 0;
        assert_eq!(
            retire_fixed_restore_artifacts(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || {
                    calls += 1;
                    if calls == 2 {
                        fs::remove_file(fixture.paths.state_directory.join(RECEIPT_MEMBER))
                            .unwrap();
                    }
                    true
                },
            ),
            Err(CleanupError::ManualRecovery)
        );
        assert!(fixture.paths.state_directory.join(INTENT).exists());
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }
}
