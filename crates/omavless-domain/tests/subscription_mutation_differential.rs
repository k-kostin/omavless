// SPDX-License-Identifier: MIT

use omavless_domain::private_store::{
    IncomingSubscriptionProfile, PrivateStoreError, SubscriptionMutation,
    SubscriptionMutationContext, apply_subscription_mutation,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const LOCAL: &str = "00000000-0000-4000-8000-000000000001";
const MANAGED_ONE: &str = "00000000-0000-4000-8000-000000000002";
const MANAGED_TWO: &str = "00000000-0000-4000-8000-000000000003";
const GENERATED: &str = "00000000-0000-4000-8000-000000000004";
const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";
const OTHER_SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000002";
const LOCAL_URI: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Local";
const FIRST_URI: &str =
    "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#First";
const SECOND_URI: &str =
    "trojan://private-password@192.0.2.3:443?security=tls&sni=edge.example.invalid#Second";
const NEW_URI: &str = "hy2://private-auth@192.0.2.4:443?sni=edge.example.invalid";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn local_profile() -> Value {
    json!({
        "id": LOCAL,
        "name": "Local",
        "uri": LOCAL_URI,
        "protocol": "vless",
        "favorite": false,
    })
}

fn empty_store() -> Value {
    json!({
        "version": 3,
        "activeId": "",
        "lastId": LOCAL,
        "profiles": [local_profile()],
        "subscriptions": [],
        "routingPreset": "custom",
        "customRules": [],
        "rulesUpdatedAt": 0,
        "startupConfigured": true,
        "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
        "onboardingComplete": true,
    })
}

fn v1_store() -> Value {
    json!({
        "version": 1,
        "activeId": "",
        "lastId": LOCAL,
        "profiles": [{"id": LOCAL, "name": "Local", "uri": LOCAL_URI}],
    })
}

fn subscribed_store(active: bool) -> Value {
    let first_key = omavless_profile::canonical::parse_canonical(FIRST_URI)
        .unwrap()
        .subscription_identity();
    let second_key = omavless_profile::canonical::parse_canonical(SECOND_URI)
        .unwrap()
        .subscription_identity();
    json!({
        "version": 3,
        "activeId": if active { MANAGED_TWO } else { "" },
        "lastId": MANAGED_TWO,
        "profiles": [local_profile(), {
            "id": MANAGED_ONE,
            "name": "First",
            "uri": FIRST_URI,
            "protocol": "vless",
            "subscriptionId": SUBSCRIPTION,
            "subscriptionKey": first_key,
            "missing": false,
            "favorite": true,
        }, {
            "id": MANAGED_TWO,
            "name": "Second",
            "uri": SECOND_URI,
            "protocol": "trojan",
            "subscriptionId": SUBSCRIPTION,
            "subscriptionKey": second_key,
            "missing": false,
            "favorite": false,
        }],
        "subscriptions": [{
            "id": SUBSCRIPTION,
            "name": "Source",
            "url": "https://provider.invalid/old-token",
            "updatedAt": 1,
        }],
        "routingPreset": "custom",
        "customRules": [],
        "rulesUpdatedAt": 0,
        "startupConfigured": true,
        "startup": {"enabled": true, "target": "profile", "profileId": MANAGED_TWO, "mode": "global"},
        "onboardingComplete": true,
    })
}

fn v2_subscribed_store() -> Value {
    let mut store = subscribed_store(false);
    store["version"] = Value::from(2);
    for profile in store["profiles"].as_array_mut().unwrap() {
        profile.as_object_mut().unwrap().remove("protocol");
    }
    store
}

fn dual_identity_store() -> Value {
    let mut store = subscribed_store(false);
    let alternate_key = omavless_profile::canonical::parse_canonical(SECOND_URI)
        .unwrap()
        .subscription_identity();
    store["profiles"].as_array_mut().unwrap().pop();
    store["profiles"][1]["subscriptionKey"] = Value::from(alternate_key);
    store["lastId"] = Value::from(MANAGED_ONE);
    store["startup"]["profileId"] = Value::from(MANAGED_ONE);
    store
}

fn entries(value: &Value) -> Vec<IncomingSubscriptionProfile> {
    value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| IncomingSubscriptionProfile {
            uri: entry["uri"].as_str().unwrap().to_owned(),
            new_id: entry["newId"].as_str().unwrap().to_owned(),
        })
        .collect()
}

