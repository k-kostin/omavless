// SPDX-License-Identifier: MIT
mod support;
use omavless_tui::{
    client::{ProfileTarget, Read, load_page_for},
    inspection::{CoreDiagnostics, Page, ProfileDetails, connection_count},
    model::ReadError,
};
use serde_json::{Value, json};

fn capable(r: Read) -> Value {
    let mut v = support::response(r);
    if r == Read::Capabilities {
        v["result"]["methods"] = json!([
            "ui.snapshot",
            "runtime.observation",
            "profiles.details",
            "runtime.connections",
            "runtime.traffic",
            "profiles.probe",
            "profiles.probe_results",
            "operations.get",
            "operations.cancel",
            "subscriptions.refresh"
        ]);
    }
    v
}

#[test]
fn private_target_is_bounded_and_debug_redacted() {
    for value in ["", "bad target", "bad\n", "тест"] {
        assert!(ProfileTarget::new(value).is_none());
    }
    assert!(ProfileTarget::new(&"x".repeat(65)).is_none());
    let target = ProfileTarget::new(&"x".repeat(64)).unwrap();
    assert_eq!(target.as_str(), "x".repeat(64));
    let read = Read::ProfileDetails(ProfileTarget::new("private-target").unwrap());
    assert_eq!(read.method(), "profiles.details");
    assert_eq!(read.params(), json!({"profileId":"private-target"}));
    assert!(!format!("{read:?}").contains("private-target"));
}

#[test]
fn profile_read_targets_exact_visible_record_and_never_falls_back() {
    for target in [None, Some("removed"), Some("fixture-b")] {
        let mut calls = Vec::new();
        let snapshot = load_page_for(
            &mut |r| {
                calls.push(r);
                Ok(capable(r))
            },
            Page::Details,
            target,
        )
        .unwrap();
        let expected = target == Some("fixture-b");
        assert_eq!(calls.len(), if expected { 5 } else { 4 });
        assert_eq!(snapshot.profile_details.is_some(), expected);
        if expected {
            let details = snapshot.profile_details.unwrap();
            assert_eq!(details.profile_id(), "fixture-b");
            assert_eq!(details.protocol, Some("vless"));
            assert_eq!(details.transport, Some("xhttp"));
            assert_eq!(details.security, Some("reality"));
        }
    }
}

#[test]
fn details_capabilities_and_revision_are_fenced() {
    let snapshot = load_page_for(
        &mut |r| Ok(support::response(r)),
        Page::Details,
        Some("fixture-b"),
    )
    .unwrap();
    assert!(snapshot.profile_details.is_none());
    for restart in [false, true] {
        let result = load_page_for(
            &mut |r| {
                let mut v = capable(r);
                if matches!(r, Read::ProfileDetails(_)) && !restart {
                    v["revision"] = json!(8);
                }
                if r == Read::Observation && restart {
                    v["result"]["instanceId"] = json!("new-runtime");
                }
                Ok(v)
            },
            Page::Details,
            Some("fixture-b"),
        );
        assert!(matches!(result, Err(ReadError::Changed)));
    }
}

#[test]
fn details_discard_all_unrecognized_or_private_tokens() {
    let target = ProfileTarget::new("fixture-b").unwrap();
    let mut v = support::response(Read::ProfileDetails(target));
    for key in ["protocol", "transport", "security"] {
        v["result"][key] = json!("secret://private.invalid/password\u{1b}");
    }
    let details = ProfileDetails::parse(&v, target).unwrap();
    assert!(
        details.protocol.is_none() && details.transport.is_none() && details.security.is_none()
    );
    v["result"]["version"] = json!(2);
    assert!(ProfileDetails::parse(&v, target).is_none());
    v["ok"] = json!(false);
    v["error"] = json!({"message":"private-password"});
    assert!(ProfileDetails::parse(&v, target).is_none());
}

