// SPDX-License-Identifier: MIT

use omavless_runtime::semantic_cli::MAX_SUBSCRIPTION_STDIN_BYTES;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct ChildGuard(Child);

#[test]
fn custom_rules_cli_maps_only_fixed_read_and_keeps_private_output_off_stderr() {
    use omavless_control_protocol::{
        FrameKind, decode_request, encode_response, read_unary_frame, success_response,
        write_unary_frame,
    };
    use std::os::unix::net::UnixListener;
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let paths = omavless_runtime::RuntimePaths::below(&base);
    fs::create_dir(&paths.directory).unwrap();
    fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(&paths.socket).unwrap();
    fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request =
            decode_request(&read_unary_frame(&mut stream, FrameKind::Request).unwrap()).unwrap();
        assert_eq!(request["method"], "routing.custom_rules.list");
        assert_eq!(request["params"], serde_json::json!({}));
        let response = success_response(request["id"].as_str().unwrap(), 7, serde_json::json!({"version":1,"rules":[{"id":"00000000-0000-4000-8000-000000000001","kind":"domain","action":"direct","value":"example.invalid"}]})).unwrap();
        write_unary_frame(
            &mut stream,
            &encode_response(&response).unwrap(),
            FrameKind::Response,
        )
        .unwrap();
    });
    let output = isolated_command(&base)
        .args(["routing", "rules"])
        .output()
        .unwrap();
    worker.join().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["revision"], 7);
    assert!(response["result"]["rules"][0]["value"] == "example.invalid");
    for argv in [
        ["routing", "rules", "private-token"],
        ["routing", "add", "private-token"],
    ] {
        let invalid = isolated_command(&base).args(argv).output().unwrap();
        assert!(!invalid.status.success());
        assert!(invalid.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&invalid.stderr).contains("private-token"));
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn store_compatibility_cli_is_bounded_read_only_and_requires_no_external_tools() {
    use omavless_runtime::cutover::{CutoverPaths, MigrationLock};
    use std::os::unix::fs::{MetadataExt, symlink};
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let directory = base.join("home/.config/omavless");
    fs::create_dir_all(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.join("profiles.json");
    let uid = fs::metadata(&base).unwrap().uid();
    let cutover = CutoverPaths::below(&base, &base.join("state"), uid);
    let run = || {
        let output = isolated_command(&base)
            .arg("store-compatibility")
            .env("PATH", base.join("no-external-tools"))
            .output()
            .unwrap();
        assert!(output.status.success(), "compatibility CLI failed");
        assert!(output.stderr.is_empty());
        assert!(output.stdout.len() < 512);
        for private in [
            "private-token",
            "private-label",
            "vless://",
            "11111111",
            "example.invalid",
        ] {
            assert!(!String::from_utf8_lossy(&output.stdout).contains(private));
        }
        assert!(!cutover.ownership_marker.exists());
        assert!(!omavless_runtime::RuntimePaths::below(&base).socket.exists());
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    assert_eq!(run()["code"], "store_unavailable");
    assert!(!path.exists());
    let valid = br#"{"version":3,"profiles":[],"subscriptions":[]}"#;
    fs::write(&path, valid).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let report = run();
    assert_eq!(report["schemaVersion"], 1);
    assert_eq!(report["compatible"], true);
    assert_eq!(report["code"], "compatible");
    assert!(fs::read(&path).unwrap() == valid);
    let corpus: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/parity_cases/vless-canonical-v1.json"
    ))
    .unwrap();
    for case_id in [
        "xhttp-unknown",
        "xhttp-stream-one-download",
        "download-mode-mismatch",
        "recursive-extra",
    ] {
        let case = corpus.iter().find(|case| case["id"] == case_id).unwrap();
        let store = serde_json::json!({"version":3,"profiles":[{
            "id":"00000000-0000-4000-8000-000000000001",
            "name":"private-label","protocol":"vless","uri":case["uri"]
        }],"subscriptions":[]})
        .to_string();
        fs::write(&path, store.as_bytes()).unwrap();
        assert_eq!(run()["code"], "store_requires_repair");
        assert!(fs::read(&path).unwrap() == store.as_bytes());
    }
    for invalid in [
        b"private-token".to_vec(),
        vec![0xff],
        vec![b'x'; 5 * 1024 * 1024 + 1],
    ] {
        fs::write(&path, &invalid).unwrap();
        let report = run();
        assert_eq!(report["compatible"], false);
        assert_eq!(report["code"], "store_requires_repair");
        assert_eq!(report["recovery"], "legacy_repair_or_export");
        assert!(fs::read(&path).unwrap() == invalid);
    }
    fs::write(&path, valid).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(run()["code"], "store_unavailable");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o644
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let saved = directory.join("saved-store.json");
    fs::rename(&path, &saved).unwrap();
    symlink(&saved, &path).unwrap();
    assert_eq!(run()["code"], "store_unavailable");
    fs::remove_file(&path).unwrap();
    fs::rename(&saved, &path).unwrap();
    let lock = MigrationLock::acquire(&cutover, uid).unwrap();
    let busy = command(&base, "store-compatibility");
    assert!(!busy.status.success());
    assert!(busy.stdout.is_empty());
    drop(lock);
    assert_eq!(run()["compatible"], true);
    let invalid = isolated_command(&base)
        .args(["store-compatibility", "private-token"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("private-token"));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn explicit_profile_reads_cli_keep_sensitive_success_off_stderr() {
    use omavless_control_protocol::{
        FrameKind, decode_request, encode_response, read_unary_frame, success_response,
        write_unary_frame,
    };
    use std::os::unix::net::UnixListener;
    for editor in [false, true] {
        let base = runtime_base();
        prepare_isolated_daemon_environment(&base);
        let paths = omavless_runtime::RuntimePaths::below(&base);
        fs::create_dir(&paths.directory).unwrap();
        fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
        let listener = UnixListener::bind(&paths.socket).unwrap();
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let request =
                decode_request(&read_unary_frame(&mut stream, FrameKind::Request).unwrap())
                    .unwrap();
            assert_eq!(
                request["method"],
                if editor {
                    "profiles.edit_input"
                } else {
                    "profiles.export"
                }
            );
            assert_eq!(
                request["params"].as_object().unwrap().len(),
                if editor { 1 } else { 2 }
            );
            if !editor {
                assert_eq!(request["params"]["purpose"], "file");
            }
            let response = success_response(request["id"].as_str().unwrap(), 7,
            if editor { serde_json::json!({"name":"Synthetic","input":"trojan://synthetic-token@203.0.113.1:443"}) }
            else { serde_json::json!({"format":"uri","content":"trojan://synthetic-token@203.0.113.1:443"}) }).unwrap();
            write_unary_frame(
                &mut stream,
                &encode_response(&response).unwrap(),
                FrameKind::Response,
            )
            .unwrap();
        });
        let argv = if editor {
            vec![
                "profile",
                "edit-input",
                "00000000-0000-4000-8000-000000000001",
            ]
        } else {
            vec![
                "profile",
                "export",
                "00000000-0000-4000-8000-000000000001",
                "file",
            ]
        };
        let output = isolated_command(&base).args(&argv).output().unwrap();
        worker.join().unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["revision"], 7);
        assert!(
            response["result"][if editor { "input" } else { "content" }]
                == "trojan://synthetic-token@203.0.113.1:443"
        );
        let mut invalid = argv;
        invalid[2] = "private-token";
        let rejected = isolated_command(&base).args(invalid).output().unwrap();
        assert!(!rejected.status.success());
        assert!(!String::from_utf8_lossy(&rejected.stderr).contains("private-token"));
        assert!(rejected.stdout.is_empty());
        fs::remove_dir_all(base).unwrap();
    }
}

