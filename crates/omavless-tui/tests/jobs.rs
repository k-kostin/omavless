// SPDX-License-Identifier: MIT
use omavless_tui::jobs::{Kind, Progress, Request, State, Tracker, Update, WATCH_BUDGET};
use omavless_tui::model::ReadError;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const TARGET: &str = "10000000-0000-4000-8000-000000000001";
#[test]
fn manual_reads_can_resume_expired_watch_but_are_rate_limited() {
    let now = Instant::now();
    let mut t = Tracker::new(now);
    assert!(t.poll_due(now));
    assert!(!t.manual_poll_due(now));
    let later = now + WATCH_BUDGET;
    assert!(!t.poll_due(later));
    assert!(t.manual_poll_due(later));
    assert!(!t.manual_poll_due(later + Duration::from_millis(999)));
    assert!(t.manual_poll_due(later + Duration::from_secs(1)));
}
fn request(kind: Kind) -> Request {
    Request::new(
        kind,
        "owner".into(),
        7,
        "operation".into(),
        (kind != Kind::RefreshAll).then(|| TARGET.to_owned()),
    )
    .unwrap()
}
fn receipt(kind: Kind, state: &str, completed: u64, total: u64) -> Value {
    let terminal = matches!(state, "succeeded" | "failed" | "cancelled");
    let revision = 7 + u64::from(kind == Kind::RefreshAll && total > 0 && state == "succeeded");
    json!({"ok":true,"revision":revision,"result":{"operation":{
        "instanceId":"owner","operationId":"operation","method":kind.method(),
        "state":state,"baseRevision":7,"outcomeRevision":terminal.then_some(revision),
        "progress":{"completed":completed,"total":total},
        "cancelRequested":state == "cancelled","cancellable":!terminal,
        "error":if state == "failed" {json!({"code":"core_rejected","message":"private input","retryable":false})} else {Value::Null}
    }}})
}

#[test]
fn fixed_requests_and_capability_gates_cover_exact_selected_and_all_shapes() {
    for target in [
        TARGET.to_owned(),
        TARGET.replace('-', ""),
        format!("{{{TARGET}}}"),
        format!("urn:uuid:{TARGET}"),
    ] {
        let request = Request::new(
            Kind::ProfileProbe,
            "owner".into(),
            7,
            "op".into(),
            Some(target.clone()),
        )
        .unwrap();
        assert_eq!(request.start().params()["profileId"], target);
    }
    for kind in [
        Kind::RefreshAll,
        Kind::SubscriptionProbe,
        Kind::ProfileProbe,
    ] {
        let req = request(kind);
        assert_eq!(req.start().method(), kind.method());
        assert_eq!(req.start().params()["expectedRevision"], 7);
        assert_eq!(req.poll().params().as_object().unwrap().len(), 2);
        assert_eq!(req.cancel().method(), "operations.cancel");
        let mut methods = vec![
            json!(kind.method()),
            json!("operations.get"),
            json!("operations.cancel"),
        ];
        if let Some(results) = req.results_call() {
            methods.push(json!(results.method()));
        }
        assert!(kind.supported(&methods));
        methods.remove(1);
        assert!(!kind.supported(&methods));
    }
    let all = Request::new(
        Kind::ProfileProbe,
        "owner".into(),
        7,
        "operation".into(),
        None,
    )
    .unwrap();
    assert!(all.start().params().get("profileId").is_none());
    assert_eq!(
        request(Kind::ProfileProbe).start().params()["profileId"],
        TARGET
    );
    assert!(
        Request::new(
            Kind::SubscriptionProbe,
            "owner".into(),
            7,
            "operation".into(),
            None
        )
        .is_none()
    );
    for (instance, revision, operation, target) in [
        (
            "bad owner".to_owned(),
            7,
            "op".to_owned(),
            Some(TARGET.to_owned()),
        ),
        (
            "owner".to_owned(),
            u64::MAX,
            "op".to_owned(),
            Some(TARGET.to_owned()),
        ),
        (
            "owner".to_owned(),
            7,
            "x".repeat(65),
            Some(TARGET.to_owned()),
        ),
        (
            "owner".to_owned(),
            7,
            "op".to_owned(),
            Some("https://secret.invalid/password".to_owned()),
        ),
    ] {
        assert!(Request::new(Kind::ProfileProbe, instance, revision, operation, target).is_none());
    }
}

