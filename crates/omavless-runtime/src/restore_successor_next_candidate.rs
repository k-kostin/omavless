// SPDX-License-Identifier: MIT
//! Inactive create-only next closure. No cleanup, exchange or owner admission.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationResult {
    PublishedStillFenced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    SourceDirectorySynced(usize),
    Created,
    Written,
    FileSynced,
    DirectorySynced,
    Reopened,
}

// Indices deliberately match the reviewed rotation snapshot. Explicit length
// checks below prevent future vector changes from silently truncating a zip.
fn locations() -> [(usize, &'static str, usize); 17] {
    [
        (0, CLOSURE_MEMBER, CLOSURE_BYTES),
        (0, NEXT_CLOSURE_MEMBER, CLOSURE_BYTES),
        (0, SUCCESSOR_MEMBER, HANDOFF_BYTES),
        (0, RECEIPT_MEMBER, RECEIPT_BYTES),
        (1, LIVE[0], MAX_PRIVATE_STORE_BYTES),
        (1, LIVE[1], MAX_TEMPLATE_BYTES),
        (0, INTENT, RECORD_BYTES),
        (0, TERMINAL, RECORD_BYTES),
        (1, NEW_SLOT[0], MAX_PRIVATE_STORE_BYTES),
        (1, NEW_SLOT[1], MAX_TEMPLATE_BYTES),
        (1, OLD_SLOT[0], MAX_PRIVATE_STORE_BYTES),
        (1, OLD_SLOT[1], MAX_TEMPLATE_BYTES),
        (2, MEMBERS[0], MAX_PRIVATE_STORE_BYTES),
        (2, MEMBERS[1], MAX_TEMPLATE_BYTES),
        (2, MEMBERS[2], MAX_PRIVATE_STORE_BYTES),
        (2, MEMBERS[3], MAX_TEMPLATE_BYTES),
        (2, READY_MEMBER, READY_BYTES),
    ]
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    snapshot: Snapshot,
    directories: [File; 3],
    receipt: RetirementReceipt,
}
impl Context<'_> {
    fn check_directories(&self) -> Result<(), ExecutionError> {
        let stage = self.paths.state_directory.join(PENDING_DIRECTORY);
        for (index, path) in [&self.paths.state_directory, self.config, &stage]
            .into_iter()
            .enumerate()
        {
            let current = open_private_directory(path, self.uid).map_err(|_| REFUSE)?;
            if !same_directory(
                &self.snapshot.directories[index],
                &current.metadata().map_err(|_| REFUSE)?,
            ) || !same_directory(
                &self.snapshot.directories[index],
                &self.directories[index].metadata().map_err(|_| REFUSE)?,
            ) {
                return Err(REFUSE);
            }
        }
        Ok(())
    }
    fn check_sources(&self, gate: &mut impl FnMut() -> bool) -> Result<(), ExecutionError> {
        if !gate() || !self.lock.authorizes(self.paths, self.uid) {
            return Err(REFUSE);
        }
        let marker = read_marker_existing(self.paths, self.uid).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust || marker.generation() != self.generation {
            return Err(REFUSE);
        }
        self.check_directories()?;
        absent(&self.directories[0], "routing-preset.pending.json")?;
        if self.snapshot.desired
            != read_desired_for_decision(self.paths, self.uid, self.lock).map_err(|_| REFUSE)?
            || inspect_cleanup_prefix(self.paths, &self.directories[0], self.uid, &self.receipt)
                .map_err(|_| REFUSE)?
                != Step::StageMember(0)
        {
            return Err(REFUSE);
        }
        // Reparse the strict prefix on every guard: known-name comparisons
        // alone cannot detect a newly injected unknown stage member.
        for (index, (directory, name, limit)) in locations().into_iter().enumerate() {
            if index == 1 {
                continue; // Only the exclusively owned destination may change.
            }
            let current = read_optional(&self.directories[directory], name, self.uid, limit)
                .map_err(|_| REFUSE)?;
            match (&self.snapshot.members[index], current) {
                (Some(prior), Some(now)) if prior.0 == now.0 && same_member(&prior.1, &now.1) => (),
                (None, None) => (),
                _ => return Err(REFUSE),
            }
        }
        if read_marker_existing(self.paths, self.uid).ok() != Some(marker)
            || self.snapshot.desired
                != read_desired_for_decision(self.paths, self.uid, self.lock).map_err(|_| REFUSE)?
        {
            return Err(REFUSE);
        }
        self.check_directories()
    }

    fn check_destination(
        &self,
        file: &File,
        expected: &Metadata,
        bytes: Option<&[u8]>,
    ) -> Result<(), ExecutionError> {
        let current = File::from(
            openat(
                &self.directories[0],
                Path::new(NEXT_CLOSURE_MEMBER),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let metadata = current.metadata().map_err(|_| REFUSE)?;
        if !metadata.is_file()
            || metadata.uid() != self.uid
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
            || metadata.len() != bytes.map_or(0, |v| v.len()) as u64
            || !same_member(expected, &metadata)
            || !same_member(expected, &file.metadata().map_err(|_| REFUSE)?)
        {
            return Err(REFUSE);
        }
        if let Some(bytes) = bytes {
            let (raw, after) = read_optional(
                &self.directories[0],
                NEXT_CLOSURE_MEMBER,
                self.uid,
                CLOSURE_BYTES,
            )
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
            if raw.as_slice() != bytes || !same_member(expected, &after) {
                return Err(REFUSE);
            }
        }
        Ok(())
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn publish_next_closure(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<PublicationResult, ExecutionError> {
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
    mut hook: impl FnMut(Checkpoint) -> bool,
) -> Result<PublicationResult, ExecutionError> {
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
    if snapshot.phase != RotationPhase::BeforeNext
        || snapshot.prefix != Step::StageMember(0)
        || snapshot.members.len() != locations().len()
        || snapshot.directories.len() != 3
        || !gate()
        || !snapshot.same(&observe_rotation(
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
    let receipt = RetirementReceipt::decode(&snapshot.members[3].as_ref().ok_or(REFUSE)?.0)
        .map_err(|_| REFUSE)?;
    let bytes = ClosureRecord::from_verified_receipt(&receipt)
        .map_err(|_| REFUSE)?
        .encode();
    let context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        snapshot,
        receipt,
        directories: [
            open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?,
            open_private_directory(config, uid).map_err(|_| REFUSE)?,
            open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
                .map_err(|_| REFUSE)?,
        ],
    };
    for (index, (directory, name, _)) in locations().into_iter().enumerate() {
        if index == 1 {
            continue;
        }
        if let Some((_, expected)) = &context.snapshot.members[index] {
            context.check_sources(&mut gate)?;
            absent(&context.directories[0], NEXT_CLOSURE_MEMBER)?;
            let file = File::from(
                openat(
                    &context.directories[directory],
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
            context.check_sources(&mut gate)?;
            absent(&context.directories[0], NEXT_CLOSURE_MEMBER)?;
        }
    }
    for index in [2, 1, 0] {
        context.check_sources(&mut gate)?;
        absent(&context.directories[0], NEXT_CLOSURE_MEMBER)?;
        context.directories[index]
            .sync_all()
            .map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::SourceDirectorySynced(index)) {
            return Err(ExecutionError::Ambiguous);
        }
        context.check_sources(&mut gate)?;
        absent(&context.directories[0], NEXT_CLOSURE_MEMBER)?;
    }
    let mut file = File::from(
        openat(
            &context.directories[0],
            Path::new(NEXT_CLOSURE_MEMBER),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| ExecutionError::Ambiguous)?,
    );
    let created = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
    let mut checkpoint = |step, expected: &Metadata, bytes: Option<&[u8]>, file: &File| {
        if !hook(step) {
            return Err(ExecutionError::Ambiguous);
        }
        context
            .check_sources(&mut gate)
            .map_err(|_| ExecutionError::Ambiguous)?;
        context
            .check_destination(file, expected, bytes)
            .map_err(|_| ExecutionError::Ambiguous)
    };
    checkpoint(Checkpoint::Created, &created, None, &file)?;
    file.write_all(&bytes)
        .map_err(|_| ExecutionError::Ambiguous)?;
    let written = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
    if created.dev() != written.dev() || created.ino() != written.ino() {
        return Err(ExecutionError::Ambiguous);
    }
    checkpoint(Checkpoint::Written, &written, Some(&bytes), &file)?;
    file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
    checkpoint(Checkpoint::FileSynced, &written, Some(&bytes), &file)?;
    context.directories[0]
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    checkpoint(Checkpoint::DirectorySynced, &written, Some(&bytes), &file)?;
    checkpoint(Checkpoint::Reopened, &written, Some(&bytes), &file)?;
    let reviewed = review_rotation(config, paths, uid, generation, lock, backup, &mut gate)
        .map_err(|_| ExecutionError::Ambiguous)?;
    if reviewed != RotationPhase::NextPublished {
        return Err(ExecutionError::Ambiguous);
    }
    context
        .check_sources(&mut gate)
        .map_err(|_| ExecutionError::Ambiguous)?;
    context
        .check_destination(&file, &written, Some(&bytes))
        .map_err(|_| ExecutionError::Ambiguous)?;
    Ok(PublicationResult::PublishedStillFenced)
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::{pair, second};
    use super::super::tests::setup;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    fn run(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<PublicationResult, ExecutionError> {
        publish(&f.config, &f.paths, f.uid, 2, lock, second(), || true, hook)
    }
    fn write(path: &Path, raw: &[u8]) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn copy_fixture_directory(source: &Path, target: &Path) {
        fs::create_dir(target).unwrap();
        fs::set_permissions(target, fs::Permissions::from_mode(0o700)).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_fixture_directory(&entry.path(), &destination);
            } else {
                assert!(entry.file_type().unwrap().is_file());
                write(&destination, &fs::read(entry.path()).unwrap());
            }
        }
    }
    fn source_path(f: &Fixture, index: usize) -> PathBuf {
        let (directory, name, _) = locations()[index];
        match directory {
            0 => f.paths.state_directory.join(name),
            1 => f.config.join(name),
            2 => f.paths.state_directory.join(PENDING_DIRECTORY).join(name),
            _ => unreachable!(),
        }
    }
    fn still_fenced(f: &Fixture, old: &[u8], commit: bool) {
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            old
        );
        for name in [SUCCESSOR_MEMBER, RECEIPT_MEMBER, INTENT, TERMINAL] {
            assert!(f.paths.state_directory.join(name).exists());
        }
        assert!(
            f.paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(READY_MEMBER)
                .exists()
        );
        pair(f, commit);
    }
    fn publication_step(step: Checkpoint) -> bool {
        !matches!(
            step,
            Checkpoint::SourceSynced(_) | Checkpoint::SourceDirectorySynced(_)
        )
    }

    #[test]
    fn publishes_only_exact_next_for_commit_and_abort_and_refuses_retry() {
        for commit in [false, true] {
            let (f, lock, old, new) = setup(commit);
            assert_eq!(
                run(&f, &lock, |_| true),
                Ok(PublicationResult::PublishedStillFenced)
            );
            assert_eq!(
                fs::read(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap(),
                new
            );
            assert_eq!(
                review_rotation(&f.config, &f.paths, f.uid, 2, &lock, second(), || true),
                Ok(RotationPhase::NextPublished)
            );
            assert!(run(&f, &lock, |_| true).is_err());
            still_fenced(&f, &old, commit);
        }
    }

    #[test]
    fn all_resync_and_publication_refusals_keep_prior_sources_and_fences() {
        for stopped in 0..20 {
            let (f, lock, old, _) = setup(true);
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
            let next = f.paths.state_directory.join(NEXT_CLOSURE_MEMBER);
            assert_eq!(next.exists(), stopped >= 15);
            if stopped >= 15 {
                assert!(run(&f, &lock, |_| true).is_err());
            }
            still_fenced(&f, &old, true);
        }
    }

    #[test]
    fn unknown_stage_after_every_resync_and_publication_checkpoint_refuses() {
        for stopped in 0..20 {
            let (f, lock, old, _) = setup(true);
            let mut index = 0;
            assert!(
                run(&f, &lock, |_| {
                    if index == stopped {
                        write(
                            &f.paths
                                .state_directory
                                .join(PENDING_DIRECTORY)
                                .join("unknown"),
                            b"bounded",
                        );
                    }
                    index += 1;
                    true
                })
                .is_err()
            );
            assert_eq!(index, stopped + 1);
            assert_eq!(
                f.paths.state_directory.join(NEXT_CLOSURE_MEMBER).exists(),
                stopped >= 15
            );
            still_fenced(&f, &old, true);
        }
    }

    #[test]
    fn late_same_byte_source_swaps_and_unknown_stage_entries_poison_publication() {
        for stopped in 0..5 {
            for member in [0, 2, 3, 4, 5, 6, 7, 12, 13, 14, 15, 16, 17] {
                let (f, lock, old, _) = setup(true);
                let mut index = 0;
                assert_eq!(
                    run(&f, &lock, |step| {
                        if publication_step(step) {
                            if index == stopped {
                                if member == 17 {
                                    write(
                                        &f.paths
                                            .state_directory
                                            .join(PENDING_DIRECTORY)
                                            .join("unknown"),
                                        b"bounded",
                                    );
                                } else {
                                    let path = source_path(&f, member);
                                    let raw = fs::read(&path).unwrap();
                                    // Preserve the old inode outside the stage:
                                    // refusal must detect substitution, not an
                                    // accidental unknown stage member.
                                    fs::rename(&path, f.root.join("preserved-source")).unwrap();
                                    write(&path, &raw);
                                }
                            }
                            index += 1;
                        }
                        true
                    }),
                    Err(ExecutionError::Ambiguous)
                );
                still_fenced(&f, &old, true);
            }
        }
    }

    #[test]
    fn destination_swaps_truncation_gates_and_directory_swaps_refuse() {
        for stopped in 0..5 {
            for change in 0..7 {
                let (f, lock, old, _) = setup(true);
                let mut index = 0;
                let enabled = std::cell::Cell::new(true);
                assert_eq!(
                    publish(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        || enabled.get(),
                        |step| {
                            if publication_step(step) {
                                if index == stopped {
                                    let path = f.paths.state_directory.join(NEXT_CLOSURE_MEMBER);
                                    match change {
                                        0 => {
                                            let raw = fs::read(&path).unwrap();
                                            fs::rename(&path, path.with_extension("saved"))
                                                .unwrap();
                                            write(&path, &raw);
                                        }
                                        1 => write(&path, b"torn"),
                                        2 => enabled.set(false),
                                        3 => {
                                            let stage =
                                                f.paths.state_directory.join(PENDING_DIRECTORY);
                                            fs::rename(&stage, stage.with_extension("saved"))
                                                .unwrap();
                                            fs::create_dir(&stage).unwrap();
                                            fs::set_permissions(
                                                &stage,
                                                fs::Permissions::from_mode(0o700),
                                            )
                                            .unwrap();
                                            for name in MEMBERS.into_iter().chain([READY_MEMBER]) {
                                                write(
                                                    &stage.join(name),
                                                    &fs::read(
                                                        stage.with_extension("saved").join(name),
                                                    )
                                                    .unwrap(),
                                                );
                                            }
                                        }
                                        4 => write(
                                            &f.paths
                                                .state_directory
                                                .join("routing-preset.pending.json"),
                                            b"pending",
                                        ),
                                        5 | 6 => {
                                            let directory = if change == 5 {
                                                &f.paths.state_directory
                                            } else {
                                                &f.config
                                            };
                                            let saved = directory.with_extension("saved");
                                            fs::rename(directory, &saved).unwrap();
                                            copy_fixture_directory(&saved, directory);
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
                still_fenced(&f, &old, true);
            }
        }
    }

    #[test]
    fn unsafe_existing_missing_and_wrong_archive_admission_never_creates_next() {
        let (f, lock, old, _) = setup(true);
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
        assert!(!f.paths.state_directory.join(NEXT_CLOSURE_MEMBER).exists());
        let next = f.paths.state_directory.join(NEXT_CLOSURE_MEMBER);
        std::os::unix::fs::symlink(f.root.join("missing"), &next).unwrap();
        assert!(run(&f, &lock, |_| true).is_err());
        assert!(!f.root.join("missing").exists());
        still_fenced(&f, &old, true);
        let (f, lock, old, _) = setup(true);
        fs::remove_file(f.paths.state_directory.join(TERMINAL)).unwrap();
        assert!(run(&f, &lock, |_| true).is_err());
        assert!(!f.paths.state_directory.join(NEXT_CLOSURE_MEMBER).exists());
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            old
        );
    }

    #[test]
    fn actual_publication_crashes_reopen_still_fenced_and_never_retry() {
        for stopped in 0..5 {
            let (f, lock, old, _) = setup(true);
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "restore_executor_candidate::successor::rotation::publication::tests::next_crash_worker"])
                .env("OMAVLESS_SYNTHETIC_NEXT_ROOT", &f.root)
                .env("OMAVLESS_SYNTHETIC_NEXT_POINT", stopped.to_string())
                .output().unwrap();
            assert_eq!(output.status.signal(), Some(9));
            let lock = f.lock();
            assert!(run(&f, &lock, |_| true).is_err());
            still_fenced(&f, &old, true);
            let review = review_rotation(&f.config, &f.paths, f.uid, 2, &lock, second(), || true);
            if stopped == 0 {
                assert!(review.is_err());
            } else {
                // Visible complete bytes classify; no durability or cleanup
                // authority follows. A later recovery writer must resync.
                assert_eq!(review, Ok(RotationPhase::NextPublished));
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic next-closure crash worker"]
    fn next_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_NEXT_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_NEXT_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = run(&f, &lock, |step| {
            if publication_step(step) {
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
