// SPDX-License-Identifier: MIT

//! Exact v1 connection-mutation parameter validation and semantic digesting.
//!
//! This parser is deliberately not registered with the runtime socket yet.
//! It converts an already bounded control request into the private owner action
//! while rejecting unknown fields and never formatting private values.

use crate::desired::RoutingMode;
use crate::mutation::MutationDigest;
use crate::owner::{OwnerAction, OwnerRequest};
use omavless_control_protocol::{MAX_REVISION, StableErrorCode, validate_request};
use omavless_domain::store::valid_record_id;
use serde_json::{Map, Value};
use std::fmt;

const CONNECT_FIELDS: &[&str] = &["profileId", "mode", "operationId", "expectedRevision"];
const DISCONNECT_FIELDS: &[&str] = &["operationId", "expectedRevision"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationProtocolError {
    InvalidRequest,
    UnknownMethod,
    InvalidArgument,
}

impl MutationProtocolError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::InvalidRequest => StableErrorCode::InvalidRequest,
            Self::UnknownMethod => StableErrorCode::UnknownMethod,
            Self::InvalidArgument => StableErrorCode::InvalidArgument,
        }
    }
}

impl fmt::Display for MutationProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "Connection mutation request is invalid",
            Self::UnknownMethod => "Connection mutation method is unsupported",
            Self::InvalidArgument => "Connection mutation argument is invalid",
        })
    }
}

impl std::error::Error for MutationProtocolError {}

fn exact_fields(params: &Map<String, Value>, allowed: &[&str], required: &[&str]) -> bool {
    params.keys().all(|field| allowed.contains(&field.as_str()))
        && required.iter().all(|field| params.contains_key(*field))
}

fn metadata(params: &Map<String, Value>) -> (Option<&str>, Option<u64>) {
    (
        params.get("operationId").and_then(Value::as_str),
        params.get("expectedRevision").and_then(Value::as_u64),
    )
}

fn mode(value: Option<&Value>) -> Result<Option<RoutingMode>, MutationProtocolError> {
    match value {
        None => Ok(None),
        Some(value) => match value.as_str() {
            Some("rule") => Ok(Some(RoutingMode::Rule)),
            Some("global") => Ok(Some(RoutingMode::Global)),
            Some("direct") => Ok(Some(RoutingMode::Direct)),
            _ => Err(MutationProtocolError::InvalidArgument),
        },
    }
}

