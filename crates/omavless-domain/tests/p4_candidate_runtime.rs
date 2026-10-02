// SPDX-License-Identifier: MIT
//! Pure, inactive preparation only. Invented credentials; no files or core.
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use omavless_domain::config::assemble_runtime_config;
use omavless_domain::private_store::{
    CompatibilityPointerTarget, PrivateStoreError, apply_candidate_compatibility_pointer_update,
    apply_compatibility_pointer_update, parse_candidate_private_store, parse_private_store,
};
use omavless_domain::routing::{CustomRule, template_with_mode};
use omavless_profile::wireguard::parse_wireguard_config;
use serde_json::{Value, json};

const URI_ID: &str = "00000000-0000-0000-0000-000000000001";
const WG_ID: &str = "00000000-0000-0000-0000-000000000002";
const MISSING_ID: &str = "00000000-0000-0000-0000-000000000003";
const SUB_ID: &str = "10000000-0000-0000-0000-000000000001";
const NAME: &str = "Synthetic Canary Label";
const SOCKET: &str = "/private/synthetic/mihomo.sock";
const TEMPLATE: &str = "external-controller: 0.0.0.0:9090\nsecret: inherited-canary\nmode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n";
const VLESS: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp";

fn native(generation: usize) -> String {
    let mut options = String::new();
    if generation != 0 {
        options.push_str("Jc = 4\nJmin = 10\nJmax = 30\nS1 = 20\nS2 = 25\nH1 = 100\nH2 = 200\nH3 = 300\nH4 = 400\n");
    }
    if generation >= 2 {
        options.push_str("S3 = 30\nS4 = 35\n");
    }
    if generation >= 3 {
        options.push_str(&format!("I1 = <r 2><b 0x0102>\nHeaderProtectionKey = {}\nContentPaddingAddition = 10-100\nRekeyAfterTime = 100-120\nRekeyTimeout = 3-7\nRejectAfterTime = 150-180\nKeepaliveTimeout = 5-15\nMaxHandshakeAttempts = 15-20\n", STANDARD.encode([15_u8; 32])));
    }
    if generation == 4 {
        options.push_str("RandomTrailers = true\nDisableCookies = false\n");
    }
    format!(
        "[Interface]\nPrivateKey = {}\nAddress = 10.203.0.2/32, fd00:203::2/128\nDNS = 192.0.2.53, 2001:db8::53\nMTU = 1420\n{options}[Peer]\nPublicKey = {}\nPresharedKey = {}\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = [2001:db8::2]:51820\nPersistentKeepalive = {}\n",
        STANDARD.encode([7_u8; 32]),
        STANDARD.encode([9_u8; 32]),
        STANDARD.encode([11_u8; 32]),
        if generation >= 3 { "25-35" } else { "25" }
    )
}

fn rules() -> Vec<CustomRule> {
    vec![CustomRule::parse("domain", "proxy", "example.invalid").unwrap()]
}

fn base() -> Value {
    json!({"version":4,"profiles":[{"id":URI_ID,"name":"URI","protocol":"vless","uri":VLESS,"favorite":true,"vendorRow":{"opaque":42}}],
        "subscriptions":[],"activeId":URI_ID,"lastId":URI_ID,
        "startup":{"enabled":true,"target":"profile","profileId":URI_ID,"mode":"global","vendorStartup":true},
        "customRules":[{"id":"20000000-0000-0000-0000-000000000001","kind":"domain","action":"proxy","value":"example.invalid","vendorRule":7}],
        "routingPreset":"custom","rulesUpdatedAt":123,"startupConfigured":true,"onboardingComplete":true,"vendorRoot":{"keep":[1,2]}})
}

