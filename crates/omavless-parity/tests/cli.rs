// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/parity_cases")
        .join(name)
}

fn temporary_report() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omavless-parity-{}-{nonce}.json",
        std::process::id()
    ))
}

fn remove_temporary(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn checked_in_smoke_reports_match() {
    let output = Command::new(env!("CARGO_BIN_EXE_omavless-parity"))
        .args([
            "compare",
            fixture("r0-reference.json").to_str().expect("fixture path"),
            fixture("r0-candidate.json").to_str().expect("fixture path"),
        ])
        .output()
        .expect("parity command");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(stdout.contains("\"matched\":true"));
    assert!(!stdout.contains("workspace-ready"));
}

#[test]
fn mismatch_output_never_echoes_semantic_facts() {
    let candidate_path = temporary_report();
    let private_marker = "private-marker-never-print";
    let candidate = format!(
        r#"{{"api":"omavless.parity","version":1,"suite":"r0-smoke","cases":[{{"id":"workspace-ready","classification":"rejected","facts":{{"marker":"{private_marker}"}}}}]}}"#
    );
    fs::write(&candidate_path, candidate).expect("temporary candidate");
    let output = Command::new(env!("CARGO_BIN_EXE_omavless-parity"))
        .args([
            "compare",
            fixture("r0-reference.json").to_str().expect("fixture path"),
            candidate_path.to_str().expect("temporary path"),
        ])
        .output()
        .expect("parity command");
    remove_temporary(&candidate_path);

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(stdout.contains("workspace-ready"));
    assert!(!stdout.contains(private_marker));
    assert!(!stderr.contains(private_marker));
    assert!(!stdout.contains("rejected"));
}

#[test]
fn malformed_private_input_is_never_echoed() {
    let candidate_path = temporary_report();
    let private_marker = "https://private-marker.invalid/credential";
    fs::write(
        &candidate_path,
        format!(r#"{{\"secret\":\"{private_marker}\""#),
    )
    .expect("temporary malformed candidate");
    let output = Command::new(env!("CARGO_BIN_EXE_omavless-parity"))
        .args([
            "compare",
            fixture("r0-reference.json").to_str().expect("fixture path"),
            candidate_path.to_str().expect("temporary path"),
        ])
        .output()
        .expect("parity command");
    remove_temporary(&candidate_path);

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(!stdout.contains(private_marker));
    assert!(!stderr.contains(private_marker));
    assert!(!stderr.contains("secret"));
}

#[cfg(unix)]
#[test]
fn symlink_input_is_rejected_without_printing_its_path() {
    use std::os::unix::fs::symlink;

    let link_path = temporary_report();
    symlink(fixture("r0-reference.json"), &link_path).expect("temporary symlink");
    let output = Command::new(env!("CARGO_BIN_EXE_omavless-parity"))
        .args([
            "compare",
            link_path.to_str().expect("temporary path"),
            fixture("r0-candidate.json").to_str().expect("fixture path"),
        ])
        .output()
        .expect("parity command");
    remove_temporary(&link_path);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(!stderr.contains(link_path.to_str().expect("temporary path")));
}
