// SPDX-License-Identifier: MIT
// Existing real owner/controller fixture plus memory image-RPC callbacks.
// No helper/CAP_SYS_PTRACE, product package attestation or VM executes here.

fn image_probe_discovery(fixture: &mut Fixture) -> CloseDiscovery {
    let mut discovery = fixture.owner.capture_connection_close().unwrap();
    discovery
        .observation
        .session_mut()
        .capture_probe_image_for_test();
    discovery
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_retirement_before_effect_is_detached_one_shot_and_next_capture_is_fresh_guarded() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let original = session.cancellation();
    let image = session.original_image_for_test();
    let checker = session.image_gate_checker_for_test();
    session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
    session.install_image_finish_probe_for_test(Box::new(move |_| {
        assert!(checker().0, "retirement_finish_is_outside_lifetime_gate");
        Ok(())
    }));
    fixture
        .owner
        .host_mut()
        .install_product_epoch_from_fixture_session(original);
    let discovered = discovery.observe().unwrap();
    let rows = fixture.owner.retain_connection_close(discovered).unwrap();
    fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    let task = match fixture.owner.admit_connection_close_snapshot().unwrap() {
        CloseSnapshotAdmission::Retire(task) => task,
        _ => panic!("old_original_snapshot_must_retire_first"),
    };
    assert!(fixture.owner.connection_close.snapshot.is_none());
    assert!(fixture.owner.connection_close.pending.is_none());
    assert!(matches!(
        fixture.owner.admit_connection_close_snapshot(),
        Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
    ));
    assert!(
        fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .is_err()
    );
    let retired = task.retire().unwrap(); // no owner/migration lock held
    assert!(!fixture.root.join("r/effects").exists());
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Busy);
    // This capless memory fixture is deliberately NOT the root product package.
    // Only a NEW guarded capture may follow; its fixed package guard must reject.
    assert!(
        fixture
            .owner
            .complete_connection_close_retirement(Ok(retired))
            .is_err()
    );
    assert_eq!(fixture.owner.host().product_history_for_test(), (2, 1));
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
    assert!(!fixture.root.join("r/effects").exists());
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_retirement_finish_error_late_finish_and_cancellation_keep_original_slot_sealed() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    for cut in [
        "finish_error",
        "late_finish",
        "cancelled",
        "panic",
        "already_cancelled",
        "already_poisoned",
        "controller_drift",
    ] {
        let mut fixture = fixture("ok");
        let mut discovery = image_probe_discovery(&mut fixture);
        let session = discovery.observation.session_mut();
        let original = session.cancellation();
        let cancel = original.clone();
        let prior_cancel = cancel.clone();
        let image = session.original_image_for_test();
        session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
        session.install_image_finish_probe_for_test(Box::new(move |_| match cut {
            "finish_error" => Err(crate::conditional_close_candidate::Outcome::RefusedBeforeWrite),
            "cancelled" => {
                cancel.cancel();
                Ok(())
            }
            "panic" => panic!("fixed_synthetic_retirement_failure"),
            "late_finish" => {
                std::thread::sleep(Duration::from_millis(30));
                Ok(())
            }
            _ => Ok(()),
        }));
        fixture
            .owner
            .host_mut()
            .install_product_epoch_from_fixture_session(original);
        let discovered = discovery.observe().unwrap();
        fixture.owner.retain_connection_close(discovered).unwrap();
        if cut == "late_finish" {
            fixture
                .owner
                .connection_close
                .snapshot
                .as_mut()
                .unwrap()
                .observation
                .session_mut()
                .retirement_budget_for_test(Duration::from_millis(20));
        }
        let task = match fixture.owner.admit_connection_close_snapshot().unwrap() {
            CloseSnapshotAdmission::Retire(task) => task,
            _ => panic!("old_original_snapshot_must_retire_first"),
        };
        match cut {
            "already_cancelled" => prior_cancel.cancel(),
            "already_poisoned" => task.snapshot.observation.session().refuse_retirement(),
            "controller_drift" => fs::set_permissions(
                fixture.root.join("r/mihomo.sock"),
                fs::Permissions::from_mode(0o644),
            )
            .unwrap(),
            _ => (),
        }
        let result = task.retire();
        assert!(result.is_err());
        assert!(
            fixture
                .owner
                .complete_connection_close_retirement(result)
                .is_err()
        );
        assert!(fixture.owner.connection_close.retiring.is_some());
        assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
        assert_eq!(fixture.owner.host().product_history_for_test(), (1, 1));
        assert!(fixture.owner.admit_connection_close_snapshot().is_err());
        assert!(!fixture.root.join("r/effects").exists());
        if cut == "controller_drift" {
            fs::set_permissions(
                fixture.root.join("r/mihomo.sock"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
    }
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_receipt_capacity_refuses_snapshot_before_source_or_helper_acquisition() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let original = session.cancellation();
    let image = session.original_image_for_test();
    session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
    session.install_image_finish_probe_for_test(Box::new(move |_| Ok(())));
    fixture
        .owner
        .host_mut()
        .install_product_epoch_from_fixture_session(original);
    let discovered = discovery.observe().unwrap();
    let rows = fixture.owner.retain_connection_close(discovered).unwrap();
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    fixture
        .owner
        .confirm_connection_close("capacity-positive", 0, rows[0].handle, confirmation.ticket)
        .unwrap();
    assert_eq!(receipt(&mut fixture).outcome, ExternalCloseOutcome::Closed);
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Ready);
    for n in 1..128 {
        assert!(
            fixture
                .owner
                .confirm_connection_close(
                    &format!("refused-{n}"),
                    1,
                    rows[0].handle,
                    confirmation.ticket
                )
                .is_err()
        );
    }
    assert!(!fixture.owner.coordinator.close_receipt_capacity_available());
    assert!(matches!(
        fixture.owner.capture_connection_close(),
        Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
    ));
    assert_eq!(fixture.owner.host().product_history_for_test(), (1, 1));
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_retirement_observed_context_drift_is_sticky_before_and_after_detached_finish() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    for after in [false, true] {
        let mut fixture = fixture("ok");
        let mut discovery = image_probe_discovery(&mut fixture);
        let session = discovery.observation.session_mut();
        let original = session.cancellation();
        let image = session.original_image_for_test();
        session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
        session.install_image_finish_probe_for_test(Box::new(move |_| Ok(())));
        fixture
            .owner
            .host_mut()
            .install_product_epoch_from_fixture_session(original);
        let discovered = discovery.observe().unwrap();
        fixture.owner.retain_connection_close(discovered).unwrap();
        let path = fixture.root.join("c/config.yaml");
        let original_bytes = fs::read(&path).unwrap();
        if after {
            let task = match fixture.owner.admit_connection_close_snapshot().unwrap() {
                CloseSnapshotAdmission::Retire(task) => task,
                _ => panic!("same_original_snapshot_required"),
            };
            let retired = task.retire().unwrap();
            fs::write(&path, b"changed fixed public fixture\n").unwrap();
            assert!(
                fixture
                    .owner
                    .complete_connection_close_retirement(Ok(retired))
                    .is_err()
            );
            assert!(fixture.owner.connection_close.retiring.is_some());
        } else {
            fs::write(&path, b"changed fixed public fixture\n").unwrap();
            assert!(fixture.owner.admit_connection_close_snapshot().is_err());
            assert!(fixture.owner.connection_close.snapshot.is_some());
        }
        fs::write(&path, original_bytes).unwrap();
        assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
        assert!(fixture.owner.admit_connection_close_snapshot().is_err());
        assert_eq!(fixture.owner.host().product_history_for_test(), (1, 1));
        assert!(!fixture.root.join("r/effects").exists());
    }
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_retirement_delayed_result_cannot_borrow_future_snapshot_expiry() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let original = session.cancellation();
    let image = session.original_image_for_test();
    session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
    session.install_image_finish_probe_for_test(Box::new(move |_| Ok(())));
    fixture
        .owner
        .host_mut()
        .install_product_epoch_from_fixture_session(original);
    let discovered = discovery.observe().unwrap();
    fixture.owner.retain_connection_close(discovered).unwrap();
    let deadline = Instant::now() + Duration::from_millis(400);
    let expiry = fixture.owner.connection_close.snapshot.as_ref().unwrap().expiry;
    fixture
        .owner
        .connection_close
        .snapshot
        .as_mut()
        .unwrap()
        .observation
        .session_mut()
        .image_deadline_for_test(deadline); // shorten, never extend the original3s
    fixture
        .owner
        .connection_close
        .snapshot
        .as_mut()
        .unwrap()
        .observation
        .session_mut()
        .retirement_budget_for_test(Duration::from_millis(200));
    let task = match fixture.owner.admit_connection_close_snapshot().unwrap() {
        CloseSnapshotAdmission::Retire(task) => task,
        _ => panic!("same_original_snapshot_required"),
    };
    let retired = task.retire().unwrap();
    let retirement_deadline = retired.original.retirement_deadline();
    assert!(retirement_deadline < deadline);
    while Instant::now() < retirement_deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        Instant::now() < expiry,
        "snapshot5s_is_still_future"
    );
    assert!(
        fixture
            .owner
            .complete_connection_close_retirement(Ok(retired))
            .is_err()
    );
    assert!(fixture.owner.connection_close.retiring.is_some());
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
    assert_eq!(fixture.owner.host().product_history_for_test(), (1, 1));
    assert!(!fixture.root.join("r/effects").exists());
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_clock_expired_snapshot_retires_without_renewing_or_observing_old_authority() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let original = session.cancellation();
    let image = session.original_image_for_test();
    let reads = Arc::new(AtomicUsize::new(0));
    let read_count = Arc::clone(&reads);
    session.install_image_probe_for_test(Box::new(move |_| {
        read_count.fetch_add(1, Ordering::SeqCst);
        Ok(image.try_clone().unwrap())
    }));
    let finished = Arc::new(AtomicUsize::new(0));
    let finish_count = Arc::clone(&finished);
    let checker = session.image_gate_checker_for_test();
    session.install_image_finish_probe_for_test(Box::new(move |_| {
        assert!(checker().0, "terminal_finish_is_off_gate");
        finish_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }));
    fixture.owner.host_mut().install_product_epoch_from_fixture_session(original);
    let discovered = discovery.observe().unwrap();
    fixture.owner.retain_connection_close(discovered).unwrap();
    let snapshot = fixture.owner.connection_close.snapshot.as_mut().unwrap();
    snapshot.expiry = Instant::now() - Duration::from_millis(1);
    snapshot.observation.session_mut().image_deadline_for_test(
        Instant::now() - Duration::from_millis(1),
    );
    let old_reads = reads.load(Ordering::SeqCst);
    let task = match fixture.owner.admit_connection_close_snapshot().unwrap() {
        CloseSnapshotAdmission::Retire(task) => task,
        _ => panic!("same_original_terminal_retirement_required"),
    };
    let retired = task.retire().unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), old_reads, "no_expired_observe_or_new_image_flight");
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    assert!(Instant::now() < retired.original.retirement_deadline());
    assert!(!fixture.root.join("r/effects").exists());
    // Only a genuinely NEW guarded package capture follows. This synthetic
    // fixture still cannot promote itself to the root close-qualified package.
    assert!(fixture.owner.complete_connection_close_retirement(Ok(retired)).is_err());
    assert_eq!(fixture.owner.host().product_history_for_test(), (2, 1));
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_epoch_same_worker_publication_drains_before_next_snapshot_admission() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let original = session.cancellation();
    let image = session.original_image_for_test();
    session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
    session.install_image_finish_probe_for_test(Box::new(move |_| Ok(())));
    fixture
        .owner
        .host_mut()
        .install_product_epoch_from_fixture_session(original);
    let discovered = discovery.observe().unwrap();
    let rows = fixture.owner.retain_connection_close(discovered).unwrap();
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Busy);
    assert!(matches!(
        fixture.owner.capture_connection_close(),
        Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
    ));
    // Busy must not invalidate the original snapshot before its confirmation.
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    fixture
        .owner
        .confirm_connection_close("product-positive", 0, rows[0].handle, confirmation.ticket)
        .unwrap();
    let completed = receipt(&mut fixture);
    assert_eq!(completed.outcome, ExternalCloseOutcome::Closed);
    assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Ready);
    assert_eq!(fixture.owner.host().product_history_for_test(), (1, 1));
    assert_eq!(
        fixture
            .owner
            .confirm_connection_close("product-positive", 0, rows[0].handle, confirmation.ticket)
            .unwrap(),
        Some(completed)
    );
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    // Admission is tested here; no real product helper/package is fabricated.
}

