// SPDX-License-Identifier: MIT
//! Public synthetic TestBackend SVG, never runtime/archive/host acceptance.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{
    app::App,
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::ReadError,
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::{io::Write, os::unix::fs::OpenOptionsExt, time::Instant};
fn escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4
        || !matches!(args[0].as_str(), "en" | "ru")
        || !matches!(args[1].as_str(), "70x24" | "50x14")
        || !matches!(
            args[2].as_str(),
            "editing"
                | "confirming"
                | "confirming-long"
                | "unknown"
                | "completed"
                | "denied"
                | "settings"
                | "settings-end"
                | "cancelled-preview-waiting"
        )
    {
        return Err("invalid synthetic Restore preview arguments".into());
    }
    let locale = if args[0] == "ru" {
        Locale::Ru
    } else {
        Locale::En
    };
    let (w, h) = if args[1] == "50x14" {
        (50u16, 14u16)
    } else {
        (70, 24)
    };
    let now = Instant::now();
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.restore_enabled = true;
    app.backup_enabled = true;
    let mut s=load_page(&mut|read|{
        let mut v=support::response(read);
        match read {Read::Snapshot=>{v["result"]["desired"]["connected"]=false.into();v["result"]["desired"]["profileId"]="".into();v["result"]["lastKnownActual"]="disconnected".into();},
            Read::Observation=>{v["result"]["desired"]["connected"]=false.into();v["result"]["lastKnownActual"]="disconnected".into();v["result"]["facts"]=json!({"ownedCoreRunning":false,"visibleMihomoCount":0,"ownedAuxiliaryMihomoCount":0,"visibleTunCount":0,"managedTunCount":0,"ownedControllerConfigVerified":false,"desiredProfileMatchesOwned":false});},_=>()};Ok(v)
    },Page::Settings).unwrap();
    s.capabilities.private_restore = true;
    s.capabilities.private_backup = true;
    app.accept(Ok(s), now);
    let press = |app: &mut App, k| app.key_at(KeyEvent::new(k, KeyModifiers::NONE), now);
    press(&mut app, KeyCode::Char(','));
    if args[2] == "settings-end" {
        press(&mut app, KeyCode::End);
        view::clamp_scroll(&mut app, w, h, now);
    }
    if !matches!(args[2].as_str(), "settings" | "settings-end") {
        press(&mut app, KeyCode::Char('R'));
        let path = if args[2] == "confirming-long" {
            format!("/private/{}.ovb", "x".repeat(147))
        } else {
            "/private/public-fixture.ovb".into()
        };
        for ch in path.chars() {
            press(&mut app, KeyCode::Char(ch));
        }
        press(&mut app, KeyCode::Tab);
        for ch in "synthetic-only-secret".chars() {
            press(&mut app, KeyCode::Char(ch));
        }
        if args[2] != "editing" {
            press(&mut app, KeyCode::Enter);
            let original = app.take_restore_request().unwrap();
            if args[2] == "cancelled-preview-waiting" {
                // Retain the actual original synthetic request without a reply.
                press(&mut app, KeyCode::Esc);
                press(&mut app, KeyCode::Char('R'));
                std::hint::black_box(&original);
            } else {
                app.finish_restore(original.settle(Ok(json!({"ok":true,"revision":7,"result":{"profiles":3,"subscriptions":1,"scope":"privatePair","ciphertextDigest":"a".repeat(64)}}))));
            }
            if matches!(args[2].as_str(), "unknown" | "completed" | "denied") {
                press(&mut app, KeyCode::Enter);
                let original = app.take_restore_request().unwrap();
                let result = match args[2].as_str() {
                    "completed" => Ok(
                        json!({"ok":true,"revision":8,"result":{"completed":true,"replayed":false,"scope":"privatePair"}}),
                    ),
                    "denied" => Ok(json!({"ok":false,"error":{"code":"conflict"}})),
                    _ => Err(ReadError::Unavailable),
                };
                app.finish_restore(original.settle(result));
            }
        }
    }
    let mut t = Terminal::new(TestBackend::new(w, h))?;
    t.draw(|f| view::draw(f, &app, now))?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"><rect width=\"100%\" height=\"100%\" fill=\"#000\"/><g font-family=\"DejaVu Sans Mono,monospace\" font-size=\"15\" fill=\"#fff\">",
        w * 10,
        h * 20
    );
    for (i, c) in t.backend().buffer().content.iter().enumerate() {
        if !c.symbol().trim().is_empty() {
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\">{}</text>",
                (i % usize::from(w)) * 10,
                (i / usize::from(w)) * 20 + 16,
                escape(c.symbol())
            ));
        }
    }
    svg.push_str("</g></svg>\n");
    assert!(!svg.contains("synthetic-only-secret"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&args[3])?;
    file.write_all(svg.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
