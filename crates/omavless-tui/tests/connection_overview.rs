// SPDX-License-Identifier: MIT
mod support;

use omavless_tui::{
    app::App,
    client::{Read, load_page},
    i18n::Locale,
    inspection::{ConnectionOverview, Page},
    model::ReadError,
    view,
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::{Value, json};
use std::time::Instant;

fn capabilities(methods: &[&str]) -> Value {
    json!({"ok":true,"revision":7,"result":{"runtimeOwnership":true,"methods":methods}})
}

fn read(read: Read, methods: &[&str]) -> Value {
    if read == Read::Capabilities {
        capabilities(methods)
    } else {
        support::response(read)
    }
}

fn render(app: &App, now: Instant) -> String {
    let mut terminal = Terminal::new(TestBackend::new(95, 30)).unwrap();
    terminal.draw(|frame| view::draw(frame, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

#[test]
fn strict_projection_rejects_partial_unknown_and_inconsistent_counts() {
    let good = support::response(Read::ConnectionOverview);
    let parsed = ConnectionOverview::parse(&good).unwrap();
    assert_eq!(parsed.total, 3);
    for (path, bad) in [
        ("/result/total", json!(4097)),
        ("/result/network/other", json!(1)),
        ("/result/outcome/vpn", json!(3)),
        ("/result/outcome/direct", json!(-1)),
        ("/result/availability", json!("unavailable")),
        ("/result/scope", json!("raw_connections")),
    ] {
        let mut changed = good.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(ConnectionOverview::parse(&changed).is_none(), "{path}");
    }
}

#[test]
fn new_read_replaces_duplicate_count_query_and_keeps_old_runtime_compatible() {
    let common = ["ui.snapshot", "runtime.observation", "runtime.traffic"];
    let methods = [
        common[0],
        common[1],
        common[2],
        "runtime.connections",
        "runtime.connection_overview",
    ];
    let mut calls = Vec::new();
    let snapshot = load_page(
        &mut |r| {
            calls.push(r);
            Ok(read(r, &methods))
        },
        Page::Traffic,
    )
    .unwrap();
    assert_eq!(
        calls,
        [
            Read::Hello,
            Read::Capabilities,
            Read::Snapshot,
            Read::Traffic,
            Read::ConnectionOverview,
            Read::Observation
        ]
    );
    assert_eq!(snapshot.active_connections, Some(3));
    assert!(snapshot.connection_overview.is_some());
    assert_eq!(Read::ConnectionOverview.params(), json!({}));

    let old = [common[0], common[1], common[2], "runtime.connections"];
    calls.clear();
    let snapshot = load_page(
        &mut |r| {
            calls.push(r);
            Ok(read(r, &old))
        },
        Page::Traffic,
    )
    .unwrap();
    assert!(calls.contains(&Read::Connections));
    assert!(!calls.contains(&Read::ConnectionOverview));
    assert_eq!(snapshot.active_connections, Some(3));
    assert!(snapshot.connection_overview.is_none());
    assert!(matches!(
        load_page(
            &mut |r| {
                let mut value = read(r, &methods);
                if r == Read::ConnectionOverview {
                    value["revision"] = json!(8);
                }
                Ok(value)
            },
            Page::Traffic
        ),
        Err(ReadError::Changed)
    ));
}

#[test]
fn bad_overview_is_unavailable_not_zero_or_private_and_rendered_in_both_languages() {
    let methods = [
        "ui.snapshot",
        "runtime.observation",
        "runtime.traffic",
        "runtime.connection_overview",
    ];
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.page = Page::Traffic;
        app.accept(
            load_page(&mut |r| Ok(read(r, &methods)), Page::Traffic),
            now,
        );
        let screen = render(&app, now);
        assert!(screen.contains(locale.text("tui.connection_categories_scope")));
        assert!(screen.contains("DIRECT 1"));
        assert!(screen.contains("PROXY 2"));
        assert!(!screen.contains("private://"));
        app.accept(
            load_page(
                &mut |r| {
                    let mut value = read(r, &methods);
                    if r == Read::ConnectionOverview {
                        value["result"]["network"]["tcp"] = json!("private://secret");
                    }
                    Ok(value)
                },
                Page::Traffic,
            ),
            now,
        );
        assert!(app.snapshot.as_ref().unwrap().active_connections.is_none());
        let screen = render(&app, now);
        assert!(!screen.contains("private://secret"));
        assert!(!screen.contains(locale.text("tui.connection_categories_scope")));
    }
}