fn mixed(generation: usize) -> Value {
    let mut source = base();
    let profile = parse_wireguard_config(&native(generation)).unwrap();
    let record = profile.private_record().unwrap();
    let value: Value = serde_json::from_slice(record.expose_private_bytes()).unwrap();
    source["profiles"].as_array_mut().unwrap().push(json!({"id":WG_ID,"name":NAME,"protocol":profile.facts().flavor.protocol_name(),"wireguard":value,"favorite":true}));
    source
}

#[test]
fn all_native_flavors_prepare_three_modes_and_native_export_reimports_identically() {
    for generation in 0..=4 {
        let source = mixed(generation);
        let store = parse_candidate_private_store(&source.to_string()).unwrap();
        let profile = parse_wireguard_config(&native(generation)).unwrap();
        let exported = store.export_private_native_credential(WG_ID).unwrap();
        let restored =
            parse_wireguard_config(std::str::from_utf8(exported.expose_private_bytes()).unwrap())
                .unwrap();
        assert!(profile.subscription_identity() == restored.subscription_identity());
        for mode in ["global", "rule", "direct"] {
            let config = store
                .prepare_private_config_mode(WG_ID, TEMPLATE, SOCKET, mode)
                .unwrap();
            let expected = assemble_runtime_config(
                &template_with_mode(TEMPLATE, mode).unwrap(),
                &profile.render_mihomo_proxy(NAME, None),
                SOCKET,
                &rules(),
            )
            .unwrap();
            let reimported = assemble_runtime_config(
                &template_with_mode(TEMPLATE, mode).unwrap(),
                &restored.render_mihomo_proxy(NAME, None),
                SOCKET,
                &rules(),
            )
            .unwrap();
            assert!(config.expose_private_bytes() == expected.as_bytes());
            assert!(config.expose_private_bytes() == reimported.as_bytes());
            assert!(config.private_name() == NAME);
            assert!(expected.contains(&format!("mode: {mode}\n")));
            assert!(expected.contains("DOMAIN,example.invalid,PROXY"));
            assert!(!expected.contains("inherited-canary") && !expected.contains("0.0.0.0:9090"));
            assert!(expected.matches("external-controller").count() == 1);
            assert!(!expected.contains("PostUp") && !expected.contains("auto-route"));
        }
        let after: Value = serde_json::from_slice(&store.into_private_bytes().unwrap()).unwrap();
        assert!(
            after == source,
            "pure preparation and export preserve the complete graph"
        );
    }
}

#[test]
fn four_uri_families_keep_legacy_config_parity_inside_a_mixed_store() {
    for (protocol, uri) in [
        ("vless", VLESS),
        (
            "trojan",
            "trojan://synthetic-canary-password@192.0.2.2:443?sni=edge.invalid",
        ),
        (
            "hysteria2",
            "hysteria2://synthetic-canary-auth@192.0.2.3:443?sni=edge.invalid",
        ),
        (
            "tuic",
            "tuic://22222222-2222-4222-8222-222222222222:synthetic-canary-password@192.0.2.4:443?congestion_control=bbr&udp_relay_mode=quic",
        ),
    ] {
        let mut source = mixed(3);
        source["profiles"][0]["protocol"] = protocol.into();
        source["profiles"][0]["uri"] = uri.into();
        let candidate = parse_candidate_private_store(&source.to_string()).unwrap();
        source["version"] = 3.into();
        source["profiles"].as_array_mut().unwrap().remove(1);
        let legacy = parse_private_store(&source.to_string()).unwrap();
        for mode in ["global", "rule", "direct"] {
            let new = candidate
                .prepare_private_config_mode(URI_ID, TEMPLATE, SOCKET, mode)
                .unwrap();
            let old = legacy
                .prepare_config_mode(URI_ID, TEMPLATE, SOCKET, mode)
                .unwrap();
            assert!(
                new.expose_private_bytes() == old.as_bytes(),
                "URI rendering stays unchanged"
            );
        }
    }
}

