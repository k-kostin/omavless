// SPDX-License-Identifier: MIT
mod support;

use omavless_tui::{
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::{CustomRules, HostSupport, Page, Providers, Rules},
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
        (Page::CustomRules, Read::CustomRules),
    ] {
        let mut calls = Vec::new();
        let snapshot = load_page(
            &mut |read| {
                calls.push(read);
                let mut result = support::response(read);
                if page == Page::CustomRules && read == Read::Capabilities {
                    result["result"]["methods"] = json!([
                        "ui.snapshot",
                        "runtime.observation",
                        "routing.custom_rules.list"
                    ]);
                }
                Ok(result)
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
        assert!(without_capability.custom_rules.is_none());
        assert!(matches!(
            load_page(
                &mut |read| {
                    let mut result = support::response(read);
                    if page == Page::CustomRules && read == Read::Capabilities {
                        result["result"]["methods"] = json!([
                            "ui.snapshot",
                            "runtime.observation",
                            "routing.custom_rules.list"
                        ]);
                    }
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
    let mut truncated_rules = rules.clone();
    truncated_rules["result"]["rules"]["total"] = json!(65_536);
    truncated_rules["result"]["rules"]["truncated"] = json!(true);
    let parsed = Rules::parse(&truncated_rules).unwrap();
    assert_eq!(parsed.total, 65_536);
    assert_eq!(parsed.items.len(), 3);
    assert!(parsed.truncated);
    for (path, bad) in [
        ("/result/rules/total", json!(65_537)),
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
        for page in [Page::Host, Page::Rules, Page::Providers, Page::CustomRules] {
            let mut app = App::new(locale);
            app.page = page;
            app.accept(
                load_page(
                    &mut |read| {
                        let mut value = support::response(read);
                        if page == Page::CustomRules && read == Read::Capabilities {
                            value["result"]["methods"] = json!([
                                "ui.snapshot",
                                "runtime.observation",
                                "routing.custom_rules.list"
                            ]);
                        }
                        Ok(value)
                    },
                    page,
                ),
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

#[test]
fn diagnostics_shows_only_typed_log_classifications_not_private_lines() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.page = Page::Diagnostics;
        app.accept(
            load_page(
                &mut |read| {
                    let mut value = support::response(read);
                    if read == Read::Observation {
                        value["result"]["coreDiagnostics"] = json!({
                            "scope":"latest_owned_core_log_counts",
                            "dnsErrors":2,"tlsErrors":1,"timeoutErrors":0,
                            "connectionErrors":3,"otherWarnings":0,"oversizedLines":1,
                            "readFailed":false,"incomplete":true,"finished":false,
                            "rawLine":"private://secret"
                        });
                    }
                    Ok(value)
                },
                Page::Diagnostics,
            ),
            now,
        );
        let screen = render_at(&app, now, 100, 40);
        assert!(screen.contains(locale.text("tui.core_log_dns")));
        assert!(screen.contains(locale.text("tui.core_log_incomplete")));
        assert!(!screen.contains("private://secret"));
    }
}

#[test]
fn doctor_facts_distinguish_last_known_state_from_unavailable_observation() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.page = Page::Diagnostics;
        app.accept(
            load_page(&mut |read| Ok(support::response(read)), Page::Diagnostics),
            now,
        );
        let observed = render_at(&app, now, 100, 40);
        assert!(observed.contains(locale.text("tui.doctor_scope")));
        assert!(observed.contains(locale.text("tui.doctor_requested")));
        assert!(observed.contains(locale.text("tui.doctor_last_actual")));
        assert!(observed.contains(locale.text("tui.doctor_profile_match")));
        assert!(observed.contains(locale.text("tui.doctor_inventory_scope")));
        assert!(observed.contains(locale.text("tui.health")));

        let mut unavailable = App::new(locale);
        unavailable.page = Page::Diagnostics;
        unavailable.accept(
            load_page(
                &mut |read| {
                    let mut value = support::response(read);
                    if read == Read::Observation {
                        value["result"]["availability"] = json!("unavailable");
                        value["result"]["facts"] = serde_json::Value::Null;
                    }
                    Ok(value)
                },
                Page::Diagnostics,
            ),
            now,
        );
        let unknown = render_at(&unavailable, now, 100, 40);
        assert!(unknown.contains(locale.text("tui.doctor_last_actual")));
        assert!(unknown.contains(locale.text("tui.metric_unavailable")));
        assert!(!unknown.contains("private://"));
    }
}

#[test]
fn doctor_last_known_labels_cover_transitions_without_network_verdict() {
    let now = Instant::now();
    for (actual, key) in [
        ("disconnected", "tui.doctor_disconnected"),
        ("starting", "tui.doctor_starting"),
        ("connected", "tui.doctor_connected"),
        ("reconnecting", "tui.doctor_reconnecting"),
        ("stopping", "tui.doctor_stopping"),
        ("failed", "tui.doctor_failed"),
        ("manualRecoveryRequired", "tui.doctor_recovery"),
    ] {
        for locale in [Locale::En, Locale::Ru] {
            let mut app = App::new(locale);
            app.page = Page::Diagnostics;
            app.accept(
                load_page(
                    &mut |read| {
                        let mut value = support::response(read);
                        if matches!(read, Read::Snapshot | Read::Observation) {
                            value["result"]["lastKnownActual"] = json!(actual);
                        }
                        if read == Read::Observation {
                            value["result"]["manualRecoveryRequired"] =
                                json!(actual == "manualRecoveryRequired");
                        }
                        Ok(value)
                    },
                    Page::Diagnostics,
                ),
                now,
            );
            let screen = render_at(&app, now, 100, 40);
            assert!(screen.contains(locale.text(key)), "{actual} {locale:?}");
            assert!(screen.contains(locale.text("tui.health")));
            assert!(!screen.contains("Missing translation"));
        }
    }
}

#[test]
fn saved_rule_overrides_are_private_page_local_and_not_mistaken_for_loaded_rules() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Diagnostics;
    assert_eq!(key(&mut app, KeyCode::Char('C')), Action::Refresh);
    assert!(app.page == Page::CustomRules);
    let mut calls = Vec::new();
    app.accept(
        load_page(
            &mut |read| {
                calls.push(read);
                let mut value = support::response(read);
                if read == Read::Capabilities {
                    value["result"]["methods"] = json!([
                        "ui.snapshot",
                        "runtime.observation",
                        "routing.custom_rules.list"
                    ]);
                }
                Ok(value)
            },
            Page::CustomRules,
        ),
        now,
    );
    assert_eq!(
        calls,
        [
            Read::Hello,
            Read::Capabilities,
            Read::Snapshot,
            Read::CustomRules,
            Read::Observation
        ]
    );
    assert_eq!(Read::CustomRules.params(), json!({}));
    let screen = render(&app, now);
    assert!(screen.contains("Configured rule overrides"));
    assert!(screen.contains("fixture.invalid"));
    assert!(!screen.contains("fixture-rule-1"));
    assert!(!screen.contains("Loaded rules"));
    assert_eq!(key(&mut app, KeyCode::Char('/')), Action::None);
    for c in "192.0.2".chars() {
        assert_eq!(key(&mut app, KeyCode::Char(c)), Action::None);
    }
    assert_eq!(key(&mut app, KeyCode::Enter), Action::None);
    let screen = render(&app, now);
    assert!(screen.contains("192.0.2.0/24"));
    assert!(!screen.contains("fixture.invalid"));
    assert!(app.pending.is_none());
    assert_eq!(key(&mut app, KeyCode::Esc), Action::Refresh);
    assert!(app.page == Page::Diagnostics);

    let mut malformed = support::response(Read::CustomRules);
    for (path, bad) in [
        ("/result/rules/0/id", json!("")),
        ("/result/rules/0/kind", json!("shell")),
        ("/result/rules/0/action", json!("unknown")),
        ("/result/rules/0/value", json!("x".repeat(1025))),
    ] {
        *malformed.pointer_mut(path).unwrap() = bad;
        assert!(CustomRules::parse(&malformed).is_none(), "{path}");
        malformed = support::response(Read::CustomRules);
    }
    malformed["result"]["rules"][0]["value"] = json!("private\u{001b}[31m.invalid");
    let parsed = CustomRules::parse(&malformed).unwrap();
    assert!(!parsed.items[0].value.contains('\u{001b}'));
}
