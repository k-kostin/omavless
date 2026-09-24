// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::{Page, saved_age},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::Instant;

fn app(locale: Locale, total: usize) -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.page = Page::Subscriptions;
    app.accept(load_page(&mut |r| {
        let mut v = support::response(r);
        if r == Read::Snapshot {
            v["result"]["subscriptions"][0]["updatedAt"] = json!(0);
            for i in 1..total {
                v["result"]["subscriptions"].as_array_mut().unwrap().push(json!({
                    "id":format!("private-id-{i}"),"name":format!("Empty subscription {i}"),"updatedAt":null
                }));
            }
            // Never trust extra provider payload or spoofed counts for this page.
            v["result"]["subscriptions"][0]["url"] = json!("https://private.invalid/secret");
            v["result"]["subscriptions"][0]["profileCount"] = json!(9999);
            v["result"]["profiles"][1]["missing"] = json!(true);
        }
        Ok(v)
    }, Page::Subscriptions), now);
    (app, now)
}
fn key(a: &mut App, key: KeyCode, now: Instant) -> Action {
    a.key_at(KeyEvent::new(key, KeyModifiers::NONE), now)
}
fn render(a: &mut App, now: Instant, width: u16, height: u16) -> String {
    view::clamp_scroll(a, width, height, now);
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| view::draw(f, a, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(width as usize)
        .map(|r| r.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn saved_time_uses_milliseconds_and_never_claims_future_or_unknown_success() {
    let now = 1_000_000_000;
    for unknown in [None, Some(0), Some(now + 1), Some(u64::MAX)] {
        assert_eq!(saved_age(unknown, now), ("tui.metric_unavailable", None));
    }
    for (delta, expected) in [
        (0, ("tui.age_recent", None)),
        (59_999, ("tui.age_recent", None)),
        (60_000, ("tui.age_minutes", Some(1))),
        (3_600_000, ("tui.age_hours", Some(1))),
        (86_400_000, ("tui.age_days", Some(1))),
    ] {
        assert_eq!(saved_age(Some(now - delta), now), expected);
    }
    for locale in [Locale::En, Locale::Ru] {
        for key in ["tui.age_minutes", "tui.age_hours", "tui.age_days"] {
            assert_eq!(locale.text(key).matches("{count}").count(), 1);
            assert!(!locale.text(key).replace("{count}", "123").contains('{'));
        }
    }
}

#[test]
fn overview_uses_only_existing_reads_and_does_not_fetch_provider() {
    let mut calls = Vec::new();
    load_page(
        &mut |r| {
            calls.push(r);
            Ok(support::response(r))
        },
        Page::Subscriptions,
    )
    .unwrap();
    assert_eq!(
        calls,
        vec![
            Read::Hello,
            Read::Capabilities,
            Read::Snapshot,
            Read::Observation
        ]
    );
}

#[test]
fn empty_feeds_missing_nodes_and_no_subscriptions_are_distinct() {
    let (mut a, now) = app(Locale::En, 2);
    let screen = render(&mut a, now, 100, 32);
    assert!(screen.contains("Saved profiles: 1 · Missing from feed: 1"));
    assert!(screen.contains("Empty subscription 1"));
    assert!(screen.contains("Saved profiles: 0 · Missing from feed: 0"));
    assert!(screen.contains("Saved list updated: Unavailable"));
    for secret in ["private-id", "private.invalid", "fixture-sub", "9999"] {
        assert!(!screen.contains(secret));
    }
    a.snapshot.as_mut().unwrap().metadata.subscriptions.clear();
    assert!(render(&mut a, now, 100, 32).contains("No subscriptions."));
}

#[test]
fn all_64_feeds_reachable_and_up_works_immediately_after_end_and_resize() {
    let (mut a, now) = app(Locale::En, 64);
    key(&mut a, KeyCode::End, now);
    assert!(render(&mut a, now, 70, 24).contains("Empty subscription 63"));
    let bottom = a.inspection_scroll;
    key(&mut a, KeyCode::Up, now);
    render(&mut a, now, 70, 24);
    assert_eq!(a.inspection_scroll, bottom - 1);
    key(&mut a, KeyCode::Home, now);
    assert!(render(&mut a, now, 70, 24).contains("Fixture subscription"));
    key(&mut a, KeyCode::End, now);
    assert!(render(&mut a, now, 120, 40).contains("Empty subscription 63"));
    a.snapshot
        .as_mut()
        .unwrap()
        .metadata
        .subscriptions
        .truncate(1);
    assert!(render(&mut a, now, 120, 40).contains("Fixture subscription"));
    assert_eq!(a.inspection_scroll, 0);
}

#[test]
fn navigation_never_mutates_or_retargets_hidden_profile() {
    let (mut a, now) = app(Locale::En, 2);
    a.selected = Some("fixture-b".into());
    for ch in ['s', 'c', 'd', '1', '2', '3', '/', 'f', 'u', 'a'] {
        assert_eq!(key(&mut a, KeyCode::Char(ch), now), Action::None);
    }
    assert_eq!(key(&mut a, KeyCode::Enter, now), Action::None);
    assert!(a.confirmation.is_none() && a.pending.is_none());
    assert_eq!(a.selected.as_deref(), Some("fixture-b"));
    assert_eq!(key(&mut a, KeyCode::Char('r'), now), Action::Refresh);
    assert_eq!(key(&mut a, KeyCode::Esc, now), Action::Refresh);
    assert!(a.page == Page::Profiles);
    assert_eq!(key(&mut a, KeyCode::BackTab, now), Action::Refresh);
    assert!(a.page == Page::Subscriptions);
    assert_eq!(key(&mut a, KeyCode::Tab, now), Action::Refresh);
    assert!(a.page == Page::Profiles);
    assert_eq!(key(&mut a, KeyCode::Char('q'), now), Action::Close);
}

#[test]
fn overview_localizes_chrome_not_names_and_never_displays_stale_metadata() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut a, now) = app(locale, 2);
        a.snapshot.as_mut().unwrap().metadata.subscriptions[0].name =
            "<Provider>\u{1b}\u{202e}".into();
        let screen = render(&mut a, now, 100, 32);
        assert!(screen.contains("<Provider>��"));
        assert!(screen.contains(locale.text("tui.subscriptions")));
        assert!(screen.contains(locale.text("tui.action_close_hint")));
        let stale = render(&mut a, now + FRESH_FOR, 100, 32);
        assert!(stale.contains(locale.text("tui.stale")));
        assert!(!stale.contains("<Provider>"));
        a.actions_enabled = false;
        assert!(render(&mut a, now, 50, 14).contains(locale.text("tui.close_hint")));
    }
}

#[test]
fn timestamp_admission_is_optional_unsigned_and_bounded_by_u64() {
    for value in [json!(-1), json!(1.5), json!("private-input"), json!({})] {
        assert!(
            load_page(
                &mut |r| {
                    let mut v = support::response(r);
                    if r == Read::Snapshot {
                        v["result"]["subscriptions"][0]["updatedAt"] = value.clone();
                    }
                    Ok(v)
                },
                Page::Subscriptions
            )
            .is_err()
        );
    }
}
