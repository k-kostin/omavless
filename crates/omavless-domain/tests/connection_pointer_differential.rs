// SPDX-License-Identifier: MIT

use omavless_domain::private_store::{
    CompatibilityPointerTarget, PrivateStoreError, apply_compatibility_pointer_update,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
const MISSING: &str = "00000000-0000-4000-8000-000000000002";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn store(legacy: bool) -> Value {
    let mut value = json!({
        "version": 3,
        "activeId": PROFILE,
        "lastId": MISSING,
        "profiles": [{
            "id": PROFILE,
            "name": "Local",
            "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Local",
            "protocol": "vless",
            "favorite": false
        }, {
            "id": MISSING,
            "name": "Retained",
            "uri": "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Retained",
            "protocol": "vless",
            "subscriptionId": "10000000-0000-4000-8000-000000000001",
            "subscriptionKey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "missing": true,
            "favorite": false
        }],
        "subscriptions": [{
            "id": "10000000-0000-4000-8000-000000000001",
            "name": "Source",
            "url": "https://provider.invalid/token",
            "updatedAt": 1
        }],
        "routingPreset": "custom",
        "customRules": [],
        "rulesUpdatedAt": 0,
        "startupConfigured": true,
        "startup": {"enabled": true, "target": "profile", "profileId": MISSING, "mode": "rule"},
        "onboardingComplete": true
    });
    if legacy {
        value["version"] = Value::from(1);
        for profile in value["profiles"].as_array_mut().unwrap() {
            profile.as_object_mut().unwrap().remove("protocol");
            profile.as_object_mut().unwrap().remove("favorite");
        }
        for key in [
            "routingPreset",
            "customRules",
            "rulesUpdatedAt",
            "startup",
            "startupConfigured",
            "onboardingComplete",
        ] {
            value.as_object_mut().unwrap().remove(key);
        }
    }
    value
}

fn rust_result(store: &Value, target: &Value) -> Value {
    let target = match target["kind"].as_str().unwrap() {
        "connected" => CompatibilityPointerTarget::Connected {
            profile_id: target["profileId"].as_str().unwrap().to_owned(),
        },
        "disconnected" => CompatibilityPointerTarget::Disconnected {
            prune_missing: target["pruneMissing"].as_bool().unwrap(),
        },
        _ => unreachable!(),
    };
    match apply_compatibility_pointer_update(&serde_json::to_string(store).unwrap(), target) {
        Ok(result) => {
            let normalized: Value = serde_json::from_slice(result.payload()).unwrap();
            let canonical = serde_json::to_vec(&normalized).unwrap();
            json!({
                "ok": true,
                "changed": result.changed,
                "pruned": result.pruned,
                "sha256": format!("{:x}", Sha256::digest(canonical)),
            })
        }
        Err(PrivateStoreError::ProfileNotFound) => {
            json!({"ok": false, "code": "profile_not_found"})
        }
        Err(_) => json!({"ok": false, "code": "invalid_store"}),
    }
}

fn python_result(store: &Value, target: &Value) -> Value {
    let mut child = Command::new("python3")
        .arg(root().join("tools/connection_pointer_parity.py"))
        .current_dir(root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"store": store, "target": target})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn python_and_rust_connection_pointer_updates_match() {
    let mut stale = store(false);
    stale["activeId"] = Value::from("00000000-0000-4000-8000-000000000098");
    stale["lastId"] = Value::from("00000000-0000-4000-8000-000000000099");
    let cases = [
        (
            store(false),
            json!({"kind": "connected", "profileId": PROFILE}),
        ),
        (
            store(false),
            json!({"kind": "connected", "profileId": MISSING}),
        ),
        (
            store(false),
            json!({"kind": "disconnected", "pruneMissing": false}),
        ),
        (
            store(false),
            json!({"kind": "disconnected", "pruneMissing": true}),
        ),
        (
            store(true),
            json!({"kind": "connected", "profileId": PROFILE}),
        ),
        (
            store(true),
            json!({"kind": "disconnected", "pruneMissing": true}),
        ),
        (
            stale,
            json!({"kind": "disconnected", "pruneMissing": false}),
        ),
        (
            store(false),
            json!({"kind": "connected", "profileId": "00000000-0000-4000-8000-000000000099"}),
        ),
    ];
    for (store, target) in cases {
        assert_eq!(rust_result(&store, &target), python_result(&store, &target));
    }
}

#[test]
fn parity_output_never_contains_private_store_values() {
    let result = python_result(
        &store(false),
        &json!({"kind": "disconnected", "pruneMissing": true}),
    );
    let public = result.to_string();
    for private in [
        "vless://",
        "provider.invalid",
        "11111111",
        "token",
        "Source",
    ] {
        assert!(!public.contains(private));
    }
}
