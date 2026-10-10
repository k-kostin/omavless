// SPDX-License-Identifier: MIT
//! Inactive atomic closure-name exchange. Both closures, handoff and receipt
//! survive. No rollback, non-atomic fallback, unlink or normal-owner admission.
use super::*;

#[path = "restore_successor_displaced_candidate.rs"]
pub(crate) mod displaced;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExchangeResult {
    ExchangedStillFenced,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Exchange,
    ResyncExchanged,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    SourceDirectorySynced(usize),
    Exchanged,
    DirectorySynced,
    Reopened,
}

fn atomic_exchange(state: &File) -> Result<(), ExecutionError> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        nix::fcntl::renameat2(
            state,
            Path::new(CLOSURE_MEMBER),
            state,
            Path::new(NEXT_CLOSURE_MEMBER),
            nix::fcntl::RenameFlags::RENAME_EXCHANGE,
        )
        .map_err(|_| ExecutionError::Ambiguous)
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = state;
        Err(ExecutionError::Admission)
    }
}

// The successful exchange may change ctime. Only these two pinned descriptors
// cross that one owned transition; all later checks use full post-call metadata.
fn same_renamed_inode(before: &Metadata, after: &Metadata) -> bool {
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.mode() == after.mode()
        && before.nlink() == after.nlink()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    off: Zeroizing<Vec<u8>>,
    template: &'a [u8],
    snapshot: Snapshot,
    directories: [File; 2],
}
impl Context<'_> {
    fn check(&self, gate: &mut impl FnMut() -> bool) -> Result<(), ExecutionError> {
        for _ in 0..2 {
            if !gate() {
                return Err(REFUSE);
            }
            let now = observe_rotation(
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
            for (expected, file) in self.snapshot.directories.iter().zip(&self.directories) {
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
            &self.snapshot.members[index].as_ref().ok_or(REFUSE)?.1,
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
        for index in 0..6 {
            // Only the separately reviewed displaced-retirement phase permits
            // this one absent source. Exchange itself still requires all six.
            if index == 1
                && self.snapshot.phase == RotationPhase::DisplacedRetired
                && self.snapshot.members[index].is_none()
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
            if !hook(Checkpoint::SourceDirectorySynced(index)) {
                return Err(ExecutionError::Ambiguous);
            }
            self.check(gate)?;
        }
        Ok(())
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn exchange_next_closure(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<ExchangeResult, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        Operation::Exchange,
        gate,
        |_| true,
    )
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn resync_exchanged_closure(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<ExchangeResult, ExecutionError> {
    run(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        Operation::ResyncExchanged,
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
) -> Result<ExchangeResult, ExecutionError> {
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
    let phase = match operation {
        Operation::Exchange => RotationPhase::NextPublished,
        Operation::ResyncExchanged => RotationPhase::Exchanged,
    };
    if snapshot.phase != phase
        || snapshot.prefix != Step::Done
        || snapshot.members.len() != publication::locations().len()
        || snapshot.directories.len() != 2
        || snapshot.members[..6].iter().any(Option::is_none)
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
    context.resync(&mut gate, &mut hook)?;
    if operation == Operation::ResyncExchanged {
        context.check(&mut gate)?;
        return Ok(ExchangeResult::ExchangedStillFenced);
    }
    let pinned = [context.open_source(0)?, context.open_source(1)?];
    context.check(&mut gate)?;
    atomic_exchange(&context.directories[0])?;
    // Capture before any test/host callback. No rollback follows any error:
    // both valid names may already be exchanged and require fresh classification.
    let post = [
        pinned[0]
            .metadata()
            .map_err(|_| ExecutionError::Ambiguous)?,
        pinned[1]
            .metadata()
            .map_err(|_| ExecutionError::Ambiguous)?,
    ];
    for (index, metadata) in post.iter().enumerate() {
        if !same_renamed_inode(
            &context.snapshot.members[index]
                .as_ref()
                .ok_or(ExecutionError::Ambiguous)?
                .1,
            metadata,
        ) {
            return Err(ExecutionError::Ambiguous);
        }
    }
    context.snapshot.members.swap(0, 1);
    context.snapshot.members[0]
        .as_mut()
        .ok_or(ExecutionError::Ambiguous)?
        .1 = post[1].clone();
    context.snapshot.members[1]
        .as_mut()
        .ok_or(ExecutionError::Ambiguous)?
        .1 = post[0].clone();
    context.snapshot.phase = RotationPhase::Exchanged;
    if !hook(Checkpoint::Exchanged) {
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
    Ok(ExchangeResult::ExchangedStillFenced)
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::super::tests::{pair, second};
    use super::super::tests::setup;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    fn ready(commit: bool) -> (Fixture, MigrationLock, Vec<u8>, Vec<u8>) {
        let (f, lock, old, new) = setup(commit);
        publication::publish_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        cleanup::cleanup_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        (f, lock, old, new)
    }
    fn drive(
        f: &Fixture,
        lock: &MigrationLock,
        operation: Operation,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<ExchangeResult, ExecutionError> {
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
    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn exchanged(f: &Fixture, old: &[u8], new: &[u8], commit: bool) {
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            new
        );
        assert_eq!(
            fs::read(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap(),
            old
        );
        assert!(f.paths.state_directory.join(SUCCESSOR_MEMBER).exists());
        assert!(f.paths.state_directory.join(RECEIPT_MEMBER).exists());
        for name in [PENDING_DIRECTORY, INTENT, TERMINAL] {
            assert!(!f.paths.state_directory.join(name).exists());
        }
        pair(f, commit);
    }
    fn effect(step: Checkpoint) -> bool {
        matches!(
            step,
            Checkpoint::Exchanged | Checkpoint::DirectorySynced | Checkpoint::Reopened
        )
    }
    #[test]
    fn exact_inode_exchange_accepts_post_ctime_and_never_exchanges_twice() {
        for commit in [false, true] {
            let (f, lock, old, new) = ready(commit);
            let old_identity = fs::metadata(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let new_identity =
                fs::metadata(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap();
            assert_eq!(
                drive(&f, &lock, Operation::Exchange, |_| true),
                Ok(ExchangeResult::ExchangedStillFenced)
            );
            let canonical = fs::metadata(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let displaced =
                fs::metadata(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap();
            assert!(same_renamed_inode(&new_identity, &canonical));
            assert!(same_renamed_inode(&old_identity, &displaced));
            exchanged(&f, &old, &new, commit);
            assert!(drive(&f, &lock, Operation::Exchange, |_| true).is_err());
            assert_eq!(
                drive(&f, &lock, Operation::ResyncExchanged, |_| true),
                Ok(ExchangeResult::ExchangedStillFenced)
            );
            assert!(same_member(
                &canonical,
                &fs::metadata(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap()
            ));
            assert!(same_member(
                &displaced,
                &fs::metadata(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap()
            ));
            exchanged(&f, &old, &new, commit);
        }
    }
    #[test]
    fn all_sync_and_exchange_interruptions_require_the_exact_restart_phase() {
        for stopped in 0..11 {
            let (f, lock, old, new) = ready(true);
            let mut index = 0;
            assert!(
                drive(&f, &lock, Operation::Exchange, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                })
                .is_err()
            );
            assert_eq!(index, stopped + 1);
            drop(lock);
            let lock = f.lock();
            if stopped < 8 {
                assert_eq!(
                    fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                    old
                );
                assert!(drive(&f, &lock, Operation::ResyncExchanged, |_| true).is_err());
                assert_eq!(
                    drive(&f, &lock, Operation::Exchange, |_| true),
                    Ok(ExchangeResult::ExchangedStillFenced)
                );
            } else {
                assert!(drive(&f, &lock, Operation::Exchange, |_| true).is_err());
                assert_eq!(
                    drive(&f, &lock, Operation::ResyncExchanged, |_| true),
                    Ok(ExchangeResult::ExchangedStillFenced)
                );
            }
            exchanged(&f, &old, &new, true);
        }
    }
    #[test]
    fn interrupted_exchanged_resync_never_rolls_back_or_unlinks() {
        for stopped in 0..8 {
            let (f, lock, old, new) = ready(true);
            drive(&f, &lock, Operation::Exchange, |_| true).unwrap();
            let mut index = 0;
            assert!(
                drive(&f, &lock, Operation::ResyncExchanged, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                })
                .is_err()
            );
            exchanged(&f, &old, &new, true);
            assert_eq!(
                drive(&f, &lock, Operation::ResyncExchanged, |_| true),
                Ok(ExchangeResult::ExchangedStillFenced)
            );
        }
    }
    #[test]
    fn every_post_syscall_boundary_refuses_substitution_rewrite_gate_or_directory_change() {
        for stopped in 0..3 {
            for change in 0..12 {
                let (f, lock, old, new) = ready(true);
                let enabled = std::cell::Cell::new(true);
                let mut index = 0;
                assert_eq!(
                    run(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        Operation::Exchange,
                        || enabled.get(),
                        |step| {
                            if effect(step) {
                                if index == stopped {
                                    match change {
                                        0..=7 => {
                                            let selected =
                                                if change >= 6 { change - 6 } else { change };
                                            let (directory, name, _) =
                                                publication::locations()[selected];
                                            let path = if directory == 0 {
                                                f.paths.state_directory.join(name)
                                            } else {
                                                f.config.join(name)
                                            };
                                            let raw = fs::read(&path).unwrap();
                                            if change < 6 {
                                                fs::rename(&path, f.root.join("saved-source"))
                                                    .unwrap();
                                                write(&path, &raw);
                                            } else {
                                                // Same mode/bytes/mtime: only
                                                // ctime changes after capture.
                                                let before = fs::metadata(&path).unwrap();
                                                fs::set_permissions(
                                                    &path,
                                                    fs::Permissions::from_mode(0o600),
                                                )
                                                .unwrap();
                                                let after = fs::metadata(&path).unwrap();
                                                assert!(same_renamed_inode(&before, &after));
                                                assert!(!same_member(&before, &after));
                                            }
                                        }
                                        8 => enabled.set(false),
                                        9 => {
                                            let stage =
                                                f.paths.state_directory.join(PENDING_DIRECTORY);
                                            fs::create_dir(&stage).unwrap();
                                            fs::set_permissions(
                                                &stage,
                                                fs::Permissions::from_mode(0o700),
                                            )
                                            .unwrap();
                                            write(&stage.join("unknown"), b"bounded");
                                        }
                                        10 | 11 => {
                                            let directory = if change == 10 {
                                                &f.paths.state_directory
                                            } else {
                                                &f.config
                                            };
                                            let saved = directory.with_extension("saved");
                                            fs::rename(directory, &saved).unwrap();
                                            fs::create_dir(directory).unwrap();
                                            fs::set_permissions(
                                                directory,
                                                fs::Permissions::from_mode(0o700),
                                            )
                                            .unwrap();
                                            for entry in fs::read_dir(saved).unwrap() {
                                                let entry = entry.unwrap();
                                                assert!(entry.file_type().unwrap().is_file());
                                                write(
                                                    &directory.join(entry.file_name()),
                                                    &fs::read(entry.path()).unwrap(),
                                                );
                                            }
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                                index += 1;
                            }
                            true
                        }
                    ),
                    Err(ExecutionError::Ambiguous)
                );
                assert_eq!(
                    fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                    new
                );
                assert_eq!(
                    fs::read(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap(),
                    old
                );
                assert!(f.paths.state_directory.join(SUCCESSOR_MEMBER).exists());
                assert!(f.paths.state_directory.join(RECEIPT_MEMBER).exists());
            }
        }
    }
    #[test]
    fn incomplete_cleanup_wrong_archive_missing_next_and_syscall_error_do_not_exchange() {
        let (f, lock, old, _) = setup(true);
        publication::publish_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        assert!(drive(&f, &lock, Operation::Exchange, |_| true).is_err());
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            old
        );
        let (f, lock, old, _) = ready(true);
        assert!(
            run(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                backup(),
                Operation::Exchange,
                || true,
                |_| true
            )
            .is_err()
        );
        fs::rename(
            f.paths.state_directory.join(NEXT_CLOSURE_MEMBER),
            f.root.join("saved-next"),
        )
        .unwrap();
        assert!(drive(&f, &lock, Operation::Exchange, |_| true).is_err());
        let state = open_private_directory(&f.paths.state_directory, f.uid).unwrap();
        assert!(atomic_exchange(&state).is_err()); // Kernel error, not a rename fallback.
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            old
        );
        assert!(!f.paths.state_directory.join(NEXT_CLOSURE_MEMBER).exists());
    }
    fn crash(f: &Fixture, point: usize, recovery: bool) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "restore_executor_candidate::successor::rotation::exchange::tests::exchange_crash_worker"])
            .env("OMAVLESS_SYNTHETIC_EXCHANGE_ROOT", &f.root)
            .env("OMAVLESS_SYNTHETIC_EXCHANGE_POINT", point.to_string())
            .env("OMAVLESS_SYNTHETIC_EXCHANGE_RECOVERY", if recovery { "yes" } else { "no" })
            .output().unwrap();
        assert_eq!(output.status.signal(), Some(9));
    }
    #[test]
    fn actual_exchange_crashes_and_recovery_crashes_never_swap_back() {
        for commit in [false, true] {
            for stopped in 0..3 {
                let (f, lock, old, new) = ready(commit);
                drop(lock);
                crash(&f, stopped, false);
                exchanged(&f, &old, &new, commit);
                crash(&f, if stopped == 0 { 0 } else { 7 }, true);
                exchanged(&f, &old, &new, commit);
                let lock = f.lock();
                assert!(drive(&f, &lock, Operation::Exchange, |_| true).is_err());
                assert_eq!(
                    drive(&f, &lock, Operation::ResyncExchanged, |_| true),
                    Ok(ExchangeResult::ExchangedStillFenced)
                );
                exchanged(&f, &old, &new, commit);
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic closure-exchange worker"]
    fn exchange_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_EXCHANGE_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_EXCHANGE_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let recovery = std::env::var("OMAVLESS_SYNTHETIC_EXCHANGE_RECOVERY").unwrap() == "yes";
        let lock = f.lock();
        let mut index = 0;
        let _ = drive(
            &f,
            &lock,
            if recovery {
                Operation::ResyncExchanged
            } else {
                Operation::Exchange
            },
            |step| {
                if recovery || effect(step) {
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
        panic!("expected worker termination");
    }
}
