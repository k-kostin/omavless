// SPDX-License-Identifier: MIT
//! NEW process after all four original26 gates, never a current/login grant.
use super::*;
use std::os::unix::net::UnixStream;
use std::thread::JoinHandle;

const PAIR: &str = "/home/kdk_vm/.cache/t4-native-ordinary-review26/operation";
const RUN: &str = "/run/user/1000/t4n26/operation";
const CALLS: usize = 7;
type ClientResult = crate::Result<serde_json::Value>;

struct RpcOriginals {
    server: Option<crate::RuntimeServer>,
    owner: Option<ProductionNativeOwner>,
    accepted: [Option<UnixStream>; CALLS],
    client: Option<JoinHandle<ClientResult>>,
}
impl RpcOriginals {
    fn reserve() -> Result<Self, ()> {
        let (limit, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| ())?;
        // Necessary only, not a global free-FD promise: seven retained accepts,
        // original listener/singleton, one client and transient lease/readers.
        if limit < 32 {
            return Err(());
        }
        Ok(Self {
            server: None,
            owner: None,
            accepted: std::array::from_fn(|_| None),
            client: None,
        })
    }
    fn exchange(
        &mut self,
        index: usize,
        paths: &RuntimePaths,
        method: &'static str,
        params: serde_json::Value,
        until: Instant,
    ) -> Result<serde_json::Value, ()> {
        if index >= CALLS
            || self.server.is_none()
            || self.accepted[index].is_some()
            || self.client.is_some()
            || Instant::now() >= until
        {
            return Err(());
        }
        let client_paths = paths.clone();
        self.client = Some(
            std::thread::Builder::new()
                .name("t4-fixed-rpc".into())
                .spawn(move || crate::call(&client_paths, method, params))
                .map_err(|_| ())?,
        );
        if Instant::now() >= until {
            return Err(());
        }
        loop {
            if Instant::now() >= until {
                return Err(());
            }
            // A failed original client cannot justify another accept/effect.
            if self.client.as_ref().ok_or(())?.is_finished() {
                return Err(());
            }
            match self.server.as_ref().ok_or(())?.listener.accept() {
                Ok((stream, _)) => {
                    self.accepted[index] = Some(stream); // positive before postcheck
                    if Instant::now() >= until {
                        return Err(());
                    }
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(_) => return Err(()),
            }
        }
        // The unchanged real credential/frame/dispatcher path, not direct
        // dispatch(), a fake client or a handler substituted for ownership.
        self.server
            .as_ref()
            .ok_or(())?
            .handle(self.accepted[index].as_mut().ok_or(())?)
            .map_err(|_| ())?;
        if Instant::now() >= until {
            return Err(());
        }
        // Normal serve drops the accepted stream after this successful frame
        // write, supplying response EOF. Keep this SAME reported descriptor,
        // but positively finish its own write direction only after handle0.
        // A failed half-close retains the prefix and admits no next request.
        self.accepted[index]
            .as_ref()
            .ok_or(())?
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| ())?;
        if Instant::now() >= until {
            return Err(());
        }
        while !self.client.as_ref().ok_or(())?.is_finished() {
            if Instant::now() >= until {
                return Err(());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // Only this observed-terminal original thread is joined. Any failure
        // before here retains its handle and all reported stream/server objects.
        let result = self
            .client
            .take()
            .ok_or(())?
            .join()
            .map_err(|_| ())?
            .map_err(|_| ())?;
        if Instant::now() >= until {
            return Err(());
        }
        Ok(result)
    }
}
impl Drop for RpcOriginals {
    fn drop(&mut self) {
        std::mem::forget(self.client.take());
        std::mem::forget(self.owner.take());
        std::mem::forget(self.server.take());
        for stream in &mut self.accepted {
            std::mem::forget(stream.take());
        }
    }
}

fn reply_status(value: &serde_json::Value) -> bool {
    value["ok"] == true
        && value["revision"] == 0
        && value["result"]["runtimeOwnership"] == true
        && value["result"]["actual"] == "disconnected"
        && value["result"]["desired"] == "disconnected"
        && value["result"]["activeProfileId"] == ""
}
fn reply_nochange(value: &serde_json::Value) -> bool {
    value["ok"] == true && value["revision"] == 0 && value["result"]["accepted"] == true
}

fn registered_inner(graph: &mut RpcOriginals, until: Instant) -> Result<(), ()> {
    let root = Path::new(PAIR);
    let runtime = Path::new(RUN);
    let config = root.join("home/.config/omavless");
    let (cutover, desired, host_paths) = disposition::recovery_paths_at(root, runtime);
    if crate::pending_private_transaction::pending_at(&cutover.state_directory) {
        return Err(());
    }
    let saved = read_desired_snapshot(&desired, UID).map_err(|_| ())?;
    if saved.connected || !saved.profile_id.is_empty() {
        return Err(());
    }
    let before = fault_matrix::metadata_bound_read(
        &config.join("profiles.json"),
        omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
    )
    .ok_or(())?;
    if !serde_json::from_slice::<serde_json::Value>(&before).is_ok_and(|v| {
        v["onboardingComplete"] == true && v["profiles"].as_array().is_some_and(Vec::is_empty)
    }) {
        return Err(());
    }
    for service in [
        crate::production_observation::LEGACY_SERVICE,
        crate::production_observation::RUST_SERVICE,
    ] {
        let state = crate::production_observation::service_state_with_timeout(
            Path::new("/usr/bin/systemctl"),
            service,
            Duration::from_millis(250),
        )
        .map_err(|_| ())?;
        if state.active
            || state.main_pid != 0
            || state.exit_status != 0
            || state.result != "success"
        {
            return Err(());
        }
    }
    let mut host = NativeLifecycleHost::new(host_paths, UID).map_err(|_| ())?;
    if !host.fresh_observation(&saved).is_ok_and(empty) || Instant::now() >= until {
        return Err(());
    }
    let paths = RuntimePaths::below(runtime);
    graph.server = Some(crate::RuntimeServer::bind(paths.clone()).map_err(|_| ())?);
    graph.owner = Some(
        ProductionNativeOwner::initialize(
            host,
            desired,
            &config.join("profiles.json"),
            cutover.clone(),
            UID,
        )
        .map_err(|_| ())?,
    );
    let owner = graph.owner.as_mut().ok_or(())?;
    if owner.actual() != ActualState::Disconnected
        || owner.startup_outcome().changed
        || owner.login_ready()
        || !owner.rust_ownership_available()
        || Instant::now() >= until
    {
        return Err(());
    }
    let transport = crate::subscription_transport::HttpsSubscriptionTransport::new();
    // register_native_owner is the existing nonfallible installation; prepare
    // transport first, then move once into the already held real server.
    graph
        .server
        .as_mut()
        .ok_or(())?
        .register_native_owner(graph.owner.take().ok_or(())?, transport);
    graph
        .server
        .as_ref()
        .ok_or(())?
        .listener
        .set_nonblocking(true)
        .map_err(|_| ())?;

    if !reply_status(&graph.exchange(0, &paths, "status.get", serde_json::json!({}), until)?) {
        return Err(());
    }
    let profiles = graph.exchange(1, &paths, "profiles.list", serde_json::json!({}), until)?;
    if profiles["ok"] != true
        || !profiles["result"]["profiles"]
            .as_array()
            .is_some_and(Vec::is_empty)
    {
        return Err(());
    }
    let params = serde_json::json!({"operationId":"registered-onboarding","expectedRevision":0});
    if !reply_nochange(&graph.exchange(2, &paths, "onboarding.complete", params.clone(), until)?) {
        return Err(());
    }
    if !reply_nochange(&graph.exchange(3, &paths, "onboarding.complete", params, until)?) {
        return Err(());
    }
    let capabilities =
        graph.exchange(4, &paths, "capabilities.get", serde_json::json!({}), until)?;
    let methods = capabilities["result"]["methods"].as_array().ok_or(())?;
    if capabilities["ok"] != true
        || capabilities["result"]["runtimeOwnership"] != true
        || !methods.iter().any(|v| v == "onboarding.complete")
        || methods.iter().any(|v| v == "startup.configure")
    {
        return Err(());
    }
    let denied = graph.exchange(5, &paths, "startup.configure", serde_json::json!({"enabled":false,"target":"last","profileId":"","mode":"rule","operationId":"unavailable-startup","expectedRevision":0}), until)?;
    if denied["ok"] != false
        || denied["error"]["code"] != "capability_unavailable"
        || denied["revision"] != 0
    {
        return Err(());
    }
    if !reply_status(&graph.exchange(6, &paths, "status.get", serde_json::json!({}), until)?) {
        return Err(());
    }
    if fault_matrix::metadata_bound_read(
        &config.join("profiles.json"),
        omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
    )
    .as_deref()
        != Some(before.as_slice())
        || crate::pending_private_transaction::pending_at(&cutover.state_directory)
        || Instant::now() >= until
    {
        return Err(());
    }
    Ok(())
}

#[test]
#[ignore = "ROOT-only AFTER all26 originals0; fresh normal owner registered real fixture RPC"]
fn isolated_native_registered_fixture_rpc_nochange() {
    disposition::selected();
    let until = Instant::now() + Duration::from_secs(90);
    let mut graph =
        RpcOriginals::reserve().expect("fixed_native_vm_registered_reservation_refused");
    let result = registered_inner(&mut graph, until);
    std::mem::forget(graph); // before output/assert, including failure path
    assert!(
        result.is_ok() && Instant::now() < until,
        "fixed_native_vm_registered_rpc_refused"
    );
    println!("T4_NATIVE_REGISTERED_FIXTURE_RPC_NOCHANGE_COMPLETED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}

#[test]
fn registered_reply_controls_do_not_mint_current_or_login_authority() {
    let good = serde_json::json!({"ok":true,"revision":0,"result":{"runtimeOwnership":true,"actual":"disconnected","desired":"disconnected","activeProfileId":""}});
    assert!(reply_status(&good));
    for (key, value) in [
        ("runtimeOwnership", serde_json::json!(false)),
        ("actual", serde_json::json!("connected")),
        ("desired", serde_json::json!("connected")),
        ("activeProfileId", serde_json::json!("public-other")),
    ] {
        let mut changed = good.clone();
        changed["result"][key] = value;
        assert!(!reply_status(&changed));
    }
    let ack = serde_json::json!({"ok":true,"revision":0,"result":{"accepted":true}});
    assert!(reply_nochange(&ack));
    for revision in [1, 2] {
        let mut changed = ack.clone();
        changed["revision"] = serde_json::json!(revision);
        assert!(!reply_nochange(&changed));
    }
    assert_eq!(CALLS, 7);
    assert_eq!(
        PAIR,
        "/home/kdk_vm/.cache/t4-native-ordinary-review26/operation"
    );
    assert_eq!(RUN, "/run/user/1000/t4n26/operation");
}

#[test]
fn registered_exchange_refuses_missing_original_before_client() {
    let mut graph = RpcOriginals::reserve().unwrap();
    let paths = RuntimePaths::below(Path::new("/unselected-public-fixture"));
    assert!(
        graph
            .exchange(
                0,
                &paths,
                "status.get",
                serde_json::json!({}),
                Instant::now() + Duration::from_secs(1)
            )
            .is_err()
    );
    assert!(graph.client.is_none() && graph.accepted.iter().all(Option::is_none));
}

#[test]
fn registered_exchange_real_local_unix_path_keeps_reported_stream_until_positive_release() {
    let root = crate::test_temp::directory("rpc-retained").unwrap();
    let paths = RuntimePaths::below(&root);
    let mut graph = RpcOriginals::reserve().unwrap();
    graph.server = Some(crate::RuntimeServer::bind(paths.clone()).unwrap());
    graph
        .server
        .as_ref()
        .unwrap()
        .listener
        .set_nonblocking(true)
        .unwrap();
    let response = graph
        .exchange(
            0,
            &paths,
            "status.get",
            serde_json::json!({}),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
    assert!(response["ok"] == true && response["result"]["runtimeOwnership"] == false);
    assert!(graph.accepted[0].is_some() && graph.client.is_none());
    // Same real server and successful baseline: these failures cannot be
    // preexisting missing-server/path refusals masquerading as cut coverage.
    for (index, until) in [
        (0, Instant::now() + Duration::from_secs(1)),
        (CALLS, Instant::now() + Duration::from_secs(1)),
        (1, Instant::now()),
    ] {
        assert!(
            graph
                .exchange(index, &paths, "status.get", serde_json::json!({}), until)
                .is_err()
        );
        assert!(
            graph.client.is_none()
                && graph.accepted[0].is_some()
                && graph.accepted[1..].iter().all(Option::is_none)
        );
    }
    graph.client = Some(std::thread::spawn(|| Err(crate::RuntimeError::Protocol)));
    assert!(
        graph
            .exchange(
                1,
                &paths,
                "status.get",
                serde_json::json!({}),
                Instant::now() + Duration::from_secs(1)
            )
            .is_err()
    );
    assert!(graph.accepted[1..].iter().all(Option::is_none));
    assert_eq!(
        graph.client.take().unwrap().join().unwrap(),
        Err(crate::RuntimeError::Protocol)
    );
    // This fully positive local read-only fixture is not native admission.
    // Release only after original client join0 and complete response framing.
    drop(graph.accepted[0].take());
    drop(graph.server.take());
    fs::remove_dir_all(root).unwrap();
}
