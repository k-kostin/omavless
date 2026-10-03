// SPDX-License-Identifier: MIT
//! Actual coordinator/lifecycle/private-writer research with private fixtures.
use super::epoch_tests::{desired_paths, proof, receipt, witness};
use super::tests::{ordinary_edit, prepared};
use super::*;
use crate::desired::{DesiredState, OwnedObservation, read_desired_snapshot};
use crate::lifecycle::{ActualState, HostStepError, LifecycleHost};
use crate::native_coordinator::{
    NativeMutationOutcome, NativeOwnerExecution, OfflineNativeCoordinator,
};
use crate::owner::OwnerRequest;
use crate::restore_successor_publication_candidate::tests::Fixture;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[derive(Default)]
struct Host {
    running: bool,
    prepared: Option<DesiredState>,
    calls: Vec<&'static str>,
    fail_start: usize,
    fail_stop: bool,
    fail_preflight: bool,
    hook: Option<HostHook>,
}
type HostHook = Box<dyn FnMut(&str)>;

/// Real parent-owned core in a deliberately no-TUN fixture. It cannot report a
/// healthy connected tunnel; its purpose is actual failed-Connect compensation.
struct NoTunCoreHost {
    binary: std::path::PathBuf,
    directory: std::path::PathBuf,
    socket: std::path::PathBuf,
    core: Option<crate::core::OwnedCore>,
    started: bool,
    stopped: bool,
}
impl LifecycleHost for NoTunCoreHost {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        let running = match self.core.as_mut() {
            Some(core) => core.running().map_err(|_| HostStepError::Observation)?,
            None => false,
        };
        Ok(OwnedObservation {
            service_active: running,
            controller_ready: running,
            core_count: u8::from(running),
            tun_count: 0,
            active_profile_matches: running,
        })
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        let config = format!(
            "mode: direct\nport: 0\nsocks-port: 0\nmixed-port: 0\nredir-port: 0\ntproxy-port: 0\nallow-lan: false\nlog-level: silent\nexternal-controller-unix: {}\ntun:\n  enable: false\n  auto-route: false\ndns:\n  enable: false\nproxies: []\nproxy-groups: []\nrules: []\n",
            serde_json::to_string(self.socket.to_str().unwrap()).unwrap()
        );
        let path = self.directory.join("core.yaml");
        fs::write(&path, config).map_err(|_| HostStepError::Prepare)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| HostStepError::Prepare)
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        let core = crate::core::OwnedCore::spawn(
            &self.binary,
            &self.directory,
            &self.directory.join("core.yaml"),
            &self.socket,
        )
        .map_err(|_| HostStepError::Start)?;
        self.core = Some(core);
        self.core
            .as_mut()
            .unwrap()
            .wait_ready(std::time::Duration::from_secs(5))
            .map_err(|_| HostStepError::Start)?;
        self.started = true;
        Ok(())
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("no-TUN fixture cannot commit healthy connection")
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        if let Some(core) = self.core.as_mut() {
            core.stop(std::time::Duration::from_secs(5))
                .map_err(|_| HostStepError::Stop)?;
            self.core = None;
            self.stopped = true;
        }
        Ok(())
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        for name in ["core.yaml", "c.sock"] {
            match fs::remove_file(self.directory.join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(HostStepError::Cleanup),
            }
        }
        Ok(())
    }
}
impl Host {
    fn step(&mut self, step: &'static str) {
        self.calls.push(step);
        if let Some(hook) = self.hook.as_mut() {
            hook(step);
        }
    }
}
impl LifecycleHost for Host {
    fn connection_preflight(&mut self) -> Result<(), HostStepError> {
        self.step("preflight");
        if self.fail_preflight {
            Err(HostStepError::Prepare)
        } else {
            Ok(())
        }
    }
    fn observe(&mut self, desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.step("observe");
        Ok(OwnedObservation {
            service_active: self.running,
            controller_ready: self.running,
            core_count: u8::from(self.running),
            tun_count: u8::from(self.running),
            active_profile_matches: self.running
                && self
                    .prepared
                    .as_ref()
                    .is_some_and(|p| p.profile_id == desired.profile_id && p.mode == desired.mode),
        })
    }
    fn prepare(&mut self, desired: &DesiredState) -> Result<(), HostStepError> {
        self.step("prepare");
        self.prepared = Some(desired.clone());
        Ok(())
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        self.step("start");
        if self.fail_start > 0 {
            self.fail_start -= 1;
            Err(HostStepError::Start)
        } else {
            self.running = true;
            Ok(())
        }
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        self.step("commit");
        Ok(())
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        self.step("stop");
        if self.fail_stop {
            Err(HostStepError::Stop)
        } else {
            self.running = false;
            Ok(())
        }
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        self.step("discard");
        self.prepared = None;
        Ok(())
    }
}
fn owner(f: &Fixture, host: Host) -> OfflineNativeCoordinator<Host> {
    OfflineNativeCoordinator::new_ownership_gated(
        host,
        desired_paths(f),
        &f.config.join(LIVE[0]),
        f.paths.clone(),
        f.uid,
        2,
    )
}
fn request(f: &Fixture, connected: bool, id: &str, revision: u64) -> OwnerRequest {
    let params = if connected {
        let store: serde_json::Value =
            serde_json::from_slice(&fs::read(f.config.join(LIVE[0])).unwrap()).unwrap();
        serde_json::json!({"profileId":store["profiles"][0]["id"],"operationId":id,"expectedRevision":revision})
    } else {
        serde_json::json!({"operationId":id,"expectedRevision":revision})
    };
    crate::mutation_protocol::parse_owner_request(&serde_json::json!({"api":"omavless.control","version":1,"id":"research","method":if connected{"connection.connect"}else{"connection.disconnect"},"params":params})).unwrap()
}
fn initialize(f: &Fixture) {
    ordinary_edit(f);
    receipt(f);
}
fn context<'a>(f: &'a Fixture, lock: &'a MigrationLock) -> HistoricalConnection<'a> {
    witness(f, lock)
        .research(proof(f, lock), || true)
        .unwrap()
        .into_connection()
        .unwrap()
}
fn success(value: NativeOwnerExecution) -> crate::mutation::CachedOutcome {
    match value {
        NativeOwnerExecution::Applied {
            cached,
            outcome: Ok(NativeMutationOutcome::Connection(_)),
        } => cached,
        _ => panic!("expected real connection transaction success"),
    }
}
fn failed(
    value: NativeOwnerExecution,
) -> crate::connection_transaction::ConnectionTransactionError {
    match value {
        NativeOwnerExecution::Applied {
            outcome: Err(crate::native_coordinator::NativeTransactionError::Connection(error)),
            ..
        } => error,
        _ => panic!("expected real connection failure"),
    }
}

