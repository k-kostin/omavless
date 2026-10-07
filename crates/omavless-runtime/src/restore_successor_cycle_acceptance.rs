// SPDX-License-Identifier: MIT
//! Synthetic composition only: real first restore, second closure with repeated
//! process loss, then a full third cycle reusing the same fixed names. This does
//! not activate a product caller or prove installed/power-loss acceptance.
use super::tests::ready;
use super::*;
use crate::restore_executor_candidate::successor as executor;
use crate::restore_successor_coexistence_candidate::preparation::prepare_successor;
use crate::restore_successor_publication_candidate::{
    publish_successor_handoff,
    tests::{Fixture, backup},
};
use executor::rotation;
use executor::tests::second;
use std::os::unix::process::ExitStatusExt;
use std::{fs, path::PathBuf, process::Command};

fn crash_second_closure(f: &Fixture, mode: &str) {
    let output=Command::new(std::env::current_exe().unwrap())
        .args(["--ignored","--exact","restore_executor_candidate::successor::rotation::final_review::handoff_retirement::last_receipt::cycle_acceptance::cycle_crash_worker"])
        .env("OMAVLESS_SYNTHETIC_CYCLE_ROOT",&f.root)
        .env("OMAVLESS_SYNTHETIC_CYCLE_CRASH",mode)
        .output().unwrap();
    assert_eq!(output.status.signal(), Some(9));
}
fn fixed_absences(f: &Fixture) {
    for name in [
        NEXT_CLOSURE_MEMBER,
        SUCCESSOR_MEMBER,
        RECEIPT_MEMBER,
        PENDING_DIRECTORY,
        INTENT,
        TERMINAL,
        "routing-preset.pending.json",
    ] {
        assert_eq!(
            fs::symlink_metadata(f.paths.state_directory.join(name))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
    for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
        assert_eq!(
            fs::symlink_metadata(f.config.join(name))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
}

#[test]
fn three_cycles_reuse_fixed_names_after_repeated_second_closure_process_loss() {
    for second_commit in [false, true] {
        for third_commit in [false, true] {
            // This fixture is a real first stage/execution/receipt/completion,
            // then second handoff/preparation/execution/rotation/H retirement.
            // It is not a synthesized already-closed predecessor.
            let (f, lock) = ready(second_commit);
            let c1 = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let c1_identity = fs::metadata(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let second_output = [
                fs::read(f.config.join(LIVE[0])).unwrap(),
                fs::read(f.config.join(LIVE[1])).unwrap(),
            ];
            drop(lock);
            crash_second_closure(&f, "last-unlink");
            crash_second_closure(
                &f,
                if third_commit {
                    "sync-canonical"
                } else {
                    "sync-directory"
                },
            );
            let lock = f.lock();
            resync_completed_successor(&f.config, &f.paths, f.uid, 2, &lock, second(), || true)
                .unwrap();
            assert_eq!(
                fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                c1
            );
            assert!(same_member(
                &c1_identity,
                &fs::metadata(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap()
            ));
            fixed_absences(&f);
            assert_eq!(
                review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, second(), || true),
                Ok(FinalPhase::HandoffAbsentReceiptAbsent)
            );

            // Always change the actual live pair. In the committed case this
            // archive differs from the second restore archive, so it cannot be
            // misused as input to the prior archive-specific final reader.
            let third = if second_commit { backup() } else { second() };
            assert!(
                publish_successor_handoff(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    third,
                    [32; 16],
                    || true
                )
                .is_err()
            );
            assert!(!f.paths.state_directory.join(SUCCESSOR_MEMBER).exists());
            publish_successor_handoff(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                third,
                [33; 16],
                || true,
            )
            .unwrap();
            prepare_successor(&f.config, &f.paths, f.uid, 2, &lock, third, || true).unwrap();
            if third_commit {
                executor::execute_successor(&f.config, &f.paths, f.uid, 2, &lock, third, || true)
                    .unwrap();
            } else {
                executor::recover_successor(&f.config, &f.paths, f.uid, 2, &lock, third, || true)
                    .unwrap();
            }
            executor::receipt::publish_successor_receipt(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                third,
                || true,
            )
            .unwrap();
            rotation::publication::publish_next_closure(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                third,
                || true,
            )
            .unwrap();
            rotation::cleanup::cleanup_successor(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                third,
                || true,
            )
            .unwrap();
            rotation::exchange::exchange_next_closure(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                third,
                || true,
            )
            .unwrap();
            rotation::exchange::displaced::retire_displaced_closure(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                third,
                || true,
            )
            .unwrap();
            retire_successor_handoff(&f.config, &f.paths, f.uid, 2, &lock, third, || true).unwrap();
            retire_last_successor_receipt(&f.config, &f.paths, f.uid, 2, &lock, third, || true)
                .unwrap();
            drop(lock);
            let lock = f.lock();
            resync_completed_successor(&f.config, &f.paths, f.uid, 2, &lock, third, || true)
                .unwrap();
            fixed_absences(&f);
            assert_eq!(
                review_final_closure(&f.config, &f.paths, f.uid, 2, &lock, third, || true),
                Ok(FinalPhase::HandoffAbsentReceiptAbsent)
            );
            let final_record = ClosureRecord::decode(
                &fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            )
            .unwrap();
            assert_eq!(final_record.receipt().terminal().transaction_id(), [33; 16]);
            assert_eq!(
                final_record.receipt().terminal().phase(),
                if third_commit {
                    DecisionPhase::Committed
                } else {
                    DecisionPhase::Aborted
                }
            );
            let expected = if third_commit {
                [
                    third.restore_store_off().unwrap().to_vec(),
                    third.template().to_vec(),
                ]
            } else {
                second_output
            };
            assert!(
                final_record
                    .receipt()
                    .matches_pair(&expected[0], &expected[1])
            );
            for (index, name) in LIVE.into_iter().enumerate() {
                assert_eq!(fs::read(f.config.join(name)).unwrap(), expected[index]);
            }
            assert!(f.paths.state_directory.join(CLOSURE_MEMBER).exists()); // Still fences startup.
        }
    }
}

#[test]
#[ignore = "internal synthetic three-cycle crash worker"]
fn cycle_crash_worker() {
    let f = std::mem::ManuallyDrop::new(Fixture::reopen(PathBuf::from(
        std::env::var_os("OMAVLESS_SYNTHETIC_CYCLE_ROOT").unwrap(),
    )));
    let mode = std::env::var("OMAVLESS_SYNTHETIC_CYCLE_CRASH").unwrap();
    let lock = f.lock();
    let operation = if mode == "last-unlink" {
        Operation::Retire
    } else {
        Operation::ResyncCompleted
    };
    let _ = run(
        &f.config,
        &f.paths,
        f.uid,
        2,
        &lock,
        second(),
        operation,
        || true,
        |point| {
            let kill = match mode.as_str() {
                "last-unlink" => point == Checkpoint::Unlinked,
                "sync-canonical" => point == Checkpoint::SourceSynced(0),
                "sync-directory" => point == Checkpoint::DirectorySynced(0),
                _ => panic!("unknown synthetic crash mode"),
            };
            if kill {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            true
        },
    );
    panic!("selected process-death checkpoint not reached");
}
