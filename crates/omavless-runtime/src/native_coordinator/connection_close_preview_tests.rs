// SPDX-License-Identifier: MIT
// SAME real owned fixture/controller with memory image/Finish callbacks only.
// No privileged helper, root package admission, broker/TUN or VM claim.
fn product_preview_fixture() -> Fixture {
    let mut fixture = fixture("ok");
    fixture
        .owner
        .host_mut()
        .install_product_preview_fixture_for_test();
    fixture
}
fn retain_inert_preview(fixture: &mut Fixture) -> Vec<CloseDisplayRow> {
    let discovery = match fixture.owner.admit_connection_close_snapshot().unwrap() {
        CloseSnapshotAdmission::Discover(d) => d,
        _ => panic!("fresh_original_discovery_required"),
    };
    assert!(discovery.product_preview());
    let ready = discovery.observe_preview().unwrap();
    fixture.owner.retain_product_preview(Ok(ready)).unwrap()
}
fn fresh_confirm(fixture: &mut Fixture, operation: &str, row: &CloseDisplayRow) {
    let confirmation = fixture.owner.prepare_connection_close(row.handle).unwrap();
    let revision = fixture.owner.revision();
    let task = match fixture
        .owner
        .admit_product_confirm(operation, revision, row.handle, confirmation.ticket)
        .unwrap()
    {
        ProductConfirmAdmission::Discover(task) => task,
        _ => panic!("one_fresh_original_confirm_required"),
    };
    let observed = task.observe();
    fixture.owner.complete_product_confirm(observed).unwrap();
}

#[test]
fn product_preview_delayed_human_has_no_session_then_fresh_exact_close_and_replay() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    assert!(fixture.owner.connection_close.snapshot.is_none());
    assert!(fixture.owner.connection_close.cancellation.is_none());
    assert!(fixture.owner.connection_close.discovery.is_none());
    assert!(fixture.owner.connection_close.retiring.is_none());
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Ready);
    std::thread::sleep(Duration::from_millis(3100)); // deliberately exceeds old3s
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    let task = match fixture
        .owner
        .admit_product_confirm(
            "inert-delayed",
            revision,
            rows[0].handle,
            confirmation.ticket,
        )
        .unwrap()
    {
        ProductConfirmAdmission::Discover(task) => task,
        _ => panic!("fresh_confirm_expected"),
    };
    assert!(matches!(
        fixture.owner.admit_product_confirm(
            "inert-delayed",
            revision,
            rows[0].handle,
            confirmation.ticket
        ),
        Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
    ));
    assert!(!fixture.root.join("r/effects").exists());
    fixture
        .owner
        .complete_product_confirm(task.observe())
        .unwrap();
    let end = Instant::now() + Duration::from_secs(4);
    let receipt = loop {
        if let Some(receipt) = fixture.owner.poll_connection_close().unwrap() {
            break receipt;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(receipt.outcome, ExternalCloseOutcome::Closed);
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    assert!(matches!(
        fixture.owner.admit_product_confirm(
            "inert-delayed",
            revision,
            rows[0].handle,
            confirmation.ticket
        ),
        Ok(ProductConfirmAdmission::Replay(_))
    ));
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Ready);
}

#[test]
fn product_preview_removed_reused_generation_and_tuple_drift_never_retarget() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    for change in ["removed", "reused", "display-drift"] {
        let mut fixture = product_preview_fixture();
        let rows = retain_inert_preview(&mut fixture);
        fs::write(fixture.root.join("r").join(change), b"public fixed marker").unwrap();
        fresh_confirm(&mut fixture, "inert-changed", &rows[0]);
        let receipt = fixture
            .owner
            .connection_close_receipt("inert-changed")
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(receipt.outcome, ExternalCloseOutcome::RefusedBeforeWrite);
        assert!(!fixture.root.join("r/effects").exists());
        assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Ready);
        let fresh = retain_inert_preview(&mut fixture);
        assert!(
            fresh
                .iter()
                .all(|r| rows.iter().all(|old| r.handle != old.handle))
        );
    }
}

