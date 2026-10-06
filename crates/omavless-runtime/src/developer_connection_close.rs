// SPDX-License-Identifier: MIT
//! Opt-in development semantic close API on the existing same-UID socket.
//!
//! No default-build methods, raw controller IDs, caller paths, adopted passive
//! receipts or privileged transports. Authority remains in the original owner,
//! its retained discovery and single-use confirmation. Private display rows are
//! returned only by this explicit connections workspace API, never status.

use crate::lifecycle::LifecycleHost;
use crate::mutation::{ExternalCloseOutcome, ExternalCloseReceipt};
use crate::native_coordinator::NativeOwnerError;
use crate::native_coordinator::connection_close::{CloseDiscovered, CloseDiscovery, OpaqueToken};
use crate::{RegisteredNativeOwner, RuntimeDispatcher, RuntimeServer};
use omavless_control_protocol::{StableErrorCode, error_response, success_response};
use serde_json::{Value, json};

pub(super) const METHODS: &[&str] = &[
    "development.connections.snapshot",
    "development.connections.prepare",
    "development.connections.confirm",
    "development.connections.receipt",
];

// No Debug: tokens and private display data must not enter a diagnostic log.
pub(super) enum Action {
    Snapshot,
    Prepare {
        handle: OpaqueToken,
    },
    Confirm {
        operation: String,
        expected_revision: u64,
        handle: OpaqueToken,
        ticket: OpaqueToken,
    },
    Receipt {
        operation: String,
    },
}

pub(super) enum Admission {
    Discover(Box<CloseDiscovery>),
    Respond(Value),
}

fn parse(request: &Value, instance: &str) -> Result<Action, StableErrorCode> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| StableErrorCode::InvalidRequest)?;
    let params = request["params"]
        .as_object()
        .ok_or(StableErrorCode::InvalidArgument)?;
    let fields: &[&str] = match request["method"].as_str() {
        Some("development.connections.snapshot") => &["instanceId"],
        Some("development.connections.prepare") => &["instanceId", "handle"],
        Some("development.connections.confirm") => &[
            "instanceId",
            "operationId",
            "expectedRevision",
            "handle",
            "ticket",
        ],
        Some("development.connections.receipt") => &["instanceId", "operationId"],
        _ => return Err(StableErrorCode::UnknownMethod),
    };
    if !crate::mutation_protocol::exact_fields(params, fields, fields) {
        return Err(StableErrorCode::InvalidArgument);
    }
    let requested = params["instanceId"]
        .as_str()
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
        .ok_or(StableErrorCode::InvalidArgument)?;
    if requested != instance {
        return Err(StableErrorCode::Conflict);
    }
    let token = |field: &str| {
        params[field]
            .as_str()
            .and_then(OpaqueToken::from_wire)
            .ok_or(StableErrorCode::InvalidArgument)
    };
    let operation = || {
        params["operationId"]
            .as_str()
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= omavless_control_protocol::MAX_ID_LENGTH
                    && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
            })
            .map(str::to_owned)
            .ok_or(StableErrorCode::InvalidArgument)
    };
    match request["method"].as_str() {
        Some("development.connections.snapshot") => Ok(Action::Snapshot),
        Some("development.connections.prepare") => Ok(Action::Prepare {
            handle: token("handle")?,
        }),
        Some("development.connections.confirm") => Ok(Action::Confirm {
            operation: operation()?,
            expected_revision: params["expectedRevision"]
                .as_u64()
                .filter(|revision| *revision <= omavless_control_protocol::MAX_REVISION)
                .ok_or(StableErrorCode::InvalidArgument)?,
            handle: token("handle")?,
            ticket: token("ticket")?,
        }),
        Some("development.connections.receipt") => Ok(Action::Receipt {
            operation: operation()?,
        }),
        _ => Err(StableErrorCode::UnknownMethod),
    }
}

