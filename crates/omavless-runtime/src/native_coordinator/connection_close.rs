// SPDX-License-Identifier: MIT
//! Inactive actual-owner close research. No method/permit/package activation.
//! Controller work is moved out of this owner; the worker never calls it back.

use super::*;
use crate::conditional_close_candidate::{Cancellation, ObservedRow, Scheduler, Worker};
use crate::desired::{DesiredState, read_desired};
use crate::mutation::{
    ExternalCloseAdmission, ExternalCloseOutcome, ExternalCloseReceipt, ExternalCloseToken,
    MutationDigest,
};
use crate::native_host::{CloseObservation, NativeLifecycleHost};
use omavless_store::read_private_utf8;
use std::collections::BTreeSet;
use std::io::Read;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CONFIRMATION_LIFETIME: Duration = Duration::from_secs(5);
const ENTROPY_LIMIT: usize = 1024;

#[derive(Clone)]
struct Context {
    instance: String,
    ownership: OwnershipFence,
    revision: u64,
    desired: DesiredState,
    desired_paths: DesiredPaths,
    cutover_paths: crate::cutover::CutoverPaths,
    store_path: PathBuf,
    config_path: PathBuf,
    store: String,
    config: String,
    uid: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::OwnedCore;
    use crate::desired::{RoutingMode, write_desired};
    use crate::native_host::NativeHostPaths;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    static FIXTURES: Mutex<()> = Mutex::new(());
    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    // Fixed owned subprocess/private Unix controller. No public listener,
    // provider, TUN, DNS, service, shell effect or injected owner facts.
    const CONTROLLER: &str = r#"#!/usr/bin/python3
import argparse,json,os,socket,time
p=argparse.ArgumentParser();p.add_argument('-d');p.add_argument('-f');a=p.parse_args()
root=a.d;variant=open(a.f).readline().strip().removeprefix('# fixture-variant:')
s=socket.socket(socket.AF_UNIX);s.bind(root+'/mihomo.sock');os.chmod(root+'/mihomo.sock',0o600);s.listen(8)
while True:
 c,_=s.accept();c.settimeout(3);raw=b''
 while b'\r\n\r\n' not in raw and len(raw)<8192:
  b=c.recv(1024)
  if not b:break
  raw+=b
 if not raw:c.close();continue
 line=raw.split(b'\r\n',1)[0];status=200
 if line.startswith(b'GET /version '):body={'version':'owned-fixture'}
 elif line.startswith(b'GET /configs '):
  if os.path.exists(root+'/stall-read'):
   open(root+'/read-entered','wb').close();until=time.monotonic()+8
   while not os.path.exists(root+'/release') and time.monotonic()<until:time.sleep(.002)
  body={'mode':'direct','tun':{'enable':False}}
 elif line.startswith(b'GET /rules '):body={'rules':[]}
 elif line.startswith(b'GET /providers/rules '):body={'providers':{}}
 elif line.startswith(b'GET /proxies '):body={'proxies':{'DIRECT':{}}}
 elif line.startswith(b'GET /connections/conditional-capabilities '):body={'abi':1,'ready':True}
 elif line.startswith(b'GET /connections '):
  rows=[{'id':'11111111-1111-4111-8111-111111111111','omavlessCloseToken':'42','metadata':{'host':'same.invalid','destinationPort':'443','network':'tcp'},'chains':['DIRECT']},{'id':'22222222-2222-4222-8222-222222222222','omavlessCloseToken':'43','metadata':{'host':'same.invalid','destinationPort':'443','network':'tcp'},'chains':['DIRECT']}]
  if os.path.exists(root+'/reverse'):rows.reverse()
  if os.path.exists(root+'/removed'):rows=rows[1:]
  if os.path.exists(root+'/reused'):rows[0]['omavlessCloseToken']='44'
  if os.path.exists(root+'/display-drift'):rows[0]['metadata']['host']='changed.invalid'
  body={'connections':rows}
 elif line.startswith(b'POST /connections/'):
  with open(root+'/effects','ab') as f:f.write(b'2\n' if b'22222222-' in line else b'1\n')
  open(root+'/effect-entered','wb').close()
  if variant=='stall-reply':
   until=time.monotonic()+8
   while not os.path.exists(root+'/release') and time.monotonic()<until:time.sleep(.002)
  if variant=='drop':c.close();continue
  status=204;body=None
 else:status=400;body=None
 data=b'' if body is None else json.dumps(body).encode()
 try:c.sendall(('HTTP/1.0 '+str(status)+' OK\r\nContent-Length: '+str(len(data))+'\r\n\r\n').encode()+data)
 except OSError:pass
 c.close()
"#;