#[test]
fn historical_connection_foreign_owner_refusal_preserves_original_replay_and_disconnect_abort() {
    connection_foreign_owner_replay_and_disconnect(false);
}

#[test]
fn historical_connection_foreign_owner_refusal_preserves_original_replay_and_disconnect_commit() {
    connection_foreign_owner_replay_and_disconnect(true);
}

fn connection_foreign_owner_replay_and_disconnect(commit: bool) {
    {
        let (f, lock) = prepared(commit);
        initialize(&f);
        let mut c = context(&f, &lock);
        let mut a = owner(&f, Host::default());
        let mut b = owner(&f, Host::default()); // Same paths, distinct actual owner.
        let connected = success(
            a.execute_connection_research(request(&f, true, "original-connect", 0), &mut c)
                .unwrap(),
        );
        let calls = a.host().calls.len();
        let receiver_actual = b.actual();
        let before = Snapshot::read_policy(
            &f.config,
            &f.paths,
            f.uid,
            2,
            &lock,
            LivePolicy::ValidCurrentBundled,
        )
        .unwrap();
        assert!(
            b.execute_connection_research(request(&f, false, "misrouted-disconnect", 0), &mut c,)
                .is_err()
        );
        assert_eq!(b.revision(), 0);
        assert_eq!(b.actual(), receiver_actual);
        assert!(b.host().calls.is_empty());
        assert_eq!(a.revision(), 1);
        assert_eq!(a.actual(), ActualState::Connected);
        assert_eq!(a.host().calls.len(), calls);
        assert!(
            before.same(
                &Snapshot::read_policy(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    LivePolicy::ValidCurrentBundled,
                )
                .unwrap()
            )
        );
        // The context is not recaptured or replaced after B's refusal.
        assert_eq!(
            a.execute_connection_research(request(&f, true, "original-connect", 0), &mut c,)
                .unwrap(),
            NativeOwnerExecution::Replay(connected),
        );
        assert!(!c.poisoned());
        assert_eq!(a.host().calls.len(), calls);
        assert!(
            before.same(
                &Snapshot::read_policy(
                    &f.config,
                    &f.paths,
                    f.uid,
                    2,
                    &lock,
                    LivePolicy::ValidCurrentBundled,
                )
                .unwrap()
            )
        );
        let disconnected = success(
            a.execute_connection_research(
                request(&f, false, "original-next-disconnect", 1),
                &mut c,
            )
            .unwrap(),
        );
        assert_eq!(disconnected.revision, 2);
        assert_eq!(a.actual(), ActualState::Disconnected);
        assert!(!a.host().running);
        assert!(b.host().calls.is_empty());
        assert_eq!(b.revision(), 0);
        // Refusal must not permanently latch B either. Its independently
        // captured Off context is not reused as authority for A.
        let mut own_b_context = context(&f, &lock);
        success(
            b.execute_connection_research(
                request(&f, true, "receiver-own-connect", 0),
                &mut own_b_context,
            )
            .unwrap(),
        );
        assert_eq!(b.revision(), 1);
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
    }
}

