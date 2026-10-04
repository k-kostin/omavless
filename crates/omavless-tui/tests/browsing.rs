// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::Kind,
    app::{Action, App},
    browsing::{self, Row},
    client::load,
    i18n::Locale,
    model::{Profile, Subscription},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use std::time::Instant;
fn app() -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.accept(load(&mut |r| Ok(support::response(r))), now);
    (app, now)
}
fn key(app: &mut App, code: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(code, KeyModifiers::NONE), now)
}
fn render(app: &App, now: Instant) -> String {
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| view::draw(f, app, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(100)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn grouped_navigation_preserves_feed_order_and_record_identity() {
    let (mut a, now) = app();
    let s = a.snapshot.as_mut().unwrap();
    s.metadata.subscriptions.push(Subscription {
        id: "second-sub".into(),
        name: "Second subscription".into(),
        updated_at: None,
    });
    s.metadata.profiles.insert(
        0,
        Profile {
            id: "second-node".into(),
            name: "Duplicate display name".into(),
            subscription_id: Some("second-sub".into()),
            missing: false,
            favorite: false,
        },
    );
    s.metadata.profiles.push(Profile {
        id: "another-local".into(),
        name: "Duplicate display name".into(),
        subscription_id: None,
        missing: false,
        favorite: false,
    });
    assert_eq!(a.visible(), vec![1, 3, 2, 0]);
    let rows = browsing::rows(a.snapshot.as_ref().unwrap(), &a.visible());
    assert_eq!(rows.len(), 7);
    assert!(matches!(
        &rows[0],
        Row::Group {
            name: None,
            count: 2
        }
    ));
    assert!(matches!(&rows[3],Row::Group{name:Some(n),count:1} if n=="Fixture subscription"));
    for expected in ["fixture-a", "another-local", "fixture-b", "second-node"] {
        assert_eq!(key(&mut a, KeyCode::Down, now), Action::None);
        assert_eq!(a.selected.as_deref(), Some(expected));
        assert!(a.pending.is_none());
    }
}
#[test]
fn favorites_filters_without_mutating_flags_and_escape_restores_all() {
    let (mut a, now) = app();
    key(&mut a, KeyCode::Down, now);
    key(&mut a, KeyCode::Char('f'), now);
    assert!(a.favorites_only);
    assert!(a.selected.is_none());
    assert_eq!(a.visible(), vec![0]);
    assert!(a.snapshot.as_ref().unwrap().metadata.profiles[0].favorite);
    assert!(!a.snapshot.as_ref().unwrap().metadata.profiles[1].favorite);
    key(&mut a, KeyCode::Esc, now);
    assert!(!a.favorites_only);
    assert_eq!(a.visible(), vec![0, 1]);
    assert!(a.pending.is_none());
}
#[test]
fn search_matches_subscription_and_combines_with_favorites() {
    let (mut a, _) = app();
    a.query = "FIXTURE SUBSCRIPTION".into();
    assert_eq!(a.visible(), vec![1]);
    a.favorites_only = true;
    assert!(a.visible().is_empty());
    a.snapshot.as_mut().unwrap().metadata.profiles[1].favorite = true;
    assert_eq!(a.visible(), vec![1]);
}
#[test]
fn search_letter_f_is_text_and_confirmation_cannot_change_filter() {
    let (mut a, now) = app();
    key(&mut a, KeyCode::Char('/'), now);
    key(&mut a, KeyCode::Char('f'), now);
    assert_eq!(a.query, "f");
    assert!(!a.favorites_only);
    key(&mut a, KeyCode::Esc, now);
    a.query.clear();
    a.actions_enabled = true;
    a.snapshot.as_mut().unwrap().actions_available = true;
    a.selected = Some("fixture-b".into());
    a.prepare(Kind::Connect, None, now);
    key(&mut a, KeyCode::Char('f'), now);
    assert!(!a.favorites_only);
    assert!(a.confirmation.is_some());
}
#[test]
fn connected_identity_remains_visible_outside_favorites() {
    let (mut a, now) = app();
    a.snapshot.as_mut().unwrap().metadata.profiles[0].favorite = false;
    a.snapshot.as_mut().unwrap().metadata.profiles[1].favorite = true;
    a.favorites_only = true;
    let text = render(&a, now);
    assert!(text.contains("Connected profile: Fixture Helsinki"));
    assert!(text.contains("Favorites (1)"));
    assert!(text.contains("Fixture subscription (1)"));
    assert!(!text.contains("Local profiles ("));
    assert!(!text.contains("fixture-sub"));
}
#[test]
fn external_unfavorite_invalidates_hidden_confirmed_target() {
    let (mut a, now) = app();
    a.actions_enabled = true;
    a.snapshot.as_mut().unwrap().actions_available = true;
    a.favorites_only = true;
    a.selected = Some("fixture-a".into());
    a.prepare(Kind::Connect, None, now);
    let mut next = a.snapshot.clone().unwrap();
    next.metadata.profiles[0].favorite = false;
    // Even an inconsistent same-revision projection cannot authorize a hidden target.
    a.accept(Ok(next), now);
    assert!(a.selected.is_none());
    assert_eq!(a.confirm(now, "never-submit".into()), Action::None);
    assert!(a.pending.is_none());
}
#[test]
fn empty_favorites_and_russian_source_are_truthful_plain_text() {
    let (mut a, now) = app();
    a.locale = Locale::Ru;
    a.favorites_only = true;
    for p in &mut a.snapshot.as_mut().unwrap().metadata.profiles {
        p.favorite = false;
    }
    let text = render(&a, now);
    assert!(text.contains(a.locale.text("tui.no_favorites")));
    a.favorites_only = false;
    a.snapshot.as_mut().unwrap().metadata.subscriptions[0].name =
        "<b>Подписка</b>\u{1b}[31m".into();
    a.query = "подписка".into();
    let text = render(&a, now);
    assert!(text.contains("<b>Подписка</b>"));
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains("Missing translation"));
}
#[test]
fn no_subscription_headers_are_selectable_or_connected() {
    let (mut a, now) = app();
    key(&mut a, KeyCode::End, now);
    assert_eq!(a.selected.as_deref(), Some("fixture-b"));
    key(&mut a, KeyCode::Up, now);
    assert_eq!(a.selected.as_deref(), Some("fixture-a"));
    assert_eq!(key(&mut a, KeyCode::Enter, now), Action::None);
    assert!(a.pending.is_none());
    let text = render(&a, now);
    let group = text
        .lines()
        .find(|l| l.contains("Fixture subscription (1)"))
        .unwrap();
    assert!(!group.contains("Connected"));
    assert!(!group.contains('>'));
}
