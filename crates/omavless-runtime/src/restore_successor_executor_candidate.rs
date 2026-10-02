// SPDX-License-Identifier: MIT

//! Inactive successor execution. Immutable handoff evidence is distinct from
//! mutable live-pair classification. No retirement or normal-owner authority.

use super::*;
use crate::restore_cleanup_candidate::read_optional;
use crate::restore_closure_model::{CLOSURE_MEMBER, ClosureRecord, RECORD_BYTES as CLOSURE_BYTES};
use crate::restore_retirement_candidate::RECEIPT_MEMBER;
use crate::restore_staging_candidate::{MEMBERS, PENDING_DIRECTORY, READY_BYTES, READY_MEMBER};
use crate::restore_successor_coexistence_candidate::{
    PreparationPhase, preparation::recover_successor_preparation,
};
use crate::restore_successor_handoff_model::{
    RECORD_BYTES as HANDOFF_BYTES, SUCCESSOR_MEMBER, SuccessorHandoff,
};
use omavless_domain::private_backup::OpenedBackup;

const REFUSE: ExecutionError = ExecutionError::ManualRecovery;
struct Evidence {
    directories: [Metadata; 3],
    members: Vec<(Zeroizing<Vec<u8>>, Metadata)>,
    terminal: Option<(Zeroizing<Vec<u8>>, Metadata)>,
    intent: DecisionRecord,
}

fn absent(directory: &File, name: &str) -> Result<(), ExecutionError> {
    match openat(
        directory,
        Path::new(name),
        OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Err(Errno::ENOENT) => Ok(()),
        _ => Err(REFUSE),
    }
}

#[allow(clippy::too_many_arguments)]
fn observe(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    new_store: &[u8],
    new_template: &[u8],
) -> Result<Evidence, ExecutionError> {
    if !lock.authorizes(paths, uid) {
        return Err(REFUSE);
    }
    let marker = read_marker_existing(paths, uid).map_err(|_| REFUSE)?;
    if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
        return Err(REFUSE);
    }
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let config_dir = open_private_directory(config, uid).map_err(|_| REFUSE)?;
    let stage_dir = open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
        .map_err(|_| REFUSE)?;
    for name in [RECEIPT_MEMBER, "routing-preset.pending.json"] {
        absent(&state, name)?;
    }
    let desired = read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?;
    let stage = read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    if stage.new_store() != new_store || stage.new_template() != new_template {
        return Err(REFUSE);
    }
    for (old, slots) in [(false, NEW_SLOT), (true, OLD_SLOT)] {
        for (index, name) in slots.into_iter().enumerate() {
            if let Some((raw, _)) = read_optional(
                &config_dir,
                name,
                uid,
                if index == 0 {
                    MAX_PRIVATE_STORE_BYTES
                } else {
                    MAX_TEMPLATE_BYTES
                },
            )
            .map_err(|_| REFUSE)?
                && raw.as_slice() != stage_bytes(&stage, old, index)
            {
                return Err(REFUSE);
            }
        }
    }
    let mut members = Vec::new();
    for (name, limit) in [
        (CLOSURE_MEMBER, CLOSURE_BYTES),
        (SUCCESSOR_MEMBER, HANDOFF_BYTES),
        (INTENT, RECORD_BYTES),
    ] {
        members.push(
            read_optional(&state, name, uid, limit)
                .map_err(|_| REFUSE)?
                .ok_or(REFUSE)?,
        );
    }
    let prior = ClosureRecord::decode(&members[0].0).map_err(|_| REFUSE)?;
    let handoff = SuccessorHandoff::decode(&members[1].0).map_err(|_| REFUSE)?;
    let intent = DecisionRecord::decode(&members[2].0).map_err(|_| REFUSE)?;
    if !handoff.matches_verified_plan(
        &prior,
        &intent,
        generation,
        desired.as_ref().map(|v| v.as_slice()),
        [
            stage.old_store(),
            stage.old_template(),
            new_store,
            new_template,
        ],
    ) {
        return Err(REFUSE);
    }
    for (index, name) in MEMBERS.into_iter().enumerate() {
        let expected = stage_bytes(&stage, index < 2, index % 2);
        let member = read_optional(
            &stage_dir,
            name,
            uid,
            if index % 2 == 0 {
                MAX_PRIVATE_STORE_BYTES
            } else {
                MAX_TEMPLATE_BYTES
            },
        )
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
        if member.0.as_slice() != expected {
            return Err(REFUSE);
        }
        members.push(member);
    }
    let ready = read_optional(&stage_dir, READY_MEMBER, uid, READY_BYTES)
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
    if !intent.matches_stage_ready(&ready.0) {
        return Err(REFUSE);
    }
    members.push(ready);
    let terminal = read_optional(&state, TERMINAL, uid, RECORD_BYTES).map_err(|_| REFUSE)?;
    let journal = inspect_decision_journal(paths, uid, lock).map_err(|_| REFUSE)?;
    if journal.intent().encode() != intent.encode()
        || journal.active().encode().as_slice()
            != terminal
                .as_ref()
                .map_or(members[2].0.as_slice(), |v| v.0.as_slice())
    {
        return Err(REFUSE);
    }
    if read_marker_existing(paths, uid).ok() != Some(marker)
        || read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)? != desired
    {
        return Err(REFUSE);
    }
    Ok(Evidence {
        directories: [
            state.metadata().map_err(|_| REFUSE)?,
            config_dir.metadata().map_err(|_| REFUSE)?,
            stage_dir.metadata().map_err(|_| REFUSE)?,
        ],
        members,
        terminal,
        intent,
    })
}

