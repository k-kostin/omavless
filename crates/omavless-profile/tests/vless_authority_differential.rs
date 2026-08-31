// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless::{VlessAuthorityError, parse_vless_authority_bytes};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const UUID: &str = "11111111-1111-4111-8111-111111111111";

#[derive(Debug)]
struct Case {
    id: &'static str,
    input: Vec<u8>,
    classification: &'static str,
    host_kind: &'static str,
    standard_https_port: bool,
    label_kind: &'static str,
    label_sanitized: bool,
    label_truncated: bool,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    host_kind: String,
    standard_https_port: bool,
    label_kind: String,
    label_sanitized: bool,
    label_truncated: bool,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn accepted(
    id: &'static str,
    input: impl Into<Vec<u8>>,
    host_kind: &'static str,
    standard_https_port: bool,
    label_kind: &'static str,
    label_sanitized: bool,
    label_truncated: bool,
) -> Case {
    Case {
        id,
        input: input.into(),
        classification: "accepted",
        host_kind,
        standard_https_port,
        label_kind,
        label_sanitized,
        label_truncated,
    }
}

fn rejected(id: &'static str, input: impl Into<Vec<u8>>, classification: &'static str) -> Case {
    Case {
        id,
        input: input.into(),
        classification,
        host_kind: "none",
        standard_https_port: false,
        label_kind: "none",
        label_sanitized: false,
        label_truncated: false,
    }
}

fn uri(user: &str, authority: &str, fragment: &str) -> String {
    format!("vless://{user}@{authority}?security=none#{fragment}")
}

fn cases() -> Vec<Case> {
    let max_prefix = format!("vless://{UUID}@example.invalid:443#");
    let max_uri = max_prefix.clone() + &"x".repeat(16 * 1024 - max_prefix.len());
    let oversized_uri = max_uri.clone() + "x";
    vec![
        accepted(
            "dns-basic",
            uri(UUID, "example.invalid:443", ""),
            "dns",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "uppercase-scheme-and-host",
            format!("VLESS://{UUID}@EXAMPLE.INVALID:443"),
            "dns",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "ipv4-other-port",
            uri(UUID, "192.0.2.1:8443", "Node"),
            "ipv4",
            false,
            "ascii",
            false,
            false,
        ),
        accepted(
            "ipv6",
            uri(UUID, "[2001:db8::1]:443", ""),
            "ipv6",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "percent-encoded-label",
            uri(UUID, "example.invalid:443", "A%20%26%20B"),
            "dns",
            true,
            "ascii",
            false,
            false,
        ),
        accepted(
            "unicode-label",
            uri(UUID, "example.invalid:443", "Метка"),
            "dns",
            true,
            "unicode",
            false,
            false,
        ),
        accepted(
            "control-and-space-label",
            uri(UUID, "example.invalid:443", "%00%20Node%20"),
            "dns",
            true,
            "ascii",
            true,
            false,
        ),
        accepted(
            "long-label",
            uri(UUID, "example.invalid:443", &"x".repeat(81)),
            "dns",
            true,
            "ascii",
            false,
            true,
        ),
        accepted(
            "lossy-percent-utf8",
            uri(UUID, "example.invalid:443", "bad%C3"),
            "dns",
            true,
            "unicode",
            false,
            false,
        ),
        accepted(
            "literal-bad-percent",
            uri(UUID, "example.invalid:443", "bad%ZZ"),
            "dns",
            true,
            "ascii",
            false,
            false,
        ),
        accepted(
            "compact-uuid",
            uri(&UUID.replace('-', ""), "example.invalid:443", ""),
            "dns",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "braced-uuid",
            uri(&format!("{{{UUID}}}"), "example.invalid:443", ""),
            "dns",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "percent-encoded-uuid",
            uri(&UUID.replace('-', "%2D"), "example.invalid:443", ""),
            "dns",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "surrounding-input",
            format!(
                "documentation\n{}\tignored",
                uri(UUID, "example.invalid:443", "")
            ),
            "dns",
            true,
            "none",
            false,
            false,
        ),
        accepted(
            "path-before-query",
            format!("vless://{UUID}@example.invalid:443/share?security=none#Node"),
            "dns",
            true,
            "ascii",
            false,
            false,
        ),
        accepted("max-uri", max_uri, "dns", true, "ascii", false, true),
        rejected("empty", Vec::new(), "missing_vless_link"),
        rejected(
            "ordinary-text",
            b"ordinary text".to_vec(),
            "missing_vless_link",
        ),
        rejected(
            "password-field",
            format!("vless://{UUID}:private-password@example.invalid:443"),
            "password_not_allowed",
        ),
        rejected(
            "invalid-uuid",
            "vless://private-user@example.invalid:443",
            "invalid_user_id",
        ),
        rejected(
            "missing-at-sign",
            format!("vless://{UUID}:443"),
            "invalid_user_id",
        ),
        rejected(
            "missing-host",
            format!("vless://{UUID}@:443"),
            "missing_server_port",
        ),
        rejected(
            "missing-port",
            format!("vless://{UUID}@example.invalid"),
            "missing_server_port",
        ),
        rejected(
            "zero-port",
            format!("vless://{UUID}@example.invalid:0"),
            "missing_server_port",
        ),
        rejected(
            "oversized-port",
            format!("vless://{UUID}@example.invalid:65536"),
            "invalid_link",
        ),
        rejected(
            "nonnumeric-port",
            format!("vless://{UUID}@example.invalid:private-port"),
            "invalid_link",
        ),
        rejected(
            "unmatched-ipv6-bracket",
            format!("vless://{UUID}@[2001:db8::1:443"),
            "invalid_link",
        ),
        rejected(
            "invalid-ipv6",
            format!("vless://{UUID}@[not-ipv6]:443"),
            "invalid_link",
        ),
        rejected("invalid-utf8", vec![0xff], "invalid_input"),
        rejected("oversized-uri", oversized_uri, "invalid_input"),
        rejected(
            "oversized-input",
            vec![b'x'; 64 * 1024 + 1],
            "invalid_input",
        ),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_authority_bytes(&case.input) {
        Ok(authority) => {
            let facts = authority.public_facts();
            Outcome {
                accepted: true,
                classification: "accepted".to_owned(),
                host_kind: facts.host_kind.as_str().to_owned(),
                standard_https_port: facts.standard_https_port,
                label_kind: facts.label_kind.as_str().to_owned(),
                label_sanitized: facts.label_sanitized,
                label_truncated: facts.label_truncated,
            }
        }
        Err(error) => Outcome {
            accepted: false,
            classification: error.code().to_owned(),
            host_kind: "none".to_owned(),
            standard_https_port: false,
            label_kind: "none".to_owned(),
            label_sanitized: false,
            label_truncated: false,
        },
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_authority_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python VLESS authority adapter");
    child
        .stdin
        .take()
        .expect("adapter stdin")
        .write_all(&case.input)
        .expect("adapter input");
    let output = child.wait_with_output().expect("adapter output");
    assert!(
        output.status.success(),
        "Python adapter failed for {}",
        case.id
    );
    let outcome = serde_json::from_slice(&output.stdout).expect("sanitized adapter JSON");
    (outcome, output.stdout, output.stderr)
}

fn report(values: &[(String, Outcome)]) -> Report {
    Report {
        api: "omavless.parity".to_owned(),
        version: 1,
        suite: "vless-authority-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("accepted".to_owned(), PublicValue::Bool(outcome.accepted)),
                    (
                        "host_kind".to_owned(),
                        PublicValue::Text(outcome.host_kind.clone()),
                    ),
                    (
                        "standard_https_port".to_owned(),
                        PublicValue::Bool(outcome.standard_https_port),
                    ),
                    (
                        "label_kind".to_owned(),
                        PublicValue::Text(outcome.label_kind.clone()),
                    ),
                    (
                        "label_sanitized".to_owned(),
                        PublicValue::Bool(outcome.label_sanitized),
                    ),
                    (
                        "label_truncated".to_owned(),
                        PublicValue::Bool(outcome.label_truncated),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_vless_authority_match() {
    let cases = cases();
    assert_eq!(cases.len(), 31);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        let expected = Outcome {
            accepted: case.classification == "accepted",
            classification: case.classification.to_owned(),
            host_kind: case.host_kind.to_owned(),
            standard_https_port: case.standard_https_port,
            label_kind: case.label_kind.to_owned(),
            label_sanitized: case.label_sanitized,
            label_truncated: case.label_truncated,
        };
        assert_eq!(rust_outcome, expected, "Rust {}", case.id);
        assert_eq!(python_outcome, expected, "Python {}", case.id);
        for marker in [b"private".as_slice(), UUID.as_bytes()] {
            assert!(!stdout.windows(marker.len()).any(|part| part == marker));
            assert!(!stderr.windows(marker.len()).any(|part| part == marker));
        }
        rust.push((case.id.to_owned(), rust_outcome));
        python.push((case.id.to_owned(), python_outcome));
    }
    let summary = compare_reports(&report(&python), &report(&rust)).expect("compatible reports");
    assert!(summary.matched);
    assert_eq!(summary.case_count, cases.len());
    assert_eq!(summary.mismatch_count, 0);
}

#[test]
fn authority_error_catalog_is_fixed_and_safe() {
    let errors = [
        VlessAuthorityError::InvalidInput,
        VlessAuthorityError::MissingLink,
        VlessAuthorityError::InvalidLink,
        VlessAuthorityError::PasswordNotAllowed,
        VlessAuthorityError::InvalidUserId,
        VlessAuthorityError::MissingServerPort,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("private"));
    }
}
