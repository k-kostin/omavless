// SPDX-License-Identifier: MIT
#![cfg(feature = "private-backup")]
mod support;
use omavless_tui::{
    actions::{Kind, Outcome},
    app::{Action, App, Confirmation, FRESH_FOR, RestoreReadiness},
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::{Actual, ReadError, Status},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use std::time::Instant;

fn response(read: Read, off: bool) -> serde_json::Value {
    let mut value = support::response(read);
    if read == Read::Capabilities {
        value["result"]["methods"] = serde_json::json!([
            "ui.snapshot",
            "runtime.observation",
            "plugin.action",
            "backup.create",
            "backup.preview",
            "backup.restore_previewed"
        ]);
    }
    if off {
        if matches!(read, Read::Snapshot | Read::Observation) {
            value["result"]["desired"]["connected"] = false.into();
            value["result"]["lastKnownActual"] = "disconnected".into();
            value["revision"] = 8.into();
        }
        if read == Read::Snapshot {
            value["result"]["desired"]["profileId"] = "".into();
        }
        if read == Read::Observation {
            value["result"]["facts"] = serde_json::json!({"ownedCoreRunning":false,"visibleMihomoCount":0,"ownedAuxiliaryMihomoCount":0,"visibleTunCount":0,"managedTunCount":0,"ownedControllerConfigVerified":false,"desiredProfileMatchesOwned":false});
        }
    }
    value
}
fn press(app: &mut App, key: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(key, KeyModifiers::NONE), now)
}
fn sample_off(app: &mut App, now: Instant) {
    app.accept(
        load_page(&mut |r| Ok(response(r, true)), Page::Settings),
        now,
    );
}

fn connected(locale: Locale) -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.restore_enabled = true;
    app.backup_enabled = true;
    let snapshot = load_page(&mut |r| Ok(response(r, false)), Page::Settings).unwrap();
    app.accept(Ok(snapshot), now);
    app.key_at(KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE), now);
    (app, now)
}

#[test]
fn readiness_distinguishes_current_blockers_without_sending_requests() {
    let (mut app, now) = connected(Locale::En);
    assert_eq!(
        app.restore_readiness(now),
        RestoreReadiness::DisconnectFirst
    );
    sample_off(&mut app, now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Ready);
    app.restore_enabled = false;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Disabled);
    app.restore_enabled = true;
    app.viewport_ready = false;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Resize);
    app.viewport_ready = true;
    app.unknown = true;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    app.unknown = false;
    app.running = true;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    app.running = false;
    assert_eq!(
        app.restore_readiness(now + FRESH_FOR),
        RestoreReadiness::Stale
    );
    app.snapshot.as_mut().unwrap().capabilities.private_restore = false;
    assert_eq!(
        app.restore_readiness(now),
        RestoreReadiness::CapabilityMissing
    );
    app.snapshot.as_mut().unwrap().capabilities.private_restore = true;
    for actual in [Actual::Starting, Actual::Stopping, Actual::Reconnecting] {
        app.snapshot.as_mut().unwrap().metadata.last_known_actual = actual;
        assert_eq!(app.restore_readiness(now), RestoreReadiness::Transition);
    }
    app.snapshot.as_mut().unwrap().metadata.last_known_actual = Actual::ManualRecoveryRequired;
    app.snapshot
        .as_mut()
        .unwrap()
        .observation
        .manual_recovery_required = true;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Recovery);
    app.snapshot
        .as_mut()
        .unwrap()
        .observation
        .manual_recovery_required = false;
    app.snapshot.as_mut().unwrap().metadata.last_known_actual = Actual::Failed;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unverified);
    app.accept(Err(ReadError::Unavailable), now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Stale);
    assert!(app.pending.is_none() && app.confirmation.is_none());
    assert!(app.take_restore_request().is_none());
}

#[test]
fn presentation_preserves_every_original_restore_admission_boolean() {
    let (mut app, now) = connected(Locale::En);
    for flags in 0u16..512 {
        sample_off(&mut app, now);
        app.restore_enabled = flags & 1 != 0;
        app.viewport_ready = flags & 2 != 0;
        app.running = flags & 4 != 0;
        app.unknown = flags & 8 != 0;
        app.snapshot.as_mut().unwrap().capabilities.private_restore = flags & 16 != 0;
        app.snapshot.as_mut().unwrap().metadata.desired.connected = flags & 32 != 0;
        if flags & 64 != 0 {
            app.snapshot.as_mut().unwrap().metadata.last_known_actual = Actual::Connected;
        }
        if flags & 128 != 0 {
            app.snapshot
                .as_mut()
                .unwrap()
                .observation
                .manual_recovery_required = true;
        }
        let time = if flags & 256 != 0 {
            now + FRESH_FOR
        } else {
            now
        };
        let s = app.snapshot.as_ref().unwrap();
        let original = app.restore_enabled
            && app.viewport_ready
            && app.fresh(time)
            && !app.running
            && !app.unknown
            && app.pending.is_none()
            && s.capabilities.private_restore
            && s.status() == Status::Disconnected
            && !s.metadata.desired.connected;
        assert_eq!(app.restore_available(time), original, "flags={flags}");
    }
}

