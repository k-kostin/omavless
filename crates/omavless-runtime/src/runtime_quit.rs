// SPDX-License-Identifier: MIT
//! Confirmed whole-application exit; ordinary UI close never calls this method.
use super::*;

fn disconnect_request(request: &Value) -> std::result::Result<Value, StableErrorCode> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| StableErrorCode::InvalidRequest)?;
    let fields = ["instanceId", "expectedRevision", "operationId"];
    let params = request["params"]
        .as_object()
        .ok_or(StableErrorCode::InvalidArgument)?;
    if request["method"] != "runtime.quit"
        || params.len() != fields.len()
        || fields.iter().any(|field| !params.contains_key(*field))
    {
        return Err(StableErrorCode::InvalidArgument);
    }
    let mut canonical = request.clone();
    canonical["method"] = json!("plugin.action");
    canonical["params"]["action"] = json!("disconnect");
    plugin_action::parse(&canonical).map_err(|_| StableErrorCode::InvalidArgument)?;
    Ok(canonical)
}

fn clean(observation: &Value) -> bool {
    observation["ok"] == true
        && observation["result"]["availability"] == "observed"
        && observation["result"]["desired"]["connected"] == false
        && observation["result"]["lastKnownActual"] == "disconnected"
        && observation["result"]["manualRecoveryRequired"] == false
        && observation["result"]["facts"]["ownedCoreRunning"] == false
        && observation["result"]["facts"]["visibleMihomoCount"] == 0
        && observation["result"]["facts"]["ownedAuxiliaryMihomoCount"] == 0
        && observation["result"]["facts"]["managedTunCount"] == 0
}

impl RuntimeServer {
    pub(super) fn dispatch_quit(
        &self,
        request: &Value,
    ) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
        let id = request["id"].as_str().unwrap_or("invalid");
        let canonical = match disconnect_request(request) {
            Ok(value) => value,
            Err(code) => return error_response(id, 0, code, false, None),
        };
        // No timeout-driven kill of another operation. A concurrent unary
        // operation must finish before the user can retry this explicit exit.
        let Ok(mut gate) = self.quit_gate.try_write() else {
            return error_response(id, 0, StableErrorCode::Busy, true, None);
        };
        if *gate {
            return error_response(id, 0, StableErrorCode::DaemonRestarting, false, None);
        }
        let response = self.dispatch_admitted(&canonical)?;
        if response["ok"] != true {
            return Ok(response);
        }
        // The existing disconnect quiesces auxiliary work. Retain its slot
        // again across the final fresh proof and admission seal.
        let _auxiliary_guard = match self.quiesce_auxiliary() {
            Ok(guard) => guard,
            Err(code) => return error_response(id, 0, code, false, None),
        };
        let mut dispatcher = self.dispatcher.lock().map_err(|_| {
            omavless_control_protocol::ProtocolError::new(StableErrorCode::InternalError)
        })?;
        let RuntimeDispatcher::Native(owner) = &mut *dispatcher else {
            return error_response(id, 0, StableErrorCode::CapabilityUnavailable, false, None);
        };
        let observation = owner.runtime_observation(&make_request(
            "quit-proof",
            "runtime.observation",
            json!({}),
        )?)?;
        if !clean(&observation) {
            return error_response(
                id,
                owner.revision(),
                StableErrorCode::ManualRecoveryRequired,
                false,
                None,
            );
        }
        owner.batch_stop();
        let result = success_response(
            id,
            owner.revision(),
            json!({
                "schemaVersion":1, "instanceId":self.instance_id,
                "operationId":request["params"]["operationId"],
                "disconnected":true, "runtimeStopping":true
            }),
        )?;
        *gate = true;
        self.quit_requested.store(true, Ordering::Release);
        // serve_until drains bounded workers/jobs, drops the private socket
        // and owner lock, and returns success (Restart=on-failure stays off).
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_request_reuses_disconnect_validation_without_private_echo() {
        let base = make_request(
            "quit",
            "runtime.quit",
            json!({
                "instanceId":"instance", "expectedRevision":0, "operationId":"quit-1"
            }),
        )
        .unwrap();
        assert!(disconnect_request(&base).is_ok());
        for (field, value) in [
            ("command", json!("private.invalid/password")),
            ("operationId", json!("")),
            ("expectedRevision", json!(-1)),
            ("instanceId", json!("")),
        ] {
            let mut bad = base.clone();
            bad["params"][field] = value;
            assert!(disconnect_request(&bad).is_err());
        }
    }
    #[test]
    fn cleanup_proof_fails_closed_for_missing_or_positive_facts() {
        let proof = json!({"ok":true,"result":{
            "availability":"observed", "desired":{"connected":false},
            "lastKnownActual":"disconnected", "manualRecoveryRequired":false,
            "facts":{"ownedCoreRunning":false,"visibleMihomoCount":0,
                "ownedAuxiliaryMihomoCount":0,"visibleTunCount":0,"managedTunCount":0}}});
        assert!(clean(&proof));
        for key in [
            "visibleMihomoCount",
            "ownedAuxiliaryMihomoCount",
            "managedTunCount",
        ] {
            for value in [json!(1), Value::Null] {
                let mut bad = proof.clone();
                bad["result"]["facts"][key] = value;
                assert!(!clean(&bad));
            }
        }
        for (key, value) in [
            ("availability", json!("unavailable")),
            ("manualRecoveryRequired", json!(true)),
            ("lastKnownActual", json!("failed")),
        ] {
            let mut bad = proof.clone();
            bad["result"][key] = value;
            assert!(!clean(&bad));
        }
        assert!(!clean(&json!({})));
        let mut foreign = proof.clone();
        foreign["result"]["facts"]["visibleTunCount"] = json!(2);
        assert!(clean(&foreign));
    }
}
