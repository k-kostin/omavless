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
    render_size(app, now, 70, 32)
}
fn render_size(app: &App, now: Instant, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
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
fn connection_route_filter_selects_categories_not_destination_substrings() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        app.accept(
            load_page(&mut |read| {
                let mut value = response(read);
                if read == Read::ConnectionRows {
                    value["result"]["total"] = json!(4);
                    value["result"]["shown"] = json!(4);
                    value["result"]["rows"] = json!([
                        {"host":"blocked.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
                        {"host":"direct.invalid","ip":null,"port":443,"network":"tcp","route":"blocked"},
                        {"host":"alpha.invalid","ip":null,"port":53,"network":"udp","route":"direct"},
                        {"host":"unknown.invalid","ip":null,"port":null,"network":"other","route":"unclassified"}
                    ]);
                }
                Ok(value)
            }, Page::Connections), now);
        // All -> Direct. This chooses a route category, not text matching.
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now),
            Action::None
        );
        let direct = render(&app, now);
        assert!(direct.contains("alpha.invalid"));
        assert!(
            !direct.contains("blocked.invalid") && !direct.contains("direct.invalid"),
            "Direct filter must select route categories, not names or all rows"
        );
        // Direct -> Blocked; its destination deliberately says direct.
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        let blocked = render(&app, now);
        assert!(blocked.contains("direct.invalid"));
        assert!(!blocked.contains("blocked.invalid") && !blocked.contains("alpha.invalid"));
        // Blocked -> Proxy chain -> Unclassified -> All.
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        let proxy = render(&app, now);
        assert!(proxy.contains("blocked.invalid") && !proxy.contains("direct.invalid"));
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        let unknown = render(&app, now);
        assert!(unknown.contains("unknown.invalid"));
        assert!(
            !unknown.contains("blocked.invalid")
                && !unknown.contains("direct.invalid")
                && !unknown.contains("alpha.invalid")
        );
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        let all = render(&app, now);
        assert!(
            all.contains("blocked.invalid")
                && all.contains("direct.invalid")
                && all.contains("alpha.invalid")
        );
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn connection_route_and_type_filters_intersect_search_and_clear_without_changing_snapshot() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
        let revision = app.snapshot.as_ref().unwrap().revision;
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
        let none = render(&app, now);
        assert!(none.contains(locale.text("tui.no_operator_matches")));
        assert!(!none.contains(locale.text("tui.connection_rows_empty")));
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for ch in "alpha".chars() {
            app.key_at(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), now);
        }
        app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        let one = render(&app, now);
        assert!(one.contains("alpha.invalid") && !one.contains("zeta.invalid"));
        app.inspection_scroll = 12;
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now),
            Action::None
        );
        assert_eq!(app.inspection_scroll, 0);
        assert!(app.operator_query.is_empty());
        assert_eq!(app.connection_order, Order::Destination);
        let all = render(&app, now);
        assert!(all.contains("alpha.invalid") && all.contains("zeta.invalid"));
        let snapshot = app.snapshot.as_ref().unwrap();
        assert!(snapshot.metadata.desired == desired);
        assert_eq!(snapshot.revision, revision);
        assert_eq!(snapshot.connection_rows.as_ref().unwrap().rows.len(), 2);
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn route_category_preference_survives_refresh_not_private_rows_and_respects_input_owners() {
    use ratatui::crossterm::event::KeyEventKind;
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        for key in [
            KeyEvent::new_with_kind(KeyCode::Char('v'), KeyModifiers::NONE, KeyEventKind::Repeat),
            KeyEvent::new_with_kind(
                KeyCode::Char('v'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ),
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(app.key_at(key, now), Action::None);
        }
        app.help = true;
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        app.help = false;
        app.page = Page::Rules;
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        app.page = Page::Connections;
        let all = render(&app, now);
        assert!(all.contains("alpha.invalid") && all.contains("zeta.invalid"));
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        assert_eq!(app.operator_query, "v");
        app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now);
        app.actions_enabled = true;
        app.snapshot.as_mut().unwrap().actions_available = true;
        app.prepare(omavless_tui::actions::Kind::Disconnect, None, now);
        assert!(app.confirmation.is_some());
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        assert!(app.confirmation.is_some() && app.pending.is_none());
        app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
        let all = render(&app, now);
        assert!(all.contains("alpha.invalid") && all.contains("zeta.invalid"));
        app.key_at(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE), now);
        app.snapshot
            .as_mut()
            .unwrap()
            .connection_rows
            .as_mut()
            .unwrap()
            .truncated = true;
        app.snapshot
            .as_mut()
            .unwrap()
            .connection_rows
            .as_mut()
            .unwrap()
            .total = 200;
        // Action mode keeps its existing 70-column minimum, unlike the
        // ordinary read-only view. Do not mistake that barrier for this page.
        assert!(!render_size(&app, now, 60, 24).contains("alpha.invalid"));
        app.actions_enabled = false;
        app.snapshot.as_mut().unwrap().actions_available = false;
        for width in [60, 70] {
            let screen = render_size(&app, now, width, 24);
            assert!(
                screen.contains(locale.text("tui.connection_filter_route")),
                "{locale:?}/{width}: {screen}"
            );
            assert!(screen.contains(locale.text("tui.connection_route_filter_keys")));
            assert!(screen.contains(locale.text("tui.connection_rows_truncated")));
            assert!(screen.contains("alpha.invalid") && !screen.contains("zeta.invalid"));
            assert!(screen.contains("q:") && screen.contains("r:"));
        }
        assert!(!render(&app, now + FRESH_FOR).contains("alpha.invalid"));
        app.accept(Err(ReadError::Changed), now);
        assert!(app.snapshot.is_none());
        app.accept(load_page(&mut |r| Ok(response(r)), Page::Connections), now);
        assert!(
            render(&app, now).contains("alpha.invalid")
                && !render(&app, now).contains("zeta.invalid")
        );
        app.key_at(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now);
        assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
        assert!(!app.accept_for(
            Page::Connections,
            None,
            load_page(&mut |r| Ok(response(r)), Page::Connections),
            now
        ));
        let (fresh, at) = sample(locale);
        let all = render(&fresh, at);
        assert!(all.contains("alpha.invalid") && all.contains("zeta.invalid"));
    }
}

