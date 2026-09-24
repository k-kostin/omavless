// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::Kind,
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::ReadError,
    settings::{Language, Settings, Theme},
    theme::Palette,
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::Color,
};
use std::time::Instant;

fn key(app: &mut App, ch: char, now: Instant) -> Action {
    app.key_at(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), now)
}
fn sample(app: &mut App, now: Instant) {
    let mut snapshot = load_page(&mut |r| Ok(support::response(r)), Page::Settings).unwrap();
    snapshot.actions_available = true;
    app.accept(Ok(snapshot), now);
}
fn render(app: &mut App, now: Instant, width: u16, height: u16) -> String {
    view::clamp_scroll(app, width, height, now);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view::draw(f, app, now)).unwrap();
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
fn locale_cycle_reset_and_new_window_follow_startup_policy() {
    for startup in [Locale::En, Locale::Ru, Locale::parse("unknown")] {
        let mut s = Settings::new(startup);
        assert_eq!(s.locale(), startup);
        for (language, locale) in [
            (Language::English, Locale::En),
            (Language::Russian, Locale::Ru),
            (Language::Automatic, startup),
        ] {
            s.next_language();
            assert_eq!(s.language, language);
            assert_eq!(s.locale(), locale);
        }
        s.next_language();
        s.next_theme();
        s.reset();
        assert_eq!(s.locale(), startup);
        assert_eq!(s.theme, Theme::Automatic);
        assert_eq!(Settings::new(startup).language, Language::Automatic);
    }
}

#[test]
fn default_override_survives_reload_and_auto_uses_latest_palette() {
    let mut app = App::new(Locale::En);
    let now = Instant::now();
    let dark = Palette {
        background: Color::Rgb(10, 10, 20),
        ..Palette::default()
    };
    let light = Palette {
        foreground: Color::Black,
        background: Color::White,
        selection: Color::Gray,
        ..Palette::default()
    };
    app.update_palette(dark);
    assert_eq!(app.palette, dark);
    key(&mut app, ',', now);
    key(&mut app, 't', now);
    assert_eq!(app.settings.theme, Theme::Default);
    assert_eq!(app.palette, Palette::default());
    app.update_palette(light);
    assert_eq!(app.palette, Palette::default());
    key(&mut app, 't', now);
    assert_eq!(app.palette, light);
    key(&mut app, 't', now);
    key(&mut app, '0', now);
    assert_eq!(app.palette, light);
    app.update_palette(Palette::default());
    assert_eq!(app.palette, Palette::default());
}

#[test]
fn offline_settings_work_without_requests_and_preserve_read_error() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.accept(Err(ReadError::Unavailable), now);
    for ch in [',', 'l', 'l', 't'] {
        assert_eq!(key(&mut app, ch, now), Action::None);
    }
    assert_eq!(app.locale, Locale::Ru);
    assert_eq!(app.error, Some(ReadError::Unavailable));
    let screen = render(&mut app, now, 100, 32);
    assert!(screen.contains(Locale::Ru.text("tui.settings")));
    assert!(screen.contains(Locale::Ru.text("tui.unavailable")));
    assert!(screen.contains("[l] Язык: Русский"));
    assert!(app.pending.is_none() && app.confirmation.is_none());
}

#[test]
fn presentation_keys_preserve_profile_filters_runtime_and_activity() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.actions_enabled = true;
    sample(&mut app, now);
    app.selected = Some("fixture-b".into());
    app.query = "Frankfurt".into();
    app.favorites_only = true;
    let events = app.activity.newest_first().count();
    let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
    for ch in [
        ',', 'l', 'l', 't', '0', 'c', 'd', 's', '1', '2', '3', 'u', 'a', 'f', '/',
    ] {
        assert_eq!(key(&mut app, ch, now), Action::None);
    }
    assert!(app.page == Page::Settings);
    assert_eq!(app.selected.as_deref(), Some("fixture-b"));
    assert_eq!(app.query, "Frankfurt");
    assert!(app.favorites_only && !app.searching);
    assert!(app.snapshot.as_ref().unwrap().metadata.desired == desired);
    assert_eq!(app.snapshot.as_ref().unwrap().revision, 7);
    assert_eq!(app.sampled_at, Some(now));
    assert_eq!(app.activity.newest_first().count(), events);
    assert!(app.pending.is_none() && app.confirmation.is_none());
}