#[test]
fn historical_connection_real_connect_disconnect_replay_noop_and_history() {
    for commit in [false, true] {
        let (f, lock) = prepared(commit);
        initialize(&f);
        let mut c = context(&f, &lock);
        let mut o = owner(&f, Host::default());
        let history = c.profile.current.as_ref().unwrap().members[..3]
            .iter()
            .map(|(b, m)| (b.to_vec(), m.clone()))
            .collect::<Vec<_>>();
        let connected = success(
            o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
                .unwrap(),
        );
        assert_eq!(connected.revision, 1);
        assert_eq!(o.actual(), ActualState::Connected);
        assert!(
            read_desired_snapshot(&desired_paths(&f), f.uid)
                .unwrap()
                .connected
        );
        let store = fs::metadata(f.config.join(LIVE[0])).unwrap();
        let desired = fs::metadata(desired_paths(&f).file).unwrap();
        let calls = o.host().calls.len();
        assert_eq!(
            o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
                .unwrap(),
            NativeOwnerExecution::Replay(connected)
        );
        assert_eq!(o.host().calls.len(), calls);
        let noop = success(
            o.execute_connection_research(request(&f, true, "noop", 1), &mut c)
                .unwrap(),
        );
        assert_eq!(noop.revision, 1);
        assert!(same_member(
            &store,
            &fs::metadata(f.config.join(LIVE[0])).unwrap()
        ));
        assert!(same_member(
            &desired,
            &fs::metadata(desired_paths(&f).file).unwrap()
        ));
        let disconnected = success(
            o.execute_connection_research(request(&f, false, "disconnect", 1), &mut c)
                .unwrap(),
        );
        assert_eq!(disconnected.revision, 2);
        assert_eq!(o.actual(), ActualState::Disconnected);
        assert!(!o.host().running);
        let calls = o.host().calls.len();
        assert_eq!(
            o.execute_connection_research(request(&f, false, "disconnect", 1), &mut c)
                .unwrap(),
            NativeOwnerExecution::Replay(disconnected)
        );
        assert_eq!(calls, o.host().calls.len());
        for ((bytes, meta), (now, m)) in history
            .iter()
            .zip(&c.profile.current.as_ref().unwrap().members[..3])
        {
            assert_eq!(bytes.as_slice(), now.as_slice());
            assert!(same_member(meta, m));
        }
        assert!(crate::pending_private_transaction::pending_at(
            &f.paths.state_directory
        ));
        assert!(
            o.execute_connection(request(&f, true, "ordinary", 2))
                .is_err()
        );
    }
}