fn rust_result(store: &Value, mutation: &Value, active_service: bool) -> Value {
    let subscription_id = mutation["subscriptionId"].as_str().unwrap().to_owned();
    let action = match mutation["kind"].as_str().unwrap() {
        "add" => SubscriptionMutation::Add {
            subscription_id,
            name: mutation["name"].as_str().unwrap().to_owned(),
            url: mutation["url"].as_str().unwrap().to_owned(),
            entries: entries(mutation),
            updated_at: mutation["updatedAt"].as_u64().unwrap(),
        },
        "update" => SubscriptionMutation::Update {
            subscription_id,
            name: mutation["name"].as_str().unwrap().to_owned(),
            url: mutation["url"].as_str().unwrap().to_owned(),
            entries: entries(mutation),
            updated_at: mutation["updatedAt"].as_u64().unwrap(),
        },
        "delete" => SubscriptionMutation::Delete { subscription_id },
        _ => unreachable!(),
    };
    let context = SubscriptionMutationContext { active_service };
    match apply_subscription_mutation(&serde_json::to_string(store).unwrap(), action, context) {
        Ok(result) => {
            let normalized: Value = serde_json::from_slice(result.payload()).unwrap();
            let canonical = serde_json::to_vec(&normalized).unwrap();
            json!({
                "ok": true,
                "changed": result.changed,
                "counts": {
                    "added": result.counts.added,
                    "removed": result.counts.removed,
                    "stale": result.counts.stale,
                    "total": result.counts.total,
                },
                "sha256": format!("{:x}", Sha256::digest(canonical)),
            })
        }
        Err(error) => json!({"ok": false, "code": error.code()}),
    }
}