impl Evidence {
    fn accept(&mut self, other: Self) -> bool {
        if self.members.len() != other.members.len()
            || !self
                .directories
                .iter()
                .zip(&other.directories)
                .all(|(a, b)| same_directory(a, b))
            || !self
                .members
                .iter()
                .zip(&other.members)
                .all(|(a, b)| a.0 == b.0 && same_member(&a.1, &b.1))
        {
            return false;
        }
        match (&self.terminal, &other.terminal) {
            (Some(a), Some(b)) if a.0 == b.0 && same_member(&a.1, &b.1) => (),
            (None, _) => (),
            _ => return false,
        }
        // A newly published terminal is valid only as the exact journal's
        // transaction; Bound additionally requires the expected execution phase.
        self.terminal = other.terminal;
        true
    }
}

/// This is not a product entry point. Fresh authenticated input remains
/// required even for rollback of an interrupted successor.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn execute_successor(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        false,
        gate,
        |_| true,
        |_| true,
    )
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn recover_successor(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        true,
        gate,
        |_| true,
        |_| true,
    )
}

#[allow(clippy::too_many_arguments)]
fn run(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    recovery: bool,
    mut gate: impl FnMut() -> bool,
    hook: impl FnMut(EffectStep) -> bool,
    mut sync_hook: impl FnMut(usize) -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    if !gate() {
        return Err(ExecutionError::Admission);
    }
    if !recovery {
        recover_successor_preparation(
            config,
            paths,
            uid,
            generation,
            lock,
            backup,
            PreparationPhase::StageWithIntent,
            &mut gate,
        )
        .map_err(|_| REFUSE)?;
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let mut evidence = observe(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    let intent = DecisionRecord::decode(&evidence.intent.encode()).map_err(|_| REFUSE)?;
    if !gate()
        || !evidence.accept(observe(
            config,
            paths,
            uid,
            generation,
            lock,
            &off,
            backup.template(),
        )?)
    {
        return Err(REFUSE);
    }
    // Re-establish durability of all immutable evidence before any live effect.
    // Recovery deliberately permits old/mixed/new live bytes and matching slots;
    // they are classified independently by the existing transaction engine.
    // A live rename may not have been directory-synced before the crash. No
    // pre-rollback durability claim is needed: durable staged OLD is the source,
    // and both restored live inodes + config directory are synced before Abort.
    // A second rollback crash therefore still has Intent and remains fenced.
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let stage_dir = open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
        .map_err(|_| REFUSE)?;
    let entries: Vec<(&File, &str)> = [CLOSURE_MEMBER, SUCCESSOR_MEMBER, INTENT]
        .into_iter()
        .map(|n| (&state, n))
        .chain(
            MEMBERS
                .into_iter()
                .chain([READY_MEMBER])
                .map(|n| (&stage_dir, n)),
        )
        .collect();
    if entries.len() != evidence.members.len() {
        return Err(REFUSE);
    }
    for (index, (directory, name)) in entries.iter().enumerate() {
        if !gate()
            || !evidence.accept(observe(
                config,
                paths,
                uid,
                generation,
                lock,
                &off,
                backup.template(),
            )?)
        {
            return Err(REFUSE);
        }
        let file = File::from(
            openat(
                *directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        if !same_member(
            &file.metadata().map_err(|_| REFUSE)?,
            &evidence.members[index].1,
        ) {
            return Err(REFUSE);
        }
        if !sync_hook(index) {
            return Err(ExecutionError::Ambiguous);
        }
        file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
    }
    stage_dir
        .sync_all()
        .and_then(|_| state.sync_all())
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !sync_hook(entries.len()) {
        return Err(ExecutionError::Ambiguous);
    }
    let mut guarded = || {
        gate()
            && observe(
                config,
                paths,
                uid,
                generation,
                lock,
                &off,
                backup.template(),
            )
            .is_ok_and(|next| evidence.accept(next))
    };
    if !guarded() {
        return Err(REFUSE);
    }
    if recovery {
        return recover_with_hook(config, paths, uid, generation, lock, (guarded, true), hook);
    }
    let stage = read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let desired = read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?;
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
            .map_err(|_| REFUSE)?
            .metadata()
            .map_err(|_| REFUSE)?,
        gate: guarded,
    };
    bound.check(DecisionPhase::Intent)?;
    if !matches!(
        bound.classify()?,
        LivePairClass::Old | LivePairClass::Identical
    ) {
        return Err(ExecutionError::Admission);
    }
    // The pinned eight-member durability pass above already synced intent;
    // terminal-oriented journal resync is only applicable after a decision.
    finish_execution(&mut bound, &stage, &mut { hook })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_cleanup_candidate::{
        finalize_fenced_restore, publish_completion_record, retire_fixed_restore_artifacts,
    };
    use crate::restore_retirement_candidate::publish_retirement_receipt;
    use crate::restore_staging_candidate::stage_private_pair;
    use crate::restore_successor_coexistence_candidate::preparation::prepare_successor;
    use crate::restore_successor_publication_candidate::{
        publish_successor_handoff,
        tests::{Fixture, OLD, backup},
    };
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command, sync::OnceLock};

    fn second() -> &'static OpenedBackup {
        static BACKUP: OnceLock<OpenedBackup> = OnceLock::new();
        BACKUP.get_or_init(|| {
            let mut store: serde_json::Value = serde_json::from_slice(backup().store()).unwrap();
            store["onboardingComplete"] = true.into();
            let template = omavless_domain::routing::template_with_mode(
                std::str::from_utf8(backup().template()).unwrap(),
                "global",
            )
            .unwrap();
            let encrypted = omavless_domain::private_backup::seal(
                &serde_json::to_vec(&store).unwrap(),
                template.as_bytes(),
                b"synthetic second restore",
            )
            .unwrap();
            omavless_domain::private_backup::open(&encrypted, b"synthetic second restore").unwrap()
        })
    }
    fn prepared() -> (Fixture, MigrationLock) {
        let f = Fixture::new();
        let lock = f.lock();
        // Replace fixture's synthetic closure with the real complete first
        // transaction, including retirement and last pending-receipt unlink.
        fs::remove_file(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        stage_private_pair(
            &f.paths.state_directory,
            f.uid,
            OLD[0],
            OLD[1],
            &backup().restore_store_off().unwrap(),
            backup().template(),
        )
        .unwrap();
        assert_eq!(
            execute_staged_pair(&f.config, &f.paths, f.uid, 2, &lock, [31; 16], || true),
            Ok(PendingOutcome::Committed)
        );
        publish_retirement_receipt(&f.config, &f.paths, f.uid, 2, &lock, || true).unwrap();
        retire_fixed_restore_artifacts(&f.config, &f.paths, f.uid, 2, &lock, || true).unwrap();
        publish_completion_record(&f.config, &f.paths, f.uid, 2, &lock, || true).unwrap();
        finalize_fenced_restore(&f.config, &f.paths, f.uid, 2, &lock, || true).unwrap();
        publish_successor_handoff(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            second(),
            [32; 16],
            || true,
        )
        .unwrap();
        prepare_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        (f, lock)
    }
    fn drive(
        f: &Fixture,
        lock: &MigrationLock,
        recovery: bool,
        hook: impl FnMut(EffectStep) -> bool,
    ) -> Result<PendingOutcome, ExecutionError> {
        run(
            &f.config,
            &f.paths,
            f.uid,
            2,
            lock,
            second(),
            recovery,
            || true,
            hook,
            |_| true,
        )
    }
    fn pair(f: &Fixture, new: bool) {
        let b = if new { second() } else { backup() };
        assert_eq!(
            fs::read(f.config.join(LIVE[0])).unwrap(),
            b.restore_store_off().unwrap().as_slice()
        );
        assert_eq!(fs::read(f.config.join(LIVE[1])).unwrap(), b.template());
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
        assert!(f.paths.state_directory.join(CLOSURE_MEMBER).exists());
        assert!(f.paths.state_directory.join(SUCCESSOR_MEMBER).exists());
    }
    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn actual_first_restore_closure_then_successor_commit_keeps_both_fences() {
        let (f, lock) = prepared();
        let intent = f.paths.state_directory.join(INTENT);
        let inode = fs::metadata(&intent).unwrap().ino();
        let closure = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        assert_eq!(
            drive(&f, &lock, false, |_| true),
            Ok(PendingOutcome::Committed)
        );
        assert_eq!(fs::metadata(intent).unwrap().ino(), inode);
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            closure
        );
        pair(&f, true);
        assert_eq!(
            drive(&f, &lock, true, |_| true),
            Ok(PendingOutcome::Committed)
        );
        assert!(drive(&f, &lock, false, |_| true).is_err());
    }
    #[test]
    fn every_effect_interruption_rolls_back_undecided_or_verifies_terminal() {
        for stopped in 0..7 {
            let (f, lock) = prepared();
            let mut index = 0;
            assert!(
                drive(&f, &lock, false, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                })
                .is_err()
            );
            assert_eq!(index, stopped + 1);
            let expected = if stopped == 6 {
                PendingOutcome::Committed
            } else {
                PendingOutcome::Aborted
            };
            assert_eq!(drive(&f, &lock, true, |_| true), Ok(expected));
            pair(&f, stopped == 6);
        }
    }
    #[test]
    fn terminal_live_mismatch_never_rolls_back_or_reports_success() {
        for committed in [false, true] {
            for mixed in [false, true] {
                let (f, lock) = prepared();
                if committed {
                    drive(&f, &lock, false, |_| true).unwrap();
                } else {
                    drive(&f, &lock, true, |_| true).unwrap();
                }
                let foreign = if committed { backup() } else { second() };
                write(
                    &f.config.join(LIVE[0]),
                    &foreign.restore_store_off().unwrap(),
                );
                if !mixed {
                    write(&f.config.join(LIVE[1]), foreign.template());
                }
                let before = [
                    fs::read(f.config.join(LIVE[0])).unwrap(),
                    fs::read(f.config.join(LIVE[1])).unwrap(),
                ];
                assert!(drive(&f, &lock, true, |_| true).is_err());
                assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), before[0]);
                assert_eq!(fs::read(f.config.join(LIVE[1])).unwrap(), before[1]);
            }
        }
    }
    #[test]
    fn same_byte_fence_substitution_at_every_effect_refuses_further_progress() {
        for stopped in 0..7 {
            for name in [CLOSURE_MEMBER, SUCCESSOR_MEMBER, INTENT]
                .into_iter()
                .chain(MEMBERS)
                .chain([READY_MEMBER])
            {
                let (f, lock) = prepared();
                let mut index = 0;
                assert!(
                    drive(&f, &lock, false, |_| {
                        if index == stopped {
                            let parent = if MEMBERS.contains(&name) || name == READY_MEMBER {
                                f.paths.state_directory.join(PENDING_DIRECTORY)
                            } else {
                                f.paths.state_directory.clone()
                            };
                            let p = parent.join(name);
                            let raw = fs::read(&p).unwrap();
                            fs::rename(&p, f.root.join("substituted")).unwrap();
                            write(&p, &raw);
                        }
                        index += 1;
                        true
                    })
                    .is_err()
                );
                assert_eq!(index, stopped + 1);
            }
        }
    }
    #[test]
    fn mixed_restart_resync_interruptions_never_start_rollback_early() {
        for stopped in 0..9 {
            let (f, lock) = prepared();
            assert!(
                drive(&f, &lock, false, |step| step
                    != EffectStep::RenameApplied(0))
                .is_err()
            );
            let before = [
                fs::read(f.config.join(LIVE[0])).unwrap(),
                fs::read(f.config.join(LIVE[1])).unwrap(),
            ];
            assert_eq!(
                run(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    second(),
                    true,
                    || true,
                    |_| panic!("effect before complete durability"),
                    |point| point != stopped
                ),
                Err(ExecutionError::Ambiguous)
            );
            assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), before[0]);
            assert_eq!(fs::read(f.config.join(LIVE[1])).unwrap(), before[1]);
            assert_eq!(
                drive(&f, &lock, true, |_| true),
                Ok(PendingOutcome::Aborted)
            );
            pair(&f, false);
        }
    }
    #[test]
    fn actual_successor_crashes_reopen_with_authenticated_recovery() {
        for stopped in 0..7 {
            let (f, lock) = prepared();
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_executor_candidate::successor::tests::successor_crash_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_SUCCESSOR_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_SUCCESSOR_POINT", stopped.to_string())
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(9));
            if stopped == 1 {
                let output = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "restore_executor_candidate::successor::tests::successor_crash_worker",
                    ])
                    .env("OMAVLESS_SYNTHETIC_SUCCESSOR_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_SUCCESSOR_POINT", "1")
                    .env("OMAVLESS_SYNTHETIC_SUCCESSOR_ROLLBACK", "1")
                    .output()
                    .unwrap();
                assert_eq!(output.status.signal(), Some(9));
                assert!(!f.paths.state_directory.join(TERMINAL).exists());
            }
            let lock = f.lock();
            assert_eq!(
                drive(&f, &lock, true, |_| true),
                Ok(if stopped == 6 {
                    PendingOutcome::Committed
                } else {
                    PendingOutcome::Aborted
                })
            );
            pair(&f, stopped == 6);
        }
    }
    #[test]
    #[ignore = "internal synthetic successor executor crash worker"]
    fn successor_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_SUCCESSOR_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_SUCCESSOR_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        if std::env::var_os("OMAVLESS_SYNTHETIC_SUCCESSOR_RESYNC").is_some() {
            let _ = run(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                true,
                || true,
                |_| true,
                |point| {
                    if point == selected {
                        nix::sys::signal::kill(
                            nix::unistd::getpid(),
                            nix::sys::signal::Signal::SIGKILL,
                        )
                        .unwrap();
                    }
                    true
                },
            );
            panic!("expected resync worker loss");
        }
        let mut index = 0;
        let recovery = std::env::var_os("OMAVLESS_SYNTHETIC_SUCCESSOR_ROLLBACK").is_some();
        let _ = drive(&f, &lock, recovery, |_| {
            if index == selected {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            index += 1;
            true
        });
        panic!("expected worker loss");
    }

    #[test]
    fn actual_mixed_recovery_resync_crashes_never_advance_live_pair() {
        for stopped in [0, 7, 8] {
            let (f, lock) = prepared();
            assert!(
                drive(&f, &lock, false, |step| step
                    != EffectStep::RenameApplied(0))
                .is_err()
            );
            let before = [
                fs::read(f.config.join(LIVE[0])).unwrap(),
                fs::read(f.config.join(LIVE[1])).unwrap(),
            ];
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_executor_candidate::successor::tests::successor_crash_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_SUCCESSOR_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_SUCCESSOR_POINT", stopped.to_string())
                .env("OMAVLESS_SYNTHETIC_SUCCESSOR_RESYNC", "1")
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(9));
            assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), before[0]);
            assert_eq!(fs::read(f.config.join(LIVE[1])).unwrap(), before[1]);
            let lock = f.lock();
            assert_eq!(
                drive(&f, &lock, true, |_| true),
                Ok(PendingOutcome::Aborted)
            );
            pair(&f, false);
        }
    }

    #[test]
    fn wrong_archive_foreign_slot_late_gate_and_terminal_swap_refuse() {
        for target in ["archive", "slot", "gate", "terminal", "stage-dir"] {
            let (f, lock) = prepared();
            match target {
                "archive" => {
                    assert!(
                        run(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            backup(),
                            true,
                            || true,
                            |_| true,
                            |_| true
                        )
                        .is_err()
                    );
                    pair(&f, false);
                }
                "slot" => {
                    write(&f.config.join(OLD_SLOT[0]), b"foreign");
                    assert!(drive(&f, &lock, true, |_| true).is_err());
                    pair(&f, false);
                }
                "gate" => {
                    let allowed = std::cell::Cell::new(true);
                    assert!(
                        run(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            second(),
                            false,
                            || allowed.get(),
                            |_| {
                                allowed.set(false);
                                true
                            },
                            |_| true
                        )
                        .is_err()
                    );
                    pair(&f, false);
                }
                "terminal" => {
                    assert!(
                        drive(&f, &lock, false, |step| {
                            if step == EffectStep::Terminal {
                                let path = f.paths.state_directory.join(TERMINAL);
                                let raw = fs::read(&path).unwrap();
                                fs::rename(&path, f.root.join("old-terminal")).unwrap();
                                write(&path, &raw);
                            }
                            true
                        })
                        .is_err()
                    );
                }
                "stage-dir" => {
                    assert!(
                        drive(&f, &lock, false, |step| {
                            if step == EffectStep::Linked(0) {
                                let path = f.paths.state_directory.join(PENDING_DIRECTORY);
                                let moved = f.root.join("old-stage");
                                fs::rename(&path, &moved).unwrap();
                                fs::create_dir(&path).unwrap();
                                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                                    .unwrap();
                                for name in MEMBERS.into_iter().chain([READY_MEMBER]) {
                                    write(&path.join(name), &fs::read(moved.join(name)).unwrap());
                                }
                            }
                            true
                        })
                        .is_err()
                    );
                    pair(&f, false);
                }
                _ => unreachable!(),
            }
        }
    }
}
