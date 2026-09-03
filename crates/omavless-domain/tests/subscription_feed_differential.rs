// SPDX-License-Identifier: MIT

use omavless_domain::subscription_feed::{PrivateSubscriptionBody, decode_subscription_feed};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_report(corpus: &Value) -> Value {
    let cases = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let id = case["id"].as_str().unwrap();
            let body = case["body"].as_str().unwrap();
            let decoded = PrivateSubscriptionBody::from_bytes(body.as_bytes().to_vec())
                .and_then(decode_subscription_feed);
            match decoded {
                Ok(feed) => {
                    let counts = feed.counts();
                    json!({"id": id, "ok": true, "accepted": counts.accepted, "skipped": counts.skipped})
                }
                Err(error) => json!({"id": id, "ok": false, "code": error.code()}),
            }
        })
        .collect::<Vec<_>>();
    json!({"version": 1, "cases": cases})
}

fn python_report(corpus: &Value) -> Value {
    let mut child = Command::new("python3")
        .arg(root().join("tools/subscription_feed_parity.py"))
        .current_dir(root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(corpus).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn python_and_rust_feed_decoders_match_safe_corpus() {
    let corpus: Value = serde_json::from_slice(
        &std::fs::read(root().join("tests/parity_cases/subscription-feed-v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(rust_report(&corpus), python_report(&corpus));
}

#[test]
fn reports_never_contain_private_feed_material() {
    let corpus = json!({
        "version": 1,
        "cases": [{
            "id": "privacy",
            "body": "vless://private-secret@private-provider.invalid:443"
        }],
    });
    for report in [rust_report(&corpus), python_report(&corpus)] {
        let public = report.to_string();
        for marker in ["private-secret", "private-provider", "vless://"] {
            assert!(!public.contains(marker));
        }
    }
}
