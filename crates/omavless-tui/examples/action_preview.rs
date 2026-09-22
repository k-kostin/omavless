// SPDX-License-Identifier: MIT
//! Synthetic action screens only; never opens a runtime socket or private store.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{client::Read, model::ReadError};
use serde_json::json;
use std::io::Write;
fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_default();
    // Test-only acknowledgement: avoids mistaking incremental ANSI bytes for a
    // rendered screen. No profile/request data is written, and never overwrite.
    let receipt = std::env::args_os().nth(2);
    let read_scenario = scenario.clone();
    let result = omavless_tui::run_actions(
        move |r| {
            let mut v = support::response(r);
            if r == Read::Capabilities {
                v["result"]["methods"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("plugin.action"));
            }
            if read_scenario == "recovery" {
                if matches!(r, Read::Snapshot | Read::Observation) {
                    v["result"]["lastKnownActual"] = json!("manualRecoveryRequired");
                }
                if r == Read::Observation {
                    v["result"]["manualRecoveryRequired"] = json!(true);
                }
            }
            Ok(v)
        },
        move |request| {
            if let Some(path) = &receipt {
                use std::os::unix::fs::OpenOptionsExt;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)
                {
                    let _ = file.write_all(b"synthetic-action-started\n");
                }
            }
            if scenario == "slow" {
                std::thread::sleep(std::time::Duration::from_secs(20));
            }
            if scenario == "unknown" {
                return Err(ReadError::Unavailable);
            }
            let p = request.params();
            Ok(
                json!({"ok":true,"revision":7,"result":{"schemaVersion":1,"instanceId":p["instanceId"],
            "operationId":p["operationId"],"action":p["action"],"applied":true}}),
            )
        },
    );
    if let Err(message) = result {
        let _ = writeln!(std::io::stderr(), "{message}");
        std::process::exit(1);
    }
}
