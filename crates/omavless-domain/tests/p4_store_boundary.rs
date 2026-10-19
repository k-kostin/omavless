// SPDX-License-Identifier: MIT

//! Negative production-domain gates while P4 storage integration is inactive.
//! All credentials below are generated public test values, never live fixtures.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use omavless_domain::private_store::{
    ProfileMutation, apply_profile_import, apply_profile_mutation, parse_private_store,
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

#[test]
fn valid_p4_credentials_do_not_enter_production_import_or_replacement() {
    let source = legacy_store().to_string();
    for amnezia in [false, true] {
        let config = native_config(amnezia);
        let parsed = parse_wireguard_config(&config).expect("synthetic native fixture");
        let record = parsed.private_record().expect("private codec");
        assert!(parse_private_wireguard_record(record.expose_private_bytes()).is_ok());
        let envelope = std::str::from_utf8(record.expose_private_bytes()).unwrap();
        for input in [config.as_str(), envelope] {
            assert!(apply_profile_import(&source, NEW_ID, "New", input).is_err());
            assert!(
                apply_profile_mutation(
                    &source,
                    ProfileMutation::Replace {
                        profile_id: ID.to_owned(),
                        new_name: "New".to_owned(),
                        new_input: input.to_owned(),
                    }
                )
                .is_err()
            );
        }
    }
    let existing = parse_private_store(&source).unwrap().projection();
    assert_eq!(existing.profile_count, 1);
    assert!(existing.active_present && existing.last_present);
}

#[test]
fn mixed_store_rejects_structured_rows_with_p4_discriminators() {
    for amnezia in [false, true] {
        let parsed = parse_wireguard_config(&native_config(amnezia)).unwrap();
        let record = parsed.private_record().unwrap();
        let private: Value = serde_json::from_slice(record.expose_private_bytes()).unwrap();
        for uri in [None, Some(URI)] {
            let mut source = legacy_store();
            let mut entry = json!({
                "id": NEW_ID, "name": "New",
                "protocol": parsed.facts().flavor.protocol_name(), "wireguard": private
            });
            if let Some(uri) = uri {
                entry["uri"] = uri.into();
            }
            source["profiles"].as_array_mut().unwrap().push(entry);
            assert!(parse_private_store(&source.to_string()).is_err());
        }
    }
}

#[test]
fn future_store_version_cannot_activate_through_current_domain_operations() {
    let mut source = legacy_store();
    source["version"] = 4.into();
    let input = source.to_string();
    assert!(parse_private_store(&input).is_err());
    assert!(apply_profile_import(&input, NEW_ID, "New", URI).is_err());
    assert!(
        apply_profile_mutation(
            &input,
            ProfileMutation::Favorite {
                profile_id: ID.to_owned(),
                enabled: true
            }
        )
        .is_err()
    );
}
