// SPDX-License-Identifier: MIT

//! Fixed-path production host composition for the explicit R5 cutover.
//!
//! This module deliberately exposes no CLI command. It binds the accepted pure
//! cutover transaction to private desired/ownership state, the two fixed user
//! services, the private runtime socket and credential-safe host observation.
//! The Omarchy compatibility bridge remains an injected fixed-purpose adapter;
//! the transaction cannot be made reachable until that adapter is accepted.

use crate::RuntimePaths;
use crate::call;
use crate::cutover::{
    CutoverPaths, CutoverReadiness, MigrationLock, OwnershipMarker, OwnershipObservation,
    TransitionBootstrap, read_marker, write_marker_locked,
};
use crate::cutover_transaction::{
    BridgeTarget, CandidateIdentity, CutoverHostError, CutoverTransactionHost,
};
use crate::desired::{
    DesiredPaths, DesiredState, MAX_GENERATION, RoutingMode, read_desired, write_desired,
};
use crate::production_observation::{
    LEGACY_SERVICE, ProductionObservationPaths, ProductionOwnershipObserver, RUST_SERVICE,
    SERVICE_QUERY_TIMEOUT, service_state_with_timeout,
};
use nix::unistd::Uid;
use omavless_domain::config::MAX_TEMPLATE_BYTES;
use omavless_domain::private_store::parse_private_store;
use omavless_store::{atomic_replace_private, read_private_utf8};
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SERVICE_ACTION_TIMEOUT: Duration = Duration::from_secs(15);
const SERVICE_SETTLE_TIMEOUT: Duration = Duration::from_secs(15);
const SERVICE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Fixed-purpose frontend bridge operation. Implementations may select only a
/// semantic legacy or Rust target; paths, commands and arbitrary arguments are
/// never supplied by the transaction or an IPC client.
pub trait ProductionPluginBridge {
    fn switch(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError>;
}

/// All filesystem and service entry points used by the production host. This
/// type intentionally has no `Debug` implementation because some paths identify
/// the current user's private profile store.
#[derive(Clone)]
pub struct ProductionCutoverPaths {
    pub systemctl: PathBuf,
    pub observation: ProductionObservationPaths,
    pub runtime: RuntimePaths,
    pub cutover: CutoverPaths,
    pub desired: DesiredPaths,
    pub store: PathBuf,
    pub template: PathBuf,
    pub active_config: PathBuf,
    pub legacy_controller: PathBuf,
}

impl ProductionCutoverPaths {
    #[must_use]
    pub fn below(
        systemctl: PathBuf,
        home: &Path,
        runtime_base: &Path,
        state_base: &Path,
        proc_root: PathBuf,
        sys_class_net: PathBuf,
        uid: u32,
    ) -> Self {
        let observation = ProductionObservationPaths::below(
            systemctl.clone(),
            home,
            runtime_base,
            proc_root,
            sys_class_net,
            uid,
        );
        let runtime = RuntimePaths::below(runtime_base);
        let cutover = CutoverPaths::below(runtime_base, state_base, uid);
        let desired = DesiredPaths::below(state_base);
        Self {
            systemctl,
            store: observation.store.clone(),
            template: observation.template.clone(),
            active_config: observation.active_config.clone(),
            legacy_controller: observation.legacy_controller.clone(),
            observation,
            runtime,
            cutover,
            desired,
        }
    }

    pub fn current(uid: u32) -> Result<Self, CutoverHostError> {
        let home = env::var_os("OMAVLESS_HOME")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or(CutoverHostError)?;
        let runtime_base = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")));
        let state_base = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        if !home.is_absolute() || !runtime_base.is_absolute() || !state_base.is_absolute() {
            return Err(CutoverHostError);
        }
        Ok(Self::below(
            PathBuf::from("/usr/bin/systemctl"),
            &home,
            &runtime_base,
            &state_base,
            PathBuf::from("/proc"),
            PathBuf::from("/sys/class/net"),
            uid,
        ))
    }
}

/// Private staging intent retained only for rollback config regeneration. It
/// intentionally has no formatting, cloning or serialization implementation.
struct StagedIntent {
    profile_id: String,
    mode: RoutingMode,
}

/// Production transaction host with the shared migration lease initially held.
/// Construction alone performs no service, marker, desired-state or bridge
/// mutation.
pub struct ProductionCutoverHost<B> {
    paths: ProductionCutoverPaths,
    uid: u32,
    bridge: B,
    lock: Option<MigrationLock>,
    captured_desired: Option<DesiredState>,
    staged_desired: Option<DesiredState>,
    staged_intent: Option<StagedIntent>,
}

impl<B: ProductionPluginBridge> ProductionCutoverHost<B> {
    pub fn new(
        paths: ProductionCutoverPaths,
        uid: u32,
        bridge: B,
    ) -> Result<Self, CutoverHostError> {
        // Construct the observer before acquiring the lease so unsafe fixed
        // paths fail without creating the migration lock file.
        ProductionOwnershipObserver::new(paths.observation.clone(), uid)
            .map_err(|_| CutoverHostError)?;
        let lock = MigrationLock::acquire(&paths.cutover, uid).map_err(|_| CutoverHostError)?;
        Ok(Self {
            paths,
            uid,
            bridge,
            lock: Some(lock),
            captured_desired: None,
            staged_desired: None,
            staged_intent: None,
        })
    }