fn pointers(
    source: &str,
    connected: Option<&str>,
    prune: bool,
) -> omavless_domain::private_store::PrivatePointerMutation {
    let target = connected.map_or(
        CompatibilityPointerTarget::Disconnected {
            prune_missing: prune,
        },
        |id| CompatibilityPointerTarget::Connected {
            profile_id: id.to_owned(),
        },
    );
    apply_candidate_compatibility_pointer_update(source, target).unwrap()
}

#[test]
fn connected_native_pointer_and_disconnect_preserve_complete_graph_and_noop_bytes() {
    for generation in 0..=4 {
        let source = mixed(generation);
        let original = serde_json::to_string_pretty(&source).unwrap();
        let noop = pointers(&original, Some(URI_ID), false);
        assert!(!noop.changed && noop.pruned == 0 && noop.payload() == original.as_bytes());
        let connected = pointers(&original, Some(WG_ID), false);
        assert!(connected.changed && connected.pruned == 0);
        let mut expected = source.clone();
        expected["activeId"] = WG_ID.into();
        expected["lastId"] = WG_ID.into();
        let output: Value = serde_json::from_slice(connected.payload()).unwrap();
        assert!(output == expected);
        let disconnected = pointers(
            std::str::from_utf8(connected.payload()).unwrap(),
            None,
            false,
        );
        expected["activeId"] = "".into();
        assert!(serde_json::from_slice::<Value>(disconnected.payload()).unwrap() == expected);
        let no_more = pointers(
            std::str::from_utf8(disconnected.payload()).unwrap(),
            None,
            false,
        );
        assert!(!no_more.changed && no_more.payload() == disconnected.payload());
        assert!(parse_private_store(std::str::from_utf8(connected.payload()).unwrap()).is_err());
    }
}

#[test]
fn missing_uri_prune_repairs_pointers_without_dropping_native_or_provider_graph() {
    let mut source = mixed(4);
    source["subscriptions"] = json!([{"id":SUB_ID,"name":"Synthetic provider","url":"https://provider.example.invalid/sub","vendorSub":99}]);
    source["profiles"][0]["subscriptionId"] = SUB_ID.into();
    source["profiles"][0]["subscriptionKey"] = "a".repeat(64).into();
    source["profiles"].as_array_mut().unwrap().push(json!({"id":MISSING_ID,"name":"Missing","protocol":"vless","uri":VLESS,"subscriptionId":SUB_ID,"subscriptionKey":"b".repeat(64),"missing":true}));
    source["activeId"] = MISSING_ID.into();
    source["lastId"] = MISSING_ID.into();
    source["startup"]["profileId"] = MISSING_ID.into();
    let unpruned = pointers(&source.to_string(), None, false);
    let kept: Value = serde_json::from_slice(unpruned.payload()).unwrap();
    assert!(kept["profiles"] == source["profiles"] && kept["lastId"] == MISSING_ID);
    let pruned = pointers(&source.to_string(), None, true);
    assert!(pruned.changed && pruned.pruned == 1);
    let output: Value = serde_json::from_slice(pruned.payload()).unwrap();
    let mut expected = source;
    expected["profiles"].as_array_mut().unwrap().remove(2);
    expected["activeId"] = "".into();
    expected["lastId"] = URI_ID.into();
    expected["startup"]["enabled"] = false.into();
    expected["startup"]["profileId"] = "".into();
    assert!(output == expected);
    // When URI is removed, last chooses the native member, not an empty legacy partition.
    expected["profiles"][0]["missing"] = true.into();
    expected["lastId"] = URI_ID.into();
    let native_only = pointers(&expected.to_string(), None, true);
    let native_only: Value = serde_json::from_slice(native_only.payload()).unwrap();
    assert!(native_only["profiles"].as_array().unwrap().len() == 1);
    assert!(
        native_only["lastId"] == WG_ID
            && native_only["profiles"][0]["wireguard"] == expected["profiles"][1]["wireguard"]
    );
}

