// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    app::{Action, App},
    client::{Read, load},
    i18n::Locale,
    model::ReadError,
    private_import::{MAX_INPUT, State, Workspace},
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const PRIVATE: &str = "synthetic-private-input-no-real-credential";
fn press(app: &mut App, key: KeyCode, now: Instant) -> Action {
    app.key_at(KeyEvent::new(key, KeyModifiers::NONE), now)
}
fn sample(app: &mut App, now: Instant) {
    let s = load(&mut |read| {
        let mut v = support::response(read);
        if read == Read::Capabilities {
            v["result"]["methods"]
                .as_array_mut()
                .unwrap()
                .extend([json!("plugin.action"), json!("imports.classify")]);
        }
        Ok(v)
    })
    .unwrap();
    app.accept(Ok(s), now);
}
fn app(locale: Locale) -> (App, Instant) {
    let now = Instant::now();
    let mut app = App::new(locale);
    app.actions_enabled = true;
    app.import_enabled = true;
    sample(&mut app, now);
    (app, now)
}
fn envelope(result: Value) -> Value {
    json!({"ok":true,"revision":7,"result":result})
}
fn profile() -> Value {
    envelope(json!({"version":1,"kind":"profile","profile":{
    "version":1,"protocol":"vless","suggestedName":"Synthetic Helsinki","insecure":false,"experimental":false}}))
}
fn bundle(accepted: u16) -> Value {
    envelope(json!({"version":1,"kind":"bundle","counts":{
    "accepted":accepted,"duplicates":2,"skipped":3},"previewFingerprint":"a".repeat(64)}))
}
fn draft(serial: u64) -> Workspace {
    let mut w = Workspace::new("fixture-runtime", 7, serial).unwrap();
    assert!(w.append(PRIVATE));
    w
}
fn preview(w: &mut Workspace, value: Value, now: Instant) {
    let request = w.classify(now).unwrap();
    w.accept(request.settle(Ok(value)));
}
fn render(app: &App, now: Instant, width: u16, height: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
    t.draw(|f| view::draw(f, app, now)).unwrap();
    t.backend()
        .buffer()
        .content
        .chunks(width as usize)
        .map(|line| line.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn closed_import_reasons_render_both_locales_at_minimum_size_and_reject_private_keys() {
    let reasons = json!({"unsupported_field":1,"unsupported_transport":1,
        "unsupported_option":1,"invalid_profile":1,"unsupported_protocol":1,"duplicate":1});
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = app(locale);
        press(&mut app, KeyCode::F(3), now);
        app.import_paste(PRIVATE.into());
        press(&mut app, KeyCode::Enter, now);
        let request = app.take_import_request().unwrap();
        let mut value = bundle(1);
        value["result"]["counts"] = json!({"accepted":1,"duplicates":1,"skipped":5});
        value["result"]["skipReasons"] = reasons.clone();
        app.finish_import(request.settle(Ok(value)));
        assert_eq!(app.import.as_ref().unwrap().state(), State::Preview);
        let output = render(&app, now, 70, 24);
        for key in [
            "unsupported_field",
            "unsupported_transport",
            "unsupported_option",
            "invalid_profile",
            "unsupported_protocol",
            "duplicate",
        ] {
            assert!(
                output.contains(locale.text(&format!("import.reason.{key}"))),
                "missing reason {key}"
            );
        }
        assert!(output.contains("Esc"));
        assert!(!output.contains(PRIVATE));
    }
    let now = Instant::now();
    let mut workspace = draft(100);
    let request = workspace.classify(now).unwrap();
    let value = json!({"ok":false,"revision":7,"error":{"code":"invalid_argument",
        "message":PRIVATE,"retryable":false,"details":{"importReasons":{"unsupported_field":1}}}});
    workspace.accept(request.settle(Ok(value)));
    assert_eq!(workspace.state(), State::Rejected);
    assert!(workspace.rejection_reasons.is_some());
    for invalid in [
        json!({"private-secret":1}),
        json!({"invalid_profile":true}),
        json!({"invalid_profile":0}),
        json!({"invalid_profile":1025}),
    ] {
        let mut workspace = draft(101);
        let request = workspace.classify(now).unwrap();
        workspace.accept(request.settle(Ok(json!({"ok":false,"revision":7,"error":{
            "code":"invalid_argument","message":PRIVATE,"retryable":false,"details":{"importReasons":invalid}}}))));
        assert!(workspace.rejection_reasons.is_none());
    }
}

#[test]
fn subscription_save_preserves_only_applied_typed_summary() {
    let now = Instant::now();
    let mut workspace = draft(201);
    preview(
        &mut workspace,
        envelope(json!({"version":1,"kind":"subscription",
        "suggestedName":"Subscription","duplicate":false})),
        now,
    );
    let request = workspace.confirm("summary-save".into(), now).unwrap();
    let p = request.params();
    let summary =
        json!({"accepted":1,"duplicates":0,"skipped":1,"skipReasons":{"unsupported_field":1}});
    workspace.accept(request.settle(Ok(json!({"ok":true,"revision":8,"result":{
        "schemaVersion":1,"applied":true,"instanceId":p["instanceId"],
        "operationId":p["operationId"],"action":"subscription-add","importSummary":summary}}))));
    assert_eq!(workspace.state(), State::Completed);
    assert_eq!(workspace.import_summary.unwrap().value(), summary);
}

#[test]
fn managed_preview_codes_are_localized_closed_and_never_change_unknown_or_stale_fences() {
    for (note, key) in [
        ("managed_ipv4_only", "import.managed_ipv4_only"),
        ("managed_keepalive_range", "import.managed_keepalive_range"),
        ("managed_header_ranges", "import.managed_header_ranges"),
        ("managed_legacy_headers", "import.managed_legacy_headers"),
        ("managed_padding", "import.managed_padding"),
        ("managed_legacy_padding", "import.managed_legacy_padding"),
        ("managed_junk_budget", "import.managed_junk_budget"),
        ("managed_special_junk", "import.managed_special_junk"),
        (
            "managed_numeric_parameters",
            "import.managed_numeric_parameters",
        ),
        ("private-sentinel\nsecret", "import.managed_unavailable"),
        ("import.managed_ipv4_only", "import.managed_unavailable"),
        ("", "import.managed_scope"),
    ] {
        for locale in [Locale::En, Locale::Ru] {
            let (mut app, now) = app(locale);
            let mut w = draft(700);
            let mut value = profile();
            let p = &mut value["result"]["profile"];
            p["protocol"] = json!("wireguard");
            p["transport"] = json!("udp");
            p["security"] = json!("wireguard");
            p["credentialHint"] = json!("••••");
            p["experimental"] = json!(true);
            p["compatibilityNote"] = json!(note);
            preview(&mut w, value.clone(), now);
            assert_eq!(w.preview().unwrap().managed_note_key, key);
            app.import = Some(w);
            app.import_open = true;
            let output = render(&app, now, 70, 24);
            let normalized = output
                .replace('│', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                normalized.contains(locale.text(key)),
                "missing complete localized warning {key}: {normalized}"
            );
            assert!(!output.contains("private-sentinel") && !output.contains(PRIVATE));
            assert!(!output.contains("import.managed_"));
            let w = app.import.as_mut().unwrap();
            let request = w.confirm("one-save".into(), now).unwrap();
            w.accept(request.settle(Err(ReadError::Unavailable)));
            assert_eq!(w.state(), State::Unknown);
            assert!(w.confirm("never-resend".into(), now).is_none());
            w.cancel();
            assert_eq!(w.state(), State::Unknown);
            let mut stale = draft(701);
            let request = stale.classify(now).unwrap();
            value["revision"] = json!(8);
            stale.accept(request.settle(Ok(value)));
            assert_ne!(stale.state(), State::Preview);
            assert!(stale.preview().is_none());
        }
    }
}

#[test]
fn native_preview_explains_ipv4_refusal_and_preserves_ordinary_import_target() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = app(locale);
        press(&mut app, KeyCode::F(3), now);
        app.import_paste(PRIVATE.into());
        press(&mut app, KeyCode::Enter, now);
        let request = app.take_import_request().unwrap();
        let mut value = profile();
        let p = &mut value["result"]["profile"];
        p["protocol"] = json!("amneziawg");
        p["transport"] = json!("udp");
        p["security"] = json!("wireguard");
        p["credentialHint"] = json!("••••");
        p["experimental"] = json!(true);
        p["compatibilityNote"] = json!("managed_ipv4_only");
        app.finish_import(request.settle(Ok(value)));
        let output = render(&app, now, 70, 24);
        assert!(output.contains(match locale {
            Locale::En => "Cannot connect: IPv4 is required.",
            Locale::Ru => "Подключение недоступно: требуется IPv4.",
        }));
        assert!(!output.contains(PRIVATE));
        press(&mut app, KeyCode::Enter, now);
        let save = app.take_import_request().unwrap();
        assert_eq!(save.method(), "plugin.action");
        assert_eq!(save.params()["action"], "profile-import");
        assert_eq!(save.params()["input"], PRIVATE);
        assert_eq!(save.params()["expectedRevision"], 7);
    }
}

