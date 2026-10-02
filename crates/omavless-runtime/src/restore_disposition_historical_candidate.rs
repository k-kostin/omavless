// SPDX-License-Identifier: MIT
//! Inactive archive-free historical observation. This only verifies that the
//! private records agree across fresh observations; it proves neither prior fsync
//! completion nor ordinary startup authority. Current-state semantic review is
//! separate from the original terminal output. All records remain fences.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HistoricalReview {
    ConsistentStillFenced,
}

#[derive(Clone, Copy)]
enum LivePolicy {
    InitialOutput,
    ValidCurrentBundled,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CurrentLiveReview {
    ValidHistoricalAndCurrentLiveStillFenced,
}

struct Snapshot {
    directories: [Metadata; 3],
    directory_handles: [File; 3],
    boundary: BoundaryMembers,
    boundary_handles: [Option<File>; 3],
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
        Self::read_policy(
            config,
            paths,
            uid,
            generation,
            lock,
            LivePolicy::InitialOutput,
        )
    }

    fn read_policy(
        config: &Path,
        paths: &CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &MigrationLock,
        policy: LivePolicy,
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
        let live_matches = match policy {
            LivePolicy::InitialOutput => {
                ticket_record.matches(
                    &canonical,
                    uid,
                    generation,
                    source_boundary[1]
                        .as_ref()
                        .map(|(bytes, _)| bytes.as_slice()),
                ) && canonical.receipt().matches_pair(&store.0, &template.0)
            }
            LivePolicy::ValidCurrentBundled => {
                ticket_record.matches_history(&canonical, uid, generation)
                    && valid_current_bundled(
                        &store.0,
                        &template.0,
                        source_boundary[1].as_ref().map(|(v, _)| v.as_slice()),
                    )
            }
        };
        if !live_matches || !complete_record.matches_ticket(&ticket_record) {
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
        let boundary_handles = [
            source_boundary[0]
                .as_ref()
                .map(|(_, metadata)| {
                    pin_member(&state, crate::cutover::OWNERSHIP_MARKER_NAME, metadata)
                })
                .transpose()?,
            source_boundary[1]
                .as_ref()
                .map(|(_, metadata)| pin_member(&state, "desired.json", metadata))
                .transpose()?,
            source_boundary[2]
                .as_ref()
                .map(|(_, metadata)| pin_member(&runtime, "omavless-login.receipt", metadata))
                .transpose()?,
        ];
        let snapshot = Self {
            directories: [
                state.metadata().map_err(|_| REFUSE)?,
                config_dir.metadata().map_err(|_| REFUSE)?,
                runtime.metadata().map_err(|_| REFUSE)?,
            ],
            directory_handles: [state, config_dir, runtime],
            boundary: source_boundary,
            boundary_handles,
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
                .boundary
                .iter()
                .zip(&self.boundary_handles)
                .all(|(member, file)| match (member, file) {
                    (None, None) => true,
                    (Some((_, expected)), Some(file)) => {
                        file.metadata().is_ok_and(|now| same_member(expected, &now))
                    }
                    _ => false,
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

/// Ordinary complete-store semantics, separately from historical digests.
/// This deliberately recognizes only exact bundled templates; custom-template
/// and installed-core validation remain a later product admission obligation.
fn valid_current_bundled(store: &[u8], template: &[u8], desired: Option<&[u8]>) -> bool {
    if omavless_domain::private_backup::validate_bundled_data(store, template).is_err() {
        return false;
    }
    let Ok(text) = std::str::from_utf8(store) else {
        return false;
    };
    let Ok(current) = omavless_domain::private_store::parse_private_store(text) else {
        return false;
    };
    let desired = match desired {
        None => crate::desired::DesiredState::default(),
        Some(raw) => match serde_json::from_slice::<crate::desired::DesiredState>(raw) {
            Ok(value) => value,
            Err(_) => return false,
        },
    };
    if desired.validate().is_err() {
        return false;
    }
    !desired.connected
        || current
            .list_projection()
            .profiles()
            .iter()
            .any(|profile| profile.id() == desired.profile_id)
}

/// Read-only candidate for the same UID/exact ownership generation after
/// legitimate ordinary edits. No historical output equality is inferred.
/// Pins all records and current live/desired/login members over two host gates.
/// Still no durability, current-epoch proof, startup exception or owner permit.
#[allow(dead_code)]
pub(crate) fn review_current_live(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: impl FnMut() -> bool,
) -> Result<CurrentLiveReview, ExecutionError> {
    let read = || {
        Snapshot::read_policy(
            config,
            paths,
            uid,
            generation,
            lock,
            LivePolicy::ValidCurrentBundled,
        )
    };
    let first = read()?;
    if !gate() {
        return Err(REFUSE);
    }
    let second = read()?;
    if !first.same(&second) || !gate() {
        return Err(REFUSE);
    }
    if !first.same(&read()?) {
        return Err(REFUSE);
    }
    Ok(CurrentLiveReview::ValidHistoricalAndCurrentLiveStillFenced)
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HistoricalResyncResult {
    ResynchronizedStillFenced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoricalSyncCheckpoint {
    File(usize),
    Boundary(usize),
    Directory(usize),
    Final,
}

fn recheck(
    original: &Snapshot,
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: &mut impl FnMut() -> bool,
) -> Result<(), ExecutionError> {
    if !original.pins_intact() {
        return Err(REFUSE);
    }
    let before = Snapshot::read(config, paths, uid, generation, lock)?;
    if !original.same(&before) || !gate() {
        return Err(REFUSE);
    }
    let after = Snapshot::read(config, paths, uid, generation, lock)?;
    if !original.same(&after) {
        return Err(REFUSE);
    }
    Ok(())
}

/// Inactive archive-free durability resync of an already complete, mutually
/// bound disposition. This retains one existing lease and original FDs through
/// all syncs; it neither repairs records nor grants normal owner admission.
/// The supplied host/Off/login gate and trusted path derivation remain caller
/// obligations. No production caller exists.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn resync_historical(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<HistoricalResyncResult, ExecutionError> {
    run_resync(config, paths, uid, generation, lock, gate, |_| true)
}

#[allow(clippy::too_many_arguments)]
fn run_resync(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
    hook: impl FnMut(HistoricalSyncCheckpoint) -> bool,
) -> Result<HistoricalResyncResult, ExecutionError> {
    run_resync_with_sync(
        config,
        paths,
        uid,
        generation,
        lock,
        gate,
        hook,
        |_, file| file.sync_all(),
    )
}

#[allow(clippy::too_many_arguments)]
fn run_resync_with_sync(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(HistoricalSyncCheckpoint) -> bool,
    mut sync: impl FnMut(HistoricalSyncCheckpoint, &File) -> std::io::Result<()>,
) -> Result<HistoricalResyncResult, ExecutionError> {
    // Capture the original source and path identities before the first host
    // callback. A later observation can reject drift, never redefine source.
    let original = Snapshot::read(config, paths, uid, generation, lock)?;
    let mut check = || recheck(&original, config, paths, uid, generation, lock, &mut gate);
    check()?;
    for (index, file) in original.member_handles.iter().enumerate() {
        sync(HistoricalSyncCheckpoint::File(index), file).map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(HistoricalSyncCheckpoint::File(index)) {
            return Err(ExecutionError::Ambiguous);
        }
        check().map_err(|_| ExecutionError::Ambiguous)?;
    }
    for (index, file) in original.boundary_handles.iter().enumerate() {
        if let Some(file) = file {
            sync(HistoricalSyncCheckpoint::Boundary(index), file)
                .map_err(|_| ExecutionError::Ambiguous)?;
            if !hook(HistoricalSyncCheckpoint::Boundary(index)) {
                return Err(ExecutionError::Ambiguous);
            }
            check().map_err(|_| ExecutionError::Ambiguous)?;
        }
    }
    for (index, directory) in original.directory_handles.iter().enumerate() {
        sync(HistoricalSyncCheckpoint::Directory(index), directory)
            .map_err(|_| ExecutionError::Ambiguous)?;
        if !hook(HistoricalSyncCheckpoint::Directory(index)) {
            return Err(ExecutionError::Ambiguous);
        }
        check().map_err(|_| ExecutionError::Ambiguous)?;
    }
    if !hook(HistoricalSyncCheckpoint::Final) {
        return Err(ExecutionError::Ambiguous);
    }
    check().map_err(|_| ExecutionError::Ambiguous)?;
    Ok(HistoricalResyncResult::ResynchronizedStillFenced)
}

#[cfg(test)]
mod tests {
    use super::super::super::tests::published;
    use super::*;
    use crate::restore_executor_candidate::successor::tests::second;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
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

    fn ordinary_edit(f: &crate::restore_successor_publication_candidate::tests::Fixture) {
        let path = f.config.join(LIVE[0]);
        let mut store: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let was = store["onboardingComplete"].as_bool().unwrap_or(false);
        store["onboardingComplete"] = (!was).into();
        fs::write(path, serde_json::to_vec(&store).unwrap()).unwrap();
        let template = omavless_domain::routing::template_with_mode(
            include_str!("../../../templates/default.yaml"),
            "direct",
        )
        .unwrap();
        fs::write(f.config.join(LIVE[1]), template).unwrap();
        let desired = crate::desired::DesiredState {
            generation: 19,
            mode: crate::desired::RoutingMode::Direct,
            ..crate::desired::DesiredState::default()
        };
        let path = f.paths.state_directory.join("desired.json");
        fs::write(&path, serde_json::to_vec(&desired).unwrap()).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn current_live_accepts_valid_ordinary_edits_without_rebinding_history() {
        for commit in [false, true] {
            let (f, lock) = prepared(commit);
            let history: Vec<_> = [CLOSURE_MEMBER, TICKET_MEMBER, COMPLETE_MEMBER]
                .iter()
                .map(|name| fs::read(f.paths.state_directory.join(name)).unwrap())
                .collect();
            ordinary_edit(&f);
            assert!(review_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
            for _ in 0..2 {
                assert_eq!(
                    review_current_live(&f.config, &f.paths, f.uid, 2, &lock, || true),
                    Ok(CurrentLiveReview::ValidHistoricalAndCurrentLiveStillFenced)
                );
            }
            for (name, bytes) in [CLOSURE_MEMBER, TICKET_MEMBER, COMPLETE_MEMBER]
                .iter()
                .zip(history)
            {
                assert_eq!(fs::read(f.paths.state_directory.join(name)).unwrap(), bytes);
            }
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
    }

    #[test]
    fn current_live_requires_independent_store_template_and_desired_semantics() {
        for kind in 0..9 {
            let (f, lock) = prepared(true);
            ordinary_edit(&f);
            match kind {
                0 => fs::write(f.config.join(LIVE[0]), b"{}").unwrap(),
                1 => fs::write(f.config.join(LIVE[1]), b"mode: direct\n").unwrap(),
                2 => fs::write(f.paths.state_directory.join("desired.json"), b"{}").unwrap(),
                3 => fs::write(f.paths.state_directory.join("desired.json"),
                    br#"{"schemaVersion":1,"generation":1,"connected":true,"profileId":"missing","mode":"direct"}"#).unwrap(),
                4 => fs::write(f.paths.state_directory.join("desired.json"),
                    br#"{"schemaVersion":1,"generation":1,"generation":2,"connected":false,"profileId":"","mode":"direct"}"#).unwrap(),
                5 => {
                    let path = f.config.join(LIVE[0]);
                    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    value["version"] = 4.into();
                    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
                }
                6 => {
                    let path = f.config.join(LIVE[0]);
                    let text = fs::read_to_string(&path).unwrap();
                    fs::write(path, text.replacen("\"version\":3", "\"version\":3,\"version\":3", 1)).unwrap();
                }
                7 => {
                    let path = f.config.join(LIVE[0]);
                    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    value["activeId"] = "10000000-0000-4000-8000-000000000099".into();
                    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
                }
                _ => {
                    let path = f.config.join(LIVE[0]);
                    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    value["unexpectedPrivateState"] = "synthetic-private".into();
                    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
                }
            }
            assert!(
                review_current_live(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err(),
                "kind={kind}"
            );
        }
    }

    #[test]
    fn current_live_never_converts_missing_history_or_new_generation_into_admission() {
        let (f, lock) = prepared(true);
        ordinary_edit(&f);
        assert!(review_current_live(&f.config, &f.paths, f.uid, 3, &lock, || true).is_err());
        assert!(
            review_current_live(&f.config, &f.paths, f.uid.wrapping_add(1), 2, &lock, || {
                true
            })
            .is_err()
        );
        assert!(review_current_live(&f.config, &f.paths, f.uid, 2, &lock, || false).is_err());
        fs::remove_file(f.paths.state_directory.join(COMPLETE_MEMBER)).unwrap();
        assert!(review_current_live(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
    }

    #[test]
    fn current_live_refuses_late_valid_edits_transients_and_same_byte_inode_replacement() {
        for callback in [1, 2] {
            for kind in 0..3 {
                let (f, lock) = prepared(true);
                ordinary_edit(&f);
                let mut calls = 0;
                let result = review_current_live(&f.config, &f.paths, f.uid, 2, &lock, || {
                    calls += 1;
                    if calls == callback {
                        match kind {
                            0 => {
                                let path = f.paths.state_directory.join("desired.json");
                                let mut value: serde_json::Value =
                                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                                value["generation"] = 20.into();
                                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
                            }
                            1 => fs::write(f.paths.state_directory.join(INTENT), b"late").unwrap(),
                            _ => {
                                let path = f.config.join(LIVE[0]);
                                let bytes = fs::read(&path).unwrap();
                                fs::rename(&path, path.with_extension("old-inode-test")).unwrap();
                                fs::write(&path, bytes).unwrap();
                                fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                                    .unwrap();
                            }
                        }
                    }
                    true
                });
                assert!(result.is_err(), "callback={callback} kind={kind}");
            }
        }
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
    fn historical_resync_commit_abort_is_repeatable_archive_free_and_still_fenced() {
        for commit in [false, true] {
            let (f, lock) = prepared(commit);
            let paths = [
                f.paths.state_directory.join(CLOSURE_MEMBER),
                f.paths.state_directory.join(TICKET_MEMBER),
                f.paths.state_directory.join(COMPLETE_MEMBER),
                f.config.join(LIVE[0]),
                f.config.join(LIVE[1]),
            ];
            let before: Vec<_> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
            for _ in 0..2 {
                assert_eq!(
                    resync_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
                    Ok(HistoricalResyncResult::ResynchronizedStillFenced)
                );
                for (path, bytes) in paths.iter().zip(&before) {
                    assert_eq!(&fs::read(path).unwrap(), bytes);
                }
                assert!(crate::pending_private_transaction::pending_at(
                    &f.paths.state_directory
                ));
            }
        }
    }

    #[test]
    fn historical_resync_refuses_missing_crossed_tampered_and_stale_sources() {
        for kind in 0..5 {
            let (f, lock) = prepared(true);
            match kind {
                0 => fs::remove_file(f.paths.state_directory.join(COMPLETE_MEMBER)).unwrap(),
                1 => fs::write(f.paths.state_directory.join(COMPLETE_MEMBER), b"partial").unwrap(),
                2 => {
                    let (other, _) = prepared(false);
                    fs::write(
                        f.paths.state_directory.join(COMPLETE_MEMBER),
                        fs::read(other.paths.state_directory.join(COMPLETE_MEMBER)).unwrap(),
                    )
                    .unwrap();
                }
                3 => {
                    let path = f.config.join(LIVE[0]);
                    let mut bytes = fs::read(&path).unwrap();
                    bytes[0] ^= 1;
                    fs::write(path, bytes).unwrap();
                }
                _ => {
                    fs::remove_file(&f.paths.operation_lock).unwrap();
                }
            }
            assert!(resync_historical(&f.config, &f.paths, f.uid, 2, &lock, || true).is_err());
        }
        let (f, lock) = prepared(true);
        assert!(resync_historical(&f.config, &f.paths, f.uid, 3, &lock, || true).is_err());
    }

    #[test]
    fn historical_resync_refuses_original_source_replacement_after_each_effect() {
        let (f, lock) = prepared(true);
        let mut points = Vec::new();
        assert_eq!(
            run_resync(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                || true,
                |point| {
                    points.push(point);
                    true
                }
            ),
            Ok(HistoricalResyncResult::ResynchronizedStillFenced)
        );
        assert!(points.contains(&HistoricalSyncCheckpoint::Final));
        for selected in 0..points.len() {
            let (f, lock) = prepared(true);
            let path = f.paths.state_directory.join(COMPLETE_MEMBER);
            let raw = fs::read(&path).unwrap();
            let mut index = 0;
            assert_eq!(
                run_resync(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    || true,
                    |_| {
                        if index == selected {
                            fs::rename(&path, path.with_extension("replaced-test")).unwrap();
                            fs::write(&path, &raw).unwrap();
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                        }
                        index += 1;
                        true
                    }
                ),
                Err(ExecutionError::Ambiguous),
                "checkpoint {}",
                selected
            );
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
    }

    #[test]
    fn historical_resync_sync_failure_at_each_file_or_directory_stays_fenced() {
        let (f, lock) = prepared(true);
        let mut points = Vec::new();
        run_resync(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            || true,
            |point| {
                if point != HistoricalSyncCheckpoint::Final {
                    points.push(point);
                }
                true
            },
        )
        .unwrap();
        for selected in points {
            let (f, lock) = prepared(true);
            let path = f.paths.state_directory.join(COMPLETE_MEMBER);
            let before = fs::read(&path).unwrap();
            assert_eq!(
                run_resync_with_sync(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    || true,
                    |_| true,
                    |point, file| {
                        if point == selected {
                            Err(std::io::Error::other("synthetic sync failure"))
                        } else {
                            file.sync_all()
                        }
                    }
                ),
                Err(ExecutionError::Ambiguous),
                "{selected:?}"
            );
            assert_eq!(fs::read(path).unwrap(), before);
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
    }

    #[test]
    fn historical_resync_refuses_late_host_gate_boundary_and_transient() {
        for kind in 0..5 {
            let (f, lock) = prepared(true);
            let refuse = std::cell::Cell::new(false);
            assert!(
                run_resync(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    || !refuse.get(),
                    |point| {
                        if point == HistoricalSyncCheckpoint::File(0) {
                            match kind {
                                0 => refuse.set(true),
                                1 => {
                                    let path = f
                                        .paths
                                        .state_directory
                                        .join(crate::cutover::OWNERSHIP_MARKER_NAME);
                                    let raw = fs::read(&path).unwrap();
                                    fs::rename(&path, path.with_extension("replaced-test"))
                                        .unwrap();
                                    fs::write(&path, raw).unwrap();
                                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                                        .unwrap();
                                }
                                2 | 3 => {
                                    let (directory, name) = if kind == 2 {
                                        (&f.paths.state_directory, "desired.json")
                                    } else {
                                        (&f.paths.runtime_base, "omavless-login.receipt")
                                    };
                                    let path = directory.join(name);
                                    if path.exists() {
                                        let raw = fs::read(&path).unwrap();
                                        fs::rename(&path, path.with_extension("replaced-test"))
                                            .unwrap();
                                        fs::write(&path, raw).unwrap();
                                    } else {
                                        fs::write(&path, b"late-boundary").unwrap();
                                    }
                                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                                        .unwrap();
                                }
                                _ => {
                                    fs::write(f.paths.state_directory.join(INTENT), b"late-intent")
                                        .unwrap();
                                }
                            }
                        }
                        true
                    }
                )
                .is_err()
            );
        }
    }

    #[test]
    fn historical_resync_initial_or_final_gate_and_final_hook_refuse() {
        let (f, lock) = prepared(true);
        assert!(resync_historical(&f.config, &f.paths, f.uid, 2, &lock, || false).is_err());
        let refuse = std::cell::Cell::new(false);
        assert_eq!(
            run_resync(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                || !refuse.get(),
                |point| {
                    if point == HistoricalSyncCheckpoint::Final {
                        refuse.set(true);
                    }
                    true
                }
            ),
            Err(ExecutionError::Ambiguous)
        );
        assert_eq!(
            run_resync(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                || true,
                |point| { point != HistoricalSyncCheckpoint::Final }
            ),
            Err(ExecutionError::Ambiguous)
        );
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
    }

    #[test]
    fn historical_resync_process_reentry_without_archive_remains_fenced() {
        for commit in [false, true] {
            let (f, lock) = prepared(commit);
            drop(lock);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::tests::historical_resync_process_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_HISTORICAL_ROOT", &f.root)
                .output()
                .unwrap();
            assert!(output.status.success());
            let lock = f.lock();
            assert_eq!(
                resync_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
                Ok(HistoricalResyncResult::ResynchronizedStillFenced)
            );
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
    }

    #[test]
    fn historical_resync_sigkill_at_every_checkpoint_reenters_still_fenced() {
        let (f, lock) = prepared(true);
        let mut points = Vec::new();
        run_resync(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            || true,
            |point| {
                points.push(point);
                true
            },
        )
        .unwrap();
        for commit in [false, true] {
            for selected in 0..points.len() {
                let (f, lock) = prepared(commit);
                drop(lock);
                let output = Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--ignored",
                        "--exact",
                        "restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::tests::historical_resync_crash_worker",
                    ])
                    .env("OMAVLESS_SYNTHETIC_HISTORICAL_ROOT", &f.root)
                    .env("OMAVLESS_SYNTHETIC_HISTORICAL_POINT", selected.to_string())
                    .output()
                    .unwrap();
                assert_eq!(output.status.signal(), Some(9), "checkpoint {selected}");
                let lock = f.lock();
                assert!(crate::pending_private_transaction::pending_at(
                    &f.paths.state_directory
                ));
                assert_eq!(
                    resync_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
                    Ok(HistoricalResyncResult::ResynchronizedStillFenced),
                    "checkpoint {selected}"
                );
            }
        }
    }

    #[test]
    #[ignore = "internal synthetic historical resync crash worker"]
    fn historical_resync_crash_worker() {
        let f = std::mem::ManuallyDrop::new(
            crate::restore_successor_publication_candidate::tests::Fixture::reopen(PathBuf::from(
                std::env::var_os("OMAVLESS_SYNTHETIC_HISTORICAL_ROOT").unwrap(),
            )),
        );
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_HISTORICAL_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let lock = f.lock();
        let mut index = 0;
        let _ = run_resync(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            || true,
            |_| {
                if index == selected {
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
        panic!("expected synthetic kill");
    }

    #[test]
    #[ignore = "internal synthetic historical resync reentry worker"]
    fn historical_resync_process_worker() {
        let f = std::mem::ManuallyDrop::new(
            crate::restore_successor_publication_candidate::tests::Fixture::reopen(PathBuf::from(
                std::env::var_os("OMAVLESS_SYNTHETIC_HISTORICAL_ROOT").unwrap(),
            )),
        );
        let lock = f.lock();
        assert_eq!(
            resync_historical(&f.config, &f.paths, f.uid, 2, &lock, || true),
            Ok(HistoricalResyncResult::ResynchronizedStillFenced)
        );
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
