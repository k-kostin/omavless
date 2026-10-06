// SPDX-License-Identifier: MIT
use super::*;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::desired::{DesiredPaths, OwnedObservation, RoutingMode, read_desired, write_desired};
use crate::lifecycle::{HostStepError, LifecycleHost};
use crate::native_coordinator::{
    OfflineNativeCoordinator,
    network_resume::{Port, ResumeBinding},
};
use omavless_store::atomic_replace_private;
use serde_json::json;
use std::fs::{self, DirBuilder};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
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
}
impl LifecycleHost for Host {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.observations += 1;
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
    cutover: CutoverPaths,
    uid: u32,
    coordinator: OfflineNativeCoordinator<Host>,
    context: Context,
    owner: EventOwner,
    source: Source,
    writer: UnixStream,
    sequence: u64,
}
impl Fixture {
    fn new() -> Self {
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
            connected: true,
            generation: 12,
            profile_id: PROFILE.to_owned(),
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
        let coordinator = OfflineNativeCoordinator::new_ownership_gated(
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
            },
            desired.clone(),
            &store_path,
            cutover.clone(),
            uid,
            7,
        );
        let context = coordinator.resume_context(BOOT, INSTANCE, 5).unwrap();
        let receipt = root.join("receipt.json");
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
        let owner = EventOwner::new(&context).unwrap();
        let (reader, writer) = UnixStream::pair().unwrap();
        let source = Source::owned(reader, std::process::id(), uid, &context).unwrap();
        Self {
            root,
            receipt,
            desired,
            cutover,
            uid,
            coordinator,
            context,
            owner,
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
        self.owner.receive(&mut self.source, &self.context, tick);
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
        let lease = match self.coordinator.resume_lease() {
            Ok(lease) => lease,
            Err(_) => {
                self.owner.pending = None;
                self.owner.status = Status::ManualRecovery;
                return 0;
            }
        };
        let mut journal = Files {
            receipt: &self.receipt,
            lease: &lease,
            paths: &self.cutover,
            uid: self.uid,
            fail_before,
            fail_after,
            fault_phase,
        };
        let mut port = Port::new(&mut self.coordinator, &lease, &self.context, tick);
        self.owner
            .poll(&mut self.source, tick, &mut journal, &mut port);
        port.effect_calls
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

#[test]
fn authenticated_source_runs_original_coordinator_once_after_a_quiet_burst() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 100);
    f.send(Kind::NetworkChanged, 101);
    f.send(Kind::NetworkChanged, 102);
    assert_eq!(f.poll(104, false, false), 0);
    f.unchanged();
    assert_eq!(f.coordinator.host().observations, 0);
    assert_eq!(f.poll(105, false, false), 1);
    assert_eq!(f.owner.status, Status::Recovered);
    assert_eq!(f.phase(), Phase::Finished);
    assert_eq!(f.coordinator.host().changes, ["prepare", "start", "commit"]);
    assert_eq!(f.coordinator.revision(), 1);
    assert!(read_desired(&f.desired, f.uid).unwrap() == f.context.desired);
    let pointers: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root.join("profiles.json")).unwrap()).unwrap();
    assert_eq!(pointers["activeId"], PROFILE);
    assert_eq!(pointers["lastId"], PROFILE);
    f.send(Kind::Resume, 106);
    assert_eq!(f.poll(109, false, false), 0);
    assert_eq!(f.coordinator.host().changes.len(), 3);
}

#[test]
fn pause_ignores_link_notifications_until_resume_without_disconnect() {
    let mut f = Fixture::new();
    f.send(Kind::NetworkChanged, 10);
    f.send(Kind::Suspend, 11);
    f.send(Kind::NetworkChanged, 12);
    assert_eq!(f.poll(15, false, false), 0);
    assert_eq!(f.owner.status, Status::Paused);
    f.unchanged();
    f.send(Kind::Resume, 20);
    assert_eq!(f.poll(23, false, false), 1);
}

