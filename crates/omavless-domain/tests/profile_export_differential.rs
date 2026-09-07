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
            let protocol = item
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or("vless");
            cases.push(json!({"caseId":item["id"], "id": ID, "store": {"version":3,"profiles":[{"id":ID,"name":"Synthetic","uri":item["uri"],"protocol":protocol}],"subscriptions":[]}}));
        }
    }
    let mut missing = cases[0].clone();
    missing["id"] = json!("00000000-0000-4000-8000-000000000099");
    missing["caseId"] = json!("missing-record");
    cases.push(missing);
    let mut managed = cases[0].clone();
    managed["caseId"] = json!("managed-record");
    managed["store"]["subscriptions"] = json!([{"id":"10000000-0000-4000-8000-000000000001","name":"Synthetic source","url":"https://example.invalid/synthetic-token","updatedAt":1}]);
    managed["store"]["profiles"][0]["subscriptionId"] =
        json!("10000000-0000-4000-8000-000000000001");
    managed["store"]["profiles"][0]["subscriptionKey"] = json!("a".repeat(64));
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
    assert!(
        expected.iter().filter(|value| value.is_some()).count() > 30,
        "export corpus must exercise successful releases, not only matching rejection"
    );
    assert!(
        expected.last().unwrap().is_some(),
        "managed export must succeed"
    );
    let mut differences = Vec::new();
    for (case, expected) in cases.iter().zip(expected) {
        let actual = parse_private_store(&case["store"].to_string())
            .ok()
            .and_then(|store| store.profile_export(case["id"].as_str().unwrap()).ok())
            .map(|export| {
                format!(
                    "{:x}",
                    Sha256::digest(format!("{}\n", export.private_uri()).as_bytes())
                )
            });
        if actual != expected {
            differences.push((
                case["caseId"].as_str().unwrap(),
                actual.is_some(),
                expected.is_some(),
            ));
        }
    }
    // Existing native store validation is strict; Python's export loader uses
    // strict_xhttp_extra=False. Preserve this fail-closed boundary explicitly,
    // not by skipping comparisons or weakening the complete native store.
    assert_eq!(
        differences,
        [
            ("xhttp-unknown", false, true),
            ("xhttp-stream-one-download", false, true),
            ("download-mode-mismatch", false, true),
            ("recursive-extra", false, true),
        ]
    );
}