#[test]
fn connection_type_filter_cycles_locally_without_changing_the_received_snapshot() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
        let revision = app.snapshot.as_ref().unwrap().revision;
        assert!(render(&app, now).contains("alpha.invalid"));
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now),
            Action::None
        );
        let tcp = render(&app, now);
        assert!(tcp.contains("zeta.invalid"));
        assert!(
            !tcp.contains("alpha.invalid"),
            "TCP filter must hide UDP rows"
        );
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        let udp = render(&app, now);
        assert!(udp.contains("alpha.invalid") && !udp.contains("zeta.invalid"));
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        let other = render(&app, now);
        assert!(!other.contains("alpha.invalid") && !other.contains("zeta.invalid"));
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        let all = render(&app, now);
        assert!(all.contains("alpha.invalid") && all.contains("zeta.invalid"));
        assert!(app.pending.is_none() && app.confirmation.is_none());
        let snapshot = app.snapshot.as_ref().unwrap();
        assert!(snapshot.metadata.desired == desired);
        assert_eq!(snapshot.revision, revision);
        let original = snapshot.connection_rows.as_ref().unwrap();
        assert_eq!(original.rows.len(), 2);
        assert_eq!(original.rows[0].host.as_deref(), Some("zeta.invalid"));
    }
}

#[test]
fn connection_type_filter_intersects_search_and_order_and_clear_resets_both_filters() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for ch in "alpha".chars() {
            app.key_at(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), now);
        }
        app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        let no_match = render(&app, now);
        assert!(!no_match.contains("alpha.invalid") && !no_match.contains("zeta.invalid"));
        assert!(no_match.contains(locale.text("tui.no_operator_matches")));
        assert!(!no_match.contains(locale.text("tui.connection_rows_empty")));
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
        let udp = render(&app, now);
        assert!(udp.contains("alpha.invalid") && !udp.contains("zeta.invalid"));
        assert!(udp.contains(locale.text("tui.connection_filter_udp")));
        assert!(udp.contains(locale.text("tui.matching_rows")));
        app.inspection_scroll = 12;
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now),
            Action::None
        );
        assert_eq!(app.inspection_scroll, 0);
        assert!(app.operator_query.is_empty());
        let all = render(&app, now);
        assert!(all.contains("alpha.invalid") && all.contains("zeta.invalid"));
        assert!(all.contains(locale.text("tui.connection_filter_all")));
        assert_eq!(app.connection_order, Order::Destination);
    }
}

