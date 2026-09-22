// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::{Kind, Outcome},
    app::{Action, App, Confirmation, FRESH_FOR},
    client::{Read, load},
    i18n::Locale,
    model::{Actual, Mode, ReadError, Status},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn app() -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.actions_enabled = true;
    app.accept(
        load(&mut |r| {
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
    (app, now)
}
fn key(app: &mut App, code: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(code, KeyModifiers::NONE), now)
}
fn begin(app: &mut App, kind: Kind, now: Instant) {
    app.selected = Some("fixture-b".into());
    app.prepare(kind, Some(Mode::Global), now);
    assert_eq!(
        app.confirm(now, "synthetic-operation".into()),
        Action::Submit
    );
}
fn applied(app: &App) -> Value {
    let p = app.pending.as_ref().unwrap().params();
    json!({"ok":true,"revision":8,"result":{"schemaVersion":1,"instanceId":p["instanceId"],
        "operationId":p["operationId"],"action":p["action"],"applied":true}})
}

#[test]
fn selected_and_active_are_different_explicit_targets() {
    for (kind, target) in [
        (Kind::Connect, "fixture-b"),
        (Kind::Disconnect, "fixture-a"),
        (Kind::Mode, "fixture-a"),
    ] {
        let (mut app, now) = app();
        begin(&mut app, kind, now);
        let p = app.pending.as_ref().unwrap().params();
        assert_eq!(p["instanceId"], "fixture-runtime");
        assert_eq!(p["expectedRevision"], 7);
        assert_eq!(p["operationId"], "synthetic-operation");
        assert_eq!(p["action"], kind.wire());
        if kind == Kind::Connect {
            assert_eq!(p["profileId"], target);
        } else {
            assert!(p.get("profileId").is_none());
        }
        assert_eq!(
            p.get("mode").and_then(Value::as_str),
            if kind == Kind::Disconnect {
                None
            } else {
                Some("global")
            }
        );
        assert_eq!(
            p.as_object().unwrap().len(),
            if kind == Kind::Connect {
                6
            } else if kind == Kind::Mode {
                5
            } else {
                4
            }
        );
    }
}
#[test]
fn navigation_search_cancel_and_repeated_key_do_not_dispatch() {
    let (mut app, now) = app();
    for code in [
        KeyCode::Down,
        KeyCode::Enter,
        KeyCode::Char('/'),
        KeyCode::Char('c'),
        KeyCode::Char('1'),
        KeyCode::Enter,
    ] {
        assert_eq!(key(&mut app, code, now), Action::None);
        assert!(app.pending.is_none());
    }
    app.query.clear();
    app.selected = Some("fixture-b".into());
    assert_eq!(key(&mut app, KeyCode::Char('c'), now), Action::None);
    let repeated =
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat);
    assert_eq!(app.key_at(repeated, now), Action::None);
    assert!(app.pending.is_none());
    key(&mut app, KeyCode::Esc, now);
    assert!(app.confirmation.is_none());
    assert!(app.pending.is_none());
}
#[test]
fn freshness_capability_recovery_and_missing_profile_gate_admission() {
    for case in 0..7 {
        let (mut app, now) = app();
        app.selected = Some("fixture-b".into());
        match case {
            0 => app.sampled_at = Some(now - FRESH_FOR),
            1 => app.snapshot.as_mut().unwrap().actions_available = false,
            2 => {
                app.snapshot
                    .as_mut()
                    .unwrap()
                    .observation
                    .manual_recovery_required = true
            }
            3 => app.snapshot.as_mut().unwrap().metadata.profiles[1].missing = true,
            4 => app.snapshot.as_mut().unwrap().metadata.last_known_actual = Actual::Starting,
            5 => app.actions_enabled = false,
            _ => app.viewport_ready = false,
        }
        app.prepare(Kind::Connect, None, now);
        assert!(app.confirmation.is_none(), "case {case}");
        assert!(app.pending.is_none());
    }
}
#[test]
fn recovery_retains_disconnect_with_current_snapshot() {
    let (mut app, now) = app();
    let s = app.snapshot.as_mut().unwrap();
    s.observation.manual_recovery_required = true;
    s.observation.facts = None;
    s.metadata.last_known_actual = Actual::ManualRecoveryRequired;
    begin(&mut app, Kind::Disconnect, now);
}
#[test]
fn confirmation_fences_revision_instance_target_removal_and_age() {
    for case in 0..5 {
        let (mut app, now) = app();
        app.selected = Some("fixture-b".into());
        app.prepare(Kind::Connect, None, now);
        match case {
            0 => app.snapshot.as_mut().unwrap().revision += 1,
            1 => app.snapshot.as_mut().unwrap().metadata.instance_id = "another-instance".into(),
            2 => {
                app.snapshot.as_mut().unwrap().metadata.profiles.pop();
            }
            3 => app.snapshot.as_mut().unwrap().metadata.profiles[1].name = "Changed target".into(),
            _ => app.sampled_at = Some(now - FRESH_FOR),
        }
        assert_eq!(app.confirm(now, "op".into()), Action::None);
        assert!(app.pending.is_none());
    }
}
#[test]
fn pending_does_not_predict_success_and_blocks_second_mutation() {
    let (mut app, now) = app();
    begin(&mut app, Kind::Connect, now);
    let original = app.pending.as_ref().unwrap().params();
    app.prepare(Kind::Disconnect, None, now);
    assert!(app.confirmation.is_none());
    assert_eq!(app.pending.as_ref().unwrap().params(), original);
    assert_eq!(app.status(now), Status::Unverified);
    assert_eq!(key(&mut app, KeyCode::Char('q'), now), Action::Close);
    assert!(app.pending.is_some()); // Closing the client is not cancel/disconnect.
}
#[test]
fn matching_reply_then_fresh_observation_are_both_required() {
    let (mut app, now) = app();
    let prior = app.snapshot.clone().unwrap();
    begin(&mut app, Kind::Connect, now);
    let outcome = app.pending.as_ref().unwrap().outcome(Ok(applied(&app)));
    assert_eq!(outcome, Outcome::Applied);
    let done = now + Duration::from_millis(100);
    app.finish(outcome, done);
    app.accept(Ok(prior.clone()), now);
    assert_eq!(app.status(done), Status::Unverified);
    app.accept(Ok(prior), done);
    assert_eq!(app.status(done), Status::Connected);
    assert!(app.pending.is_none());
}
#[test]
fn malformed_mismatched_private_errors_never_become_success() {
    let (mut app, now) = app();
    begin(&mut app, Kind::Connect, now);
    for field in ["instanceId", "operationId", "action"] {
        let mut reply = applied(&app);
        reply["result"][field] = json!("private://do-not-echo/password");
        assert_eq!(
            app.pending.as_ref().unwrap().outcome(Ok(reply)),
            Outcome::Unknown
        );
    }
    for reply in [
        json!(null),
        json!({"ok":true}),
        json!({"ok":false,"revision":8,"error":{"code":"private://password","message":"secret"}}),
    ] {
        assert_eq!(
            app.pending.as_ref().unwrap().outcome(Ok(reply)),
            Outcome::Unknown
        );
    }
    let denied = json!({"ok":false,"revision":7,"error":{"code":"permission_denied","message":"private://password"}});
    assert_eq!(
        app.pending.as_ref().unwrap().outcome(Ok(denied)),
        Outcome::Rejected("tui.action_denied")
    );
}
#[test]
fn transport_loss_exact_retry_keeps_original_fences_and_payload() {
    let (mut app, now) = app();
    begin(&mut app, Kind::Connect, now);
    let original = app.pending.as_ref().unwrap().params();
    let outcome = app
        .pending
        .as_ref()
        .unwrap()
        .outcome(Err(ReadError::Unavailable));
    app.finish(outcome, now);
    assert!(app.unknown);
    key(&mut app, KeyCode::Char('u'), now);
    assert!(matches!(app.confirmation, Some(Confirmation::Retry)));
    assert_eq!(
        app.confirm(now, "must-not-replace-original".into()),
        Action::Submit
    );
    assert_eq!(app.pending.as_ref().unwrap().params(), original);
    app.finish(Outcome::Rejected("tui.action_changed"), now);
    assert!(app.unknown); // Restart rejection doesn't resolve earlier outcome.
}
#[test]
fn unknown_ack_requires_new_fresh_review_and_is_not_success() {
    let (mut app, now) = app();
    let s = app.snapshot.clone().unwrap();
    begin(&mut app, Kind::Connect, now);
    app.finish(Outcome::Unknown, now);
    key(&mut app, KeyCode::Char('a'), now);
    assert!(app.confirmation.is_none());
    app.accept(Ok(s), now);
    key(&mut app, KeyCode::Char('a'), now);
    assert!(matches!(
        app.confirmation,
        Some(Confirmation::Acknowledge { .. })
    ));
    assert_eq!(app.confirm(now, "unused".into()), Action::None);
    assert!(app.pending.is_none());
    assert_eq!(app.notice, "tui.action_acknowledged");
}
#[test]
fn mode_keys_are_explicit_and_search_cannot_activate_them() {
    for (code, mode) in [('1', Mode::Global), ('2', Mode::Rule), ('3', Mode::Direct)] {
        let (mut app, now) = app();
        key(&mut app, KeyCode::Char(code), now);
        match &app.confirmation {
            Some(Confirmation::New(c)) => assert_eq!(c.mode, mode),
            _ => panic!("missing confirmation"),
        }
        assert!(app.pending.is_none());
    }
}
#[test]
fn confirmation_render_is_plain_localized_and_keeps_controls_visible() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = app();
        app.locale = locale;
        app.selected = Some("fixture-b".into());
        app.snapshot.as_mut().unwrap().metadata.profiles[1].name =
            "<b>Private label</b>\u{1b}[31m".into();
        app.prepare(Kind::Connect, None, now);
        let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
        terminal.draw(|f| view::draw(f, &app, now)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(rendered.contains("<b>Private label</b>"));
        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains(locale.text("tui.confirm_keys")));
        assert!(rendered.contains(locale.text("tui.action_close_hint")));
        assert!(!rendered.contains("Missing translation"));
        assert!(!rendered.contains("fixture-b"));
    }
}
#[test]
fn tiny_terminal_cannot_confirm_invisible_action() {
    let (mut app, now) = app();
    app.selected = Some("fixture-b".into());
    app.prepare(Kind::Connect, None, now);
    app.viewport_ready = false;
    assert_eq!(key(&mut app, KeyCode::Enter, now), Action::None);
    assert!(app.pending.is_none());
    assert_eq!(key(&mut app, KeyCode::Char('q'), now), Action::Close);
}
