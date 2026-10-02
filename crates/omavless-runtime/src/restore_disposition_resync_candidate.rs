// SPDX-License-Identifier: MIT
//! Inactive existing-complete-ticket resync. Never completes historical
//! admission, repairs a partial ticket, unlinks a fence or starts an owner.
use super::*;

#[path = "restore_disposition_completion_candidate.rs"]
pub(crate) mod completion;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryResult {
    ResynchronizedStillFenced,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    SourceSynced(usize),
    DirectorySynced(usize),
    TicketSynced,
    ParentSynced,
    Reopened,
}

/// Authenticated NEW payload and retained caller Off/idle/login boundaries are
/// mandatory even for Abort. A complete record by itself grants no authority.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn resync_disposition(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<RecoveryResult, ExecutionError> {
    run(config, paths, uid, generation, lock, backup, gate, |_| true)
}

#[allow(clippy::too_many_arguments)]
fn run(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
    hook: impl FnMut(Checkpoint) -> bool,
) -> Result<RecoveryResult, ExecutionError> {
    run_with_final(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        gate,
        hook,
        |_, _, _, _, _, _| Ok(RecoveryResult::ResynchronizedStillFenced),
    )
}

#[allow(clippy::too_many_arguments)]
fn run_with_final<R, G, H, P>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    mut gate: G,
    mut hook: H,
    finalizer: P,
) -> Result<R, ExecutionError>
where
    G: FnMut() -> bool,
    H: FnMut(Checkpoint) -> bool,
    P: FnOnce(&Context<'_>, &File, &Metadata, &[u8], &Ticket, &mut G) -> Result<R, ExecutionError>,
{
    if !lock.authorizes(paths, uid) {
        return Err(REFUSE);
    }
    // Capture boundaries before either host callback, not after a callback
    // could substitute semantically identical owner/login/desired members.
    let source_boundary = boundary(paths, uid)?;
    let directories = [
        open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?,
        open_private_directory(config, uid).map_err(|_| REFUSE)?,
    ];
    let runtime = open_private_directory(&paths.runtime_base, uid).map_err(|_| REFUSE)?;
    absent(
        &directories[0],
        crate::restore_disposition_complete_model::COMPLETE_MEMBER,
    )?;
    let (raw, identity) = read_optional(&directories[0], TICKET_MEMBER, uid, TICKET_BYTES)
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
    let ticket = Ticket::decode(&raw).ok_or(REFUSE)?;
    let file = File::from(
        openat(
            &directories[0],
            Path::new(TICKET_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| REFUSE)?,
    );
    if !same_member(&identity, &file.metadata().map_err(|_| REFUSE)?) {
        return Err(REFUSE);
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let first = observe_final_sources(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    // Pin authenticated output evidence before the first external observation;
    // a semantically identical C1/live replacement in that callback must not
    // silently become the resynchronization baseline.
    if !gate() {
        return Err(REFUSE);
    }
    let second = observe_final_sources(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    if !first.same(&second)
        || second.phase != FinalPhase::HandoffAbsentReceiptAbsent
        || second.evidence.members.len() != publication::locations().len()
        || second.evidence.directories.len() != 2
    {
        return Err(REFUSE);
    }
    let closure = ClosureRecord::decode(&second.evidence.members[0].as_ref().ok_or(REFUSE)?.0)
        .map_err(|_| REFUSE)?;
    if !ticket.matches(
        &closure,
        uid,
        generation,
        second.evidence.desired.as_ref().map(|v| v.as_slice()),
    ) {
        return Err(REFUSE);
    }
    let context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        directories,
        runtime,
        boundary: source_boundary,
        evidence: FinalEvidence {
            snapshot: second,
            off,
            template: backup.template(),
        },
    };
    {
        let mut check = || {
            // Refuse stale original evidence before invoking the host observer,
            // then recheck after it; observing is not authority to rebind sources.
            context.sources(&mut || true)?;
            context.destination(&file, &identity, &raw)?;
            context.sources(&mut gate)?;
            context.destination(&file, &identity, &raw)?;
            absent(
                &context.directories[0],
                crate::restore_disposition_complete_model::COMPLETE_MEMBER,
            )
        };
        check()?;
        // No pre-existing durability inference: every entry and parent is freshly
        // synchronized from pinned descriptors before the final still-fenced result.
        for (index, directory, name) in [(0, 0, CLOSURE_MEMBER), (4, 1, LIVE[0]), (5, 1, LIVE[1])] {
            check()?;
            let source = File::from(
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
            if !same_member(expected, &source.metadata().map_err(|_| REFUSE)?) {
                return Err(REFUSE);
            }
            source.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
            if !hook(Checkpoint::SourceSynced(index)) {
                return Err(ExecutionError::Ambiguous);
            }
            check().map_err(|_| ExecutionError::Ambiguous)?;
        }
        for index in [1, 0, 2] {
            check()?;
            let directory = if index == 2 {
                &context.runtime
            } else {
                &context.directories[index]
            };
            directory
                .sync_all()
                .map_err(|_| ExecutionError::Ambiguous)?;
            if !hook(Checkpoint::DirectorySynced(index)) {
                return Err(ExecutionError::Ambiguous);
            }
            check().map_err(|_| ExecutionError::Ambiguous)?;
        }
        check()?;
        file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::TicketSynced) {
            return Err(ExecutionError::Ambiguous);
        }
        check().map_err(|_| ExecutionError::Ambiguous)?;
        context.directories[0]
            .sync_all()
            .map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::ParentSynced) {
            return Err(ExecutionError::Ambiguous);
        }
        check().map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(Checkpoint::Reopened) {
            return Err(ExecutionError::Ambiguous);
        }
        check().map_err(|_| ExecutionError::Ambiguous)?;
    }
    // The finalizer owns the last check. A host callback after publishing a
    // completion record could otherwise change it without a subsequent check.
    finalizer(&context, &file, &identity, &raw, &ticket, &mut gate)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{ready, snapshots, unchanged};
    use super::*;
    use crate::restore_executor_candidate::successor::tests::second;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    use std::{fs, path::PathBuf, process::Command};

    pub(super) fn published(commit: bool) -> (Fixture, MigrationLock) {
        let (f, lock) = ready(commit);
        publish_disposition(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        (f, lock)
    }
    fn recover(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(Checkpoint) -> bool,
    ) -> Result<RecoveryResult, ExecutionError> {
        run(&f.config, &f.paths, f.uid, 2, lock, second(), || true, hook)
    }
    fn all(f: &Fixture) -> Vec<(PathBuf, Vec<u8>, Metadata)> {
        let mut result = snapshots(f);
        let path = f.paths.state_directory.join(TICKET_MEMBER);
        result.push((
            path.clone(),
            fs::read(&path).unwrap(),
            fs::metadata(path).unwrap(),
        ));
        result
    }
    fn member(path: &Path, raw: &[u8]) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn disposition_resync_complete_commit_abort_is_repeatable_and_never_admits() {
        for commit in [false, true] {
            let (f, lock) = published(commit);
            let before = all(&f);
            for _ in 0..2 {
                assert_eq!(
                    recover(&f, &lock, |_| true),
                    Ok(RecoveryResult::ResynchronizedStillFenced)
                );
                unchanged(&before);
                assert!(crate::pending_private_transaction::pending_at(
                    &f.paths.state_directory
                ));
                assert!(
                    review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
                        .is_err()
                );
                assert!(
                    publish_disposition(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
                        .is_err()
                );
            }
            assert!(
                resync_disposition(&f.config, &f.paths, f.uid, 2, &lock, backup(), || true)
                    .is_err()
            );
            unchanged(&before);
        }
    }
    #[test]
    fn disposition_resync_missing_partial_unsafe_foreign_and_wrong_owner_refuse_without_effects() {
        for kind in 0..8 {
            let (f, lock) = published(true);
            let before = snapshots(&f);
            let path = f.paths.state_directory.join(TICKET_MEMBER);
            fs::remove_file(&path).unwrap();
            match kind {
                0 => (),
                1 => member(&path, b""),
                2 => member(&path, b"OVRDSP01partial"),
                3 => fs::create_dir(&path).unwrap(),
                4 => std::os::unix::fs::symlink("missing", &path).unwrap(),
                5 => fs::hard_link(f.paths.state_directory.join(CLOSURE_MEMBER), &path).unwrap(),
                6 => {
                    let (other, _lease) = published(false);
                    member(
                        &path,
                        &fs::read(other.paths.state_directory.join(TICKET_MEMBER)).unwrap(),
                    );
                }
                _ => {
                    member(&path, b"unsafe");
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                }
            }
            assert!(recover(&f, &lock, |_| true).is_err());
            // The hardlink changes C1 nlink/ctime; remove only this fixture
            // link before comparing payload, never claim prior inode metadata.
            if kind != 5 {
                unchanged(&before);
            } else {
                assert_eq!(fs::read(&before[0].0).unwrap(), before[0].1);
            }
        }
        let (f, lock) = published(true);
        let before = all(&f);
        assert!(
            resync_disposition(&f.config, &f.paths, f.uid, 3, &lock, second(), || true).is_err()
        );
        fs::remove_file(&f.paths.operation_lock).unwrap();
        let _other = f.lock();
        assert!(recover(&f, &lock, |_| true).is_err());
        unchanged(&before);
    }
    #[test]
    fn disposition_resync_every_effect_detects_ticket_source_or_pending_substitution() {
        for selected in 0..9 {
            for kind in 0..3 {
                let (f, lock) = published(true);
                let mut index = 0;
                assert!(
                    recover(&f, &lock, |_| {
                        if index == selected {
                            let path = f.paths.state_directory.join(if kind == 0 {
                                TICKET_MEMBER
                            } else if kind == 1 {
                                CLOSURE_MEMBER
                            } else {
                                INTENT
                            });
                            if kind == 2 {
                                member(&path, b"foreign");
                            } else {
                                let raw = fs::read(&path).unwrap();
                                fs::rename(&path, path.with_extension("old-test")).unwrap();
                                member(&path, &raw);
                            }
                        }
                        index += 1;
                        true
                    })
                    .is_err()
                );
                assert!(f.paths.state_directory.join(TICKET_MEMBER).exists());
            }
        }
    }
    #[test]
    fn disposition_resync_admission_boundaries_and_last_gate_are_pinned() {
        for kind in 0..4 {
            let (f, lock) = published(true);
            let mut calls = 0;
            let result = run(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                || {
                    calls += 1;
                    if calls == 2 {
                        if kind == 3 {
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
                        } else {
                            let path = if kind == 0 {
                                f.paths.ownership_marker.clone()
                            } else if kind == 1 {
                                f.paths.state_directory.join(TICKET_MEMBER)
                            } else {
                                f.paths.runtime_base.join("omavless-login.receipt")
                            };
                            if kind == 2 {
                                member(&path, b"foreign");
                            } else {
                                let raw = fs::read(&path).unwrap();
                                fs::rename(&path, path.with_extension("old-test")).unwrap();
                                member(&path, &raw);
                            }
                        }
                    }
                    true
                },
                |_| true,
            );
            assert!(result.is_err());
        }
        let (f, lock) = published(true);
        let before = all(&f);
        let refuse = std::cell::Cell::new(false);
        assert_eq!(
            run(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                || !refuse.get(),
                |step| {
                    if step == Checkpoint::Reopened {
                        refuse.set(true);
                    }
                    true
                }
            ),
            Err(ExecutionError::Ambiguous)
        );
        unchanged(&before);
    }
    #[test]
    fn disposition_resync_actual_writer_and_resync_deaths_reopen_without_unfencing() {
        for commit in [false, true] {
            for selected in [1, 3] {
                let (f, lock) = ready(commit);
                drop(lock);
                let output = Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored", "--exact", "restore_executor_candidate::successor::rotation::final_review::disposition::tests::disposition_ticket_crash_worker"])
                    .env("OMAVLESS_SYNTHETIC_TICKET_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_TICKET_POINT", selected.to_string()).output().unwrap();
                assert_eq!(output.status.signal(), Some(9));
                let before = all(&f);
                let lock = f.lock();
                assert_eq!(
                    recover(&f, &lock, |_| true),
                    Ok(RecoveryResult::ResynchronizedStillFenced)
                );
                unchanged(&before);
            }
            for selected in 0..9 {
                let (f, lock) = published(commit);
                let before = all(&f);
                drop(lock);
                let output = Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored", "--exact", "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::tests::disposition_resync_crash_worker"])
                    .env("OMAVLESS_SYNTHETIC_TICKET_RESYNC_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_TICKET_RESYNC_POINT", selected.to_string()).output().unwrap();
                assert_eq!(
                    output.status.signal(),
                    Some(9),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let lock = f.lock();
                assert_eq!(
                    recover(&f, &lock, |_| true),
                    Ok(RecoveryResult::ResynchronizedStillFenced)
                );
                assert!(crate::pending_private_transaction::pending_at(
                    &f.paths.state_directory
                ));
                unchanged(&before);
            }
        }
    }
    #[test]
    #[ignore = "internal synthetic ticket resync worker"]
    fn disposition_resync_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_TICKET_RESYNC_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_TICKET_RESYNC_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = recover(&f, &lock, |_| {
            if index == selected {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            index += 1;
            true
        });
        panic!("expected synthetic kill");
    }
}
