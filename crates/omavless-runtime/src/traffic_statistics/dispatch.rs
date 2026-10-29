// SPDX-License-Identifier: MIT
use super::{protocol, worker};
use crate::{RuntimeDispatcher, RuntimeServer, error_response, success_response};
use omavless_control_protocol::{ProtocolError, StableErrorCode};
use serde_json::Value;

fn code(error: worker::Error) -> StableErrorCode {
    match error {
        worker::Error::Busy => StableErrorCode::Busy,
        worker::Error::Conflict => StableErrorCode::Conflict,
        worker::Error::Unavailable => StableErrorCode::CapabilityUnavailable,
        worker::Error::Unknown => StableErrorCode::ManualRecoveryRequired,
    }
}

impl RuntimeServer {
    pub(crate) fn dispatch_statistics(&self, request: &Value) -> Result<Value, ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        if crate::product_scope::backup_only() {
            return error_response(id, 0, StableErrorCode::CapabilityUnavailable, false, None);
        }
        let fail = |revision, code| error_response(id, revision, code, false, None);
        let Some(Some(worker)) = self.statistics.get() else {
            return fail(0, StableErrorCode::CapabilityUnavailable);
        };
        let parsed = if request["method"] == "stats.get" {
            None
        } else {
            let parsed = match protocol::parse(request) {
                Ok(parsed) => parsed,
                Err(error) => return fail(0, error.stable_code()),
            };
            if parsed.instance != self.instance_id {
                return fail(parsed.owner_revision, StableErrorCode::DaemonRestarting);
            }
            match worker.lookup(&parsed) {
                Ok(Some(Ok(mut value))) => {
                    value["instanceId"] = self.instance_id.clone().into();
                    value["operationId"] = parsed.operation.into();
                    return success_response(id, parsed.owner_revision, value);
                }
                Ok(Some(Err(error))) | Err(error) => {
                    return fail(parsed.owner_revision, code(error));
                }
                Ok(None) => {}
            }
            Some(parsed)
        };
        let capture = {
            let Ok(dispatcher) = self.dispatcher.try_lock() else {
                return fail(0, StableErrorCode::Busy);
            };
            let RuntimeDispatcher::Native(owner) = &*dispatcher else {
                return fail(0, StableErrorCode::CapabilityUnavailable);
            };
            let Some(capture) = owner.stats_capture() else {
                return fail(owner.revision(), StableErrorCode::CapabilityUnavailable);
            };
            capture
        };
        let Some(desired) = capture.desired() else {
            return fail(capture.revision, StableErrorCode::CapabilityUnavailable);
        };
        if request["method"] == "stats.get" {
            if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
                return fail(capture.revision, StableErrorCode::InvalidArgument);
            }
            let mut value = worker.view();
            value["instanceId"] = self.instance_id.clone().into();
            return success_response(id, capture.revision, value);
        }
        let parsed = parsed.expect("non-read request parsed before new-operation admission");
        if parsed.owner_revision != capture.revision {
            return fail(capture.revision, StableErrorCode::Conflict);
        }
        // The metadata operation is admitted while independently settled Off.
        // Later Connect is not blocked by its disk write. This is not a promise
        // that the VPN remains Off throughout an asynchronous metadata commit.
        if desired.connected {
            return fail(capture.revision, StableErrorCode::Busy);
        }
        let off = {
            let Ok(dispatcher) = self.dispatcher.try_lock() else {
                return fail(capture.revision, StableErrorCode::Busy);
            };
            let RuntimeDispatcher::Native(owner) = &*dispatcher else {
                return fail(capture.revision, StableErrorCode::CapabilityUnavailable);
            };
            let Some(off) = owner.stats_off_context(&capture) else {
                return fail(capture.revision, StableErrorCode::Busy);
            };
            off
        };
        if !off.settled() || capture.desired().as_ref() != Some(&desired) {
            return fail(capture.revision, StableErrorCode::CapabilityUnavailable);
        }
        let utc = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let receive = {
            let Ok(dispatcher) = self.dispatcher.try_lock() else {
                return fail(capture.revision, StableErrorCode::Busy);
            };
            let RuntimeDispatcher::Native(owner) = &*dispatcher else {
                return fail(capture.revision, StableErrorCode::CapabilityUnavailable);
            };
            if !owner.stats_off_current(&capture) {
                return fail(capture.revision, StableErrorCode::Conflict);
            }
            match worker.submit(&parsed, utc) {
                Ok(receive) => receive,
                Err(error) => return fail(capture.revision, code(error)),
            }
        };
        // No dispatcher, lifecycle or migration lease survives this wait.
        match receive.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Ok(mut value)) => {
                value["instanceId"] = self.instance_id.clone().into();
                value["operationId"] = parsed.operation.into();
                success_response(id, capture.revision, value)
            }
            Ok(Err(error)) => fail(capture.revision, code(error)),
            Err(_) => fail(capture.revision, StableErrorCode::ManualRecoveryRequired),
        }
    }
}
