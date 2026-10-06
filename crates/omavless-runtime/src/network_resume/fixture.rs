// SPDX-License-Identifier: MIT
use super::*;
use crate::cutover::CutoverPaths;
#[cfg(test)]
use crate::cutover::MigrationLock;
use crate::desired::{DesiredPaths, OwnedObservation, RoutingMode, read_desired, write_desired};
use crate::lifecycle::{HostStepError, LifecycleHost};
use crate::native_coordinator::{OfflineNativeCoordinator, network_resume::ResumeBinding};
use omavless_store::atomic_replace_private;
use serde_json::json;
use std::fs::{self, DirBuilder};
use std::io::Write;
use std::os::unix::fs::DirBuilderExt;
#[cfg(test)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const PROFILE: &str = "11111111-1111-4111-8111-111111111111";
const BOOT: [u8; 16] = [1; 16];
const INSTANCE: [u8; 16] = [2; 16];

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

struct Host {
    observed: OwnedObservation,
    unavailable: bool,
    safe: bool,
    binding_target: DesiredState,
    observations: usize,
    bindings: usize,
    changes: Vec<&'static str>,
    fail_start: bool,
    fail_stop: bool,
    change_after_observation: Option<(usize, DesiredPaths, u32, DesiredState)>,
    #[cfg(test)]
    change_observation_at: Option<(usize, OwnedObservation)>,
    #[cfg(test)]
    panic_observation_at: Option<usize>,
}
impl LifecycleHost for Host {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.observations += 1;
        #[cfg(test)]
        if self.panic_observation_at == Some(self.observations) {
            panic!("fixed fixture observation lost");
        }
        #[cfg(test)]
        if let Some((at, observed)) = self.change_observation_at
            && at == self.observations
        {
            self.observed = observed;
        }
        if let Some((after, path, uid, target)) = &self.change_after_observation
            && self.observations == *after
        {
            write_desired(path, *uid, target).unwrap();
        }
        if self.unavailable {
            Err(HostStepError::Observation)
        } else {
            Ok(self.observed)
        }
    }
    fn prepare(&mut self, desired: &DesiredState) -> Result<(), HostStepError> {
        assert!(desired == &self.binding_target);
        self.changes.push("prepare");
        Ok(())
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        self.changes.push("start");
        if self.fail_start {
            return Err(HostStepError::Start);
        }
        self.observed = healthy();
        Ok(())
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        self.changes.push("commit");
        Ok(())
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        self.changes.push("stop");
        if self.fail_stop {
            return Err(HostStepError::Stop);
        }
        self.observed = empty();
        Ok(())
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        self.changes.push("discard");
        Ok(())
    }
}
impl ResumeBinding for Host {
    fn binding_safe_for(&mut self, desired: &DesiredState) -> bool {
        self.bindings += 1;
        self.safe && desired == &self.binding_target
    }
}

