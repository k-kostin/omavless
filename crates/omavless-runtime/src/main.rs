// SPDX-License-Identifier: MIT

use nix::unistd::Uid;
use omavless_runtime::desired::{DesiredPaths, read_desired};
use omavless_runtime::production_observation::current_cutover_preflight;
use omavless_runtime::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES;
use omavless_runtime::semantic_cli::{
    MAX_SUBSCRIPTION_STDIN_BYTES, parse_semantic_import_preview, parse_semantic_mutation,
    parse_semantic_read,
};
use omavless_runtime::store_preflight::current_store_preflight;
use omavless_runtime::{RuntimePaths, RuntimeServer, call};
use serde_json::json;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::env;
use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const USAGE: &str = "Usage: omavless COMMAND\n\nCommands:\n  daemon\n  hello\n  status\n  capabilities\n  connect PROFILE_ID [rule|global|direct]\n  disconnect\n  mode rule|global|direct\n  profile list\n  profile rename PROFILE_ID       read the new name from stdin\n  profile favorite PROFILE_ID on|off\n  profile delete PROFILE_ID\n  subscription list\n  subscription edit-input SUBSCRIPTION_ID\n  subscription add                read name + URL lines from stdin\n  subscription update SUBSCRIPTION_ID\n                                  read name + URL lines from stdin\n  subscription delete SUBSCRIPTION_ID\n  subscription refresh SUBSCRIPTION_ID\n  subscription refresh-all INSTANCE_ID OPERATION_ID [REVISION]\n  operation get INSTANCE_ID OPERATION_ID\n  operation cancel INSTANCE_ID OPERATION_ID\n  preflight\n  store-preflight\n  cutover-preflight";

fn read_semantic_input(maximum_bytes: usize) -> Result<String, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take((maximum_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "OmaVLESS semantic command input could not be read".to_owned())?;
    if bytes.len() > maximum_bytes {
        return Err("OmaVLESS semantic command input is too large".to_owned());
    }
    String::from_utf8(bytes).map_err(|_| "OmaVLESS semantic command input is invalid".to_owned())
}

enum CliError {
    Message(String),
    DesktopCancelled,
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_owned())
    }
}