fn append_field(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn semantic_digest(
    method: &str,
    profile_id: Option<&str>,
    mode: Option<RoutingMode>,
    expected_revision: Option<u64>,
) -> MutationDigest {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(b"omavless.control/mutation/v1\0");
    append_field(&mut bytes, method);
    match profile_id {
        Some(value) => {
            bytes.push(1);
            append_field(&mut bytes, value);
        }
        None => bytes.push(0),
    }
    bytes.push(match mode {
        None => 0,
        Some(RoutingMode::Rule) => 1,
        Some(RoutingMode::Global) => 2,
        Some(RoutingMode::Direct) => 3,
    });
    match expected_revision {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    MutationDigest::from_semantic_bytes(&bytes)
}

pub fn parse_owner_request(request: &Value) -> Result<OwnerRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    let method = request["method"]
        .as_str()
        .ok_or(MutationProtocolError::InvalidRequest)?;
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let (operation_id, expected_revision) = metadata(params);
    if expected_revision.is_some_and(|revision| revision > MAX_REVISION) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    match method {
        "connection.connect" => {
            if !exact_fields(params, CONNECT_FIELDS, &["profileId"])
                || params
                    .get("profileId")
                    .and_then(Value::as_str)
                    .is_none_or(|profile_id| !valid_record_id(profile_id))
            {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let profile_id = params["profileId"]
                .as_str()
                .ok_or(MutationProtocolError::InvalidArgument)?;
            let mode = mode(params.get("mode"))?;
            let digest = semantic_digest(method, Some(profile_id), mode, expected_revision);
            Ok(OwnerRequest::new(
                OwnerAction::Connect {
                    profile_id: profile_id.to_owned(),
                    mode,
                },
                operation_id,
                expected_revision,
                digest,
            ))
        }
        "connection.disconnect" => {
            if !exact_fields(params, DISCONNECT_FIELDS, &[]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let digest = semantic_digest(method, None, None, expected_revision);
            Ok(OwnerRequest::new(
                OwnerAction::Disconnect,
                operation_id,
                expected_revision,
                digest,
            ))
        }
        _ => Err(MutationProtocolError::UnknownMethod),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_control_protocol::make_request;
    use serde_json::json;

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";

    fn request(method: &str, params: Value) -> Value {
        make_request("request-1", method, params).unwrap()
    }

    #[test]
    fn exact_connect_and_disconnect_shapes_are_accepted() {
        for params in [
            json!({"profileId": PROFILE_ID}),
            json!({
                "profileId": PROFILE_ID,
                "mode": "global",
                "operationId": "operation-1",
                "expectedRevision": 7
            }),
        ] {
            assert!(parse_owner_request(&request("connection.connect", params)).is_ok());
        }
        assert!(
            parse_owner_request(&request(
                "connection.disconnect",
                json!({"operationId": "operation-2", "expectedRevision": 8})
            ))
            .is_ok()
        );
    }

    #[test]
    fn unknown_fields_bad_ids_modes_and_shapes_fail_closed() {
        for params in [
            json!({}),
            json!({"profileId": "not-a-record-id"}),
            json!({"profileId": PROFILE_ID, "mode": "full"}),
            json!({"profileId": PROFILE_ID, "mode": 1}),
            json!({"profileId": PROFILE_ID, "uri": "private.example/password"}),
            json!({"profileId": PROFILE_ID, "extra": true}),
        ] {
            assert!(matches!(
                parse_owner_request(&request("connection.connect", params)),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
        assert!(matches!(
            parse_owner_request(&request(
                "connection.disconnect",
                json!({"profileId": PROFILE_ID})
            )),
            Err(MutationProtocolError::InvalidArgument)
        ));
        assert!(matches!(
            parse_owner_request(&request("connection.restart", json!({}))),
            Err(MutationProtocolError::UnknownMethod)
        ));
    }

    #[test]
    fn semantic_digest_ignores_operation_id_but_covers_all_action_inputs() {
        let baseline = semantic_digest(
            "connection.connect",
            Some(PROFILE_ID),
            Some(RoutingMode::Rule),
            Some(4),
        );
        // operationId is deliberately absent from semantic_digest arguments.
        assert!(
            baseline
                == semantic_digest(
                    "connection.connect",
                    Some(PROFILE_ID),
                    Some(RoutingMode::Rule),
                    Some(4)
                )
        );
        for changed in [
            semantic_digest(
                "connection.disconnect",
                Some(PROFILE_ID),
                Some(RoutingMode::Rule),
                Some(4),
            ),
            semantic_digest(
                "connection.connect",
                Some("00000000-0000-4000-8000-000000000002"),
                Some(RoutingMode::Rule),
                Some(4),
            ),
            semantic_digest(
                "connection.connect",
                Some(PROFILE_ID),
                Some(RoutingMode::Direct),
                Some(4),
            ),
            semantic_digest(
                "connection.connect",
                Some(PROFILE_ID),
                Some(RoutingMode::Rule),
                Some(5),
            ),
            semantic_digest("connection.connect", Some(PROFILE_ID), None, Some(4)),
        ] {
            assert!(baseline != changed);
        }
    }

    #[test]
    fn public_errors_never_echo_private_request_values() {
        let private = "private.example/password";
        let error = match parse_owner_request(&request(
            "connection.connect",
            json!({"profileId": PROFILE_ID, "uri": private}),
        )) {
            Ok(_) => panic!("private extra field accepted"),
            Err(error) => error,
        };
        let public = format!("{error:?} {error}");
        assert!(!public.contains(private));
        assert!(!public.contains("password"));
        assert!(!public.contains("private.example"));
    }
}