#[test]
fn native_profile_preview_uses_existing_confirmation_and_cancel_without_connecting() {
    for protocol in ["wireguard", "amneziawg"] {
        let now = Instant::now();
        let mut w = draft(1);
        let mut result = profile();
        result["result"]["profile"]["protocol"] = protocol.into();
        result["result"]["profile"]["experimental"] = true.into();
        result["result"]["profile"]["transport"] = "udp".into();
        result["result"]["profile"]["security"] = "wireguard".into();
        result["result"]["profile"]["credentialHint"] = "••••".into();
        preview(&mut w, result, now);
        assert_eq!(w.state(), State::Preview);
        assert_eq!(
            w.preview().unwrap().kind,
            omavless_tui::private_import::Kind::Profile
        );
        w.cancel();
        assert_eq!(w.input_bytes(), 0);
        assert!(w.preview().is_none());
    }
}

#[test]
fn native_preview_discloses_managed_subset_in_both_locales_without_private_input() {
    for locale in [Locale::En, Locale::Ru] {
        for protocol in ["wireguard", "amneziawg"] {
            let (mut app, now) = app(locale);
            let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
            press(&mut app, KeyCode::F(3), now);
            app.import_paste(PRIVATE.into());
            assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitImport);
            let request = app.take_import_request().unwrap();
            let mut result = profile();
            let p = &mut result["result"]["profile"];
            p["protocol"] = protocol.into();
            p["experimental"] = true.into();
            p["transport"] = "udp".into();
            p["security"] = "wireguard".into();
            p["credentialHint"] = "••••".into();
            app.finish_import(request.settle(Ok(result)));
            let text = render(&app, now, 70, 24);
            assert!(text.contains("IPv4"));
            assert!(!text.contains(PRIVATE));
            assert!(!text.contains("TLS"));
            assert!(text.contains("Esc"));
            assert!(app.import.as_ref().unwrap().preview().unwrap().native);
            press(&mut app, KeyCode::Esc, now);
            assert!(app.take_import_request().is_none());
            assert_eq!(
                app.snapshot.as_ref().unwrap().metadata.desired.connected,
                desired.connected
            );
            assert_eq!(
                app.snapshot.as_ref().unwrap().metadata.desired.profile_id,
                desired.profile_id
            );
        }
    }
}

