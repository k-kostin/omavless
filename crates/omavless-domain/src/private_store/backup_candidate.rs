// SPDX-License-Identifier: MIT

//! Inactive strict store admission for an already authenticated inner payload.
//! No encryption, template admission, filesystem operation or restore authority.

use super::{MAX_PRIVATE_STORE_BYTES, parse_private_store};
use serde::Deserialize;
use serde_json::Value;

// These deserialization-only types enforce exact keys and reject duplicate
// decoded member names before Value can collapse them. No private type has
// Debug, Display, Clone or Serialize; errors never retain serde diagnostics.
macro_rules! shape {
    ($name:ident { $($field:ident : $ty:ty => $wire:literal),* $(,)? }) => {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $name {
            $(#[serde(rename = $wire)] $field: $ty),*
        }
    };
}

shape!(StoreShape {
    _version: u8 => "version",
    _profiles: Vec<ProfileShape> => "profiles",
    _subscriptions: Vec<SubscriptionShape> => "subscriptions",
    _active: String => "activeId",
    _last: String => "lastId",
    _preset: String => "routingPreset",
    _rules: Vec<RuleShape> => "customRules",
    _rules_updated: u64 => "rulesUpdatedAt",
    _startup: StartupShape => "startup",
    _configured: bool => "startupConfigured",
    _onboarding: bool => "onboardingComplete",
});
shape!(ProfileShape {
    _id: String => "id", _name: String => "name", _uri: String => "uri",
    _protocol: String => "protocol", _favorite: bool => "favorite",
    _subscription: Option<String> => "subscriptionId",
    _key: Option<String> => "subscriptionKey", _missing: Option<bool> => "missing",
});
shape!(StoreV4Shape {
    _version: u8 => "version",
    _profiles: Vec<ProfileV4Shape> => "profiles",
    _subscriptions: Vec<SubscriptionShape> => "subscriptions",
    _active: String => "activeId", _last: String => "lastId",
    _preset: String => "routingPreset", _rules: Vec<RuleShape> => "customRules",
    _rules_updated: u64 => "rulesUpdatedAt", _startup: StartupShape => "startup",
    _configured: bool => "startupConfigured", _onboarding: bool => "onboardingComplete",
});
// Closed union of row keys; the shared strict v4 loader then enforces exactly
// one credential and the narrower standalone WG row key/ownership contract.
shape!(ProfileV4Shape {
    _id: String => "id", _name: String => "name", _protocol: String => "protocol",
    _uri: Option<String> => "uri", _wireguard: Option<Value> => "wireguard",
    _favorite: Option<bool> => "favorite", _subscription: Option<String> => "subscriptionId",
    _key: Option<String> => "subscriptionKey", _missing: Option<bool> => "missing",
});
shape!(SubscriptionShape {
    _id: String => "id", _name: String => "name", _url: String => "url",
    _updated: u64 => "updatedAt",
});
shape!(RuleShape {
    _id: String => "id", _kind: String => "kind", _value: String => "value",
    _action: String => "action",
});
shape!(StartupShape {
    _enabled: bool => "enabled", _target: String => "target",
    _profile: String => "profileId", _mode: String => "mode",
});

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InvalidBackupStore;

impl std::fmt::Display for InvalidBackupStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("backup_unreadable")
    }
}

pub(crate) struct ValidatedBackupStore<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) profiles: usize,
    pub(crate) subscriptions: usize,
    pub(crate) routing_preset: String,
}

