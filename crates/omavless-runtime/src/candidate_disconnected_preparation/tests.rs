// SPDX-License-Identifier: MIT
//! Synthetic policy/rendering controls only. No artifact, file, core or host.
use super::*;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use omavless_domain::private_store::{parse_candidate_private_store, parse_private_store};
use omavless_profile::wireguard::parse_wireguard_config;
use serde_json::{Value, json};

const URI_ID: &str = "00000000-0000-0000-0000-000000000001";
const WG_ID: &str = "00000000-0000-0000-0000-000000000002";
const MISSING: &str = "00000000-0000-0000-0000-000000000003";
const VLESS: &str =
    "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp";
const TEMPLATE: &str = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n";
const CONTROLLER: &str = "/private/synthetic/mihomo.sock";
const LABEL: &str = "SYNTHETIC_PRIVATE_LABEL_CANARY";

fn native(generation: usize) -> String {
    let mut options = String::new();
    if generation > 0 {
        options.push_str("Jc=4\nJmin=10\nJmax=30\nS1=20\nS2=25\nH1=100\nH2=200\nH3=300\nH4=400\n");
    }
    if generation >= 2 {
        options.push_str("S3=30\nS4=35\n");
    }
    if generation >= 3 {
        options.push_str(&format!("I1=<r 2><b 0x0102>\nHeaderProtectionKey={}\nContentPaddingAddition=10-100\nRekeyAfterTime=100-120\nRekeyTimeout=3-7\nRejectAfterTime=150-180\nKeepaliveTimeout=5-15\nMaxHandshakeAttempts=15-20\n", STANDARD.encode([15u8; 32])));
    }
    if generation == 4 {
        options.push_str("RandomTrailers=true\nDisableCookies=false\n");
    }
    format!(
        "[Interface]\nPrivateKey={}\nAddress=10.203.0.2/32\n{options}[Peer]\nPublicKey={}\nAllowedIPs=0.0.0.0/0\nEndpoint=192.0.2.2:51820\n",
        STANDARD.encode([7u8; 32]),
        STANDARD.encode([9u8; 32])
    )
}

fn source(generation: usize) -> Value {
    let profile = parse_wireguard_config(&native(generation)).unwrap();
    let record = profile.private_record().unwrap();
    let record: Value = serde_json::from_slice(record.expose_private_bytes()).unwrap();
    json!({"version":4,"profiles":[
        {"id":URI_ID,"name":"URI","protocol":"vless","uri":VLESS},
        {"id":WG_ID,"name":LABEL,"protocol":profile.facts().flavor.protocol_name(),"wireguard":record}],
        "subscriptions":[],"activeId":"","lastId":URI_ID,"startup":{"enabled":false}})
}

fn synthetic_elf() -> Vec<u8> {
    let mut bytes = vec![0u8; 160];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
    bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
    bytes[72..80].copy_from_slice(&128u64.to_le_bytes());
    bytes[96..104].copy_from_slice(&32u64.to_le_bytes());
    bytes
}

#[test]
fn credential_flavor_is_derived_from_validated_source_not_a_caller_claim() {
    for (generation, expected) in [
        (0, WireGuardFlavor::Standard),
        (1, WireGuardFlavor::Amnezia(AwgGeneration::V1)),
        (2, WireGuardFlavor::Amnezia(AwgGeneration::V2)),
        (3, WireGuardFlavor::Amnezia(AwgGeneration::V3)),
        (4, WireGuardFlavor::Amnezia(AwgGeneration::V3_1)),
    ] {
        let document = source(generation);
        let candidate = parse_candidate_private_store(&document.to_string()).unwrap();
        assert_eq!(
            candidate.selected_wireguard_flavor(WG_ID).unwrap(),
            Some(expected)
        );
        assert_eq!(candidate.selected_wireguard_flavor(URI_ID).unwrap(), None);
        assert!(candidate.selected_wireguard_flavor(MISSING).is_err());
        let mut contradictory = document;
        contradictory["profiles"][1]["protocol"] = "vless".into();
        assert!(parse_candidate_private_store(&contradictory.to_string()).is_err());
    }
}

