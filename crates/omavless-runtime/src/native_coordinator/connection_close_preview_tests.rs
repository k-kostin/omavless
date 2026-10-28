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
fn product_selected_set_actual_socket_closes_both_members_in_one_operation() {
    let _fixtures = FIXTURES.lock().unwrap();
    let (root, mut owner) = fixture_parts("ok", None);
    owner.host_mut().install_product_preview_fixture_for_test();
    let fixture = SocketFixture::from_parts(root, owner);
    let require_ok = |reply: &serde_json::Value, stage: &str| {
        assert!(
            reply["ok"] == true,
            "selected-set {stage} refused: {}",
            reply["error"]["code"]
        );
    };
    let snapshot = fixture.call(
        "development.connection_sets.snapshot",
        json!({"instanceId":fixture.instance}),
    );
    require_ok(&snapshot, "snapshot");
    assert_eq!(
        snapshot["result"]["scope"],
        "development_owned_connection_set_close"
    );
    let rows = snapshot["result"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let handles = rows.iter().map(|r| r["handle"].clone()).collect::<Vec<_>>();
    let prepared = fixture.call(
        "development.connection_sets.prepare",
        json!({"instanceId":fixture.instance,"handles":handles}),
    );
    require_ok(&prepared, "prepared");
    assert_eq!(prepared["result"]["rows"], snapshot["result"]["rows"]);
    let submitted = fixture.call(
        "development.connection_sets.confirm",
        json!({"instanceId":fixture.instance,"expectedRevision":snapshot["revision"],
            "operationId":"selected-set-two","handles":handles,
            "ticket":prepared["result"]["ticket"]}),
    );
    require_ok(&submitted, "submitted");
    let deadline = Instant::now() + Duration::from_secs(4);
    let result = loop {
        let receipt = fixture.call(
            "development.connection_sets.receipt",
            json!({"instanceId":fixture.instance,"operationId":"selected-set-two"}),
        );
        require_ok(&receipt, "receipt");
        if receipt["result"]["state"] == "finished" {
            break receipt;
        }
        assert!(
            Instant::now() < deadline,
            "selected-set original did not finish"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(result["result"]["aggregate"], "all_closed");
    let members = result["result"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    for (index, member) in members.iter().enumerate() {
        assert_eq!(member["index"], index);
        assert_eq!(member["outcome"], "closed");
    }
    assert_eq!(
        result["result"]["receiptRevision"],
        snapshot["revision"].as_u64().unwrap() + 1
    );
}

#[test]
fn product_preview_older_confirm_has_retained_refusal_without_fresh_acquisition() {
    use crate::mutation::{BeginOutcome, MutationKind, MutationRequest, MutationResult};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    let before = fixture.owner.host().product_history_for_test();
    fixture
        .owner
        .coordinator
        .submit(
            MutationRequest::new(
                MutationKind::Other,
                Some("metadata-change"),
                Some(revision),
                MutationDigest::from_semantic_bytes(b"fixed-test-metadata"),
            )
            .unwrap(),
        )
        .unwrap();
    let BeginOutcome::Started(active) = fixture.owner.coordinator.begin_next().unwrap() else {
        panic!("original metadata scheduler admission required");
    };
    fixture
        .owner
        .coordinator
        .finish(active.token, MutationResult::Success)
        .unwrap();
    fixture.owner.invalidate_connection_close();
    let current = fixture.owner.revision();
    assert_eq!(current, revision + 1);
    let result = fixture
        .owner
        .admit_product_confirm(
            "stale-confirm",
            revision,
            rows[0].handle,
            confirmation.ticket,
        )
        .unwrap();
    let ProductConfirmAdmission::Replay(receipt) = result else {
        panic!("stale confirmation must never acquire a fresh image");
    };
    assert_eq!(receipt.outcome, ExternalCloseOutcome::RefusedBeforeWrite);
    assert_eq!(receipt.revision, current);
    assert_eq!(
        fixture
            .owner
            .connection_close_receipt("stale-confirm")
            .unwrap(),
        Some(Some(receipt))
    );
    assert!(matches!(
        fixture.owner.admit_product_confirm("stale-confirm", revision, rows[0].handle, confirmation.ticket),
        Ok(ProductConfirmAdmission::Replay(replayed)) if replayed == receipt
    ));
    for (requested_revision, handle, ticket) in [
        (current, rows[0].handle, confirmation.ticket),
        (revision, rows[1].handle, confirmation.ticket),
        (revision, rows[0].handle, OpaqueToken([0x55; 32])),
    ] {
        assert!(matches!(
            fixture.owner.admit_product_confirm(
                "stale-confirm",
                requested_revision,
                handle,
                ticket
            ),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        ));
    }
    assert_eq!(fixture.owner.revision(), current);
    assert_eq!(fixture.owner.host().product_history_for_test(), before);
    assert!(!fixture.root.join("r/effects").exists());
    assert!(fixture.owner.connection_close.discovery.is_none());
    assert!(fixture.owner.connection_close.snapshot.is_none());
}

#[test]
fn developer_stale_confirmation_keeps_revision_conflict_without_product_receipt() {
    use crate::lifecycle::{CloseRegistration, LifecycleHost};
    use crate::mutation::{BeginOutcome, MutationKind, MutationRequest, MutationResult};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    assert!(matches!(
        fixture.owner.host().close_registration(),
        CloseRegistration::Developer
    ));
    let rows = snapshot(&mut fixture);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let revision = fixture.owner.revision();
    fixture
        .owner
        .coordinator
        .submit(
            MutationRequest::new(
                MutationKind::Other,
                Some("developer-metadata"),
                Some(revision),
                MutationDigest::from_semantic_bytes(b"fixed-developer-metadata"),
            )
            .unwrap(),
        )
        .unwrap();
    let BeginOutcome::Started(active) = fixture.owner.coordinator.begin_next().unwrap() else {
        panic!("original scheduler admission required");
    };
    fixture
        .owner
        .coordinator
        .finish(active.token, MutationResult::Success)
        .unwrap();
    fixture.owner.invalidate_connection_close();
    assert!(matches!(
        fixture.owner.confirm_connection_close(
            "developer-stale",
            revision,
            rows[0].handle,
            confirmation.ticket,
        ),
        Err(NativeOwnerError::Coordinator(
            CoordinatorError::RevisionConflict
        ))
    ));
    assert_eq!(
        fixture
            .owner
            .connection_close_receipt("developer-stale")
            .unwrap(),
        None
    );
    assert_eq!(fixture.owner.revision(), revision + 1);
    assert!(!fixture.root.join("r/effects").exists());
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

fn start_product_set(
    fixture: &mut Fixture,
    operation: &str,
    fail_finish: bool,
) -> (Vec<OpaqueToken>, OpaqueToken, u64) {
    let rows = retain_inert_preview(fixture);
    let handles = rows.iter().map(|r| r.handle).collect::<Vec<_>>();
    let confirmation = fixture
        .owner
        .prepare_connection_set(handles.clone())
        .unwrap();
    let revision = fixture.owner.revision();
    let mut task = match fixture
        .owner
        .admit_connection_set(operation, revision, handles.clone(), confirmation.ticket)
        .unwrap()
    {
        SetAdmission::Discover(t) => t,
        _ => panic!("fresh set required"),
    };
    if fail_finish {
        task.discovery
            .observation
            .session_mut()
            .install_image_finish_probe_for_test(Box::new(|_| {
                Err(crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)
            }));
    }
    fixture
        .owner
        .complete_connection_set(task.observe())
        .unwrap();
    (handles, confirmation.ticket, revision)
}
fn set_result(fixture: &mut Fixture, operation: &str) -> ExternalSetReceipt {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(receipt) = fixture
            .owner
            .connection_set_receipt(operation)
            .unwrap()
            .unwrap()
        {
            return receipt;
        }
        assert!(Instant::now() < deadline, "bounded group terminal absent");
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn product_selected_set_partial_unknown_and_finish_failure_retain_exact_members_without_resend() {
    let _fixtures = FIXTURES.lock().unwrap();
    for (variant, fail_finish, expected) in [
        (
            "set-partial",
            false,
            vec![
                ExternalSetMember::Closed,
                ExternalSetMember::Changed,
                ExternalSetMember::NotAttempted,
            ],
        ),
        (
            "set-drop-second",
            false,
            vec![
                ExternalSetMember::Unknown,
                ExternalSetMember::Unknown,
                ExternalSetMember::NotAttempted,
            ],
        ),
        ("ok", true, vec![ExternalSetMember::Unknown; 2]),
    ] {
        let mut fixture = fixture(variant);
        fixture
            .owner
            .host_mut()
            .install_product_preview_fixture_for_test();
        let (handles, ticket, revision) =
            start_product_set(&mut fixture, "group-original", fail_finish);
        let receipt = set_result(&mut fixture, "group-original");
        assert_eq!(receipt.members, expected);
        assert_eq!(receipt.revision, revision + 1);
        let effects = fs::read(fixture.root.join("r/effects")).unwrap();
        assert_eq!(effects, b"1\n2\n");
        match fixture
            .owner
            .admit_connection_set("group-original", revision, handles, ticket)
            .unwrap()
        {
            SetAdmission::Replay(r) => assert_eq!(r, receipt),
            _ => panic!("replay must not rediscover"),
        }
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), effects);
        assert!(
            fixture
                .owner
                .coordinator
                .external_close_receipt("group-original")
                .is_err(),
            "wrong family read refused"
        );
        assert!(
            fixture.owner.admit_connection_close_snapshot().is_err(),
            "partial or Unknown cannot renew image factory"
        );
    }
}
#[test]
fn product_selected_set_final_finish_is_once_and_all_closed_renews_after_drop() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let rows = retain_inert_preview(&mut fixture);
    let handles = rows.iter().map(|r| r.handle).collect::<Vec<_>>();
    let confirmation = fixture
        .owner
        .prepare_connection_set(handles.clone())
        .unwrap();
    let revision = fixture.owner.revision();
    let mut task = match fixture
        .owner
        .admit_connection_set("group-finish", revision, handles, confirmation.ticket)
        .unwrap()
    {
        SetAdmission::Discover(t) => t,
        _ => panic!("fresh set required"),
    };
    let count = Arc::new(AtomicUsize::new(0));
    let finished = Arc::clone(&count);
    let checker = task
        .discovery
        .observation
        .session()
        .image_gate_checker_for_test();
    task.discovery
        .observation
        .session_mut()
        .install_image_finish_probe_for_test(Box::new(move |_| {
            assert!(checker().0);
            finished.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }));
    fixture
        .owner
        .complete_connection_set(task.observe())
        .unwrap();
    assert_eq!(
        set_result(&mut fixture, "group-finish").members,
        vec![ExternalSetMember::Closed; 2]
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(fixture.owner.connection_close.cancellation.is_none());
    assert!(
        fixture.owner.admit_connection_close_snapshot().is_ok(),
        "only original after-Drop group proof renews"
    );
}
#[test]
fn product_selected_set_full_preview_match_rejects_any_changed_member_before_all_posts() {
    let _fixtures = FIXTURES.lock().unwrap();
    for cut in ["removed", "reused", "display-drift"] {
        let mut fixture = product_preview_fixture();
        let rows = retain_inert_preview(&mut fixture);
        let handles = rows.iter().map(|r| r.handle).collect::<Vec<_>>();
        let confirmation = fixture
            .owner
            .prepare_connection_set(handles.clone())
            .unwrap();
        fs::write(fixture.root.join("r").join(cut), b"").unwrap();
        let task = match fixture
            .owner
            .admit_connection_set("group-changed", 0, handles, confirmation.ticket)
            .unwrap()
        {
            SetAdmission::Discover(t) => t,
            _ => panic!("fresh set required"),
        };
        let receipt = fixture
            .owner
            .complete_connection_set(task.observe())
            .unwrap()
            .unwrap();
        assert_eq!(receipt.members, vec![ExternalSetMember::NotAttempted; 2]);
        assert_eq!(receipt.revision, 0);
        assert!(!fixture.root.join("r/effects").exists());
    }
}

#[cfg(feature = "tui")]
#[test]
fn product_selected_set_actual_socket_production_tui_retains_one_original_operation() {
    use omavless_tui::developer_close::{Input, KeyCode, KeyEvent, KeyModifiers, Workspace};
    use omavless_tui::i18n::Locale;
    let _fixtures = FIXTURES.lock().unwrap();
    for locale in [Locale::En, Locale::Ru] {
        for (variant, aggregate, notice, members) in [
            (
                "ok",
                "all_closed",
                "tui.set_close_all_closed",
                vec!["closed", "closed"],
            ),
            (
                "set-partial",
                "partial",
                "tui.set_close_partial",
                vec!["closed", "changed", "not_attempted"],
            ),
            (
                "set-drop-second",
                "unknown",
                "tui.dev_close_unknown",
                vec!["unknown", "unknown", "not_attempted"],
            ),
        ] {
            let (root, mut owner) = fixture_parts(variant, None);
            owner.host_mut().install_product_preview_fixture_for_test();
            let fixture = SocketFixture::from_parts(root, owner);
            let now = Instant::now();
            let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
            let mut workspace = Workspace::new_selected_set(locale);
            let Input::Send(mut request) = workspace.input(key(KeyCode::Char('r')), now, true)
            else {
                panic!("explicit refresh")
            };
            loop {
                let wire = fixture.call(request.method(), request.params());
                assert_eq!(wire["ok"], true);
                match workspace.accept(Ok(wire), now) {
                    Some(next) => request = next,
                    None => break,
                }
            }
            workspace.input(key(KeyCode::Char('a')), now, true);
            workspace.input(key(KeyCode::Char('x')), now, true);
            let Input::Send(prepare) = workspace.input(key(KeyCode::Enter), now, true) else {
                panic!("review then prepare")
            };
            assert_eq!(prepare.method(), "development.connection_sets.prepare");
            workspace.accept(Ok(fixture.call(prepare.method(), prepare.params())), now);
            let Input::Send(confirm) = workspace.input(key(KeyCode::Enter), now, true) else {
                panic!("one explicit confirmation")
            };
            assert_eq!(confirm.method(), "development.connection_sets.confirm");
            let operation = confirm.params()["operationId"].clone();
            workspace.accept(Ok(fixture.call(confirm.method(), confirm.params())), now);
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                let Input::Send(receipt) =
                    workspace.input(key(KeyCode::Char('u')), Instant::now(), true)
                else {
                    panic!("receipt only")
                };
                assert_eq!(receipt.method(), "development.connection_sets.receipt");
                assert_eq!(receipt.params()["operationId"], operation);
                let wire = fixture.call(receipt.method(), receipt.params());
                let finished = wire["result"]["state"] == "finished";
                if finished {
                    assert_eq!(wire["result"]["aggregate"], aggregate);
                }
                workspace.accept(Ok(wire), Instant::now());
                if finished {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            let view = workspace.selected_set_result();
            assert_eq!(
                view,
                Some((locale.text(notice), members.as_slice())),
                "production client must consume actual aggregate and every member"
            );
            assert!(matches!(
                workspace.input(key(KeyCode::Char('r')), Instant::now(), true),
                Input::None
            ));
            assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n2\n");
        }
    }
}

// Existing real owned controller/socket with memory image callbacks: source
// propagation evidence only. The actual channel cut has separate SEQPACKET tests.
#[cfg(feature = "acceptance-connection-close")]
#[test]
fn acceptance_cut_claims_only_set_and_preserves_original_attempt_and_receipt_semantics() {
    use crate::close_acceptance::{Arm, Scenario};
    let _fixtures = FIXTURES.lock().unwrap();
    for cut in [Scenario::BeforeAttempt, Scenario::FinalProof] {
        let mut fixture = product_preview_fixture();
        let arm = std::sync::Arc::new(Arm::new(cut).unwrap());
        fixture
            .owner
            .host_mut()
            .install_close_acceptance_for_test(arm.clone());
        let _preview = retain_inert_preview(&mut fixture);
        assert!(
            !arm.is_claimed(),
            "inert preview must never consume effect cut"
        );
        let (handles, ticket, revision) =
            start_product_set(&mut fixture, "acceptance-original", false);
        let result = set_result(&mut fixture, "acceptance-original");
        assert!(arm.is_claimed());
        let expected = match cut {
            Scenario::BeforeAttempt => vec![ExternalSetMember::NotAttempted; 2],
            Scenario::FinalProof => vec![ExternalSetMember::Unknown; 2],
            Scenario::BetweenMembers | Scenario::LateFinish | Scenario::LatePreviewPublication => unreachable!("separate terminal/barrier control"),
        };
        assert_eq!(result.members, expected);
        let effects = fs::read(fixture.root.join("r/effects")).unwrap_or_default();
        assert_eq!(
            effects,
            if cut == Scenario::FinalProof {
                b"1\n2\n".to_vec()
            } else {
                vec![]
            }
        );
        match fixture
            .owner
            .admit_connection_set("acceptance-original", revision, handles, ticket)
            .unwrap()
        {
            SetAdmission::Replay(replay) => assert_eq!(replay, result),
            _ => panic!("original receipt must replay without any new effect"),
        }
        assert_eq!(
            fixture
                .owner
                .connection_set_receipt("acceptance-original")
                .unwrap()
                .unwrap()
                .unwrap(),
            result
        );
        assert_eq!(
            fs::read(fixture.root.join("r/effects")).unwrap_or_default(),
            effects
        );
        assert!(fixture.owner.admit_connection_close_snapshot().is_err());
    }
}

#[cfg(feature = "acceptance-connection-close")]
#[test]
fn acceptance_between_members_retains_shared_worker_busy_and_real_partial_receipt() {
    use crate::close_acceptance::{Arm, SIGNAL_TESTS, Scenario};
    let _signals = SIGNAL_TESTS.lock().unwrap();
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("set-partial");
    fixture
        .owner
        .host_mut()
        .install_product_preview_fixture_for_test();
    let arm = std::sync::Arc::new(Arm::new(Scenario::BetweenMembers).unwrap());
    let release = arm.release_for_test();
    fixture
        .owner
        .host_mut()
        .install_close_acceptance_for_test(arm);
    let _original = start_product_set(&mut fixture, "acceptance-partial", false);
    let until = Instant::now() + Duration::from_secs(2);
    while !release.entered() {
        assert!(
            Instant::now() < until,
            "original worker never reached between-member boundary"
        );
        std::thread::yield_now();
    }
    match fixture.owner.admit_connection_close_snapshot() {
        Err(e) => assert_eq!(
            e.stable_code(),
            omavless_control_protocol::StableErrorCode::Busy
        ),
        Ok(_) => panic!("competing capture admitted while original set worker owns reservation"),
    }
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    signal_hook::low_level::raise(signal_hook::consts::SIGUSR2).unwrap();
    let result = set_result(&mut fixture, "acceptance-partial");
    assert_eq!(
        result.members,
        vec![
            ExternalSetMember::Closed,
            ExternalSetMember::Changed,
            ExternalSetMember::NotAttempted
        ]
    );
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n2\n");
    assert!(fixture.owner.admit_connection_close_snapshot().is_err());
}

#[test]
fn product_selected_set_busy_snapshot_preserves_the_original_worker_until_finish() {
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let preview = retain_inert_preview(&mut fixture);
    let handles: Vec<_> = preview.iter().map(|r| r.handle).collect();
    let confirmation = fixture
        .owner
        .prepare_connection_set(handles.clone())
        .unwrap();
    let revision = fixture.owner.revision();
    let mut task = match fixture
        .owner
        .admit_connection_set("busy-original", revision, handles, confirmation.ticket)
        .unwrap()
    {
        SetAdmission::Discover(task) => task,
        _ => panic!("one fresh original group discovery required"),
    };
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    task.discovery
        .observation
        .session_mut()
        .install_image_finish_probe_for_test(Box::new(move |deadline| {
            entered_tx
                .try_send(())
                .map_err(|_| crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)?;
            let left = deadline
                .checked_duration_since(Instant::now())
                .ok_or(crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)?;
            release_rx
                .recv_timeout(left)
                .map_err(|_| crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)
        }));
    assert!(
        fixture
            .owner
            .complete_connection_set(task.observe())
            .unwrap()
            .is_none()
    );
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    match fixture.owner.admit_connection_close_snapshot() {
        Err(e) => assert_eq!(
            e.stable_code(),
            omavless_control_protocol::StableErrorCode::Busy
        ),
        Ok(_) => panic!("competing Snapshot invalidated or replaced an active group"),
    }
    assert_eq!(fixture.owner.revision(), revision);
    release_tx.send(()).unwrap();
    let result = set_result(&mut fixture, "busy-original");
    assert_eq!(result.members, vec![ExternalSetMember::Closed; 2]);
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n2\n");
    assert_eq!(
        fixture
            .owner
            .connection_set_receipt("busy-original")
            .unwrap()
            .unwrap()
            .unwrap(),
        result
    );
    assert!(
        fixture.owner.admit_connection_close_snapshot().is_ok(),
        "completed original retains normal next-epoch admission"
    );
}

#[cfg(feature = "acceptance-connection-close")]
#[test]
fn acceptance_late_publication_uses_real_retirement_object_and_actual_owner_refusal() {
    use crate::close_acceptance::{Arm, Scenario};
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = product_preview_fixture();
    let arm = std::sync::Arc::new(Arm::new(Scenario::LatePreviewPublication).unwrap());
    fixture.owner.host_mut().install_close_acceptance_for_test(arm.clone());
    let mut discovery = match fixture.owner.admit_connection_close_snapshot().unwrap() {
        CloseSnapshotAdmission::Discover(d) => d,
        _ => panic!("one original preview capture required"),
    };
    discovery.observation.session_mut().retirement_budget_for_test(Duration::from_millis(30));
    let ready = discovery.observe_preview().unwrap();
    assert!(arm.is_claimed());
    assert!(Instant::now() >= ready.retired.retirement_deadline());
    assert!(fixture.owner.retain_product_preview(Ok(ready)).is_err());
    assert!(fixture.owner.connection_close.preview.is_none());
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
    assert!(fixture.owner.admit_connection_close_snapshot().is_err());
    assert!(!fixture.root.join("r/effects").exists());
}
