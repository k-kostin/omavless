// SPDX-License-Identifier: MIT

//! Inactive v4 admission only. Synthetic keys and documentation-range hosts.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use omavless_domain::private_store::{
    CandidateExportFormat, CandidateProfileInput, PrivateStoreError,
    migrate_legacy_store_candidate, parse_candidate_private_store, parse_private_store,
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

#[test]
fn deletion_repairs_only_target_references_across_the_complete_graph() {
    let mut source = mixed();
    source["activeId"] = AWG_ID.into();
    source["lastId"] = AWG_ID.into();
    let candidate = parse_candidate_private_store(&source.to_string())
        .unwrap()
        .delete_standalone(AWG_ID)
        .unwrap();
    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    assert_eq!(output["activeId"], "");
    assert_eq!(
        output["lastId"], URI_ID,
        "first member is in the complete mixed order"
    );
    assert_eq!(output["startup"]["enabled"], false);
    assert_eq!(output["startup"]["profileId"], "");
    assert!(output["profiles"][0] == source["profiles"][0]);
    assert!(output["profiles"][1] == source["profiles"][1]);
    assert!(output["subscriptions"] == source["subscriptions"]);
    assert!(output["vendorExtension"] == source["vendorExtension"]);

    let candidate = parse_candidate_private_store(&mixed().to_string())
        .unwrap()
        .delete_standalone(WG_ID)
        .unwrap();
    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    assert_eq!(output["activeId"], "");
    assert_eq!(output["lastId"], AWG_ID);
    assert_eq!(output["startup"]["profileId"], AWG_ID);
    assert_eq!(output["startup"]["enabled"], true);
    assert!(matches!(
        parse_candidate_private_store(&mixed().to_string())
            .unwrap()
            .delete_standalone(URI_ID),
        Err(PrivateStoreError::SubscribedProfile)
    ));
    assert!(matches!(
        parse_candidate_private_store(&mixed().to_string())
            .unwrap()
            .delete_standalone(SUB_ID),
        Err(PrivateStoreError::ProfileNotFound)
    ));
}

#[test]
fn replacement_and_typed_export_preserve_identity_favorite_and_pointers() {
    let source = mixed();
    let (candidate, changed) = parse_candidate_private_store(&source.to_string())
        .unwrap()
        .replace_standalone(
            WG_ID,
            "WG",
            CandidateProfileInput::WireGuard(parse_wireguard_config(&native(false)).unwrap()),
        )
        .unwrap();
    assert!(!changed);
    let (candidate, changed) = candidate
        .replace_standalone(
            WG_ID,
            "New AWG",
            CandidateProfileInput::WireGuard(parse_wireguard_config(&native(true)).unwrap()),
        )
        .unwrap();
    assert!(changed);
    let exported = candidate.export_private_credential(WG_ID).unwrap();
    assert_eq!(exported.format(), CandidateExportFormat::WireGuardRecord);
    let restored = omavless_profile::wireguard::private_record::parse_private_wireguard_record(
        exported.expose_private_bytes(),
    )
    .unwrap();
    assert_eq!(restored.facts().flavor.protocol_name(), "amneziawg");
    let uri_export = candidate.export_private_credential(URI_ID).unwrap();
    assert_eq!(uri_export.format(), CandidateExportFormat::Uri);
    assert!(uri_export.expose_private_bytes() == URI.as_bytes());
    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    assert_eq!(output["profiles"][1]["id"], WG_ID);
    assert_eq!(output["profiles"][1]["favorite"], true);
    assert_eq!(output["activeId"], WG_ID);
    assert_eq!(output["lastId"], AWG_ID);
    assert_eq!(output["startup"]["profileId"], AWG_ID);
    assert!(output["profiles"][0] == source["profiles"][0]);
    assert!(output["profiles"][2] == source["profiles"][2]);
    assert!(
        parse_private_store(std::str::from_utf8(&serde_json::to_vec(&output).unwrap()).unwrap())
            .is_ok()
    );
}

