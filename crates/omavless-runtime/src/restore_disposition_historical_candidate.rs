// SPDX-License-Identifier: MIT
//! Inactive archive-free historical observation. This only verifies that the
//! private records agree across fresh observations; it proves neither prior fsync
//! completion nor ordinary startup authority. All records remain fences.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HistoricalReview {
    ConsistentStillFenced,
}

struct Snapshot {
    directories: [Metadata; 3],
    directory_handles: [File; 3],
    boundary: BoundaryMembers,
    members: [(Zeroizing<Vec<u8>>, Metadata); 5],
    member_handles: [File; 5],
}

fn pin_member(directory: &File, name: &str, expected: &Metadata) -> Result<File, ExecutionError> {
    let file = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| REFUSE)?,
    );
    if !same_member(expected, &file.metadata().map_err(|_| REFUSE)?) {
        return Err(REFUSE);
    }
    Ok(file)
}

fn no_other_transients(state: &File, config: &File) -> Result<(), ExecutionError> {
    for name in [
        NEXT_CLOSURE_MEMBER,
        SUCCESSOR_MEMBER,
        RECEIPT_MEMBER,
        PENDING_DIRECTORY,
        INTENT,
        TERMINAL,
        "routing-preset.pending.json",
    ] {
        absent(state, name)?;
    }
    for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
        absent(config, name)?;
    }
    Ok(())
}

fn same_boundary(a: &BoundaryMembers, b: &BoundaryMembers) -> bool {
    a.iter().zip(b).all(|(a, b)| match (a, b) {
        (None, None) => true,
        (Some((av, am)), Some((bv, bm))) => av == bv && same_member(am, bm),
        _ => false,
    })
}

