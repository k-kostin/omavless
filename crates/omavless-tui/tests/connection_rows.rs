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

fn read_details(request: Read) -> Value {
    let mut value = read(request);
    if request == Read::Capabilities {
        value["result"]["methods"]
            .as_array_mut()
            .unwrap()
            .push(json!("runtime.connection_details"));
    }
    if request == Read::ConnectionDetails {
        value["result"]["rows"][0]["ruleType"] = json!("DomainSuffix");
    }
    value
}

#[test]
fn connection_rule_type_search_is_local_intersects_filters_and_clear_keeps_order() {
    use omavless_tui::{app::Action, connection_browsing::Order};
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(
            load_page(&mut |r| Ok(read_details(r)), Page::Connections),
            now,
        );
        let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
        let revision = app.snapshot.as_ref().unwrap().revision;
        // Download order, TCP and PROXY category must intersect the local query.
        for key in ['o', 'o', 'o', 'o', 'n', 'v', 'v', 'v', '/'] {
            assert_eq!(
                app.key_at(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), now),
                Action::None
            );
        }
        for key in "dOmAiNsUfFiX".chars() {
            assert_eq!(
                app.key_at(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), now),
                Action::None
            );
        }
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now),
            Action::None
        );
        let found = render(&app, now, 70);
        assert!(found.contains("example.invalid"));
        assert!(found.contains("DomainSuffix"));
        assert!(!found.contains("192.0.2.2"));
        assert_eq!(app.connection_order, Order::Download);
        assert!(app.pending.is_none() && app.confirmation.is_none());
        app.key_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now);
        let cleared = render(&app, now, 70);
        assert!(cleared.contains("example.invalid") && cleared.contains("192.0.2.2"));
        assert_eq!(app.connection_order, Order::Download);
        assert!(app.operator_query.is_empty());
        let snapshot = app.snapshot.as_ref().unwrap();
        assert!(snapshot.metadata.desired == desired);
        assert_eq!(snapshot.revision, revision);
        assert_eq!(snapshot.connection_rows.as_ref().unwrap().rows.len(), 2);
    }
}

#[test]
fn connection_rule_type_search_never_promotes_absent_unsafe_stale_or_legacy_facts() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(
            load_page(
                &mut |request| {
                    let mut value = read_details(request);
                    if request == Read::ConnectionDetails {
                        value["result"]["total"] = json!(5);
                        value["result"]["shown"] = json!(5);
                        value["result"]["rows"] = json!([
                            {"host":"alpha.invalid","network":"tcp","route":"vpn","ruleType":"Unknown"},
                            {"host":"beta.invalid","network":"tcp","route":"vpn","ruleType":null,"rulePayload":"Unknown"},
                            {"host":"gamma.invalid","network":"tcp","route":"vpn","ruleType":"Unknown\n"},
                            {"host":"delta.invalid","network":"tcp","route":"vpn"},
                            {"host":"epsilon.invalid","network":"tcp","route":"vpn","ruleType":"DomainSuffix"}
                        ]);
                    }
                    Ok(value)
                },
                Page::Connections,
            ),
            now,
        );
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for key in "uNkNoWn".chars() {
            app.key_at(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), now);
        }
        app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        let found = render(&app, now, 70);
        assert!(found.contains("alpha.invalid"));
        for absent in [
            "beta.invalid",
            "gamma.invalid",
            "delta.invalid",
            "epsilon.invalid",
        ] {
            assert!(!found.contains(absent));
        }
        // The matching rule type does not bypass a conflicting network filter.
        for _ in 0..2 {
            app.key_at(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), now);
        }
        assert!(render(&app, now, 70).contains(locale.text("tui.no_operator_matches")));
        app.key_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now);
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for key in "suffix".chars() {
            app.key_at(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), now);
        }
        app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert!(render(&app, now, 70).contains("epsilon.invalid"));
        app.accept(Err(ReadError::Changed), now);
        assert!(!render(&app, now, 70).contains("epsilon.invalid"));
        assert!(app.snapshot.is_none());
        // An undeclared ruleType on the old rows read cannot become searchable.
        app.accept(
            load_page(
                &mut |request| {
                    assert_ne!(request, Read::ConnectionDetails);
                    let mut value = read(request);
                    if request == Read::ConnectionRows {
                        value["result"]["rows"][0]["ruleType"] = json!("DomainSuffix");
                    }
                    Ok(value)
                },
                Page::Connections,
            ),
            now,
        );
        assert!(render(&app, now, 70).contains(locale.text("tui.no_operator_matches")));
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn connection_rule_type_is_finite_optional_and_never_borrowed_from_legacy() {
    for kind in [
        "Domain",
        "DomainSuffix",
        "DomainKeyword",
        "DomainRegex",
        "DomainWildcard",
        "GeoSite",
        "GeoIP",
        "SrcGeoIP",
        "IPASN",
        "SrcIPASN",
        "IPCIDR",
        "SrcIPCIDR",
        "IPSuffix",
        "SrcIPSuffix",
        "SrcPort",
        "DstPort",
        "InPort",
        "DSCP",
        "InUser",
        "InName",
        "InType",
        "ProcessName",
        "ProcessPath",
        "ProcessNameRegex",
        "ProcessPathRegex",
        "ProcessNameWildcard",
        "ProcessPathWildcard",
        "RematchName",
        "RuleSet",
        "Network",
        "Uid",
        "SubRules",
        "Match",
        "AND",
        "OR",
        "NOT",
        "Unknown",
    ] {
        let mut value = read_details(Read::ConnectionDetails);
        value["result"]["rows"][0]["ruleType"] = json!(kind);
        assert_eq!(
            ConnectionRows::parse_details(&value).unwrap().rows[0].rule_type,
            Some(kind)
        );
    }
    for bad in [
        json!(null),
        json!(false),
        json!(1),
        json!(""),
        json!("DOMAIN"),
        json!("DomainSuffix,private.invalid"),
        json!("/private/path"),
        json!("Domain\n"),
        json!("Domain\u{202e}"),
        json!({"type":"Domain"}),
        json!("x".repeat(4096)),
    ] {
        let mut value = read_details(Read::ConnectionDetails);
        value["result"]["rows"][0]["ruleType"] = bad;
        assert_eq!(
            ConnectionRows::parse_details(&value).unwrap().rows[0].rule_type,
            None
        );
    }
    let mut legacy = read(Read::ConnectionRows);
    legacy["result"]["rows"][0]["ruleType"] = json!("DomainSuffix");
    assert_eq!(
        ConnectionRows::parse(&legacy).unwrap().rows[0].rule_type,
        None
    );
    let mut missing = read_details(Read::ConnectionDetails);
    missing["result"]["rows"][0]
        .as_object_mut()
        .unwrap()
        .remove("ruleType");
    assert_eq!(
        ConnectionRows::parse_details(&missing).unwrap().rows[0].rule_type,
        None
    );
}

