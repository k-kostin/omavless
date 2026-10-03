use omavless_netguard::{nft::*, policy::Policy, transaction::Table};
use serde_json::{Value, json};

const BOOT: [u8; 16] = [7; 16];
fn receipt() -> TrustedTableIdentity {
    TrustedTableIdentity {
        boot: BOOT,
        netns_inode: 42,
        table_handle: 17,
    }
}
fn readback(policy: Policy) -> Value {
    let commands: Value = serde_json::from_slice(&render_create(policy)).unwrap();
    let mut objects = vec![
        json!({"metainfo":{"version":"1.1.6","release_name":"fixture","json_schema_version":1}}),
    ];
    for (index, command) in commands["nftables"].as_array().unwrap().iter().enumerate() {
        let mut object = command[if index == 0 { "create" } else { "add" }].clone();
        let inner = object
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .as_object_mut()
            .unwrap();
        inner.insert(
            "handle".into(),
            json!(if index == 0 { 17 } else { index as u64 }),
        );
        objects.push(object);
    }
    json!({"nftables":objects})
}
fn classify(value: &Value) -> Table {
    classify_readback(
        &serde_json::to_vec(value).unwrap(),
        BOOT,
        42,
        Some(receipt()),
    )
}

#[test]
fn emergency_golden_has_no_non_loopback_bypass() {
    let expected = json!({"nftables":[
        {"create":{"table":{"family":"inet","name":"omavless_netguard"}}},
        {"add":{"chain":{"family":"inet","table":"omavless_netguard","name":"output_guard","type":"filter","hook":"output","prio":300,"policy":"drop"}}},
        {"add":{"rule":{"family":"inet","table":"omavless_netguard","chain":"output_guard","expr":[{"match":{"op":"==","left":{"meta":{"key":"oifname"}},"right":"lo"}},{"accept":null}]}}},
        {"add":{"rule":{"family":"inet","table":"omavless_netguard","chain":"output_guard","expr":[{"drop":null}]}}}
    ]});
    let actual: Value = serde_json::from_slice(&render_create(Policy::Emergency)).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn full_template_has_only_fixed_guarded_exceptions() {
    let raw = render_create(Policy::FullVpn);
    let value: Value = serde_json::from_slice(&raw).unwrap();
    let list = value["nftables"].as_array().unwrap();
    assert_eq!(list.len(), 12);
    assert_eq!(
        list[3]["add"]["rule"]["expr"][0]["match"]["right"],
        "omavless0"
    );
    assert_eq!(
        list[4]["add"]["rule"]["expr"][0]["match"]["right"],
        0x4f4d4101u32
    );
    assert_eq!(
        list[5]["add"]["rule"]["expr"][1]["match"]["right"],
        "255.255.255.255"
    );
    assert_eq!(
        list[6]["add"]["rule"]["expr"][2]["match"]["right"],
        "ff02::1:2"
    );
    let text = std::str::from_utf8(&raw).unwrap();
    for forbidden in [
        "established",
        "skuid",
        "flush",
        "delete",
        "192.168",
        "10.0.",
        "53",
        "endpoint",
    ] {
        assert!(
            !text.contains(forbidden),
            "unexpected policy term: {forbidden}"
        );
    }
    for command in list.iter().take(11).skip(7) {
        let predicates = command["add"]["rule"]["expr"].as_array().unwrap();
        assert!(predicates.iter().any(|p| p["match"]["left"]
            == json!({"payload":{"protocol":"ip6","field":"hoplimit"}})
            && p["match"]["right"] == 255));
        assert!(predicates.iter().any(|p| p["match"]["left"]
            == json!({"payload":{"protocol":"icmpv6","field":"code"}})
            && p["match"]["right"] == 0));
    }
}

#[test]
fn matching_policy_requires_independent_same_kernel_identity() {
    for policy in [Policy::FullVpn, Policy::Emergency] {
        let value = readback(policy);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(classify(&value), Table::OwnedVerified(policy));
        assert_eq!(classify_readback(&bytes, BOOT, 42, None), Table::Foreign);
        for identity in [
            TrustedTableIdentity {
                boot: [8; 16],
                ..receipt()
            },
            TrustedTableIdentity {
                netns_inode: 43,
                ..receipt()
            },
            TrustedTableIdentity {
                table_handle: 18,
                ..receipt()
            },
        ] {
            assert_eq!(
                classify_readback(&bytes, BOOT, 42, Some(identity)),
                Table::Foreign
            );
        }
    }
}

#[test]
fn changed_rules_and_dormant_tables_are_never_verified() {
    let original = readback(Policy::FullVpn);
    for (pointer, replacement) in [
        ("/nftables/1/table/flags", json!(["dormant"])),
        ("/nftables/2/chain/policy", json!("accept")),
        ("/nftables/2/chain/prio", json!(0)),
        ("/nftables/2/chain/hook", json!("input")),
        ("/nftables/5/rule/expr/0/match/right", json!(0)),
        ("/nftables/12/rule/expr", json!([{"accept":null}])),
    ] {
        let mut changed = original.clone();
        if pointer.ends_with("/flags") {
            changed["nftables"][1]["table"]["flags"] = replacement;
        } else {
            *changed.pointer_mut(pointer).unwrap() = replacement;
        }
        assert_eq!(classify(&changed), Table::OwnedUnrecognized);
    }
    let mut changed = original.clone();
    changed["nftables"].as_array_mut().unwrap().remove(12);
    assert_eq!(classify(&changed), Table::OwnedUnrecognized);
    changed["nftables"][1]["table"]["name"] = json!("foreign");
    assert_eq!(classify(&changed), Table::Foreign);
}

#[test]
fn malformed_duplicate_incomplete_or_empty_readback_is_unreadable() {
    for bytes in [
        b"{\"nftables\":[]}".as_slice(),
        b"{\"nftables\":[],\"nftables\":[]}",
        b"{\"nftables\":[",
        b"[]",
        b"\xff",
    ] {
        assert_eq!(
            classify_readback(bytes, BOOT, 42, Some(receipt())),
            Table::Unreadable
        );
    }
    let mut value = readback(Policy::FullVpn);
    value["nftables"][2]["chain"]["handle"] = json!(0);
    assert_eq!(classify(&value), Table::Unreadable);
    assert_eq!(
        classify_readback(
            &vec![b' '; MAX_READBACK_BYTES + 1],
            BOOT,
            42,
            Some(receipt())
        ),
        Table::Unreadable
    );
    let mut value = readback(Policy::Emergency);
    value["nftables"][3]["rule"]["handle"] = json!(1);
    assert_eq!(classify(&value), Table::Unreadable);
}

#[test]
fn no_subset_or_injected_object_can_masquerade_as_full_readback() {
    let mut value = readback(Policy::Emergency);
    value["nftables"]
        .as_array_mut()
        .unwrap()
        .push(json!({"chain":{"family":"inet","table":"foreign","name":"other","handle":55}}));
    assert_eq!(classify(&value), Table::OwnedUnrecognized);
    let raw = serde_json::to_string(&readback(Policy::Emergency)).unwrap();
    let duplicate = raw.replacen(
        "\"policy\":\"drop\"",
        "\"policy\":\"accept\",\"policy\":\"drop\"",
        1,
    );
    assert_eq!(
        classify_readback(duplicate.as_bytes(), BOOT, 42, Some(receipt())),
        Table::Unreadable
    );
}

#[test]
fn fixed_kernel_family_elision_preserves_all_other_constraints() {
    let mut value = readback(Policy::FullVpn);
    // Metainfo shifts object indexes by one. The six known maintenance rules
    // retain their protocol-specific IP payloads, only nfproto is redundant.
    for entry in &mut value["nftables"].as_array_mut().unwrap()[6..12] {
        entry["rule"]["expr"].as_array_mut().unwrap().remove(0);
    }
    assert_eq!(classify(&value), Table::OwnedVerified(Policy::FullVpn));
    let encoded = serde_json::to_vec(&value).unwrap();
    assert_eq!(classify_readback(&encoded, BOOT, 42, None), Table::Foreign);

    for (pointer, replacement) in [
        ("/nftables/6/rule/expr/0/match/right", json!("0.0.0.0")),
        (
            "/nftables/6/rule/expr/0/match/left/payload/protocol",
            json!("ip6"),
        ),
        ("/nftables/6/rule/expr/1/match/right", json!(53)),
        ("/nftables/7/rule/expr/0/match/right/prefix/len", json!(0)),
        ("/nftables/8/rule/expr/2/match/right", json!(64)),
        ("/nftables/9/rule/expr/3/match/right", json!(128)),
        ("/nftables/10/rule/expr/3/match/right", json!(1)),
        ("/nftables/11/rule/expr/1/match/right/prefix/len", json!(0)),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(classify(&changed), Table::OwnedUnrecognized, "{pointer}");
    }
    for index in 6..12 {
        let mut missing = value.clone();
        missing["nftables"][index]["rule"]["expr"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        assert_eq!(classify(&missing), Table::OwnedUnrecognized);
        let mut added = value.clone();
        added["nftables"][index]["rule"]["expr"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({"accept":null}));
        assert_eq!(classify(&added), Table::OwnedUnrecognized);
    }
    // Unmeasured partial elision is not generalized into a predicate stripper.
    let mut mixed = readback(Policy::FullVpn);
    mixed["nftables"][6]["rule"]["expr"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    assert_eq!(classify(&mixed), Table::OwnedUnrecognized);
}

#[test]
fn captured_nft_1_1_7_synthetic_readback_requires_independent_receipt() {
    // Captured in a new loopback-only child netns in the development VM, not
    // from a host ruleset. All addresses and rules are compiled test literals.
    let bytes = include_bytes!("fixtures/nft-1.1.7-full.json");
    assert_eq!(
        classify_untrusted_shape(bytes),
        UntrustedPolicyShape::Exact(Policy::FullVpn)
    );
    let identity = TrustedTableIdentity {
        table_handle: 4,
        ..receipt()
    };
    assert_eq!(
        classify_readback(bytes, BOOT, 42, Some(identity)),
        Table::OwnedVerified(Policy::FullVpn)
    );
    assert_eq!(classify_readback(bytes, BOOT, 42, None), Table::Foreign);
    assert_eq!(
        classify_readback(bytes, BOOT, 42, Some(receipt())),
        Table::Foreign
    );
    let mut changed: Value = serde_json::from_slice(bytes).unwrap();
    changed["nftables"][2]["chain"]["policy"] = json!("accept");
    assert_eq!(
        classify_untrusted_shape(&serde_json::to_vec(&changed).unwrap()),
        UntrustedPolicyShape::OtherUntrusted
    );
    assert_eq!(
        classify_readback(
            &serde_json::to_vec(&changed).unwrap(),
            BOOT,
            42,
            Some(identity)
        ),
        Table::OwnedUnrecognized
    );
}

#[test]
fn complete_untrusted_inventory_rejects_extra_objects_and_partial_output() {
    let expected = readback(Policy::Emergency);
    let bytes = serde_json::to_vec(&expected).unwrap();
    assert_eq!(
        classify_untrusted_shape(&bytes),
        UntrustedPolicyShape::Exact(Policy::Emergency)
    );
    for object in [
        json!({"set":{"family":"inet","table":"omavless_netguard","name":"extra","handle":51}}),
        json!({"chain":{"family":"inet","table":"omavless_netguard","name":"extra","handle":52}}),
        json!({"rule":{"family":"inet","table":"omavless_netguard","chain":"output_guard","expr":[{"accept":null}],"handle":53}}),
    ] {
        let mut changed = expected.clone();
        changed["nftables"].as_array_mut().unwrap().push(object);
        assert_eq!(
            classify_untrusted_shape(&serde_json::to_vec(&changed).unwrap()),
            UntrustedPolicyShape::OtherUntrusted
        );
    }
    assert_eq!(
        classify_untrusted_shape(b"{\"nftables\":[]}"),
        UntrustedPolicyShape::Unreadable
    );
    assert_eq!(
        classify_untrusted_shape(&bytes[..bytes.len() - 1]),
        UntrustedPolicyShape::Unreadable
    );
    let mut wrong = expected;
    wrong["nftables"][1]["table"]["name"] = json!("unrelated");
    assert_eq!(
        classify_untrusted_shape(&serde_json::to_vec(&wrong).unwrap()),
        UntrustedPolicyShape::ForeignTable
    );
}
