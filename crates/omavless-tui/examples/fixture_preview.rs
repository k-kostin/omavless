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
        if scenario == "unavailable" {
            return Err(omavless_tui::model::ReadError::Unavailable);
        }
        let mut response = support::response(r);
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