#[test]
fn connection_rule_type_renders_in_en_ru_and_disappears_with_stale_private_rows() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(
            load_page(&mut |r| Ok(read_details(r)), Page::Connections),
            now,
        );
        let screen = render(&app, now, 70);
        assert!(screen.contains(locale.text("tui.connection_rule_type")));
        assert!(screen.contains("DomainSuffix"));
        view::clamp_scroll(&mut app, 70, 24, now + omavless_tui::app::FRESH_FOR);
        let stale = render(&app, now + omavless_tui::app::FRESH_FOR, 70);
        assert!(!stale.contains("DomainSuffix"));
        assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn connection_byte_totals_render_exactly_and_keep_unknown_distinct_from_zero() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(
            load_page(
                &mut |r| {
                    let mut value = read_details(r);
                    if r == Read::ConnectionDetails {
                        value["result"]["rows"][0]["uploadBytes"] = json!("0");
                        value["result"]["rows"][0]["downloadBytes"] = json!("9223372036854775807");
                    }
                    Ok(value)
                },
                Page::Connections,
            ),
            now,
        );
        let mut screens = String::new();
        for _ in 0..24 {
            screens.push_str(&render(&app, now, 70));
            app.key_at(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        }
        assert!(screens.contains(locale.text("tui.connection_download_bytes")));
        assert!(screens.contains(locale.text("tui.connection_upload_bytes")));
        assert!(
            screens.contains("9223372036854775807"),
            "byte totals must not round"
        );
        assert!(screens.contains(locale.text("tui.metric_unavailable")));
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn connection_byte_totals_require_bounded_canonical_decimal_strings() {
    for invalid in [
        json!(null),
        json!(false),
        json!(-1),
        json!(0),
        json!(1.5),
        json!(""),
        json!("-1"),
        json!("01"),
        json!("+1"),
        json!("1e3"),
        json!("1\n2"),
        json!("9223372036854775808"),
        json!("9".repeat(64)),
    ] {
        let mut value = read_details(Read::ConnectionDetails);
        value["result"]["rows"][0]["uploadBytes"] = invalid;
        value["result"]["rows"][0]["downloadBytes"] = json!("0");
        let rows = ConnectionRows::parse_details(&value).unwrap();
        assert_eq!(rows.rows[0].upload_bytes, None);
        assert_eq!(rows.rows[0].download_bytes, Some(0));
        assert_eq!(
            ConnectionRows::parse(&read(Read::ConnectionRows))
                .unwrap()
                .rows[0]
                .download_bytes,
            None
        );
    }
}

#[test]
fn ordinary_controls_keep_connection_details_and_fixed_navigation_visible() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.actions_enabled = true;
        app.page = Page::Connections;
        app.accept(
            load_page(&mut |r| Ok(read_details(r)), Page::Connections),
            now,
        );
        let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
        terminal.draw(|f| view::draw(f, &app, now)).unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            screen.contains("example.invalid"),
            "ordinary controls must not reserve unused footer rows over the first destination"
        );
        assert!(
            screen.contains("fixture-browser"),
            "the first process fact must be visible at the supported ordinary minimum"
        );
        for _ in 0..8 {
            app.key_at(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        }
        terminal.draw(|f| view::draw(f, &app, now)).unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(screen.contains(locale.text("tui.action_close_hint")));
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn normal_frame_expiry_discards_private_details_without_a_reply_or_key() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Connections;
    app.accept(
        load_page(&mut |r| Ok(read_details(r)), Page::Connections),
        now,
    );
    view::clamp_scroll(&mut app, 70, 24, now);
    assert!(app.snapshot.as_ref().unwrap().connection_rows.is_some());
    // The normal frame path must discard expired private fields even while
    // another read has not returned and the user is not pressing a key.
    view::clamp_scroll(&mut app, 70, 24, now + omavless_tui::app::FRESH_FOR);
    assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
}

