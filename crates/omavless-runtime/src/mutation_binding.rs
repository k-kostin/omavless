// SPDX-License-Identifier: MIT

//! Offline binding from exact v1 connection requests to the internal owner.
//!
//! This module is deliberately not registered with `RuntimeServer`. It proves
//! the parser/coordinator/lifecycle/response seam behind an explicit Rust
//! ownership marker while the installed daemon remains read-only and reports
//! `runtimeOwnership: false`.

use crate::cutover::{OwnershipMarker, OwnershipPhase};
use crate::lifecycle::{LifecycleHost, LifecycleOutcome};
use crate::mutation::CachedOutcome;
use crate::mutation_protocol::parse_owner_request;
use crate::owner::{OwnerEngine, OwnerExecution};
use omavless_control_protocol::{
    ProtocolError, StableErrorCode, error_response, success_response, validate_request,
};
use serde_json::{Value, json};

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

fn applied_response(
    request_id: &str,
    cached: CachedOutcome,
    lifecycle: Result<LifecycleOutcome, crate::lifecycle::LifecycleError>,
) -> Result<Value, ProtocolError> {
    match (cached.error, lifecycle) {
        (None, Ok(_)) => cached_response(request_id, cached),
        (Some(code), Err(error)) if code == error.stable_code() => {
            cached_response(request_id, cached)
        }
        _ => error_response(
            request_id,
            cached.revision,
            StableErrorCode::InternalError,
            false,
            None,
        ),
    }
}

