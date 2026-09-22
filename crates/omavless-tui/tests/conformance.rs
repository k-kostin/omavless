// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    app::{Action, App, FRESH_FOR},
    client::{Read, load},
    i18n::Locale,
    model::{ReadError, Snapshot, Status, display},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::response;

fn snapshot() -> Snapshot {
    load(&mut |r| Ok(response(r))).unwrap_or_else(|_| panic!("synthetic fixture invalid"))
}
fn key(app: &mut App, code: KeyCode) -> Action {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn render(app: &App, now: Instant, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| view::draw(f, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(usize::from(w))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn altered(method: Read, path: &str, value: Value) -> Result<Snapshot, ReadError> {
    load(&mut |r| {
        let mut v = response(r);
        if r == method {
            *v.pointer_mut(path).unwrap() = value.clone();
        }
        Ok(v)
    })
}

#[test]
fn only_four_fixed_read_methods() {
    let mut calls = Vec::new();
    let s = load(&mut |r| {
        calls.push(r);
        Ok(response(r))
    })
    .unwrap_or_else(|_| panic!("fixture"));
    assert_eq!(
        calls,
        [
            Read::Hello,
            Read::Capabilities,
            Read::Snapshot,
            Read::Observation
        ]
    );
    assert_eq!(
        calls.iter().map(|r| r.method()).collect::<Vec<_>>(),
        [
            "system.hello",
            "capabilities.get",
            "ui.snapshot",
            "runtime.observation"
        ]
    );
    assert_eq!(Read::Hello.params(), json!({"versions":[1]}));
    for r in [Read::Capabilities, Read::Snapshot, Read::Observation] {
        assert_eq!(r.params(), json!({}));
    }
    assert_eq!(s.status(), Status::Connected);
}
#[test]
fn negotiation_rejects_unknown_or_inactive_runtime() {
    for (method, path, value) in [
        (Read::Hello, "/result/version", json!(2)),
        (Read::Hello, "/result/runtimeOwnership", json!(false)),
        (Read::Capabilities, "/result/runtimeOwnership", json!(false)),
        (
            Read::Capabilities,
            "/result/methods",
            json!(["ui.snapshot"]),
        ),
    ] {
        assert!(matches!(
            altered(method, path, value),
            Err(ReadError::Incompatible)
        ));
    }
}
#[test]
fn coherence_rejects_mixed_observations() {
    for (path, value) in [
        ("/revision", json!(8)),
        ("/result/instanceId", json!("new-runtime")),
        ("/result/desired/generation", json!(6)),
        ("/result/desired/mode", json!("global")),
        ("/result/desired/connected", json!(false)),
        ("/result/lastKnownActual", json!("starting")),
    ] {
        assert!(matches!(
            altered(Read::Observation, path, value),
            Err(ReadError::Changed)
        ));
    }
}
#[test]
fn shape_bounds_and_claims_fail_closed() {
    for (method, path, value) in [
        (Read::Snapshot, "/result/schemaVersion", json!(2)),
        (Read::Snapshot, "/result/scope", json!("other")),
        (Read::Snapshot, "/result/healthFresh", json!(true)),
        (Read::Snapshot, "/result/transition", json!({})),
        (
            Read::Snapshot,
            "/result/profiles/0/name",
            json!("x".repeat(81)),
        ),
        (
            Read::Snapshot,
            "/result/profiles/0/id",
            json!("x".repeat(65)),
        ),
        (Read::Snapshot, "/result/profiles/1/id", json!("fixture-a")),
        (
            Read::Snapshot,
            "/result/profiles/0/subscriptionId",
            json!("missing"),
        ),
        (
            Read::Snapshot,
            "/result/profiles",
            json!(vec![
                response(Read::Snapshot)["result"]["profiles"][0]
                    .clone();
                257
            ]),
        ),
        (
            Read::Observation,
            "/result/facts/visibleMihomoCount",
            json!(65),
        ),
        (
            Read::Observation,
            "/result/facts/visibleMihomoCount",
            json!(0),
        ),
        (Read::Observation, "/result/facts/visibleTunCount", json!(9)),
        (Read::Observation, "/result/facts/managedTunCount", json!(2)),
        (
            Read::Observation,
            "/result/facts/ownedCoreRunning",
            json!(false),
        ),
        (
            Read::Observation,
            "/result/facts/desiredProfileMatchesOwned",
            json!(false),
        ),
        (
            Read::Observation,
            "/result/manualRecoveryRequired",
            json!(true),
        ),
        (
            Read::Observation,
            "/result/availability",
            json!("unavailable"),
        ),
    ] {
        assert!(
            matches!(altered(method, path, value), Err(ReadError::Invalid)),
            "{path}"
        );
    }
}
#[test]
fn maximum_projection_and_unicode_names_are_bounded() {
    let s = load(&mut |r| {
        let mut v = response(r);
        if r == Read::Snapshot {
            v["result"]["profiles"] = json!((0..256).map(|i|json!({"id":format!("fixture-{i}"),"name":"界".repeat(80),"favorite":false,"missing":false,"subscriptionId":null})).collect::<Vec<_>>());
            v["result"]["desired"]["profileId"]=json!("fixture-0");
            v["result"]["subscriptions"] = json!((0..64).map(|i|json!({"id":format!("sub-{i}"),"name":"🦀".repeat(80)})).collect::<Vec<_>>());
        }
        Ok(v)
    }).unwrap_or_else(|_|panic!("maximum fixture rejected"));
    assert_eq!(s.metadata.profiles.len(), 256);
}
#[test]
fn remote_error_text_never_enters_public_error() {
    let result = load(&mut |_| {
        Ok(
            json!({"ok":false,"error":{"message":"vless://synthetic-secret@example.invalid password=synthetic"}}),
        )
    });
    assert!(matches!(result, Err(ReadError::Unavailable)));
    for error in [
        ReadError::Unavailable,
        ReadError::Invalid,
        ReadError::Incompatible,
        ReadError::Changed,
    ] {
        let mut app = App::new(Locale::En);
        app.accept(Err(error), Instant::now());
        let screen = render(&app, Instant::now(), 100, 24);
        assert!(!screen.contains("synthetic-secret"));
        assert!(!screen.contains("vless://"));
    }
}
#[test]
fn selection_and_filter_never_change_connected_identity() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.accept(Ok(snapshot()), now);
    assert!(app.selected.is_none());
    key(&mut app, KeyCode::End);
    assert_eq!(app.selected.as_deref(), Some("fixture-b"));
    assert_eq!(key(&mut app, KeyCode::Enter), Action::None);
    key(&mut app, KeyCode::Char('/'));
    assert!(render(&app, now, 100, 24).contains("Esc, then q:"));
    for c in "Frankfurt".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.visible(), vec![1]);
    let screen = render(&app, now, 100, 24);
    assert!(screen.contains("Connected profile: Fixture Helsinki"));
    assert!(screen.contains("Fixture Frankfurt"));
    assert_eq!(app.status(now), Status::Connected);
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.visible().len(), 2);
}
#[test]
fn keyboard_help_search_bounds_and_close() {
    let mut app = App::new(Locale::En);
    key(&mut app, KeyCode::Char('/'));
    for _ in 0..100 {
        key(&mut app, KeyCode::Char('я'));
    }
    assert_eq!(app.query.chars().count(), 80);
    assert_eq!(key(&mut app, KeyCode::Char('q')), Action::None);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char('?'));
    assert!(app.help);
    assert_eq!(key(&mut app, KeyCode::Char('q')), Action::Close);
    key(&mut app, KeyCode::Esc);
    assert!(!app.help);
    assert_eq!(key(&mut app, KeyCode::Char('r')), Action::Refresh);
    assert_eq!(
        app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Action::Close
    );
}
#[test]
fn freshness_and_revision_regression_clear_connection_claim() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.accept(Ok(snapshot()), now);
    assert_eq!(app.status(now + FRESH_FOR), Status::Unverified);
    assert!(
        !render(&app, now + FRESH_FOR, 100, 24).contains("Connected profile: Fixture Helsinki")
    );
    let mut old = snapshot();
    old.revision = 6;
    app.accept(Ok(old), now);
    assert_eq!(app.error, Some(ReadError::Changed));
    assert!(app.snapshot.is_none());
    app.accept(Ok(snapshot()), now);
    app.accept(Err(ReadError::Unavailable), now);
    assert_eq!(app.status(now), Status::Unverified);
    let mut restart = snapshot();
    restart.metadata.instance_id = "new-instance".into();
    restart.revision = 1;
    app.accept(Ok(restart), now);
    assert!(app.error.is_none());
    assert!(app.selected.is_none());
}
#[test]
fn disconnected_and_recovery_not_inferred_from_cache() {
    let mut s = snapshot();
    s.observation.facts = None;
    assert_eq!(s.status(), Status::Unverified);
    s.observation.manual_recovery_required = true;
    assert_eq!(s.status(), Status::Recovery);
    let mut s = snapshot();
    s.metadata.desired.connected = false;
    s.metadata.last_known_actual = omavless_tui::model::Actual::Disconnected;
    let f = s.observation.facts.as_mut().unwrap();
    f.owned_core_running = false;
    f.managed_tun_count = 0;
    assert_eq!(s.status(), Status::Disconnected);
    s.observation
        .facts
        .as_mut()
        .unwrap()
        .owned_auxiliary_mihomo_count = 1;
    assert_eq!(s.status(), Status::Unverified);
}
#[test]
fn locale_and_plain_text_safety() {
    assert_eq!(Locale::parse("ru_RU.UTF-8"), Locale::Ru);
    assert_eq!(Locale::parse("ru-RU"), Locale::Ru);
    for s in ["", "de_DE", "C.UTF-8", "en_US"] {
        assert_eq!(Locale::parse(s), Locale::En);
    }
    assert_eq!(Locale::Ru.text("status.connected"), "Подключено");
    assert_eq!(
        Locale::En.text("invalid-private-key"),
        "Missing translation"
    );
    assert_eq!(display("<b>name</b>", 80), "<b>name</b>");
    let safe = display("\x1b]52;c;secret\x07\n\u{202e}name\u{2066}", 80);
    assert!(!safe.chars().any(char::is_control));
    assert!(!safe.contains('\u{202e}'));
    assert!(!safe.contains('\u{2066}'));
    assert_eq!(display("🦀".repeat(100).as_str(), 80).chars().count(), 80);
}