#[test]
fn native_export_and_editor_seed_restore_every_credential_without_store_changes() {
    let source = mixed().to_string();
    let candidate = parse_candidate_private_store(&source).unwrap();
    for (id, name, amnezia) in [(WG_ID, "WG", false), (AWG_ID, "AWG", true)] {
        let original = parse_wireguard_config(&native(amnezia)).unwrap();
        let export = candidate.export_private_native_credential(id).unwrap();
        assert_eq!(export.format(), CandidateExportFormat::WireGuardConfig);
        let text = std::str::from_utf8(export.expose_private_bytes()).unwrap();
        let restored = parse_wireguard_config(text).unwrap();
        assert!(restored.subscription_identity() == original.subscription_identity());
        assert!(
            restored.render_mihomo_proxy("test", None)
                == original.render_mihomo_proxy("test", None)
        );
        let editor = candidate.private_edit_input(id).unwrap();
        assert!(editor.private_name() == name);
        assert!(
            editor.private_credential().expose_private_bytes() == export.expose_private_bytes()
        );
    }
    let export = candidate.export_private_native_credential(URI_ID).unwrap();
    assert_eq!(export.format(), CandidateExportFormat::Uri);
    assert!(export.expose_private_bytes() == URI.as_bytes());
    assert!(matches!(
        candidate.private_edit_input(URI_ID),
        Err(PrivateStoreError::SubscribedProfile)
    ));
    assert!(matches!(
        candidate.private_edit_input(SUB_ID),
        Err(PrivateStoreError::ProfileNotFound)
    ));
    assert_eq!(candidate.profile_counts(), (1, 2, 1));
    assert_eq!(candidate.pointer_presence(), (true, true, true));
    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    let before: Value = serde_json::from_str(&source).unwrap();
    assert!(
        output == before,
        "explicit private reads must not mutate complete candidate"
    );
}

