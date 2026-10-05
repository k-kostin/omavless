// SPDX-License-Identifier: MIT
// CLOSED synthetic backend for private real-terminal visual review only.
// This file has no socket/process/core/service/filesystem/privileged operations.

struct TerminalDemo {
    mode: String,
    prepared: Option<String>,
    operation: Option<String>,
    confirm_attempted: bool,
    valid_confirm: bool,
}
impl TerminalDemo {
    fn new(mode: &str) -> Self {
        assert!(["empty", "rows", "closed", "unknown"].contains(&mode));
        Self {
            mode: mode.to_owned(),
            prepared: None,
            operation: None,
            confirm_attempted: false,
            valid_confirm: false,
        }
    }
    fn display(handle: &str) -> serde_json::Value {
        json!({"host":if handle == "1".repeat(64) {"private-demo-a.invalid"} else {"private-demo-b.invalid"},
            "ip":if handle == "1".repeat(64) {"192.0.2.1"} else {"192.0.2.2"},
            "port":443,"network":"tcp","route":"unclassified"})
    }
    fn respond(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, omavless_tui::model::ReadError> {
        use omavless_tui::model::ReadError;
        let invalid = || ReadError::Invalid;
        let exact = |names: &[&str]| {
            params
                .as_object()
                .is_some_and(|p| p.len() == names.len() && names.iter().all(|n| p.contains_key(*n)))
        };
        let methods = [
            "development.connections.snapshot",
            "development.connections.prepare",
            "development.connections.confirm",
            "development.connections.receipt",
        ];
        let instance = "synthetic-terminal-owner";
        let scope = "development_owned_single_connection_close";
        let result = match method {
            "system.hello" if exact(&[]) => {
                json!({"version":1,"runtimeOwnership":true,"instanceId":instance})
            }
            "capabilities.get" if exact(&[]) => json!({"runtimeOwnership":true,"methods":methods}),
            "development.connections.snapshot"
                if exact(&["instanceId"])
                    && params["instanceId"] == instance
                    && !self.confirm_attempted =>
            {
                self.prepared = None;
                let rows = if self.mode == "empty" {
                    Vec::new()
                } else {
                    ["1".repeat(64), "2".repeat(64)]
                        .into_iter()
                        .map(|handle| json!({"handle":handle,"display":Self::display(&handle)}))
                        .collect()
                };
                json!({"schemaVersion":1,"scope":scope,"instanceId":instance,"rows":rows})
            }
            "development.connections.prepare"
                if exact(&["instanceId", "handle"])
                    && params["instanceId"] == instance
                    && !self.confirm_attempted
                    && self.mode != "empty" =>
            {
                let handle = params["handle"]
                    .as_str()
                    .filter(|h| *h == "1".repeat(64) || *h == "2".repeat(64))
                    .ok_or_else(invalid)?;
                self.prepared = Some(handle.to_owned());
                json!({"schemaVersion":1,"scope":scope,"instanceId":instance,"handle":handle,"ticket":"3".repeat(64),"display":Self::display(handle)})
            }
            "development.connections.confirm"
                if exact(&[
                    "instanceId",
                    "operationId",
                    "expectedRevision",
                    "handle",
                    "ticket",
                ]) =>
            {
                if self.confirm_attempted {
                    return Err(invalid());
                }
                self.confirm_attempted = true; // before every fallible request check
                let operation = params["operationId"]
                    .as_str()
                    .filter(|s| {
                        !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| (33..=126).contains(&b))
                    })
                    .ok_or_else(invalid)?;
                self.operation = Some(operation.to_owned());
                if self.prepared.is_none()
                    || params["instanceId"] != instance
                    || params["expectedRevision"] != 7
                    || params["handle"].as_str() != self.prepared.as_deref()
                    || params["ticket"] != "3".repeat(64)
                {
                    return Err(invalid());
                }
                self.valid_confirm = true;
                self.receipt(self.mode == "closed")
            }
            "development.connections.receipt"
                if exact(&["instanceId", "operationId"])
                    && params["instanceId"] == instance
                    && self.confirm_attempted
                    && self.valid_confirm
                    && self.operation.is_some()
                    && params["operationId"].as_str() == self.operation.as_deref() =>
            {
                self.receipt(true)
            }
            _ => return Err(invalid()),
        };
        omavless_control_protocol::success_response("synthetic-terminal-reply", 7, result)
            .map_err(|_| invalid())
    }
    fn receipt(&self, finished: bool) -> serde_json::Value {
        json!({"schemaVersion":1,"scope":"development_owned_single_connection_close","instanceId":"synthetic-terminal-owner",
            "operationId":self.operation,"state":if finished{"finished"}else{"pending"},
            "outcome":if !finished{None}else if self.mode=="unknown"{Some("unknown")}else{Some("closed")},
            "receiptRevision":if finished{Some(7)}else{None}})
    }
}