    pub fn current(bridge: B) -> Result<Self, CutoverHostError> {
        let uid = Uid::current().as_raw();
        Self::new(ProductionCutoverPaths::current(uid)?, uid, bridge)
    }

    fn lock(&self) -> Result<&MigrationLock, CutoverHostError> {
        self.lock.as_ref().ok_or(CutoverHostError)
    }

    fn observer(&self) -> Result<ProductionOwnershipObserver, CutoverHostError> {
        ProductionOwnershipObserver::new(self.paths.observation.clone(), self.uid)
            .map_err(|_| CutoverHostError)
    }

    fn run_service_action(&self, action: &str, service: &str) -> Result<(), CutoverHostError> {
        if !matches!(action, "start" | "stop") || !matches!(service, LEGACY_SERVICE | RUST_SERVICE)
        {
            return Err(CutoverHostError);
        }
        let mut child = Command::new(&self.paths.systemctl)
            .args(["--user", action, service])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| CutoverHostError)?;
        let deadline = Instant::now() + SERVICE_ACTION_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) | Err(_) => return Err(CutoverHostError),
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(SERVICE_POLL_INTERVAL);
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(CutoverHostError);
                }
            }
        }
    }

    fn wait_service(&self, service: &str, active: bool) -> Result<(), CutoverHostError> {
        let deadline = Instant::now() + SERVICE_SETTLE_TIMEOUT;
        loop {
            let state =
                service_state_with_timeout(&self.paths.systemctl, service, SERVICE_QUERY_TIMEOUT)
                    .map_err(|_| CutoverHostError)?;
            if state.active == active {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(CutoverHostError);
            }
            thread::sleep(SERVICE_POLL_INTERVAL);
        }
    }

    fn call_runtime(&self, method: &str, params: Value) -> Result<Value, CutoverHostError> {
        call(&self.paths.runtime, method, params).map_err(|_| CutoverHostError)
    }

    fn bootstrap_runtime(&self, bootstrap: TransitionBootstrap) -> Result<Value, CutoverHostError> {
        let deadline = Instant::now() + SERVICE_SETTLE_TIMEOUT;
        loop {
            match call(
                &self.paths.runtime,
                "runtime.transitionBootstrap",
                json!({"preparingGeneration": bootstrap.preparing_generation()}),
            ) {
                Ok(response) if response["ok"] == true => return Ok(response),
                Ok(response) if response["error"]["retryable"] == true => {}
                Ok(_) => return Err(CutoverHostError),
                Err(_) => {}
            }
            if Instant::now() >= deadline {
                return Err(CutoverHostError);
            }
            thread::sleep(SERVICE_POLL_INTERVAL);
        }
    }

    fn identity_from_hello(
        &self,
        bootstrap: TransitionBootstrap,
    ) -> Result<CandidateIdentity, CutoverHostError> {
        let response = self.call_runtime("system.hello", json!({"versions": [1]}))?;
        if response["ok"] != true
            || response["result"]["version"] != 1
            || response["result"]["runtimeOwnership"] != false
        {
            return Err(CutoverHostError);
        }
        let instance_id = response["result"]["instanceId"]
            .as_str()
            .ok_or(CutoverHostError)?;
        CandidateIdentity::from_instance_id(instance_id, bootstrap)
    }

    fn parse_mode(text: &str) -> Result<RoutingMode, CutoverHostError> {
        if text.len() > MAX_TEMPLATE_BYTES {
            return Err(CutoverHostError);
        }
        let mut mode = None;
        for line in text.lines() {
            if line.starts_with([' ', '\t']) {
                continue;
            }
            let Some((key, raw_value)) = line.split_once(':') else {
                continue;
            };
            if key.trim() != "mode" {
                continue;
            }
            if mode.is_some() {
                return Err(CutoverHostError);
            }
            mode = Some(parse_mode_scalar(raw_value)?);
        }
        mode.ok_or(CutoverHostError)
    }

    fn prepare_staged_desired(
        &self,
        readiness: CutoverReadiness,
        captured: &DesiredState,
    ) -> Result<(DesiredState, Option<StagedIntent>), CutoverHostError> {
        let store_text =
            read_private_utf8(&self.paths.store, self.uid).map_err(|_| CutoverHostError)?;
        let template =
            read_private_utf8(&self.paths.template, self.uid).map_err(|_| CutoverHostError)?;
        let store = parse_private_store(&store_text).map_err(|_| CutoverHostError)?;
        let generation = captured
            .generation
            .checked_add(1)
            .filter(|value| *value <= MAX_GENERATION)
            .ok_or(CutoverHostError)?;
        let (connected, profile_id, mode, intent) = match readiness {
            CutoverReadiness::ReadyDisconnected => {
                (false, String::new(), Self::parse_mode(&template)?, None)
            }
            CutoverReadiness::ReadyToAdopt => {
                let active = read_private_utf8(&self.paths.active_config, self.uid)
                    .map_err(|_| CutoverHostError)?;
                let mode = Self::parse_mode(&active)?;
                let controller = self
                    .paths
                    .legacy_controller
                    .to_str()
                    .ok_or(CutoverHostError)?;
                if !store
                    .active_config_matches(&template, controller, &active)
                    .map_err(|_| CutoverHostError)?
                {
                    return Err(CutoverHostError);
                }
                let profile_id = store
                    .active_profile_id()
                    .ok_or(CutoverHostError)?
                    .to_owned();
                let intent = StagedIntent {
                    profile_id: profile_id.clone(),
                    mode,
                };
                (true, profile_id, mode, Some(intent))
            }
            CutoverReadiness::Blocked(_) => return Err(CutoverHostError),
        };
        Ok((
            DesiredState {
                schema_version: crate::desired::DESIRED_SCHEMA_VERSION,
                generation,
                connected,
                profile_id,
                mode,
            },
            intent,
        ))
    }

    fn restore_legacy_config(&self, intent: &StagedIntent) -> Result<(), CutoverHostError> {
        let store_text =
            read_private_utf8(&self.paths.store, self.uid).map_err(|_| CutoverHostError)?;
        let template =
            read_private_utf8(&self.paths.template, self.uid).map_err(|_| CutoverHostError)?;
        let store = parse_private_store(&store_text).map_err(|_| CutoverHostError)?;
        let controller = self
            .paths
            .legacy_controller
            .to_str()
            .ok_or(CutoverHostError)?;
        let config = store
            .prepare_config_mode(
                &intent.profile_id,
                &template,
                controller,
                intent.mode.as_str(),
            )
            .map_err(|_| CutoverHostError)?;
        atomic_replace_private(&self.paths.active_config, config.as_bytes(), self.uid)
            .map_err(|_| CutoverHostError)
    }
}