#[cfg(feature = "product-image-witness")]
#[test]
fn product_epoch_unknown_or_failed_finish_never_renews_from_a_helper_ack() {
    use crate::lifecycle::{CloseEpochAdmission, LifecycleHost};
    let _fixtures = FIXTURES.lock().unwrap();
    for reply in ["drop", "ok"] {
        let mut fixture = fixture(reply);
        let mut discovery = image_probe_discovery(&mut fixture);
        let session = discovery.observation.session_mut();
        let original = session.cancellation();
        let image = session.original_image_for_test();
        session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
        session.install_image_finish_probe_for_test(Box::new(move |_| {
            Err(crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)
        }));
        fixture
            .owner
            .host_mut()
            .install_product_epoch_from_fixture_session(original);
        let discovered = discovery.observe().unwrap();
        let rows = fixture.owner.retain_connection_close(discovered).unwrap();
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        fixture
            .owner
            .confirm_connection_close("product-unknown", 0, rows[0].handle, confirmation.ticket)
            .unwrap();
        let completed = receipt(&mut fixture);
        assert_eq!(completed.outcome, ExternalCloseOutcome::Unknown);
        assert!(fixture.owner.host().close_epoch_admission() == CloseEpochAdmission::Refused);
        assert!(fixture.owner.capture_connection_close().is_err());
        assert_eq!(fixture.owner.host().product_history_for_test(), (1, 1));
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close("product-unknown", 0, rows[0].handle, confirmation.ticket)
                .unwrap(),
            Some(completed)
        );
    }
}

