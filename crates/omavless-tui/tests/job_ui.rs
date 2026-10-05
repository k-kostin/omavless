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
            }
            if r == Read::Capabilities {
                v["result"]["methods"].as_array_mut().unwrap().extend([
                    json!("plugin.action"),
                    json!("profiles.probe"),
                    json!("profiles.probe_results"),
                    json!("subscriptions.refresh_all"),
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