/// Parse only the three scalar spellings which OmaVLESS itself emits. The
/// trailing fragment may be a YAML comment, but a `#` inside a quoted scalar
/// is data and must not make an otherwise invalid mode look valid.
fn parse_mode_scalar(raw: &str) -> Result<RoutingMode, CutoverHostError> {
    let raw = raw.trim_start();
    if raw.is_empty() {
        return Err(CutoverHostError);
    }
    let (value, tail) = if let Some(quoted) = raw.strip_prefix('\'') {
        let end = quoted.find('\'').ok_or(CutoverHostError)?;
        (&quoted[..end], &quoted[end + 1..])
    } else if let Some(quoted) = raw.strip_prefix('"') {
        let end = quoted.find('"').ok_or(CutoverHostError)?;
        (&quoted[..end], &quoted[end + 1..])
    } else {
        let end = raw
            .find(|character: char| character.is_ascii_whitespace() || character == '#')
            .unwrap_or(raw.len());
        (&raw[..end], &raw[end..])
    };
    let tail = tail.trim_start();
    if !tail.is_empty() && !tail.starts_with('#') {
        return Err(CutoverHostError);
    }
    match value.to_ascii_lowercase().as_str() {
        "rule" => Ok(RoutingMode::Rule),
        "global" => Ok(RoutingMode::Global),
        "direct" => Ok(RoutingMode::Direct),
        _ => Err(CutoverHostError),
    }
}

