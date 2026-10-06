// SPDX-License-Identifier: MIT

use super::*;
use crate::cutover::{CutoverPaths, read_marker};
use crate::desired::{DesiredState, OwnedObservation, RoutingMode, write_desired};
use crate::lifecycle::HostStepError;
use crate::remote_fetch::{MAX_CONCURRENT_REMOTE_FETCHES, RemoteFetchPool};
use crate::subscription_mutation::commit_subscription_refresh_batch;
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

struct InertHost {
    calls: Arc<AtomicUsize>,
}
impl LifecycleHost for InertHost {
    fn observe(&mut self, _: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
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
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

struct Fixture {
    root: PathBuf,
    store: PathBuf,
    paths: CutoverPaths,
    desired: DesiredPaths,
    uid: u32,
    owner: Arc<Mutex<OfflineNativeCoordinator<InertHost>>>,
    pool: RemoteFetchPool,
    host_calls: Arc<AtomicUsize>,
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
        let host_calls = Arc::new(AtomicUsize::new(0));
        let owner = Arc::new(Mutex::new(OfflineNativeCoordinator::new_ownership_gated(
            InertHost {
                calls: Arc::clone(&host_calls),
            },
            desired.clone(),
            &store,
            paths.clone(),
            uid,
            GENERATION,
        )));
        Self {
            root,
            store,
            paths,
            desired,
            uid,
            owner,
            pool: RemoteFetchPool::default(),
            host_calls,
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
    loopback_content_server(
        listener,
        statuses
            .into_iter()
            .map(|status| {
                (
                    status,
                    if status == 200 {
                        BODY.to_owned()
                    } else {
                        "synthetic refusal".to_owned()
                    },
                )
            })
            .collect(),
        arrived,
    )
}

fn loopback_content_server<F>(
    listener: TcpListener,
    replies: Vec<(u16, String)>,
    arrived: F,
) -> thread::JoinHandle<()>
where
    F: Fn() + Send + 'static,
{
    listener.set_nonblocking(true).unwrap();
    thread::spawn(move || {
        for (status, body) in replies {
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
impl SubscriptionTransport for CountingTransport {
    fn fetch(&self, url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        self.fetch_with_budget(url, Duration::from_secs(1))
    }
}
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

#[test]
fn automatic_driver_real_http_refuses_disable_cancel_disconnect_and_stale_members() {
    for scenario in [
        "off",
        "interval",
        "reenable",
        "cancel",
        "disconnect",
        "delete",
        "url",
        "revoked",
        "shutdown",
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let fixture = Fixture::new(Some(&format!(
            "http://{}/synthetic-feed",
            listener.local_addr().unwrap()
        )));
        fixture.enable();
        let (arrived_tx, arrived_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let owner = Arc::clone(&fixture.owner);
        let server = loopback_server(listener, vec![200], move || {
            assert!(owner.try_lock().is_ok());
            arrived_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        });
        let mut driver = fixture.driver();
        let worker = thread::spawn(move || {
            driver.tick(
                &HttpsSubscriptionTransport::new(),
                &mut || PROFILE.to_owned(),
                || 100,
            )
        });
        arrived_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        {
            let mut owner = fixture.owner.lock().unwrap();
            match scenario {
                "off" | "reenable" => {
                    owner
                        .set_automatic_subscription_preference(1, RefreshSchedule::Off)
                        .unwrap();
                    if scenario == "reenable" {
                        owner
                            .set_automatic_subscription_preference(2, EVERY)
                            .unwrap();
                    }
                }
                "interval" => {
                    owner
                        .set_automatic_subscription_preference(
                            1,
                            RefreshSchedule::Every {
                                interval_secs: MIN_INTERVAL_SECS + 1,
                            },
                        )
                        .unwrap();
                }
                "cancel" => {
                    assert!(owner.cancel_automatic_subscription_refresh().unwrap());
                }
                "disconnect" => {
                    owner
                        .execute_connection(OwnerRequest::new(
                            OwnerAction::Disconnect,
                            Some("manual-disconnect"),
                            Some(0),
                            crate::mutation::MutationDigest::from_semantic_bytes(
                                b"fixture-disconnect",
                            ),
                        ))
                        .unwrap();
                    // Explicit NoChange Disconnect still cancels maintenance.
                    assert_eq!(owner.revision(), 0);
                }
                "delete" => {
                    let request = json!({"api":"omavless.control","version":1,"id":"delete",
                        "method":"subscriptions.delete","params":{"subscriptionId":SUBSCRIPTION,"operationId":"manual-delete","expectedRevision":0}});
                    owner
                        .execute_subscription(
                            &request,
                            &CountingTransport(AtomicUsize::new(0)),
                            || panic!("delete minted ID"),
                            || panic!("delete read clock"),
                        )
                        .unwrap();
                    assert_eq!(owner.revision(), 1);
                }
                "url" => {
                    let mut document: Value = serde_json::from_slice(&fixture.bytes()).unwrap();
                    document["subscriptions"][0]["url"] =
                        json!("http://127.0.0.1:9/changed-synthetic-feed");
                    atomic_replace_private(
                        &fixture.store,
                        serde_json::to_vec(&document).unwrap().as_slice(),
                        fixture.uid,
                    )
                    .unwrap();
                }
                "revoked" => {
                    atomic_replace_private(
                        &fixture.paths.ownership_marker,
                        b"{\"schemaVersion\":1,\"generation\":3,\"phase\":\"rust\"}\n",
                        fixture.uid,
                    )
                    .unwrap();
                }
                "shutdown" => {
                    owner.stop_batch_operations().unwrap();
                }
                _ => unreachable!(),
            }
        }
        let expected = fixture.bytes();
        release_tx.send(()).unwrap();
        let result = worker.join().unwrap();
        server.join().unwrap();
        assert_eq!(
            fixture.bytes(),
            expected,
            "late worker replaced current store"
        );
        if scenario == "revoked" {
            assert!(result.is_err());
            let attempt: Value = serde_json::from_slice(
                &fs::read(
                    fixture
                        .paths
                        .state_directory
                        .join("subscription-refresh-attempt.json"),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(attempt["state"], "started");
        } else {
            let AutomaticRefreshTick::Finished(snapshot) = result.unwrap() else {
                panic!("missing exact terminal");
            };
            assert_eq!(
                snapshot.state,
                match scenario {
                    "off" | "reenable" | "interval" => AttemptState::Superseded,
                    "cancel" | "disconnect" => AttemptState::Cancelled,
                    _ => AttemptState::Failed,
                }
            );
        }
    }
}

#[test]
fn automatic_commit_rechecks_preference_inside_the_store_lease() {
    let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
    fixture.enable();
    let before = fixture.bytes();
    let mut owner = fixture.owner.lock().unwrap();
    let AutomaticRefreshStart::Started(mut work) = owner
        .start_automatic_subscription_refresh(INSTANCE, 100)
        .unwrap()
    else {
        panic!("not admitted");
    };
    owner.check_automatic_subscription_work(&work).unwrap();
    work.step(
        &CountingTransport(AtomicUsize::new(0)),
        &fixture.pool,
        &mut || PROFILE.to_owned(),
    )
    .unwrap();
    // Change through the predecessor offline setter, without notification.
    // Only the final SAME-lease policy can prevent this already-ready commit.
    set_preference(
        &fixture.paths,
        fixture.uid,
        GENERATION,
        1,
        RefreshSchedule::Off,
    )
    .unwrap();
    let snapshot = owner
        .finish_automatic_with_store(work, 101, |_, _, _, _, _| {
            panic!("changed preference reached store publication");
        })
        .unwrap();
    assert_eq!(snapshot.state, AttemptState::Superseded);
    assert_eq!(fixture.bytes(), before);
    assert_eq!(owner.revision(), 0);
}

#[test]
fn automatic_empty_success_and_store_commit_hold_the_same_migration_lease() {
    for empty in [true, false] {
        let fixture = Fixture::new(if empty {
            None
        } else {
            Some("http://127.0.0.1:9/synthetic-feed")
        });
        fixture.enable();
        let before = fixture.bytes();
        let mut owner = fixture.owner.lock().unwrap();
        let AutomaticRefreshStart::Started(mut work) = owner
            .start_automatic_subscription_refresh(INSTANCE, 100)
            .unwrap()
        else {
            panic!("not admitted");
        };
        let transport = CountingTransport(AtomicUsize::new(0));
        work.step(&transport, &fixture.pool, &mut || PROFILE.to_owned())
            .unwrap();
        let snapshot = owner
            .finish_automatic_with_store(work, 101, |path, uid, snapshot, updates, stamp| {
                assert!(matches!(
                    MigrationLock::acquire(&fixture.paths, fixture.uid),
                    Err(crate::cutover::CutoverError::Busy)
                ));
                commit_subscription_refresh_batch(path, uid, snapshot, updates, stamp)
            })
            .unwrap();
        assert_eq!(
            snapshot.state,
            if empty {
                AttemptState::Empty
            } else {
                AttemptState::Succeeded
            }
        );
        assert_eq!(transport.0.load(Ordering::Relaxed), usize::from(!empty));
        assert_eq!(owner.revision(), u64::from(!empty));
        assert_eq!(fixture.bytes() == before, empty);
    }
}

#[test]
fn automatic_unknown_store_publication_blocks_replay_without_fabricating_failure() {
    let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
    fixture.enable();
    let before = fixture.bytes();
    let mut owner = fixture.owner.lock().unwrap();
    let AutomaticRefreshStart::Started(mut work) = owner
        .start_automatic_subscription_refresh(INSTANCE, 100)
        .unwrap()
    else {
        panic!("not admitted");
    };
    work.step(
        &CountingTransport(AtomicUsize::new(0)),
        &fixture.pool,
        &mut || PROFILE.to_owned(),
    )
    .unwrap();
    let result =
        owner.finish_automatic_with_store(work, 101, |path, uid, snapshot, updates, stamp| {
            commit_subscription_refresh_batch(path, uid, snapshot, updates, stamp).unwrap();
            // Models uncertainty after atomic replacement, before confirmed sync.
            Err(SubscriptionMutationCommitError::StoreIo)
        });
    assert_eq!(
        result,
        Err(AutomaticRefreshError::Attempt(
            AttemptError::OutcomeUncertain
        ))
    );
    assert_ne!(fixture.bytes(), before);
    assert_eq!(
        fixture.snapshot().unwrap().state,
        AttemptState::StartedInCurrentInstance
    );
    assert!(
        owner
            .start_automatic_subscription_refresh(INSTANCE, 100000)
            .is_err()
    );
}

#[test]
fn automatic_terminal_write_failure_cannot_replay_committed_store() {
    let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
    fixture.enable();
    let mut owner = fixture.owner.lock().unwrap();
    let AutomaticRefreshStart::Started(mut work) = owner
        .start_automatic_subscription_refresh(INSTANCE, 100)
        .unwrap()
    else {
        panic!("not admitted");
    };
    work.step(
        &CountingTransport(AtomicUsize::new(0)),
        &fixture.pool,
        &mut || PROFILE.to_owned(),
    )
    .unwrap();
    let journal = fixture
        .paths
        .state_directory
        .join("subscription-refresh-attempt.json");
    let original_started = fs::read(&journal).unwrap();
    let result =
        owner.finish_automatic_with_store(work, 101, |path, uid, snapshot, updates, stamp| {
            let result = commit_subscription_refresh_batch(path, uid, snapshot, updates, stamp);
            fs::set_permissions(&journal, fs::Permissions::from_mode(0o400)).unwrap();
            result
        });
    assert_eq!(
        result,
        Err(AutomaticRefreshError::Attempt(AttemptError::UnsafeState))
    );
    assert_eq!(owner.revision(), 1);
    assert_eq!(fs::read(&journal).unwrap(), original_started);
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        owner
            .start_automatic_subscription_refresh(INSTANCE, 100000)
            .is_err()
    );
    owner
        .set_automatic_subscription_preference(1, RefreshSchedule::Off)
        .unwrap();
    owner
        .set_automatic_subscription_preference(2, EVERY)
        .unwrap();
    assert!(
        owner
            .start_automatic_subscription_refresh(INSTANCE, 100000)
            .is_err()
    );
    let mut restarted = OfflineNativeCoordinator::new_ownership_gated(
        InertHost {
            calls: Arc::clone(&fixture.host_calls),
        },
        fixture.desired.clone(),
        &fixture.store,
        fixture.paths.clone(),
        fixture.uid,
        GENERATION,
    );
    assert!(matches!(
        restarted.start_automatic_subscription_refresh("new-daemon", 100000),
        Err(AutomaticRefreshError::Attempt(
            AttemptError::AttemptUncertain
        ))
    ));
    // Even an identical instance string without the live registry is unknown.
    assert!(matches!(
        restarted.start_automatic_subscription_refresh(INSTANCE, 100000),
        Err(AutomaticRefreshError::Attempt(
            AttemptError::AttemptUncertain
        ))
    ));
}

#[test]
fn automatic_worker_loss_and_panic_leave_started_and_block_restart() {
    for panic_worker in [false, true] {
        let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
        fixture.enable();
        let before = fixture.bytes();
        let mut driver = fixture.driver();
        if panic_worker {
            struct PanicTransport;
            impl BudgetedSubscriptionTransport for PanicTransport {
                fn fetch_with_budget(
                    &self,
                    _: &str,
                    _: Duration,
                ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
                    panic!("synthetic worker loss");
                }
            }
            assert_eq!(
                driver.tick(&PanicTransport, &mut || PROFILE.to_owned(), || 100),
                Err(AutomaticRefreshError::WorkerLost)
            );
        } else {
            let permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
                .map(|_| fixture.pool.try_acquire().unwrap())
                .collect();
            assert_eq!(
                driver.tick(
                    &CountingTransport(AtomicUsize::new(0)),
                    &mut || PROFILE.to_owned(),
                    || 100
                ),
                Ok(AutomaticRefreshTick::Running)
            );
            drop(permits);
        }
        drop(driver);
        assert_eq!(fixture.bytes(), before);
        assert_eq!(
            fixture.snapshot().unwrap().state,
            AttemptState::StartedInCurrentInstance
        );
        let mut next = fixture.driver();
        assert_eq!(
            next.tick(
                &CountingTransport(AtomicUsize::new(0)),
                &mut || PROFILE.to_owned(),
                || 100000
            ),
            Err(AutomaticRefreshError::WorkerLost)
        );
    }
}

#[test]
fn automatic_clock_regression_at_completion_refuses_store_and_blocks_replay() {
    let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
    fixture.enable();
    let before = fixture.bytes();
    let calls = AtomicUsize::new(0);
    let mut driver = fixture.driver();
    assert_eq!(
        driver.tick(
            &CountingTransport(AtomicUsize::new(0)),
            &mut || PROFILE.to_owned(),
            || {
                if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                    100
                } else {
                    99
                }
            }
        ),
        Err(AutomaticRefreshError::ClockInvalid)
    );
    assert_eq!(fixture.bytes(), before);
    assert_eq!(
        fixture.snapshot().unwrap().state,
        AttemptState::StartedInCurrentInstance
    );
    assert_eq!(
        driver.tick(
            &CountingTransport(AtomicUsize::new(0)),
            &mut || PROFILE.to_owned(),
            || 100000
        ),
        Err(AutomaticRefreshError::ClockInvalid)
    );
}

#[test]
fn automatic_connected_selected_profile_unchanged_refresh_has_zero_lifecycle_calls() {
    for scenario in ["unchanged", "changed", "deleted"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let fixture = Fixture::new(Some(&format!(
            "http://{}/synthetic-feed",
            listener.local_addr().unwrap()
        )));
        fixture.enable();
        let mut driver = fixture.driver();
        assert!(matches!(
            driver.tick(
                &CountingTransport(AtomicUsize::new(0)),
                &mut || PROFILE.to_owned(),
                || 100
            ),
            Ok(AutomaticRefreshTick::Finished(AttemptSnapshot {
                state: AttemptState::Succeeded,
                ..
            }))
        ));
        let mut document: Value = serde_json::from_slice(&fixture.bytes()).unwrap();
        document["activeId"] = json!(PROFILE);
        document["lastId"] = json!(PROFILE);
        atomic_replace_private(
            &fixture.store,
            &serde_json::to_vec(&document).unwrap(),
            fixture.uid,
        )
        .unwrap();
        let desired_paths = fixture.desired.clone();
        let desired = DesiredState {
            schema_version: 1,
            generation: 1,
            connected: true,
            profile_id: PROFILE.to_owned(),
            mode: RoutingMode::Rule,
        };
        write_desired(&desired_paths, fixture.uid, &desired).unwrap();
        let before = fixture.bytes();
        let body = match scenario {
            "unchanged" => BODY.to_owned(),
            "changed" => BODY.replace("#Synthetic", "#Changed-synthetic-name"),
            _ => BODY.replace("192.0.2.2", "192.0.2.3"),
        };
        let server = loopback_content_server(listener, vec![(200, body)], || {});
        let result = driver
            .tick(
                &HttpsSubscriptionTransport::new(),
                &mut || "20000000-0000-4000-8000-000000000002".to_owned(),
                || 100 + MIN_INTERVAL_SECS,
            )
            .unwrap();
        server.join().unwrap();
        let AutomaticRefreshTick::Finished(snapshot) = result else {
            panic!("missing active-profile terminal");
        };
        assert_eq!(
            fixture.host_calls.load(Ordering::Relaxed),
            0,
            "automatic refresh reached lifecycle host"
        );
        assert_eq!(
            crate::desired::read_desired(&desired_paths, fixture.uid).unwrap(),
            desired
        );
        if scenario == "unchanged" {
            assert_eq!(snapshot.state, AttemptState::Succeeded);
            assert_ne!(fixture.bytes(), before);
            assert_eq!(fixture.owner.lock().unwrap().revision(), 2);
        } else {
            assert_eq!(
                snapshot.state,
                AttemptState::Failed,
                "active scenario {scenario}"
            );
            assert_eq!(fixture.bytes(), before);
            assert_eq!(fixture.owner.lock().unwrap().revision(), 1);
        }
    }
}

#[test]
fn automatic_preference_post_publication_read_failure_latches_and_cancels_exact_worker() {
    let fixture = Fixture::new(Some("http://127.0.0.1:9/synthetic-feed"));
    fixture.enable();
    let before = fixture.bytes();
    let permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
        .map(|_| fixture.pool.try_acquire().unwrap())
        .collect();
    let transport = CountingTransport(AtomicUsize::new(0));
    let mut driver = fixture.driver();
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 100),
        Ok(AutomaticRefreshTick::Running)
    );
    let preference = fixture
        .paths
        .state_directory
        .join("subscription-refresh-preference.json");
    let mut owner = fixture.owner.lock().unwrap();
    let result = crate::subscription_schedule_preference::set_preference_with_publication_fault(
        &fixture.paths,
        fixture.uid,
        GENERATION,
        1,
        RefreshSchedule::Every {
            interval_secs: MIN_INTERVAL_SECS + 1,
        },
        || {
            fs::set_permissions(&preference, fs::Permissions::from_mode(0o400)).unwrap();
        },
    );
    assert_eq!(result, Err(PreferenceError::WriteUncertain));
    assert_eq!(
        owner.accept_automatic_preference_result(
            RefreshSchedule::Every {
                interval_secs: MIN_INTERVAL_SECS + 1
            },
            result
        ),
        Err(AutomaticRefreshError::Preference(
            PreferenceError::WriteUncertain
        ))
    );
    fs::set_permissions(&preference, fs::Permissions::from_mode(0o600)).unwrap();
    owner
        .set_automatic_subscription_preference(2, RefreshSchedule::Off)
        .unwrap();
    owner
        .set_automatic_subscription_preference(3, EVERY)
        .unwrap();
    drop(owner);
    drop(permits);
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 101),
        Err(AutomaticRefreshError::Preference(
            PreferenceError::WriteUncertain
        ))
    );
    assert_eq!(transport.0.load(Ordering::Relaxed), 0);
    assert_eq!(fixture.bytes(), before);
    assert_eq!(
        fixture.snapshot().unwrap().state,
        AttemptState::StartedInCurrentInstance
    );
    assert_eq!(
        driver.tick(&transport, &mut || PROFILE.to_owned(), || 100000),
        Err(AutomaticRefreshError::Preference(
            PreferenceError::WriteUncertain
        ))
    );
}
