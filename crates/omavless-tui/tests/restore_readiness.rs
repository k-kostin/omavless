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
use std::time::{Duration, Instant};

fn response(read: Read, off: bool) -> serde_json::Value {
    let mut value = support::response(read);
    if read == Read::Capabilities {
        value["result"]["methods"] = serde_json::json!([
            "ui.snapshot",
            "runtime.observation",
            "plugin.action",
            "backup.create",
            "backup.preview",
            "backup.restore_previewed",
            "imports.classify"
        ]);
    }
    if off && matches!(read, Read::Snapshot | Read::Observation) {
        value["result"]["desired"]["connected"] = false.into();
        value["result"]["lastKnownActual"] = "disconnected".into();
        value["revision"] = 8.into();
        if read == Read::Snapshot {
            value["result"]["desired"]["profileId"] = "".into();
        } else {
            value["result"]["facts"] = serde_json::json!({
                "ownedCoreRunning":false,"visibleMihomoCount":0,"ownedAuxiliaryMihomoCount":0,
                "visibleTunCount":0,"managedTunCount":0,"ownedControllerConfigVerified":false,
                "desiredProfileMatchesOwned":false
            });
        }
    }
    value
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
    let snapshot = load_page(&mut |read| Ok(response(read, false)), Page::Settings).unwrap();
    app.accept(Ok(snapshot), now);
    press(&mut app, KeyCode::Char(','), now);
    (app, now)
}

fn press(app: &mut App, key: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(key, KeyModifiers::NONE), now)
}

fn render(app: &mut App, now: Instant) -> String {
    view::clamp_scroll(app, 70, 24, now);
    let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
    terminal.draw(|frame| view::draw(frame, app, now)).unwrap();
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
fn connected_settings_explains_restore_without_submitting_or_hiding_exit() {
    for (locale, reason) in [
        (Locale::En, "Restore: disconnect VPN first"),
        (Locale::Ru, "Отключите VPN."),
    ] {
        let (mut app, now) = connected(locale);
        press(&mut app, KeyCode::End, now);
        assert!(joined(&render(&mut app, now)).contains(&joined(reason)));
        assert_eq!(press(&mut app, KeyCode::Char('R'), now), Action::None);
        assert!(!app.restore_open && app.restore.is_none());
        assert!(app.pending.is_none() && app.confirmation.is_none());
        assert!(app.take_restore_request().is_none());
        let blocked = render(&mut app, now);
        assert!(joined(&blocked).contains(&joined(locale.text("tui.action_close_hint"))));
    }
}

#[test]
fn settings_disconnect_opens_existing_confirmation_and_escape_sends_nothing() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = connected(locale);
        assert_eq!(press(&mut app, KeyCode::Char('d'), now), Action::None);
        assert!(
            matches!(&app.confirmation, Some(Confirmation::New(command)) if command.kind == Kind::Disconnect)
        );
        assert!(app.pending.is_none() && app.take_restore_request().is_none());
        assert_eq!(press(&mut app, KeyCode::Esc, now), Action::None);
        assert!(app.confirmation.is_none() && app.pending.is_none());
        assert!(!app.restore_open && app.restore.is_none());
        assert!(app.snapshot.as_ref().unwrap().metadata.desired.connected);
    }
}

#[test]
fn confirmed_disconnect_requires_a_new_off_observation_and_a_separate_restore_key() {
    let (mut app, now) = connected(Locale::Ru);
    press(&mut app, KeyCode::Char('d'), now);
    assert_eq!(
        app.confirm(now, "fixture-disconnect-once".into()),
        Action::Submit
    );
    let params = app.pending.as_ref().unwrap().params();
    assert_eq!(params["action"], "disconnect");
    assert_eq!(params["instanceId"], "fixture-runtime");
    assert_eq!(params["expectedRevision"], 7);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    app.finish(Outcome::Applied, now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Stale);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.take_restore_request().is_none());
    let later = now + Duration::from_millis(1);
    sample_off(&mut app, later);
    assert!(app.restore_available(later));
    assert!(!app.restore_open && app.restore.is_none());
    press(&mut app, KeyCode::Char('R'), later);
    let editor = app.restore.as_ref().unwrap();
    assert!(app.restore_open && editor.archive().is_empty() && editor.masked().is_empty());
    assert!(app.take_restore_request().is_none());
    press(&mut app, KeyCode::Esc, later);
    assert!(!app.restore_open && app.take_restore_request().is_none());
}

