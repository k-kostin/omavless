// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::{Kind, Outcome},
    activity::{Activity, CAPACITY, Event},
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::{Mode, ReadError, Snapshot, Status},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use std::time::{Duration, Instant};

fn snapshot() -> Snapshot {
    let mut s = load_page(&mut |r| Ok(support::response(r)), Page::Activity).unwrap();
    s.actions_available = true;
    s
}
fn key(a: &mut App, code: KeyCode, now: Instant) -> Action {
    a.key_at(KeyEvent::new(code, KeyModifiers::NONE), now)
}
fn render(a: &App, now: Instant, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view::draw(f, a, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(w as usize)
        .map(|r| r.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn unchanged_polling_and_repeated_read_errors_do_not_flood_history() {
    let mut a = App::new(Locale::En);
    let now = Instant::now();
    for _ in 0..100 {
        a.accept(Ok(snapshot()), now);
    }
    assert_eq!(a.activity.newest_first().count(), 2);
    for _ in 0..100 {
        a.accept(Err(ReadError::Unavailable), now);
    }
    assert_eq!(a.activity.newest_first().count(), 3);
    a.accept(Ok(snapshot()), now);
    assert_eq!(a.activity.newest_first().count(), 5);
    assert!(
        a.activity
            .newest_first()
            .any(|e| e.event == Event::Observed(Status::Connected))
    );
}

#[test]
fn ring_is_bounded_newest_first_and_times_never_regress() {
    let mut history = Activity::default();
    let now = Instant::now();
    for n in 0..100 {
        history.record(Event::Applied, now + Duration::from_secs(n));
    }
    assert_eq!(history.newest_first().count(), CAPACITY);
    let elapsed = history.newest_first().next().unwrap().elapsed_seconds;
    history.record(Event::Rejected, now);
    assert_eq!(
        history.newest_first().next().unwrap().elapsed_seconds,
        elapsed
    );
    assert!(history.newest_first().next().unwrap().event == Event::Rejected);
    assert_eq!(Activity::default().newest_first().count(), 0);
}

#[test]
fn instance_mode_and_local_health_changes_record_only_semantic_events() {
    let mut a = App::new(Locale::En);
    let now = Instant::now();
    a.accept(Ok(snapshot()), now);
    let mut changed = snapshot();
    changed.metadata.instance_id = "private-new-instance".into();
    changed.metadata.desired.mode = Mode::Direct;
    changed.observation.facts = None;
    a.accept(Ok(changed), now);
    for event in [
        Event::RuntimeChanged,
        Event::ModeObserved(Mode::Direct),
        Event::Observed(Status::Unverified),
    ] {
        assert!(a.activity.newest_first().any(|e| e.event == event));
    }
}

#[test]
fn confirmation_cancel_is_not_submission_and_outcomes_never_retain_error_text() {
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    let now = Instant::now();
    a.accept(Ok(snapshot()), now);
    a.prepare(Kind::Disconnect, None, now);
    key(&mut a, KeyCode::Esc, now);
    assert_eq!(a.activity.newest_first().count(), 2);
    a.prepare(Kind::Disconnect, None, now);
    assert_eq!(
        a.confirm(now, "private-operation-id".into()),
        Action::Submit
    );
    assert!(a.activity.newest_first().next().unwrap().event == Event::Submitted(Kind::Disconnect));
    a.finish(Outcome::Rejected("PRIVATE raw backend error"), now);
    assert!(a.activity.newest_first().next().unwrap().event == Event::Rejected);
    a.page = Page::Activity;
    let screen = render(&a, now, 120, 32);
    assert!(!screen.contains("PRIVATE") && !screen.contains("private-operation-id"));
    assert!(!screen.contains("Fixture Frankfurt"));
    assert!(screen.contains("Runtime rejected command"));
}

#[test]
fn unknown_retry_remains_unknown_and_acknowledgement_does_not_rewrite_history() {
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    let now = Instant::now();
    a.accept(Ok(snapshot()), now);
    a.prepare(Kind::Disconnect, None, now);
    a.confirm(now, "synthetic-operation".into());
    a.finish(Outcome::Unknown, now);
    key(&mut a, KeyCode::Char('u'), now);
    assert_eq!(a.confirm(now, String::new()), Action::Submit);
    assert!(a.activity.newest_first().next().unwrap().event == Event::Retried);
    a.finish(Outcome::Rejected("tui.action_rejected"), now);
    assert!(a.activity.newest_first().next().unwrap().event == Event::Unknown);
    a.accept(Ok(snapshot()), now);
    key(&mut a, KeyCode::Char('a'), now);
    a.confirm(now, String::new());
    assert!(a.activity.newest_first().next().unwrap().event == Event::Acknowledged);
    assert!(a.activity.newest_first().any(|e| e.event == Event::Unknown));
}

#[test]
fn page_has_no_extra_reads_no_hidden_actions_and_preserves_selection() {
    let now = Instant::now();
    let mut calls = Vec::new();
    let s = load_page(
        &mut |r| {
            calls.push(r);
            Ok(support::response(r))
        },
        Page::Activity,
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
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    a.accept(Ok(s), now);
    a.selected = Some("fixture-b".into());
    a.page = Page::Activity;
    for ch in ['c', 'd', 's', '1', '2', '3', 'u', 'a'] {
        assert_eq!(key(&mut a, KeyCode::Char(ch), now), Action::None);
    }
    assert!(a.confirmation.is_none() && a.pending.is_none());
    assert_eq!(a.selected.as_deref(), Some("fixture-b"));
    assert_eq!(key(&mut a, KeyCode::Esc, now), Action::Refresh);
    assert!(a.page == Page::Profiles);
    assert_eq!(key(&mut a, KeyCode::Char('q'), now), Action::Close);
}

#[test]
fn stale_or_unavailable_runtime_keeps_history_with_separate_live_header() {
    for locale in [Locale::En, Locale::Ru] {
        let now = Instant::now();
        let mut a = App::new(locale);
        a.page = Page::Activity;
        a.accept(Ok(snapshot()), now);
        let stale = render(&a, now + FRESH_FOR, 100, 32);
        assert!(stale.contains(locale.text("tui.stale")));
        assert!(stale.contains(locale.text("tui.event_connected")));
        a.accept(Err(ReadError::Unavailable), now + FRESH_FOR);
        let unavailable = render(&a, now + FRESH_FOR, 100, 32);
        assert!(unavailable.contains(locale.text("tui.event_unavailable")));
        assert!(unavailable.contains(locale.text("tui.event_connected")));
        assert!(unavailable.contains(locale.text("tui.activity")));
        assert!(render(&a, now, 50, 14).contains(locale.text("tui.close_hint")));
    }
}

#[test]
fn rejected_old_reads_do_not_add_fictitious_post_command_history() {
    let now = Instant::now();
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    a.accept(Ok(snapshot()), now);
    a.prepare(Kind::Disconnect, None, now);
    a.confirm(now, "synthetic-operation".into());
    let count = a.activity.newest_first().count();
    a.accept(Err(ReadError::Unavailable), now - Duration::from_secs(1));
    assert_eq!(a.activity.newest_first().count(), count);
}