#[test]
fn local_search_finds_received_process_and_chain_names_without_core_queries() {
    for query in ["FIXTURE-BROWSER", "домашний"] {
        let now = Instant::now();
        let mut app = App::new(Locale::En);
        app.page = Page::Connections;
        app.accept(
            load_page(&mut |r| Ok(read_details(r)), Page::Connections),
            now,
        );
        app.key_at(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for c in query.chars() {
            assert_eq!(
                app.key_at(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), now),
                omavless_tui::app::Action::None
            );
        }
        app.key_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        let screen = render(&app, now, 70);
        assert!(
            screen.contains("example.invalid"),
            "search must include received process and chain labels"
        );
        assert!(!screen.contains("192.0.2.2"));
        assert!(app.pending.is_none() && app.confirmation.is_none());
    }
}

#[test]
fn explicit_connections_negotiate_private_details_but_never_on_other_pages() {
    for locale in [Locale::En, Locale::Ru] {
        let mut calls = Vec::new();
        let mut source = |r: Read| {
            calls.push(r.method());
            let mut value = read(r);
            if r == Read::Capabilities {
                value["result"]["methods"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("runtime.connection_details"));
            }
            if r.method() == "runtime.connection_details" {
                value = read(Read::ConnectionRows);
                value["result"]["scope"] = json!("owned_core_private_connection_details");
                value["result"]["rows"][0]["processName"] = json!("fixture-browser");
                value["result"]["rows"][0]["chain"] = json!(["Домашний узел", "PROXY"]);
                value["result"]["rows"][1]["processName"] = Value::Null;
                value["result"]["rows"][1]["chain"] = json!(["DIRECT"]);
            }
            Ok(value)
        };
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(load_page(&mut source, Page::Connections), now);
        let screen = render(&app, now, 70);
        assert!(
            screen.contains("fixture-browser"),
            "explicit page must show safe core process label"
        );
        assert!(screen.contains("Домашний узел"));
        assert!(calls.contains(&"runtime.connection_details"));
        assert!(!calls.contains(&"runtime.connection_rows"));
    }
}

#[test]
fn private_details_are_page_local_and_a_failed_new_read_never_retries_legacy() {
    for page in [
        Page::Profiles,
        Page::Traffic,
        Page::Diagnostics,
        Page::Rules,
        Page::Settings,
        Page::Host,
    ] {
        let mut calls = Vec::new();
        let snapshot = load_page(
            &mut |r| {
                calls.push(r);
                Ok(read_details(r))
            },
            page,
        )
        .unwrap();
        assert!(snapshot.connection_rows.is_none());
        assert!(
            !calls
                .iter()
                .any(|r| r.method() == "runtime.connection_details"
                    || r.method() == "runtime.connection_rows")
        );
    }
    for malformed in [false, true] {
        let mut calls = Vec::new();
        let now = Instant::now();
        let mut app = App::new(Locale::En);
        app.page = Page::Connections;
        let snapshot = load_page(
            &mut |r| {
                calls.push(r);
                if r.method() == "runtime.connection_details" {
                    assert_eq!(r.params(), json!({}));
                    if !malformed {
                        return Err(ReadError::Unavailable);
                    }
                    return Ok(read(Read::ConnectionRows)); // wrong v1 scope is not a downgrade
                }
                Ok(read_details(r))
            },
            Page::Connections,
        );
        app.accept(snapshot, now);
        assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
        assert!(!calls.contains(&Read::ConnectionRows));
        let screen = render(&app, now, 70);
        assert!(!screen.contains("example.invalid") && !screen.contains("fixture-browser"));
        assert!(screen.contains(Locale::En.text("tui.metric_unavailable")));
    }
}

