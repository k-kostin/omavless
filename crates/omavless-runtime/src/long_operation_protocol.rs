// SPDX-License-Identifier: MIT

//! Exact, inactive v1 protocol boundary for bounded long operations.
//!
//! These parsers and projections do not schedule work or register socket
//! methods. They establish the start/poll/cancel contract required before a
//! multi-provider refresh can safely outlive one five-second unary exchange.

use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use omavless_control_protocol::{MAX_ID_LENGTH, MAX_REVISION, StableErrorCode, validate_request};
use serde_json::{Value, json};

const START_FIELDS: &[&str] = &["instanceId", "operationId", "expectedRevision"];
const LOOKUP_FIELDS: &[&str] = &["instanceId", "operationId"];
pub const MAX_REFRESH_ALL_SUBSCRIPTIONS: usize = 64;
pub const MAX_INSTANCE_ID_BYTES: usize = 128;

fn instance_id(value: Option<&Value>) -> Result<&str, MutationProtocolError> {
    value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_INSTANCE_ID_BYTES
                && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
        .ok_or(MutationProtocolError::InvalidArgument)
}

fn operation_id(value: Option<&Value>) -> Result<&str, MutationProtocolError> {
    value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_ID_LENGTH
                && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
        .ok_or(MutationProtocolError::InvalidArgument)
}

fn refresh_all_digest(instance_id: &str, expected_revision: Option<u64>) -> MutationDigest {
    let mut bytes = Vec::with_capacity(96);
    bytes.extend_from_slice(b"omavless.control/long-operation/v1\0");
    append_field(&mut bytes, "subscriptions.refresh_all");
    append_field(&mut bytes, instance_id);
    match expected_revision {
        Some(revision) => {
            bytes.push(1);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        None => bytes.push(0),
    }
    MutationDigest::from_semantic_bytes(&bytes)
}

/// Private start intent. The correlation ID is deliberately not formattable
/// or serializable and must not be copied to journals.
pub struct RefreshAllStartRequest {
    instance_id: String,
    operation_id: String,
    expected_revision: Option<u64>,
    digest: MutationDigest,
}

impl RefreshAllStartRequest {
    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    #[must_use]
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    #[must_use]
    pub const fn expected_revision(&self) -> Option<u64> {
        self.expected_revision
    }

    #[must_use]
    pub const fn digest(&self) -> MutationDigest {
        self.digest
    }
}

/// Private poll/cancel lookup. It deliberately has no formatting or generic
/// serialization implementation.
pub struct OperationLookupRequest {
    instance_id: String,
    operation_id: String,
}

impl OperationLookupRequest {
    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    #[must_use]
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
}

pub fn parse_refresh_all_start(
    request: &Value,
) -> Result<RefreshAllStartRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "subscriptions.refresh_all" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, START_FIELDS, &["instanceId", "operationId"]) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let metadata = metadata(params)?;
    let instance_id = instance_id(params.get("instanceId"))?;
    let operation_id = operation_id(params.get("operationId"))?;
    Ok(RefreshAllStartRequest {
        instance_id: instance_id.to_owned(),
        operation_id: operation_id.to_owned(),
        expected_revision: metadata.expected_revision,
        digest: refresh_all_digest(instance_id, metadata.expected_revision),
    })
}

fn parse_lookup(
    request: &Value,
    method: &str,
) -> Result<OperationLookupRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != method {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, LOOKUP_FIELDS, &["instanceId", "operationId"]) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(OperationLookupRequest {
        instance_id: instance_id(params.get("instanceId"))?.to_owned(),
        operation_id: operation_id(params.get("operationId"))?.to_owned(),
    })
}

pub fn parse_operation_get(
    request: &Value,
) -> Result<OperationLookupRequest, MutationProtocolError> {
    parse_lookup(request, "operations.get")
}

pub fn parse_operation_cancel(
    request: &Value,
) -> Result<OperationLookupRequest, MutationProtocolError> {
    parse_lookup(request, "operations.cancel")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongOperationState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl LongOperationState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongOperationProgress {
    pub completed: usize,
    pub total: usize,
}

/// Credential-free operation projection for an intentional same-user poll.
/// It deliberately has no `Debug` implementation because operation IDs must
/// not drift into ordinary logs.
pub struct LongOperationProjection<'a> {
    pub instance_id: &'a str,
    pub operation_id: &'a str,
    pub state: LongOperationState,
    pub base_revision: u64,
    pub outcome_revision: Option<u64>,
    pub progress: LongOperationProgress,
    pub cancel_requested: bool,
    pub cancellable: bool,
    pub error: Option<StableErrorCode>,
}

