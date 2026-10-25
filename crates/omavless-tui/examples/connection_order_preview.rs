// SPDX-License-Identifier: MIT
//! Fixed synthetic TestBackend rendering: no runtime, network or mutation adapter.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{
    app::App,
    client::{Read, load_page},
    connection_browsing::Order,
    i18n::Locale,
    inspection::Page,
    model::ReadError,
    view,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
use std::{io::Write, os::unix::fs::OpenOptionsExt, time::Instant};
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(feature = "developer-conditional-close")]
    if args.len() == 4 && args[2].starts_with("set-") {
        return selected_set_preview(&args);
    }

    if args.len() != 4
        || !matches!(args[0].as_str(), "en" | "ru")
        || !matches!(args[1].as_str(), "70x24" | "60x24")
        || !matches!(
            args[2].as_str(),
            "original"
                | "destination"
                | "network"
                | "route"
                | "filtered"
                | "stale"
                | "loading"
                | "empty"
                | "type-tcp"
                | "type-udp"
                | "type-other"
                | "type-search"
                | "route-direct"
                | "route-blocked"
                | "route-proxy"
                | "route-unclassified"
                | "route-combined"
                | "route-no-match"
                | "details"
                | "details-middle"
                | "details-end"
                | "details-search"
                | "details-rule-search"
                | "details-rule-search-end"
                | "details-rule-unknown"
                | "details-rule-unknown-end"
                | "details-rule-missing"
                | "details-rule-missing-end"
                | "details-rule-legacy"
                | "details-rule-legacy-end"
                | "details-rule-stale"
                | "details-missing"
                | "details-legacy"
                | "details-stale"
                | "details-actions"
                | "details-actions-middle"
                | "details-actions-end"
                | "details-long"
                | "details-long-middle"
                | "details-long-later"
                | "details-long-end"
                | "details-download"
                | "details-download-end"
                | "details-upload"
                | "details-upload-end"
                | "details-upload-legacy"
                | "details-upload-stale"
        )
    {
        return Err("invalid fixed synthetic Connections arguments".into());
    }
    let locale = if args[0] == "ru" {
        Locale::Ru
    } else {
        Locale::En
    };
    let (w, h) = if args[1] == "60x24" {
        (60u16, 24u16)
    } else {
        (70, 24)
    };
    let now = Instant::now();
    let details_scene = args[2].starts_with("details") && !args[2].contains("-legacy");
    let mut app = App::new(locale);
    app.actions_enabled = args[2].starts_with("details-actions");
    app.page = if args[2] == "loading" {
        Page::Profiles
    } else {
        Page::Connections
    };
    app.connection_order = match args[2].as_str() {
        "network" => Order::Network,
        "route" => Order::Route,
        "original" => Order::Original,
        _ => Order::Destination,
    };
    if args[2].starts_with("details-long") {
        app.connection_order = Order::Original;
    }
    if args[2].starts_with("details-download") {
        app.connection_order = Order::Download;
    } else if args[2].starts_with("details-upload") {
        app.connection_order = Order::Upload;
    }
    app.accept(load_page(&mut |r| {
        let mut value=support::response(r);
        if r==Read::Capabilities {
            value["result"]["methods"]=json!(["ui.snapshot","runtime.observation","runtime.connection_rows"]);
            if details_scene {value["result"]["methods"].as_array_mut().unwrap().push(json!("runtime.connection_details"));}
        }
        if matches!(r, Read::ConnectionRows | Read::ConnectionDetails) {
            value["result"]["total"]=json!(200);
            value["result"]["shown"]=json!(4);
            value["result"]["truncated"]=json!(true);
            value["result"]["rows"]=json!([
                {"host":"zeta.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
                {"host":"alpha.invalid","ip":null,"port":53,"network":"udp","route":"direct"},
                {"host":"omega.invalid","ip":null,"port":443,"network":"tcp","route":"blocked"},
                {"host":null,"ip":null,"port":null,"network":"other","route":"unclassified"}
            ]);
            if r==Read::ConnectionDetails {
                value["result"]["scope"]=json!("owned_core_private_connection_details");
                for (index,row) in value["result"]["rows"].as_array_mut().unwrap().iter_mut().enumerate() {
                    row["processName"] = match index {0=>json!("fixture-browser"),2=>json!("test-client"),_=>json!(null)};
                    row["chain"] = match index {0=>json!(["Домашний узел 🌐", "PROXY"]),1=>json!(["DIRECT"]),2=>json!(["REJECT"]),_=>json!(null)};
                    row["downloadBytes"] = match index {0=>json!("9223372036854775807"),2=>json!("512"),_=>json!(null)};
                    row["uploadBytes"] = match index {0=>json!("0"),2=>json!("9007199254740993"),_=>json!(null)};
                    row["ruleType"] = match index {0=>json!("DomainSuffix"),1=>json!("Match"),2=>json!("Unknown"),_=>json!(null)};
                    if args[2].starts_with("details-rule-missing") {row["ruleType"]=json!(null);}
                    if args[2]=="details-missing" {row["processName"]=json!("/private/never-display");row["chain"]=json!(["unsafe\u{202e}label"]);row["ruleType"]=json!("/private/never-rule");}
                    if args[2].starts_with("details-long") && index==0 {
                        row["processName"]=json!("long-".repeat(12)+"test");
                        row["chain"]=json!(["A".repeat(64),"B".repeat(64),"C".repeat(64),"D".repeat(64)]);
                    }
                }
            }
            if args[2]=="empty" {value["result"]["rows"]=json!([]);value["result"]["total"]=json!(0);value["result"]["shown"]=json!(0);value["result"]["truncated"]=json!(false);}
        }
        Ok(value)
    },if args[2]=="loading" {Page::Profiles} else {Page::Connections}),now);
    app.page = Page::Connections;
    if args[2] == "filtered" {
        app.operator_query = "tcp".into();
    }
    let cycles = match args[2].as_str() {
        "type-tcp" | "type-search" | "route-combined" => 1,
        "type-udp" | "route-no-match" => 2,
        "type-other" => 3,
        _ => 0,
    };
    for _ in 0..cycles {
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
    }
    let route_cycles = match args[2].as_str() {
        "route-direct" => 1,
        "route-blocked" | "route-combined" | "route-no-match" => 2,
        "route-proxy" => 3,
        "route-unclassified" => 4,
        _ => 0,
    };
    for _ in 0..route_cycles {
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
    }
    if args[2] == "type-search"
        || args[2] == "details-search"
        || args[2].starts_with("details-rule-")
    {
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for ch in if args[2] == "details-search" {
            "домашний"
        } else if args[2].starts_with("details-rule-unknown")
            || args[2].starts_with("details-rule-missing")
        {
            "uNkNoWn"
        } else if args[2].starts_with("details-rule-") {
            "dOmAiNsUfFiX"
        } else {
            "alpha"
        }
        .chars()
        {
            app.key_at(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), now);
        }
        app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
    }
    if args[2] == "details-middle"
        || args[2] == "details-end"
        || args[2] == "details-legacy"
        || args[2] == "details-actions-middle"
        || args[2] == "details-actions-end"
        || args[2] == "details-long-middle"
        || args[2] == "details-long-later"
        || args[2] == "details-long-end"
        || args[2] == "details-download-end"
        || args[2] == "details-upload-end"
        || (args[2].starts_with("details-rule-") && args[2].ends_with("-end"))
    {
        let steps = if args[2].ends_with("-end") {
            64
        } else if args[2].ends_with("-later") {
            16
        } else {
            8
        };
        for _ in 0..steps {
            app.key_at(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        }
    }
    if matches!(
        args[2].as_str(),
        "stale" | "details-stale" | "details-upload-stale" | "details-rule-stale"
    ) {
        app.accept(Err(ReadError::Changed), now);
    }
    view::clamp_scroll(&mut app, w, h, now);
    let mut terminal = Terminal::new(TestBackend::new(w, h))?;
    terminal.draw(|f| view::draw(f, &app, now))?;
    write_cells(terminal.backend().buffer(), w, h, &args[3])
}
fn write_cells(
    buffer: &ratatui::buffer::Buffer,
    w: u16,
    h: u16,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"><rect width=\"100%\" height=\"100%\" fill=\"#000\"/><g fill=\"#fff\" font-family=\"DejaVu Sans Mono,monospace\" font-size=\"15\">",
        w * 10,
        h * 20
    );
    for (i, cell) in buffer.content.iter().enumerate() {
        if !cell.symbol().trim().is_empty() {
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\">{}</text>",
                (i % usize::from(w)) * 10,
                (i / usize::from(w)) * 20 + 16,
                escape(cell.symbol())
            ));
        }
    }
    svg.push_str("</g></svg>\n");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(svg.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(feature = "developer-conditional-close")]
