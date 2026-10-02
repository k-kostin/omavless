// SPDX-License-Identifier: MIT

//! Inactive two-file restore transaction experiment. Only synthetic tests call
//! this module. It does not register a runtime, startup, CLI or IPC operation.
//! Its caller must supply a fresh disconnected/idle host gate under the same
//! migration lease; a future product adapter needs separate review.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::restore_decision_candidate::{
    DecisionPhase, DecisionRecord, RECORD_BYTES, RecoveryReview, TerminalChoice,
};
use crate::restore_journal_candidate::{inspect_decision_journal, read_desired_for_decision};
use crate::restore_staging_candidate::{
    LivePairClass, StageIdentity, VerifiedStage, classify_live_pair_bound, inspect_stage_identity,
    read_member, read_staged_pair, same_directory, same_member,
};
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, openat, renameat};
use nix::sys::stat::Mode;
use nix::unistd::linkat;
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use zeroize::Zeroizing;

#[path = "restore_successor_executor_candidate.rs"]
pub(crate) mod successor;

const INTENT: &str = "restore-decision.intent";
const TERMINAL: &str = "restore-decision.terminal";
const LIVE: [&str; 2] = ["profiles.json", "route-template.yaml"];
pub(crate) const NEW_SLOT: [&str; 2] = [".restore-profiles.new", ".restore-template.new"];
pub(crate) const OLD_SLOT: [&str; 2] = [".restore-profiles.old", ".restore-template.old"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionError {
    Admission,
    ManualRecovery,
    /// An effect or its synchronization may have happened. Preserve all
    /// staged and journal bytes; never blindly repeat this call.
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingOutcome {
    Committed,
    Aborted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectStep {
    Intent,
    Linked(usize),
    RenameApplied(usize),
    Renamed(usize),
    Terminal,
}

fn same_parent(left: &Metadata, right: &Metadata) -> bool {
    same_directory(left, right)
}

fn live_bytes(
    directory: &File,
    index: usize,
    uid: u32,
) -> Result<Zeroizing<Vec<u8>>, ExecutionError> {
    let limit = if index == 0 {
        MAX_PRIVATE_STORE_BYTES
    } else {
        MAX_TEMPLATE_BYTES
    };
    read_member(directory, LIVE[index], uid, limit).map_err(|_| ExecutionError::ManualRecovery)
}

fn stage_bytes(stage: &VerifiedStage, old: bool, index: usize) -> &[u8] {
    match (old, index) {
        (true, 0) => stage.old_store(),
        (true, _) => stage.old_template(),
        (false, 0) => stage.new_store(),
        (false, _) => stage.new_template(),
    }
}

fn write_record(
    paths: &CutoverPaths,
    uid: u32,
    name: &str,
    bytes: &[u8],
) -> Result<(), ExecutionError> {
    let directory = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let before = directory
        .metadata()
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let mut file = File::from(
        openat(
            &directory,
            Path::new(name),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| ExecutionError::Ambiguous)?,
    );
    let metadata = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(ExecutionError::Ambiguous);
    }
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| directory.sync_all())
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !same_parent(
        &before,
        &open_private_directory(&paths.state_directory, uid)
            .map_err(|_| ExecutionError::Ambiguous)?
            .metadata()
            .map_err(|_| ExecutionError::Ambiguous)?,
    ) {
        return Err(ExecutionError::Ambiguous);
    }
    Ok(())
}

struct Bound<'a, G: FnMut() -> bool> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    desired: Option<Zeroizing<Vec<u8>>>,
    stage: StageIdentity,
    intent: &'a DecisionRecord,
    config_before: Metadata,
    gate: G,
}

