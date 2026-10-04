// SPDX-License-Identifier: MIT

//! Inactive create-only successor stage/intent preparation. Both predecessor
//! and handoff fences survive. Partial artifacts refuse retry; no live effects.

use super::*;
use crate::restore_staging_candidate::stage_private_pair_checked;
use std::io::Write;
use std::os::unix::fs::MetadataExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrepareError {
    Admission,
    Ambiguous,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PrepareResult {
    PreparedStillFenced,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    StageProgress,
    IntentCreated,
    IntentWritten,
    IntentSynced,
    DirectorySynced,
    Reopened,
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    backup: &'a OpenedBackup,
    state: File,
    config_dir: File,
    original: Snapshot,
}
impl Context<'_> {
    /// Checks only invariant predecessor/live evidence during partial staging;
    /// never treats that stage as complete or authorizes an intent from it.
    fn base(&self, intent_absent: bool) -> Result<()> {
        if !self.lock.authorizes(self.paths, self.uid) {
            return Err(REFUSE);
        }
        let marker = read_marker_existing(self.paths, self.uid).map_err(|_| REFUSE)?;
        let desired =
            read_desired_for_decision(self.paths, self.uid, self.lock).map_err(|_| REFUSE)?;
        let handoff = SuccessorHandoff::decode(&self.original.handoff).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust
            || marker.generation() != self.generation
            || !handoff
                .successor_intent()
                .matches_owner_desired(self.generation, desired.as_ref().map(|b| b.as_slice()))
        {
            return Err(REFUSE);
        }
        for name in [
            RECEIPT_MEMBER,
            "restore-decision.terminal",
            "routing-preset.pending.json",
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
        ] {
            absent(&self.state, name)?;
        }
        if intent_absent {
            absent(&self.state, "restore-decision.intent")?;
        }
        for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
            absent(&self.config_dir, name)?;
        }
        for (index, (directory, name, bytes, limit)) in [
            (
                &self.state,
                CLOSURE_MEMBER,
                handoff.predecessor().encode().as_slice(),
                CLOSURE_BYTES,
            ),
            (
                &self.state,
                SUCCESSOR_MEMBER,
                self.original.handoff.as_slice(),
                RECORD_BYTES,
            ),
            (
                &self.config_dir,
                "profiles.json",
                self.original.old[0].as_slice(),
                MAX_PRIVATE_STORE_BYTES,
            ),
            (
                &self.config_dir,
                "route-template.yaml",
                self.original.old[1].as_slice(),
                MAX_TEMPLATE_BYTES,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let (raw, metadata) = read_optional(directory, name, self.uid, limit)
                .map_err(|_| REFUSE)?
                .ok_or(REFUSE)?;
            if raw.as_slice() != bytes
                || !same_member(&metadata, &self.original.base_identities[index])
            {
                return Err(REFUSE);
            }
        }
        for (index, path) in [self.paths.state_directory.as_path(), self.config]
            .into_iter()
            .enumerate()
        {
            if !same_directory(
                &self.original.directories[index],
                &open_private_directory(path, self.uid)
                    .map_err(|_| REFUSE)?
                    .metadata()
                    .map_err(|_| REFUSE)?,
            ) {
                return Err(REFUSE);
            }
        }
        if read_marker_existing(self.paths, self.uid).ok() != Some(marker)
            || read_desired_for_decision(self.paths, self.uid, self.lock).map_err(|_| REFUSE)?
                != desired
        {
            return Err(REFUSE);
        }
        Ok(())
    }
    fn phase(&self, phase: PreparationPhase, gate: &mut impl FnMut() -> bool) -> Result<Snapshot> {
        let current = inspect_snapshot(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
            self.backup,
            phase,
            gate,
        )?;
        if !self.original.same_base(&current) {
            return Err(REFUSE);
        }
        Ok(current)
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn prepare_successor(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> std::result::Result<PrepareResult, PrepareError> {
    prepare_with_hook(config, paths, uid, generation, lock, backup, gate, |_| true)
}

#[allow(clippy::too_many_arguments)]
fn prepare_with_hook(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(Checkpoint) -> bool,
) -> std::result::Result<PrepareResult, PrepareError> {
    let initial = inspect_snapshot(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        PreparationPhase::HandoffOnly,
        &mut gate,
    )
    .map_err(|_| PrepareError::Admission)?;
    let context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        state: open_private_directory(&paths.state_directory, uid)
            .map_err(|_| PrepareError::Admission)?,
        config_dir: open_private_directory(config, uid).map_err(|_| PrepareError::Admission)?,
        original: initial,
    };
    let off = backup
        .restore_store_off()
        .map_err(|_| PrepareError::Admission)?;
    // Re-establish durability of both invariant fence records; a visible file
    // from an earlier interrupted publication is not by itself a sync proof.
    for (index, name) in [CLOSURE_MEMBER, SUCCESSOR_MEMBER].into_iter().enumerate() {
        let file = File::from(
            openat(
                &context.state,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| PrepareError::Admission)?,
        );
        if !same_member(
            &file.metadata().map_err(|_| PrepareError::Admission)?,
            &context.original.base_identities[index],
        ) {
            return Err(PrepareError::Admission);
        }
        file.sync_all().map_err(|_| PrepareError::Admission)?;
    }
    context
        .state
        .sync_all()
        .map_err(|_| PrepareError::Admission)?;
    context
        .phase(PreparationPhase::HandoffOnly, &mut gate)
        .map_err(|_| PrepareError::Admission)?;
    // Never retry over partial or even complete pre-existing preparation.
    let mut effects = || -> Result<PrepareResult> {
        stage_private_pair_checked(
            &paths.state_directory,
            uid,
            [
                context.original.old[0].as_slice(),
                context.original.old[1].as_slice(),
                &off,
                backup.template(),
            ],
            || hook(Checkpoint::StageProgress) && gate() && context.base(true).is_ok(),
        )
        .map_err(|_| REFUSE)?;
        let staged = context.phase(PreparationPhase::StageWithoutIntent, &mut gate)?;
        publish_intent(&context, &staged, &mut gate, &mut hook)
    };
    effects().map_err(|_| PrepareError::Ambiguous)
}

fn publish_intent(
    context: &Context<'_>,
    staged: &Snapshot,
    gate: &mut impl FnMut() -> bool,
    hook: &mut impl FnMut(Checkpoint) -> bool,
) -> Result<PrepareResult> {
    let uid = context.uid;
    let handoff = SuccessorHandoff::decode(&context.original.handoff).map_err(|_| REFUSE)?;
    let bytes = handoff.successor_intent().encode();
    let mut file = File::from(
        openat(
            &context.state,
            Path::new("restore-decision.intent"),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| REFUSE)?,
    );
    let created = file.metadata().map_err(|_| REFUSE)?;
    if !created.is_file()
        || created.uid() != uid
        || created.mode() & 0o7777 != 0o600
        || created.nlink() != 1
        || !hook(Checkpoint::IntentCreated)
    {
        return Err(REFUSE);
    }
    file.write_all(&bytes).map_err(|_| REFUSE)?;
    if !hook(Checkpoint::IntentWritten) {
        return Err(REFUSE);
    }
    file.sync_all().map_err(|_| REFUSE)?;
    if !hook(Checkpoint::IntentSynced) {
        return Err(REFUSE);
    }
    context.state.sync_all().map_err(|_| REFUSE)?;
    if !hook(Checkpoint::DirectorySynced) {
        return Err(REFUSE);
    }
    let durable = file.metadata().map_err(|_| REFUSE)?;
    let verify = || -> Result<()> {
        let (raw, meta) = read_optional(
            &context.state,
            "restore-decision.intent",
            uid,
            DECISION_BYTES,
        )
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
        if raw.as_slice() != bytes
            || created.dev() != durable.dev()
            || created.ino() != durable.ino()
            || !same_member(&durable, &meta)
            || !same_member(&durable, &file.metadata().map_err(|_| REFUSE)?)
        {
            return Err(REFUSE);
        }
        Ok(())
    };
    verify()?;
    if !hook(Checkpoint::Reopened) {
        return Err(REFUSE);
    }
    let prepared = context.phase(PreparationPhase::StageWithIntent, gate)?;
    if !staged.same_stage(&prepared) {
        return Err(REFUSE);
    }
    context.base(false)?;
    verify()?;
    Ok(PrepareResult::PreparedStillFenced)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryCheckpoint {
    FileSynced,
    StageDirectorySynced,
    StateDirectorySynced,
    Rechecked,
}

/// Explicit inactive recovery of an exact authenticated phase. Complete
/// existing bytes are resynchronized before continuation; partial stage/intent
/// and terminal evidence are never overwritten, rolled back or auto-repaired.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn recover_successor_preparation(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    phase: PreparationPhase,
    gate: impl FnMut() -> bool,
) -> std::result::Result<PrepareResult, PrepareError> {
    recover_with_hook(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        phase,
        gate,
        |_| true,
    )
}

#[allow(clippy::too_many_arguments)]
fn recover_with_hook(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    phase: PreparationPhase,
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(RecoveryCheckpoint) -> bool,
) -> std::result::Result<PrepareResult, PrepareError> {
    if phase == PreparationPhase::HandoffOnly {
        return prepare_successor(config, paths, uid, generation, lock, backup, gate);
    }
    let original = inspect_snapshot(
        config, paths, uid, generation, lock, backup, phase, &mut gate,
    )
    .map_err(|_| PrepareError::Admission)?;
    let context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        state: open_private_directory(&paths.state_directory, uid)
            .map_err(|_| PrepareError::Admission)?,
        config_dir: open_private_directory(config, uid).map_err(|_| PrepareError::Admission)?,
        original,
    };
    let mut resume = || -> Result<PrepareResult> {
        let stage = open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
            .map_err(|_| REFUSE)?;
        if !same_directory(
            &context.original.directories[2],
            &stage.metadata().map_err(|_| REFUSE)?,
        ) {
            return Err(REFUSE);
        }
        // Same ordering as observe(): closure, handoff, optional intent,
        // live pair, then all five stage files. Synchronize only those pinned
        // exact members, never a path discovered from credential-bearing data.
        let mut members: Vec<(&File, &str)> = vec![
            (&context.state, CLOSURE_MEMBER),
            (&context.state, SUCCESSOR_MEMBER),
        ];
        if phase == PreparationPhase::StageWithIntent {
            members.push((&context.state, "restore-decision.intent"));
        }
        members.extend([
            (&context.config_dir, "profiles.json"),
            (&context.config_dir, "route-template.yaml"),
        ]);
        members.extend(
            MEMBERS
                .into_iter()
                .chain([READY_MEMBER])
                .map(|name| (&stage, name)),
        );
        if members.len() != context.original.members.len() {
            return Err(REFUSE);
        }
        for ((parent, name), expected) in members.iter().zip(&context.original.members) {
            if !gate() {
                return Err(REFUSE);
            }
            let checked = context.phase(phase, &mut gate)?;
            if !context.original.same(&checked) {
                return Err(REFUSE);
            }
            let file = File::from(
                openat(
                    *parent,
                    Path::new(name),
                    OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| REFUSE)?,
            );
            if !same_member(expected, &file.metadata().map_err(|_| REFUSE)?) {
                return Err(REFUSE);
            }
            file.sync_all().map_err(|_| REFUSE)?;
            if !hook(RecoveryCheckpoint::FileSynced) {
                return Err(REFUSE);
            }
        }
        stage.sync_all().map_err(|_| REFUSE)?;
        if !hook(RecoveryCheckpoint::StageDirectorySynced) {
            return Err(REFUSE);
        }
        context.config_dir.sync_all().map_err(|_| REFUSE)?;
        context.state.sync_all().map_err(|_| REFUSE)?;
        if !hook(RecoveryCheckpoint::StateDirectorySynced) {
            return Err(REFUSE);
        }
        let synced = context.phase(phase, &mut gate)?;
        if !context.original.same(&synced) || !hook(RecoveryCheckpoint::Rechecked) {
            return Err(REFUSE);
        }
        // No hook or host callback may leave stale evidence before intent create.
        let final_check = context.phase(phase, &mut gate)?;
        if !synced.same(&final_check) {
            return Err(REFUSE);
        }
        if phase == PreparationPhase::StageWithoutIntent {
            publish_intent(&context, &final_check, &mut gate, &mut |_| true)
        } else {
            Ok(PrepareResult::PreparedStillFenced)
        }
    };
    resume().map_err(|_| PrepareError::Ambiguous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_successor_publication_candidate::publish_successor_handoff;
    use crate::restore_successor_publication_candidate::tests::{Fixture, OLD, backup};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Command;

    fn handoff() -> (Fixture, MigrationLock) {
        let f = Fixture::new();
        let lock = f.lock();
        publish_successor_handoff(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            backup(),
            [2; 16],
            || true,
        )
        .unwrap();
        (f, lock)
    }
    fn prepare(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> std::result::Result<PrepareResult, PrepareError> {
        prepare_with_hook(&f.config, &f.paths, f.uid, 2, lock, backup(), || true, hook)
    }
    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn preserved(f: &Fixture, before: &[u8], handoff: &[u8]) {
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            before
        );
        assert_eq!(
            fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap(),
            handoff
        );
        assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), OLD[0]);
        assert_eq!(
            fs::read(f.config.join("route-template.yaml")).unwrap(),
            OLD[1]
        );
        assert!(!f.paths.state_directory.join("desired.json").exists());
        assert!(
            !f.paths
                .state_directory
                .join("restore-decision.terminal")
                .exists()
        );
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
    }

    #[test]
    fn create_only_preparation_keeps_fences_live_pair_and_matching_intent() {
        let (f, lock) = handoff();
        let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        assert_eq!(
            prepare(&f, &lock, |_| true),
            Ok(PrepareResult::PreparedStillFenced)
        );
        assert_eq!(
            inspect_successor_coexistence(&f.config, &f.paths, f.uid, 2, &lock, backup(), || true),
            Ok(CoexistenceReview::MatchingIntentStillFenced)
        );
        preserved(&f, &before, &h);
        assert_eq!(prepare(&f, &lock, |_| true), Err(PrepareError::Admission));
        preserved(&f, &before, &h);
    }

    #[test]
    fn preparation_interruptions_are_ambiguous_and_never_auto_retried() {
        for stopped in 0..12 {
            let (f, lock) = handoff();
            let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
            let mut index = 0;
            assert_eq!(
                prepare(&f, &lock, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                }),
                Err(PrepareError::Ambiguous)
            );
            preserved(&f, &before, &h);
            assert_eq!(prepare(&f, &lock, |_| true), Err(PrepareError::Admission));
        }
    }

    #[test]
    fn final_stage_or_intent_substitution_cannot_report_prepared() {
        for target in ["stage", "intent", "receipt", "owner"] {
            let (f, lock) = handoff();
            let result = prepare(&f, &lock, |point| {
                if point == Checkpoint::Reopened {
                    match target {
                        "stage" => {
                            let path = f.paths.state_directory.join(PENDING_DIRECTORY);
                            let moved = f.root.join("moved-stage");
                            fs::rename(&path, &moved).unwrap();
                            fs::create_dir(&path).unwrap();
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                            for name in MEMBERS.into_iter().chain([READY_MEMBER]) {
                                write(&path.join(name), &fs::read(moved.join(name)).unwrap());
                            }
                        }
                        "intent" => {
                            let path = f.paths.state_directory.join("restore-decision.intent");
                            let raw = fs::read(&path).unwrap();
                            fs::rename(&path, f.root.join("moved-intent")).unwrap();
                            write(&path, &raw);
                        }
                        "receipt" => write(&f.paths.state_directory.join(RECEIPT_MEMBER), b"late"),
                        "owner" => write(
                            &f.paths.ownership_marker,
                            br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                        ),
                        _ => unreachable!(),
                    }
                }
                true
            });
            assert_eq!(result, Err(PrepareError::Ambiguous));
        }
    }

    #[test]
    fn owner_drift_during_partial_stage_stops_before_next_member() {
        let (f, lock) = handoff();
        assert_eq!(
            prepare(&f, &lock, |point| {
                if point == Checkpoint::StageProgress {
                    write(
                        &f.paths.ownership_marker,
                        br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                    );
                }
                true
            }),
            Err(PrepareError::Ambiguous)
        );
        let stage = f.paths.state_directory.join(PENDING_DIRECTORY);
        assert_eq!(fs::read_dir(stage).unwrap().count(), 0);
        assert!(
            !f.paths
                .state_directory
                .join("restore-decision.intent")
                .exists()
        );
    }

    #[test]
    fn actual_preparation_crashes_reopen_only_as_fenced_evidence() {
        for stopped in 0..12 {
            let (f, lock) = handoff();
            drop(lock);
            let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "restore_successor_coexistence_candidate::preparation::tests::preparation_crash_worker"])
                .env("OMAVLESS_SYNTHETIC_PREPARATION_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_PREPARATION_POINT", stopped.to_string())
                .output().unwrap();
            assert_eq!(output.status.signal(), Some(9));
            let lock = f.lock();
            assert_eq!(prepare(&f, &lock, |_| true), Err(PrepareError::Admission));
            for phase in [
                PreparationPhase::HandoffOnly,
                PreparationPhase::StageWithoutIntent,
                PreparationPhase::StageWithIntent,
            ] {
                let expected = matches!(
                    (stopped, phase),
                    (5 | 6, PreparationPhase::StageWithoutIntent)
                        | (8..=11, PreparationPhase::StageWithIntent)
                );
                let result = inspect_successor_preparation(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    phase,
                    || true,
                );
                assert_eq!(result, if expected { Ok(phase) } else { Err(REFUSE) });
            }
            preserved(&f, &before, &h);
            let phase = match stopped {
                5 | 6 => PreparationPhase::StageWithoutIntent,
                _ => PreparationPhase::StageWithIntent,
            };
            let recovered = recover_successor_preparation(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                backup(),
                phase,
                || true,
            );
            assert_eq!(
                recovered,
                if matches!(stopped, 5 | 6 | 8..=11) {
                    Ok(PrepareResult::PreparedStillFenced)
                } else {
                    Err(PrepareError::Admission)
                }
            );
            preserved(&f, &before, &h);
        }
    }

    #[test]
    fn recovery_resync_interruptions_preserve_exact_fences_and_can_be_reinspected() {
        for (phase, count) in [
            (PreparationPhase::StageWithoutIntent, 12),
            (PreparationPhase::StageWithIntent, 13),
        ] {
            for stopped in 0..count {
                let (f, lock) = handoff();
                let mut index = 0;
                let _ = prepare(&f, &lock, |_| {
                    let keep = phase == PreparationPhase::StageWithIntent || index != 5;
                    index += 1;
                    keep
                });
                let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
                let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
                let mut index = 0;
                assert_eq!(
                    recover_with_hook(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        backup(),
                        phase,
                        || true,
                        |_| {
                            let keep = index != stopped;
                            index += 1;
                            keep
                        }
                    ),
                    Err(PrepareError::Ambiguous)
                );
                assert_eq!(index, stopped + 1);
                preserved(&f, &before, &h);
                assert_eq!(
                    recover_successor_preparation(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        backup(),
                        phase,
                        || true
                    ),
                    Ok(PrepareResult::PreparedStillFenced)
                );
                preserved(&f, &before, &h);
            }
        }
    }

    #[test]
    fn recovery_final_recheck_rejects_same_byte_replacement_and_late_gate() {
        for target in ["intent", "receipt", "gate"] {
            let (f, lock) = handoff();
            prepare(&f, &lock, |_| true).unwrap();
            let allowed = std::cell::Cell::new(true);
            assert_eq!(
                recover_with_hook(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    PreparationPhase::StageWithIntent,
                    || allowed.get(),
                    |point| {
                        if point == RecoveryCheckpoint::Rechecked {
                            match target {
                                "intent" => {
                                    let path =
                                        f.paths.state_directory.join("restore-decision.intent");
                                    let raw = fs::read(&path).unwrap();
                                    fs::rename(&path, f.root.join("old-intent")).unwrap();
                                    write(&path, &raw);
                                }
                                "receipt" => {
                                    write(&f.paths.state_directory.join(RECEIPT_MEMBER), b"late")
                                }
                                "gate" => allowed.set(false),
                                _ => unreachable!(),
                            }
                        }
                        true
                    }
                ),
                Err(PrepareError::Ambiguous)
            );
        }
    }

    #[test]
    fn recovery_requires_fresh_matching_authenticated_archive() {
        let (f, lock) = handoff();
        prepare(&f, &lock, |_| true).unwrap();
        let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(backup().store()).unwrap();
        value["onboardingComplete"] = true.into();
        let encrypted = omavless_domain::private_backup::seal(
            &serde_json::to_vec(&value).unwrap(),
            backup().template(),
            b"synthetic recovery archive",
        )
        .unwrap();
        assert!(omavless_domain::private_backup::open(&encrypted, b"wrong passphrase").is_err());
        assert!(omavless_domain::private_backup::open(&[], b"missing archive").is_err());
        let other =
            omavless_domain::private_backup::open(&encrypted, b"synthetic recovery archive")
                .unwrap();
        assert_eq!(
            recover_successor_preparation(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                &other,
                PreparationPhase::StageWithIntent,
                || true
            ),
            Err(PrepareError::Admission)
        );
        preserved(&f, &before, &h);
    }

    #[test]
    fn composed_publication_preparation_recovery_stays_fenced_without_live_effects() {
        let (f, lock) = handoff();
        let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        assert_eq!(
            recover_successor_preparation(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                backup(),
                PreparationPhase::HandoffOnly,
                || true
            ),
            Ok(PrepareResult::PreparedStillFenced)
        );
        let path = f.paths.state_directory.join("restore-decision.intent");
        let inode = fs::metadata(&path).unwrap().ino();
        assert_eq!(
            recover_successor_preparation(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                backup(),
                PreparationPhase::StageWithIntent,
                || true
            ),
            Ok(PrepareResult::PreparedStillFenced)
        );
        assert_eq!(fs::metadata(path).unwrap().ino(), inode);
        preserved(&f, &before, &h);
    }

    #[test]
    #[ignore = "internal synthetic preparation crash worker"]
    fn preparation_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_PREPARATION_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_PREPARATION_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = prepare(&f, &lock, |_| {
            if index == selected {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            index += 1;
            true
        });
        panic!("expected synthetic worker termination");
    }

    #[test]
    fn actual_resync_crashes_reopen_and_recover_without_live_effects() {
        for stopped in [0, 11] {
            let (f, lock) = handoff();
            let mut index = 0;
            let _ = prepare(&f, &lock, |_| {
                let keep = index != 5;
                index += 1;
                keep
            });
            drop(lock);
            let before = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "restore_successor_coexistence_candidate::preparation::tests::resync_crash_worker"])
                .env("OMAVLESS_SYNTHETIC_PREPARATION_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_PREPARATION_POINT", stopped.to_string())
                .output().unwrap();
            assert_eq!(output.status.signal(), Some(9));
            let lock = f.lock();
            assert!(
                !f.paths
                    .state_directory
                    .join("restore-decision.intent")
                    .exists()
            );
            assert_eq!(
                recover_successor_preparation(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    PreparationPhase::StageWithoutIntent,
                    || true
                ),
                Ok(PrepareResult::PreparedStillFenced)
            );
            preserved(&f, &before, &h);
        }
    }

    #[test]
    #[ignore = "internal synthetic resync crash worker"]
    fn resync_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_PREPARATION_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_PREPARATION_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = recover_with_hook(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            backup(),
            PreparationPhase::StageWithoutIntent,
            || true,
            |_| {
                if index == selected {
                    nix::sys::signal::kill(
                        nix::unistd::getpid(),
                        nix::sys::signal::Signal::SIGKILL,
                    )
                    .unwrap();
                }
                index += 1;
                true
            },
        );
        panic!("expected synthetic worker termination");
    }
}
