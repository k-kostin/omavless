// SPDX-License-Identifier: MIT
#![cfg(feature = "private-backup")]
mod support;
use omavless_tui::{
    app::{Action, App},
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::ReadError,
    private_restore::State,
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
    let mut v = support::response(read);
    match read {
        Read::Snapshot => {
            v["result"]["desired"]["connected"] = false.into();
            v["result"]["desired"]["profileId"] = "".into();
            v["result"]["lastKnownActual"] = "disconnected".into();
        }
        Read::Observation => {
            v["result"]["desired"]["connected"] = false.into();
            v["result"]["lastKnownActual"] = "disconnected".into();
            v["result"]["facts"] = json!({"ownedCoreRunning":false,"visibleMihomoCount":0,"ownedAuxiliaryMihomoCount":0,"visibleTunCount":0,"managedTunCount":0,"ownedControllerConfigVerified":false,"desiredProfileMatchesOwned":false});
        }
        _ => (),
    }
    v
}
fn sample(app: &mut App, now: Instant) {
    let mut s = load_page(&mut |r| Ok(response(r)), Page::Settings).unwrap();
    s.capabilities.private_restore = true;
    s.capabilities.private_backup = true;
    app.accept(Ok(s), now);
}
fn press(app: &mut App, key: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(key, KeyModifiers::NONE), now)
}
fn open(locale: Locale) -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.restore_enabled = true;
    app.backup_enabled = true;
    sample(&mut app, now);
    press(&mut app, KeyCode::Char(','), now);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(app.restore_open);
    (app, now)
}
fn fill(app: &mut App, now: Instant, path: &str) {
    for ch in path.chars() {
        press(app, KeyCode::Char(ch), now);
    }
    press(app, KeyCode::Tab, now);
    for ch in "synthetic-only-secret".chars() {
        press(app, KeyCode::Char(ch), now);
    }
}
fn preview(app: &mut App, now: Instant) {
    assert_eq!(press(app, KeyCode::Enter, now), Action::SubmitRestore);
    let r = app.take_restore_request().unwrap();
    assert_eq!(r.method(), "backup.preview");
    app.finish_restore(r.settle(Ok(json!({"ok":true,"revision":7,"result":{"profiles":3,"subscriptions":1,"scope":"privatePair","ciphertextDigest":"a".repeat(64)}}))));
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Confirming);
}
fn render(app: &App, now: Instant, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view::draw(f, app, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(w as usize)
        .map(|r| r.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn joined(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && *c != '│')
        .collect()
}
#[test]
fn absent_selector_capability_connected_and_narrow_never_send() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    sample(&mut app, now);
    press(&mut app, KeyCode::Char(','), now);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open);
    app.snapshot.as_mut().unwrap().capabilities.private_restore = true;
    app.restore_enabled = true;
    app.snapshot.as_mut().unwrap().metadata.desired.connected = true;
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open);
    app.restore_enabled = true;
    app.snapshot.as_mut().unwrap().metadata.desired.connected = false;
    app.snapshot.as_mut().unwrap().capabilities.private_restore = false;
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open);
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now, "/private/public-fixture.ovb");
    app.viewport_ready = false;
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    assert!(app.take_restore_request().is_none());
    let text = render(&app, now, 50, 14);
    assert!(!text.contains("synthetic-only-secret"));
    assert!(joined(&text).contains(&joined(Locale::En.text("tui.action_resize"))));
}

