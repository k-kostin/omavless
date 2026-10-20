// SPDX-License-Identifier: MIT

//! Shared typed domain admission only; not IPC/core/live product acceptance.
//! All credentials below are generated public test values, never live fixtures.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use omavless_domain::private_store::{
    CandidateExportFormat, CompatibilityPointerTarget, IncomingSubscriptionProfile,
    PrivateProfileCredential, PrivateStoreError, ProfileMutation, StartupPreferences,
    apply_compatibility_pointer_update, apply_profile_import, apply_profile_mutation,
    apply_startup_preferences, complete_onboarding, parse_private_store,
    validate_existing_store_without_repair,
};
use omavless_profile::wireguard::parse_wireguard_config;
use omavless_profile::wireguard::private_record::parse_private_wireguard_record;
use serde_json::{Value, json};

const ID: &str = "00000000-0000-0000-0000-000000000001";
const NEW_ID: &str = "00000000-0000-0000-0000-000000000002";
const URI: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp";

fn legacy_store() -> Value {
    json!({
        "version": 3,
        "profiles": [{"id": ID, "name": "Existing", "protocol": "vless", "uri": URI}],
        "activeId": ID, "lastId": ID,
        "startup": {"enabled": true, "target": "profile", "profileId": ID, "mode": "global"}
    })
}

