// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    connection_browsing::{Order, rows},
    i18n::Locale,
    inspection::{ConnectionRows, Page},
    model::ReadError,
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::{Value, json};
use std::time::Instant;

fn response(read: Read) -> Value {
    let mut value = support::response(read);
    if read == Read::Capabilities {
        value["result"]["methods"] = json!([
            "ui.snapshot",
            "runtime.observation",
            "runtime.connection_rows"
        ]);
    }
    if read == Read::ConnectionRows {
        value["result"]["total"] = json!(2);
        value["result"]["shown"] = json!(2);
        value["result"]["truncated"] = json!(false);
        value["result"]["rows"] = json!([
            {"host":"zeta.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
            {"host":"alpha.invalid","ip":null,"port":53,"network":"udp","route":"direct"}
        ]);
    }
    value
}
fn sample(locale: Locale) -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(locale);
    app.page = Page::Connections;
    app.accept(load_page(&mut |r| Ok(response(r)), Page::Connections), now);
    (app, now)
}
fn render(app: &App, now: Instant) -> String {
    let mut terminal = Terminal::new(TestBackend::new(70, 32)).unwrap();
    terminal.draw(|f| view::draw(f, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn order_shortcut_sorts_the_received_rows_without_requesting_refresh_or_action() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        app.actions_enabled = true;
        app.snapshot.as_mut().unwrap().actions_available = true;
        let before = render(&app, now);
        assert!(before.find("zeta.invalid").unwrap() < before.find("alpha.invalid").unwrap());
        let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
        let revision = app.snapshot.as_ref().unwrap().revision;
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now),
            Action::None
        );
        let sorted = render(&app, now);
        assert!(sorted.find("alpha.invalid").unwrap() < sorted.find("zeta.invalid").unwrap());
        assert!(app.pending.is_none() && app.confirmation.is_none());
        assert!(app.snapshot.as_ref().unwrap().metadata.desired == desired);
        assert_eq!(app.snapshot.as_ref().unwrap().revision, revision);
        assert_eq!(
            app.snapshot
                .as_ref()
                .unwrap()
                .connection_rows
                .as_ref()
                .unwrap()
                .rows[0]
                .host
                .as_deref(),
            Some("zeta.invalid")
        );
    }
}

#[test]
fn source_order_is_unchanged_and_the_four_choices_cycle_in_this_window_only() {
    let (mut app, now) = sample(Locale::En);
    assert_eq!(app.connection_order, Order::Original);
    for choice in [
        Order::Destination,
        Order::Network,
        Order::Route,
        Order::Original,
    ] {
        app.inspection_scroll = 12;
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now),
            Action::None
        );
        assert_eq!(app.connection_order, choice);
        assert_eq!(app.inspection_scroll, 0);
    }
    assert!(
        render(&app, now).find("zeta.invalid").unwrap()
            < render(&app, now).find("alpha.invalid").unwrap()
    );
    assert_eq!(App::new(Locale::En).connection_order, Order::Original);
}

#[test]
fn port_ties_missing_destinations_and_case_folding_have_a_stable_order() {
    let mut value = response(Read::ConnectionRows);
    value["result"]["total"] = json!(5);
    value["result"]["shown"] = json!(5);
    value["result"]["rows"] = json!([
        {"host":null,"ip":null,"port":null,"network":"other","route":"unclassified"},
        {"host":"ALPHA.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
        {"host":"alpha.invalid","ip":null,"port":53,"network":"udp","route":"direct"},
        {"host":"alpha.invalid","ip":null,"port":53,"network":"tcp","route":"vpn"},
        {"host":"alpha.invalid","ip":null,"port":null,"network":"tcp","route":"blocked"}
    ]);
    let projection = ConnectionRows::parse(&value).unwrap();
    let sorted = rows(&projection, "", Order::Destination);
    for (shown, source) in sorted.iter().zip([2, 3, 1, 4, 0]) {
        assert!(std::ptr::eq(*shown, &projection.rows[source]));
    }
    let by_network = rows(&projection, "", Order::Network);
    assert_eq!(
        by_network.iter().map(|r| r.network).collect::<Vec<_>>(),
        ["tcp", "tcp", "tcp", "udp", "other"]
    );
    let by_route = rows(&projection, "", Order::Route);
    assert_eq!(
        by_route.iter().map(|r| r.route).collect::<Vec<_>>(),
        ["vpn", "vpn", "direct", "blocked", "unclassified"]
    );
    // The comparator never changes displayed casing or the canonical row order.
    assert_eq!(projection.rows[1].host.as_deref(), Some("ALPHA.invalid"));
    assert!(projection.rows[0].host.is_none());
}

#[test]
fn filtering_stays_local_and_ordering_does_not_hide_partial_coverage() {
    let (mut app, now) = sample(Locale::Ru);
    let projection = app
        .snapshot
        .as_mut()
        .unwrap()
        .connection_rows
        .as_mut()
        .unwrap();
    projection.total = 200;
    projection.truncated = true;
    for order in [
        Order::Original,
        Order::Destination,
        Order::Network,
        Order::Route,
    ] {
        let filtered = rows(projection, "ALPHA", order);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].host.as_deref(), Some("alpha.invalid"));
        assert_eq!(rows(projection, "udp", order).len(), 1);
        assert_eq!(rows(projection, "direct", order).len(), 1);
        assert_eq!(rows(projection, "443", order).len(), 1);
        assert!(rows(projection, "absent-fixture", order).is_empty());
    }
    app.connection_order = Order::Route;
    let screen = render(&app, now);
    assert!(screen.contains("200"));
    assert!(screen.contains(Locale::Ru.text("tui.connection_rows_truncated")));
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .connection_rows
            .as_ref()
            .unwrap()
            .rows
            .len(),
        2
    );
}