#[test]
fn historical_connection_actual_pointer_failure_compensates_and_retries() {
    let (f, lock) = prepared(true);
    initialize(&f);
    let store_path = f.config.join(LIVE[0]);
    let mut store: serde_json::Value =
        serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
    store["activeId"] = "".into();
    fs::write(&store_path, serde_json::to_vec(&store).unwrap()).unwrap();
    let before = fs::read(f.config.join(LIVE[0])).unwrap();
    let original_store_identity = fs::metadata(f.config.join(LIVE[0])).unwrap();
    let marker_path = f
        .paths
        .state_directory
        .join(crate::cutover::OWNERSHIP_MARKER_NAME);
    let original_marker = fs::read(&marker_path).unwrap();
    let marker_identity = fs::metadata(&marker_path).unwrap();
    let mut c = context(&f, &lock);
    let mut o = owner(&f, Host::default());
    let path = f.config.clone();
    c.fault = Some(Box::new(move |checkpoint| {
        if checkpoint == ConnectionCheckpoint::BeforePointer {
            for n in 0..128 {
                fs::write(
                    path.join(format!(".profiles.json.{}.{n}", std::process::id())),
                    b"occupied",
                )
                .unwrap();
            }
        }
        Ok(())
    }));
    assert_eq!(
        failed(
            o.execute_connection_research(request(&f, true, "failed", 0), &mut c)
                .unwrap()
        ),
        crate::connection_transaction::ConnectionTransactionError::TransitionFailedRestored
    );
    assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), before);
    assert!(same_member(
        &original_store_identity,
        &fs::metadata(f.config.join(LIVE[0])).unwrap()
    ));
    assert_eq!(fs::read(&marker_path).unwrap(), original_marker);
    assert!(same_member(
        &marker_identity,
        &fs::metadata(&marker_path).unwrap()
    ));
    assert!(!o.host().running);
    assert!(o.host().prepared.is_none());
    assert_eq!(o.actual(), ActualState::Disconnected);
    assert!(!c.poisoned());
    assert_eq!(o.revision(), 0);
    let desired = read_desired_snapshot(&desired_paths(&f), f.uid).unwrap();
    assert!(!desired.connected);
    assert_eq!(desired.generation, 21);
    c.fault = None;
    for n in 0..128 {
        fs::remove_file(
            f.config
                .join(format!(".profiles.json.{}.{n}", std::process::id())),
        )
        .unwrap();
    }
    success(
        o.execute_connection_research(request(&f, true, "retry", 0), &mut c)
            .unwrap(),
    );
    success(
        o.execute_connection_research(request(&f, false, "stop", 1), &mut c)
            .unwrap(),
    );
}

#[test]
fn historical_connection_lifecycle_failure_restores_off_but_unknown_cleanup_latches() {
    for fail_stop in [false, true] {
        let (f, lock) = prepared(true);
        initialize(&f);
        let original = fs::read(f.config.join(LIVE[0])).unwrap();
        let mut c = context(&f, &lock);
        let mut o = owner(
            &f,
            Host {
                fail_start: 1,
                fail_stop,
                ..Host::default()
            },
        );
        let error = failed(
            o.execute_connection_research(request(&f, true, "start-fail", 0), &mut c)
                .unwrap(),
        );
        assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), original);
        if fail_stop {
            assert_eq!(
                error,
                crate::connection_transaction::ConnectionTransactionError::ManualRecoveryRequired
            );
            assert!(
                o.execute_connection_research(request(&f, false, "retry", 0), &mut c)
                    .is_err()
            );
        } else {
            assert_eq!(
                error,
                crate::connection_transaction::ConnectionTransactionError::TransitionFailedRestored
            );
            assert!(
                !read_desired_snapshot(&desired_paths(&f), f.uid)
                    .unwrap()
                    .connected
            );
            assert!(!o.host().running);
            assert!(!c.poisoned());
        }
    }
}