#[test]
fn disconnect_guidance_is_navigation_to_existing_confirmation_and_cancel_only() {
    let (mut app, now) = connected(Locale::En);
    assert!(app.restore_disconnect_available(now));
    assert_eq!(press(&mut app, KeyCode::Char('R'), now), Action::None);
    assert_eq!(app.notice, "tui.restore_blocked_hint");
    assert!(app.pending.is_none() && app.confirmation.is_none());
    assert_eq!(press(&mut app, KeyCode::Char('d'), now), Action::None);
    let Some(Confirmation::New(command)) = &app.confirmation else {
        panic!("existing disconnect confirmation missing")
    };
    assert!(command.kind == Kind::Disconnect);
    assert!(app.pending.is_none() && app.take_restore_request().is_none());
    press(&mut app, KeyCode::Esc, now);
    assert!(app.confirmation.is_none() && app.pending.is_none());
    assert_eq!(app.status(now), Status::Connected);
    assert!(!app.restore_open && app.restore.is_none());
}

#[test]
fn confirmed_disconnect_needs_new_fresh_off_and_a_separate_restore_key() {
    let (mut app, now) = connected(Locale::Ru);
    press(&mut app, KeyCode::Char('d'), now);
    assert_eq!(
        app.confirm(now, "fixture-disconnect-once".into()),
        Action::Submit
    );
    assert_eq!(
        app.pending.as_ref().unwrap().params()["action"],
        "disconnect"
    );
    assert_eq!(
        app.pending.as_ref().unwrap().params()["instanceId"],
        "fixture-runtime"
    );
    assert_eq!(
        app.pending.as_ref().unwrap().params()["expectedRevision"],
        7
    );
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    app.finish(Outcome::Applied, now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Stale);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.take_restore_request().is_none());
    sample_off(&mut app, now);
    assert!(app.restore_available(now));
    assert!(!app.restore_open && app.restore.is_none());
    assert!(app.take_restore_request().is_none());
    press(&mut app, KeyCode::Char('R'), now);
    assert!(app.restore_open);
    assert!(app.restore.as_ref().unwrap().archive().is_empty());
    assert!(app.restore.as_ref().unwrap().masked().is_empty());
    assert!(app.take_restore_request().is_none());
}

#[test]
fn unknown_disconnect_is_not_cleared_by_off_observation_or_restore_navigation() {
    let (mut app, now) = connected(Locale::En);
    press(&mut app, KeyCode::Char('d'), now);
    app.confirm(now, "fixture-unknown-original".into());
    let original = app.pending.as_ref().unwrap().params();
    app.finish(Outcome::Unknown, now);
    sample_off(&mut app, now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    assert!(!app.restore_disconnect_available(now));
    press(&mut app, KeyCode::Char('R'), now);
    assert!(app.unknown && !app.restore_open);
    assert_eq!(app.pending.as_ref().unwrap().params(), original);
    assert!(app.confirmation.is_none() && app.take_restore_request().is_none());
}

#[test]
fn submitted_backup_waits_but_unknown_backup_is_not_a_retry_hint() {
    let (mut app, now) = connected(Locale::En);
    press(&mut app, KeyCode::Char('b'), now);
    assert!(app.backup_open);
    for text in [
        "/private/public-fixture.ovb",
        "synthetic-backup-secret",
        "synthetic-backup-secret",
    ] {
        for c in text.chars() {
            press(&mut app, KeyCode::Char(c), now);
        }
        press(&mut app, KeyCode::Tab, now);
    }
    press(&mut app, KeyCode::Enter, now);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitBackup);
    let request = app.take_backup_request().unwrap();
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    press(&mut app, KeyCode::Esc, now);
    assert!(!app.restore_disconnect_available(now));
    app.finish_backup(request.settle(Err(ReadError::Unavailable)));
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    sample_off(&mut app, now);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.restore.is_none());
    assert!(app.take_backup_request().is_none() && app.take_restore_request().is_none());
}