impl<B: ProductionPluginBridge> CutoverTransactionHost for ProductionCutoverHost<B> {
    type DesiredSnapshot = DesiredState;

    fn observe(&mut self) -> Result<OwnershipObservation, CutoverHostError> {
        self.observer()?.observe().map_err(|_| CutoverHostError)
    }

    fn read_marker(&mut self) -> Result<OwnershipMarker, CutoverHostError> {
        read_marker(&self.paths.cutover, self.uid).map_err(|_| CutoverHostError)
    }

    fn persist_marker(
        &mut self,
        expected: &OwnershipMarker,
        next: &OwnershipMarker,
    ) -> Result<(), CutoverHostError> {
        write_marker_locked(&self.paths.cutover, self.uid, self.lock()?, expected, next)
            .map_err(|_| CutoverHostError)
    }

    fn capture_desired(&mut self) -> Result<Self::DesiredSnapshot, CutoverHostError> {
        let desired = read_desired(&self.paths.desired, self.uid).map_err(|_| CutoverHostError)?;
        self.captured_desired = Some(desired.clone());
        Ok(desired)
    }

    fn stage_desired(&mut self, readiness: CutoverReadiness) -> Result<(), CutoverHostError> {
        let captured = self.captured_desired.as_ref().ok_or(CutoverHostError)?;
        let (desired, intent) = self.prepare_staged_desired(readiness, captured)?;
        write_desired(&self.paths.desired, self.uid, &desired).map_err(|_| CutoverHostError)?;
        self.staged_desired = Some(desired);
        self.staged_intent = intent;
        Ok(())
    }

    fn stop_legacy(&mut self) -> Result<(), CutoverHostError> {
        self.run_service_action("stop", LEGACY_SERVICE)?;
        self.wait_service(LEGACY_SERVICE, false)
    }

    fn release_for_candidate(
        &mut self,
        preparing: &OwnershipMarker,
    ) -> Result<(), CutoverHostError> {
        if self.read_marker()? != *preparing || self.lock.is_none() {
            return Err(CutoverHostError);
        }
        self.lock.take();
        Ok(())
    }

    fn start_rust_candidate(
        &mut self,
        bootstrap: TransitionBootstrap,
    ) -> Result<CandidateIdentity, CutoverHostError> {
        if self.lock.is_some() {
            return Err(CutoverHostError);
        }
        self.run_service_action("start", RUST_SERVICE)?;
        self.wait_service(RUST_SERVICE, true)?;
        // ActiveState may become active just before the private socket is
        // bound. Retry only connection failures and explicitly retryable
        // bootstrap replies within the fixed service-settle budget.
        let response = self.bootstrap_runtime(bootstrap)?;
        if response["ok"] != true
            || response["result"]["preparingGeneration"] != bootstrap.preparing_generation()
            || response["result"]["rustGeneration"] != bootstrap.rust_generation()
            || response["result"]["runtimeOwnership"] != false
        {
            return Err(CutoverHostError);
        }
        let instance_id = response["result"]["instanceId"]
            .as_str()
            .ok_or(CutoverHostError)?;
        CandidateIdentity::from_instance_id(instance_id, bootstrap)
    }

    fn hello_compatible(&mut self, candidate: CandidateIdentity) -> Result<bool, CutoverHostError> {
        Ok(self.identity_from_hello(candidate.bootstrap())? == candidate)
    }

    fn status_consistent(
        &mut self,
        candidate: CandidateIdentity,
    ) -> Result<bool, CutoverHostError> {
        let desired = self.staged_desired.as_ref().ok_or(CutoverHostError)?;
        let response = self.call_runtime("status.get", json!({}))?;
        let result = &response["result"];
        let expected = if desired.connected {
            "connected"
        } else {
            "disconnected"
        };
        let status_matches = response["ok"] == true
            && result["desired"] == expected
            && result["actual"] == expected
            && result["activeProfileId"] == desired.profile_id
            && result["mode"] == desired.mode.as_str()
            && result["transition"] == "cutoverPreparing"
            && result["runtimeOwnership"] == false;
        Ok(status_matches && self.identity_from_hello(candidate.bootstrap())? == candidate)
    }

