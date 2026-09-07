// SPDX-License-Identifier: MIT
use omavless_domain::private_store::{CustomRuleMutation, apply_custom_rule_mutation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const NEW: &str = "00000000-0000-4000-8000-000000000002";
fn base() -> Value {
    json!({"version":3,"profiles":[],"subscriptions":[],"customRules":[]})
}
fn add(kind: &str, action: &str, value: &str) -> Value {
    json!({"store":base(),"method":"add","kind":kind,"action":action,"value":value,"generatedId":NEW})
}
#[test]
fn custom_rule_add_delete_matches_actual_python() {
    let mut cases = Vec::new();
    for (kind, value) in [
        ("domain", " Example.Invalid. "),
        ("suffix", " *.Example.Invalid "),
        ("suffix", ".example.invalid"),
        ("ipcidr", "192.0.2.123/24"),
        ("ipcidr", "2001:DB8::9/32"),
        ("ipcidr", "192.0.2.1"),
    ] {
        for action in ["proxy", "direct", "reject"] {
            cases.push(add(kind, action, value));
        }
    }
    for (kind, action, value) in [
        ("unknown", "proxy", "example.invalid"),
        ("domain", "unknown", "example.invalid"),
        ("domain", "proxy", "private-token/password"),
        ("domain", "proxy", ""),
        ("ipcidr", "proxy", "192.0.2.1/99"),
        ("domain", "proxy", "a\nb.example.invalid"),
    ] {
        cases.push(add(kind, action, value));
    }
    let rule = json!({"id":ID,"kind":"domain","action":"direct","value":"example.invalid","extension":"private-token"});
    let mut duplicate = add("domain", "proxy", "EXAMPLE.INVALID");
    duplicate["store"]["customRules"] = json!([rule.clone()]);
    cases.push(duplicate);
    for id in [ID, NEW] {
        let mut case = json!({"store":base(),"method":"delete","ruleId":id,"generatedId":NEW});
        case["store"]["customRules"] = json!([rule.clone()]);
        cases.push(case);
    }
    for version in [1, 2] {
        let mut case = add("suffix", "reject", "example.invalid");
        case["store"]["version"] = json!(version);
        cases.push(case);
    }
    for count in [127, 128] {
        let mut case = add("domain", "direct", "added.example.invalid");
        case["store"]["customRules"]=json!((0..count).map(|i| json!({"id":format!("10000000-0000-4000-8000-{i:012}"),"kind":"domain","action":"proxy","value":format!("r{i}.example.invalid")})).collect::<Vec<_>>());
        cases.push(case);
    }
    let mut extended = add("suffix", "proxy", "example.invalid");
    extended["store"]["customRules"] = json!([rule]);
    extended["store"]["extension"] = json!({"private":"private-token"});
    cases.push(extended);
    assert_eq!(cases.len(), 32);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("python3")
        .arg(root.join("tools/custom_rule_mutation_parity.py"))
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
    let mut passed = 0;
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let mutation = if case["method"] == "add" {
            CustomRuleMutation::Add {
                kind: case["kind"].as_str().unwrap().into(),
                action: case["action"].as_str().unwrap().into(),
                value: case["value"].as_str().unwrap().into(),
            }
        } else {
            CustomRuleMutation::Delete {
                rule_id: case["ruleId"].as_str().unwrap().into(),
            }
        };
        let result = apply_custom_rule_mutation(&case["store"].to_string(), mutation, NEW);
        let actual = match result {
            Ok(value) => {
                passed += 1;
                let payload: Value = serde_json::from_slice(value.payload()).unwrap();
                format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&payload).unwrap())
                )
            }
            Err(_) => "rejected".into(),
        };
        assert!(
            actual == expected,
            "custom-rule parity mismatch case {index}"
        );
    }
    assert!(
        passed >= 20,
        "corpus failed to exercise successful mutations"
    );
}
