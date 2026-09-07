// SPDX-License-Identifier: MIT
use omavless_domain::private_store::{
    StartupPreferences, apply_startup_preferences, parse_private_store,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ID: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn preferences_match_python_without_changing_current_connection() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let store = json!({"version":3,"profiles":[{"id":ID,"name":"Synthetic","protocol":"trojan","uri":"trojan://synthetic-token@example.invalid:443"}],"subscriptions":[],"lastId":ID,"activeId":ID,"routingPreset":"custom","startupConfigured":true});
    let mut cases = Vec::new();
    for enabled in [false, true] {
        for target in ["last", "profile"] {
            for mode in ["rule", "global"] {
                cases.push(json!({"store":store,"enabled":enabled,"target":target,"profileId":if target=="profile" {ID} else {""},"mode":mode}));
            }
        }
    }
    let mut child = Command::new("python3")
        .arg(root.join("tools/startup_preferences_parity.py"))
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
    assert!(output.status.success(), "startup preference oracle failed");
    let expected: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let preferences = StartupPreferences {
            enabled: case["enabled"].as_bool().unwrap(),
            target: case["target"].as_str().unwrap().into(),
            profile_id: case["profileId"].as_str().unwrap().into(),
            mode: case["mode"].as_str().unwrap().into(),
        };
        let (payload, _) =
            apply_startup_preferences(&case["store"].to_string(), &preferences).unwrap();
        let parsed: Value = serde_json::from_slice(&payload).unwrap();
        assert!(parsed["activeId"] == ID && parsed["lastId"] == ID);
        let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&parsed).unwrap()));
        assert!(digest == expected, "startup parity mismatch {index}");
        assert!(
            !apply_startup_preferences(std::str::from_utf8(&payload).unwrap(), &preferences)
                .unwrap()
                .1
        );
        let validated = parse_private_store(std::str::from_utf8(&payload).unwrap()).unwrap();
        assert!(validated.startup_is_configured());
        assert_eq!(validated.startup_preferences().enabled, preferences.enabled);
    }
}
