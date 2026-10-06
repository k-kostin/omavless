// SPDX-License-Identifier: MIT
use super::*;

#[test]
#[ignore = "constructor/socket boundary: execute only after primary and independent review"]
fn constructor_real_observation_refuses_empty_mixed_residual_and_changed_pointer_or_health() {
    for initial in [
        Initial::EmptyOn,
        Initial::MixedOn,
        Initial::ResidualOff,
        Initial::PointerMismatch,
        Initial::HealthyThenEmpty,
    ] {
        let result = Fixture::construct(
            Clock::injected(Arc::new(AtomicU64::new(0))),
            false,
            subscription_transport::HttpsSubscriptionTransport::new(),
            initial,
        );
        let refusal = match result {
            Err(refusal) => refusal,
            Ok(_) => panic!("unsafe initial facts accepted"),
        };
        assert_eq!(refusal.code, RuntimeError::NativeOwnerUnavailable);
        assert!(refusal.facts.lock().unwrap().calls.is_empty());
    }
    let mut off = match Fixture::construct(
        Clock::injected(Arc::new(AtomicU64::new(0))),
        false,
        subscription_transport::HttpsSubscriptionTransport::new(),
        Initial::SettledOff,
    ) {
        Ok(fixture) => fixture,
        Err(_) => panic!("settled Off refused"),
    };
    off.start();
    assert_eq!(off.state()["result"]["state"], "observe_only");
    assert!(off.facts.lock().unwrap().calls.is_empty());
    off.shutdown();
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn bounded_backlog_retains_queued_suspend_before_any_recovery() {
    let (mut fixture, now) = controlled();
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    for _ in 0..4 {
        fixture.hint("NetworkChanged");
    }
    fixture.hint("Suspend");
    let server = fixture.server.as_ref().unwrap();
    server.wake_network_resume();
    now.store(3, Ordering::Release);
    server.wake_network_resume();
    let request = make_request("get", METHOD, json!({"instanceId":fixture.instance})).unwrap();
    assert_eq!(
        server.dispatch(&request).unwrap()["result"]["state"],
        "paused"
    );
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    fixture.shutdown();
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn observer_panic_keeps_original_inflight_barrier_terminal() {
    let (mut fixture, now) = controlled();
    fixture.start();
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    fixture.wait("checking");
    let next = fixture.facts.lock().unwrap().observations + 1;
    fixture.facts.lock().unwrap().panic_at = Some(next);
    now.store(3, Ordering::Release);
    fixture.wait("manual_recovery");
    now.store(9, Ordering::Release);
    assert_eq!(fixture.state()["result"]["terminal"], true);
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn quit_accepted_before_or_after_effect_seals_the_same_owner_event_context() {
    for after in [false, true] {
        let (mut fixture, now) = controlled();
        fixture.start();
        fixture.facts.lock().unwrap().observed = empty();
        fixture.hint("Resume");
        fixture.wait("checking");
        if after {
            now.store(3, Ordering::Release);
            fixture.wait("recovered");
        }
        let response = call(
            &fixture.paths,
            "runtime.quit",
            json!({"instanceId":fixture.instance,
            "expectedRevision":if after {1} else {0},"operationId":"fixed-quit"}),
        )
        .unwrap();
        assert_eq!(response["ok"], true);
        fixture.shutdown();
        assert!(fixture.joined);
        assert_eq!(
            fixture
                .facts
                .lock()
                .unwrap()
                .calls
                .iter()
                .filter(|call| **call == "start")
                .count(),
            usize::from(after)
        );
    }
}

struct Feed {
    panic: AtomicBool,
}
impl subscription_transport::SubscriptionTransport for Feed {
    fn fetch(
        &self,
        url: &str,
    ) -> std::result::Result<
        omavless_domain::subscription_feed::PrivateSubscriptionBody,
        subscription_transport::SubscriptionTransportError,
    > {
        subscription_batch_work::BudgetedSubscriptionTransport::fetch_with_budget(
            self,
            url,
            Duration::from_secs(1),
        )
    }
}
impl subscription_batch_work::BudgetedSubscriptionTransport for Feed {
    fn fetch_with_budget(
        &self,
        _: &str,
        _: Duration,
    ) -> std::result::Result<
        omavless_domain::subscription_feed::PrivateSubscriptionBody,
        subscription_transport::SubscriptionTransportError,
    > {
        if self.panic.swap(false, Ordering::AcqRel) {
            panic!("fixed synthetic transport loss");
        }
        Ok(omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
            format!("{SELECTED_URI}\nvless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Synthetic").into_bytes()).unwrap())
    }
}
fn automatic_fixture(panic: bool) -> (Fixture, Arc<AtomicU64>, Arc<AtomicBool>, Arc<AtomicU64>) {
    let network_now = Arc::new(AtomicU64::new(0));
    let mut fixture = Fixture::with_transport(
        Clock::injected(Arc::clone(&network_now)),
        true,
        Feed {
            panic: AtomicBool::new(panic),
        },
    );
    let automatic_now = Arc::new(AtomicU64::new(100));
    let wake = Arc::new(AtomicBool::new(false));
    let clock = Arc::clone(&automatic_now);
    let wakeup = Arc::clone(&wake);
    fixture
        .server
        .as_mut()
        .unwrap()
        .register_developer_subscription_schedule(
            crate::developer_subscription_schedule::DeveloperSubscriptionSchedule::new(
                move || clock.load(Ordering::Acquire),
                move || wakeup.swap(false, Ordering::AcqRel),
            ),
        )
        .unwrap();
    (fixture, network_now, wake, automatic_now)
}
fn schedule(fixture: &Fixture) -> Value {
    call(
        &fixture.paths,
        "developer.subscription_schedule.get",
        json!({"instanceId":fixture.instance}),
    )
    .unwrap()
}
fn enable(fixture: &Fixture) {
    assert_eq!(
        call(
            &fixture.paths,
            "developer.subscription_schedule.set",
            json!({"instanceId":fixture.instance,"expectedPreferenceRevision":0,
        "intervalSecs":crate::subscription_schedule_plan::MIN_INTERVAL_SECS})
        )
        .unwrap()["ok"],
        true
    );
}
fn wait_attempt(fixture: &Fixture, state: &str) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        let value = schedule(fixture);
        if value["result"]["attempt"]["state"] == state {
            return value;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fixed automatic fixture deadline"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "combined socket boundary: execute only after primary and independent review"]
fn automatic_commit_changes_revision_and_store_before_old_resume_context_can_recover() {
    let (mut fixture, network_now, wake, _) = automatic_fixture(false);
    fixture.start();
    enable(&fixture);
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    fixture.wait("checking");
    wake.store(true, Ordering::Release);
    let completed = wait_attempt(&fixture, "succeeded");
    assert_eq!(completed["revision"], 1);
    network_now.store(3, Ordering::Release);
    fixture.wait("cancelled");
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "combined socket boundary: execute only after primary and independent review"]
fn automatic_acknowledgement_cannot_clear_source_lost_network_eligibility() {
    let (mut fixture, network_now, wake, automatic_now) = automatic_fixture(true);
    fixture.start();
    enable(&fixture);
    fixture
        .producer
        .shutdown(std::net::Shutdown::Write)
        .unwrap();
    fixture.wait("source_unavailable");
    wake.store(true, Ordering::Release);
    let failed = wait_attempt(&fixture, "uncertain");
    assert_eq!(
        call(
            &fixture.paths,
            "developer.subscription_schedule.set",
            json!({"instanceId":fixture.instance,
        "expectedPreferenceRevision":failed["result"]["preferenceRevision"],"intervalSecs":0})
        )
        .unwrap()["ok"],
        true
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let response = call(
            &fixture.paths,
            "developer.subscription_schedule.acknowledge",
            json!({"instanceId":fixture.instance,
            "attemptSequence":failed["result"]["attempt"]["sequence"],"expectedPreferenceRevision":2,
            "expectedRevision":failed["revision"]}),
        )
        .unwrap();
        if response["ok"] == true {
            break;
        }
        assert_eq!(response["error"]["code"], "busy");
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    automatic_now.store(200, Ordering::Release);
    network_now.store(10, Ordering::Release);
    assert_eq!(fixture.state()["result"]["state"], "source_unavailable");
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    fixture.shutdown();
    assert!(fixture.joined);
}

fn controlled() -> (Fixture, Arc<AtomicU64>) {
    let now = Arc::new(AtomicU64::new(0));
    (Fixture::new(Clock::injected(Arc::clone(&now))), now)
}

#[test]
#[ignore = "constructor/owned fixture boundary: execute only after primary and independent review"]
fn failed_runtime_join_preserves_first_terminal_result_and_owned_evidence_on_drop() {
    let (mut fixture, _) = controlled();
    drop(fixture.server.take());
    fixture.runtime = Some(thread::spawn(|| Err(RuntimeError::NativeOwnerUnavailable)));
    let evidence = fixture.base.join("failed-join-evidence.json");
    atomic_replace_private(&evidence, b"{\"syntheticFailedJoin\":true}", fixture.uid).unwrap();
    let base = fixture.base.clone();
    fixture.shutdown();
    assert!(!fixture.joined);
    fixture.shutdown();
    assert!(!fixture.joined);
    drop(fixture);
    assert!(evidence.is_file());
    // Test-only cleanup after proving retention of this exact owned fixture.
    fs::remove_dir_all(base).unwrap();
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn private_socket_and_owned_event_source_recover_on_the_same_registered_owner_once() {
    let (mut fixture, now) = controlled();
    fixture.start();
    assert_eq!(fixture.state()["result"]["state"], "observe_only");
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    fixture.wait("checking");
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    now.store(3, Ordering::Release);
    let recovered = fixture.wait("recovered");
    assert_eq!(recovered["revision"], 1);
    assert_eq!(
        fixture.facts.lock().unwrap().calls,
        ["prepare", "start", "commit"]
    );
    fixture.hint("Resume");
    now.store(10, Ordering::Release);
    thread::sleep(Duration::from_millis(60));
    assert_eq!(fixture.facts.lock().unwrap().calls.len(), 3);
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn disconnect_accepted_before_effect_wins_and_after_effect_stops_the_one_recovery() {
    for before in [true, false] {
        let (mut fixture, now) = controlled();
        fixture.start();
        fixture.facts.lock().unwrap().observed = empty();
        fixture.hint("Resume");
        fixture.wait("checking");
        if !before {
            now.store(3, Ordering::Release);
            fixture.wait("recovered");
        }
        let revision = if before { 0 } else { 1 };
        let response = call(
            &fixture.paths,
            "connection.disconnect",
            json!({"expectedRevision":revision,"operationId":"fixed-disconnect"}),
        )
        .unwrap();
        assert_eq!(response["ok"], true);
        now.store(5, Ordering::Release);
        if before {
            fixture.wait("cancelled");
        }
        assert_eq!(
            fixture
                .facts
                .lock()
                .unwrap()
                .calls
                .iter()
                .filter(|step| **step == "start")
                .count(),
            usize::from(!before)
        );
        assert!(
            !crate::desired::read_desired(&fixture.desired, fixture.uid)
                .unwrap()
                .connected
        );
        fixture.shutdown();
        assert!(fixture.joined);
    }
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn pure_get_never_peeks_drains_initializes_or_observes_pending_source() {
    let (mut fixture, _now) = controlled();
    fixture
        .producer
        .write_all(b"{\"sequence\":1,\"kind\":\"private-unsupported\"}\n")
        .unwrap();
    let server = fixture.server.as_ref().unwrap();
    for _ in 0..3 {
        let request = make_request("get", METHOD, json!({"instanceId":fixture.instance})).unwrap();
        assert_eq!(
            server.dispatch(&request).unwrap()["result"]["state"],
            "observe_only"
        );
    }
    assert_eq!(fixture.facts.lock().unwrap().observations, 2);
    server.wake_network_resume();
    let request = make_request("get", METHOD, json!({"instanceId":fixture.instance})).unwrap();
    assert_eq!(
        server.dispatch(&request).unwrap()["result"]["state"],
        "source_unavailable"
    );
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    fixture.shutdown();
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn source_eof_gap_partial_timeout_and_clock_regression_are_terminal_without_effects() {
    for failure in 0..4 {
        let (mut fixture, now) = controlled();
        fixture.start();
        fixture.facts.lock().unwrap().observed = empty();
        match failure {
            0 => fixture
                .producer
                .shutdown(std::net::Shutdown::Write)
                .unwrap(),
            1 => fixture
                .producer
                .write_all(b"{\"sequence\":2,\"kind\":\"Resume\"}\n")
                .unwrap(),
            2 => fixture.producer.write_all(b"{\"sequence\":1,").unwrap(),
            _ => {
                now.store(10, Ordering::Release);
                fixture.hint("Resume");
                fixture.wait("checking");
                now.store(9, Ordering::Release);
            }
        }
        fixture.wait("source_unavailable");
        now.store(20, Ordering::Release);
        thread::sleep(Duration::from_millis(60));
        assert!(fixture.facts.lock().unwrap().calls.is_empty());
        fixture.shutdown();
        assert!(fixture.joined);
    }
}

#[test]
#[ignore = "new socket boundary: execute only after primary and independent review"]
fn shutdown_terminalizes_network_before_jobs_and_does_not_rearm_from_ready() {
    let (mut fixture, now) = controlled();
    fixture.start();
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    fixture.wait("checking");
    fixture.shutdown();
    assert!(fixture.joined);
    now.store(3, Ordering::Release);
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    let receipt: Receipt = serde_json::from_slice(
        &fs::read(
            fixture
                .desired
                .directory
                .join("network-resume-receipt.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt.phase, Phase::Ready);
}
