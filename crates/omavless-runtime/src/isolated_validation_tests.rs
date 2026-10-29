// SPDX-License-Identifier: MIT
use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

const BUNDLES: [&str; 3] = [
    include_str!("../../../templates/default.yaml"),
    include_str!("../../../templates/china.yaml"),
    include_str!("../../../templates/iran.yaml"),
];

#[test]
fn dns_snapshot_matches_the_normal_transient_policy_without_template_writes() {
    use omavless_domain::private_store::DnsPreset;
    let root = root();
    let uid = nix::unistd::Uid::current().as_raw();
    let (mut desired, _) = candidate();
    fs::create_dir(root.join("ruleset")).unwrap();
    for bundle in BUNDLES {
        for name in manifest(bundle).unwrap() {
            fs::write(root.join("ruleset").join(name), b"synthetic cache snapshot").unwrap();
        }
        for mode in [
            crate::desired::RoutingMode::Rule,
            crate::desired::RoutingMode::Global,
            crate::desired::RoutingMode::Direct,
        ] {
            desired.mode = mode;
            let template =
                omavless_domain::routing::template_with_mode(bundle, mode.as_str()).unwrap();
            for preset in [
                DnsPreset::PresetDefault,
                DnsPreset::Google,
                DnsPreset::Cloudflare,
            ] {
                let (_, store) = candidate_with_dns(Some(preset.id()));
                let snapshot =
                    ValidationSnapshot::capture(&root, uid, &desired, &store, &template).unwrap();
                let transformed = crate::managed_template::with_dns(&template, preset).unwrap();
                let expected = store
                    .prepare_config_mode(
                        &desired.profile_id,
                        &transformed,
                        "/work/mihomo.sock",
                        mode.as_str(),
                    )
                    .unwrap();
                assert!(snapshot.config == expected);
                assert_eq!(snapshot.resources.len(), manifest(&template).unwrap().len());
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

fn root() -> PathBuf {
    let root = crate::test_temp::directory("isolated-validation").unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}

fn candidate() -> (DesiredState, PrivateStore) {
    candidate_with_dns(None)
}

fn candidate_with_dns(preset: Option<&str>) -> (DesiredState, PrivateStore) {
    let id = "00000000-0000-4000-8000-000000000001";
    let mut document = serde_json::json!({
        "version":3,"activeId":"","lastId":id,
        "profiles":[{"id":id,"name":"Synthetic","protocol":"vless",
        "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp",
        "subscriptionId":"","subscriptionKey":"","missing":false,"favorite":false}],
        "subscriptions":[],"customRules":[]});
    if let Some(preset) = preset {
        document["dnsPreset"] = preset.into();
    }
    let store = omavless_domain::private_store::parse_private_store(&document.to_string()).unwrap();
    (
        DesiredState {
            connected: true,
            profile_id: id.into(),
            ..DesiredState::default()
        },
        store,
    )
}

#[test]
fn capture_once_preserves_exact_canonical_render_and_requires_complete_map() {
    let root = root();
    let uid = nix::unistd::Uid::current().as_raw();
    let (mut desired, store) = candidate();
    assert!(ValidationSnapshot::capture(&root, uid, &desired, &store, BUNDLES[1]).is_err());
    fs::create_dir(root.join("ruleset")).unwrap();
    for name in manifest(BUNDLES[1]).unwrap() {
        fs::write(root.join("ruleset").join(name), b"synthetic cache snapshot").unwrap();
    }
    for mode in [
        crate::desired::RoutingMode::Rule,
        crate::desired::RoutingMode::Global,
        crate::desired::RoutingMode::Direct,
    ] {
        desired.mode = mode;
        let snapshot =
            ValidationSnapshot::capture(&root, uid, &desired, &store, BUNDLES[1]).unwrap();
        let expected = store
            .prepare_config_mode(
                &desired.profile_id,
                BUNDLES[1],
                "/work/mihomo.sock",
                mode.as_str(),
            )
            .unwrap();
        assert!(snapshot.config == expected); // never dump rendered private config
        assert_eq!(snapshot.resources.len(), 5);
    }
    let snapshot = ValidationSnapshot::capture(&root, uid, &desired, &store, BUNDLES[1]).unwrap();
    let first = root.join("ruleset").join(&snapshot.resources[0].0);
    fs::write(&first, b"changed after capture").unwrap();
    assert!(snapshot.resources[0].1 == b"synthetic cache snapshot");
    fs::remove_file(&first).unwrap();
    assert!(ValidationSnapshot::capture(&root, uid, &desired, &store, BUNDLES[1]).is_err());
    symlink("missing", &first).unwrap();
    assert!(ValidationSnapshot::capture(&root, uid, &desired, &store, BUNDLES[1]).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_bundled_manifest_has_all_expected_resources_and_modes() {
    for (bundle, count) in BUNDLES.iter().zip([23, 5, 7]) {
        for mode in ["rule", "global", "direct"] {
            let template = bundle.replace("\nmode: rule\n", &format!("\nmode: {mode}\n"));
            assert_eq!(manifest(&template).unwrap().len(), count);
        }
        for suffix in [
            "\n",
            "\nexternal-ui: /private\n",
            "\ngeodata-loader: standard\n",
        ] {
            assert_eq!(
                manifest(&format!("{bundle}{suffix}")),
                Err(ValidationError::UnsupportedTemplate)
            );
        }
    }
    assert_eq!(
        manifest("private malformed input"),
        Err(ValidationError::UnsupportedTemplate)
    );
}

#[test]
fn bounded_resource_reads_refuse_missing_symlink_empty_oversize_and_writable() {
    let root = root();
    let path = root.join("resource");
    let uid = nix::unistd::Uid::current().as_raw();
    assert!(read_bounded(&path, uid, 4, false).is_err());
    fs::write(&path, b"abcd").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(read_bounded(&path, uid, 4, false).unwrap(), b"abcd");
    assert!(read_bounded(&path, uid, 3, false).is_err());
    symlink(&path, root.join("link")).unwrap();
    assert!(read_bounded(&root.join("link"), uid, 4, false).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(read_bounded(&path, uid, 4, false).is_err());
    fs::write(&path, b"").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(read_bounded(&path, uid, 4, false).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn static_elf_refuses_scripts_interpreters_bad_offsets_and_truncation() {
    assert!(!static_elf(b"#!/bin/sh\nexit 0\n"));
    let mut elf = vec![0u8; 120];
    elf[..6].copy_from_slice(b"\x7fELF\x02\x01");
    elf[32..40].copy_from_slice(&64u64.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1u16.to_le_bytes());
    assert!(static_elf(&elf));
    elf[64..68].copy_from_slice(&3u32.to_le_bytes());
    assert!(!static_elf(&elf));
    elf[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(!static_elf(&elf));
    assert!(!static_elf(&elf[..60]));
}

#[test]
fn fixed_sandbox_has_no_host_tree_environment_network_or_optional_namespace() {
    let command = sandbox_command(Path::new("/synthetic"));
    assert_eq!(command.get_program(), "/usr/bin/bwrap");
    let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
    for required in [
        "--unshare-all",
        "--unshare-user",
        "--disable-userns",
        "--die-with-parent",
        "--cap-drop",
        "--clearenv",
        "--remount-ro",
    ] {
        assert!(args.contains(&required));
    }
    for forbidden in [
        "--share-net",
        "--unshare-user-try",
        "/usr",
        "/home",
        "--bind",
    ] {
        assert!(!args.contains(&forbidden));
    }
    assert_eq!(
        &args[args.len() - 6..],
        [
            "/input/core",
            "-t",
            "-d",
            "/work",
            "-f",
            "/input/config.yaml"
        ]
    );
}

#[test]
fn scratch_cleanup_is_exact_private_and_preserves_replacement() {
    let root = root();
    let uid = nix::unistd::Uid::current().as_raw();
    let mut scratch = Scratch::new(&root, uid).unwrap();
    scratch.add("config.yaml", b"private", 0o400).unwrap();
    assert_eq!(fs::metadata(&scratch.path).unwrap().mode() & 0o777, 0o700);
    let original = scratch.path.join("config.yaml");
    fs::rename(&original, scratch.path.join("saved")).unwrap();
    fs::write(&original, b"replacement").unwrap();
    assert_eq!(scratch.verify(), Err(ValidationError::Cleanup));
    assert_eq!(scratch.cleanup(), Err(ValidationError::Cleanup));
    assert_eq!(fs::read(&original).unwrap(), b"replacement");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn partial_initialization_cleans_only_proven_empty_directory() {
    let root = root();
    let uid = nix::unistd::Uid::current().as_raw();
    let result = Scratch::initialize(&root, uid, |_| {
        Err(std::io::ErrorKind::PermissionDenied.into())
    });
    assert!(matches!(result, Err(ValidationError::UnsafeInput)));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    let result = Scratch::initialize(&root, uid, |path| {
        fs::write(path.join("unexpected"), b"preserved")?;
        Err(std::io::ErrorKind::PermissionDenied.into())
    });
    assert!(matches!(result, Err(ValidationError::Cleanup)));
    let path = fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
    assert_eq!(fs::read(path.join("unexpected")).unwrap(), b"preserved");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn prelaunch_directory_replacement_refuses_without_deleting_either_inode() {
    let root = root();
    let uid = nix::unistd::Uid::current().as_raw();
    let mut scratch = Scratch::new(&root, uid).unwrap();
    let original = scratch.path.join("ruleset");
    fs::rename(&original, scratch.path.join("original-ruleset")).unwrap();
    fs::create_dir(&original).unwrap();
    assert_eq!(scratch.verify(), Err(ValidationError::Cleanup));
    assert_eq!(scratch.cleanup(), Err(ValidationError::Cleanup));
    assert!(original.exists());
    assert!(scratch.path.join("original-ruleset").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sandbox_failure_and_timeout_have_fixed_errors() {
    assert_eq!(
        run(
            &mut Command::new("/nonexistent/synthetic"),
            Duration::from_millis(10)
        ),
        Err(ValidationError::SandboxRejected)
    );
    assert_eq!(
        run(
            Command::new("/usr/bin/sleep").arg("1"),
            Duration::from_millis(10)
        ),
        Err(ValidationError::Timeout)
    );
    for error in [
        ValidationError::UnsupportedTemplate,
        ValidationError::UnsafeInput,
        ValidationError::ResourceUnavailable,
        ValidationError::UnsupportedCore,
        ValidationError::SandboxRejected,
        ValidationError::Timeout,
        ValidationError::Cleanup,
    ] {
        assert!(format!("{error:?}").len() < 32);
    }
}

#[test]
fn installed_core_isolated_offline_snapshot_optin() {
    let Some(core) = std::env::var_os("OMAVLESS_TEST_MIHOMO") else {
        return;
    };
    let root = root();
    let uid = nix::unistd::Uid::current().as_raw();
    // Same credential-free profile/canonical renderer as the accepted snapshot
    // reference; never inspect installed private input.
    let (desired, store) = candidate();
    let snapshot = ValidationSnapshot {
        config: store.prepare_config_mode(&desired.profile_id,
            "mixed-port: 0\nmode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n",
            "/work/mihomo.sock", "rule").unwrap(),
        resources: vec![],
    };
    assert_eq!(snapshot.validate(Path::new(&core), &root, uid), Ok(()));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    // Known -t geodata effect from R5_STARTUP_VALIDATION_SNAPSHOT: this
    // endpoint is a controlled local observer, not a provider or live fixture.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let attempted = ValidationSnapshot {
        config: format!(
            "mixed-port: 0\nmode: rule\ngeodata-mode: true\ngeox-url:\n  geosite: http://127.0.0.1:{port}/synthetic.dat\nproxies: []\nrules:\n  - GEOSITE,synthetic,DIRECT\n  - MATCH,DIRECT\n"
        ),
        resources: vec![],
    };
    assert_eq!(
        attempted.validate(Path::new(&core), &root, uid),
        Err(ValidationError::SandboxRejected)
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
}

/// Real cache bytes, synthetic profile only. No installed configuration is
/// generated or changed; bwrap keeps every core validation offline.
fn check_installed_bundle(label: &str, bundle: &str) {
    let core = std::env::var_os("OMAVLESS_TEST_MIHOMO").expect("installed core opt-in is required");
    let data = std::env::var_os("OMAVLESS_TEST_RULE_CACHE")
        .expect("explicit read-only cache opt-in is required");
    let scratch = root();
    let uid = nix::unistd::Uid::current().as_raw();
    let (mut desired, store) = candidate();
    let mut passed = 0;
    for mode in [
        crate::desired::RoutingMode::Rule,
        crate::desired::RoutingMode::Global,
        crate::desired::RoutingMode::Direct,
    ] {
        desired.mode = mode;
        let before = ValidationSnapshot::capture(Path::new(&data), uid, &desired, &store, bundle)
            .unwrap_or_else(|error| panic!("{label} cache capture: {error:?}"));
        let result = before.validate(Path::new(&core), &scratch, uid);
        // Check cleanup even when the selected core refuses the bundle.
        assert_eq!(fs::read_dir(&scratch).unwrap().count(), 0);
        assert_eq!(result, Ok(()), "{label} / {}", mode.as_str());
        let after = ValidationSnapshot::capture(Path::new(&data), uid, &desired, &store, bundle)
            .unwrap_or_else(|error| panic!("{label} cache recheck: {error:?}"));
        // Never format config/cache bytes in a failing assertion.
        assert!(
            before.config == after.config && before.resources == after.resources,
            "read-only cache changed during isolated validation"
        );
        passed += 1;
    }
    assert_eq!(passed, 3);
    fs::remove_dir(&scratch).unwrap();
}

#[test]
#[ignore = "requires installed Mihomo, bubblewrap and explicit RU cache directory"]
fn installed_bundled_ru_resources_isolated_optin() {
    check_installed_bundle("ru", BUNDLES[0]);
}

#[test]
#[ignore = "requires installed Mihomo, bubblewrap and explicit CN cache directory"]
fn installed_bundled_cn_resources_isolated_optin() {
    check_installed_bundle("cn", BUNDLES[1]);
}

#[test]
#[ignore = "requires installed Mihomo, bubblewrap and explicit IR cache directory"]
fn installed_bundled_ir_resources_isolated_optin() {
    check_installed_bundle("ir", BUNDLES[2]);
}