#[test]
fn healthy_never_restarts_and_incomplete_foreign_or_unsafe_facts_never_connect() {
    for case in 0..7 {
        let mut f = Fixture::new();
        match case {
            0 => f.coordinator.host_mut().observed = healthy(),
            1 => f.coordinator.host_mut().unavailable = true,
            2 => f.coordinator.host_mut().observed.core_count = 1,
            3 => f.coordinator.host_mut().observed.tun_count = 1,
            4 => f.coordinator.host_mut().safe = false,
            5 => {
                f.coordinator.host_mut().observed = OwnedObservation {
                    core_count: 2,
                    ..healthy()
                }
            }
            _ => {
                f.coordinator.host_mut().observed = OwnedObservation {
                    active_profile_matches: false,
                    ..healthy()
                }
            }
        }
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll(13, false, false), 0);
        f.unchanged();
        assert_eq!(f.phase(), Phase::Ready);
        assert_eq!(
            f.owner.status,
            if case == 0 {
                Status::ObserveOnly
            } else {
                Status::ManualRecovery
            }
        );
    }
}

#[test]
fn every_desired_change_and_off_cancel_before_any_host_effect() {
    for case in 0..5 {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        let mut changed = f.context.desired.clone();
        match case {
            0 => {
                changed.connected = false;
                changed.profile_id.clear();
            } // ManualOff/Disconnect/Quit intent
            1 => changed.mode = RoutingMode::Global,
            2 => changed.profile_id = "22222222-2222-4222-8222-222222222222".into(),
            3 => changed.generation += 1,
            _ => {
                let mut marker = fs::read(&f.cutover.ownership_marker).unwrap();
                marker = String::from_utf8(marker)
                    .unwrap()
                    .replace("\"generation\":7", "\"generation\":8")
                    .into_bytes();
                atomic_replace_private(&f.cutover.ownership_marker, &marker, f.uid).unwrap();
            }
        }
        if case != 4 {
            write_desired(&f.desired, f.uid, &changed).unwrap();
        }
        assert_eq!(f.poll(13, false, false), 0);
        assert!(f.coordinator.host().changes.is_empty());
        assert_eq!(f.coordinator.host().observations, 0);
        assert_eq!(f.phase(), Phase::Ready);
    }
    let mut f = Fixture::new();
    let mut off = f.context.desired.clone();
    off.connected = false;
    off.profile_id.clear();
    write_desired(&f.desired, f.uid, &off).unwrap();
    f.context = f.coordinator.resume_context(BOOT, INSTANCE, 5).unwrap();
    f.send(Kind::Resume, 10);
    assert_eq!(f.owner.status, Status::Cancelled);
    assert_eq!(f.poll(13, false, false), 0);
    assert!(f.coordinator.host().changes.is_empty());
}

#[test]
fn lost_gapped_bad_or_wrong_boot_source_is_terminal() {
    for case in 0..7 {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        match case {
            0 => {
                f.writer.shutdown(std::net::Shutdown::Write).unwrap();
            }
            1 => {
                f.writer
                    .write_all(b"{\"sequence\":3,\"kind\":\"Resume\"}\n")
                    .unwrap();
            }
            2 => {
                f.writer
                    .write_all(b"{\"sequence\":2,\"sequence\":2,\"kind\":\"Resume\"}\n")
                    .unwrap();
            }
            3 => {
                f.writer.write_all(&[b'x'; 257]).unwrap();
            }
            4 => {
                f.source.boot = [9; 16];
            }
            5 => {
                f.source.instance = [9; 16];
            }
            _ => {} // timeout is also unavailable, never fabricated absence
        }
        f.owner.receive(&mut f.source, &f.context, 11);
        assert_eq!(f.owner.status, Status::SourceUnavailable);
        assert_eq!(f.poll(14, false, false), 0);
        f.unchanged();
        assert_eq!(f.phase(), Phase::Ready);
    }
}

