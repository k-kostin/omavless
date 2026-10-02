// SPDX-License-Identifier: MIT
//! Read-only rotation evidence. No publication, sync, unlink, exchange,
//! recovery owner or startup authority is provided by this module.
use super::*;
use crate::restore_cleanup_candidate::{Step, inspect_cleanup_prefix};
use crate::restore_closure_model::NEXT_CLOSURE_MEMBER;
use crate::restore_retirement_candidate::{RECEIPT_BYTES, RetirementReceipt};

#[path = "restore_successor_cleanup_candidate.rs"]
pub(crate) mod cleanup;
#[path = "restore_successor_exchange_candidate.rs"]
pub(crate) mod exchange;
#[path = "restore_successor_final_review_candidate.rs"]
pub(crate) mod final_review;
#[path = "restore_successor_next_candidate.rs"]
pub(crate) mod publication;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RotationPhase {
    BeforeNext,
    NextPublished,
    Exchanged,
    DisplacedRetired,
}

#[allow(clippy::too_many_arguments)]
fn classify(
    canonical: &ClosureRecord,
    next: Option<&ClosureRecord>,
    handoff: &SuccessorHandoff,
    receipt: &RetirementReceipt,
    generation: u64,
    desired: Option<&[u8]>,
    new_store: &[u8],
    new_template: &[u8],
    prefix: Step,
    slots: bool,
) -> Result<RotationPhase, ExecutionError> {
    let prior = handoff.predecessor();
    let intent = handoff.successor_intent();
    if !prior
        .receipt()
        .terminal()
        .matches_owner_desired(generation, desired)
        || !intent.matches_owner_desired(generation, desired)
        || !receipt.terminal().same_transaction(intent)
        || !prior
            .receipt()
            .matches_successor_stage(intent, new_store, new_template)
    {
        return Err(REFUSE);
    }
    match receipt.terminal().phase() {
        DecisionPhase::Committed if receipt.matches_pair(new_store, new_template) => (),
        DecisionPhase::Aborted if receipt.same_pair_binding(prior.receipt()) => (),
        _ => return Err(REFUSE),
    }
    if slots && prefix != Step::StageMember(0) {
        return Err(REFUSE);
    }
    let successor = ClosureRecord::from_verified_receipt(receipt).map_err(|_| REFUSE)?;
    if canonical.encode() == prior.encode() {
        match next {
            None if prefix == Step::StageMember(0) => Ok(RotationPhase::BeforeNext),
            Some(next) if next.encode() == successor.encode() => Ok(RotationPhase::NextPublished),
            _ => Err(REFUSE),
        }
    } else if canonical.encode() == successor.encode() && prefix == Step::Done && !slots {
        match next {
            Some(next) if next.encode() == prior.encode() => Ok(RotationPhase::Exchanged),
            None => Ok(RotationPhase::DisplacedRetired),
            _ => Err(REFUSE),
        }
    } else {
        Err(REFUSE)
    }
}