impl<G: FnMut() -> bool> Bound<'_, G> {
    fn check_common(&mut self) -> Result<(), ExecutionError> {
        if !(self.gate)()
            || !self.lock.authorizes(self.paths, self.uid)
            || crate::restore_disposition_ticket_model::pending_at(&self.paths.state_directory)
            || crate::restore_disposition_complete_model::pending_at(&self.paths.state_directory)
        {
            return Err(ExecutionError::Admission);
        }
        let marker =
            read_marker_existing(self.paths, self.uid).map_err(|_| ExecutionError::Admission)?;
        if marker.phase() != OwnershipPhase::Rust || marker.generation() != self.generation {
            return Err(ExecutionError::Admission);
        }
        let desired = read_desired_for_decision(self.paths, self.uid, self.lock)
            .map_err(|_| ExecutionError::ManualRecovery)?;
        if desired.as_ref().map(|value| value.as_slice())
            != self.desired.as_ref().map(|value| value.as_slice())
            || inspect_stage_identity(&self.paths.state_directory, self.uid)
                .map_err(|_| ExecutionError::ManualRecovery)?
                .digest()
                != self.stage.digest()
        {
            return Err(ExecutionError::ManualRecovery);
        }
        if !same_parent(
            &self.config_before,
            &open_private_directory(self.config, self.uid)
                .map_err(|_| ExecutionError::ManualRecovery)?
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) {
            return Err(ExecutionError::ManualRecovery);
        }
        Ok(())
    }

    fn check(&mut self, phase: DecisionPhase) -> Result<(), ExecutionError> {
        self.check_common()?;
        let chain = inspect_decision_journal(self.paths, self.uid, self.lock)
            .map_err(|_| ExecutionError::ManualRecovery)?;
        if chain.active().phase() != phase || !chain.intent().same_transaction(self.intent) {
            return Err(ExecutionError::ManualRecovery);
        }
        Ok(())
    }

    fn classify(&self) -> Result<LivePairClass, ExecutionError> {
        let pair = classify_live_pair_bound(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
        )
        .map_err(|_| ExecutionError::ManualRecovery)?;
        if pair.stage().digest() != self.stage.digest() {
            return Err(ExecutionError::ManualRecovery);
        }
        Ok(pair.class())
    }
}

fn replace_member<G: FnMut() -> bool, H: FnMut(EffectStep) -> bool>(
    bound: &mut Bound<'_, G>,
    index: usize,
    target: &[u8],
    other: &[u8],
    slot: &str,
    hook: &mut H,
) -> Result<(), ExecutionError> {
    bound.check(DecisionPhase::Intent)?;
    let directory = open_private_directory(bound.config, bound.uid)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let before = directory
        .metadata()
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let current = live_bytes(&directory, index, bound.uid)?;
    if current.as_slice() == target {
        return Ok(());
    }
    if current.as_slice() != other {
        return Err(ExecutionError::ManualRecovery);
    }
    let mut temporary = File::from(
        openat(
            &directory,
            Path::new("."),
            OFlag::O_TMPFILE | OFlag::O_RDWR | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| ExecutionError::ManualRecovery)?,
    );
    let tmp_before = temporary
        .metadata()
        .map_err(|_| ExecutionError::ManualRecovery)?;
    if !tmp_before.is_file()
        || tmp_before.uid() != bound.uid
        || tmp_before.mode() & 0o7777 != 0o600
        || tmp_before.nlink() != 0
    {
        return Err(ExecutionError::ManualRecovery);
    }
    temporary
        .write_all(target)
        .and_then(|_| temporary.sync_all())
        .map_err(|_| ExecutionError::Ambiguous)?;
    if temporary
        .metadata()
        .map_err(|_| ExecutionError::Ambiguous)?
        .len()
        != target.len() as u64
    {
        return Err(ExecutionError::Ambiguous);
    }
    bound.check(DecisionPhase::Intent)?;
    match linkat(
        &temporary,
        Path::new(""),
        &directory,
        Path::new(slot),
        AtFlags::AT_EMPTY_PATH,
    ) {
        Ok(()) => {}
        Err(Errno::EEXIST) => {
            if read_member(
                &directory,
                slot,
                bound.uid,
                if index == 0 {
                    MAX_PRIVATE_STORE_BYTES
                } else {
                    MAX_TEMPLATE_BYTES
                },
            )
            .map_err(|_| ExecutionError::ManualRecovery)?
            .as_slice()
                != target
            {
                return Err(ExecutionError::ManualRecovery);
            }
        }
        Err(_) => return Err(ExecutionError::Ambiguous),
    }
    directory
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    let selected_slot = File::from(
        openat(
            &directory,
            Path::new(slot),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ExecutionError::ManualRecovery)?,
    );
    let slot_before = selected_slot
        .metadata()
        .map_err(|_| ExecutionError::ManualRecovery)?;
    if !hook(EffectStep::Linked(index)) {
        return Err(ExecutionError::Ambiguous);
    }
    bound.check(DecisionPhase::Intent)?;
    if !same_parent(
        &before,
        &open_private_directory(bound.config, bound.uid)
            .map_err(|_| ExecutionError::ManualRecovery)?
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?,
    ) || live_bytes(&directory, index, bound.uid)? != current
        || !same_member(
            &slot_before,
            &selected_slot
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        )
        || !same_member(
            &slot_before,
            &File::from(
                openat(
                    &directory,
                    Path::new(slot),
                    OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| ExecutionError::ManualRecovery)?,
            )
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?,
        )
        || read_member(
            &directory,
            slot,
            bound.uid,
            if index == 0 {
                MAX_PRIVATE_STORE_BYTES
            } else {
                MAX_TEMPLATE_BYTES
            },
        )
        .map_err(|_| ExecutionError::ManualRecovery)?
        .as_slice()
            != target
    {
        return Err(ExecutionError::ManualRecovery);
    }
    renameat(&directory, slot, &directory, LIVE[index]).map_err(|_| ExecutionError::Ambiguous)?;
    if !hook(EffectStep::RenameApplied(index)) {
        return Err(ExecutionError::Ambiguous);
    }
    bound.check(DecisionPhase::Intent)?;
    directory
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !hook(EffectStep::Renamed(index)) {
        return Err(ExecutionError::Ambiguous);
    }
    bound.check(DecisionPhase::Intent)?;
    if live_bytes(&directory, index, bound.uid)?.as_slice() != target
        || !same_parent(
            &before,
            &open_private_directory(bound.config, bound.uid)
                .map_err(|_| ExecutionError::Ambiguous)?
                .metadata()
                .map_err(|_| ExecutionError::Ambiguous)?,
        )
    {
        return Err(ExecutionError::Ambiguous);
    }
    Ok(())
}