#[test]
fn exact_catalog_policy_refuses_unknown_artifacts_and_unreviewed_flavors() {
    // Digest arguments test the private policy table, not authentic artifact
    // acquisition or a public constructor accepting caller-supplied digests.
    for flavor in [
        None,
        Some(WireGuardFlavor::Standard),
        Some(WireGuardFlavor::Amnezia(AwgGeneration::V3)),
        Some(WireGuardFlavor::Amnezia(AwgGeneration::V3_1)),
    ] {
        assert_eq!(compatible_flavor(REVIEWED_CORE_SHA256, flavor), Ok(()));
        assert_eq!(
            compatible_flavor([0u8; 32], flavor),
            Err(PreparationError::UnsupportedCore)
        );
    }
    for generation in [AwgGeneration::V1, AwgGeneration::V2] {
        assert_eq!(
            compatible_flavor(
                REVIEWED_CORE_SHA256,
                Some(WireGuardFlavor::Amnezia(generation))
            ),
            Err(PreparationError::UnsupportedFlavor)
        );
    }
}

#[test]
fn malformed_off_active_intent_and_active_candidate_refuse_before_core() {
    let candidate = parse_candidate_private_store(&source(3).to_string()).unwrap();
    for desired in [
        DesiredState {
            connected: true,
            profile_id: WG_ID.into(),
            ..DesiredState::default()
        },
        DesiredState {
            profile_id: WG_ID.into(),
            ..DesiredState::default()
        },
        DesiredState {
            schema_version: 0,
            ..DesiredState::default()
        },
        DesiredState {
            generation: u64::MAX,
            ..DesiredState::default()
        },
    ] {
        let error =
            prepare_disconnected_candidate(&candidate, &desired, WG_ID, TEMPLATE, CONTROLLER, &[])
                .err()
                .unwrap();
        assert_eq!(error, PreparationError::InvalidOffIntent);
    }
    let mut active = source(3);
    active["activeId"] = URI_ID.into();
    let active = parse_candidate_private_store(&active.to_string()).unwrap();
    assert_eq!(
        prepare_disconnected_candidate(
            &active,
            &DesiredState::default(),
            WG_ID,
            TEMPLATE,
            CONTROLLER,
            &[]
        )
        .err()
        .unwrap(),
        PreparationError::ActiveCandidate
    );
}

#[test]
fn a_label_or_synthetic_static_elf_cannot_authenticate_a_core() {
    let candidate = parse_candidate_private_store(&source(3).to_string()).unwrap();
    let mut label = synthetic_elf();
    label.extend_from_slice(b"Mihomo Meta 1.19.31 with_gvisor reviewed ba7a74ed");
    for core in [
        vec![],
        b"Mihomo Meta 1.19.31".to_vec(),
        synthetic_elf(),
        label,
    ] {
        assert_eq!(
            prepare_disconnected_candidate(
                &candidate,
                &DesiredState::default(),
                WG_ID,
                TEMPLATE,
                CONTROLLER,
                &core
            )
            .err()
            .unwrap(),
            PreparationError::UnsupportedCore
        );
    }
}

#[test]
fn static_elf_shape_is_bounded_and_has_no_interpreter_or_dynamic_dependencies() {
    let good = synthetic_elf();
    assert!(static_x86_elf(&good));
    for segment in [2u32, 3u32] {
        let mut bad = good.clone();
        bad[64..68].copy_from_slice(&segment.to_le_bytes());
        assert!(!static_x86_elf(&bad));
    }
    let mut bad = good.clone();
    bad[96..104].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(!static_x86_elf(&bad));
    let mut bad = good.clone();
    bad[18..20].copy_from_slice(&183u16.to_le_bytes());
    assert!(!static_x86_elf(&bad));
    let mut bad = good.clone();
    bad[20..24].copy_from_slice(&0u32.to_le_bytes());
    assert!(!static_x86_elf(&bad));
    assert!(!static_x86_elf(&good[..100]));
}