struct Fixture {
    root: PathBuf,
    receipt: PathBuf,
    desired: DesiredPaths,
    #[cfg(test)]
    cutover: CutoverPaths,
    uid: u32,
    coordinator: OfflineNativeCoordinator<Host>,
    context: Context,
    source: Source,
    writer: UnixStream,
    sequence: u64,
}
impl Fixture {
    fn new() -> Self {
        Self::with_intent(true)
    }
    fn with_intent(connected: bool) -> Self {
        Self::with_guard(connected, true)
    }
    fn with_guard(connected: bool, guard: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ov-network-resume-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        let cutover = CutoverPaths::below(&root, &root, uid);
        DirBuilder::new()
            .mode(0o700)
            .create(&cutover.state_directory)
            .unwrap();
        atomic_replace_private(
            &cutover.ownership_marker,
            br#"{"schemaVersion":1,"generation":7,"phase":"rust"}"#,
            uid,
        )
        .unwrap();
        let desired = DesiredPaths::below(&root);
        let target = DesiredState {
            connected,
            generation: 12,
            profile_id: if connected {
                PROFILE.to_owned()
            } else {
                String::new()
            },
            mode: RoutingMode::Rule,
            ..DesiredState::default()
        };
        write_desired(&desired, uid, &target).unwrap();
        let store_path = root.join("profiles.json");
        atomic_replace_private(&store_path, &serde_json::to_vec(&json!({
            "version":3,"activeId":"","lastId":PROFILE,
            "profiles":[{"id":PROFILE,"name":"Fixture","protocol":"vless","favorite":false,
                "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Fixture"}],
            "subscriptions":[],"routingPreset":"custom","customRules":[],"rulesUpdatedAt":0,
            "startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},
            "onboardingComplete":true
        })).unwrap(), uid).unwrap();
        let mut coordinator = OfflineNativeCoordinator::new_ownership_gated(
            Host {
                observed: empty(),
                unavailable: false,
                safe: true,
                binding_target: target,
                observations: 0,
                bindings: 0,
                changes: Vec::new(),
                fail_start: false,
                fail_stop: false,
                change_after_observation: None,
                #[cfg(test)]
                change_observation_at: None,
                #[cfg(test)]
                panic_observation_at: None,
            },
            desired.clone(),
            &store_path,
            cutover.clone(),
            uid,
            7,
        );
        let context = coordinator.resume_context(BOOT, INSTANCE, 5).unwrap();
        let receipt = desired.directory.join("network-resume-receipt.json");
        atomic_replace_private(
            &receipt,
            &serde_json::to_vec(&Receipt {
                schema: 1,
                fence: context.fence,
                phase: Phase::Ready,
            })
            .unwrap(),
            uid,
        )
        .unwrap();
        if guard {
            coordinator
                .install_resume_barrier(BOOT, INSTANCE, 5, 0)
                .unwrap();
        }
        let (reader, writer) = UnixStream::pair().unwrap();
        let source = Source::owned(reader, std::process::id(), uid, &context).unwrap();
        Self {
            root,
            receipt,
            desired,
            #[cfg(test)]
            cutover,
            uid,
            coordinator,
            context,
            source,
            writer,
            sequence: 0,
        }
    }
    fn send(&mut self, kind: Kind, tick: u64) {
        self.sequence += 1;
        self.send_sequence(kind, tick, self.sequence);
    }
    fn send_sequence(&mut self, kind: Kind, tick: u64, sequence: u64) {
        serde_json::to_writer(&mut self.writer, &Frame { sequence, kind }).unwrap();
        self.writer.write_all(b"\n").unwrap();
        self.receive(tick);
    }
    fn receive(&mut self, tick: u64) {
        self.coordinator.resume_receive(&mut self.source, tick);
    }
    #[cfg(test)]
    fn status(&self) -> Status {
        self.coordinator.resume_status()
    }
    fn projection(&self) -> serde_json::Value {
        self.coordinator.resume_projection()
    }
    fn startup(&mut self, tick: u64) -> usize {
        self.coordinator
            .resume_startup(&mut self.source, tick, false, false, None)
    }
    #[cfg(test)]
    fn restart(&mut self, instance: [u8; 16]) {
        let target = read_desired(&self.desired, self.uid).unwrap();
        self.coordinator = OfflineNativeCoordinator::new_ownership_gated(
            Host {
                observed: empty(),
                unavailable: false,
                safe: true,
                binding_target: target,
                observations: 0,
                bindings: 0,
                changes: Vec::new(),
                fail_start: false,
                fail_stop: false,
                change_after_observation: None,
                change_observation_at: None,
                panic_observation_at: None,
            },
            self.desired.clone(),
            &self.root.join("profiles.json"),
            self.cutover.clone(),
            self.uid,
            7,
        );
        self.context = self.coordinator.resume_context(BOOT, instance, 5).unwrap();
        self.coordinator
            .install_resume_barrier(BOOT, instance, 5, 0)
            .unwrap();
    }
    fn poll(&mut self, tick: u64, fail_before: bool, fail_after: bool) -> usize {
        self.poll_fault(tick, fail_before, fail_after, None)
    }
    fn poll_fault(
        &mut self,
        tick: u64,
        fail_before: bool,
        fail_after: bool,
        fault_phase: Option<Phase>,
    ) -> usize {
        self.coordinator
            .resume_poll(&mut self.source, tick, fail_before, fail_after, fault_phase)
    }
    fn phase(&self) -> Phase {
        decode(fs::read(&self.receipt).unwrap().as_slice())
            .unwrap()
            .phase
    }
    fn unchanged(&self) {
        assert!(self.coordinator.host().changes.is_empty());
        assert!(read_desired(&self.desired, self.uid).unwrap() == self.context.desired);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[cfg(feature = "network-resume-fixture")]
pub(super) fn run_developer_fixture() -> serde_json::Value {
    let mut fixture = Fixture::new();
    fixture.send(Kind::Resume, 100);
    fixture.send(Kind::NetworkChanged, 101);
    fixture.send(Kind::NetworkChanged, 102);
    assert_eq!(fixture.poll(104, false, false), 0);
    fixture.unchanged();
    let effects = fixture.poll(105, false, false);
    assert_eq!(effects, 1);
    assert_eq!(fixture.phase(), Phase::Finished);
    assert_eq!(
        fixture.coordinator.host().changes,
        ["prepare", "start", "commit"]
    );
    assert!(read_desired(&fixture.desired, fixture.uid).unwrap() == fixture.context.desired);
    fixture.send(Kind::Resume, 106);
    assert_eq!(fixture.poll(109, false, false), 0);
    assert_eq!(fixture.startup(110), 0);
    let mut report = fixture.projection();
    report["scope"] = json!("owned_fixture");
    report["effectCalls"] = json!(effects);
    report["revision"] = json!(fixture.coordinator.revision());
    report["intentPreserved"] = json!(true);
    report
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