/// A skipped replacement may follow an interrupted directory sync from a
/// previous process. Synchronize both exact live inodes and the directory
/// again before any terminal decision, then independently classify the pair.
fn sync_and_verify_pair<G: FnMut() -> bool>(
    bound: &mut Bound<'_, G>,
    stage: &VerifiedStage,
    old: bool,
    phase: DecisionPhase,
) -> Result<(), ExecutionError> {
    bound.check(phase)?;
    let directory = open_private_directory(bound.config, bound.uid)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let before = directory
        .metadata()
        .map_err(|_| ExecutionError::ManualRecovery)?;
    for (index, name) in LIVE.iter().enumerate() {
        if live_bytes(&directory, index, bound.uid)?.as_slice() != stage_bytes(stage, old, index) {
            return Err(ExecutionError::ManualRecovery);
        }
        let file = File::from(
            openat(
                &directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ExecutionError::ManualRecovery)?,
        );
        let inode_before = file
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?;
        file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
        let reopened = File::from(
            openat(
                &directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ExecutionError::ManualRecovery)?,
        );
        if !same_member(
            &inode_before,
            &file
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) || !same_member(
            &inode_before,
            &reopened
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) || live_bytes(&directory, index, bound.uid)?.as_slice()
            != stage_bytes(stage, old, index)
        {
            return Err(ExecutionError::ManualRecovery);
        }
    }
    directory
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !same_parent(
        &before,
        &open_private_directory(bound.config, bound.uid)
            .map_err(|_| ExecutionError::ManualRecovery)?
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?,
    ) {
        return Err(ExecutionError::ManualRecovery);
    }
    bound.check(phase)?;
    let class = bound.classify()?;
    if (old && !matches!(class, LivePairClass::Old | LivePairClass::Identical))
        || (!old && !matches!(class, LivePairClass::New | LivePairClass::Identical))
    {
        return Err(ExecutionError::ManualRecovery);
    }
    Ok(())
}

fn sync_decision_journal<G: FnMut() -> bool>(
    bound: &mut Bound<'_, G>,
    phase: DecisionPhase,
) -> Result<(), ExecutionError> {
    sync_decision_journal_with_hook(bound, phase, |_| {})
}

fn sync_decision_journal_with_hook<G: FnMut() -> bool, H: FnMut(&str)>(
    bound: &mut Bound<'_, G>,
    phase: DecisionPhase,
    mut hook: H,
) -> Result<(), ExecutionError> {
    bound.check(phase)?;
    let directory = open_private_directory(&bound.paths.state_directory, bound.uid)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let directory_before = directory
        .metadata()
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let mut synced_members = Vec::with_capacity(2);
    for member in [INTENT, TERMINAL] {
        let file = File::from(
            openat(
                &directory,
                Path::new(member),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ExecutionError::ManualRecovery)?,
        );
        let metadata = file
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?;
        if !metadata.is_file()
            || metadata.uid() != bound.uid
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
            || metadata.len() != RECORD_BYTES as u64
        {
            return Err(ExecutionError::ManualRecovery);
        }
        file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
        hook(member);
        let reopened = File::from(
            openat(
                &directory,
                Path::new(member),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ExecutionError::ManualRecovery)?,
        );
        if !same_member(
            &metadata,
            &file
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) || !same_member(
            &metadata,
            &reopened
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) {
            return Err(ExecutionError::ManualRecovery);
        }
        synced_members.push((member, file, metadata));
    }
    directory
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    for (member, synced_file, before) in synced_members {
        let current = File::from(
            openat(
                &directory,
                Path::new(member),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ExecutionError::ManualRecovery)?,
        );
        if !same_member(
            &before,
            &synced_file
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) || !same_member(
            &before,
            &current
                .metadata()
                .map_err(|_| ExecutionError::ManualRecovery)?,
        ) {
            return Err(ExecutionError::ManualRecovery);
        }
    }
    if !same_parent(
        &directory_before,
        &open_private_directory(&bound.paths.state_directory, bound.uid)
            .map_err(|_| ExecutionError::ManualRecovery)?
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?,
    ) {
        return Err(ExecutionError::ManualRecovery);
    }
    bound.check(phase)
}

