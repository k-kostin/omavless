// SPDX-License-Identifier: MIT
use omavless_domain::private_store::complete_onboarding;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[test]
fn onboarding_matches_actual_python_command_and_preserves_private_store() {
    let mut cases = Vec::new();
    for version in [1, 2, 3] {
        for flag in [None, Some(false), Some(true)] {
            for populated in [false, true] {
                let mut source = json!({"version":version, "profiles":[], "subscriptions":[],
                    "extension":{"nested":[1,true,"synthetic"]}});
                if populated {
                    source["profiles"] = json!([{"id":"00000000-0000-4000-8000-000000000001",
                        "name":"Synthetic", "protocol":"vless",
                        "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp"}]);
                    source["activeId"] = json!("00000000-0000-4000-8000-000000000001");
                    source["lastId"] = source["activeId"].clone();
                }
                if let Some(flag) = flag {
                    source["onboardingComplete"] = json!(flag);
                }
                cases.push(source);
            }
        }
    }
    cases.extend([
        json!({"version":3,"profiles":[],"subscriptions":[],"onboardingComplete":"private-marker"}),
        json!({"version":99,"profiles":[],"subscriptions":[]}),
        json!([]),
    ]);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("python3")
        .arg(root.join("tools/onboarding_completion_parity.py"))
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
    assert!(output.status.success(), "onboarding reference failed");
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let result = complete_onboarding(&case.to_string());
        assert!(
            result.is_ok() == (expected["accepted"] == true),
            "acceptance mismatch {index}"
        );
        if let Ok((payload, _changed)) = result {
            let parsed: Value = serde_json::from_slice(&payload).unwrap();
            let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&parsed).unwrap()));
            assert!(
                Some(digest.as_str()) == expected["digest"].as_str(),
                "store mismatch {index}"
            );
            assert_eq!(parsed["onboardingComplete"], true);
            assert!(parsed["extension"] == case["extension"]);
            assert!(
                !complete_onboarding(std::str::from_utf8(&payload).unwrap())
                    .unwrap()
                    .1
            );
        }
    }
}
