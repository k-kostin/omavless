// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    actions::Kind as Command,
    app::{Action, App},
    client::{self, Read},
    i18n::Locale,
    inspection::Page,
    job_ui::{Phase, Session},
    jobs::Kind,
    model::ReadError,
    view,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use serde_json::{Value, json};
use std::time::Instant;
const ID: &str = "10000000-0000-4000-8000-000000000001";
const SUB: &str = "20000000-0000-4000-8000-000000000002";
fn app() -> (App, Instant) {
    let now = Instant::now();
    let mut a = App::new(Locale::En);
    a.actions_enabled = true;
    a.jobs_enabled = true;
    a.accept(
        client::load(&mut |r| {
            let mut v = support::response(r);
            if r == Read::Snapshot {
                v["result"]["desired"]["profileId"] = json!(ID);
                v["result"]["profiles"][0]["id"] = json!(ID);
                v["result"]["subscriptions"][0]["id"] = json!(SUB);
                v["result"]["profiles"][1]["subscriptionId"] = json!(SUB);
            }
            if r == Read::Capabilities {
                v["result"]["methods"].as_array_mut().unwrap().extend([
                    json!("plugin.action"),
                    json!("profiles.probe"),
                    json!("profiles.probe_results"),
                    json!("subscriptions.refresh_all"),
                    json!("subscriptions.refresh_selected"),
                    json!("operations.get"),
                    json!("operations.cancel"),
                ]);
            }
            Ok(v)
        }),
        now,
    );
    a.selected = Some(ID.into());
    (a, now)
}
fn start(a: &mut App, now: Instant, kind: Kind, target: Option<String>) {
    a.prepare_job(kind, target, now);
    assert!(a.confirmation.is_some());
    assert_eq!(a.confirm(now, "job-test".into()), Action::StartJob);
    a.job.as_mut().unwrap().call(Phase::Start).unwrap();
}
fn receipt(state: &str, completed: usize, total: usize) -> Value {
    let terminal = matches!(state, "succeeded" | "cancelled");
    json!({"ok":true,"revision":7,"result":{"operation":{"instanceId":"fixture-runtime","operationId":"job-test","method":"profiles.probe","baseRevision":7,"outcomeRevision":terminal.then_some(7),"state":state,"progress":{"completed":completed,"total":total},"cancellable":!terminal,"cancelRequested":state=="cancelled","error":null}}})
}
#[test]
fn job_start_is_explicit_fenced_and_not_navigation() {
    let (mut a, now) = app();
    a.key_at(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), now);
    assert!(a.confirmation.is_some() && a.job.is_none());
    a.key_at(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);
    assert!(a.job.is_none());
    start(&mut a, now, Kind::ProfileProbe, Some(ID.into()));
    let call = a.job.as_ref().unwrap().request.start();
    assert_eq!(call.method(), "profiles.probe");
    assert_eq!(call.params()["profileId"], ID);
    assert!(a.page == Page::Jobs);
}
#[test]
fn selected_subscription_refresh_enters_jobs_with_its_own_cancel_handle() {
    let (mut a, now) = app();
    a.page = Page::Subscriptions;
    a.selected_subscription = Some(SUB.into());
    assert_eq!(
        a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now),
        Action::None
    );
    assert!(a.job.is_none());
    assert_eq!(a.confirm(now, "selected-job".into()), Action::StartJob);
    assert!(a.page == Page::Jobs);
    let job = a.job.as_mut().unwrap();
    let start = job.call(Phase::Start).unwrap();
    assert_eq!(start.method(), "subscriptions.refresh_selected");
    assert_eq!(
        start.params(),
        json!({"instanceId":"fixture-runtime","operationId":"selected-job","expectedRevision":7,"subscriptionId":SUB})
    );
    job.accept(Phase::Start, Err(ReadError::Unavailable));
    assert!(matches!(job.next(now), Some(Phase::Poll)));
    let poll = job.call(Phase::Poll).unwrap();
    assert_eq!(poll.method(), "operations.get");
    assert_eq!(
        poll.params(),
        json!({"instanceId":"fixture-runtime","operationId":"selected-job"})
    );
    assert!(job.blocks_actions());
}

