// SPDX-License-Identifier: MIT
use super::*;
use omavless_profile::canonical::parse_canonical;
use serde_json::Value;
use std::os::unix::fs::symlink;

const URI: &str = "vless://11111111-1111-4111-8111-111111111111@192.0.2.2:443?type=tcp&security=tls&sni=fixture.invalid";
const ID: &str = "22222222-2222-4222-8222-222222222222";

fn rendered(uri: &str) -> Result<Vec<u8>, PreparationError> {
    render(
        parse_canonical(uri).map_err(|_| PreparationError::Refused)?,
        Path::new("/private/runtime/mihomo.sock"),
    )
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

fn fixture() -> (tempfile::TempDir, NativeLifecycleHost, DesiredState) {
    let root = tempfile::tempdir().unwrap();
    let uid = nix::unistd::getuid().as_raw();
    for name in ["config", "data", "runtime", "proc", "sys"] {
        fs::create_dir(root.path().join(name)).unwrap();
        fs::set_permissions(root.path().join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    // Not an executable program: constructor checks mode, never executes it.
    fs::write(root.path().join("core"), b"synthetic-core-identity").unwrap();
    fs::set_permissions(root.path().join("core"), fs::Permissions::from_mode(0o700)).unwrap();
    let paths = NativeHostPaths::new(
        root.path().join("core"),
        root.path().join("data"),
        root.path().join("config"),
        root.path().join("runtime"),
        root.path().join("proc"),
        root.path().join("sys"),
    );
    let store = json!({"version": 3, "profiles": [{"id":ID, "name":"Private display name", "uri":URI, "protocol":"vless"}],
        "subscriptions": [], "activeId":"", "lastId":"", "customRules": []});
    assert!(parse_private_store(&store.to_string()).is_ok());
    write_private(&paths.store, store.to_string().as_bytes());
    write_private(&paths.template, b"arbitrary: template-must-not-be-read\n");
    write_private(&paths.active_config, b"previous-active-bytes\n");
    let host = NativeLifecycleHost::new(paths, uid).unwrap();
    let desired = DesiredState {
        connected: true,
        mode: RoutingMode::Global,
        profile_id: ID.into(),
        generation: 7,
        ..DesiredState::default()
    };
    (root, host, desired)
}

fn stage(host: &mut NativeLifecycleHost, desired: &DesiredState) -> Result<(), PreparationError> {
    // Test-only identity injection. No managed package admission is forged.
    let core = HeldFile::capture(&host.paths.core, host.uid, 0o700, MAX_CORE)?;
    host.prepare_bound_candidate(desired, core)
}

#[test]
fn coverage_issuer_has_no_accepted_digest_or_native_spawn() {
    for digest in [[0; 32], [1; 32], [255; 32]] {
        assert!(matches!(
            issue_coverage(digest, None),
            Err(PreparationError::Unsupported)
        ));
    }
    let (_root, mut host, desired) = fixture();
    stage(&mut host, &desired).unwrap();
    assert!(matches!(
        host.admit_prepared_protection(&desired),
        Err(PreparationError::Unsupported)
    ));
    let preparation = host.protected_preparation.as_ref().unwrap();
    assert!(!preparation.admitted && !preparation.started);
    assert!(preparation.bound.is_some());
    assert!(host.core.is_none());
}

#[test]
fn validation_data_directory_replacement_is_not_an_equivalent_input() {
    let (root, mut host, desired) = fixture();
    stage(&mut host, &desired).unwrap();
    fs::rename(&host.paths.data_directory, root.path().join("old-data")).unwrap();
    fs::create_dir(&host.paths.data_directory).unwrap();
    fs::set_permissions(
        &host.paths.data_directory,
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert_eq!(
        host.recheck_protected_candidate(&desired),
        Err(PreparationError::Changed)
    );
}

#[test]
fn canonical_policy_is_fixed_and_has_no_external_resource_dependencies() {
    let bytes = rendered(URI).unwrap();
    let config: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(config["mode"] == "global");
    assert!(config["routing-mark"] == omavless_netguard::nft::CORE_MARK);
    assert!(config["tun"]["device"] == omavless_netguard::nft::TUN);
    assert!(config["tun"]["auto-redirect"] == false);
    assert!(config["tun"]["disable-system-dns"] == true);
    assert!(config["tun"]["omavless-dns-broker"] == true);
    assert!(config["proxies"][0]["name"] == PROFILE);
    assert!(config["proxies"][0]["udp"] == false);
    assert!(config["proxies"][0]["tls"] == true);
    assert!(config["proxies"][0]["skip-cert-verify"].is_null());
    assert!(config["dns"]["nameserver"] == json!([RESOLVER]));
    assert!(config["dns"]["use-system-hosts"] == false);
    assert!(config["ipv6"] == false);
    assert!(config["proxy-groups"][0]["proxies"] == json!([PROFILE]));
    assert!(config["proxy-groups"][1]["proxies"] == json!(["PROXY"]));
    for key in [
        "rule-providers",
        "proxy-providers",
        "geodata-mode",
        "geox-url",
        "external-controller",
        "secret",
        "sniffer",
    ] {
        assert!(config[key].is_null());
    }
    for key in [
        "port",
        "socks-port",
        "mixed-port",
        "redir-port",
        "tproxy-port",
    ] {
        assert!(config[key] == 0);
    }
    // A JSON scalar controller path cannot inject another config field.
    let profile = parse_canonical(URI).unwrap();
    let quoted = render(profile, Path::new("/private/a\"b.sock")).unwrap();
    let parsed: Value = serde_json::from_slice(&quoted).unwrap();
    assert!(parsed["external-controller-unix"] == "/private/a\"b.sock");
}

#[test]
fn unsupported_transports_and_options_never_enter_a_candidate() {
    for (old, new) in [
        ("type=tcp", "type=ws"),
        ("type=tcp", "type=grpc"),
        ("security=tls", "security=none"),
        ("192.0.2.2", "endpoint.invalid"),
        ("192.0.2.2", "[2001:db8::2]"),
        ("192.0.2.2", "127.0.0.2"),
        ("192.0.2.2", "169.254.1.2"),
        ("192.0.2.2", "224.0.0.1"),
        ("192.0.2.2", "0.0.0.0"),
        ("192.0.2.2", "255.255.255.255"),
    ] {
        assert!(rendered(&URI.replace(old, new)).is_err());
    }
    for suffix in [
        "&allowInsecure=1",
        "&fp=chrome",
        "&alpn=h2",
        "&flow=xtls-rprx-vision",
        "&packetEncoding=xudp",
    ] {
        assert!(rendered(&format!("{URI}{suffix}")).is_err());
    }
    assert!(rendered("trojan://synthetic@192.0.2.2:443?security=tls&sni=fixture.invalid").is_err());
}

#[test]
fn source_preparation_is_not_an_arm_capability_or_ordinary_start() {
    let (_root, mut host, desired) = fixture();
    stage(&mut host, &desired).unwrap();
    assert!(host.recheck_protected_candidate(&desired).is_ok());
    assert!(matches!(
        host.admit_prepared_protection(&desired),
        Err(PreparationError::Unsupported)
    ));
    assert!(host.start_prepared().is_err());
    assert!(host.core.is_none() && host.profile_id.is_none() && host.readiness.is_none());
    assert!(!host.paths.staged_config.exists());
    assert!(fs::read(&host.paths.template).unwrap() == b"arbitrary: template-must-not-be-read\n");
    assert!(fs::read(&host.paths.active_config).unwrap() == b"previous-active-bytes\n");
    assert!(stage(&mut host, &desired).is_err());
    let path = host.paths.config_directory.join(STAGING);
    assert!(fs::metadata(path).unwrap().mode() & 0o7777 == 0o600);
}

#[test]
fn normal_native_path_cannot_claim_missing_managed_package_is_supported() {
    let (_root, mut host, desired) = fixture();
    assert_eq!(
        host.prepare_protected_candidate(&desired),
        Err(PreparationError::Unsupported)
    );
    assert!(host.protected_preparation.is_none());
    assert!(!host.paths.config_directory.join(STAGING).exists());
    assert_eq!(
        host.protected_preflight(&desired),
        Err(HostStepError::Prepare)
    );
}

#[test]
fn every_desired_field_and_store_bytes_are_bound() {
    let (_root, mut host, desired) = fixture();
    stage(&mut host, &desired).unwrap();
    for changed in [
        DesiredState {
            generation: 8,
            ..desired.clone()
        },
        DesiredState {
            profile_id: "p2".into(),
            ..desired.clone()
        },
        DesiredState {
            connected: false,
            ..desired.clone()
        },
        DesiredState {
            mode: RoutingMode::Rule,
            ..desired.clone()
        },
        DesiredState {
            schema_version: 2,
            ..desired.clone()
        },
    ] {
        assert_eq!(
            host.recheck_protected_candidate(&changed),
            Err(PreparationError::Changed)
        );
    }
    let mut bytes = fs::read(&host.paths.store).unwrap();
    bytes.push(b'\n');
    write_private(&host.paths.store, &bytes);
    assert_eq!(
        host.recheck_protected_candidate(&desired),
        Err(PreparationError::Changed)
    );
}

#[test]
fn held_config_refuses_mutation_replacement_symlink_and_permission_drift() {
    for mutation in 0..4 {
        let (_root, mut host, desired) = fixture();
        stage(&mut host, &desired).unwrap();
        let path = host.paths.config_directory.join(STAGING);
        match mutation {
            0 => {
                fs::write(&path, b"{}").unwrap();
            }
            1 => {
                let bytes = fs::read(&path).unwrap();
                fs::remove_file(&path).unwrap();
                write_private(&path, &bytes);
            }
            2 => {
                fs::remove_file(&path).unwrap();
                symlink(&host.paths.active_config, &path).unwrap();
            }
            _ => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            }
        }
        assert_eq!(
            host.recheck_protected_candidate(&desired),
            Err(PreparationError::Changed)
        );
    }
}

#[test]
fn held_core_refuses_same_bytes_replacement_and_changed_bytes() {
    for replace in [false, true] {
        let (_root, mut host, desired) = fixture();
        stage(&mut host, &desired).unwrap();
        if replace {
            fs::remove_file(&host.paths.core).unwrap();
            fs::write(&host.paths.core, b"synthetic-core-identity").unwrap();
            fs::set_permissions(&host.paths.core, fs::Permissions::from_mode(0o700)).unwrap();
        } else {
            fs::write(&host.paths.core, b"different-core-content").unwrap();
        }
        assert_eq!(
            host.recheck_protected_candidate(&desired),
            Err(PreparationError::Changed)
        );
    }
}

#[test]
fn collision_or_publication_failure_never_overwrites_or_admits_retry() {
    let (_root, mut host, desired) = fixture();
    let path = host.paths.config_directory.join(STAGING);
    write_private(&path, b"existing-private-candidate");
    assert_eq!(
        stage(&mut host, &desired),
        Err(PreparationError::OutcomeUnknown)
    );
    assert!(host.protected_preparation.as_ref().unwrap().bound.is_none());
    assert!(fs::read(&path).unwrap() == b"existing-private-candidate");
    assert_eq!(stage(&mut host, &desired), Err(PreparationError::Refused));
    assert!(host.admit_prepared_protection(&desired).is_err());
}

#[test]
fn reserved_tun_collision_and_non_full_intent_refuse_before_file_publication() {
    let (_root, mut host, desired) = fixture();
    let wrong = DesiredState {
        mode: RoutingMode::Rule,
        ..desired.clone()
    };
    assert_eq!(stage(&mut host, &wrong), Err(PreparationError::Refused));
    fs::create_dir(host.paths.sys_class_net.join(omavless_netguard::nft::TUN)).unwrap();
    assert_eq!(stage(&mut host, &desired), Err(PreparationError::Refused));
    assert!(!host.paths.config_directory.join(STAGING).exists());
    assert!(host.protected_preparation.is_none());
}

#[test]
fn missing_profile_never_falls_back_to_another_record() {
    let (_root, mut host, desired) = fixture();
    let wrong = DesiredState {
        profile_id: "missing".into(),
        ..desired
    };
    assert_eq!(stage(&mut host, &wrong), Err(PreparationError::Refused));
    assert!(host.protected_preparation.is_none());
}
