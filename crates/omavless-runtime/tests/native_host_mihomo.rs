// SPDX-License-Identifier: MIT

use nix::unistd::Uid;
use omavless_runtime::desired::{DesiredState, RoutingMode};
use omavless_runtime::lifecycle::LifecycleHost;
use omavless_runtime::native_host::{NativeHostPaths, NativeLifecycleHost};
use std::env;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";

fn configured_core() -> Option<PathBuf> {
    env::var_os("OMAVLESS_TEST_MIHOMO").map(PathBuf::from)
}

fn root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "omavless-native-host-mihomo-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}

fn private_directory(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[test]
fn native_host_stages_validates_owns_observes_commits_and_stops_mihomo() {
    let Some(core) = configured_core() else {
        return;
    };
    let root = root();
    let data = private_directory(&root, "data");
    let config = private_directory(&root, "config");
    let runtime = private_directory(&root, "runtime");
    let proc_root = private_directory(&root, "proc");
    let sys_class_net = private_directory(&root, "sys-class-net");
    let store = format!(
        r#"{{
          "version": 3,
          "activeId": "",
          "lastId": "{PROFILE_ID}",
          "profiles": [{{
            "id": "{PROFILE_ID}",
            "name": "Synthetic",
            "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic",
            "protocol": "vless",
            "subscriptionId": "",
            "subscriptionKey": "",
            "missing": false,
            "favorite": false
          }}],
          "subscriptions": [],
          "routingPreset": "custom",
          "customRules": [],
          "rulesUpdatedAt": 0,
          "startupConfigured": false,
          "startup": {{"enabled": false, "target": "last", "profileId": "", "mode": "rule"}},
          "onboardingComplete": true
        }}"#
    );
    fs::write(config.join("profiles.json"), store).unwrap();
    fs::write(
        config.join("route-template.yaml"),
        "mixed-port: 0\nallow-lan: false\nmode: rule\nlog-level: silent\nproxies:\n{{OMAVLESS_PROXY}}\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies:\n      - Synthetic\nrules:\n  - MATCH,DIRECT\n",
    )
    .unwrap();
    fs::write(config.join("config.yaml"), "previous-generated-config\n").unwrap();
    for name in ["profiles.json", "route-template.yaml", "config.yaml"] {
        fs::set_permissions(config.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }

    let paths = NativeHostPaths::new(
        core,
        data,
        config.clone(),
        runtime,
        proc_root.clone(),
        sys_class_net,
    );
    let uid = Uid::current().as_raw();
    let mut host = NativeLifecycleHost::new(paths, uid).unwrap();
    let desired = DesiredState {
        generation: 1,
        connected: true,
        profile_id: PROFILE_ID.to_owned(),
        mode: RoutingMode::Direct,
        ..DesiredState::default()
    };

    let external = proc_root.join("4242");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("comm"), "mihomo\n").unwrap();
    let conflict = host.observe(&DesiredState::default()).unwrap();
    assert_eq!(conflict.core_count, 1);
    assert!(!conflict.service_active);
    fs::remove_dir_all(external).unwrap();

    host.prepare(&desired).unwrap();
    assert_eq!(
        fs::metadata(config.join(".config.candidate.yaml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::read_to_string(config.join("config.yaml")).unwrap(),
        "previous-generated-config\n"
    );

    host.start_prepared().unwrap();
    let running = host.observe(&desired).unwrap();
    assert!(running.service_active);
    assert!(running.controller_ready);
    assert_eq!(running.core_count, 1);
    assert_eq!(running.tun_count, 0);
    assert!(running.active_profile_matches);
    assert!(host.core_pid().is_some());

    host.commit_prepared().unwrap();
    let active = fs::read_to_string(config.join("config.yaml")).unwrap();
    assert!(active.contains("\nmode: direct\n"));
    assert!(active.contains("external-controller-unix:"));
    assert!(!active.contains("external-controller:"));
    assert!(!config.join(".config.candidate.yaml").exists());

    host.stop_owned().unwrap();
    host.discard_prepared().unwrap();
    let stopped = host.observe(&DesiredState::default()).unwrap();
    assert!(!stopped.service_active);
    assert!(!stopped.controller_ready);
    assert_eq!(stopped.core_count, 0);
    assert_eq!(stopped.tun_count, 0);
    assert!(host.core_pid().is_none());
    assert!(!config.join("../runtime/mihomo.sock").exists());
    assert_eq!(fs::metadata(&config).unwrap().uid(), uid);

    // Exercise the new domain mutation through the actual native host/core
    // adapter, not only generated text. This isolated config has no TUN,
    // listener or provider fetch; it is not live VPN/cutover evidence.
    let rule_id = "00000000-0000-4000-8000-000000000099";
    for add in [true, false] {
        use omavless_domain::private_store::{CustomRuleMutation, apply_custom_rule_mutation};
        let mutation = if add {
            CustomRuleMutation::Add {
                kind: "domain".into(),
                action: "direct".into(),
                value: "example.invalid".into(),
            }
        } else {
            CustomRuleMutation::Delete {
                rule_id: rule_id.into(),
            }
        };
        let before = fs::read_to_string(config.join("profiles.json")).unwrap();
        let candidate = apply_custom_rule_mutation(&before, mutation, rule_id).unwrap();
        omavless_store::atomic_replace_private(
            &config.join("profiles.json"),
            candidate.payload(),
            uid,
        )
        .unwrap();
        host.prepare(&desired).unwrap();
        let staged = fs::read_to_string(config.join(".config.candidate.yaml")).unwrap();
        assert_eq!(staged.contains("DOMAIN,example.invalid,DIRECT"), add);
        host.start_prepared().unwrap();
        let health = host.observe(&desired).unwrap();
        assert!(health.service_active && health.controller_ready);
        assert_eq!(health.core_count, 1);
        assert_eq!(health.tun_count, 0);
        // /version can respond before core rule initialization finishes. Wait
        // for this exact synthetic configuration, not a fixed delay or an empty
        // array (which could falsely satisfy the deletion case).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .expect("Mihomo did not load the committed synthetic rules before the deadline");
            let loaded = omavless_mihomo::controller_get(
                &root.join("runtime/mihomo.sock"),
                omavless_mihomo::ReadOnlyEndpoint::Rules,
                remaining.min(std::time::Duration::from_millis(250)),
                64 * 1024,
            )
            .ok()
            .is_some_and(|response| {
                response.status == 200
                    && response.payload["rules"].as_array().is_some_and(|rules| {
                        rules.len() == if add { 2 } else { 1 }
                            && rules
                                .iter()
                                .any(|rule| rule["type"] == "Match" && rule["proxy"] == "DIRECT")
                            && rules.iter().any(|rule| {
                                rule["type"] == "Domain"
                                    && rule["payload"] == "example.invalid"
                                    && rule["proxy"] == "DIRECT"
                            }) == add
                    })
            });
            if loaded {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        host.commit_prepared().unwrap();
        let active = fs::read_to_string(config.join("config.yaml")).unwrap();
        assert!(active.contains("external-controller-unix:"));
        assert!(!active.contains("external-controller:"));
        host.stop_owned().unwrap();
        host.discard_prepared().unwrap();
        let health = host.observe(&DesiredState::default()).unwrap();
        assert!(!health.service_active && !health.controller_ready);
        assert_eq!(health.core_count, 0);
        assert_eq!(health.tun_count, 0);
        assert!(!root.join("runtime/mihomo.sock").exists());
    }

    let committed = fs::read(config.join("config.yaml")).unwrap();
    let missing = DesiredState {
        connected: true,
        profile_id: "00000000-0000-4000-8000-000000000099".to_owned(),
        ..desired
    };
    assert!(host.prepare(&missing).is_err());
    host.discard_prepared().unwrap();
    assert_eq!(fs::read(config.join("config.yaml")).unwrap(), committed);
    assert!(!config.join(".config.candidate.yaml").exists());
    fs::remove_dir_all(root).unwrap();
}