#[test]
fn entry_is_discoverable_in_ordinary_control_client_and_never_navigates_by_search_typing() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = app(locale);
        assert!(render(&app, now, 70, 24).contains("i/F3"));
        assert_eq!(press(&mut app, KeyCode::Char('i'), now), Action::None);
        assert!(app.import_open);
        assert!(app.take_import_request().is_none());
        press(&mut app, KeyCode::Esc, now);
        app.searching = true;
        press(&mut app, KeyCode::Char('i'), now);
        assert_eq!(app.query, "i");
        assert!(!app.import_open);
    }
}
#[test]
fn two_explicit_enters_classify_then_save_without_changing_selection_or_connection() {
    let (mut app, now) = app(Locale::En);
    let selected = app.selected.clone();
    let desired = app.snapshot.as_ref().unwrap().metadata.desired.clone();
    press(&mut app, KeyCode::F(3), now);
    app.import_paste(PRIVATE.into());
    assert!(!render(&app, now, 70, 24).contains(PRIVATE));
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitImport);
    let request = app.take_import_request().unwrap();
    assert!(!request.mutation());
    assert_eq!(request.method(), "imports.classify");
    assert_eq!(request.params(), json!({"input":PRIVATE}));
    app.finish_import(request.settle(Ok(profile())));
    assert_eq!(app.import.as_ref().unwrap().state(), State::Preview);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitImport);
    let request = app.take_import_request().unwrap();
    let p = request.params();
    assert_eq!(p["action"], "profile-import");
    assert_eq!(p["input"], PRIVATE);
    assert_eq!(p["instanceId"], "fixture-runtime");
    assert_eq!(p["expectedRevision"], 7);
    let saved = json!({"ok":true,"revision":8,"result":{"schemaVersion":1,"applied":true,
        "instanceId":p["instanceId"],"operationId":p["operationId"],"action":p["action"]}});
    app.finish_import(request.settle(Ok(saved)));
    assert_eq!(app.import.as_ref().unwrap().state(), State::Completed);
    assert_eq!(app.selected, selected);
    assert_eq!(
        app.snapshot.as_ref().unwrap().metadata.desired.connected,
        desired.connected
    );
    assert_eq!(
        app.snapshot.as_ref().unwrap().metadata.desired.profile_id,
        desired.profile_id
    );
}
#[test]
fn counts_only_bundle_uses_exact_input_and_preview_binding() {
    let now = Instant::now();
    let mut w = draft(1);
    preview(&mut w, bundle(2), now);
    let request = w.confirm("operation-one".into(), now).unwrap();
    assert_eq!(
        request.params(),
        json!({"instanceId":"fixture-runtime","expectedRevision":7,
        "operationId":"operation-one","action":"profile-bundle-import","input":PRIVATE,"previewFingerprint":"a".repeat(64)})
    );
    assert_eq!(w.input_bytes(), 0);
}
#[test]
fn local_snapshot_never_receives_subscription_or_fetch_authority() {
    let now = Instant::now();
    let mut w = draft(1);
    preview(&mut w, bundle(1), now);
    let p = w.confirm("operation-one".into(), now).unwrap().params();
    assert!(p.get("url").is_none());
    assert!(p.get("refresh").is_none());
    assert!(p.get("connect").is_none());
}
#[test]
fn subscription_confirmation_uses_explicit_add_and_duplicate_refuses() {
    let now = Instant::now();
    let mut w = draft(1);
    let value = |duplicate| {
        envelope(
            json!({"version":1,"kind":"subscription","suggestedName":"Synthetic feed","duplicate":duplicate}),
        )
    };
    preview(&mut w, value(false), now);
    let p = w.confirm("operation-one".into(), now).unwrap().params();
    assert_eq!(p["action"], "subscription-add");
    assert_eq!(p["url"], PRIVATE);
    let mut duplicate = draft(2);
    preview(&mut duplicate, value(true), now);
    assert_eq!(duplicate.state(), State::Rejected);
    assert!(duplicate.confirm("other-op".into(), now).is_none());
}
#[test]
fn empty_and_zero_new_bundles_never_send_a_mutation() {
    let now = Instant::now();
    let mut w = Workspace::new("fixture-runtime", 7, 1).unwrap();
    assert!(w.classify(now).is_none());
    assert!(w.append(" \n"));
    assert!(w.classify(now).is_none());
    w = draft(1);
    preview(&mut w, bundle(0), now);
    assert_eq!(w.state(), State::Preview);
    assert!(w.confirm("op".into(), now).is_none());
}
#[test]
fn whole_input_bounds_unicode_and_controls_refuse_without_partial_paste() {
    let mut w = Workspace::new("fixture-runtime", 7, 1).unwrap();
    assert!(w.append(&"x".repeat(MAX_INPUT - 1)));
    assert!(!w.append("я"));
    assert_eq!(w.input_bytes(), MAX_INPUT - 1);
    assert!(!w.append("\x1b[2J"));
    assert!(!w.append("\0"));
    assert!(!w.append("\x7f"));
    assert!(w.append("x"));
    assert!(!w.append("x"));
    w.clear();
    assert_eq!(w.input_bytes(), 0);
    assert!(w.append("первая\nвторая\r\tтретья"));
}
#[test]
fn cancel_and_late_classification_do_not_restore_private_input() {
    let now = Instant::now();
    let mut w = draft(1);
    let request = w.classify(now).unwrap();
    w.cancel();
    w.accept(request.settle(Ok(profile())));
    assert_eq!(w.state(), State::Changed);
    assert_eq!(w.input_bytes(), 0);
    assert!(w.preview().is_none());
}
#[test]
fn another_workspace_cannot_accept_the_original_preview() {
    let now = Instant::now();
    let mut first = draft(1);
    let request = first.classify(now).unwrap();
    let mut second = draft(2);
    second.classify(now).unwrap();
    second.accept(request.settle(Ok(profile())));
    assert_eq!(second.state(), State::Classifying);
    assert!(second.preview().is_none());
}
#[test]
fn metadata_change_invalidates_preview_and_input_before_save() {
    let now = Instant::now();
    for (id, rev, available) in [
        ("other-runtime", 7, true),
        ("fixture-runtime", 8, true),
        ("fixture-runtime", 7, false),
    ] {
        let mut w = draft(1);
        preview(&mut w, profile(), now);
        w.observe(id, rev, available, now);
        assert_eq!(w.state(), State::Changed);
        assert_eq!(w.input_bytes(), 0);
        assert!(w.confirm("op".into(), now).is_none());
    }
}
#[test]
fn malformed_preview_and_untrusted_error_are_never_rendered() {
    let now = Instant::now();
    for value in [
        json!({"ok":false,"revision":7,"error":{"message":PRIVATE}}),
        json!({"ok":true,"revision":8,"result":profile()["result"]}),
        envelope(
            json!({"version":1,"kind":"bundle","counts":{"accepted":257,"duplicates":0,"skipped":0},"previewFingerprint":"a".repeat(64)}),
        ),
        envelope(
            json!({"version":1,"kind":"subscription","suggestedName":"\x1b[2J","duplicate":false}),
        ),
    ] {
        let mut w = draft(1);
        preview(&mut w, value, now);
        assert_eq!(w.state(), State::Rejected);
        assert_eq!(w.input_bytes(), 0);
    }
}
#[test]
fn uncertain_save_blocks_mutations_and_cannot_be_revived_by_late_success() {
    let now = Instant::now();
    let mut w = draft(1);
    preview(&mut w, profile(), now);
    let request = w.confirm("original-op".into(), now).unwrap();
    let p = request.params();
    w.observe("other-runtime", 7, true, now);
    w.accept(request.settle(Ok(
        json!({"ok":true,"revision":8,"result":{"schemaVersion":1,"applied":true,
        "instanceId":p["instanceId"],"operationId":p["operationId"],"action":p["action"]}}),
    )));
    assert_eq!(w.state(), State::Unknown);
    assert!(w.blocks_actions());
    assert_eq!(w.input_bytes(), 0);
    w.cancel();
    assert_eq!(w.state(), State::Unknown);
    assert!(w.confirm("another".into(), now).is_none());
}
#[test]
fn hidden_unresolved_import_footer_keeps_state_review_and_close_visible_at_minimum_size() {
    for locale in [Locale::En, Locale::Ru] {
        for unknown in [false, true] {
            let (mut app, now) = app(locale);
            app.import_reasons = Some(
                omavless_domain::subscription_feed::ImportSummary::rejected(
                    omavless_domain::subscription_feed::ImportReason::UnsupportedField,
                )
                .reasons,
            );
            let mut w = draft(1);
            preview(&mut w, profile(), now);
            let request = w.confirm("original-op".into(), now).unwrap();
            if unknown {
                w.accept(request.settle(Err(ReadError::Unavailable)));
            }
            app.import = Some(w);
            let footer = locale.text(if unknown {
                "tui.import_unknown_footer"
            } else {
                "tui.import_pending_footer"
            });
            assert!(footer.chars().count() <= 68);
            let rendered = render(&app, now, 70, 24);
            assert!(rendered.contains(footer));
            assert!(!rendered.contains(PRIVATE));
            assert!(app.import_unresolved());
            assert_eq!(press(&mut app, KeyCode::Char('c'), now), Action::None);
            assert!(app.confirmation.is_none());
            assert_eq!(press(&mut app, KeyCode::F(3), now), Action::None);
            assert!(app.import_open);
            assert!(render(&app, now, 70, 24).contains(locale.text("tui.import_title")));
            assert_eq!(press(&mut app, KeyCode::Esc, now), Action::None);
            assert!(!app.import_open && app.import_unresolved());
            assert!(render(&app, now, 70, 24).contains(footer));
            assert_eq!(press(&mut app, KeyCode::Char('q'), now), Action::Close);
        }
    }
}
#[test]
fn ambiguous_store_recovery_is_unknown_not_safe_rejection_for_every_import_kind() {
    for value in [
        profile(),
        bundle(1),
        envelope(json!({"version":1,"kind":"subscription",
        "suggestedName":"Synthetic", "duplicate":false})),
    ] {
        let (mut app, now) = app(Locale::En);
        let mut w = draft(1);
        preview(&mut w, value, now);
        let request = w.confirm("original-op".into(), now).unwrap();
        w.accept(request.settle(Ok(json!({"ok":false,"revision":7,
            "error":{"code":"manual_recovery_required"}}))));
        assert_eq!(w.state(), State::Unknown);
        assert!(w.blocks_actions());
        assert!(w.confirm("replacement-op".into(), now).is_none());
        app.import = Some(w);
        assert!(app.import_unresolved());
        assert_eq!(press(&mut app, KeyCode::Char('c'), now), Action::None);
        assert!(app.confirmation.is_none());
        assert_eq!(press(&mut app, KeyCode::F(3), now), Action::None);
        assert!(app.import_open);
        assert_eq!(press(&mut app, KeyCode::Enter, now), Action::None);
        assert!(app.take_import_request().is_none());
        press(&mut app, KeyCode::Esc, now);
        assert!(!app.import_open && app.import_unresolved());
        assert_eq!(press(&mut app, KeyCode::Char('q'), now), Action::Close);
    }
}
#[test]
fn wrong_mutation_operation_action_or_instance_never_reports_saved() {
    let now = Instant::now();
    for (field, bad) in [
        ("operationId", "different"),
        ("instanceId", "other"),
        ("action", "connect"),
    ] {
        let mut w = draft(1);
        preview(&mut w, profile(), now);
        let request = w.confirm("op".into(), now).unwrap();
        let mut r = json!({"schemaVersion":1,"applied":true,"instanceId":"fixture-runtime","operationId":"op","action":"profile-import"});
        r[field] = json!(bad);
        w.accept(request.settle(Ok(json!({"ok":true,"revision":8,"result":r}))));
        assert_eq!(w.state(), State::Unknown);
    }
}
#[test]
fn known_queue_refusal_differs_from_dispatched_transport_failure() {
    let now = Instant::now();
    for sent in [false, true] {
        let mut w = draft(1);
        preview(&mut w, profile(), now);
        let request = w.confirm("op".into(), now).unwrap();
        w.accept(if sent {
            request.settle(Err(ReadError::Unavailable))
        } else {
            request.not_sent()
        });
        assert_eq!(
            w.state(),
            if sent {
                State::Unknown
            } else {
                State::Rejected
            }
        );
    }
}
#[test]
fn deadline_and_worker_loss_are_read_or_mutation_specific() {
    let now = Instant::now();
    for save in [false, true] {
        for loss in [false, true] {
            let mut w = draft(1);
            if save {
                preview(&mut w, profile(), now);
                w.confirm("op".into(), now).unwrap();
            } else {
                w.classify(now).unwrap();
            }
            if loss {
                w.worker_lost();
            } else {
                w.observe("fixture-runtime", 7, true, now + Duration::from_secs(131));
            }
            assert_eq!(
                w.state(),
                if save {
                    State::Unknown
                } else {
                    State::Rejected
                }
            );
            assert_eq!(w.input_bytes(), 0);
        }
    }
}
#[test]
fn editing_q_is_data_but_control_c_always_closes_only_ui() {
    let (mut app, now) = app(Locale::En);
    press(&mut app, KeyCode::F(3), now);
    assert_eq!(press(&mut app, KeyCode::Char('q'), now), Action::None);
    assert_eq!(app.import.as_ref().unwrap().input_bytes(), 1);
    assert_eq!(
        app.key_at(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            now
        ),
        Action::Close
    );
}
#[test]
fn minimum_viewport_locale_states_are_masked_and_exit_hint_remains_visible() {
    for locale in [Locale::En, Locale::Ru] {
        for value in [profile(), bundle(4)] {
            let (mut app, now) = app(locale);
            press(&mut app, KeyCode::F(3), now);
            app.import_paste(PRIVATE.into());
            for (width, height) in [(70, 24), (110, 35)] {
                let text = render(&app, now, width, height);
                assert!(!text.contains(PRIVATE));
                assert!(text.contains("Ctrl+C"));
                assert!(!text.contains("Missing translation"));
            }
            press(&mut app, KeyCode::Enter, now);
            let request = app.take_import_request().unwrap();
            app.finish_import(request.settle(Ok(value)));
            let text = render(&app, now, 70, 24);
            assert!(text.contains("Enter"));
            assert!(text.contains("Ctrl+C"));
            assert!(!text.contains(PRIVATE));
        }
    }
}
#[test]
fn unavailable_capability_or_small_viewport_never_opens_import() {
    for cap in [false, true] {
        let (mut app, now) = app(Locale::En);
        if cap {
            app.viewport_ready = false;
        } else {
            app.snapshot.as_mut().unwrap().capabilities.private_import = false;
        }
        press(&mut app, KeyCode::F(3), now);
        assert!(!app.import_open);
        assert!(app.take_import_request().is_none());
    }
}