fn selected_set_preview(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use omavless_tui::developer_close::{Input, Workspace};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    if !matches!(args[0].as_str(), "en" | "ru")
        || !matches!(args[1].as_str(), "70x24" | "60x24")
        || !matches!(
            args[2].as_str(),
            "set-select"
                | "set-selected"
                | "set-review"
                | "set-review-middle"
                | "set-review-later"
                | "set-review-end"
                | "set-confirm"
                | "set-pending"
                | "set-partial"
                | "set-partial-middle"
                | "set-partial-later"
                | "set-partial-end"
                | "set-unknown"
                | "set-unknown-middle"
                | "set-unknown-end"
                | "set-unknown-later"
                | "set-closed"
                | "set-closed-middle"
                | "set-expired"
                | "set-closed-later"
                | "set-empty"
                | "set-refused"
                | "set-busy-metadata"
                | "set-busy-confirm"
                | "set-closed-end"
        )
    {
        return Err("invalid fixed selected-set preview".into());
    }
    let now = Instant::now();
    let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
    let call = |input| match input {
        Input::Send(c) => c,
        _ => panic!("fixed synthetic call"),
    };
    let ok = |result| Ok(json!({"ok":true,"revision":0,"result":result}));
    let mut workspace = Workspace::new_selected_set(Locale::parse(&args[0]));
    call(workspace.input(key(KeyCode::Char('r')), now, true));
    if args[2] == "set-busy-metadata" {
        return draw_selected_set(&workspace, args, now);
    }

    workspace.accept(
        ok(json!({"version":1,"runtimeOwnership":true,"instanceId":"synthetic-set-preview"})),
        now,
    );
    workspace.accept(ok(json!({"runtimeOwnership":true,"connectionCloseView":"inert-preview-v1","methods":["development.connection_sets.snapshot","development.connection_sets.prepare","development.connection_sets.confirm","development.connection_sets.receipt"]})),now);
    let mut rows=(1..=6).map(|i|json!({"handle":format!("{i:x}").repeat(64),"display":{"host":format!("connection-{i}-{}.invalid","long-destination-".repeat(5)),"ip":"2001:db8::7","port":443,"network":"tcp","route":"direct"}})).collect::<Vec<_>>();
    if args[2] == "set-empty" {
        rows.clear();
    }
    workspace.accept(ok(json!({"schemaVersion":1,"scope":"development_owned_connection_set_close","instanceId":"synthetic-set-preview","rows":rows})),now);
    if !matches!(args[2].as_str(), "set-select" | "set-empty") {
        workspace.input(key(KeyCode::Char('a')), now, true);
        if args[2] != "set-selected" {
            workspace.input(key(KeyCode::Char('x')), now, true);
            if !args[2].starts_with("set-review") {
                let prepare = call(workspace.input(key(KeyCode::Enter), now, true));
                assert_eq!(prepare.params()["handles"].as_array().unwrap().len(), 6);
                workspace.accept(ok(json!({"schemaVersion":1,"scope":"development_owned_connection_set_close","instanceId":"synthetic-set-preview","rows":rows,"ticket":"a".repeat(64)})),now);
                if !matches!(args[2].as_str(), "set-confirm" | "set-expired") {
                    let confirm = call(workspace.input(key(KeyCode::Enter), now, true));
                    let operation = confirm.params()["operationId"].clone();
                    if args[2] == "set-busy-confirm" {
                        return draw_selected_set(&workspace, args, now);
                    }

                    let (aggregate, members) = if args[2].starts_with("set-partial") {
                        (
                            "partial",
                            vec![
                                "closed",
                                "closed",
                                "changed",
                                "not_attempted",
                                "not_attempted",
                                "not_attempted",
                            ],
                        )
                    } else if args[2].starts_with("set-unknown") {
                        (
                            "unknown",
                            vec![
                                "unknown",
                                "unknown",
                                "not_attempted",
                                "not_attempted",
                                "not_attempted",
                                "not_attempted",
                            ],
                        )
                    } else if args[2] == "set-refused" {
                        ("refused_before_write", vec!["not_attempted"; 6])
                    } else {
                        ("all_closed", vec!["closed"; 6])
                    };
                    let pending = args[2] == "set-pending";
                    let wire = json!({"ok":true,"revision":1,"result":{"schemaVersion":1,"scope":"development_owned_connection_set_close","instanceId":"synthetic-set-preview","operationId":operation,"state":if pending{"pending"}else{"finished"},"receiptRevision":if pending{None}else{Some(1)},"aggregate":if pending{None}else{Some(aggregate)},"members":if pending{None}else{Some(members.iter().enumerate().map(|(index,outcome)|json!({"index":index,"outcome":outcome})).collect::<Vec<_>>())}}});
                    workspace.accept(Ok(wire), now);
                }
            }
        }
    }
    if args[2].ends_with("-middle") {
        workspace.input(key(KeyCode::PageDown), now, true);
    }
    if args[2].ends_with("-later") {
        workspace.input(key(KeyCode::PageDown), now, true);
        workspace.input(key(KeyCode::PageDown), now, true);
    }
    if args[2].ends_with("-end") {
        workspace.input(key(KeyCode::End), now, true);
    }
    let draw_at = if args[2] == "set-expired" {
        now + std::time::Duration::from_secs(6)
    } else {
        now
    };
    draw_selected_set(&workspace, args, draw_at)
}
#[cfg(feature = "developer-conditional-close")]
fn draw_selected_set(
    workspace: &omavless_tui::developer_close::Workspace,
    args: &[String],
    now: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = if args[1] == "70x24" {
        (70, 24)
    } else {
        (60, 24)
    };
    let mut terminal = Terminal::new(TestBackend::new(w, h))?;
    terminal.draw(|frame| workspace.draw(frame, now))?;
    write_cells(terminal.backend().buffer(), w, h, &args[3])
}
