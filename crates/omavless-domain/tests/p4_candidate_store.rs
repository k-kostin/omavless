// SPDX-License-Identifier: MIT

//! Inactive v4 admission only. Synthetic keys and documentation-range hosts.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use omavless_domain::private_store::{
    PrivateStoreError, migrate_legacy_store_candidate, parse_candidate_private_store,
    parse_private_store,
};
use omavless_profile::wireguard::parse_wireguard_config;
use serde_json::{Value, json};

const URI_ID: &str = "00000000-0000-0000-0000-000000000001";
const WG_ID: &str = "00000000-0000-0000-0000-000000000002";
const AWG_ID: &str = "00000000-0000-0000-0000-000000000003";
const SUB_ID: &str = "10000000-0000-0000-0000-000000000001";
const URI: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp";

fn native(amnezia: bool) -> String {
    let options = if amnezia {
        "Jc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n"
    } else {
        ""
    };
    format!(
        "[Interface]\nPrivateKey = {}\nAddress = 10.8.0.2/32\n{options}\n[Peer]\nPublicKey = {}\nAllowedIPs = 0.0.0.0/0\nEndpoint = 192.0.2.2:51820\n",
        STANDARD.encode([7_u8; 32]),
        STANDARD.encode([9_u8; 32])
    )
}

fn wireguard_value(amnezia: bool) -> Value {
    let parsed = parse_wireguard_config(&native(amnezia)).expect("synthetic config");
    serde_json::from_slice(
        parsed
            .private_record()
            .expect("record")
            .expose_private_bytes(),
    )
    .expect("JSON record")
}

fn mixed() -> Value {
    json!({
        "version": 4,
        "subscriptions": [{"id": SUB_ID, "name": "Provider", "url": "https://example.org/sub"}],
        "profiles": [
            {"id": URI_ID, "name": "URI", "protocol": "vless", "uri": URI,
             "subscriptionId": SUB_ID, "subscriptionKey": "a".repeat(64), "extraLegacy": {"retained": true}},
            {"id": WG_ID, "name": "WG", "protocol": "wireguard", "wireguard": wireguard_value(false), "favorite": true},
            {"id": AWG_ID, "name": "AWG", "protocol": "amneziawg", "wireguard": wireguard_value(true)}
        ],
        "activeId": WG_ID, "lastId": AWG_ID,
        "startup": {"enabled": true, "target": "profile", "profileId": AWG_ID, "mode": "global"},
        "routingPreset": "custom", "customRules": [], "onboardingComplete": true,
        "vendorExtension": {"kept": 1}
    })
}

#[test]
fn mixed_store_validates_one_graph_and_roundtrips_privately() {
    let source = mixed().to_string();
    let candidate = parse_candidate_private_store(&source).expect("inactive candidate");
    assert_eq!(candidate.profile_counts(), (1, 2, 1));
    assert_eq!(candidate.pointer_presence(), (true, true, true));
    let encoded = candidate.into_private_bytes().expect("private bytes");
    let encoded_value: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(encoded_value["vendorExtension"]["kept"], 1);
    assert_eq!(
        encoded_value["profiles"][0]["extraLegacy"]["retained"],
        true
    );
    assert_eq!(
        parse_candidate_private_store(std::str::from_utf8(&encoded).unwrap())
            .unwrap()
            .profile_counts(),
        (1, 2, 1)
    );
    assert!(
        parse_private_store(&source).is_err(),
        "runtime v3 loader stays closed"
    );
}

#[test]
fn wireguard_only_store_keeps_defaults_and_valid_startup_pointer() {
    let mut source = mixed();
    source["profiles"].as_array_mut().unwrap().remove(0);
    source.as_object_mut().unwrap().remove("subscriptions");
    source.as_object_mut().unwrap().remove("routingPreset");
    let candidate = parse_candidate_private_store(&source.to_string()).unwrap();
    assert_eq!(candidate.profile_counts(), (0, 2, 0));
    assert_eq!(candidate.pointer_presence(), (true, true, true));
    assert!(parse_private_store(&source.to_string()).is_err());

    source["startup"]["profileId"] = URI_ID.into();
    let stale = parse_candidate_private_store(&source.to_string()).unwrap();
    assert_eq!(stale.pointer_presence(), (true, true, false));
}

