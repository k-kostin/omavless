// SPDX-License-Identifier: MIT

//! Exact v1 connection-mutation parameter validation and semantic digesting.
//!
//! This parser is deliberately not registered with the runtime socket yet.
//! It converts an already bounded control request into the private owner action
//! while rejecting unknown fields and never formatting private values.

use crate::desired::RoutingMode;
use crate::mutation::MutationDigest;
use crate::owner::{OwnerAction, OwnerRequest};
use omavless_control_protocol::{MAX_ID_LENGTH, MAX_REVISION, StableErrorCode, validate_request};
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
            Self::InvalidRequest => "Runtime mutation request is invalid",
            Self::UnknownMethod => "Runtime mutation method is unsupported",
            Self::InvalidArgument => "Runtime mutation argument is invalid",
        })
    }
}

impl std::error::Error for MutationProtocolError {}

pub(crate) fn exact_fields(
    params: &Map<String, Value>,
    allowed: &[&str],
    required: &[&str],
) -> bool {
    params.keys().all(|field| allowed.contains(&field.as_str()))
        && required.iter().all(|field| params.contains_key(*field))
}

pub(crate) struct MutationMetadata<'a> {
    pub operation_id: Option<&'a str>,
    pub expected_revision: Option<u64>,
}

pub(crate) fn metadata(
    params: &Map<String, Value>,
) -> Result<MutationMetadata<'_>, MutationProtocolError> {
    let operation_id = params
        .get("operationId")
        .map(|value| value.as_str().ok_or(MutationProtocolError::InvalidArgument))
        .transpose()?;
    if operation_id.is_some_and(|value| {
        value.is_empty()
            || value.len() > MAX_ID_LENGTH
            || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    }) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let expected_revision = params
        .get("expectedRevision")
        .map(|value| value.as_u64().ok_or(MutationProtocolError::InvalidArgument))
        .transpose()?;
    if expected_revision.is_some_and(|revision| revision > MAX_REVISION) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(MutationMetadata {
        operation_id,
        expected_revision,
    })
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

pub(crate) fn append_field(bytes: &mut Vec<u8>, value: &str) {
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
    let metadata = metadata(params)?;
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
            let digest =
                semantic_digest(method, Some(profile_id), mode, metadata.expected_revision);
            Ok(OwnerRequest::new(
                OwnerAction::Connect {
                    profile_id: profile_id.to_owned(),
                    mode,
                },
                metadata.operation_id,
                metadata.expected_revision,
                digest,
            ))
        }
        "connection.disconnect" => {
            if !exact_fields(params, DISCONNECT_FIELDS, &[]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let digest = semantic_digest(method, None, None, metadata.expected_revision);
            Ok(OwnerRequest::new(
                OwnerAction::Disconnect,
                metadata.operation_id,
                metadata.expected_revision,
                digest,
            ))
        }
        _ => Err(MutationProtocolError::UnknownMethod),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";

    fn request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "request-1",
            "method": method,
            "params": params,
        })
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
    fn malformed_mutation_metadata_is_rejected_instead_of_treated_as_omitted() {
        for params in [
            json!({"profileId": PROFILE_ID, "operationId": 7}),
            json!({"profileId": PROFILE_ID, "operationId": ""}),
            json!({"profileId": PROFILE_ID, "operationId": "has space"}),
            json!({"profileId": PROFILE_ID, "operationId": "x".repeat(65)}),
            json!({"profileId": PROFILE_ID, "expectedRevision": "7"}),
            json!({"profileId": PROFILE_ID, "expectedRevision": -1}),
            json!({"profileId": PROFILE_ID, "expectedRevision": MAX_REVISION + 1}),
        ] {
            assert!(matches!(
                parse_owner_request(&request("connection.connect", params)),
                Err(MutationProtocolError::InvalidRequest)
            ));
        }
        for params in [
            json!({"operationId": false}),
            json!({"expectedRevision": null}),
        ] {
            assert!(matches!(
                parse_owner_request(&request("connection.disconnect", params)),
                Err(MutationProtocolError::InvalidRequest)
            ));
        }
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