#[test]
fn import_preview_cli_sends_only_fixed_private_request_and_prints_response() {
    use omavless_control_protocol::{
        FrameKind, decode_request, encode_response, read_unary_frame, success_response,
        write_unary_frame,
    };
    use std::os::unix::net::UnixListener;
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let paths = omavless_runtime::RuntimePaths::below(&base);
    fs::create_dir(&paths.directory).unwrap();
    fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(&paths.socket).unwrap();
    fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request =
            decode_request(&read_unary_frame(&mut stream, FrameKind::Request).unwrap()).unwrap();
        assert_eq!(request["method"], "imports.classify");
        assert_eq!(request["params"].as_object().unwrap().len(), 1);
        assert!(request["params"]["input"] == "https://example.invalid/synthetic-token\n");
        let response = success_response(
            request["id"].as_str().unwrap(),
            7,
            serde_json::json!({"version": 1, "kind": "subscription",
                "suggestedName": "Subscription", "duplicate": true}),
        )
        .unwrap();
        write_unary_frame(
            &mut stream,
            &encode_response(&response).unwrap(),
            FrameKind::Response,
        )
        .unwrap();
    });
    let mut child = isolated_command(&base)
        .args(["import", "preview"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"https://example.invalid/synthetic-token\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    worker.join().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["revision"], 7);
    assert_eq!(response["result"]["duplicate"], true);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("synthetic-token"));
    fs::remove_dir_all(base).unwrap();
}

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