fn selected_receipt(state: &str) -> Value {
    let terminal = matches!(state, "succeeded" | "failed" | "cancelled");
    let revision = if state == "succeeded" { 8 } else { 7 };
    json!({"ok":true,"revision":revision,"result":{"operation":{
        "instanceId":"fixture-runtime","operationId":"selected-job",
        "method":"subscriptions.refresh_selected","baseRevision":7,
        "outcomeRevision":terminal.then_some(revision),"state":state,
        "progress":{"completed":u8::from(state=="succeeded"),"total":1},
        "cancellable":!terminal,"cancelRequested":state=="cancelled","error":null
    }}})
}
fn start_selected(a: &mut App, now: Instant) {
    a.page = Page::Subscriptions;
    a.selected_subscription = Some(SUB.into());
    a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now);
    assert_eq!(a.confirm(now, "selected-job".into()), Action::StartJob);
    a.job.as_mut().unwrap().call(Phase::Start).unwrap();
}
#[test]
fn selected_refresh_success_reports_the_import_summary_not_probe_results() {
    let (mut a, now) = app();
    start_selected(&mut a, now);
    let mut value = selected_receipt("succeeded");
    value["result"]["operation"]["importSummary"] = json!({
        "accepted":1,"duplicates":0,"skipped":0,"skipReasons":{}
    });
    let job = a.job.as_mut().unwrap();
    job.accept(Phase::Start, Ok(value));
    assert!(job.finished);
    assert_eq!(job.notice, "tui.job_succeeded");
    assert_eq!(
        job.tracker
            .progress
            .unwrap()
            .import_summary
            .unwrap()
            .accepted,
        1
    );
    assert!(job.next(now).is_none());
    assert!(job.request.results_call().is_none());
}

#[test]
fn selected_refresh_malformed_initial_error_is_unknown_not_safe_rejection() {
    let (mut a, now) = app();
    start_selected(&mut a, now);
    let job = a.job.as_mut().unwrap();
    job.accept(
        Phase::Start,
        Ok(json!({"ok":false,"revision":7,"error":{"code":"busy"}})),
    );
    assert!(!job.finished && job.tracker.unknown);
    assert_eq!(job.notice, "tui.job_unknown");
}

#[test]
fn selected_refresh_outer_result_and_phase_shapes_are_exact() {
    for case in 0..5 {
        let (mut a, now) = app();
        start_selected(&mut a, now);
        let mut value = selected_receipt("succeeded");
        match case {
            0 => value["result"]["url"] = json!("https://private.invalid/token"),
            1 => value["result"]["accepted"] = json!("yes"),
            2 => value["result"]["accepted"] = json!(true),
            3 => {
                value = selected_receipt("failed");
                value["result"]["operation"]["error"] = json!({"code":"not-a-native-error","message":"private payload","retryable":false});
            }
            _ => {
                value = json!({"ok":false,"revision":7,"error":{"code":"busy","message":"Busy","retryable":true},"result":{"private":"payload"}});
            }
        }
        let job = a.job.as_mut().unwrap();
        job.accept(Phase::Start, Ok(value));
        assert!(!job.finished && job.tracker.unknown, "case {case}");
    }
    for code in [
        "invalid_request",
        "unsupported_version",
        "unknown_method",
        "invalid_argument",
        "not_found",
        "conflict",
        "busy",
        "capability_unavailable",
        "dns_pair_required",
        "permission_denied",
        "core_rejected",
        "subscription_unavailable",
        "transition_failed_restored",
        "manual_recovery_required",
        "daemon_restarting",
        "internal_error",
    ] {
        let (mut a, now) = app();
        start_selected(&mut a, now);
        let mut value = selected_receipt("failed");
        value["result"]["operation"]["error"] =
            json!({"code":code,"message":"bounded native text","retryable":false});
        a.job.as_mut().unwrap().accept(Phase::Start, Ok(value));
        assert!(a.job.as_ref().unwrap().finished, "valid native code {code}");
    }
}

