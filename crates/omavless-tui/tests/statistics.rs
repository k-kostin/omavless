// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::Kind,
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::{ReadError, Snapshot},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::json;
use std::time::{Duration, Instant};

fn snapshot() -> Snapshot {
    load_page(&mut |read| {
        let mut value = support::response(read);
        match read {
            Read::Capabilities => value["result"]["methods"].as_array_mut().unwrap().extend([json!("plugin.action"),json!("stats.get"),json!("stats.configure"),json!("stats.reset")]),
            Read::Snapshot | Read::Observation => {
                value["result"]["desired"]["connected"] = false.into();
                value["result"]["desired"]["profileId"] = "".into();
                value["result"]["lastKnownActual"] = "disconnected".into();
                if read == Read::Observation { value["result"]["facts"] = json!({"ownedCoreRunning":false,"visibleMihomoCount":0,"ownedAuxiliaryMihomoCount":0,"visibleTunCount":0,"managedTunCount":0,"ownedControllerConfigVerified":false,"desiredProfileMatchesOwned":false}); }
            }
            _ => {}
        }
        Ok(value)
    }, Page::Traffic).unwrap()
}
fn build_app(locale: Locale, now: Instant) -> App {
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.statistics_enabled = true;
    app.viewport_ready = true;
    app.accept(Ok(snapshot()), now);
    app.page = Page::Traffic;
    app
}
fn key(code: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(code), KeyModifiers::NONE)
}

#[test]
fn explicit_controls_are_separate_from_network_pending_and_reset_is_period_fenced() {
    let now = Instant::now();
    let mut app = build_app(Locale::En, now);
    assert_eq!(app.key_at(key('e'), now), Action::None);
    assert_eq!(
        app.confirm(now, "stats-enable".into()),
        Action::SubmitStatistics
    );
    let request = app.take_statistics_request().unwrap();
    assert_eq!(request.method(), "stats.configure");
    assert_eq!(request.params()["enabled"], true);
    assert!(app.pending.is_none() && app.statistics_pending.is_some());
    assert!(
        !app.snapshot
            .as_ref()
            .unwrap()
            .statistics
            .as_ref()
            .unwrap()
            .enabled
    );
    assert_eq!(app.key_at(key('x'), now), Action::None);
    assert!(app.confirmation.is_none());
    // Another client can connect while metadata I/O is pending. Ordinary
    // Disconnect is still an independent, reachable action in this frontend.
    let mut connected = app.snapshot.clone().unwrap();
    connected.revision += 1;
    connected.metadata.desired.connected = true;
    connected.metadata.desired.profile_id = "fixture-a".into();
    connected.metadata.last_known_actual = omavless_tui::model::Actual::Connected;
    connected.observation.desired = connected.metadata.desired.clone();
    connected.observation.last_known_actual = connected.metadata.last_known_actual;
    let facts = connected.observation.facts.as_mut().unwrap();
    facts.owned_core_running = true;
    facts.owned_controller_config_verified = true;
    facts.desired_profile_matches_owned = true;
    facts.managed_tun_count = 1;
    app.accept(Ok(connected), now);
    app.prepare(Kind::Disconnect, None, now);
    assert!(app.confirmation.is_some());
    assert_eq!(app.confirm(now, "ordinary-down".into()), Action::Submit);
    assert!(app.pending.is_some() && app.statistics_pending.is_some());
    drop(request);
    let mut reset = build_app(Locale::En, now);
    reset.key_at(key('x'), now);
    reset
        .snapshot
        .as_mut()
        .unwrap()
        .statistics
        .as_mut()
        .unwrap()
        .revision += 1;
    assert_eq!(reset.confirm(now, "stale-period".into()), Action::None);
    assert!(reset.statistics_pending.is_none());
}

#[test]
fn refusal_unknown_and_old_read_never_become_optimistic_success_or_automatic_retry() {
    let now = Instant::now();
    let mut app = build_app(Locale::En, now);
    app.key_at(key('e'), now);
    assert_eq!(app.confirm(now, "lost".into()), Action::SubmitStatistics);
    let request = app.take_statistics_request().unwrap();
    app.finish_statistics(request.settle(Err(ReadError::Unavailable)), now);
    assert!(app.statistics_unknown && !app.unknown && app.pending.is_none());
    assert!(!app.statistics_can_change(now));
    let check = now + Duration::from_millis(2);
    assert_eq!(app.key_at(key('r'), check), Action::Refresh);
    app.accept(Ok(snapshot()), now);
    assert!(
        app.statistics_unknown,
        "old in-flight read cannot acknowledge unknown"
    );
    app.accept(Ok(snapshot()), check);
    assert!(!app.statistics_unknown);
    assert!(app.take_statistics_request().is_none());
    assert!(!app.statistics_can_change(check + FRESH_FOR));
}

#[test]
fn unknown_or_forged_completion_does_not_accept_provider_or_extra_state() {
    let now = Instant::now();
    let mut app = build_app(Locale::En, now);
    app.key_at(key('e'), now);
    app.confirm(now, "original".into());
    let request = app.take_statistics_request().unwrap();
    let mut value = support::response(Read::Statistics);
    value["result"]["operationId"] = "original".into();
    value["result"]["controlRevision"] = 3.into();
    value["result"]["enabled"] = true.into();
    let good = request.clone().settle(Ok(value.clone()));
    assert!(!good.unknown);
    value["result"]["provider"] = "private".into();
    assert!(request.clone().settle(Ok(value)).unknown);
    let rejected = request.settle(Ok(
        json!({"ok":false,"revision":7,"error":{"code":"conflict"}}),
    ));
    assert!(!rejected.unknown);
    assert_eq!(rejected.error, "error.conflict");
}

#[test]
fn recorded_and_partial_labels_and_confirmation_render_in_both_locales() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        for (width, height) in [(70, 24), (100, 36)] {
            let mut app = build_app(locale, now);
            for confirmation in [false, true] {
                if confirmation {
                    app.key_at(key('x'), now);
                }
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal.draw(|f| view::draw(f, &app, now)).unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>();
                assert!(!text.contains("Missing translation"));
                assert!(text.contains(locale.text("stats.title")));
                if !confirmation {
                    assert!(text.contains(&omavless_tui::statistics::grouped(u64::MAX, locale)));
                }
            }
        }
    }
}