/// Execute one already-decoded request through the future Rust owner seam.
///
/// The caller must still provide the durable ownership marker. Legacy or
/// preparing ownership always returns `capability_unavailable` before any host
/// side effect. Runtime socket dispatch intentionally does not call this yet.
pub fn respond_to_owner_mutation<H: LifecycleHost>(
    marker: &OwnershipMarker,
    owner: &mut OwnerEngine<H>,
    request: &Value,
) -> Result<Value, ProtocolError> {
    if let Err(error) = validate_request(request) {
        return error_response("invalid", owner.revision(), error.code(), false, None);
    }
    let request_id = request["id"].as_str().unwrap_or("invalid");
    let owner_request = match parse_owner_request(request) {
        Ok(request) => request,
        Err(error) => {
            let code = error.stable_code();
            return error_response(request_id, owner.revision(), code, retryable(code), None);
        }
    };
    if marker.phase() != OwnershipPhase::Rust {
        return error_response(
            request_id,
            owner.revision(),
            StableErrorCode::CapabilityUnavailable,
            false,
            None,
        );
    }
    match owner.execute(owner_request) {
        Ok(OwnerExecution::Applied { cached, lifecycle }) => {
            applied_response(request_id, cached, lifecycle)
        }
        Ok(OwnerExecution::Replay(cached) | OwnerExecution::Rejected(cached)) => {
            cached_response(request_id, cached)
        }
        Err(error) => {
            let code = error.stable_code();
            error_response(request_id, owner.revision(), code, retryable(code), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::{
        CutoverReadiness, OwnershipObservation, RustCommitEvidence, begin_cutover, commit_cutover,
    };
    use crate::desired::{DesiredPaths, DesiredState, OwnedObservation};
    use crate::lifecycle::HostStepError;
    use omavless_control_protocol::make_request;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";
    const OTHER_PROFILE_ID: &str = "00000000-0000-4000-8000-000000000002";

    #[derive(Default)]
    struct FakeHost {
        observation: Option<OwnedObservation>,
        calls: usize,
        fail_start: bool,
    }

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

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.calls += 1;
            Ok(self.observation.unwrap_or_else(empty))
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            self.observation = Some(healthy());
            if self.fail_start {
                Err(HostStepError::Start)
            } else {
                Ok(())
            }
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            self.observation = Some(empty());
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }
    }

    fn engine(label: &str, host: FakeHost) -> (PathBuf, OwnerEngine<FakeHost>) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-binding-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        let paths = DesiredPaths::below(&root);
        (root, OwnerEngine::new(host, paths, uid))
    }

    fn rust_marker() -> OwnershipMarker {
        let legacy = OwnershipMarker::default();
        let preparing = begin_cutover(&legacy, CutoverReadiness::ReadyDisconnected).unwrap();
        commit_cutover(
            &preparing,
            RustCommitEvidence {
                hello_verified: true,
                status_verified: true,
                plugin_bridge_switched: true,
                observation: OwnershipObservation {
                    legacy_owner_active: false,
                    rust_owner_active: true,
                    legacy_controller_ready: false,
                    rust_controller_ready: false,
                    core_count: 0,
                    tun_count: 0,
                    active_profile_matches: false,
                },
            },
        )
        .unwrap()
    }

    fn request(profile_id: &str, operation_id: &str, expected_revision: u64) -> Value {
        make_request(
            "request-1",
            "connection.connect",
            json!({
                "profileId": profile_id,
                "mode": "global",
                "operationId": operation_id,
                "expectedRevision": expected_revision
            }),
        )
        .unwrap()
    }

    #[test]
    fn legacy_and_preparing_markers_block_before_host_side_effects() {
        let (root, mut owner) = engine("gate", FakeHost::default());
        let legacy = OwnershipMarker::default();
        let response =
            respond_to_owner_mutation(&legacy, &mut owner, &request(PROFILE_ID, "connect-1", 0))
                .unwrap();
        assert_eq!(response["error"]["code"], "capability_unavailable");
        assert_eq!(owner.host().calls, 0);

        let preparing = begin_cutover(&legacy, CutoverReadiness::ReadyDisconnected).unwrap();
        let response =
            respond_to_owner_mutation(&preparing, &mut owner, &request(PROFILE_ID, "connect-2", 0))
                .unwrap();
        assert_eq!(response["error"]["code"], "capability_unavailable");
        assert_eq!(owner.host().calls, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exact_retry_replays_original_revision_before_stale_revision_check() {
        let (root, mut owner) = engine("replay", FakeHost::default());
        let marker = rust_marker();
        let original = request(PROFILE_ID, "connect-1", 0);
        let first = respond_to_owner_mutation(&marker, &mut owner, &original).unwrap();
        assert_eq!(first["ok"], true);
        assert_eq!(first["revision"], 1);
        assert_eq!(first["result"], json!({"accepted": true}));
        let calls = owner.host().calls;

        let replay = respond_to_owner_mutation(&marker, &mut owner, &original).unwrap();
        assert_eq!(replay, first);
        assert_eq!(owner.host().calls, calls);

        let conflict = respond_to_owner_mutation(
            &marker,
            &mut owner,
            &request(OTHER_PROFILE_ID, "connect-1", 0),
        )
        .unwrap();
        assert_eq!(conflict["error"]["code"], "conflict");
        assert_eq!(conflict["revision"], 1);
        assert_eq!(owner.host().calls, calls);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lifecycle_failure_and_private_input_return_only_stable_errors() {
        let (root, mut owner) = engine(
            "private",
            FakeHost {
                fail_start: true,
                ..FakeHost::default()
            },
        );
        let marker = rust_marker();
        let private = "vless://private.example/password";
        let invalid = make_request(
            "private-request",
            "connection.connect",
            json!({"profileId": private}),
        )
        .unwrap();
        let response = respond_to_owner_mutation(&marker, &mut owner, &invalid).unwrap();
        assert_eq!(response["error"]["code"], "invalid_argument");
        assert_eq!(owner.host().calls, 0);
        assert!(!response.to_string().contains(private));
        assert!(!response.to_string().contains("password"));

        let response =
            respond_to_owner_mutation(&marker, &mut owner, &request(PROFILE_ID, "failed", 0))
                .unwrap();
        assert_eq!(response["error"]["code"], "transition_failed_restored");
        assert_eq!(response["revision"], 0);
        assert!(!response.to_string().contains(PROFILE_ID));
        fs::remove_dir_all(root).unwrap();
    }
}