#[test]
fn selected_refresh_cancel_ack_is_phase_bound_and_is_not_terminal_proof() {
    for valid in [true, false] {
        let (mut a, now) = app();
        start_selected(&mut a, now);
        let job = a.job.as_mut().unwrap();
        job.accept(Phase::Start, Ok(selected_receipt("running")));
        job.call(Phase::Cancel).unwrap();
        let mut value = selected_receipt("running");
        value["result"]["operation"]["cancelRequested"] = json!(true);
        if valid {
            value["result"]["accepted"] = json!(true);
        }
        job.accept(Phase::Cancel, Ok(value));
        assert!(!job.finished);
        assert_eq!(job.tracker.unknown, !valid);
        assert_eq!(
            job.notice,
            if valid {
                "tui.job_cancelling"
            } else {
                "tui.job_unknown"
            }
        );
    }
}

#[test]
fn selected_refresh_confirmation_rechecks_subscription_and_runtime_not_profile_selection() {
    for case in 0..9 {
        let (mut a, now) = app();
        a.page = Page::Subscriptions;
        a.selected_subscription = Some(SUB.into());
        a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now);
        match case {
            0 => a.selected_subscription = None,
            1 => a.snapshot.as_mut().unwrap().metadata.subscriptions.clear(),
            2 => a.snapshot.as_mut().unwrap().metadata.subscriptions[0].name = "Renamed".into(),
            3 => a.snapshot.as_mut().unwrap().revision += 1,
            4 => a.snapshot.as_mut().unwrap().metadata.instance_id = "new-owner".into(),
            5 => a.page = Page::Profiles,
            6 => a.jobs_enabled = false,
            7 => a.snapshot.as_mut().unwrap().capabilities.refresh_selected = false,
            _ => a.sampled_at = Some(now - omavless_tui::app::FRESH_FOR),
        }
        assert_eq!(
            a.confirm(now, "selected-job".into()),
            Action::None,
            "case {case}"
        );
        assert!(a.job.is_none() && a.pending.is_none());
    }
}

#[test]
fn selected_refresh_cancel_and_restart_never_resubmit_or_relabel_unknown() {
    let (mut a, now) = app();
    start_selected(&mut a, now);
    a.job
        .as_mut()
        .unwrap()
        .accept(Phase::Start, Ok(selected_receipt("running")));
    a.key_at(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), now);
    assert_eq!(a.confirm(now, "not-a-new-start".into()), Action::CancelJob);
    let job = a.job.as_mut().unwrap();
    let cancel = job.call(Phase::Cancel).unwrap();
    assert_eq!(cancel.method(), "operations.cancel");
    assert_eq!(
        cancel.params(),
        json!({"instanceId":"fixture-runtime","operationId":"selected-job"})
    );
    job.accept(Phase::Cancel, Err(ReadError::Unavailable));
    assert!(!job.finished && job.tracker.unknown);
    assert!(job.call(Phase::Cancel).is_none());
    job.accept(
        Phase::Poll,
        Ok(json!({"ok":false,"revision":0,"error":{
        "code":"not_found","message":"Not found","retryable":false}})),
    );
    assert!(!job.finished && job.blocks_actions());
    a.page = Page::Subscriptions;
    a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now);
    assert!(a.confirmation.is_none());
    a.page = Page::Jobs;
    assert_eq!(
        a.key_at(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), now),
        Action::Close
    );
    a.job
        .as_mut()
        .unwrap()
        .accept(Phase::Poll, Ok(selected_receipt("cancelled")));
    assert!(a.job.as_ref().unwrap().finished);
    assert_eq!(a.job.as_ref().unwrap().notice, "tui.job_cancelled");
}