#[test]
fn private_model_rendering_keeps_v3_uri_parity_without_qualifying_test_core() {
    for (protocol, uri) in [
        ("vless", VLESS),
        (
            "trojan",
            "trojan://synthetic-password@192.0.2.2:443?sni=edge.invalid",
        ),
        (
            "hysteria2",
            "hysteria2://synthetic-auth@192.0.2.3:443?sni=edge.invalid",
        ),
        (
            "tuic",
            "tuic://22222222-2222-4222-8222-222222222222:synthetic-password@192.0.2.4:443?congestion_control=bbr&udp_relay_mode=quic",
        ),
    ] {
        let mut document = source(3);
        document["profiles"][0]["protocol"] = protocol.into();
        document["profiles"][0]["uri"] = uri.into();
        let candidate = parse_candidate_private_store(&document.to_string()).unwrap();
        document["version"] = 3.into();
        document["profiles"].as_array_mut().unwrap().remove(1);
        let legacy = parse_private_store(&document.to_string()).unwrap();
        for mode in [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct] {
            let desired = DesiredState {
                mode,
                ..DesiredState::default()
            };
            let prepared =
                render_snapshot(&candidate, &desired, URI_ID, TEMPLATE, CONTROLLER).unwrap();
            let old = legacy
                .prepare_config_mode(URI_ID, TEMPLATE, CONTROLLER, mode.as_str())
                .unwrap();
            assert!(prepared.expose_private_bytes() == old.as_bytes());
        }
    }
}

#[test]
fn carrier_debug_and_errors_hide_private_labels_credentials_and_core_bytes() {
    let candidate = parse_candidate_private_store(&source(3).to_string()).unwrap();
    let desired = DesiredState::default();
    let config = render_snapshot(&candidate, &desired, WG_ID, TEMPLATE, CONTROLLER).unwrap();
    // Direct private test construction demonstrates redaction only, NOT a
    // successfully authenticated public preparation result or execution grant.
    let model = CandidateDisconnectedSnapshot {
        config,
        core: synthetic_elf(),
        intent_generation: 0,
        mode: RoutingMode::Rule,
    };
    assert_eq!(
        format!("{model:?}"),
        "CandidateDisconnectedSnapshot([REDACTED])"
    );
    assert!(model.private_readiness_name() == LABEL);
    assert_eq!(model.intent_generation(), 0);
    assert_eq!(model.mode(), RoutingMode::Rule);
    for error in [
        PreparationError::InvalidOffIntent,
        PreparationError::ActiveCandidate,
        PreparationError::ProfileUnavailable,
        PreparationError::UnsupportedCore,
        PreparationError::UnsupportedFlavor,
        PreparationError::InvalidConfiguration,
    ] {
        assert!(!format!("{error:?} {error}").contains(LABEL));
        assert!(!format!("{error:?} {error}").contains(CONTROLLER));
    }
    assert_eq!(
        prepare_disconnected_candidate(
            &candidate,
            &desired,
            WG_ID,
            TEMPLATE,
            CONTROLLER,
            model.expose_captured_core_bytes()
        )
        .err()
        .unwrap(),
        PreparationError::UnsupportedCore
    );
}

#[test]
fn existing_strict_domain_parser_keeps_v3_and_duplicate_sources_out() {
    let mut legacy = source(0);
    legacy["version"] = 3.into();
    assert!(parse_candidate_private_store(&legacy.to_string()).is_err());
    let raw = source(3).to_string();
    let duplicate = raw.replacen("\"version\":4", "\"version\":4,\"version\":4", 1);
    assert!(parse_candidate_private_store(&duplicate).is_err());
    assert!(parse_private_store(&raw).is_err());
}