#[test]
fn cross_family_import_replace_refuse_loss_and_preserve_unrelated_credentials() {
    let source = mixed();
    let candidate = parse_candidate_private_store(&source.to_string()).unwrap();
    let (candidate, changed) = candidate
        .replace_standalone(
            WG_ID,
            "Converted",
            CandidateProfileInput::Uri(URI.to_owned()),
        )
        .unwrap();
    assert!(changed);
    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    assert!(output["profiles"][1].get("wireguard").is_none());
    assert_eq!(output["profiles"][1]["favorite"], true);
    assert_eq!(output["activeId"], WG_ID);
    assert!(output["profiles"][2] == source["profiles"][2]);
    let candidate = parse_candidate_private_store(&output.to_string())
        .unwrap()
        .with_profile(
            "00000000-0000-0000-0000-000000000099",
            "New URI",
            CandidateProfileInput::Uri(URI.to_owned()),
        )
        .unwrap();
    assert_eq!(candidate.profile_counts(), (3, 1, 1));
    assert!(matches!(
        parse_candidate_private_store(&source.to_string())
            .unwrap()
            .replace_standalone(
                URI_ID,
                "Managed",
                CandidateProfileInput::Uri(URI.to_owned())
            ),
        Err(PrivateStoreError::SubscribedProfile)
    ));
    assert!(matches!(
        parse_candidate_private_store(&source.to_string())
            .unwrap()
            .replace_standalone(WG_ID, "AWG", CandidateProfileInput::Uri(URI.to_owned())),
        Err(PrivateStoreError::DuplicateProfileName)
    ));
    let mut standalone = source;
    let row = standalone["profiles"][0].as_object_mut().unwrap();
    row.remove("subscriptionId");
    row.remove("subscriptionKey");
    assert!(
        matches!(
            parse_candidate_private_store(&standalone.to_string())
                .unwrap()
                .replace_standalone(
                    URI_ID,
                    "Converted",
                    CandidateProfileInput::WireGuard(
                        parse_wireguard_config(&native(false)).unwrap()
                    )
                ),
            Err(PrivateStoreError::InvalidShape)
        ),
        "legacy extensions cannot disappear during conversion"
    );
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
        parse_private_store(&source).is_ok(),
        "shared domain loader admits strict v4; runtime publication is separate"
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
    assert!(parse_private_store(&source.to_string()).is_ok());

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
    wrong["profiles"][1]["id"] = SUB_ID.into();
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
fn candidate_uri_delete_and_replace_match_existing_v3_semantics() {
    use omavless_domain::private_store::{ProfileMutation, apply_profile_mutation};
    let source = json!({
        "version":3,
        "profiles":[
            {"id":URI_ID,"name":"First","protocol":"vless","uri":URI,"favorite":true,"extension":{"keep":true}},
            {"id":WG_ID,"name":"Second","protocol":"vless","uri":URI}
        ],
        "activeId":URI_ID,"lastId":URI_ID,
        "startup":{"enabled":true,"target":"profile","profileId":URI_ID,"mode":"rule"}
    }).to_string();
    for replace in [false, true] {
        let mutation = if replace {
            ProfileMutation::Replace {
                profile_id: URI_ID.into(),
                new_name: "Renamed".into(),
                new_input: format!("{URI}#Changed"),
            }
        } else {
            ProfileMutation::Delete {
                profile_id: URI_ID.into(),
            }
        };
        let expected = apply_profile_mutation(&source, mutation).unwrap();
        let mut expected: Value = serde_json::from_slice(expected.payload()).unwrap();
        expected["version"] = 4.into();
        let candidate = migrate_legacy_store_candidate(&source).unwrap();
        let candidate = if replace {
            candidate
                .replace_standalone(
                    URI_ID,
                    "Renamed",
                    CandidateProfileInput::Uri(format!("{URI}#Changed")),
                )
                .unwrap()
                .0
        } else {
            candidate.delete_standalone(URI_ID).unwrap()
        };
        let actual: Value =
            serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
        assert!(
            actual == expected,
            "candidate URI mutation matches established v3 normalization"
        );
    }
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
fn candidate_reuses_opaque_id_count_and_structured_row_ownership_bounds() {
    for id in [
        "",
        "display-name",
        "https://example.invalid/private-id",
        "00000000",
    ] {
        for field in ["profiles", "subscriptions"] {
            let mut source = mixed();
            source[field][0]["id"] = id.into();
            assert!(parse_candidate_private_store(&source.to_string()).is_err());
        }
    }
    // Cross-graph collisions must reject for either credential family, not
    // merely when an append helper notices the subscription's existing ID.
    for row in [0, 1, 2] {
        let mut source = mixed();
        source["profiles"][row]["id"] = SUB_ID.into();
        assert!(parse_candidate_private_store(&source.to_string()).is_err());
    }
    for row in [1, 2] {
        for field in ["extension", "subscriptionId", "missing", "uri"] {
            let mut source = mixed();
            source["profiles"][row][field] = json!("synthetic-unowned-metadata");
            assert!(parse_candidate_private_store(&source.to_string()).is_err());
        }
    }
    let mut source = mixed();
    let row = source["profiles"][0].clone();
    source["profiles"] = Value::Array(
        (0..=omavless_domain::store::MAX_PROFILES)
            .map(|index| {
                let mut row = row.clone();
                row["id"] = format!("00000000-0000-0000-0000-{index:012x}").into();
                row
            })
            .collect(),
    );
    assert!(parse_candidate_private_store(&source.to_string()).is_err());
    let mut source = mixed();
    source["subscriptions"] = Value::Array((0..=omavless_domain::store::MAX_SUBSCRIPTIONS).map(|index| {
        json!({"id":format!("10000000-0000-0000-0000-{index:012x}"),
            "name":format!("Synthetic {index}"),"url":format!("https://provider{index}.example.invalid/sub")})
    }).collect());
    assert!(parse_candidate_private_store(&source.to_string()).is_err());
}

#[test]
fn explicit_candidate_migration_preserves_current_bundle_and_backup_boundaries() {
    use omavless_domain::private_store::{
        apply_profile_import, validate_existing_store_without_repair,
    };
    let empty = json!({"version":3,"profiles":[]}).to_string();
    let initial = apply_profile_import(&empty, URI_ID, "Original", URI).unwrap();
    let initial = std::str::from_utf8(initial.payload()).unwrap();
    let second = URI.replace("192.0.2.1", "192.0.2.9");
    let plan = parse_private_store(initial)
        .unwrap()
        .plan_local_bundle(&second)
        .unwrap();
    assert!(plan.counts().accepted == 1 && plan.counts().duplicates == 0);
    let mutation = plan.into_mutation(|| WG_ID.to_owned()).unwrap();
    let original = std::str::from_utf8(mutation.payload()).unwrap();
    assert!(validate_existing_store_without_repair(original).is_ok());
    assert!(
        parse_private_store(original)
            .unwrap()
            .into_profile_probe_profiles(None)
            .unwrap()
            .len()
            == 2
    );
    let before: Value = serde_json::from_str(original).unwrap();
    assert!(before["version"] == 3 && before["lastId"] == "");
    let candidate = migrate_legacy_store_candidate(original).unwrap();
    assert!(candidate.profile_counts() == (2, 0, 0));
    let bytes = candidate.into_private_bytes().unwrap();
    let mut expected = before;
    expected["version"] = 4.into();
    assert!(serde_json::from_slice::<Value>(&bytes).unwrap() == expected);
    let candidate = std::str::from_utf8(&bytes).unwrap();
    assert!(parse_private_store(candidate).is_ok());
    assert!(validate_existing_store_without_repair(candidate).is_ok());
    assert!(
        parse_private_store(original)
            .unwrap()
            .projection()
            .profile_count
            == 2
    );
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
    assert!(parse_private_store(std::str::from_utf8(&bytes).unwrap()).is_ok());
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

fn stale_candidate_source(pointers: usize, existing: bool) -> Value {
    let mut source = mixed();
    source["profiles"] = if existing {
        json!([{"id":URI_ID,"name":"Existing","protocol":"vless","uri":URI,"favorite":false}])
    } else {
        json!([])
    };
    source["activeId"] = if pointers == 0 || pointers >= 3 {
        WG_ID
    } else {
        ""
    }
    .into();
    source["lastId"] = if pointers == 1 || pointers >= 3 {
        WG_ID
    } else {
        ""
    }
    .into();
    source["startup"] = json!({"enabled":pointers != 4,"target":if pointers == 5 {"last"} else {"profile"},
        "profileId":if pointers == 5 {""} else {WG_ID},"mode":"global","vendorStartup":{"keep":true}});
    source
}

fn candidate_input(family: usize) -> CandidateProfileInput {
    match family {
        0 => CandidateProfileInput::Uri(URI.to_owned()),
        _ => {
            CandidateProfileInput::WireGuard(parse_wireguard_config(&native(family == 2)).unwrap())
        }
    }
}

fn assert_no_resurrected_candidate_pointers(
    candidate: omavless_domain::private_store::CandidatePrivateStore,
) {
    assert!(candidate.pointer_presence() == (false, false, false));
    let output: Value = serde_json::from_slice(&candidate.into_private_bytes().unwrap()).unwrap();
    assert!(output["activeId"] == "" && output["lastId"] == "");
    assert!(output["startup"]["enabled"] == false && output["startup"]["profileId"] == "");
    assert!(output["startup"]["vendorStartup"]["keep"] == true);
    assert!(output["vendorExtension"]["kept"] == 1);
}

#[test]
fn appending_matching_id_cannot_resurrect_stale_connection_or_startup_pointers() {
    for family in 0..3 {
        for pointers in 0..6 {
            let source = stale_candidate_source(pointers, false);
            let candidate = parse_candidate_private_store(&source.to_string()).unwrap();
            assert!(candidate.pointer_presence() == (false, false, false));
            let candidate = candidate
                .with_profile(WG_ID, "New", candidate_input(family))
                .unwrap();
            assert_no_resurrected_candidate_pointers(candidate);
        }
    }
}

#[test]
fn normalizing_stale_ids_does_not_disable_valid_existing_last_or_pinned_policy() {
    for family in 0..3 {
        for pinned in [false, true] {
            let mut source = stale_candidate_source(5, true);
            if pinned {
                source["activeId"] = URI_ID.into();
                source["lastId"] = URI_ID.into();
                source["startup"]["target"] = "profile".into();
                source["startup"]["profileId"] = URI_ID.into();
            }
            let before = parse_candidate_private_store(&source.to_string()).unwrap();
            let expected = if pinned {
                (true, true, true)
            } else {
                (false, false, true)
            };
            assert!(before.pointer_presence() == expected);
            let after = before
                .with_profile(WG_ID, "New", candidate_input(family))
                .unwrap();
            assert!(after.pointer_presence() == expected);
            let output: Value =
                serde_json::from_slice(&after.into_private_bytes().unwrap()).unwrap();
            assert!(output["activeId"] == if pinned { URI_ID } else { "" });
            assert!(output["lastId"] == if pinned { URI_ID } else { "" });
            assert!(output["startup"]["profileId"] == if pinned { URI_ID } else { "" });
            assert!(output["startup"]["enabled"] == true);
        }
    }
}

#[test]
fn metadata_noop_replace_and_delete_keep_normalized_pointers_before_later_append() {
    for family in 0..3 {
        for operation in 0..6 {
            let source = stale_candidate_source(3, true);
            let candidate = parse_candidate_private_store(&source.to_string()).unwrap();
            assert!(candidate.pointer_presence() == (false, false, false));
            let candidate = match operation {
                0 => candidate.rename_standalone(URI_ID, "Existing").unwrap().0,
                1 => candidate.rename_standalone(URI_ID, "Renamed").unwrap().0,
                2 => candidate.set_favorite(URI_ID, false).unwrap().0,
                3 => candidate.set_favorite(URI_ID, true).unwrap().0,
                4 => {
                    candidate
                        .replace_standalone(
                            URI_ID,
                            "Existing",
                            CandidateProfileInput::Uri(URI.to_owned()),
                        )
                        .unwrap()
                        .0
                }
                _ => candidate.delete_standalone(URI_ID).unwrap(),
            };
            assert!(candidate.pointer_presence() == (false, false, false));
            let candidate = candidate
                .with_profile(WG_ID, "New", candidate_input(family))
                .unwrap();
            assert_no_resurrected_candidate_pointers(candidate);
        }
    }
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