#[test]
fn private_detail_fields_are_optional_bounded_and_ignored_by_legacy_projection() {
    let good = read_details(Read::ConnectionDetails);
    for bad in [
        Value::Null,
        json!("/private/path"),
        json!("C:\\private\\app"),
        json!(".."),
        json!("a".repeat(65)),
        json!(" bad"),
        json!("bad\u{2066}label"),
        json!("bad\nlabel"),
    ] {
        let mut value = good.clone();
        value["result"]["rows"][0]["processName"] = bad;
        assert!(
            ConnectionRows::parse_details(&value).unwrap().rows[0]
                .process_name
                .is_none()
        );
    }
    for bad in [
        Value::Null,
        json!([]),
        json!(["bad\u{200b}label"]),
        json!(["a".repeat(65)]),
        json!([
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64),
            "d".repeat(64),
            "e"
        ]),
        json!(vec!["node"; 17]),
    ] {
        let mut value = good.clone();
        value["result"]["rows"][0]["chain"] = bad;
        assert!(
            ConnectionRows::parse_details(&value).unwrap().rows[0]
                .chain
                .is_none()
        );
    }
    let mut legacy = read(Read::ConnectionRows);
    legacy["result"]["rows"][0]["processName"] = json!("must-not-adopt");
    legacy["result"]["rows"][0]["chain"] = json!(["must-not-adopt"]);
    let rows = ConnectionRows::parse(&legacy).unwrap();
    assert!(!rows.details && rows.rows[0].process_name.is_none() && rows.rows[0].chain.is_none());
    assert!(ConnectionRows::parse(&good).is_none());
    assert!(ConnectionRows::parse_details(&legacy).is_none());
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Connections;
        app.accept(load_page(&mut |r| Ok(read(r)), Page::Connections), now);
        let screen = render(&app, now, 70);
        assert!(screen.contains(locale.text("tui.connection_details_legacy")));
        assert!(!screen.contains("must-not-adopt") && !screen.contains("private-node"));
    }
}

#[test]
fn detail_snapshot_cannot_survive_wrong_owner_revision_stale_error_or_page_leave() {
    for changed in ["instanceId", "revision"] {
        let now = Instant::now();
        let mut app = App::new(Locale::En);
        app.page = Page::Connections;
        app.accept(
            load_page(&mut |r| Ok(read_details(r)), Page::Connections),
            now,
        );
        assert!(render(&app, now, 70).contains("fixture-browser"));
        assert!(!render(&app, now + omavless_tui::app::FRESH_FOR, 70).contains("fixture-browser"));
        let bad = load_page(
            &mut |r| {
                let mut value = read_details(r);
                if r.method() == "runtime.connection_details" {
                    if changed == "revision" {
                        value["revision"] = json!(8);
                    } else {
                        value["result"]["instanceId"] = json!("other-runtime");
                    }
                }
                Ok(value)
            },
            Page::Connections,
        );
        assert!(matches!(bad, Err(ReadError::Changed)));
        app.accept(bad, now);
        assert!(app.snapshot.is_none());
        assert!(!render(&app, now, 70).contains("fixture-browser"));
        app.accept(
            load_page(&mut |r| Ok(read_details(r)), Page::Connections),
            now,
        );
        app.key_at(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), now);
        assert!(app.snapshot.as_ref().unwrap().connection_rows.is_none());
        let late = load_page(&mut |r| Ok(read_details(r)), Page::Connections);
        assert!(!app.accept_for(Page::Connections, None, late, now));
        assert!(!render(&app, now, 70).contains("fixture-browser"));
    }
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
fn first_page_load_is_waiting_not_a_false_connection_failure() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut app = App::new(locale);
        app.accept(load_page(&mut |r| Ok(read(r)), Page::Profiles), now);
        app.page = Page::Connections;

        let waiting = render(&app, now, 70);
        assert!(waiting.contains(locale.text("tui.connection_rows_loading")));

        // A completed read from a runtime without this optional method is a
        // real unavailable state, not an endless loading indicator.
        app.accept(
            load_page(&mut |r| Ok(support::response(r)), Page::Connections),
            now,
        );
        let unavailable = render(&app, now, 70);
        assert!(!unavailable.contains(locale.text("tui.connection_rows_loading")));
        assert!(unavailable.contains(locale.text("tui.metric_unavailable")));
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
    app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert!(app.page == Page::Connections);
    assert!(render(&app, now, 70).contains(Locale::En.text("tui.connection_rows_loading")));
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