#[test]
fn duplicate_reordered_events_clock_reversal_and_endless_burst_cannot_extend_permit() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    f.send_sequence(Kind::NetworkChanged, 11, 1);
    assert_eq!(f.owner.pending.as_ref().unwrap().hint.last_hint_tick, 10);
    assert_eq!(f.poll(13, false, false), 1);
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    assert_eq!(f.poll(9, false, false), 0);
    assert_eq!(f.owner.status, Status::SourceUnavailable);
    f.unchanged();
    let mut f = Fixture::new();
    for tick in 0..=61 {
        f.send(Kind::NetworkChanged, tick);
    }
    assert_eq!(f.poll(64, false, false), 0);
    assert_eq!(f.owner.status, Status::Cancelled);
    f.unchanged();
}

#[test]
fn failed_or_unacknowledged_reservation_and_failed_recovery_never_rearm() {
    for (before, after, failed_start, failed_stop) in [
        (true, false, false, false),
        (false, true, false, false),
        (false, false, true, false),
        (false, false, true, true),
    ] {
        let mut f = Fixture::new();
        f.coordinator.host_mut().fail_start = failed_start;
        f.coordinator.host_mut().fail_stop = failed_stop;
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll(13, before, after), usize::from(failed_start));
        assert_eq!(f.owner.status, Status::ManualRecovery);
        assert_eq!(
            f.phase(),
            if before {
                Phase::Ready
            } else {
                Phase::Reserved
            }
        );
        let changes = f.coordinator.host().changes.len();
        f.send(Kind::Resume, 14);
        assert_eq!(f.poll(17, false, false), 0);
        assert_eq!(f.coordinator.host().changes.len(), changes);
        // New runtime identity cannot adopt the old Ready, even when the
        // attempted reservation failed before any file was changed.
        f.context.fence.owner_instance = [3; 16];
        f.owner = EventOwner::new(&f.context).unwrap();
        let (reader, writer) = UnixStream::pair().unwrap();
        f.writer = writer;
        f.sequence = 0;
        f.source = Source::owned(reader, std::process::id(), f.uid, &f.context).unwrap();
        f.send(Kind::Resume, 20);
        assert_eq!(f.poll(23, false, false), 0);
        assert_eq!(f.coordinator.host().changes.len(), changes);
    }
}

#[test]
fn intent_race_after_reservation_burns_receipt_with_zero_recovery() {
    let mut f = Fixture::new();
    let mut off = f.context.desired.clone();
    off.connected = false;
    off.profile_id.clear();
    // Third observation is post-reservation revalidation. The following effect
    // boundary rereads exact desired state under the same original lease.
    f.coordinator.host_mut().change_after_observation = Some((3, f.desired.clone(), f.uid, off));
    f.send(Kind::Resume, 10);
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.owner.status, Status::ManualRecovery);
    assert_eq!(f.phase(), Phase::Reserved);
    assert!(f.coordinator.host().changes.is_empty());
}

#[test]
fn fixture_peer_credentials_and_lost_or_unsafe_receipts_fail_closed() {
    let f = Fixture::new();
    let (reader, _) = UnixStream::pair().unwrap();
    assert!(Source::owned(reader, std::process::id() + 1, f.uid, &f.context).is_err());
    for case in 0..5 {
        let mut f = Fixture::new();
        match case {
            0 => fs::remove_file(&f.receipt).unwrap(),
            1 => atomic_replace_private(&f.receipt, b"{}", f.uid).unwrap(),
            2 => fs::set_permissions(&f.receipt, fs::Permissions::from_mode(0o644)).unwrap(),
            3 => {
                fs::remove_file(&f.receipt).unwrap();
                std::os::unix::fs::symlink(f.root.join("profiles.json"), &f.receipt).unwrap();
            }
            _ => {
                let receipt = Receipt {
                    schema: 1,
                    phase: Phase::Reserved,
                    fence: f.context.fence,
                };
                atomic_replace_private(&f.receipt, &serde_json::to_vec(&receipt).unwrap(), f.uid)
                    .unwrap();
            }
        }
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll(13, false, false), 0);
        f.unchanged();
        assert_eq!(f.owner.status, Status::ManualRecovery);
    }
}