    fn reacquire_after_candidate(
        &mut self,
        preparing: &OwnershipMarker,
    ) -> Result<(), CutoverHostError> {
        if self.lock.is_some() {
            return Err(CutoverHostError);
        }
        let lock =
            MigrationLock::acquire(&self.paths.cutover, self.uid).map_err(|_| CutoverHostError)?;
        let marker = read_marker(&self.paths.cutover, self.uid).map_err(|_| CutoverHostError)?;
        if marker != *preparing {
            drop(lock);
            return Err(CutoverHostError);
        }
        self.lock = Some(lock);
        Ok(())
    }

    fn observe_candidate(
        &mut self,
        candidate: CandidateIdentity,
    ) -> Result<OwnershipObservation, CutoverHostError> {
        if self.identity_from_hello(candidate.bootstrap())? != candidate {
            return Err(CutoverHostError);
        }
        self.observe()
    }

    fn switch_bridge(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError> {
        self.bridge.switch(target)
    }

    fn stop_rust(&mut self) -> Result<(), CutoverHostError> {
        self.run_service_action("stop", RUST_SERVICE)?;
        self.wait_service(RUST_SERVICE, false)?;
        let observation = self.observe()?;
        if observation.rust_owner_active
            || observation.rust_controller_ready
            || observation.core_count != 0
            || observation.tun_count != 0
        {
            return Err(CutoverHostError);
        }
        Ok(())
    }

    fn restore_legacy(
        &mut self,
        readiness: CutoverReadiness,
        desired: &Self::DesiredSnapshot,
    ) -> Result<(), CutoverHostError> {
        if self.lock.is_none() || self.captured_desired.as_ref() != Some(desired) {
            return Err(CutoverHostError);
        }
        write_desired(&self.paths.desired, self.uid, desired).map_err(|_| CutoverHostError)?;
        match readiness {
            CutoverReadiness::ReadyDisconnected => {
                self.run_service_action("stop", LEGACY_SERVICE)?;
                self.wait_service(LEGACY_SERVICE, false)
            }
            CutoverReadiness::ReadyToAdopt => {
                let intent = self.staged_intent.as_ref().ok_or(CutoverHostError)?;
                self.restore_legacy_config(intent)?;
                self.run_service_action("start", LEGACY_SERVICE)?;
                self.wait_service(LEGACY_SERVICE, true)
            }
            CutoverReadiness::Blocked(_) => Err(CutoverHostError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::{OwnershipPhase, read_marker};
    use crate::cutover_transaction::{
        CutoverTransactionError, CutoverTransactionHost, execute_cutover,
    };
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";

    #[derive(Clone)]
    struct FakeBridge {
        calls: Arc<Mutex<Vec<BridgeTarget>>>,
        fail_rust: bool,
    }

    impl FakeBridge {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                fail_rust: false,
            }
        }

        fn failing_rust() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                fail_rust: true,
            }
        }
    }

    impl ProductionPluginBridge for FakeBridge {
        fn switch(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError> {
            self.calls
                .lock()
                .map_err(|_| CutoverHostError)?
                .push(target);
            if self.fail_rust && target == BridgeTarget::Rust {
                return Err(CutoverHostError);
            }
            Ok(())
        }
    }