#[test]
fn historical_connection_preflight_late_host_fences_refuse() {
    for step in [
        "observe",
        "preflight",
        "prepare",
        "start",
        "commit",
        "stop",
        "discard",
    ] {
        let (f, lock) = prepared(true);
        initialize(&f);
        let mut c = context(&f, &lock);
        let mut o = owner(&f, Host::default());
        if matches!(step, "stop" | "discard") {
            success(
                o.execute_connection_research(request(&f, true, "initial", 0), &mut c)
                    .unwrap(),
            );
        }
        let path = f.paths.state_directory.join("restore-successor.pending");
        o.host_mut().hook = Some(Box::new(move |seen| {
            if seen == step {
                fs::write(&path, b"late").unwrap();
            }
        }));
        let connected = !matches!(step, "stop" | "discard");
        let revision = u64::from(!connected);
        assert_eq!(
            failed(
                o.execute_connection_research(request(&f, connected, "late", revision), &mut c)
                    .unwrap()
            ),
            crate::connection_transaction::ConnectionTransactionError::ManualRecoveryRequired
        );
        assert!(c.poisoned());
        let calls = o.host().calls.len();
        assert!(
            o.execute_connection_research(request(&f, false, "later", revision), &mut c)
                .is_err()
        );
        assert_eq!(calls, o.host().calls.len());
    }
}

#[test]
fn historical_connection_after_writer_loss_and_same_byte_noop_swap_poison() {
    for checkpoint in [
        ConnectionCheckpoint::AfterDesired,
        ConnectionCheckpoint::AfterPointer,
    ] {
        let (f, lock) = prepared(true);
        initialize(&f);
        let mut c = context(&f, &lock);
        let mut o = owner(&f, Host::default());
        c.fault = Some(Box::new(
            move |seen| if seen == checkpoint { Err(()) } else { Ok(()) },
        ));
        assert_eq!(
            failed(
                o.execute_connection_research(request(&f, true, "lost", 0), &mut c)
                    .unwrap()
            ),
            crate::connection_transaction::ConnectionTransactionError::ManualRecoveryRequired
        );
        assert!(c.poisoned());
        assert_eq!(o.revision(), 0);
        assert!(
            o.execute_connection_research(request(&f, true, "lost", 0), &mut c)
                .is_err()
        );
    }
    let (f, lock) = prepared(true);
    initialize(&f);
    let mut c = context(&f, &lock);
    let mut o = owner(&f, Host::default());
    success(
        o.execute_connection_research(request(&f, true, "initial", 0), &mut c)
            .unwrap(),
    );
    let path = f.config.join(LIVE[0]);
    c.fault = Some(Box::new(move |seen| {
        if seen == ConnectionCheckpoint::AfterPointer {
            let replacement = path.with_extension("swap");
            fs::write(&replacement, fs::read(&path).unwrap()).unwrap();
            fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
            fs::rename(replacement, &path).unwrap();
        }
        Ok(())
    }));
    assert_eq!(
        failed(
            o.execute_connection_research(request(&f, true, "noop-swap", 1), &mut c)
                .unwrap()
        ),
        crate::connection_transaction::ConnectionTransactionError::ManualRecoveryRequired
    );
    assert!(c.poisoned());
    assert_eq!(o.revision(), 1);
}