struct Snapshot {
    phase: RotationPhase,
    prefix: Step,
    desired: Option<Zeroizing<Vec<u8>>>,
    directories: Vec<Metadata>,
    members: Vec<Option<(Zeroizing<Vec<u8>>, Metadata)>>,
}
impl Snapshot {
    fn same(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.prefix == other.prefix
            && self.desired == other.desired
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
                .all(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) => a.0 == b.0 && same_member(&a.1, &b.1),
                    (None, None) => true,
                    _ => false,
                })
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_rotation(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    new_store: &[u8],
    new_template: &[u8],
) -> Result<Snapshot, ExecutionError> {
    if !lock.authorizes(paths, uid) {
        return Err(REFUSE);
    }
    let marker = read_marker_existing(paths, uid).map_err(|_| REFUSE)?;
    if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
        return Err(REFUSE);
    }
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let config_dir = open_private_directory(config, uid).map_err(|_| REFUSE)?;
    absent(&state, "routing-preset.pending.json")?;
    absent(
        &state,
        crate::restore_disposition_ticket_model::TICKET_MEMBER,
    )?;
    absent(
        &state,
        crate::restore_disposition_complete_model::COMPLETE_MEMBER,
    )?;
    let desired = read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?;
    let mut members = Vec::new();
    for (name, limit) in [
        (CLOSURE_MEMBER, CLOSURE_BYTES),
        (NEXT_CLOSURE_MEMBER, CLOSURE_BYTES),
        (SUCCESSOR_MEMBER, HANDOFF_BYTES),
        (RECEIPT_MEMBER, RECEIPT_BYTES),
    ] {
        members.push(read_optional(&state, name, uid, limit).map_err(|_| REFUSE)?);
    }
    let canonical =
        ClosureRecord::decode(&members[0].as_ref().ok_or(REFUSE)?.0).map_err(|_| REFUSE)?;
    let next = members[1]
        .as_ref()
        .map(|v| ClosureRecord::decode(&v.0).map_err(|_| REFUSE))
        .transpose()?;
    let handoff =
        SuccessorHandoff::decode(&members[2].as_ref().ok_or(REFUSE)?.0).map_err(|_| REFUSE)?;
    let receipt =
        RetirementReceipt::decode(&members[3].as_ref().ok_or(REFUSE)?.0).map_err(|_| REFUSE)?;
    if !receipt.matches_live(config, uid).map_err(|_| REFUSE)? {
        return Err(REFUSE);
    }
    let prefix = inspect_cleanup_prefix(paths, &state, uid, &receipt).map_err(|_| REFUSE)?;
    for (index, name) in LIVE.into_iter().enumerate() {
        members.push(Some(
            read_optional(
                &config_dir,
                name,
                uid,
                if index == 0 {
                    MAX_PRIVATE_STORE_BYTES
                } else {
                    MAX_TEMPLATE_BYTES
                },
            )
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?,
        ));
    }
    for name in [INTENT, TERMINAL] {
        members.push(read_optional(&state, name, uid, RECORD_BYTES).map_err(|_| REFUSE)?);
    }
    let mut slots = false;
    let stage = if prefix == Step::StageMember(0) {
        Some(read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE)?)
    } else {
        None
    };
    for (old, names) in [(false, NEW_SLOT), (true, OLD_SLOT)] {
        for (index, name) in names.into_iter().enumerate() {
            let member = read_optional(
                &config_dir,
                name,
                uid,
                if index == 0 {
                    MAX_PRIVATE_STORE_BYTES
                } else {
                    MAX_TEMPLATE_BYTES
                },
            )
            .map_err(|_| REFUSE)?;
            if let Some((raw, _)) = &member {
                slots = true;
                if raw.as_slice() != stage_bytes(stage.as_ref().ok_or(REFUSE)?, old, index) {
                    return Err(REFUSE);
                }
            }
            members.push(member);
        }
    }
    let phase = classify(
        &canonical,
        next.as_ref(),
        &handoff,
        &receipt,
        generation,
        desired.as_ref().map(|v| v.as_slice()),
        new_store,
        new_template,
        prefix,
        slots,
    )?;
    let mut directories = vec![
        state.metadata().map_err(|_| REFUSE)?,
        config_dir.metadata().map_err(|_| REFUSE)?,
    ];
    if absent(&state, PENDING_DIRECTORY).is_err() {
        let stage_dir = open_private_directory(&paths.state_directory.join(PENDING_DIRECTORY), uid)
            .map_err(|_| REFUSE)?;
        directories.push(stage_dir.metadata().map_err(|_| REFUSE)?);
        for (index, name) in MEMBERS.into_iter().chain([READY_MEMBER]).enumerate() {
            members.push(
                read_optional(
                    &stage_dir,
                    name,
                    uid,
                    if index == 4 {
                        READY_BYTES
                    } else if index % 2 == 0 {
                        MAX_PRIVATE_STORE_BYTES
                    } else {
                        MAX_TEMPLATE_BYTES
                    },
                )
                .map_err(|_| REFUSE)?,
            );
        }
    } else {
        members.extend((0..5).map(|_| None));
    }
    for (index, path) in [
        &paths.state_directory,
        config,
        &paths.state_directory.join(PENDING_DIRECTORY),
    ]
    .into_iter()
    .enumerate()
    .take(directories.len())
    {
        if !same_directory(
            &directories[index],
            &open_private_directory(path, uid)
                .map_err(|_| REFUSE)?
                .metadata()
                .map_err(|_| REFUSE)?,
        ) {
            return Err(REFUSE);
        }
    }
    if read_marker_existing(paths, uid).ok() != Some(marker)
        || read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)? != desired
    {
        return Err(REFUSE);
    }
    Ok(Snapshot {
        phase,
        prefix,
        desired,
        directories,
        members,
    })
}

