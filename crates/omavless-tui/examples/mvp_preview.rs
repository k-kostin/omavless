// SPDX-License-Identifier: MIT
//! Synthetic full-client screens. No socket, private store, network or service.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{client::Read, model::ReadError};
use serde_json::json;
const A: &str = "10000000-0000-4000-8000-000000000001";
const B: &str = "10000000-0000-4000-8000-000000000002";
fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_default();
    let mut intent = None;
    let mut polls = 0;
    let result = omavless_tui::run_full(
        |r| {
            let mut v = support::response(r);
            if r == Read::Capabilities {
                v["result"]["methods"].as_array_mut().unwrap().extend([
                    json!("plugin.action"),
                    json!("profiles.details"),
                    json!("runtime.connections"),
                    json!("profiles.probe"),
                    json!("profiles.probe_results"),
                    json!("subscriptions.refresh_all"),
                    json!("operations.get"),
                    json!("operations.cancel"),
                ]);
            }
            if r == Read::Snapshot {
                v["result"]["desired"]["profileId"] = json!(A);
                v["result"]["profiles"][0]["id"] = json!(A);
                v["result"]["profiles"][1]["id"] = json!(B);
                v["result"]["subscriptions"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":"fixture-empty","name":"Empty fixture feed"}));
            }
            Ok(v)
        },
        |request| {
            let p = request.params();
            Ok(
                json!({"ok":true,"revision":7,"result":{"schemaVersion":1,"instanceId":p["instanceId"],"operationId":p["operationId"],"action":p["action"],"applied":true}}),
            )
        },
        move |call| {
            if matches!(
                call.method(),
                "profiles.probe" | "subscriptions.refresh_all"
            ) {
                intent = Some((call.method(), call.params()));
                polls = 0;
            }
            let Some((method, p)) = &intent else {
                return Err(ReadError::Unavailable);
            };
            if scenario == "unknown" {
                return Err(ReadError::Unavailable);
            }
            if call.method() == "profiles.probe_results" {
                let target = p["profileId"].as_str();
                let rows: Vec<_> = [A, B]
                    .into_iter()
                    .filter(|id| target.is_none_or(|t| t == *id))
                    .map(|id| json!({"id":id,"resolved":true,"reachable":true,"latencyMs":42}))
                    .collect();
                return Ok(
                    json!({"ok":true,"revision":7,"result":{"version":1,"profileId":p["profileId"],"results":rows}}),
                );
            }
            polls += 1;
            let cancelled = call.method() == "operations.cancel";
            let terminal = cancelled || (polls > 3 && scenario != "slow");
            let total = if p.get("profileId").is_some() { 1 } else { 2 };
            let revision =
                7 + u64::from(terminal && !cancelled && *method == "subscriptions.refresh_all");
            Ok(json!({"ok":true,"revision":revision,"result":{"operation":{
                "instanceId":p["instanceId"],"operationId":p["operationId"],"method":method,
                "baseRevision":7,"outcomeRevision":terminal.then_some(revision),
                "state":if cancelled {"cancelled"} else if terminal {"succeeded"} else {"running"},
                "progress":{"completed":if terminal && !cancelled {total}else{0},"total":total},
                "cancelRequested":cancelled,"cancellable":!terminal,"error":null}}}))
        },
    );
    if result.is_err() {
        eprintln!("Synthetic TUI unavailable");
        std::process::exit(1);
    }
}