fn receipt_projection(
    instance: &str,
    operation: &str,
    receipt: Option<ExternalCloseReceipt>,
) -> Value {
    let outcome = receipt.map(|receipt| match receipt.outcome {
        ExternalCloseOutcome::Closed => "closed",
        ExternalCloseOutcome::Missing => "missing",
        ExternalCloseOutcome::Changed => "changed",
        ExternalCloseOutcome::Unsupported => "unsupported",
        ExternalCloseOutcome::Unknown => "unknown",
        ExternalCloseOutcome::RefusedBeforeWrite => "refused_before_write",
        ExternalCloseOutcome::MissingAttestation => "missing_attestation",
    });
    json!({
        "schemaVersion": 1,
        "scope": "development_owned_single_connection_close",
        "instanceId": instance,
        "operationId": operation,
        "state": if receipt.is_some() { "finished" } else { "pending" },
        "outcome": outcome,
        "receiptRevision": receipt.map(|receipt| receipt.revision),
    })
}

impl<H: LifecycleHost + Send + 'static> RegisteredNativeOwner<H> {
    pub(super) fn admit_developer_close(
        &mut self,
        action: Action,
        instance: &str,
    ) -> Result<Admission, NativeOwnerError> {
        let coordinator = self.owner.batch_coordinator();
        if !self.batch_initialized {
            // Bind to the actual server instance, never a caller's identifier.
            coordinator.initialize_batch_operations(instance)?;
            self.batch_initialized = true;
        }
        coordinator.poll_connection_close()?;
        match action {
            Action::Snapshot => coordinator
                .capture_connection_close()
                .map(|discovery| Admission::Discover(Box::new(discovery))),
            Action::Prepare { handle } => {
                let confirmation = coordinator.prepare_connection_close(handle)?;
                Ok(Admission::Respond(json!({
                    "schemaVersion": 1,
                    "scope": "development_owned_single_connection_close",
                    "instanceId": instance,
                    "handle": handle.to_wire(),
                    "ticket": confirmation.ticket.to_wire(),
                    "display": confirmation.display,
                })))
            }
            Action::Confirm {
                operation,
                expected_revision,
                handle,
                ticket,
            } => {
                // Existing exact replay precedes fresh ownership, revision and
                // expiry checks. It cannot send a second controller request.
                let receipt = coordinator.confirm_connection_close(
                    &operation,
                    expected_revision,
                    handle,
                    ticket,
                )?;
                Ok(Admission::Respond(receipt_projection(
                    instance, &operation, receipt,
                )))
            }
            Action::Receipt { operation } => {
                let receipt = coordinator
                    .connection_close_receipt(&operation)?
                    .ok_or(NativeOwnerError::RecordNotFound)?;
                Ok(Admission::Respond(receipt_projection(
                    instance, &operation, receipt,
                )))
            }
        }
    }

    pub(super) fn retain_developer_close(
        &mut self,
        discovered: CloseDiscovered,
    ) -> Result<Value, NativeOwnerError> {
        let rows = self
            .owner
            .batch_coordinator()
            .retain_connection_close(discovered)?;
        let rows = rows
            .into_iter()
            .map(|row| json!({"handle": row.handle.to_wire(), "display": row.display}))
            .collect::<Vec<_>>();
        Ok(json!({
            "schemaVersion": 1,
            "scope": "development_owned_single_connection_close",
            "rows": rows,
        }))
    }
}