fn isolated_command(base: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omavless"));
    command
        .env("XDG_RUNTIME_DIR", base)
        .env("XDG_STATE_HOME", base.join("state"))
        .env("XDG_CONFIG_HOME", base.join("xdg-config"))
        .env("HOME", base.join("home"))
        .env("OMAVLESS_HOME", base.join("home"));
    command
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
fn help_exposes_only_fixed_semantic_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_omavless"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    for command in [
        "connect PROFILE_ID [rule|global|direct]",
        "disconnect",
        "mode rule|global|direct",
        "profile list",
        "profile rename PROFILE_ID",
        "profile favorite PROFILE_ID on|off",
        "profile delete PROFILE_ID",
        "profile import",
        "profile replace PROFILE_ID",
        "profile export PROFILE_ID qr|file",
        "profile edit-input PROFILE_ID",
        "store-compatibility",
        "subscription list",
        "subscription edit-input SUBSCRIPTION_ID",
        "subscription add",
        "subscription update SUBSCRIPTION_ID",
        "subscription delete SUBSCRIPTION_ID",
        "subscription refresh SUBSCRIPTION_ID",
        "import preview",
    ] {
        assert!(help.contains(command));
    }
    assert!(!help.contains("METHOD"));
    assert!(!help.contains("JSON"));
}

#[test]
fn raw_and_extra_commands_fail_before_socket_without_echoing_arguments() {
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let private = "private.example/password";
    for arguments in [
        vec!["request", private],
        vec!["connect", private, "rule", "extra"],
        vec!["mode", private],
        vec!["import", "preview", private],
        vec!["profile", "import", private],
    ] {
        let output = isolated_command(&base).args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("semantic command"));
        assert!(!error.contains("private.example"));
        assert!(!error.contains("password"));
    }
    assert!(!base.join("omavless/control.sock").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn rename_stdin_is_bounded_and_never_echoed() {
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let profile = "00000000-0000-4000-8000-000000000001";
    let private = "private.example/password".repeat(20);
    let mut child = isolated_command(&base)
        .args(["profile", "rename", profile])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let write_result = child.stdin.take().unwrap().write_all(private.as_bytes());
    assert!(
        write_result.is_ok()
            || write_result
                .as_ref()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
    );
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("input is too large"));
    assert!(!error.contains("private.example"));
    assert!(!error.contains("password"));
    assert!(!base.join("omavless/control.sock").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn import_preview_stdin_rejects_oversize_invalid_utf8_and_empty_without_echo() {
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    for input in [
        vec![b'x'; omavless_runtime::import_read_protocol::MAX_IMPORT_STDIN_BYTES + 1],
        b"synthetic-password\xff".to_vec(),
        b" \n".to_vec(),
    ] {
        let mut child = isolated_command(&base)
            .args(["import", "preview"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let written = child.stdin.take().unwrap().write_all(&input);
        assert!(
            written.is_ok() || written.is_err_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe)
        );
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("synthetic-password"));
    }
    assert!(!base.join("omavless/control.sock").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn subscription_stdin_is_bounded_and_never_echoed() {
    let base = runtime_base();
    prepare_isolated_daemon_environment(&base);
    let private = format!(
        "Private source\nhttps://private.example/{}",
        "password".repeat(MAX_SUBSCRIPTION_STDIN_BYTES)
    );
    let mut child = isolated_command(&base)
        .args(["subscription", "add"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let write_result = child.stdin.take().unwrap().write_all(private.as_bytes());
    assert!(
        write_result.is_ok()
            || write_result
                .as_ref()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
    );
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("input is too large"));
    assert!(!error.contains("private.example"));
    assert!(!error.contains("password"));
    assert!(!base.join("omavless/control.sock").exists());
    fs::remove_dir_all(base).unwrap();
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

    for arguments in [["profile", "list"], ["subscription", "list"]] {
        let output = isolated_command(&base).args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(payload["error"]["code"], "unknown_method");
        assert!(output.stderr.starts_with(b"OmaVLESS runtime rejected"));
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