#[test]
fn image_rpc_cancelled_late_positive_is_offlock_and_never_published() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let image = session.original_image_for_test();
    let checker = session.image_gate_checker_for_test();
    let cancellation = session.cancellation();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let entered = Arc::clone(&barrier);
    let calls = Arc::new(AtomicUsize::new(0));
    let called = Arc::clone(&calls);
    session.install_image_probe_for_test(Box::new(move |_| {
        assert!(checker().0, "image_rpc_must_not_hold_lifetime_gate");
        called.fetch_add(1, Ordering::SeqCst);
        entered.wait();
        entered.wait();
        Ok(image.try_clone().unwrap())
    }));
    let worker = std::thread::spawn(move || discovery.observe());
    barrier.wait();
    // The real owner is still available while the counted image flight waits.
    assert_eq!(fixture.owner.revision(), 0);
    cancellation.cancel();
    barrier.wait();
    assert!(worker.join().unwrap().is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!fixture.root.join("r/effects").exists());
}

#[test]
fn image_rpc_expired_positive_and_wrong_image_are_sticky_before_effect() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    for wrong in [false, true] {
        let mut fixture = fixture("ok");
        let mut discovery = image_probe_discovery(&mut fixture);
        let session = discovery.observation.session_mut();
        let image = if wrong {
            fs::File::open(fixture.root.join("c/config.yaml")).unwrap()
        } else {
            session.original_image_for_test()
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let called = Arc::clone(&calls);
        if !wrong {
            session.image_deadline_for_test(Instant::now() + Duration::from_millis(10));
        }
        session.install_image_probe_for_test(Box::new(move |until| {
            called.fetch_add(1, Ordering::SeqCst);
            if !wrong {
                while Instant::now() <= until {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            Ok(image.try_clone().unwrap())
        }));
        assert!(!session.proves_live());
        assert!(!session.proves_live());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(!fixture.root.join("r/effects").exists());
    }
}

#[test]
fn image_rpc_positive_then_source_drift_never_admits_effect_or_retries() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    // Keep the existing interpreter AND its distinct original script evidence;
    // mutate only this fixture's public script after a valid memory RPC return.
    let mut discovery = fixture.owner.capture_connection_close().unwrap();
    let session = discovery.observation.session_mut();
    let image = session.original_image_for_test();
    let path = fixture.root.join("core.py");
    let calls = Arc::new(AtomicUsize::new(0));
    let called = Arc::clone(&calls);
    session.install_image_probe_for_test(Box::new(move |_| {
        called.fetch_add(1, Ordering::SeqCst);
        fs::write(&path, b"changed fixed public fixture\n").unwrap();
        Ok(image.try_clone().unwrap())
    }));
    assert!(!session.proves_live());
    assert!(!session.proves_live());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!fixture.root.join("r/effects").exists());
}