    struct Fixture {
        root: PathBuf,
        owner: OfflineNativeCoordinator<NativeLifecycleHost>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.owner.invalidate_connection_close();
            let _ = self.owner.host_mut().stop_owned();
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn write(path: &Path, bytes: &[u8], mode: u32) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    fn fixture(variant: &str) -> Fixture {
        let root = crate::test_temp::directory("owner-close").unwrap();
        let runtime = root.join("r");
        let config = root.join("c");
        let state = root.join("s");
        for path in [&runtime, &config, &state] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = nix::unistd::Uid::current().as_raw();
        let exe = root.join("core.py");
        write(&exe, CONTROLLER.as_bytes(), 0o700);
        let store = json!({"version":3,"activeId":PROFILE,"lastId":PROFILE,
            "profiles":[{"id":PROFILE,"name":"DIRECT","uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp","protocol":"vless","favorite":false}],
            "subscriptions":[],"routingPreset":"custom","customRules":[],"rulesUpdatedAt":0,
            "startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":true});
        write(
            &config.join("profiles.json"),
            store.to_string().as_bytes(),
            0o600,
        );
        write(
            &config.join("config.yaml"),
            format!("# fixture-variant:{variant}\ntun:\n  enable: false\n").as_bytes(),
            0o600,
        );
        let desired_paths = DesiredPaths::below(&state);
        write_desired(
            &desired_paths,
            uid,
            &DesiredState {
                connected: true,
                profile_id: PROFILE.into(),
                mode: RoutingMode::Direct,
                schema_version: 1,
                generation: 7,
            },
        )
        .unwrap();
        let cutover = crate::cutover::CutoverPaths::below(&runtime, &state, uid);
        write(
            &cutover.ownership_marker,
            br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
            0o600,
        );
        let socket = runtime.join("mihomo.sock");
        let mut core =
            OwnedCore::spawn(&exe, &runtime, &config.join("config.yaml"), &socket).unwrap();
        core.wait_ready(Duration::from_secs(5)).unwrap();
        let paths = NativeHostPaths::new(
            exe,
            config.clone(),
            config.clone(),
            runtime,
            PathBuf::from("/proc"),
            PathBuf::from("/sys/class/net"),
        );
        let host = NativeLifecycleHost::owned_close_fixture(paths, uid, core).unwrap();
        let mut owner = OfflineNativeCoordinator::new_ownership_gated(
            host,
            desired_paths,
            &config.join("profiles.json"),
            cutover,
            uid,
            2,
        );
        owner
            .initialize_batch_operations("actual-owner-close-fixture")
            .unwrap();
        owner
            .transaction
            .lifecycle_mut()
            .adopt_owned_close_fixture()
            .unwrap();
        Fixture { root, owner }
    }
    fn snapshot(fixture: &mut Fixture) -> Vec<CloseDisplayRow> {
        let discovery = fixture.owner.capture_connection_close().unwrap();
        let observed = discovery.observe().unwrap();
        fixture.owner.retain_connection_close(observed).unwrap()
    }
    fn receipt(fixture: &mut Fixture) -> ExternalCloseReceipt {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            if let Some(receipt) = fixture.owner.poll_connection_close().unwrap() {
                return receipt;
            }
            assert!(
                Instant::now() < deadline,
                "bounded actual owner close did not terminalize"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn marker(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "owned controller marker timed out"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn disconnect(
        owner: &mut OfflineNativeCoordinator<NativeLifecycleHost>,
        operation_id: &str,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let request = omavless_control_protocol::make_request(
            "disconnect",
            "connection.disconnect",
            json!({"operationId":operation_id,"expectedRevision":owner.revision()}),
        )
        .unwrap();
        owner.execute_connection(crate::mutation_protocol::parse_owner_request(&request).unwrap())
    }

    #[test]
    fn actual_owner_snapshot_confirm_typed_receipt_and_expired_exact_replay() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let desired = fixture.owner.desired().unwrap();
        let rows = snapshot(&mut fixture);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].display, rows[1].display);
        assert_ne!(rows[0].handle, rows[1].handle);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[1].handle)
            .unwrap();
        assert_eq!(confirmation.display, rows[1].display);
        write(&fixture.root.join("r/reverse"), b"fixed", 0o600);
        assert!(
            fixture
                .owner
                .confirm_connection_close("close-one", 0, rows[1].handle, confirmation.ticket)
                .unwrap()
                .is_none()
        );
        let result = receipt(&mut fixture);
        assert_eq!(
            result,
            ExternalCloseReceipt {
                outcome: ExternalCloseOutcome::Closed,
                revision: 1
            }
        );
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"2\n");
        assert_eq!(fixture.owner.desired().unwrap(), desired);
        fixture.owner.invalidate_connection_close();
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close("close-one", 0, rows[1].handle, confirmation.ticket)
                .unwrap(),
            Some(result)
        );
        assert!(
            fixture
                .owner
                .confirm_connection_close("close-one", 0, rows[0].handle, confirmation.ticket)
                .is_err()
        );
        assert!(disconnect(&mut fixture.owner, "close-one").is_err());
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"2\n");
    }

    #[test]
    fn actual_owner_unknown_is_retained_without_vpn_recovery_or_resend_and_disconnect_remains_available()
     {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("drop");
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        let before = fixture.owner.desired().unwrap();
        fixture
            .owner
            .confirm_connection_close("unknown-close", 0, rows[0].handle, confirmation.ticket)
            .unwrap();
        let result = receipt(&mut fixture);
        assert_eq!(result.outcome, ExternalCloseOutcome::Unknown);
        assert_eq!(fixture.owner.actual(), ActualState::Connected);
        assert_eq!(fixture.owner.desired().unwrap(), before);
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close("unknown-close", 0, rows[0].handle, confirmation.ticket)
                .unwrap(),
            Some(result)
        );
        disconnect(&mut fixture.owner, "urgent-disconnect").unwrap();
        assert!(!fixture.owner.desired().unwrap().connected);
        assert_eq!(fixture.owner.actual(), ActualState::Disconnected);
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    }

    #[test]
    fn actual_owner_disconnect_cancels_stalled_read_or_reply_before_lifecycle_effects() {
        let _fixtures = FIXTURES.lock().unwrap();
        for after_write in [false, true] {
            let mut fixture = fixture(if after_write { "stall-reply" } else { "ok" });
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            if !after_write {
                write(&fixture.root.join("r/stall-read"), b"fixed", 0o600);
            }
            fixture
                .owner
                .confirm_connection_close("cancelled-close", 0, rows[0].handle, confirmation.ticket)
                .unwrap();
            marker(&fixture.root.join(if after_write {
                "r/effect-entered"
            } else {
                "r/read-entered"
            }));
            let started = Instant::now();
            assert_eq!(fixture.owner.revision(), 0);
            disconnect(&mut fixture.owner, "disconnect-wins").unwrap();
            assert!(started.elapsed() < Duration::from_secs(1));
            let result = receipt(&mut fixture);
            assert_eq!(
                result.outcome,
                if after_write {
                    ExternalCloseOutcome::Unknown
                } else {
                    ExternalCloseOutcome::RefusedBeforeWrite
                }
            );
            assert_eq!(fixture.owner.revision(), 1);
            assert!(!fixture.owner.desired().unwrap().connected);
            assert_eq!(
                fixture
                    .owner
                    .confirm_connection_close(
                        "cancelled-close",
                        0,
                        rows[0].handle,
                        confirmation.ticket
                    )
                    .unwrap(),
                Some(result)
            );
            if after_write {
                assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
            } else {
                assert!(!fixture.root.join("r/effects").exists());
            }
        }
    }

    #[test]
    fn actual_owner_expiry_ticket_replacement_and_durable_drift_refuse_before_post() {
        let _fixtures = FIXTURES.lock().unwrap();
        for drift in [
            "expiry",
            "ticket",
            "store",
            "config",
            "desired",
            "ownership",
            "pending",
            "executable",
            "display",
            "reuse",
            "removed",
        ] {
            let mut fixture = fixture("ok");
            let rows = snapshot(&mut fixture);
            let expiry = fixture
                .owner
                .connection_close
                .snapshot
                .as_ref()
                .unwrap()
                .expiry;
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            assert_eq!(
                fixture
                    .owner
                    .connection_close
                    .snapshot
                    .as_ref()
                    .unwrap()
                    .expiry,
                expiry
            );
            match drift {
                "expiry" => {
                    fixture
                        .owner
                        .connection_close
                        .snapshot
                        .as_mut()
                        .unwrap()
                        .expiry = Instant::now()
                }
                "ticket" => {
                    fixture
                        .owner
                        .prepare_connection_close(rows[1].handle)
                        .unwrap();
                }
                "store" => {
                    let path = fixture.root.join("c/profiles.json");
                    let mut value: Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    value["rulesUpdatedAt"] = json!(1);
                    write(&path, value.to_string().as_bytes(), 0o600);
                }
                "config" => write(&fixture.root.join("c/config.yaml"), b"changed", 0o600),
                "desired" => {
                    let mut desired = fixture.owner.desired().unwrap();
                    desired.generation += 1;
                    write_desired(
                        fixture.owner.transaction.desired_paths(),
                        fixture.owner.uid(),
                        &desired,
                    )
                    .unwrap();
                }
                "ownership" => write(
                    &fixture.root.join("s/omavless/ownership.json"),
                    br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                    0o600,
                ),
                "pending" => write(
                    &fixture.root.join("s/omavless/routing-preset.pending.json"),
                    b"pending",
                    0o600,
                ),
                "executable" => {
                    fs::rename(
                        fixture.root.join("core.py"),
                        fixture.root.join("retained-script"),
                    )
                    .unwrap();
                    write(&fixture.root.join("core.py"), CONTROLLER.as_bytes(), 0o700);
                }
                "display" => write(&fixture.root.join("r/display-drift"), b"fixed", 0o600),
                "reuse" => write(&fixture.root.join("r/reused"), b"fixed", 0o600),
                "removed" => write(&fixture.root.join("r/removed"), b"fixed", 0o600),
                _ => unreachable!(),
            }
            if fixture
                .owner
                .confirm_connection_close("drift-close", 0, rows[0].handle, confirmation.ticket)
                .is_ok()
            {
                assert_eq!(
                    receipt(&mut fixture).outcome,
                    ExternalCloseOutcome::RefusedBeforeWrite,
                    "{drift}"
                );
            }
            assert!(!fixture.root.join("r/effects").exists(), "{drift}");
        }
    }
}

