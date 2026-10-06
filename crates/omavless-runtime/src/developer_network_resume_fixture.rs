// SPDX-License-Identifier: MIT
//! Fixed owned files/control socket/event pair. No caller data or host bus.
use super::*;
use crate::cutover::CutoverPaths;
use crate::desired::{DesiredPaths, DesiredState, OwnedObservation, RoutingMode, write_desired};
use crate::developer_network_resume::{Clock, Driver, Enrollment, METHOD, OwnerInputs};
use crate::lifecycle::{HostStepError, LifecycleHost};
use crate::native_coordinator::network_resume::ResumeBinding;
use crate::network_recovery_receipt::{Fence, Phase, Receipt};
use crate::network_resume::{Context, Source};
use omavless_store::atomic_replace_private;
use std::io::Write;
use std::sync::atomic::AtomicU64;

const PROFILE: &str = "11111111-1111-4111-8111-111111111111";
const SELECTED_URI: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Fixture";
const BOOT: [u8; 16] = [11; 16];
const INSTANCE: [u8; 16] = [22; 16];
fn empty() -> OwnedObservation {
    OwnedObservation {
        service_active: false,
        controller_ready: false,
        core_count: 0,
        tun_count: 0,
        active_profile_matches: false,
    }
}
fn healthy() -> OwnedObservation {
    OwnedObservation {
        service_active: true,
        controller_ready: true,
        core_count: 1,
        tun_count: 1,
        active_profile_matches: true,
    }
}

struct Facts {
    observed: OwnedObservation,
    target: DesiredState,
    calls: Vec<&'static str>,
    safe: bool,
    observations: usize,
    #[cfg(test)]
    observe_change: Option<(usize, OwnedObservation)>,
    #[cfg(test)]
    panic_at: Option<usize>,
    #[cfg(test)]
    first_connect: bool,
    #[cfg(test)]
    connect_event: Option<UnixStream>,
}
struct Host(Arc<Mutex<Facts>>);
impl LifecycleHost for Host {
    fn observe(
        &mut self,
        _: &DesiredState,
    ) -> std::result::Result<OwnedObservation, HostStepError> {
        let mut facts = self.0.lock().map_err(|_| HostStepError::Observation)?;
        facts.observations += 1;
        #[cfg(test)]
        if facts.panic_at == Some(facts.observations) {
            panic!("fixed network observer loss");
        }
        #[cfg(test)]
        if let Some((at, observed)) = facts.observe_change
            && at == facts.observations
        {
            facts.observed = observed;
        }
        Ok(facts.observed)
    }
    fn prepare(&mut self, desired: &DesiredState) -> std::result::Result<(), HostStepError> {
        let mut facts = self.0.lock().map_err(|_| HostStepError::Prepare)?;
        #[cfg(test)]
        if facts.first_connect {
            if !desired.connected
                || desired.profile_id != PROFILE
                || desired.mode != RoutingMode::Rule
                || facts.target.connected
                || facts.target.generation.checked_add(1) != Some(desired.generation)
            {
                return Err(HostStepError::Prepare);
            }
            facts.first_connect = false;
            facts.target = desired.clone();
            if let Some(mut sender) = facts.connect_event.take() {
                // Optional event delivery cannot turn a completed user Connect
                // into a failure; the original source check handles loss.
                let _ = writeln!(sender, "{}", json!({"sequence":1,"kind":"NetworkChanged"}));
            }
        }
        if desired != &facts.target {
            return Err(HostStepError::Prepare);
        }
        facts.calls.push("prepare");
        Ok(())
    }
    fn start_prepared(&mut self) -> std::result::Result<(), HostStepError> {
        let mut facts = self.0.lock().map_err(|_| HostStepError::Start)?;
        facts.calls.push("start");
        facts.observed = healthy();
        Ok(())
    }
    fn commit_prepared(&mut self) -> std::result::Result<(), HostStepError> {
        self.0
            .lock()
            .map_err(|_| HostStepError::Commit)?
            .calls
            .push("commit");
        Ok(())
    }
    fn stop_owned(&mut self) -> std::result::Result<(), HostStepError> {
        let mut facts = self.0.lock().map_err(|_| HostStepError::Stop)?;
        facts.calls.push("stop");
        facts.observed = empty();
        Ok(())
    }
    fn discard_prepared(&mut self) -> std::result::Result<(), HostStepError> {
        self.0
            .lock()
            .map_err(|_| HostStepError::Cleanup)?
            .calls
            .push("discard");
        Ok(())
    }
    fn fresh_observation(
        &mut self,
        _: &DesiredState,
    ) -> std::result::Result<crate::lifecycle::NativeLocalObservation, HostStepError> {
        let facts = self.0.lock().map_err(|_| HostStepError::Observation)?;
        let observed = facts.observed;
        Ok(crate::lifecycle::NativeLocalObservation {
            owned_core_running: observed.core_count > 0,
            visible_mihomo_count: observed.core_count,
            owned_auxiliary_mihomo_count: 0,
            visible_tun_count: observed.tun_count,
            managed_tun_count: observed.tun_count,
            owned_controller_config_verified: observed.controller_ready,
            desired_profile_matches_owned: observed.active_profile_matches,
        })
    }
}
impl ResumeBinding for Host {
    fn binding_safe_for(&mut self, desired: &DesiredState) -> bool {
        self.0
            .lock()
            .is_ok_and(|facts| facts.safe && desired == &facts.target)
    }
}

