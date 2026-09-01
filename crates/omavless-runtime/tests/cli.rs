// SPDX-License-Identifier: MIT

use serde_json::Value;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ovr-cli-{}-{nonce}", std::process::id()));
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(&path).unwrap();
    path
}

fn command(base: &Path, action: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg(action)
        .env("XDG_RUNTIME_DIR", base)
        .output()
        .unwrap()
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
fn daemon_and_semantic_cli_use_one_private_runtime() {
    let base = runtime_base();
    let child = Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg("daemon")
        .env("XDG_RUNTIME_DIR", &base)
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
