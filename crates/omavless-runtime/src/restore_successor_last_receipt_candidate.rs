// SPDX-License-Identifier: MIT
//! Inactive R1-last retirement. Permanent canonical C1 survives and still
//! fences startup; this is neither third-cycle nor normal-owner admission.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReceiptResult {
    ReceiptRetiredStillFenced,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Retire,
    ResyncCompleted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    DirectorySynced(usize),
    Pinned,
    Unlinked,
    StateSynced,
    Reopened,
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn retire_last_successor_receipt(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<ReceiptResult, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        Operation::Retire,
        gate,
        |_| true,
    )
}
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn resync_completed_successor(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<ReceiptResult, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        Operation::ResyncCompleted,
        gate,
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
    operation: Operation,
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(Checkpoint) -> bool,
) -> Result<ReceiptResult, ExecutionError> {
    if !gate() {
        return Err(ExecutionError::Admission);
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let snapshot = observe_final(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    let expected = if operation == Operation::Retire {
        FinalPhase::HandoffAbsentReceiptPresent
    } else {
        FinalPhase::HandoffAbsentReceiptAbsent
    };
    if snapshot.phase != expected
        || snapshot.evidence.prefix != Step::Done
        || snapshot.evidence.members.len() != publication::locations().len()
        || snapshot.evidence.directories.len() != 2
        || [0, 4, 5]
            .iter()
            .any(|&i| snapshot.evidence.members[i].is_none())
        || snapshot.evidence.members[1].is_some()
        || snapshot.evidence.members[2].is_some()
        || snapshot.evidence.members[3].is_some() != (operation == Operation::Retire)
        || snapshot.evidence.members[6..].iter().any(Option::is_some)
    {
        return Err(REFUSE);
    }
    let mut context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        off,
        template: backup.template(),
        snapshot,
        directories: [
            open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?,
            open_private_directory(config, uid).map_err(|_| REFUSE)?,
        ],
    };
    context.check(&mut gate)?;
    // A visible canonical completion after process loss is not treated as
    // durable until it, the live pair and both directories are synced/rechecked.
    context.resync(&mut gate, &mut |p| match p {
        super::Checkpoint::SourceSynced(i) => hook(Checkpoint::SourceSynced(i)),
        super::Checkpoint::DirectorySynced(i) => hook(Checkpoint::DirectorySynced(i)),
        _ => false,
    })?;
    if operation == Operation::ResyncCompleted {
        context.check(&mut gate)?;
        return Ok(ReceiptResult::ReceiptRetiredStillFenced);
    }
    let _pinned = context.open_source(3)?;
    if !hook(Checkpoint::Pinned) {
        return Err(ExecutionError::Ambiguous);
    }
    context.check(&mut gate)?;
    nix::unistd::unlinkat(
        &context.directories[0],
        Path::new(RECEIPT_MEMBER),
        nix::unistd::UnlinkatFlags::NoRemoveDir,
    )
    .map_err(|_| ExecutionError::Ambiguous)?;
    context.snapshot.evidence.members[3] = None;
    context.snapshot.phase = FinalPhase::HandoffAbsentReceiptAbsent;
    if !hook(Checkpoint::Unlinked) {
        return Err(ExecutionError::Ambiguous);
    }
    context
        .check(&mut gate)
        .map_err(|_| ExecutionError::Ambiguous)?;
    context.directories[0]
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !hook(Checkpoint::StateSynced) {
        return Err(ExecutionError::Ambiguous);
    }
    context
        .check(&mut gate)
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !hook(Checkpoint::Reopened) {
        return Err(ExecutionError::Ambiguous);
    }
    context
        .check(&mut gate)
        .map_err(|_| ExecutionError::Ambiguous)?;
    Ok(ReceiptResult::ReceiptRetiredStillFenced)
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::super::super::super::tests::second;
    use super::super::super::tests::ready as before_handoff;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};
    fn ready(commit: bool) -> (Fixture, MigrationLock) {
        let (f, lock) = before_handoff(commit);
        retire_successor_handoff(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        (f, lock)
    }
    fn drive(
        f: &Fixture,
        lock: &MigrationLock,
        op: Operation,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<ReceiptResult, ExecutionError> {
        run(
            &f.config,
            &f.paths,
            f.uid,
            2,
            lock,
            second(),
            op,
            || true,
            hook,
        )
    }
    fn path(f: &Fixture, index: usize) -> PathBuf {
        let (dir, name, _) = publication::locations()[index];
        if dir == 0 {
            f.paths.state_directory.join(name)
        } else {
            f.config.join(name)
        }
    }
    fn retained(f: &Fixture) -> Vec<(Vec<u8>, Metadata)> {
        [0, 4, 5]
            .iter()
            .map(|&i| {
                (
                    fs::read(path(f, i)).unwrap(),
                    fs::metadata(path(f, i)).unwrap(),
                )
            })
            .collect()
    }
    fn preserved(f: &Fixture, before: &[(Vec<u8>, Metadata)]) {
        let after = retained(f);
        assert_eq!(before.len(), after.len());
        for ((a, am), (b, bm)) in before.iter().zip(after) {
            assert_eq!(*a, b);
            assert!(same_member(am, &bm));
        }
        assert!(!path(f, 1).exists() && !path(f, 2).exists());
    }
    fn write(p: &Path, raw: &[u8]) {
        fs::write(p, raw).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn effect(p: Checkpoint) -> bool {
        matches!(
            p,
            Checkpoint::Unlinked | Checkpoint::StateSynced | Checkpoint::Reopened
        )
    }
    #[test]
    fn commit_abort_last_receipt_retirement_keeps_canonical_fence_and_never_retires_twice() {
        for commit in [false, true] {
            let (f, lock) = ready(commit);
            let before = retained(&f);
            assert!(drive(&f, &lock, Operation::ResyncCompleted, |_| true).is_err());
            drive(&f, &lock, Operation::Retire, |_| true).unwrap();
            assert!(!path(&f, 3).exists());
            assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
            for _ in 0..2 {
                drive(&f, &lock, Operation::ResyncCompleted, |_| true).unwrap();
            }
            preserved(&f, &before);
            assert_eq!(
                review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true),
                Ok(FinalPhase::HandoffAbsentReceiptAbsent)
            );
        }
    }
    #[test]
    fn all_ten_operation_and_five_completion_sync_interruptions_reopen_fenced() {
        for stopped in 0..10 {
            let (f, lock) = ready(true);
            let before = retained(&f);
            let mut index = 0;
            assert!(
                drive(&f, &lock, Operation::Retire, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                })
                .is_err()
            );
            assert_eq!(index, stopped + 1);
            drop(lock);
            let lock = f.lock();
            drive(
                &f,
                &lock,
                if stopped < 7 {
                    Operation::Retire
                } else {
                    Operation::ResyncCompleted
                },
                |_| true,
            )
            .unwrap();
            for point in 0..5 {
                let mut index = 0;
                assert!(
                    drive(&f, &lock, Operation::ResyncCompleted, |_| {
                        let keep = index != point;
                        index += 1;
                        keep
                    })
                    .is_err()
                );
                assert_eq!(index, point + 1);
            }
            drive(&f, &lock, Operation::ResyncCompleted, |_| true).unwrap();
            preserved(&f, &before);
        }
    }
    #[test]
    fn pinned_receipt_and_retained_source_substitution_prevent_unlink() {
        for selected in [0, 3, 4, 5] {
            let (f, lock) = ready(true);
            assert!(
                drive(&f, &lock, Operation::Retire, |point| {
                    if point == Checkpoint::Pinned {
                        let p = path(&f, selected);
                        let raw = fs::read(&p).unwrap();
                        fs::rename(&p, f.root.join("saved-source")).unwrap();
                        write(&p, &raw);
                    }
                    true
                })
                .is_err()
            );
            assert!(path(&f, 0).exists() && path(&f, 3).exists());
        }
    }
    #[test]
    fn every_post_unlink_boundary_refuses_substitution_reappearance_and_gate_or_directory_change() {
        for stopped in 0..3 {
            for change in 0..10 {
                let (f, lock) = ready(true);
                let r = fs::read(path(&f, 3)).unwrap();
                let enabled = std::cell::Cell::new(true);
                let mut index = 0;
                let result = run(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    second(),
                    Operation::Retire,
                    || enabled.get(),
                    |point| {
                        if effect(point) {
                            if index == stopped {
                                match change {
                                    0..=2 => {
                                        let p = path(&f, [0, 4, 5][change]);
                                        let raw = fs::read(&p).unwrap();
                                        fs::rename(&p, f.root.join("saved-source")).unwrap();
                                        write(&p, &raw);
                                    }
                                    3 => write(&path(&f, 3), &r),
                                    4 => write(&path(&f, 2), b"late-handoff"),
                                    5 => write(&path(&f, 1), b"late-next"),
                                    6 => enabled.set(false),
                                    7 | 8 => {
                                        let dir = if change == 7 {
                                            &f.config
                                        } else {
                                            &f.paths.state_directory
                                        };
                                        let saved = dir.with_extension("saved");
                                        fs::rename(dir, &saved).unwrap();
                                        fs::create_dir(dir).unwrap();
                                        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                                            .unwrap();
                                        for e in fs::read_dir(saved).unwrap() {
                                            let e = e.unwrap();
                                            write(
                                                &dir.join(e.file_name()),
                                                &fs::read(e.path()).unwrap(),
                                            );
                                        }
                                    }
                                    9 => {
                                        write(&f.paths.state_directory.join(INTENT), b"late-intent")
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            index += 1;
                        }
                        true
                    },
                );
                assert_eq!(result, Err(ExecutionError::Ambiguous));
                assert!(path(&f, 0).exists());
            }
        }
    }
    #[test]
    fn unavailable_or_wrong_new_archive_cannot_be_replaced_by_old_abort_output() {
        let encrypted = omavless_domain::private_backup::seal(
            second().store(),
            second().template(),
            b"synthetic final archive",
        )
        .unwrap();
        assert!(omavless_domain::private_backup::open(&encrypted, b"wrong passphrase").is_err());
        assert!(omavless_domain::private_backup::open(&[], b"missing archive").is_err());
        for commit in [false, true] {
            let (f, lock) = ready(commit);
            let before = retained(&f);
            assert!(
                run(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    Operation::Retire,
                    || true,
                    |_| true
                )
                .is_err()
            );
            assert!(path(&f, 3).exists());
            drive(&f, &lock, Operation::Retire, |_| true).unwrap();
            // On Abort, backup() supplies exactly the old live pair. It is
            // not the authenticated NEW archive needed for the stage digest.
            assert!(
                run(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    Operation::ResyncCompleted,
                    || true,
                    |_| true
                )
                .is_err()
            );
            preserved(&f, &before);
            fs::rename(path(&f, 0), f.root.join("saved-canonical")).unwrap();
            assert!(drive(&f, &lock, Operation::ResyncCompleted, |_| true).is_err());
        }
    }
    #[test]
    fn handoff_present_or_next_reappeared_blocks_last_receipt_retirement() {
        let (f, lock) = before_handoff(true);
        assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
        assert!(path(&f, 3).exists());
        retire_successor_handoff(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        write(&path(&f, 1), b"unexpected next");
        assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
        assert!(path(&f, 3).exists());
    }
    fn crash(f: &Fixture, point: usize, recovery: bool) {
        let output=Command::new(std::env::current_exe().unwrap())
            .args(["--ignored","--exact","restore_executor_candidate::successor::rotation::final_review::handoff_retirement::last_receipt::tests::last_receipt_crash_worker"])
            .env("OMAVLESS_SYNTHETIC_R_RETIRE_ROOT",&f.root)
            .env("OMAVLESS_SYNTHETIC_R_RETIRE_POINT",point.to_string())
            .env("OMAVLESS_SYNTHETIC_R_RETIRE_RECOVERY",if recovery {"yes"} else {"no"})
            .output().unwrap();
        assert_eq!(output.status.signal(), Some(9));
    }
    #[test]
    fn actual_pre_post_last_unlink_and_repeated_completion_sync_crashes_keep_c1() {
        for commit in [false, true] {
            for point in 6..10 {
                let (f, lock) = ready(commit);
                let before = retained(&f);
                drop(lock);
                crash(&f, point, false);
                assert_eq!(path(&f, 3).exists(), point == 6);
                if point == 6 {
                    let lock = f.lock();
                    drive(&f, &lock, Operation::Retire, |_| true).unwrap();
                }
                crash(&f, if point == 6 { 0 } else { 4 }, true);
                let lock = f.lock();
                drive(&f, &lock, Operation::ResyncCompleted, |_| true).unwrap();
                preserved(&f, &before);
                assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
                assert!(!path(&f, 3).exists());
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic last receipt worker"]
    fn last_receipt_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_R_RETIRE_ROOT").unwrap(),
        )));
        let point: usize = std::env::var("OMAVLESS_SYNTHETIC_R_RETIRE_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let recovery = std::env::var("OMAVLESS_SYNTHETIC_R_RETIRE_RECOVERY").unwrap() == "yes";
        let lock = f.lock();
        let mut index = 0;
        let _ = drive(
            &f,
            &lock,
            if recovery {
                Operation::ResyncCompleted
            } else {
                Operation::Retire
            },
            |_| {
                if index == point {
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
        panic!("selected process-death checkpoint not reached");
    }
}
