// SPDX-License-Identifier: MIT

//! Exact response binding for the unified native mutation coordinator.
//!
//! This module routes only the accepted v1 mutation method families. The
//! coordinator supplied here must be constructed with `new_ownership_gated`,
//! which rechecks the durable `rust` marker under the migration lock before
//! every store/lifecycle phase. `RuntimeServer` reaches this binder only after
//! the committed production-owner constructor succeeds.

use crate::lifecycle::LifecycleHost;
use crate::mutation::CachedOutcome;
use crate::mutation_protocol::parse_owner_request;
use crate::native_coordinator::{
    NativeOwnerError, NativeOwnerExecution, NativeTransactionError, OfflineNativeCoordinator,
    PreparedSubscriptionFetch, PreparedSubscriptionRefresh, SubscriptionFetchPreflight,
    SubscriptionRefreshPreflight,
};
use crate::subscription_transport::{SubscriptionTransport, SubscriptionTransportError};
use omavless_control_protocol::{
    ProtocolError, StableErrorCode, error_response, success_response, validate_request,
};
use omavless_domain::subscription_feed::PrivateSubscriptionBody;
use serde_json::{Value, json};

const LIFECYCLE_METHODS: &[&str] = &[
    "connection.connect",
    "connection.disconnect",
    "routing.set_mode",
];
const PROFILE_METHODS: &[&str] = &["profiles.rename", "profiles.favorite", "profiles.delete"];
const SUBSCRIPTION_METHODS: &[&str] = &[
    "subscriptions.add",
    "subscriptions.update",
    "subscriptions.delete",
];

fn retryable(code: StableErrorCode) -> bool {
    matches!(
        code,
        StableErrorCode::Conflict | StableErrorCode::Busy | StableErrorCode::DaemonRestarting
    )
}

fn cached_response(request_id: &str, outcome: CachedOutcome) -> Result<Value, ProtocolError> {
    match outcome.error {
        Some(code) => error_response(request_id, outcome.revision, code, retryable(code), None),
        None => success_response(request_id, outcome.revision, json!({"accepted": true})),
    }
}

fn transaction_response(
    request_id: &str,
    revision: u64,
    error: NativeTransactionError,
) -> Result<Value, ProtocolError> {
    let code = error.stable_code();
    error_response(request_id, revision, code, retryable(code), None)
}

fn execution_response(
    request_id: &str,
    execution: NativeOwnerExecution,
) -> Result<Value, ProtocolError> {
    match execution {
        NativeOwnerExecution::Applied { cached, .. }
        | NativeOwnerExecution::Replay(cached)
        | NativeOwnerExecution::Rejected(cached) => cached_response(request_id, cached),
        NativeOwnerExecution::UncachedPreflightFailure { revision, error } => {
            transaction_response(request_id, revision, error)
        }
    }
}

fn owner_error_response(
    request_id: &str,
    revision: u64,
    error: NativeOwnerError,
) -> Result<Value, ProtocolError> {
    let code = error.stable_code();
    error_response(request_id, revision, code, retryable(code), None)
}

pub(crate) enum RemoteSubscriptionPreflight {
    Fetch(PreparedSubscriptionFetch),
    Respond(Value),
}

pub(crate) enum RemoteSubscriptionRefreshPreflight {
    Fetch(PreparedSubscriptionRefresh),
    Respond(Value),
}

/// Bind reservation-free external-work preflight to the ordinary v1 response
/// envelope. Only add/update can return a private fetch target; delete and all
/// invalid inputs receive a bounded response without transport work.
pub(crate) fn preflight_native_subscription<H: LifecycleHost>(
    owner: &mut OfflineNativeCoordinator<H>,
    request: &Value,
) -> Result<RemoteSubscriptionPreflight, ProtocolError> {
    let request_id = request["id"].as_str().unwrap_or("invalid");
    match owner.preflight_subscription_fetch(request) {
        Ok(SubscriptionFetchPreflight::Ready(prepared)) => {
            Ok(RemoteSubscriptionPreflight::Fetch(prepared))
        }
        Ok(SubscriptionFetchPreflight::Replay(cached)) => {
            cached_response(request_id, cached).map(RemoteSubscriptionPreflight::Respond)
        }
        Err(error) => owner_error_response(request_id, owner.revision(), error)
            .map(RemoteSubscriptionPreflight::Respond),
    }
}

