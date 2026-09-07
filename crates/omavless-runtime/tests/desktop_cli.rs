// SPDX-License-Identifier: MIT

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omavless-desktop-cli-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }

    fn call(&self, args: &[&str], input: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omavless"))
            .args(args)
            .env("PATH", &self.0)
            .env("XDG_RUNTIME_DIR", &self.0)
            .env("HOME", &self.0)
            .env("OMAVLESS_HOME", &self.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if !input.is_empty() {
            child.stdin.take().unwrap().write_all(input).unwrap();
        } else {
            drop(child.stdin.take());
        }
        child.wait_with_output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn desktop_cli_never_needs_owner_or_daemon_and_errors_do_not_echo() {
    let f = Fixture::new();
    let response = f.call(&["desktop", "capabilities"], &[]);
    assert!(response.status.success());
    let value: serde_json::Value = serde_json::from_slice(&response.stdout).unwrap();
    assert_eq!(value["clipboardReadAvailable"], false);
    assert_eq!(value["gtk4FallbackAvailable"], false);
    for args in [
        vec!["desktop", "clipboard-read"],
        vec!["desktop", "pick-import"],
        vec!["desktop", "private-token"],
        vec!["desktop", "file-read", "private-token"],
    ] {
        let response = f.call(&args, &[]);
        assert!(!response.status.success());
        assert!(response.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&response.stderr).contains("private-token"));
    }
    assert!(!f.0.join("omavless/control.sock").exists());
    assert!(!f.0.join(".config").exists());
}

#[test]
fn desktop_cli_paths_and_secrets_are_only_stdin_and_private_output() {
    let f = Fixture::new();
    let path = f.0.join("profile $(false).txt");
    let input = format!("{}\nsynthetic-private-input\n", path.display());
    let response = f.call(&["desktop", "export-file"], input.as_bytes());
    assert!(response.status.success());
    assert!(response.stdout.is_empty() && response.stderr.is_empty());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let response = f.call(
        &["desktop", "file-read"],
        path.as_os_str().as_encoded_bytes(),
    );
    assert!(response.status.success());
    assert!(response.stdout == b"synthetic-private-input\n");
    assert!(response.stderr.is_empty());
    let response = f.call(&["desktop", "file-read"], b"private-token");
    assert!(!response.status.success());
    assert!(response.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&response.stderr).contains("private-token"));
    let response = f.call(&["desktop", "cleanup"], &[]);
    assert!(response.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.stdout).unwrap()["removed"],
        0
    );
    assert!(!f.0.join("omavless/control.sock").exists());
}

#[test]
fn desktop_dialog_cancellation_retains_exit_three_without_error_output() {
    let f = Fixture::new();
    let tool = f.0.join("zenity");
    fs::write(&tool, b"#!/bin/bash\nexit 1\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    for operation in ["pick-import", "edit"] {
        let response = f.call(&["desktop", operation], b"");
        assert_eq!(response.status.code(), Some(3));
        assert!(response.stdout.is_empty() && response.stderr.is_empty());
    }
    assert_eq!(
        fs::read_dir(f.0.join("omavless-desktop")).unwrap().count(),
        0
    );
    assert!(!f.0.join("omavless").exists());
}

#[test]
fn desktop_editor_cold_start_creates_only_safe_client_scratch() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let tool = f.0.join("zenity");
    fs::write(
        &tool,
        b"#!/bin/bash\nfile=${3#--filename=}\n/usr/bin/cat -- \"$file\"\n",
    )
    .unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let response = f.call(&["desktop", "edit"], b"synthetic editor seed");
    assert!(response.status.success());
    assert!(response.stdout == b"synthetic editor seed");
    let scratch = f.0.join("omavless-desktop");
    assert_eq!(
        fs::metadata(&scratch).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(fs::read_dir(&scratch).unwrap().count(), 0);
    assert!(!f.0.join("omavless").exists());
    fs::set_permissions(&scratch, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        !f.call(&["desktop", "edit"], b"synthetic seed")
            .status
            .success()
    );
    assert_eq!(
        fs::metadata(&scratch).unwrap().permissions().mode() & 0o777,
        0o755
    );
    fs::remove_dir(&scratch).unwrap();
    let elsewhere = f.0.join("elsewhere");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&elsewhere)
        .unwrap();
    symlink(&elsewhere, &scratch).unwrap();
    assert!(!f.call(&["desktop", "cleanup"], b"").status.success());
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    fs::remove_file(&scratch).unwrap();
    fs::set_permissions(&f.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        !f.call(&["desktop", "edit"], b"synthetic seed")
            .status
            .success()
    );
    assert!(!scratch.exists());
    fs::set_permissions(&f.0, fs::Permissions::from_mode(0o700)).unwrap();
}
