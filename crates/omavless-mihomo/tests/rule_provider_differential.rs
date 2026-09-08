// SPDX-License-Identifier: MIT
use omavless_mihomo::rule_provider::refresh_targets;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn actual_python_discovery_and_fixed_update_paths_match() {
    let mut cases = vec![
        json!(null),
        json!([]),
        json!({}),
        json!({"providers":null}),
        json!({"providers":{}}),
    ];
    for vehicle in [
        json!("HTTP"),
        json!("hTtP"),
        json!("file"),
        json!("inline"),
        json!("future"),
        json!(null),
        json!(false),
        json!(7),
    ] {
        cases.push(json!({"providers":{"sample":{"vehicleType":vehicle}}}));
    }
    for name in [
        "plain-._~",
        "space 界:@&",
        "",
        ".",
        "..",
        "slash/",
        "back\\",
        "query?",
        "fragment#",
        "percent%",
        "control\n",
        "del\u{7f}",
    ] {
        cases.push(json!({"providers":{name:{"vehicleType":"http"}}}));
    }
    for name in [
        "x".repeat(256),
        "x".repeat(257),
        "界".repeat(85),
        "界".repeat(86),
    ] {
        cases.push(json!({"providers":{name:{"vehicleType":"http"}}}));
    }
    for count in [256, 257] {
        let rows: serde_json::Map<String, Value> = (0..count)
            .map(|n| (format!("sample-{n}"), json!({"vehicleType":"http"})))
            .collect();
        cases.push(json!({"providers":rows}));
    }
    cases.push(json!({"providers":{"valid":{"vehicleType":"http"},"invalid/":{}}}));
    cases.push(json!({"providers":{"valid":false}}));
    cases.push(json!({"providers":{"valid":{}}}));
    let mut child = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rule_provider_parity.py"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "synthetic provider oracle failed");
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let actual = match refresh_targets(case) {
            Ok(targets) => {
                let mut paths: Vec<_> = targets.iter().map(|target| target.update_path()).collect();
                paths.sort();
                json!({"ok":true,"digest":format!("{:x}",Sha256::digest(serde_json::to_vec(&paths).unwrap()))})
            }
            Err(_) => json!({"ok":false}),
        };
        assert!(
            actual == expected,
            "provider parity mismatch at public case {index}"
        );
    }
}