#[test]
fn connection_type_shortcut_respects_search_modifiers_repeat_help_and_confirmation() {
    use ratatui::crossterm::event::KeyEventKind;
    let (mut app, now) = sample(Locale::En);
    for key in [
        KeyEvent::new_with_kind(KeyCode::Char('n'), KeyModifiers::NONE, KeyEventKind::Repeat),
        KeyEvent::new_with_kind(
            KeyCode::Char('n'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
    ] {
        assert_eq!(app.key_at(key, now), Action::None);
        assert!(render(&app, now).contains("alpha.invalid"));
    }
    app.help = true;
    app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
    app.help = false;
    assert!(render(&app, now).contains("alpha.invalid"));
    app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
    app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
    assert_eq!(app.operator_query, "n");
    app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
    app.key_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now);
    assert!(render(&app, now).contains("alpha.invalid"));
    app.page = Page::Rules;
    app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
    app.page = Page::Connections;
    assert!(render(&app, now).contains("alpha.invalid"));
    app.actions_enabled = true;
    app.snapshot.as_mut().unwrap().actions_available = true;
    app.prepare(omavless_tui::actions::Kind::Disconnect, None, now);
    assert!(app.confirmation.is_some());
    app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
    assert!(app.confirmation.is_some() && app.pending.is_none());
    app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
    assert!(render(&app, now).contains("alpha.invalid"));
}

#[test]
fn connection_type_filter_retains_partial_coverage_but_never_stale_rows() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = sample(locale);
        app.snapshot
            .as_mut()
            .unwrap()
            .connection_rows
            .as_mut()
            .unwrap()
            .truncated = true;
        app.snapshot
            .as_mut()
            .unwrap()
            .connection_rows
            .as_mut()
            .unwrap()
            .total = 200;
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        for width in [60, 70] {
            let screen = render_size(&app, now, width, 24);
            assert!(screen.contains(locale.text("tui.connection_filter_network")));
            assert!(screen.contains(locale.text("tui.connection_filter_tcp")));
            assert!(screen.contains(locale.text("tui.connection_rows_truncated")));
            assert!(screen.contains("zeta.invalid") && !screen.contains("alpha.invalid"));
            assert!(screen.contains("n:") && screen.contains("r:"));
        }
        assert!(!render(&app, now + FRESH_FOR).contains("zeta.invalid"));
        app.accept(Err(ReadError::Changed), now);
        assert!(app.snapshot.is_none());
        assert!(!render(&app, now).contains("zeta.invalid"));
        app.accept(load_page(&mut |r| Ok(response(r)), Page::Connections), now);
        assert!(render(&app, now).contains("zeta.invalid"));
        app.key_at(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now);
        assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
        assert!(!render(&app, now).contains("zeta.invalid"));
        assert!(!app.accept_for(
            Page::Connections,
            None,
            load_page(&mut |r| Ok(response(r)), Page::Connections),
            now
        ));
        assert!(app.pending.is_none() && app.confirmation.is_none());
        let (fresh_window, fresh_now) = sample(locale);
        let fresh = render(&fresh_window, fresh_now);
        assert!(fresh.contains("alpha.invalid") && fresh.contains("zeta.invalid"));
    }
}

#[test]
fn other_connection_type_is_an_explicit_received_category_not_an_empty_placeholder() {
    let (mut app, now) = sample(Locale::En);
    app.accept(
        load_page(
            &mut |read| {
                let mut value = response(read);
                if read == Read::ConnectionRows {
                    value["result"]["total"] = json!(3);
                    value["result"]["shown"] = json!(3);
                    value["result"]["rows"].as_array_mut().unwrap().push(json!({
                        "host":"other.invalid","ip":null,"port":null,
                        "network":"other","route":"unclassified"
                    }));
                }
                Ok(value)
            },
            Page::Connections,
        ),
        now,
    );
    for _ in 0..3 {
        app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
    }
    let other = render(&app, now);
    assert!(other.contains("other.invalid"));
    assert!(!other.contains("alpha.invalid") && !other.contains("zeta.invalid"));
    assert!(other.contains(Locale::En.text("tui.connection_filter_other")));
}

