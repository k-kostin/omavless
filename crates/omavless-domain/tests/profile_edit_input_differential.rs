// SPDX-License-Identifier: MIT
use omavless_domain::private_store::{PrivateStoreError, parse_private_store};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ID: &str = "00000000-0000-4000-8000-000000000001";
fn cases() -> Vec<Value> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for corpus in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let values: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("tests/parity_cases").join(corpus)).unwrap(),
        )
        .unwrap();
        for value in values
            .into_iter()
            .filter(|value| value["classification"] == "accepted")
        {
            let protocol = value
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or("vless");
            cases.push(json!({"id":ID,"store":{"version":3,"profiles":[{"id":ID,"name":"Synthetic","protocol":protocol,"uri":value["uri"]}],"subscriptions":[]}}));
        }
    }
    cases
}
#[test]
fn standalone_editor_input_matches_actual_python_editor_seed() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = cases();
    assert!(cases.len() > 30, "positive canonical editor corpus missing");
    for name in [
        "Русское имя".to_owned(),
        "x".repeat(80),
        "<b>Plain text</b>".to_owned(),
    ] {
        let mut case = cases[0].clone();
        case["store"]["profiles"][0]["name"] = json!(name);
        cases.push(case);
    }
    let mut child = Command::new("python3")
        .arg(root.join("tools/profile_edit_input_parity.py"))
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
    assert!(output.status.success(), "editor oracle failed");
    let expected: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let store = parse_private_store(&case["store"].to_string()).unwrap();
        let input = store.profile_edit_input(ID).unwrap();
        let value = json!({"name":input.private_name(),"input":input.private_input()});
        let actual = format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()));
        assert!(actual == expected, "editor parity mismatch at case {index}");
    }
}
#[test]
fn missing_or_managed_records_do_not_enter_standalone_editor() {
    let mut case = cases().remove(0);
    let store = parse_private_store(&case["store"].to_string()).unwrap();
    assert!(matches!(
        store.profile_edit_input("00000000-0000-4000-8000-000000000099"),
        Err(PrivateStoreError::ProfileNotFound)
    ));
    case["store"]["subscriptions"] = json!([{"id":"10000000-0000-4000-8000-000000000001","name":"Source","url":"https://example.invalid/synthetic-token","updatedAt":0}]);
    case["store"]["profiles"][0]["subscriptionId"] = json!("10000000-0000-4000-8000-000000000001");
    case["store"]["profiles"][0]["subscriptionKey"] = json!("a".repeat(64));
    let store = parse_private_store(&case["store"].to_string()).unwrap();
    assert!(matches!(
        store.profile_edit_input(ID),
        Err(PrivateStoreError::SubscribedProfile)
    ));
    assert!(
        store.profile_export(ID).is_ok(),
        "managed QR/file export must remain separate"
    );
}