impl LongOperationProjection<'_> {
    pub fn validate(&self) -> Result<(), MutationProtocolError> {
        instance_id(Some(&Value::String(self.instance_id.to_owned())))?;
        operation_id(Some(&Value::String(self.operation_id.to_owned())))?;
        if self.base_revision > MAX_REVISION
            || self
                .outcome_revision
                .is_some_and(|revision| revision > MAX_REVISION)
            || self.progress.total > MAX_REFRESH_ALL_SUBSCRIPTIONS
            || self.progress.completed > self.progress.total
            || self.state.terminal() != self.outcome_revision.is_some()
            || (self.state.terminal() && self.cancellable)
            || (self.state == LongOperationState::Failed) != self.error.is_some()
            || (self.state == LongOperationState::Cancelled && !self.cancel_requested)
            || (self.state == LongOperationState::Queued && self.progress.completed != 0)
            || (self.state == LongOperationState::Succeeded
                && self.progress.completed != self.progress.total)
            || self
                .outcome_revision
                .is_some_and(|revision| revision < self.base_revision)
            || (self.state == LongOperationState::Succeeded
                && self.outcome_revision
                    != Some(if self.progress.total == 0 {
                        self.base_revision
                    } else {
                        self.base_revision
                            .checked_add(1)
                            .ok_or(MutationProtocolError::InvalidArgument)?
                    }))
        {
            return Err(MutationProtocolError::InvalidArgument);
        }
        Ok(())
    }

    pub fn result_value(&self) -> Result<Value, MutationProtocolError> {
        self.validate()?;
        let error = self.error.map(|code| {
            json!({
                "code": code.as_str(),
                "message": code.message(),
                "retryable": matches!(
                    code,
                    StableErrorCode::Conflict
                        | StableErrorCode::Busy
                        | StableErrorCode::DaemonRestarting
                ),
            })
        });
        Ok(json!({
            "operation": {
                "instanceId": self.instance_id,
                "operationId": self.operation_id,
                "method": "subscriptions.refresh_all",
                "state": self.state.as_str(),
                "baseRevision": self.base_revision,
                "outcomeRevision": self.outcome_revision,
                "progress": {
                    "completed": self.progress.completed,
                    "total": self.progress.total,
                },
                "cancelRequested": self.cancel_requested,
                "cancellable": self.cancellable,
                "error": error,
            }
        }))
    }

    pub fn start_result_value(&self) -> Result<Value, MutationProtocolError> {
        self.result_value()
    }

    pub fn get_result_value(&self) -> Result<Value, MutationProtocolError> {
        self.result_value()
    }

    pub fn cancel_result_value(&self, accepted: bool) -> Result<Value, MutationProtocolError> {
        let operation = self.result_value()?["operation"].clone();
        Ok(json!({"accepted": accepted, "operation": operation}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const INSTANCE: &str = "instance-1";

    fn request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control", "version": 1, "id": "request-1",
            "method": method, "params": params,
        })
    }

    #[test]
    fn exact_start_poll_and_cancel_shapes_are_accepted() {
        let start = parse_refresh_all_start(&request(
            "subscriptions.refresh_all",
            json!({"instanceId": INSTANCE, "operationId": "refresh-all-1", "expectedRevision": 7}),
        ))
        .unwrap();
        assert_eq!(start.operation_id(), "refresh-all-1");
        assert_eq!(start.instance_id(), INSTANCE);
        assert_eq!(start.expected_revision(), Some(7));
        let get = parse_operation_get(&request(
            "operations.get",
            json!({"instanceId": INSTANCE, "operationId": "refresh-all-1"}),
        ))
        .unwrap();
        assert_eq!(get.operation_id(), "refresh-all-1");
        assert_eq!(get.instance_id(), INSTANCE);
        assert!(
            parse_operation_cancel(&request(
                "operations.cancel",
                json!({"instanceId": INSTANCE, "operationId": "refresh-all-1"})
            ))
            .is_ok()
        );
    }

    #[test]
    fn ids_revision_types_and_exact_fields_fail_closed() {
        for params in [
            json!({}),
            json!({"instanceId": INSTANCE, "operationId": ""}),
            json!({"instanceId": INSTANCE, "operationId": "has space"}),
            json!({"instanceId": INSTANCE, "operationId": "x".repeat(65)}),
            json!({"instanceId": INSTANCE, "operationId": 7}),
            json!({"instanceId": "", "operationId": "ok"}),
            json!({"instanceId": "x".repeat(MAX_INSTANCE_ID_BYTES + 1), "operationId": "ok"}),
            json!({"instanceId": INSTANCE, "operationId": "ok", "expectedRevision": -1}),
            json!({"instanceId": INSTANCE, "operationId": "ok", "expectedRevision": MAX_REVISION + 1}),
            json!({"instanceId": INSTANCE, "operationId": "ok", "url": "https://private.invalid/token"}),
        ] {
            assert!(
                parse_refresh_all_start(&request("subscriptions.refresh_all", params)).is_err()
            );
        }
        for method in ["operations.get", "operations.cancel"] {
            assert!(parse_lookup(&request(method, json!({})), method).is_err());
            assert!(
                parse_lookup(
                    &request(
                        method,
                        json!({"instanceId": INSTANCE, "operationId": "ok", "expectedRevision": 1})
                    ),
                    method
                )
                .is_err()
            );
        }
    }

    #[test]
    fn digest_excludes_id_and_covers_revision_presence_and_value() {
        let parsed = |operation_id: &str, revision: Option<u64>| {
            let mut params = json!({"instanceId": INSTANCE, "operationId": operation_id});
            if let Some(revision) = revision {
                params["expectedRevision"] = Value::from(revision);
            }
            parse_refresh_all_start(&request("subscriptions.refresh_all", params))
                .unwrap()
                .digest()
        };
        assert!(parsed("one", Some(7)) == parsed("two", Some(7)));
        assert!(parsed("one", Some(7)) != parsed("one", Some(8)));
        assert!(parsed("one", Some(0)) != parsed("one", None));
        let next_instance = parse_refresh_all_start(&request(
            "subscriptions.refresh_all",
            json!({
                "instanceId": "instance-2",
                "operationId": "one",
                "expectedRevision": 7,
            }),
        ))
        .unwrap()
        .digest();
        assert!(parsed("one", Some(7)) != next_instance);
    }

    #[test]
    fn projections_are_exact_bounded_and_credential_safe() {
        let projection = LongOperationProjection {
            instance_id: INSTANCE,
            operation_id: "refresh-all-1",
            state: LongOperationState::Running,
            base_revision: 7,
            outcome_revision: None,
            progress: LongOperationProgress {
                completed: 2,
                total: 3,
            },
            cancel_requested: false,
            cancellable: true,
            error: None,
        };
        assert_eq!(
            projection.result_value().unwrap(),
            json!({"operation": {
                "instanceId": INSTANCE, "operationId": "refresh-all-1", "method": "subscriptions.refresh_all",
                "state": "running", "baseRevision": 7, "outcomeRevision": null,
                "progress": {"completed": 2, "total": 3},
                "cancelRequested": false, "cancellable": true, "error": null,
            }})
        );
        assert_eq!(
            projection.start_result_value().unwrap(),
            projection.get_result_value().unwrap()
        );
        for invalid in [
            LongOperationProjection {
                instance_id: "bad instance",
                ..projection
            },
            LongOperationProjection {
                operation_id: "bad id",
                ..projection
            },
            LongOperationProjection {
                progress: LongOperationProgress {
                    completed: 4,
                    total: 3,
                },
                ..projection
            },
            LongOperationProjection {
                state: LongOperationState::Succeeded,
                ..projection
            },
            LongOperationProjection {
                error: Some(StableErrorCode::InternalError),
                ..projection
            },
            LongOperationProjection {
                state: LongOperationState::Failed,
                outcome_revision: Some(6),
                cancellable: false,
                error: Some(StableErrorCode::CoreRejected),
                ..projection
            },
            LongOperationProjection {
                state: LongOperationState::Succeeded,
                outcome_revision: Some(7),
                progress: LongOperationProgress {
                    completed: 3,
                    total: 3,
                },
                cancellable: false,
                ..projection
            },
        ] {
            assert!(invalid.result_value().is_err());
        }
    }

    #[test]
    fn parser_and_projection_errors_never_echo_private_input() {
        let private = "private.example/password";
        let error = parse_refresh_all_start(&request(
            "subscriptions.refresh_all",
            json!({"instanceId": INSTANCE, "operationId": private, "url": private}),
        ))
        .err()
        .unwrap();
        let public = format!("{error:?} {error}");
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
    }

    #[test]
    fn terminal_failure_and_cancel_responses_are_fixed_and_bounded() {
        let failed = LongOperationProjection {
            instance_id: INSTANCE,
            operation_id: "failed-1",
            state: LongOperationState::Failed,
            base_revision: 4,
            outcome_revision: Some(4),
            progress: LongOperationProgress {
                completed: 1,
                total: 3,
            },
            cancel_requested: false,
            cancellable: false,
            error: Some(StableErrorCode::CoreRejected),
        };
        assert_eq!(
            failed.result_value().unwrap()["operation"]["error"],
            json!({
                "code": "core_rejected",
                "message": "The proxy core rejected the operation",
                "retryable": false,
            })
        );
        let cancelled = LongOperationProjection {
            instance_id: INSTANCE,
            operation_id: "cancelled-1",
            state: LongOperationState::Cancelled,
            base_revision: 4,
            outcome_revision: Some(5),
            progress: LongOperationProgress {
                completed: 2,
                total: 3,
            },
            cancel_requested: true,
            cancellable: false,
            error: None,
        };
        let result = cancelled.cancel_result_value(false).unwrap();
        assert_eq!(result["accepted"], false);
        assert_eq!(result["operation"]["state"], "cancelled");
        assert_eq!(result["operation"]["error"], Value::Null);
    }
}
