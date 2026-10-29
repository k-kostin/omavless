// SPDX-License-Identifier: MIT
mod support;
use omavless_domain::private_store::DnsPreset;
use omavless_tui::{
    actions::Kind,
    app::{Action, App, FRESH_FOR},
    client::{Read, load_page},
    i18n::Locale,
    inspection::Page,
    model::{Actual, DnsSettings},
    view,
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;
use std::time::Instant;

fn app(locale: Locale, now: Instant) -> App {
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.viewport_ready = true;
    let snapshot = load_page(&mut |r| {
        let mut value = support::response(r);
        match r {
            Read::Capabilities => value["result"]["methods"].as_array_mut().unwrap()
                .extend([json!("plugin.action"),json!("dns.configure")]),
            Read::Snapshot | Read::Observation => {
                value["result"]["desired"]["connected"] = false.into();
                value["result"]["desired"]["profileId"] = "".into();
                value["result"]["lastKnownActual"] = "disconnected".into();
                if r == Read::Snapshot {
                    value["result"]["dns"] = json!({"preset":"preset-default","source":"bundled"});
                } else {
                    value["result"]["facts"] = json!({"ownedCoreRunning":false,"visibleMihomoCount":0,
                        "ownedAuxiliaryMihomoCount":0,"visibleTunCount":0,"managedTunCount":0,
                        "ownedControllerConfigVerified":false,"desiredProfileMatchesOwned":false});
                }
            }
            _ => {},
        }
        Ok(value)
    }, Page::Settings).unwrap();
    app.accept(Ok(snapshot), now);
    app.page = Page::Settings;
    app
}

#[test]
fn dns_confirmation_targets_preference_without_optimistic_metadata() {
    let now = Instant::now();
    let mut app = app(Locale::En, now);
    app.prepare(Kind::Dns(DnsPreset::Google), None, now);
    assert!(app.confirmation.is_some());
    assert_eq!(app.confirm(now, "dns-original".into()), Action::Submit);
    let request = app.pending.as_ref().unwrap().params();
    assert_eq!(request["action"], "dns-configure");
    assert_eq!(request["preset"], "google");
    assert!(request.get("profileId").is_none());
    assert!(request.get("mode").is_none());
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .metadata
            .dns
            .as_ref()
            .unwrap()
            .preset,
        "preset-default"
    );
    let original = request.to_string();
    app.prepare(Kind::Dns(DnsPreset::Cloudflare), None, now);
    assert!(app.confirmation.is_none());
    assert_eq!(app.pending.as_ref().unwrap().params().to_string(), original);
}

#[test]
fn dns_refuses_stale_active_custom_missing_capability_and_recovery() {
    let now = Instant::now();
    for case in 0..7 {
        let mut app = app(Locale::En, now);
        let snapshot = app.snapshot.as_mut().unwrap();
        match case {
            0 => snapshot.metadata.desired.connected = true,
            1 => snapshot.metadata.last_known_actual = Actual::Starting,
            2 => {
                snapshot.metadata.dns = Some(DnsSettings {
                    preset: "google".into(),
                    source: omavless_tui::model::DnsSource::Custom,
                })
            }
            3 => snapshot.metadata.dns = None,
            4 => snapshot.capabilities.dns = false,
            5 => snapshot.observation.manual_recovery_required = true,
            _ => {}
        }
        app.prepare(
            Kind::Dns(DnsPreset::Google),
            None,
            if case == 6 { now + FRESH_FOR } else { now },
        );
        assert!(app.confirmation.is_none() && app.pending.is_none());
    }
}

#[test]
fn dns_settings_and_confirmation_render_both_locales_without_fallback() {
    let now = Instant::now();
    for locale in [Locale::En, Locale::Ru] {
        let mut app = app(locale, now);
        for confirm in [false, true] {
            if confirm {
                app.prepare(Kind::Dns(DnsPreset::Cloudflare), None, now);
            }
            let mut terminal = Terminal::new(TestBackend::new(100, 36)).unwrap();
            terminal.draw(|f| view::draw(f, &app, now)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(!text.contains("Missing translation"));
            assert!(text.contains(if confirm {
                locale.text("tui.dns_confirm")
            } else {
                "Cloudflare"
            }));
        }
    }
}