#[test]
fn mixed_store_rejects_conflicts_and_missing_sources() {
    let mut wrong = mixed();
    wrong["profiles"][1]["id"] = URI_ID.into();
    assert!(parse_candidate_private_store(&wrong.to_string()).is_err());

    let mut wrong = mixed();
    wrong["profiles"][1]["uri"] = URI.into();
    assert!(parse_candidate_private_store(&wrong.to_string()).is_err());

    let mut wrong = mixed();
    wrong["profiles"][1]
        .as_object_mut()
        .unwrap()
        .remove("wireguard");
    assert!(parse_candidate_private_store(&wrong.to_string()).is_err());

    let mut wrong = mixed();
    wrong["profiles"][1]["subscriptionId"] = SUB_ID.into();
    assert!(parse_candidate_private_store(&wrong.to_string()).is_err());

    let mut wrong = mixed();
    wrong["profiles"][1]["protocol"] = "amneziawg".into();
    assert!(parse_candidate_private_store(&wrong.to_string()).is_err());

    let mut wrong = mixed();
    wrong["profiles"][2]["wireguard"]["interface"]["postup"] = "echo bad".into();
    assert!(parse_candidate_private_store(&wrong.to_string()).is_err());
}

#[test]
fn original_bytes_reject_duplicate_keys_at_every_depth() {
    let source = mixed().to_string();
    let duplicate_root = source.replacen("\"version\":4", "\"version\":4,\"version\":4", 1);
    assert!(parse_candidate_private_store(&duplicate_root).is_err());
    let duplicate_escaped =
        source.replacen("\"version\":4", "\"version\":4,\"versi\\u006fn\":4", 1);
    assert!(parse_candidate_private_store(&duplicate_escaped).is_err());
    let duplicate_nested = source.replacen(
        "\"schemaVersion\":1",
        "\"schemaVersion\":1,\"schemaVersion\":1",
        1,
    );
    assert!(parse_candidate_private_store(&duplicate_nested).is_err());
}

#[test]
fn unsupported_versions_and_bounds_fail_closed() {
    for version in [3, 5] {
        let mut source = mixed();
        source["version"] = version.into();
        assert!(parse_candidate_private_store(&source.to_string()).is_err());
    }
    let mut source = mixed();
    source["padding"] = "x".repeat(5 * 1024 * 1024).into();
    assert!(parse_candidate_private_store(&source.to_string()).is_err());
}

#[test]
fn legacy_migration_and_standalone_append_are_private_in_memory_only() {
    let source = json!({
        "version": 3,
        "profiles": [{"id": URI_ID, "name": "URI", "protocol": "vless", "uri": URI,
            "extension": {"preserve": true}}],
        "activeId": URI_ID, "lastId": URI_ID,
        "startup": {"enabled": true, "target": "profile", "profileId": URI_ID, "mode": "global"}
    })
    .to_string();
    let candidate = migrate_legacy_store_candidate(&source).unwrap();
    assert_eq!(candidate.profile_counts(), (1, 0, 0));
    assert_eq!(candidate.pointer_presence(), (true, true, true));
    let wg = parse_wireguard_config(&native(false)).unwrap();
    let candidate = candidate.with_wireguard(WG_ID, "WG", wg).unwrap();
    assert_eq!(candidate.profile_counts(), (1, 1, 0));
    assert_eq!(candidate.pointer_presence(), (true, true, true));
    let bytes = candidate.into_private_bytes().unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["profiles"][0]["extension"]["preserve"], true);
    assert_eq!(document["version"], 4);
    assert!(parse_private_store(std::str::from_utf8(&bytes).unwrap()).is_err());
    assert_eq!(
        parse_private_store(&source)
            .unwrap()
            .projection()
            .profile_count,
        1
    );

    let duplicate = source.replacen("\"version\":3", "\"version\":3,\"version\":3", 1);
    assert!(migrate_legacy_store_candidate(&duplicate).is_err());
    let duplicate_id = migrate_legacy_store_candidate(&source).unwrap();
    assert!(
        duplicate_id
            .with_wireguard(
                URI_ID,
                "WG",
                parse_wireguard_config(&native(false)).unwrap()
            )
            .is_err()
    );

    for version in [1, 2] {
        let mut old: Value = serde_json::from_str(&source).unwrap();
        old["version"] = version.into();
        old["profiles"][0]
            .as_object_mut()
            .unwrap()
            .remove("protocol");
        let migrated = migrate_legacy_store_candidate(&old.to_string()).unwrap();
        assert_eq!(migrated.profile_counts(), (1, 0, 0));
        assert_eq!(migrated.pointer_presence(), (true, true, true));
    }
}

