// SPDX-License-Identifier: MIT

use super::*;
use crate::cutover::{CutoverPaths, read_marker};
use crate::desired::{DesiredState, OwnedObservation, RoutingMode, write_desired};
use crate::lifecycle::HostStepError;
use crate::remote_fetch::{MAX_CONCURRENT_REMOTE_FETCHES, RemoteFetchPool};
use crate::subscription_schedule_driver::{AutomaticRefreshTick, AutomaticSubscriptionDriver};
use crate::subscription_schedule_plan::{INITIAL_RETRY_SECS, MIN_INTERVAL_SECS};
use crate::subscription_transport::{HttpsSubscriptionTransport, SubscriptionTransportError};
use omavless_domain::subscription_feed::PrivateSubscriptionBody;
use omavless_store::atomic_replace_private;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const INSTANCE: &str = "automatic-fixture-1";
const GENERATION: u64 = 2;
const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";
const PROFILE: &str = "20000000-0000-4000-8000-000000000001";
const BODY: &str =
    "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Synthetic";
const EVERY: RefreshSchedule = RefreshSchedule::Every {
    interval_secs: MIN_INTERVAL_SECS,
};
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct InertHost;
impl LifecycleHost for InertHost {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        Ok(OwnedObservation {
            service_active: false,
            controller_ready: false,
            core_count: 0,
            tun_count: 0,
            active_profile_matches: false,
        })
    }
    fn prepare(&mut self, _: &DesiredState) -> Result<(), HostStepError> {
        panic!("unexpected core preparation");
    }
    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("unexpected core start");
    }
    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        panic!("unexpected core commit");
    }
    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        Ok(())
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        Ok(())
    }
}

struct Fixture {
    root: PathBuf,
    store: PathBuf,
    paths: CutoverPaths,
    uid: u32,
    owner: Arc<Mutex<OfflineNativeCoordinator<InertHost>>>,
    pool: RemoteFetchPool,
}
impl Fixture {
    fn new(url: Option<&str>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omavless-automatic-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let runtime = root.join("runtime");
        let state = root.join("state");
        for directory in [&root, &runtime, &state] {
            fs::DirBuilder::new().mode(0o700).create(directory).unwrap();
        }
        let uid = nix::unistd::Uid::current().as_raw();
        let paths = CutoverPaths::below(&runtime, &state, uid);
        read_marker(&paths, uid).unwrap();
        atomic_replace_private(
            &paths.ownership_marker,
            b"{\"schemaVersion\":1,\"generation\":2,\"phase\":\"rust\"}\n",
            uid,
        )
        .unwrap();
        let subscriptions = url
            .map(|url| {
                vec![json!({"id":SUBSCRIPTION,
            "name":"Synthetic source", "url":url, "updatedAt":7})]
            })
            .unwrap_or_default();
        let store = root.join("profiles.json");
        atomic_replace_private(&store, serde_json::to_string(&json!({
            "version":3, "activeId":"", "lastId":"", "profiles":[], "subscriptions":subscriptions,
            "routingPreset":"custom", "customRules":[], "rulesUpdatedAt":0,
            "startupConfigured":true, "startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},
            "onboardingComplete":true,
        })).unwrap().as_bytes(), uid).unwrap();
        let desired = DesiredPaths::below(&state);
        write_desired(
            &desired,
            uid,
            &DesiredState {
                schema_version: 1,
                generation: 0,
                connected: false,
                profile_id: String::new(),
                mode: RoutingMode::Rule,
            },
        )
        .unwrap();
        let owner = Arc::new(Mutex::new(OfflineNativeCoordinator::new_ownership_gated(
            InertHost,
            desired,
            &store,
            paths.clone(),
            uid,
            GENERATION,
        )));
        Self {
            root,
            store,
            paths,
            uid,
            owner,
            pool: RemoteFetchPool::default(),
        }
    }
    fn enable(&self) -> u64 {
        self.owner
            .lock()
            .unwrap()
            .set_automatic_subscription_preference(0, EVERY)
            .unwrap()
            .revision
    }
    fn driver(&self) -> AutomaticSubscriptionDriver<InertHost> {
        AutomaticSubscriptionDriver::new(Arc::clone(&self.owner), INSTANCE, &self.pool)
    }
    fn snapshot(&self) -> Option<AttemptSnapshot> {
        read_attempt(&self.paths, self.uid, GENERATION, INSTANCE).unwrap()
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(&self.store).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A real HTTP listener and the production bounded HTTP client. The callback
/// executes on receipt of GET, so it can prove durable admission before I/O.
fn loopback_server<F>(
    listener: TcpListener,
    statuses: Vec<u16>,
    arrived: F,
) -> thread::JoinHandle<()>
where
    F: Fn() + Send + 'static,
{
    listener.set_nonblocking(true).unwrap();
    thread::spawn(move || {
        for status in statuses {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "synthetic GET did not arrive");
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => panic!("synthetic accept failed"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0u8; 1024];
            assert!(stream.read(&mut request).unwrap() > 0);
            arrived();
            let body = if status == 200 {
                BODY
            } else {
                "synthetic refusal"
            };
            write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    })
}

#[test]
fn automatic_driver_real_http_is_off_by_default_and_publishes_begin_before_get() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fixture = Fixture::new(Some(&format!(
        "http://{}/synthetic-feed",
        listener.local_addr().unwrap()
    )));
    let before = fixture.bytes();
    let mut driver = fixture.driver();
    let transport = HttpsSubscriptionTransport::new();
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 100),
        Ok(AutomaticRefreshTick::Idle(ScheduleDecision::Off))
    );
    assert_eq!(fixture.bytes(), before);
    assert!(fixture.snapshot().is_none());
    fixture.enable();
    let paths = fixture.paths.clone();
    let uid = fixture.uid;
    let owner = Arc::clone(&fixture.owner);
    let server = loopback_server(listener, vec![200], move || {
        assert!(owner.try_lock().is_ok(), "GET held serialized owner");
        let probe = MigrationLock::acquire(&paths, uid).unwrap();
        drop(probe);
        assert_eq!(
            read_attempt(&paths, uid, GENERATION, INSTANCE)
                .unwrap()
                .unwrap()
                .state,
            AttemptState::StartedInCurrentInstance
        );
    });
    let result = driver
        .tick(&transport, &mut || PROFILE.to_owned(), || 100)
        .unwrap();
    assert!(matches!(
        result,
        AutomaticRefreshTick::Finished(AttemptSnapshot {
            state: AttemptState::Succeeded,
            ..
        })
    ));
    server.join().unwrap();
    assert_eq!(fixture.owner.lock().unwrap().revision(), 1);
    assert_ne!(fixture.bytes(), before);
    for now in [99, 100, 100 + MIN_INTERVAL_SECS - 1] {
        assert_eq!(
            driver.tick(&transport, &mut || panic!("early record ID"), || now),
            Ok(AutomaticRefreshTick::Idle(ScheduleDecision::WaitUntil(
                100 + MIN_INTERVAL_SECS
            )))
        );
    }
    let raw = fs::read(
        fixture
            .paths
            .state_directory
            .join("subscription-refresh-attempt.json"),
    )
    .unwrap();
    assert!(!raw.windows(4).any(|bytes| bytes == b"http"));
    assert_eq!(
        fs::metadata(
            fixture
                .paths
                .state_directory
                .join("subscription-refresh-attempt.json")
        )
        .unwrap()
        .permissions()
        .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn automatic_driver_real_http_failure_uses_injected_backoff_then_success() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fixture = Fixture::new(Some(&format!(
        "http://{}/synthetic-feed",
        listener.local_addr().unwrap()
    )));
    let before = fixture.bytes();
    fixture.enable();
    let hits = Arc::new(AtomicUsize::new(0));
    let server_hits = Arc::clone(&hits);
    let server = loopback_server(listener, vec![503, 200], move || {
        server_hits.fetch_add(1, Ordering::Relaxed);
    });
    let mut driver = fixture.driver();
    let transport = HttpsSubscriptionTransport::new();
    assert!(matches!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 100),
        Ok(AutomaticRefreshTick::Finished(AttemptSnapshot {
            state: AttemptState::Failed,
            consecutive_failures: 1,
            ..
        }))
    ));
    assert_eq!(fixture.bytes(), before);
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 399),
        Ok(AutomaticRefreshTick::Idle(ScheduleDecision::WaitUntil(
            100 + INITIAL_RETRY_SECS
        )))
    );
    assert_eq!(hits.load(Ordering::Relaxed), 1);
    assert!(matches!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 400),
        Ok(AutomaticRefreshTick::Finished(AttemptSnapshot {
            state: AttemptState::Succeeded,
            consecutive_failures: 0,
            ..
        }))
    ));
    server.join().unwrap();
    assert_eq!(hits.load(Ordering::Relaxed), 2);
    assert_eq!(fixture.snapshot().unwrap().sequence, 2);
}

