// SPDX-License-Identifier: MIT
use super::*;
use crate::restore_decision_candidate::{DecisionRecord, TerminalChoice};
use crate::restore_executor_candidate::successor::tests::second;
use crate::restore_retirement_candidate::RetirementReceipt;
use crate::restore_staging_candidate::{planned_stage_identity, stage_private_pair};
use crate::restore_successor_publication_candidate::tests::Fixture;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
pub(crate) fn final_shape(commit: bool) -> (Fixture, MigrationLock, Ticket, CompleteRecord) {
    let f = Fixture::new();
    let lock = f.lock();
    let old_store = fs::read(f.config.join("profiles.json")).unwrap();
    let old_template = fs::read(f.config.join("route-template.yaml")).unwrap();
    let new = second().restore_store_off().unwrap();
    let stage =
        planned_stage_identity([&old_store, &old_template, &new, second().template()]).unwrap();
    let terminal = DecisionRecord::intent(2, None, &stage, [25; 16])
        .unwrap()
        .terminal(if commit {
            TerminalChoice::Commit
        } else {
            TerminalChoice::Abort
        })
        .unwrap();
    let live = if commit {
        [&*new, second().template()]
    } else {
        [&*old_store, &*old_template]
    };
    let closure = ClosureRecord::from_verified_receipt(&RetirementReceipt::synthetic(
        &terminal, live[0], live[1],
    ))
    .unwrap();
    write(
        &f.paths.state_directory.join(CLOSURE_MEMBER),
        &closure.encode(),
    );
    write(&f.config.join("profiles.json"), live[0]);
    write(&f.config.join("route-template.yaml"), live[1]);
    let ticket = Ticket::from_bound_closure(&closure, f.uid, 2, None).unwrap();
    let complete = CompleteRecord::from_ticket(&ticket).unwrap();
    (f, lock, ticket, complete)
}
fn inspect(f: &Fixture, lock: &MigrationLock) -> Result<Preview> {
    inspect_authenticated(&f.config, &f.paths, f.uid, lock, second(), [42; 32])
}

#[test]
fn recovery_preview_late_same_byte_replacement_and_catalogue_change_are_not_stable() {
    for replacement in [true, false] {
        let (f, lock, _, _) = final_shape(true);
        let result = inspect_with_hook(
            &f.config,
            &f.paths,
            f.uid,
            &lock,
            second(),
            [42; 32],
            || {
                if replacement {
                    let path = f.paths.state_directory.join(CLOSURE_MEMBER);
                    let bytes = fs::read(&path).unwrap();
                    fs::rename(&path, f.root.join("old-inode")).unwrap();
                    write(&path, &bytes);
                } else {
                    write(
                        &f.paths.state_directory.join("restore-unknown-late"),
                        b"unknown",
                    );
                }
            },
        );
        assert!(result.is_err());
        assert!(lock.authorizes(&f.paths, f.uid));
        assert!(!f.paths.state_directory.join(COMPLETE_MEMBER).exists());
    }
}

#[cfg(feature = "tui")]
#[test]
fn recovery_consumer_missing_preview_foreign_nonce_and_spent_worker_do_not_execute() {
    use omavless_tui::private_restore::{RecoveryResult, State, Workspace};
    let edit = || {
        let mut w = Workspace::new_recovery().unwrap();
        assert!(w.paste("/private/synthetic.ovb"));
        w.next_field();
        assert!(w.paste("synthetic source password"));
        w
    };
    for foreign in [false, true] {
        let mut w = edit();
        let r = w.begin_preview().unwrap();
        let nonce = r.recovery_input().unwrap().0;
        w.accept(r.settle_recovery(RecoveryResult::Preview {
            profiles: 0,
            subscriptions: 0,
            keep_new: true,
        }));
        let mut consumer = Consumer::default();
        if foreign {
            let (f, lock, _, _) = final_shape(true);
            consumer.preview = Some((nonce + 1, inspect(&f, &lock).unwrap()));
        }
        let confirm = w.submit("synthetic-operation".into()).unwrap();
        w.accept(consumer.execute(confirm));
        assert_eq!(w.state(), State::Denied);
        assert!(!consumer.submitted);
    }
    let mut w = edit();
    let mut consumer = Consumer {
        preview: None,
        submitted: true,
    };
    let r = w.begin_preview().unwrap();
    w.accept(consumer.execute(r));
    assert_eq!(w.state(), State::PreviewUnavailable);
    assert!(consumer.submitted);
}

