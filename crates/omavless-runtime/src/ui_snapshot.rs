// SPDX-License-Identifier: MIT
//! Private same-user UI metadata, NOT a shareable support report or live health.
use crate::desired::DesiredState;
use crate::lifecycle::ActualState;
use crate::mutation_protocol::MutationProtocolError;
use omavless_domain::private_store::PrivateStore;
use serde_json::{Value, json};

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "ui.snapshot" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn project(store: &PrivateStore, desired: &DesiredState, actual: ActualState) -> Value {
    let list = store.list_projection();
    let projection = store.projection();
    let startup = store.startup_preferences();
    let actual = match actual {
        ActualState::Disconnected => "disconnected",
        ActualState::Starting => "starting",
        ActualState::Connected => "connected",
        ActualState::Reconnecting => "reconnecting",
        ActualState::Stopping => "stopping",
        ActualState::Failed => "failed",
        ActualState::ManualRecoveryRequired => "manualRecoveryRequired",
    };
    json!({"schemaVersion":1,"scope":"private_ui_metadata",
        "desired":{"connected":desired.connected,"profileId":desired.profile_id,"mode":desired.mode.as_str(),"generation":desired.generation},
        "lastKnownActual":actual,"healthFresh":false,"liveHealth":"unavailable",
        "profiles":crate::profile_list_json(&list)["profiles"],
        "lastProfileId":list.last_profile_id(),
        "subscriptions":crate::subscription_list_json(&list)["subscriptions"],
        "startup":{"configured":projection.startup_configured,"enabled":startup.enabled,"target":startup.target,"profileId":startup.profile_id,"mode":startup.mode},
        "onboardingComplete":projection.onboarding_complete,
        "routing":{"storedPreset":projection.routing_preset,"customRuleCount":projection.custom_rule_count}
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_does_not_claim_health_or_hide_manual_recovery() {
        let store = omavless_domain::private_store::parse_private_store(
            r#"{"version":3,"profiles":[],"subscriptions":[]}"#,
        )
        .unwrap();
        for (actual, expected) in [
            (ActualState::Failed, "failed"),
            (
                ActualState::ManualRecoveryRequired,
                "manualRecoveryRequired",
            ),
            (ActualState::Connected, "connected"),
        ] {
            let report = project(&store, &DesiredState::default(), actual);
            assert_eq!(report["healthFresh"], false);
            assert_eq!(report["liveHealth"], "unavailable");
            assert_eq!(report["desired"]["connected"], false);
            assert_eq!(report["lastKnownActual"], expected);
        }
    }
    #[test]
    fn request_rejects_all_extra_client_data() {
        for params in [
            json!({"path":"private-token"}),
            json!({"expectedRevision":0}),
            json!([]),
        ] {
            let request = json!({"api":"omavless.control","version":1,"id":"test","method":"ui.snapshot","params":params});
            let error = validate(&request).unwrap_err();
            assert!(!error.to_string().contains("private-token"));
        }
    }
}