    struct Fixture {
        root: PathBuf,
        uid: u32,
        paths: ProductionCutoverPaths,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            // Keep the root short enough for Linux's bounded Unix-socket path.
            let root = env::temp_dir().join(format!("ovpc-{label}-{}-{nonce}", std::process::id()));
            let home = root.join("home");
            let runtime = root.join("runtime");
            let state = root.join("state");
            let proc_root = root.join("proc");
            let sys_class_net = root.join("sys/class/net");
            for path in [
                &root,
                &home,
                &runtime,
                &state,
                &proc_root,
                &sys_class_net,
                &home.join(".config/omavless"),
            ] {
                fs::create_dir_all(path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            let systemctl = root.join("systemctl");
            fs::write(
                &systemctl,
                "#!/bin/sh\nif [ \"$2\" = show ]; then printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n'; fi\nexit 0\n",
            )
            .unwrap();
            fs::set_permissions(&systemctl, fs::Permissions::from_mode(0o700)).unwrap();
            let paths = ProductionCutoverPaths::below(
                systemctl,
                &home,
                &runtime,
                &state,
                proc_root,
                sys_class_net,
                uid,
            );
            Self { root, uid, paths }
        }

        fn write_private(&self, path: &Path, value: &str) {
            fs::write(path, value).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn disconnected_staging_preserves_template_mode_and_is_private() {
        let fixture = Fixture::new("disconnected");
        fixture.write_private(
            &fixture.paths.store,
            r#"{"version":3,"activeId":"","lastId":"","profiles":[],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":false}"#,
        );
        fixture.write_private(
            &fixture.paths.template,
            "mode: direct\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
        );
        let mut host =
            ProductionCutoverHost::new(fixture.paths.clone(), fixture.uid, FakeBridge::new())
                .unwrap();
        let captured = host.capture_desired().unwrap();
        assert_eq!(captured, DesiredState::default());
        host.stage_desired(CutoverReadiness::ReadyDisconnected)
            .unwrap();
        let desired = read_desired(&host.paths.desired, fixture.uid).unwrap();
        assert!(!desired.connected);
        assert_eq!(desired.generation, 1);
        assert_eq!(desired.mode, RoutingMode::Direct);
        assert_eq!(
            fs::metadata(&host.paths.desired.file)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn connected_staging_requires_exact_legacy_config_and_keeps_identity_private() {
        let fixture = Fixture::new("connected");
        let private_uri = "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private";
        let store_text = format!(
            r#"{{"version":3,"activeId":"{PROFILE_ID}","lastId":"{PROFILE_ID}","profiles":[{{"id":"{PROFILE_ID}","name":"Private","uri":"{private_uri}","protocol":"vless"}}],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{{"enabled":false,"target":"last","profileId":"","mode":"rule"}},"onboardingComplete":true}}"#
        );
        let template = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n";
        fixture.write_private(&fixture.paths.store, &store_text);
        fixture.write_private(&fixture.paths.template, template);
        let store = parse_private_store(&store_text).unwrap();
        let active = store
            .prepare_config_mode(
                PROFILE_ID,
                template,
                fixture.paths.legacy_controller.to_str().unwrap(),
                "global",
            )
            .unwrap();
        fixture.write_private(&fixture.paths.active_config, &active);

        let mut host =
            ProductionCutoverHost::new(fixture.paths.clone(), fixture.uid, FakeBridge::new())
                .unwrap();
        host.capture_desired().unwrap();
        host.stage_desired(CutoverReadiness::ReadyToAdopt).unwrap();
        let desired = read_desired(&host.paths.desired, fixture.uid).unwrap();
        assert!(desired.connected);
        assert_eq!(desired.profile_id, PROFILE_ID);
        assert_eq!(desired.mode, RoutingMode::Global);
        let rendered = format!("{:?}", host.stage_desired(CutoverReadiness::ReadyToAdopt));
        for private in ["11111111", "203.0.113.1", "Private"] {
            assert!(!rendered.contains(private));
        }
    }

    #[test]
    fn malformed_or_duplicate_mode_fails_before_desired_write() {
        let fixture = Fixture::new("bad-mode");
        fixture.write_private(
            &fixture.paths.store,
            r#"{"version":3,"activeId":"","lastId":"","profiles":[],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":false}"#,
        );
        fixture.write_private(
            &fixture.paths.template,
            "mode: rule\nmode: global\nproxies:\n{{OMAVLESS_PROXY}}\n",
        );
        let mut host =
            ProductionCutoverHost::new(fixture.paths.clone(), fixture.uid, FakeBridge::new())
                .unwrap();
        host.capture_desired().unwrap();
        assert!(
            host.stage_desired(CutoverReadiness::ReadyDisconnected)
                .is_err()
        );
        assert!(!host.paths.desired.file.exists());
    }

    #[test]
    fn mode_scalar_accepts_emitted_forms_and_rejects_quoted_comment_confusion() {
        for (raw, expected) in [
            (" rule", RoutingMode::Rule),
            ("GLOBAL # selected by the user", RoutingMode::Global),
            (" 'direct' # selected by the user", RoutingMode::Direct),
            (" \"rule\"", RoutingMode::Rule),
        ] {
            assert_eq!(parse_mode_scalar(raw).unwrap(), expected);
        }
        for raw in [
            "",
            "rule extra",
            "'rule # not a comment'",
            "\"global # not a comment\"",
            "'direct' trailing",
            "\"rule",
        ] {
            assert_eq!(parse_mode_scalar(raw), Err(CutoverHostError));
        }
    }

    #[test]
    fn disconnected_transaction_uses_fixed_services_and_commits_exact_candidate() {
        let fixture = Fixture::new("transaction");
        fixture.write_private(
            &fixture.paths.store,
            r#"{"version":3,"activeId":"","lastId":"","profiles":[],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":false}"#,
        );
        fixture.write_private(
            &fixture.paths.template,
            "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
        );

        let service_state = fixture.root.join("rust-service-state");
        fs::write(&service_state, "inactive\n").unwrap();
        let script = format!(
            "#!/bin/sh\nstate='{}'\ncase \"$2:$3\" in\n  start:omavless-runtime.service) printf 'active\\n' > \"$state\" ; exit 0 ;;\n  stop:omavless-runtime.service) printf 'inactive\\n' > \"$state\" ; exit 0 ;;\n  stop:omavless.service) exit 0 ;;\n  show:omavless.service) printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' ; exit 0 ;;\n  show:omavless-runtime.service) value=$(tr -d '\\n' < \"$state\"); printf 'ActiveState=%s\\nMainPID=42\\nExecMainStatus=0\\nResult=success\\n' \"$value\" ; exit 0 ;;\nesac\nexit 9\n",
            service_state.display()
        );
        fs::write(&fixture.paths.systemctl, script).unwrap();
        fs::set_permissions(&fixture.paths.systemctl, fs::Permissions::from_mode(0o700)).unwrap();

        let socket = fixture.paths.runtime.socket.clone();
        let runtime_directory = fixture.paths.runtime.directory.clone();
        let service_state_for_server = service_state.clone();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while fs::read_to_string(&service_state_for_server).unwrap() != "active\n" {
                assert!(Instant::now() < deadline, "runtime service did not start");
                thread::sleep(Duration::from_millis(10));
            }
            fs::create_dir(&runtime_directory).unwrap();
            fs::set_permissions(&runtime_directory, fs::Permissions::from_mode(0o700)).unwrap();
            let listener = UnixListener::bind(&socket).unwrap();
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
            for _ in 0..5 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request_line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut request_line)
                    .unwrap();
                let request: Value = serde_json::from_str(&request_line).unwrap();
                let method = request["method"].as_str().unwrap();
                let result = match method {
                    "runtime.transitionBootstrap" => json!({
                        "instanceId": "candidate-instance",
                        "preparingGeneration": 1,
                        "rustGeneration": 2,
                        "runtimeOwnership": false
                    }),
                    "system.hello" => json!({
                        "instanceId": "candidate-instance",
                        "version": 1,
                        "versions": [1],
                        "limits": {"requestFrameBytes": 65536, "responseFrameBytes": 262144},
                        "runtimeOwnership": false
                    }),
                    "status.get" => json!({
                        "desired": "disconnected",
                        "actual": "disconnected",
                        "activeProfileId": "",
                        "mode": "rule",
                        "transition": "cutoverPreparing",
                        "runtimeOwnership": false
                    }),
                    _ => panic!("unexpected runtime method"),
                };
                let response = json!({
                    "api": "omavless.control",
                    "version": 1,
                    "id": request["id"],
                    "ok": true,
                    "revision": 0,
                    "result": result
                });
                writeln!(stream, "{}", serde_json::to_string(&response).unwrap()).unwrap();
            }
        });

        let bridge = FakeBridge::new();
        let observed_bridge = bridge.clone();
        let paths = fixture.paths.clone();
        let marker = OwnershipMarker::default();
        let cutover_paths = paths.cutover.clone();
        let mut host = ProductionCutoverHost::new(paths, fixture.uid, bridge).unwrap();
        let outcome = execute_cutover(&mut host, &marker).unwrap();
        assert_eq!(outcome.marker.phase(), OwnershipPhase::Rust);
        assert_eq!(outcome.marker.generation(), 2);
        assert_eq!(
            read_marker(&cutover_paths, fixture.uid).unwrap(),
            outcome.marker
        );
        assert_eq!(
            *observed_bridge.calls.lock().unwrap(),
            vec![BridgeTarget::Rust]
        );
        server.join().unwrap();
    }