/// Immutable owner-issued receipt, never a caller-selected proof or boolean.
/// Each write tries the cooperative durable lease and compares actual bytes.
pub(crate) struct EffectProof(Context);
impl EffectProof {
    pub(crate) fn lease(&self) -> Result<MigrationLock, ()> {
        let context = &self.0;
        let lease = MigrationLock::acquire(&context.cutover_paths, context.uid).map_err(|_| ())?;
        let marker = crate::cutover::read_marker_existing(&context.cutover_paths, context.uid)
            .map_err(|_| ())?;
        if marker.phase() != context.ownership.phase
            || marker.generation() != context.ownership.generation
            || read_desired(&context.desired_paths, context.uid).map_err(|_| ())? != context.desired
            || crate::routing_preset::pending(&context.desired_paths)
            || read_private_utf8(&context.store_path, context.uid).map_err(|_| ())? != context.store
            || read_private_utf8(&context.config_path, context.uid).map_err(|_| ())?
                != context.config
        {
            return Err(());
        }
        Ok(lease)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OpaqueToken([u8; 32]);
impl fmt::Debug for OpaqueToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueCloseToken([private])")
    }
}

pub(crate) struct CloseDisplayRow {
    pub(crate) handle: OpaqueToken,
    pub(crate) display: Value,
}
pub(crate) struct CloseConfirmation {
    pub(crate) ticket: OpaqueToken,
    pub(crate) display: Value,
}
struct RetainedRow {
    handle: OpaqueToken,
    observed: Option<ObservedRow>,
}
struct Snapshot {
    context: Context,
    expiry: Instant,
    observation: CloseObservation,
    rows: Vec<RetainedRow>,
}
struct Pending {
    handle: OpaqueToken,
    ticket: OpaqueToken,
}
struct ActiveClose {
    token: ExternalCloseToken,
    worker: Worker,
}
#[derive(Default)]
pub(super) struct CloseState {
    discovery: Option<Arc<()>>,
    snapshot: Option<Snapshot>,
    pending: Option<Pending>,
    issued: BTreeSet<OpaqueToken>,
    cancellation: Option<Cancellation>,
    scheduler: Scheduler,
    active: Option<ActiveClose>,
}
impl CloseState {
    fn entropy(&mut self) -> Result<OpaqueToken, NativeOwnerError> {
        if self.issued.len() >= ENTROPY_LIMIT {
            return Err(NativeOwnerError::Coordinator(CoordinatorError::Busy));
        }
        let mut bytes = [0; 32];
        // Fixed kernel CSPRNG device, never symlink/fallback or caller path.
        let mut source = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
            .open("/dev/urandom")
            .map_err(|_| NativeOwnerError::Invariant)?;
        let metadata = source.metadata().map_err(|_| NativeOwnerError::Invariant)?;
        if !metadata.file_type().is_char_device()
            || metadata.uid() != 0
            || metadata.rdev() != nix::libc::makedev(1, 9)
        {
            return Err(NativeOwnerError::Invariant);
        }
        source
            .read_exact(&mut bytes)
            .map_err(|_| NativeOwnerError::Invariant)?;
        let token = OpaqueToken(bytes);
        if bytes == [0; 32] || !self.issued.insert(token) {
            return Err(NativeOwnerError::Invariant);
        }
        Ok(token)
    }
    pub(super) fn invalidate(&mut self) {
        // Lifetime gate cancellation linearizes BEFORE any competing owner
        // changes state/revision or attempts a migration lease.
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        if let Some(active) = &self.active {
            active.worker.cancel();
        }
        self.pending = None;
        self.snapshot = None;
        self.discovery = None;
    }
}

