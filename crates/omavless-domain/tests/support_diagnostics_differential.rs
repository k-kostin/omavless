// SPDX-License-Identifier: MIT
use omavless_domain::private_store::parse_private_store;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn populated() -> Value {
    json!({"version":3,"profiles":[{
        "id":"00000000-0000-4000-8000-000000000001","name":"Private label",
        "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp",
        "favorite":true,"protocol":"vless"}],
        "subscriptions":[{"id":"00000000-0000-4000-8000-000000000002",
            "name":"Private provider","url":"https://example.invalid/private-token","updatedAt":123}],
        "customRules":[{"id":"00000000-0000-4000-8000-000000000003",
            "kind":"domain","action":"proxy","value":"private.example.invalid"}],
        "routingPreset":"custom","rulesUpdatedAt":456,"onboardingComplete":true,
        "startupConfigured":true,
        "startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"}})
}

#[test]
fn support_configuration_matches_actual_python_snapshot() {
    let mut cases = vec![
        json!({"version":3,"profiles":[],"subscriptions":[]}),
        populated(),
    ];
    for preset in [
        "",
        "custom",
        "roscomvpn-default",
        "china-cn-direct",
        "iran-ir-direct",
    ] {
        for mode in ["rule", "global"] {
            for enabled in [false, true] {
                let mut case = populated();
                case["routingPreset"] = json!(preset);
                case["startup"]["mode"] = json!(mode);
                case["startup"]["enabled"] = json!(enabled);
                cases.push(case);
            }
        }
    }
    for version in [1, 2] {
        let mut case = populated();
        case["version"] = json!(version);
        cases.push(case);
    }
    let mut specific = populated();
    specific["startup"]["target"] = json!("profile");
    specific["startup"]["profileId"] = specific["profiles"][0]["id"].clone();
    specific["startup"]["enabled"] = json!(true);
    cases.push(specific);
    let mut extension = populated();
    extension["extension"] = json!({"password":"private-token","controller":"/private/path"});
    cases.push(extension);
    assert_eq!(cases.len(), 26);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("python3")
        .arg(root.join("tools/support_diagnostics_parity.py"))
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
    assert!(output.status.success(), "support oracle failed");
    let expected: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let payload = parse_private_store(&case.to_string())
            .unwrap()
            .support_projection();
        let bytes = serde_json::to_vec(&payload).unwrap();
        assert!(bytes.len() < 1024);
        for forbidden in [
            "Private",
            "private-token",
            "192.0.2.1",
            "example.invalid",
            "11111111",
            "00000000",
            "://",
            "/private/path",
        ] {
            assert!(
                !payload.to_string().contains(forbidden),
                "support privacy case {index}"
            );
        }
        assert!(
            format!("{:x}", Sha256::digest(bytes)) == expected,
            "support parity case {index}"
        );
    }
}

#[test]
fn support_projection_is_constant_sized_at_store_capacity() {
    let mut case = populated();
    let profile = case["profiles"][0].clone();
    case["profiles"] = json!(
        (0..256)
            .map(|i| {
                let mut p = profile.clone();
                p["id"] = json!(format!("00000000-0000-4000-8000-{i:012}"));
                p["name"] = json!(format!("Private label {i}"));
                p
            })
            .collect::<Vec<_>>()
    );
    case["subscriptions"] = json!(
        (0..64)
            .map(|i| json!({
                "id":format!("10000000-0000-4000-8000-{i:012}"),
                "name":format!("Private provider {i}"),
                "url":format!("https://example.invalid/private-token-{i}"), "updatedAt":i
            }))
            .collect::<Vec<_>>()
    );
    case["customRules"] = json!(
        (0..128)
            .map(|i| json!({
                "id":format!("20000000-0000-4000-8000-{i:012}"),
                "kind":"domain","action":"proxy","value":format!("private-{i}.example.invalid")
            }))
            .collect::<Vec<_>>()
    );
    let report = parse_private_store(&case.to_string())
        .unwrap()
        .support_projection();
    assert_eq!(
        report["inventory"],
        json!({"profiles":256,"favorites":256,"subscriptions":64,"customRules":128})
    );
    assert!(report.to_string().len() < 1024);
    assert!(!report.to_string().contains("Private"));
    assert!(!report.to_string().contains("example.invalid"));
}

#[test]
fn support_startup_is_stored_intent_not_implicit_legacy_unit_state() {
    let mut case = populated();
    case["startupConfigured"] = json!(false);
    case["startup"]["enabled"] = json!(true);
    case["startup"]["mode"] = json!("global");
    let report = parse_private_store(&case.to_string())
        .unwrap()
        .support_projection();
    assert_eq!(
        report["startup"],
        json!({"configured":false,"enabled":true,"target":"last","mode":"global"})
    );
}
