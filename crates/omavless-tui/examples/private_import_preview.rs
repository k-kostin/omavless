// SPDX-License-Identifier: MIT
//! Explicit synthetic terminal fixture: no socket/store/provider/core/service.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{client::Read, model::ReadError};
use serde_json::json;
use std::io::Write;
fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "bundle".into());
    if !matches!(
        scenario.as_str(),
        "bundle" | "profile" | "subscription" | "unknown" | "no-new"
    ) {
        std::process::exit(2);
    }
    let mut receipt = std::env::args_os().nth(2).map(|path| {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("new synthetic receipt")
    });
    let result = omavless_tui::run_full(
        |read| {
            let mut v = support::response(read);
            if read == Read::Capabilities {
                v["result"]["methods"]
                    .as_array_mut()
                    .unwrap()
                    .extend([json!("plugin.action"), json!("imports.classify")]);
            }
            Ok(v)
        },
        |_| Err(ReadError::Unavailable),
        |_| Err(ReadError::Unavailable),
        Some(Box::new(move |request| {
            let p = request.params();
            let response = if request.mutation() {
                if let Some(file) = &mut receipt {
                    let _ = file.write_all(b"save\n");
                    let _ = file.flush();
                }
                if scenario == "unknown" {
                    Err(ReadError::Unavailable)
                } else {
                    Ok(json!({"ok":true,"revision":8,"result":{
                    "schemaVersion":1,"applied":true,"instanceId":p["instanceId"],"operationId":p["operationId"],"action":p["action"]}}))
                }
            } else {
                if p["input"] != "synthetic one\nsynthetic two" {
                    return request.settle(Ok(json!({"ok":false,"revision":7,"error":{"code":"invalid_argument","message":"Synthetic input mismatch","retryable":false}})));
                }
                if let Some(file) = &mut receipt {
                    let _ = file.write_all(b"classify\n");
                    let _ = file.flush();
                }
                let preview = match scenario.as_str() {
                    "profile" => {
                        json!({"version":1,"kind":"profile","profile":{"version":1,"protocol":"vless","suggestedName":"Synthetic import fixture","insecure":false,"experimental":false}})
                    }
                    "subscription" => {
                        json!({"version":1,"kind":"subscription","suggestedName":"Synthetic feed fixture","duplicate":false})
                    }
                    _ => {
                        json!({"version":1,"kind":"bundle","counts":{"accepted":if scenario=="no-new" {0}else{2},"duplicates":1,"skipped":0},"previewFingerprint":"a".repeat(64)})
                    }
                };
                Ok(json!({"ok":true,"revision":7,"result":preview}))
            };
            request.settle(response)
        })),
    );
    if result.is_err() {
        eprintln!("Synthetic import terminal unavailable");
        std::process::exit(1);
    }
}
