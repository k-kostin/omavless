// SPDX-License-Identifier: MIT

use omavless_domain::private_store::{PrivateStoreError, ProfileMutation, apply_profile_mutation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const PROFILE_ONE: &str = "00000000-0000-4000-8000-000000000001";
const PROFILE_TWO: &str = "00000000-0000-4000-8000-000000000002";
const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn base_store(subscribed: bool, legacy: bool) -> Value {
    let subscription_id = if subscribed { SUBSCRIPTION } else { "" };
    let subscription_key = if subscribed {
        "a".repeat(64)
    } else {
        String::new()
    };
    let mut profile = json!({
        "id": PROFILE_ONE,
        "name": "First",
        "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#First",
        "protocol": "vless",
        "favorite": false,
    });
    if subscribed {
        profile["subscriptionId"] = Value::from(subscription_id);
        profile["subscriptionKey"] = Value::from(subscription_key);
        profile["missing"] = Value::from(false);
    }
    let mut store = json!({
        "version": 3,
        "activeId": PROFILE_ONE,
        "lastId": PROFILE_ONE,
        "profiles": [profile, {
            "id": PROFILE_TWO,
            "name": "Second",
            "uri": "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Second",
            "protocol": "vless",
            "favorite": true,
        }],
        "subscriptions": if subscribed { json!([{
            "id": SUBSCRIPTION,
            "name": "Source",
            "url": "https://provider.invalid/token",
            "updatedAt": 1,
        }]) } else { json!([]) },
        "routingPreset": "custom",
        "customRules": [],
        "rulesUpdatedAt": 0,
        "startupConfigured": true,
        "startup": {"enabled": true, "target": "profile", "profileId": PROFILE_ONE, "mode": "global"},
        "onboardingComplete": true,
    });
    if legacy {
        store["version"] = Value::from(1);
        for profile in store["profiles"].as_array_mut().unwrap() {
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
            store.as_object_mut().unwrap().remove(key);
        }
    }
    store
}

fn rust_error(error: PrivateStoreError) -> &'static str {
    match error {
        PrivateStoreError::ProfileNotFound => "profile_not_found",
        PrivateStoreError::SubscribedProfile => "subscribed_profile",
        PrivateStoreError::DuplicateProfileName => "duplicate_profile_name",
        PrivateStoreError::InvalidName => "invalid_name",
        _ => "invalid_store",
    }
}

fn rust_result(store: &Value, mutation: &Value) -> Value {
    let profile_id = mutation["profileId"].as_str().unwrap().to_owned();
    let action = match mutation["kind"].as_str().unwrap() {
        "rename" => ProfileMutation::Rename {
            profile_id,
            new_name: mutation["newName"].as_str().unwrap().to_owned(),
        },
        "favorite" => ProfileMutation::Favorite {
            profile_id,
            enabled: mutation["enabled"].as_bool().unwrap(),
        },
        "delete" => ProfileMutation::Delete { profile_id },
        _ => unreachable!(),
    };
    match apply_profile_mutation(&serde_json::to_string(store).unwrap(), action) {
        Ok(result) => {
            let normalized: Value = serde_json::from_slice(result.payload()).unwrap();
            let canonical = serde_json::to_vec(&normalized).unwrap();
            json!({
                "ok": true,
                "changed": result.changed,
                "sha256": format!("{:x}", Sha256::digest(canonical)),
            })
        }
        Err(error) => json!({"ok": false, "code": rust_error(error)}),
    }
}

fn python_result(store: &Value, mutation: &Value) -> Value {
    let mut child = Command::new("python3")
        .arg(root().join("tools/profile_mutation_parity.py"))
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
        .write_all(&serde_json::to_vec(&json!({"store": store, "mutation": mutation})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn python_and_rust_profile_mutations_match() {
    let cases = [
        (
            base_store(false, false),
            json!({"kind": "rename", "profileId": PROFILE_ONE, "newName": "Renamed"}),
        ),
        (
            base_store(false, false),
            json!({"kind": "rename", "profileId": PROFILE_ONE, "newName": "First"}),
        ),
        (
            base_store(false, false),
            json!({"kind": "favorite", "profileId": PROFILE_ONE, "enabled": true}),
        ),
        (
            base_store(false, false),
            json!({"kind": "favorite", "profileId": PROFILE_ONE, "enabled": false}),
        ),
        (
            base_store(false, false),
            json!({"kind": "delete", "profileId": PROFILE_ONE}),
        ),
        (
            base_store(false, true),
            json!({"kind": "favorite", "profileId": PROFILE_ONE, "enabled": true}),
        ),
        (
            base_store(true, false),
            json!({"kind": "rename", "profileId": PROFILE_ONE, "newName": "Renamed"}),
        ),
        (
            base_store(true, false),
            json!({"kind": "delete", "profileId": PROFILE_ONE}),
        ),
        (
            base_store(false, false),
            json!({"kind": "rename", "profileId": PROFILE_ONE, "newName": "Second"}),
        ),
        (
            base_store(false, false),
            json!({"kind": "rename", "profileId": PROFILE_ONE, "newName": "bad\nname"}),
        ),
        (
            base_store(false, false),
            json!({"kind": "delete", "profileId": "00000000-0000-4000-8000-000000000099"}),
        ),
    ];
    for (store, mutation) in cases {
        assert_eq!(
            rust_result(&store, &mutation),
            python_result(&store, &mutation)
        );
    }
}

#[test]
fn parity_output_never_contains_private_store_values() {
    let store = base_store(true, false);
    let result = python_result(
        &store,
        &json!({"kind": "rename", "profileId": PROFILE_ONE, "newName": "blocked"}),
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