impl Snapshot {
    fn read(
        config: &Path,
        paths: &CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &MigrationLock,
    ) -> Result<Self, ExecutionError> {
        if !lock.authorizes(paths, uid) {
            return Err(REFUSE);
        }
        let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
        let config_dir = open_private_directory(config, uid).map_err(|_| REFUSE)?;
        let runtime = open_private_directory(&paths.runtime_base, uid).map_err(|_| REFUSE)?;
        let marker = read_marker_existing(paths, uid).map_err(|_| REFUSE)?;
        if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
            return Err(REFUSE);
        }
        let source_boundary = boundary(paths, uid)?;
        no_other_transients(&state, &config_dir)?;
        let closure = read_optional(&state, CLOSURE_MEMBER, uid, CLOSURE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let ticket = read_optional(&state, TICKET_MEMBER, uid, TICKET_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let complete = read_optional(&state, COMPLETE_MEMBER, uid, COMPLETE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let store = read_optional(&config_dir, LIVE[0], uid, MAX_PRIVATE_STORE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let template = read_optional(&config_dir, LIVE[1], uid, MAX_TEMPLATE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        let canonical = ClosureRecord::decode(&closure.0).map_err(|_| REFUSE)?;
        let ticket_record = Ticket::decode(&ticket.0).ok_or(REFUSE)?;
        let complete_record = CompleteRecord::decode(&complete.0).ok_or(REFUSE)?;
        if !ticket_record.matches(
            &canonical,
            uid,
            generation,
            source_boundary[1]
                .as_ref()
                .map(|(bytes, _)| bytes.as_slice()),
        ) || !complete_record.matches_ticket(&ticket_record)
            || !canonical.receipt().matches_pair(&store.0, &template.0)
        {
            return Err(REFUSE);
        }
        no_other_transients(&state, &config_dir)?;
        if !lock.authorizes(paths, uid) {
            return Err(REFUSE);
        }
        if !same_boundary(&source_boundary, &boundary(paths, uid)?) {
            return Err(REFUSE);
        }
        for (opened, path) in [
            (&state, paths.state_directory.as_path()),
            (&config_dir, config),
            (&runtime, paths.runtime_base.as_path()),
        ] {
            let current = open_private_directory(path, uid).map_err(|_| REFUSE)?;
            if !same_directory(
                &opened.metadata().map_err(|_| REFUSE)?,
                &current.metadata().map_err(|_| REFUSE)?,
            ) {
                return Err(REFUSE);
            }
        }
        let members = [closure, ticket, complete, store, template];
        let member_handles = [
            pin_member(&state, CLOSURE_MEMBER, &members[0].1)?,
            pin_member(&state, TICKET_MEMBER, &members[1].1)?,
            pin_member(&state, COMPLETE_MEMBER, &members[2].1)?,
            pin_member(&config_dir, LIVE[0], &members[3].1)?,
            pin_member(&config_dir, LIVE[1], &members[4].1)?,
        ];
        let snapshot = Self {
            directories: [
                state.metadata().map_err(|_| REFUSE)?,
                config_dir.metadata().map_err(|_| REFUSE)?,
                runtime.metadata().map_err(|_| REFUSE)?,
            ],
            directory_handles: [state, config_dir, runtime],
            boundary: source_boundary,
            members,
            member_handles,
        };
        if !snapshot.pins_intact() {
            return Err(REFUSE);
        }
        Ok(snapshot)
    }

    fn pins_intact(&self) -> bool {
        self.directories
            .iter()
            .zip(&self.directory_handles)
            .all(|(expected, file)| {
                file.metadata()
                    .is_ok_and(|now| same_directory(expected, &now))
            })
            && self
                .members
                .iter()
                .zip(&self.member_handles)
                .all(|((_, expected), file)| {
                    file.metadata().is_ok_and(|now| same_member(expected, &now))
                })
    }

    fn same(&self, other: &Self) -> bool {
        self.pins_intact()
            && other.pins_intact()
            && self
                .directories
                .iter()
                .zip(&other.directories)
                .all(|(a, b)| same_directory(a, b))
            && same_boundary(&self.boundary, &other.boundary)
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|((a, am), (b, bm))| a == b && same_member(am, bm))
    }
}

/// The caller holds an existing exclusive migration lease and supplies a
/// fresh owner/Off/login/empty-host gate. A current boot epoch, live semantic
/// validity and durability resync are separate obligations; none is inferred
/// by this read-only result. No normal owner consumes it.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn review_historical(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: impl FnMut() -> bool,
) -> Result<HistoricalReview, ExecutionError> {
    let first = Snapshot::read(config, paths, uid, generation, lock)?;
    if !gate() {
        return Err(REFUSE);
    }
    let second = Snapshot::read(config, paths, uid, generation, lock)?;
    if !first.same(&second) || !gate() {
        return Err(REFUSE);
    }
    let last = Snapshot::read(config, paths, uid, generation, lock)?;
    if !first.same(&last) {
        return Err(REFUSE);
    }
    Ok(HistoricalReview::ConsistentStillFenced)
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::published;
    use super::*;
    use crate::restore_executor_candidate::successor::tests::second;
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, path::PathBuf, process::Command};

    fn prepared(
        commit: bool,
    ) -> (
        crate::restore_successor_publication_candidate::tests::Fixture,
        MigrationLock,
    ) {
        let (f, lock) = published(commit);
        complete_disposition(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        (f, lock)
    }

    #[test]
    fn historical_read_is_archive_free_read_only_and_still_fenced() {
        for commit in [false, true] {
            let (f, lock) = prepared(commit);
            let names = [CLOSURE_MEMBER, TICKET_MEMBER, COMPLETE_MEMBER];
            let before: Vec<_> = names
                .iter()
                .map(|name| {
                    let path = f.paths.state_directory.join(name);
                    (fs::read(&path).unwrap(), fs::metadata(&path).unwrap())
                })
                .collect();
            assert_eq!(
                review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
                Ok(HistoricalReview::ConsistentStillFenced)
            );
            for (name, (bytes, metadata)) in names.iter().zip(&before) {
                let path = f.paths.state_directory.join(name);
                assert_eq!(&fs::read(&path).unwrap(), bytes);
                assert!(same_member(metadata, &fs::metadata(path).unwrap()));
            }
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
    }

    #[test]
    fn historical_read_refuses_orphans_torn_records_and_late_transient() {
        let (f, lock) = prepared(true);
        fs::remove_file(f.paths.state_directory.join(TICKET_MEMBER)).unwrap();
        assert!(review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
        let (f, lock) = prepared(true);
        fs::write(f.paths.state_directory.join(COMPLETE_MEMBER), b"partial").unwrap();
        assert!(review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
        let (f, lock) = prepared(true);
        let mut calls = 0;
        assert!(
            review_historical(&f.config, &f.paths, f.uid, 2, &lock, || {
                calls += 1;
                if calls == 1 {
                    fs::write(f.paths.state_directory.join(INTENT), b"late").unwrap();
                }
                true
            })
            .is_err()
        );
    }

    #[test]
    fn historical_read_refuses_live_bytes_outside_terminal_pair() {
        for name in LIVE {
            let (f, lock) = prepared(true);
            let path = f.config.join(name);
            let mut bytes = fs::read(&path).unwrap();
            bytes[0] ^= 1;
            fs::write(path, bytes).unwrap();
            assert!(review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
        }
    }

    #[test]
    fn historical_read_refuses_crossed_records_generation_and_each_transient() {
        let (f, lock) = prepared(true);
        assert!(review_historical(&f.config, &f.paths, f.uid, 3, &lock, || true).is_err());
        let (other, _) = prepared(false);
        fs::write(
            f.paths.state_directory.join(COMPLETE_MEMBER),
            fs::read(other.paths.state_directory.join(COMPLETE_MEMBER)).unwrap(),
        )
        .unwrap();
        assert!(review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
        for (config_member, name) in [
            (false, NEXT_CLOSURE_MEMBER),
            (false, SUCCESSOR_MEMBER),
            (false, RECEIPT_MEMBER),
            (false, PENDING_DIRECTORY),
            (false, INTENT),
            (false, TERMINAL),
            (false, "routing-preset.pending.json"),
            (true, NEW_SLOT[0]),
            (true, NEW_SLOT[1]),
            (true, OLD_SLOT[0]),
            (true, OLD_SLOT[1]),
        ] {
            let (f, lock) = prepared(true);
            let root = if config_member {
                &f.config
            } else {
                &f.paths.state_directory
            };
            fs::write(root.join(name), b"unexpected").unwrap();
            assert!(
                review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn historical_read_refuses_same_byte_replacement_in_each_host_callback() {
        for callback in [1, 2] {
            let (f, lock) = prepared(true);
            let path = f.paths.state_directory.join(COMPLETE_MEMBER);
            let bytes = fs::read(&path).unwrap();
            let mut calls = 0;
            assert!(
                review_historical(&f.config, &f.paths, f.uid, 2, &lock, || {
                    calls += 1;
                    if calls == callback {
                        fs::rename(&path, path.with_extension("replaced-test")).unwrap();
                        fs::write(&path, &bytes).unwrap();
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                    }
                    true
                })
                .is_err()
            );
        }
    }

    #[test]
    fn historical_read_process_reentry_needs_no_archive_and_remains_fenced() {
        for commit in [false, true] {
            let (f, lock) = prepared(commit);
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::tests::historical_read_process_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_HISTORICAL_ROOT", &f.root)
                .output()
                .unwrap();
            assert!(output.status.success());
            let lock = f.lock();
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
            assert_eq!(
                review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
                Ok(HistoricalReview::ConsistentStillFenced)
            );
        }
    }

    #[test]
    fn historical_read_refuses_late_live_boundary_directory_and_lease_changes() {
        for callback in [1, 2] {
            for kind in 0..4 {
                let (f, lock) = prepared(true);
                let mut calls = 0;
                assert!(
                    review_historical(&f.config, &f.paths, f.uid, 2, &lock, || {
                        calls += 1;
                        if calls == callback {
                            match kind {
                                0 => {
                                    let path = f.config.join(LIVE[0]);
                                    let bytes = fs::read(&path).unwrap();
                                    fs::rename(&path, path.with_extension("replaced-test"))
                                        .unwrap();
                                    fs::write(&path, bytes).unwrap();
                                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                                        .unwrap();
                                }
                                1 => {
                                    let path = f
                                        .paths
                                        .state_directory
                                        .join(crate::cutover::OWNERSHIP_MARKER_NAME);
                                    let bytes = fs::read(&path).unwrap();
                                    fs::rename(&path, path.with_extension("replaced-test"))
                                        .unwrap();
                                    fs::write(&path, bytes).unwrap();
                                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                                        .unwrap();
                                }
                                2 => {
                                    let old = f.config.with_extension("replaced-test");
                                    fs::rename(&f.config, old).unwrap();
                                    fs::create_dir(&f.config).unwrap();
                                    fs::set_permissions(
                                        &f.config,
                                        fs::Permissions::from_mode(0o700),
                                    )
                                    .unwrap();
                                }
                                _ => {
                                    fs::remove_file(&f.paths.operation_lock).unwrap();
                                }
                            }
                        }
                        true
                    })
                    .is_err(),
                    "callback={callback} kind={kind}"
                );
            }
        }
    }

    #[test]
    #[ignore = "internal synthetic historical reentry worker"]
    fn historical_read_process_worker() {
        let f = std::mem::ManuallyDrop::new(
            crate::restore_successor_publication_candidate::tests::Fixture::reopen(PathBuf::from(
                std::env::var_os("OMAVLESS_SYNTHETIC_HISTORICAL_ROOT").unwrap(),
            )),
        );
        let lock = f.lock();
        assert_eq!(
            review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
            Ok(HistoricalReview::ConsistentStillFenced)
        );
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
    }
}