pub(crate) fn validate(input: &[u8]) -> Result<ValidatedBackupStore<'_>, InvalidBackupStore> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(InvalidBackupStore);
    }
    let text = std::str::from_utf8(input).map_err(|_| InvalidBackupStore)?;
    let original: Value = serde_json::from_str(text).map_err(|_| InvalidBackupStore)?;
    match original["version"].as_u64() {
        Some(3) => {
            let _: StoreShape = serde_json::from_str(text).map_err(|_| InvalidBackupStore)?;
        }
        Some(4) => {
            let _: StoreV4Shape = serde_json::from_str(text).map_err(|_| InvalidBackupStore)?;
        }
        _ => return Err(InvalidBackupStore),
    }
    let validated = parse_private_store(text).map_err(|_| InvalidBackupStore)?;
    // Ordinary reads migrate versions, add defaults and repair stale pointers.
    // A portable archive must instead refuse every such semantic rewrite.
    if original != validated.document {
        return Err(InvalidBackupStore);
    }
    let projection = validated.projection();
    Ok(ValidatedBackupStore {
        bytes: input,
        profiles: projection.profile_count,
        subscriptions: projection.subscription_count,
        routing_preset: projection.routing_preset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PROFILE: &str = "10000000-0000-4000-8000-000000000001";
    const SUB: &str = "20000000-0000-4000-8000-000000000001";

    fn fixture() -> Value {
        json!({"version":3,"profiles":[{
            "id":PROFILE,"name":"Synthetic","uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic",
            "protocol":"vless","favorite":true,
            "subscriptionId":SUB,"subscriptionKey":"a".repeat(64),"missing":false
        }],"subscriptions":[{"id":SUB,"name":"Synthetic feed","url":"https://example.invalid/synthetic","updatedAt":10}],
        "activeId":PROFILE,"lastId":PROFILE,"routingPreset":"custom",
        "customRules":[{"id":"30000000-0000-4000-8000-000000000001","kind":"domain","value":"example.invalid","action":"direct"}],
        "rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},
        "startupConfigured":true,"onboardingComplete":true})
    }

    #[test]
    fn strict_store_preserves_exact_private_bytes_and_only_counts() {
        let input = format!("\n  {}\n", fixture());
        let result = validate(input.as_bytes()).unwrap();
        assert!(std::ptr::eq(result.bytes.as_ptr(), input.as_ptr()));
        assert_eq!(result.profiles, 1);
        assert_eq!(result.subscriptions, 1);
        assert!(result.bytes == input.as_bytes());
    }

    fn v4_fixture(amnezia: bool) -> Value {
        use base64::Engine as _;
        let options = if amnezia {
            "Jc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n"
        } else {
            ""
        };
        let config = format!(
            "[Interface]\nPrivateKey = {}\nAddress = 10.8.0.2/32\n{options}[Peer]\nPublicKey = {}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\n",
            base64::engine::general_purpose::STANDARD.encode([7u8; 32]),
            base64::engine::general_purpose::STANDARD.encode([9u8; 32])
        );
        let result = crate::private_store::apply_profile_import(
            &fixture().to_string(),
            "40000000-0000-4000-8000-000000000001",
            "Native",
            &config,
        )
        .unwrap();
        serde_json::from_slice(result.payload()).unwrap()
    }

    #[test]
    fn strict_v4_preserves_original_bytes_and_legacy_v3_archives() {
        for amnezia in [false, true] {
            let value = v4_fixture(amnezia);
            let input = format!(" \n{}\n", value);
            let validated = validate(input.as_bytes()).unwrap();
            assert!(std::ptr::eq(validated.bytes.as_ptr(), input.as_ptr()));
            assert!(validated.bytes == input.as_bytes());
            assert_eq!(validated.profiles, 2);
            assert_eq!(validated.subscriptions, 1);
        }
        assert!(validate(fixture().to_string().as_bytes()).is_ok());
    }

    #[test]
    fn strict_v4_refuses_ambiguous_credentials_unknown_nested_keys_and_duplicates() {
        let source = v4_fixture(false);
        for pointer in [
            "",
            "/profiles/0",
            "/profiles/1",
            "/subscriptions/0",
            "/startup",
            "/customRules/0",
            "/profiles/1/wireguard",
            "/profiles/1/wireguard/interface",
            "/profiles/1/wireguard/peer",
        ] {
            let mut value = source.clone();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("future".into(), json!("synthetic-private"));
            assert!(validate(value.to_string().as_bytes()).is_err());
        }
        for (key, value) in [
            ("uri", json!(source["profiles"][0]["uri"])),
            ("subscriptionId", json!(SUB)),
            ("missing", json!(false)),
            ("protocol", json!("amneziawg")),
        ] {
            let mut input = source.clone();
            input["profiles"][1][key] = value;
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
        let input = source.to_string();
        for needle in [
            "\"version\":4",
            "\"schemaVersion\":1",
            "\"address\":\"10.8.0.2/32\"",
            "\"endpoint\":\"192.0.2.2:51820\"",
        ] {
            let duplicate = input.replacen(needle, &format!("{needle},{needle}"), 1);
            assert_ne!(duplicate, input);
            assert!(validate(duplicate.as_bytes()).is_err());
        }
    }

    #[test]
    fn strict_v4_never_repairs_missing_defaults_stale_ids_or_semantic_record_rewrites() {
        let source = v4_fixture(true);
        for field in [
            "startup",
            "subscriptions",
            "customRules",
            "rulesUpdatedAt",
            "startupConfigured",
            "onboardingComplete",
        ] {
            let mut input = source.clone();
            input.as_object_mut().unwrap().remove(field);
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
        for pointer in ["/activeId", "/lastId", "/startup/profileId"] {
            let mut input = source.clone();
            *input.pointer_mut(pointer).unwrap() = json!("50000000-0000-4000-8000-000000000001");
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
        let mut input = source.clone();
        input["profiles"][1]["wireguard"]["peer"]["endpoint"] = json!("192.0.2.2:051820");
        assert!(validate(input.to_string().as_bytes()).is_err());
    }

    #[test]
    fn strict_store_accepts_standalone_and_escaped_display_text_without_rewriting() {
        let mut input = fixture();
        for field in ["subscriptionId", "subscriptionKey", "missing"] {
            input["profiles"][0].as_object_mut().unwrap().remove(field);
        }
        input["subscriptions"] = json!([]);
        input["profiles"][0]["name"] = "Синтетика".into();
        let original = input.to_string().replace("С", "\\u0421");
        let validated = validate(original.as_bytes()).unwrap();
        assert_eq!(validated.profiles, 1);
        assert_eq!(validated.subscriptions, 0);
        assert!(validated.bytes == original.as_bytes());
    }

    #[test]
    fn strict_store_refuses_unknown_members_at_every_object_scope() {
        for pointer in [
            "",
            "/profiles/0",
            "/subscriptions/0",
            "/customRules/0",
            "/startup",
        ] {
            for key in ["desired", "providerUsageV1", "path", "future"] {
                let mut input = fixture();
                input
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), json!("synthetic-private"));
                assert!(validate(input.to_string().as_bytes()).is_err());
                assert!(
                    parse_private_store(&input.to_string()).is_ok(),
                    "ordinary compatibility reader remains unchanged"
                );
            }
        }
    }

    #[test]
    fn strict_store_rejects_duplicate_and_escaped_equivalent_keys() {
        let input = fixture().to_string();
        for needle in [
            "\"version\":3",
            "\"favorite\":true",
            "\"updatedAt\":10",
            "\"action\":\"direct\"",
            "\"enabled\":false",
        ] {
            let duplicate = input.replacen(needle, &format!("{needle},{needle}"), 1);
            assert_ne!(duplicate, input);
            assert!(validate(duplicate.as_bytes()).is_err());
        }
        let escaped = input.replacen("\"version\":3", "\"version\":3,\"\\u0076ersion\":3", 1);
        assert!(validate(escaped.as_bytes()).is_err());
    }

    #[test]
    fn strict_store_refuses_migration_defaults_and_pointer_repair() {
        for version in [1, 2, 5] {
            let mut input = fixture();
            input["version"] = version.into();
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
        for field in [
            "startup",
            "subscriptions",
            "customRules",
            "rulesUpdatedAt",
            "onboardingComplete",
        ] {
            let mut input = fixture();
            input.as_object_mut().unwrap().remove(field);
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
        for pointer in ["/activeId", "/lastId", "/startup/profileId"] {
            let mut input = fixture();
            *input.pointer_mut(pointer).unwrap() = "40000000-0000-4000-8000-000000000001".into();
            assert!(parse_private_store(&input.to_string()).is_ok());
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
    }

    #[test]
    fn strict_store_reuses_existing_profile_relationship_and_rule_semantics() {
        for (pointer, invalid) in [
            ("/profiles/0/uri", json!("synthetic-private-invalid")),
            ("/profiles/0/protocol", json!("trojan")),
            (
                "/profiles/0/subscriptionId",
                json!("40000000-0000-4000-8000-000000000001"),
            ),
            ("/customRules/0/kind", json!("EXEC")),
            ("/subscriptions/0/url", json!("file:///synthetic")),
            ("/startup/enabled", json!("true")),
        ] {
            let mut input = fixture();
            *input.pointer_mut(pointer).unwrap() = invalid;
            assert!(validate(input.to_string().as_bytes()).is_err());
        }
    }

    #[test]
    fn strict_store_bounds_errors_and_rejects_non_json_or_oversize() {
        for bytes in [
            vec![0xff],
            b"synthetic-private-marker".to_vec(),
            vec![b' '; MAX_PRIVATE_STORE_BYTES + 1],
        ] {
            let error = validate(&bytes).err().unwrap();
            assert_eq!(format!("{error}"), "backup_unreadable");
            assert_eq!(format!("{error:?}"), "InvalidBackupStore");
        }
        let trailing = format!("{}{{}}", fixture());
        assert!(validate(trailing.as_bytes()).is_err());
    }
}