pub(crate) struct Fixture {
    base: PathBuf,
    paths: RuntimePaths,
    desired: DesiredPaths,
    uid: u32,
    facts: Arc<Mutex<Facts>>,
    producer: UnixStream,
    server: Option<RuntimeServer>,
    runtime: Option<thread::JoinHandle<Result<()>>>,
    stop: Arc<AtomicBool>,
    instance: String,
    sequence: u64,
    joined: bool,
}
#[derive(Clone, Copy)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "fixed no-argument feature selects HealthyOn; other selectors belong to compiled fault tests"
    )
)]
enum Initial {
    HealthyOn,
    EmptyOn,
    MixedOn,
    SettledOff,
    ResidualOff,
    PointerMismatch,
    HealthyThenEmpty,
}
struct Refusal {
    base: PathBuf,
    facts: Arc<Mutex<Facts>>,
    code: RuntimeError,
}
impl Drop for Refusal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
impl Fixture {
    fn new(clock: Clock) -> Self {
        Self::with_transport(
            clock,
            false,
            subscription_transport::HttpsSubscriptionTransport::new(),
        )
    }
    fn with_transport<T: NativeSubscriptionTransport + 'static>(
        clock: Clock,
        subscription: bool,
        transport: T,
    ) -> Self {
        match Self::construct(clock, subscription, transport, Initial::HealthyOn) {
            Ok(fixture) => fixture,
            Err(refusal) => {
                let _observed = refusal.facts.lock().is_ok();
                match refusal.code {
                    RuntimeError::NativeOwnerUnavailable => {
                        panic!("fixed owned fixture construction refused")
                    }
                    _ => panic!("fixed owned fixture construction unavailable"),
                }
            }
        }
    }
    fn construct<T: NativeSubscriptionTransport + 'static>(
        clock: Clock,
        subscription: bool,
        transport: T,
        initial: Initial,
    ) -> std::result::Result<Self, Refusal> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = env::temp_dir().join(format!(
            "mn-net-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        for child in ["runtime", "state", "config"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(base.join(child))
                .unwrap();
        }
        let uid = Uid::current().as_raw();
        let cutover = CutoverPaths::below(&base.join("runtime"), &base.join("state"), uid);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&cutover.state_directory)
            .unwrap();
        atomic_replace_private(
            &cutover.ownership_marker,
            br#"{"schemaVersion":1,"generation":7,"phase":"rust"}"#,
            uid,
        )
        .unwrap();
        let desired = DesiredPaths::below(&base.join("state"));
        let connected = !matches!(initial, Initial::SettledOff | Initial::ResidualOff);
        let target = DesiredState {
            connected,
            profile_id: if connected {
                PROFILE.into()
            } else {
                String::new()
            },
            mode: RoutingMode::Rule,
            generation: 12,
            ..DesiredState::default()
        };
        write_desired(&desired, uid, &target).unwrap();
        let store = base.join("config/profiles.json");
        let mut document = json!({"version":3,"activeId":PROFILE,"lastId":PROFILE,
            "profiles":[{"id":PROFILE,"name":"Fixture","protocol":"vless","favorite":false,
                "uri":SELECTED_URI}],
            "subscriptions":[],"routingPreset":"custom","customRules":[],"rulesUpdatedAt":0,
            "startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":true});
        if !connected || matches!(initial, Initial::PointerMismatch) {
            document["activeId"] = json!("");
        }
        if subscription {
            let id = "10000000-0000-4000-8000-000000000001";
            document["profiles"][0]["subscriptionId"] = json!(id);
            document["profiles"][0]["subscriptionKey"] = json!(
                omavless_profile::canonical::parse_canonical(SELECTED_URI)
                    .unwrap()
                    .subscription_identity()
            );
            document["subscriptions"] = json!([{"id":id,"name":"Fixture source","url":"https://example.invalid/fixed-feed","updatedAt":1}]);
        }
        let raw = serde_json::to_vec(&document).unwrap();
        atomic_replace_private(&store, &raw, uid).unwrap();
        let fence = Fence {
            boot: BOOT,
            owner_instance: INSTANCE,
            owner_generation: 7,
            desired_revision: 0,
            network_epoch: 1,
        };
        atomic_replace_private(
            &desired.directory.join("network-resume-receipt.json"),
            &serde_json::to_vec(&Receipt {
                schema: 1,
                fence,
                phase: Phase::Ready,
            })
            .unwrap(),
            uid,
        )
        .unwrap();
        // Fixture metadata only: there is NO preliminary coordinator/owner.
        let context = Context {
            fence,
            desired: target.clone(),
            store_digest: Sha256::digest(&raw).into(),
        };
        let (reader, producer) = UnixStream::pair().unwrap();
        let source = Source::owned(reader, std::process::id(), uid, &context).unwrap();
        let facts = Arc::new(Mutex::new(Facts {
            observed: match initial {
                Initial::EmptyOn | Initial::SettledOff => empty(),
                Initial::MixedOn => OwnedObservation {
                    core_count: 2,
                    ..healthy()
                },
                _ => healthy(),
            },
            target,
            calls: Vec::new(),
            safe: true,
            observations: 0,
            #[cfg(test)]
            observe_change: matches!(initial, Initial::HealthyThenEmpty).then_some((2, empty())),
            #[cfg(test)]
            panic_at: None,
            #[cfg(test)]
            first_connect: false,
            #[cfg(test)]
            connect_event: None,
        }));
        let paths = RuntimePaths::below(&base.join("runtime"));
        let server = RuntimeServer::bind_network_fixture(
            paths.clone(),
            Host(Arc::clone(&facts)),
            OwnerInputs {
                desired: desired.clone(),
                store,
                cutover,
                enrollment: Enrollment {
                    boot: BOOT,
                    instance: INSTANCE,
                    epoch: 1,
                },
            },
            Driver::new(source, clock),
            transport,
        );
        let server = match server {
            Ok(server) => server,
            Err(code) => return Err(Refusal { base, facts, code }),
        };
        let instance = server.instance_id.clone();
        Ok(Self {
            base,
            paths,
            desired,
            uid,
            facts,
            producer,
            server: Some(server),
            runtime: None,
            stop: Arc::new(AtomicBool::new(false)),
            instance,
            sequence: 0,
            joined: false,
        })
    }
    fn start(&mut self) {
        let server = self.server.take().unwrap();
        let stop = Arc::clone(&self.stop);
        self.runtime = Some(thread::spawn(move || server.serve_until(&stop)));
    }
    fn hint(&mut self, kind: &str) {
        self.sequence += 1;
        writeln!(
            self.producer,
            "{}",
            json!({"sequence":self.sequence,"kind":kind})
        )
        .unwrap();
    }
    fn state(&self) -> Value {
        call(&self.paths, METHOD, json!({"instanceId":self.instance})).unwrap()
    }
    fn wait(&self, state: &str) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        loop {
            let value = self.state();
            if value["result"]["state"] == state {
                return value;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fixed owned fixture deadline"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(runtime) = self.runtime.take() {
            self.joined = matches!(runtime.join(), Ok(Ok(())));
        } else if self.server.is_some() {
            drop(self.server.take());
            self.joined = true;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.shutdown();
        if self.joined {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
}

#[cfg(feature = "network-resume-fixture")]
pub(crate) fn run() -> Value {
    let mut fixture = Fixture::new(Clock::monotonic());
    fixture.start();
    assert_eq!(fixture.state()["result"]["state"], "observe_only");
    fixture.facts.lock().unwrap().observed = empty();
    fixture.hint("Resume");
    let result = fixture.wait("recovered");
    assert_eq!(
        fixture.facts.lock().unwrap().calls,
        ["prepare", "start", "commit"]
    );
    let intent = crate::desired::read_desired(&fixture.desired, fixture.uid).unwrap();
    assert!(intent.connected);
    fixture.shutdown();
    assert!(fixture.joined, "owned fixture runtime unavailable");
    json!({"schema":1,"scope":"owned_fixture","state":"recovered","effectCalls":1,
        "revision":result["revision"],"intentPreserved":intent.connected})
}

#[cfg(test)]
pub(crate) mod connect_enrollment;
#[cfg(test)]
mod os_event_enrollment;
#[cfg(test)]
mod tests;
