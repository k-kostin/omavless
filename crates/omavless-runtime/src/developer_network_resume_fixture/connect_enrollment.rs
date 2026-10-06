// SPDX-License-Identifier: MIT
//! Test-only reusable fresh setup. No preliminary coordinator and no Ready seed.
use super::*;
use crate::native_coordinator::network_enrollment::{FirstPermitFault, FreshSetupAuthority};

pub(crate) struct PreparedConnectFixture {
    pub(crate) context: Context,
    pub(crate) inputs: OwnerInputs,
    setup: FreshSetupAuthority,
    paths: RuntimePaths,
    facts: Arc<Mutex<Facts>>,
    clock: Clock,
    uid: u32,
}
impl PreparedConnectFixture {
    pub(crate) fn fault(mut self, fault: FirstPermitFault) -> Self {
        self.setup.fault = Some(fault);
        self
    }
    pub(crate) fn new(clock: Clock) -> Self {
        let setup = FreshSetupAuthority::create().expect("exclusive fresh setup");
        let base = setup.base();
        let uid = Uid::current().as_raw();
        let cutover = CutoverPaths::below(&base.join("runtime"), &base.join("state"), uid);
        atomic_replace_private(
            &cutover.ownership_marker,
            br#"{"schemaVersion":1,"generation":7,"phase":"rust"}"#,
            uid,
        )
        .unwrap();
        let desired = DesiredPaths::below(&base.join("state"));
        let target = DesiredState {
            mode: RoutingMode::Rule,
            generation: 12,
            ..DesiredState::default()
        };
        write_desired(&desired, uid, &target).unwrap();
        let store = base.join("config/profiles.json");
        let raw = serde_json::to_vec(&json!({"version":3,"activeId":"","lastId":PROFILE,
            "profiles":[{"id":PROFILE,"name":"Fixture","protocol":"vless","favorite":false,"uri":SELECTED_URI}],
            "subscriptions":[],"routingPreset":"custom","customRules":[],"rulesUpdatedAt":0,
            "startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":true})).unwrap();
        atomic_replace_private(&store, &raw, uid).unwrap();
        let context = Context {
            fence: Fence {
                boot: BOOT,
                owner_instance: INSTANCE,
                owner_generation: 7,
                desired_revision: 0,
                network_epoch: 1,
            },
            desired: target.clone(),
            store_digest: Sha256::digest(&raw).into(),
        };
        let paths = RuntimePaths::below(&base.join("runtime"));
        let facts = Arc::new(Mutex::new(Facts {
            observed: empty(),
            target,
            calls: Vec::new(),
            safe: true,
            observations: 0,
            observe_change: None,
            panic_at: None,
            first_connect: true,
            connect_event: None,
        }));
        Self {
            inputs: OwnerInputs {
                desired,
                store,
                cutover,
                enrollment: Enrollment {
                    boot: BOOT,
                    instance: INSTANCE,
                    epoch: 1,
                },
            },
            context,
            setup,
            paths,
            facts,
            clock,
            uid,
        }
    }
    // Source is constructed by a test's fixed private backend after fresh file
    // setup, before this ONE RuntimeServer/original owner is bound. No product
    // API accepts a source factory, and no safety bool comes from the source.
    pub(crate) fn bind(self, source: Source, producer: UnixStream, own_change: bool) -> Fixture {
        if own_change {
            self.facts.lock().unwrap().connect_event = Some(producer.try_clone().unwrap());
        }
        let base = self.setup.base().to_owned();
        let desired = self.inputs.desired.clone();
        let server = RuntimeServer::bind_connect_fixture(
            self.paths.clone(),
            Host(Arc::clone(&self.facts)),
            self.inputs,
            self.setup,
            Driver::new(source, self.clock),
            subscription_transport::HttpsSubscriptionTransport::new(),
        )
        .expect("original awaiting owner");
        let instance = server.instance_id.clone();
        Fixture {
            base,
            paths: self.paths,
            desired,
            uid: self.uid,
            facts: self.facts,
            producer,
            server: Some(server),
            runtime: None,
            stop: Arc::new(AtomicBool::new(false)),
            instance,
            sequence: u64::from(own_change),
            joined: false,
        }
    }
}

