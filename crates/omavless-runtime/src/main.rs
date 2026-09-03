// SPDX-License-Identifier: MIT

use nix::unistd::Uid;
use omavless_runtime::desired::{DesiredPaths, read_desired};
use omavless_runtime::production_observation::current_cutover_preflight;
use omavless_runtime::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES;
use omavless_runtime::semantic_cli::parse_semantic_mutation;
use omavless_runtime::store_preflight::current_store_preflight;
use omavless_runtime::{RuntimePaths, RuntimeServer, call};
use serde_json::json;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::env;
use std::io::{self, Read};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const USAGE: &str = "Usage: omavless COMMAND\n\nCommands:\n  daemon\n  hello\n  status\n  capabilities\n  connect PROFILE_ID [rule|global|direct]\n  disconnect\n  profile rename PROFILE_ID       read the new name from stdin\n  profile favorite PROFILE_ID on|off\n  profile delete PROFILE_ID\n  preflight\n  store-preflight\n  cutover-preflight";

fn read_rename_input() -> Result<String, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take((MAX_PROFILE_NAME_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "OmaVLESS semantic command input could not be read".to_owned())?;
    if bytes.len() > MAX_PROFILE_NAME_INPUT_BYTES {
        return Err("OmaVLESS semantic command input is too large".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "OmaVLESS semantic command input is invalid".to_owned())
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments == ["-h"] || arguments == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if arguments == ["preflight"] {
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
    if arguments == ["store-preflight"] {
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
    if arguments == ["cutover-preflight"] {
        let result = current_cutover_preflight().map_err(|error| error.to_string())?;
        println!(
            "{}",
            serde_json::to_string(&result.public_json()).map_err(|_| "Output failed")?
        );
        return Ok(());
    }
    let paths = RuntimePaths::current().map_err(|error| error.to_string())?;
    if arguments == ["daemon"] {
        let stop = Arc::new(AtomicBool::new(false));
        flag::register(SIGINT, Arc::clone(&stop)).map_err(|_| "Signal setup failed")?;
        flag::register(SIGTERM, Arc::clone(&stop)).map_err(|_| "Signal setup failed")?;
        return RuntimeServer::bind_current(paths)
            .and_then(|server| server.serve_until(&stop))
            .map_err(|error| error.to_string());
    }
    let (method, params) = if arguments == ["hello"] {
        ("system.hello", json!({"versions": [1]}))
    } else if arguments == ["status"] {
        ("status.get", json!({}))
    } else if arguments == ["capabilities"] {
        ("capabilities.get", json!({}))
    } else {
        let rename_input =
            (arguments.len() == 3 && arguments[0] == "profile" && arguments[1] == "rename")
                .then(read_rename_input)
                .transpose()?;
        parse_semantic_mutation(&arguments, rename_input.as_deref())
            .map_err(|error| error.to_string())?
            .into_parts()
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
