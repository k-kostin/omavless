// SPDX-License-Identifier: MIT
//! Read-only terminal output/completion evidence. After handoff disappearance
//! predecessor lineage is unavailable: this is not execution, durability,
//! mutation or normal-owner startup authority. No sync or unlink is performed.
use super::*;
use crate::restore_staging_candidate::planned_stage_identity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FinalPhase {
    BeforeHandoffRetirement,
    HandoffAbsentReceiptPresent,
    HandoffAbsentReceiptAbsent,
}
struct FinalSnapshot {
    phase: FinalPhase,
    evidence: Snapshot,
}
impl FinalSnapshot {
    fn same(&self, other: &Self) -> bool {
        self.phase == other.phase && self.evidence.same(&other.evidence)
    }
}

fn output_binding(receipt: &RetirementReceipt, live: [&[u8]; 2], new: [&[u8]; 2]) -> bool {
    if !receipt.matches_pair(live[0], live[1]) {
        return false;
    }
    match receipt.terminal().phase() {
        DecisionPhase::Committed => receipt.matches_pair(new[0], new[1]),
        DecisionPhase::Aborted => planned_stage_identity([live[0], live[1], new[0], new[1]])
            .is_ok_and(|stage| receipt.terminal().matches_stage_identity(&stage)),
        DecisionPhase::Intent => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_final(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    new_store: &[u8],
    new_template: &[u8],
) -> Result<FinalSnapshot, ExecutionError> {
    if !lock.authorizes(paths, uid) {
        return Err(REFUSE);
    }
    let marker = read_marker_existing(paths, uid).map_err(|_| REFUSE)?;
    if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
        return Err(REFUSE);
    }
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let config_dir = open_private_directory(config, uid).map_err(|_| REFUSE)?;
    let desired = read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?;
    for name in [
        NEXT_CLOSURE_MEMBER,
        PENDING_DIRECTORY,
        INTENT,
        TERMINAL,
        "routing-preset.pending.json",
    ] {
        absent(&state, name)?;
    }
    for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
        absent(&config_dir, name)?;
    }
    let mut members = Vec::new();
    for (name, limit) in [
        (CLOSURE_MEMBER, CLOSURE_BYTES),
        (NEXT_CLOSURE_MEMBER, CLOSURE_BYTES),
        (SUCCESSOR_MEMBER, HANDOFF_BYTES),
        (RECEIPT_MEMBER, RECEIPT_BYTES),
    ] {
        members.push(read_optional(&state, name, uid, limit).map_err(|_| REFUSE)?);
    }
    if members[1].is_some() {
        return Err(REFUSE);
    }
    if members[2].is_some() {
        // The original classifier supplies the full predecessor relationship
        // only while H and R1 both survive. H-present/R1-absent always refuses.
        if members[3].is_none() {
            return Err(REFUSE);
        }
        let evidence = observe_rotation(
            config,
            paths,
            uid,
            generation,
            lock,
            new_store,
            new_template,
        )?;
        if evidence.phase != RotationPhase::DisplacedRetired || evidence.prefix != Step::Done {
            return Err(REFUSE);
        }
        return Ok(FinalSnapshot {
            phase: FinalPhase::BeforeHandoffRetirement,
            evidence,
        });
    }
    let canonical =
        ClosureRecord::decode(&members[0].as_ref().ok_or(REFUSE)?.0).map_err(|_| REFUSE)?;
    if !canonical
        .receipt()
        .terminal()
        .matches_owner_desired(generation, desired.as_ref().map(|v| v.as_slice()))
    {
        return Err(REFUSE);
    }
    if let Some((raw, _)) = &members[3] {
        let receipt = RetirementReceipt::decode(raw).map_err(|_| REFUSE)?;
        if !canonical.matches_pending(&receipt) {
            return Err(REFUSE);
        }
    }
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
    if !output_binding(
        canonical.receipt(),
        [
            &members[4].as_ref().ok_or(REFUSE)?.0,
            &members[5].as_ref().ok_or(REFUSE)?.0,
        ],
        [new_store, new_template],
    ) {
        return Err(REFUSE);
    }
    let phase = if members[3].is_some() {
        FinalPhase::HandoffAbsentReceiptPresent
    } else {
        FinalPhase::HandoffAbsentReceiptAbsent
    };
    members.extend((6..publication::locations().len()).map(|_| None));
    let directories = vec![
        state.metadata().map_err(|_| REFUSE)?,
        config_dir.metadata().map_err(|_| REFUSE)?,
    ];
    for (expected, path) in directories.iter().zip([&paths.state_directory, config]) {
        if !same_directory(
            expected,
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
    Ok(FinalSnapshot {
        phase,
        evidence: Snapshot {
            phase: RotationPhase::DisplacedRetired,
            prefix: Step::Done,
            desired,
            directories,
            members,
        },
    })
}

#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn review_final_closure(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    mut gate: impl FnMut() -> bool,
) -> Result<FinalPhase, ExecutionError> {
    if !gate() {
        return Err(REFUSE);
    }
    let off = backup.restore_store_off().map_err(|_| REFUSE)?;
    let first = observe_final(
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
    let second = observe_final(
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

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::super::tests::second;
    use super::super::tests::setup;
    use super::*;
    use crate::restore_successor_publication_candidate::tests::{Fixture, backup};
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    fn ready(commit: bool) -> (Fixture, MigrationLock) {
        let (f, lock, _, _) = setup(commit);
        publication::publish_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        cleanup::cleanup_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        exchange::exchange_next_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
            .unwrap();
        exchange::displaced::retire_displaced_closure(
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
    fn review(f: &Fixture, lock: &MigrationLock) -> Result<FinalPhase, ExecutionError> {
        crate::restore_cleanup_candidate::without_inventory_sync(|| {
            review_final_closure(&f.config, &f.paths, f.uid, 2, lock, second(), || true)
        })
    }
    fn advance(f: &Fixture, phase: usize) {
        if phase > 0 {
            fs::remove_file(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        }
        if phase > 1 {
            fs::remove_file(f.paths.state_directory.join(RECEIPT_MEMBER)).unwrap();
        }
    }
    fn write(path: &Path, raw: &[u8]) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn all_three_commit_and_abort_phases_are_readonly_output_evidence() {
        for commit in [false, true] {
            let (f, lock) = ready(commit);
            for (phase, expected) in [
                FinalPhase::BeforeHandoffRetirement,
                FinalPhase::HandoffAbsentReceiptPresent,
                FinalPhase::HandoffAbsentReceiptAbsent,
            ]
            .into_iter()
            .enumerate()
            {
                if phase == 1 {
                    fs::remove_file(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
                }
                if phase == 2 {
                    fs::remove_file(f.paths.state_directory.join(RECEIPT_MEMBER)).unwrap();
                }
                let canonical = f.paths.state_directory.join(CLOSURE_MEMBER);
                let bytes = fs::read(&canonical).unwrap();
                let meta = fs::metadata(&canonical).unwrap();
                assert_eq!(review(&f, &lock), Ok(expected));
                assert_eq!(fs::read(&canonical).unwrap(), bytes);
                assert!(same_member(&meta, &fs::metadata(&canonical).unwrap()));
                assert!(canonical.exists()); // Permanent startup fence survives.
            }
        }
    }
    #[test]
    fn wrong_archive_and_handoff_without_receipt_never_supply_completion_authority() {
        for commit in [false, true] {
            for phase in 0..3 {
                let (f, lock) = ready(commit);
                advance(&f, phase);
                assert!(
                    review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, backup(), || true)
                        .is_err()
                );
            }
        }
        let (f, lock) = ready(true);
        fs::remove_file(f.paths.state_directory.join(RECEIPT_MEMBER)).unwrap();
        assert!(review(&f, &lock).is_err());
    }
    #[test]
    fn every_forbidden_fixed_path_and_dangling_symlink_refuses() {
        for phase in 1..3 {
            for index in 0..9 {
                let (f, lock) = ready(true);
                advance(&f, phase);
                let names = [
                    NEXT_CLOSURE_MEMBER,
                    PENDING_DIRECTORY,
                    INTENT,
                    TERMINAL,
                    "routing-preset.pending.json",
                    NEW_SLOT[0],
                    NEW_SLOT[1],
                    OLD_SLOT[0],
                    OLD_SLOT[1],
                ];
                let path = if index < 5 {
                    f.paths.state_directory.join(names[index])
                } else {
                    f.config.join(names[index])
                };
                symlink(f.root.join("missing"), &path).unwrap();
                assert!(review(&f, &lock).is_err());
                fs::remove_file(&path).unwrap();
                let mut pass = 0;
                assert!(
                    review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || {
                        pass += 1;
                        if pass == 2 {
                            write(&path, b"late fixed artifact");
                        }
                        true
                    })
                    .is_err()
                );
            }
        }
    }
    #[test]
    fn optional_receipt_torn_empty_symlink_and_nonprivate_members_never_count_as_absent() {
        for phase in 1..3 {
            for change in 0..4 {
                let (f, lock) = ready(true);
                advance(&f, phase);
                let path = f.paths.state_directory.join(RECEIPT_MEMBER);
                if path.exists() {
                    fs::remove_file(&path).unwrap();
                }
                match change {
                    0 => write(&path, b"torn"),
                    1 => write(&path, b""),
                    2 => symlink(f.root.join("missing"), &path).unwrap(),
                    3 => {
                        write(&path, b"unsafe");
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                    }
                    _ => unreachable!(),
                }
                assert!(review(&f, &lock).is_err());
            }
        }
    }
    #[test]
    fn two_pass_observation_refuses_late_reappearance_substitution_and_gate_change() {
        for phase in 0..3 {
            for change in 0..10 {
                let (f, lock) = ready(true);
                let h = fs::read(f.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
                advance(&f, phase);
                let mut pass = 0;
                let result =
                    review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || {
                        pass += 1;
                        if pass == 2 {
                            match change {
                                0..=3 => {
                                    let path = match change {
                                        0 => f.paths.state_directory.join(CLOSURE_MEMBER),
                                        1 => f.config.join(LIVE[0]),
                                        2 => f.config.join(LIVE[1]),
                                        _ => f.paths.state_directory.join(RECEIPT_MEMBER),
                                    };
                                    if path.exists() {
                                        let raw = fs::read(&path).unwrap();
                                        fs::rename(&path, f.root.join("saved")).unwrap();
                                        write(&path, &raw);
                                    } else {
                                        write(&path, b"torn");
                                    }
                                }
                                4 => write(
                                    &f.paths.state_directory.join(NEXT_CLOSURE_MEMBER),
                                    b"late",
                                ),
                                5 => {
                                    let p = f.paths.state_directory.join(SUCCESSOR_MEMBER);
                                    if p.exists() {
                                        fs::rename(&p, f.root.join("saved")).unwrap();
                                    }
                                    write(&p, &h);
                                }
                                6 => return false,
                                7 => write(
                                    &f.paths.state_directory.join("routing-preset.pending.json"),
                                    b"late",
                                ),
                                8 | 9 => {
                                    let dir = if change == 8 {
                                        &f.config
                                    } else {
                                        &f.paths.state_directory
                                    };
                                    let saved = dir.with_extension("saved");
                                    fs::rename(dir, &saved).unwrap();
                                    fs::create_dir(dir).unwrap();
                                    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                                        .unwrap();
                                    for e in fs::read_dir(saved).unwrap() {
                                        let e = e.unwrap();
                                        write(
                                            &dir.join(e.file_name()),
                                            &fs::read(e.path()).unwrap(),
                                        );
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                        true
                    });
                assert!(result.is_err(), "phase {phase}, change {change}");
            }
        }
    }
    #[test]
    fn pure_output_binding_rejects_crossed_members_and_wrong_abort_stage_even_if_old_equals_new() {
        let old = [b"store".as_slice(), b"template".as_slice()];
        let new = [b"new-store".as_slice(), b"new-template".as_slice()];
        for identical in [false, true] {
            let new = if identical { old } else { new };
            let stage = planned_stage_identity([old[0], old[1], new[0], new[1]]).unwrap();
            let intent = DecisionRecord::intent(2, None, &stage, [77; 16]).unwrap();
            for choice in [TerminalChoice::Commit, TerminalChoice::Abort] {
                let terminal = intent.terminal(choice).unwrap();
                let live = if terminal.phase() == DecisionPhase::Committed {
                    new
                } else {
                    old
                };
                let receipt = RetirementReceipt::synthetic(&terminal, live[0], live[1]);
                assert!(output_binding(&receipt, live, new));
                assert!(!output_binding(&receipt, [live[1], live[0]], new));
                assert!(!output_binding(&receipt, live, [new[1], new[0]]));
                if terminal.phase() == DecisionPhase::Aborted {
                    let wrong =
                        planned_stage_identity([old[0], old[1], b"foreign", new[1]]).unwrap();
                    let wrong = DecisionRecord::intent(2, None, &wrong, [77; 16])
                        .unwrap()
                        .terminal(TerminalChoice::Abort)
                        .unwrap();
                    let wrong = RetirementReceipt::synthetic(&wrong, old[0], old[1]);
                    assert!(!output_binding(&wrong, old, new));
                }
            }
        }
    }
    #[test]
    fn receipt_must_be_the_exact_terminal_wrapped_by_canonical_closure() {
        for phase in [DecisionPhase::Committed, DecisionPhase::Aborted] {
            for changed_transaction in [false, true] {
                let (f, lock) = ready(true);
                advance(&f, 1);
                let live = [
                    fs::read(f.config.join(LIVE[0])).unwrap(),
                    fs::read(f.config.join(LIVE[1])).unwrap(),
                ];
                let old = backup().restore_store_off().unwrap();
                let stage = planned_stage_identity([&old, backup().template(), &live[0], &live[1]])
                    .unwrap();
                let intent = DecisionRecord::intent(
                    2,
                    None,
                    &stage,
                    if changed_transaction {
                        [99; 16]
                    } else {
                        [32; 16]
                    },
                )
                .unwrap();
                let terminal = intent
                    .terminal(if phase == DecisionPhase::Committed {
                        TerminalChoice::Commit
                    } else {
                        TerminalChoice::Abort
                    })
                    .unwrap();
                let receipt = RetirementReceipt::synthetic(&terminal, &live[0], &live[1]);
                write(
                    &f.paths.state_directory.join(RECEIPT_MEMBER),
                    &receipt.encode(),
                );
                if phase == DecisionPhase::Committed && !changed_transaction {
                    assert_eq!(
                        review(&f, &lock),
                        Ok(FinalPhase::HandoffAbsentReceiptPresent)
                    );
                } else {
                    assert!(review(&f, &lock).is_err());
                }
            }
        }
    }
}
