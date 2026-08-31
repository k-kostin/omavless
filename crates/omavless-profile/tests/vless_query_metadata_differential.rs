// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless::VlessAuthorityError;
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
    transport: String,
    security: String,
    allow_insecure: bool,
    xhttp_mode: String,
    provider_metadata_present: bool,
    non_xhttp_mode_metadata: bool,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn uri(query: &str) -> String {
    let separator = if query.is_empty() { "" } else { "?" };
    format!("vless://{UUID}@example.invalid:443{separator}{query}#Node")
}

macro_rules! accepted {
    (
        $id:expr,
        $query:expr,
        $transport:expr,
        $security:expr,
        $allow_insecure:expr,
        $xhttp_mode:expr,
        $provider_metadata_present:expr,
        $non_xhttp_mode_metadata:expr
        $(,)?
    ) => {
        Case {
            id: $id,
            input: uri($query).into_bytes(),
            expected: Outcome {
                accepted: true,
                classification: "accepted".to_owned(),
                transport: $transport.to_owned(),
                security: $security.to_owned(),
                allow_insecure: $allow_insecure,
                xhttp_mode: $xhttp_mode.to_owned(),
                provider_metadata_present: $provider_metadata_present,
                non_xhttp_mode_metadata: $non_xhttp_mode_metadata,
            },
        }
    };
}

fn rejected(id: &'static str, input: impl Into<Vec<u8>>, classification: &str) -> Case {
    Case {
        id,
        input: input.into(),
        expected: Outcome {
            accepted: false,
            classification: classification.to_owned(),
            transport: "none".to_owned(),
            security: "none".to_owned(),
            allow_insecure: false,
            xhttp_mode: "none".to_owned(),
            provider_metadata_present: false,
            non_xhttp_mode_metadata: false,
        },
    }
}

