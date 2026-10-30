// SPDX-License-Identifier: MIT
//! Explicit synthetic terminal fixture: no socket/store/provider/core/service.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{client::Read, model::ReadError};
use serde_json::json;
use std::io::Write;
fn render_managed(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use omavless_tui::{app::App, client::load, i18n::Locale, private_import::Workspace, view};
    use ratatui::{Terminal, backend::TestBackend};
    use std::{os::unix::fs::OpenOptionsExt, time::Instant};
    if args.len() != 4
        || !matches!(args[0].as_str(), "en" | "ru")
        || !matches!(args[1].as_str(), "60x24" | "70x24")
    {
        return Err("invalid fixed synthetic import arguments".into());
    }
    let now = Instant::now();
    let mut app = App::new(if args[0] == "ru" {
        Locale::Ru
    } else {
        Locale::En
    });
    app.actions_enabled = true;
    app.import_enabled = true;
    app.accept(
        load(&mut |read| {
            let mut value = support::response(read);
            if read == Read::Capabilities {
                value["result"]["methods"]
                    .as_array_mut()
                    .unwrap()
                    .extend([json!("plugin.action"), json!("imports.classify")]);
            }
            Ok(value)
        }),
        now,
    );
    let mut workspace = Workspace::new("fixture-runtime", 7, 1).unwrap();
    workspace.append("synthetic input");
    let request = workspace.classify(now).unwrap();
    workspace.accept(request.settle(Ok(json!({"ok":true,"revision":7,"result":{
        "version":1,"kind":"profile","profile":{"version":1,"protocol":"amneziawg",
        "suggestedName":"Synthetic import fixture","transport":"udp","security":"wireguard",
        "credentialHint":"••••","experimental":true,"insecure":false,"compatibilityNote":args[2]}}}))));
    app.import = Some(workspace);
    app.import_open = true;
    let width = if args[1] == "60x24" { 60 } else { 70 };
    let mut terminal = Terminal::new(TestBackend::new(width, 24))?;
    terminal.draw(|frame| view::draw(frame, &app, now))?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"480\"><rect width=\"100%\" height=\"100%\" fill=\"#000\"/><g fill=\"#fff\" font-family=\"DejaVu Sans Mono,monospace\" font-size=\"15\">",
        width * 10
    );
    for (i, cell) in terminal.backend().buffer().content.iter().enumerate() {
        if !cell.symbol().trim().is_empty() {
            let escaped = cell
                .symbol()
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\">{escaped}</text>",
                (i % usize::from(width)) * 10,
                (i / usize::from(width)) * 20 + 16
            ));
        }
    }
    svg.push_str("</g></svg>\n");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&args[3])?;
    file.write_all(svg.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--render-managed") {
        if render_managed(&args[1..]).is_err() {
            std::process::exit(2);
        }
        return;
    }
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
        omavless_tui::ClientTools {
            import: Some(Box::new(move |request| {
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
            statistics: None,
        },
    );
    if result.is_err() {
        eprintln!("Synthetic import terminal unavailable");
        std::process::exit(1);
    }
}
