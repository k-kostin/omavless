// SPDX-License-Identifier: MIT
mod support;

use omavless_tui::{
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::{HostSupport, Page, Providers, Rules},
    model::{ReadError, Status},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::Instant;

fn key(app: &mut App, code: KeyCode) -> Action {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn render(app: &App, now: Instant) -> String {
    render_at(app, now, 100, 30)
}

fn render_at(app: &App, now: Instant, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| view::draw(frame, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn operator_reads_are_page_local_capability_gated_and_fenced() {
    for (page, read_kind) in [
        (Page::Host, Read::HostSupport),
        (Page::Rules, Read::Rules),
        (Page::Providers, Read::Providers),
    ] {
        let mut calls = Vec::new();
        let snapshot = load_page(
            &mut |read| {
                calls.push(read);
                Ok(support::response(read))
            },
            page,
        )
        .unwrap();
        assert_eq!(
            calls,
            [
                Read::Hello,
                Read::Capabilities,
                Read::Snapshot,
                read_kind,
                Read::Observation
            ]
        );
        assert_eq!(snapshot.status(), Status::Connected);
        assert_eq!(read_kind.params(), json!({}));
        let without_capability = load_page(
            &mut |read| {
                let mut result = support::response(read);
                if read == Read::Capabilities {
                    result["result"]["methods"] = json!(["ui.snapshot", "runtime.observation"]);
                }
                if read == read_kind {
                    panic!("unadvertised operator method was called");
                }
                Ok(result)
            },
            page,
        )
        .unwrap();
        assert!(without_capability.rules.is_none());
        assert!(without_capability.providers.is_none());
        assert!(without_capability.host_support.is_none());
        assert!(matches!(
            load_page(
                &mut |read| {
                    let mut result = support::response(read);
                    if read == read_kind {
                        result["revision"] = json!(8);
                    }
                    Ok(result)
                },
                page,
            ),
            Err(ReadError::Changed)
        ));
    }
}

#[test]
fn projections_reject_malformed_counts_rows_and_statuses() {
    let rules = support::response(Read::Rules);
    assert_eq!(Rules::parse(&rules).unwrap().items.len(), 3);
    for (path, bad) in [
        ("/result/rules/total", json!(65537)),
        ("/result/rules/shown", json!(2)),
        ("/result/rules/truncated", json!(true)),
        ("/result/rules/items/0/target", json!("private-chain")),
        ("/result/rules/items/0/payload", json!("x".repeat(513))),
    ] {
        let mut changed = rules.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(Rules::parse(&changed).is_none(), "{path}");
    }
    let providers = support::response(Read::Providers);
    assert_eq!(Providers::parse(&providers).unwrap().items.len(), 2);
    for (path, bad) in [
        ("/result/providers/total", json!(257)),
        ("/result/providers/shown", json!(1)),
        ("/result/providers/items/0/ruleCount", json!(-2)),
        ("/result/providers/items/0/status", json!("unknown")),
        ("/result/providers/items/0/refreshable", json!("yes")),
    ] {
        let mut changed = providers.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(Providers::parse(&changed).is_none(), "{path}");
    }
}

#[test]
fn host_report_is_typed_unknown_is_not_no_and_no_raw_report_is_retained() {
    let report = support::response(Read::HostSupport);
    let parsed = HostSupport::parse(&report).unwrap();
    assert_eq!(parsed.core_installed, Some(true));
    assert_eq!(parsed.runtime_unit_enabled, Some(false));
    for (path, bad) in [
        ("/result/schemaVersion", json!(2)),
        ("/result/coverage/coreSetupVerified", json!(false)),
        ("/result/host/core/fileNetworkCapabilities", json!("yes")),
        ("/result/host/files/store", json!("private://secret")),
        (
            "/result/host/runtimeService/ownsCurrentProcess",
            json!("true"),
        ),
    ] {
        let mut changed = report.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(HostSupport::parse(&changed).is_none(), "{path}");
    }
    let mut unknown = report.clone();
    unknown["result"]["host"]["core"] = json!(null);
    unknown["result"]["coverage"]["coreSetupVerified"] = json!(false);
    let parsed = HostSupport::parse(&unknown).unwrap();
    assert_eq!(parsed.core_installed, None);
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Host;
    app.accept(
        load_page(
            &mut |read| {
                if read == Read::HostSupport {
                    Ok(unknown.clone())
                } else {
                    Ok(support::response(read))
                }
            },
            Page::Host,
        ),
        now,
    );
    let screen = render(&app, now);
    assert!(screen.contains("Mihomo executable present: Unavailable"));
    assert!(!screen.contains("Mihomo executable present: No"));
}

#[test]
fn local_filter_never_mutates_connection_and_never_claims_global_search() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.actions_enabled = true;
    app.page = Page::Diagnostics;
    assert_eq!(key(&mut app, KeyCode::Char('R')), Action::Refresh);
    assert!(app.page == Page::Rules);
    app.accept(
        load_page(&mut |read| Ok(support::response(read)), Page::Rules),
        now,
    );
    let initial = render(&app, now);
    assert!(initial.contains("example.invalid"));
    assert!(initial.contains("192.0.2.0/24"));
    assert!(initial.contains("Total in core: 3"));
    assert_eq!(key(&mut app, KeyCode::Char('/')), Action::None);
    for character in "DIRECT".chars() {
        assert_eq!(key(&mut app, KeyCode::Char(character)), Action::None);
    }
    assert_eq!(key(&mut app, KeyCode::Enter), Action::None);
    let filtered = render(&app, now);
    assert!(filtered.contains("192.0.2.0/24"));
    assert!(!filtered.contains("example.invalid"));
    assert!(filtered.contains("Matching loaded rows: 1"));
    for code in [KeyCode::Char('1'), KeyCode::Char('d'), KeyCode::Char('s')] {
        assert_eq!(key(&mut app, code), Action::None);
    }
    assert!(app.pending.is_none());
    assert!(app.confirmation.is_none());
    assert_eq!(key(&mut app, KeyCode::Char('c')), Action::None);
    assert!(app.operator_query.is_empty());
    assert!(render(&app, now).contains("example.invalid"));
    assert_eq!(key(&mut app, KeyCode::Esc), Action::Refresh);
    assert!(app.page == Page::Diagnostics);
    assert_eq!(key(&mut app, KeyCode::Char('H')), Action::Refresh);
    assert!(app.page == Page::Host);
    app.accept(
        load_page(&mut |read| Ok(support::response(read)), Page::Host),
        now,
    );
    assert!(render(&app, now).contains("Mihomo executable present: Yes"));
    assert_eq!(key(&mut app, KeyCode::Esc), Action::Refresh);
    assert!(app.page == Page::Diagnostics);
    assert_eq!(key(&mut app, KeyCode::Char('P')), Action::Refresh);
    assert!(app.page == Page::Providers);
    assert!(render(&app, now + FRESH_FOR).contains("State is stale"));
    assert!(!render(&app, now + FRESH_FOR).contains("example.invalid"));

    app.page = Page::Providers;
    app.accept(
        load_page(&mut |read| Ok(support::response(read)), Page::Providers),
        now,
    );
    let providers = render(&app, now);
    assert!(providers.contains("fixture-domains"));
    assert!(providers.contains("Core update time"));
    assert!(providers.contains("Core supports refresh: Yes"));
}

#[test]
fn backend_truncation_and_failures_are_explicit_without_raw_error_echo() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.page = Page::Rules;
        app.accept(
            load_page(
                &mut |read| {
                    let mut value = support::response(read);
                    if read == Read::Rules {
                        value["result"]["rules"]["total"] = json!(9);
                        value["result"]["rules"]["truncated"] = json!(true);
                    }
                    Ok(value)
                },
                Page::Rules,
            ),
            now,
        );
        let screen = render(&app, now);
        assert!(screen.contains(locale.text("tui.backend_truncated")));
        assert!(screen.contains(locale.text("tui.loaded_total")));
        app.accept(
            load_page(
                &mut |read| {
                    if read == Read::Rules {
                        return Ok(json!({"ok":false,"error":{"message":"private://secret"}}));
                    }
                    Ok(support::response(read))
                },
                Page::Rules,
            ),
            now,
        );
        assert_eq!(app.status(now), Status::Connected);
        let screen = render(&app, now);
        assert!(screen.contains(locale.text("tui.metric_unavailable")));
        assert!(!screen.contains("private://secret"));
    }
}

#[test]
fn operator_pages_render_safely_in_both_languages_at_small_sizes() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        for page in [Page::Host, Page::Rules, Page::Providers] {
            let mut app = App::new(locale);
            app.page = page;
            app.accept(
                load_page(&mut |read| Ok(support::response(read)), page),
                now,
            );
            let wide = render_at(&app, now, 70, 24);
            assert!(wide.contains(locale.text(page.key())));
            assert!(!wide.contains("private://"));
            for (width, height) in [(40, 12), (24, 8)] {
                let small = render_at(&app, now, width, height);
                assert!(!small.is_empty());
                assert!(!small.contains("private://"));
            }
            assert!(app.pending.is_none());
        }
    }
}