#[test]
fn original_mutation_lease_contention_never_turns_into_a_queued_connect() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    let lease = MigrationLock::acquire(&f.cutover, f.uid).unwrap();
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.owner.status, Status::ManualRecovery);
    drop(lease);
    f.send(Kind::Resume, 14);
    assert_eq!(f.poll(17, false, false), 0);
    f.unchanged();
}

#[test]
fn disconnect_through_original_owner_wins_and_receipt_cannot_reconnect() {
    use crate::mutation::MutationDigest;
    use crate::owner::{OwnerAction, OwnerRequest};
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    let result = f
        .coordinator
        .execute_connection(OwnerRequest::new(
            OwnerAction::Disconnect,
            Some("fixture-disconnect"),
            Some(0),
            MutationDigest::new([7; 32]),
        ))
        .unwrap();
    assert!(matches!(
        result,
        crate::native_coordinator::NativeOwnerExecution::Applied { outcome: Ok(_), .. }
    ));
    assert_eq!(f.coordinator.revision(), 1);
    let changes = f.coordinator.host().changes.clone();
    assert_eq!(f.poll(13, false, false), 0);
    assert_eq!(f.coordinator.host().changes, changes);
    assert!(!read_desired(&f.desired, f.uid).unwrap().connected);
    assert_eq!(f.phase(), Phase::Ready);
}

#[test]
fn changed_profile_store_content_or_deleted_target_cancels_without_observation() {
    for delete in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        let path = f.root.join("profiles.json");
        let mut store: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if delete {
            store["profiles"] = json!([]);
            store["lastId"] = json!("");
        } else {
            store["profiles"][0]["name"] = json!("Changed");
        }
        atomic_replace_private(&path, &serde_json::to_vec(&store).unwrap(), f.uid).unwrap();
        assert_eq!(f.poll(13, false, false), 0);
        assert!(f.coordinator.host().changes.is_empty());
        assert_eq!(f.coordinator.host().observations, 0);
    }
}

#[test]
fn source_loss_or_unprocessed_new_event_at_due_tick_blocks_old_event() {
    for queued in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        if queued {
            f.writer
                .write_all(b"{\"sequence\":2,\"kind\":\"Suspend\"}\n")
                .unwrap();
        } else {
            f.writer.shutdown(std::net::Shutdown::Write).unwrap();
        }
        assert_eq!(f.poll(13, false, false), 0);
        assert_eq!(f.owner.status, Status::SourceUnavailable);
        f.unchanged();
        assert_eq!(f.coordinator.host().observations, 0);
    }
}

#[test]
fn status_projection_is_coarse_and_contains_no_identity_or_target() {
    let mut f = Fixture::new();
    f.send(Kind::Resume, 10);
    assert_eq!(
        f.owner.status_projection(),
        json!({"schema":1,"state":"checking"})
    );
    assert_eq!(f.poll(13, false, false), 1);
    assert_eq!(
        f.owner.status_projection(),
        json!({"schema":1,"state":"recovered"})
    );
}

#[test]
fn completion_publication_failure_never_repeats_a_completed_coordinator_operation() {
    for after in [false, true] {
        let mut f = Fixture::new();
        f.send(Kind::Resume, 10);
        assert_eq!(f.poll_fault(13, !after, after, Some(Phase::Finished)), 1);
        assert_eq!(f.owner.status, Status::ManualRecovery);
        assert_eq!(
            f.phase(),
            if after {
                Phase::Finished
            } else {
                Phase::Reserved
            }
        );
        assert_eq!(f.coordinator.host().changes, ["prepare", "start", "commit"]);
        assert_eq!(f.coordinator.revision(), 1);
        f.send(Kind::Resume, 14);
        assert_eq!(f.poll(17, false, false), 0);
        assert_eq!(f.coordinator.host().changes.len(), 3);
    }
}
