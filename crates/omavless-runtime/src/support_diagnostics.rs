// SPDX-License-Identifier: MIT
//! Bounded shareable report, distinct from live rule/provider diagnostics.
use crate::lifecycle::ActualState;
use crate::mutation_protocol::MutationProtocolError;
use omavless_control_protocol::validate_request;
use serde_json::{Value, json};

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "diagnostics.export" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn report(configuration: Value, actual: ActualState, pending: bool) -> Value {
    let actual = match actual {
        ActualState::Disconnected => "disconnected",
        ActualState::Starting => "starting",
        ActualState::Connected => "connected",
        ActualState::Reconnecting => "reconnecting",
        ActualState::Stopping => "stopping",
        ActualState::Failed => "failed",
        ActualState::ManualRecoveryRequired => "manual_recovery_required",
    };
    json!({
        "schemaVersion": 1,
        "scope": "native_configuration",
        "runtime": {
            "implementation": "rust",
            "version": env!("CARGO_PKG_VERSION"),
            "lastKnownState": actual,
            "routingTransactionPending": pending,
        },
        "configuration": configuration,
        "coverage": {
            "privateStoreValidated": true,
            "liveHostObservation": false,
            "controllerQuery": false,
            "loginActivationVerified": false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_control_protocol::make_request;
    #[test]
    fn support_requests_reject_all_client_data_without_echo() {
        assert!(
            validate(&make_request("support", "diagnostics.export", json!({})).unwrap()).is_ok()
        );
        for params in [
            json!({"path":"private-token"}),
            json!({"raw":true}),
            json!({"operationId":"private-token"}),
            json!({"expectedRevision":0}),
        ] {
            let error = validate(&make_request("support", "diagnostics.export", params).unwrap())
                .unwrap_err();
            assert!(!format!("{error} {error:?}").contains("private-token"));
        }
        assert!(
            validate(&make_request("support", "diagnostics.summary", json!({})).unwrap()).is_err()
        );
    }

    #[test]
    fn support_state_is_explicitly_last_known_and_recovery_is_not_disconnected() {
        let store = omavless_domain::private_store::parse_private_store(
            r#"{"version":3,"profiles":[],"subscriptions":[]}"#,
        )
        .unwrap();
        let value = report(
            store.support_projection(),
            ActualState::ManualRecoveryRequired,
            true,
        );
        assert_eq!(
            value["runtime"]["lastKnownState"],
            "manual_recovery_required"
        );
        assert_eq!(value["runtime"]["routingTransactionPending"], true);
        assert_eq!(value["coverage"]["liveHostObservation"], false);
        assert!(value.to_string().len() < 4096);
    }
}