#[test]
fn candidate_private_output_clears_stale_references_without_losing_extensions() {
    let mut source = mixed();
    source["activeId"] = "00000000-0000-0000-0000-000000000099".into();
    source["lastId"] = "00000000-0000-0000-0000-000000000099".into();
    source["startup"]["profileId"] = "00000000-0000-0000-0000-000000000099".into();
    source["startup"]["extension"] = json!({"keep": true});
    let candidate = parse_candidate_private_store(&source.to_string()).unwrap();
    assert_eq!(candidate.pointer_presence(), (false, false, false));

    let bytes = candidate.into_private_bytes().unwrap();
    let output: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(output["activeId"], "");
    assert_eq!(output["lastId"], "");
    assert_eq!(output["startup"]["profileId"], "");
    assert_eq!(output["startup"]["enabled"], false);
    assert_eq!(output["startup"]["extension"]["keep"], true);
    assert_eq!(output["vendorExtension"]["kept"], 1);
    assert_eq!(output["profiles"][0]["extraLegacy"]["retained"], true);
    assert_eq!(
        parse_candidate_private_store(std::str::from_utf8(&bytes).unwrap())
            .unwrap()
            .pointer_presence(),
        (false, false, false)
    );
}

#[test]
fn standalone_append_refuses_cross_graph_ids_and_duplicate_names() {
    let wg = || parse_wireguard_config(&native(false)).unwrap();
    let source = mixed().to_string();
    assert!(
        parse_candidate_private_store(&source)
            .unwrap()
            .with_wireguard(SUB_ID, "New", wg())
            .is_err()
    );
    assert!(
        parse_candidate_private_store(&source)
            .unwrap()
            .with_wireguard(URI_ID, "New", wg())
            .is_err()
    );
    assert!(matches!(
        parse_candidate_private_store(&source)
            .unwrap()
            .with_wireguard("00000000-0000-0000-0000-000000000099", "URI", wg()),
        Err(PrivateStoreError::DuplicateProfileName)
    ));
}

#[test]
fn mixed_metadata_mutations_preserve_all_credentials_and_pointers() {
    let source = mixed();
    let original_wg = source["profiles"][1]["wireguard"].clone();
    let original_awg = source["profiles"][2]["wireguard"].clone();
    let original_uri = source["profiles"][0]["uri"].clone();
    let (candidate, changed) = parse_candidate_private_store(&source.to_string())
        .unwrap()
        .rename_standalone(WG_ID, "  Renamed WG  ")
        .unwrap();
    assert!(changed);
    let (candidate, changed) = candidate.set_favorite(AWG_ID, true).unwrap();
    assert!(changed);
    let (candidate, changed) = candidate.set_favorite(URI_ID, true).unwrap();
    assert!(changed, "subscribed URI favorites remain supported");
    assert_eq!(candidate.profile_counts(), (1, 2, 1));
    assert_eq!(candidate.pointer_presence(), (true, true, true));

    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    assert_eq!(output["profiles"][0]["uri"], original_uri);
    assert_eq!(output["profiles"][0]["extraLegacy"]["retained"], true);
    assert_eq!(output["profiles"][1]["wireguard"], original_wg);
    assert_eq!(output["profiles"][2]["wireguard"], original_awg);
    assert_eq!(output["profiles"][1]["name"], "Renamed WG");
    assert_eq!(output["profiles"][0]["favorite"], true);
    assert_eq!(output["profiles"][2]["favorite"], true);
    assert_eq!(output["activeId"], WG_ID);
    assert_eq!(output["lastId"], AWG_ID);
    assert_eq!(output["startup"]["profileId"], AWG_ID);
    assert_eq!(output["vendorExtension"]["kept"], 1);
}

#[test]
fn mixed_metadata_mutations_refuse_ambiguous_or_missing_targets() {
    let source = mixed().to_string();
    let candidate = || parse_candidate_private_store(&source).unwrap();
    assert!(matches!(
        candidate().rename_standalone(URI_ID, "Provider rename"),
        Err(PrivateStoreError::SubscribedProfile)
    ));
    assert!(matches!(
        candidate().rename_standalone(WG_ID, "URI"),
        Err(PrivateStoreError::DuplicateProfileName)
    ));
    assert!(matches!(
        candidate().rename_standalone("00000000-0000-0000-0000-000000000099", "Missing"),
        Err(PrivateStoreError::ProfileNotFound)
    ));
    assert!(matches!(
        candidate().set_favorite("00000000-0000-0000-0000-000000000099", true),
        Err(PrivateStoreError::ProfileNotFound)
    ));
    let (candidate, changed) = candidate().rename_standalone(WG_ID, "WG").unwrap();
    assert!(!changed);
    let (candidate, changed) = candidate.set_favorite(WG_ID, true).unwrap();
    assert!(!changed);
    assert_eq!(candidate.profile_counts(), (1, 2, 1));
}