/// Snapshot one existing subscription under the migration lock before its
/// private URL is used outside the serialized owner. The snapshot is retained
/// through completion so a concurrent URL/store change fails closed.
pub(crate) fn preflight_native_subscription_refresh<H: LifecycleHost>(
    owner: &mut OfflineNativeCoordinator<H>,
    request: &Value,
) -> Result<RemoteSubscriptionRefreshPreflight, ProtocolError> {
    let request_id = request["id"].as_str().unwrap_or("invalid");
    match owner.preflight_subscription_refresh(request) {
        Ok(SubscriptionRefreshPreflight::Ready(prepared)) => {
            Ok(RemoteSubscriptionRefreshPreflight::Fetch(prepared))
        }
        Ok(SubscriptionRefreshPreflight::Replay(cached)) => {
            cached_response(request_id, cached).map(RemoteSubscriptionRefreshPreflight::Respond)
        }
        Err(error) => owner_error_response(request_id, owner.revision(), error)
            .map(RemoteSubscriptionRefreshPreflight::Respond),
    }
}

/// Release one bounded explicit editor payload through the private same-user
/// control response. This is the sole runtime response allowed to contain a
/// subscription bearer URL.
pub(crate) fn respond_to_subscription_edit_input<H: LifecycleHost>(
    owner: &mut OfflineNativeCoordinator<H>,
    request: &Value,
) -> Result<Value, ProtocolError> {
    let request_id = request["id"].as_str().unwrap_or("invalid");
    match owner.subscription_edit_input(request) {
        Ok(input) => success_response(
            request_id,
            owner.revision(),
            json!({"name": input.private_name(), "url": input.private_url()}),
        ),
        Err(error) => owner_error_response(request_id, owner.revision(), error),
    }
}

/// Complete one externally fetched subscription request through the same
/// serialized owner and stable response contract as every other mutation.
pub(crate) fn respond_to_fetched_subscription<H, G, N>(
    owner: &mut OfflineNativeCoordinator<H>,
    request: &Value,
    preflight_revision: u64,
    fetched: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
    next_record_id: G,
    now_millis: N,
) -> Result<Value, ProtocolError>
where
    H: LifecycleHost,
    G: FnMut() -> String,
    N: FnOnce() -> u64,
{
    let request_id = request["id"].as_str().unwrap_or("invalid");
    match owner.execute_fetched_subscription(
        request,
        preflight_revision,
        fetched,
        next_record_id,
        now_millis,
    ) {
        Ok(execution) => execution_response(request_id, execution),
        Err(error) => owner_error_response(request_id, owner.revision(), error),
    }
}

/// Complete a refresh using the exact private snapshot captured before the
/// fetch. Neither its URL nor decoded credential material enters the response.
pub(crate) fn respond_to_fetched_subscription_refresh<H, G, N>(
    owner: &mut OfflineNativeCoordinator<H>,
    request: &Value,
    preflight_revision: u64,
    prepared: PreparedSubscriptionRefresh,
    fetched: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
    next_record_id: G,
    now_millis: N,
) -> Result<Value, ProtocolError>
where
    H: LifecycleHost,
    G: FnMut() -> String,
    N: FnOnce() -> u64,
{
    let request_id = request["id"].as_str().unwrap_or("invalid");
    match owner.execute_fetched_subscription_refresh(
        request,
        preflight_revision,
        prepared,
        fetched,
        next_record_id,
        now_millis,
    ) {
        Ok(execution) => execution_response(request_id, execution),
        Err(error) => owner_error_response(request_id, owner.revision(), error),
    }
}