#[test]
fn uri_only_pointer_transitions_match_legacy_semantics() {
    for connected in [Some(URI_ID), None] {
        for prune in [false, true] {
            let source = base();
            let candidate = pointers(&source.to_string(), connected, prune);
            let mut old = source.clone();
            old["version"] = 3.into();
            let target = connected.map_or(
                CompatibilityPointerTarget::Disconnected {
                    prune_missing: prune,
                },
                |id| CompatibilityPointerTarget::Connected {
                    profile_id: id.to_owned(),
                },
            );
            let legacy = apply_compatibility_pointer_update(&old.to_string(), target).unwrap();
            let legacy_value: Value = serde_json::from_slice(legacy.payload()).unwrap();
            // Legacy adds default row/root fields during encoding. Compare its
            // independent pointer semantics, not that unrelated normalization.
            let mut expected = source;
            for key in ["activeId", "lastId"] {
                expected[key] = legacy_value[key].clone();
            }
            for key in ["enabled", "profileId", "target", "mode"] {
                expected["startup"][key] = legacy_value["startup"][key].clone();
            }
            assert!(serde_json::from_slice::<Value>(candidate.payload()).unwrap() == expected);
            assert!(candidate.changed == legacy.changed && candidate.pruned == legacy.pruned);
        }
    }
}

#[test]
fn empty_and_stale_pointer_repairs_keep_last_startup_policy_explicit() {
    let mut source = mixed(0);
    source["profiles"].as_array_mut().unwrap().remove(0);
    source["activeId"] = WG_ID.into();
    source["lastId"] = "".into();
    source["startup"]["target"] = "last".into();
    source["startup"]["profileId"] = "".into();
    let selected = pointers(&source.to_string(), None, true);
    let selected: Value = serde_json::from_slice(selected.payload()).unwrap();
    assert!(selected["lastId"] == WG_ID && selected["startup"]["enabled"] == true);

    let mut source = base();
    source["profiles"][0]["missing"] = true.into();
    source["startup"]["target"] = "last".into();
    let empty = pointers(&source.to_string(), None, true);
    assert!(empty.changed && empty.pruned == 1);
    let empty: Value = serde_json::from_slice(empty.payload()).unwrap();
    assert!(empty["profiles"].as_array().unwrap().is_empty());
    assert!(empty["activeId"] == "" && empty["lastId"] == "");
    assert!(empty["startup"]["enabled"] == false && empty["startup"]["profileId"] == "");
    assert!(
        empty["startup"]["vendorStartup"] == true && empty["vendorRoot"] == source["vendorRoot"]
    );

    let mut source = mixed(0);
    source["activeId"] = SUB_ID.into();
    source["lastId"] = SUB_ID.into();
    source["startup"]["profileId"] = SUB_ID.into();
    let repaired = pointers(&source.to_string(), None, false);
    assert!(repaired.changed && repaired.pruned == 0);
    let repaired: Value = serde_json::from_slice(repaired.payload()).unwrap();
    assert!(repaired["activeId"] == "" && repaired["lastId"] == "");
    assert!(repaired["startup"]["enabled"] == false && repaired["startup"]["profileId"] == "");
    assert!(repaired["profiles"] == source["profiles"]);
}

#[test]
fn configuration_reuses_rule_admission_and_template_controller_bounds() {
    let source = mixed(0);
    let store = parse_candidate_private_store(&source.to_string()).unwrap();
    for (template, socket) in [
        (
            TEMPLATE.replace("{{OMAVLESS_PROXY}}", "{{OMAVLESS_PROXY}}{{OMAVLESS_PROXY}}"),
            SOCKET,
        ),
        (
            TEMPLATE.replace("mode: rule", "mode: rule\nmode: global"),
            SOCKET,
        ),
        (TEMPLATE.replace("rules:\n", ""), SOCKET),
        (TEMPLATE.to_owned(), ""),
    ] {
        assert!(
            store
                .prepare_private_config_mode(WG_ID, &template, socket, "rule")
                .is_err()
        );
    }
    assert!(
        store
            .prepare_private_config_mode(WG_ID, TEMPLATE, &"x".repeat(4097), "rule")
            .is_err()
    );
    let mut duplicate = source.clone();
    let rule = duplicate["customRules"][0].clone();
    duplicate["customRules"].as_array_mut().unwrap().push(rule);
    assert!(parse_candidate_private_store(&duplicate.to_string()).is_err());
    let mut injected = source;
    injected["customRules"][0]["value"] = "example.invalid\nMATCH,DIRECT".into();
    assert!(parse_candidate_private_store(&injected.to_string()).is_err());
}