pub(crate) struct CloseDiscovery {
    identity: Arc<()>,
    context: Context,
    expiry: Instant,
    observation: CloseObservation,
}
pub(crate) struct CloseDiscovered {
    identity: Arc<()>,
    context: Context,
    expiry: Instant,
    observation: CloseObservation,
    rows: Vec<ObservedRow>,
}
impl CloseDiscovery {
    /// Run outside the actual owner's mutex AND migration lease.
    pub(crate) fn observe(mut self) -> Result<CloseDiscovered, NativeOwnerError> {
        self.observation
            .observe()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let rows = self
            .observation
            .session_mut()
            .discover_rows()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        self.observation
            .observe()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        if Instant::now() >= self.expiry {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        Ok(CloseDiscovered {
            identity: self.identity,
            context: self.context,
            expiry: self.expiry,
            observation: self.observation,
            rows,
        })
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(super) fn invalidate_connection_close(&mut self) {
        self.connection_close.invalidate();
    }
}

impl OfflineNativeCoordinator<NativeLifecycleHost> {
    fn close_context(&self) -> Result<Context, NativeOwnerError> {
        let ownership = self
            .required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust)
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let state = self
            .batch
            .as_ref()
            .filter(|state| !state.stopped && state.active.is_none())
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if self.coordinator.active()
            || self.coordinator.queued() != 0
            || self.actual() != ActualState::Connected
            || self.transaction.blocked()
            || crate::routing_preset::pending(self.transaction.desired_paths())
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if !self
            .transaction
            .ownership_matches(ownership.phase, ownership.generation)
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let desired = self
            .transaction
            .desired()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        if !desired.connected {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let store_path = self.transaction.store_path();
        crate::private_store_transaction::validate_store_path(store_path, self.uid())
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let store = read_private_utf8(store_path, self.uid())
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        omavless_domain::private_store::parse_private_store(&store)
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let config_path = store_path
            .parent()
            .ok_or(NativeOwnerError::Invariant)?
            .join("config.yaml");
        let config = read_private_utf8(&config_path, self.uid())
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        if config.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        Ok(Context {
            instance: state.instance.clone(),
            ownership,
            revision: self.revision(),
            desired,
            desired_paths: self.transaction.desired_paths().clone(),
            cutover_paths: self.transaction.cutover_paths().clone(),
            store_path: store_path.to_owned(),
            config_path,
            store,
            config,
            uid: self.uid(),
        })
    }

    fn close_context_matches(&self, expected: &Context) -> Result<(), NativeOwnerError> {
        let actual = self.close_context()?;
        if actual.instance != expected.instance
            || actual.ownership != expected.ownership
            || actual.revision != expected.revision
            || actual.desired != expected.desired
            || actual.store != expected.store
            || actual.config != expected.config
        {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict,
            ));
        }
        Ok(())
    }