fn fresh() -> (Fixture, Arc<AtomicU64>) {
    fresh_fault(None)
}
fn fresh_fault(fault: Option<FirstPermitFault>) -> (Fixture, Arc<AtomicU64>) {
    let now = Arc::new(AtomicU64::new(0));
    let mut prepared = PreparedConnectFixture::new(Clock::injected(Arc::clone(&now)));
    if let Some(fault) = fault {
        prepared = prepared.fault(fault);
    }
    let (receiver, producer) = UnixStream::pair().unwrap();
    let source = Source::owned(
        receiver,
        std::process::id(),
        prepared.uid,
        &prepared.context,
    )
    .unwrap();
    (prepared.bind(source, producer, true), now)
}
fn connect(fixture: &Fixture, id: &str, revision: u64) -> Value {
    call(
        &fixture.paths,
        "connection.connect",
        json!({"profileId":PROFILE,"mode":"rule","operationId":id,"expectedRevision":revision}),
    )
    .unwrap()
}
fn receipt(fixture: &Fixture) -> PathBuf {
    fixture
        .desired
        .directory
        .join("network-resume-receipt.json")
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn socket_changed_connect_publishes_ready_then_one_original_guarded_recovery() {
    let (mut fixture, now) = fresh();
    assert!(!receipt(&fixture).exists());
    fixture.start();
    assert_eq!(fixture.state()["result"]["state"], "awaiting_connect");
    let accepted = connect(&fixture, "first-connect", 0);
    assert_eq!(accepted["ok"], true);
    assert_eq!(accepted["revision"], 1);
    let ready: Receipt = serde_json::from_slice(&fs::read(receipt(&fixture)).unwrap()).unwrap();
    assert_eq!(ready.phase, Phase::Ready);
    assert_eq!(ready.fence.desired_revision, 1);
    assert_eq!(
        fixture.facts.lock().unwrap().calls,
        ["prepare", "start", "commit"]
    );
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    fixture.wait("checking");
    now.store(3, Ordering::Release);
    fixture.wait("recovered");
    assert_eq!(
        fixture.facts.lock().unwrap().calls,
        ["prepare", "start", "commit", "prepare", "start", "commit"]
    );
    fixture.hint("NetworkChanged");
    now.store(9, Ordering::Release);
    assert_eq!(fixture.state()["result"]["state"], "recovered");
    assert_eq!(
        serde_json::from_slice::<Receipt>(&fs::read(receipt(&fixture)).unwrap())
            .unwrap()
            .phase,
        Phase::Finished
    );
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn accepted_nochange_disconnect_cancels_awaiting_and_later_connect_cannot_rearm() {
    let (mut fixture, _) = fresh();
    fixture.start();
    let response = call(
        &fixture.paths,
        "connection.disconnect",
        json!({"operationId":"off","expectedRevision":0}),
    )
    .unwrap();
    assert_eq!(response["ok"], true);
    assert_eq!(response["revision"], 0);
    assert_eq!(fixture.state()["result"]["state"], "cancelled");
    assert_eq!(connect(&fixture, "later", 0)["ok"], true);
    assert!(!receipt(&fixture).exists());
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn existing_ready_is_never_overwritten_and_does_not_change_completed_connect_response() {
    let (mut fixture, _) = fresh();
    fixture.start();
    let existing = b"previous-state";
    atomic_replace_private(&receipt(&fixture), existing, fixture.uid).unwrap();
    assert_eq!(connect(&fixture, "first", 0)["ok"], true);
    assert_eq!(fs::read(receipt(&fixture)).unwrap(), existing);
    assert_eq!(fixture.state()["result"]["state"], "manual_recovery");
    assert_eq!(connect(&fixture, "first", 0)["ok"], true); // exact replay cannot rearm
    assert_eq!(fs::read(receipt(&fixture)).unwrap(), existing);
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn provisioning_faults_preserve_completed_connect_and_never_rearm_on_replay_or_nochange() {
    for fault in [
        FirstPermitFault::AnchorBefore,
        FirstPermitFault::AnchorAfter,
        FirstPermitFault::ReadyBefore,
        FirstPermitFault::ReadyAfter,
        FirstPermitFault::Readback,
    ] {
        let (mut fixture, _) = fresh_fault(Some(fault));
        fixture.start();
        let response = connect(&fixture, "first", 0);
        assert_eq!(response["ok"], true);
        assert_eq!(response["revision"], 1);
        let current = crate::desired::read_desired(&fixture.desired, fixture.uid).unwrap();
        assert!(current.connected);
        assert_eq!(fixture.state()["result"]["state"], "manual_recovery");
        let before = fs::read(receipt(&fixture)).ok();
        assert_eq!(connect(&fixture, "first", 0)["ok"], true);
        assert_eq!(connect(&fixture, "no-change", 1)["ok"], true);
        assert_eq!(fs::read(receipt(&fixture)).ok(), before);
        assert_eq!(
            fixture.facts.lock().unwrap().calls,
            ["prepare", "start", "commit"]
        );
        fixture.shutdown();
        assert!(fixture.joined);
    }
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn hints_do_not_mint_ready_and_suspend_resume_or_lost_source_revoke_first_use() {
    for kind in ["NetworkChanged", "Suspend", "Resume"] {
        let (mut fixture, _) = fresh();
        fixture.server.as_ref().unwrap().wake_network_resume();
        fixture.sequence = 0;
        fixture.hint(kind);
        fixture.server.as_ref().unwrap().wake_network_resume();
        assert!(!receipt(&fixture).exists());
        assert!(fixture.facts.lock().unwrap().calls.is_empty());
        if kind != "NetworkChanged" {
            assert_eq!(
                fixture
                    .server
                    .as_ref()
                    .unwrap()
                    .dispatch(
                        &make_request("get", METHOD, json!({"instanceId":fixture.instance}))
                            .unwrap()
                    )
                    .unwrap()["result"]["state"],
                "manual_recovery"
            );
        }
        fixture.shutdown();
        assert!(fixture.joined);
    }
    let (mut fixture, _) = fresh();
    fixture.producer.shutdown(std::net::Shutdown::Both).unwrap();
    fixture.server.as_ref().unwrap().wake_network_resume();
    fixture.start();
    assert_eq!(connect(&fixture, "later", 0)["ok"], true);
    assert!(!receipt(&fixture).exists());
    fixture.shutdown();
    assert!(fixture.joined);
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn rejected_or_invalid_requests_leave_awaiting_unchanged_but_accepted_quit_seals_it() {
    let (mut fixture, _) = fresh();
    fixture.start();
    let rejected = connect(&fixture, "stale", 99);
    assert_eq!(rejected["ok"], false);
    let invalid = call(&fixture.paths,"connection.connect",json!({"profileId":PROFILE,"mode":"rule","path":"forbidden","operationId":"invalid","expectedRevision":0})).unwrap();
    assert_eq!(invalid["ok"], false);
    assert_eq!(fixture.state()["result"]["state"], "awaiting_connect");
    assert!(!receipt(&fixture).exists());
    assert!(fixture.facts.lock().unwrap().calls.is_empty());
    // No background effect runs while Awaiting; admission remains the existing
    // actual Quit path and its ordinary bounded pre-admission Busy behavior.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let response = call(
            &fixture.paths,
            "runtime.quit",
            json!({"instanceId":fixture.instance,"expectedRevision":0,"operationId":"quit"}),
        )
        .unwrap();
        if response["ok"] == true {
            break;
        }
        assert_eq!(response["error"]["code"], "busy");
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    fixture.shutdown();
    assert!(fixture.joined);
    assert!(!receipt(&fixture).exists());
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn removed_anchor_or_unsafe_binding_never_rearms_and_original_connect_stays_successful() {
    for lost_anchor in [true, false] {
        let (mut fixture, _) = fresh();
        if lost_anchor {
            fs::remove_file(
                fixture
                    .desired
                    .directory
                    .join("network-resume-enrollment.json"),
            )
            .unwrap();
        } else {
            fixture.facts.lock().unwrap().safe = false;
        }
        fixture.start();
        assert_eq!(connect(&fixture, "first", 0)["ok"], true);
        assert!(!receipt(&fixture).exists());
        assert_eq!(connect(&fixture, "later", 1)["ok"], true);
        assert!(!receipt(&fixture).exists());
        fixture.shutdown();
        assert!(fixture.joined);
    }
}

#[test]
#[ignore = "NEW enrollment boundary requires ROOT primary and independent exact-head review"]
fn replacement_source_with_identical_logical_fence_cannot_use_original_first_permit() {
    let prepared = PreparedConnectFixture::new(Clock::injected(Arc::new(AtomicU64::new(0))));
    let (receiver, original_sender) = UnixStream::pair().unwrap();
    let original = Source::owned(
        receiver,
        std::process::id(),
        prepared.uid,
        &prepared.context,
    )
    .unwrap();
    let (receiver, replacement_sender) = UnixStream::pair().unwrap();
    let mut replacement = Source::owned(
        receiver,
        std::process::id(),
        prepared.uid,
        &prepared.context,
    )
    .unwrap();
    let base = prepared.setup.base().to_owned();
    let desired = prepared.inputs.desired.clone();
    let mut driver = Driver::new(original, prepared.clock);
    let mut owner = crate::production_owner::ProductionNativeOwner::initialize_connect_fixture(
        Host(Arc::clone(&prepared.facts)),
        prepared.inputs,
        prepared.uid,
        prepared.setup,
        &mut driver,
    )
    .unwrap();
    let request = make_request(
        "connect",
        "connection.connect",
        json!({"profileId":PROFILE,"mode":"rule","expectedRevision":0,"operationId":"first"}),
    )
    .unwrap();
    let response = owner
        .respond_connection_with_source(&request, &mut replacement, &driver.clock)
        .unwrap();
    assert_eq!(response["ok"], true);
    assert_eq!(
        owner.network_resume_status(),
        crate::network_resume::Status::ManualRecovery
    );
    assert!(
        !desired
            .directory
            .join("network-resume-receipt.json")
            .exists()
    );
    drop((
        owner,
        driver,
        replacement,
        original_sender,
        replacement_sender,
    ));
    fs::remove_dir_all(base).unwrap();
}