#[test]
fn operation_receipts_are_fenced_bounded_and_never_echo_private_errors() {
    for kind in [
        Kind::RefreshAll,
        Kind::SubscriptionProbe,
        Kind::ProfileProbe,
    ] {
        let req = request(kind);
        for state in ["queued", "running", "succeeded", "failed", "cancelled"] {
            let completed = u64::from(state == "succeeded");
            assert!(matches!(
                req.progress(Ok(receipt(kind, state, completed, 1))),
                Update::Progress(_)
            ));
        }
        for (key, value) in [
            ("instanceId", json!("old-owner")),
            ("operationId", json!("other")),
            ("method", json!("arbitrary.method")),
            ("baseRevision", json!(6)),
            ("state", json!("private message")),
            ("cancellable", json!("yes")),
            ("extra", json!("vless://private/password")),
        ] {
            let mut v = receipt(kind, "running", 0, 1);
            v["result"]["operation"][key] = value;
            assert_eq!(req.progress(Ok(v)), Update::Unknown);
        }
        let mut over = receipt(kind, "running", 0, kind.maximum() as u64 + 1);
        assert_eq!(req.progress(Ok(over.clone())), Update::Unknown);
        over["result"]["operation"]["progress"] = json!({"completed":2,"total":1});
        assert_eq!(req.progress(Ok(over)), Update::Unknown);
        let unknown = req.progress(Err(ReadError::Unavailable));
        assert_eq!(unknown, Update::Unknown);
        let rejected=req.progress(Ok(json!({"ok":false,"revision":7,"error":{"code":"core_rejected","message":"vless://private/password","details":{"key":"secret"}}})));
        assert_eq!(rejected, Update::Rejected("tui.action_rejected"));
        assert!(!format!("{rejected:?}").contains("private"));
        assert!(!format!("{rejected:?}").contains("secret"));
    }
}

#[test]
fn malformed_terminal_receipts_do_not_assert_completion() {
    let req = request(Kind::ProfileProbe);
    for (key, value) in [
        ("outcomeRevision", json!(8)),
        ("cancellable", json!(true)),
        ("error", json!({"code":"core_rejected"})),
        ("progress", json!({"completed":0,"total":1})),
    ] {
        let mut v = receipt(Kind::ProfileProbe, "succeeded", 1, 1);
        v["result"]["operation"][key] = value;
        assert_eq!(req.progress(Ok(v)), Update::Unknown);
    }
    let mut cancelled = receipt(Kind::ProfileProbe, "cancelled", 0, 1);
    cancelled["result"]["operation"]["cancelRequested"] = json!(false);
    assert_eq!(req.progress(Ok(cancelled)), Update::Unknown);
    let mut missing = receipt(Kind::ProfileProbe, "running", 0, 1);
    let op = missing["result"]["operation"].as_object_mut().unwrap();
    op.remove("outcomeRevision");
    op.insert("unrelated".into(), Value::Null);
    assert_eq!(req.progress(Ok(missing)), Update::Unknown);
}

fn rows(target: Value, rows: Value) -> Value {
    json!({"ok":true,"revision":7,"result":{"version":1,"profileId":target,"results":rows}})
}
fn row() -> Value {
    json!({"id":TARGET,"resolved":true,"reachable":true,"latencyMs":12})
}

