// SPDX-License-Identifier: MIT

use omavless_domain::import::{MAX_IMPORT_BYTES, preview_import};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[test]
fn unified_preview_matches_python_without_printing_private_projections() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for corpus in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let values: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("tests/parity_cases").join(corpus)).unwrap(),
        )
        .unwrap();
        for value in values {
            let input = value["uri"].as_str().unwrap();
            cases.push(json!({"input": input, "urls": []}));
            // File contents and clipboard text share the same trim/classifier.
            cases.push(json!({"input": format!("\n{input}\n"), "urls": []}));
        }
    }
    for input in [
        "",
        " ",
        "vless://opaque.invalid",
        "https://example.invalid/sub",
        "https://user:secret@example.invalid/sub",
        "http://example.invalid/sub",
        "https://example.invalid/sub#secret",
        "https://example.invalid/a https://example.invalid/b",
        "vless://private-invalid https://example.invalid/sub",
    ] {
        cases.push(json!({"input": input, "urls": []}));
    }
    cases.push(json!({"input": " https://example.invalid/sub\n",
                     "urls": ["https://example.invalid/sub"]}));
    let mut child = Command::new("python3")
        .arg(root.join("tools/import_preview_parity.py"))
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
    assert!(output.status.success(), "oracle execution failed");
    let expected: Vec<Option<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let urls: Vec<String> = serde_json::from_value(case["urls"].clone()).unwrap();
        let actual = preview_import(case["input"].as_str().unwrap(), &urls)
            .ok()
            .map(|preview| {
                format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&preview.private_ui_value()).unwrap())
                )
            });
        assert!(
            actual == expected,
            "preview parity mismatch at case {index}"
        );
    }
}

#[test]
fn preview_bounds_debug_and_errors_are_safe() {
    assert!(preview_import(&"x".repeat(MAX_IMPORT_BYTES + 1), &[]).is_err());
    let input = "trojan://synthetic-password@203.0.113.1:443#SyntheticLabel";
    let preview = preview_import(input, &[]).unwrap();
    let debug = format!("{preview:?}");
    for marker in ["synthetic-password", "203.0.113.1", "SyntheticLabel"] {
        assert!(!debug.contains(marker));
    }
    let value = preview.private_ui_value();
    assert_eq!(value["kind"], "profile");
    assert_eq!(value["profile"]["protocol"], "trojan");
    assert!(!value.to_string().contains("synthetic-password"));
    for input in [
        "trojan://synthetic-password@bad",
        "https://user:synthetic-password@example.invalid/sub",
    ] {
        let error = preview_import(input, &[]).unwrap_err();
        assert!(!format!("{error}: {error:?}").contains("synthetic-password"));
    }
    let subscription = preview_import(
        "https://example.invalid/private-token",
        &["https://example.invalid/private-token".to_owned()],
    )
    .unwrap();
    assert_eq!(
        subscription.private_ui_value(),
        json!({
            "version": 1, "kind": "subscription", "duplicate": true,
            "suggestedName": "Subscription",
        })
    );
    assert!(!format!("{subscription:?}").contains("private-token"));
}
