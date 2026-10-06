// SPDX-License-Identifier: MIT

use super::*;
use crate::developer_subscription_schedule::DeveloperSubscriptionSchedule;
use crate::subscription_schedule_attempt::{AttemptState, read_attempt};
use crate::subscription_schedule_plan::MIN_INTERVAL_SECS;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicU64;

struct DormantRuntime {
    base: PathBuf,
    paths: RuntimePaths,
    cutover: CutoverPaths,
    instance: String,
    stop: Arc<AtomicBool>,
    wake: Arc<AtomicBool>,
    now: Arc<AtomicU64>,
    pool: remote_fetch::RemoteFetchPool,
    runtime: Option<thread::JoinHandle<()>>,
}
impl DormantRuntime {
    fn start(url: &str) -> Self {
        let base = temporary_base("automatic-runtime");
        let (mut owner, cutover, _) = native_owner_fixture(&base);
        owner.batch_coordinator().host_mut().fresh_result = Ok(fresh_empty_facts());
        let store = base.join("config/profiles.json");
        let mut document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        document["subscriptions"][0]["url"] = json!(url);
        fs::write(&store, serde_json::to_vec(&document).unwrap()).unwrap();
        let mut server = RuntimeServer::bind_with_owner_factory(
            RuntimePaths::below(&base.join("runtime")),
            move |_| Ok(owner),
        )
        .unwrap();
        let instance = server.instance_id.clone();
        let paths = server.paths.clone();
        let pool = server.remote_fetches.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(AtomicBool::new(false));
        let now = Arc::new(AtomicU64::new(100));
        let clock = Arc::clone(&now);
        let wakeup = Arc::clone(&wake);
        server
            .register_developer_subscription_schedule(DeveloperSubscriptionSchedule::new(
                move || clock.load(Ordering::Acquire),
                move || wakeup.swap(false, Ordering::AcqRel),
            ))
            .unwrap();
        let stopper = Arc::clone(&stop);
        let runtime = Some(thread::spawn(move || server.serve_until(&stopper).unwrap()));
        Self {
            base,
            paths,
            cutover,
            instance,
            stop,
            wake,
            now,
            pool,
            runtime,
        }
    }
    fn enable(&self) {
        assert_eq!(
            schedule_call(&self.paths, &self.instance, Some((0, MIN_INTERVAL_SECS)))["ok"],
            true
        );
        self.wake.store(true, Ordering::Release);
    }
    fn store(&self) -> PathBuf {
        self.base.join("config/profiles.json")
    }
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(runtime) = self.runtime.take() {
            runtime.join().unwrap();
        }
    }
}
impl Drop for DormantRuntime {
    fn drop(&mut self) {
        self.shutdown();
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn developer_schedule_socket_cancel_disconnect_member_and_owner_races_refuse_late_http() {
    for scenario in ["off", "disconnect", "member", "revoked", "shutdown", "quit"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut fixture = DormantRuntime::start(&format!(
            "http://{}/synthetic-feed",
            listener.local_addr().unwrap()
        ));
        let (arrived_tx, arrived_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let http = thread::spawn(move || {
            let mut stream = accept_http(&listener);
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0u8; 1024];
            assert!(stream.read(&mut request).unwrap() > 0);
            arrived_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(4)).unwrap();
            let body = "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Synthetic";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        fixture.enable();
        arrived_rx.recv_timeout(Duration::from_secs(4)).unwrap();
        let manual = call(
            &fixture.paths,
            "subscriptions.refresh_all",
            json!({"instanceId":fixture.instance,"operationId":"manual-overlap"}),
        )
        .unwrap();
        assert_eq!(manual["error"]["code"], "busy");
        let mut joined = None;
        fixture.now.store(101, Ordering::Release);
        match scenario {
            "off" => {
                assert_eq!(
                    schedule_call(&fixture.paths, &fixture.instance, Some((1, 0)))["ok"],
                    true
                );
            }
            "disconnect" => {
                assert_eq!(
                    call(&fixture.paths, "connection.disconnect", json!({})).unwrap()["ok"],
                    true
                );
            }
            "member" => {
                let mut document: Value =
                    serde_json::from_slice(&fs::read(fixture.store()).unwrap()).unwrap();
                document["subscriptions"][0]["url"] =
                    json!("http://127.0.0.1:9/changed-synthetic-feed");
                fs::write(fixture.store(), serde_json::to_vec(&document).unwrap()).unwrap();
            }
            "revoked" => write_marker(&fixture.cutover, OwnershipPhase::Rust, 2),
            "quit" => {
                let response=call(&fixture.paths,"runtime.quit",json!({"instanceId":fixture.instance,"expectedRevision":0,"operationId":"automatic-quit"})).unwrap();
                assert_eq!(response["ok"], true);
                let runtime = fixture.runtime.take().unwrap();
                let done = Arc::new(AtomicBool::new(false));
                let worker_done = Arc::clone(&done);
                let join = thread::spawn(move || {
                    runtime.join().unwrap();
                    worker_done.store(true, Ordering::Release);
                });
                thread::sleep(Duration::from_millis(50));
                assert!(
                    !done.load(Ordering::Acquire),
                    "Full Quit failed to drain worker"
                );
                joined = Some((join, done));
            }
            "shutdown" => {
                fixture.stop.store(true, Ordering::Release);
                let runtime = fixture.runtime.take().unwrap();
                let done = Arc::new(AtomicBool::new(false));
                let worker_done = Arc::clone(&done);
                let join = thread::spawn(move || {
                    runtime.join().unwrap();
                    worker_done.store(true, Ordering::Release);
                });
                thread::sleep(Duration::from_millis(50));
                assert!(
                    !done.load(Ordering::Acquire),
                    "shutdown failed to drain worker"
                );
                joined = Some((join, done));
            }
            _ => unreachable!(),
        }
        let expected = fs::read(fixture.store()).unwrap();
        release_tx.send(()).unwrap();
        http.join().unwrap();
        if let Some((join, done)) = joined {
            join.join().unwrap();
            assert!(done.load(Ordering::Acquire));
            assert_eq!(
                read_attempt(
                    &fixture.cutover,
                    Uid::current().as_raw(),
                    1,
                    &fixture.instance
                )
                .unwrap()
                .unwrap()
                .state,
                AttemptState::Cancelled
            );
        } else if scenario == "revoked" {
            assert_eq!(
                schedule_call(&fixture.paths, &fixture.instance, None)["error"]["code"],
                "capability_unavailable"
            );
            fixture.shutdown();
            let journal: Value = serde_json::from_slice(
                &fs::read(
                    fixture
                        .cutover
                        .state_directory
                        .join("subscription-refresh-attempt.json"),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(journal["state"], "started");
        } else {
            let state = match scenario {
                "off" => "superseded",
                "disconnect" => "cancelled",
                _ => "failed",
            };
            wait_schedule(&fixture.paths, &fixture.instance, state);
        }
        assert_eq!(
            fs::read(fixture.store()).unwrap(),
            expected,
            "late HTTP changed current store"
        );
    }
}

#[test]
fn developer_schedule_wakeup_cannot_admit_first_worker_past_full_quit_gate() {
    let base = temporary_base("automatic-quit-gate");
    let (mut owner, cutover, _) = native_owner_fixture(&base);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let store = base.join("config/profiles.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    document["subscriptions"][0]["url"] = json!(format!(
        "http://{}/synthetic-feed",
        listener.local_addr().unwrap()
    ));
    fs::write(&store, serde_json::to_vec(&document).unwrap()).unwrap();
    let before = fs::read(&store).unwrap();
    let wake = Arc::new(AtomicBool::new(false));
    let hook_wake = Arc::clone(&wake);
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let host = owner.batch_coordinator().host_mut();
    host.fresh_result = Ok(fresh_empty_facts());
    host.on_fresh = Some(Box::new(move || {
        hook_wake.store(true, Ordering::Release);
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(4)).unwrap();
    }));
    let mut server = RuntimeServer::bind_with_owner_factory(
        RuntimePaths::below(&base.join("runtime")),
        move |_| Ok(owner),
    )
    .unwrap();
    let instance = server.instance_id.clone();
    let paths = server.paths.clone();
    let wakeup = Arc::clone(&wake);
    server
        .register_developer_subscription_schedule(DeveloperSubscriptionSchedule::new(
            || 100,
            move || wakeup.swap(false, Ordering::AcqRel),
        ))
        .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopper = Arc::clone(&stop);
    let runtime = thread::spawn(move || server.serve_until(&stopper).unwrap());
    assert_eq!(
        schedule_call(&paths, &instance, Some((0, MIN_INTERVAL_SECS)))["ok"],
        true
    );
    let quit_paths = paths.clone();
    let quit = thread::spawn(move || {
        call(&quit_paths,"runtime.quit",json!({"instanceId":instance,"expectedRevision":0,"operationId":"first-automatic-quit"})).unwrap()
    });
    entered_rx.recv_timeout(Duration::from_secs(4)).unwrap();
    thread::sleep(Duration::from_millis(50));
    assert!(
        !cutover
            .state_directory
            .join("subscription-refresh-attempt.json")
            .exists()
    );
    release_tx.send(()).unwrap();
    assert_eq!(quit.join().unwrap()["ok"], true);
    runtime.join().unwrap();
    assert!(
        !cutover
            .state_directory
            .join("subscription-refresh-attempt.json")
            .exists()
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read(store).unwrap(), before);
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn developer_schedule_socket_shared_permits_shutdown_has_no_fetch_and_joins_worker() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut fixture = DormantRuntime::start(&format!(
        "http://{}/synthetic-feed",
        listener.local_addr().unwrap()
    ));
    let permits: Vec<_> = (0..MAX_CONCURRENT_REMOTE_FETCHES)
        .map(|_| fixture.pool.try_acquire().unwrap())
        .collect();
    let before = fs::read(fixture.store()).unwrap();
    fixture.enable();
    wait_schedule(&fixture.paths, &fixture.instance, "running");
    assert_eq!(
        call(&fixture.paths, "status.get", json!({})).unwrap()["ok"],
        true
    );
    fixture.shutdown();
    assert_eq!(
        read_attempt(
            &fixture.cutover,
            Uid::current().as_raw(),
            1,
            &fixture.instance
        )
        .unwrap()
        .unwrap()
        .state,
        AttemptState::Cancelled
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read(fixture.store()).unwrap(), before);
    drop(permits);
}

#[test]
fn developer_schedule_socket_refuses_client_time_urls_invalid_preference_and_stale_instance() {
    let fixture = DormantRuntime::start("http://127.0.0.1:9/synthetic-feed");
    for params in [
        json!({"instanceId":fixture.instance,"expectedPreferenceRevision":0,"intervalSecs":MIN_INTERVAL_SECS,"nowSecs":100}),
        json!({"instanceId":fixture.instance,"expectedPreferenceRevision":0,"intervalSecs":MIN_INTERVAL_SECS,"url":"http://127.0.0.1:9/synthetic-feed"}),
        json!({"instanceId":fixture.instance,"expectedPreferenceRevision":true,"intervalSecs":MIN_INTERVAL_SECS}),
        json!({"instanceId":fixture.instance,"expectedPreferenceRevision":0,"intervalSecs":MIN_INTERVAL_SECS-1}),
    ] {
        assert_eq!(
            call(
                &fixture.paths,
                "developer.subscription_schedule.set",
                params
            )
            .unwrap()["error"]["code"],
            "invalid_argument"
        );
    }
    assert_eq!(
        schedule_call(
            &fixture.paths,
            "different-instance",
            Some((0, MIN_INTERVAL_SECS))
        )["error"]["code"],
        "conflict"
    );
    assert_eq!(
        schedule_call(
            &fixture.paths,
            &fixture.instance,
            Some((1, MIN_INTERVAL_SECS))
        )["error"]["code"],
        "conflict"
    );
    assert_eq!(
        schedule_call(&fixture.paths, &fixture.instance, None)["result"]["intervalSecs"],
        0
    );
    assert!(
        !fixture
            .cutover
            .state_directory
            .join("subscription-refresh-attempt.json")
            .exists()
    );
}

fn accept_http(listener: &TcpListener) -> std::net::TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "synthetic GET did not arrive"
                );
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => panic!("synthetic accept failed"),
        }
    }
}

