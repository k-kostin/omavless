// SPDX-License-Identifier: MIT
//! Inactive H-only retirement. C1 and R1 remain; no last-receipt unlink,
//! third-cycle admission or normal-owner permission is provided here.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HandoffResult {
    HandoffRetiredStillFenced,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Retire,
    ResyncRetired,
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
struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    off: Zeroizing<Vec<u8>>,
    template: &'a [u8],
    snapshot: FinalSnapshot,
    directories: [File; 2],
}
impl Context<'_> {
    fn check(&self, gate: &mut impl FnMut() -> bool) -> Result<(), ExecutionError> {
        for _ in 0..2 {
            if !gate() {
                return Err(REFUSE);
            }
            let now = observe_final(
                self.config,
                self.paths,
                self.uid,
                self.generation,
                self.lock,
                &self.off,
                self.template,
            )?;
            if !self.snapshot.same(&now) {
                return Err(REFUSE);
            }
            if self.snapshot.evidence.directories.len() != self.directories.len() {
                return Err(REFUSE);
            }
            for (expected, file) in self
                .snapshot
                .evidence
                .directories
                .iter()
                .zip(&self.directories)
            {
                if !same_directory(expected, &file.metadata().map_err(|_| REFUSE)?) {
                    return Err(REFUSE);
                }
            }
        }
        Ok(())
    }
    fn open_source(&self, index: usize) -> Result<File, ExecutionError> {
        let (directory, name, _) = publication::locations()[index];
        let file = File::from(
            openat(
                &self.directories[directory],
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        if !same_member(
            &self.snapshot.evidence.members[index]
                .as_ref()
                .ok_or(REFUSE)?
                .1,
            &file.metadata().map_err(|_| REFUSE)?,
        ) {
            return Err(REFUSE);
        }
        Ok(file)
    }
    fn resync(
        &self,
        gate: &mut impl FnMut() -> bool,
        hook: &mut impl FnMut(Checkpoint) -> bool,
    ) -> Result<(), ExecutionError> {
        for index in [0, 2, 3, 4, 5] {
            if index == 2
                && self.snapshot.phase == FinalPhase::HandoffAbsentReceiptPresent
                && self.snapshot.evidence.members[2].is_none()
            {
                continue;
            }
            self.check(gate)?;
            self.open_source(index)?
                .sync_all()
                .map_err(|_| ExecutionError::Ambiguous)?;
            if !hook(Checkpoint::SourceSynced(index)) {
                return Err(ExecutionError::Ambiguous);
            }
            self.check(gate)?;
        }
        for index in [1, 0] {
            self.check(gate)?;
            self.directories[index]
                .sync_all()
                .map_err(|_| ExecutionError::Ambiguous)?;
            if !hook(Checkpoint::DirectorySynced(index)) {
                return Err(ExecutionError::Ambiguous);
            }
            self.check(gate)?;
        }
        Ok(())
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn retire_successor_handoff(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<HandoffResult, ExecutionError> {
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
pub(crate) fn resync_handoff_retirement(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<HandoffResult, ExecutionError> {
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
) -> Result<HandoffResult, ExecutionError> {
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
        FinalPhase::BeforeHandoffRetirement
    } else {
        FinalPhase::HandoffAbsentReceiptPresent
    };
    if snapshot.phase != expected
        || snapshot.evidence.prefix != Step::Done
        || snapshot.evidence.members.len() != publication::locations().len()
        || snapshot.evidence.directories.len() != 2
        || [0, 3, 4, 5]
            .iter()
            .any(|&i| snapshot.evidence.members[i].is_none())
        || snapshot.evidence.members[1].is_some()
        || snapshot.evidence.members[2].is_some() != (operation == Operation::Retire)
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
    context.resync(&mut gate, &mut hook)?;
    if operation == Operation::ResyncRetired {
        context.check(&mut gate)?;
        return Ok(HandoffResult::HandoffRetiredStillFenced);
    }
    let _pinned = context.open_source(2)?;
    if !hook(Checkpoint::Pinned) {
        return Err(ExecutionError::Ambiguous);
    }
    context.check(&mut gate)?;
    nix::unistd::unlinkat(
        &context.directories[0],
        Path::new(SUCCESSOR_MEMBER),
        nix::unistd::UnlinkatFlags::NoRemoveDir,
    )
    .map_err(|_| ExecutionError::Ambiguous)?;
    context.snapshot.evidence.members[2] = None;
    context.snapshot.phase = FinalPhase::HandoffAbsentReceiptPresent;
    // Output evidence survives loss of predecessor lineage. C1/R1/live pins
    // remain exact; no recreation, rollback or last-receipt retirement follows.
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
    Ok(HandoffResult::HandoffRetiredStillFenced)
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::super::super::tests::second;
    use super::super::tests::ready;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    fn drive(
        f: &Fixture,
        lock: &MigrationLock,
        op: Operation,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<HandoffResult, ExecutionError> {
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
        [0, 3, 4, 5]
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
        for ((raw, meta), (new, now)) in before.iter().zip(after) {
            assert_eq!(*raw, new);
            assert!(same_member(meta, &now));
        }
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
    fn commit_abort_retire_h_only_and_restart_never_unlinks_receipt() {
        for commit in [false, true] {
            let (f, lock) = ready(commit);
            let before = retained(&f);
            assert!(drive(&f, &lock, Operation::ResyncRetired, |_| true).is_err());
            drive(&f, &lock, Operation::Retire, |_| true).unwrap();
            assert!(!path(&f, 2).exists());
            assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
            for _ in 0..2 {
                drive(&f, &lock, Operation::ResyncRetired, |_| true).unwrap();
            }
            preserved(&f, &before);
            assert_eq!(
                review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true),
                Ok(FinalPhase::HandoffAbsentReceiptPresent)
            );
        }
    }
    #[test]
    fn every_operation_and_recovery_sync_interruption_requires_fresh_exact_phase() {
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
            drive(
                &f,
                &lock,
                if stopped < 8 {
                    Operation::Retire
                } else {
                    Operation::ResyncRetired
                },
                |_| true,
            )
            .unwrap();
            for point in 0..6 {
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
            preserved(&f, &before);
        }
    }
    #[test]
    fn pinned_handoff_or_other_same_byte_source_replacement_refuses_before_unlink() {
        for selected in [0, 2, 3, 4, 5] {
            let (f, lock) = ready(true);
            assert!(
                drive(&f, &lock, Operation::Retire, |p| {
                    if p == Checkpoint::Pinned {
                        let p = path(&f, selected);
                        let raw = fs::read(&p).unwrap();
                        fs::rename(&p, f.root.join("saved-source")).unwrap();
                        write(&p, &raw);
                    }
                    true
                })
                .is_err()
            );
            assert!(path(&f, 2).exists() && path(&f, 3).exists());
        }
    }
    #[test]
    fn each_post_unlink_boundary_refuses_remaining_source_substitution_and_new_artifacts() {
        for stopped in 0..3 {
            for change in 0..9 {
                let (f, lock) = ready(true);
                let h = fs::read(path(&f, 2)).unwrap();
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
                    |p| {
                        if effect(p) {
                            if index == stopped {
                                match change {
                                    0..=3 => {
                                        let p = path(&f, [0, 3, 4, 5][change]);
                                        let raw = fs::read(&p).unwrap();
                                        fs::rename(&p, f.root.join("saved-source")).unwrap();
                                        write(&p, &raw);
                                    }
                                    4 => write(&path(&f, 2), &h),
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
                                    _ => unreachable!(),
                                }
                            }
                            index += 1;
                        }
                        true
                    },
                );
                assert_eq!(result, Err(ExecutionError::Ambiguous));
                assert!(path(&f, 0).exists() && path(&f, 3).exists());
            }
        }
    }
    #[test]
    fn wrong_archive_and_missing_receipt_refuse_both_retirement_and_recovery() {
        for commit in [false, true] {
            let (f, lock) = ready(commit);
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
            assert!(path(&f, 2).exists());
            drive(&f, &lock, Operation::Retire, |_| true).unwrap();
            assert!(
                run(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    Operation::ResyncRetired,
                    || true,
                    |_| true
                )
                .is_err()
            );
            fs::rename(path(&f, 3), f.root.join("saved-receipt")).unwrap();
            assert!(drive(&f, &lock, Operation::ResyncRetired, |_| true).is_err());
            assert!(path(&f, 0).exists());
        }
    }
    fn crash(f: &Fixture, point: usize, recovery: bool) {
        let output=Command::new(std::env::current_exe().unwrap())
            .args(["--ignored","--exact","restore_executor_candidate::successor::rotation::final_review::handoff_retirement::tests::handoff_retirement_crash_worker"])
            .env("OMAVLESS_SYNTHETIC_H_RETIRE_ROOT",&f.root)
            .env("OMAVLESS_SYNTHETIC_H_RETIRE_POINT",point.to_string())
            .env("OMAVLESS_SYNTHETIC_H_RETIRE_RECOVERY",if recovery {"yes"} else {"no"})
            .output().unwrap();
        assert_eq!(output.status.signal(), Some(9));
    }
    #[test]
    fn actual_pre_post_unlink_and_directory_sync_deaths_reopen_without_last_receipt_loss() {
        for commit in [false, true] {
            for point in 7..11 {
                let (f, lock) = ready(commit);
                let before = retained(&f);
                drop(lock);
                crash(&f, point, false);
                assert_eq!(path(&f, 2).exists(), point == 7);
                if point == 7 {
                    let lock = f.lock();
                    drive(&f, &lock, Operation::Retire, |_| true).unwrap();
                }
                crash(&f, if point == 7 { 0 } else { 5 }, true);
                let lock = f.lock();
                drive(&f, &lock, Operation::ResyncRetired, |_| true).unwrap();
                preserved(&f, &before);
                assert!(drive(&f, &lock, Operation::Retire, |_| true).is_err());
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic handoff retirement worker"]
    fn handoff_retirement_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_H_RETIRE_ROOT").unwrap(),
        )));
        let point: usize = std::env::var("OMAVLESS_SYNTHETIC_H_RETIRE_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let recovery = std::env::var("OMAVLESS_SYNTHETIC_H_RETIRE_RECOVERY").unwrap() == "yes";
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
