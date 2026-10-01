// SPDX-License-Identifier: MIT
//! Isolated synthetic UI review only; does not link the runtime or open its socket.
#[path = "../tests/support/mod.rs"]
mod support;
use std::io::Write;
fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_default();
    if let Err(message) = omavless_tui::run(move |r| {
        if scenario == "slow" {
            std::thread::sleep(std::time::Duration::from_secs(20));
        }
        if scenario == "operator-slow-rows" && r == omavless_tui::client::Read::ConnectionRows {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        if scenario == "unavailable" {
            return Err(omavless_tui::model::ReadError::Unavailable);
        }
        let mut response = support::response(r);
        // The operator scenario enables only synthetic read capabilities. It
        // never exposes a mutation method or contacts a runtime socket.
        if (scenario == "operator" || scenario == "operator-slow-rows")
            && r == omavless_tui::client::Read::Capabilities
        {
            response["result"]["methods"] = serde_json::json!([
                "ui.snapshot",
                "runtime.observation",
                "runtime.traffic",
                "runtime.connection_overview",
                "runtime.connection_rows",
                "diagnostics.summary",
                "diagnostics.rules",
                "diagnostics.providers",
                "diagnostics.export",
                "routing.custom_rules.list",
                "routing.check",
                "profiles.details"
            ]);
        }
        if scenario == "subscriptions" && r == omavless_tui::client::Read::Snapshot {
            response["result"]["subscriptions"][0]["updatedAt"] =
                serde_json::json!(1_790_000_000_000_u64);
            response["result"]["profiles"][1]["missing"] = true.into();
            for i in 1..64 {
                response["result"]["subscriptions"].as_array_mut().unwrap().push(serde_json::json!({
                    "id":format!("fixture-sub-{i}"),"name":format!("Empty subscription {i}"),"updatedAt":0
                }));
            }
        }
        if matches!(
            r,
            omavless_tui::client::Read::Snapshot | omavless_tui::client::Read::Observation
        ) {
            if scenario == "operator" && r == omavless_tui::client::Read::Observation {
                response["result"]["coreDiagnostics"] = serde_json::json!({
                    "scope":"latest_owned_core_log_counts",
                    "dnsErrors":2,"tlsErrors":1,"timeoutErrors":0,
                    "connectionErrors":3,"otherWarnings":0,"oversizedLines":0,
                    "readFailed":false,"incomplete":false,"finished":false
                });
                response["result"]["coreLogHints"] = serde_json::json!({
                    "schemaVersion":1,"scope":"latest_owned_core_log_categories",
                    "availability":"observed","interpretation":"log_hints_not_health",
                    "items":[{"sequence":1,"category":"dns"},
                             {"sequence":2,"category":"connection"}],
                    "incomplete":false
                });
            }
            if scenario == "recovery" {
                response["result"]["lastKnownActual"] = "manualRecoveryRequired".into();
                if r == omavless_tui::client::Read::Observation {
                    response["result"]["manualRecoveryRequired"] = true.into();
                }
            }
            if scenario == "empty" {
                response["result"]["lastKnownActual"] = "disconnected".into();
                response["result"]["desired"]["connected"] = false.into();
                if r == omavless_tui::client::Read::Snapshot {
                    response["result"]["desired"]["profileId"] = "".into();
                    response["result"]["profiles"] = serde_json::json!([]);
                    response["result"]["subscriptions"] = serde_json::json!([]);
                } else {
                    for field in [
                        "ownedCoreRunning",
                        "ownedControllerConfigVerified",
                        "desiredProfileMatchesOwned",
                    ] {
                        response["result"]["facts"][field] = false.into();
                    }
                    for field in ["visibleMihomoCount", "managedTunCount", "visibleTunCount"] {
                        response["result"]["facts"][field] = 0.into();
                    }
                }
            }
        }
        Ok(response)
    }) {
        let _ = writeln!(std::io::stderr(), "{message}");
        std::process::exit(1);
    }
}