#[test]
fn historical_connection_operation_collision_owner_and_dns_preflight() {
    let (f, lock) = prepared(true);
    initialize(&f);
    let mut c = context(&f, &lock);
    let mut o = owner(
        &f,
        Host {
            fail_preflight: true,
            ..Host::default()
        },
    );
    let desired = fs::metadata(desired_paths(&f).file).unwrap();
    assert_eq!(
        failed(
            o.execute_connection_research(request(&f, true, "dns", 0), &mut c)
                .unwrap()
        ),
        crate::connection_transaction::ConnectionTransactionError::DnsPairRequired
    );
    assert!(same_member(
        &desired,
        &fs::metadata(desired_paths(&f).file).unwrap()
    ));
    assert!(!o.host().calls.contains(&"prepare"));
    o.host_mut().fail_preflight = false;
    let connected = success(
        o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
            .unwrap(),
    );
    let calls = o.host().calls.len();
    assert!(
        o.execute_connection_research(request(&f, false, "connect", 0), &mut c)
            .is_err()
    );
    assert_eq!(calls, o.host().calls.len());
    let before = Snapshot::read_policy(
        &f.config,
        &f.paths,
        f.uid,
        2,
        &lock,
        LivePolicy::ValidCurrentBundled,
    )
    .unwrap();
    let mut other = owner(&f, Host::default());
    assert!(
        other
            .execute_connection_research(request(&f, false, "fresh", 0), &mut c)
            .is_err()
    );
    assert!(!c.poisoned());
    assert_eq!(other.revision(), 0);
    assert_eq!(other.actual(), ActualState::Disconnected);
    assert!(other.host().calls.is_empty());
    assert_eq!(
        o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
            .unwrap(),
        NativeOwnerExecution::Replay(connected),
    );
    assert_eq!(calls, o.host().calls.len());
    assert_eq!(o.revision(), 1);
    assert!(
        before.same(
            &Snapshot::read_policy(
                &f.config,
                &f.paths,
                f.uid,
                2,
                &lock,
                LivePolicy::ValidCurrentBundled,
            )
            .unwrap()
        )
    );
    // Caller admission is nonmutating; direct misbinding is still fatal.
    assert!(c.bind_owner(&std::sync::Arc::new(())).is_err());
    assert!(c.poisoned());
    assert!(
        o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
            .is_err()
    );
    assert_eq!(calls, o.host().calls.len());
}

#[test]
fn historical_connection_replay_revalidates_receipt_manager_and_current_identity() {
    use crate::login_activation::epoch_candidate::CurrentEpochProof;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    for drift in 0..3 {
        let (f, lock) = prepared(true);
        initialize(&f);
        let current = Arc::new(AtomicBool::new(true));
        let source = current.clone();
        let proof = CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
            Ok(if source.load(Ordering::SeqCst) {
                "11111111111111111111111111111111"
            } else {
                "22222222222222222222222222222222"
            }
            .into())
        })
        .unwrap();
        let mut c = witness(&f, &lock)
            .research(proof, || true)
            .unwrap()
            .into_connection()
            .unwrap();
        let mut o = owner(&f, Host::default());
        success(
            o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
                .unwrap(),
        );
        let calls = o.host().calls.len();
        match drift {
            0 => current.store(false, Ordering::SeqCst),
            1 => fs::remove_file(f.paths.runtime_base.join("omavless-login.receipt")).unwrap(),
            _ => {
                let path = f.config.join(LIVE[0]);
                let replacement = path.with_extension("replacement");
                fs::write(&replacement, fs::read(&path).unwrap()).unwrap();
                fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
                fs::rename(replacement, path).unwrap();
            }
        }
        assert!(
            o.execute_connection_research(request(&f, true, "connect", 0), &mut c)
                .is_err()
        );
        assert!(c.poisoned());
        assert_eq!(calls, o.host().calls.len());
        assert_eq!(o.revision(), 1);
    }
}