#[test]
fn ordinary_order_keys_find_largest_connection_counters_without_rounding_or_queries() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(load_page(&mut |read| {
            let mut value = response(read);
            if read == Read::Capabilities {
                value["result"]["methods"].as_array_mut().unwrap()
                    .push(json!("runtime.connection_details"));
            }
            if read == Read::ConnectionDetails {
                value["result"]["total"] = json!(7);
                value["result"]["shown"] = json!(7);
                value["result"]["truncated"] = json!(false);
                value["result"]["rows"] = json!([
                    {"host":"nine.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"9","uploadBytes":"10"},
                    {"host":"unknown.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
                    {"host":"ten.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"10","uploadBytes":"9"},
                    {"host":"zero.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"0","uploadBytes":"0"},
                    {"host":"high.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"9007199254740993"},
                    {"host":"low.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"9007199254740992"},
                    {"host":"max.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"9223372036854775807"}
                ]);
            }
            Ok(value)
        }, Page::Connections), now);
        let revision = app.snapshot.as_ref().unwrap().revision;
        let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
        // Four ordinary o presses reach largest downloaded bytes, not a new read.
        for _ in 0..4 {
            assert_eq!(
                app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now),
                Action::None
            );
        }
        let screen = render_size(&app, now, 70, 120);
        let mut previous = 0;
        for host in [
            "max.invalid",
            "high.invalid",
            "low.invalid",
            "ten.invalid",
            "nine.invalid",
            "zero.invalid",
            "unknown.invalid",
        ] {
            let position = screen.find(host).unwrap();
            assert!(
                position > previous,
                "largest downloaded integer first, unknown last: {host}"
            );
            previous = position;
        }
        app.key_at(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), now);
        let screen = render_size(&app, now, 70, 120);
        let mut previous = 0;
        for host in [
            "nine.invalid",
            "ten.invalid",
            "zero.invalid",
            "unknown.invalid",
            "high.invalid",
            "low.invalid",
            "max.invalid",
        ] {
            let position = screen.find(host).unwrap();
            assert!(
                position > previous,
                "largest uploaded integer first, unknown stable and last: {host}"
            );
            previous = position;
        }
        assert!(app.pending.is_none() && app.confirmation.is_none());
        assert_eq!(app.snapshot.as_ref().unwrap().revision, revision);
        assert!(app.snapshot.as_ref().unwrap().metadata.desired == desired);
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
            Some("nine.invalid")
        );
    }
}

#[test]
fn source_order_is_unchanged_and_the_six_choices_cycle_in_this_window_only() {
    let (mut app, now) = sample(Locale::En);
    assert_eq!(app.connection_order, Order::Original);
    for choice in [
        Order::Destination,
        Order::Network,
        Order::Route,
        Order::Download,
        Order::Upload,
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
fn equal_counter_values_keep_received_order_and_filter_only_the_same_borrowed_rows() {
    let mut value = response(Read::ConnectionDetails);
    value["result"]["total"] = json!(200);
    value["result"]["shown"] = json!(5);
    value["result"]["truncated"] = json!(true);
    value["result"]["rows"] = json!([
        {"host":"zeta.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"10","uploadBytes":"0"},
        {"host":"alpha.invalid","ip":null,"port":443,"network":"tcp","route":"vpn","downloadBytes":"10","uploadBytes":"0"},
        {"host":"zero.invalid","ip":null,"port":443,"network":"udp","route":"direct","downloadBytes":"0","uploadBytes":"10"},
        {"host":"unknown-z.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"},
        {"host":"unknown-a.invalid","ip":null,"port":443,"network":"tcp","route":"vpn"}
    ]);
    let projection = ConnectionRows::parse_details(&value).unwrap();
    for (order, indices) in [
        (Order::Download, [0, 1, 2, 3, 4]),
        (Order::Upload, [2, 0, 1, 3, 4]),
    ] {
        let sorted = rows(&projection, "", order);
        for (shown, source) in sorted.iter().zip(indices) {
            assert!(std::ptr::eq(*shown, &projection.rows[source]));
        }
        let filtered = rows(&projection, "TCP", order);
        for (shown, source) in filtered.iter().zip([0, 1, 3, 4]) {
            assert!(std::ptr::eq(*shown, &projection.rows[source]));
        }
        assert_eq!(filtered.len(), 4);
        assert!(rows(&projection, "absent-fixture", order).is_empty());
    }
    assert_eq!(projection.total, 200);
    assert!(projection.truncated);
    assert_eq!(projection.rows[0].host.as_deref(), Some("zeta.invalid"));
    // A legacy/count-only runtime cannot invent zero counters or a new order.
    let legacy = ConnectionRows::parse(&response(Read::ConnectionRows)).unwrap();
    for order in [Order::Download, Order::Upload] {
        let sorted = rows(&legacy, "", order);
        assert_eq!(sorted.len(), 2);
        assert!(std::ptr::eq(sorted[0], &legacy.rows[0]));
        assert!(std::ptr::eq(sorted[1], &legacy.rows[1]));
    }
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
