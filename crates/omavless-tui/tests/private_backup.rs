// SPDX-License-Identifier: MIT
#![cfg(feature = "private-backup")]
mod support;
use omavless_tui::{
    app::{Action, App},
    client::load_page,
    i18n::Locale,
    inspection::Page,
    model::ReadError,
    private_backup::State,
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::Instant;

fn press(app: &mut App, code: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(code, KeyModifiers::NONE), now)
}
fn sample(app: &mut App, now: Instant) {
    let mut s = load_page(&mut |r| Ok(support::response(r)), Page::Settings).unwrap();
    s.capabilities.private_backup = true;
    app.accept(Ok(s), now);
}
fn open(locale: Locale) -> (App, Instant) {
    let mut app = App::new(locale);
    let now = Instant::now();
    app.actions_enabled = true;
    app.backup_enabled = true;
    sample(&mut app, now);
    press(&mut app, KeyCode::Char(','), now);
    press(&mut app, KeyCode::Char('b'), now);
    assert!(app.backup_open);
    (app, now)
}
fn fill(app: &mut App, now: Instant) {
    for ch in "/private/public-fixture.ovb".chars() {
        press(app, KeyCode::Char(ch), now);
    }
    press(app, KeyCode::Tab, now);
    for ch in "synthetic-backup-secret".chars() {
        press(app, KeyCode::Char(ch), now);
    }
    press(app, KeyCode::Tab, now);
    for ch in "synthetic-backup-secret".chars() {
        press(app, KeyCode::Char(ch), now);
    }
}
fn render(app: &App, now: Instant, width: u16, height: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| view::draw(f, app, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(width as usize)
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
fn default_invocation_no_adapter_or_missing_capability_never_opens() {
    let mut app = App::new(Locale::En);
    let now = Instant::now();
    sample(&mut app, now);
    press(&mut app, KeyCode::Char(','), now);
    press(&mut app, KeyCode::Char('b'), now);
    assert!(!app.backup_open);
    assert!(app.take_backup_request().is_none());
    app.backup_enabled = true;
    app.snapshot.as_mut().unwrap().capabilities.private_backup = false;
    press(&mut app, KeyCode::Char('b'), now);
    assert!(!app.backup_open);
}
#[test]
fn cancel_and_focus_are_local_and_masked_in_both_locales() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = open(locale);
        fill(&mut app, now);
        let text = render(&app, now, 70, 24);
        assert!(text.contains("********"));
        assert!(!text.contains("synthetic-backup-secret"));
        assert!(text.contains("/private/public-fixture.ovb"));
        press(&mut app, KeyCode::BackTab, now);
        assert_eq!(
            app.backup.as_ref().unwrap().field(),
            omavless_tui::private_backup::Field::Passphrase
        );
        press(&mut app, KeyCode::Esc, now);
        assert!(!app.backup_open);
        assert_eq!(app.backup.as_ref().unwrap().state(), State::Cancelled);
        assert!(app.backup.as_ref().unwrap().destination().is_empty());
        assert!(app.take_backup_request().is_none());
    }
}
#[test]
fn full_target_warning_confirmation_and_submit_once_are_rendered() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = open(locale);
        fill(&mut app, now);
        assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
        assert_eq!(app.backup.as_ref().unwrap().state(), State::Confirming);
        let text = render(&app, now, 70, 24);
        for key in [
            "tui.backup_private",
            "tui.backup_loss",
            "tui.backup_confirm",
        ] {
            assert!(joined(&text).contains(&joined(locale.text(key))), "{key}");
        }
        assert!(text.contains("/private/public-fixture.ovb"));
        assert!(!text.contains("synthetic-backup-secret"));
        assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitBackup);
        assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
        assert!(app.take_backup_request().is_some());
        assert!(app.take_backup_request().is_none());
    }
}
#[test]
fn submitted_and_unknown_survive_settings_leave_reentry_without_new_id() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now);
    press(&mut app, KeyCode::Enter, now);
    press(&mut app, KeyCode::Enter, now);
    let request = app.take_backup_request().unwrap();
    let original = request.params();
    press(&mut app, KeyCode::Esc, now);
    assert!(!app.backup_open);
    assert_eq!(press(&mut app, KeyCode::Tab, now), Action::Refresh);
    assert_eq!(press(&mut app, KeyCode::Char('r'), now), Action::Refresh);
    press(&mut app, KeyCode::Char(','), now);
    assert_eq!(press(&mut app, KeyCode::Char('c'), now), Action::None);
    press(&mut app, KeyCode::Char('b'), now);
    assert!(app.backup_open);
    assert_eq!(app.backup.as_ref().unwrap().state(), State::Submitted);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    app.finish_backup(request.settle(Err(ReadError::Unavailable)));
    press(&mut app, KeyCode::Esc, now);
    press(&mut app, KeyCode::Char('b'), now);
    assert_eq!(app.backup.as_ref().unwrap().state(), State::Unknown);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    assert!(app.take_backup_request().is_none());
    let text = render(&app, now, 70, 24);
    assert!(joined(&text).contains(&joined(Locale::En.text("tui.backup_unknown"))));
    assert_eq!(original["archive"], "/private/public-fixture.ovb");
}
#[test]
fn changed_metadata_and_narrow_viewport_cannot_submit() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now);
    press(&mut app, KeyCode::Enter, now);
    app.snapshot.as_mut().unwrap().revision += 1;
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    assert_eq!(app.backup.as_ref().unwrap().state(), State::Changed);
    assert!(app.take_backup_request().is_none());
    let (mut app, now) = open(Locale::Ru);
    fill(&mut app, now);
    app.viewport_ready = false;
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
    let text = render(&app, now, 50, 14);
    assert!(!text.contains("synthetic-backup-secret"));
    assert!(joined(&text).contains(&joined(Locale::Ru.text("tui.action_resize"))));
}
#[test]
fn original_worker_completion_survives_hidden_screen_not_a_backend_cancel() {
    let (mut app, now) = open(Locale::En);
    fill(&mut app, now);
    press(&mut app, KeyCode::Enter, now);
    press(&mut app, KeyCode::Enter, now);
    let r = app.take_backup_request().unwrap();
    let revision = r.params()["expectedRevision"].as_u64().unwrap();
    press(&mut app, KeyCode::Esc, now);
    app.finish_backup(r.settle(Ok(json!({"ok":true,"revision":revision,"result":{"completed":true,"replayed":false,"scope":"privatePair"}}))));
    assert!(!app.backup_open);
    assert_eq!(app.backup.as_ref().unwrap().state(), State::Completed);
    assert!(app.take_backup_request().is_none());
}
