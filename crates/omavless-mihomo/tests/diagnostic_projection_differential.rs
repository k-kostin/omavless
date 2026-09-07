// SPDX-License-Identifier: MIT
use omavless_mihomo::diagnostics::{providers_projection, rules_projection};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn actual_python_diagnostic_projections_match_bounded_native_rows() {
    let mut cases = vec![
        json!({"kind":"rules","payload":{"rules":[]}}),
        json!({"kind":"providers","payload":{"providers":{}}}),
    ];
    for target in [
        "DIRECT",
        "REJECT",
        "REJECT-DROP",
        "not-REJECT",
        "private-group",
    ] {
        cases.push(json!({"kind":"rules","payload":{"rules":[{"type":"DOMAIN","payload":"safe.example","proxy":target}]}}));
    }
    for text in [
        "private-label.example",
        "vless://private-input",
        "11111111-1111-4111-8111-111111111111",
        "x\n\t y",
        "界",
        "",
    ] {
        cases.push(json!({"kind":"rules","private":["private-label"],"payload":{"rules":[{"type":"DOMAIN","payload":text,"proxy":"opaque"}]}}));
    }
    for count in [Value::Null, json!(0), json!(1), json!(1000000000)] {
        cases.push(json!({"kind":"providers","payload":{"providers":{"safe":{"vehicleType":"HTTP","ruleCount":count,"updatedAt":"2026-09-07","behavior":"domain"}}}}));
    }
    for bad in [json!(-1), json!(true), json!(1.5), json!(1000000001)] {
        cases.push(json!({"kind":"providers","payload":{"providers":{"safe":{"ruleCount":bad}}}}));
    }
    for payload in [
        json!({"rules":false}),
        json!({"rules":[false]}),
        json!({"rules":[{"type":3}]}),
        json!({"rules":[{"payload":null}]}),
    ] {
        cases.push(json!({"kind":"rules","payload":payload}));
    }
    for payload in [
        json!({"providers":[]}),
        json!({"providers":{"../unsafe":{}}}),
        json!({"providers":{"safe":{"updatedAt":true}}}),
        json!({"providers":{"safe":{"behavior":3}}}),
    ] {
        cases.push(json!({"kind":"providers","payload":payload}));
    }
    cases.push(json!({"kind":"rules","payload":{"rules":[{"type":"界".repeat(100),"payload":"x".repeat(1024)}]}}));
    let mut child = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/diagnostic_projection_parity.py"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "diagnostic oracle failed");
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let private: Vec<String> = case
            .get("private")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        let result = if case["kind"] == "rules" {
            rules_projection(&case["payload"], &private)
        } else {
            providers_projection(&case["payload"], &private)
        };
        let actual = match result {
            Ok(result) => {
                json!({"ok":true,"digest":format!("{:x}",Sha256::digest(serde_json::to_vec(&result).unwrap()))})
            }
            Err(_) => json!({"ok":false}),
        };
        assert!(
            actual == expected,
            "diagnostic parity mismatch at public case {index}"
        );
    }
}

#[test]
fn diagnostic_rows_fit_native_envelope_without_silent_truncation() {
    let rules = rules_projection(&json!({"rules":vec![json!({"type":"DOMAIN","payload":"x".repeat(512),"proxy":"private"});2048]}), &[]).unwrap();
    assert_eq!(rules["total"], 2048);
    assert_eq!(rules["truncated"], true);
    assert!(rules.to_string().len() < 160 * 1024);
    let providers: serde_json::Map<String, Value> = (0..256)
        .map(|i| {
            (
                format!("provider-{i}"),
                json!({"behavior":"x".repeat(80),"updatedAt":"y".repeat(80)}),
            )
        })
        .collect();
    let providers = providers_projection(&json!({"providers":providers}), &[]).unwrap();
    let output = json!({"version":1,"rules":rules,"providers":providers});
    assert!(output.to_string().len() < 225 * 1024);
}
