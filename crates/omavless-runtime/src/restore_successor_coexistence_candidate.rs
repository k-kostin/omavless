// SPDX-License-Identifier: MIT

//! Inactive read-only handoff/stage/intent coexistence verification. This does
//! not prove durability, authorize retirement, resume a restore or admit startup.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::restore_cleanup_candidate::read_optional;
use crate::restore_closure_model::{CLOSURE_MEMBER, ClosureRecord, RECORD_BYTES as CLOSURE_BYTES};
use crate::restore_decision_candidate::{
    DecisionPhase, DecisionRecord, RECORD_BYTES as DECISION_BYTES,
};
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_journal_candidate::read_desired_for_decision;
use crate::restore_retirement_candidate::RECEIPT_MEMBER;
use crate::restore_staging_candidate::{
    MEMBERS, PENDING_DIRECTORY, READY_BYTES, READY_MEMBER, read_staged_pair, same_directory,
    same_member,
};
use crate::restore_successor_handoff_model::{RECORD_BYTES, SUCCESSOR_MEMBER, SuccessorHandoff};
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use omavless_domain::{
    config::MAX_TEMPLATE_BYTES, private_backup::OpenedBackup,
    private_store::MAX_PRIVATE_STORE_BYTES,
};
use std::fs::{File, Metadata};
use std::path::Path;
use zeroize::Zeroizing;

#[path = "restore_successor_preparation_candidate.rs"]
pub(crate) mod preparation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoexistenceError {
    UnavailableOrUncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoexistenceReview {
    MatchingIntentStillFenced,
}
/// Exact expected preparation phase, never an instruction to advance it.
#[allow(dead_code)] // Earlier phases have only synthetic callers until preparation is reviewed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreparationPhase {
    HandoffOnly,
    StageWithoutIntent,
    StageWithIntent,
}
type Result<T> = std::result::Result<T, CoexistenceError>;
const REFUSE: CoexistenceError = CoexistenceError::UnavailableOrUncertain;

struct Snapshot {
    directories: Vec<Metadata>,
    members: Vec<Metadata>,
    handoff: [u8; RECORD_BYTES],
    old: [Zeroizing<Vec<u8>>; 2],
    base_identities: [Metadata; 4],
}
impl Snapshot {
    fn same_stage(&self, other: &Self) -> bool {
        self.directories.len() == 3
            && other.directories.len() == 3
            && same_directory(&self.directories[2], &other.directories[2])
            && self
                .members
                .iter()
                .rev()
                .take(5)
                .zip(other.members.iter().rev().take(5))
                .all(|(a, b)| same_member(a, b))
    }
    fn same_base(&self, other: &Self) -> bool {
        self.handoff == other.handoff
            && self.old == other.old
            && self.directories[..2]
                .iter()
                .zip(&other.directories[..2])
                .all(|(a, b)| same_directory(a, b))
            && self
                .base_identities
                .iter()
                .zip(&other.base_identities)
                .all(|(a, b)| same_member(a, b))
    }
    fn same(&self, other: &Self) -> bool {
        self.handoff == other.handoff
            && self.directories.len() == other.directories.len()
            && self
                .directories
                .iter()
                .zip(&other.directories)
                .all(|(a, b)| same_directory(a, b))
            && self.members.len() == other.members.len()
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|(a, b)| same_member(a, b))
    }
}

fn absent(parent: &File, name: &str) -> Result<()> {
    match openat(
        parent,
        Path::new(name),
        OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Err(Errno::ENOENT) => Ok(()),
        _ => Err(REFUSE),
    }
}

