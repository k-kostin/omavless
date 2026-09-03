// SPDX-License-Identifier: MIT

use serde_json::Value;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl ChildGuard {
    fn terminate(mut self) {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(self.0.id().to_string())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(self.0.wait().unwrap().success());
    }
}

fn runtime_base() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("ovr-cli-{}-{nonce}-{sequence}", std::process::id()));
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(&path).unwrap();
    path
}

fn command(base: &Path, action: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg(action)
        .env("XDG_RUNTIME_DIR", base)
        .env("XDG_STATE_HOME", base.join("state"))
        .env("XDG_CONFIG_HOME", base.join("xdg-config"))
        .env("HOME", base.join("home"))
        .env("OMAVLESS_HOME", base.join("home"))
        .output()
        .unwrap()
}

fn prepare_isolated_daemon_environment(base: &Path) {
    for path in [
        base.join("state"),
        base.join("xdg-config"),
        base.join("home"),
    ] {
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

#[test]
fn preflight_reads_desired_state_without_disclosing_profile_id() {
    let base = runtime_base();
    let state_root = base.join("state");
    fs::create_dir(&state_root).unwrap();
    fs::set_permissions(&state_root, fs::Permissions::from_mode(0o700)).unwrap();
    let state_dir = state_root.join("omavless");
    fs::create_dir(&state_dir).unwrap();
    fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let desired = state_dir.join("desired.json");
    fs::write(
        &desired,
        r#"{"schemaVersion":1,"generation":7,"connected":true,"profileId":"private-profile-id","mode":"global"}"#,
    )
    .unwrap();
    fs::set_permissions(&desired, fs::Permissions::from_mode(0o600)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg("preflight")
        .env("XDG_STATE_HOME", &state_root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let rendered = String::from_utf8(output.stdout).unwrap();
    let payload: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(payload["generation"], 7);
    assert_eq!(payload["connected"], true);
    assert_eq!(payload["profilePresent"], true);
    assert_eq!(payload["mode"], "global");
    assert!(!rendered.contains("private-profile-id"));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn store_preflight_validates_private_config_but_returns_only_safe_facts() {
    let base = runtime_base();
    let home = base.join("home");
    let config = home.join(".config/omavless");
    fs::create_dir_all(&config).unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o700)).unwrap();
    let id = "00000000-0000-0000-0000-000000000001";
    let store = format!(
        r#"{{"version":3,"activeId":"","lastId":"{id}","profiles":[{{"id":"{id}","name":"Synthetic","uri":"vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp","protocol":"vless"}}],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{{"enabled":false,"target":"last","profileId":"","mode":"rule"}},"onboardingComplete":true}}"#
    );
    fs::write(config.join("profiles.json"), &store).unwrap();
    fs::write(
        config.join("route-template.yaml"),
        "proxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
    )
    .unwrap();
    for path in [
        config.join("profiles.json"),
        config.join("route-template.yaml"),
    ] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg("store-preflight")
        .env("OMAVLESS_HOME", &home)
        .env("XDG_RUNTIME_DIR", &base)
        .output()
        .unwrap();
    assert!(output.status.success());
    let rendered = String::from_utf8(output.stdout).unwrap();
    let payload: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(payload["profileCount"], 1);
    assert_eq!(payload["protocolCounts"]["vless"], 1);
    assert_eq!(payload["configReady"], true);
    for private in [id, "11111111", "203.0.113.1", "Synthetic"] {
        assert!(!rendered.contains(private));
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn daemon_and_semantic_cli_use_one_private_runtime() {
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let child = Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg("daemon")
        .env("XDG_RUNTIME_DIR", &base)
        .env("XDG_STATE_HOME", base.join("state"))
        .env("XDG_CONFIG_HOME", base.join("xdg-config"))
        .env("HOME", base.join("home"))
        .env("OMAVLESS_HOME", base.join("home"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let guard = ChildGuard(child);
    let socket = base.join("omavless/control.sock");
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(socket.exists());
    assert_eq!(
        fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );

    for action in ["hello", "status", "capabilities"] {
        let output = command(&base, action);
        assert!(output.status.success());
        let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["result"]["runtimeOwnership"], false);
        assert!(output.stderr.is_empty());
    }

    let second = command(&base, "daemon");
    assert_eq!(second.status.code(), Some(2));
    let error = String::from_utf8(second.stderr).unwrap();
    assert!(error.contains("already owns this session"));
    assert!(!error.contains(base.to_string_lossy().as_ref()));

    guard.terminate();
    assert!(!socket.exists());
    fs::remove_dir_all(base).unwrap();
}