#[test]
fn private_config_debug_and_errors_never_release_canary_material() {
    let source = mixed(3);
    let store = parse_candidate_private_store(&source.to_string()).unwrap();
    let config = store
        .prepare_private_config_mode(WG_ID, TEMPLATE, SOCKET, "global")
        .unwrap();
    assert!(
        config
            .expose_private_bytes()
            .windows(STANDARD.encode([7_u8; 32]).len())
            .any(|value| value == STANDARD.encode([7_u8; 32]).as_bytes())
    );
    let debug = format!("{config:?}");
    assert!(debug == "CandidateRuntimeConfig([REDACTED])");
    for result in [
        store.prepare_private_config_mode(WG_ID, TEMPLATE, SOCKET, "synthetic-mode-canary"),
        store.prepare_private_config_mode(WG_ID, "synthetic-template-canary", SOCKET, "rule"),
        store.prepare_private_config_mode(WG_ID, TEMPLATE, "synthetic-socket-canary\n", "rule"),
        store.prepare_private_config_mode("synthetic-id-canary", TEMPLATE, SOCKET, "rule"),
    ] {
        let error = result.unwrap_err();
        let safe = format!("{error:?} {error}");
        for canary in [
            NAME,
            SOCKET,
            "canary",
            "192.0.2.",
            "2001:db8",
            STANDARD.encode([7_u8; 32]).as_str(),
            STANDARD.encode([15_u8; 32]).as_str(),
        ] {
            assert!(!safe.contains(canary));
        }
    }
}

#[test]
fn invalid_or_legacy_sources_unknown_ids_and_oversized_templates_refuse() {
    let source = mixed(0);
    assert!(matches!(
        apply_candidate_compatibility_pointer_update(
            &source.to_string(),
            CompatibilityPointerTarget::Connected {
                profile_id: SUB_ID.to_owned()
            }
        ),
        Err(PrivateStoreError::ProfileNotFound)
    ));
    let store = parse_candidate_private_store(&source.to_string()).unwrap();
    assert!(
        store
            .prepare_private_config_mode(
                WG_ID,
                &"x".repeat(omavless_domain::config::MAX_TEMPLATE_BYTES + 1),
                SOCKET,
                "rule"
            )
            .is_err()
    );
    for version in 1..=3 {
        let mut legacy = base();
        legacy["version"] = version.into();
        assert!(
            apply_candidate_compatibility_pointer_update(
                &legacy.to_string(),
                CompatibilityPointerTarget::Disconnected {
                    prune_missing: true
                }
            )
            .is_err()
        );
        assert!(parse_candidate_private_store(&legacy.to_string()).is_err());
    }
    let mut corrupt = source.clone();
    corrupt["profiles"][0]["uri"] = "trojan://".into();
    assert!(
        apply_candidate_compatibility_pointer_update(
            &corrupt.to_string(),
            CompatibilityPointerTarget::Disconnected {
                prune_missing: true
            }
        )
        .is_err()
    );
    let duplicate = source
        .to_string()
        .replacen("\"version\":4", "\"version\":4,\"version\":4", 1);
    assert!(
        apply_candidate_compatibility_pointer_update(
            &duplicate,
            CompatibilityPointerTarget::Disconnected {
                prune_missing: false
            }
        )
        .is_err()
    );
}