impl RuntimeServer {
    pub(super) fn dispatch_developer_close(
        &self,
        request: &Value,
    ) -> Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let fail = |revision, code| {
            error_response(id, revision, code, code == StableErrorCode::Busy, None)
        };
        let action = match parse(request, &self.instance_id) {
            Ok(action) => action,
            Err(code) => return fail(0, code),
        };
        // Only detached discovery consumes a remote-work permit. Poll/replay
        // and confirmations never queue behind unrelated remote HTTP traffic.
        let _permit = if matches!(action, Action::Snapshot) {
            let Some(permit) = self.remote_fetches.try_acquire() else {
                return fail(0, StableErrorCode::Busy);
            };
            Some(permit)
        } else {
            None
        };
        let admission = {
            let Ok(mut dispatcher) = self.dispatcher.try_lock() else {
                return fail(0, StableErrorCode::Busy);
            };
            let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
                return fail(0, StableErrorCode::UnknownMethod);
            };
            match owner.developer_close(action, &self.instance_id) {
                Ok(Admission::Respond(result)) => {
                    return success_response(id, owner.revision(), result);
                }
                Ok(Admission::Discover(discovery)) => discovery,
                Err(error) => return fail(owner.revision(), error.stable_code()),
            }
        };
        // Controller reads and full developer-pair proof are outside the owner
        // mutex/lease. The original absolute expiry is never renewed here.
        let discovered = (*admission).observe();
        let Ok(mut dispatcher) = self.dispatcher.try_lock() else {
            return fail(0, StableErrorCode::Busy);
        };
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return fail(0, StableErrorCode::CapabilityUnavailable);
        };
        let discovered = match discovered {
            Ok(discovered) => discovered,
            Err(error) => return fail(owner.revision(), error.stable_code()),
        };
        match owner.developer_close_retain(discovered) {
            Ok(mut result) => {
                result["instanceId"] = json!(self.instance_id);
                success_response(id, owner.revision(), result)
            }
            Err(error) => fail(owner.revision(), error.stable_code()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_control_protocol::make_request;

    fn request(method: &str, params: Value) -> Value {
        // Deliberately malformed metadata must reach the validator under test,
        // not be rejected first by the normal well-formed request constructor.
        let mut request = make_request("dev-close", method, json!({})).unwrap();
        request["params"] = params;
        request
    }

    #[test]
    fn exact_shapes_reject_extra_or_missing_fields() {
        let token = "1".repeat(64);
        for (method, params) in [
            (METHODS[0], json!({"instanceId":"actual"})),
            (METHODS[1], json!({"instanceId":"actual","handle":token})),
            (
                METHODS[2],
                json!({"instanceId":"actual","operationId":"once","expectedRevision":0,"handle":token,"ticket":token}),
            ),
            (
                METHODS[3],
                json!({"instanceId":"actual","operationId":"once"}),
            ),
        ] {
            assert!(parse(&request(method, params.clone()), "actual").is_ok());
            let mut extra = params.clone();
            extra["rawControllerId"] = json!("not-authority");
            assert!(matches!(
                parse(&request(method, extra), "actual"),
                Err(StableErrorCode::InvalidArgument)
            ));
            for key in params.as_object().unwrap().keys() {
                let mut missing = params.clone();
                missing.as_object_mut().unwrap().remove(key);
                assert!(parse(&request(method, missing), "actual").is_err());
            }
        }
    }

    #[test]
    fn tokens_accept_only_canonical_nonzero_256_bit_handles() {
        for invalid in [
            "".to_owned(),
            "0".repeat(64),
            "1".repeat(63),
            "1".repeat(65),
            "A".repeat(64),
            "g".repeat(64),
            "é".repeat(32),
        ] {
            assert!(OpaqueToken::from_wire(&invalid).is_none());
        }
        let text = "0123456789abcdef".repeat(4);
        assert_eq!(OpaqueToken::from_wire(&text).unwrap().to_wire(), text);
        assert_eq!(
            format!("{:?}", OpaqueToken::from_wire(&text).unwrap()),
            "OpaqueCloseToken([private])"
        );
    }

    #[test]
    fn stale_instance_and_invalid_metadata_never_reach_owner() {
        assert!(matches!(
            parse(&request(METHODS[0], json!({"instanceId":"old"})), "actual"),
            Err(StableErrorCode::Conflict)
        ));
        for instance in [
            json!(null),
            json!(""),
            json!("bad instance"),
            json!("i".repeat(129)),
        ] {
            assert!(
                parse(
                    &request(METHODS[0], json!({"instanceId":instance})),
                    "actual"
                )
                .is_err()
            );
        }
        for operation in [
            json!(null),
            json!(""),
            json!("bad operation"),
            json!("o".repeat(65)),
        ] {
            assert!(
                parse(
                    &request(
                        METHODS[3],
                        json!({"instanceId":"actual","operationId":operation})
                    ),
                    "actual"
                )
                .is_err()
            );
        }
        for revision in [
            json!(-1),
            json!(1.5),
            json!("0"),
            json!(omavless_control_protocol::MAX_REVISION + 1),
        ] {
            assert!(parse(&request(METHODS[2], json!({"instanceId":"actual","operationId":"once","expectedRevision":revision,"handle":"1".repeat(64),"ticket":"2".repeat(64)})), "actual").is_err());
        }
    }

    #[test]
    fn pending_unknown_and_terminal_receipts_are_distinct() {
        let pending = receipt_projection("actual", "once", None);
        assert_eq!(pending["state"], "pending");
        assert!(pending["outcome"].is_null());
        assert!(pending["receiptRevision"].is_null());
        for outcome in [
            ExternalCloseOutcome::Closed,
            ExternalCloseOutcome::Missing,
            ExternalCloseOutcome::Changed,
            ExternalCloseOutcome::Unsupported,
            ExternalCloseOutcome::Unknown,
            ExternalCloseOutcome::RefusedBeforeWrite,
            ExternalCloseOutcome::MissingAttestation,
        ] {
            let result = receipt_projection(
                "actual",
                "once",
                Some(ExternalCloseReceipt {
                    outcome,
                    revision: 7,
                }),
            );
            assert_eq!(result["state"], "finished");
            assert_eq!(result["receiptRevision"], 7);
            assert!(result["outcome"].is_string());
        }
    }

    #[cfg(feature = "tui")]
    #[test]
    fn actual_client_hello_uses_canonical_dispatch_not_synthetic_empty_params() {
        use omavless_tui::developer_close::{Input, KeyCode, KeyEvent, KeyModifiers, Workspace};
        let mut workspace = Workspace::new(omavless_tui::i18n::Locale::En);
        let Input::Send(call) = workspace.input(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            std::time::Instant::now(),
            true,
        ) else {
            panic!("expected fixed hello");
        };
        let request = make_request("hello", call.method(), call.params()).unwrap();
        let response = crate::dispatch_read_only(&request, "actual").unwrap();
        assert!(response["ok"] == true && response["result"]["version"] == 1);
        let bad = make_request("hello", call.method(), json!({})).unwrap();
        let refused = crate::dispatch_read_only(&bad, "actual").unwrap();
        assert!(refused["ok"] == false && refused["error"]["code"] == "invalid_argument");
        // Read-only canonical dispatch does NOT mint native ownership. This is
        // real grammar/negotiation regression, not a native-authority fixture.
        assert!(response["result"]["runtimeOwnership"] == false);
    }

    #[cfg(feature = "tui")]
    #[test]
    fn actual_tui_workspace_calls_match_the_canonical_development_parser() {
        use omavless_tui::developer_close::{
            Call, Input, KeyCode, KeyEvent, KeyModifiers, Workspace,
        };
        fn call(input: Input) -> Call {
            match input {
                Input::Send(call) => call,
                _ => panic!("fixed call required"),
            }
        }
        let now = std::time::Instant::now();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let ok = |result| Ok(success_response("client", 0, result).unwrap());
        let mut workspace = Workspace::new(omavless_tui::i18n::Locale::En);
        call(workspace.input(key(KeyCode::Char('r')), now, true));
        workspace.accept(
            ok(json!({"version":1,"runtimeOwnership":true,"instanceId":"actual"})),
            now,
        );
        let snapshot = workspace
            .accept(ok(json!({"runtimeOwnership":true,"methods":METHODS})), now)
            .unwrap();
        assert!(matches!(
            parse(&request(snapshot.method(), snapshot.params()), "actual"),
            Ok(Action::Snapshot)
        ));
        let display = json!({"host":"synthetic.invalid","ip":null,"port":443,"network":"tcp","route":"direct"});
        workspace.accept(
            ok(
                json!({"schemaVersion":1,"scope":"development_owned_single_connection_close",
            "instanceId":"actual","rows":[{"handle":"1".repeat(64),"display":display}]}),
            ),
            now,
        );
        let prepare = call(workspace.input(key(KeyCode::Char('x')), now, true));
        assert!(matches!(
            parse(&request(prepare.method(), prepare.params()), "actual"),
            Ok(Action::Prepare { .. })
        ));
        workspace.accept(ok(json!({"schemaVersion":1,"scope":"development_owned_single_connection_close",
            "instanceId":"actual","handle":"1".repeat(64),"ticket":"2".repeat(64),"display":display})),now);
        let confirm = call(workspace.input(key(KeyCode::Enter), now, true));
        assert!(matches!(
            parse(&request(confirm.method(), confirm.params()), "actual"),
            Ok(Action::Confirm { .. })
        ));
        let operation = confirm.params()["operationId"].as_str().unwrap().to_owned();
        workspace.accept(ok(receipt_projection("actual", &operation, None)), now);
        let receipt = call(workspace.input(key(KeyCode::Char('u')), now, true));
        assert!(matches!(
            parse(&request(receipt.method(), receipt.params()), "actual"),
            Ok(Action::Receipt { .. })
        ));
        assert_eq!(receipt.params()["operationId"], operation);
    }
}
