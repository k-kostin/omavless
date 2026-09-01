// SPDX-License-Identifier: MIT

use nix::unistd::Uid;
use omavless_runtime::desired::{DesiredPaths, read_desired};
use omavless_runtime::store_preflight::current_store_preflight;
use omavless_runtime::{RuntimePaths, RuntimeServer, call};
use serde_json::json;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const USAGE: &str = "Usage: omavless daemon|hello|status|capabilities|preflight|store-preflight";

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments == ["-h"] || arguments == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if arguments.len() != 1 {
        return Err("Invalid command. Use --help.".to_owned());
    }
    if arguments[0] == "preflight" {
        let paths = DesiredPaths::current().map_err(|error| error.to_string())?;
        let state =
            read_desired(&paths, Uid::current().as_raw()).map_err(|error| error.to_string())?;
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schemaVersion": state.schema_version,
                "generation": state.generation,
                "connected": state.connected,
                "profilePresent": !state.profile_id.is_empty(),
                "mode": state.mode,
            }))
            .map_err(|_| "Output failed")?
        );
        return Ok(());
    }
    if arguments[0] == "store-preflight" {
        let result = current_store_preflight().map_err(|error| error.to_string())?;
        let projection = result.projection;
        println!(
            "{}",
            serde_json::to_string(&json!({
                "version": projection.version,
                "profileCount": projection.profile_count,
                "subscriptionCount": projection.subscription_count,
                "protocolCounts": {
                    "vless": projection.vless_count,
                    "trojan": projection.trojan_count,
                    "hysteria2": projection.hysteria2_count,
                    "tuic": projection.tuic_count,
                },
                "activePresent": projection.active_present,
                "lastPresent": projection.last_present,
                "routingPreset": projection.routing_preset,
                "customRuleCount": projection.custom_rule_count,
                "startupConfigured": projection.startup_configured,
                "onboardingComplete": projection.onboarding_complete,
                "configReady": result.config_ready,
            }))
            .map_err(|_| "Output failed")?
        );
        return Ok(());
    }
    let paths = RuntimePaths::current().map_err(|error| error.to_string())?;
    if arguments[0] == "daemon" {
        let stop = Arc::new(AtomicBool::new(false));
        flag::register(SIGINT, Arc::clone(&stop)).map_err(|_| "Signal setup failed")?;
        flag::register(SIGTERM, Arc::clone(&stop)).map_err(|_| "Signal setup failed")?;
        return RuntimeServer::bind(paths)
            .and_then(|server| server.serve_until(&stop))
            .map_err(|error| error.to_string());
    }
    let (method, params) = if arguments[0] == "hello" {
        ("system.hello", json!({"versions": [1]}))
    } else if arguments[0] == "status" {
        ("status.get", json!({}))
    } else if arguments[0] == "capabilities" {
        ("capabilities.get", json!({}))
    } else {
        return Err("Invalid command. Use --help.".to_owned());
    };
    let response = call(&paths, method, params).map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string(&response).map_err(|_| "Output failed")?
    );
    if response["ok"] == true {
        Ok(())
    } else {
        Err("OmaVLESS runtime rejected the request".to_owned())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
