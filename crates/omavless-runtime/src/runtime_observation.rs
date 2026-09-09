// SPDX-License-Identifier: MIT
//! Credential-free point-in-time local facts, never a VPN connectivity verdict.
use crate::desired::DesiredState;
use crate::lifecycle::{ActualState, NativeLocalObservation};
use crate::mutation_protocol::MutationProtocolError;
use serde_json::{Value, json};

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "runtime.observation" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn project(
    desired: &DesiredState,
    actual: ActualState,
    observation: Option<NativeLocalObservation>,
) -> Value {
    let actual = match actual {
        ActualState::Disconnected => "disconnected",
        ActualState::Starting => "starting",
        ActualState::Connected => "connected",
        ActualState::Reconnecting => "reconnecting",
        ActualState::Stopping => "stopping",
        ActualState::Failed => "failed",
        ActualState::ManualRecoveryRequired => "manualRecoveryRequired",
    };
    let facts = observation.map(|o| {
        json!({
            "ownedCoreRunning":o.owned_core_running,
            "visibleMihomoCount":o.visible_mihomo_count,
            "visibleTunCount":o.visible_tun_count,
            "ownedControllerConfigVerified":o.owned_controller_config_verified,
            "desiredProfileMatchesOwned":o.desired_profile_matches_owned
        })
    });
    json!({
        "schemaVersion":1,"scope":"local_runtime_observation",
        "availability":if facts.is_some() {"observed"} else {"unavailable"},
        "desired":{"connected":desired.connected,"mode":desired.mode.as_str(),"generation":desired.generation},
        "lastKnownActual":actual,
        "manualRecoveryRequired":actual == "manualRecoveryRequired",
        "facts":facts,
        "verification":{"serviceOwnership":false,"tunOwnership":false,"routes":false,"dns":false,"internet":false}
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_is_not_zero_or_disconnected_and_recovery_remains_visible() {
        let value = project(
            &DesiredState::default(),
            ActualState::ManualRecoveryRequired,
            None,
        );
        assert_eq!(value["availability"], "unavailable");
        assert!(value["facts"].is_null());
        assert_eq!(value["lastKnownActual"], "manualRecoveryRequired");
        assert_eq!(value["manualRecoveryRequired"], true);
        assert!(
            value["verification"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == false)
        );
    }
    #[test]
    fn request_is_fixed_and_errors_do_not_echo_private_input() {
        for params in [
            json!({}),
            json!({"path":"https://private.invalid/password"}),
            json!([]),
        ] {
            let request = json!({"api":"omavless.control","version":1,"id":"test","method":"runtime.observation","params":params});
            let result = validate(&request);
            assert_eq!(result.is_ok(), params == json!({}));
            if let Err(error) = result {
                assert!(!error.to_string().contains("private.invalid"));
            }
        }
    }

    #[test]
    fn successful_local_facts_do_not_promote_cached_state_or_expose_identity() {
        let desired = DesiredState {
            connected: true,
            profile_id: "private-record-id".into(),
            ..DesiredState::default()
        };
        let value = project(
            &desired,
            ActualState::Failed,
            Some(NativeLocalObservation {
                owned_core_running: true,
                visible_mihomo_count: 1,
                visible_tun_count: 1,
                owned_controller_config_verified: true,
                desired_profile_matches_owned: true,
            }),
        );
        assert_eq!(value["availability"], "observed");
        assert_eq!(value["lastKnownActual"], "failed");
        assert_eq!(value["facts"]["ownedControllerConfigVerified"], true);
        assert!(
            value["verification"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v == false)
        );
        assert!(!value.to_string().contains("private-record-id"));
        assert!(serde_json::to_vec(&value).unwrap().len() < 1024);
    }
}
