// SPDX-License-Identifier: MIT

use omavless_runtime::{RuntimePaths, RuntimeServer, call};
use serde_json::json;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const USAGE: &str = "Usage: omavless daemon|hello|status|capabilities";

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments == ["-h"] || arguments == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if arguments.len() != 1 {
        return Err("Invalid command. Use --help.".to_owned());
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
