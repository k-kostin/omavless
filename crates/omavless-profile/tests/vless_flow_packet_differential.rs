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
    flow: String,
    mihomo_flow: String,
    packet_encoding: String,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn uri(query: &str) -> String {
    let separator = if query.is_empty() { "" } else { "?" };
    format!("vless://{UUID}@example.invalid:443{separator}{query}#Node")
}

fn accepted(
    id: &'static str,
    query: &str,
    flow: &str,
    mihomo_flow: &str,
    packet_encoding: &str,
) -> Case {
    Case {
        id,
        input: uri(query).into_bytes(),
        expected: Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            flow: flow.to_owned(),
            mihomo_flow: mihomo_flow.to_owned(),
            packet_encoding: packet_encoding.to_owned(),
        },
    }
}

fn rejected(id: &'static str, input: impl Into<Vec<u8>>, classification: &str) -> Case {
    Case {
        id,
        input: input.into(),
        expected: Outcome {
            accepted: false,
            classification: classification.to_owned(),
            flow: "none".to_owned(),
            mihomo_flow: "none".to_owned(),
            packet_encoding: "none".to_owned(),
        },
    }
}

fn cases() -> Vec<Case> {
    vec![
        accepted("defaults", "", "none", "none", "none"),
        accepted(
            "empty-fields",
            "flow=&packetEncoding=",
            "none",
            "none",
            "none",
        ),
        accepted(
            "vision-tls",
            "type=tcp&security=tls&flow=xtls-rprx-vision",
            "xtls-rprx-vision",
            "xtls-rprx-vision",
            "none",
        ),
        accepted(
            "vision-uppercase",
            "type=TCP&security=TLS&flow=XTLS-RPRX-VISION",
            "xtls-rprx-vision",
            "xtls-rprx-vision",
            "none",
        ),
        accepted(
            "vision-udp443",
            "type=raw&security=tls&flow=xtls-rprx-vision-udp443",
            "xtls-rprx-vision-udp443",
            "xtls-rprx-vision",
            "none",
        ),
        accepted(
            "vision-reality",
            "type=tcp&security=reality&flow=xtls-rprx-vision&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "xtls-rprx-vision",
            "xtls-rprx-vision",
            "none",
        ),
        accepted("packet-xudp", "packetEncoding=xudp", "none", "none", "xudp"),
        accepted(
            "packet-xudp-uppercase",
            "packetEncoding=XUDP",
            "none",
            "none",
            "xudp",
        ),
        accepted(
            "packetaddr",
            "packetEncoding=packetaddr",
            "none",
            "none",
            "packetaddr",
        ),
        accepted(
            "packet-hyphen-alias",
            "packet-encoding=packetaddr",
            "none",
            "none",
            "packetaddr",
        ),
        accepted(
            "combined",
            "security=tls&flow=xtls-rprx-vision&packet-encoding=xudp",
            "xtls-rprx-vision",
            "xtls-rprx-vision",
            "xudp",
        ),
        rejected(
            "unsupported-flow",
            uri("flow=private-flow"),
            "unsupported_flow",
        ),
        rejected(
            "vision-ws",
            uri("type=ws&security=tls&flow=xtls-rprx-vision"),
            "vision_requires_tcp",
        ),
        rejected(
            "vision-grpc",
            uri("type=grpc&security=tls&flow=xtls-rprx-vision"),
            "vision_requires_tcp",
        ),
        rejected(
            "vision-no-security",
            uri("type=tcp&security=none&flow=xtls-rprx-vision"),
            "vision_requires_security",
        ),
        rejected(
            "unsupported-packet",
            uri("packetEncoding=private-packet"),
            "unsupported_packet_encoding",
        ),
        rejected(
            "packet-alias-conflict",
            uri("packetEncoding=xudp&packet-encoding=xudp"),
            "conflicting_aliases",
        ),
        rejected(
            "missing-port",
            format!("vless://{UUID}@example.invalid?packetEncoding=xudp"),
            "missing_server_port",
        ),
        rejected("invalid-utf8", vec![0xff], "invalid_input"),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_query_metadata_bytes(&case.input) {
        Ok(metadata) => Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            flow: metadata
                .flow
                .map_or("none", |value| value.as_str())
                .to_owned(),
            mihomo_flow: metadata
                .flow
                .map_or("none", |value| value.mihomo_str())
                .to_owned(),
            packet_encoding: metadata
                .packet_encoding
                .map_or("none", |value| value.as_str())
                .to_owned(),
        },
        Err(error) => Outcome {
            accepted: false,
            classification: error.code().to_owned(),
            flow: "none".to_owned(),
            mihomo_flow: "none".to_owned(),
            packet_encoding: "none".to_owned(),
        },
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_flow_packet_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python VLESS flow/packet adapter");
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
        suite: "vless-flow-packet-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("accepted".to_owned(), PublicValue::Bool(outcome.accepted)),
                    ("flow".to_owned(), PublicValue::Text(outcome.flow.clone())),
                    (
                        "mihomo_flow".to_owned(),
                        PublicValue::Text(outcome.mihomo_flow.clone()),
                    ),
                    (
                        "packet_encoding".to_owned(),
                        PublicValue::Text(outcome.packet_encoding.clone()),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_vless_flow_packet_match() {
    let cases = cases();
    assert_eq!(cases.len(), 19);
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
fn flow_packet_error_catalog_is_fixed_and_safe() {
    let errors = [
        VlessQueryError::UnsupportedFlow,
        VlessQueryError::VisionRequiresTcp,
        VlessQueryError::VisionRequiresSecurity,
        VlessQueryError::UnsupportedPacketEncoding,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("private"));
    }
}