/// Read-only point-in-time evidence, never durability or mutation permission.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn review_rotation(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    mut gate: impl FnMut() -> bool,
) -> Result<RotationPhase, ExecutionError> {
    if !gate() {
        return Err(REFUSE);
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let first = observe_rotation(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    if !gate() {
        return Err(REFUSE);
    }
    let second = observe_rotation(
        config,
        paths,
        uid,
        generation,
        lock,
        &off,
        backup.template(),
    )?;
    if !first.same(&second) {
        return Err(REFUSE);
    }
    Ok(second.phase)
}

#[cfg(test)]
mod tests {
    use super::super::receipt::publish_successor_receipt;
    use super::super::tests::{drive, pair, prepared, second};
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::{fs, os::unix::fs::PermissionsExt};
    fn write(path: &Path, raw: &[u8]) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    pub(super) fn setup(commit: bool) -> (Fixture, MigrationLock, Vec<u8>, Vec<u8>) {
        let (f, lock) = prepared();
        drive(&f, &lock, !commit, |_| true).unwrap();
        publish_successor_receipt(&f.config, &f.paths, f.uid, 2, &lock, second(), || true).unwrap();
        let old = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let receipt = RetirementReceipt::decode(
            &fs::read(f.paths.state_directory.join(RECEIPT_MEMBER)).unwrap(),
        )
        .unwrap();
        let new = ClosureRecord::from_verified_receipt(&receipt)
            .unwrap()
            .encode()
            .to_vec();
        (f, lock, old, new)
    }
    fn review(f: &Fixture, lock: &MigrationLock) -> Result<RotationPhase, ExecutionError> {
        crate::restore_cleanup_candidate::without_inventory_sync(|| {
            review_rotation(&f.config, &f.paths, f.uid, 2, lock, second(), || true)
        })
    }
    fn cleanup_step(f: &Fixture, index: usize) {
        let stage = f.paths.state_directory.join(PENDING_DIRECTORY);
        match index {
            0..=3 => fs::remove_file(stage.join(MEMBERS[index])).unwrap(),
            4 => fs::remove_file(stage.join(READY_MEMBER)).unwrap(),
            5 => fs::remove_dir(stage).unwrap(),
            6 => fs::remove_file(f.paths.state_directory.join(TERMINAL)).unwrap(),
            7 => fs::remove_file(f.paths.state_directory.join(INTENT)).unwrap(),
            _ => unreachable!(),
        }
    }
    #[test]
    fn exact_commit_and_abort_phases_are_readonly_and_fenced() {
        for commit in [false, true] {
            let (f, lock, old, new) = setup(commit);
            assert_eq!(review(&f, &lock), Ok(RotationPhase::BeforeNext));
            write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &new);
            for index in 0..8 {
                assert_eq!(review(&f, &lock), Ok(RotationPhase::NextPublished));
                cleanup_step(&f, index);
            }
            assert_eq!(review(&f, &lock), Ok(RotationPhase::NextPublished));
            write(&f.paths.state_directory.join(CLOSURE_MEMBER), &new);
            write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &old);
            assert_eq!(review(&f, &lock), Ok(RotationPhase::Exchanged));
            fs::remove_file(f.paths.state_directory.join(NEXT_CLOSURE_MEMBER)).unwrap();
            assert_eq!(review(&f, &lock), Ok(RotationPhase::DisplacedRetired));
            pair(&f, commit);
        }
    }
    #[test]
    fn wrong_crossed_missing_or_early_cleanup_evidence_refuses() {
        for case in [
            "c0-c0",
            "c1-c1",
            "missing",
            "wrong-next",
            "partial-no-next",
            "slot-after-cleanup",
            "early-exchange",
            "hole",
            "wrong-archive",
        ] {
            let (f, lock, old, new) = setup(true);
            match case {
                "c0-c0" => write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &old),
                "c1-c1" => {
                    write(&f.paths.state_directory.join(CLOSURE_MEMBER), &new);
                    write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &new);
                }
                "missing" => fs::remove_file(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                "wrong-next" => write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), b"torn"),
                "partial-no-next" => cleanup_step(&f, 0),
                "slot-after-cleanup" => {
                    write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &new);
                    cleanup_step(&f, 0);
                    write(
                        &f.config.join(NEW_SLOT[0]),
                        &second().restore_store_off().unwrap(),
                    );
                }
                "early-exchange" => {
                    write(&f.paths.state_directory.join(CLOSURE_MEMBER), &new);
                    write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &old);
                }
                "hole" => {
                    write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &new);
                    cleanup_step(&f, 1);
                }
                "wrong-archive" => {
                    assert!(
                        review_rotation(&f.config, &f.paths, f.uid, 2, &lock, backup(), || true)
                            .is_err()
                    );
                    continue;
                }
                _ => unreachable!(),
            }
            assert!(review(&f, &lock).is_err(), "{case}");
        }
    }
    #[test]
    fn late_same_byte_replacements_and_host_change_are_refused() {
        for target in [
            CLOSURE_MEMBER,
            NEXT_CLOSURE_MEMBER,
            SUCCESSOR_MEMBER,
            RECEIPT_MEMBER,
            INTENT,
            TERMINAL,
            "stage",
            "stage-dir",
            "gate",
        ] {
            let (f, lock, _, new) = setup(true);
            write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &new);
            let mut count = 0;
            assert!(
                review_rotation(&f.config, &f.paths, f.uid, 2, &lock, second(), || {
                    count += 1;
                    if count == 2 {
                        if target == "gate" {
                            return false;
                        }
                        if target == "stage-dir" {
                            let path = f.paths.state_directory.join(PENDING_DIRECTORY);
                            let moved = f.root.join("old-stage-directory");
                            fs::rename(&path, &moved).unwrap();
                            fs::create_dir(&path).unwrap();
                            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                            for name in MEMBERS.into_iter().chain([READY_MEMBER]) {
                                fs::rename(moved.join(name), path.join(name)).unwrap();
                            }
                            return true;
                        }
                        let path = if target == "stage" {
                            f.paths
                                .state_directory
                                .join(PENDING_DIRECTORY)
                                .join(MEMBERS[0])
                        } else {
                            f.paths.state_directory.join(target)
                        };
                        let raw = fs::read(&path).unwrap();
                        fs::rename(&path, f.root.join("swapped")).unwrap();
                        write(&path, &raw);
                    }
                    true
                })
                .is_err()
            );
        }
    }
    #[test]
    fn reconstructed_stage_uses_prior_old_binding_and_both_new_members() {
        let (f, lock, _, _) = setup(true);
        let raw = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        let h = SuccessorHandoff::decode(&raw).unwrap();
        let off = second().restore_store_off().unwrap();
        assert!(h.predecessor().receipt().matches_successor_stage(
            h.successor_intent(),
            &off,
            second().template()
        ));
        assert!(!h.predecessor().receipt().matches_successor_stage(
            h.successor_intent(),
            backup().store(),
            second().template()
        ));
        assert!(!h.predecessor().receipt().matches_successor_stage(
            h.successor_intent(),
            &off,
            backup().template()
        ));
        let foreign = RetirementReceipt::synthetic(
            h.predecessor().receipt().terminal(),
            b"wrong old",
            b"wrong template",
        );
        assert!(!foreign.matches_successor_stage(h.successor_intent(), &off, second().template()));
        drop(lock);
    }

    #[test]
    fn exact_slots_only_precede_stage_cleanup_and_unknown_entries_refuse() {
        let (f, lock, _, new) = setup(true);
        write(
            &f.config.join(NEW_SLOT[0]),
            &second().restore_store_off().unwrap(),
        );
        assert_eq!(review(&f, &lock), Ok(RotationPhase::BeforeNext));
        write(&f.paths.state_directory.join(NEXT_CLOSURE_MEMBER), &new);
        assert_eq!(review(&f, &lock), Ok(RotationPhase::NextPublished));
        write(
            &f.paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join("foreign"),
            b"unexpected",
        );
        assert!(review(&f, &lock).is_err());
        fs::remove_file(
            f.paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join("foreign"),
        )
        .unwrap();
        write(&f.config.join(NEW_SLOT[0]), b"wrong slot");
        assert!(review(&f, &lock).is_err());
    }

    #[test]
    fn reserved_next_fence_blocks_earlier_execution_and_receipt_candidates() {
        for operation in ["execute", "recover", "receipt"] {
            let (f, lock) = prepared();
            if operation == "receipt" {
                drive(&f, &lock, false, |_| true).unwrap();
            }
            write(
                &f.paths.state_directory.join(NEXT_CLOSURE_MEMBER),
                b"incomplete reserved next",
            );
            let result = match operation {
                "execute" => {
                    execute_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
                }
                "recover" => {
                    recover_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
                }
                _ => publish_successor_receipt(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    second(),
                    || true,
                ),
            };
            assert!(result.is_err());
            assert!(!f.paths.state_directory.join(RECEIPT_MEMBER).exists());
            pair(&f, operation == "receipt");
        }
    }
}