#[test]
fn selected_refresh_preserves_older_runtime_and_read_only_client_behavior() {
    for legacy in [true, false] {
        let (mut a, now) = app();
        a.page = Page::Subscriptions;
        a.selected_subscription = Some(SUB.into());
        if legacy {
            a.snapshot.as_mut().unwrap().capabilities.refresh_selected = false;
        } else {
            a.jobs_enabled = false;
        }
        a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now);
        assert_eq!(a.confirm(now, "legacy".into()), Action::Submit);
        assert_eq!(a.pending.as_ref().unwrap().params()["subscriptionId"], SUB);
        assert!(a.job.is_none());
    }
    let (mut a, now) = app();
    a.actions_enabled = false;
    a.page = Page::Subscriptions;
    a.selected_subscription = Some(SUB.into());
    a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now);
    assert!(a.confirmation.is_none() && a.job.is_none());
}

#[test]
fn selected_refresh_receipt_rejects_wrong_handle_method_count_revision_and_private_fields() {
    for case in 0..17 {
        let (mut a, now) = app();
        start_selected(&mut a, now);
        let mut value = selected_receipt("succeeded");
        let op = &mut value["result"]["operation"];
        match case {
            0 => op["instanceId"] = json!("new-owner"),
            1 => op["operationId"] = json!("foreign-op"),
            2 => op["method"] = json!("subscriptions.refresh_all"),
            3 => op["progress"]["total"] = json!(0),
            4 => op["progress"]["total"] = json!(2),
            5 => op["progress"]["total"] = json!(true),
            6 => op["baseRevision"] = json!(8),
            7 => op["outcomeRevision"] = json!(7),
            8 => op["outcomeRevision"] = json!(9),
            9 => op["cancellable"] = json!(true),
            10 => op["subscriptionId"] = json!(SUB),
            11 => op["url"] = json!("https://private.invalid/token"),
            12 => {
                op["importSummary"] =
                    json!({"accepted":true,"duplicates":0,"skipped":0,"skipReasons":{}})
            }
            13 => {
                op["importSummary"] =
                    json!({"accepted":1,"duplicates":0,"skipped":0,"skipReasons":{"private-url":1}})
            }
            14 => op["progress"]["completed"] = json!(0),
            15 => op["cancelRequested"] = json!("false"),
            _ => value["revision"] = json!(7),
        }
        let job = a.job.as_mut().unwrap();
        job.accept(Phase::Start, Ok(value));
        assert!(!job.finished && job.tracker.unknown, "case {case}");
    }
    let (mut a, now) = app();
    start_selected(&mut a, now);
    let mut failed = selected_receipt("failed");
    failed["result"]["operation"]["error"] = json!({"code":"invalid_argument",
        "message":"synthetic private remote text","retryable":false,
        "details":{"importReasons":{"unsupported_field":1}}});
    let job = a.job.as_mut().unwrap();
    job.accept(Phase::Start, Ok(failed));
    assert!(job.finished);
    assert_eq!(
        job.tracker
            .progress
            .unwrap()
            .import_reasons
            .unwrap()
            .value(),
        json!({"unsupported_field":1})
    );
}

