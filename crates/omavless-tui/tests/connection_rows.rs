// SPDX-License-Identifier: MIT
mod support;

use omavless_tui::{
    app::App,
    client::{Read, load_page},
    i18n::Locale,
    inspection::{ConnectionRows, Page},
    model::ReadError,
    view,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn read(request: Read) -> Value {
    let mut response = support::response(request);
    if request == Read::Capabilities {
        response["result"]["methods"] = json!([
            "ui.snapshot",
            "runtime.observation",
            "runtime.connection_rows"
        ]);
    }
    response
}

fn render(app: &App, now: Instant, width: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 32)).unwrap();
    terminal.draw(|frame| view::draw(frame, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn private_rows_only_load_on_explicit_connections_page() {
    let mut calls = Vec::new();
    let profiles = load_page(
        &mut |r| {
            calls.push(r);
            Ok(read(r))
        },
        Page::Profiles,
    )
    .unwrap();
    assert!(profiles.connection_rows.is_none());
    assert!(!calls.contains(&Read::ConnectionRows));

    calls.clear();
    let connections = load_page(
        &mut |r| {
            calls.push(r);
            Ok(read(r))
        },
        Page::Connections,
    )
    .unwrap();
    assert_eq!(connections.connection_rows.unwrap().total, 2);
    assert!(calls.contains(&Read::ConnectionRows));
    assert_eq!(Read::ConnectionRows.params(), json!({}));
}

#[test]
fn stale_revision_or_changed_instance_refuses_rows() {
    for field in ["revision", "instanceId"] {
        let result = load_page(
            &mut |r| {
                let mut value = read(r);
                if r == Read::ConnectionRows {
                    if field == "revision" {
                        value["revision"] = json!(8);
                    } else {
                        value["result"]["instanceId"] = json!("foreign");
                    }
                }
                Ok(value)
            },
            Page::Connections,
        );
        assert!(matches!(result, Err(ReadError::Changed)));
    }
}

#[test]
fn changed_owner_drops_previously_rendered_private_rows() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Connections;
    app.accept(load_page(&mut |r| Ok(read(r)), Page::Connections), now);
    assert!(render(&app, now, 70).contains("example.invalid"));

    // The new owner replies to the bracket reads, but a late Connections
    // response still belongs to the previous owner. Refuse the whole refresh
    // and erase the previously displayed private snapshot.
    let changed = load_page(
        &mut |r| {
            let mut value = read(r);
            match r {
                Read::Hello | Read::Snapshot | Read::Observation => {
                    value["result"]["instanceId"] = json!("replacement-runtime");
                }
                _ => {}
            }
            Ok(value)
        },
        Page::Connections,
    );
    assert!(matches!(changed, Err(ReadError::Changed)));
    let later = now + Duration::from_millis(10);
    app.accept(changed, later);
    assert!(app.snapshot.is_none());
    assert!(!render(&app, later, 70).contains("example.invalid"));
}

#[test]
fn malformed_projection_is_unavailable_not_partial() {
    let good = read(Read::ConnectionRows);
    assert!(ConnectionRows::parse(&good).is_some());
    for (path, bad) in [
        ("/result/total", json!(1)),
        ("/result/shown", json!(129)),
        ("/result/rows/0/network", json!("private")),
        ("/result/rows/1/route", json!("unknown")),
        ("/result/truncated", json!(true)),
    ] {
        let mut changed = good.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(ConnectionRows::parse(&changed).is_none(), "{path}");
    }
}

#[test]
fn local_en_ru_narrow_render_never_shows_raw_controller_fields() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(load_page(&mut |r| Ok(read(r)), Page::Connections), now);
        let screen = render(&app, now, 60);
        assert!(screen.contains("example.invalid"));
        assert!(screen.contains("203.0.113.8"));
        assert!(!screen.contains("private-node"));
        assert!(!screen.contains("secret-id"));
    }
}

#[test]
fn leaving_explicit_page_drops_private_destination_snapshot() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Connections;
    app.accept(load_page(&mut |r| Ok(read(r)), Page::Connections), now);
    assert!(app.snapshot.as_ref().unwrap().connection_rows.is_some());
    app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
    let late = load_page(&mut |r| Ok(read(r)), Page::Connections);
    assert!(!app.accept_for(Page::Connections, None, late, now));
    assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
    assert!(!render(&app, now, 70).contains("example.invalid"));
}

#[test]
fn search_filters_only_the_received_private_snapshot() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Connections;
    app.accept(load_page(&mut |r| Ok(read(r)), Page::Connections), now);
    app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for character in "example".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    let screen = render(&app, now, 70);
    assert!(screen.contains("example.invalid"));
    assert!(!screen.contains("192.0.2.2"));
}