struct CountingTransport(AtomicUsize);
impl BudgetedSubscriptionTransport for CountingTransport {
    fn fetch_with_budget(
        &self,
        _: &str,
        _: Duration,
    ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        PrivateSubscriptionBody::from_bytes(BODY.as_bytes().to_vec())
            .map_err(|_| SubscriptionTransportError::Unavailable)
    }
}

#[test]
fn automatic_driver_shares_permits_and_global_worker_slot_and_stops_without_get() {
    let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
    let before = fixture.bytes();
    fixture.enable();
    let permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
        .map(|_| fixture.pool.try_acquire().unwrap())
        .collect();
    let transport = CountingTransport(AtomicUsize::new(0));
    let mut driver = fixture.driver();
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 100),
        Ok(AutomaticRefreshTick::Running)
    );
    let mut second = fixture.driver();
    assert!(matches!(
        second.tick(&transport, &mut || PROFILE.to_owned(), || 100),
        Err(AutomaticRefreshError::Owner(
            NativeOwnerError::LongOperation(crate::long_operation::LongOperationError::Busy)
        ))
    ));
    let manual = json!({"api":"omavless.control","version":1,"id":"manual", "method":"subscriptions.refresh_all", "params":{"instanceId":INSTANCE,"operationId":"manual"}});
    assert!(matches!(
        fixture
            .owner
            .lock()
            .unwrap()
            .start_subscription_batch(&manual),
        Err(NativeOwnerError::LongOperation(
            crate::long_operation::LongOperationError::Busy
        ))
    ));
    assert_eq!(transport.0.load(Ordering::Relaxed), 0);
    assert!(matches!(
        driver.stop(101),
        Ok(AutomaticRefreshTick::Finished(AttemptSnapshot {
            state: AttemptState::Cancelled,
            ..
        }))
    ));
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 100000),
        Ok(AutomaticRefreshTick::Stopped)
    );
    assert_eq!(fixture.bytes(), before);
    assert_eq!(transport.0.load(Ordering::Relaxed), 0);
    drop(permits);
}