#[test]
fn selected_refresh_enru_rendered_states_keep_the_target_exit_and_privacy_boundary() {
    for locale in [Locale::En, Locale::Ru] {
        for scenario in [
            "confirm",
            "long-confirm",
            "running",
            "cancel-confirm",
            "cancelled",
            "succeeded",
            "failed",
            "unknown",
            "unknown-end",
            "failed-end",
        ] {
            let (mut a, now) = app();
            a.locale = locale;
            if scenario == "long-confirm" {
                a.snapshot.as_mut().unwrap().metadata.subscriptions[0].name =
                    "Synthetic subscription ".repeat(3);
            }
            a.page = Page::Subscriptions;
            a.selected_subscription = Some(SUB.into());
            a.key_at(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), now);
            if !matches!(scenario, "confirm" | "long-confirm") {
                assert_eq!(a.confirm(now, "selected-job".into()), Action::StartJob);
                let job = a.job.as_mut().unwrap();
                job.call(Phase::Start).unwrap();
                if scenario.starts_with("unknown") {
                    job.accept(Phase::Start, Err(ReadError::Unavailable));
                } else {
                    let state = match scenario {
                        "succeeded" => "succeeded",
                        "cancelled" => "cancelled",
                        s if s.starts_with("failed") => "failed",
                        _ => "running",
                    };
                    let mut value = selected_receipt(state);
                    if state == "failed" {
                        value["result"]["operation"]["error"] = json!({"code":"invalid_argument","message":"https://private.invalid/token","retryable":false,"details":{"importReasons":{"unsupported_field":1}}});
                    }
                    if state == "succeeded" {
                        value["result"]["operation"]["importSummary"] = json!({"accepted":3,"duplicates":1,"skipped":0,"skipReasons":{"duplicate":1}});
                    }
                    job.accept(Phase::Start, Ok(value));
                    if scenario == "cancel-confirm" {
                        a.key_at(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), now);
                    }
                }
            }
            if scenario.ends_with("-end") {
                a.inspection_scroll = u16::MAX;
            }
            let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
            terminal.draw(|frame| view::draw(frame, &a, now)).unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(
                text.contains(locale.text("tui.action_close_hint")),
                "{locale:?}/{scenario}"
            );
            assert!(
                !text.contains(SUB)
                    && !text.contains("selected-job")
                    && !text.contains("private.invalid")
                    && !text.contains(locale.text("tui.probe_scope"))
            );
            if !scenario.ends_with("-end") {
                assert!(text.contains(locale.text("tui.refresh_subscription")));
            }
            if let Some(directory) = std::env::var_os("OVT_TUI_RENDER_DIR") {
                write_render(
                    &terminal,
                    &std::path::PathBuf::from(directory)
                        .join(format!("{locale:?}-{scenario}-70.svg")),
                );
            }
        }
        let (mut a, now) = app();
        a.locale = locale;
        start_selected(&mut a, now);
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
        terminal.draw(|frame| view::draw(frame, &a, now)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains(locale.text("tui.action_resize")));
        if let Some(directory) = std::env::var_os("OVT_TUI_RENDER_DIR") {
            write_render(
                &terminal,
                &std::path::PathBuf::from(directory).join(format!("{locale:?}-resize-60.svg")),
            );
        }
    }
}