#[test]
fn active_and_unknown_jobs_have_distinct_readiness_reasons() {
    let (mut app, now) = connected(Locale::En);
    sample_off(&mut app, now);
    app.snapshot.as_mut().unwrap().capabilities.profile_probe = true;
    let intent = omavless_tui::job_ui::Intent::new(
        app.snapshot.as_ref().unwrap(),
        omavless_tui::jobs::Kind::ProfileProbe,
        None,
    )
    .unwrap();
    let request = intent.request("fixture-existing-job".into()).unwrap();
    app.job = Some(omavless_tui::job_ui::Session::new(intent, request, now));
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    app.job.as_mut().unwrap().tracker.unknown = true;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    assert!(!app.restore_disconnect_available(now));
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.take_restore_request().is_none());
}

#[test]
fn disconnect_guidance_never_handles_input_inside_restore_or_held_keys() {
    use ratatui::crossterm::event::KeyEventKind;
    let (mut app, now) = connected(Locale::En);
    app.key_at(
        KeyEvent::new_with_kind(KeyCode::Char('d'), KeyModifiers::NONE, KeyEventKind::Repeat),
        now,
    );
    assert!(app.confirmation.is_none());
    app.key_at(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT), now);
    assert!(app.confirmation.is_none());
    sample_off(&mut app, now);
    press(&mut app, KeyCode::Char('R'), now);
    press(&mut app, KeyCode::Char('d'), now);
    assert_eq!(app.restore.as_ref().unwrap().archive(), "d");
    assert!(app.confirmation.is_none() && app.pending.is_none());
    assert!(app.take_restore_request().is_none());
}

#[test]
fn current_guidance_refreshes_but_original_outcome_notices_are_not_acknowledged() {
    let (mut app, now) = connected(Locale::En);
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.notice, "tui.restore_blocked_hint");
    sample_off(&mut app, now);
    assert!(app.notice.is_empty());
    app.accept(Err(ReadError::Unavailable), now);
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.notice, "tui.restore_blocked_hint");
    sample_off(&mut app, now);
    assert!(app.notice.is_empty());
    for outcome in [
        "tui.action_unknown",
        "tui.backup_unknown",
        "tui.restore_unknown",
    ] {
        app.notice = outcome;
        sample_off(&mut app, now);
        assert_eq!(app.notice, outcome);
    }
    assert!(!app.restore_open && app.restore.is_none());
    assert!(app.take_restore_request().is_none());
}

#[test]
fn disconnect_hint_does_not_offer_unavailable_or_noncurrent_actions() {
    for case in 0..7 {
        let (mut app, now) = connected(Locale::En);
        let time = if case == 0 { now + FRESH_FOR } else { now };
        match case {
            1 => app.actions_enabled = false,
            2 => app.snapshot.as_mut().unwrap().actions_available = false,
            3 => app.unknown = true,
            4 => {
                app.snapshot
                    .as_mut()
                    .unwrap()
                    .observation
                    .manual_recovery_required = true
            }
            5 => app.snapshot.as_mut().unwrap().capabilities.private_restore = false,
            6 => app.snapshot.as_mut().unwrap().observation.facts = None,
            _ => (),
        }
        assert!(!app.restore_disconnect_available(time), "case={case}");
        app.inspection_scroll = u16::MAX;
        view::clamp_scroll(&mut app, 70, 24, time);
        assert!(
            !joined(&render(&app, time))
                .contains(&joined(app.locale.text("tui.restore_disconnect_hint")))
        );
    }
}
fn render(app: &App, now: Instant) -> String {
    let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
    terminal.draw(|f| view::draw(f, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
fn joined(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && *c != '│')
        .collect()
}

#[test]
fn connected_settings_explains_restore_and_the_existing_explicit_disconnect() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = connected(locale);
        // Settings is scrollable; inspect the lower section, not a hidden helper entry.
        app.key_at(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), now);
        view::clamp_scroll(&mut app, 70, 24, now);
        let text = joined(&render(&app, now));
        assert!(text.contains(&joined(locale.text("tui.restore_disconnect_first"))));
        assert!(text.contains(&joined(locale.text("tui.restore_disconnect_hint"))));
        assert_eq!(
            app.key_at(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE), now),
            Action::None
        );
        assert!(!app.restore_open && app.restore.is_none());
        assert!(app.pending.is_none() && app.confirmation.is_none());
        assert!(app.take_restore_request().is_none());
        let refused = joined(&render(&app, now));
        assert!(refused.contains(&joined(locale.text("tui.restore_blocked_hint"))));
        assert!(refused.contains(&joined(locale.text("tui.action_close_hint"))));
    }
}