#[test]
fn search_modifier_repeat_modal_and_other_pages_keep_their_input_semantics() {
    use ratatui::crossterm::event::KeyEventKind;
    let (mut app, now) = sample(Locale::En);
    app.key_at(
        KeyEvent::new_with_kind(KeyCode::Char('o'), KeyModifiers::NONE, KeyEventKind::Repeat),
        now,
    );
    app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::ALT), now);
    assert_eq!(app.connection_order, Order::Original);
    app.help = true;
    app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
    assert_eq!(app.connection_order, Order::Original);
    app.help = false;
    app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
    app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
    assert_eq!(app.operator_query, "o");
    assert_eq!(app.connection_order, Order::Original);
    app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
    app.page = Page::Rules;
    app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
    assert_eq!(app.connection_order, Order::Original);
    app.page = Page::Connections;
    app.actions_enabled = true;
    app.snapshot.as_mut().unwrap().actions_available = true;
    app.prepare(omavless_tui::actions::Kind::Disconnect, None, now);
    assert!(app.confirmation.is_some());
    app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
    assert_eq!(app.connection_order, Order::Original);
    assert!(app.confirmation.is_some() && app.pending.is_none());
    app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
    assert!(app.pending.is_none() && app.confirmation.is_none());
}

#[test]
fn selected_order_never_retains_rows_after_stale_error_or_page_leave() {
    let (mut app, now) = sample(Locale::En);
    app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
    assert!(render(&app, now).contains("alpha.invalid"));
    assert!(!render(&app, now + FRESH_FOR).contains("alpha.invalid"));
    app.accept(Err(ReadError::Changed), now);
    assert!(app.snapshot.is_none());
    assert!(!render(&app, now).contains("alpha.invalid"));
    app.accept(load_page(&mut |r| Ok(response(r)), Page::Connections), now);
    app.key_at(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now);
    assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
    assert!(!app.accept_for(
        Page::Connections,
        None,
        load_page(&mut |r| Ok(response(r)), Page::Connections),
        now
    ));
    assert!(!render(&app, now).contains("alpha.invalid"));
}

#[test]
fn max_projection_unknown_ip_empty_and_no_match_stay_bounded() {
    let mut value = response(Read::ConnectionRows);
    value["result"]["total"] = json!(4096);
    value["result"]["shown"] = json!(128);
    value["result"]["truncated"] = json!(true);
    value["result"]["rows"] = Value::Array(
        (0..128)
            .rev()
            .map(|i| {
                json!({
                    "host":format!("fixture-{i:03}.invalid"),"ip":null,"port":443,
                    "network":"tcp","route":"vpn"
                })
            })
            .collect(),
    );
    let projection = ConnectionRows::parse(&value).unwrap();
    for order in [
        Order::Original,
        Order::Destination,
        Order::Network,
        Order::Route,
    ] {
        let sorted = rows(&projection, "", order);
        assert_eq!(sorted.len(), 128);
        assert!(sorted.iter().all(|r| {
            projection
                .rows
                .iter()
                .any(|original| std::ptr::eq(*r, original))
        }));
        assert_eq!(rows(&projection, "fixture-000", order).len(), 1);
        assert!(rows(&projection, "no-synthetic-match", order).is_empty());
        assert_eq!(projection.total, 4096);
        assert!(projection.truncated);
    }
    assert_eq!(
        projection.rows[0].host.as_deref(),
        Some("fixture-127.invalid")
    );
    value["result"]["rows"] =
        json!([{ "host":null,"ip":"2001:db8::2","port":53,"network":"udp","route":"direct" }]);
    value["result"]["total"] = json!(1);
    value["result"]["shown"] = json!(1);
    value["result"]["truncated"] = json!(false);
    let projection = ConnectionRows::parse(&value).unwrap();
    assert_eq!(rows(&projection, "2001:DB8", Order::Destination).len(), 1);
    value["result"]["rows"] = json!([]);
    value["result"]["total"] = json!(0);
    value["result"]["shown"] = json!(0);
    let empty = ConnectionRows::parse(&value).unwrap();
    assert!(rows(&empty, "", Order::Destination).is_empty());
}