#[test]
fn selected_results_match_target_and_reject_impossible_rows() {
    let req = request(Kind::ProfileProbe);
    let result = req
        .results(Ok(rows(json!(TARGET), json!([row()]))))
        .unwrap();
    assert_eq!(result[0].latency_ms, Some(12));
    assert!(result[0].resolved);
    for (key, value) in [
        ("id", json!("10000000-0000-4000-8000-000000000002")),
        ("resolved", json!(false)),
        ("latencyMs", json!(60001)),
        ("latencyMs", json!(-1)),
        ("latencyMs", json!(1.5)),
        ("url", json!("https://private.invalid/password")),
    ] {
        let mut invalid = row();
        invalid[key] = value;
        assert!(
            req.results(Ok(rows(json!(TARGET), json!([invalid]))))
                .is_none()
        );
    }
    assert!(req.results(Ok(rows(Value::Null, json!([row()])))).is_none());
    assert!(req.results(Ok(rows(json!(TARGET), json!([])))).is_none());
    let mut old = rows(json!(TARGET), json!([row()]));
    old["revision"] = json!(8);
    assert!(req.results(Ok(old)).is_none());
}

#[test]
fn all_results_bound_rows_distinguish_unresolved_unreachable_and_reject_duplicates() {
    let req = Request::new(
        Kind::ProfileProbe,
        "owner".into(),
        7,
        "operation".into(),
        None,
    )
    .unwrap();
    let missing =
        json!({"ok":true,"revision":7,"result":{"version":1,"unrelated":null,"results":[]}});
    assert!(req.results(Ok(missing)).is_none());
    assert!(
        req.results(Ok(rows(Value::Null, json!([row(), row()]))))
            .is_none()
    );
    assert!(
        req.results(Ok(rows(Value::Null, json!(vec![row(); 257]))))
            .is_none()
    );
    assert!(
        req.results(Ok(rows(Value::Null, json!([]))))
            .unwrap()
            .is_empty()
    );
    let maximum: Vec<_> = (0..256)
        .map(|index| {
            let mut row = row();
            row["id"] = json!(format!("10000000-0000-4000-8000-{index:012x}"));
            row
        })
        .collect();
    assert_eq!(
        req.results(Ok(rows(Value::Null, json!(maximum))))
            .unwrap()
            .len(),
        256
    );
    for resolved in [false, true] {
        let mut r = row();
        r["reachable"] = json!(false);
        r["resolved"] = json!(resolved);
        r["latencyMs"] = json!(-1);
        let result = req.results(Ok(rows(Value::Null, json!([r])))).unwrap();
        assert_eq!(result[0].resolved, resolved);
        assert_eq!(result[0].latency_ms, None);
    }
}

fn progress(state: State, completed: usize) -> Progress {
    Progress {
        state,
        completed,
        total: 2,
        cancel_requested: false,
        cancellable: !state.terminal(),
        error: None,
    }
}
#[test]
fn watch_bounds_poll_rate_and_never_restarts_after_timeout_or_cancel_request() {
    let now = Instant::now();
    let mut tracker = Tracker::new(now);
    assert!(tracker.poll_due(now));
    assert!(!tracker.poll_due(now));
    assert!(tracker.poll_due(now + Duration::from_secs(1)));
    assert!(!tracker.request_cancel());
    assert!(tracker.accept(Update::Progress(progress(State::Running, 0))));
    assert!(tracker.request_cancel());
    assert!(!tracker.request_cancel());
    assert!(!tracker.poll_due(now + WATCH_BUDGET));
    assert!(tracker.unknown);
    assert_eq!(tracker.progress.unwrap().state, State::Running);
}

#[test]
fn watch_rejects_regression_but_unknown_can_recover_from_exact_poll() {
    let mut tracker = Tracker::new(Instant::now());
    assert!(tracker.accept(Update::Progress(progress(State::Running, 1))));
    assert!(!tracker.accept(Update::Unknown));
    assert!(tracker.unknown);
    assert!(!tracker.accept(Update::Progress(progress(State::Running, 0))));
    assert!(!tracker.accept(Update::Progress(progress(State::Queued, 1))));
    assert!(tracker.accept(Update::Progress(progress(State::Succeeded, 2))));
    assert!(!tracker.unknown);
    assert!(!tracker.accept(Update::Progress(progress(State::Running, 2))));
    assert_eq!(tracker.progress.unwrap().state, State::Succeeded);
}