#[test]
fn current_blockers_are_distinct_and_never_offer_restore_or_send() {
    let (mut app, now) = connected(Locale::En);
    assert_eq!(
        app.restore_readiness(now),
        RestoreReadiness::DisconnectFirst
    );
    sample_off(&mut app, now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Ready);
    app.import_open = true;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    app.import_open = false;
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
fn classifier_preserves_the_f1eb_restore_predicate_including_open_import() {
    let (mut app, now) = connected(Locale::En);
    for flags in 0u16..1024 {
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
        app.snapshot
            .as_mut()
            .unwrap()
            .observation
            .manual_recovery_required = flags & 128 != 0;
        let time = if flags & 256 != 0 {
            now + FRESH_FOR
        } else {
            now
        };
        app.import_open = flags & 512 != 0;
        let s = app.snapshot.as_ref().unwrap();
        // Literal pre-change f1eb predicate, not the new reason classifier.
        // Pending/unknown Import, Backup, Restore and job owners have separate controls below.
        let original = app.restore_enabled
            && !app.import_open
            && !app.import_unresolved()
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
fn original_unknown_disconnect_and_outcome_notice_survive_off_and_blocked_r() {
    let (mut app, now) = connected(Locale::En);
    press(&mut app, KeyCode::Char('d'), now);
    assert_eq!(
        app.confirm(now, "fixture-unknown-original".into()),
        Action::Submit
    );
    let original = app.pending.as_ref().unwrap().params();
    app.finish(Outcome::Unknown, now);
    sample_off(&mut app, now);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(app.unknown && !app.restore_open && !app.restore_disconnect_available(now));
    assert_eq!(app.notice, "tui.action_unknown");
    assert_eq!(app.pending.as_ref().unwrap().params(), original);
    assert!(app.confirmation.is_none() && app.take_restore_request().is_none());
}

#[test]
fn advice_resolves_without_acknowledging_original_outcome_notices() {
    let (mut app, now) = connected(Locale::En);
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.notice, "tui.restore_blocked_hint");
    sample_off(&mut app, now);
    assert!(app.notice.is_empty());
    for notice in [
        "tui.action_unknown",
        "tui.backup_unknown",
        "tui.restore_unknown",
    ] {
        app.notice = notice;
        sample_off(&mut app, now);
        assert_eq!(app.notice, notice);
        app.accept(Err(ReadError::Unavailable), now);
        press(&mut app, KeyCode::Char('R'), now);
        assert_eq!(app.notice, notice);
    }
}

#[test]
fn import_submission_and_unknown_remain_fenced_without_clearing_private_owner() {
    use omavless_tui::private_import::{State, Workspace};
    let (mut app, now) = connected(Locale::En);
    let mut import = Workspace::new("fixture-runtime", 7, 1).unwrap();
    assert!(import.append("synthetic-only-import-input"));
    let classify = import.classify(now).unwrap();
    import.accept(
        classify.settle(Ok(serde_json::json!({"ok":true,"revision":7,"result":{
            "version":1,"kind":"bundle","counts":{"accepted":1,"duplicates":0,"skipped":0},
            "previewFingerprint":"a".repeat(64)
        }}))),
    );
    assert_eq!(import.state(), State::Preview);
    let original = import
        .confirm("fixture-import-original".into(), now)
        .unwrap();
    app.import = Some(import);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    assert!(!app.restore_available(now) && !app.restore_disconnect_available(now));
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.confirmation.is_none());
    app.import.as_mut().unwrap().worker_lost();
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    sample_off(&mut app, now);
    for key in ['R', 'd'] {
        press(&mut app, KeyCode::Char(key), now);
    }
    assert_eq!(app.import.as_ref().unwrap().state(), State::Unknown);
    assert!(!app.restore_open && app.confirmation.is_none());
    assert!(app.take_restore_request().is_none());
    assert_eq!(original.params()["operationId"], "fixture-import-original");
}

#[test]
fn submitted_backup_waits_and_unknown_backup_never_becomes_a_retry_hint() {
    let (mut app, now) = connected(Locale::En);
    press(&mut app, KeyCode::Char('b'), now);
    assert!(app.backup_open);
    for text in [
        "/private/fixture.ovb",
        "synthetic-only-secret",
        "synthetic-only-secret",
    ] {
        for c in text.chars() {
            press(&mut app, KeyCode::Char(c), now);
        }
        press(&mut app, KeyCode::Tab, now);
    }
    press(&mut app, KeyCode::Enter, now);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitBackup);
    let original = app.take_backup_request().unwrap();
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    press(&mut app, KeyCode::Esc, now);
    assert!(!app.restore_disconnect_available(now));
    app.finish_backup(original.settle(Err(ReadError::Unavailable)));
    sample_off(&mut app, now);
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.restore.is_none());
    assert!(app.take_backup_request().is_none() && app.take_restore_request().is_none());
}