#[allow(clippy::too_many_arguments)]
fn observe(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    new_store: &[u8],
    new_template: &[u8],
    phase: PreparationPhase,
) -> Result<Snapshot> {
    if !lock.authorizes(paths, uid) {
        return Err(REFUSE);
    }
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let config_dir = open_private_directory(config, uid).map_err(|_| REFUSE)?;
    let stage_dir = if phase == PreparationPhase::HandoffOnly {
        absent(&state, PENDING_DIRECTORY)?;
        None
    } else {
        Some(
            open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
                .map_err(|_| REFUSE)?,
        )
    };
    let mut directories = vec![
        state.metadata().map_err(|_| REFUSE)?,
        config_dir.metadata().map_err(|_| REFUSE)?,
    ];
    if let Some(stage) = &stage_dir {
        directories.push(stage.metadata().map_err(|_| REFUSE)?);
    }
    let marker = read_marker_existing(paths, uid).map_err(|_| REFUSE)?;
    if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
        return Err(REFUSE);
    }
    for name in [
        RECEIPT_MEMBER,
        "restore-decision.terminal",
        "routing-preset.pending.json",
    ] {
        absent(&state, name)?;
    }
    for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
        absent(&config_dir, name)?;
    }
    let desired = read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?;
    let stage = stage_dir
        .as_ref()
        .map(|_| read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE))
        .transpose()?;
    if let Some(stage) = &stage
        && (stage.new_store() != new_store || stage.new_template() != new_template)
    {
        return Err(REFUSE);
    }
    let mut identities = Vec::new();
    let mut read = |directory: &File, name: &str, limit: usize| {
        let (bytes, metadata) = read_optional(directory, name, uid, limit)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        identities.push(metadata);
        Ok::<_, CoexistenceError>(bytes)
    };
    let prior_raw = read(&state, CLOSURE_MEMBER, CLOSURE_BYTES)?;
    let prior = ClosureRecord::decode(&prior_raw).map_err(|_| REFUSE)?;
    let raw = read(&state, SUCCESSOR_MEMBER, RECORD_BYTES)?;
    let handoff = SuccessorHandoff::decode(&raw).map_err(|_| REFUSE)?;
    let intent = if phase == PreparationPhase::StageWithIntent {
        DecisionRecord::decode(&read(&state, "restore-decision.intent", DECISION_BYTES)?)
            .map_err(|_| REFUSE)?
    } else {
        absent(&state, "restore-decision.intent")?;
        DecisionRecord::decode(&handoff.successor_intent().encode()).map_err(|_| REFUSE)?
    };
    let old_store = read(&config_dir, "profiles.json", MAX_PRIVATE_STORE_BYTES)?;
    let old_template = read(&config_dir, "route-template.yaml", MAX_TEMPLATE_BYTES)?;
    if let Some(stage) = &stage
        && (stage.old_store() != old_store.as_slice()
            || stage.old_template() != old_template.as_slice())
    {
        return Err(REFUSE);
    }
    if intent.phase() != DecisionPhase::Intent
        || !handoff.matches_verified_plan(
            &prior,
            &intent,
            generation,
            desired.as_ref().map(|bytes| bytes.as_slice()),
            [
                old_store.as_slice(),
                old_template.as_slice(),
                new_store,
                new_template,
            ],
        )
    {
        return Err(REFUSE);
    }
    // Reopen actual staged entries against verified bytes while pinning their
    // inode identities across the external gate; identical substitution refuses.
    if let (Some(stage), Some(stage_dir)) = (&stage, &stage_dir) {
        for (index, bytes) in [
            stage.old_store(),
            stage.old_template(),
            stage.new_store(),
            stage.new_template(),
        ]
        .into_iter()
        .enumerate()
        {
            let limit = if index % 2 == 0 {
                MAX_PRIVATE_STORE_BYTES
            } else {
                MAX_TEMPLATE_BYTES
            };
            if read(stage_dir, MEMBERS[index], limit)?.as_slice() != bytes {
                return Err(REFUSE);
            }
        }
        let ready = read(stage_dir, READY_MEMBER, READY_BYTES)?;
        if !intent.matches_stage_ready(&ready) {
            return Err(REFUSE);
        }
    }
    if read_marker_existing(paths, uid).ok() != Some(marker)
        || read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)? != desired
    {
        return Err(REFUSE);
    }
    // The next full pass rechecks all absences and contents as well as these
    // identities. No caller callback runs after the final verification pass.
    Ok(Snapshot {
        directories,
        base_identities: {
            let offset = usize::from(phase == PreparationPhase::StageWithIntent);
            [
                identities[0].clone(),
                identities[1].clone(),
                identities[2 + offset].clone(),
                identities[3 + offset].clone(),
            ]
        },
        members: identities,
        handoff: handoff.encode(),
        old: [old_store, old_template],
    })
}