#[test]
fn settings_do_not_clear_unknown_action_or_replay_it() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.actions_enabled = true;
    sample(&mut app, now);
    app.prepare(Kind::Disconnect, None, now);
    assert_eq!(app.confirm(now, "op-settings".into()), Action::Submit);
    app.finish(omavless_tui::actions::Outcome::Unknown, now);
    let request = app.pending.as_ref().unwrap().params();
    for ch in [',', 'l', 't', '0', 'a', 'u'] {
        assert_eq!(key(&mut app, ch, now), Action::None);
    }
    assert!(app.unknown);
    assert_eq!(app.pending.as_ref().unwrap().params(), request);
    assert_eq!(app.notice, "tui.action_unknown");
    assert!(app.confirmation.is_none());
}

#[test]
fn search_help_and_confirmation_own_keys_before_settings() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.actions_enabled = true;
    sample(&mut app, now);
    key(&mut app, '/', now);
    key(&mut app, ',', now);
    assert_eq!(app.query, ",");
    assert!(app.page == Page::Profiles);
    app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
    app.help = true;
    key(&mut app, ',', now);
    assert!(app.help && app.page == Page::Profiles);
    app.help = false;
    app.prepare(Kind::Disconnect, None, now);
    for ch in [',', 'l', 't', '0'] {
        key(&mut app, ch, now);
    }
    assert!(app.confirmation.is_some() && app.page == Page::Profiles);
    assert_eq!(app.settings.language, Language::Automatic);
    assert_eq!(app.settings.theme, Theme::Automatic);
}

#[test]
fn repeat_modifiers_and_small_viewport_never_change_preferences() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.actions_enabled = true;
    key(&mut app, ',', now);
    for ch in ['l', 't', '0'] {
        for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
            app.key_at(
                KeyEvent::new_with_kind(KeyCode::Char(ch), KeyModifiers::NONE, kind),
                now,
            );
        }
        for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            app.key_at(KeyEvent::new(KeyCode::Char(ch), modifier), now);
        }
    }
    app.viewport_ready = false;
    key(&mut app, 'l', now);
    key(&mut app, 't', now);
    assert_eq!(app.settings.language, Language::Automatic);
    assert_eq!(app.settings.theme, Theme::Automatic);
    assert_eq!(key(&mut app, 'q', now), Action::Close);
}

#[test]
fn settings_page_has_no_extra_ipc_and_navigation_retains_adjacent_pages() {
    let mut reads = Vec::new();
    load_page(
        &mut |r| {
            reads.push(r);
            Ok(support::response(r))
        },
        Page::Settings,
    )
    .unwrap();
    assert_eq!(
        reads,
        vec![
            Read::Hello,
            Read::Capabilities,
            Read::Snapshot,
            Read::Observation
        ]
    );
    assert!(Page::Diagnostics.next(false) == Page::Settings);
    assert!(Page::Settings.next(false) == Page::Activity);
    assert!(Page::Settings.next(true) == Page::Diagnostics);
    assert!(Page::Profiles.next(true) == Page::Subscriptions);
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    key(&mut app, ',', now);
    assert_eq!(
        app.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now),
        Action::Refresh
    );
    assert!(app.page == Page::Profiles);
}

#[test]
fn english_russian_resize_scroll_and_stale_state_keep_preferences_visible() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.actions_enabled = true;
        sample(&mut app, now);
        key(&mut app, ',', now);
        for (width, height) in [(70, 24), (100, 32)] {
            let top = render(&mut app, now + FRESH_FOR, width, height);
            assert!(top.contains(locale.text("tui.settings")));
            assert!(top.contains(locale.text("tui.settings_language")));
            assert!(top.contains(locale.text("tui.action_close_hint")));
            app.key_at(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), now);
            let bottom = render(&mut app, now + FRESH_FOR, width, height);
            assert!(bottom.contains("[0]"));
            assert!(!bottom.contains("Missing translation"));
            for secret in ["fixture-a", "fixture-runtime", "private://"] {
                assert!(!bottom.contains(secret));
            }
            app.key_at(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), now);
        }
    }
}
