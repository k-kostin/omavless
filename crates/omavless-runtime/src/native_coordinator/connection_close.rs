// SPDX-License-Identifier: MIT
//! Actual-owner close research, with a separate opt-in development workspace.
//! No default-product method/permit/package activation.
//! Controller work is moved out of this owner; the worker never calls it back.

use super::*;
use crate::conditional_close_candidate::{Cancellation, ObservedRow, Scheduler, Worker};
use crate::desired::{DesiredState, read_desired};
use crate::mutation::{
    ExternalCloseAdmission, ExternalCloseOutcome, ExternalCloseReceipt, ExternalCloseToken,
    MutationDigest,
};
use crate::native_host::CloseObservation;
#[cfg(test)]
use crate::native_host::NativeLifecycleHost;
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
    use crate::long_operation::LongOperationError;
    use crate::native_host::NativeHostPaths;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    static FIXTURES: Mutex<()> = Mutex::new(());
    #[cfg(feature = "developer-conditional-close")]
    #[test]
    #[ignore = "ROOT-reviewed fresh normal-package-path qualification namespace only"]
    fn actual_owner_qualified_package_socket_close_in_dev_vm() {
        assert!(std::env::var("OMAVLESS_CLOSE_QUALIFIED_PAIR_VM").as_deref() == Ok("1"));
        assert_eq!(nix::unistd::getuid().as_raw(), 1000);
        assert_eq!(nix::unistd::getgid().as_raw(), 1000);
        assert_eq!(nix::unistd::getpid().as_raw(), 1);
        composed_core_selected_close_with_client(
            PathBuf::from(crate::managed_pair::RELEASE_CORE),
            true,
            false,
            true,
            false,
            false,
            true,
        );
    }
    #[test]
    fn qualified_selector_is_written_before_first_adoption_observation() {
        let helper = include_str!("connection_close.rs")
            .split(concat!(
                "    fn composed_core_",
                "selected_close_with_client("
            ))
            .nth(1)
            .unwrap();
        assert!(
            helper
                .find("&config.join(crate::managed_pair::SELECTOR)")
                .unwrap()
                < helper.find(".adopt_owned_close_fixture()").unwrap()
        );
    }
    #[cfg(all(feature = "developer-conditional-close", feature = "tui"))]
    include!("connection_close_client_integration.rs");
    #[cfg(all(feature = "developer-conditional-close", feature = "tui"))]
    include!("connection_close_client_terminal.rs");
    #[cfg(all(feature = "developer-conditional-close", feature = "tui"))]
    include!("connection_close_real_cli_foot.rs");
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
  if raw.startswith(b'POST ') and b'\r\n\r\n' not in raw:
   with open(root+'/partial-entered.tmp','wb') as f:f.write(raw)
   os.replace(root+'/partial-entered.tmp',root+'/partial-entered')
 if not raw:c.close();continue
 line=raw.split(b'\r\n',1)[0];status=200
 if line.startswith(b'GET /version '):body={'version':'owned-fixture'}
 elif line.startswith(b'GET /configs '):
  if os.path.exists(root+'/exec-replace'):os.execl('/usr/bin/sleep','sleep','30')
  if os.path.exists(root+'/stall-read'):
   open(root+'/read-entered','wb').close();until=time.monotonic()+8
   while not os.path.exists(root+'/release') and time.monotonic()<until:time.sleep(.002)
  body={'mode':'rule' if variant.startswith('rule-') else 'direct','tun':{'enable':False}}
 elif line.startswith(b'GET /rules '):body={'rules':[]}
 elif line.startswith(b'GET /providers/rules '):body={'providers':{'owned-fixture':{'vehicleType':'HTTP'}}} if variant.startswith('rule-') else {'providers':{}}
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
        owner: FixtureCoordinator,
    }
    // Test-only movable slot: transfers the SAME original coordinator into the
    // socket fixture without constructing a replacement owner or using unsafe.
    struct FixtureCoordinator(Option<OfflineNativeCoordinator<NativeLifecycleHost>>);
    impl std::ops::Deref for FixtureCoordinator {
        type Target = OfflineNativeCoordinator<NativeLifecycleHost>;
        fn deref(&self) -> &Self::Target {
            self.0.as_ref().unwrap()
        }
    }
    impl std::ops::DerefMut for FixtureCoordinator {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.0.as_mut().unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(owner) = self.owner.0.as_mut() {
                owner.invalidate_connection_close();
                let _ = owner.host_mut().stop_owned();
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }
    fn write(path: &Path, bytes: &[u8], mode: u32) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    fn fixture(variant: &str) -> Fixture {
        let (root, owner) = fixture_parts(variant, Some("actual-owner-close-fixture"));
        Fixture {
            root,
            owner: FixtureCoordinator(Some(owner)),
        }
    }
    fn fixture_parts(
        variant: &str,
        batch_instance: Option<&str>,
    ) -> (PathBuf, OfflineNativeCoordinator<NativeLifecycleHost>) {
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
        write(
            &config.join("route-template.yaml"),
            b"tun:\n  enable: false\n",
            0o600,
        );
        let desired_paths = DesiredPaths::below(&state);
        write_desired(
            &desired_paths,
            uid,
            &DesiredState {
                connected: true,
                profile_id: PROFILE.into(),
                mode: if variant.starts_with("rule-") {
                    RoutingMode::Rule
                } else {
                    RoutingMode::Direct
                },
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
        let host = if variant.starts_with("rule-") {
            NativeLifecycleHost::owned_rule_close_fixture(paths, uid, core)
        } else if variant == "passive-ok" {
            NativeLifecycleHost::passive_owned_close_fixture(paths, uid, core)
        } else {
            NativeLifecycleHost::owned_close_fixture(paths, uid, core)
        }
        .unwrap();
        let mut owner = OfflineNativeCoordinator::new_ownership_gated(
            host,
            desired_paths,
            &config.join("profiles.json"),
            cutover,
            uid,
            2,
        );
        if let Some(instance) = batch_instance {
            owner.initialize_batch_operations(instance).unwrap();
        }
        owner
            .transaction
            .lifecycle_mut()
            .adopt_owned_close_fixture()
            .unwrap();
        (root, owner)
    }

    #[cfg(feature = "developer-conditional-close")]
    struct SocketFixture {
        root: PathBuf,
        paths: crate::RuntimePaths,
        instance: String,
        stop: Arc<std::sync::atomic::AtomicBool>,
        worker: Option<std::thread::JoinHandle<()>>,
    }

    #[cfg(feature = "developer-conditional-close")]
    impl SocketFixture {
        fn new(variant: &str) -> Self {
            // Deliberately leave batch admission untouched: registration must
            // lazily bind the real server instance, not a fixture identifier.
            let (root, owner) = fixture_parts(variant, None);
            Self::from_parts(root, owner)
        }

        fn from_parts(root: PathBuf, owner: OfflineNativeCoordinator<NativeLifecycleHost>) -> Self {
            let paths = crate::RuntimePaths::below(&root);
            let mut server = crate::RuntimeServer::bind(paths.clone()).unwrap();
            server.register_native_owner(
                crate::production_owner::ProductionNativeOwner::from_owned_close_socket_fixture(
                    owner,
                ),
                crate::subscription_transport::HttpsSubscriptionTransport::new(),
            );
            let instance = server.instance_id.clone();
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let worker = std::thread::spawn(move || server.serve_until(&worker_stop).unwrap());
            Self {
                root,
                paths,
                instance,
                stop,
                worker: Some(worker),
            }
        }

        fn from_fixture(mut fixture: Fixture) -> Self {
            // This successor consumes the already-owned live host. The old
            // fixture must not stop it or remove the transferred directory.
            let owner = fixture.owner.0.take().unwrap();
            let root = std::mem::take(&mut fixture.root);
            Self::from_parts(root, owner)
        }

        fn call(&self, method: &str, params: serde_json::Value) -> serde_json::Value {
            crate::call(&self.paths, method, params).unwrap()
        }

        fn confirmation(&self, operation: &str) -> serde_json::Value {
            let snapshot = self.call(
                "development.connections.snapshot",
                json!({"instanceId":self.instance}),
            );
            assert_eq!(snapshot["ok"], true);
            assert_eq!(snapshot["result"]["instanceId"], self.instance);
            let rows = snapshot["result"]["rows"].as_array().unwrap();
            assert_eq!(rows.len(), 2);
            let handle = rows[0]["handle"].as_str().unwrap();
            let prepared = self.call(
                "development.connections.prepare",
                json!({"instanceId":self.instance,"handle":handle}),
            );
            assert_eq!(prepared["ok"], true);
            json!({"instanceId":self.instance,"operationId":operation,
                "expectedRevision":0,"handle":handle,"ticket":prepared["result"]["ticket"]})
        }

        fn receipt(&self, operation: &str) -> serde_json::Value {
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                let response = self.call(
                    "development.connections.receipt",
                    json!({"instanceId":self.instance,"operationId":operation}),
                );
                if response["ok"] == true && response["result"]["state"] == "finished" {
                    return response;
                }
                assert!(
                    Instant::now() < deadline,
                    "socket close did not terminalize"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        fn desired_bytes(&self) -> Vec<u8> {
            fs::read(DesiredPaths::below(&self.root.join("s")).file).unwrap()
        }
    }

    #[cfg(feature = "developer-conditional-close")]
    impl Drop for SocketFixture {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Release);
            if let Some(worker) = self.worker.take() {
                // The original server and concrete owned host are dropped by
                // this worker, before removing only its synthetic directory.
                let _ = worker.join();
            }
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(feature = "developer-conditional-close")]
    #[test]
    fn developer_close_socket_lost_client_reply_replays_exactly_once() {
        use omavless_control_protocol::{
            FrameKind, encode_request, make_request, write_unary_frame,
        };
        use std::os::unix::net::UnixStream;
        let _fixtures = FIXTURES.lock().unwrap();
        let fixture = SocketFixture::new("ok");
        let before = fixture.desired_bytes();
        let params = fixture.confirmation("lost-client-close");
        let request = make_request(
            "lost-client",
            "development.connections.confirm",
            params.clone(),
        )
        .unwrap();
        let mut client = UnixStream::connect(&fixture.paths.socket).unwrap();
        write_unary_frame(
            &mut client,
            &encode_request(&request).unwrap(),
            FrameKind::Request,
        )
        .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        // A complete request was written to the actual same-UID listener; no
        // response is consumed. Whether the server's write succeeds is unknown
        // and irrelevant to receipt reconciliation (not an injected effect).
        drop(client);
        // Establish this test's lost-*applied*-reply premise using the owned
        // controller fixture. Starting a poll client before admission could
        // legitimately cause the original try-lock admission to return Busy.
        // No request is resent and the marker creates no runtime authority.
        marker(&fixture.root.join("r/effect-entered"));
        let receipt = fixture.receipt("lost-client-close");
        assert_eq!(receipt["result"]["outcome"], "closed");
        assert_eq!(receipt["result"]["receiptRevision"], 1);
        assert_eq!(receipt["revision"], 1);
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
        assert_eq!(fixture.desired_bytes(), before);
        // Fresh authority is no longer valid, but exact replay remains a read
        // of the retained original result and must not send a second POST.
        fs::write(fixture.root.join("r/reused"), b"").unwrap();
        std::thread::sleep(CONFIRMATION_LIFETIME + Duration::from_millis(10));
        let replay = fixture.call("development.connections.confirm", params);
        assert_eq!(replay["ok"], true);
        assert_eq!(replay["result"], receipt["result"]);
        assert_eq!(replay["revision"], 1);
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
        assert_eq!(fixture.desired_bytes(), before);
    }

    #[cfg(feature = "developer-conditional-close")]
    #[test]
    fn developer_close_socket_unknown_controller_reply_is_retained_without_resend() {
        let _fixtures = FIXTURES.lock().unwrap();
        let fixture = SocketFixture::new("drop");
        let before = fixture.desired_bytes();
        let params = fixture.confirmation("unknown-controller-close");
        let admitted = fixture.call("development.connections.confirm", params.clone());
        assert_eq!(admitted["ok"], true);
        let receipt = fixture.receipt("unknown-controller-close");
        assert_eq!(receipt["result"]["outcome"], "unknown");
        assert_eq!(receipt["result"]["receiptRevision"], 1);
        assert_eq!(receipt["revision"], 1);
        let replay = fixture.call("development.connections.confirm", params);
        assert_eq!(replay["ok"], true);
        assert_eq!(replay["result"], receipt["result"]);
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
        assert_eq!(fixture.desired_bytes(), before);
        let status = fixture.call("status.get", json!({}));
        assert_eq!(status["ok"], true);
        assert_eq!(status["revision"], 1);
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
    #[test]
    fn actual_owner_detached_discovery_completion_cannot_revive_cancelled_session() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let discovered = fixture
            .owner
            .capture_connection_close()
            .unwrap()
            .observe()
            .unwrap();
        let desired = fixture.owner.desired().unwrap();
        fixture
            .owner
            .connection_close
            .cancellation
            .as_ref()
            .unwrap()
            .cancel();
        assert!(fixture.owner.retain_connection_close(discovered).is_err());
        assert!(fixture.owner.connection_close.snapshot.is_none());
        assert!(fixture.owner.connection_close.discovery.is_none());
        assert_eq!(fixture.owner.desired().unwrap(), desired);
        assert_eq!(fixture.owner.revision(), 0);
        assert!(!fixture.root.join("r/effects").exists());
    }
    #[test]
    fn actual_host_trait_bridge_reuses_owned_observation_without_effect() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let desired = fixture.owner.desired().unwrap();
        let mut observed =
            LifecycleHost::capture_connection_close(fixture.owner.host_mut(), &desired).unwrap();
        observed.observe().unwrap();
        assert_eq!(fixture.owner.revision(), 0);
        assert_eq!(fixture.owner.desired().unwrap(), desired);
        assert!(!fixture.root.join("r/effects").exists());
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
    fn actual_owner_shared_scheduler_invalidates_capture_but_preserves_exact_replay() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        let digest = MutationDigest::from_semantic_bytes(b"fixed-historical-scheduling-fixture");
        // The typed historical profile/connection alternatives enter this same
        // schedule method after their own retained proof, without ordinary admit.
        let admission = fixture
            .owner
            .schedule(
                MutationKind::Other,
                Some("historical-fixture"),
                Some(0),
                digest,
            )
            .unwrap();
        let Admission::Execute(token) = admission else {
            panic!("expected new scheduling");
        };
        assert!(fixture.owner.connection_close.pending.is_none());
        assert!(fixture.owner.connection_close.snapshot.is_none());
        assert!(
            fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .is_err()
        );
        let cached = fixture
            .owner
            .coordinator
            .finish(token, crate::mutation::MutationResult::NoChange)
            .unwrap();

        let successor = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(successor[1].handle)
            .unwrap();
        assert!(matches!(fixture.owner.schedule(
            MutationKind::Other,
            Some("historical-fixture"),
            Some(0),
            digest,
        ).unwrap(), Admission::Replay(receipt) if receipt == cached));
        let retained = fixture.owner.connection_close.pending.as_ref().unwrap();
        assert_eq!(retained.handle, successor[1].handle);
        assert_eq!(retained.ticket, confirmation.ticket);
        assert!(!fixture.root.join("r/effects").exists());
    }

    #[test]
    fn actual_owner_shared_scheduler_cancels_detached_effect_before_host_publication() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        write(&fixture.root.join("r/stall-read"), b"fixed", 0o600);
        fixture
            .owner
            .confirm_connection_close(
                "close-before-history",
                0,
                rows[0].handle,
                confirmation.ticket,
            )
            .unwrap();
        marker(&fixture.root.join("r/read-entered"));
        let cancellation = fixture
            .owner
            .connection_close
            .cancellation
            .as_ref()
            .unwrap()
            .clone();
        let admission = fixture
            .owner
            .schedule(
                MutationKind::Other,
                Some("historical-after-close"),
                Some(0),
                MutationDigest::from_semantic_bytes(b"fixed-historical-scheduling-after-close"),
            )
            .unwrap();
        let Admission::Execute(token) = admission else {
            panic!("expected new scheduling");
        };
        assert!(cancellation.is_cancelled());
        let cancelled = Instant::now();
        assert_eq!(
            receipt(&mut fixture).outcome,
            ExternalCloseOutcome::RefusedBeforeWrite
        );
        assert!(cancelled.elapsed() < Duration::from_secs(1));
        assert!(!fixture.root.join("r/effects").exists());
        assert!(fixture.owner.desired().unwrap().connected);
        assert_eq!(fixture.owner.actual(), ActualState::Connected);
        fixture
            .owner
            .coordinator
            .abort_active_uncached(token)
            .unwrap();
    }

    #[test]
    fn actual_owner_typed_batch_entry_revokes_capture_but_known_retry_preserves_successor() {
        let _fixtures = FIXTURES.lock().unwrap();
        use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::batch_tests::foreign_empty_context_fixture;
        let (_foreign, mut context) = foreign_empty_context_fixture();
        let mut fixture = fixture("ok");
        let request = |id: &str| json!({"api":"omavless.control","version":1,"id":"fixed-batch-entry","method":"subscriptions.refresh_all","params":{"instanceId":"actual-owner-close-fixture","operationId":id}});
        let rows = snapshot(&mut fixture);
        fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        // The genuine foreign Off witness must refuse. Nevertheless this new
        // mutation intent revokes old close authority BEFORE typed lock/proof
        // admission, through the same body as ordinary batch entry.
        assert!(
            fixture
                .owner
                .start_subscription_batch_research(&request("new-typed-batch"), &mut context)
                .is_err()
        );
        assert!(fixture.owner.connection_close.pending.is_none());
        assert!(fixture.owner.connection_close.snapshot.is_none());
        assert!(!fixture.root.join("r/effects").exists());

        // A real ordinary empty batch creates a retained terminal registry
        // entry. A later exact ID retry must not revoke newer confirmation,
        // even when the supplied typed historical proof itself refuses.
        let known = request("known-empty-batch");
        let job = fixture
            .owner
            .start_subscription_batch(&known)
            .unwrap()
            .unwrap();
        fixture
            .owner
            .complete_subscription_batch(job, || panic!("empty batch must not read clock"))
            .unwrap();
        let successor = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(successor[1].handle)
            .unwrap();
        assert!(
            fixture
                .owner
                .start_subscription_batch_research(&known, &mut context)
                .is_err()
        );
        let pending = fixture.owner.connection_close.pending.as_ref().unwrap();
        assert_eq!(pending.handle, successor[1].handle);
        assert_eq!(pending.ticket, confirmation.ticket);
        assert!(!fixture.root.join("r/effects").exists());
        assert_eq!(fixture.owner.actual(), ActualState::Connected);
    }

    #[test]
    fn actual_owner_typed_batch_entry_cancels_detached_close_before_proof_refusal() {
        let _fixtures = FIXTURES.lock().unwrap();
        use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::batch_tests::foreign_empty_context_fixture;
        let (_foreign, mut context) = foreign_empty_context_fixture();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        write(&fixture.root.join("r/stall-read"), b"fixed", 0o600);
        fixture
            .owner
            .confirm_connection_close(
                "close-before-typed-batch",
                0,
                rows[0].handle,
                confirmation.ticket,
            )
            .unwrap();
        marker(&fixture.root.join("r/read-entered"));
        let cancellation = fixture
            .owner
            .connection_close
            .cancellation
            .as_ref()
            .unwrap()
            .clone();
        let request = json!({"api":"omavless.control","version":1,"id":"fixed-batch-entry","method":"subscriptions.refresh_all","params":{"instanceId":"actual-owner-close-fixture","operationId":"new-typed-after-close"}});
        assert!(
            fixture
                .owner
                .start_subscription_batch_research(&request, &mut context)
                .is_err()
        );
        assert!(cancellation.is_cancelled());
        assert_eq!(
            receipt(&mut fixture).outcome,
            ExternalCloseOutcome::RefusedBeforeWrite
        );
        assert!(!fixture.root.join("r/effects").exists());
        assert!(fixture.owner.desired().unwrap().connected);
        assert_eq!(fixture.owner.actual(), ActualState::Connected);
    }

    #[test]
    fn actual_owner_admitted_startup_invalidates_capture_before_pending_refusal() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        let paths = fixture.owner.transaction.desired_paths().clone();
        write(
            &paths.directory.join("routing-preset.pending.json"),
            b"fixed",
            0o600,
        );
        let lease = fixture.owner.transaction.acquire_lock().unwrap();
        assert!(
            fixture
                .owner
                .reconcile_startup_admitted(
                    &lease,
                    &mut crate::startup_admission::StartupAdmission::ordinary(),
                )
                .is_err()
        );
        assert!(fixture.owner.connection_close.pending.is_none());
        assert!(fixture.owner.connection_close.snapshot.is_none());
        assert!(!fixture.root.join("r/effects").exists());
    }

    #[test]
    fn actual_owner_late_restore_fence_refuses_detached_post_without_context_byte_drift() {
        let _fixtures = FIXTURES.lock().unwrap();
        for (name, entry_type) in [
            "routing-preset.pending.json",
            crate::restore_staging_candidate::PENDING_DIRECTORY,
            "restore-decision.intent",
            "restore-decision.terminal",
            "restore-finalization.pending",
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        ]
        .into_iter()
        .flat_map(|name| ["file", "directory", "symlink"].map(|entry_type| (name, entry_type)))
        {
            let mut fixture = fixture("ok");
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            let desired = fixture.owner.desired().unwrap();
            let config = fs::read(fixture.root.join("c/config.yaml")).unwrap();
            let store = fs::read(fixture.root.join("c/profiles.json")).unwrap();
            write(&fixture.root.join("r/stall-read"), b"fixed", 0o600);
            fixture
                .owner
                .confirm_connection_close(
                    "close-before-fence",
                    0,
                    rows[0].handle,
                    confirmation.ticket,
                )
                .unwrap();
            marker(&fixture.root.join("r/read-entered"));
            // An independently serialized private transaction need not change
            // desired/store/config/ownership bytes or call this owner's admit.
            // Its actual existence fence must still be checked before the POST.
            {
                let _lease = fixture.owner.transaction.acquire_lock().unwrap();
                let member = fixture
                    .owner
                    .transaction
                    .desired_paths()
                    .directory
                    .join(name);
                match entry_type {
                    "file" => write(&member, b"malformed-but-still-a-fence", 0o600),
                    "directory" => fs::create_dir(member).unwrap(),
                    "symlink" => std::os::unix::fs::symlink("missing-member", member).unwrap(),
                    _ => unreachable!(),
                }
            }
            write(&fixture.root.join("r/release"), b"fixed", 0o600);
            let result = receipt(&mut fixture);
            assert_eq!(result.outcome, ExternalCloseOutcome::RefusedBeforeWrite);
            assert_eq!(result.revision, 0);
            assert!(!fixture.root.join("r/effects").exists());
            assert!(fixture.owner.capture_connection_close().is_err());
            assert!(
                fs::symlink_metadata(
                    fixture
                        .owner
                        .transaction
                        .desired_paths()
                        .directory
                        .join(name)
                )
                .is_ok()
            );
            assert_eq!(fixture.owner.desired().unwrap(), desired);
            assert_eq!(
                fs::read(fixture.root.join("c/config.yaml")).unwrap(),
                config
            );
            assert_eq!(
                fs::read(fixture.root.join("c/profiles.json")).unwrap(),
                store
            );
        }
    }

    #[test]
    fn actual_owner_late_restore_fence_after_post_retains_unknown_without_replay_effect() {
        let _fixtures = FIXTURES.lock().unwrap();
        for entry_type in ["file", "directory", "symlink"] {
            let mut fixture = fixture("stall-reply");
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            let desired = fixture.owner.desired().unwrap();
            let config = fs::read(fixture.root.join("c/config.yaml")).unwrap();
            let store = fs::read(fixture.root.join("c/profiles.json")).unwrap();
            fixture
                .owner
                .confirm_connection_close(
                    "fence-after-post",
                    0,
                    rows[0].handle,
                    confirmation.ticket,
                )
                .unwrap();
            marker(&fixture.root.join("r/effect-entered"));
            let member = fixture
                .owner
                .transaction
                .desired_paths()
                .directory
                .join(crate::restore_disposition_complete_model::COMPLETE_MEMBER);
            {
                let _lease = fixture.owner.transaction.acquire_lock().unwrap();
                match entry_type {
                    "file" => write(&member, b"malformed-but-still-a-fence", 0o600),
                    "directory" => fs::create_dir(&member).unwrap(),
                    "symlink" => std::os::unix::fs::symlink("missing-member", &member).unwrap(),
                    _ => unreachable!(),
                }
            }
            write(&fixture.root.join("r/release"), b"fixed", 0o600);
            let result = receipt(&mut fixture);
            assert_eq!(result.outcome, ExternalCloseOutcome::Unknown);
            assert_eq!(result.revision, 1);
            assert_eq!(
                fixture
                    .owner
                    .confirm_connection_close(
                        "fence-after-post",
                        0,
                        rows[0].handle,
                        confirmation.ticket,
                    )
                    .unwrap(),
                Some(result)
            );
            assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
            assert!(fixture.owner.capture_connection_close().is_err());
            assert!(fs::symlink_metadata(&member).is_ok());
            assert_eq!(fixture.owner.desired().unwrap(), desired);
            assert_eq!(
                fs::read(fixture.root.join("c/config.yaml")).unwrap(),
                config
            );
            assert_eq!(
                fs::read(fixture.root.join("c/profiles.json")).unwrap(),
                store
            );
        }
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
    fn long_request(method: &str, id: &str, revision: u64) -> Value {
        omavless_control_protocol::make_request("fixed-long", method,
            json!({"instanceId":"actual-owner-close-fixture", "operationId":id, "expectedRevision":revision})).unwrap()
    }

    #[test]
    fn actual_owner_close_ids_conflict_with_ordinary_batch_probe_and_provider() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        fixture
            .owner
            .confirm_connection_close("common-id", 0, rows[0].handle, confirmation.ticket)
            .unwrap();
        let closed = receipt(&mut fixture);
        assert!(matches!(
            disconnect(&mut fixture.owner, "common-id"),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        ));
        let request = long_request("subscriptions.refresh_all", "common-id", 1);
        assert!(matches!(
            fixture.owner.start_subscription_batch(&request),
            Err(NativeOwnerError::LongOperation(
                LongOperationError::OperationConflict
            ))
        ));
        let request = long_request("profiles.probe", "common-id", 1);
        assert!(matches!(
            fixture.owner.start_subscription_probe(&request),
            Err(NativeOwnerError::LongOperation(
                LongOperationError::OperationConflict
            ))
        ));
        let request = long_request("routing.refresh_providers", "common-id", 1);
        assert!(matches!(
            fixture.owner.preflight_provider_refresh(&request),
            Err(NativeOwnerError::LongOperation(
                LongOperationError::OperationConflict
            ))
        ));
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close("common-id", 0, rows[0].handle, confirmation.ticket)
                .unwrap(),
            Some(closed)
        );
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    }

    #[test]
    fn actual_owner_ordinary_batch_and_probe_ids_cannot_be_reused_for_close() {
        let _fixtures = FIXTURES.lock().unwrap();
        for method in [
            "connection.disconnect",
            "subscriptions.refresh_all",
            "profiles.probe",
        ] {
            let mut fixture = fixture("ok");
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            match method {
                "connection.disconnect" => {
                    disconnect(&mut fixture.owner, "first-family").unwrap();
                }
                "subscriptions.refresh_all" => {
                    assert!(
                        fixture
                            .owner
                            .start_subscription_batch(&long_request(method, "first-family", 0))
                            .unwrap()
                            .is_some()
                    );
                }
                "profiles.probe" => {
                    assert!(
                        fixture
                            .owner
                            .start_subscription_probe(&long_request(method, "first-family", 0))
                            .unwrap()
                            .is_some()
                    );
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    fixture.owner.confirm_connection_close(
                        "first-family",
                        0,
                        rows[0].handle,
                        confirmation.ticket
                    ),
                    Err(NativeOwnerError::Coordinator(
                        CoordinatorError::OperationConflict
                    ))
                ),
                "{method}"
            );
            assert!(!fixture.root.join("r/effects").exists(), "{method}");
        }
    }

    #[test]
    fn actual_owner_long_admission_cancels_before_contended_migration_lease() {
        let _fixtures = FIXTURES.lock().unwrap();
        for method in [
            "subscriptions.refresh_all",
            "profiles.probe",
            "routing.refresh_providers",
            "shutdown",
            "recovery",
        ] {
            let mut fixture = fixture("ok");
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            write(&fixture.root.join("r/stall-read"), b"fixed", 0o600);
            fixture
                .owner
                .confirm_connection_close("stalled-close", 0, rows[0].handle, confirmation.ticket)
                .unwrap();
            marker(&fixture.root.join("r/read-entered"));
            let paths = fixture.owner.transaction.cutover_paths().clone();
            let lease = MigrationLock::acquire(&paths, fixture.owner.uid()).unwrap();
            let start = Instant::now();
            let request = long_request(method, "competing-family", 0);
            match method {
                "subscriptions.refresh_all" => {
                    assert!(fixture.owner.start_subscription_batch(&request).is_err());
                }
                "profiles.probe" => {
                    assert!(fixture.owner.start_subscription_probe(&request).is_err());
                }
                "routing.refresh_providers" => {
                    assert!(fixture.owner.preflight_provider_refresh(&request).is_err());
                }
                "shutdown" => {
                    let _ = fixture.owner.stop_batch_operations();
                }
                "recovery" => fixture.owner.mark_auxiliary_recovery_required(),
                _ => unreachable!(),
            }
            assert!(start.elapsed() < Duration::from_millis(250), "{method}");
            drop(lease);
            assert_eq!(
                receipt(&mut fixture).outcome,
                ExternalCloseOutcome::RefusedBeforeWrite,
                "{method}"
            );
            assert!(fixture.owner.desired().unwrap().connected, "{method}");
            assert!(!fixture.root.join("r/effects").exists(), "{method}");
        }
    }

    #[test]
    fn actual_owner_disconnect_drains_exact_proof_lease_or_returns_bounded_busy() {
        let _fixtures = FIXTURES.lock().unwrap();
        for (partial, timeout) in [(false, false), (false, true), (true, false)] {
            let mut fixture = fixture("ok");
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let session = fixture
                .owner
                .connection_close
                .snapshot
                .as_mut()
                .unwrap()
                .observation
                .session_mut();
            session.pause_proof_after(usize::from(partial), Arc::clone(&barrier));
            if partial {
                session.partial_effect_chunks(16);
            }
            fixture
                .owner
                .confirm_connection_close("proof-close", 0, rows[0].handle, confirmation.ticket)
                .unwrap();
            barrier.wait(); // real migration lease acquired, before effect gate
            if partial {
                marker(&fixture.root.join("r/partial-entered"));
                let prefix = fs::read(fixture.root.join("r/partial-entered")).unwrap();
                assert_eq!(prefix.len(), 16);
                assert!(prefix.starts_with(b"POST "));
            }
            let cancellation = fixture
                .owner
                .connection_close
                .cancellation
                .as_ref()
                .unwrap()
                .clone();
            let owner = std::thread::spawn(move || {
                let start = Instant::now();
                let result = disconnect(&mut fixture.owner, "proof-disconnect");
                (fixture, result, start.elapsed())
            });
            let deadline = Instant::now() + Duration::from_secs(1);
            while !cancellation.is_cancelled() {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            if !timeout {
                // The owner is waiting only for this exact proof lease, not
                // holding the lifetime gate or waiting on controller I/O.
                std::thread::sleep(Duration::from_millis(5));
                assert!(!owner.is_finished());
                barrier.wait();
            }
            let (mut fixture, result, elapsed) = owner.join().unwrap();
            assert!(elapsed < Duration::from_millis(250));
            if timeout {
                assert!(matches!(result, Err(NativeOwnerError::OwnershipBusy)));
                assert!(fixture.owner.desired().unwrap().connected);
                assert_eq!(fixture.owner.actual(), ActualState::Connected);
                assert!(!fixture.root.join("r/effects").exists());
                barrier.wait();
            } else {
                assert!(matches!(
                    result.unwrap(),
                    NativeOwnerExecution::Applied { outcome: Ok(_), .. }
                ));
                assert!(!fixture.owner.desired().unwrap().connected);
            }
            let result = receipt(&mut fixture);
            assert_eq!(
                result.outcome,
                if partial {
                    ExternalCloseOutcome::Unknown
                } else {
                    ExternalCloseOutcome::RefusedBeforeWrite
                }
            );
            if timeout {
                assert!(matches!(
                    disconnect(&mut fixture.owner, "proof-disconnect").unwrap(),
                    NativeOwnerExecution::Applied { outcome: Ok(_), .. }
                ));
            }
            assert_eq!(fixture.owner.revision(), 1);
            assert_eq!(
                fixture
                    .owner
                    .confirm_connection_close("proof-close", 0, rows[0].handle, confirmation.ticket)
                    .unwrap(),
                Some(result)
            );
            assert!(!fixture.root.join("r/effects").exists());
        }
    }

    #[test]
    fn normal_compiled_scheduler_refuses_a_replaced_admission_lease() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        let lease = fixture.owner.batch_lock().unwrap();
        let mut captured = fixture.owner.connection_close.snapshot.take().unwrap();
        let selected = captured
            .rows
            .iter_mut()
            .find(|row| row.handle == rows[0].handle)
            .and_then(|row| row.observed.take())
            .unwrap();
        let permit = captured.observation.fixture_permit().unwrap();
        let lock_path = &captured.context.cutover_paths.operation_lock;
        fs::rename(lock_path, lock_path.with_extension("original-held")).unwrap();
        write(lock_path, b"", 0o600);
        // This token is an actual reserved receipt, not caller-injected facts.
        let token = match fixture
            .owner
            .coordinator
            .reserve_external_close(
                "replaced-lease-close",
                0,
                MutationDigest::from_semantic_bytes(b"fixed synthetic lease test"),
                false,
            )
            .unwrap()
        {
            ExternalCloseAdmission::Reserved(token) => token,
            ExternalCloseAdmission::Replay(_) => panic!("fresh fixed operation must reserve"),
        };
        assert!(matches!(
            fixture
                .owner
                .schedule_permitted_connection_close(captured, selected, &token, permit, &lease,),
            Err(NativeOwnerError::OwnershipUnavailable)
        ));
        assert!(fixture.owner.connection_close.active.is_none());
        assert!(!fixture.root.join("r/effects").exists());
        assert_eq!(fixture.owner.revision(), 0);
        assert!(fixture.owner.desired().unwrap().connected);
    }

    #[test]
    fn actual_owned_bytes_and_abi_do_not_mint_normal_package_attestation() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("passive-ok");
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        let result = fixture
            .owner
            .confirm_connection_close("passive-close", 0, rows[0].handle, confirmation.ticket)
            .unwrap()
            .unwrap();
        assert_eq!(
            result,
            ExternalCloseReceipt {
                outcome: ExternalCloseOutcome::MissingAttestation,
                revision: 0
            }
        );
        assert!(!fixture.root.join("r/effects").exists());
        fixture.owner.invalidate_connection_close();
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close("passive-close", 0, rows[0].handle, confirmation.ticket)
                .unwrap(),
            Some(result)
        );
    }

    #[test]
    fn actual_owner_confirmation_after_old_discovery_budget_does_not_renew_expiry() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let rows = snapshot(&mut fixture);
        let original = fixture
            .owner
            .connection_close
            .snapshot
            .as_ref()
            .unwrap()
            .expiry;
        std::thread::sleep(Duration::from_millis(3100));
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
            original
        );
        fixture
            .owner
            .confirm_connection_close("late-confirm", 0, rows[0].handle, confirmation.ticket)
            .unwrap();
        assert_eq!(receipt(&mut fixture).outcome, ExternalCloseOutcome::Closed);
        assert_eq!(fs::read(fixture.root.join("r/effects")).unwrap(), b"1\n");
    }

    #[test]
    fn actual_owner_same_pid_exec_permanently_revokes_old_image_binding() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("ok");
        let pid = fixture.owner.host().core_pid().unwrap();
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        write(&fixture.root.join("r/exec-replace"), b"fixed", 0o600);
        fixture
            .owner
            .confirm_connection_close("exec-close", 0, rows[0].handle, confirmation.ticket)
            .unwrap();
        assert_eq!(
            receipt(&mut fixture).outcome,
            ExternalCloseOutcome::RefusedBeforeWrite
        );
        assert_eq!(fixture.owner.host().core_pid(), Some(pid));
        assert!(fixture.owner.capture_connection_close().is_err());
        assert!(!fixture.root.join("r/effects").exists());
    }

    fn provider_job(fixture: &mut Fixture, id: &str) -> NativeProviderRefresh {
        use crate::provider_refresh::RuleProviderTransport;
        let request = long_request("routing.refresh_providers", id, fixture.owner.revision());
        let ProviderRefreshAdmission::Discover(snapshot) =
            fixture.owner.preflight_provider_refresh(&request).unwrap()
        else {
            panic!("actual provider discovery expected");
        };
        let transport = crate::provider_refresh::UnixRuleProviderTransport::new(
            &fixture.root.join("r"),
            fixture.owner.uid(),
        );
        let targets = transport.discover(Duration::from_secs(1)).unwrap();
        assert_eq!(targets.len(), 1);
        fixture
            .owner
            .start_provider_refresh(&request, snapshot, targets)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn actual_owned_rule_provider_reservation_conflicts_with_close_id() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("rule-ok");
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        let _job = provider_job(&mut fixture, "provider-first");
        assert!(matches!(
            fixture.owner.confirm_connection_close(
                "provider-first",
                0,
                rows[0].handle,
                confirmation.ticket
            ),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        ));
        assert!(!fixture.root.join("r/effects").exists());
    }

    #[test]
    fn actual_owner_late_long_completions_cancel_before_publication_and_cannot_restore_state() {
        let _fixtures = FIXTURES.lock().unwrap();
        for method in [
            "subscriptions.refresh_all",
            "profiles.probe",
            "routing.refresh_providers",
        ] {
            let mut fixture = fixture(if method == "routing.refresh_providers" {
                "rule-ok"
            } else {
                "ok"
            });
            let request = long_request(method, "old-work", 0);
            let mut batch = None;
            let mut probe = None;
            let mut provider = None;
            let ticket = match method {
                "subscriptions.refresh_all" => {
                    let job = fixture
                        .owner
                        .start_subscription_batch(&request)
                        .unwrap()
                        .unwrap();
                    let ticket = job.supervisor_ticket();
                    batch = Some(job);
                    ticket
                }
                "profiles.probe" => {
                    let job = fixture
                        .owner
                        .start_subscription_probe(&request)
                        .unwrap()
                        .unwrap();
                    let ticket = job.supervisor_ticket();
                    probe = Some(job);
                    ticket
                }
                "routing.refresh_providers" => {
                    let job = provider_job(&mut fixture, "old-work");
                    let ticket = job.supervisor_ticket();
                    provider = Some(job);
                    ticket
                }
                _ => unreachable!(),
            };
            fixture.owner.abort_subscription_batch(ticket).unwrap();
            let desired = fixture.owner.desired().unwrap();
            let store = fs::read(fixture.root.join("c/profiles.json")).unwrap();
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            write(&fixture.root.join("r/stall-read"), b"fixed", 0o600);
            fixture
                .owner
                .confirm_connection_close("new-close", 0, rows[0].handle, confirmation.ticket)
                .unwrap();
            marker(&fixture.root.join("r/read-entered"));
            match method {
                "subscriptions.refresh_all" => assert!(
                    fixture
                        .owner
                        .complete_subscription_batch(batch.take().unwrap(), || panic!(
                            "stale job clock"
                        ))
                        .is_err()
                ),
                "profiles.probe" => assert!(
                    fixture
                        .owner
                        .complete_subscription_probe(
                            probe.take().unwrap(),
                            Err(StableErrorCode::InternalError)
                        )
                        .is_err()
                ),
                "routing.refresh_providers" => assert!(
                    fixture
                        .owner
                        .complete_provider_refresh(
                            provider.take().unwrap(),
                            || panic!("stale job clock"),
                            || panic!("stale identity callback")
                        )
                        .is_err()
                ),
                _ => unreachable!(),
            }
            assert_eq!(
                receipt(&mut fixture).outcome,
                ExternalCloseOutcome::RefusedBeforeWrite,
                "{method}"
            );
            assert_eq!(fixture.owner.revision(), 0);
            assert_eq!(fixture.owner.desired().unwrap(), desired);
            assert_eq!(
                fs::read(fixture.root.join("c/profiles.json")).unwrap(),
                store
            );
            assert!(!fixture.root.join("r/effects").exists());
        }
    }

    #[test]
    fn actual_owner_composed_core_selected_close_optin() {
        let Some(executable) = std::env::var_os("OMAVLESS_TEST_OWNER_CONDITIONAL_CORE") else {
            return;
        };
        composed_core_selected_close(PathBuf::from(executable), false, false, false);
    }

    #[cfg(feature = "developer-conditional-close")]
    #[test]
    #[ignore = "exclusive Dev-VM lease; separately root-provisioned exact developer pair"]
    fn actual_owner_developer_pair_selected_close_in_dev_vm() {
        assert_eq!(
            std::env::var("OMAVLESS_CLOSE_DEVELOPER_PAIR_VM").as_deref(),
            Ok("1")
        );
        assert_ne!(nix::unistd::getuid().as_raw(), 0);
        composed_core_selected_close(
            PathBuf::from("/var/lib/omavless-close-development-pair/mihomo"),
            true,
            false,
            false,
        );
    }

    #[cfg(feature = "developer-conditional-close")]
    #[test]
    #[ignore = "exclusive Dev-VM namespace lease; exact root-provisioned developer pair"]
    fn actual_owner_developer_pair_socket_selected_close_in_dev_vm() {
        assert_eq!(
            std::env::var("OMAVLESS_CLOSE_DEVELOPER_SOCKET_VM").as_deref(),
            Ok("1")
        );
        assert_ne!(nix::unistd::getuid().as_raw(), 0);
        composed_core_selected_close(
            PathBuf::from("/var/lib/omavless-close-development-pair/mihomo"),
            true,
            false,
            true,
        );
    }

    #[cfg(feature = "developer-conditional-close")]
    #[test]
    #[ignore = "root disposable PID/mount/network namespace; separately admitted private pair copy"]
    fn actual_owner_developer_pair_rebind_restoration_in_dev_vm() {
        assert_eq!(
            std::env::var("OMAVLESS_CLOSE_DEVELOPER_PAIR_DRIFT_VM").as_deref(),
            Ok("1")
        );
        assert_eq!(nix::unistd::getuid().as_raw(), 0);
        assert_eq!(nix::unistd::getpid().as_raw(), 1);
        let pair = Path::new("/var/lib/omavless-close-development-pair");
        let marker = pair.join("disposable-rebind-scope");
        let metadata = fs::symlink_metadata(&marker).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert_eq!(
            (metadata.uid(), metadata.gid(), metadata.nlink()),
            (0, 0, 1)
        );
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(
            fs::read(marker).unwrap(),
            b"root-owned-disposable-pair-rebind-v1\n"
        );
        composed_core_selected_close(pair.join("mihomo"), true, true, false);
    }

    fn composed_core_selected_close(
        executable: PathBuf,
        developer_pair: bool,
        rebind: bool,
        socket_workspace: bool,
    ) {
        composed_core_selected_close_with_client(
            executable,
            developer_pair,
            rebind,
            socket_workspace,
            false,
            false,
            false,
        );
    }

    fn composed_core_selected_close_with_client(
        executable: PathBuf,
        developer_pair: bool,
        rebind: bool,
        socket_workspace: bool,
        client_workspace: bool,
        real_cli: bool,
        qualified_pair: bool,
    ) {
        use sha2::{Digest, Sha256};
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        let _fixtures = FIXTURES.lock().unwrap();
        let metadata = fs::symlink_metadata(&executable).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.mode() & 0o022, 0);
        let expected = if qualified_pair {
            assert_eq!(executable, Path::new(crate::managed_pair::RELEASE_CORE));
            let expected = std::env::var("OMAVLESS_CLOSE_QUALIFIED_CORE_SHA").unwrap();
            assert!(
                expected.len() == 64
                    && expected
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
            expected
        } else {
            "3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544".to_owned()
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(&executable).unwrap())),
            expected
        );
        let after = fs::symlink_metadata(&executable).unwrap();
        assert_eq!(
            (
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
                metadata.len()
            ),
            (
                after.dev(),
                after.ino(),
                after.ctime(),
                after.ctime_nsec(),
                after.len()
            )
        );
        let mut fixture = if socket_workspace {
            let (root, owner) = fixture_parts("ok", None);
            Fixture {
                root,
                owner: FixtureCoordinator(Some(owner)),
            }
        } else {
            fixture("ok")
        };
        fixture.owner.invalidate_connection_close();
        fixture.owner.host_mut().stop_owned().unwrap();
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let mixed = reservation.local_addr().unwrap().port();
        let mut listeners = vec![
            TcpListener::bind(if real_cli {
                "127.0.0.1:19180"
            } else {
                "127.0.0.1:0"
            })
            .unwrap(),
        ];
        let target = listeners[0].local_addr().unwrap().port();
        if client_workspace || real_cli {
            assert!(developer_pair && socket_workspace && !rebind);
            listeners.push(
                TcpListener::bind(if real_cli {
                    "127.0.0.1:19181"
                } else {
                    "127.0.0.1:0"
                })
                .unwrap(),
            );
        }
        let targets = [
            target,
            listeners.last().unwrap().local_addr().unwrap().port(),
        ];
        assert_ne!(mixed, target);
        drop(reservation);
        for listener in &listeners {
            listener.set_nonblocking(true).unwrap();
        }
        let echo = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut peers = Vec::new();
            while peers.len() < 2 && Instant::now() < deadline {
                for listener in &listeners {
                    let Ok((mut peer, _)) = listener.accept() else {
                        continue;
                    };
                    assert!(peers.len() < 2);
                    peers.push(std::thread::spawn(move || {
                        peer.set_read_timeout(Some(Duration::from_secs(if real_cli {
                            900
                        } else if client_workspace {
                            20
                        } else {
                            5
                        })))
                        .unwrap();
                        let mut bytes = [0; 256];
                        loop {
                            let count = match peer.read(&mut bytes) {
                                Ok(count) => count,
                                Err(error)
                                    if real_cli
                                        && matches!(
                                            error.kind(),
                                            std::io::ErrorKind::TimedOut
                                                | std::io::ErrorKind::WouldBlock
                                        ) =>
                                {
                                    // Interactive timeout is unavailable, not
                                    // permission to release this original peer.
                                    loop {
                                        std::thread::sleep(Duration::from_secs(60));
                                    }
                                }
                                Err(_) => break,
                            };
                            if count == 0 || peer.write_all(&bytes[..count]).is_err() {
                                break;
                            }
                        }
                    }));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(peers.len(), 2);
            for peer in peers {
                peer.join().unwrap();
            }
        });
        let runtime = fixture.root.join("r");
        let config = fixture.root.join("c");
        if qualified_pair {
            assert!(
                developer_pair && socket_workspace && !rebind && !client_workspace && !real_cli
            );
            // Establish this fixture-local user selection BEFORE the first
            // adoption observation. It does not create any root package object.
            write(
                &config.join(crate::managed_pair::SELECTOR),
                crate::managed_pair::SELECTION_BYTES,
                0o600,
            );
        }
        let socket = runtime.join("mihomo.sock");
        let paths = NativeHostPaths::new(
            executable.clone(),
            config.clone(),
            config.clone(),
            runtime.clone(),
            PathBuf::from("/proc"),
            PathBuf::from("/sys/class/net"),
        );
        // Drop the stopped predecessor before the successor creates its socket.
        // Host cleanup deliberately still owns this private controller path.
        *fixture.owner.host_mut() = NativeLifecycleHost::new(paths, fixture.owner.uid()).unwrap();
        write(&config.join("config.yaml"), format!("mixed-port: {mixed}\nexternal-controller-unix: {}\nallow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\ntun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n", socket.display()).as_bytes(), 0o600);
        let mut core =
            OwnedCore::spawn(&executable, &runtime, &config.join("config.yaml"), &socket).unwrap();
        core.wait_ready(Duration::from_secs(10)).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let loaded = Instant::now() + Duration::from_secs(5);
        loop {
            let result = omavless_mihomo::controller_get(
                &socket,
                omavless_mihomo::ReadOnlyEndpoint::Configs,
                Duration::from_millis(250),
                16384,
            );
            if result.is_ok_and(|r| {
                r.status == 200
                    && r.payload["mode"] == "direct"
                    && r.payload["mixed-port"] == mixed
                    && r.payload["tun"]["enable"] == false
            }) {
                break;
            }
            assert!(Instant::now() < loaded);
            std::thread::sleep(Duration::from_millis(5));
        }
        if developer_pair {
            #[cfg(feature = "developer-conditional-close")]
            fixture
                .owner
                .host_mut()
                .install_passive_owned_close_fixture(core)
                .unwrap();
            #[cfg(not(feature = "developer-conditional-close"))]
            panic!("developer pair requires explicit feature");
        } else {
            fixture
                .owner
                .host_mut()
                .install_owned_close_fixture(core)
                .unwrap();
        }
        fixture
            .owner
            .transaction
            .lifecycle_mut()
            .adopt_owned_close_fixture()
            .unwrap();
        let mut clients = Vec::new();
        for target in targets {
            let mut client = TcpStream::connect(("127.0.0.1", mixed)).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            if client_workspace || real_cli {
                client
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
            }
            client
                .write_all(
                    format!(
                        "CONNECT 127.0.0.1:{target} HTTP/1.1\r\nHost: 127.0.0.1:{target}\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                client.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 8192);
            }
            assert!(header.starts_with(b"HTTP/1.1 200 "));
            client.write_all(b"before").unwrap();
            let mut bytes = [0; 6];
            client.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"before");
            clients.push(client);
        }
        let desired = fixture.owner.desired().unwrap();
        if socket_workspace {
            #[cfg(feature = "developer-conditional-close")]
            {
                assert!(developer_pair && !rebind);
                // Passive owned host has no fixture permit. Its development
                // authority is instead the separately admitted exact pair.
                let observation = fixture
                    .owner
                    .host_mut()
                    .capture_connection_close(&desired)
                    .unwrap();
                assert!(observation.fixture_permit().is_none());
                drop(observation);
                let fixture = SocketFixture::from_fixture(fixture);
                let before = fixture.desired_bytes();
                if real_cli {
                    #[cfg(all(feature = "developer-conditional-close", feature = "tui"))]
                    {
                        let mut original_foot = None;
                        let mut completion_file = None;
                        // Failure keeps the SAME original server/core/streams.
                        // No kill/retry/Drop cleanup; ROOT may administer this
                        // disposable unavailable scope separately.
                        if exercise_real_cli_foot(
                            &fixture,
                            &mut clients,
                            targets,
                            &mut original_foot,
                            &mut completion_file,
                        )
                        .is_err()
                        {
                            eprintln!("T3_REAL_CLI_FOOT_UNAVAILABLE");
                            loop {
                                std::thread::sleep(Duration::from_secs(60));
                            }
                        }
                        assert!(fixture.desired_bytes() == before);
                        drop(clients);
                        drop(fixture);
                        echo.join().unwrap();
                        return;
                    }
                    #[cfg(not(all(feature = "developer-conditional-close", feature = "tui")))]
                    panic!("real CLI requires both explicit features");
                }
                if client_workspace {
                    #[cfg(all(feature = "developer-conditional-close", feature = "tui"))]
                    {
                        exercise_actual_tui_workspace(&fixture, &mut clients, targets);
                        assert!(fixture.desired_bytes() == before);
                        drop(clients);
                        drop(fixture);
                        echo.join().unwrap();
                        return;
                    }
                    #[cfg(not(all(feature = "developer-conditional-close", feature = "tui")))]
                    panic!("development client requires both explicit features");
                }
                let params = fixture.confirmation("real-socket-selected-close");
                let admitted = fixture.call("development.connections.confirm", params.clone());
                assert_eq!(admitted["ok"], true);
                let result = fixture.receipt("real-socket-selected-close");
                assert_eq!(result["result"]["outcome"], "closed");
                assert_eq!(result["result"]["receiptRevision"], 1);
                assert_eq!(result["revision"], 1);
                let replay = fixture.call("development.connections.confirm", params);
                assert_eq!(replay["ok"], true);
                assert_eq!(replay["result"], result["result"]);
                assert_eq!(fixture.desired_bytes(), before);
                assert_one_survivor(&mut clients);
                drop(clients);
                drop(fixture); // Original server/host teardown precedes echo join.
                echo.join().unwrap();
                return;
            }
            #[cfg(not(feature = "developer-conditional-close"))]
            panic!("development socket requires explicit feature");
        }
        let rows = snapshot(&mut fixture);
        assert_eq!(rows.len(), 2);
        if developer_pair {
            assert!(
                fixture
                    .owner
                    .connection_close
                    .snapshot
                    .as_ref()
                    .unwrap()
                    .observation
                    .fixture_permit()
                    .is_none()
            );
        }
        assert_eq!(rows[0].display, rows[1].display);
        assert_ne!(rows[0].handle, rows[1].handle);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        if rebind {
            // Test-only root administration in the explicitly admitted fresh
            // mount namespace. The broker is NOT running; no production API
            // accepts paths, mounts or authority from this fixture.
            use nix::mount::{MsFlags, mount, umount};
            let pair = executable.parent().unwrap();
            let broker = pair.join("omavless-dns-broker");
            let replacement = pair.join("broker-replacement");
            let identity = |path: &Path| {
                let m = fs::symlink_metadata(path).unwrap();
                (
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.uid(),
                    m.gid(),
                    m.nlink(),
                    m.len(),
                    m.ctime(),
                    m.ctime_nsec(),
                    m.mtime(),
                    m.mtime_nsec(),
                )
            };
            let original = identity(&broker);
            let replacement_identity = identity(&replacement);
            assert_eq!(
                (
                    replacement_identity.3,
                    replacement_identity.4,
                    replacement_identity.5
                ),
                (0, 0, 1)
            );
            assert_eq!(replacement_identity.2 & 0o7777, 0o755);
            assert_ne!(
                (original.0, original.1),
                (replacement_identity.0, replacement_identity.1)
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(fs::read(&replacement).unwrap())),
                "ea958302d745b901294df6164c624a431a7493b67457a255306ec8216545eb9d"
            );
            let session = fixture
                .owner
                .connection_close
                .snapshot
                .as_mut()
                .unwrap()
                .observation
                .session_mut();
            assert!(session.proves_live());
            mount(
                Some(replacement.as_path()),
                broker.as_path(),
                None::<&str>,
                MsFlags::MS_BIND,
                None::<&str>,
            )
            .unwrap();
            assert_eq!(identity(&broker), replacement_identity);
            assert!(!session.proves_live());
            umount(broker.as_path()).unwrap();
            // Restore the actual ORIGINAL object, including ctime/mtime: a
            // filename/copy approximation could not prove sticky revocation.
            assert_eq!(identity(&broker), original);
            assert!(!session.proves_live());
            assert!(matches!(
                fixture.owner.confirm_connection_close(
                    "real-rebind-refusal",
                    0,
                    rows[0].handle,
                    confirmation.ticket
                ),
                Err(NativeOwnerError::OwnershipUnavailable)
            ));
            let refused = ExternalCloseReceipt {
                outcome: ExternalCloseOutcome::RefusedBeforeWrite,
                revision: 0,
            };
            assert_eq!(
                fixture
                    .owner
                    .confirm_connection_close(
                        "real-rebind-refusal",
                        0,
                        rows[0].handle,
                        confirmation.ticket
                    )
                    .unwrap(),
                Some(refused)
            );
            assert_eq!(fixture.owner.desired().unwrap(), desired);
            for client in &mut clients {
                client.write_all(b"after").unwrap();
                let mut bytes = [0; 5];
                client.read_exact(&mut bytes).unwrap();
                assert_eq!(&bytes, b"after");
            }
            drop(clients);
            fixture.owner.host_mut().stop_owned().unwrap();
            echo.join().unwrap();
            return;
        }
        fixture
            .owner
            .confirm_connection_close(
                "real-selected-close",
                0,
                rows[0].handle,
                confirmation.ticket,
            )
            .unwrap();
        let closed = receipt(&mut fixture);
        assert_eq!(
            closed,
            ExternalCloseReceipt {
                outcome: ExternalCloseOutcome::Closed,
                revision: 1
            }
        );
        assert_eq!(fixture.owner.desired().unwrap(), desired);
        fixture.owner.invalidate_connection_close();
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close(
                    "real-selected-close",
                    0,
                    rows[0].handle,
                    confirmation.ticket
                )
                .unwrap(),
            Some(closed)
        );
        assert_one_survivor(&mut clients);
        drop(clients);
        fixture.owner.host_mut().stop_owned().unwrap();
        echo.join().unwrap();
    }

    fn assert_one_survivor(clients: &mut [std::net::TcpStream]) {
        use std::io::{Read, Write};
        let mut alive = 0;
        let mut terminated = 0;
        for client in clients {
            let _ = client.write_all(b"after");
            let mut bytes = [0; 5];
            match client.read(&mut bytes) {
                Ok(0) => terminated += 1,
                Ok(n) => {
                    client.read_exact(&mut bytes[n..]).unwrap();
                    assert_eq!(&bytes, b"after");
                    alive += 1;
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                    ) =>
                {
                    terminated += 1
                }
                Err(_) => panic!("closed tunnel must terminate; unrelated tunnel must echo"),
            }
        }
        assert_eq!((terminated, alive), (1, 1));
    }

    #[test]
    fn actual_owner_128_uncertain_receipts_never_evict_or_admit_129th_effect() {
        let _fixtures = FIXTURES.lock().unwrap();
        let mut fixture = fixture("drop");
        let mut first = None;
        for index in 0..128 {
            let rows = snapshot(&mut fixture);
            let confirmation = fixture
                .owner
                .prepare_connection_close(rows[0].handle)
                .unwrap();
            let id = format!("bounded-close-{index}");
            let revision = fixture.owner.revision();
            fixture
                .owner
                .confirm_connection_close(&id, revision, rows[0].handle, confirmation.ticket)
                .unwrap();
            // A pending reservation is non-evicting too, and does not grant
            // another operation authority merely because no receipt exists.
            assert!(matches!(
                fixture.owner.confirm_connection_close(
                    "parallel-close",
                    revision,
                    rows[0].handle,
                    confirmation.ticket
                ),
                Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
            ));
            let result = receipt(&mut fixture);
            assert_eq!(result.outcome, ExternalCloseOutcome::Unknown);
            if index == 0 {
                first = Some((rows[0].handle, confirmation.ticket, result));
            }
        }
        let rows = snapshot(&mut fixture);
        let confirmation = fixture
            .owner
            .prepare_connection_close(rows[0].handle)
            .unwrap();
        assert!(matches!(
            fixture.owner.confirm_connection_close(
                "overflow-close",
                128,
                rows[0].handle,
                confirmation.ticket
            ),
            Err(NativeOwnerError::Coordinator(CoordinatorError::Busy))
        ));
        fixture.owner.invalidate_connection_close();
        let (handle, ticket, original) = first.unwrap();
        assert_eq!(
            fixture
                .owner
                .confirm_connection_close("bounded-close-0", 0, handle, ticket)
                .unwrap(),
            Some(original)
        );
        assert_eq!(fixture.owner.revision(), 128);
        assert_eq!(
            fs::read(fixture.root.join("r/effects")).unwrap(),
            b"1\n".repeat(128)
        );
        assert!(fixture.owner.desired().unwrap().connected);
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
            || crate::pending_private_transaction::pending(&context.desired_paths)
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
#[cfg(feature = "developer-conditional-close")]
impl OpaqueToken {
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        if value.len() != 64 {
            return None;
        }
        let mut bytes = [0; 32];
        let digit = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
        }
        (bytes != [0; 32]).then_some(Self(bytes))
    }

    pub(crate) fn to_wire(self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(64);
        for byte in self.0 {
            text.push(char::from(DIGITS[usize::from(byte >> 4)]));
            text.push(char::from(DIGITS[usize::from(byte & 15)]));
        }
        text
    }
}
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
        if let Some(cancellation) = self.cancellation.take()
            && !cancellation.cancel_and_drain()
        {
            // Keep exact cancellation authority for a later explicit
            // retry if bounded proof draining timed out.
            self.cancellation = Some(cancellation);
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
        // Full developer-object filesystem proof belongs to detached discovery,
        // never its owner-held completion. It uses the original counted proof
        // flight; cancellation and the absolute expiry still win at retention.
        if Instant::now() >= self.expiry || !self.observation.session_mut().proves_live() {
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

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
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
            || crate::pending_private_transaction::pending(self.transaction.desired_paths())
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
            || !discovered
                .observation
                .session_mut()
                .proves_live_for_scheduling()
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
        if Instant::now() >= snapshot.expiry
            || !snapshot
                .observation
                .session_mut()
                .proves_live_for_scheduling()
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let selected = snapshot
            .rows
            .iter_mut()
            .find(|row| row.handle == handle)
            .and_then(|row| row.observed.take())
            .ok_or(NativeOwnerError::RecordNotFound)?;
        // The opt-in development pair is a distinct root-admin object policy,
        // not an adoption of the old passive source receipt or released pair.
        // The distinct close-qualified package evidence is independently
        // retained by this SAME original Session; an error cannot fall back.
        #[cfg(feature = "developer-conditional-close")]
        if let Some(permit) = snapshot
            .observation
            .session_mut()
            .qualified_pair_permit()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?
        {
            return self
                .schedule_permitted_connection_close(snapshot, selected, token, permit, &_lease);
        }
        #[cfg(feature = "developer-conditional-close")]
        if let Some(permit) = snapshot.observation.session_mut().developer_pair_permit() {
            return self
                .schedule_permitted_connection_close(snapshot, selected, token, permit, &_lease);
        }
        // Default builds still admit effects only via the cfg(test) fixture.
        #[cfg(test)]
        if let Some(permit) = snapshot.observation.fixture_permit() {
            return self
                .schedule_permitted_connection_close(snapshot, selected, token, permit, &_lease);
        }
        let _ = (selected, token);
        Ok(Some(ExternalCloseOutcome::MissingAttestation))
    }

    // The actual scheduling transition compiles identically in normal builds.
    // Default builds have no production permit constructor. The separate opt-in
    // developer pair does not adopt passive receipts or release-package authority.
    // Both callers retain the checked admission lease across this call;
    // existing per-chunk durable/lifetime proofs remain inside the worker.
    fn schedule_permitted_connection_close(
        &mut self,
        snapshot: Snapshot,
        selected: ObservedRow,
        token: &ExternalCloseToken,
        permit: crate::conditional_close_candidate::CandidateEffectPermit,
        lease: &MigrationLock,
    ) -> Result<Option<ExternalCloseOutcome>, NativeOwnerError> {
        if !lease.authorizes(&snapshot.context.cutover_paths, snapshot.context.uid) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
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
        Ok(None)
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

    #[cfg(feature = "developer-conditional-close")]
    pub(crate) fn connection_close_receipt(
        &mut self,
        operation_id: &str,
    ) -> Result<Option<Option<ExternalCloseReceipt>>, NativeOwnerError> {
        // Polling completes only the original worker's reservation. It never
        // constructs a new controller request, confirmation or effect permit.
        self.poll_connection_close()?;
        self.coordinator
            .external_close_receipt(operation_id)
            .map_err(Into::into)
    }
}
