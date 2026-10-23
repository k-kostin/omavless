// SPDX-License-Identifier: MIT
use super::*;
use crate::desired::{DesiredState, OwnedObservation, write_desired};
use crate::lifecycle::{HostStepError, NativeLocalObservation};
use crate::mutation::MutationDigest;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::sync::Arc;

const PASSWORD: &[u8] = b"synthetic fixture passphrase";
const PORTABLE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
fn ticket_new_input(f: &Fixture) {
    private(
        &f.store.with_file_name("route-template.yaml"),
        include_bytes!("../../../../templates/default.yaml"),
    );
    let mut incoming: serde_json::Value = serde_json::from_slice(PORTABLE).unwrap();
    incoming["routingPreset"] = "china-cn-direct".into();
    private(
        &f.backup,
        &omavless_domain::private_backup::seal(
            &serde_json::to_vec(&incoming).unwrap(),
            include_bytes!("../../../../templates/china.yaml"),
            PASSWORD,
        )
        .unwrap(),
    );
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_ticket_real_cut_and_always_ok_publisher_never_complete_or_handoff() {
    for noop in [false, true] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        let original = fs::File::open(&f.store).unwrap();
        let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
        let expected = opened.restore_store_off().unwrap();
        let result = if noop {
            f.owner
                .test_ticket_cut_callback(&f.backup, PASSWORD, |_| Ok(()))
        } else {
            f.owner.cut_current_committed_ticket(&f.backup, PASSWORD)
        };
        assert_eq!(
            result,
            if noop {
                Err(FirstError::StillFenced)
            } else {
                Ok(())
            }
        );
        assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
        assert!(f.owner.held_restore_execution.test_ticket_cut_unavailable());
        assert_eq!(f.owner.revision(), 0);
        assert_eq!(original.metadata().unwrap().nlink(), 0);
        assert_eq!(fs::read(&f.store).unwrap(), expected.as_slice());
        let closure = crate::restore_closure_model::ClosureRecord::decode(
            &fs::read(f.state().join(crate::restore_closure_model::CLOSURE_MEMBER)).unwrap(),
        )
        .unwrap();
        let ticket = crate::restore_disposition_ticket_model::Ticket::decode(
            &fs::read(
                f.state()
                    .join(crate::restore_disposition_ticket_model::TICKET_MEMBER),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(closure.receipt().matches_pair(&expected, opened.template()));
        assert_eq!(
            closure.receipt().terminal().phase(),
            crate::restore_decision_candidate::DecisionPhase::Committed
        );
        let marker = crate::cutover::read_marker_existing(
            f.owner.transaction.cutover_paths(),
            f.owner.uid(),
        )
        .unwrap();
        assert!(ticket.matches(
            &closure,
            f.owner.uid(),
            marker.generation(),
            Some(&fs::read(&f.owner.transaction.desired_paths().file).unwrap())
        ));
        for name in [
            "restore-pair.pending",
            "restore-decision.intent",
            "restore-decision.terminal",
            "restore-finalization.pending",
            "restore-disposition.complete",
            "restore-disposition.history",
        ] {
            assert!(fs::symlink_metadata(f.state().join(name)).is_err());
        }
        let paths = f.owner.transaction.cutover_paths().clone();
        assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
        let before = f.owner.held_restore_execution.test_original_identity();
        assert!(
            f.owner
                .cut_current_committed_ticket(&f.backup, PASSWORD)
                .is_err()
        );
        assert_eq!(
            before,
            f.owner.held_restore_execution.test_original_identity()
        );
        let (mut reply, lost) = std::os::unix::net::UnixStream::pair().unwrap();
        drop(lost);
        assert!(
            crate::developer_current_restore::publish_positive_cut_response(
                crate::success_response(
                    "ticket-cut",
                    0,
                    serde_json::json!({"cutReachedStillFenced":true})
                ),
                &mut reply,
            )
            .is_err()
        );
        assert!(
            f.owner.held_restore_execution.test_ticket_cut_unavailable()
                && f.owner.transaction.independently_blocked()
        );
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_ticket_panic_and_error_before_or_after_ticket_always_block_the_admitted_graph() {
    for (point, panic) in [
        (crate::manager_actor_service::NativeStep::StageReady, true),
        (
            crate::manager_actor_service::NativeStep::DispositionTicket,
            false,
        ),
        (
            crate::manager_actor_service::NativeStep::DispositionTicket,
            true,
        ),
    ] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        assert!(
            f.owner
                .test_ticket_cut_callback(&f.backup, PASSWORD, |step| {
                    if step == point {
                        if panic {
                            panic!("source fixed ticket-cut unwind");
                        }
                        return Err(FirstError::StillFenced);
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
        assert_eq!(f.owner.revision(), 0);
        assert!(
            !f.state()
                .join(crate::restore_disposition_complete_model::COMPLETE_MEMBER)
                .exists()
        );
        assert!(!f.state().join("restore-disposition.history").exists());
        let before = f.owner.held_restore_execution.test_original_identity();
        assert!(
            f.owner
                .cut_current_committed_ticket(&f.backup, PASSWORD)
                .is_err()
        );
        assert_eq!(
            before,
            f.owner.held_restore_execution.test_original_identity()
        );
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_ticket_actual_partial_write_error_and_unwind_keep_fence_ticket_fd_and_block() {
    for panic in [false, true] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        f.owner
            .held_restore_execution
            .test_reserve_ticket_write_fault(panic);
        assert!(
            f.owner
                .cut_current_committed_ticket(&f.backup, PASSWORD)
                .is_err()
        );
        assert!(f.owner.held_restore_execution.test_ticket_cut_unavailable());
        assert!(f.owner.transaction.independently_blocked());
        let raw = fs::read(
            f.state()
                .join(crate::restore_disposition_ticket_model::TICKET_MEMBER),
        )
        .unwrap();
        assert_eq!(raw.len(), 8);
        assert!(crate::restore_disposition_ticket_model::Ticket::decode(&raw).is_none());
        assert!(
            f.state()
                .join(crate::restore_closure_model::CLOSURE_MEMBER)
                .exists()
        );
        assert!(
            !f.state()
                .join(crate::restore_disposition_complete_model::COMPLETE_MEMBER)
                .exists()
        );
        let paths = f.owner.transaction.cutover_paths().clone();
        assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_ticket_busy_pause_auth_and_queue_refusals_leave_previous_or_unsubmitted_state_untouched()
 {
    let mut paused = Fixture::new();
    paused
        .owner
        .pause_current_restore_intent(&paused.backup, PASSWORD)
        .unwrap();
    let before = paused.owner.held_restore_execution.test_original_identity();
    assert!(
        paused
            .owner
            .cut_current_committed_ticket(Path::new("/public/absent"), b"wrong")
            .is_err()
    );
    assert_eq!(
        before,
        paused.owner.held_restore_execution.test_original_identity()
    );
    assert!(
        paused.owner.current_intent_paused() && !paused.owner.transaction.independently_blocked()
    );
    paused.owner.abort_current_restore_intent().unwrap();
    let mut f = Fixture::new();
    assert_eq!(
        f.owner.cut_current_committed_ticket(&f.backup, b"wrong"),
        Err(FirstError::Prepare)
    );
    assert!(
        !f.owner.held_restore_execution.occupied() && !f.owner.transaction.independently_blocked()
    );
    use crate::mutation::{MutationKind, MutationRequest};
    f.owner
        .coordinator
        .submit(
            MutationRequest::new(
                MutationKind::Other,
                Some("queued"),
                Some(0),
                MutationDigest::from_semantic_bytes(b"source queued ticket"),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        f.owner
            .cut_current_committed_ticket(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(
        !f.owner.held_restore_execution.occupied() && !f.owner.transaction.independently_blocked()
    );
}

#[cfg(feature = "product-private-backup")]
#[test]
fn native_new_ticket_product_direct_producer_denies_before_acquisition() {
    let mut f = Fixture::new();
    assert_eq!(
        f.owner
            .cut_current_committed_ticket(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(!f.owner.held_restore_execution.occupied() && !f.stage().exists());
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_complete_real_all_three_cut_and_noop_never_unlink_complete_or_grant() {
    for noop in [false, true] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
        let expected = opened.restore_store_off().unwrap();
        let result = if noop {
            f.owner
                .test_complete_cut_callback(&f.backup, PASSWORD, |_| Ok(()))
        } else {
            f.owner.cut_current_committed_complete(&f.backup, PASSWORD)
        };
        assert_eq!(
            result,
            if noop {
                Err(FirstError::StillFenced)
            } else {
                Ok(())
            }
        );
        assert!(
            f.owner
                .held_restore_execution
                .test_complete_cut_unavailable()
        );
        assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
        assert_eq!(f.owner.revision(), 0);
        assert_eq!(fs::read(&f.store).unwrap(), expected.as_slice());
        let state = f.state();
        let closure = crate::restore_closure_model::ClosureRecord::decode(
            &fs::read(state.join(crate::restore_closure_model::CLOSURE_MEMBER)).unwrap(),
        )
        .unwrap();
        let ticket = crate::restore_disposition_ticket_model::Ticket::decode(
            &fs::read(state.join(crate::restore_disposition_ticket_model::TICKET_MEMBER)).unwrap(),
        )
        .unwrap();
        let complete = crate::restore_disposition_complete_model::CompleteRecord::decode(
            &fs::read(state.join(crate::restore_disposition_complete_model::COMPLETE_MEMBER))
                .unwrap(),
        )
        .unwrap();
        let marker = crate::cutover::read_marker_existing(
            f.owner.transaction.cutover_paths(),
            f.owner.uid(),
        )
        .unwrap();
        assert!(ticket.matches(
            &closure,
            f.owner.uid(),
            marker.generation(),
            Some(&fs::read(&f.owner.transaction.desired_paths().file).unwrap())
        ));
        assert!(
            complete.matches_ticket(&ticket)
                && closure.receipt().matches_pair(&expected, opened.template())
        );
        for name in [
            "restore-pair.pending",
            "restore-decision.intent",
            "restore-decision.terminal",
            "restore-finalization.pending",
            "restore-disposition.history",
        ] {
            assert!(fs::symlink_metadata(state.join(name)).is_err());
        }
        let before = f.owner.held_restore_execution.test_original_identity();
        assert!(
            f.owner
                .cut_current_committed_complete(&f.backup, PASSWORD)
                .is_err()
        );
        assert_eq!(
            before,
            f.owner.held_restore_execution.test_original_identity()
        );
        assert!(
            MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
                .is_err()
        );
        let (mut reply, lost) = std::os::unix::net::UnixStream::pair().unwrap();
        drop(lost);
        assert!(
            crate::developer_current_restore::publish_positive_cut_response(
                crate::success_response(
                    "complete-cut",
                    0,
                    serde_json::json!({"cutReachedStillFenced":true})
                ),
                &mut reply,
            )
            .is_err()
        );
        assert!(
            f.owner
                .held_restore_execution
                .test_complete_cut_unavailable()
                && f.owner.transaction.independently_blocked()
        );
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_complete_genuine_current_owner_cut_preserves_prior_audit_before_stage() {
    let (past_fixture, past_lock, _, past) =
        crate::restore_recovery_preview::tests::final_shape(true);
    let raw = past.encode();
    drop((past_lock, past_fixture));
    for prior in [1, 7, 8] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        let mut originals = Vec::new();
        for index in 0..prior {
            let name = if index == 0 {
                "restore-disposition.history".to_owned()
            } else {
                format!("restore-disposition.history.{index}")
            };
            let path = f.state().join(name);
            private(&path, &raw);
            originals.push((path.clone(), fs::metadata(path).unwrap().ino()));
        }
        let result = f.owner.cut_current_committed_complete(&f.backup, PASSWORD);
        assert_eq!(
            result.is_ok(),
            prior < 8,
            "genuine producer with {prior} audits: {result:?}"
        );
        for (path, inode) in originals {
            assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
            assert_eq!(fs::read(path).unwrap(), raw);
        }
        assert!(!f.stage().exists());
        for name in [
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
        ] {
            assert_eq!(f.state().join(name).exists(), prior < 8);
        }
        if prior < 8 {
            assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
        }
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_complete_actual_partial_complete_write_error_and_unwind_retain_all_three() {
    for panic in [false, true] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        f.owner
            .held_restore_execution
            .test_reserve_complete_write_fault(panic);
        assert!(
            f.owner
                .cut_current_committed_complete(&f.backup, PASSWORD)
                .is_err()
        );
        assert!(
            f.owner
                .held_restore_execution
                .test_complete_cut_unavailable()
        );
        assert!(f.owner.transaction.independently_blocked());
        let state = f.state();
        let raw = fs::read(state.join(crate::restore_disposition_complete_model::COMPLETE_MEMBER))
            .unwrap();
        assert_eq!(raw.len(), 8);
        assert!(crate::restore_disposition_complete_model::CompleteRecord::decode(&raw).is_none());
        assert!(
            state
                .join(crate::restore_closure_model::CLOSURE_MEMBER)
                .exists()
        );
        assert!(
            state
                .join(crate::restore_disposition_ticket_model::TICKET_MEMBER)
                .exists()
        );
        assert!(!state.join("restore-disposition.history").exists());
        assert!(
            MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
                .is_err()
        );
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_complete_panic_before_capability_and_after_complete_never_yields_cut_success() {
    for point in [
        crate::manager_actor_service::NativeStep::StageReady,
        crate::manager_actor_service::NativeStep::DispositionComplete,
    ] {
        let mut f = Fixture::new();
        ticket_new_input(&f);
        assert!(
            f.owner
                .test_complete_cut_callback(&f.backup, PASSWORD, |step| {
                    if step == point {
                        panic!("source fixed Complete cut unwind");
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
        assert_eq!(f.owner.revision(), 0);
        assert!(!f.state().join("restore-disposition.history").exists());
        assert!(
            f.owner
                .cut_current_committed_complete(&f.backup, PASSWORD)
                .is_err()
        );
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_new_complete_prior_pause_authentication_and_queue_are_not_submitted_or_touched() {
    let mut paused = Fixture::new();
    paused
        .owner
        .pause_current_restore_intent(&paused.backup, PASSWORD)
        .unwrap();
    let before = paused.owner.held_restore_execution.test_original_identity();
    assert!(
        paused
            .owner
            .cut_current_committed_complete(Path::new("/public/absent"), b"wrong")
            .is_err()
    );
    assert_eq!(
        before,
        paused.owner.held_restore_execution.test_original_identity()
    );
    assert!(
        paused.owner.current_intent_paused() && !paused.owner.transaction.independently_blocked()
    );
    paused.owner.abort_current_restore_intent().unwrap();
    let mut f = Fixture::new();
    assert_eq!(
        f.owner.cut_current_committed_complete(&f.backup, b"wrong"),
        Err(FirstError::Prepare)
    );
    assert!(
        !f.owner.held_restore_execution.occupied() && !f.owner.transaction.independently_blocked()
    );
    use crate::mutation::{MutationKind, MutationRequest};
    f.owner
        .coordinator
        .submit(
            MutationRequest::new(
                MutationKind::Other,
                Some("queued"),
                Some(0),
                MutationDigest::from_semantic_bytes(b"source queued Complete"),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        f.owner
            .cut_current_committed_complete(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(
        !f.owner.held_restore_execution.occupied() && !f.owner.transaction.independently_blocked()
    );
}

#[cfg(feature = "product-private-backup")]
#[test]
fn native_new_complete_product_direct_producer_denies_before_acquisition() {
    let mut f = Fixture::new();
    assert_eq!(
        f.owner
            .cut_current_committed_complete(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(!f.owner.held_restore_execution.occupied() && !f.stage().exists());
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_final_new_real_committed_retirement_cut_and_unreached_callback_never_grant_completion() {
    for no_callback in [false, true] {
        let mut f = Fixture::new();
        let template = f.store.with_file_name("route-template.yaml");
        private(
            &template,
            include_bytes!("../../../../templates/default.yaml"),
        );
        let mut incoming: serde_json::Value = serde_json::from_slice(PORTABLE).unwrap();
        incoming["routingPreset"] = "china-cn-direct".into();
        private(
            &f.backup,
            &omavless_domain::private_backup::seal(
                &serde_json::to_vec(&incoming).unwrap(),
                include_bytes!("../../../../templates/china.yaml"),
                PASSWORD,
            )
            .unwrap(),
        );
        let old_store = fs::File::open(&f.store).unwrap();
        let old_template = fs::File::open(&template).unwrap();
        let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
        let expected = opened.restore_store_off().unwrap();
        let result = if no_callback {
            f.owner
                .test_unreached_committed_retirement_cut(&f.backup, PASSWORD)
        } else {
            f.owner
                .cut_current_committed_retirement(&f.backup, PASSWORD)
        };
        assert_eq!(
            result,
            if no_callback {
                Err(FirstError::StillFenced)
            } else {
                Ok(())
            }
        );
        assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
        assert_eq!(f.owner.revision(), 0);
        assert_eq!(fs::read(&f.store).unwrap(), expected.as_slice());
        assert_eq!(fs::read(&template).unwrap(), opened.template());
        assert_eq!(old_store.metadata().unwrap().nlink(), 0);
        assert_eq!(old_template.metadata().unwrap().nlink(), 0);
        let state = f.state();
        for name in [
            "restore-pair.pending",
            "restore-decision.intent",
            "restore-decision.terminal",
            "restore-finalization.pending",
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            "restore-disposition.complete",
            "restore-disposition.history",
        ] {
            assert!(
                fs::symlink_metadata(state.join(name)).is_err(),
                "public cut absence {name}"
            );
        }
        let closure = crate::restore_closure_model::ClosureRecord::decode(
            &fs::read(state.join(crate::restore_closure_model::CLOSURE_MEMBER)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            closure.receipt().terminal().phase(),
            crate::restore_decision_candidate::DecisionPhase::Committed
        );
        assert!(closure.receipt().matches_pair(&expected, opened.template()));
        assert!(f.owner.held_restore_execution.test_cut_only_unavailable());
        let paths = f.owner.transaction.cutover_paths().clone();
        assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
        assert!(
            f.owner
                .cut_current_committed_retirement(&f.backup, PASSWORD)
                .is_err()
        );
        let (mut reply, lost) = std::os::unix::net::UnixStream::pair().unwrap();
        drop(lost);
        assert!(
            crate::developer_current_restore::publish_positive_cut_response(
                crate::success_response(
                    "cut",
                    0,
                    serde_json::json!({"cutReachedStillFenced":true})
                ),
                &mut reply
            )
            .is_err()
        );
        assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_final_new_busy_original_is_not_sealed_or_reclassified_by_unsubmitted_cut() {
    let mut f = Fixture::new();
    f.owner
        .pause_current_restore_intent(&f.backup, PASSWORD)
        .unwrap();
    let original = f.owner.held_restore_execution.test_original_identity();
    assert!(f.owner.current_intent_paused() && !f.owner.transaction.independently_blocked());
    assert_eq!(
        f.owner.cut_current_committed_retirement(
            Path::new("/public/absent"),
            b"wrong source password"
        ),
        Err(FirstError::StillFenced)
    );
    assert_eq!(
        original,
        f.owner.held_restore_execution.test_original_identity()
    );
    assert!(f.owner.current_intent_paused() && !f.owner.transaction.independently_blocked());
    f.owner.abort_current_restore_intent().unwrap(); // same genuine positive pause remains usable
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_final_new_authentication_and_queued_admission_refusals_never_submit_or_return_cut_data() {
    let mut f = Fixture::new();
    assert_eq!(
        f.owner
            .cut_current_committed_retirement(&f.backup, b"wrong source password"),
        Err(FirstError::Prepare)
    );
    assert!(
        !f.owner.held_restore_execution.occupied() && !f.owner.transaction.independently_blocked()
    );
    use crate::mutation::{MutationKind, MutationRequest};
    f.owner
        .coordinator
        .submit(
            MutationRequest::new(
                MutationKind::Other,
                Some("queued"),
                Some(0),
                MutationDigest::from_semantic_bytes(b"source queued cut"),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        f.owner
            .cut_current_committed_retirement(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(
        !f.owner.held_restore_execution.occupied() && !f.owner.transaction.independently_blocked()
    );
    assert!(!f.stage().exists());
}

#[cfg(feature = "product-private-backup")]
#[test]
fn native_final_new_product_direct_cut_helper_denies_before_original_acquisition() {
    let mut f = Fixture::new();
    assert_eq!(
        f.owner
            .cut_current_committed_retirement(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(!f.owner.held_restore_execution.occupied() && !f.stage().exists());
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_final_old_producer_consumes_real_pause_and_cuts_only_receipt_absent() {
    let mut f = Fixture::new();
    let store = fs::read(&f.store).unwrap();
    let template = f.store.with_file_name("route-template.yaml");
    let template_bytes = fs::read(&template).unwrap();
    let inode = fs::metadata(&f.store).unwrap().ino();
    assert!(f.owner.abort_current_after_retirement_cut().is_err());
    assert_eq!(
        f.owner.pause_current_restore_intent(&f.backup, PASSWORD),
        Ok(())
    );
    assert_eq!(f.owner.abort_current_after_retirement_cut(), Ok(()));
    assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
    assert!(!f.owner.current_intent_paused());
    assert_eq!(f.owner.revision(), 0);
    assert_eq!(fs::metadata(&f.store).unwrap().ino(), inode);
    assert_eq!(fs::read(&f.store).unwrap(), store);
    assert_eq!(fs::read(&template).unwrap(), template_bytes);
    let state = f.state();
    for name in [
        "restore-pair.pending",
        "restore-decision.intent",
        "restore-decision.terminal",
        "restore-finalization.pending",
        crate::restore_disposition_ticket_model::TICKET_MEMBER,
        "restore-disposition.complete",
        "restore-disposition.history",
    ] {
        assert!(
            fs::symlink_metadata(state.join(name)).is_err(),
            "public fixture {name}"
        );
    }
    let closure = crate::restore_closure_model::ClosureRecord::decode(
        &fs::read(state.join(crate::restore_closure_model::CLOSURE_MEMBER)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        closure.receipt().terminal().phase(),
        crate::restore_decision_candidate::DecisionPhase::Aborted
    );
    assert!(closure.receipt().matches_pair(&store, &template_bytes));
    let paths = f.owner.transaction.cutover_paths().clone();
    assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
    assert!(f.owner.abort_current_after_retirement_cut().is_err());
    assert!(f.owner.abort_current_restore_intent().is_err());
    let (mut reply, lost) = std::os::unix::net::UnixStream::pair().unwrap();
    drop(lost);
    assert!(
        crate::developer_current_restore::publish_positive_cut_response(
            crate::success_response("cut", 0, serde_json::json!({"cutReachedStillFenced":true})),
            &mut reply,
        )
        .is_err()
    );
    assert!(f.owner.retained_restore_busy() && f.owner.transaction.independently_blocked());
    assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
}

#[cfg(feature = "product-private-backup")]
#[test]
fn native_final_old_producer_product_helper_denies_before_pause_acquisition() {
    let mut f = Fixture::new();
    assert_eq!(
        f.owner.abort_current_after_retirement_cut(),
        Err(FirstError::Admission)
    );
    assert!(!f.owner.held_restore_execution.occupied());
    assert!(!f.stage().exists());
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_developer_loss_cuts_produce_real_mixed_or_terminal_and_retain_same_lease() {
    for terminal_cut in [false, true] {
        let mut f = Fixture::new();
        let template = f.store.with_file_name("route-template.yaml");
        private(
            &template,
            include_bytes!("../../../../templates/default.yaml"),
        );
        let mut new: serde_json::Value = serde_json::from_slice(PORTABLE).unwrap();
        new["routingPreset"] = "china-cn-direct".into();
        private(
            &f.backup,
            &omavless_domain::private_backup::seal(
                &serde_json::to_vec(&new).unwrap(),
                include_bytes!("../../../../templates/china.yaml"),
                PASSWORD,
            )
            .unwrap(),
        );
        let old_store = fs::read(&f.store).unwrap();
        let old_template = fs::read(&template).unwrap();
        let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
        let expected = opened.restore_store_off().unwrap();
        assert_ne!(old_store, expected.as_slice());
        assert_ne!(old_template, opened.template());
        let result = if terminal_cut {
            f.owner.cut_current_terminal(&f.backup, PASSWORD)
        } else {
            f.owner.cut_current_first_rename(&f.backup, PASSWORD)
        };
        assert_eq!(result, Ok(())); // reached-cut data only
        assert_eq!(fs::read(&f.store).unwrap(), expected.as_slice());
        assert_eq!(
            fs::read(&template).unwrap(),
            if terminal_cut {
                opened.template()
            } else {
                &old_template
            }
        );
        assert!(f.owner.held_restore_execution.occupied());
        assert!(f.owner.retained_restore_busy());
        assert!(f.owner.transaction.independently_blocked());
        assert!(f.owner.transaction.original_lease_vacant()); // never handed off
        assert_eq!(f.owner.revision(), 0);
        let paths = f.owner.transaction.cutover_paths().clone();
        assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
        assert_eq!(
            fs::read(f.stage().join("old-profiles.json")).unwrap(),
            old_store
        );
        assert_eq!(
            fs::read(f.stage().join("old-route-template.yaml")).unwrap(),
            old_template
        );
        assert!(f.state().join("restore-decision.intent").is_file());
        let terminal = f.state().join("restore-decision.terminal");
        if terminal_cut {
            let record = crate::restore_decision_candidate::DecisionRecord::decode(
                &fs::read(terminal).unwrap(),
            )
            .unwrap();
            assert_eq!(
                record.phase(),
                crate::restore_decision_candidate::DecisionPhase::Committed
            );
        } else {
            assert!(!terminal.exists());
        }
        assert!(!f.state().join("restore-disposition.history").exists());
        assert!(!f.state().join("restore-closure.complete").exists());
        assert!(
            f.owner
                .cut_current_first_rename(&f.backup, PASSWORD)
                .is_err()
        );
        assert!(f.owner.cut_current_terminal(&f.backup, PASSWORD).is_err());
        assert!(f.owner.abort_current_restore_intent().is_err());
        assert!(
            f.owner
                .execute_first_restore_completed(&f.backup, PASSWORD)
                .is_err()
        );
        let ordinary = crate::make_request(
            "after-cut",
            "onboarding.complete",
            serde_json::json!({"operationId":"after-cut","expectedRevision":0}),
        )
        .unwrap();
        assert!(f.owner.execute_onboarding(&ordinary).is_err());
        let (mut producer, consumer) = std::os::unix::net::UnixStream::pair().unwrap();
        drop(consumer);
        assert!(
            crate::developer_current_restore::publish_positive_cut_response(
                crate::success_response(
                    "cut",
                    0,
                    serde_json::json!({"cutReachedStillFenced":true})
                ),
                &mut producer
            )
            .is_err()
        );
        assert!(f.owner.retained_restore_busy());
        assert!(f.owner.transaction.independently_blocked());
        assert!(!f.owner.current_intent_paused());
        assert!(MigrationLock::acquire_existing(&paths, f.owner.uid()).is_err());
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_developer_loss_cuts_wrong_auth_or_unreached_gate_never_return_positive() {
    for terminal in [false, true] {
        let mut f = Fixture::new();
        let before = fs::read(&f.store).unwrap();
        let failed = if terminal {
            f.owner
                .cut_current_terminal(&f.backup, b"wrong synthetic passphrase")
        } else {
            f.owner
                .cut_current_first_rename(&f.backup, b"wrong synthetic passphrase")
        };
        assert_eq!(failed, Err(FirstError::Prepare));
        assert!(!f.owner.held_restore_execution.occupied());
        assert!(!f.stage().exists());
        assert_eq!(fs::read(&f.store).unwrap(), before);
        // A real earlier stage refusal is StillFenced, but not the selected cut.
        private(
            &f.state().join("restore-pair.pending"),
            b"foreign fixture fence",
        );
        let failed = if terminal {
            f.owner.cut_current_terminal(&f.backup, PASSWORD)
        } else {
            f.owner.cut_current_first_rename(&f.backup, PASSWORD)
        };
        assert!(failed.is_err());
        assert_eq!(fs::read(&f.store).unwrap(), before);
        assert!(!f.state().join("restore-decision.terminal").exists());
    }
}

#[cfg(all(
    feature = "t4-manager-actor-service",
    not(feature = "product-private-backup")
))]
#[test]
fn native_developer_loss_cuts_queued_revision_admission_precedes_authentication() {
    use crate::mutation::{MutationKind, MutationRequest};
    for terminal in [false, true] {
        let mut f = Fixture::new();
        let mutation = MutationRequest::new(
            MutationKind::Other,
            Some("queued"),
            Some(0),
            MutationDigest::from_semantic_bytes(b"public queued mutation"),
        )
        .unwrap();
        f.owner.coordinator.submit(mutation).unwrap();
        let error = if terminal {
            f.owner.cut_current_terminal(&f.backup, PASSWORD)
        } else {
            f.owner.cut_current_first_rename(&f.backup, PASSWORD)
        };
        assert_eq!(error, Err(FirstError::Admission));
        assert!(!f.owner.held_restore_execution.occupied());
        assert!(!f.stage().exists());
        assert_eq!(f.owner.revision(), 0);
    }
}

#[cfg(feature = "product-private-backup")]
#[test]
fn native_developer_loss_cuts_product_direct_helpers_deny_before_acquisition() {
    let mut f = Fixture::new();
    assert_eq!(
        f.owner
            .cut_current_first_rename(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert_eq!(
        f.owner
            .cut_current_terminal(Path::new("/public/absent"), PASSWORD),
        Err(FirstError::Admission)
    );
    assert!(!f.owner.held_restore_execution.occupied());
    assert!(!f.stage().exists());
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_positive_intent_pause_aborts_old_without_live_rename_and_moves_same_lease() {
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    let old_inode = fs::metadata(&f.store).unwrap().ino();
    assert_eq!(
        f.owner.pause_current_restore_intent(&f.backup, PASSWORD),
        Ok(())
    );
    assert!(f.owner.current_intent_paused());
    assert!(f.owner.retained_restore_busy());
    assert!(!f.owner.transaction.independently_blocked());
    assert_eq!(f.owner.revision(), 0);
    assert_eq!(fs::read(&f.store).unwrap(), old);
    assert_eq!(fs::metadata(&f.store).unwrap().ino(), old_inode);
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    assert!(
        f.owner
            .pause_current_restore_intent(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(
        f.owner
            .execute_first_restore_completed(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(f.owner.current_intent_paused()); // expected busy did not revoke
    let denied = crate::make_request(
        "paused-mutation",
        "onboarding.complete",
        serde_json::json!({"operationId":"paused-mutation","expectedRevision":0}),
    )
    .unwrap();
    assert!(f.owner.execute_onboarding(&denied).is_err());
    assert!(f.owner.current_intent_paused());
    assert_eq!(f.owner.abort_current_restore_intent(), Ok(()));
    assert!(!f.owner.current_intent_paused());
    assert!(!f.owner.retained_restore_busy());
    assert_eq!(f.owner.revision(), 1);
    assert_eq!(fs::read(&f.store).unwrap(), old);
    assert_eq!(fs::metadata(&f.store).unwrap().ino(), old_inode);
    assert!(!crate::pending_private_transaction::pending_at(&f.state()));
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    assert!(f.owner.abort_current_restore_intent().is_err());
    assert!(!f.owner.retained_restore_busy()); // stale resume cannot revoke success
    let request = crate::make_request(
        "after-abort",
        "onboarding.complete",
        serde_json::json!({"operationId":"after-abort","expectedRevision":1}),
    )
    .unwrap();
    assert!(matches!(
        f.owner.execute_onboarding(&request).unwrap(),
        NativeOwnerExecution::Applied { outcome: Ok(_), .. }
    ));
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_paused_abort_publication_and_retirement_cuts_are_sticky() {
    use crate::manager_actor_service::NativeStep;
    for selected in [
        NativeStep::Terminal,
        NativeStep::RetirementReceipt,
        NativeStep::StageRetired,
        NativeStep::Closure,
        NativeStep::DispositionComplete,
        NativeStep::History,
    ] {
        let mut f = Fixture::new();
        let old = fs::read(&f.store).unwrap();
        f.owner
            .pause_current_restore_intent(&f.backup, PASSWORD)
            .unwrap();
        let mut reached = false;
        assert!(
            f.owner
                .abort_current_restore_intent_cut(&mut |step| {
                    if step == selected {
                        reached = true;
                        return Err(FirstError::StillFenced);
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(reached, "selected cut was not reached");
        assert!(!f.owner.current_intent_paused());
        assert!(f.owner.retained_restore_busy());
        assert!(f.owner.transaction.independently_blocked());
        assert!(f.owner.abort_current_restore_intent().is_err());
        assert!(
            MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
                .is_err()
        );
        assert_eq!(fs::read(&f.store).unwrap(), old);
    }
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_pause_first_publication_unknown_and_original_substitution_never_resume() {
    use crate::manager_actor_service::NativeStep;
    for selected in [NativeStep::StageReady, NativeStep::Intent] {
        let mut f = Fixture::new();
        let old = fs::read(&f.store).unwrap();
        let mut reached = false;
        assert!(
            f.owner
                .pause_current_restore_intent_cut(&f.backup, PASSWORD, |step| {
                    if step == selected {
                        reached = true;
                        return Err(FirstError::StillFenced);
                    }
                    Ok(())
                })
                .is_err()
        );
        assert!(reached);
        assert!(!f.owner.current_intent_paused());
        assert!(f.owner.abort_current_restore_intent().is_err());
        assert!(f.owner.retained_restore_busy());
        assert_eq!(fs::read(&f.store).unwrap(), old);
    }
    for selected in ["intent", "live", "instance", "history"] {
        let mut f = Fixture::new();
        f.owner
            .initialize_batch_operations("original-instance")
            .unwrap();
        f.owner
            .pause_current_restore_intent(&f.backup, PASSWORD)
            .unwrap();
        match selected {
            "instance" => f.owner.batch.as_mut().unwrap().instance = "foreign-instance".into(),
            "history" => private(&f.state().join("restore-disposition.history"), b"foreign"),
            _ => {
                let path = if selected == "intent" {
                    f.state().join("restore-decision.intent")
                } else {
                    f.store.clone()
                };
                let bytes = fs::read(&path).unwrap();
                fs::rename(&path, f.root.join("substituted-original")).unwrap();
                private(&path, &bytes);
            }
        }
        assert!(f.owner.abort_current_restore_intent().is_err());
        assert!(!f.owner.current_intent_paused());
        assert!(f.owner.retained_restore_busy());
        assert!(f.owner.abort_current_restore_intent().is_err());
        assert!(
            MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
                .is_err()
        );
    }
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_positive_pause_lost_response_publication_seals_same_original_slot() {
    use std::os::unix::net::UnixStream;
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    f.owner
        .pause_current_restore_intent(&f.backup, PASSWORD)
        .unwrap();
    let (mut producer, consumer) = UnixStream::pair().unwrap();
    drop(consumer);
    let publication = crate::developer_current_restore::publish_positive_pause_response(
        omavless_control_protocol::success_response(
            "public",
            0,
            serde_json::json!({"intentPaused":true}),
        ),
        &mut producer,
        0,
        |revision| {
            assert!(f.owner.refuse_unpublished_intent_pause(revision));
            struct Full;
            impl std::io::Write for Full {
                fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                    Err(std::io::ErrorKind::StorageFull.into())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    panic!("no diagnostic flush")
                }
            }
            crate::developer_current_restore::write_publication_sealed_diagnostic(&mut Full);
        },
    );
    assert_eq!(publication, Err(crate::RuntimeError::Io));
    assert!(!f.owner.refuse_unpublished_intent_pause(0)); // never a second diagnostic
    assert!(!f.owner.current_intent_paused());
    assert!(f.owner.retained_restore_busy());
    assert!(f.owner.transaction.independently_blocked());
    assert!(f.owner.abort_current_restore_intent().is_err());
    assert!(
        f.owner
            .pause_current_restore_intent(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    assert_eq!(fs::read(&f.store).unwrap(), old);
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn publication_seal_diagnostic_requires_original_positive_pause_and_exact_revision() {
    let mut f = Fixture::new();
    assert!(!f.owner.refuse_unpublished_intent_pause(0));
    assert!(!f.owner.held_restore_execution.occupied());
    f.owner
        .pause_current_restore_intent(&f.backup, PASSWORD)
        .unwrap();
    assert!(!f.owner.refuse_unpublished_intent_pause(1));
    assert!(f.owner.current_intent_paused());
    assert!(!f.owner.transaction.independently_blocked());
    assert!(f.owner.refuse_unpublished_intent_pause(0));
    assert!(!f.owner.current_intent_paused());
    assert!(f.owner.transaction.independently_blocked());
    assert!(f.owner.abort_current_restore_intent().is_err());
    assert!(!f.owner.refuse_unpublished_intent_pause(0));
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_pause_queue_and_active_refuse_before_original_acquisition() {
    use crate::mutation::{BeginOutcome, MutationKind, MutationRequest, SubmitOutcome};
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    let request = MutationRequest::new(
        MutationKind::Other,
        Some("existing"),
        Some(0),
        MutationDigest::from_semantic_bytes(b"existing queued operation"),
    )
    .unwrap();
    assert!(matches!(
        f.owner.coordinator.submit(request).unwrap(),
        SubmitOutcome::Queued { .. }
    ));
    assert!(
        f.owner
            .pause_current_restore_intent(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(!f.owner.held_restore_execution.occupied());
    assert!(matches!(
        f.owner.coordinator.begin_next().unwrap(),
        BeginOutcome::Started(_)
    ));
    assert!(
        f.owner
            .pause_current_restore_intent(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(!f.owner.held_restore_execution.occupied());
    assert!(!f.stage().exists());
    assert_eq!(fs::read(&f.store).unwrap(), old);
    assert_eq!(f.owner.revision(), 0);
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_intent_pause_identical_pair_uses_aborted_not_committed_proof() {
    let mut f = Fixture::new();
    let incoming = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
    private(&f.store, &incoming.restore_store_off().unwrap());
    private(
        &f.store.with_file_name("route-template.yaml"),
        incoming.template(),
    );
    let old_inode = fs::metadata(&f.store).unwrap().ino();
    f.owner
        .pause_current_restore_intent(&f.backup, PASSWORD)
        .unwrap();
    f.owner.abort_current_restore_intent().unwrap();
    assert_eq!(fs::metadata(&f.store).unwrap().ino(), old_inode);
    assert_eq!(f.owner.revision(), 1);
    assert!(!f.owner.retained_restore_busy());
    assert!(!crate::pending_private_transaction::pending_at(&f.state()));
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn normal_pair_completed_restore_replays_then_denies_nested_entry_without_poisoning() {
    use crate::private_pair_api::{Action, Request};
    let mut f = Fixture::new();
    let input = |method: &str, id, revision, destination: &Path| {
        Request::parse(method, &serde_json::json!({"schema":1,"archive":destination,
            "passphrase":std::str::from_utf8(PASSWORD).unwrap(),
            "confirmation":if method=="backup.restore" {"replace-current-private-pair"} else {"export-current-private-pair"},
            "instanceId":"local-engine-control","operationId":id,"expectedRevision":revision})).unwrap().0
    };
    let first = input("backup.restore", "first-restore", 0, &f.backup);
    let result = f
        .owner
        .execute_normal_pair(&first, Action::Restore)
        .unwrap();
    assert!(result.error.is_none());
    assert_eq!(result.revision, 1);
    assert!(!f.owner.transaction.original_lease_vacant());
    let before = fs::read(&f.store).unwrap();
    let inode = fs::metadata(&f.store).unwrap().ino();
    let history = fs::read(f.state().join("restore-disposition.history")).unwrap();
    assert_eq!(
        f.owner
            .execute_normal_pair(&first, Action::Restore)
            .unwrap(),
        result
    );
    let nested = input("backup.restore", "fresh-restore", 1, &f.backup);
    assert_eq!(
        f.owner.execute_normal_pair(&nested, Action::Restore),
        Err(NativeOwnerError::OwnershipUnavailable)
    );
    assert!(
        !f.owner
            .coordinator
            .operation_id_in_use("fresh-restore")
            .unwrap()
    );
    assert!(!f.owner.retained_restore_busy());
    assert!(!f.owner.coordinator.pair_abandoned());
    assert!(!crate::pending_private_transaction::pending_at(&f.state()));
    assert_eq!(f.owner.revision(), 1);
    assert_eq!(fs::read(&f.store).unwrap(), before);
    assert_eq!(fs::metadata(&f.store).unwrap().ino(), inode);
    assert_eq!(
        fs::read(f.state().join("restore-disposition.history")).unwrap(),
        history
    );
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    // Unrelated read-only export borrows that SAME installed ordinary lease.
    let export = f.root.join("after-completion.ovb");
    let backup = input("backup.create", "next-backup", 1, &export);
    assert!(
        f.owner
            .execute_normal_pair(&backup, Action::Create)
            .unwrap()
            .error
            .is_none()
    );
    assert_eq!(f.owner.revision(), 1);
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    let onboarding = crate::make_request(
        "ordinary-after-pair",
        "onboarding.complete",
        serde_json::json!({"operationId":"ordinary-after-pair","expectedRevision":1}),
    )
    .unwrap();
    assert!(matches!(
        f.owner.execute_onboarding(&onboarding).unwrap(),
        NativeOwnerExecution::Applied { outcome: Ok(_), .. }
    ));
    assert_eq!(f.owner.revision(), 2);
    assert_eq!(
        f.owner
            .execute_normal_pair(&first, Action::Restore)
            .unwrap(),
        result
    );
    assert!(!f.owner.retained_restore_busy());
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn previewed_pair_same_count_replacement_denied_before_slot_then_matching_real_completion() {
    use crate::private_pair_api::{PreviewedRestoreRequest, ciphertext_hex};
    let mut f = Fixture::new();
    let (_, digest) = crate::backup_destination_candidate::open_existing_with_digest(
        &f.backup,
        f.owner.uid(),
        PASSWORD,
    )
    .unwrap();
    let original_store = fs::read(&f.store).unwrap();
    let original_inode = fs::metadata(&f.store).unwrap().ino();
    let incoming = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
    let expected_store = incoming.restore_store_off().unwrap();
    let expected_template = incoming.template().to_vec();
    let desired_before = f.owner.desired().unwrap();
    let other =
        omavless_domain::private_backup::seal(incoming.store(), incoming.template(), PASSWORD)
            .unwrap();
    private(&f.backup, &other);
    let make = |id, revision, digest: &[u8; 32]| {
        PreviewedRestoreRequest::parse(&serde_json::json!({"schema":1,"archive":f.backup,
        "passphrase":std::str::from_utf8(PASSWORD).unwrap(),"confirmation":"replace-previewed-current-private-pair",
        "instanceId":"local-engine-control","operationId":id,"expectedRevision":revision,"expectedCiphertextDigest":ciphertext_hex(digest)})).unwrap()
    };
    let stale = make("stale-envelope", 0, &digest);
    let denied = f.owner.execute_previewed_pair(&stale).unwrap();
    assert_eq!(
        denied.error,
        Some(omavless_control_protocol::StableErrorCode::InvalidArgument)
    );
    assert_eq!(f.owner.revision(), 0);
    assert!(!f.owner.held_restore_execution.occupied());
    assert!(f.owner.transaction.original_lease_vacant());
    assert!(!f.owner.retained_restore_busy());
    assert!(!crate::pending_private_transaction::pending_at(&f.state()));
    assert_eq!(fs::read(&f.store).unwrap(), original_store);
    assert_eq!(fs::metadata(&f.store).unwrap().ino(), original_inode);
    let (_, actual) = crate::backup_destination_candidate::open_existing_with_digest(
        &f.backup,
        f.owner.uid(),
        PASSWORD,
    )
    .unwrap();
    assert_ne!(digest, actual);
    #[cfg(not(feature = "tui"))]
    let first = make("matching-envelope", 0, &actual);
    #[cfg(feature = "tui")]
    let (mut client, client_request) = {
        use omavless_tui::private_restore::Workspace;
        let mut client = Workspace::new("local-engine-control", 0).unwrap();
        for ch in f.backup.to_str().unwrap().chars() {
            assert!(client.push(ch));
        }
        client.next_field();
        for ch in std::str::from_utf8(PASSWORD).unwrap().chars() {
            assert!(client.push(ch));
        }
        let preview_request = client.begin_preview().unwrap();
        let (profiles, subscriptions, confirmed_digest) = f
            .owner
            .preview_current_pair(&f.backup, PASSWORD, 0)
            .unwrap();
        assert_eq!(confirmed_digest, actual);
        client.accept(preview_request.settle(Ok(
            serde_json::json!({"ok":true,"revision":0,"result":{
                "profiles":profiles,"subscriptions":subscriptions,"scope":"privatePair",
                "ciphertextDigest":ciphertext_hex(&confirmed_digest)
            }}),
        )));
        let request = client.submit("matching-envelope".into()).unwrap();
        (client, request)
    };
    #[cfg(feature = "tui")]
    let first = PreviewedRestoreRequest::parse(&client_request.params()).unwrap();
    let result = f.owner.execute_previewed_pair(&first).unwrap();
    assert!(result.error.is_none());
    assert_eq!(result.revision, 1);
    #[cfg(feature = "tui")]
    {
        // Source composition only: this is the actual local retained engine,
        // not a genuine-current factory or installed positive socket fixture.
        client.accept(client_request.settle(Ok(
            serde_json::json!({"ok":true,"revision":result.revision,
                "result":{"completed":true,"replayed":false,"scope":"privatePair"}
            }),
        )));
        assert_eq!(
            client.state(),
            omavless_tui::private_restore::State::Completed
        );
        assert!(client.submit("second-submit".into()).is_none());
    }
    // Independent file/projection readback, not merely a positive engine label.
    assert_eq!(fs::read(&f.store).unwrap(), expected_store.as_slice());
    assert_eq!(
        fs::read(f.store.parent().unwrap().join("route-template.yaml")).unwrap(),
        expected_template
    );
    let restored: serde_json::Value = serde_json::from_slice(&fs::read(&f.store).unwrap()).unwrap();
    assert_eq!(restored["profiles"], serde_json::json!([]));
    assert_eq!(restored["subscriptions"], serde_json::json!([]));
    assert_eq!(restored["onboardingComplete"], false);
    assert_eq!(restored["startup"]["enabled"], false);
    assert_eq!(f.owner.desired().unwrap(), desired_before);
    let history = fs::read(f.state().join("restore-disposition.history")).unwrap();
    assert!(crate::restore_disposition_complete_model::CompleteRecord::decode(&history).is_some());
    assert_eq!(f.owner.execute_previewed_pair(&first).unwrap(), result);
    assert_eq!(fs::read(&f.store).unwrap(), expected_store.as_slice());
    assert_eq!(
        fs::read(f.state().join("restore-disposition.history")).unwrap(),
        history
    );
    let changed_same_id = make("matching-envelope", 0, &digest);
    assert_eq!(
        f.owner
            .execute_previewed_pair(&changed_same_id)
            .unwrap_err()
            .stable_code(),
        omavless_control_protocol::StableErrorCode::Conflict
    );
    assert!(!f.owner.transaction.original_lease_vacant());
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    let nested = make("fresh-bound", 1, &actual);
    assert_eq!(
        f.owner.execute_previewed_pair(&nested),
        Err(NativeOwnerError::OwnershipUnavailable)
    );
    assert!(
        !f.owner
            .coordinator
            .operation_id_in_use("fresh-bound")
            .unwrap()
    );
    assert!(!f.owner.retained_restore_busy());
    assert!(!crate::pending_private_transaction::pending_at(&f.state()));
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_committed_completion_retains_new_pair_and_one_original_ordinary_lease() {
    let mut f = Fixture::new();
    let original = f.owner.transaction.acquire_lock().unwrap();
    assert!(super::retained::completion_owns_lease(&original));
    drop(original); // wholly positive, before any restore effects
    use crate::mutation::{
        BeginOutcome, MutationKind, MutationRequest, MutationResult, SubmitOutcome,
    };
    let digest = MutationDigest::from_semantic_bytes(b"public previous operation");
    let old = || {
        MutationRequest::new(MutationKind::Other, Some("before-restore"), Some(0), digest).unwrap()
    };
    assert!(matches!(
        f.owner.coordinator.submit(old()).unwrap(),
        SubmitOutcome::Queued { .. }
    ));
    let BeginOutcome::Started(active) = f.owner.coordinator.begin_next().unwrap() else {
        panic!("previous operation missing");
    };
    let previous = f
        .owner
        .coordinator
        .finish(active.token, MutationResult::NoChange)
        .unwrap();
    let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
    let incoming = opened.restore_store_off().unwrap();
    assert_eq!(
        f.owner.execute_first_restore_completed(&f.backup, PASSWORD),
        Ok(())
    );
    assert!(f.owner.held_restore_execution.occupied());
    assert!(!f.owner.retained_restore_busy());
    assert!(!f.owner.transaction.original_lease_vacant());
    let borrower = f.owner.transaction.acquire_lock().unwrap();
    assert!(!super::retained::completion_owns_lease(&borrower));
    assert!(matches!(
        borrower,
        crate::connection_transaction::MigrationLease::Original(_)
    ));
    drop(borrower); // non-owning guard cannot unlock the same original Flock
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
    assert!(!crate::pending_private_transaction::pending_at(&f.state()));
    assert!(!f.stage().exists());
    assert_eq!(fs::read(&f.store).unwrap(), incoming.as_slice());
    assert_eq!(f.owner.revision(), 1);
    assert_eq!(
        f.owner.coordinator.submit(old()).unwrap(),
        SubmitOutcome::Replay(previous)
    );
    assert!(
        f.owner
            .coordinator
            .submit(
                MutationRequest::new(MutationKind::Other, Some("new-stale"), Some(0), digest)
                    .unwrap()
            )
            .is_err()
    );
    let request = crate::make_request(
        "after-restore",
        "onboarding.complete",
        serde_json::json!({"operationId":"after-restore", "expectedRevision":1}),
    )
    .unwrap();
    let result = f.owner.execute_onboarding(&request).unwrap();
    assert!(matches!(
        result,
        NativeOwnerExecution::Applied { outcome: Ok(_), .. }
    ));
    assert_eq!(f.owner.revision(), 2);
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&f.store).unwrap()).unwrap()["onboardingComplete"].as_bool().unwrap());
    assert!(matches!(
        f.owner.execute_onboarding(&request).unwrap(),
        NativeOwnerExecution::Replay(_)
    ));
    let history = crate::restore_disposition_complete_model::CompleteRecord::decode(
        &fs::read(f.state().join("restore-disposition.history")).unwrap(),
    )
    .unwrap();
    let _ = history; // decoded audit bytes are never reused as authority
    assert!(
        f.owner
            .execute_first_restore_completed(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(
        MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
            .is_err()
    );
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_committed_completion_late_terminal_source_history_and_effect_cuts_keep_originals() {
    for case in [
        "terminal-swap",
        "late-fence",
        "history-collision",
        "known-write",
        "known-rename",
        "known-terminal",
        "receipt-refusal",
        "stage-retired-refusal",
        "closure-refusal",
        "complete-refusal",
        "history-refusal",
        "receipt-swap",
        "history-late-fence",
    ] {
        let mut f = Fixture::new();
        let state = f.state();
        let terminal = state.join("restore-decision.terminal");
        let result = f
            .owner
            .execute_first_restore_completed_cut(&f.backup, PASSWORD, |step| {
                use crate::manager_actor_service::NativeStep;
                match (case, step) {
                    ("terminal-swap", NativeStep::Terminal) => {
                        fs::rename(&terminal, state.join("displaced-terminal")).unwrap();
                        private(
                            &terminal,
                            &fs::read(state.join("displaced-terminal")).unwrap(),
                        );
                    }
                    ("late-fence", NativeStep::Terminal) => private(
                        &state.join(crate::restore_closure_model::NEXT_CLOSURE_MEMBER),
                        b"foreign",
                    ),
                    ("history-collision", NativeStep::Terminal) => private(
                        &state.join("restore-disposition.history"),
                        b"existing audit; never overwrite",
                    ),
                    ("known-write", NativeStep::Replacement(0))
                    | ("known-rename", NativeStep::Renamed(0))
                    | ("known-terminal", NativeStep::Terminal)
                    | ("receipt-refusal", NativeStep::RetirementReceipt)
                    | ("stage-retired-refusal", NativeStep::StageRetired)
                    | ("closure-refusal", NativeStep::Closure)
                    | ("complete-refusal", NativeStep::DispositionComplete)
                    | ("history-refusal", NativeStep::History) => {
                        return Err(FirstError::StillFenced);
                    }
                    ("receipt-swap", NativeStep::RetirementReceipt) => {
                        let receipt =
                            state.join(crate::restore_retirement_candidate::RECEIPT_MEMBER);
                        fs::rename(&receipt, state.join("displaced-receipt")).unwrap();
                        private(
                            &receipt,
                            &fs::read(state.join("displaced-receipt")).unwrap(),
                        );
                    }
                    ("history-late-fence", NativeStep::History) => private(
                        &state.join(crate::restore_closure_model::NEXT_CLOSURE_MEMBER),
                        b"late foreign fence",
                    ),
                    _ => {}
                }
                Ok(())
            });
        assert!(result.is_err(), "{case}");
        assert!(f.owner.held_restore_execution.occupied() && f.owner.retained_restore_busy());
        assert!(f.owner.transaction.independently_blocked());
        assert!(
            MigrationLock::acquire_existing(f.owner.transaction.cutover_paths(), f.owner.uid())
                .is_err()
        );
        assert!(
            !f.owner.transaction.original_lease_vacant()
                || f.owner
                    .held_restore_execution
                    .original_lease_held(f.owner.transaction.cutover_paths(), f.owner.uid())
        );
        assert!(f.owner.batch_lock().is_err());
        if case == "history-collision" {
            assert_eq!(
                fs::read(state.join("restore-disposition.history")).unwrap(),
                b"existing audit; never overwrite"
            );
        }
    }
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_retained_vm_fault_bridge_executes_actual_original_lease_intent_and_mixed_cuts() {
    let reservations = [
        HeldExecutionSlot::reserve_vm().unwrap(),
        HeldExecutionSlot::reserve_vm().unwrap(),
        HeldExecutionSlot::reserve_vm().unwrap(),
    ];
    for (case, reservation) in reservations.into_iter().enumerate() {
        let mut f = Fixture::new();
        f.owner
            .retained_vm_install_reservation(reservation)
            .unwrap();
        assert!(
            f.owner
                .retained_vm_install_reservation(HeldExecutionSlot::reserve_vm().unwrap())
                .is_err()
        );
        let original = fs::read(&f.store).unwrap();
        assert!(f.owner.retained_vm_fault(&f.backup, PASSWORD, case as u8));
        assert!(f.owner.retained_vm_custody());
        assert!(f.owner.retained_vm_ordinary_and_recovery_denied());
        assert!(
            MigrationLock::acquire(f.owner.transaction.cutover_paths(), f.owner.uid()).is_err()
        );
        assert!(!f.state().join("restore-decision.terminal").exists());
        if case == 0 {
            assert!(!f.stage().exists());
            assert!(!f.state().join("restore-decision.intent").exists());
        } else {
            assert!(f.stage().is_dir());
            assert!(f.state().join("restore-decision.intent").is_file());
        }
        if case == 2 {
            assert_ne!(fs::read(&f.store).unwrap(), original);
        } else {
            assert_eq!(fs::read(&f.store).unwrap(), original);
        }
    }
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_retained_execution_keeps_real_lease_and_denies_mutable_host_after_result() {
    let mut f = Fixture::new();
    let result = f.owner.execute_first_restore_retained(&f.backup, PASSWORD);
    assert_eq!(result, Ok(FirstOutcome::CommittedStillFenced));
    assert!(f.owner.held_restore_execution.occupied());
    assert!(
        f.owner
            .held_restore_execution
            .original_lease_held(f.owner.transaction.cutover_paths(), f.owner.uid())
    );
    assert!(MigrationLock::acquire(f.owner.transaction.cutover_paths(), f.owner.uid()).is_err());
    let calls = f.owner.host().observations;
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = f.owner.host_mut();
    }));
    assert!(rejected.is_err());
    assert_eq!(f.owner.host().observations, calls);
    assert!(f.owner.initialize_batch_operations("new-instance").is_err());
    assert!(
        f.owner
            .admit(
                crate::mutation::MutationKind::Other,
                Some("held-no-replay"),
                Some(f.owner.revision()),
                MutationDigest::from_semantic_bytes(b"held")
            )
            .is_err()
    );
    assert!(
        f.owner
            .schedule(
                crate::mutation::MutationKind::Other,
                Some("held-no-replay"),
                Some(f.owner.revision()),
                MutationDigest::from_semantic_bytes(b"held")
            )
            .is_err()
    );
    assert!(f.owner.stop_batch_operations().is_err());
    assert!(f.owner.try_promote_candidate().is_err());
    assert!(f.owner.reconcile_startup().is_err());
    assert!(
        f.owner
            .execute_first_restore_retained(&f.backup, PASSWORD)
            .is_err()
    );
    assert!(
        f.owner
            .held_restore_execution
            .original_lease_held(f.owner.transaction.cutover_paths(), f.owner.uid())
    );
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_retained_error_unwind_and_owner_drop_never_free_original_lease() {
    for unwind in [false, true] {
        let mut f = Fixture::new();
        let paths = f.owner.transaction.cutover_paths().clone();
        let uid = f.owner.uid();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.owner
                .retained_test_after_lease(&f.backup, PASSWORD, unwind)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(FirstError::StillFenced));
        }
        assert!(f.owner.held_restore_execution.occupied());
        assert!(MigrationLock::acquire(&paths, uid).is_err());
        let observations = f.owner.host().observations;
        assert!(f.owner.initialize_batch_operations("replacement").is_err());
        assert!(f.owner.batch_lock().is_err());
        assert!(
            f.owner
                .with_owned_read::<()>(|_| panic!("occupied slot reached a host/store projection"))
                .is_err()
        );
        assert!(f.owner.stop_batch_operations().is_err());
        assert!(f.owner.reconcile_startup().is_err());
        assert!(
            f.owner
                .execute_first_restore_retained(&f.backup, PASSWORD)
                .is_err()
        );
        let host = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = f.owner.host_mut();
        }));
        assert!(host.is_err());
        assert_eq!(f.owner.host().observations, observations);
        // Exercise the exact owning-slot destructor used by coordinator Drop.
        // The test-only take is not a production reset/re-entry API.
        let original_slot = std::mem::take(&mut f.owner.held_restore_execution);
        drop(original_slot);
        assert!(MigrationLock::acquire(&paths, uid).is_err());
    }
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_retained_every_real_pair_effect_cut_keeps_lease_and_never_compensates() {
    use crate::manager_actor_service::NativeStep;
    let steps = [
        NativeStep::StageReady,
        NativeStep::Intent,
        NativeStep::Replacement(0),
        NativeStep::Replacement(1),
        NativeStep::Renamed(0),
        NativeStep::Renamed(1),
        NativeStep::Terminal,
    ];
    for step in steps {
        for unwind in [false, true] {
            let mut f = Fixture::new();
            let old = fs::read(&f.store).unwrap();
            let state = f.state();
            let paths = f.owner.transaction.cutover_paths().clone();
            let uid = f.owner.uid();
            let mut reached = false;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f.owner
                    .retained_test_at_step(&f.backup, PASSWORD, |current| {
                        if current == step {
                            reached = true;
                            if unwind {
                                panic!("fixed_native_effect_cut");
                            }
                            return Err(FirstError::StillFenced);
                        }
                        Ok(())
                    })
            }));
            assert!(reached, "{step:?}");
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(FirstError::StillFenced));
            }
            assert!(f.owner.held_restore_execution.occupied());
            assert!(MigrationLock::acquire(&paths, uid).is_err());
            assert!(
                f.owner
                    .execute_first_restore_retained(&f.backup, PASSWORD)
                    .is_err()
            );
            assert!(f.owner.batch_lock().is_err());
            assert!(
                f.owner
                    .stage_restore_candidate(&f.backup, PASSWORD)
                    .is_err()
            );
            assert!(f.owner.publish_restore_completion_candidate().is_err());
            assert!(f.owner.finalize_terminal_restore_candidate().is_err());
            assert!(f.owner.retire_terminal_restore_candidate().is_err());
            // The writer never runs Abort/cleanup after an uncertain cut.
            assert!(f.stage().is_dir());
            if step == NativeStep::StageReady {
                assert!(!state.join("restore-decision.intent").exists());
            }
            if matches!(step, NativeStep::Renamed(_) | NativeStep::Terminal) {
                assert_ne!(fs::read(&f.store).unwrap(), old);
            } else {
                assert_eq!(fs::read(&f.store).unwrap(), old);
            }
            if step == NativeStep::Terminal {
                let terminal = crate::restore_decision_candidate::DecisionRecord::decode(
                    &fs::read(state.join("restore-decision.terminal")).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    terminal.phase(),
                    crate::restore_decision_candidate::DecisionPhase::Committed
                );
            } else {
                assert!(!state.join("restore-decision.terminal").exists());
            }
        }
    }
}

#[cfg(feature = "t4-manager-actor-service")]
#[test]
fn native_retained_fences_reject_real_stage_name_content_absence_and_catalogue_drift() {
    use crate::manager_actor_service::NativeStep;
    for case in 0..7 {
        let mut f = Fixture::new();
        let stage = f.stage();
        let state = f.state();
        let config = f.store.parent().unwrap().to_owned();
        let login = f
            .owner
            .transaction
            .cutover_paths()
            .runtime_base
            .join("omavless-login.receipt");
        let paths = f.owner.transaction.cutover_paths().clone();
        let uid = f.owner.uid();
        let mut reached = false;
        let result = f.owner.retained_test_at_step(&f.backup, PASSWORD, |step| {
            if step != NativeStep::StageReady {
                return Ok(());
            }
            assert!(!reached);
            reached = true;
            match case {
                0 => private(
                    &stage.join(crate::restore_staging_candidate::MEMBERS[0]),
                    b"corrupt",
                ),
                1 => private(&stage.join("unexpected"), b"unknown"),
                2 => {
                    let name = stage.join(crate::restore_staging_candidate::MEMBERS[0]);
                    let same = fs::read(&name).unwrap();
                    fs::rename(&name, stage.join("held-old-name")).unwrap();
                    private(&name, &same);
                    fs::remove_file(stage.join("held-old-name")).unwrap();
                }
                3 => {
                    fs::rename(&stage, state.join("held-stage")).unwrap();
                    fs::create_dir(&stage).unwrap();
                    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap();
                }
                4 => private(&login, b"formerly absent"),
                5 => private(&config.join("unexpected"), b"unknown"),
                6 => private(&state.join("unexpected"), b"unknown"),
                _ => unreachable!(),
            }
            Ok(())
        });
        assert!(reached);
        assert_eq!(result, Err(FirstError::StillFenced), "case {case}");
        assert!(f.owner.held_restore_execution.occupied());
        assert!(MigrationLock::acquire(&paths, uid).is_err());
        assert!(!state.join("restore-decision.intent").exists());
    }
}

struct OffHost {
    idle: bool,
    observations: usize,
    auxiliary: Arc<crate::auxiliary_core::AuxiliarySlot>,
}
impl LifecycleHost for OffHost {
    fn auxiliary_slot(&self) -> Option<Arc<crate::auxiliary_core::AuxiliarySlot>> {
        Some(self.auxiliary.clone())
    }
    fn fresh_observation(
        &mut self,
        _: &DesiredState,
    ) -> Result<NativeLocalObservation, HostStepError> {
        self.observations += 1;
        Ok(NativeLocalObservation {
            owned_core_running: !self.idle,
            visible_mihomo_count: 0,
            owned_auxiliary_mihomo_count: 0,
            visible_tun_count: 0,
            managed_tun_count: 0,
            owned_controller_config_verified: false,
            desired_profile_matches_owned: false,
        })
    }
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        panic!("no lifecycle effect")
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no lifecycle effect")
    }
}

struct Fixture {
    root: PathBuf,
    backup: PathBuf,
    store: PathBuf,
    owner: OfflineNativeCoordinator<OffHost>,
}
fn private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let root =
            crate::test_temp::directory_under(Path::new(&std::env::var_os("HOME").unwrap()), "fre")
                .unwrap();
        let runtime = root.join("runtime");
        let config = root.join("config");
        let state = root.join("state");
        let desired = DesiredPaths::below(&state);
        for dir in [&root, &runtime, &config, &state, &desired.directory] {
            fs::create_dir_all(dir).unwrap();
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = nix::unistd::Uid::current().as_raw();
        let paths = crate::cutover::CutoverPaths::below(&runtime, &state, uid);
        private(
            &paths.ownership_marker,
            br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
        );
        write_desired(&desired, uid, &DesiredState::default()).unwrap();
        let store = config.join("profiles.json");
        let mut old: serde_json::Value = serde_json::from_slice(PORTABLE).unwrap();
        old["onboardingComplete"] = true.into();
        private(&store, &serde_json::to_vec(&old).unwrap());
        private(
            &config.join("route-template.yaml"),
            b"synthetic exact old template\n",
        );
        let backup = root.join("synthetic.ovb");
        private(
            &backup,
            &omavless_domain::private_backup::seal(
                PORTABLE,
                include_bytes!("../../../../templates/default.yaml"),
                PASSWORD,
            )
            .unwrap(),
        );
        let owner = OfflineNativeCoordinator::new_ownership_gated(
            OffHost {
                idle: true,
                observations: 0,
                auxiliary: Arc::default(),
            },
            desired,
            &store,
            paths,
            uid,
            2,
        );
        Self {
            root,
            backup,
            store,
            owner,
        }
    }
    fn state(&self) -> PathBuf {
        self.owner
            .transaction
            .cutover_paths()
            .state_directory
            .clone()
    }
    fn stage(&self) -> PathBuf {
        self.state().join("restore-pair.pending")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn assert_fenced(f: &mut Fixture) {
    assert!(f.owner.transaction.independently_blocked());
    assert!(crate::pending_private_transaction::pending(
        f.owner.transaction.desired_paths()
    ));
    assert!(f.owner.restore_readiness_candidate().is_err());
    let paths = f.owner.transaction.cutover_paths().clone();
    let lock = MigrationLock::acquire_existing(&paths, f.owner.uid()).unwrap();
    assert!(
        crate::login_transaction::check_startup_receipt(&paths, f.owner.uid(), &lock, Some(2))
            .is_err()
    );
}

#[test]
fn first_restore_owner_executes_actual_pair_under_one_lease_and_retains_fence() {
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    let desired = fs::read(&f.owner.transaction.desired_paths().file).unwrap();
    let opened = open_existing(&f.backup, f.owner.uid(), PASSWORD).unwrap();
    let expected = opened.restore_store_off().unwrap();
    let mut stages = 0;
    let mut execution = 0;
    let result = f
        .owner
        .execute_first_restore_body(&f.backup, PASSWORD, |owner, point| {
            assert!(
                MigrationLock::acquire_existing(owner.transaction.cutover_paths(), owner.uid())
                    .is_err()
            );
            if point == Checkpoint::Staging {
                stages += 1;
            } else {
                execution += 1;
            }
            true
        });
    assert_eq!(result, Ok(FirstOutcome::CommittedStillFenced));
    assert_eq!(stages, 7);
    assert!(execution > 2);
    assert_ne!(fs::read(&f.store).unwrap(), old);
    assert_eq!(fs::read(&f.store).unwrap(), expected.as_slice());
    assert_eq!(
        fs::read(f.store.with_file_name("route-template.yaml")).unwrap(),
        opened.template()
    );
    assert_eq!(
        fs::read(&f.owner.transaction.desired_paths().file).unwrap(),
        desired
    );
    assert_eq!(fs::read(f.stage().join("old-profiles.json")).unwrap(), old);
    assert!(f.state().join("restore-decision.terminal").is_file());
    assert_fenced(&mut f);
}

#[test]
fn first_restore_owner_original_stage_swap_in_last_callback_refuses_before_intent() {
    let mut f = Fixture::new();
    let old = fs::read(&f.store).unwrap();
    let member = f.stage().join("old-profiles.json");
    let mut count = 0;
    let result = f
        .owner
        .execute_first_restore_body(&f.backup, PASSWORD, |_, point| {
            assert!(point == Checkpoint::Staging);
            count += 1;
            if count == 7 {
                let before = fs::metadata(&member).unwrap().ino();
                let bytes = fs::read(&member).unwrap();
                private(&member.with_extension("swap"), &bytes);
                fs::rename(member.with_extension("swap"), &member).unwrap();
                assert_ne!(fs::metadata(&member).unwrap().ino(), before);
            }
            true
        });
    assert_eq!(count, 7);
    assert_eq!(fs::read(&f.store).unwrap(), old);
    assert!(!f.state().join("restore-decision.intent").exists());
    assert_eq!(result, Err(FirstError::StillFenced));
    assert_fenced(&mut f);
}

#[test]
fn first_restore_owner_late_execution_gates_refuse_without_automatic_rollback() {
    for fault in 0..8 {
        let mut f = Fixture::new();
        let state = f.state();
        let store = f.store.clone();
        let old = fs::read(&store).unwrap();
        let mut injected = false;
        let result = f
            .owner
            .execute_first_restore_body(&f.backup, PASSWORD, |owner, point| {
                if !injected
                    && point == Checkpoint::Execution
                    && state.join("restore-decision.intent").exists()
                    && (fault != 6 || fs::read(&store).unwrap() != old)
                {
                    injected = true;
                    match fault {
                        0 => private(
                            &state.join("ownership.json"),
                            br#"{"schemaVersion":1,"generation":4,"phase":"rust"}"#,
                        ),
                        1 => write_desired(
                            owner.transaction.desired_paths(),
                            owner.uid(),
                            &DesiredState {
                                generation: 1,
                                ..DesiredState::default()
                            },
                        )
                        .unwrap(),
                        2 | 6 => private(
                            &state.join(crate::restore_closure_model::CLOSURE_MEMBER),
                            b"late foreign fence",
                        ),
                        3 => owner.transaction.host_mut().idle = false,
                        4 => {
                            owner
                                .coordinator
                                .submit(
                                    MutationRequest::new(
                                        MutationKind::Other,
                                        Some("late"),
                                        Some(0),
                                        MutationDigest::from_semantic_bytes(b"late"),
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                        }
                        5 => {
                            owner
                                .coordinator
                                .submit(
                                    MutationRequest::new(
                                        MutationKind::Other,
                                        Some("drift"),
                                        Some(0),
                                        MutationDigest::from_semantic_bytes(b"drift"),
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                            let BeginOutcome::Started(active) =
                                owner.coordinator.begin_next().unwrap()
                            else {
                                panic!("expected mutation")
                            };
                            owner
                                .coordinator
                                .finish(active.token, MutationResult::Success)
                                .unwrap();
                        }
                        7 => owner
                            .initialize_batch_operations("different-instance")
                            .unwrap(),
                        _ => unreachable!(),
                    }
                }
                true
            });
        assert!(injected, "late boundary reached");
        assert_eq!(result, Err(FirstError::StillFenced));
        if fault == 6 {
            assert_ne!(fs::read(&store).unwrap(), old);
        } else {
            assert_eq!(fs::read(&store).unwrap(), old);
        }
        assert!(!state.join("restore-decision.terminal").exists());
        assert_fenced(&mut f);
    }
}

#[test]
fn first_restore_owner_wrong_archive_and_existing_fence_have_no_stage_effect() {
    for pending in [false, true] {
        let mut f = Fixture::new();
        if pending {
            private(
                &f.state().join(crate::restore_closure_model::CLOSURE_MEMBER),
                b"history",
            );
        }
        let old = fs::read(&f.store).unwrap();
        let result = f.owner.execute_first_restore(
            &f.backup,
            if pending {
                PASSWORD
            } else {
                b"wrong synthetic password"
            },
        );
        assert!(result.is_err());
        assert!(!f.stage().exists());
        assert_eq!(fs::read(&f.store).unwrap(), old);
    }
}

#[test]
fn first_restore_owner_every_unrelated_fence_refuses_late_actual_execution() {
    for name in [
        "restore-finalization.pending",
        crate::restore_closure_model::CLOSURE_MEMBER,
        crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
        crate::restore_disposition_ticket_model::TICKET_MEMBER,
        crate::restore_disposition_complete_model::COMPLETE_MEMBER,
        crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        "routing-preset.pending.json",
    ] {
        let mut f = Fixture::new();
        let state = f.state();
        let before = fs::read(&f.store).unwrap();
        let mut injected = false;
        let result = f
            .owner
            .execute_first_restore_body(&f.backup, PASSWORD, |_, point| {
                if !injected
                    && point == Checkpoint::Execution
                    && state.join("restore-decision.intent").exists()
                {
                    private(&state.join(name), b"unrelated retained evidence");
                    injected = true;
                }
                true
            });
        assert!(injected);
        assert_eq!(fs::read(&f.store).unwrap(), before);
        assert_eq!(
            fs::read(state.join(name)).unwrap(),
            b"unrelated retained evidence"
        );
        assert!(!state.join("restore-decision.terminal").exists());
        assert_eq!(result, Err(FirstError::StillFenced));
        assert_fenced(&mut f);
    }
}

#[test]
fn first_restore_owner_preexisting_journal_and_slots_never_create_stage() {
    for (config, name) in [
        (false, "restore-decision.intent"),
        (false, "restore-decision.terminal"),
    ]
    .into_iter()
    .chain(
        crate::restore_executor_candidate::NEW_SLOT
            .into_iter()
            .chain(crate::restore_executor_candidate::OLD_SLOT)
            .map(|name| (true, name)),
    ) {
        let mut f = Fixture::new();
        let member = if config {
            f.store.parent().unwrap().join(name)
        } else {
            f.state().join(name)
        };
        private(&member, b"preexisting evidence");
        let before = fs::read(&f.store).unwrap();
        assert!(f.owner.execute_first_restore(&f.backup, PASSWORD).is_err());
        assert!(!f.stage().exists());
        assert_eq!(fs::read(&f.store).unwrap(), before);
        assert_eq!(fs::read(member).unwrap(), b"preexisting evidence");
    }
}

#[test]
fn first_restore_owner_existing_batch_instance_is_pinned_not_only_revision() {
    for replace in [false, true] {
        let mut f = Fixture::new();
        f.owner
            .initialize_batch_operations("original-instance")
            .unwrap();
        let state = f.state();
        let mut changed = false;
        let result = f
            .owner
            .execute_first_restore_body(&f.backup, PASSWORD, |owner, point| {
                if !changed
                    && point == Checkpoint::Execution
                    && state.join("restore-decision.intent").exists()
                {
                    if replace {
                        owner.batch.as_mut().unwrap().instance = "replacement-instance".to_owned();
                    } else {
                        owner.batch = None;
                    }
                    changed = true;
                }
                true
            });
        assert!(changed);
        assert_eq!(f.owner.coordinator.revision(), 0);
        assert_eq!(result, Err(FirstError::StillFenced));
        assert_fenced(&mut f);
    }
}