#[test]
fn restore_capability_needs_both_exact_methods_not_one_or_other_backup() {
    use omavless_tui::inspection::Capabilities;
    for methods in [
        vec![],
        vec![json!("backup.preview")],
        vec![json!("backup.restore_previewed")],
        vec![json!("backup.create"), json!("backup.restore")],
    ] {
        assert!(!Capabilities::parse(&methods).private_restore);
    }
    assert!(
        Capabilities::parse(&[json!("backup.preview"), json!("backup.restore_previewed")])
            .private_restore
    );
}
#[test]
fn bilingual_form_confirmation_full_target_warning_and_one_submit() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = open(locale);
        fill(&mut app, now, "/private/public-fixture.ovb");
        let text = render(&app, now, 70, 24);
        assert!(text.contains("********"));
        assert!(!text.contains("synthetic-only-secret"));
        preview(&mut app, now);
        let text = render(&app, now, 70, 24);
        for key in [
            "tui.restore_replace",
            "tui.restore_confirm",
            "tui.restore_return",
            "tui.restore_preview_data",
        ] {
            assert!(joined(&text).contains(&joined(locale.text(key))), "{key}");
        }
        assert!(text.contains("/private/public-fixture.ovb"));
        assert!(!text.contains("synthetic-only-secret"));
        assert!(!text.contains(&"a".repeat(64)));
        assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitRestore);
        assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
        let r = app.take_restore_request().unwrap();
        assert_eq!(r.params()["expectedCiphertextDigest"], "a".repeat(64));
        assert!(app.take_restore_request().is_none());
    }
}
#[test]
fn cancelled_confirm_and_changed_preview_context_never_replace() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now, "/private/public-fixture.ovb");
    preview(&mut app, now);
    press(&mut app, KeyCode::Esc, now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Cancelled);
    assert!(app.restore.as_ref().unwrap().archive().is_empty());
    assert!(app.take_restore_request().is_none());
    let (mut app, now) = open(Locale::Ru);
    fill(&mut app, now, "/private/public-fixture.ovb");
    preview(&mut app, now);
    app.snapshot.as_mut().unwrap().revision += 1;
    press(&mut app, KeyCode::Enter, now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Changed);
    assert!(app.take_restore_request().is_none());
}
#[test]
fn hidden_submitted_unknown_blocks_backup_mutations_and_full_quit_without_new_id() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now, "/private/public-fixture.ovb");
    preview(&mut app, now);
    press(&mut app, KeyCode::Enter, now);
    let r = app.take_restore_request().unwrap();
    let id = r.params()["operationId"].clone();
    press(&mut app, KeyCode::Esc, now);
    assert!(!app.restore_open);
    for key in ['b', 'c', 'd', 'Q', 'a', 'u'] {
        assert_eq!(press(&mut app, KeyCode::Char(key), now), Action::None);
    }
    assert!(app.take_backup_request().is_none());
    assert!(app.pending.is_none() && app.confirmation.is_none());
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Submitted);
    app.finish_restore(r.settle(Err(ReadError::Unavailable)));
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Unknown);
    press(&mut app, KeyCode::Esc, now);
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Unknown);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    assert!(app.take_restore_request().is_none());
    assert!(id.as_str().is_some());
}
#[test]
fn unavailable_snapshot_while_pending_does_not_discard_original_success() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now, "/private/public-fixture.ovb");
    preview(&mut app, now);
    press(&mut app, KeyCode::Enter, now);
    let r = app.take_restore_request().unwrap();
    app.accept(Err(ReadError::Unavailable), now);
    press(&mut app, KeyCode::Enter, now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Submitted);
    app.finish_restore(r.settle(Ok(json!({"ok":true,"revision":8,"result":{"completed":true,"replayed":false,"scope":"privatePair"}}))));
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Completed);
}
#[test]
fn known_other_parsed_header_permanently_rejects_late_positive() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now, "/private/public-fixture.ovb");
    preview(&mut app, now);
    press(&mut app, KeyCode::Enter, now);
    let r = app.take_restore_request().unwrap();
    let other = load_page(
        &mut |read| {
            let mut v = response(read);
            if matches!(read, Read::Hello | Read::Snapshot | Read::Observation) {
                v["result"]["instanceId"] = "known-other-daemon".into();
            }
            Ok(v)
        },
        Page::Settings,
    )
    .unwrap();
    app.accept(Ok(other), now); // NO intervening key, refresh or tick
    app.finish_restore(r.settle(Ok(json!({"ok":true,"revision":8,"result":{"completed":true,"replayed":false,"scope":"privatePair"}}))));
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Unknown);
}

#[test]
fn cancel_preview_then_reentry_waits_for_exact_original_without_spawning_another() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now, "/private/public-fixture.ovb");
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitRestore);
    let original = app.take_restore_request().unwrap();
    press(&mut app, KeyCode::Esc, now);
    assert!(!app.restore_open);
    for key in ['b', 'c', 'Q'] {
        assert_eq!(press(&mut app, KeyCode::Char(key), now), Action::None);
    }
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Cancelled);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    assert!(app.take_restore_request().is_none());
    assert!(app.take_backup_request().is_none());
    app.finish_restore(original.settle(Ok(json!({"ok":true,"revision":7,"result":{"profiles":3,"subscriptions":1,"scope":"privatePair","ciphertextDigest":"a".repeat(64)}}))));
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Cancelled);
    assert!(app.restore.as_ref().unwrap().counts().is_none());
    press(&mut app, KeyCode::Esc, now);
    press(&mut app, KeyCode::Char('R'), now);
    assert_eq!(app.restore.as_ref().unwrap().state(), State::Editing);
}
#[test]
fn backup_unresolved_blocks_restore_and_original_backup_only_selector_keeps_layout() {
    let (mut app, now) = open(Locale::En);
    press(&mut app, KeyCode::Esc, now);
    press(&mut app, KeyCode::Char('b'), now);
    assert!(app.backup_open);
    fill(&mut app, now, "/private/public-backup.ovb");
    press(&mut app, KeyCode::Tab, now);
    for ch in "synthetic-only-secret".chars() {
        press(&mut app, KeyCode::Char(ch), now);
    }
    press(&mut app, KeyCode::Enter, now);
    press(&mut app, KeyCode::Enter, now);
    assert!(app.take_backup_request().is_some());
    press(&mut app, KeyCode::Esc, now);
    press(&mut app, KeyCode::Char('R'), now);
    assert!(!app.restore_open);
    let mut app = App::new(Locale::En);
    app.backup_enabled = true;
    sample(&mut app, now);
    press(&mut app, KeyCode::Char(','), now);
    let text = render(&app, now, 100, 32);
    assert!(!text.contains("EXPERIMENTAL:"));
    assert!(text.contains(Locale::En.text("tui.backup_entry")));
}

#[test]
fn maximum_path_and_primary_footer_keys_are_visible_at_70x24_in_both_locales() {
    let path = format!("/private/{}.ovb", "x".repeat(147));
    assert_eq!(path.len(), 160);
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = open(locale);
        fill(&mut app, now, &path);
        preview(&mut app, now);
        let text = render(&app, now, 70, 24);
        assert!(joined(&text).contains(&path));
        for key in [
            "tui.restore_replace",
            "tui.restore_off",
            "tui.restore_confirm",
            "tui.restore_return",
        ] {
            assert!(joined(&text).contains(&joined(locale.text(key))), "{key}");
        }
        press(&mut app, KeyCode::Esc, now);
        let text = render(&app, now, 70, 24);
        assert!(joined(&text).contains(&joined(locale.text("tui.settings_restore_keys"))));
        press(&mut app, KeyCode::End, now);
        view::clamp_scroll(&mut app, 70, 24, now);
        let text = render(&app, now, 70, 24);
        assert!(joined(&text).contains(&joined(locale.text("tui.restore_entry"))));
    }
}