// Optional synthetic cell export for personal visual review, never private IPC
// or an installed-screen claim. Existing files are never overwritten.
fn write_render(terminal: &Terminal<TestBackend>, path: &std::path::Path) {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let width = terminal.backend().buffer().area.width;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"480\"><rect width=\"100%\" height=\"100%\" fill=\"black\"/><g fill=\"white\" font-family=\"DejaVu Sans Mono\" font-size=\"14\">",
        width * 10
    );
    for (i, cell) in terminal.backend().buffer().content.iter().enumerate() {
        let text = cell
            .symbol()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;");
        if !text.trim().is_empty() {
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\">{text}</text>",
                (i % usize::from(width)) * 10,
                (i / usize::from(width)) * 20 + 16
            ));
        }
    }
    svg.push_str("</g></svg>\n");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(svg.as_bytes()).unwrap();
}
#[test]
fn stale_confirmation_changed_selection_and_old_runtime_block_start() {
    for case in 0..5 {
        let (mut a, now) = app();
        a.prepare_job(Kind::ProfileProbe, Some(ID.into()), now);
        match case {
            0 => a.selected = None,
            1 => a.snapshot.as_mut().unwrap().revision += 1,
            2 => a.snapshot.as_mut().unwrap().metadata.instance_id = "next".into(),
            3 => a.jobs_enabled = false,
            _ => a.snapshot.as_mut().unwrap().capabilities.profile_probe = false,
        }
        assert_eq!(a.confirm(now, "job-test".into()), Action::None);
        assert!(a.job.is_none());
    }
    let (mut a, now) = app();
    a.snapshot.as_mut().unwrap().capabilities.profile_probe = false;
    a.prepare_job(Kind::ProfileProbe, None, now);
    assert!(a.confirmation.is_none());
}
#[test]
fn lost_start_only_polls_same_operation_and_never_blocks_explicit_disconnect() {
    let (mut a, now) = app();
    start(&mut a, now, Kind::ProfileProbe, Some(ID.into()));
    let job = a.job.as_mut().unwrap();
    job.accept(Phase::Start, Err(ReadError::Unavailable));
    assert!(job.blocks_actions());
    assert!(matches!(job.next(now), Some(Phase::Poll)));
    let poll = job.call(Phase::Poll).unwrap();
    assert_eq!(poll.method(), "operations.get");
    assert_eq!(poll.params()["operationId"], "job-test");
    job.accept(
        Phase::Poll,
        Ok(json!({"ok":false,"revision":7,"error":{"code":"not_found"}})),
    );
    assert!(!job.finished && job.tracker.unknown);
    a.prepare(Command::Connect, None, now);
    assert!(a.confirmation.is_none());
    a.prepare(Command::Disconnect, None, now);
    assert!(a.confirmation.is_some());
}
#[test]
fn cancel_does_not_claim_cancelled_until_receipt_and_close_does_not_cancel() {
    let (mut a, now) = app();
    start(&mut a, now, Kind::ProfileProbe, None);
    a.job
        .as_mut()
        .unwrap()
        .accept(Phase::Start, Ok(receipt("running", 0, 2)));
    assert_eq!(
        a.key_at(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), now),
        Action::Close
    );
    assert!(!a.job.as_ref().unwrap().finished);
    a.key_at(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), now);
    assert_eq!(a.confirm(now, String::new()), Action::CancelJob);
    let job = a.job.as_mut().unwrap();
    assert!(job.call(Phase::Cancel).is_some());
    job.accept(Phase::Cancel, Err(ReadError::Unavailable));
    assert!(!job.finished);
    assert!(job.call(Phase::Cancel).is_none());
    job.accept(Phase::Poll, Ok(receipt("cancelled", 0, 2)));
    assert!(job.finished);
}
#[test]
fn result_count_must_match_terminal_receipt_and_old_revision_is_not_relabelled() {
    let (mut a, now) = app();
    start(&mut a, now, Kind::ProfileProbe, None);
    let job = a.job.as_mut().unwrap();
    job.accept(Phase::Start, Ok(receipt("succeeded", 2, 2)));
    assert!(matches!(job.next(now), Some(Phase::Results)));
    job.accept(
        Phase::Results,
        Ok(json!({"ok":true,"revision":7,"result":{"version":1,"profileId":null,"results":[]}})),
    );
    assert!(job.rows.is_none());
    assert_eq!(job.notice, "tui.job_results_unavailable");
}
#[test]
fn job_target_and_exit_remain_visible_in_both_languages_without_private_ids() {
    for locale in [Locale::En, Locale::Ru] {
        let (mut a, now) = app();
        a.locale = locale;
        start(&mut a, now, Kind::ProfileProbe, Some(ID.into()));
        let mut t = Terminal::new(TestBackend::new(70, 24)).unwrap();
        t.draw(|f| view::draw(f, &a, now)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Fixture Helsinki"));
        assert!(text.contains(locale.text("tui.action_close_hint")));
        assert!(!text.contains(ID));
    }
}
#[test]
fn read_only_client_and_missing_capability_do_not_prepare_jobs() {
    let (mut a, now) = app();
    a.actions_enabled = false;
    a.prepare_job(Kind::RefreshAll, None, now);
    assert!(a.confirmation.is_none());
    a.actions_enabled = true;
    a.jobs_enabled = false;
    a.prepare_job(Kind::ProfileProbe, None, now);
    assert!(a.confirmation.is_none());
}
#[test]
fn initial_known_rejection_finishes_but_remote_error_text_is_discarded() {
    let (mut a, now) = app();
    start(&mut a, now, Kind::ProfileProbe, None);
    let job: &mut Session = a.job.as_mut().unwrap();
    job.accept(Phase::Start,Ok(json!({"ok":false,"revision":7,"error":{"code":"busy","message":"https://private.invalid/secret"}})));
    assert!(job.finished);
    assert_eq!(job.notice, "tui.action_busy");
    assert!(job.next(now).is_none());
}