#[test]
fn unknown_profile_count_is_not_zero_and_ids_are_not_rendered() {
    let now = Instant::now();
    let mut app = App::new(Locale::En);
    app.accept(Err(ReadError::Unavailable), now);
    assert!(render(&app, now, 100, 24).contains("Profiles (—)"));
    app.accept(Ok(snapshot()), now);
    let screen = render(&app, now, 100, 24);
    assert!(!screen.contains("fixture-a"));
    assert!(!screen.contains("fixture-runtime"));
    let mut s = snapshot();
    s.metadata.profiles.remove(0);
    assert_eq!(s.status(), Status::Unverified);
}
#[test]
fn responsive_rendering_never_hides_exit_with_long_names() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = App::new(locale);
        let mut s = snapshot();
        s.metadata.profiles[0].name = "🦀".repeat(80);
        app.accept(Ok(s), now);
        key(&mut app, KeyCode::Home);
        app.query = "я".repeat(80);
        for (w, h) in [(50, 14), (80, 24), (120, 35)] {
            let screen = render(&app, now, w, h);
            assert!(screen.contains(locale.text("tui.close_hint")), "{w}x{h}");
            assert!(screen.contains(locale.text("status.connected")));
        }
        for (w, h) in [(1, 1), (30, 8), (49, 13)] {
            render(&app, now, w, h);
        }
        app.help = true;
        render(&app, now, 80, 24);
        app.accept(Err(ReadError::Unavailable), now);
        assert!(!render(&app, now, 80, 24).contains("Fixture Helsinki"));
        app.accept(Ok(snapshot()), now);
        render(&app, now + Duration::from_secs(10), 80, 24);
    }
}
