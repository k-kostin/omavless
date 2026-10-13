// SPDX-License-Identifier: MIT
//! Fixed TUI adapter vocabulary, not a caller-selectable raw IPC bridge.
use omavless_tui::{
    model::ReadError,
    private_import::{Completion, Request},
};
use serde_json::{Value, json};
pub(crate) fn dispatch(
    request: Request,
    mut call: impl FnMut(&str, Value) -> Result<Value, ReadError>,
) -> Completion {
    let hello = call("system.hello", json!({"versions":[1]}));
    if !hello.is_ok_and(|v| {
        v["ok"] == true
            && v["result"]["instanceId"] == request.instance()
            && v["revision"].as_u64() == Some(request.revision())
    }) {
        return request.not_sent();
    }
    let result = if request.mutation() {
        call("plugin.action", request.params())
    } else {
        call("imports.classify", request.params()).and_then(|value| {
            let after = call("system.hello", json!({"versions":[1]}))?;
            if after["ok"] != true
                || after["result"]["instanceId"] != request.instance()
                || after["revision"].as_u64() != Some(request.revision())
            {
                return Err(ReadError::Changed);
            }
            Ok(value)
        })
    };
    request.settle(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_tui::private_import::{State, Workspace};
    use std::time::Instant;
    fn hello() -> Value {
        json!({"ok":true,"revision":7,"result":{"instanceId":"fixture"}})
    }
    fn preview() -> Value {
        json!({"ok":true,"revision":7,"result":{"version":1,"kind":"bundle",
        "counts":{"accepted":2,"duplicates":0,"skipped":0},"previewFingerprint":"a".repeat(64)}})
    }
    fn workspace() -> Workspace {
        let mut w = Workspace::new("fixture", 7, 1).unwrap();
        w.append("synthetic raw body");
        w
    }
    #[test]
    fn classifier_is_bracketed_and_never_uses_a_mutation_transport() {
        let mut w = workspace();
        let r = w.classify(Instant::now()).unwrap();
        let mut calls = Vec::new();
        let result = dispatch(r, |method, params| {
            calls.push(method.to_owned());
            match method {
                "system.hello" => Ok(hello()),
                "imports.classify" => {
                    assert_eq!(params, json!({"input":"synthetic raw body"}));
                    Ok(preview())
                }
                _ => panic!("unexpected call"),
            }
        });
        w.accept(result);
        assert_eq!(calls, ["system.hello", "imports.classify", "system.hello"]);
        assert_eq!(w.state(), State::Preview);
    }
    #[test]
    fn owner_or_revision_change_before_dispatch_never_sends_private_input() {
        for changed in [
            json!({"ok":true,"revision":8,"result":{"instanceId":"fixture"}}),
            json!({"ok":true,"revision":7,"result":{"instanceId":"other"}}),
        ] {
            let mut w = workspace();
            let r = w.classify(Instant::now()).unwrap();
            let mut count = 0;
            let result = dispatch(r, |method, params| {
                count += 1;
                assert_eq!(method, "system.hello");
                assert_eq!(params, json!({"versions":[1]}));
                Ok(changed.clone())
            });
            w.accept(result);
            assert_eq!(count, 1);
            assert_eq!(w.state(), State::Rejected);
        }
    }
    #[test]
    fn changed_post_classification_owner_never_yields_confirmation() {
        let mut w = workspace();
        let r = w.classify(Instant::now()).unwrap();
        let mut calls = 0;
        w.accept(dispatch(r, |method, _| {
            calls += 1;
            Ok(match method {
                "imports.classify" => preview(),
                _ if calls == 3 => json!({"ok":true,"revision":7,"result":{"instanceId":"other"}}),
                _ => hello(),
            })
        }));
        assert_eq!(w.state(), State::Rejected);
    }
    #[test]
    fn save_uses_one_original_fixed_bridge_without_read_retry() {
        let now = Instant::now();
        let mut w = workspace();
        let r = w.classify(now).unwrap();
        w.accept(r.settle(Ok(preview())));
        let r = w.confirm("original-op".into(), now).unwrap();
        let mut calls = Vec::new();
        w.accept(dispatch(r, |method, params| {
            calls.push(method.to_owned());
            if method == "system.hello" {
                return Ok(hello());
            }
            assert_eq!(method, "plugin.action");
            assert_eq!(params["action"], "profile-bundle-import");
            assert_eq!(params["operationId"], "original-op");
            Err(ReadError::Unavailable)
        }));
        assert_eq!(calls, ["system.hello", "plugin.action"]);
        assert_eq!(w.state(), State::Unknown);
    }
}