fn native_config(amnezia: bool) -> String {
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

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}
fn mixed_bytes() -> Vec<u8> {
    apply_profile_import(
        &legacy_store().to_string(),
        NEW_ID,
        "WG",
        &native_config(false),
    )
    .unwrap()
    .payload()
    .to_vec()
}
fn mixed() -> String {
    String::from_utf8(mixed_bytes()).unwrap()
}
fn document(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn managed_configuration_refuses_keepalive_range_without_lowering_or_data_loss() {
    let options = format!(
        "S3 = 30\nS4 = 35\nHeaderProtectionKey = {}\nContentPaddingAddition = 12-100\nRekeyAfterTime = 100-120\nRekeyTimeout = 3-7\nRejectAfterTime = 150-180\nKeepaliveTimeout = 5-15\nMaxHandshakeAttempts = 15-20\n[Peer]",
        STANDARD.encode([15_u8; 32])
    );
    let config = native_config(true).replace("[Peer]", &options) + "PersistentKeepalive = 25-35\n";
    let changed =
        apply_profile_import(&legacy_store().to_string(), NEW_ID, "AWG", &config).unwrap();
    let original = changed.payload().to_vec();
    let store = parse_private_store(text(&original)).unwrap();
    let template = "external-controller: 0.0.0.0:9090\nmode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n";
    assert!(
        store
            .prepare_config(NEW_ID, template, "/private/synthetic/mihomo.sock")
            .is_err()
    );
    let exported = store.profile_credential_export(NEW_ID).unwrap();
    assert!(text(exported.expose_private_bytes()).contains("PersistentKeepalive = 25-35"));
    assert!(changed.payload() == original);
}

#[test]
fn first_native_import_prepares_atomic_typed_v4_without_changing_original() {
    for amnezia in [false, true] {
        for version in [1, 2, 3] {
            let mut original = legacy_store();
            original["version"] = version.into();
            original["rootExtension"] = json!({"kept":true});
            let source = original.to_string();
            let result =
                apply_profile_import(&source, NEW_ID, "Native", &native_config(amnezia)).unwrap();
            assert!(result.changed);
            let next = document(result.payload());
            assert_eq!(next["version"], 4);
            assert_eq!(
                next["profiles"][1]["protocol"],
                if amnezia { "amneziawg" } else { "wireguard" }
            );
            assert!(next["profiles"][1].get("uri").is_none());
            assert!(next["profiles"][1]["wireguard"].is_object());
            assert_eq!(next["profiles"][0]["uri"], URI);
            assert_eq!(next["activeId"], ID);
            assert_eq!(next["lastId"], ID);
            assert_eq!(next["startup"]["profileId"], ID);
            assert_eq!(next["rootExtension"], original["rootExtension"]);
            assert!(
                serde_json::from_str::<Value>(&source).unwrap() == original,
                "input remains immutable"
            );
        }
    }
}

#[test]
fn uri_only_mutations_do_not_promote_and_v4_never_silently_downgrades() {
    let initial = apply_profile_import(&legacy_store().to_string(), NEW_ID, "URI2", URI).unwrap();
    assert_eq!(document(initial.payload())["version"], 3);
    let source = mixed();
    let removed = apply_profile_mutation(
        &source,
        ProfileMutation::Delete {
            profile_id: NEW_ID.into(),
        },
    )
    .unwrap();
    assert_eq!(document(removed.payload())["version"], 4);
    let replaced = apply_profile_mutation(
        &source,
        ProfileMutation::Replace {
            profile_id: NEW_ID.into(),
            new_name: "URI2".into(),
            new_input: URI.into(),
        },
    )
    .unwrap();
    let value = document(replaced.payload());
    assert_eq!(value["version"], 4);
    assert_eq!(value["profiles"][1]["uri"], URI);
    assert!(value["profiles"][1].get("wireguard").is_none());
}

#[test]
fn first_native_promotion_never_resurrects_legacy_dangling_authority() {
    for amnezia in [false, true] {
        for target in ["last", "profile"] {
            for retained_uri in [false, true] {
                let mut source = legacy_store();
                if !retained_uri {
                    source["profiles"] = json!([]);
                }
                source["activeId"] = NEW_ID.into();
                source["lastId"] = NEW_ID.into();
                source["startup"]["target"] = target.into();
                source["startup"]["profileId"] =
                    if target == "profile" { NEW_ID } else { "" }.into();
                let before_enabled = parse_private_store(&source.to_string())
                    .unwrap()
                    .startup_preferences()
                    .enabled;
                let result = apply_profile_import(
                    &source.to_string(),
                    NEW_ID,
                    "Native",
                    &native_config(amnezia),
                )
                .unwrap();
                let after = document(result.payload());
                assert_eq!(after["activeId"], "");
                assert_eq!(after["lastId"], "");
                assert_eq!(after["startup"]["profileId"], "");
                assert_eq!(after["startup"]["enabled"], before_enabled);
                assert_eq!(
                    before_enabled,
                    target == "last" && retained_uri,
                    "valid existing Last policy is not a stale pin"
                );
            }
        }
    }
}

#[test]
fn structured_private_record_is_not_an_implicit_import_format() {
    let config = parse_wireguard_config(&native_config(false)).unwrap();
    let record = config.private_record().unwrap();
    assert!(parse_private_wireguard_record(record.expose_private_bytes()).is_ok());
    assert!(
        apply_profile_import(
            &legacy_store().to_string(),
            NEW_ID,
            "Wrong",
            text(record.expose_private_bytes())
        )
        .is_err()
    );
    let mut invalid = legacy_store();
    invalid["profiles"].as_array_mut().unwrap().push(json!({"id":NEW_ID,"name":"WG","protocol":"wireguard","wireguard":document(record.expose_private_bytes())}));
    assert!(
        parse_private_store(&invalid.to_string()).is_err(),
        "v3 cannot disguise a v4 credential row"
    );
}

#[test]
fn strict_v4_rejects_duplicates_discriminator_conflicts_and_unknown_native_fields() {
    let source = mixed();
    for needle in ["\"version\": 4", "\"favorite\": false", "\"enabled\": true"] {
        let duplicated = source.replacen(needle, &format!("{needle},{needle}"), 1);
        if duplicated != source {
            assert!(parse_private_store(&duplicated).is_err());
        }
    }
    let duplicate = source.replacen("\"version\": 4", "\"version\":4,\"\\u0076ersion\":4", 1);
    assert_ne!(duplicate, source);
    assert!(parse_private_store(&duplicate).is_err());
    let value: Value = serde_json::from_str(&source).unwrap();
    for (key, extra) in [
        ("uri", json!(URI)),
        ("subscriptionId", json!("")),
        ("missing", json!(false)),
        ("opaque", json!(true)),
    ] {
        let mut altered = value.clone();
        altered["profiles"][1][key] = extra;
        assert!(parse_private_store(&altered.to_string()).is_err());
    }
    for protocol in ["vless", "amneziawg", "unknown"] {
        let mut altered = value.clone();
        altered["profiles"][1]["protocol"] = protocol.into();
        assert!(parse_private_store(&altered.to_string()).is_err());
    }
    let mut altered = value.clone();
    altered["profiles"][1]["wireguard"]["peer"]["PostUp"] = "synthetic-unrun".into();
    assert!(parse_private_store(&altered.to_string()).is_err());
}

#[test]
fn v4_cross_graph_ids_and_duplicate_before_first_promotion_refuse() {
    let source = legacy_store().to_string();
    let duplicate = source.replacen("\"version\":3", "\"version\":3,\"version\":3", 1);
    assert_ne!(duplicate, source);
    assert!(apply_profile_import(&duplicate, NEW_ID, "WG", &native_config(false)).is_err());
    let mut source: Value = serde_json::from_str(&mixed()).unwrap();
    source["subscriptions"] =
        json!([{"id":NEW_ID,"name":"Collision","url":"https://example.invalid/sub"}]);
    assert!(parse_private_store(&source.to_string()).is_err());
}

#[test]
fn typed_export_edit_and_probes_never_invent_uri_or_omit_native() {
    let source = mixed();
    let store = parse_private_store(&source).unwrap();
    assert!(matches!(
        store.profile_export(NEW_ID),
        Err(PrivateStoreError::TypedCredentialRequired)
    ));
    let export = store.profile_credential_export(NEW_ID).unwrap();
    assert_eq!(export.format(), CandidateExportFormat::WireGuardConfig);
    let native = parse_wireguard_config(text(export.expose_private_bytes())).unwrap();
    assert_eq!(native.facts().flavor.protocol_name(), "wireguard");
    assert!(
        store.profile_edit_input(NEW_ID).unwrap().private_input()
            == text(export.expose_private_bytes())
    );
    assert!(store.profile_export(ID).unwrap().private_uri() == URI);
    assert!(matches!(
        parse_private_store(&source)
            .unwrap()
            .into_profile_probe_profiles(None),
        Err(PrivateStoreError::TypedCredentialRequired)
    ));
    let probes = parse_private_store(&source)
        .unwrap()
        .into_typed_profile_probe_profiles(None)
        .unwrap();
    assert_eq!(probes.len(), 2);
    assert!(matches!(probes[0].1, PrivateProfileCredential::Uri { .. }));
    assert!(matches!(
        probes[1].1,
        PrivateProfileCredential::WireGuard(_)
    ));
    assert_eq!(
        parse_private_store(&source)
            .unwrap()
            .into_profile_probe_profiles(Some(ID))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn all_six_families_have_complete_list_counts_and_private_details() {
    let mut source = legacy_store().to_string();
    let uris = [
        (
            "trojan://synthetic@192.0.2.3:443?security=tls&type=tcp",
            "trojan",
        ),
        ("hysteria2://synthetic@192.0.2.4:443", "hysteria2"),
        (
            "tuic://11111111-1111-4111-8111-111111111111:synthetic@192.0.2.5:443",
            "tuic",
        ),
    ];
    for (index, (uri, name)) in uris.into_iter().enumerate() {
        let id = format!("00000000-0000-0000-0000-{:012}", index + 3);
        source = text(
            apply_profile_import(&source, &id, name, uri)
                .unwrap()
                .payload(),
        )
        .into();
    }
    source = text(
        apply_profile_import(&source, NEW_ID, "WG", &native_config(false))
            .unwrap()
            .payload(),
    )
    .into();
    let awg_id = "00000000-0000-0000-0000-000000000008";
    source = text(
        apply_profile_import(&source, awg_id, "AWG", &native_config(true))
            .unwrap()
            .payload(),
    )
    .into();
    let store = parse_private_store(&source).unwrap();
    let counts = store.projection();
    assert_eq!(
        (
            counts.vless_count,
            counts.trojan_count,
            counts.hysteria2_count,
            counts.tuic_count,
            counts.wireguard_count,
            counts.amneziawg_count
        ),
        (1, 1, 1, 1, 1, 1)
    );
    assert_eq!(counts.profile_count, 6);
    let list = store.list_projection();
    let names = list
        .profiles()
        .iter()
        .map(|item| item.protocol().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "vless",
            "trojan",
            "hysteria2",
            "tuic",
            "wireguard",
            "amneziawg"
        ]
    );
    for (id, family) in [(NEW_ID, "wireguard"), (awg_id, "amneziawg")] {
        let details = store.profile_details(id).unwrap().into_private_ui_value();
        assert_eq!(details["protocol"], family);
        assert_eq!(details["server"], "192.0.2.2:51820");
        assert!(details.get("privatekey").is_none());
    }
}

#[test]
fn native_preview_has_one_shared_typed_model_and_secret_safe_debug() {
    for amnezia in [false, true] {
        let input = native_config(amnezia);
        let preview = omavless_domain::import::preview_import(&input, &[]).unwrap();
        let value = preview.private_ui_value();
        assert_eq!(value["kind"], "profile");
        assert_eq!(value["profile"]["version"], 1);
        assert_eq!(value["profile"]["experimental"], true);
        assert_eq!(value["profile"]["credentialHint"], "••••");
        assert_eq!(
            value["profile"]["suggestedName"],
            if amnezia { "AmneziaWG" } else { "WireGuard" }
        );
        assert_eq!(
            value["profile"]["protocol"],
            if amnezia { "amneziawg" } else { "wireguard" }
        );
        for canary in [
            STANDARD.encode([7_u8; 32]),
            STANDARD.encode([9_u8; 32]),
            input,
        ] {
            assert!(!format!("{preview:?}").contains(&canary));
            assert!(!value.to_string().contains(&canary));
        }
    }
}

#[test]
fn mixed_bundle_and_subscription_refresh_preserve_native_ownership() {
    use omavless_domain::private_store::{
        SubscriptionMutation, SubscriptionMutationContext, apply_subscription_mutation,
        apply_subscription_refresh, prepare_subscription_refresh,
    };
    let original = mixed();
    let plan = parse_private_store(&original)
        .unwrap()
        .plan_local_bundle(&URI.replace("192.0.2.1", "192.0.2.7"))
        .unwrap();
    let addition = plan
        .into_mutation(|| "00000000-0000-4000-8000-000000000010".into())
        .unwrap();
    let before = document(addition.payload());
    assert_eq!(before["version"], 4);
    let sub_id = "10000000-0000-4000-8000-000000000001";
    let result = apply_subscription_mutation(
        text(addition.payload()),
        SubscriptionMutation::Add {
            subscription_id: sub_id.into(),
            name: "Provider".into(),
            url: "https://example.invalid/sub".into(),
            updated_at: 10,
            entries: vec![IncomingSubscriptionProfile {
                uri: URI.replace("192.0.2.1", "192.0.2.8"),
                new_id: "00000000-0000-4000-8000-000000000011".into(),
            }],
        },
        SubscriptionMutationContext {
            active_service: false,
        },
    )
    .unwrap();
    let snapshot = prepare_subscription_refresh(text(result.payload()), sub_id).unwrap();
    let (refreshed, _) =
        apply_subscription_refresh(text(result.payload()), snapshot, vec![], 20, 0).unwrap();
    let after = document(refreshed.payload());
    assert_eq!(after["profiles"][1], before["profiles"][1]);
    assert_eq!(after["profiles"].as_array().unwrap().len(), 3);
    assert_eq!(after["version"], 4);
}

#[test]
fn startup_onboarding_and_pointer_operations_keep_native_bytes_and_extensions() {
    let mut source: Value = serde_json::from_str(&mixed()).unwrap();
    source["startup"]["extension"] = true.into();
    source["rootExtension"] = json!({"retained":true});
    let original_row = source["profiles"][1].clone();
    let preferences = StartupPreferences {
        enabled: true,
        target: "profile".into(),
        profile_id: NEW_ID.into(),
        mode: "global".into(),
    };
    let (settings, _) = apply_startup_preferences(&source.to_string(), &preferences).unwrap();
    let (onboarded, _) = complete_onboarding(text(&settings)).unwrap();
    let connected = apply_compatibility_pointer_update(
        text(&onboarded),
        CompatibilityPointerTarget::Connected {
            profile_id: NEW_ID.into(),
        },
    )
    .unwrap();
    let disconnected = apply_compatibility_pointer_update(
        text(connected.payload()),
        CompatibilityPointerTarget::Disconnected {
            prune_missing: true,
        },
    )
    .unwrap();
    let output = document(disconnected.payload());
    assert_eq!(output["profiles"][1], original_row);
    assert_eq!(output["lastId"], NEW_ID);
    assert_eq!(output["startup"]["profileId"], NEW_ID);
    assert_eq!(output["rootExtension"], source["rootExtension"]);
    assert_eq!(output["startup"]["extension"], true);
}

#[test]
fn strict_portable_v4_and_v3_keep_original_bytes_and_refuse_repair_or_extensions() {
    let source = mixed();
    assert!(validate_existing_store_without_repair(&source).is_ok());
    let legacy = apply_profile_import(
        &json!({"version":3,"profiles":[]}).to_string(),
        ID,
        "URI",
        URI,
    )
    .unwrap();
    assert!(validate_existing_store_without_repair(text(legacy.payload())).is_ok());
    for pointer in ["/activeId", "/lastId", "/startup/profileId"] {
        let mut source: Value = serde_json::from_str(&mixed()).unwrap();
        *source.pointer_mut(pointer).unwrap() = "00000000-0000-0000-0000-000000000099".into();
        assert!(parse_private_store(&source.to_string()).is_ok());
        assert!(validate_existing_store_without_repair(&source.to_string()).is_err());
    }
    for pointer in ["", "/profiles/0", "/profiles/1", "/startup"] {
        let mut source: Value = serde_json::from_str(&mixed()).unwrap();
        source
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        assert!(validate_existing_store_without_repair(&source.to_string()).is_err());
    }
}

#[test]
fn failed_conversion_cannot_drop_extensions_or_change_credential_ownership() {
    let mut source = legacy_store();
    source["profiles"][0]["unknown"] = json!({"retain":true});
    assert!(
        apply_profile_mutation(
            &source.to_string(),
            ProfileMutation::Replace {
                profile_id: ID.into(),
                new_name: "Native".into(),
                new_input: native_config(false)
            }
        )
        .is_err()
    );
    let mut source: Value = serde_json::from_str(&mixed()).unwrap();
    source["subscriptions"] = json!([{"id":"10000000-0000-0000-0000-000000000001","name":"Feed","url":"https://example.invalid/feed"}]);
    source["profiles"][0]["subscriptionId"] = "10000000-0000-0000-0000-000000000001".into();
    source["profiles"][0]["subscriptionKey"] = "a".repeat(64).into();
    assert!(
        apply_profile_mutation(
            &source.to_string(),
            ProfileMutation::Replace {
                profile_id: ID.into(),
                new_name: "Native".into(),
                new_input: native_config(true)
            }
        )
        .is_err()
    );
}

#[test]
fn replacement_promotion_validates_other_rows_and_cross_kind_ids_before_returning_bytes() {
    let source = legacy_store().to_string();
    let duplicate = source.replacen("\"version\":3", "\"version\":3,\"version\":3", 1);
    assert_ne!(duplicate, source);
    assert!(
        parse_private_store(&duplicate).is_ok(),
        "legacy URI duplicate policy unchanged"
    );
    assert!(
        apply_profile_mutation(
            &duplicate,
            ProfileMutation::Replace {
                profile_id: ID.into(),
                new_name: "Native".into(),
                new_input: native_config(false)
            }
        )
        .is_err()
    );
    assert!(apply_profile_import(&duplicate, NEW_ID, "Native", &native_config(false)).is_err());
    for collision in [false, true] {
        let mut source = legacy_store();
        if collision {
            source["subscriptions"] =
                json!([{"id":ID,"name":"Legacy collision","url":"https://example.invalid/sub"}]);
        } else {
            source["profiles"].as_array_mut().unwrap().push(json!({"id":NEW_ID,"name":"Other URI","uri":URI,"protocol":"vless","wireguard":{"legacyOpaque":"preserved"}}));
        }
        let original = source.to_string();
        assert!(
            parse_private_store(&original).is_ok(),
            "historical v3 admission"
        );
        for amnezia in [false, true] {
            assert!(
                apply_profile_mutation(
                    &original,
                    ProfileMutation::Replace {
                        profile_id: ID.into(),
                        new_name: "Native".into(),
                        new_input: native_config(amnezia)
                    }
                )
                .is_err(),
                "whole v4 graph must validate before a payload is returned"
            );
            assert!(
                apply_profile_import(
                    &original,
                    "00000000-0000-0000-0000-000000000003",
                    "Native",
                    &native_config(amnezia)
                )
                .is_err()
            );
        }
        assert!(serde_json::from_str::<Value>(&original).unwrap() == source);
    }
}

#[test]
fn native_redaction_inputs_include_each_credential_address_and_endpoint_fragment() {
    let store = parse_private_store(&mixed()).unwrap();
    let fragments = store.diagnostic_private_fragments();
    for private in [
        STANDARD.encode([7_u8; 32]),
        STANDARD.encode([9_u8; 32]),
        "10.8.0.2/32".into(),
        "192.0.2.2:51820".into(),
        "192.0.2.2".into(),
    ] {
        assert!(fragments.contains(&private));
    }
}

#[test]
fn legacy_uri_opaque_extensions_are_preserved_and_never_silently_promoted() {
    let mut source = legacy_store();
    source["profiles"][0]["wireguard"] = json!({"legacyOpaque":"kept"});
    let result = apply_profile_mutation(
        &source.to_string(),
        ProfileMutation::Favorite {
            profile_id: ID.into(),
            enabled: true,
        },
    )
    .unwrap();
    assert_eq!(
        document(result.payload())["profiles"][0]["wireguard"],
        source["profiles"][0]["wireguard"]
    );
    let result = apply_profile_mutation(
        &source.to_string(),
        ProfileMutation::Replace {
            profile_id: ID.into(),
            new_name: "URI renamed".into(),
            new_input: URI.into(),
        },
    )
    .unwrap();
    assert_eq!(
        document(result.payload())["profiles"][0]["wireguard"],
        source["profiles"][0]["wireguard"]
    );
    assert!(
        apply_profile_import(&source.to_string(), NEW_ID, "WG", &native_config(false)).is_err()
    );
}

#[test]
fn authenticated_mixed_v4_backup_roundtrips_and_restore_disables_startup_without_loss() {
    use omavless_domain::private_backup::{open, seal};
    let mut source: Value = serde_json::from_str(&mixed()).unwrap();
    source["routingPreset"] = "roscomvpn-default".into();
    let source = apply_profile_import(
        &source.to_string(),
        "00000000-0000-0000-0000-000000000008",
        "AWG",
        &native_config(true),
    )
    .unwrap();
    let original = document(source.payload());
    let template = include_bytes!("../../../templates/default.yaml");
    let sealed = seal(
        source.payload(),
        template,
        b"synthetic-only-long-passphrase",
    )
    .unwrap();
    let opened = open(&sealed, b"synthetic-only-long-passphrase").unwrap();
    assert!(opened.store() == source.payload());
    assert!(opened.template() == template);
    assert_eq!(opened.profile_count(), 3);
    let restored = opened.restore_store_off().unwrap();
    let after = document(&restored);
    let mut expected = original;
    expected["startup"]["enabled"] = false.into();
    assert!(after == expected);
    assert!(validate_existing_store_without_repair(text(&restored)).is_ok());
}