fn run() -> Result<(), CliError> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments == ["-h"] || arguments == ["--help"] {
        println!(
            "{USAGE}\n  import preview                  read private input from stdin; private UI output"
        );
        println!("  profile import                  read confirmed name + profile link from stdin");
        println!("  profile export PROFILE_ID qr|file  explicit private credential output");
        println!("  profile edit-input PROFILE_ID    explicit private editor input");
        println!("  routing rules                    private custom-rule editor list");
        println!("  plugin snapshot                  private UI metadata; not live health");
        println!("  diagnostics summary|rules|providers  bounded live controller diagnostics");
        println!(
            "  diagnostics export               shareable native configuration report (no live host checks)"
        );
        println!("  routing preset PRESET [keep-mode]  adopt a bundled routing policy");
        println!("  routing rule-add KIND ACTION     read private rule value from stdin");
        println!("  routing rule-delete RULE_ID       remove one custom rule");
        println!("  routing refresh-providers INSTANCE_ID OPERATION_ID [REVISION]");
        println!("  routing check                    read private domain/IP query from stdin");
        println!("  onboarding complete              mark first-use setup complete");
        println!(
            "  store-compatibility              read-only native store check and recovery guidance"
        );
        println!(
            "  profile replace PROFILE_ID      read confirmed name + replacement link from stdin"
        );
        println!(
            "  desktop capabilities|clipboard-read|clipboard-copy|pick-import|file-read|edit|qr|export-file|cleanup"
        );
        println!(
            "                                  explicit private client-only helpers; input through stdin"
        );
        return Ok(());
    }
    if arguments.first().is_some_and(|arg| arg == "desktop") {
        use omavless_runtime::desktop_helpers::{
            self, DesktopHelpers, MAX_PATH_BYTES, MAX_TEXT_BYTES,
        };
        if arguments.len() != 2 {
            return Err("Invalid desktop helper command".into());
        }
        let helpers = DesktopHelpers::current();
        let output = match arguments[1].to_str() {
            Some("capabilities") => {
                println!("{}", helpers.capabilities());
                return Ok(());
            }
            Some("clipboard-read") => helpers.clipboard_read(),
            Some("cleanup") => {
                let directory =
                    desktop_helpers::current_desktop_runtime().map_err(|e| e.to_string())?;
                let removed = desktop_helpers::cleanup(&directory).map_err(|e| e.to_string())?;
                println!("{}", json!({"removed": removed}));
                return Ok(());
            }
            Some("pick-import") => helpers.pick_import(),
            Some("file-read") => {
                let path = read_semantic_input(MAX_PATH_BYTES + 1)?;
                desktop_helpers::read_import_file(path.trim_end_matches('\n').as_bytes())
            }
            Some("clipboard-copy" | "edit" | "qr") => {
                let input = read_semantic_input(MAX_TEXT_BYTES)?;
                match arguments[1].to_str() {
                    Some("clipboard-copy") => helpers
                        .clipboard_copy(input.as_bytes())
                        .map(|()| Vec::new()),
                    Some("qr") => helpers.qr_png(input.as_bytes()),
                    _ => {
                        let directory = desktop_helpers::current_desktop_runtime()
                            .map_err(|e| e.to_string())?;
                        helpers.edit(input.as_bytes(), &directory)
                    }
                }
            }
            Some("export-file") => {
                let input = read_semantic_input(MAX_PATH_BYTES + 1 + MAX_TEXT_BYTES)?;
                let (path, content) = input
                    .split_once('\n')
                    .ok_or("Invalid desktop helper input")?;
                desktop_helpers::export_file(path.as_bytes(), content.as_bytes())
                    .map(|()| Vec::new())
            }
            _ => return Err("Invalid desktop helper command".into()),
        }
        .map_err(|e| match e {
            desktop_helpers::Error::Cancelled => CliError::DesktopCancelled,
            _ => CliError::Message(e.to_string()),
        })?;
        io::stdout()
            .lock()
            .write_all(&output)
            .map_err(|_| "Desktop helper output failed".to_string())?;
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
    if arguments == ["store-compatibility"] {
        let result = omavless_runtime::store_preflight::current_store_compatibility()
            .map_err(|error| error.to_string())?;
        println!("{}", result.public_json());
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
            .map_err(|error| CliError::Message(error.to_string()));
    }
    let (method, params) = if arguments == ["hello"] {
        ("system.hello", json!({"versions": [1]}))
    } else if arguments == ["status"] {
        ("status.get", json!({}))
    } else if arguments == ["capabilities"] {
        ("capabilities.get", json!({}))
    } else if arguments == ["routing", "check"] {
        let input = read_semantic_input(omavless_domain::routing::MAX_CUSTOM_RULE_VALUE_BYTES)?;
        omavless_runtime::semantic_cli::parse_semantic_route_check(&arguments, &input)
            .map_err(|error| error.to_string())?
            .into_parts()
    } else if arguments == ["import", "preview"] {
        let input =
            read_semantic_input(omavless_runtime::import_read_protocol::MAX_IMPORT_STDIN_BYTES)?;
        parse_semantic_import_preview(&arguments, Some(&input))
            .map_err(|error| error.to_string())?
            .into_parts()
    } else if arguments == ["profile", "import"] {
        let input =
            read_semantic_input(omavless_runtime::semantic_cli::MAX_PROFILE_IMPORT_STDIN_BYTES)?;
        omavless_runtime::semantic_cli::parse_semantic_profile_import(&arguments, Some(&input))
            .map_err(|error| error.to_string())?
            .into_parts()
    } else if arguments.len() == 3 && arguments[0] == "profile" && arguments[1] == "replace" {
        let input =
            read_semantic_input(omavless_runtime::semantic_cli::MAX_PROFILE_IMPORT_STDIN_BYTES)?;
        omavless_runtime::semantic_cli::parse_semantic_profile_replace(&arguments, Some(&input))
            .map_err(|error| error.to_string())?
            .into_parts()
    } else {
        match parse_semantic_read(&arguments).map_err(|error| error.to_string())? {
            Some(request) => request.into_parts(),
            None => {
                let input_limit = if arguments.len() == 3
                    && arguments[0] == "profile"
                    && arguments[1] == "rename"
                {
                    Some(MAX_PROFILE_NAME_INPUT_BYTES)
                } else if (arguments.len() == 2
                    && arguments[0] == "subscription"
                    && arguments[1] == "add")
                    || (arguments.len() == 3
                        && arguments[0] == "subscription"
                        && arguments[1] == "update")
                {
                    Some(MAX_SUBSCRIPTION_STDIN_BYTES)
                } else if arguments.len() == 4
                    && arguments[0] == "routing"
                    && arguments[1] == "rule-add"
                {
                    Some(omavless_domain::routing::MAX_CUSTOM_RULE_VALUE_BYTES)
                } else {
                    None
                };
                let private_input = input_limit.map(read_semantic_input).transpose()?;
                parse_semantic_mutation(&arguments, private_input.as_deref())
                    .map_err(|error| error.to_string())?
                    .into_parts()
            }
        }
    };
    let response = call(&paths, method, params).map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::to_string(&response).map_err(|_| "Output failed")?
    );
    if response["ok"] == true {
        Ok(())
    } else {
        Err("OmaVLESS runtime rejected the request".into())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::DesktopCancelled) => ExitCode::from(3),
        Err(CliError::Message(message)) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