fn python_result(store: &Value, mutation: &Value, active_service: bool) -> Value {
    let mut child = Command::new("python3")
        .arg(root().join("tools/subscription_mutation_parity.py"))
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
        .write_all(
            &serde_json::to_vec(&json!({
                "store": store,
                "mutation": mutation,
                "context": {"activeService": active_service},
            }))
            .unwrap(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn python_and_rust_subscription_store_mutations_match() {
    let add = json!({
        "kind": "add", "subscriptionId": SUBSCRIPTION,
        "name": "  Source  ", "url": " https://provider.invalid/private-token ",
        "entries": [
            {"uri": FIRST_URI, "newId": MANAGED_ONE},
            {"uri": NEW_URI, "newId": GENERATED},
        ],
        "updatedAt": 123,
    });
    let update = json!({
        "kind": "update", "subscriptionId": SUBSCRIPTION,
        "name": "Updated", "url": "https://provider.invalid/new-token",
        "entries": [
            {"uri": FIRST_URI, "newId": "00000000-0000-4000-8000-000000000099"},
            {"uri": NEW_URI, "newId": GENERATED},
        ],
        "updatedAt": 456,
    });
    let c1_label_uri = FIRST_URI.replace("#First", "#First%C2%85Marker");
    let cases = [
        (empty_store(), add.clone(), false),
        (v1_store(), add, false),
        (subscribed_store(false), update.clone(), false),
        (v2_subscribed_store(), update.clone(), false),
        (subscribed_store(true), update, false),
        (
            subscribed_store(false),
            json!({
                "kind": "update", "subscriptionId": SUBSCRIPTION,
                "name": "Updated", "url": "https://provider.invalid/new-token",
                "entries": [{"uri": FIRST_URI, "newId": "not-a-generated-id"}],
                "updatedAt": 456,
            }),
            false,
        ),
        (
            subscribed_store(false),
            json!({
                "kind": "update", "subscriptionId": SUBSCRIPTION,
                "name": "Updated", "url": "https://provider.invalid/new-token",
                "entries": [{"uri": c1_label_uri, "newId": "ignored"}],
                "updatedAt": 456,
            }),
            false,
        ),
        (
            dual_identity_store(),
            json!({
                "kind": "update", "subscriptionId": SUBSCRIPTION,
                "name": "Updated", "url": "https://provider.invalid/new-token",
                "entries": [
                    {"uri": FIRST_URI, "newId": "ignored-one"},
                    {"uri": SECOND_URI, "newId": "ignored-two"},
                ],
                "updatedAt": 456,
            }),
            false,
        ),
        (
            subscribed_store(false),
            json!({"kind": "delete", "subscriptionId": SUBSCRIPTION}),
            false,
        ),
        (
            subscribed_store(true),
            json!({"kind": "delete", "subscriptionId": SUBSCRIPTION}),
            true,
        ),
        (
            empty_store(),
            json!({"kind": "delete", "subscriptionId": OTHER_SUBSCRIPTION}),
            false,
        ),
        (
            subscribed_store(false),
            json!({
                "kind": "update", "subscriptionId": OTHER_SUBSCRIPTION,
                "name": "Other", "url": "https://provider.invalid/old-token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
        (
            subscribed_store(false),
            json!({
                "kind": "update", "subscriptionId": OTHER_SUBSCRIPTION,
                "name": "Other", "url": "https://provider.invalid/unique-token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "bad\nname", "url": "https://provider.invalid/token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://example.invalid:",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[::1]:/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[v1.fe]/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[v1.]/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://example.invalid／feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[fe80::1%25eth0]/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[::1]suffix/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[::1/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://[::1]:65536/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://example.invalid:not-a-port/feed",
                "entries": [{"uri": FIRST_URI, "newId": MANAGED_ONE}], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "http://provider.invalid/private-token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "x".repeat(81), "url": "https://provider.invalid/private-token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://provider.invalid/private-token",
                "entries": [{
                    "uri": FIRST_URI,
                    "newId": "00000000-0000-4000-8000-00000000000A",
                }],
                "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": SUBSCRIPTION,
                "name": "Source", "url": "https://provider.invalid/private-token",
                "entries": [{"uri": FIRST_URI, "newId": "not-a-uuid"}],
                "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add",
                "subscriptionId": "10000000-0000-1000-8000-000000000001",
                "name": "Source", "url": "https://provider.invalid/private-token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
        (
            empty_store(),
            json!({
                "kind": "add", "subscriptionId": "not-a-uuid",
                "name": "Source", "url": "https://provider.invalid/private-token",
                "entries": [], "updatedAt": 2,
            }),
            false,
        ),
    ];
    for (index, (store, mutation, active_service)) in cases.into_iter().enumerate() {
        assert_eq!(
            rust_result(&store, &mutation, active_service),
            python_result(&store, &mutation, active_service),
            "differential case {index}"
        );
    }
}

fn boundary_entries(count: usize) -> Value {
    Value::Array(
        (0..count)
            .map(|index| {
                json!({
                    "uri": format!(
                        "vless://22222222-2222-4222-8222-{index:012x}@node-{index}.invalid:443?security=none&type=tcp"
                    ),
                    "newId": format!("20000000-0000-4000-8000-{index:012x}"),
                })
            })
            .collect(),
    )
}

#[test]
fn subscription_entry_limit_matches_at_1024_and_rejects_1025_first() {
    for (count, expected) in [
        (1024, "too_many_profiles"),
        (1025, "too_many_subscription_entries"),
    ] {
        let mutation = json!({
            "kind": "add", "subscriptionId": SUBSCRIPTION,
            "name": "Source", "url": "https://provider.invalid/private-token",
            "entries": boundary_entries(count), "updatedAt": 2,
        });
        let rust = rust_result(&empty_store(), &mutation, false);
        assert_eq!(rust, python_result(&empty_store(), &mutation, false));
        assert_eq!(rust["code"], expected);
    }
}

#[test]
fn parity_output_and_errors_never_publish_private_subscription_data() {
    let store = subscribed_store(true);
    let mutation = json!({
        "kind": "delete", "subscriptionId": SUBSCRIPTION,
    });
    let public = python_result(&store, &mutation, true).to_string();
    for private in [
        "vless://",
        "trojan://",
        "provider.invalid",
        "private-password",
        "private-token",
        SUBSCRIPTION,
    ] {
        assert!(!public.contains(private));
    }
    let error = PrivateStoreError::SubscriptionNotFound;
    assert!(!format!("{error:?} {error}").contains(SUBSCRIPTION));
}