#[test]
fn definitive_subscription_fetch_failure_is_not_an_unknown_save() {
    let now = Instant::now();
    for kind in ["subscription", "profile"] {
        let mut w = draft(1);
        let value = if kind == "subscription" {
            envelope(
                json!({"version":1,"kind":"subscription","suggestedName":"Synthetic feed","duplicate":false}),
            )
        } else {
            profile()
        };
        preview(&mut w, value, now);
        let request = w.confirm("op".into(), now).unwrap();
        w.accept(request.settle(Ok(json!({"ok":false,"revision":7,"error":{"code":"subscription_unavailable","message":PRIVATE,"retryable":false}}))));
        assert_eq!(
            w.state(),
            if kind == "subscription" {
                State::Rejected
            } else {
                State::Unknown
            }
        );
    }
}
#[test]
fn portable_control_n_adds_newline_without_classification_or_confirmation() {
    let (mut app, now) = app(Locale::En);
    press(&mut app, KeyCode::F(3), now);
    app.import_paste("first".into());
    assert_eq!(
        app.key_at(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            now
        ),
        Action::None
    );
    app.import_paste("second".into());
    assert!(app.take_import_request().is_none());
    assert_eq!(press(&mut app, KeyCode::Enter, now), Action::SubmitImport);
    assert_eq!(
        app.take_import_request().unwrap().params()["input"],
        "first\nsecond"
    );
}
#[test]
fn page_legend_matches_actual_global_function_key() {
    use omavless_tui::inspection::Page;
    let (mut app, now) = app(Locale::En);
    for page in [
        Page::Profiles,
        Page::Settings,
        Page::Subscriptions,
        Page::Traffic,
    ] {
        app.page = page;
        let text = render(&app, now, 70, 24);
        assert!(text.contains(if page == Page::Profiles {
            "i/F3:"
        } else {
            "F3:"
        }));
        if page != Page::Profiles {
            assert!(!text.contains("i/F3:"));
        }
        press(&mut app, KeyCode::F(3), now);
        assert!(app.import_open);
        press(&mut app, KeyCode::Esc, now);
    }
}
#[test]
fn terminal_paste_preserves_existing_editors_and_never_replays_shortcuts_or_enter() {
    use omavless_tui::inspection::Page;
    use zeroize::Zeroizing;
    let (mut app, now) = app(Locale::En);
    app.paste_input(Zeroizing::new("cd123q\n".into()), now);
    assert!(app.confirmation.is_none());
    assert!(!app.running);
    assert!(app.query.is_empty());
    app.searching = true;
    app.paste_input(Zeroizing::new("Fixture Helsinki\nc".into()), now);
    assert_eq!(app.query, "Fixture Helsinkic");
    assert!(app.searching);
    assert!(app.confirmation.is_none());
    app.query.clear();
    app.page = Page::Rules;
    app.paste_input(Zeroizing::new("DOMAIN".into()), now);
    assert_eq!(app.operator_query, "DOMAIN");
    app.searching = false;
    app.page = Page::RouteCheck;
    app.route_editing = true;
    app.paste_input(Zeroizing::new("example.invalid\n".into()), now);
    assert_eq!(app.route_query, "example.invalid");
    assert!(app.route_editing);
    assert!(app.route_request.is_none());
}
#[cfg(feature = "private-backup")]
#[test]
fn bracketed_paste_routes_only_to_active_backup_restore_field_with_atomic_bounds() {
    use omavless_tui::{private_backup, private_restore};
    use zeroize::Zeroizing;
    let (mut app, now) = app(Locale::En);
    app.backup = private_backup::Workspace::new("fixture-runtime", 7);
    app.backup_open = true;
    app.paste_input(Zeroizing::new("/private/synthetic.ovb".into()), now);
    assert_eq!(
        app.backup.as_ref().unwrap().destination(),
        "/private/synthetic.ovb"
    );
    app.paste_input(Zeroizing::new("\n/q\0".into()), now);
    assert_eq!(
        app.backup.as_ref().unwrap().destination(),
        "/private/synthetic.ovb"
    );
    assert!(app.take_backup_request().is_none());
    app.backup_open = false;
    app.restore = private_restore::Workspace::new("fixture-runtime", 7);
    app.restore_open = true;
    app.paste_input(Zeroizing::new("/private/synthetic.ovb".into()), now);
    assert_eq!(
        app.restore.as_ref().unwrap().archive(),
        "/private/synthetic.ovb"
    );
    app.restore.as_mut().unwrap().next_field();
    app.paste_input(Zeroizing::new("synthetic-only-passphrase".into()), now);
    assert_eq!(app.restore.as_ref().unwrap().masked(), "********");
    assert!(app.take_restore_request().is_none());
}
#[test]
fn profile_security_warning_is_visible_without_raw_credentials() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut app, now) = app(locale);
        press(&mut app, KeyCode::F(3), now);
        app.import_paste(PRIVATE.into());
        press(&mut app, KeyCode::Enter, now);
        let request = app.take_import_request().unwrap();
        let mut value = profile();
        value["result"]["profile"]["insecure"] = json!(true);
        value["result"]["profile"]["suggestedName"] = json!("Synthetic ".repeat(7));
        app.finish_import(request.settle(Ok(value)));
        let text = render(&app, now, 70, 24);
        assert!(text.contains(if locale == Locale::En {
            "Warning:"
        } else {
            "Внимание:"
        }));
        assert!(text.contains("Ctrl+C"));
        assert!(text.contains("Enter"));
        assert!(!text.contains(PRIVATE));
    }
}

