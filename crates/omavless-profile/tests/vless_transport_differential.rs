// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless_query::{VlessQueryError, parse_vless_query_metadata_bytes};
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
    expected: Outcome,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    path_default: bool,
    path_starts_with_slash: bool,
    path_non_ascii: bool,
    host_present: bool,
    host_non_ascii: bool,
    service_name_present: bool,
    service_name_non_ascii: bool,
    fingerprint_present: bool,
    fingerprint_non_ascii: bool,
    alpn_count: usize,
    alpn_non_ascii: bool,
}

impl Outcome {
    fn accepted() -> Self {
        Self {
            accepted: true,
            classification: "accepted".to_owned(),
            path_default: true,
            path_starts_with_slash: true,
            path_non_ascii: false,
            host_present: false,
            host_non_ascii: false,
            service_name_present: false,
            service_name_non_ascii: false,
            fingerprint_present: false,
            fingerprint_non_ascii: false,
            alpn_count: 0,
            alpn_non_ascii: false,
        }
    }

    fn rejected(classification: &str) -> Self {
        Self {
            accepted: false,
            classification: classification.to_owned(),
            path_default: false,
            path_starts_with_slash: false,
            path_non_ascii: false,
            host_present: false,
            host_non_ascii: false,
            service_name_present: false,
            service_name_non_ascii: false,
            fingerprint_present: false,
            fingerprint_non_ascii: false,
            alpn_count: 0,
            alpn_non_ascii: false,
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn uri(query: &str) -> String {
    let separator = if query.is_empty() { "" } else { "?" };
    format!("vless://{UUID}@example.invalid:443{separator}{query}#Node")
}

fn accepted(id: &'static str, query: &str, update: impl FnOnce(&mut Outcome)) -> Case {
    let mut expected = Outcome::accepted();
    update(&mut expected);
    Case {
        id,
        input: uri(query).into_bytes(),
        expected,
    }
}

fn rejected(id: &'static str, input: impl Into<Vec<u8>>, classification: &str) -> Case {
    Case {
        id,
        input: input.into(),
        expected: Outcome::rejected(classification),
    }
}

fn cases() -> Vec<Case> {
    vec![
        accepted("defaults", "", |_| {}),
        accepted("blank-path-default", "path=", |_| {}),
        accepted("explicit-root-path", "path=%2F", |_| {}),
        accepted("slash-path", "path=%2Fedge", |outcome| {
            outcome.path_default = false;
        }),
        accepted("relative-path", "path=edge", |outcome| {
            outcome.path_default = false;
            outcome.path_starts_with_slash = false;
        }),
        accepted("double-decoded-root-path", "path=%252F", |_| {}),
        accepted(
            "double-decoded-unicode-path",
            "path=%25E2%2582%25AC",
            |outcome| {
                outcome.path_default = false;
                outcome.path_starts_with_slash = false;
                outcome.path_non_ascii = true;
            },
        ),
        accepted("lossy-second-path-decode", "path=%25C3", |outcome| {
            outcome.path_default = false;
            outcome.path_starts_with_slash = false;
            outcome.path_non_ascii = true;
        }),
        accepted("form-plus-path", "path=+", |outcome| {
            outcome.path_default = false;
            outcome.path_starts_with_slash = false;
        }),
        accepted(
            "ascii-host",
            "type=ws&host=private-host.invalid",
            |outcome| {
                outcome.host_present = true;
            },
        ),
        accepted(
            "unicode-host",
            "type=h2&host=%D1%82%D0%B5%D1%81%D1%82",
            |outcome| {
                outcome.host_present = true;
                outcome.host_non_ascii = true;
            },
        ),
        accepted(
            "camel-service-name",
            "type=grpc&serviceName=private-service",
            |outcome| {
                outcome.service_name_present = true;
            },
        ),
        accepted(
            "hyphen-service-name",
            "type=grpc&service-name=%D1%82%D0%B5%D1%81%D1%82",
            |outcome| {
                outcome.service_name_present = true;
                outcome.service_name_non_ascii = true;
            },
        ),
        accepted(
            "fp-alias",
            "security=tls&fp=private-fingerprint",
            |outcome| {
                outcome.fingerprint_present = true;
            },
        ),
        accepted(
            "fingerprint-alias",
            "security=tls&fingerprint=%D1%82%D0%B5%D1%81%D1%82",
            |outcome| {
                outcome.fingerprint_present = true;
                outcome.fingerprint_non_ascii = true;
            },
        ),
        accepted(
            "client-fingerprint-alias",
            "security=tls&client-fingerprint=chrome",
            |outcome| {
                outcome.fingerprint_present = true;
            },
        ),
        accepted(
            "alpn-list",
            "security=tls&alpn=h2%2Chttp%2F1.1",
            |outcome| {
                outcome.alpn_count = 2;
            },
        ),
        accepted(
            "alpn-trim-empty",
            "security=tls&alpn=%20h2%20%2C%2C%20http%2F1.1%20%2C",
            |outcome| {
                outcome.alpn_count = 2;
            },
        ),
        accepted(
            "alpn-unicode",
            "security=tls&alpn=h2%2C%D1%82%D0%B5%D1%81%D1%82",
            |outcome| {
                outcome.alpn_count = 2;
                outcome.alpn_non_ascii = true;
            },
        ),
        accepted("tcp-header-blank", "headerType=", |_| {}),
        accepted("tcp-header-none", "headerType=none", |_| {}),
        accepted("tcp-header-uppercase-none", "header-type=NONE", |_| {}),
        accepted(
            "non-tcp-header-ignored",
            "type=ws&headerType=private-header",
            |_| {},
        ),
        accepted(
            "control-host-remains-private",
            "type=ws&host=%00private",
            |outcome| {
                outcome.host_present = true;
            },
        ),
        accepted(
            "combined-private-options",
            "type=ws&security=tls&path=%2Fprivate&host=private-host&serviceName=private-service&fp=private-fingerprint&alpn=h2%2Chttp%2F1.1",
            |outcome| {
                outcome.path_default = false;
                outcome.host_present = true;
                outcome.service_name_present = true;
                outcome.fingerprint_present = true;
                outcome.alpn_count = 2;
            },
        ),
        accepted(
            "xhttp-path-without-extra",
            "type=xhttp&path=%2Fupload",
            |outcome| {
                outcome.path_default = false;
            },
        ),
        accepted(
            "http-host-and-path",
            "type=http&host=private-host&path=private",
            |outcome| {
                outcome.path_default = false;
                outcome.path_starts_with_slash = false;
                outcome.host_present = true;
            },
        ),
        rejected(
            "unsupported-tcp-header",
            uri("headerType=private-header"),
            "unsupported_tcp_header",
        ),
        rejected(
            "tcp-header-alias-conflict",
            uri("headerType=none&header-type=none"),
            "conflicting_aliases",
        ),
        rejected(
            "service-name-alias-conflict",
            uri("type=grpc&serviceName=one&service-name=two"),
            "conflicting_aliases",
        ),
        rejected(
            "fingerprint-alias-conflict",
            uri("security=tls&fp=one&fingerprint=two"),
            "conflicting_aliases",
        ),
        rejected(
            "three-fingerprint-aliases",
            uri("security=tls&fp=one&fingerprint=two&client-fingerprint=three"),
            "conflicting_aliases",
        ),
        rejected(
            "invalid-first-path-decode",
            uri("path=%C3"),
            "invalid_query",
        ),
        rejected(
            "invalid-first-host-decode",
            uri("type=ws&host=%C3"),
            "invalid_query",
        ),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_query_metadata_bytes(&case.input) {
        Ok(metadata) => {
            let facts = metadata.transport_options;
            Outcome {
                accepted: true,
                classification: "accepted".to_owned(),
                path_default: facts.path_default,
                path_starts_with_slash: facts.path_starts_with_slash,
                path_non_ascii: facts.path_non_ascii,
                host_present: facts.host_present,
                host_non_ascii: facts.host_non_ascii,
                service_name_present: facts.service_name_present,
                service_name_non_ascii: facts.service_name_non_ascii,
                fingerprint_present: facts.fingerprint_present,
                fingerprint_non_ascii: facts.fingerprint_non_ascii,
                alpn_count: facts.alpn_count,
                alpn_non_ascii: facts.alpn_non_ascii,
            }
        }
        Err(error) => Outcome::rejected(error.code()),
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_transport_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python transport adapter");
    child
        .stdin
        .take()
        .expect("adapter stdin")
        .write_all(&case.input)
        .expect("write adapter input");
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
        suite: "vless-transport-options-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    (
                        "path_default".to_owned(),
                        PublicValue::Bool(outcome.path_default),
                    ),
                    (
                        "path_starts_with_slash".to_owned(),
                        PublicValue::Bool(outcome.path_starts_with_slash),
                    ),
                    (
                        "path_non_ascii".to_owned(),
                        PublicValue::Bool(outcome.path_non_ascii),
                    ),
                    (
                        "host_present".to_owned(),
                        PublicValue::Bool(outcome.host_present),
                    ),
                    (
                        "host_non_ascii".to_owned(),
                        PublicValue::Bool(outcome.host_non_ascii),
                    ),
                    (
                        "service_name_present".to_owned(),
                        PublicValue::Bool(outcome.service_name_present),
                    ),
                    (
                        "service_name_non_ascii".to_owned(),
                        PublicValue::Bool(outcome.service_name_non_ascii),
                    ),
                    (
                        "fingerprint_present".to_owned(),
                        PublicValue::Bool(outcome.fingerprint_present),
                    ),
                    (
                        "fingerprint_non_ascii".to_owned(),
                        PublicValue::Bool(outcome.fingerprint_non_ascii),
                    ),
                    (
                        "alpn_count".to_owned(),
                        PublicValue::Integer(outcome.alpn_count as i64),
                    ),
                    (
                        "alpn_non_ascii".to_owned(),
                        PublicValue::Bool(outcome.alpn_non_ascii),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_vless_transport_options_match() {
    let cases = cases();
    assert_eq!(cases.len(), 34);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(rust_outcome, case.expected, "Rust {}", case.id);
        assert_eq!(python_outcome, case.expected, "Python {}", case.id);
        for marker in [
            b"private".as_slice(),
            UUID.as_bytes(),
            b"vless://".as_slice(),
        ] {
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
fn transport_error_catalog_is_fixed_and_safe() {
    let error = VlessQueryError::UnsupportedTcpHeader;
    assert_eq!(error.code(), "unsupported_tcp_header");
    assert!(error.to_string().len() <= 80);
    assert!(!error.to_string().contains("private"));
}
