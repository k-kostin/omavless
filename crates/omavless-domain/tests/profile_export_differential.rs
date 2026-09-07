// SPDX-License-Identifier: MIT

use omavless_domain::private_store::parse_private_store;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ID: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn explicit_export_preserves_python_file_content_without_writing_files() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for corpus in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let corpus: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("tests/parity_cases").join(corpus)).unwrap(),
        )
        .unwrap();
        for item in corpus {
            cases.push(json!({"id": ID, "store": {"version":3,"profiles":[{"id":ID,"name":"Synthetic","uri":item["uri"]}],"subscriptions":[]}}));
        }
    }
    let mut missing = cases[0].clone();
    missing["id"] = json!("00000000-0000-4000-8000-000000000099");
    cases.push(missing);
    let mut managed = cases[0].clone();
    managed["store"]["subscriptions"] = json!([{"id":"10000000-0000-4000-8000-000000000001","name":"Synthetic source","url":"https://example.invalid/synthetic-token","updatedAt":1}]);
    managed["store"]["profiles"][0]["subscriptionId"] =
        json!("10000000-0000-4000-8000-000000000001");
    cases.push(managed);
    let mut child = Command::new("python3")
        .arg(root.join("tools/profile_export_parity.py"))
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
    assert!(output.status.success(), "export oracle failed");
    let expected: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let actual = parse_private_store(&case["store"].to_string())
            .ok()
            .and_then(|store| store.profile_export(case["id"].as_str().unwrap()).ok())
            .map(|export| {
                format!(
                    "{:x}",
                    Sha256::digest(format!("{}\n", export.private_uri()).as_bytes())
                )
            });
        assert!(actual == expected, "export parity mismatch at case {index}");
    }
}
