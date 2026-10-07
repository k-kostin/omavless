// SPDX-License-Identifier: MIT
//! Public synthetic TestBackend SVG only. No runtime/socket/store/engine calls.
#[path = "../tests/support/mod.rs"]
mod support;
use omavless_tui::{app::App, client::load_page, i18n::Locale, inspection::Page, view};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use std::{io::Write, os::unix::fs::OpenOptionsExt, time::Instant};

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4
        || !matches!(args[0].as_str(), "en" | "ru")
        || !matches!(args[1].as_str(), "70x24" | "100x32")
        || !matches!(args[2].as_str(), "editing" | "confirming" | "unknown")
    {
        return Err("invalid synthetic preview arguments".into());
    }
    let locale = if args[0] == "ru" {
        Locale::Ru
    } else {
        Locale::En
    };
    let (width, height) = if args[1] == "70x24" {
        (70u16, 24u16)
    } else {
        (100, 32)
    };
    let now = Instant::now();
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.backup_enabled = true;
    let mut snapshot = load_page(&mut |r| Ok(support::response(r)), Page::Settings).unwrap();
    snapshot.capabilities.private_backup = true;
    app.accept(Ok(snapshot), now);
    let press = |app: &mut App, code| app.key_at(KeyEvent::new(code, KeyModifiers::NONE), now);
    press(&mut app, KeyCode::Char(','));
    press(&mut app, KeyCode::Char('b'));
    for ch in "/private/public-fixture.ovb".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    press(&mut app, KeyCode::Tab);
    for ch in "synthetic-backup-secret".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    press(&mut app, KeyCode::Tab);
    for ch in "synthetic-backup-secret".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    if args[2] != "editing" {
        press(&mut app, KeyCode::Enter);
    }
    if args[2] == "unknown" {
        press(&mut app, KeyCode::Enter);
        let original = app.take_backup_request().unwrap();
        app.finish_backup(original.settle(Err(omavless_tui::model::ReadError::Unavailable)));
    }
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|f| view::draw(f, &app, now))?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#000000\"/><g font-family=\"DejaVu Sans Mono,monospace\" font-size=\"15\" fill=\"#ffffff\">",
        width * 10,
        height * 20,
        width * 10,
        height * 20
    );
    for (index, cell) in terminal.backend().buffer().content.iter().enumerate() {
        if cell.symbol().trim().is_empty() {
            continue;
        }
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\">{}</text>",
            (index % usize::from(width)) * 10,
            (index / usize::from(width)) * 20 + 16,
            escape(cell.symbol())
        ));
    }
    svg.push_str("</g></svg>\n");
    assert!(!svg.contains("synthetic-backup-secret"));
    // Explicit SOURCE output, private/exclusive. Never a runtime archive path.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&args[3])?;
    file.write_all(svg.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