/// The caller retains the lease and supplies fresh idle/owned-host evidence.
/// Result is a read-only point-in-time classification, never a capability.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn inspect_successor_coexistence(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    gate: impl FnMut() -> bool,
) -> Result<CoexistenceReview> {
    inspect_successor_preparation(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        PreparationPhase::StageWithIntent,
        gate,
    )?;
    Ok(CoexistenceReview::MatchingIntentStillFenced)
}

/// Verify one exact pre-execution phase. Missing, partial and contradictory
/// evidence refuses; this never creates, repairs or advances a preparation.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn inspect_successor_preparation(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    phase: PreparationPhase,
    gate: impl FnMut() -> bool,
) -> Result<PreparationPhase> {
    inspect_snapshot(config, paths, uid, generation, lock, backup, phase, gate)?;
    Ok(phase)
}

#[allow(clippy::too_many_arguments)]
fn inspect_snapshot(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    phase: PreparationPhase,
    mut gate: impl FnMut() -> bool,
) -> Result<Snapshot> {
    if !gate() {
        return Err(REFUSE);
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let before = observe(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
        phase,
    )?;
    if !gate() {
        return Err(REFUSE);
    }
    let after = observe(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
        phase,
    )?;
    if !before.same(&after) {
        return Err(REFUSE);
    }
    // Bind final directory names again, after the final file reads.
    let stage_path = paths.state_directory.join(PENDING_DIRECTORY);
    for (path, identity) in [
        paths.state_directory.as_path(),
        config,
        stage_path.as_path(),
    ]
    .into_iter()
    .zip(&after.directories)
    {
        if !same_directory(
            identity,
            &open_private_directory(path, uid)
                .map_err(|_| REFUSE)?
                .metadata()
                .map_err(|_| REFUSE)?,
        ) {
            return Err(REFUSE);
        }
    }
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_staging_candidate::stage_private_pair;
    use crate::restore_successor_publication_candidate::tests::{Fixture, OLD, backup};
    use crate::restore_successor_publication_candidate::{
        PublicationResult, publish_successor_handoff,
    };
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Command;

    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn publish(f: &Fixture, lock: &MigrationLock) {
        assert_eq!(
            publish_successor_handoff(
                &f.config,
                &f.paths,
                f.uid,
                2,
                lock,
                backup(),
                [2; 16],
                || true
            ),
            Ok(PublicationResult::PublishedStillFenced)
        );
    }
    fn stage(f: &Fixture) {
        let off = backup().restore_store_off().unwrap();
        stage_private_pair(
            &f.paths.state_directory,
            f.uid,
            OLD[0],
            OLD[1],
            &off,
            backup().template(),
        )
        .unwrap();
    }
    fn intent(f: &Fixture) {
        let record = SuccessorHandoff::decode(
            &fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap(),
        )
        .unwrap();
        write(
            &f.paths.state_directory.join("restore-decision.intent"),
            &record.successor_intent().encode(),
        );
    }
    fn inspect(f: &Fixture, lock: &MigrationLock) -> Result<CoexistenceReview> {
        inspect_successor_coexistence(&f.config, &f.paths, f.uid, 2, lock, backup(), || true)
    }
    fn ready() -> (Fixture, MigrationLock) {
        let f = Fixture::new();
        let lock = f.lock();
        publish(&f, &lock);
        stage(&f);
        intent(&f);
        (f, lock)
    }
    fn unchanged(f: &Fixture, predecessor: &[u8], handoff: &[u8]) {
        assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), OLD[0]);
        assert_eq!(
            fs::read(f.config.join("route-template.yaml")).unwrap(),
            OLD[1]
        );
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            predecessor
        );
        assert_eq!(
            fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap(),
            handoff
        );
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
    }

    #[test]
    fn authenticated_coexistence_is_read_only_and_requires_every_member() {
        let f = Fixture::new();
        let lock = f.lock();
        assert!(inspect(&f, &lock).is_err());
        publish(&f, &lock);
        assert!(inspect(&f, &lock).is_err());
        stage(&f);
        assert!(inspect(&f, &lock).is_err());
        intent(&f);
        let prior = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let handoff = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        let state_names = fs::read_dir(&f.paths.state_directory).unwrap().count();
        assert_eq!(
            inspect(&f, &lock),
            Ok(CoexistenceReview::MatchingIntentStillFenced)
        );
        assert_eq!(
            inspect(&f, &lock),
            Ok(CoexistenceReview::MatchingIntentStillFenced)
        );
        unchanged(&f, &prior, &handoff);
        assert_eq!(
            fs::read_dir(&f.paths.state_directory).unwrap().count(),
            state_names
        );
        // The completion-only inspector must remain strict and unchanged.
        assert!(
            crate::restore_cleanup_candidate::inspect_completion_record(
                &f.config, &f.paths, f.uid, 2, &lock
            )
            .is_err()
        );
    }

    #[test]
    fn preparation_review_matches_only_the_exact_complete_phase() {
        let f = Fixture::new();
        let lock = f.lock();
        publish(&f, &lock);
        for expected in [
            PreparationPhase::HandoffOnly,
            PreparationPhase::StageWithoutIntent,
            PreparationPhase::StageWithIntent,
        ] {
            if expected == PreparationPhase::StageWithoutIntent {
                stage(&f);
            }
            if expected == PreparationPhase::StageWithIntent {
                intent(&f);
            }
            for requested in [
                PreparationPhase::HandoffOnly,
                PreparationPhase::StageWithoutIntent,
                PreparationPhase::StageWithIntent,
            ] {
                let result = inspect_successor_preparation(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    requested,
                    || true,
                );
                assert_eq!(
                    result,
                    if requested == expected {
                        Ok(expected)
                    } else {
                        Err(REFUSE)
                    }
                );
            }
        }
    }

    #[test]
    fn early_phases_refuse_late_artifacts_and_changed_handoff() {
        for phase in [
            PreparationPhase::HandoffOnly,
            PreparationPhase::StageWithoutIntent,
        ] {
            for drift in ["intent", "receipt", "handoff", "gate"] {
                let f = Fixture::new();
                let lock = f.lock();
                publish(&f, &lock);
                if phase == PreparationPhase::StageWithoutIntent {
                    stage(&f);
                }
                let mut calls = 0;
                let result = inspect_successor_preparation(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    backup(),
                    phase,
                    || {
                        calls += 1;
                        if calls == 2 {
                            match drift {
                                "intent" => intent(&f),
                                "receipt" => {
                                    write(&f.paths.state_directory.join(RECEIPT_MEMBER), b"late")
                                }
                                "handoff" => {
                                    let path = f.paths.state_directory.join(SUCCESSOR_MEMBER);
                                    let raw = fs::read(&path).unwrap();
                                    fs::rename(&path, f.root.join("replaced")).unwrap();
                                    write(&path, &raw);
                                }
                                "gate" => return false,
                                _ => unreachable!(),
                            }
                        }
                        true
                    },
                );
                assert_eq!(result, Err(REFUSE));
            }
        }
    }

    #[test]
    fn mismatched_and_unsafe_evidence_never_becomes_a_verification_candidate() {
        for drift in [
            "old", "new", "live", "intent", "terminal", "receipt", "routing", "slot", "owner",
            "desired", "symlink", "hardlink", "mode", "extra", "partial",
        ] {
            let (f, lock) = ready();
            let stage = f.paths.state_directory.join(PENDING_DIRECTORY);
            let intent = f.paths.state_directory.join("restore-decision.intent");
            match drift {
                "old" => write(&stage.join(MEMBERS[0]), b"foreign old"),
                "new" => write(&stage.join(MEMBERS[2]), b"foreign new"),
                "live" => write(&f.config.join("profiles.json"), b"changed live"),
                "intent" => {
                    let record = DecisionRecord::decode(&fs::read(&intent).unwrap()).unwrap();
                    write(
                        &intent,
                        &record
                            .terminal(crate::restore_decision_candidate::TerminalChoice::Commit)
                            .unwrap()
                            .encode(),
                    );
                }
                "terminal" => write(
                    &f.paths.state_directory.join("restore-decision.terminal"),
                    b"unknown",
                ),
                "receipt" => write(&f.paths.state_directory.join(RECEIPT_MEMBER), b"unknown"),
                "routing" => write(
                    &f.paths.state_directory.join("routing-preset.pending.json"),
                    b"unknown",
                ),
                "slot" => write(&f.config.join(NEW_SLOT[0]), b"unknown"),
                "owner" => write(
                    &f.paths.ownership_marker,
                    br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                ),
                "desired" => write(
                    &f.paths.state_directory.join("desired.json"),
                    &serde_json::to_vec(&crate::desired::DesiredState::default()).unwrap(),
                ),
                "symlink" => {
                    fs::remove_file(&intent).unwrap();
                    symlink("missing", &intent).unwrap();
                }
                "hardlink" => fs::hard_link(&intent, f.root.join("alias")).unwrap(),
                "mode" => fs::set_permissions(&intent, fs::Permissions::from_mode(0o644)).unwrap(),
                "extra" => write(&stage.join("extra"), b"unknown"),
                "partial" => {
                    fs::remove_file(stage.join(READY_MEMBER)).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(inspect(&f, &lock).is_err());
            assert!(crate::pending_private_transaction::pending_at(
                &f.paths.state_directory
            ));
        }
    }

    #[test]
    fn gate_drift_and_same_byte_replacements_are_refused() {
        for target in [
            CLOSURE_MEMBER,
            SUCCESSOR_MEMBER,
            "restore-decision.intent",
            MEMBERS[0],
            READY_MEMBER,
            "state",
            "config",
            "stage",
            "artifact",
            "gate",
        ] {
            let (f, lock) = ready();
            let mut calls = 0;
            let result = inspect_successor_coexistence(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                backup(),
                || {
                    calls += 1;
                    if calls == 2 {
                        if target == "gate" {
                            return false;
                        }
                        if target == "artifact" {
                            write(
                                &f.paths.state_directory.join(RECEIPT_MEMBER),
                                b"late receipt",
                            );
                            return true;
                        }
                        if target == "stage" {
                            let path = f.paths.state_directory.join(PENDING_DIRECTORY);
                            let moved = f.root.join("replaced-stage");
                            fs::rename(&path, &moved).unwrap();
                            fs::create_dir(&path).unwrap();
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                            for name in MEMBERS.into_iter().chain([READY_MEMBER]) {
                                write(&path.join(name), &fs::read(moved.join(name)).unwrap());
                            }
                            return true;
                        }
                        if target == "state" || target == "config" {
                            let path = if target == "state" {
                                &f.paths.state_directory
                            } else {
                                &f.config
                            };
                            fs::rename(path, f.root.join("moved")).unwrap();
                            fs::create_dir(path).unwrap();
                            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
                        } else {
                            let parent = if target == MEMBERS[0] || target == READY_MEMBER {
                                f.paths.state_directory.join(PENDING_DIRECTORY)
                            } else {
                                f.paths.state_directory.clone()
                            };
                            let path = parent.join(target);
                            let raw = fs::read(&path).unwrap();
                            fs::rename(&path, f.root.join("replaced")).unwrap();
                            write(&path, &raw);
                        }
                    }
                    true
                },
            );
            assert_eq!(result, Err(REFUSE));
        }
    }

    #[test]
    fn a_different_authenticated_archive_cannot_validate_the_stage() {
        let (f, lock) = ready();
        let mut value: serde_json::Value = serde_json::from_slice(backup().store()).unwrap();
        value["onboardingComplete"] = true.into();
        let encrypted = omavless_domain::private_backup::seal(
            &serde_json::to_vec(&value).unwrap(),
            backup().template(),
            b"synthetic different archive",
        )
        .unwrap();
        let other =
            omavless_domain::private_backup::open(&encrypted, b"synthetic different archive")
                .unwrap();
        assert_eq!(
            inspect_successor_coexistence(&f.config, &f.paths, f.uid, 2, &lock, &other, || true),
            Err(REFUSE)
        );
    }

    #[test]
    fn interrupted_coexistence_preparation_never_grants_execution() {
        for checkpoint in 0..5 {
            let f = Fixture::new();
            drop(f.lock());
            let prior = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_successor_coexistence_candidate::tests::coexistence_crash_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_COEXISTENCE_ROOT", &f.root)
                .env(
                    "OMAVLESS_SYNTHETIC_COEXISTENCE_CHECKPOINT",
                    checkpoint.to_string(),
                )
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(9));
            let handoff = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
            let lock = f.lock();
            let expected = if checkpoint == 4 {
                Ok(CoexistenceReview::MatchingIntentStillFenced)
            } else {
                Err(REFUSE)
            };
            assert_eq!(inspect(&f, &lock), expected);
            for phase in [
                PreparationPhase::HandoffOnly,
                PreparationPhase::StageWithoutIntent,
                PreparationPhase::StageWithIntent,
            ] {
                let matches = matches!(
                    (checkpoint, phase),
                    (0, PreparationPhase::HandoffOnly)
                        | (2, PreparationPhase::StageWithoutIntent)
                        | (4, PreparationPhase::StageWithIntent)
                );
                assert_eq!(
                    inspect_successor_preparation(
                        &f.config,
                        &f.paths,
                        f.uid,
                        2,
                        &lock,
                        backup(),
                        phase,
                        || true
                    ),
                    if matches { Ok(phase) } else { Err(REFUSE) }
                );
            }
            unchanged(&f, &prior, &handoff);
        }
    }

    #[test]
    #[ignore = "internal synthetic coexistence crash worker"]
    fn coexistence_crash_worker() {
        let root = PathBuf::from(std::env::var_os("OMAVLESS_SYNTHETIC_COEXISTENCE_ROOT").unwrap());
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_COEXISTENCE_CHECKPOINT")
            .unwrap()
            .parse()
            .unwrap();
        let f = std::mem::ManuallyDrop::new(Fixture::reopen(root));
        let lock = f.lock();
        let kill = |point| {
            if selected == point {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
        };
        publish(&f, &lock);
        kill(0);
        if selected == 1 {
            let dir = f.paths.state_directory.join(PENDING_DIRECTORY);
            fs::create_dir(&dir).unwrap();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
            write(&dir.join(MEMBERS[0]), OLD[0]);
            kill(1);
        }
        stage(&f);
        kill(2);
        write(
            &f.paths.state_directory.join("restore-decision.intent"),
            b"partial synthetic intent",
        );
        kill(3);
        intent(&f);
        kill(4);
        panic!("expected synthetic worker termination");
    }
}
