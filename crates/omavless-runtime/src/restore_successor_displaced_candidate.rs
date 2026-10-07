// SPDX-License-Identifier: MIT
//! Inactive retirement of the displaced predecessor only. Canonical closure,
//! handoff and receipt survive; this never grants normal-owner admission.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetirementResult {
    DisplacedRetiredStillFenced,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Retire,
    ResyncRetired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    SourceDirectorySynced(usize),
    Unlinked,
    DirectorySynced,
    Reopened,
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn retire_displaced_closure(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<RetirementResult, ExecutionError> {
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
pub(crate) fn resync_displaced_retirement(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<RetirementResult, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        Operation::ResyncRetired,
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
) -> Result<RetirementResult, ExecutionError> {
    if !gate() {
        return Err(ExecutionError::Admission);
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let snapshot = observe_rotation(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    let expected = match operation {
        Operation::Retire => RotationPhase::Exchanged,
        Operation::ResyncRetired => RotationPhase::DisplacedRetired,
    };
    if snapshot.phase != expected
        || snapshot.prefix != Step::Done
        || snapshot.members.len() != publication::locations().len()
        || snapshot.directories.len() != 2
        || [0, 2, 3, 4, 5]
            .iter()
            .any(|&i| snapshot.members[i].is_none())
        || snapshot.members[1].is_some() != (operation == Operation::Retire)
        || snapshot.members[6..].iter().any(Option::is_some)
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
    context.resync(&mut gate, &mut |point| match point {
        super::Checkpoint::SourceSynced(index) => hook(Checkpoint::SourceSynced(index)),
        super::Checkpoint::SourceDirectorySynced(index) => {
            hook(Checkpoint::SourceDirectorySynced(index))
        }
        _ => false,
    })?;
    if operation == Operation::ResyncRetired {
        context.check(&mut gate)?;
        return Ok(RetirementResult::DisplacedRetiredStillFenced);
    }
    let _pinned = context.open_source(1)?;
    context.check(&mut gate)?;
    nix::unistd::unlinkat(
        &context.directories[0],
        Path::new(NEXT_CLOSURE_MEMBER),
        nix::unistd::UnlinkatFlags::NoRemoveDir,
    )
    .map_err(|_| ExecutionError::Ambiguous)?;
    // No repair follows an error: a fresh authenticated phase observation is
    // required, including re-sync after process death before directory fsync.
    context.snapshot.members[1] = None;
    context.snapshot.phase = RotationPhase::DisplacedRetired;
    if !hook(Checkpoint::Unlinked) {
        return Err(ExecutionError::Ambiguous);
    }
    context
        .check(&mut gate)
        .map_err(|_| ExecutionError::Ambiguous)?;
    context.directories[0]
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    if !hook(Checkpoint::DirectorySynced) {
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
    Ok(RetirementResult::DisplacedRetiredStillFenced)
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::super::super::tests::{pair, second};
    use super::super::super::tests::setup;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    fn ready(commit: bool) -> (Fixture, MigrationLock) {
        let (f, lock, _, _) = setup(commit);
        publication::publish_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        cleanup::cleanup_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        exchange_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        (f, lock)
    }
    fn drive(
        f: &Fixture,
        lock: &MigrationLock,
        operation: Operation,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<RetirementResult, ExecutionError> {
        run(
            &f.config,
            &f.paths,
            f.uid,
            2,
            lock,
            second(),
            operation,
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
        [0, 2, 3, 4, 5]
            .iter()
            .map(|&i| {
                (
                    fs::read(path(f, i)).unwrap(),
                    fs::metadata(path(f, i)).unwrap(),
                )
            })
            .collect()
    }
    fn check_retained(f: &Fixture, before: &[(Vec<u8>, Metadata)]) {
        let after = retained(f);
        assert_eq!(before.len(), after.len());
        for ((a, am), (b, bm)) in before.iter().zip(after) {
            assert_eq!(*a, b);
            assert!(same_member(am, &bm));
        }
    }
    fn effect(p: Checkpoint) -> bool {
        matches!(
            p,
            Checkpoint::Unlinked | Checkpoint::DirectorySynced | Checkpoint::Reopened
        )
    }

    #[test]
    fn commit_and_abort_retire_only_displaced_and_repeated_resync_preserves_inodes() {
        for commit in [false, true] {
            let (f, lock) = ready(commit);
            let before = retained(&f);
            assert!(drive(&f, &lock, Operation::ResyncRetired, |_| true).is_err());
            drive(&f, &lock, Operation::Retire, |_| true).unwrap();
            assert!(!path(&f, 1).exists());
            assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
            for _ in 0..2 {
                drive(&f, &lock, Operation::ResyncRetired, |_| true).unwrap();
            }
            check_retained(&f, &before);
            pair(&f, commit);
        }
    }
    #[test]
    fn all_eleven_interruptions_and_seven_recovery_syncs_reopen_in_exact_phase() {
        for stopped in 0..11 {
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
            let op = if stopped < 8 {
                Operation::Retire
            } else {
                Operation::ResyncRetired
            };
            drive(&f, &lock, op, |_| true).unwrap();
            for point in 0..7 {
                let mut index = 0;
                assert!(
                    drive(&f, &lock, Operation::ResyncRetired, |_| {
                        let keep = index != point;
                        index += 1;
                        keep
                    })
                    .is_err()
                );
                assert_eq!(index, point + 1);
            }
            drive(&f, &lock, Operation::ResyncRetired, |_| true).unwrap();
            check_retained(&f, &before);
        }
    }
    #[test]
    fn every_post_unlink_point_refuses_member_substitution_next_reappearance_and_late_gate() {
        for stopped in 0..3 {
            for change in 0..9 {
                let (f, lock) = ready(true);
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
                                    0..=4 => {
                                        let p = path(&f, [0, 2, 3, 4, 5][change]);
                                        let bytes = fs::read(&p).unwrap();
                                        fs::rename(&p, f.root.join("saved-member")).unwrap();
                                        fs::write(&p, bytes).unwrap();
                                        fs::set_permissions(&p, fs::Permissions::from_mode(0o600))
                                            .unwrap();
                                    }
                                    5 => {
                                        fs::write(path(&f, 1), b"unexpected").unwrap();
                                    }
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
                                            let p = dir.join(e.file_name());
                                            fs::write(&p, fs::read(e.path()).unwrap()).unwrap();
                                            fs::set_permissions(
                                                p,
                                                fs::Permissions::from_mode(0o600),
                                            )
                                            .unwrap();
                                        }
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
                assert!(path(&f, 0).exists() && path(&f, 2).exists() && path(&f, 3).exists());
            }
        }
    }
    #[test]
    fn early_phase_wrong_archive_and_missing_receipt_refuse_without_unlink() {
        let (f, lock, _, _) = setup(true);
        assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
        let (f, lock) = ready(true);
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
        fs::rename(path(&f, 3), f.root.join("saved-receipt")).unwrap();
        assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
        assert!(path(&f, 1).exists());
    }
    fn crash(f: &Fixture, point: usize, recovery: bool) {
        let output=Command::new(std::env::current_exe().unwrap())
            .args(["--ignored","--exact","restore_executor_candidate::successor::rotation::exchange::displaced::tests::displaced_crash_worker"])
            .env("OMAVLESS_SYNTHETIC_DISPLACED_ROOT",&f.root)
            .env("OMAVLESS_SYNTHETIC_DISPLACED_POINT",point.to_string())
            .env("OMAVLESS_SYNTHETIC_DISPLACED_RECOVERY",if recovery {"yes"} else {"no"})
            .output().unwrap();
        assert_eq!(output.status.signal(), Some(9));
    }
    #[test]
    fn actual_unlink_and_resync_process_deaths_preserve_all_remaining_fences() {
        for commit in [false, true] {
            for point in 0..3 {
                let (f, lock) = ready(commit);
                let before = retained(&f);
                drop(lock);
                crash(&f, point, false);
                assert!(!path(&f, 1).exists());
                crash(&f, if point == 0 { 0 } else { 6 }, true);
                let lock = f.lock();
                assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
                drive(&f, &lock, Operation::ResyncRetired, |_| true).unwrap();
                check_retained(&f, &before);
                pair(&f, commit);
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic displaced-retirement worker"]
    fn displaced_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_DISPLACED_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_DISPLACED_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let recovery = std::env::var("OMAVLESS_SYNTHETIC_DISPLACED_RECOVERY").unwrap() == "yes";
        let lock = f.lock();
        let mut index = 0;
        let _ = drive(
            &f,
            &lock,
            if recovery {
                Operation::ResyncRetired
            } else {
                Operation::Retire
            },
            |p| {
                if recovery || effect(p) {
                    if index == selected {
                        nix::sys::signal::kill(
                            nix::unistd::getpid(),
                            nix::sys::signal::Signal::SIGKILL,
                        )
                        .unwrap();
                    }
                    index += 1;
                }
                true
            },
        );
        panic!("selected process-death checkpoint not reached");
    }
}