#[test]
fn product_preview_context_change_and_expired_modal_refuse_before_fresh_acquisition() {
    let _fixtures = FIXTURES.lock().unwrap();
    for change in ["revision", "context", "modal_expiry"] {
        let mut fixture = product_preview_fixture();
        let rows = retain_inert_preview(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        let revision = fixture.owner.revision();
        let before = fixture.owner.host().product_history_for_test();
        match change {
            "revision" => (),
            "context" => fs::write(
                fixture.root.join("c/config.yaml"),
                b"changed fixed fixture\n",
            )
            .unwrap(),
            _ => {
                fixture
                    .owner
                    .connection_close
                    .pending
                    .as_mut()
                    .unwrap()
                    .expiry = Instant::now() - Duration::from_millis(1)
            }
        }
        assert!(
            fixture
                .owner
                .admit_product_confirm(
                    "inert-invalid",
                    if change == "revision" {
                        revision + 1
                    } else {
                        revision
                    },
                    rows[0].handle,
                    confirmation.ticket
                )
                .is_err()
        );
        assert_eq!(fixture.owner.host().product_history_for_test(), before);
        assert!(!fixture.root.join("r/effects").exists());
    }
}

#[test]
fn product_preview_cancel_during_detached_confirm_cannot_publish_or_write() {
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    let task = match fixture
        .owner
        .admit_product_confirm(
            "inert-cancel",
            revision,
            rows[0].handle,
            confirmation.ticket,
        )
        .unwrap()
    {
        ProductConfirmAdmission::Discover(task) => task,
        _ => panic!("fresh_confirm_expected"),
    };
    fixture.owner.invalidate_connection_close();
    assert!(
        fixture
            .owner
            .complete_product_confirm(task.observe())
            .is_err()
    );
    assert!(!fixture.root.join("r/effects").exists());
}

#[test]
fn product_preview_late_result_refuses_and_completed_reservation_cannot_capture() {
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    let mut task = match fixture
        .owner
        .admit_product_confirm("inert-late", revision, rows[0].handle, confirmation.ticket)
        .unwrap()
    {
        ProductConfirmAdmission::Discover(task) => task,
        _ => panic!("fresh_confirm_expected"),
    };
    let old_token = task.token.clone();
    task.expiry = Instant::now() - Duration::from_millis(1);
    assert!(
        fixture
            .owner
            .complete_product_confirm(task.observe())
            .is_err()
    );
    assert!(!fixture.root.join("r/effects").exists());
    let before = fixture.owner.host().product_history_for_test();
    assert!(
        fixture
            .owner
            .capture_connection_close_inner(Some(&old_token))
            .is_err()
    );
    assert_eq!(fixture.owner.host().product_history_for_test(), before);
}

#[test]
fn product_preview_full_receipt_history_refuses_new_confirm_before_capture() {
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    for n in 0_u64..128 {
        let token = match fixture
            .owner
            .coordinator
            .reserve_external_close(
                &format!("preview-full-{n}"),
                revision,
                MutationDigest::from_semantic_bytes(&n.to_be_bytes()),
                false,
            )
            .unwrap()
        {
            ExternalCloseAdmission::Reserved(token) => token,
            _ => panic!("new_original_reservation_required"),
        };
        fixture
            .owner
            .coordinator
            .finish_external_close(&token, ExternalCloseOutcome::RefusedBeforeWrite)
            .unwrap();
    }
    let before = fixture.owner.host().product_history_for_test();
    assert!(matches!(
        fixture.owner.admit_product_confirm(
            "preview-overflow",
            revision,
            rows[0].handle,
            confirmation.ticket
        ),
        Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
    ));
    assert_eq!(fixture.owner.host().product_history_for_test(), before);
    assert!(fixture.owner.connection_close.preview.is_some());
    assert!(!fixture.root.join("r/effects").exists());
}

#[test]
fn product_preview_fresh_image_provider_refusal_never_falls_back_or_writes() {
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    let mut task = match fixture
        .owner
        .admit_product_confirm(
            "preview-provider-refused",
            revision,
            rows[0].handle,
            confirmation.ticket,
        )
        .unwrap()
    {
        ProductConfirmAdmission::Discover(task) => task,
        _ => panic!("fresh_original_discovery_required"),
    };
    task.discovery
        .observation
        .session_mut()
        .fail_installed_image_probe_for_test();
    assert!(
        fixture
            .owner
            .complete_product_confirm(task.observe())
            .is_err()
    );
    assert!(!fixture.root.join("r/effects").exists());
    assert!(fixture.owner.admit_connection_close_snapshot().is_err());
}