#[test]
fn historical_connection_before_writer_fence_and_unsupported_mode_have_no_effects() {
    for checkpoint in [
        ConnectionCheckpoint::BeforeDesired,
        ConnectionCheckpoint::BeforePointer,
    ] {
        let (f, lock) = prepared(true);
        initialize(&f);
        let mut c = context(&f, &lock);
        let mut o = owner(&f, Host::default());
        let store = fs::read(f.config.join(LIVE[0])).unwrap();
        let path = f.paths.state_directory.join("restore-successor.pending");
        c.fault = Some(Box::new(move |seen| {
            if seen == checkpoint {
                fs::write(&path, b"late").unwrap();
            }
            Ok(())
        }));
        assert_eq!(
            failed(
                o.execute_connection_research(request(&f, true, "late", 0), &mut c)
                    .unwrap()
            ),
            crate::connection_transaction::ConnectionTransactionError::ManualRecoveryRequired
        );
        assert!(c.poisoned());
        assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), store);
        assert!(
            !o.host().calls.contains(&"commit")
                || checkpoint == ConnectionCheckpoint::BeforePointer
        );
    }
    let (f, lock) = prepared(true);
    initialize(&f);
    let mut c = context(&f, &lock);
    let mut o = owner(&f, Host::default());
    let mode=crate::mutation_protocol::parse_owner_request(&serde_json::json!({"api":"omavless.control","version":1,"id":"research","method":"routing.set_mode","params":{"mode":"direct","operationId":"mode","expectedRevision":0}})).unwrap();
    assert!(o.execute_connection_research(mode, &mut c).is_err());
    assert!(o.host().calls.is_empty());
    assert!(!c.poisoned());
}

#[test]
fn historical_connection_absent_live_member_is_not_default_authority() {
    for store in [false, true] {
        let (f, lock) = prepared(true);
        initialize(&f);
        let mut c = context(&f, &lock);
        let mut o = owner(&f, Host::default());
        let req = request(&f, true, "absent", 0);
        fs::remove_file(if store {
            f.config.join(LIVE[0])
        } else {
            desired_paths(&f).file
        })
        .unwrap();
        assert!(o.execute_connection_research(req, &mut c).is_err());
        assert!(c.poisoned());
        assert!(o.host().calls.is_empty());
        assert_eq!(o.revision(), 0);
    }
}

#[test]
#[ignore = "requires explicitly authorized OMAVLESS_TEST_MIHOMO actual-core invocation"]
fn historical_connection_actual_no_tun_core_is_cleaned_and_off_restored() {
    let binary = fs::canonicalize(
        std::env::var_os("OMAVLESS_TEST_MIHOMO").expect("explicit actual-core test binary"),
    )
    .unwrap();
    let (f, lock) = prepared(true);
    initialize(&f);
    let mut c = context(&f, &lock);
    // Short socket path outside repository, exclusively owned private directory.
    let directory = crate::test_temp::directory("t4core").unwrap();
    let socket = directory.join("c.sock");
    let host = NoTunCoreHost {
        binary,
        directory: directory.clone(),
        socket,
        core: None,
        started: false,
        stopped: false,
    };
    let mut o = OfflineNativeCoordinator::new_ownership_gated(
        host,
        desired_paths(&f),
        &f.config.join(LIVE[0]),
        f.paths.clone(),
        f.uid,
        2,
    );
    let store = fs::read(f.config.join(LIVE[0])).unwrap();
    assert_eq!(
        failed(
            o.execute_connection_research(request(&f, true, "actual-core", 0), &mut c)
                .unwrap()
        ),
        crate::connection_transaction::ConnectionTransactionError::TransitionFailedRestored
    );
    assert!(o.host().started && o.host().stopped);
    assert!(o.host().core.is_none());
    assert_eq!(o.actual(), ActualState::Disconnected);
    assert!(!c.poisoned());
    assert_eq!(o.revision(), 0);
    let desired = read_desired_snapshot(&desired_paths(&f), f.uid).unwrap();
    assert!(!desired.connected);
    assert_eq!(desired.generation, 21);
    assert_eq!(fs::read(f.config.join(LIVE[0])).unwrap(), store);
    assert!(!directory.join("c.sock").exists());
    assert!(!directory.join("core.yaml").exists());
    drop(o);
    fs::remove_dir_all(directory).unwrap();
}
