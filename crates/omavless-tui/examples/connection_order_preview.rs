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
    let mut app = App::new(locale);
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
    app.accept(load_page(&mut |r| {
        let mut value=support::response(r);
        if r==Read::Capabilities {value["result"]["methods"]=json!(["ui.snapshot","runtime.observation","runtime.connection_rows"]);}
        if r==Read::ConnectionRows {
            value["result"]["total"]=json!(200);
            value["result"]["shown"]=json!(4);
            value["result"]["truncated"]=json!(true);
            value["result"]["rows"]=json!([
                {"host":"zeta.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
                {"host":"alpha.invalid","ip":null,"port":53,"network":"udp","route":"direct"},
                {"host":"omega.invalid","ip":null,"port":443,"network":"tcp","route":"blocked"},
                {"host":null,"ip":null,"port":null,"network":"other","route":"unclassified"}
            ]);
            if args[2]=="empty" {value["result"]["rows"]=json!([]);value["result"]["total"]=json!(0);value["result"]["shown"]=json!(0);value["result"]["truncated"]=json!(false);}
        }
        Ok(value)
    },if args[2]=="loading" {Page::Profiles} else {Page::Connections}),now);
    app.page = Page::Connections;
    if args[2] == "filtered" {
        app.operator_query = "tcp".into();
    }
    if args[2] == "stale" {
        app.accept(Err(ReadError::Changed), now);
    }
    let mut terminal = Terminal::new(TestBackend::new(w, h))?;
    terminal.draw(|f| view::draw(f, &app, now))?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"><rect width=\"100%\" height=\"100%\" fill=\"#000\"/><g fill=\"#fff\" font-family=\"DejaVu Sans Mono,monospace\" font-size=\"15\">",
        w * 10,
        h * 20
    );
    for (i, cell) in terminal.backend().buffer().content.iter().enumerate() {
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
        .open(&args[3])?;
    file.write_all(svg.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