#[test]
fn completed_save_requires_post_reply_metadata_at_least_the_applied_revision() {
    let (mut app, now) = app(Locale::En);
    press(&mut app, KeyCode::F(3), now);
    app.import_paste(PRIVATE.into());
    press(&mut app, KeyCode::Enter, now);
    let request = app.take_import_request().unwrap();
    app.finish_import(request.settle(Ok(profile())));
    press(&mut app, KeyCode::Enter, now);
    let request = app.take_import_request().unwrap();
    let p = request.params();
    app.finish_import(request.settle(Ok(
        json!({"ok":true,"revision":8,"result":{"schemaVersion":1,"applied":true,
        "instanceId":p["instanceId"],"operationId":p["operationId"],"action":p["action"]}}),
    )));
    assert!(!app.import_available(Instant::now()));
    press(&mut app, KeyCode::Esc, Instant::now());
    sample(&mut app, Instant::now()); // old revision7, even though sampled later
    assert!(app.error.is_some());
    assert!(!app.import_available(Instant::now()));
    let s = load(&mut |read| {
        let mut v = support::response(read);
        v["revision"] = json!(8);
        if read == Read::Capabilities {
            v["result"]["methods"]
                .as_array_mut()
                .unwrap()
                .extend([json!("plugin.action"), json!("imports.classify")]);
        }
        Ok(v)
    })
    .unwrap();
    app.accept(Ok(s), Instant::now());
    assert!(app.import_available(Instant::now()));
}