    #[test]
    fn failed_bridge_switch_stops_candidate_and_restores_exact_desired_state() {
        let fixture = Fixture::new("bridge-rollback");
        fixture.write_private(
            &fixture.paths.store,
            r#"{"version":3,"activeId":"","lastId":"","profiles":[],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":false}"#,
        );
        fixture.write_private(
            &fixture.paths.template,
            "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
        );
        let captured = DesiredState {
            generation: 7,
            mode: RoutingMode::Direct,
            ..DesiredState::default()
        };
        write_desired(&fixture.paths.desired, fixture.uid, &captured).unwrap();

        let service_state = fixture.root.join("rust-service-state");
        fs::write(&service_state, "inactive\n").unwrap();
        let socket = fixture.paths.runtime.socket.clone();
        let script = format!(
            "#!/bin/sh\nstate='{}'\nsocket='{}'\ncase \"$2:$3\" in\n  start:omavless-runtime.service) printf 'active\\n' > \"$state\" ; exit 0 ;;\n  stop:omavless-runtime.service) printf 'inactive\\n' > \"$state\" ; rm -f -- \"$socket\" ; exit 0 ;;\n  stop:omavless.service) exit 0 ;;\n  show:omavless.service) printf 'ActiveState=inactive\\nMainPID=0\\nExecMainStatus=0\\nResult=success\\n' ; exit 0 ;;\n  show:omavless-runtime.service) value=$(tr -d '\\n' < \"$state\"); printf 'ActiveState=%s\\nMainPID=42\\nExecMainStatus=0\\nResult=success\\n' \"$value\" ; exit 0 ;;\nesac\nexit 9\n",
            service_state.display(),
            socket.display()
        );
        fs::write(&fixture.paths.systemctl, script).unwrap();
        fs::set_permissions(&fixture.paths.systemctl, fs::Permissions::from_mode(0o700)).unwrap();

        let runtime_directory = fixture.paths.runtime.directory.clone();
        let service_state_for_server = service_state.clone();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while fs::read_to_string(&service_state_for_server).unwrap() != "active\n" {
                assert!(Instant::now() < deadline, "runtime service did not start");
                thread::sleep(Duration::from_millis(10));
            }
            fs::create_dir(&runtime_directory).unwrap();
            fs::set_permissions(&runtime_directory, fs::Permissions::from_mode(0o700)).unwrap();
            let listener = UnixListener::bind(&socket).unwrap();
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request_line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut request_line)
                    .unwrap();
                let request: Value = serde_json::from_str(&request_line).unwrap();
                let result = match request["method"].as_str().unwrap() {
                    "runtime.transitionBootstrap" => json!({
                        "instanceId": "rollback-candidate",
                        "preparingGeneration": 1,
                        "rustGeneration": 2,
                        "runtimeOwnership": false
                    }),
                    "system.hello" => json!({
                        "instanceId": "rollback-candidate",
                        "version": 1,
                        "versions": [1],
                        "limits": {"requestFrameBytes": 65536, "responseFrameBytes": 262144},
                        "runtimeOwnership": false
                    }),
                    "status.get" => json!({
                        "desired": "disconnected",
                        "actual": "disconnected",
                        "activeProfileId": "",
                        "mode": "rule",
                        "transition": "cutoverPreparing",
                        "runtimeOwnership": false
                    }),
                    _ => panic!("unexpected runtime method"),
                };
                let response = json!({
                    "api": "omavless.control",
                    "version": 1,
                    "id": request["id"],
                    "ok": true,
                    "revision": 0,
                    "result": result
                });
                writeln!(stream, "{}", serde_json::to_string(&response).unwrap()).unwrap();
            }
        });

        let bridge = FakeBridge::failing_rust();
        let observed_bridge = bridge.clone();
        let cutover_paths = fixture.paths.cutover.clone();
        let mut host =
            ProductionCutoverHost::new(fixture.paths.clone(), fixture.uid, bridge).unwrap();
        assert_eq!(
            execute_cutover(&mut host, &OwnershipMarker::default()),
            Err(CutoverTransactionError::TransitionFailedRestored)
        );
        server.join().unwrap();

        assert_eq!(
            read_desired(&fixture.paths.desired, fixture.uid).unwrap(),
            captured
        );
        let marker = read_marker(&cutover_paths, fixture.uid).unwrap();
        assert_eq!(marker.phase(), OwnershipPhase::Legacy);
        assert_eq!(marker.generation(), 2);
        assert_eq!(
            *observed_bridge.calls.lock().unwrap(),
            vec![BridgeTarget::Rust, BridgeTarget::Legacy]
        );
        assert_eq!(fs::read_to_string(service_state).unwrap(), "inactive\n");
        assert!(!fixture.paths.runtime.socket.exists());
    }
}
