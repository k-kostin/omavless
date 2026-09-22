// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::{Diagnostics, Page, Traffic, bytes},
    model::{ReadError, Status},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::{Duration, Instant};

fn key(a: &mut App, code: KeyCode) -> Action {
    a.key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn render(a: &App, now: Instant, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view::draw(f, a, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(usize::from(w))
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn extra_reads_are_fixed_on_demand_and_capability_gated() {
    for (page, extra) in [
        (Page::Profiles, None),
        (Page::Details, None),
        (Page::Traffic, Some(Read::Traffic)),
        (Page::Diagnostics, Some(Read::Diagnostics)),
    ] {
        let mut calls = Vec::new();
        load_page(
            &mut |r| {
                calls.push(r);
                Ok(support::response(r))
            },
            page,
        )
        .unwrap();
        assert_eq!(calls.len(), 4 + usize::from(extra.is_some()));
        assert_eq!(calls.last(), Some(&Read::Observation));
        if let Some(extra) = extra {
            assert_eq!(calls[3], extra);
            assert_eq!(extra.params(), json!({}));
        }
        calls.clear();
        load_page(
            &mut |r| {
                calls.push(r);
                let mut v = support::response(r);
                if r == Read::Capabilities {
                    v["result"]["methods"] = json!(["ui.snapshot", "runtime.observation"]);
                }
                Ok(v)
            },
            page,
        )
        .unwrap();
        assert_eq!(calls.len(), 4);
    }
}
#[test]
fn unavailable_extra_keeps_health_but_never_fabricates_zero_or_error_text() {
    let now = Instant::now();
    for page in [Page::Traffic, Page::Diagnostics] {
        let mut a = App::new(Locale::En);
        a.page = page;
        a.accept(
            load_page(
                &mut |r| {
                    if matches!(r, Read::Traffic | Read::Diagnostics) {
                        return Ok(json!({"ok":false,"error":{"message":"PRIVATE password uri"}}));
                    }
                    Ok(support::response(r))
                },
                page,
            ),
            now,
        );
        assert_eq!(a.status(now), Status::Connected);
        let screen = render(&a, now, 100, 30);
        assert!(screen.contains("Unavailable"));
        assert!(!screen.contains("PRIVATE"));
        assert!(!screen.contains("0 B"));
    }
}
#[test]
fn optional_read_is_fenced_by_revision_instance_and_actual_observation() {
    for (path, value) in [
        ("/revision", json!(8)),
        ("/result/instanceId", json!("restarted")),
        ("/result/desired/generation", json!(6)),
        ("/result/facts/managedTunCount", json!(99)),
    ] {
        let result = load_page(
            &mut |r| {
                let mut v = support::response(r);
                if r == Read::Observation {
                    *v.pointer_mut(path).unwrap() = value.clone();
                }
                Ok(v)
            },
            Page::Traffic,
        );
        assert!(result.is_err());
    }
    assert!(matches!(
        load_page(
            &mut |r| {
                let mut v = support::response(r);
                if r == Read::Traffic {
                    v["revision"] = json!(6);
                }
                Ok(v)
            },
            Page::Traffic
        ),
        Err(ReadError::Changed)
    ));
}
#[test]
fn traffic_bounds_rates_reset_and_tun_directions_match_qml() {
    let v = support::response(Read::Traffic);
    let a = Traffic::parse(&v).unwrap();
    assert_eq!((a.upload, a.download), (8192, 4096));
    let mut next = v.clone();
    next["result"]["sample"]["rxBytes"] = json!(8192);
    next["result"]["sample"]["txBytes"] = json!(16384);
    next["result"]["sample"]["sampledAtMs"] = json!(3000);
    assert_eq!(Traffic::parse(&next).unwrap().rates(&a), Some((4096, 2048)));
    for (field, val) in [
        ("sampledAtMs", json!(1000)),
        ("sampledAtMs", json!(12000)),
        ("rxBytes", json!(1)),
        ("identity", json!("b".repeat(64))),
    ] {
        let mut changed = next.clone();
        changed["result"]["sample"][field] = val;
        assert!(Traffic::parse(&changed).unwrap().rates(&a).is_none());
    }
    for (field, val) in [
        ("rxBytes", json!(-1)),
        ("txBytes", json!(9_007_199_254_740_992_u64)),
        ("sampledAtMs", json!(1.5)),
        ("identity", json!("private://bad")),
    ] {
        let mut bad = v.clone();
        bad["result"]["sample"][field] = val;
        assert!(Traffic::parse(&bad).is_none());
    }
    assert_eq!(bytes(1024), "1 KiB");
    assert_eq!(bytes(0), "0 B");
}
#[test]
fn diagnostics_retains_only_counts_and_rejects_oversize_unknown_schema() {
    let mut v = support::response(Read::Diagnostics);
    v["result"]["rules"]["items"] = json!([{"payload":"private://secret"}]);
    assert_eq!(Diagnostics::parse(&v).unwrap().rules, 42);
    v["result"]["providers"]["total"] = json!(257);
    assert!(Diagnostics::parse(&v).is_none());
    v["result"]["providers"]["total"] = json!(2);
    v["result"]["version"] = json!(2);
    assert!(Diagnostics::parse(&v).is_none());
}
#[test]
fn navigation_and_details_never_mutate_or_retarget_profile() {
    let now = Instant::now();
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    a.accept(
        load_page(&mut |r| Ok(support::response(r)), Page::Profiles),
        now,
    );
    key(&mut a, KeyCode::End);
    let selected = a.selected.clone();
    assert_eq!(key(&mut a, KeyCode::Tab), Action::Refresh);
    assert!(a.page == Page::Traffic);
    key(&mut a, KeyCode::Tab);
    assert!(a.page == Page::Details);
    let screen = render(&a, now, 100, 30);
    assert!(screen.contains("Fixture Frankfurt"));
    assert!(screen.contains("This profile is connected: No"));
    for k in ['c', 'd', '1', 'f'] {
        assert_eq!(key(&mut a, KeyCode::Char(k)), Action::None);
    }
    assert!(a.pending.is_none());
    assert!(a.confirmation.is_none());
    assert_eq!(a.selected, selected);
    key(&mut a, KeyCode::BackTab);
    assert!(a.page == Page::Traffic);
    key(&mut a, KeyCode::Esc);
    assert!(a.page == Page::Profiles);
    assert_eq!(a.selected, selected);
    key(&mut a, KeyCode::Char('/'));
    assert_eq!(key(&mut a, KeyCode::Tab), Action::None);
    assert!(a.page == Page::Profiles);
}
#[test]
fn rates_never_bridge_restart_old_sample_or_page_gap() {
    let now = Instant::now();
    let mut a = App::new(Locale::En);
    let sample = || load_page(&mut |r| Ok(support::response(r)), Page::Traffic);
    a.accept(sample(), now);
    assert!(a.traffic_rates.is_none());
    a.accept(
        load_page(
            &mut |r| {
                let mut v = support::response(r);
                if r == Read::Traffic {
                    v["result"]["sample"]["sampledAtMs"] = json!(4000);
                }
                Ok(v)
            },
            Page::Traffic,
        ),
        now + Duration::from_secs(3),
    );
    assert_eq!(a.traffic_rates, Some((0, 0)));
    a.accept(Err(ReadError::Unavailable), now + Duration::from_secs(4));
    a.accept(sample(), now + Duration::from_secs(5));
    assert!(a.traffic_rates.is_none());
}
#[test]
fn english_russian_all_pages_resize_stale_and_private_field_exclusion() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        for page in [Page::Traffic, Page::Details, Page::Diagnostics] {
            let mut a = App::new(locale);
            a.page = page;
            a.accept(load_page(&mut |r| Ok(support::response(r)), page), now);
            a.selected = Some("fixture-a".into());
            for (w, h) in [(50, 14), (70, 24), (100, 30)] {
                let screen = render(&a, now, w, h);
                assert!(screen.contains(locale.text("tui.close_hint")));
                for secret in ["fixture-a", "fixture-runtime", "aaaaaaa", "private://"] {
                    assert!(!screen.contains(secret));
                }
            }
            assert!(render(&a, now + FRESH_FOR, 100, 30).contains(locale.text("tui.stale")));
            assert!(!render(&a, now + FRESH_FOR, 100, 30).contains("42"));
        }
    }
}
