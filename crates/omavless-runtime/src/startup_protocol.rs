// SPDX-License-Identifier: MIT
//! Fixed login-policy semantics; no host commands or current connect intent.
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, exact_fields, metadata};
use omavless_control_protocol::validate_request;
use omavless_domain::private_store::StartupPreferences;
use serde_json::Value;

pub struct StartupRequest {
    pub preferences: StartupPreferences,
    pub operation_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub digest: MutationDigest,
}

pub fn parse_startup_request(request: &Value) -> Result<StartupRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "startup.configure" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(
        params,
        &[
            "enabled",
            "target",
            "profileId",
            "mode",
            "operationId",
            "expectedRevision",
        ],
        &["enabled", "target", "profileId", "mode"],
    ) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let preferences = StartupPreferences {
        enabled: params["enabled"]
            .as_bool()
            .ok_or(MutationProtocolError::InvalidArgument)?,
        target: params["target"]
            .as_str()
            .ok_or(MutationProtocolError::InvalidArgument)?
            .to_owned(),
        profile_id: params["profileId"]
            .as_str()
            .ok_or(MutationProtocolError::InvalidArgument)?
            .to_owned(),
        mode: params["mode"]
            .as_str()
            .ok_or(MutationProtocolError::InvalidArgument)?
            .to_owned(),
    };
    preferences
        .validate()
        .map_err(|_| MutationProtocolError::InvalidArgument)?;
    let metadata = metadata(params)?;
    let operation_id = metadata.operation_id.map(str::to_owned);
    let expected_revision = metadata.expected_revision;
    let mut identity = params.clone();
    identity.remove("operationId");
    let mut bytes = b"omavless/startup.configure/v1\0".to_vec();
    bytes
        .extend(serde_json::to_vec(&identity).map_err(|_| MutationProtocolError::InvalidArgument)?);
    Ok(StartupRequest {
        preferences,
        operation_id,
        expected_revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        json!({"api":"omavless.control","version":1,"id":"startup","method":"startup.configure",
            "params":{"enabled":false,"target":"last","profileId":"","mode":"rule"}})
    }

    #[test]
    fn startup_exact_envelope_and_private_errors() {
        assert!(parse_startup_request(&request()).is_ok());
        for (field, value) in [
            ("enabled", json!("true")),
            ("target", json!("arbitrary")),
            ("profileId", json!("password=private-input")),
            ("mode", json!("direct")),
            ("operationId", json!("")),
            ("expectedRevision", json!(-1)),
            ("extra", json!("https://private-input.invalid/token")),
        ] {
            let mut invalid = request();
            invalid["params"][field] = value;
            let error = parse_startup_request(&invalid)
                .err()
                .expect("invalid startup accepted");
            assert!(!format!("{error:?} {error}").contains("private-input"));
        }
        for field in ["enabled", "target", "profileId", "mode"] {
            let mut invalid = request();
            invalid["params"].as_object_mut().unwrap().remove(field);
            assert!(parse_startup_request(&invalid).is_err());
        }
    }

    #[test]
    fn startup_replay_digest_ignores_operation_id_not_policy() {
        let original = request();
        let mut retry = original.clone();
        retry["params"]["operationId"] = json!("startup-1");
        assert!(
            parse_startup_request(&original).unwrap().digest
                == parse_startup_request(&retry).unwrap().digest
        );
        retry["params"]["mode"] = json!("global");
        assert!(
            parse_startup_request(&original).unwrap().digest
                != parse_startup_request(&retry).unwrap().digest
        );
    }
}
