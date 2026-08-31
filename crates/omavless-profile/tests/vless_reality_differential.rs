// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless_query::{
    VlessQueryError, VlessSecurity, parse_vless_query_metadata_bytes,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const UUID: &str = "11111111-1111-4111-8111-111111111111";
const PUBLIC_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

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
    reality: bool,
    pq: bool,
    pq_present: bool,
    short_id_present: bool,
    spider_x_present: bool,
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
    reality: bool,
    pq: bool,
    pq_present: bool,
    short_id_present: bool,
    spider_x_present: bool,
) -> Case {
    Case {
        id,
        input: uri(query).into_bytes(),
        expected: Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            reality,
            pq,
            pq_present,
            short_id_present,
            spider_x_present,
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
            reality: false,
            pq: false,
            pq_present: false,
            short_id_present: false,
            spider_x_present: false,
        },
    }
}

fn reality(extra: &str) -> String {
    format!("security=reality&sni=example.invalid&pbk={PUBLIC_KEY}{extra}")
}

fn cases() -> Vec<Case> {
    vec![
        accepted("defaults", "", false, false, false, false, false),
        accepted(
            "reality-minimal",
            &reality(""),
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "publickey-alias",
            &format!("security=reality&sni=example.invalid&publickey={PUBLIC_KEY}"),
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "public-key-servername-aliases",
            &format!("security=reality&servername=example.invalid&public-key={PUBLIC_KEY}"),
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "canonical-final-q",
            "security=reality&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQ",
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "canonical-final-e",
            "security=reality&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAE",
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "short-empty",
            &reality("&sid="),
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "short-lower",
            &reality("&sid=0123456789abcdef"),
            true,
            false,
            false,
            true,
            false,
        ),
        accepted(
            "short-upper",
            &reality("&short-id=ABCDEF"),
            true,
            false,
            false,
            true,
            false,
        ),
        accepted(
            "spider-x",
            &reality("&spider-x=%2Fdocs"),
            true,
            false,
            false,
            false,
            true,
        ),
        accepted(
            "pq-true",
            &reality("&supportX25519MLKEM768=true"),
            true,
            true,
            true,
            false,
            false,
        ),
        accepted(
            "pq-one",
            &reality("&supportX25519MLKEM768=1"),
            true,
            true,
            true,
            false,
            false,
        ),
        accepted(
            "pq-yes",
            &reality("&supportX25519MLKEM768=yes"),
            true,
            true,
            true,
            false,
            false,
        ),
        accepted(
            "pq-on",
            &reality("&supportX25519MLKEM768=on"),
            true,
            true,
            true,
            false,
            false,
        ),
        accepted(
            "pq-false",
            &reality("&support-x25519mlkem768=false"),
            true,
            false,
            true,
            false,
            false,
        ),
        accepted(
            "pq-zero",
            &reality("&support-x25519mlkem768=0"),
            true,
            false,
            true,
            false,
            false,
        ),
        accepted(
            "pq-no",
            &reality("&support-x25519mlkem768=no"),
            true,
            false,
            true,
            false,
            false,
        ),
        accepted(
            "pq-off",
            &reality("&support-x25519mlkem768=off"),
            true,
            false,
            true,
            false,
            false,
        ),
        accepted(
            "pq-empty",
            &reality("&support-x25519mlkem768="),
            true,
            false,
            true,
            false,
            false,
        ),
        accepted(
            "mldsa-empty",
            &reality("&mldsa65Verify="),
            true,
            false,
            false,
            false,
            false,
        ),
        accepted(
            "nonreality-metadata",
            &format!("pbk={PUBLIC_KEY}&sni=example.invalid&sid=12&spx=%2Fdocs"),
            false,
            false,
            false,
            true,
            true,
        ),
        accepted(
            "tls-metadata",
            &format!("security=tls&pbk={PUBLIC_KEY}&sni=example.invalid"),
            false,
            false,
            false,
            false,
            false,
        ),
        rejected(
            "missing-both",
            uri("security=reality"),
            "reality_fields_required",
        ),
        rejected(
            "missing-key",
            uri("security=reality&sni=example.invalid"),
            "reality_fields_required",
        ),
        rejected(
            "missing-sni",
            uri(&format!("security=reality&pbk={PUBLIC_KEY}")),
            "reality_fields_required",
        ),
        rejected(
            "key-short",
            uri("security=reality&sni=example.invalid&pbk=AAAA"),
            "invalid_reality_public_key",
        ),
        rejected(
            "key-invalid-character",
            uri(
                "security=reality&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA%2B",
            ),
            "invalid_reality_public_key",
        ),
        rejected(
            "key-noncanonical",
            uri(
                "security=reality&sni=example.invalid&pbk=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
            ),
            "invalid_reality_public_key",
        ),
        rejected(
            "short-odd",
            uri(&reality("&sid=abc")),
            "invalid_reality_short_id",
        ),
        rejected(
            "short-long",
            uri(&reality("&sid=0123456789abcdef00")),
            "invalid_reality_short_id",
        ),
        rejected(
            "short-nonhex",
            uri(&reality("&sid=private-id")),
            "invalid_reality_short_id",
        ),
        rejected(
            "pq-invalid",
            uri(&reality("&supportX25519MLKEM768=private-value")),
            "invalid_reality_pq_boolean",
        ),
        rejected(
            "pq-alias-conflict",
            uri(&reality(
                "&supportX25519MLKEM768=true&support-x25519mlkem768=false",
            )),
            "conflicting_aliases",
        ),
        rejected(
            "pq-nonreality-true",
            uri("security=tls&supportX25519MLKEM768=true"),
            "reality_pq_requires_reality",
        ),
        rejected(
            "pq-nonreality-false",
            uri("support-x25519mlkem768=false"),
            "reality_pq_requires_reality",
        ),
        rejected(
            "mldsa",
            uri(&reality("&mldsa65Verify=private-verifier")),
            "reality_mldsa_unsupported",
        ),
        rejected(
            "mldsa-alias-conflict",
            uri(&reality("&mldsa65Verify=&mldsa65-verify=")),
            "conflicting_aliases",
        ),
        rejected(
            "key-alias-conflict",
            uri(&format!(
                "security=reality&sni=example.invalid&pbk={PUBLIC_KEY}&public-key={PUBLIC_KEY}"
            )),
            "conflicting_aliases",
        ),
        rejected(
            "sni-alias-conflict",
            uri(&format!(
                "security=reality&sni=example.invalid&servername=example.invalid&pbk={PUBLIC_KEY}"
            )),
            "conflicting_aliases",
        ),
        rejected(
            "short-alias-conflict",
            uri(&reality("&sid=12&short-id=12")),
            "conflicting_aliases",
        ),
        rejected(
            "spider-alias-conflict",
            uri(&reality("&spx=%2F&spider-x=%2F")),
            "conflicting_aliases",
        ),
        rejected("invalid-utf8", vec![0xff], "invalid_input"),
        rejected(
            "missing-port",
            format!("vless://{UUID}@example.invalid?security=reality"),
            "missing_server_port",
        ),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_query_metadata_bytes(&case.input) {
        Ok(metadata) => Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            reality: metadata.security == VlessSecurity::Reality,
            pq: metadata.reality_pq,
            pq_present: metadata.reality_pq_present,
            short_id_present: metadata.reality_short_id_present,
            spider_x_present: metadata.reality_spider_x_present,
        },
        Err(error) => Outcome {
            accepted: false,
            classification: error.code().to_owned(),
            reality: false,
            pq: false,
            pq_present: false,
            short_id_present: false,
            spider_x_present: false,
        },
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_reality_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python VLESS REALITY adapter");
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
        suite: "vless-reality-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("accepted".to_owned(), PublicValue::Bool(outcome.accepted)),
                    ("reality".to_owned(), PublicValue::Bool(outcome.reality)),
                    ("pq".to_owned(), PublicValue::Bool(outcome.pq)),
                    (
                        "pq_present".to_owned(),
                        PublicValue::Bool(outcome.pq_present),
                    ),
                    (
                        "short_id_present".to_owned(),
                        PublicValue::Bool(outcome.short_id_present),
                    ),
                    (
                        "spider_x_present".to_owned(),
                        PublicValue::Bool(outcome.spider_x_present),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_vless_reality_match() {
    let cases = cases();
    assert_eq!(cases.len(), 43);
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
            PUBLIC_KEY.as_bytes(),
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
fn reality_error_catalog_is_fixed_and_safe() {
    let errors = [
        VlessQueryError::RealityFieldsRequired,
        VlessQueryError::InvalidRealityPqBoolean,
        VlessQueryError::RealityPqRequiresReality,
        VlessQueryError::RealityMldsaUnsupported,
        VlessQueryError::InvalidRealityPublicKey,
        VlessQueryError::InvalidRealityShortId,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("private"));
    }
}