#[test]
fn image_rpc_failure_after_first_effect_chunk_stays_unknown_without_resend() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let image = session.original_image_for_test();
    let checker = session.image_gate_checker_for_test();
    let failures = Arc::new(AtomicUsize::new(0));
    let failed = Arc::clone(&failures);
    session.install_image_probe_for_test(Box::new(move |_| {
        let (free, attempted) = checker();
        assert!(free);
        if attempted {
            failed.fetch_add(1, Ordering::SeqCst);
            Err(crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)
        } else {
            Ok(image.try_clone().unwrap())
        }
    }));
    let discovered = discovery.observe().unwrap();
    let rows = fixture.owner.retain_connection_close(discovered).unwrap();
    fixture
        .owner
        .connection_close
        .snapshot
        .as_mut()
        .unwrap()
        .observation
        .session_mut()
        .partial_effect_chunks(16);
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    fixture
        .owner
        .confirm_connection_close("image-partial", 0, rows[0].handle, confirmation.ticket)
        .unwrap();
    let result = receipt(&mut fixture);
    assert_eq!(result.outcome, ExternalCloseOutcome::Unknown);
    assert_eq!(failures.load(Ordering::SeqCst), 1);
    assert!(!fixture.root.join("r/effects").exists()); // No complete POST; attempted bytes still Unknown.
    assert_eq!(
        fixture
            .owner
            .confirm_connection_close("image-partial", 0, rows[0].handle, confirmation.ticket)
            .unwrap(),
        Some(result)
    );
    assert_eq!(failures.load(Ordering::SeqCst), 1);
}

