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

    drop(guard);
    fs::remove_dir_all(base).unwrap();
}