#[test]
#[ignore = "ROOT-selected private real-terminal synthetic rendering; no live authority"]
fn developer_close_private_terminal_demo() {
    let mode = std::env::var("OMAVLESS_T3_CLIENT_TERMINAL_DEMO").unwrap();
    let mut backend = TerminalDemo::new(&mode);
    // Locale comes only from the existing bounded process-local EN/RU override.
    // No real RuntimePaths, controller, user unit or interpreter callback exists.
    assert!(
        omavless_tui::developer_close::run(
            move |call| backend.respond(call.method(), call.params())
        )
        .is_ok()
    );
}

#[test]
fn terminal_demo_is_closed_and_original_operation_cannot_be_replaced_or_resent() {
    let instance = "synthetic-terminal-owner";
    for mode in ["rows", "closed", "unknown"] {
        let mut backend = TerminalDemo::new(mode);
        assert!(backend.respond("system.hello", json!({})).is_ok());
        assert!(backend.respond("capabilities.get", json!({})).is_ok());
        assert!(
            backend
                .respond(
                    "development.connections.snapshot",
                    json!({"instanceId":instance})
                )
                .is_ok()
        );
        let prepare = backend
            .respond(
                "development.connections.prepare",
                json!({"instanceId":instance,"handle":"2".repeat(64)}),
            )
            .unwrap();
        assert!(prepare["result"]["display"]["host"] == "private-demo-b.invalid");
        let params = json!({"instanceId":instance,"operationId":"original-demo","expectedRevision":7,"handle":"2".repeat(64),"ticket":"3".repeat(64)});
        assert!(
            backend
                .respond("development.connections.confirm", params.clone())
                .is_ok()
        );
        let read = backend
            .respond(
                "development.connections.receipt",
                json!({"instanceId":instance,"operationId":"original-demo"}),
            )
            .unwrap();
        let replay = backend
            .respond(
                "development.connections.receipt",
                json!({"instanceId":instance,"operationId":"original-demo"}),
            )
            .unwrap();
        assert!(read == replay);
        assert!(
            backend
                .respond("development.connections.confirm", params)
                .is_err()
        );
        assert!(
            backend
                .respond(
                    "development.connections.snapshot",
                    json!({"instanceId":instance})
                )
                .is_err()
        );
        assert!(
            backend
                .respond(
                    "development.connections.receipt",
                    json!({"instanceId":instance,"operationId":"foreign"})
                )
                .is_err()
        );
        assert!(backend.respond("systemctl", json!({})).is_err());
    }
    let mut empty = TerminalDemo::new("empty");
    let response = empty
        .respond(
            "development.connections.snapshot",
            json!({"instanceId":instance}),
        )
        .unwrap();
    assert!(response["result"]["rows"].as_array().unwrap().is_empty());
    assert!(
        empty
            .respond(
                "development.connections.prepare",
                json!({"instanceId":instance,"handle":"1".repeat(64)})
            )
            .is_err()
    );
    let mut invalid = TerminalDemo::new("rows");
    assert!(invalid.respond("development.connections.confirm",json!({"instanceId":instance,"operationId":"original-demo","expectedRevision":7,"handle":null,"ticket":"3".repeat(64)})).is_err());
    assert!(
        invalid
            .respond(
                "development.connections.receipt",
                json!({"instanceId":instance,"operationId":"original-demo"})
            )
            .is_err()
    );
}
