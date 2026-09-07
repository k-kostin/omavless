// SPDX-License-Identifier: MIT

use omavless_domain::private_store::apply_profile_import;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ID: &str = "00000000-0000-4000-8000-000000000002";
const INPUT: &str =
    "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Synthetic";

fn store() -> Value {
    json!({"version": 3, "activeId": "", "lastId": "", "profiles": [],
           "subscriptions": [], "routingPreset": "", "customRules": [], "rulesUpdatedAt": 0,
           "startupConfigured": true,
           "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
           "onboardingComplete": true})
}

#[test]
fn new_profile_import_matches_effect_isolated_python_reference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for corpus in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let corpus: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("tests/parity_cases").join(corpus)).unwrap(),
        )
        .unwrap();
        for item in corpus {
            cases.push(
                json!({"store": store(), "id": ID, "name": "Confirmed", "input": item["uri"]}),
            );
        }
    }
    for name in [
        "  Trimmed  ".to_owned(),
        "Импорт".to_owned(),
        "x".repeat(80),
        "x".repeat(81),
        " \n".to_owned(),
        "Clean\u{0001}Name".to_owned(),
    ] {
        cases.push(json!({"store": store(), "id": ID, "name": name, "input": INPUT}));
    }
    let existing = apply_profile_import(&store().to_string(), ID, "Existing", INPUT).unwrap();
    let existing: Value = serde_json::from_slice(existing.payload()).unwrap();
    for name in ["Existing", "Another"] {
        cases.push(
            json!({"store": existing, "id": "00000000-0000-4000-8000-000000000003",
                         "name": name, "input": INPUT}),
        );
    }
    let mut child = Command::new("python3")
        .arg(root.join("tools/profile_import_parity.py"))
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
    assert!(output.status.success(), "oracle failed");
    let expected: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let actual = apply_profile_import(
            &case["store"].to_string(),
            case["id"].as_str().unwrap(),
            case["name"].as_str().unwrap(),
            case["input"].as_str().unwrap(),
        )
        .ok()
        .map(|result| {
            let value: Value = serde_json::from_slice(result.payload()).unwrap();
            format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()))
        });
        assert!(
            actual == expected,
            "profile import parity mismatch at case {index}"
        );
    }
}

#[test]
fn import_never_replaces_selects_or_accepts_subscription_or_ambiguous_input() {
    let first = apply_profile_import(&store().to_string(), ID, "Existing", INPUT).unwrap();
    let value: Value = serde_json::from_slice(first.payload()).unwrap();
    assert_eq!(value["activeId"], "");
    assert_eq!(value["lastId"], "");
    assert_eq!(value["profiles"][0]["favorite"], false);
    assert!(apply_profile_import(&value.to_string(), ID, "Other", INPUT).is_err());
    for input in [
        "https://example.invalid/private-token",
        "vless://invalid private-token",
        "trojan://private-token@bad",
    ] {
        let error = apply_profile_import(&store().to_string(), ID, "Confirmed", input)
            .err()
            .unwrap();
        assert!(!format!("{error} {error:?}").contains("private-token"));
    }
}

#[test]
fn import_enforces_store_capacity_and_owner_generated_id_validity() {
    let mut full = store();
    full["profiles"] = Value::Array(
        (0..omavless_domain::store::MAX_PROFILES)
            .map(|n| {
                json!({
                    "id": format!("10000000-0000-4000-8000-{n:012}"),
                    "name": format!("Synthetic {n}"), "uri": INPUT, "protocol": "vless",
                })
            })
            .collect(),
    );
    assert!(apply_profile_import(&full.to_string(), ID, "Additional", INPUT).is_err());
    assert!(apply_profile_import(&store().to_string(), "invalid-id", "Confirmed", INPUT).is_err());
}
