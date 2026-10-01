// SPDX-License-Identifier: MIT
mod support;

use omavless_tui::{
    app::App,
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    traffic_history::{CAPACITY, History, LONG_WINDOW, WINDOW},
    view,
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
use std::time::{Duration, Instant};

fn render(terminal: &mut Terminal<TestBackend>, app: &App, at: Instant) -> String {
    terminal.draw(|frame| view::draw(frame, app, at)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

#[test]
fn bounded_relative_history_distinguishes_zero_missing_and_expired() {
    let start = Instant::now();
    let mut history = History::default();
    assert!(history.sparkline(start, true).is_none());
    history.observe(Some((0, 0)), start);
    assert_eq!(history.sparkline(start, true).as_deref(), Some("·"));
    history.observe(Some((5, 10)), start + Duration::from_secs(3));
    history.observe(Some((10, 20)), start + Duration::from_secs(6));
    assert_eq!(history.peak(start + Duration::from_secs(6), true), Some(10));
    assert_eq!(
        history
            .sparkline(start + Duration::from_secs(6), true)
            .as_deref(),
        Some("·▄█")
    );
    assert_eq!(
        history
            .sparkline(start + Duration::from_secs(6), false)
            .as_deref(),
        Some("·▄█")
    );
    assert_eq!(
        history
            .recent(start + WINDOW + Duration::from_secs(7))
            .count(),
        0
    );
    history.observe(None, start + Duration::from_secs(9));
    assert!(
        history
            .sparkline(start + Duration::from_secs(9), false)
            .is_none()
    );
}

#[test]
fn samples_cannot_be_unbounded_or_joined_across_non_monotonic_time() {
    let start = Instant::now();
    let mut history = History::default();
    for index in 0..100 {
        history.observe(Some((index, index)), start + Duration::from_secs(index));
    }
    assert!(history.recent(start + Duration::from_secs(99)).count() <= CAPACITY);
    history.observe(Some((1, 1)), start + Duration::from_secs(10));
    assert_eq!(history.recent(start + Duration::from_secs(10)).count(), 1);
}

#[test]
fn five_minute_view_is_bounded_and_preserves_short_bursts() {
    let start = Instant::now();
    let mut history = History::default();
    for index in 0..100 {
        let upload = if index == 17 { 10_000 } else { 10 };
        history.observe(
            Some((upload, index)),
            start + Duration::from_secs(index * 3),
        );
    }
    let now = start + Duration::from_secs(297);
    assert!(history.has_long_trend(now));
    assert_eq!(history.peak_window(now, true, LONG_WINDOW), Some(10_000));
    assert_eq!(history.peak(now, true), Some(10));
    let graph = history.sparkline_window(now, true, LONG_WINDOW).unwrap();
    assert_eq!(graph.chars().count(), 40);
    assert!(graph.contains('█'));

    history.observe(Some((1, 1)), now + Duration::from_secs(13));
    assert!(!history.has_long_trend(now + Duration::from_secs(13)));
    assert_eq!(
        history
            .recent_window(now + Duration::from_secs(13), LONG_WINDOW)
            .count(),
        1
    );
}

#[test]
fn app_history_requires_consecutive_owned_samples_and_clears_on_restart_or_error() {
    let start = Instant::now();
    let mut app = App::new(Locale::En);
    app.page = Page::Traffic;
    let sample = |sampled_at: u64, tx: u64, revision: u64| {
        load_page(
            &mut |read| {
                let mut value = support::response(read);
                if read == Read::Traffic {
                    value["result"]["sample"]["sampledAtMs"] = json!(sampled_at);
                    value["result"]["sample"]["txBytes"] = json!(tx);
                }
                value["revision"] = json!(revision);
                Ok(value)
            },
            Page::Traffic,
        )
    };
    app.accept(sample(1000, 8192, 7), start);
    assert!(app.traffic_history.sparkline(start, true).is_none());
    app.accept(sample(4000, 9192, 7), start + Duration::from_secs(3));
    assert_eq!(
        app.traffic_history
            .recent(start + Duration::from_secs(3))
            .count(),
        1
    );
    app.accept(sample(7000, 10192, 8), start + Duration::from_secs(6));
    assert!(
        app.traffic_history
            .sparkline(start + Duration::from_secs(6), true)
            .is_none()
    );
    app.accept(
        Err(omavless_tui::model::ReadError::Unavailable),
        start + Duration::from_secs(7),
    );
    assert!(
        app.traffic_history
            .sparkline(start + Duration::from_secs(7), true)
            .is_none()
    );
}

#[test]
fn traffic_screen_labels_relative_samples_in_en_and_ru_without_live_probe() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        app.page = Page::Traffic;
        app.accept(
            load_page(&mut |read| Ok(support::response(read)), Page::Traffic),
            now,
        );
        let mut terminal = Terminal::new(TestBackend::new(90, 30)).unwrap();
        assert!(
            render(&mut terminal, &app, now)
                .contains(locale.text("tui.traffic_history_unavailable"))
        );
        app.traffic_history
            .observe(Some((42, 21)), now + Duration::from_secs(3));
        let screen = render(&mut terminal, &app, now + Duration::from_secs(3));
        assert!(screen.contains(locale.text("tui.traffic_history_scope")));
        assert!(screen.contains(locale.text("tui.upload_history")));
        assert!(screen.contains(locale.text("tui.download_history")));
        assert!(!screen.contains("private://"));
        for index in 2..=22 {
            app.traffic_history
                .observe(Some((42, 21)), now + Duration::from_secs(index * 3));
        }
        let later = now + Duration::from_secs(66);
        app.sampled_at = Some(later);
        assert!(
            render(&mut terminal, &app, later)
                .contains(locale.text("tui.traffic_history_long_scope"))
        );
        let mut compact = Terminal::new(TestBackend::new(80, 20)).unwrap();
        app.inspection_scroll = u16::MAX;
        view::clamp_scroll(&mut app, 80, 20, later);
        assert!(
            render(&mut compact, &app, later)
                .contains(locale.text("tui.traffic_history_long_scope"))
        );
    }
}