#[test]
fn unknown_restore_reopens_its_original_workspace_and_never_prepares_disconnect() {
    use omavless_tui::private_restore::State;
    let (mut app, now) = connected(Locale::En);
    sample_off(&mut app, now);
    press(&mut app, KeyCode::Char('R'), now);
    for c in "/private/fixture.ovb".chars() {
        press(&mut app, KeyCode::Char(c), now);
    }
    press(&mut app, KeyCode::Tab, now);
    for c in "synthetic-only-secret".chars() {
        press(&mut app, KeyCode::Char(c), now);
    }
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitRestore);
    let preview = app.take_restore_request().unwrap();
    app.finish_restore(
        preview.settle(Ok(serde_json::json!({"ok":true,"revision":8,"result":{
            "profiles":1,"subscriptions":0,"scope":"privatePair","ciphertextDigest":"a".repeat(64)
        }}))),
    );
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Confirming);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitRestore);
    let original = app.take_restore_request().unwrap();
    app.finish_restore(original.settle(Err(ReadError::Unavailable)));
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Unknown);
    press(&mut app, KeyCode::Esc, now);
    app.accept(
        load_page(&mut |r| Ok(response(r, false)), Page::Settings),
        now,
    );
    assert!(!app.restore_disconnect_available(now));
    press(&mut app, KeyCode::Char('d'), now);
    assert!(app.confirmation.is_none() && app.pending.is_none());
    press(&mut app, KeyCode::Char('R'), now);
    assert!(app.restore_open);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Unknown);
    assert!(app.take_restore_request().is_none());
}

#[test]
fn active_and_unknown_job_owners_keep_restore_blocked() {
    let (mut app, now) = connected(Locale::En);
    sample_off(&mut app, now);
    app.snapshot.as_mut().unwrap().capabilities.profile_probe = true;
    let intent = omavless_tui::job_ui::Intent::new(
        app.snapshot.as_ref().unwrap(),
        omavless_tui::jobs::Kind::ProfileProbe,
        None,
    )
    .unwrap();
    let original = intent.request("fixture-existing-job".into()).unwrap();
    app.job = Some(omavless_tui::job_ui::Session::new(intent, original, now));
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Busy);
    app.job.as_mut().unwrap().tracker.unknown = true;
    assert_eq!(app.restore_readiness(now), RestoreReadiness::Unresolved);
    assert!(!app.restore_available(now) && !app.restore_disconnect_available(now));
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open && app.take_restore_request().is_none());
    assert!(app.job.as_ref().unwrap().tracker.unknown);
}

#[test]
fn disconnect_guidance_respects_current_facts_capabilities_and_action_admission() {
    for case in 0..8 {
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
            7 => app.restore_enabled = false,
            _ => (),
        }
        assert!(!app.restore_disconnect_available(time), "case={case}");
        app.inspection_scroll = u16::MAX;
        assert!(
            !joined(&render(&mut app, time))
                .contains(&joined(app.locale.text("tui.restore_disconnect_hint")))
        );
        press(&mut app, KeyCode::Char('d'), time);
        assert!(app.confirmation.is_none() && app.pending.is_none());
    }
}

#[test]
fn held_modified_and_modal_input_cannot_open_disconnect_confirmation() {
    use ratatui::crossterm::event::KeyEventKind;
    let (mut app, now) = connected(Locale::En);
    for event in [
        KeyEvent::new_with_kind(KeyCode::Char('d'), KeyModifiers::NONE, KeyEventKind::Repeat),
        KeyEvent::new_with_kind(
            KeyCode::Char('d'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT),
    ] {
        assert_eq!(app.key_at(event, now), Action::None);
        assert!(app.confirmation.is_none() && app.pending.is_none());
    }
    app.help = true;
    press(&mut app, KeyCode::Char('d'), now);
    assert!(app.help && app.confirmation.is_none());
    app.help = false;
    app.page = Page::Traffic;
    press(&mut app, KeyCode::Char('d'), now);
    assert!(app.confirmation.is_none() && app.pending.is_none());
    app.page = Page::Settings;
    sample_off(&mut app, now);
    press(&mut app, KeyCode::Char('R'), now);
    press(&mut app, KeyCode::Char('d'), now);
    assert_eq!(app.restore.as_ref().unwrap().archive(), "d");
    assert!(app.confirmation.is_none() && app.pending.is_none());
    assert!(app.take_restore_request().is_none());
}
