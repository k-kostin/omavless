// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::{ClassificationError, Protocol, classify_protocol_bytes};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug)]
struct Case {
    id: &'static str,
    input: Vec<u8>,
    classification: &'static str,
    protocol: &'static str,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    protocol: String,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "vless",
            input: b"vless://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "vless",
        },
        Case {
            id: "trojan-uppercase",
            input: b"TROJAN://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "trojan",
        },
        Case {
            id: "hysteria2",
            input: b"hysteria2://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "hysteria2",
        },
        Case {
            id: "hy2-alias",
            input: b"hy2://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "hysteria2",
        },
        Case {
            id: "tuic",
            input: b"tuic://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "tuic",
        },
        Case {
            id: "surrounding-whitespace",
            input: b"  \n vless://opaque.invalid \t".to_vec(),
            classification: "accepted",
            protocol: "vless",
        },
        Case {
            id: "unicode-whitespace",
            input: "docs\u{2003}tuic://opaque.invalid".as_bytes().to_vec(),
            classification: "accepted",
            protocol: "tuic",
        },
        Case {
            id: "documentation-before-profile",
            input: b"https://docs.invalid/help vless://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "vless",
        },
        Case {
            id: "unsupported-before-profile",
            input: b"unknown://private.invalid hy2://opaque.invalid".to_vec(),
            classification: "accepted",
            protocol: "hysteria2",
        },
        Case {
            id: "first-supported-wins",
            input: b"trojan://first.invalid vless://second.invalid".to_vec(),
            classification: "accepted",
            protocol: "trojan",
        },
        Case {
            id: "unsupported",
            input: b"unknown://private.invalid".to_vec(),
            classification: "unsupported_protocol",
            protocol: "none",
        },
        Case {
            id: "https-only",
            input: b"https://docs.invalid".to_vec(),
            classification: "unsupported_protocol",
            protocol: "none",
        },
        Case {
            id: "ordinary-text",
            input: b"ordinary private text".to_vec(),
            classification: "missing_profile_link",
            protocol: "none",
        },
        Case {
            id: "empty",
            input: Vec::new(),
            classification: "missing_profile_link",
            protocol: "none",
        },
        Case {
            id: "bad-scheme-prefix",
            input: b"1vless://opaque.invalid".to_vec(),
            classification: "missing_profile_link",
            protocol: "none",
        },
        Case {
            id: "missing-slashes",
            input: b"vless:/opaque.invalid".to_vec(),
            classification: "missing_profile_link",
            protocol: "none",
        },
        Case {
            id: "unicode-scheme-prefix",
            input: "ſless://opaque.invalid".as_bytes().to_vec(),
            classification: "missing_profile_link",
            protocol: "none",
        },
        Case {
            id: "invalid-utf8",
            input: vec![0xff],
            classification: "invalid_input",
            protocol: "none",
        },
        Case {
            id: "max-plain-input",
            input: vec![b'x'; 64 * 1024],
            classification: "missing_profile_link",
            protocol: "none",
        },
        Case {
            id: "oversized-input",
            input: vec![b'x'; 64 * 1024 + 1],
            classification: "invalid_input",
            protocol: "none",
        },
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match classify_protocol_bytes(&case.input) {
        Ok(protocol) => Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            protocol: protocol.as_str().to_owned(),
        },
        Err(error) => Outcome {
            accepted: false,
            classification: error.code().to_owned(),
            protocol: "none".to_owned(),
        },
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/profile_classification_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python classification adapter");
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
        suite: "profile-classification-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("accepted".to_owned(), PublicValue::Bool(outcome.accepted)),
                    (
                        "protocol".to_owned(),
                        PublicValue::Text(outcome.protocol.clone()),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_profile_classification_match() {
    let cases = cases();
    assert_eq!(cases.len(), 20);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(
            rust_outcome.classification, case.classification,
            "Rust {}",
            case.id
        );
        assert_eq!(
            python_outcome.classification, case.classification,
            "Python {}",
            case.id
        );
        assert_eq!(rust_outcome.protocol, case.protocol, "Rust {}", case.id);
        assert_eq!(python_outcome.protocol, case.protocol, "Python {}", case.id);
        if case.id == "unsupported" {
            assert!(!stdout.windows(7).any(|part| part == b"private"));
            assert!(!stderr.windows(7).any(|part| part == b"private"));
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
fn error_catalog_is_fixed_and_safe() {
    let errors = [
        ClassificationError::InvalidInput,
        ClassificationError::UnsupportedProtocol,
        ClassificationError::MissingProfileLink,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("opaque"));
    }
    assert_eq!(Protocol::ALL.len(), 4);
}