/// Execute only an already complete synthetic stage. Even after durable
/// commit the pending stage/journal fence is retained for a separate, reviewed
/// finalization policy. There is intentionally no production caller.
#[allow(dead_code)]
pub(crate) fn execute_staged_pair(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    transaction_id: [u8; 16],
    gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    execute_with_hook(
        config,
        paths,
        uid,
        generation,
        lock,
        transaction_id,
        gate,
        |_| true,
    )
}

// The final parameter is a test-only crash hook; the public candidate keeps
// the fixed owner/lease identity explicit at its security boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_with_hook<G: FnMut() -> bool, H: FnMut(EffectStep) -> bool>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    transaction_id: [u8; 16],
    gate: G,
    mut hook: H,
) -> Result<PendingOutcome, ExecutionError> {
    if !lock.authorizes(paths, uid) {
        return Err(ExecutionError::Admission);
    }
    let stage = read_staged_pair(&paths.state_directory, uid)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let desired =
        read_desired_for_decision(paths, uid, lock).map_err(|_| ExecutionError::ManualRecovery)?;
    let intent = DecisionRecord::intent(
        generation,
        desired.as_ref().map(|value| value.as_slice()),
        stage.identity(),
        transaction_id,
    )
    .map_err(|_| ExecutionError::Admission)?;
    let mut bound = Bound {
        config,
        paths,
        uid,
        generation,
        lock,
        desired,
        stage: *stage.identity(),
        intent: &intent,
        config_before: open_private_directory(config, uid)
            .map_err(|_| ExecutionError::ManualRecovery)?
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?,
        gate,
    };
    bound.check_common()?;
    if !matches!(
        bound.classify()?,
        LivePairClass::Old | LivePairClass::Identical
    ) {
        return Err(ExecutionError::Admission);
    }
    // Existing or unreadable decisions are never overwritten. The stage stays
    // fenced even when intent publication itself fails.
    write_record(paths, uid, INTENT, &intent.encode())?;
    bound.check(DecisionPhase::Intent)?;
    if !hook(EffectStep::Intent) {
        return Err(ExecutionError::Ambiguous);
    }
    finish_execution(&mut bound, &stage, &mut hook)
}

fn finish_execution<G: FnMut() -> bool, H: FnMut(EffectStep) -> bool>(
    bound: &mut Bound<'_, G>,
    stage: &VerifiedStage,
    hook: &mut H,
) -> Result<PendingOutcome, ExecutionError> {
    for (index, slot) in NEW_SLOT.iter().enumerate() {
        replace_member(
            bound,
            index,
            stage_bytes(stage, false, index),
            stage_bytes(stage, true, index),
            slot,
            hook,
        )?;
    }
    sync_and_verify_pair(bound, stage, false, DecisionPhase::Intent)?;
    bound.check(DecisionPhase::Intent)?;
    if !matches!(
        bound.classify()?,
        LivePairClass::New | LivePairClass::Identical
    ) {
        return Err(ExecutionError::ManualRecovery);
    }
    write_record(
        bound.paths,
        bound.uid,
        TERMINAL,
        &bound
            .intent
            .terminal(TerminalChoice::Commit)
            .map_err(|_| ExecutionError::ManualRecovery)?
            .encode(),
    )?;
    bound.check(DecisionPhase::Committed)?;
    if !hook(EffectStep::Terminal) {
        return Err(ExecutionError::Ambiguous);
    }
    sync_and_verify_pair(bound, stage, false, DecisionPhase::Committed)?;
    sync_decision_journal(bound, DecisionPhase::Committed)?;
    Ok(PendingOutcome::Committed)
}

/// Independent reopen/recovery entry. Intent without a durable terminal is
/// always rolled back to exact old bytes. A terminal is verified, never
/// reversed. A surviving stage/journal continues to fence startup.
#[allow(dead_code)]
pub(crate) fn recover_staged_pair(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    recover_with_hook(config, paths, uid, generation, lock, (gate, true), |_| true)
}

