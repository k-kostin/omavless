// SPDX-License-Identifier: MIT
use omavless_control_protocol::{
    FrameKind, decode_request, encode_response, read_unary_frame, success_response,
};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::thread;

#[path = "../../../tests/support/temp.rs"]
mod test_temp;

fn private_invoke(
    base: &std::path::Path,
    action: &str,
    input: &[u8],
    extra: &[&str],
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omavless"))
        .env("XDG_RUNTIME_DIR", base)
        .args(["plugin", action, "instance-1", "3", "operation-1"])
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // An invalid argv may exit without reading its input.
    let _ = child.stdin.take().unwrap().write_all(input);
    child.wait_with_output().unwrap()
}

#[test]
fn private_profile_cli_only_sends_validated_stdin_and_never_echoes_it() {
    let base = test_temp::directory("private-action-cli").unwrap();
    let directory = base.join("omavless");
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let id = "00000000-0000-4000-8000-000000000001";
    let worker = thread::spawn(move || {
        for (action, extra) in [
            ("profile-rename", json!({"name":"Private label"})),
            ("profile-favorite", json!({"enabled":true})),
            ("profile-delete", json!({})),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let request =
                decode_request(&read_unary_frame(&mut stream, FrameKind::Request).unwrap())
                    .unwrap();
            let mut expected = json!({"action":action,"instanceId":"instance-1","expectedRevision":3,"operationId":"operation-1","profileId":id});
            expected
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert_eq!(request["method"], "plugin.action");
            assert_eq!(request["params"], expected);
            // A lost delete reply is unknown, never reported as safe failure.
            if action == "profile-delete" {
                continue;
            }
            let response = success_response(request["id"].as_str().unwrap(), 4, json!({"schemaVersion":1,"instanceId":"instance-1","operationId":"operation-1","action":action,"applied":true})).unwrap();
            stream
                .write_all(&encode_response(&response).unwrap())
                .unwrap();
        }
    });
    for (action, input) in [
        ("profile-rename", format!("{id}\nPrivate label\n")),
        ("profile-favorite", format!("{id}\non")),
        ("profile-delete", id.to_owned()),
    ] {
        let output = private_invoke(&base, action, input.as_bytes(), &[]);
        assert_eq!(
            output.status.code(),
            Some(if action == "profile-delete" { 73 } else { 0 })
        );
        for bytes in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(bytes);
            assert!(!text.contains("Private label"));
            assert!(!text.contains(id));
        }
    }
    worker.join().unwrap();
    // No listener remains: these must fail locally with exit 2, not a transport
    // outcome. Invalid UTF-8, framing, oversize and private argv are rejected.
    for (action, input, extra) in [
        ("profile-rename", Vec::new(), vec![]),
        (
            "profile-rename",
            format!("{id}\n{}", "x".repeat(321)).into_bytes(),
            vec![],
        ),
        ("profile-rename", vec![0xff], vec![]),
        (
            "profile-rename",
            format!("{id}\nPrivate label\nextra").into_bytes(),
            vec![],
        ),
        (
            "profile-favorite",
            format!("{id}\ntrue").into_bytes(),
            vec![],
        ),
        (
            "profile-delete",
            format!("{id}\nextra").into_bytes(),
            vec![],
        ),
        (
            "profile-delete",
            id.as_bytes().to_vec(),
            vec!["Private label"],
        ),
    ] {
        let output = private_invoke(&base, action, &input, &extra);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains(id));
        assert!(!stderr.contains("Private label"));
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn fixed_plugin_cli_maps_actions_and_reports_lost_reply_as_unknown() {
    let base = test_temp::directory("action-cli").unwrap();
    let directory = base.join("omavless");
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory.join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    let worker = thread::spawn(move || {
        for (index, action) in ["disconnect", "mode", "connect", "disconnect"]
            .into_iter()
            .enumerate()
        {
            let (mut stream, _) = listener.accept().unwrap();
            let request =
                decode_request(&read_unary_frame(&mut stream, FrameKind::Request).unwrap())
                    .unwrap();
            assert_eq!(request["method"], "plugin.action");
            assert_eq!(request["params"]["action"], action);
            assert_eq!(request["params"]["instanceId"], "instance-1");
            assert_eq!(request["params"]["operationId"], "operation-1");
            assert_eq!(request["params"]["expectedRevision"], 3);
            if action == "connect" {
                assert_eq!(request["params"]["mode"], "rule");
            }
            // The fourth call intentionally loses the acknowledgement.
            if index == 3 {
                continue;
            }
            let response = success_response(request["id"].as_str().unwrap(), 4, json!({"schemaVersion":1,"instanceId":"instance-1","operationId":"operation-1","action":action,"applied":true})).unwrap();
            stream
                .write_all(&encode_response(&response).unwrap())
                .unwrap();
        }
    });
    let invoke = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_omavless"))
            .env("XDG_RUNTIME_DIR", &base)
            .args(extra)
            .output()
            .unwrap()
    };
    for args in [
        vec!["plugin", "disconnect", "instance-1", "3", "operation-1"],
        vec!["plugin", "mode", "instance-1", "3", "operation-1", "global"],
        vec![
            "plugin",
            "connect",
            "instance-1",
            "3",
            "operation-1",
            "00000000-0000-4000-8000-000000000001",
            "rule",
        ],
    ] {
        assert!(invoke(&args).status.success());
    }
    let unknown = invoke(&["plugin", "disconnect", "instance-1", "3", "operation-1"]);
    assert_eq!(unknown.status.code(), Some(73));
    assert!(unknown.stdout.is_empty());
    assert!(
        !String::from_utf8(unknown.stderr)
            .unwrap()
            .contains("operation-1")
    );
    worker.join().unwrap();
    let invalid = invoke(&[
        "plugin",
        "disconnect",
        "instance-1",
        "3",
        "operation-1",
        "extra",
    ]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    fs::remove_dir_all(base).unwrap();
}
