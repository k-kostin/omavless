// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::{Kind, Outcome},
    app::{Action, App, FRESH_FOR},
    client::{self, Read},
    i18n::Locale,
    inspection::Page,
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::Instant;

fn app() -> (App, Instant) {
    let now = Instant::now();
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    a.accept(
        client::load(&mut |r| {
            let mut v = support::response(r);
            if r == Read::Capabilities {
                v["result"]["methods"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("plugin.action"));
            }
            Ok(v)
        }),
        now,
    );
    a.selected = Some("fixture-b".into());
    (a, now)
}
fn key(a: &mut App, code: KeyCode, now: Instant) -> Action {
    a.key_at(KeyEvent::new(code, KeyModifiers::NONE), now)
}

#[test]
fn refresh_is_exact_confirmed_subscription_not_connect_or_reload() {
    let (mut a, now) = app();
    assert_eq!(key(&mut a, KeyCode::Char('r'), now), Action::Refresh);
    assert!(a.confirmation.is_none());
    assert_eq!(key(&mut a, KeyCode::Char('s'), now), Action::None);
    assert!(a.pending.is_none());
    assert_eq!(a.confirm(now, "refresh-fixture".into()), Action::Submit);
    let p = a.pending.as_ref().unwrap().params();
    assert_eq!(
        p,
        json!({"instanceId":"fixture-runtime","expectedRevision":7,
        "operationId":"refresh-fixture","action":"subscription-refresh","subscriptionId":"fixture-sub"})
    );
    assert!(p.get("profileId").is_none() && p.get("url").is_none() && p.get("mode").is_none());
}

#[test]
fn standalone_missing_selection_filtered_and_stale_targets_are_rejected() {
    for case in 0..7 {
        let (mut a, now) = app();
        match case {
            0 => a.selected = None,
            1 => a.selected = Some("fixture-a".into()),
            2 => a.query = "no matching profile".into(),
            3 => a.sampled_at = Some(now - FRESH_FOR),
            4 => a.snapshot.as_mut().unwrap().metadata.subscriptions.clear(),
            5 => a.snapshot.as_mut().unwrap().actions_available = false,
            _ => a.actions_enabled = false,
        }
        a.prepare(Kind::RefreshSubscription, None, now);
        assert!(a.confirmation.is_none(), "case {case}");
        assert!(a.pending.is_none());
    }
    // A feed-missing node may still identify its subscription for recovery.
    let (mut a, now) = app();
    a.snapshot.as_mut().unwrap().metadata.profiles[1].missing = true;
    a.prepare(Kind::RefreshSubscription, None, now);
    assert!(a.confirmation.is_some());
}

#[test]
fn confirmation_refuses_changed_selection_source_identity_name_or_revision() {
    for case in 0..6 {
        let (mut a, now) = app();
        a.prepare(Kind::RefreshSubscription, None, now);
        match case {
            0 => a.selected = Some("fixture-a".into()),
            1 => a.snapshot.as_mut().unwrap().metadata.profiles[1].subscription_id = None,
            2 => {
                a.snapshot.as_mut().unwrap().metadata.subscriptions[0].name =
                    "Renamed fixture".into()
            }
            3 => a.snapshot.as_mut().unwrap().metadata.subscriptions.clear(),
            4 => a.snapshot.as_mut().unwrap().revision += 1,
            _ => a.snapshot.as_mut().unwrap().metadata.instance_id = "next-instance".into(),
        }
        assert_eq!(a.confirm(now, "refresh-fixture".into()), Action::None);
        assert!(a.pending.is_none());
    }
}

#[test]
fn search_other_pages_and_escape_never_refresh_a_provider() {
    let (mut a, now) = app();
    a.searching = true;
    key(&mut a, KeyCode::Char('s'), now);
    assert!(a.confirmation.is_none());
    a.searching = false;
    a.query.clear();
    for page in [Page::Traffic, Page::Details, Page::Diagnostics] {
        a.page = page;
        key(&mut a, KeyCode::Char('s'), now);
        assert!(a.confirmation.is_none());
    }
    a.page = Page::Profiles;
    key(&mut a, KeyCode::Char('s'), now);
    key(&mut a, KeyCode::Esc, now);
    assert!(a.confirmation.is_none() && a.pending.is_none());
}

#[test]
fn refresh_receipts_are_fenced_and_remote_private_errors_are_not_rendered() {
    let (mut a, now) = app();
    a.prepare(Kind::RefreshSubscription, None, now);
    a.confirm(now, "refresh-fixture".into());
    let request = a.pending.as_ref().unwrap();
    let mut ok = json!({"ok":true,"revision":8,"result":{"schemaVersion":1,
        "instanceId":"fixture-runtime","operationId":"refresh-fixture","action":"subscription-refresh","applied":true}});
    assert_eq!(request.outcome(Ok(ok.clone())), Outcome::Applied);
    ok["result"]["action"] = json!("connect");
    assert_eq!(request.outcome(Ok(ok)), Outcome::Unknown);
    assert_eq!(
        request.outcome(Ok(json!({"ok":false,"revision":7,"error":{
        "code":"provider_error","message":"https://private.invalid/secret-token"}}))),
        Outcome::Unknown
    );
}

#[test]
fn unknown_refresh_retry_keeps_original_subscription_and_operation() {
    let (mut a, now) = app();
    a.prepare(Kind::RefreshSubscription, None, now);
    a.confirm(now, "refresh-fixture".into());
    let original = a.pending.as_ref().unwrap().params();
    a.finish(Outcome::Unknown, now);
    key(&mut a, KeyCode::Char('u'), now);
    assert_eq!(a.confirm(now, "not-used".into()), Action::Submit);
    assert_eq!(a.pending.as_ref().unwrap().params(), original);
}

#[test]
fn both_languages_show_subscription_and_profile_before_confirmation() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut a, now) = app();
        a.locale = locale;
        a.prepare(Kind::RefreshSubscription, None, now);
        let mut t = Terminal::new(TestBackend::new(70, 24)).unwrap();
        t.draw(|f| view::draw(f, &a, now)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Fixture Frankfurt") && text.contains("Fixture subscription"));
        assert!(text.contains(locale.text("tui.refresh_subscription")));
        assert!(text.contains(locale.text("tui.confirm_keys")));
        assert!(!text.contains("fixture-sub") && !text.contains("fixture-b"));
    }
}