#[test]
fn recovery_preview_closed_final_old_new_ticket_and_complete_shapes_are_data_only() {
    for commit in [false, true] {
        let (f, lock, ticket, complete) = final_shape(commit);
        let live = fs::read(f.config.join("profiles.json")).unwrap();
        let closure = fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let before = inspect(&f, &lock).unwrap();
        assert_eq!(
            before.choice(),
            if commit {
                Choice::FinalNew
            } else {
                Choice::FinalOld
            }
        );
        assert!(before.same(&inspect(&f, &lock).unwrap()));
        assert!(lock.authorizes(&f.paths, f.uid));
        assert_eq!(fs::read(f.config.join("profiles.json")).unwrap(), live);
        assert_eq!(
            fs::read(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
            closure
        );
        assert!(!f.paths.state_directory.join(TICKET_MEMBER).exists());
        assert!(!f.paths.state_directory.join(COMPLETE_MEMBER).exists());
        write(
            &f.paths.state_directory.join(TICKET_MEMBER),
            &ticket.encode(),
        );
        if !commit {
            assert!(inspect(&f, &lock).is_err());
            continue;
        }
        assert_eq!(inspect(&f, &lock).unwrap().choice(), Choice::NewTicket);
        write(
            &f.paths.state_directory.join(COMPLETE_MEMBER),
            &complete.encode(),
        );
        assert_eq!(inspect(&f, &lock).unwrap().choice(), Choice::NewComplete);
        fs::remove_file(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        assert_eq!(inspect(&f, &lock).unwrap().choice(), Choice::NewComplete);
        fs::remove_file(f.paths.state_directory.join(TICKET_MEMBER)).unwrap();
        assert_eq!(inspect(&f, &lock).unwrap().choice(), Choice::NewComplete);
        assert!(!f.paths.state_directory.join(HISTORY[0]).exists());
    }
}

#[test]
fn recovery_preview_staged_intent_old_mixed_new_and_committed_have_fixed_outcomes() {
    for (new_store, new_template, terminal, expected) in [
        (false, false, false, Choice::Old),
        (true, false, false, Choice::Mixed),
        (true, true, false, Choice::Mixed),
        (true, true, true, Choice::Committed),
    ] {
        let f = Fixture::new();
        let lock = f.lock();
        fs::remove_file(f.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        let old_store = fs::read(f.config.join("profiles.json")).unwrap();
        let old_template = fs::read(f.config.join("route-template.yaml")).unwrap();
        let new = second().restore_store_off().unwrap();
        stage_private_pair(
            &f.paths.state_directory,
            f.uid,
            &old_store,
            &old_template,
            &new,
            second().template(),
        )
        .unwrap();
        let stage =
            planned_stage_identity([&old_store, &old_template, &new, second().template()]).unwrap();
        let intent = DecisionRecord::intent(2, None, &stage, [29; 16]).unwrap();
        write(
            &f.paths.state_directory.join("restore-decision.intent"),
            &intent.encode(),
        );
        if new_store {
            write(&f.config.join("profiles.json"), &new);
        }
        if new_template {
            write(&f.config.join("route-template.yaml"), second().template());
        }
        if terminal {
            write(
                &f.paths.state_directory.join("restore-decision.terminal"),
                &intent.terminal(TerminalChoice::Commit).unwrap().encode(),
            );
        }
        let before = inspect(&f, &lock).unwrap();
        assert_eq!(before.choice(), expected);
        assert!(before.same(&inspect(&f, &lock).unwrap()));
        assert!(!f.paths.state_directory.join(COMPLETE_MEMBER).exists());
    }
}

#[test]
fn recovery_preview_history_capacity_holes_unknown_names_and_unsafe_members_refuse() {
    let (f, lock, _, complete) = final_shape(true);
    for name in HISTORY.iter().take(7) {
        write(&f.paths.state_directory.join(name), &complete.encode());
        assert_eq!(inspect(&f, &lock).unwrap().choice(), Choice::FinalNew);
    }
    write(
        &f.paths.state_directory.join(HISTORY[7]),
        &complete.encode(),
    );
    assert!(inspect(&f, &lock).is_err());
    fs::remove_file(f.paths.state_directory.join(HISTORY[3])).unwrap();
    assert!(inspect(&f, &lock).is_err());
    for variant in 0..7 {
        let (f, lock, _, _) = final_shape(true);
        let closure = f.paths.state_directory.join(CLOSURE_MEMBER);
        match variant {
            0 => write(&f.paths.state_directory.join("restore-unknown"), b"unknown"),
            1 => write(&f.config.join("restore-unknown"), b"unknown"),
            2 => write(
                &f.paths
                    .state_directory
                    .join("restore-disposition.history.extra"),
                b"unknown",
            ),
            3 => fs::set_permissions(&closure, fs::Permissions::from_mode(0o644)).unwrap(),
            4 => {
                fs::remove_file(&closure).unwrap();
                symlink("missing", &closure).unwrap();
            }
            5 => fs::hard_link(&closure, f.paths.state_directory.join("alias")).unwrap(),
            _ => write(&closure, b"torn"),
        }
        assert!(inspect(&f, &lock).is_err(), "variant {variant}");
    }
}

#[test]
fn recovery_preview_binding_changes_on_same_byte_inode_ciphertext_generation_and_desired_change() {
    let (f, lock, _, _) = final_shape(true);
    let original = inspect(&f, &lock).unwrap();
    let closure = f.paths.state_directory.join(CLOSURE_MEMBER);
    let bytes = fs::read(&closure).unwrap();
    fs::rename(&closure, f.root.join("displaced")).unwrap();
    write(&closure, &bytes);
    assert!(!original.same(&inspect(&f, &lock).unwrap()));
    let other_digest =
        inspect_authenticated(&f.config, &f.paths, f.uid, &lock, second(), [43; 32]).unwrap();
    assert!(!original.same(&other_digest));
    write(
        &f.paths.ownership_marker,
        br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
    );
    assert!(inspect(&f, &lock).is_err());
    write(
        &f.paths.ownership_marker,
        br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
    );
    write(
        &f.paths.state_directory.join("desired.json"),
        br#"{"schemaVersion":1,"generation":1,"connected":true}"#,
    );
    assert!(inspect(&f, &lock).is_err());
}