#[test]
fn image_rpc_final_refresh_failure_cannot_publish_closed_or_resend() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let image = session.original_image_for_test();
    let fail = Arc::new(AtomicBool::new(false));
    let failed = Arc::clone(&fail);
    session.install_image_probe_for_test(Box::new(move |_| {
        if failed.load(Ordering::SeqCst) {
            Err(crate::conditional_close_candidate::Outcome::RefusedBeforeWrite)
        } else {
            Ok(image.try_clone().unwrap())
        }
    }));
    let discovered = discovery.observe().unwrap();
    let rows = fixture.owner.retain_connection_close(discovered).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    fixture
        .owner
        .connection_close
        .snapshot
        .as_mut()
        .unwrap()
        .observation
        .session_mut()
        .pause_before_finish(Arc::clone(&barrier));
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    fixture
        .owner
        .confirm_connection_close("image-final", 0, rows[0].handle, confirmation.ticket)
        .unwrap();
    barrier.wait();
    fail.store(true, Ordering::SeqCst);
    barrier.wait();
    let result = receipt(&mut fixture);
    assert_eq!(result.outcome, ExternalCloseOutcome::Unknown);
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    assert_eq!(
        fixture
            .owner
            .confirm_connection_close("image-final", 0, rows[0].handle, confirmation.ticket)
            .unwrap(),
        Some(result)
    );
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
}

#[test]
fn image_rpc_cancel_during_finish_discards_ack_before_publication() {
    let _fixtures = FIXTURES.lock().unwrap();
    let mut fixture = fixture("ok");
    let mut discovery = image_probe_discovery(&mut fixture);
    let session = discovery.observation.session_mut();
    let image = session.original_image_for_test();
    let cancellation = session.cancellation();
    let checker = session.image_gate_checker_for_test();
    session.install_image_probe_for_test(Box::new(move |_| Ok(image.try_clone().unwrap())));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let entered = Arc::clone(&barrier);
    session.install_image_finish_probe_for_test(Box::new(move |_| {
        assert!(checker().0);
        entered.wait();
        entered.wait();
        Ok(())
    }));
    let discovered = discovery.observe().unwrap();
    let rows = fixture.owner.retain_connection_close(discovered).unwrap();
    let confirmation = fixture
        .owner
        .prepare_connection_close(rows[0].handle)
        .unwrap();
    fixture
        .owner
        .confirm_connection_close(
            "image-finish-cancel",
            0,
            rows[0].handle,
            confirmation.ticket,
        )
        .unwrap();
    barrier.wait();
    cancellation.cancel();
    barrier.wait();
    let result = receipt(&mut fixture);
    assert_eq!(result.outcome, ExternalCloseOutcome::Unknown);
    assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    assert_eq!(
        fixture
            .owner
            .confirm_connection_close(
                "image-finish-cancel",
                0,
                rows[0].handle,
                confirmation.ticket
            )
            .unwrap(),
        Some(result)
    );
}

#[test]
fn image_prepared_field_presence_changed_context_and_second_take_never_adopt() {
    let _fixtures = FIXTURES.lock().unwrap();
    for changed in [false, true] {
        let mut fixture = fixture("ok");
        let desired = fixture.owner.desired().unwrap();
        let observation = fixture
            .owner
            .host_mut()
            .capture_connection_close(&desired)
            .unwrap();
        let cancellation = observation.session().cancellation();
        fixture
            .owner
            .host_mut()
            .install_invalid_prepared_image_for_test(&desired, observation);
        if changed {
            fs::write(fixture.root.join("c/config.yaml"), b"changed\n").unwrap();
        }
        assert!(
            fixture
                .owner
                .host_mut()
                .verify_close_fixture(&desired)
                .is_err()
        );
        assert!(cancellation.is_cancelled());
        assert!(
            fixture
                .owner
                .host_mut()
                .capture_connection_close(&desired)
                .is_err()
        );
        assert!(
            fixture
                .owner
                .host_mut()
                .prepare_image_witness_close_fixture(&desired)
                .is_err()
        );
        assert!(
            fixture
                .owner
                .host_mut()
                .capture_connection_close(&desired)
                .is_err()
        );
        assert!(!fixture.root.join("r/effects").exists());
    }
}
