// SPDX-License-Identifier: MIT
//! Inactive fixed cleanup under both closures, handoff and terminal receipt.
//! No closure exchange, retained-record retirement or normal-owner authority.
use super::*;
use nix::unistd::{UnlinkatFlags, unlinkat};
use publication::locations;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CleanupResult {
    CleanedStillFenced,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Slot(usize),
    StageMember(usize),
    Ready,
    StageDirectory,
    Terminal,
    Intent,
}
impl Target {
    fn member(self) -> Option<usize> {
        match self {
            Self::Slot(index) => Some(8 + index),
            Self::StageMember(index) => Some(12 + index),
            Self::Ready => Some(16),
            Self::StageDirectory => None,
            Self::Terminal => Some(7),
            Self::Intent => Some(6),
        }
    }
    fn next_prefix(self) -> Step {
        match self {
            Self::Slot(_) => Step::StageMember(0),
            Self::StageMember(index) if index < 3 => Step::StageMember(index + 1),
            Self::StageMember(_) => Step::Ready,
            Self::Ready => Step::StageDirectory,
            Self::StageDirectory => Step::Terminal,
            Self::Terminal => Step::Intent,
            Self::Intent => Step::Done,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    SourceDirectorySynced(usize),
    Unlinked(Target),
    DirectorySynced(Target),
}

fn next_target(snapshot: &Snapshot) -> Option<Target> {
    // A fresh executor result may have any independently verified slot subset.
    // Absence is not attributed to this writer or used as recovery authority.
    // During this invocation exactly_removed allows only our selected absence.
    for index in 0..4 {
        if snapshot.members[8 + index].is_some() {
            return Some(Target::Slot(index));
        }
    }
    match snapshot.prefix {
        Step::StageMember(index) => Some(Target::StageMember(index)),
        Step::Ready => Some(Target::Ready),
        Step::StageDirectory => Some(Target::StageDirectory),
        Step::Terminal => Some(Target::Terminal),
        Step::Intent => Some(Target::Intent),
        Step::Done => None,
    }
}

fn exactly_removed(before: &Snapshot, after: &Snapshot, target: Target) -> bool {
    if before.phase != RotationPhase::NextPublished
        || after.phase != RotationPhase::NextPublished
        || before.desired != after.desired
        || after.prefix != target.next_prefix()
        || next_target(before) != Some(target)
        || before.members.len() != locations().len()
        || after.members.len() != locations().len()
        || after.directories.len()
            != before.directories.len() - usize::from(target == Target::StageDirectory)
        || !before
            .directories
            .iter()
            .zip(&after.directories)
            .all(|(a, b)| same_directory(a, b))
    {
        return false;
    }
    before
        .members
        .iter()
        .zip(&after.members)
        .enumerate()
        .all(|(index, (a, b))| {
            if target.member() == Some(index) {
                a.is_some() && b.is_none()
            } else {
                match (a, b) {
                    (Some(a), Some(b)) => a.0 == b.0 && same_member(&a.1, &b.1),
                    (None, None) => true,
                    _ => false,
                }
            }
        })
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
    directories: Vec<File>,
}
impl Context<'_> {
    fn fresh(&self, gate: &mut impl FnMut() -> bool) -> Result<Snapshot, ExecutionError> {
        if !gate() {
            return Err(REFUSE);
        }
        let first = observe_rotation(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
            &self.off,
            self.template,
        )?;
        if first.phase != RotationPhase::NextPublished
            || first.members.len() != locations().len()
            || !gate()
        {
            return Err(REFUSE);
        }
        let second = observe_rotation(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
            &self.off,
            self.template,
        )?;
        if !first.same(&second) {
            return Err(REFUSE);
        }
        Ok(second)
    }
    fn check_descriptors(&self) -> Result<(), ExecutionError> {
        if self.snapshot.directories.len() > self.directories.len() {
            return Err(REFUSE);
        }
        for (expected, file) in self.snapshot.directories.iter().zip(&self.directories) {
            if !same_directory(expected, &file.metadata().map_err(|_| REFUSE)?) {
                return Err(REFUSE);
            }
        }
        Ok(())
    }
    fn check(&self, gate: &mut impl FnMut() -> bool) -> Result<(), ExecutionError> {
        self.check_descriptors()?;
        if !self.snapshot.same(&self.fresh(gate)?) {
            return Err(REFUSE);
        }
        self.check_descriptors()
    }
    fn accept_removal(
        &mut self,
        target: Target,
        gate: &mut impl FnMut() -> bool,
    ) -> Result<(), ExecutionError> {
        let after = self.fresh(gate)?;
        if !exactly_removed(&self.snapshot, &after, target) {
            return Err(REFUSE);
        }
        self.snapshot = after;
        self.check_descriptors()
    }
    fn resync(
        &self,
        gate: &mut impl FnMut() -> bool,
        hook: &mut impl FnMut(Checkpoint) -> bool,
    ) -> Result<(), ExecutionError> {
        // Visible valid bytes after process loss are not durability evidence.
        // This runs again at every admitted prefix, before its first deletion.
        for (index, (directory, name, _)) in locations().into_iter().enumerate() {
            if let Some((_, expected)) = &self.snapshot.members[index] {
                self.check(gate)?;
                let file = File::from(
                    openat(
                        &self.directories[directory],
                        Path::new(name),
                        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| REFUSE)?,
                );
                if !same_member(expected, &file.metadata().map_err(|_| REFUSE)?) {
                    return Err(REFUSE);
                }
                file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
                if !hook(Checkpoint::SourceSynced(index)) {
                    return Err(ExecutionError::Ambiguous);
                }
                self.check(gate)?;
            }
        }
        for index in (0..self.snapshot.directories.len()).rev() {
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
pub(crate) fn cleanup_successor(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<CleanupResult, ExecutionError> {
    cleanup(config, paths, uid, generation, lock, backup, gate, |_| true)
}

#[allow(clippy::too_many_arguments)]
fn cleanup(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(Checkpoint) -> bool,
) -> Result<CleanupResult, ExecutionError> {
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
    if snapshot.phase != RotationPhase::NextPublished || snapshot.members.len() != locations().len()
    {
        return Err(REFUSE);
    }
    let mut directories = vec![
        open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?,
        open_private_directory(config, uid).map_err(|_| REFUSE)?,
    ];
    if snapshot.directories.len() == 3 {
        directories.push(
            open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
                .map_err(|_| REFUSE)?,
        );
    }
    if directories.len() != snapshot.directories.len() {
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
        directories,
    };
    context.check(&mut gate)?;
    context.resync(&mut gate, &mut hook)?;
    // Four slots and eight fixed stage/journal steps. Every iteration either
    // removes exactly one selected entry or returns; there is no repair loop.
    for _ in 0..=12 {
        context.check(&mut gate)?;
        let Some(target) = next_target(&context.snapshot) else {
            return Ok(CleanupResult::CleanedStillFenced);
        };
        let (directory, name, flags, expected) = if let Some(index) = target.member() {
            let (directory, name, _) = locations()[index];
            (
                directory,
                name,
                UnlinkatFlags::NoRemoveDir,
                &context.snapshot.members[index].as_ref().ok_or(REFUSE)?.1,
            )
        } else {
            (
                0,
                PENDING_DIRECTORY,
                UnlinkatFlags::RemoveDir,
                &context.snapshot.directories[2],
            )
        };
        let pinned = File::from(
            openat(
                &context.directories[directory],
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let metadata = pinned.metadata().map_err(|_| REFUSE)?;
        if if target == Target::StageDirectory {
            !same_directory(expected, &metadata)
        } else {
            !same_member(expected, &metadata)
        } {
            return Err(REFUSE);
        }
        context.check(&mut gate)?;
        unlinkat(&context.directories[directory], Path::new(name), flags)
            .map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::Unlinked(target)) {
            return Err(ExecutionError::Ambiguous);
        }
        context
            .accept_removal(target, &mut gate)
            .map_err(|_| ExecutionError::Ambiguous)?;
        context.directories[directory]
            .sync_all()
            .map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::DirectorySynced(target)) {
            return Err(ExecutionError::Ambiguous);
        }
        context
            .check(&mut gate)
            .map_err(|_| ExecutionError::Ambiguous)?;
    }
    Err(ExecutionError::Ambiguous)
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::{pair, second};
    use super::super::tests::setup;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    pub(super) fn ready(commit: bool, mask: usize) -> (Fixture, MigrationLock, Vec<u8>, Vec<u8>) {
        let (f, lock, old, new) = setup(commit);
        for index in 0..4 {
            if mask & (1 << index) != 0 {
                let source = if index < 2 { index + 2 } else { index - 2 };
                let bytes = fs::read(
                    f.paths
                        .state_directory
                        .join(PENDING_DIRECTORY)
                        .join(MEMBERS[source]),
                )
                .unwrap();
                write(&f.config.join(locations()[8 + index].1), &bytes);
            }
        }
        publication::publish_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        (f, lock, old, new)
    }
    fn run(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<CleanupResult, ExecutionError> {
        cleanup(&f.config, &f.paths, f.uid, 2, lock, second(), || true, hook)
    }
    fn fences(f: &Fixture, old: &[u8], new: &[u8], commit: bool) {
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            old
        );
        assert_eq!(
            fs::read(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap(),
            new
        );
        for name in [SUCCESSOR_MEMBER, RECEIPT_MEMBER] {
            assert!(f.paths.state_directory.join(name).exists());
        }
        pair(f, commit);
    }
    fn done(f: &Fixture, old: &[u8], new: &[u8], commit: bool) {
        fences(f, old, new, commit);
        for name in [INTENT, TERMINAL, PENDING_DIRECTORY] {
            assert!(!f.paths.state_directory.join(name).exists());
        }
        for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
            assert!(!f.config.join(name).exists());
        }
    }
    fn effect(step: Checkpoint) -> bool {
        matches!(
            step,
            Checkpoint::Unlinked(_) | Checkpoint::DirectorySynced(_)
        )
    }

    #[test]
    fn all_sixteen_matching_slot_subsets_clean_in_order_for_commit_and_abort() {
        for commit in [false, true] {
            for mask in 0..16 {
                let (f, lock, old, new) = ready(commit, mask);
                let retained = [
                    CLOSURE_MEMBER,
                    NEXT_CLOSURE_MEMBER,
                    SUCCESSOR_MEMBER,
                    RECEIPT_MEMBER,
                ];
                let identities: Vec<_> = retained
                    .iter()
                    .map(|name| fs::metadata(f.paths.state_directory.join(name)).unwrap())
                    .collect();
                let mut removed = Vec::new();
                assert_eq!(
                    run(&f, &lock, |step| {
                        if let Checkpoint::Unlinked(target) = step {
                            removed.push(target);
                        }
                        true
                    }),
                    Ok(CleanupResult::CleanedStillFenced)
                );
                let expected: Vec<Target> = (0..4)
                    .filter(|i| mask & (1 << i) != 0)
                    .map(Target::Slot)
                    .chain((0..4).map(Target::StageMember))
                    .chain([
                        Target::Ready,
                        Target::StageDirectory,
                        Target::Terminal,
                        Target::Intent,
                    ])
                    .collect();
                assert_eq!(removed, expected);
                done(&f, &old, &new, commit);
                for (name, expected) in retained.iter().zip(&identities) {
                    assert!(same_member(
                        expected,
                        &fs::metadata(f.paths.state_directory.join(name)).unwrap()
                    ));
                }
                assert_eq!(
                    review_rotation(&f.config, &f.paths, f.uid, 2, &lock, second(), || true),
                    Ok(RotationPhase::NextPublished)
                );
                assert_eq!(
                    run(&f, &lock, |_| true),
                    Ok(CleanupResult::CleanedStillFenced)
                );
            }
        }
    }

    #[test]
    fn all_resync_and_cleanup_interruptions_reopen_under_retained_fences() {
        for stopped in 0..44 {
            let (f, lock, old, new) = ready(true, 15);
            let mut index = 0;
            assert!(
                run(&f, &lock, |_| {
                    let keep = index != stopped;
                    index += 1;
                    keep
                })
                .is_err()
            );
            assert_eq!(index, stopped + 1);
            fences(&f, &old, &new, true);
            drop(lock);
            let lock = f.lock();
            assert_eq!(
                run(&f, &lock, |_| true),
                Ok(CleanupResult::CleanedStillFenced)
            );
            done(&f, &old, &new, true);
        }
    }

    #[test]
    fn wrong_archive_torn_next_missing_canonical_and_early_stage_holes_refuse() {
        for case in 0..6 {
            let (f, lock, old, new) = ready(true, 15);
            match case {
                0 => {
                    assert!(
                        cleanup(
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
                    fences(&f, &old, &new, true);
                    continue;
                }
                1 => write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), b"torn"),
                2 => {
                    fs::remove_file(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
                }
                3 => {
                    fs::remove_file(
                        f.paths
                            .state_directory
                            .join(PENDING_DIRECTORY)
                            .join(MEMBERS[1]),
                    )
                    .unwrap();
                }
                4 => {
                    fs::remove_file(
                        f.paths
                            .state_directory
                            .join(PENDING_DIRECTORY)
                            .join(MEMBERS[0]),
                    )
                    .unwrap();
                }
                5 => write(
                    &f.paths
                        .state_directory
                        .join(PENDING_DIRECTORY)
                        .join("unknown"),
                    b"bounded",
                ),
                _ => unreachable!(),
            }
            assert!(run(&f, &lock, |_| true).is_err());
            for name in [SUCCESSOR_MEMBER, RECEIPT_MEMBER, INTENT, TERMINAL] {
                assert!(f.paths.state_directory.join(name).exists());
            }
            for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
                assert!(f.config.join(name).exists());
            }
        }
    }

    #[test]
    fn foreign_slot_disappearance_or_same_byte_replacement_during_effect_poison() {
        for change in 0..3 {
            let (f, lock, old, new) = ready(true, 15);
            assert_eq!(
                run(&f, &lock, |step| {
                    if step == Checkpoint::Unlinked(Target::Slot(0)) {
                        let path = f.config.join(OLD_SLOT[1]);
                        let bytes = fs::read(&path).unwrap();
                        fs::rename(&path, f.root.join("saved-slot")).unwrap();
                        if change != 0 {
                            write(&path, if change == 1 { &bytes } else { b"foreign" });
                        }
                    }
                    true
                }),
                Err(ExecutionError::Ambiguous)
            );
            fences(&f, &old, &new, true);
            assert!(
                f.paths
                    .state_directory
                    .join(PENDING_DIRECTORY)
                    .join(MEMBERS[0])
                    .exists()
            );
        }
    }

    #[test]
    fn each_effect_refuses_retained_record_swap_gate_change_and_unknown_stage() {
        for stopped in 0..24 {
            for change in 0..8 {
                let (f, lock, old, new) = ready(true, 15);
                let enabled = std::cell::Cell::new(true);
                let mut index = 0;
                assert_eq!(
                    cleanup(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        || enabled.get(),
                        |step| {
                            if effect(step) {
                                if index == stopped {
                                    match change {
                                        0..=3 => {
                                            let name = [
                                                CLOSURE_MEMBER,
                                                NEXT_CLOSURE_MEMBER,
                                                SUCCESSOR_MEMBER,
                                                RECEIPT_MEMBER,
                                            ][change];
                                            let path = f.paths.state_directory.join(name);
                                            let raw = fs::read(&path).unwrap();
                                            fs::rename(&path, f.root.join("saved-record")).unwrap();
                                            write(&path, &raw);
                                        }
                                        4 => enabled.set(false),
                                        5 => {
                                            let stage =
                                                f.paths.state_directory.join(PENDING_DIRECTORY);
                                            if !stage.exists() {
                                                fs::create_dir(&stage).unwrap();
                                                fs::set_permissions(
                                                    &stage,
                                                    fs::Permissions::from_mode(0o700),
                                                )
                                                .unwrap();
                                            }
                                            write(&stage.join("unknown"), b"bounded");
                                        }
                                        6 | 7 => {
                                            let path = f.config.join(LIVE[change - 6]);
                                            let raw = fs::read(&path).unwrap();
                                            fs::rename(&path, f.root.join("saved-live")).unwrap();
                                            write(&path, &raw);
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
                assert_eq!(index, stopped + 1);
                fences(&f, &old, &new, true);
            }
        }
    }

    #[test]
    fn every_surviving_member_substitution_before_first_slot_sync_is_refused() {
        for selected in 0..17 {
            if selected == 8 {
                continue;
            } // This is the deliberately removed slot.
            let (f, lock, old, new) = ready(true, 15);
            assert_eq!(
                run(&f, &lock, |step| {
                    if step == Checkpoint::Unlinked(Target::Slot(0)) {
                        let (directory, name, _) = locations()[selected];
                        let path = match directory {
                            0 => f.paths.state_directory.join(name),
                            1 => f.config.join(name),
                            2 => f.paths.state_directory.join(PENDING_DIRECTORY).join(name),
                            _ => unreachable!(),
                        };
                        let bytes = fs::read(&path).unwrap();
                        fs::rename(&path, f.root.join("saved-source")).unwrap();
                        write(&path, &bytes);
                    }
                    true
                }),
                Err(ExecutionError::Ambiguous)
            );
            fences(&f, &old, &new, true);
        }
    }

    fn crash(f: &Fixture, selected: usize, only_effects: bool) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "restore_executor_candidate::successor::rotation::cleanup::tests::cleanup_crash_worker"])
            .env("OMAVLESS_SYNTHETIC_CLEAN_ROOT", &f.root)
            .env("OMAVLESS_SYNTHETIC_CLEAN_POINT", selected.to_string())
            .env("OMAVLESS_SYNTHETIC_CLEAN_EFFECT", if only_effects { "yes" } else { "no" })
            .output().unwrap();
        assert_eq!(output.status.signal(), Some(9));
    }
    #[test]
    fn actual_unlink_and_directory_sync_crashes_reopen_each_prefix() {
        for stopped in 0..24 {
            let (f, lock, old, new) = ready(true, 15);
            drop(lock);
            crash(&f, stopped, true);
            fences(&f, &old, &new, true);
            let lock = f.lock();
            assert_eq!(
                run(&f, &lock, |_| true),
                Ok(CleanupResult::CleanedStillFenced)
            );
            done(&f, &old, &new, true);
        }
    }
    #[test]
    fn repeated_process_loss_before_resync_and_after_next_unlink_stays_recoverable() {
        let (f, lock, old, new) = ready(true, 15);
        drop(lock);
        crash(&f, 8, true); // First stage unlink, before its directory sync.
        crash(&f, 0, false); // Restart dies while re-establishing fence durability.
        crash(&f, 0, true); // Another restart dies after the next stage unlink.
        fences(&f, &old, &new, true);
        let lock = f.lock();
        assert_eq!(
            run(&f, &lock, |_| true),
            Ok(CleanupResult::CleanedStillFenced)
        );
        done(&f, &old, &new, true);
    }
    #[test]
    #[ignore = "internal synthetic successor cleanup worker"]
    fn cleanup_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_CLEAN_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_CLEAN_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let only_effects = std::env::var("OMAVLESS_SYNTHETIC_CLEAN_EFFECT").unwrap() == "yes";
        let lock = f.lock();
        let mut index = 0;
        let _ = run(&f, &lock, |step| {
            if !only_effects || effect(step) {
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
        });
        panic!("expected worker termination");
    }
}
