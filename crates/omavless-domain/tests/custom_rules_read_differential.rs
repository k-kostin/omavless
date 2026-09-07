// SPDX-License-Identifier: MIT
use omavless_domain::private_store::parse_private_store;
use omavless_domain::routing::MAX_CUSTOM_RULES;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn store(rules: Vec<Value>) -> Value {
    json!({"version":3,"profiles":[],"subscriptions":[],"customRules":rules})
}
fn rule(index: usize, kind: &str, action: &str, value: &str) -> Value {
    json!({"id":format!("00000000-0000-4000-8000-{index:012}"),"kind":kind,"action":action,"value":value})
}

#[test]
fn custom_rule_editor_matches_actual_python_allowlisted_projection() {
    let mut cases = vec![store(vec![])];
    for (kind, value) in [
        ("domain", "example.invalid"),
        ("suffix", "example.invalid"),
        ("ipcidr", "192.0.2.0/24"),
        ("ipcidr", "2001:db8::/32"),
    ] {
        for action in ["proxy", "direct", "reject"] {
            cases.push(store(vec![rule(1, kind, action, value)]));
        }
    }
    let rules = (0..MAX_CUSTOM_RULES)
        .map(|i| rule(i, "domain", "direct", &format!("r{i}.example.invalid")))
        .collect::<Vec<_>>();
    cases.push(store(rules));
    let mut extended = store(vec![rule(1, "domain", "proxy", "example.invalid")]);
    extended["customRules"][0]["privateExtra"] = json!("private-token");
    cases.push(extended);
    for version in [1, 2] {
        let mut legacy = cases[1].clone();
        legacy["version"] = json!(version);
        cases.push(legacy);
    }
    assert_eq!(cases.len(), 17);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("python3")
        .arg(root.join("tools/custom_rules_read_parity.py"))
        .current_dir(&root)
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
    assert!(output.status.success(), "custom-rule oracle failed");
    let expected: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let parsed = parse_private_store(&case.to_string()).unwrap();
        let payload = parsed.custom_rules_for_editor().private_ui_value();
        assert_eq!(payload["version"], 1);
        assert_eq!(
            payload["rules"].as_array().unwrap().len(),
            case["customRules"].as_array().unwrap().len()
        );
        assert!(!payload.to_string().contains("private-token"));
        let actual = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&payload).unwrap())
        );
        assert!(
            actual == expected,
            "custom-rule parity mismatch at case {index}"
        );
    }
}