/// Route one decoded mutation request to the single native owner.
///
/// Generated identifiers and timestamps are trusted owner inputs and are used
/// only for subscription add/update. No private mutation value is returned in
/// the response. Read-only and unknown methods are deliberately outside this
/// binder and receive `unknown_method`.
pub fn respond_to_native_mutation<H, T, G, N>(
    owner: &mut OfflineNativeCoordinator<H>,
    request: &Value,
    transport: &T,
    next_record_id: G,
    now_millis: N,
) -> Result<Value, ProtocolError>
where
    H: LifecycleHost,
    T: SubscriptionTransport,
    G: FnMut() -> String,
    N: FnOnce() -> u64,
{
    if let Err(error) = validate_request(request) {
        return error_response("invalid", owner.revision(), error.code(), false, None);
    }
    let request_id = request["id"].as_str().unwrap_or("invalid");
    let method = request["method"].as_str().unwrap_or_default();
    let execution = if LIFECYCLE_METHODS.contains(&method) {
        match parse_owner_request(request) {
            Ok(parsed) => owner.execute_connection(parsed),
            Err(error) => {
                let code = error.stable_code();
                return error_response(request_id, owner.revision(), code, retryable(code), None);
            }
        }
    } else if PROFILE_METHODS.contains(&method) {
        owner.execute_profile(request)
    } else if SUBSCRIPTION_METHODS.contains(&method) {
        owner.execute_subscription(request, transport, next_record_id, now_millis)
    } else {
        return error_response(
            request_id,
            owner.revision(),
            StableErrorCode::UnknownMethod,
            false,
            None,
        );
    };
    match execution {
        Ok(execution) => execution_response(request_id, execution),
        Err(error) => owner_error_response(request_id, owner.revision(), error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::{CutoverPaths, OwnershipPhase};
    use crate::desired::{
        DesiredPaths, DesiredState, OwnedObservation, RoutingMode, write_desired,
    };
    use crate::lifecycle::HostStepError;
    use crate::subscription_transport::SubscriptionTransportError;
    use omavless_control_protocol::make_request;
    use omavless_domain::subscription_feed::PrivateSubscriptionBody;
    use serde_json::json;
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";
    const MANAGED_PROFILE: &str = "20000000-0000-4000-8000-000000000001";
    const PRIVATE_URL: &str = "https://provider.invalid/private-token";
    const PRIVATE_BODY: &str =
        "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Managed";

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

    struct FakeHost {
        observation: OwnedObservation,
        calls: usize,
    }

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.calls += 1;
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            self.observation = healthy();
            Ok(())
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            self.observation = empty();
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }
    }

    struct FakeTransport {
        calls: Cell<usize>,
    }

    impl SubscriptionTransport for FakeTransport {
        fn fetch(&self, _url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
            self.calls.set(self.calls.get() + 1);
            PrivateSubscriptionBody::from_bytes(PRIVATE_BODY.as_bytes().to_vec())
                .map_err(|_| SubscriptionTransportError::Unavailable)
        }
    }

    struct RevokingTransport {
        cutover: CutoverPaths,
    }

    impl SubscriptionTransport for RevokingTransport {
        fn fetch(&self, _url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
            marker(&self.cutover, OwnershipPhase::RollbackPreparing);
            PrivateSubscriptionBody::from_bytes(PRIVATE_BODY.as_bytes().to_vec())
                .map_err(|_| SubscriptionTransportError::Unavailable)
        }
    }

    fn marker(paths: &CutoverPaths, phase: OwnershipPhase) {
        let value = json!({
            "schemaVersion": 1,
            "generation": 7,
            "phase": phase.as_str(),
        });
        fs::write(&paths.ownership_marker, serde_json::to_vec(&value).unwrap()).unwrap();
        fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn fixture() -> (
        PathBuf,
        PathBuf,
        CutoverPaths,
        OfflineNativeCoordinator<FakeHost>,
    ) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-native-dispatch-{}-{nonce}",
            std::process::id()
        ));
        let config = root.join("config");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for path in [&root, &config, &runtime, &state] {
            fs::create_dir_all(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let store_path = config.join("profiles.json");
        let store = json!({
            "version": 3,
            "activeId": "",
            "lastId": "",
            "profiles": [{
                "id": PROFILE,
                "name": "Example",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example",
                "protocol": "vless",
                "favorite": false
            }],
            "subscriptions": [],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true
        });
        fs::write(&store_path, serde_json::to_vec(&store).unwrap()).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let desired_paths = DesiredPaths::below(&state);
        write_desired(
            &desired_paths,
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
        let cutover = CutoverPaths::below(&runtime, &state, uid);
        let owner = OfflineNativeCoordinator::new_ownership_gated(
            FakeHost {
                observation: empty(),
                calls: 0,
            },
            desired_paths,
            &store_path,
            cutover.clone(),
            uid,
            7,
        );
        (root, store_path, cutover, owner)
    }

    fn request(method: &str, params: Value) -> Value {
        make_request("request-1", method, params).unwrap()
    }

    #[test]
    fn every_family_routes_through_one_rust_owned_revision() {
        let (root, store_path, cutover, mut owner) = fixture();
        marker(&cutover, OwnershipPhase::Rust);
        let transport = FakeTransport {
            calls: Cell::new(0),
        };

        let connected = respond_to_native_mutation(
            &mut owner,
            &request(
                "connection.connect",
                json!({"profileId": PROFILE, "mode": "global", "operationId": "connect-1", "expectedRevision": 0}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(connected["ok"], true);
        assert_eq!(connected["revision"], 1);

        let mode = respond_to_native_mutation(
            &mut owner,
            &request(
                "routing.set_mode",
                json!({"mode": "direct", "operationId": "mode-1", "expectedRevision": 1}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(mode["ok"], true);
        assert_eq!(mode["revision"], 2);

        let favorite = respond_to_native_mutation(
            &mut owner,
            &request(
                "profiles.favorite",
                json!({"profileId": PROFILE, "enabled": true, "operationId": "favorite-1", "expectedRevision": 2}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(favorite["ok"], true);
        assert_eq!(favorite["revision"], 3);

        let ids = [SUBSCRIPTION.to_owned(), MANAGED_PROFILE.to_owned()];
        let mut ids = ids.into_iter();
        let added = respond_to_native_mutation(
            &mut owner,
            &request(
                "subscriptions.add",
                json!({"name": "Source", "url": PRIVATE_URL, "operationId": "add-1", "expectedRevision": 3}),
            ),
            &transport,
            || ids.next().unwrap(),
            || 1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(added["ok"], true);
        assert_eq!(added["revision"], 4);
        assert_eq!(transport.calls.get(), 1);
        assert_eq!(added["result"], json!({"accepted": true}));
        let store: Value = serde_json::from_slice(&fs::read(store_path).unwrap()).unwrap();
        assert_eq!(store["profiles"][0]["favorite"], true);
        assert_eq!(store["subscriptions"].as_array().unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn non_rust_or_changed_ownership_blocks_before_host_fetch_or_store_effects() {
        let (root, store_path, cutover, mut owner) = fixture();
        marker(&cutover, OwnershipPhase::Legacy);
        let transport = FakeTransport {
            calls: Cell::new(0),
        };
        let before = fs::read(&store_path).unwrap();
        let rejected = respond_to_native_mutation(
            &mut owner,
            &request(
                "subscriptions.add",
                json!({"name": "Private source", "url": PRIVATE_URL, "operationId": "add-1", "expectedRevision": 0}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(rejected["ok"], false);
        assert_eq!(rejected["error"]["code"], "capability_unavailable");
        assert_eq!(owner.revision(), 0);
        assert_eq!(owner.host().calls, 0);
        assert_eq!(transport.calls.get(), 0);
        assert_eq!(fs::read(&store_path).unwrap(), before);
        let public = rejected.to_string();
        assert!(!public.contains("provider.invalid"));
        assert!(!public.contains("private-token"));

        marker(&cutover, OwnershipPhase::Rust);
        let accepted = respond_to_native_mutation(
            &mut owner,
            &request(
                "profiles.favorite",
                json!({"profileId": PROFILE, "enabled": true, "operationId": "favorite-1", "expectedRevision": 0}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(accepted["ok"], true);
        assert_eq!(accepted["revision"], 1);

        marker(&cutover, OwnershipPhase::RollbackPreparing);
        let calls = owner.host().calls;
        let replay_blocked = respond_to_native_mutation(
            &mut owner,
            &request(
                "profiles.favorite",
                json!({"profileId": PROFILE, "enabled": true, "operationId": "favorite-1", "expectedRevision": 0}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(replay_blocked["error"]["code"], "capability_unavailable");
        let rejected = respond_to_native_mutation(
            &mut owner,
            &request(
                "connection.disconnect",
                json!({"operationId": "disconnect-1", "expectedRevision": 1}),
            ),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(rejected["error"]["code"], "capability_unavailable");
        assert_eq!(owner.revision(), 1);
        assert_eq!(owner.host().calls, calls);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ownership_is_rechecked_after_network_and_before_subscription_commit() {
        let (root, store_path, cutover, mut owner) = fixture();
        marker(&cutover, OwnershipPhase::Rust);
        let transport = RevokingTransport {
            cutover: cutover.clone(),
        };
        let before = fs::read(&store_path).unwrap();
        let ids = [SUBSCRIPTION.to_owned(), MANAGED_PROFILE.to_owned()];
        let mut ids = ids.into_iter();
        let rejected = respond_to_native_mutation(
            &mut owner,
            &request(
                "subscriptions.add",
                json!({"name": "Source", "url": PRIVATE_URL, "operationId": "add-1", "expectedRevision": 0}),
            ),
            &transport,
            || ids.next().unwrap(),
            || 1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(rejected["error"]["code"], "capability_unavailable");
        assert_eq!(owner.revision(), 0);
        assert_eq!(owner.host().calls, 0);
        assert_eq!(fs::read(store_path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unknown_and_invalid_requests_never_enter_the_owner() {
        let (root, _store_path, cutover, mut owner) = fixture();
        marker(&cutover, OwnershipPhase::Rust);
        let transport = FakeTransport {
            calls: Cell::new(0),
        };
        let unknown = respond_to_native_mutation(
            &mut owner,
            &request("status.get", json!({})),
            &transport,
            || unreachable!(),
            || 0,
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], "unknown_method");
        assert_eq!(owner.host().calls, 0);
        assert_eq!(transport.calls.get(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
