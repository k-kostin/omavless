// SPDX-License-Identifier: MIT
use super::{admission, protocol, worker};
use crate::{RuntimeDispatcher, RuntimeServer, error_response, success_response};
use omavless_control_protocol::{ProtocolError, StableErrorCode};
use serde_json::Value;

fn code(error: worker::Error) -> StableErrorCode {
    match error {
        worker::Error::Busy => StableErrorCode::Busy,
        worker::Error::Conflict => StableErrorCode::Conflict,
        worker::Error::Unavailable => StableErrorCode::CapabilityUnavailable,
        worker::Error::Unknown => StableErrorCode::ManualRecoveryRequired,
        worker::Error::Restarting => StableErrorCode::DaemonRestarting,
    }
}

impl RuntimeServer {
    pub(crate) fn dispatch_statistics(&self, request: &Value) -> Result<Value, ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let fail = |revision, code| error_response(id, revision, code, false, None);
        if crate::product_scope::backup_only() {
            return fail(0, StableErrorCode::CapabilityUnavailable);
        }
        let Some(Some(worker)) = self.statistics.get() else {
            return fail(0, StableErrorCode::CapabilityUnavailable);
        };
        let read = request["method"] == "stats.get";
        if read && !request["params"].as_object().is_some_and(|p| p.is_empty()) {
            return fail(0, StableErrorCode::InvalidArgument);
        }
        let parsed = if read {
            None
        } else {
            match protocol::parse(request) {
                Ok(parsed) => Some(parsed),
                Err(error) => return fail(0, error.stable_code()),
            }
        };
        let capture = {
            // Only memory reads and retained-result lookup under this gate.
            let Ok(gate) = self.quit_gate.try_read() else {
                return fail(0, StableErrorCode::Busy);
            };
            if *gate {
                return fail(0, StableErrorCode::DaemonRestarting);
            }
            if let Some(parsed) = &parsed {
                if parsed.instance != self.instance_id {
                    return fail(parsed.owner_revision, StableErrorCode::DaemonRestarting);
                }
                match worker.lookup(parsed) {
                    Ok(Some(Ok(mut value))) => {
                        value["instanceId"] = self.instance_id.clone().into();
                        value["operationId"] = parsed.operation.clone().into();
                        return success_response(id, parsed.owner_revision, value);
                    }
                    Ok(Some(Err(error))) | Err(error) => {
                        return fail(parsed.owner_revision, code(error));
                    }
                    Ok(None) => {}
                }
            }
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
        if parsed
            .as_ref()
            .is_some_and(|p| p.owner_revision != capture.revision)
        {
            return fail(capture.revision, StableErrorCode::Conflict);
        }
        let revision = capture.revision;
        let dispatcher = std::sync::Arc::downgrade(&self.dispatcher);
        let gate = std::sync::Arc::clone(&self.quit_gate);
        let preflight: admission::Preflight = Box::new(move || {
            // No RuntimeServer or strong lifecycle-owner reference crosses I/O.
            let desired = capture.desired().ok_or(worker::Error::Unavailable)?;
            if !read {
                if desired.connected {
                    return Err(worker::Error::Busy);
                }
                let off = {
                    let owner = dispatcher.upgrade().ok_or(worker::Error::Restarting)?;
                    let dispatcher = owner.try_lock().map_err(|_| worker::Error::Busy)?;
                    let RuntimeDispatcher::Native(owner) = &*dispatcher else {
                        return Err(worker::Error::Unavailable);
                    };
                    owner
                        .stats_off_context(&capture)
                        .ok_or(worker::Error::Busy)?
                };
                if !off.settled() || capture.desired().as_ref() != Some(&desired) {
                    return Err(worker::Error::Unavailable);
                }
            }
            Ok(admission::Admission::current_owner(
                gate, dispatcher, capture, read,
            ))
        });
        let receive = if let Some(parsed) = &parsed {
            let utc = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            worker.submit(parsed, utc, preflight)
        } else {
            worker.read(preflight)
        };
        let receive = match receive {
            Ok(receive) => receive,
            Err(error) => return fail(revision, code(error)),
        };
        // The ordinary unary worker is bounded even if statistics preflight or
        // storage stalls. There is no quit, dispatcher or migration lease here.
        match receive.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Ok(mut value)) => {
                value["instanceId"] = self.instance_id.clone().into();
                if let Some(parsed) = parsed {
                    value["operationId"] = parsed.operation.into();
                }
                success_response(id, revision, value)
            }
            Ok(Err(error)) => fail(revision, code(error)),
            Err(_) => fail(
                revision,
                if read {
                    StableErrorCode::CapabilityUnavailable
                } else {
                    StableErrorCode::ManualRecoveryRequired
                },
            ),
        }
    }
}
