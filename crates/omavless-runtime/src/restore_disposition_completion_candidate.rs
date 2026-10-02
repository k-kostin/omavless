// SPDX-License-Identifier: MIT
//! Inactive ordered completion of an authenticated disposition. A complete
//! record is still an existence fence until a separate historical reader and
//! normal-owner admission are reviewed. No entrypoint calls this candidate.
use super::*;
use crate::restore_disposition_complete_model::{COMPLETE_BYTES, COMPLETE_MEMBER, CompleteRecord};
use std::io::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompletionResult {
    CompletedStillFenced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionCheckpoint {
    Created,
    Written,
    FileSynced,
    ParentSynced,
    Reopened,
}

fn complete_absent(state: &File) -> Result<(), ExecutionError> {
    absent(state, COMPLETE_MEMBER)
}

fn complete_destination(
    state: &File,
    file: &File,
    expected: &Metadata,
    uid: u32,
    bytes: &[u8],
) -> Result<(), ExecutionError> {
    let reopened = File::from(
        openat(
            state,
            Path::new(COMPLETE_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| REFUSE)?,
    );
    let metadata = reopened.metadata().map_err(|_| REFUSE)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() != bytes.len() as u64
        || !same_member(expected, &metadata)
        || !same_member(expected, &file.metadata().map_err(|_| REFUSE)?)
    {
        return Err(REFUSE);
    }
    if bytes.is_empty() {
        return Ok(());
    }
    let (now, observed) = read_optional(state, COMPLETE_MEMBER, uid, COMPLETE_BYTES)
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
    if now.as_slice() != bytes || !same_member(expected, &observed) {
        return Err(REFUSE);
    }
    Ok(())
}

/// Retains the same pinned C1/live/ticket and continuous existing lease from
/// the complete resync through final publication. The caller supplies fresh
/// owner/Off/login/empty-host gates and a freshly authenticated archive.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn complete_disposition(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<CompletionResult, ExecutionError> {
    complete(config, paths, uid, generation, lock, backup, gate, |_| true)
}

#[allow(clippy::too_many_arguments)]
fn complete(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
    mut hook: impl FnMut(CompletionCheckpoint) -> bool,
) -> Result<CompletionResult, ExecutionError> {
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    complete_absent(&state)?;
    run_with_final(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        gate,
        |_| true,
        |context, ticket_file, ticket_identity, ticket_raw, ticket, gate| {
            let bytes = CompleteRecord::from_ticket(ticket).ok_or(REFUSE)?.encode();
            let mut verify = || {
                context.sources(&mut || true)?;
                context.destination(ticket_file, ticket_identity, ticket_raw)?;
                context.sources(&mut *gate)?;
                context.destination(ticket_file, ticket_identity, ticket_raw)?;
                complete_absent(&context.directories[0])
            };
            verify()?;
            let mut file = File::from(
                openat(
                    &context.directories[0],
                    Path::new(COMPLETE_MEMBER),
                    OFlag::O_WRONLY
                        | OFlag::O_CREAT
                        | OFlag::O_EXCL
                        | OFlag::O_NOFOLLOW
                        | OFlag::O_CLOEXEC,
                    Mode::from_bits_truncate(0o600),
                )
                .map_err(|_| REFUSE)?,
            );
            // After O_EXCL succeeds every error is ambiguous. Never remove a
            // prefix: both records remain visible conservative fences.
            let created = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
            let mut checkpoint = |point, identity: &Metadata, raw: &[u8], file: &File| {
                if !hook(point) {
                    return Err(ExecutionError::Ambiguous);
                }
                context
                    .sources(&mut || true)
                    .map_err(|_| ExecutionError::Ambiguous)?;
                context
                    .destination(ticket_file, ticket_identity, ticket_raw)
                    .map_err(|_| ExecutionError::Ambiguous)?;
                context
                    .sources(&mut *gate)
                    .map_err(|_| ExecutionError::Ambiguous)?;
                context
                    .destination(ticket_file, ticket_identity, ticket_raw)
                    .map_err(|_| ExecutionError::Ambiguous)?;
                complete_destination(&context.directories[0], file, identity, uid, raw)
                    .map_err(|_| ExecutionError::Ambiguous)
            };
            checkpoint(CompletionCheckpoint::Created, &created, &[], &file)?;
            file.write_all(&bytes)
                .map_err(|_| ExecutionError::Ambiguous)?;
            let written = file.metadata().map_err(|_| ExecutionError::Ambiguous)?;
            if created.dev() != written.dev() || created.ino() != written.ino() {
                return Err(ExecutionError::Ambiguous);
            }
            checkpoint(CompletionCheckpoint::Written, &written, &bytes, &file)?;
            file.sync_all().map_err(|_| ExecutionError::Ambiguous)?;
            checkpoint(CompletionCheckpoint::FileSynced, &written, &bytes, &file)?;
            context.directories[0]
                .sync_all()
                .map_err(|_| ExecutionError::Ambiguous)?;
            checkpoint(CompletionCheckpoint::ParentSynced, &written, &bytes, &file)?;
            checkpoint(CompletionCheckpoint::Reopened, &written, &bytes, &file)?;
            let parsed = CompleteRecord::decode(&bytes).ok_or(ExecutionError::Ambiguous)?;
            if !parsed.matches_ticket(ticket) {
                return Err(ExecutionError::Ambiguous);
            }
            Ok(CompletionResult::CompletedStillFenced)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::published;
    use super::*;
    use crate::restore_executor_candidate::successor::tests::second;
    use crate::restore_successor_publication_candidate::tests::Fixture;
    use std::{fs, os::unix::process::ExitStatusExt, path::PathBuf, process::Command};

    fn run_fixture(
        f: &Fixture,
        lock: &MigrationLock,
        hook: impl FnMut(CompletionCheckpoint) -> bool,
    ) -> Result<CompletionResult, ExecutionError> {
        complete(&f.config, &f.paths, f.uid, 2, lock, second(), || true, hook)
    }

    #[test]
    fn complete_disposition_commit_abort_preserves_both_fences() {
        for commit in [false, true] {
            let (f, lock) = published(commit);
            assert_eq!(
                run_fixture(&f, &lock, |_| true),
                Ok(CompletionResult::CompletedStillFenced)
            );
            let ticket_raw = fs::read(f.paths.state_directory.join(TICKET_MEMBER)).unwrap();
            let ticket = Ticket::decode(&ticket_raw).unwrap();
            let complete_raw = fs::read(f.paths.state_directory.join(COMPLETE_MEMBER)).unwrap();
            assert!(
                CompleteRecord::decode(&complete_raw)
                    .unwrap()
                    .matches_ticket(&ticket)
            );
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
            assert!(run_fixture(&f, &lock, |_| true).is_err());
            assert!(
                crate::login_transaction::check_startup_receipt(&f.paths, f.uid, &lock, Some(2))
                    .is_err()
            );
        }
    }

    #[test]
    fn complete_disposition_existing_unsafe_prefix_and_late_gate_refuse() {
        for prefix in [b"".as_slice(), b"OVRDON01partial".as_slice()] {
            let (f, lock) = published(true);
            let path = f.paths.state_directory.join(COMPLETE_MEMBER);
            fs::write(&path, prefix).unwrap();
            assert!(run_fixture(&f, &lock, |_| true).is_err());
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
        let (f, lock) = published(true);
        let refuse = std::cell::Cell::new(false);
        assert_eq!(
            complete(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                second(),
                || !refuse.get(),
                |step| {
                    if step == CompletionCheckpoint::ParentSynced {
                        refuse.set(true);
                    }
                    true
                }
            ),
            Err(ExecutionError::Ambiguous)
        );
        assert!(f.paths.state_directory.join(COMPLETE_MEMBER).exists());
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
    }

    #[test]
    fn complete_disposition_orphan_without_ticket_still_blocks_startup() {
        let (f, lock) = published(true);
        assert_eq!(
            run_fixture(&f, &lock, |_| true),
            Ok(CompletionResult::CompletedStillFenced)
        );
        fs::remove_file(f.paths.state_directory.join(TICKET_MEMBER)).unwrap();
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
        assert!(
            crate::login_transaction::check_startup_receipt(&f.paths, f.uid, &lock, Some(2))
                .is_err()
        );
    }

    #[test]
    fn complete_disposition_process_death_never_admits_or_overwrites() {
        for commit in [false, true] {
            for selected in 0..5 {
                let (f, lock) = published(commit);
                drop(lock);
                let output = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::tests::complete_disposition_crash_worker",
                    ])
                    .env("OMAVLESS_SYNTHETIC_COMPLETE_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_COMPLETE_POINT", selected.to_string())
                    .output()
                    .unwrap();
                assert_eq!(output.status.signal(), Some(9));
                let lock = f.lock();
                assert!(crate::pending_private_transaction::pending_at(
                    &f.paths.state_directory
                ));
                let path = f.paths.state_directory.join(COMPLETE_MEMBER);
                assert!(path.exists());
                assert!(run_fixture(&f, &lock, |_| true).is_err());
            }
        }
    }

    #[test]
    #[ignore = "internal synthetic complete disposition worker"]
    fn complete_disposition_crash_worker() {
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
            std::env::var_os("OMAVLESS_SYNTHETIC_COMPLETE_ROOT").unwrap(),
        )));
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_COMPLETE_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = run_fixture(&f, &lock, |_| {
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
