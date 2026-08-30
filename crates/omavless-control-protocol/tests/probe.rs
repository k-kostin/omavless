// SPDX-License-Identifier: MIT

use serde_json::{Value, json};
use std::process::{Command, Stdio};

fn probe(arguments: &[&str], frame: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omavless-control-protocol-probe"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Rust protocol probe");
    std::io::Write::write_all(child.stdin.as_mut().expect("probe stdin"), frame)
        .expect("probe input");
    child.wait_with_output().expect("probe output")
}

#[test]
fn hello_emits_only_the_fixed_v1_request() {
    let output = probe(&["hello"], b"");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).expect("hello JSON");
    assert_eq!(
        value,
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "hello-1",
            "method": "system.hello",
            "params": {"versions": [1]},
        })
    );
    assert!(output.stdout.ends_with(b"\n"));
}

#[test]
fn request_and_response_validation_are_end_to_end() {
    let request = b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"status-1\",\"method\":\"status.get\",\"params\":{}}\n";
    let response = b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"status-1\",\"ok\":true,\"revision\":4,\"result\":{}}\n";
    let request_output = probe(&["request"], request);
    let response_output = probe(&["response"], response);
    assert_eq!(request_output.stdout, b"VALID request\n");
    assert_eq!(response_output.stdout, b"VALID response\n");
    assert!(request_output.status.success());
    assert!(response_output.status.success());
    assert!(request_output.stderr.is_empty());
    assert!(response_output.stderr.is_empty());
}

#[test]
fn invalid_private_input_is_never_echoed() {
    let marker = b"vless://private.invalid?password=secret";
    let mut frame = b"{broken:".to_vec();
    frame.extend_from_slice(marker);
    frame.extend_from_slice(b"}\n");
    let output = probe(&["request"], &frame);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        !output
            .stderr
            .windows(marker.len())
            .any(|value| value == marker)
    );
    assert!(!output.stderr.windows(8).any(|value| value == b"password"));
    assert!(output.stderr.len() <= 256);
}

#[test]
fn command_line_cannot_select_methods_or_echo_arguments() {
    let marker = "vless://private.invalid";
    let output = probe(&["hello", marker], b"");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        !output
            .stderr
            .windows(marker.len())
            .any(|value| value == marker.as_bytes())
    );
}