#[test]
fn count_is_traffic_only_and_fails_closed_on_missing_capability_or_failed_read() {
    for page in [Page::Profiles, Page::Details, Page::Traffic] {
        let mut calls = Vec::new();
        let snapshot = load_page_for(
            &mut |r| {
                calls.push(r);
                Ok(capable(r))
            },
            page,
            None,
        )
        .unwrap();
        assert_eq!(
            snapshot.active_connections,
            if page == Page::Traffic { Some(3) } else { None }
        );
        assert_eq!(calls.contains(&Read::Connections), page == Page::Traffic);
        assert!(snapshot.capabilities.profile_probe && snapshot.capabilities.connection_count);
    }
    for failed in [false, true] {
        let snapshot = load_page_for(
            &mut |r| {
                if r == Read::Connections {
                    return Err(ReadError::Unavailable);
                }
                Ok(if failed {
                    capable(r)
                } else {
                    support::response(r)
                })
            },
            Page::Traffic,
            None,
        )
        .unwrap();
        assert!(snapshot.active_connections.is_none());
    }
}

#[test]
fn count_bounds_and_owner_revision_never_render_false_zero() {
    let v = support::response(Read::Connections);
    for value in [json!(-1), json!(4097), json!(1.5), json!(null)] {
        let mut bad = v.clone();
        bad["result"]["count"] = value;
        assert_eq!(connection_count(&bad), None);
    }
    for (path, value) in [
        ("/revision", json!(8)),
        ("/result/instanceId", json!("restarted")),
    ] {
        assert!(matches!(
            load_page_for(
                &mut |r| {
                    let mut v = capable(r);
                    if r == Read::Connections {
                        *v.pointer_mut(path).unwrap() = value.clone();
                    }
                    Ok(v)
                },
                Page::Traffic,
                None
            ),
            Err(ReadError::Changed)
        ));
    }
}

#[test]
fn core_log_counts_are_optional_bounded_hints_not_raw_error_text() {
    let mut v = json!({"scope":"latest_owned_core_log_counts","dnsErrors":1,"tlsErrors":2,"timeoutErrors":3,"connectionErrors":4,"otherWarnings":5,"oversizedLines":6,"readFailed":false,"finished":false,"incomplete":false,"raw":"private endpoint"});
    let counts = CoreDiagnostics::parse(&v).unwrap();
    assert_eq!(
        (
            counts.dns,
            counts.tls,
            counts.timeout,
            counts.connection,
            counts.other,
            counts.oversized
        ),
        (1, 2, 3, 4, 5, 6)
    );
    v["readFailed"] = json!(true);
    assert!(CoreDiagnostics::parse(&v).unwrap().incomplete);
    v["dnsErrors"] = json!(u64::MAX);
    assert!(CoreDiagnostics::parse(&v).is_none());
    assert!(CoreDiagnostics::parse(&Value::Null).is_none());
}

#[test]
fn job_capabilities_require_poll_cancel_and_results_not_just_start_method() {
    use omavless_tui::inspection::Capabilities;
    let methods = [
        "profiles.probe",
        "profiles.probe_results",
        "subscriptions.probe",
        "subscriptions.probe_results",
        "subscriptions.refresh_all",
        "operations.get",
        "operations.cancel",
    ];
    let full: Vec<Value> = methods.iter().map(|s| json!(s)).collect();
    let caps = Capabilities::parse(&full);
    assert!(caps.profile_probe && caps.subscription_probe && caps.refresh_all);
    for omitted in ["operations.get", "operations.cancel"] {
        let partial: Vec<Value> = methods
            .iter()
            .filter(|s| **s != omitted)
            .map(|s| json!(s))
            .collect();
        let caps = Capabilities::parse(&partial);
        assert!(!caps.profile_probe && !caps.subscription_probe && !caps.refresh_all);
    }
    let partial: Vec<Value> = methods
        .iter()
        .filter(|s| !s.ends_with("probe_results"))
        .map(|s| json!(s))
        .collect();
    let caps = Capabilities::parse(&partial);
    assert!(!caps.profile_probe && !caps.subscription_probe && caps.refresh_all);
}