    pub(crate) fn capture_connection_close(&mut self) -> Result<CloseDiscovery, NativeOwnerError> {
        self.invalidate_connection_close();
        if self.connection_close.active.is_some() {
            return Err(NativeOwnerError::Coordinator(CoordinatorError::Busy));
        }
        let _lease = self.batch_lock()?;
        let context = self.close_context()?;
        let observation = self
            .host_mut()
            .capture_connection_close(&context.desired)
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        self.connection_close.cancellation = Some(observation.session().cancellation());
        let identity = Arc::new(());
        self.connection_close.discovery = Some(Arc::clone(&identity));
        Ok(CloseDiscovery {
            identity,
            context,
            expiry: Instant::now() + CONFIRMATION_LIFETIME,
            observation,
        })
    }

    pub(crate) fn retain_connection_close(
        &mut self,
        mut discovered: CloseDiscovered,
    ) -> Result<Vec<CloseDisplayRow>, NativeOwnerError> {
        if !self
            .connection_close
            .discovery
            .as_ref()
            .is_some_and(|identity| Arc::ptr_eq(identity, &discovered.identity))
        {
            // A late result may never cancel or replace a newer capture.
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let _lease = self.batch_lock()?;
        self.close_context_matches(&discovered.context)?;
        if Instant::now() >= discovered.expiry
            || !discovered.observation.session_mut().proves_live()
        {
            self.invalidate_connection_close();
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let mut rows = Vec::with_capacity(discovered.rows.len());
        let mut displayed = Vec::with_capacity(discovered.rows.len());
        for observed in discovered.rows {
            let handle = self.connection_close.entropy()?;
            displayed.push(CloseDisplayRow {
                handle,
                display: observed.display.clone(),
            });
            rows.push(RetainedRow {
                handle,
                observed: Some(observed),
            });
        }
        self.connection_close.snapshot = Some(Snapshot {
            context: discovered.context,
            expiry: discovered.expiry,
            observation: discovered.observation,
            rows,
        });
        self.connection_close.discovery = None;
        Ok(displayed)
    }

    pub(crate) fn prepare_connection_close(
        &mut self,
        handle: OpaqueToken,
    ) -> Result<CloseConfirmation, NativeOwnerError> {
        self.connection_close.pending = None;
        let _lease = self.batch_lock()?;
        let snapshot = self
            .connection_close
            .snapshot
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        self.close_context_matches(&snapshot.context)?;
        if Instant::now() >= snapshot.expiry {
            self.invalidate_connection_close();
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let display = snapshot
            .rows
            .iter()
            .find(|row| row.handle == handle)
            .and_then(|row| row.observed.as_ref())
            .ok_or(NativeOwnerError::RecordNotFound)?
            .display
            .clone();
        let ticket = self.connection_close.entropy()?;
        self.connection_close.pending = Some(Pending { handle, ticket });
        Ok(CloseConfirmation { ticket, display })
    }

    pub(crate) fn confirm_connection_close(
        &mut self,
        operation_id: &str,
        expected_revision: u64,
        handle: OpaqueToken,
        ticket: OpaqueToken,
    ) -> Result<Option<ExternalCloseReceipt>, NativeOwnerError> {
        let mut semantic = b"omavless-owner-single-conditional-close-v1\0".to_vec();
        let instance = self
            .batch
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?
            .instance
            .as_bytes();
        semantic.extend_from_slice(&(instance.len() as u64).to_be_bytes());
        semantic.extend_from_slice(instance);
        semantic.extend_from_slice(&handle.0);
        semantic.extend_from_slice(&ticket.0);
        semantic.extend_from_slice(&expected_revision.to_be_bytes());
        let long_id = self
            .batch
            .as_ref()
            .is_some_and(|state| state.registry.has_operation_id(operation_id));
        let token = match self.coordinator.reserve_external_close(
            operation_id,
            expected_revision,
            MutationDigest::from_semantic_bytes(&semantic),
            long_id,
        )? {
            ExternalCloseAdmission::Replay(receipt) => return Ok(Some(receipt)),
            ExternalCloseAdmission::Reserved(token) => token,
        };
        let result = self.confirm_connection_close_reserved(handle, ticket, &token);
        match result {
            Ok(None) => Ok(None),
            Ok(Some(outcome)) => self
                .coordinator
                .finish_external_close(&token, outcome)
                .map(Some)
                .map_err(Into::into),
            Err(error) => {
                self.invalidate_connection_close();
                self.coordinator
                    .finish_external_close(&token, ExternalCloseOutcome::RefusedBeforeWrite)?;
                Err(error)
            }
        }
    }

    fn confirm_connection_close_reserved(
        &mut self,
        handle: OpaqueToken,
        ticket: OpaqueToken,
        token: &ExternalCloseToken,
    ) -> Result<Option<ExternalCloseOutcome>, NativeOwnerError> {
        let _lease = self.batch_lock()?;
        let pending = self
            .connection_close
            .pending
            .take()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if pending.handle != handle || pending.ticket != ticket {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let mut snapshot = self
            .connection_close
            .snapshot
            .take()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        self.close_context_matches(&snapshot.context)?;
        if Instant::now() >= snapshot.expiry || !snapshot.observation.session_mut().proves_live() {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let selected = snapshot
            .rows
            .iter_mut()
            .find(|row| row.handle == handle)
            .and_then(|row| row.observed.take())
            .ok_or(NativeOwnerError::RecordNotFound)?;
        // Passive bytes/ABI are NOT conditional package attestation. Only the
        // cfg(test) fixed owned fixture constructor can exercise the effect.
        #[cfg(test)]
        if let Some(permit) = snapshot.observation.fixture_permit() {
            let mut session = snapshot.observation.into_session();
            session.authorize_effect(
                EffectProof(snapshot.context),
                snapshot.expiry,
                selected.display,
            );
            let worker = self
                .connection_close
                .scheduler
                .start(session, selected.target, permit)
                .map_err(|_| NativeOwnerError::Invariant)?;
            self.connection_close.active = Some(ActiveClose {
                token: token.clone(),
                worker,
            });
            return Ok(None);
        }
        let _ = (selected, token);
        Ok(Some(ExternalCloseOutcome::MissingAttestation))
    }

    pub(crate) fn poll_connection_close(
        &mut self,
    ) -> Result<Option<ExternalCloseReceipt>, NativeOwnerError> {
        let Some(active) = self.connection_close.active.as_mut() else {
            return Ok(None);
        };
        let Some(outcome) = active.worker.poll() else {
            return Ok(None);
        };
        let active = self
            .connection_close
            .active
            .take()
            .ok_or(NativeOwnerError::Invariant)?;
        self.connection_close.cancellation = None;
        self.coordinator
            .finish_external_close(&active.token, outcome.into())
            .map(Some)
            .map_err(Into::into)
    }
}
