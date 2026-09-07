// SPDX-License-Identifier: MIT

use omavless_domain::private_store::{ProfileMutation, apply_profile_mutation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const INPUT: &str =
    "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Synthetic";
fn store() -> Value {
    json!({"version": 3, "activeId": ID, "lastId": ID,
        "profiles": [{"id": ID, "name": "Original", "uri": INPUT, "protocol": "vless", "favorite": true}],
        "subscriptions": [], "routingPreset": "", "customRules": [], "rulesUpdatedAt": 0,
        "startupConfigured": true, "startup": {"enabled": true, "target": "profile", "profileId": ID, "mode": "global"},
        "onboardingComplete": true})
}
fn replace(store: &Value, id: &str, name: &str, input: &str) -> Option<Value> {
    apply_profile_mutation(
        &store.to_string(),
        ProfileMutation::Replace {
            profile_id: id.to_owned(),
            new_name: name.to_owned(),
            new_input: input.to_owned(),
        },
    )
    .ok()
    .map(|result| serde_json::from_slice(result.payload()).unwrap())
}

#[test]
fn replacement_matches_python_and_preserves_identity_favorite_and_startup() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for corpus in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let corpus: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("tests/parity_cases").join(corpus)).unwrap(),
        )
        .unwrap();
        for item in corpus {
            cases.push(json!({"store": store(), "id": ID, "oldId": ID,
                             "name": "Replacement", "input": item["uri"]}));
        }
    }
    for name in [
        "Original".to_owned(),
        "  Renamed  ".to_owned(),
        "Замена".to_owned(),
        "x".repeat(80),
        "x".repeat(81),
    ] {
        cases.push(json!({"store": store(), "id": ID, "oldId": ID, "name": name, "input": INPUT}));
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
    assert!(output.status.success(), "replacement oracle failed");
    let expected: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let actual = replace(
            &case["store"],
            ID,
            case["name"].as_str().unwrap(),
            case["input"].as_str().unwrap(),
        );
        if let Some(value) = &actual {
            assert_eq!(value["profiles"][0]["id"], ID);
            assert_eq!(value["profiles"][0]["favorite"], true);
            assert_eq!(value["activeId"], ID);
            assert_eq!(value["lastId"], ID);
            assert!(value["startup"] == store()["startup"]);
        }
        let digest = actual
            .map(|value| format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap())));
        assert!(
            digest == expected,
            "replacement parity mismatch at case {index}"
        );
    }
}

#[test]
fn replacement_never_imports_new_or_edits_managed_profiles_and_detects_noop() {
    assert!(
        replace(
            &store(),
            "00000000-0000-4000-8000-000000000099",
            "Name",
            INPUT
        )
        .is_none()
    );
    let original = store();
    let result = apply_profile_mutation(
        &original.to_string(),
        ProfileMutation::Replace {
            profile_id: ID.to_owned(),
            new_name: "Original".to_owned(),
            new_input: INPUT.to_owned(),
        },
    )
    .unwrap();
    assert!(!result.changed);
    let mut managed = store();
    let sub = "10000000-0000-4000-8000-000000000001";
    managed["subscriptions"] = json!([{"id": sub, "name": "Source", "url": "https://example.invalid/token", "updatedAt": 1}]);
    managed["profiles"][0]["subscriptionId"] = json!(sub);
    managed["profiles"][0]["subscriptionKey"] = json!("a".repeat(64));
    managed["profiles"][0]["missing"] = json!(false);
    assert!(replace(&managed, ID, "Name", INPUT).is_none());
    let mut duplicate = store();
    let mut other = duplicate["profiles"][0].clone();
    other["id"] = json!("00000000-0000-4000-8000-000000000099");
    other["name"] = json!("Other");
    duplicate["profiles"].as_array_mut().unwrap().push(other);
    assert!(replace(&duplicate, ID, "Other", INPUT).is_none());
}
