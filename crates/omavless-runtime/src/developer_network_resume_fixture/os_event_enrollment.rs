// SPDX-License-Identifier: MIT
//! Same original owner + real private D-Bus wire, not host/kernel acceptance.
use super::connect_enrollment::PreparedConnectFixture;
use super::*;
use crate::host_event_source::{
    HostEventSource,
    tests::{PrivateBus, signal},
};

fn prepare() -> (Fixture, Arc<AtomicU64>, PrivateBus, zbus::Connection) {
    let now = Arc::new(AtomicU64::new(0));
    let prepared = PreparedConnectFixture::new(Clock::injected(Arc::clone(&now)));
    let bus = PrivateBus::new();
    let login = bus.login(false, false, false);
    let source = Source::from_host(
        HostEventSource::fixture(&bus.socket).unwrap(),
        &prepared.context,
    )
    .unwrap();
    // Legacy fixture owns a producer for its old hint helpers. This endpoint is
    // unused here; Source retains the actual original adapter directly.
    let (unused_receiver, unused_producer) = UnixStream::pair().unwrap();
    drop(unused_receiver);
    (
        prepared.bind(source, unused_producer, false),
        now,
        bus,
        login,
    )
}
fn connect(fixture: &Fixture) -> Value {
    call(
        &fixture.paths,
        "connection.connect",
        json!({
            "profileId":PROFILE,"mode":"rule","operationId":"wire-connect","expectedRevision":0
        }),
    )
    .unwrap()
}

#[test]
#[ignore = "new combined original-source boundary requires primary and independent review"]
fn actual_private_bus_connect_creates_first_ready_then_sleep_resume_recovers_once() {
    let (mut fixture, now, bus, login) = prepare();
    let receipt = fixture
        .desired
        .directory
        .join("network-resume-receipt.json");
    assert!(!receipt.exists());
    fixture.start();
    assert_eq!(connect(&fixture)["ok"], true);
    assert_eq!(
        serde_json::from_slice::<Receipt>(&fs::read(&receipt).unwrap())
            .unwrap()
            .phase,
        Phase::Ready
    );
    fixture.facts.lock().unwrap().observed = empty();
    signal(&login, true);
    fixture.wait("paused");
    now.store(4, Ordering::Release);
    assert_eq!(fixture.facts.lock().unwrap().calls.len(), 3);
    signal(&login, false);
    fixture.wait("checking");
    now.store(7, Ordering::Release);
    fixture.wait("recovered");
    assert_eq!(
        fixture.facts.lock().unwrap().calls,
        ["prepare", "start", "commit", "prepare", "start", "commit"]
    );
    let finished: Receipt = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
    assert_eq!(finished.phase, Phase::Finished);
    assert_eq!(finished.fence.desired_revision, 1);
    signal(&login, false);
    now.store(12, Ordering::Release);
    assert_eq!(fixture.state()["result"]["state"], "recovered");
    assert_eq!(fixture.facts.lock().unwrap().calls.len(), 6);
    fixture.shutdown();
    assert!(fixture.joined);
    drop((login, bus));
}

#[test]
#[ignore = "new combined original-source boundary requires primary and independent review"]
fn actual_private_bus_owner_replacement_revokes_ready_without_a_recovery_or_rearm() {
    let (mut fixture, now, bus, login) = prepare();
    fixture.start();
    assert_eq!(connect(&fixture)["ok"], true);
    let receipt = fixture
        .desired
        .directory
        .join("network-resume-receipt.json");
    let ready = fs::read(&receipt).unwrap();
    fixture.facts.lock().unwrap().observed = empty();
    async_io::block_on(futures_lite::future::or(
        async {
            login
                .release_name("org.freedesktop.login1")
                .await
                .map_err(|_| crate::host_event_source::Lost::Unavailable)
        },
        async {
            async_io::Timer::after(Duration::from_secs(2)).await;
            Err(crate::host_event_source::Lost::Deadline)
        },
    ))
    .unwrap();
    let replacement = bus.login(false, false, false);
    fixture.wait("source_unavailable");
    signal(&replacement, false);
    now.store(10, Ordering::Release);
    assert_eq!(fixture.state()["result"]["state"], "source_unavailable");
    assert_eq!(fixture.facts.lock().unwrap().calls.len(), 3);
    assert_eq!(fs::read(receipt).unwrap(), ready);
    fixture.shutdown();
    assert!(fixture.joined);
    drop((replacement, login, bus));
}

#[test]
#[ignore = "new combined original-source boundary requires primary and independent review"]
fn actual_private_bus_loss_while_awaiting_preserves_explicit_connect_without_ready() {
    let (mut fixture, _, mut bus, login) = prepare();
    fixture.start();
    bus.stop().unwrap();
    fixture.wait("manual_recovery");
    assert_eq!(connect(&fixture)["ok"], true);
    assert!(
        crate::desired::read_desired(&fixture.desired, fixture.uid)
            .unwrap()
            .connected
    );
    assert!(
        !fixture
            .desired
            .directory
            .join("network-resume-receipt.json")
            .exists()
    );
    assert_eq!(
        fixture.facts.lock().unwrap().calls,
        ["prepare", "start", "commit"]
    );
    fixture.shutdown();
    assert!(fixture.joined);
    drop((login, bus));
}