fn cases() -> Vec<Case> {
    let too_many = (0..129)
        .map(|index| format!("host=value{index}"))
        .collect::<Vec<_>>()
        .join("&");
    let oversized_prefix = format!("vless://{UUID}@example.invalid:443#");
    let oversized = oversized_prefix.clone() + &"x".repeat(16 * 1024 + 1 - oversized_prefix.len());
    vec![
        accepted!("defaults", "", "tcp", "none", false, "none", false, false),
        accepted!(
            "blank-defaults",
            "type=&security=&allowInsecure=",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "raw-is-tcp",
            "type=raw",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "tcp-uppercase",
            "type=TCP",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "ws-tls",
            "type=WS&security=TLS",
            "ws",
            "tls",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "http",
            "type=http",
            "http",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!("h2", "type=h2", "h2", "none", false, "none", false, false),
        accepted!(
            "grpc",
            "type=grpc",
            "grpc",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "xhttp-default",
            "type=xhttp",
            "xhttp",
            "none",
            false,
            "default",
            false,
            false,
        ),
        accepted!(
            "xhttp-auto",
            "type=xhttp&mode=auto",
            "xhttp",
            "none",
            false,
            "auto",
            false,
            false,
        ),
        accepted!(
            "xhttp-stream-one",
            "type=xhttp&mode=stream-one",
            "xhttp",
            "none",
            false,
            "stream-one",
            false,
            false,
        ),
        accepted!(
            "xhttp-stream-up",
            "type=xhttp&mode=stream-up",
            "xhttp",
            "none",
            false,
            "stream-up",
            false,
            false,
        ),
        accepted!(
            "xhttp-packet-up",
            "type=xhttp&mode=packet-up",
            "xhttp",
            "none",
            false,
            "packet-up",
            false,
            false,
        ),
        accepted!(
            "bool-one",
            "allowInsecure=1",
            "tcp",
            "none",
            true,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-true",
            "allowInsecure=true",
            "tcp",
            "none",
            true,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-yes",
            "allowInsecure=yes",
            "tcp",
            "none",
            true,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-on",
            "allowInsecure=on",
            "tcp",
            "none",
            true,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-zero",
            "allowInsecure=0",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-false",
            "allowInsecure=false",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-no",
            "allowInsecure=no",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-off",
            "allowInsecure=off",
            "tcp",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "bool-alias",
            "skip-cert-verify=true",
            "tcp",
            "none",
            true,
            "none",
            false,
            false,
        ),
        accepted!(
            "percent-key",
            "t%79pe=ws",
            "ws",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "uppercase-key",
            "TYPE=ws",
            "ws",
            "none",
            false,
            "none",
            false,
            false,
        ),
        accepted!(
            "provider-concurrency",
            "concurrency=two+streams",
            "tcp",
            "none",
            false,
            "none",
            true,
            false,
        ),
        accepted!(
            "provider-block",
            "x-durev-block=value",
            "tcp",
            "none",
            false,
            "none",
            true,
            false,
        ),
        accepted!(
            "provider-priority",
            "x-durev-prio=value",
            "tcp",
            "none",
            false,
            "none",
            true,
            false,
        ),
        accepted!(
            "non-xhttp-mode",
            "type=grpc&mode=gun",
            "grpc",
            "none",
            false,
            "none",
            false,
            true,
        ),
        accepted!(
            "empty-segments",
            "&type=ws&&security=tls&",
            "ws",
            "tls",
            false,
            "none",
            false,
            false,
        ),
        Case {
            id: "surrounding-input",
            input: format!("documentation\n{}\tignored", uri("type=ws")).into_bytes(),
            expected: Outcome {
                accepted: true,
                classification: "accepted".to_owned(),
                transport: "ws".to_owned(),
                security: "none".to_owned(),
                allow_insecure: false,
                xhttp_mode: "none".to_owned(),
                provider_metadata_present: false,
                non_xhttp_mode_metadata: false,
            },
        },
        rejected("empty", Vec::new(), "missing_vless_link"),
        rejected("invalid-input-utf8", vec![0xff], "invalid_input"),
        rejected("invalid-query-utf8", uri("host=%C3"), "invalid_query"),
        rejected("too-many-fields", uri(&too_many), "invalid_query"),
        rejected(
            "duplicate-field",
            uri("type=ws&type=grpc"),
            "duplicate_fields",
        ),
        rejected(
            "duplicate-casefold",
            uri("TYPE=ws&type=grpc"),
            "duplicate_fields",
        ),
        rejected(
            "unsupported-field",
            uri("private-field=value"),
            "unsupported_fields",
        ),
        rejected(
            "transport-alias-conflict",
            uri("type=tcp&network=tcp"),
            "conflicting_aliases",
        ),
        rejected(
            "boolean-alias-conflict",
            uri("allowInsecure=1&skip-cert-verify=1"),
            "conflicting_aliases",
        ),
        rejected(
            "unsupported-transport",
            uri("type=private-transport"),
            "unsupported_transport",
        ),
        rejected(
            "unsupported-security",
            uri("security=private-security"),
            "unsupported_security",
        ),
        rejected(
            "invalid-boolean",
            uri("allowInsecure=private-boolean"),
            "invalid_boolean",
        ),
        rejected(
            "unsupported-xhttp-mode",
            uri("type=xhttp&mode=private-mode"),
            "unsupported_xhttp_mode",
        ),
        rejected(
            "provider-metadata-long",
            uri(&format!("concurrency={}", "x".repeat(129))),
            "invalid_provider_metadata",
        ),
        rejected(
            "provider-metadata-control",
            uri("x-durev-block=bad%00value"),
            "invalid_provider_metadata",
        ),
        rejected(
            "missing-port",
            format!("vless://{UUID}@example.invalid?type=ws"),
            "missing_server_port",
        ),
        rejected("oversized-uri", oversized, "invalid_input"),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_query_metadata_bytes(&case.input) {
        Ok(metadata) => Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            transport: metadata.transport.as_str().to_owned(),
            security: metadata.security.as_str().to_owned(),
            allow_insecure: metadata.allow_insecure,
            xhttp_mode: metadata
                .xhttp_mode
                .map_or("none", |mode| mode.as_str())
                .to_owned(),
            provider_metadata_present: metadata.provider_metadata_present,
            non_xhttp_mode_metadata: metadata.non_xhttp_mode_metadata,
        },
        Err(error) => Outcome {
            accepted: false,
            classification: error.code().to_owned(),
            transport: "none".to_owned(),
            security: "none".to_owned(),
            allow_insecure: false,
            xhttp_mode: "none".to_owned(),
            provider_metadata_present: false,
            non_xhttp_mode_metadata: false,
        },
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_query_metadata_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python VLESS query metadata adapter");
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
        suite: "vless-query-metadata-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("accepted".to_owned(), PublicValue::Bool(outcome.accepted)),
                    (
                        "transport".to_owned(),
                        PublicValue::Text(outcome.transport.clone()),
                    ),
                    (
                        "security".to_owned(),
                        PublicValue::Text(outcome.security.clone()),
                    ),
                    (
                        "allow_insecure".to_owned(),
                        PublicValue::Bool(outcome.allow_insecure),
                    ),
                    (
                        "xhttp_mode".to_owned(),
                        PublicValue::Text(outcome.xhttp_mode.clone()),
                    ),
                    (
                        "provider_metadata_present".to_owned(),
                        PublicValue::Bool(outcome.provider_metadata_present),
                    ),
                    (
                        "non_xhttp_mode_metadata".to_owned(),
                        PublicValue::Bool(outcome.non_xhttp_mode_metadata),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_vless_query_metadata_match() {
    let cases = cases();
    assert_eq!(cases.len(), 47);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(rust_outcome, case.expected, "Rust {}", case.id);
        assert_eq!(python_outcome, case.expected, "Python {}", case.id);
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
fn query_error_catalog_is_fixed_and_safe() {
    let errors = [
        VlessQueryError::Authority(VlessAuthorityError::InvalidInput),
        VlessQueryError::InvalidQuery,
        VlessQueryError::DuplicateFields,
        VlessQueryError::UnsupportedFields,
        VlessQueryError::InvalidProviderMetadata,
        VlessQueryError::ConflictingAliases,
        VlessQueryError::UnsupportedTransport,
        VlessQueryError::UnsupportedSecurity,
        VlessQueryError::InvalidBoolean,
        VlessQueryError::UnsupportedXhttpMode,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("private"));
    }
}
