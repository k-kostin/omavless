// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    app::{Action, App},
    client::{ProfileTarget, Read, load_page},
    i18n::Locale,
    inspection::Page,
    subscription_usage::{self, Request, Status, Usage},
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
    let mut value = support::response(read);
    if read == Read::Capabilities {
        value["result"]["methods"]
            .as_array_mut()
            .unwrap()
            .push(json!("subscriptions.usage"));
    }
    value
}
fn request() -> Request {
    Request::new(
        ProfileTarget::new("fixture-sub").unwrap(),
        "fixture-runtime".into(),
        7,
    )
}
fn render(app: &mut App, now: Instant, width: u16, height: u16) -> String {
    view::clamp_scroll(app, width, height, now);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view::draw(f, app, now)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn explicit_only_and_fenced_against_owner_revision_and_membership() {
    let snapshot = load_page(&mut |r| Ok(response(r)), Page::Subscriptions).unwrap();
    let mut reads = Vec::new();
    assert!(
        subscription_usage::load(
            &mut |r| {
                reads.push(r);
                Ok(response(r))
            },
            Page::Subscriptions,
            snapshot.clone(),
            None
        )
        .unwrap()
        .1
        .is_none()
    );
    assert!(reads.is_empty());
    let (_, status) = subscription_usage::load(
        &mut |r| {
            reads.push(r);
            Ok(response(r))
        },
        Page::Subscriptions,
        snapshot.clone(),
        Some(&request()),
    )
    .unwrap();
    assert!(matches!(status, Some(Status::Reported(_))));
    assert_eq!(
        reads
            .iter()
            .filter(|r| matches!(r, Read::SubscriptionUsage(_)))
            .count(),
        1
    );
    for changed in 0..3 {
        let mut latest = snapshot.clone();
        match changed {
            0 => latest.revision += 1,
            1 => latest.metadata.instance_id = "other-runtime".into(),
            _ => latest.metadata.subscriptions.clear(),
        }
        let mut sent = false;
        let result = subscription_usage::load(
            &mut |r| {
                sent |= matches!(r, Read::SubscriptionUsage(_));
                Ok(response(r))
            },
            Page::Subscriptions,
            latest,
            Some(&request()),
        )
        .unwrap();
        assert!(!sent);
        assert!(matches!(result.1, Some(Status::Unavailable)));
    }
    let result = subscription_usage::load(
        &mut |r| {
            let mut v = response(r);
            if r == Read::Snapshot {
                v["revision"] = json!(8);
            }
            Ok(v)
        },
        Page::Subscriptions,
        snapshot,
        Some(&request()),
    );
    assert!(result.is_err());
}
#[test]
fn counters_are_exact_bounded_and_never_infer_unlimited_or_health() {
    let req = request();
    let original = response(Read::SubscriptionUsage(req.target));
    let Some(Status::Reported(usage)) = Usage::parse(&original, &req) else {
        panic!("synthetic usage missing")
    };
    assert_eq!(usage.remaining(), Some(7 << 30));
    assert_eq!(usage.expiry_utc.as_deref(), Some("2030-01-01 00:00 UTC"));
    for (path, value) in [
        ("/result/usage/uploadBytes", json!("18446744073709551616")),
        ("/result/usage/downloadBytes", json!(12)),
        ("/result/usage/totalBytes", json!("01")),
        ("/result/usage/expiryUnixSeconds", json!("253402300800")),
        ("/result/availability", json!("healthy")),
        ("/result/instanceId", json!("other-runtime")),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(path).unwrap() = value;
        assert!(Usage::parse(&v, &req).is_none());
    }
    for (upload, download, total) in [(u64::MAX, 1, u64::MAX), (3, 4, 5), (0, 0, 0)] {
        let u = Usage {
            upload,
            download,
            total,
            expiry_utc: None,
        };
        assert_eq!(u.remaining(), None);
    }
    assert!(!format!("{:?}", Read::SubscriptionUsage(req.target)).contains("fixture-sub"));
}
#[test]
fn utc_formatter_handles_leap_centuries_and_calendar_bounds() {
    for (seconds, expected) in [
        (1, "1970-01-01 00:00 UTC"),
        (951782400, "2000-02-29 00:00 UTC"),
        (4107542400, "2100-03-01 00:00 UTC"),
        (253402300799, "9999-12-31 23:59 UTC"),
    ] {
        assert_eq!(
            subscription_usage::utc_date(seconds).as_deref(),
            Some(expected)
        );
    }
    assert!(subscription_usage::utc_date(0).is_none());
    assert!(subscription_usage::utc_date(u64::MAX).is_none());
}
#[test]
fn page_selection_repeat_request_and_revision_erase_private_claims() {
    for locale in [Locale::En, Locale::Ru] {
        let at = Instant::now();
        let mut app = App::new(locale);
        app.page = Page::Subscriptions;
        app.accept(load_page(&mut |r| Ok(response(r)), app.page), at);
        app.selected_subscription = Some("fixture-sub".into());
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert_eq!(app.key_at(key(KeyCode::Char('u')), at), Action::Refresh);
        let old = app.usage_request.take().unwrap();
        assert_eq!(app.key_at(key(KeyCode::Char('u')), at), Action::Refresh);
        app.accept_usage(Some(&old), Some(Status::NotProvided), at);
        assert!(matches!(app.usage_result, Some(Status::Loading)));
        let req = app.usage_request.take().unwrap();
        app.accept_usage(
            Some(&req),
            Usage::parse(&response(Read::SubscriptionUsage(req.target)), &req),
            at,
        );
        let screen = render(&mut app, at, 120, 40);
        assert!(screen.contains(locale.text("tui.usage_reported")));
        assert!(screen.contains("2030-01-01"));
        for private in [
            "fixture-sub",
            "https://",
            "subscription-token",
            "Subscription-Userinfo",
        ] {
            assert!(!screen.contains(private));
        }
        for (w, h) in [(70, 24), (40, 12), (24, 8)] {
            render(&mut app, at, w, h);
        }
        app.key_at(key(KeyCode::Char('n')), at);
        assert!(app.usage_result.is_none());
        app.key_at(key(KeyCode::Esc), at);
        assert!(app.usage_request.is_none());
        assert!(app.usage_result.is_none());
        app.accept_usage(Some(&req), Some(Status::NotProvided), at);
        assert!(app.usage_result.is_none());
    }
}