/// Verify and re-synchronize an existing terminal pair only. This variant
/// never interprets an undecided intent as authority to write old bytes.
#[allow(dead_code)]
pub(crate) fn verify_terminal_staged_pair(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    recover_with_hook(config, paths, uid, generation, lock, (gate, false), |_| {
        true
    })
}

fn recover_with_hook<G: FnMut() -> bool, H: FnMut(EffectStep) -> bool>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    (gate, allow_rollback): (G, bool),
    mut hook: H,
) -> Result<PendingOutcome, ExecutionError> {
    let stage = read_staged_pair(&paths.state_directory, uid)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    let chain =
        inspect_decision_journal(paths, uid, lock).map_err(|_| ExecutionError::ManualRecovery)?;
    let desired =
        read_desired_for_decision(paths, uid, lock).map_err(|_| ExecutionError::ManualRecovery)?;
    let mut bound = Bound {
        config,
        paths,
        uid,
        generation,
        lock,
        desired,
        stage: *stage.identity(),
        intent: chain.intent(),
        config_before: open_private_directory(config, uid)
            .map_err(|_| ExecutionError::ManualRecovery)?
            .metadata()
            .map_err(|_| ExecutionError::ManualRecovery)?,
        gate,
    };
    let phase = chain.active().phase();
    bound.check(phase)?;
    let observed = classify_live_pair_bound(config, paths, uid, generation, lock)
        .map_err(|_| ExecutionError::ManualRecovery)?;
    match chain.review(
        generation,
        bound.desired.as_ref().map(|value| value.as_slice()),
        &observed,
    ) {
        RecoveryReview::VerifyCommittedCandidate => {
            sync_and_verify_pair(&mut bound, &stage, false, DecisionPhase::Committed)?;
            sync_decision_journal(&mut bound, DecisionPhase::Committed)?;
            Ok(PendingOutcome::Committed)
        }
        RecoveryReview::VerifyAbortedCandidate => {
            sync_and_verify_pair(&mut bound, &stage, true, DecisionPhase::Aborted)?;
            sync_decision_journal(&mut bound, DecisionPhase::Aborted)?;
            Ok(PendingOutcome::Aborted)
        }
        RecoveryReview::OldRollbackCandidate => {
            if !allow_rollback {
                return Err(ExecutionError::ManualRecovery);
            }
            for (index, slot) in OLD_SLOT.iter().enumerate() {
                replace_member(
                    &mut bound,
                    index,
                    stage_bytes(&stage, true, index),
                    stage_bytes(&stage, false, index),
                    slot,
                    &mut hook,
                )?;
            }
            sync_and_verify_pair(&mut bound, &stage, true, DecisionPhase::Intent)?;
            bound.check(DecisionPhase::Intent)?;
            if !matches!(
                bound.classify()?,
                LivePairClass::Old | LivePairClass::Identical
            ) {
                return Err(ExecutionError::ManualRecovery);
            }
            write_record(
                paths,
                uid,
                TERMINAL,
                &chain
                    .active()
                    .terminal(TerminalChoice::Abort)
                    .map_err(|_| ExecutionError::ManualRecovery)?
                    .encode(),
            )?;
            bound.check(DecisionPhase::Aborted)?;
            if !hook(EffectStep::Terminal) {
                return Err(ExecutionError::Ambiguous);
            }
            sync_and_verify_pair(&mut bound, &stage, true, DecisionPhase::Aborted)?;
            sync_decision_journal(&mut bound, DecisionPhase::Aborted)?;
            Ok(PendingOutcome::Aborted)
        }
        RecoveryReview::ManualRecovery => Err(ExecutionError::ManualRecovery),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::MigrationLock;
    use crate::desired::DesiredState;
    use crate::restore_staging_candidate::stage_private_pair;
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
            let home = std::env::var_os("HOME").expect("test needs home");
            let root =
                crate::test_temp::directory_under(Path::new(&home), "restore-executor").unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let config = root.join("config");
            let runtime = root.join("runtime");
            let state_base = root.join("state");
            for path in [&config, &runtime, &state_base] {
                fs::create_dir(path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            let paths = CutoverPaths::below(&runtime, &state_base, uid);
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
            Self::member(&config.join(LIVE[0]), OLD_STORE);
            Self::member(&config.join(LIVE[1]), OLD_TEMPLATE);
            stage_private_pair(
                &paths.state_directory,
                uid,
                OLD_STORE,
                OLD_TEMPLATE,
                NEW_STORE,
                NEW_TEMPLATE,
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

        fn assert_pair(&self, old: bool) {
            assert_eq!(
                fs::read(self.config.join(LIVE[0])).unwrap(),
                if old { OLD_STORE } else { NEW_STORE }
            );
            assert_eq!(
                fs::read(self.config.join(LIVE[1])).unwrap(),
                if old { OLD_TEMPLATE } else { NEW_TEMPLATE }
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn exact_commit_keeps_stage_and_terminal_fenced() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        assert_eq!(
            execute_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                [1; 16],
                || true,
            ),
            Ok(PendingOutcome::Committed)
        );
        fixture.assert_pair(false);
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
        assert_eq!(
            recover_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Ok(PendingOutcome::Committed)
        );
        fixture.assert_pair(false);
    }

    #[test]
    fn terminal_only_verification_refuses_undecided_intent_without_live_write() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        let stage = read_staged_pair(&fixture.paths.state_directory, fixture.uid).unwrap();
        let desired = fs::read(fixture.paths.state_directory.join("desired.json")).unwrap();
        let intent = DecisionRecord::intent(2, Some(&desired), stage.identity(), [19; 16]).unwrap();
        Fixture::member(
            &fixture
                .paths
                .state_directory
                .join("restore-decision.intent"),
            &intent.encode(),
        );
        assert_eq!(
            verify_terminal_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            ),
            Err(ExecutionError::ManualRecovery)
        );
        fixture.assert_pair(true);
        assert!(
            !fixture
                .paths
                .state_directory
                .join("restore-decision.terminal")
                .exists()
        );
    }

    #[test]
    fn each_forward_interruption_reopens_to_exact_old_or_durable_new() {
        for stop in [
            EffectStep::Intent,
            EffectStep::Linked(0),
            EffectStep::RenameApplied(0),
            EffectStep::Renamed(0),
            EffectStep::Linked(1),
            EffectStep::RenameApplied(1),
            EffectStep::Renamed(1),
            EffectStep::Terminal,
        ] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            assert_eq!(
                execute_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [2; 16],
                    || true,
                    |step| step != stop,
                ),
                Err(ExecutionError::Ambiguous),
                "interrupted at {stop:?}"
            );
            drop(lock);
            let restarted_lock = fixture.lock();
            let outcome = recover_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &restarted_lock,
                || true,
            );
            if stop == EffectStep::Terminal {
                assert_eq!(outcome, Ok(PendingOutcome::Committed));
                fixture.assert_pair(false);
            } else {
                assert_eq!(outcome, Ok(PendingOutcome::Aborted), "at {stop:?}");
                fixture.assert_pair(true);
            }
            assert!(fixture.paths.state_directory.join(INTENT).is_file());
            assert!(fixture.paths.state_directory.join(TERMINAL).is_file());
        }
    }

    #[test]
    fn interrupted_rollback_is_idempotent_after_reopen() {
        for stop in [
            EffectStep::Linked(0),
            EffectStep::RenameApplied(0),
            EffectStep::Renamed(0),
            EffectStep::Linked(1),
            EffectStep::RenameApplied(1),
            EffectStep::Renamed(1),
            EffectStep::Terminal,
        ] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            assert_eq!(
                execute_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [3; 16],
                    || true,
                    |step| step != EffectStep::Renamed(1),
                ),
                Err(ExecutionError::Ambiguous)
            );
            drop(lock);
            let lock = fixture.lock();
            assert_eq!(
                recover_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    (|| true, true),
                    |step| step != stop,
                ),
                Err(ExecutionError::Ambiguous),
                "rollback stopped at {stop:?}"
            );
            drop(lock);
            let lock = fixture.lock();
            assert_eq!(
                recover_staged_pair(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true
                ),
                Ok(PendingOutcome::Aborted)
            );
            fixture.assert_pair(true);
        }
    }

    #[test]
    fn changed_owner_desired_or_live_file_never_becomes_a_restore_effect() {
        for change in 0..4 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            assert_eq!(
                execute_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [4; 16],
                    || true,
                    |step| step != EffectStep::Intent,
                ),
                Err(ExecutionError::Ambiguous)
            );
            match change {
                0 => change_owner(&fixture),
                1 => Fixture::member(
                    &fixture.paths.state_directory.join("desired.json"),
                    &serde_json::to_vec(&DesiredState {
                        generation: 1,
                        ..DesiredState::default()
                    })
                    .unwrap(),
                ),
                2 => Fixture::member(&fixture.config.join(LIVE[0]), b"unrelated diverged data"),
                _ => {
                    fs::remove_file(fixture.config.join(LIVE[0])).unwrap();
                    symlink(fixture.config.join(LIVE[1]), fixture.config.join(LIVE[0])).unwrap();
                }
            }
            assert!(matches!(
                recover_staged_pair(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true
                ),
                Err(ExecutionError::Admission | ExecutionError::ManualRecovery)
            ));
            assert!(fixture.paths.state_directory.join(INTENT).exists());
            assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        }
    }

    #[test]
    fn substituted_intent_identity_cannot_authorize_live_replacement() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        let stage = inspect_stage_identity(&fixture.paths.state_directory, fixture.uid).unwrap();
        let desired = fs::read(fixture.paths.state_directory.join("desired.json")).unwrap();
        let substitute = DecisionRecord::intent(2, Some(&desired), &stage, [9; 16]).unwrap();
        assert_eq!(
            execute_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                [4; 16],
                || true,
                |step| {
                    if step == EffectStep::Intent {
                        Fixture::member(
                            &fixture.paths.state_directory.join(INTENT),
                            &substitute.encode(),
                        );
                    }
                    true
                },
            ),
            Err(ExecutionError::ManualRecovery)
        );
        fixture.assert_pair(true);
        assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
    }

    #[test]
    fn altered_replacement_slot_never_reaches_a_live_member() {
        for mutation in 0..3 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            assert_eq!(
                execute_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [5; 16],
                    || true,
                    |step| {
                        if step == EffectStep::Linked(0) {
                            let slot = fixture.config.join(NEW_SLOT[0]);
                            fs::remove_file(&slot).unwrap();
                            match mutation {
                                0 => Fixture::member(&slot, b"safe but wrong content"),
                                1 => symlink(fixture.config.join(LIVE[1]), slot).unwrap(),
                                _ => fs::hard_link(fixture.config.join(LIVE[1]), slot).unwrap(),
                            }
                        }
                        true
                    },
                ),
                Err(ExecutionError::ManualRecovery)
            );
            fixture.assert_pair(true);
            assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        }
    }

    #[test]
    fn absent_off_desired_and_identical_pair_have_durable_terminal_records() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.paths.state_directory.join("desired.json")).unwrap();
        fs::remove_dir_all(fixture.paths.state_directory.join("restore-pair.pending")).unwrap();
        stage_private_pair(
            &fixture.paths.state_directory,
            fixture.uid,
            OLD_STORE,
            OLD_TEMPLATE,
            OLD_STORE,
            OLD_TEMPLATE,
        )
        .unwrap();
        let lock = fixture.lock();
        assert_eq!(
            execute_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                [6; 16],
                || true,
            ),
            Ok(PendingOutcome::Committed)
        );
        fixture.assert_pair(true);
        drop(lock);
        let lock = fixture.lock();
        assert_eq!(
            recover_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Ok(PendingOutcome::Committed)
        );
        assert!(!fixture.paths.state_directory.join("desired.json").exists());
    }

    #[test]
    fn changed_config_directory_or_host_gate_refuses_after_intent() {
        for change in 0..2 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            let gate_open = std::cell::Cell::new(true);
            assert!(matches!(
                execute_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [7; 16],
                    || gate_open.get(),
                    |step| {
                        if step == EffectStep::Intent {
                            if change == 0 {
                                gate_open.set(false);
                            } else {
                                let moved = fixture.root.join("config-moved");
                                fs::rename(&fixture.config, &moved).unwrap();
                                fs::create_dir(&fixture.config).unwrap();
                                fs::set_permissions(
                                    &fixture.config,
                                    fs::Permissions::from_mode(0o700),
                                )
                                .unwrap();
                                for name in LIVE {
                                    Fixture::member(
                                        &fixture.config.join(name),
                                        &fs::read(moved.join(name)).unwrap(),
                                    );
                                }
                            }
                        }
                        true
                    },
                ),
                Err(ExecutionError::Admission | ExecutionError::ManualRecovery)
            ));
            fixture.assert_pair(true);
            assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        }
    }

    fn change_owner(fixture: &Fixture) {
        Fixture::member(
            &fixture.paths.ownership_marker,
            br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
        );
    }

    #[test]
    fn abrupt_process_loss_at_durable_effect_boundaries_recovers() {
        for (code, committed) in [
            ("intent", false),
            ("linked0", false),
            ("applied0", false),
            ("renamed0", false),
            ("linked1", false),
            ("applied1", false),
            ("renamed1", false),
            ("terminal", true),
        ] {
            let fixture = Fixture::new();
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("restore_executor_candidate::tests::crash_worker")
                .env("OMAVLESS_RESTORE_TEST_ROOT", &fixture.root)
                .env("OMAVLESS_RESTORE_TEST_STEP", code)
                .status()
                .unwrap();
            assert_eq!(child.signal(), Some(9), "worker did not die at {code}");
            let lock = fixture.lock();
            assert_eq!(
                recover_staged_pair(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true
                ),
                Ok(if committed {
                    PendingOutcome::Committed
                } else {
                    PendingOutcome::Aborted
                }),
                "reopen after {code}"
            );
            fixture.assert_pair(!committed);
        }
    }

    #[test]
    fn abrupt_process_loss_during_rollback_reopens_to_exact_old_pair() {
        for code in [
            "linked0", "applied0", "renamed0", "linked1", "applied1", "renamed1", "terminal",
        ] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            assert_eq!(
                execute_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [10; 16],
                    || true,
                    |step| step != EffectStep::Renamed(1),
                ),
                Err(ExecutionError::Ambiguous)
            );
            drop(lock);
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("restore_executor_candidate::tests::crash_worker")
                .env("OMAVLESS_RESTORE_TEST_ROOT", &fixture.root)
                .env("OMAVLESS_RESTORE_TEST_PHASE", "rollback")
                .env("OMAVLESS_RESTORE_TEST_STEP", code)
                .status()
                .unwrap();
            assert_eq!(child.signal(), Some(9), "worker did not die at {code}");
            let lock = fixture.lock();
            assert_eq!(
                recover_staged_pair(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true
                ),
                Ok(PendingOutcome::Aborted),
                "reopen rollback after {code}"
            );
            fixture.assert_pair(true);
        }
    }

    #[test]
    fn torn_terminal_refuses_instead_of_guessing_rollback_or_commit() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        assert_eq!(
            execute_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                [11; 16],
                || true,
                |step| step != EffectStep::Renamed(1),
            ),
            Err(ExecutionError::Ambiguous)
        );
        Fixture::member(&fixture.paths.state_directory.join(TERMINAL), b"torn");
        assert_eq!(
            recover_staged_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Err(ExecutionError::ManualRecovery)
        );
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }

    #[test]
    fn byte_identical_journal_replacement_after_sync_is_not_durable_success() {
        for (after_sync, swapped_member) in
            [(INTENT, INTENT), (TERMINAL, TERMINAL), (TERMINAL, INTENT)]
        {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            assert_eq!(
                execute_staged_pair(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    [12; 16],
                    || true,
                ),
                Ok(PendingOutcome::Committed)
            );
            let stage = read_staged_pair(&fixture.paths.state_directory, fixture.uid).unwrap();
            let desired = read_desired_for_decision(&fixture.paths, fixture.uid, &lock).unwrap();
            let chain = inspect_decision_journal(&fixture.paths, fixture.uid, &lock).unwrap();
            let mut bound = Bound {
                config: &fixture.config,
                paths: &fixture.paths,
                uid: fixture.uid,
                generation: 2,
                lock: &lock,
                desired,
                stage: *stage.identity(),
                intent: chain.intent(),
                config_before: fs::metadata(&fixture.config).unwrap(),
                gate: || true,
            };
            assert_eq!(
                sync_decision_journal_with_hook(&mut bound, DecisionPhase::Committed, |member| {
                    if member == after_sync {
                        let path = fixture.paths.state_directory.join(swapped_member);
                        let bytes = fs::read(&path).unwrap();
                        fs::remove_file(&path).unwrap();
                        Fixture::member(&path, &bytes);
                    }
                }),
                Err(ExecutionError::ManualRecovery)
            );
            fixture.assert_pair(false);
        }
    }

    #[test]
    fn crash_worker() {
        let Some(root) = std::env::var_os("OMAVLESS_RESTORE_TEST_ROOT") else {
            return;
        };
        let fixture = Fixture::reopen(PathBuf::from(root));
        let lock = fixture.lock();
        let target = std::env::var("OMAVLESS_RESTORE_TEST_STEP").unwrap();
        let step_name = |step| match step {
            EffectStep::Intent => "intent",
            EffectStep::Linked(0) => "linked0",
            EffectStep::RenameApplied(0) => "applied0",
            EffectStep::Renamed(0) => "renamed0",
            EffectStep::Linked(1) => "linked1",
            EffectStep::RenameApplied(1) => "applied1",
            EffectStep::Renamed(1) => "renamed1",
            EffectStep::Terminal => "terminal",
            _ => "invalid",
        };
        let mut crash_at_step = |step| {
            if step_name(step) == target {
                nix::sys::signal::kill(nix::unistd::Pid::this(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            true
        };
        if std::env::var("OMAVLESS_RESTORE_TEST_PHASE").as_deref() == Ok("rollback") {
            let _ = recover_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                (|| true, true),
                &mut crash_at_step,
            );
        } else {
            let _ = execute_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                [8; 16],
                || true,
                &mut crash_at_step,
            );
        }
        panic!("expected an abrupt worker termination");
    }
}