/// Explicit DATA-only opt-in: the selected file is a public, non-executable
/// cache projection, never an installed/private core or a process target.
#[test]
#[ignore = "requires separately admitted exact public core DATA; never executes it"]
fn authentic_pinned_core_constructor_data_only_optin() {
    use std::fs::{Metadata, OpenOptions};
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let path = std::env::var_os("OMAVLESS_TEST_CANDIDATE_CORE_DATA")
        .expect("explicitly selected public core DATA is required");
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&path)
        .expect("selected DATA open must succeed");
    let stable = |metadata: &Metadata| {
        (
            metadata.dev(),
            metadata.ino(),
            metadata.mode(),
            metadata.uid(),
            metadata.gid(),
            metadata.nlink(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    };
    let before = file.metadata().expect("selected DATA metadata is required");
    assert!(before.is_file());
    assert_eq!(before.uid(), nix::unistd::Uid::current().as_raw());
    assert_eq!(before.mode() & 0o7777, 0o400);
    assert_eq!(before.nlink(), 1);
    assert_eq!(before.len(), 61_886_168);
    assert!(before.len() <= MAX_CORE_BYTES as u64);
    assert!(
        stable(&std::fs::symlink_metadata(&path).expect("selected DATA path is required"))
            == stable(&before)
    );
    let mut core = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_CORE_BYTES as u64 + 1)
        .read_to_end(&mut core)
        .expect("selected bounded DATA read must succeed");
    assert_eq!(core.len() as u64, before.len());
    assert!(
        stable(
            &file
                .metadata()
                .expect("selected DATA final metadata is required")
        ) == stable(&before)
    );
    assert!(
        stable(&std::fs::symlink_metadata(&path).expect("selected DATA final path is required"))
            == stable(&before)
    );
    assert!(<[u8; 32]>::from(Sha256::digest(&core)) == REVIEWED_CORE_SHA256);

    let modes = [RoutingMode::Rule, RoutingMode::Global, RoutingMode::Direct];
    let mut accepted = 0;
    let mut refused = 0;
    let mut config_hashes = Vec::new();
    for generation in 0..=4 {
        let candidate = parse_candidate_private_store(&source(generation).to_string()).unwrap();
        for mode in modes {
            let desired = DesiredState {
                mode,
                ..DesiredState::default()
            };
            let result = prepare_disconnected_candidate(
                &candidate, &desired, WG_ID, TEMPLATE, CONTROLLER, &core,
            );
            if generation == 1 || generation == 2 {
                assert_eq!(result.err().unwrap(), PreparationError::UnsupportedFlavor);
                refused += 1;
            } else {
                let prepared = result.unwrap();
                assert!(prepared.expose_captured_core_bytes() == core);
                assert_eq!(prepared.mode(), mode);
                let expected = candidate
                    .prepare_private_config_mode(WG_ID, TEMPLATE, CONTROLLER, mode.as_str())
                    .unwrap();
                assert!(prepared.expose_private_config() == expected.expose_private_bytes());
                config_hashes.push(format!(
                    "{:x}",
                    Sha256::digest(prepared.expose_private_config())
                ));
                accepted += 1;
            }
        }
    }
    for (protocol, uri) in [
        ("vless", VLESS),
        (
            "trojan",
            "trojan://synthetic-password@192.0.2.2:443?sni=edge.invalid",
        ),
        (
            "hysteria2",
            "hysteria2://synthetic-auth@192.0.2.3:443?sni=edge.invalid",
        ),
        (
            "tuic",
            "tuic://22222222-2222-4222-8222-222222222222:synthetic-password@192.0.2.4:443?congestion_control=bbr&udp_relay_mode=quic",
        ),
    ] {
        let mut document = source(3);
        document["profiles"][0]["protocol"] = protocol.into();
        document["profiles"][0]["uri"] = uri.into();
        let candidate = parse_candidate_private_store(&document.to_string()).unwrap();
        for mode in modes {
            let desired = DesiredState {
                mode,
                ..DesiredState::default()
            };
            let prepared = prepare_disconnected_candidate(
                &candidate, &desired, URI_ID, TEMPLATE, CONTROLLER, &core,
            )
            .unwrap();
            let expected = candidate
                .prepare_private_config_mode(URI_ID, TEMPLATE, CONTROLLER, mode.as_str())
                .unwrap();
            assert!(prepared.expose_private_config() == expected.expose_private_bytes());
            config_hashes.push(format!(
                "{:x}",
                Sha256::digest(prepared.expose_private_config())
            ));
            accepted += 1;
        }
    }
    let candidate = parse_candidate_private_store(&source(3).to_string()).unwrap();
    core[128] ^= 1;
    assert_eq!(
        prepare_disconnected_candidate(
            &candidate,
            &DesiredState::default(),
            WG_ID,
            TEMPLATE,
            CONTROLLER,
            &core
        )
        .err()
        .unwrap(),
        PreparationError::UnsupportedCore
    );
    refused += 1;
    assert_eq!((accepted, refused), (21, 7));
    // Public projection only: synthetic config bytes/names/keys never print.
    eprintln!(
        "candidate constructor DATA: accepted={accepted} refused={refused} config_hashes={config_hashes:?}"
    );
}
