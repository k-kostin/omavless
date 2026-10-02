// SPDX-License-Identifier: MIT
//! Inactive terminal-only successor receipt publication. All prior fences,
//! stage and journals survive; no rotation, deletion or normal-owner admission.

use super::*;
use crate::restore_retirement_candidate::{
    RECEIPT_BYTES, ReceiptWriteStep, RetirementReceipt, write_receipt_with_hook,
};

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn publish_successor_receipt(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    publish(config, paths, uid, generation, lock, backup, gate, |_| true)
}

#[allow(clippy::too_many_arguments)]
fn publish(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(ReceiptWriteStep) -> bool,
) -> Result<PendingOutcome, ExecutionError> {
    if !gate() {
        return Err(ExecutionError::Admission);
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
    let terminal =
        DecisionRecord::decode(&evidence.terminal.as_ref().ok_or(REFUSE)?.0).map_err(|_| REFUSE)?;
    if terminal.phase() == DecisionPhase::Intent {
        return Err(REFUSE);
    }
    // Never call recovery here: disappearance of terminal must not turn
    // publication into authority to roll back live bytes.
    let outcome = verify_terminal_staged_pair(config, paths, uid, generation, lock, || {
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
    })?;
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let stage_dir = open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
        .map_err(|_| REFUSE)?;
    let entries: Vec<(&File, &str)> = [CLOSURE_MEMBER, SUCCESSOR_MEMBER, INTENT]
        .into_iter()
        .map(|name| (&state, name))
        .chain(
            MEMBERS
                .into_iter()
                .chain([READY_MEMBER])
                .map(|name| (&stage_dir, name)),
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
        file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
    }
    stage_dir
        .sync_all()
        .and_then(|_| state.sync_all())
        .map_err(|_| ExecutionError::Ambiguous)?;
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
    let stage = read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let receipt = RetirementReceipt::from_terminal(&terminal, &stage).map_err(|_| REFUSE)?;
    if !receipt.matches_live(config, uid).map_err(|_| REFUSE)? {
        return Err(REFUSE);
    }
    let bytes = receipt.encode();
    let mut guarded = || {
        gate()
            && observe_evidence(
                config,
                paths,
                uid,
                generation,
                lock,
                &off,
                backup.template(),
            )
            .is_ok_and(|next| evidence.accept(next))
            && receipt.matches_live(config, uid).unwrap_or(false)
    };
    if !guarded() {
        return Err(REFUSE);
    }
    // Only the low-level create-only writer is shared with single-cycle
    // retirement. Its returned inode is checked after the final callback.
    let written = write_receipt_with_hook(paths, uid, &bytes, |step| hook(step) && guarded())
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !guarded() {
        return Err(ExecutionError::Ambiguous);
    }
    let (raw, current) = read_optional(&state, RECEIPT_MEMBER, uid, RECEIPT_BYTES)
        .map_err(|_| ExecutionError::Ambiguous)?
        .ok_or(ExecutionError::Ambiguous)?;
    if raw.as_slice() != bytes || !same_member(&written, &current) {
        return Err(ExecutionError::Ambiguous);
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{drive, pair, prepared, second};
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};
    fn receipt(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(ReceiptWriteStep) -> bool,
    ) -> Result<PendingOutcome, ExecutionError> {
        publish(&f.config, &f.paths, f.uid, 2, lock, second(), || true, hook)
    }
    fn write(path: &Path, raw: &[u8]) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn terminal_only_receipt_preserves_every_prior_fence_for_commit_and_abort() {
        for commit in [false, true] {
            let (f, lock) = prepared();
            let outcome = drive(&f, &lock, !commit, |_| true).unwrap();
            let prior = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let handoff = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
            assert_eq!(receipt(&f, &lock, |_| true), Ok(outcome));
            assert!(receipt(&f, &lock, |_| true).is_err());
            assert_eq!(
                fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                prior
            );
            assert_eq!(
                fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap(),
                handoff
            );
            assert!(f.paths.state_directory.join(PENDING_DIRECTORY).exists());
            assert!(f.paths.state_directory.join(INTENT).exists());
            assert!(f.paths.state_directory.join(TERMINAL).exists());
            pair(&f, commit);
        }
    }
    #[test]
    fn undecided_wrong_archive_existing_and_unsafe_receipts_refuse() {
        let (f, lock) = prepared();
        assert!(receipt(&f, &lock, |_| true).is_err());
        pair(&f, false);
        assert!(!f.paths.state_directory.join(RECEIPT_MEMBER).exists());
        drive(&f, &lock, false, |_| true).unwrap();
        assert!(
            publish(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                backup(),
                || true,
                |_| true
            )
            .is_err()
        );
        std::os::unix::fs::symlink(
            f.root.join("absent-target"),
            f.paths.state_directory.join(RECEIPT_MEMBER),
        )
        .unwrap();
        assert!(receipt(&f, &lock, |_| true).is_err());
        assert!(!f.root.join("absent-target").exists());
        pair(&f, true);
    }
    #[test]
    fn each_publication_interruption_preserves_fences_and_refuses_retry() {
        for stopped in 0..5 {
            let (f, lock) = prepared();
            drive(&f, &lock, false, |_| true).unwrap();
            let mut index = 0;
            assert_eq!(
                receipt(&f, &lock, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                }),
                Err(ExecutionError::Ambiguous)
            );
            assert!(receipt(&f, &lock, |_| true).is_err());
            pair(&f, true);
        }
    }
    #[test]
    fn every_boundary_detects_foreign_same_byte_record_and_gate_drift() {
        for stopped in 0..5 {
            for target in [
                CLOSURE_MEMBER,
                SUCCESSOR_MEMBER,
                INTENT,
                TERMINAL,
                RECEIPT_MEMBER,
                "gate",
            ] {
                let (f, lock) = prepared();
                drive(&f, &lock, false, |_| true).unwrap();
                let allowed = std::cell::Cell::new(true);
                let mut index = 0;
                assert_eq!(
                    publish(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        || allowed.get(),
                        |_| {
                            if index == stopped {
                                if target == "gate" {
                                    allowed.set(false);
                                } else {
                                    let path = f.paths.state_directory.join(target);
                                    let raw = fs::read(&path).unwrap();
                                    fs::rename(&path, f.root.join("replaced")).unwrap();
                                    write(&path, &raw);
                                }
                            }
                            index += 1;
                            true
                        }
                    ),
                    Err(ExecutionError::Ambiguous)
                );
                pair(&f, true);
            }
        }
    }
    #[test]
    fn actual_receipt_crashes_reopen_without_losing_prior_fences() {
        for stopped in 0..5 {
            let (f, lock) = prepared();
            drive(&f, &lock, false, |_| true).unwrap();
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_executor_candidate::successor::receipt::tests::receipt_crash_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_RECEIPT_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_RECEIPT_POINT", stopped.to_string())
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(9));
            let lock = f.lock();
            assert!(receipt(&f, &lock, |_| true).is_err());
            pair(&f, true);
            assert!(f.paths.state_directory.join(INTENT).exists());
            assert!(f.paths.state_directory.join(TERMINAL).exists());
        }
    }
    #[test]
    #[ignore = "internal synthetic successor receipt worker"]
    fn receipt_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_RECEIPT_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_RECEIPT_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = receipt(&f, &lock, |_| {
            if index == selected {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            index += 1;
            true
        });
        panic!("expected worker termination");
    }
}
