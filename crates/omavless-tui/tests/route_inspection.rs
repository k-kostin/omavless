// SPDX-License-Identifier: MIT
mod support;

use omavless_tui::{
    app::{Action, App},
    client::{Read, load_page, load_page_for_route},
    i18n::Locale,
    inspection::Page,
    route_inspection::{Outcome, Result as RouteResult, Source, Status, Target},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::Instant;

fn key(app: &mut App, code: KeyCode, at: Instant) -> Action {
    app.key_at(KeyEvent::new(code, KeyModifiers::NONE), at)
}

fn render(app: &App, at: Instant) -> String {
    render_size(app, at, 100, 30)
}

fn render_size(app: &App, at: Instant, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view::draw(f, app, at)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn target_is_canonical_bounded_and_never_debug_prints_private_query() {
    let target = Target::new(" EXAMPLE.invalid. ").unwrap();
    assert_eq!(target.as_str(), "example.invalid");
    assert!(!format!("{target:?}").contains("example.invalid"));
    for bad in [
        "https://example.invalid/path",
        "a b.invalid",
        "fe80::1%eth0",
        "\x1b[31m",
    ] {
        assert!(Target::new(bad).is_none());
    }
}

#[test]
fn parser_accepts_only_exact_query_revision_and_consistent_bounded_outcome() {
    let target = Target::new("example.invalid").unwrap();
    let response = support::response(Read::RouteCheck(target));
    let parsed = RouteResult::parse(&response, target, 7).unwrap();
    assert!(matches!(parsed.outcome, Outcome::Vpn));
    assert!(matches!(parsed.source, Source::Custom));
    assert_eq!(parsed.rule_payload, "example.invalid");
    assert!(RouteResult::parse(&response, target, 8).is_none());
    for (pointer, value) in [
        ("/result/query", json!("other.invalid")),
        ("/result/source", json!("untrusted")),
        ("/result/outcome", json!("unknown")),
        ("/result/rulePayload", json!("\x1b[31m")),
        ("/result/target", json!("DIRECT")),
    ] {
        let mut corrupted = response.clone();
        *corrupted.pointer_mut(pointer).unwrap() = value;
        assert!(RouteResult::parse(&corrupted, target, 7).is_none());
    }
}

#[test]
fn one_shot_client_checks_only_when_requested_and_fences_the_owner() {
    let target = Target::new("example.invalid").unwrap();
    let mut reads = Vec::new();
    let (snapshot, route) = load_page_for_route(
        &mut |request| {
            reads.push(request);
            let mut response = support::response(request);
            if request == Read::Capabilities {
                response["result"]["methods"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("routing.check"));
            }
            Ok(response)
        },
        Page::RouteCheck,
        None,
        Some(target),
    )
    .unwrap();
    assert_eq!(snapshot.revision, 7);
    assert!(reads.contains(&Read::RouteCheck(target)));
    assert!(matches!(route, Some(Status::Observed(_))));

    reads.clear();
    let (_, route) = load_page_for_route(
        &mut |request| {
            reads.push(request);
            Ok(support::response(request))
        },
        Page::RouteCheck,
        None,
        None,
    )
    .unwrap();
    assert!(route.is_none());
    assert!(
        !reads
            .iter()
            .any(|request| matches!(request, Read::RouteCheck(_)))
    );

    let (_, route) = load_page_for_route(
        &mut |request| Ok(support::response(request)),
        Page::RouteCheck,
        None,
        Some(target),
    )
    .unwrap();
    assert!(matches!(route, Some(Status::Unsupported)));
}

#[test]
fn input_result_and_navigation_are_private_and_do_not_mutate_runtime() {
    let at = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.page = Page::RouteCheck;
        app.accept(
            Ok(load_page(&mut |read| Ok(support::response(read)), Page::RouteCheck).unwrap()),
            at,
        );
        assert_eq!(key(&mut app, KeyCode::Char('/'), at), Action::None);
        for c in "example.invalid".chars() {
            assert_eq!(key(&mut app, KeyCode::Char(c), at), Action::None);
        }
        assert_eq!(key(&mut app, KeyCode::Enter, at), Action::Refresh);
        let request = app.route_request.take().unwrap();
        assert_eq!(request.as_str(), "example.invalid");
        assert!(render(&app, at).contains("example.invalid"));
        let result =
            RouteResult::parse(&support::response(Read::RouteCheck(request)), request, 7).unwrap();
        app.accept_route(Some(request), Some(Status::Observed(result)), at);
        assert!(matches!(app.route_result, Some(Status::Observed(_))));
        assert!(!render(&app, at).contains("Missing translation"));
        assert!(render_size(&app, at, 70, 24).contains("example.invalid"));
        let later = Target::new("later.invalid").unwrap();
        app.accept_route(Some(later), Some(Status::Unavailable), at);
        assert!(matches!(app.route_result, Some(Status::Observed(_))));
        assert_eq!(key(&mut app, KeyCode::Char('/'), at), Action::None);
        assert!(app.route_result.is_none());
        app.accept_route(Some(request), Some(Status::Unavailable), at);
        assert!(app.route_result.is_none());
        assert_eq!(key(&mut app, KeyCode::Esc, at), Action::None);
        assert_eq!(key(&mut app, KeyCode::Esc, at), Action::Refresh);
        assert!(app.page == Page::Diagnostics);
        assert!(app.route_query.is_empty());
        assert!(app.route_result.is_none());
        assert!(app.route_request.is_none());
        assert!(!render(&app, at).contains("example.invalid"));
    }
}

#[test]
fn changed_revision_or_failed_read_clears_old_route_evidence() {
    let at = Instant::now();
    let target = Target::new("example.invalid").unwrap();
    let mut app = App::new(Locale::En);
    app.page = Page::RouteCheck;
    app.accept(
        Ok(load_page(&mut |read| Ok(support::response(read)), Page::RouteCheck).unwrap()),
        at,
    );
    app.route_query = target.as_str().to_owned();
    assert_eq!(key(&mut app, KeyCode::Char('/'), at), Action::None);
    assert_eq!(key(&mut app, KeyCode::Enter, at), Action::Refresh);
    let result =
        RouteResult::parse(&support::response(Read::RouteCheck(target)), target, 7).unwrap();
    app.accept_route(Some(target), Some(Status::Observed(result)), at);
    assert!(app.route_result.is_some());
    let changed = load_page(
        &mut |read| {
            let mut response = support::response(read);
            response["revision"] = json!(8);
            Ok(response)
        },
        Page::RouteCheck,
    )
    .unwrap();
    app.accept(Ok(changed), at);
    assert!(app.route_result.is_none());
    app.accept_route(Some(target), Some(Status::Unavailable), at);
    // Even a response stamped with the same synthetic Instant cannot attach
    // after the owner revision moved.
    assert!(app.route_result.is_none());
}
