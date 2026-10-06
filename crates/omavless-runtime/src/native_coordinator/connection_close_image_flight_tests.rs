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
