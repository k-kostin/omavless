// SPDX-License-Identifier: MIT
//! Inactive create-only ticket. Every prefix remains a manual startup fence.
//! No existing-ticket retry, historical permission, cleanup or normal owner.
use super::*;
use crate::restore_disposition_ticket_model::{TICKET_BYTES, TICKET_MEMBER, Ticket};
use std::io::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationResult {
    PublishedStillFenced,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    DirectorySynced(usize),
    Created,
    Written,
    FileSynced,
    ParentSynced,
    Reopened,
}

type BoundaryMembers = [Option<(Zeroizing<Vec<u8>>, Metadata)>; 3];
struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    evidence: FinalEvidence<'a>,
    directories: [File; 2],
    runtime: File,
    boundary: BoundaryMembers,
}
fn boundary(paths: &CutoverPaths, uid: u32) -> Result<BoundaryMembers, ExecutionError> {
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let runtime = open_private_directory(&paths.runtime_base, uid).map_err(|_| REFUSE)?;
    Ok([
        read_optional(
            &state,
            crate::cutover::OWNERSHIP_MARKER_NAME,
            uid,
            crate::cutover::MAX_OWNERSHIP_MARKER_BYTES as usize,
        )
        .map_err(|_| REFUSE)?,
        read_optional(
            &state,
            "desired.json",
            uid,
            crate::desired::MAX_DESIRED_STATE_BYTES as usize,
        )
        .map_err(|_| REFUSE)?,
        read_optional(&runtime, "omavless-login.receipt", uid, 1024).map_err(|_| REFUSE)?,
    ])
}
impl Context<'_> {
    fn sources(&self, gate: &mut impl FnMut() -> bool) -> Result<(), ExecutionError> {
        if !gate() || !self.lock.authorizes(self.paths, self.uid) {
            return Err(REFUSE);
        }
        let now = observe_final_sources(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
            &self.evidence.off,
            self.evidence.template,
        )?;
        if !self.evidence.snapshot.same(&now) {
            return Err(REFUSE);
        }
        if !same_directory(
            &self.runtime.metadata().map_err(|_| REFUSE)?,
            &open_private_directory(&self.paths.runtime_base, self.uid)
                .map_err(|_| REFUSE)?
                .metadata()
                .map_err(|_| REFUSE)?,
        ) {
            return Err(REFUSE);
        }
        for (before, after) in self.boundary.iter().zip(boundary(self.paths, self.uid)?) {
            match (before, after) {
                (None, None) => (),
                (Some((a, am)), Some((b, bm))) if a == &b && same_member(am, &bm) => (),
                _ => return Err(REFUSE),
            }
        }
        for (index, path) in [&self.paths.state_directory, self.config]
            .into_iter()
            .enumerate()
        {
            let current = open_private_directory(path, self.uid).map_err(|_| REFUSE)?;
            let expected = &self.evidence.snapshot.evidence.directories[index];
            if !same_directory(expected, &current.metadata().map_err(|_| REFUSE)?)
                || !same_directory(
                    expected,
                    &self.directories[index].metadata().map_err(|_| REFUSE)?,
                )
            {
                return Err(REFUSE);
            }
        }
        Ok(())
    }
    fn destination(
        &self,
        file: &File,
        expected: &Metadata,
        bytes: &[u8],
    ) -> Result<(), ExecutionError> {
        let reopened = File::from(
            openat(
                &self.directories[0],
                Path::new(TICKET_MEMBER),
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let metadata = reopened.metadata().map_err(|_| REFUSE)?;
        if !metadata.is_file()
            || metadata.uid() != self.uid
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
            || metadata.len() != bytes.len() as u64
            || !same_member(expected, &metadata)
            || !same_member(expected, &file.metadata().map_err(|_| REFUSE)?)
        {
            return Err(REFUSE);
        }
        // General private-member readers correctly reject empty source files;
        // only our just-created exact destination may have this one prefix.
        if bytes.is_empty() {
            return Ok(());
        }
        let (now, metadata) =
            read_optional(&self.directories[0], TICKET_MEMBER, self.uid, TICKET_BYTES)
                .map_err(|_| REFUSE)?
                .ok_or(REFUSE)?;
        if now.as_slice() != bytes
            || !same_member(expected, &metadata)
            || !same_member(expected, &file.metadata().map_err(|_| REFUSE)?)
        {
            return Err(REFUSE);
        }
        Ok(())
    }
}

/// Caller must retain exact owner/desired/login/host boundary gates under this
/// same lease. No product caller exists; a Boolean alone is not authority.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn publish_disposition(
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
    let evidence = capture_final_closure(config, paths, uid, generation, lock, backup, &mut gate)?;
    if evidence.phase() != FinalPhase::HandoffAbsentReceiptAbsent
        || evidence.snapshot.evidence.members.len() != publication::locations().len()
        || evidence.snapshot.evidence.directories.len() != 2
    {
        return Err(REFUSE);
    }
    let closure = ClosureRecord::decode(
        &evidence.snapshot.evidence.members[0]
            .as_ref()
            .ok_or(REFUSE)?
            .0,
    )
    .map_err(|_| REFUSE)?;
    let ticket = Ticket::from_bound_closure(
        &closure,
        uid,
        generation,
        evidence
            .snapshot
            .evidence
            .desired
            .as_ref()
            .map(|v| v.as_slice()),
    )
    .ok_or(REFUSE)?;
    let bytes = ticket.encode();
    let context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        evidence,
        boundary: boundary(paths, uid)?,
        runtime: open_private_directory(&paths.runtime_base, uid).map_err(|_| REFUSE)?,
        directories: [
            open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?,
            open_private_directory(config, uid).map_err(|_| REFUSE)?,
        ],
    };
    // Durable terminal output before any ticket byte. Reopen each pinned
    // source, synchronize it and recheck exact bytes/inode after every effect.
    for (index, directory, name) in [(0, 0, CLOSURE_MEMBER), (4, 1, LIVE[0]), (5, 1, LIVE[1])] {
        context.sources(&mut gate)?;
        ticket_absent(paths, uid)?;
        let file = File::from(
            openat(
                &context.directories[directory],
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let expected = &context.evidence.snapshot.evidence.members[index]
            .as_ref()
            .ok_or(REFUSE)?
            .1;
        if !same_member(expected, &file.metadata().map_err(|_| REFUSE)?) {
            return Err(REFUSE);
        }
        file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::SourceSynced(index)) {
            return Err(ExecutionError::Ambiguous);
        }
        context.sources(&mut gate)?;
        ticket_absent(paths, uid)?;
    }
    for index in [1, 0] {
        context.sources(&mut gate)?;
        ticket_absent(paths, uid)?;
        context.directories[index]
            .sync_all()
            .map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::DirectorySynced(index)) {
            return Err(ExecutionError::Ambiguous);
        }
        context.sources(&mut gate)?;
        ticket_absent(paths, uid)?;
    }
    context.sources(&mut gate)?;
    ticket_absent(paths, uid)?;
    let mut file = File::from(
        openat(
            &context.directories[0],
            Path::new(TICKET_MEMBER),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(|_| REFUSE)?,
    );
    // From O_EXCL success onward, every failure is ambiguous and leaves the
    // exact visible prefix intact. There is deliberately no rollback/unlink.
    let created = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
    let mut checkpoint = |point, expected: &Metadata, raw: &[u8], file: &File| {
        if !hook(point) {
            return Err(ExecutionError::Ambiguous);
        }
        context
            .sources(&mut gate)
            .map_err(|_| ExecutionError::Ambiguous)?;
        context
            .destination(file, expected, raw)
            .map_err(|_| ExecutionError::Ambiguous)
    };
    checkpoint(Checkpoint::Created, &created, &[], &file)?;
    file.write_all(&bytes)
        .map_err(|_| ExecutionError::Ambiguous)?;
    let written = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
    if created.dev() != written.dev() || created.ino() != written.ino() {
        return Err(ExecutionError::Ambiguous);
    }
    checkpoint(Checkpoint::Written, &written, &bytes, &file)?;
    file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
    checkpoint(Checkpoint::FileSynced, &written, &bytes, &file)?;
    context.directories[0]
        .sync_all()
        .map_err(|_| ExecutionError::Ambiguous)?;
    checkpoint(Checkpoint::ParentSynced, &written, &bytes, &file)?;
    checkpoint(Checkpoint::Reopened, &written, &bytes, &file)?;
    let decoded = Ticket::decode(&bytes).ok_or(ExecutionError::Ambiguous)?;
    if !decoded.matches(
        &closure,
        uid,
        generation,
        context
            .evidence
            .snapshot
            .evidence
            .desired
            .as_ref()
            .map(|v| v.as_slice()),
    ) {
        return Err(ExecutionError::Ambiguous);
    }
    Ok(PublicationResult::PublishedStillFenced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_executor_candidate::successor::tests::second;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    fn ready(commit: bool) -> (Fixture, MigrationLock) {
        let (f, lock) = super::super::tests::ready(commit);
        handoff_retirement::retire_successor_handoff(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            second(),
            || true,
        )
        .unwrap();
        handoff_retirement::last_receipt::retire_last_successor_receipt(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            second(),
            || true,
        )
        .unwrap();
        (f, lock)
    }
    fn run(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<PublicationResult, ExecutionError> {
        publish(&f.config, &f.paths, f.uid, 2, lock, second(), || true, hook)
    }
    fn snapshots(f: &Fixture) -> Vec<(PathBuf, Vec<u8>, Metadata)> {
        [
            f.paths.state_directory.join(CLOSURE_MEMBER),
            f.config.join(LIVE[0]),
            f.config.join(LIVE[1]),
        ]
        .into_iter()
        .map(|p| {
            let raw = fs::read(&p).unwrap();
            let m = fs::metadata(&p).unwrap();
            (p, raw, m)
        })
        .collect()
    }
    fn unchanged(before: &[(PathBuf, Vec<u8>, Metadata)]) {
        for (path, bytes, metadata) in before {
            assert_eq!(&fs::read(path).unwrap(), bytes);
            assert!(same_member(metadata, &fs::metadata(path).unwrap()));
        }
    }
    fn member(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn disposition_ticket_success_commit_abort_keeps_c1_and_refuses_replay_or_review() {
        for commit in [false, true] {
            let (f, lock) = ready(commit);
            let before = snapshots(&f);
            assert_eq!(
                run(&f, &lock, |_| true),
                Ok(PublicationResult::PublishedStillFenced)
            );
            let raw = fs::read(f.paths.state_directory.join(TICKET_MEMBER)).unwrap();
            let parsed = Ticket::decode(&raw).unwrap();
            let closure = ClosureRecord::decode(&before[0].1).unwrap();
            assert!(parsed.matches(&closure, f.uid, 2, None));
            assert!(!parsed.matches(&closure, f.uid.wrapping_add(1), 2, None));
            assert!(!parsed.matches(&closure, f.uid, 3, None));
            assert!(run(&f, &lock, |_| true).is_err());
            assert!(
                crate::restore_successor_publication_candidate::publish_successor_handoff(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    second(),
                    [99; 16],
                    || true,
                )
                .is_err()
            );
            assert!(!f.paths.state_directory.join(SUCCESSOR_MEMBER).exists());
            assert!(
                review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
                    .is_err()
            );
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
            unchanged(&before);
        }
    }
    #[test]
    fn disposition_ticket_fixed_format_rejects_torn_reserved_version_and_crossed_closure() {
        let (f, lock) = ready(true);
        run(&f, &lock, |_| true).unwrap();
        let raw = fs::read(f.paths.state_directory.join(TICKET_MEMBER)).unwrap();
        for len in 0..raw.len() {
            assert!(Ticket::decode(&raw[..len]).is_none());
        }
        let mut extra = raw.clone();
        extra.push(0);
        assert!(Ticket::decode(&extra).is_none());
        for index in 0..raw.len() {
            let mut changed = raw.clone();
            changed[index] ^= 1;
            assert!(Ticket::decode(&changed).is_none());
        }
        let (other, _lease) = ready(false);
        let wrong = ClosureRecord::decode(
            &fs::read(other.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
        )
        .unwrap();
        assert!(
            !Ticket::decode(&raw)
                .unwrap()
                .matches(&wrong, f.uid, 2, None)
        );
    }
    #[test]
    fn disposition_ticket_wrong_archive_unsafe_existing_and_partial_prefix_never_repair() {
        let (f, lock) = ready(false);
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
        assert!(!f.paths.state_directory.join(TICKET_MEMBER).exists());
        for kind in 0..5 {
            let path = f.paths.state_directory.join(TICKET_MEMBER);
            match kind {
                0 => member(&path, b""),
                1 => member(&path, b"partial"),
                2 => fs::create_dir(&path).unwrap(),
                3 => std::os::unix::fs::symlink("missing", &path).unwrap(),
                _ => fs::hard_link(f.paths.state_directory.join(CLOSURE_MEMBER), &path).unwrap(),
            }
            assert!(run(&f, &lock, |_| true).is_err());
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
            if kind == 2 {
                fs::remove_dir(&path).unwrap();
            } else {
                fs::remove_file(&path).unwrap();
            }
        }
    }
    #[test]
    fn disposition_ticket_every_checkpoint_refuses_source_or_destination_same_byte_swap() {
        for point in 0..10 {
            let (f, lock) = ready(true);
            let mut index = 0;
            assert!(
                run(&f, &lock, |_| {
                    if index == point {
                        let path = if point >= 5 {
                            f.paths.state_directory.join(TICKET_MEMBER)
                        } else {
                            f.paths.state_directory.join(CLOSURE_MEMBER)
                        };
                        let raw = fs::read(&path).unwrap();
                        fs::rename(&path, path.with_extension("displaced-test")).unwrap();
                        member(&path, &raw);
                    }
                    index += 1;
                    true
                })
                .is_err()
            );
        }
    }
    #[test]
    fn disposition_ticket_late_marker_pending_directory_and_final_gate_refuse() {
        for kind in 0..4 {
            for selected in [0, 5, 9] {
                let (f, lock) = ready(true);
                let mut index = 0;
                assert!(
                    run(&f, &lock, |_| {
                        if index == selected {
                            match kind {
                                0 => {
                                    let path = f
                                        .paths
                                        .state_directory
                                        .join(crate::cutover::OWNERSHIP_MARKER_NAME);
                                    let raw = fs::read(&path).unwrap();
                                    fs::rename(&path, path.with_extension("old-test")).unwrap();
                                    member(&path, &raw);
                                }
                                1 => member(&f.paths.state_directory.join(INTENT), b"foreign"),
                                2 => {
                                    fs::rename(&f.config, f.config.with_extension("old-test"))
                                        .unwrap();
                                    fs::create_dir(&f.config).unwrap();
                                    fs::set_permissions(
                                        &f.config,
                                        fs::Permissions::from_mode(0o700),
                                    )
                                    .unwrap();
                                }
                                _ => {
                                    let old = f.paths.runtime_base.with_extension("old-test");
                                    fs::rename(&f.paths.runtime_base, &old).unwrap();
                                    fs::create_dir(&f.paths.runtime_base).unwrap();
                                    fs::set_permissions(
                                        &f.paths.runtime_base,
                                        fs::Permissions::from_mode(0o700),
                                    )
                                    .unwrap();
                                    fs::rename(
                                        old.join(f.paths.operation_lock.file_name().unwrap()),
                                        &f.paths.operation_lock,
                                    )
                                    .unwrap();
                                }
                            }
                        }
                        index += 1;
                        true
                    })
                    .is_err()
                );
            }
        }
        let (f, lock) = ready(true);
        let mut calls = 0;
        publish(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            second(),
            || {
                calls += 1;
                true
            },
            |_| true,
        )
        .unwrap();
        let (f, lock) = ready(true);
        let mut observed = 0;
        assert_eq!(
            publish(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                || {
                    observed += 1;
                    observed < calls
                },
                |_| true
            ),
            Err(ExecutionError::Ambiguous)
        );
        assert!(f.paths.state_directory.join(TICKET_MEMBER).exists());
    }
    #[test]
    fn disposition_ticket_actual_prefix_sigkill_never_retries_or_unfences() {
        for commit in [false, true] {
            for selected in 0..5 {
                let (f, lock) = ready(commit);
                let before = snapshots(&f);
                drop(lock);
                let output = Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored", "--exact", "restore_executor_candidate::successor::rotation::final_review::disposition::tests::disposition_ticket_crash_worker"])
                    .env("OMAVLESS_SYNTHETIC_TICKET_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_TICKET_POINT", selected.to_string())
                    .output().unwrap();
                assert_eq!(
                    output.status.signal(),
                    Some(9),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let lock = f.lock();
                assert!(run(&f, &lock, |_| true).is_err());
                assert!(crate::pending_private_transaction::pending_at(
                    &f.paths.state_directory
                ));
                let len = fs::metadata(f.paths.state_directory.join(TICKET_MEMBER))
                    .unwrap()
                    .len();
                assert_eq!(
                    len,
                    if selected == 0 {
                        0
                    } else {
                        TICKET_BYTES as u64
                    }
                );
                unchanged(&before);
            }
        }
    }
    #[test]
    fn disposition_ticket_direct_retirement_callers_reject_initial_and_late_presence() {
        for receipt in [false, true] {
            for selected in [1, 3] {
                let (f, lock) = super::super::tests::ready(true);
                if receipt {
                    handoff_retirement::retire_successor_handoff(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        || true,
                    )
                    .unwrap();
                }
                let path = f.paths.state_directory.join(if receipt {
                    RECEIPT_MEMBER
                } else {
                    SUCCESSOR_MEMBER
                });
                let before = fs::read(&path).unwrap();
                let metadata = fs::metadata(&path).unwrap();
                let mut calls = 0;
                let gate = || {
                    calls += 1;
                    if calls == selected {
                        member(&f.paths.state_directory.join(TICKET_MEMBER), b"partial");
                    }
                    true
                };
                if receipt {
                    assert!(
                        handoff_retirement::last_receipt::retire_last_successor_receipt(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            second(),
                            gate
                        )
                        .is_err()
                    );
                } else {
                    assert!(
                        handoff_retirement::retire_successor_handoff(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            second(),
                            gate
                        )
                        .is_err()
                    );
                }
                assert_eq!(fs::read(&path).unwrap(), before);
                assert!(same_member(&metadata, &fs::metadata(&path).unwrap()));
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic ticket worker"]
    fn disposition_ticket_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_TICKET_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_TICKET_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = run(&f, &lock, |point| {
            if matches!(
                point,
                Checkpoint::Created
                    | Checkpoint::Written
                    | Checkpoint::FileSynced
                    | Checkpoint::ParentSynced
                    | Checkpoint::Reopened
            ) {
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
        panic!("expected synthetic kill");
    }

    #[test]
    fn disposition_ticket_earlier_successor_effects_refuse_initial_and_late_ticket() {
        use crate::restore_executor_candidate::successor::{self, rotation};
        for phase in 0..4 {
            for selected in [1, 3] {
                let (f, lock) = if phase == 0 {
                    successor::tests::prepared()
                } else {
                    let (f, lock, _, _) = rotation::tests::setup(true);
                    if phase >= 2 {
                        rotation::publication::publish_next_closure(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            second(),
                            || true,
                        )
                        .unwrap();
                        rotation::cleanup::cleanup_successor(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            second(),
                            || true,
                        )
                        .unwrap();
                    }
                    if phase == 3 {
                        rotation::exchange::exchange_next_closure(
                            &f.config,
                            &f.paths,
                            f.uid,
                            2,
                            &lock,
                            second(),
                            || true,
                        )
                        .unwrap();
                    }
                    (f, lock)
                };
                let before = snapshots(&f);
                let mut calls = 0;
                let gate = || {
                    calls += 1;
                    if calls == selected {
                        member(&f.paths.state_directory.join(TICKET_MEMBER), b"foreign");
                    }
                    true
                };
                let refused = match phase {
                    0 => successor::execute_successor(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        gate,
                    )
                    .is_err(),
                    1 => rotation::publication::publish_next_closure(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        gate,
                    )
                    .is_err(),
                    2 => rotation::exchange::exchange_next_closure(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        gate,
                    )
                    .is_err(),
                    _ => rotation::exchange::displaced::retire_displaced_closure(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        second(),
                        gate,
                    )
                    .is_err(),
                };
                assert!(refused, "phase {phase} gate {selected}");
                unchanged(&before);
            }
        }
    }

    #[test]
    fn disposition_ticket_static_direct_effect_gate_matrix() {
        // Retention guard, NOT substitute for phase/effect integration tests.
        for (name, source) in [
            ("stage", include_str!("restore_staging_candidate.rs")),
            ("executor", include_str!("restore_executor_candidate.rs")),
            (
                "retirement",
                include_str!("restore_retirement_candidate.rs"),
            ),
            (
                "slots",
                include_str!("restore_slot_retirement_candidate.rs"),
            ),
            ("cleanup", include_str!("restore_cleanup_candidate.rs")),
            (
                "successor",
                include_str!("restore_successor_executor_candidate.rs"),
            ),
            (
                "coexistence",
                include_str!("restore_successor_coexistence_candidate.rs"),
            ),
            (
                "preparation",
                include_str!("restore_successor_preparation_candidate.rs"),
            ),
            (
                "rotation",
                include_str!("restore_successor_rotation_candidate.rs"),
            ),
            ("next", include_str!("restore_successor_next_candidate.rs")),
        ] {
            assert!(
                source.contains("restore_disposition_ticket_model::"),
                "{name}"
            );
        }
    }
}