fn schedule_call(paths: &RuntimePaths, instance: &str, interval: Option<(u64, u64)>) -> Value {
    let (method, params) = if let Some((revision, interval)) = interval {
        (
            "developer.subscription_schedule.set",
            json!({"instanceId":instance,"expectedPreferenceRevision":revision,"intervalSecs":interval}),
        )
    } else {
        (
            "developer.subscription_schedule.get",
            json!({"instanceId":instance}),
        )
    };
    call(paths, method, params).unwrap()
}

fn wait_schedule(paths: &RuntimePaths, instance: &str, state: &str) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        let result = schedule_call(paths, instance, None);
        assert_eq!(result["ok"], true);
        if result["result"]["attempt"]["state"] == state {
            return result;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "automatic terminal did not arrive"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn developer_schedule_socket_happy_path_uses_same_owner_supervisor_and_real_http() {
    let base = temporary_base("automatic-socket");
    let (owner, cutover, host_calls) = native_owner_fixture(&base);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let store = base.join("config/profiles.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    document["subscriptions"][0]["url"] = json!(format!(
        "http://{}/synthetic-feed",
        listener.local_addr().unwrap()
    ));
    fs::write(&store, serde_json::to_vec(&document).unwrap()).unwrap();
    let initial_host_calls = host_calls.load(Ordering::Relaxed);
    let (arrived_tx, arrived_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let uid = Uid::current().as_raw();
    let mut server = RuntimeServer::bind_with_owner_factory(
        RuntimePaths::below(&base.join("runtime")),
        move |_| Ok(owner),
    )
    .unwrap();
    let instance = server.instance_id.clone();
    let http_instance = instance.clone();
    let http_cutover = cutover.clone();
    let http = thread::spawn(move || {
        let mut stream = accept_http(&listener);
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = [0u8; 1024];
        assert!(stream.read(&mut request).unwrap() > 0);
        assert_eq!(
            read_attempt(&http_cutover, uid, 1, &http_instance)
                .unwrap()
                .unwrap()
                .state,
            AttemptState::StartedInCurrentInstance
        );
        arrived_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(4)).unwrap();
        let body = "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Synthetic";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let now = Arc::new(AtomicU64::new(100));
    let wake = Arc::new(AtomicBool::new(false));
    let clock = Arc::clone(&now);
    let wakeup = Arc::clone(&wake);
    server
        .register_developer_subscription_schedule(DeveloperSubscriptionSchedule::new(
            move || clock.load(Ordering::Acquire),
            move || wakeup.swap(false, Ordering::AcqRel),
        ))
        .unwrap();
    let paths = server.paths.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_worker = Arc::clone(&stop);
    let runtime = thread::spawn(move || server.serve_until(&stop_worker).unwrap());
    assert_eq!(
        schedule_call(&paths, &instance, None)["result"]["intervalSecs"],
        0
    );
    assert_eq!(
        schedule_call(&paths, &instance, Some((0, MIN_INTERVAL_SECS)))["ok"],
        true
    );
    wake.store(true, Ordering::Release);
    arrived_rx.recv_timeout(Duration::from_secs(4)).unwrap();
    let status = call(&paths, "status.get", json!({})).unwrap();
    assert_eq!(status["ok"], true);
    assert_eq!(status["revision"], 0);
    assert_eq!(
        schedule_call(&paths, &instance, None)["result"]["workerRegistered"],
        true
    );
    release_tx.send(()).unwrap();
    let completed = wait_schedule(&paths, &instance, "succeeded");
    assert_eq!(completed["revision"], 1);
    assert_eq!(completed["result"]["workerRegistered"], false);
    assert_eq!(host_calls.load(Ordering::Relaxed), initial_host_calls);
    let after = fs::read(&store).unwrap();
    now.store(100 + MIN_INTERVAL_SECS - 1, Ordering::Release);
    wake.store(true, Ordering::Release);
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        schedule_call(&paths, &instance, None)["result"]["attempt"]["sequence"],
        1
    );
    assert_eq!(fs::read(&store).unwrap(), after);
    stop.store(true, Ordering::Release);
    runtime.join().unwrap();
    http.join().unwrap();
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn developer_schedule_normal_registration_has_no_method_or_worker() {
    let base = temporary_base("automatic-default");
    let (owner, cutover, _) = native_owner_fixture(&base);
    let server = RuntimeServer::bind_with_owner_factory(
        RuntimePaths::below(&base.join("runtime")),
        move |_| Ok(owner),
    )
    .unwrap();
    let capabilities = server
        .dispatch(&make_request("caps", "capabilities.get", json!({})).unwrap())
        .unwrap();
    assert!(
        !capabilities["result"]["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| method.as_str().unwrap().starts_with("developer."))
    );
    let response = server
        .dispatch(
            &make_request(
                "get",
                "developer.subscription_schedule.get",
                json!({"instanceId":server.instance_id}),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(response["error"]["code"], "unknown_method");
    assert!(
        !cutover
            .state_directory
            .join("subscription-refresh-preference.json")
            .exists()
    );
    assert!(
        !cutover
            .state_directory
            .join("subscription-refresh-attempt.json")
            .exists()
    );
    drop(server);
    fs::remove_dir_all(base).unwrap();
}
