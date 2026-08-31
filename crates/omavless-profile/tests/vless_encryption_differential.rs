// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless_encryption::{VlessEncryptionError, VlessEncryptionMode};
use omavless_profile::vless_query::{VlessQueryError, parse_vless_query_metadata_bytes};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const UUID: &str = "11111111-1111-4111-8111-111111111111";
const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";

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
    enabled: bool,
    mode: String,
    rtt: String,
    key_count: usize,
    large_key_present: bool,
    padding_count: usize,
    total_padding: usize,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn uri(encryption: Option<&str>) -> String {
    encryption.map_or_else(
        || format!("vless://{UUID}@example.invalid:443#Node"),
        |value| format!("vless://{UUID}@example.invalid:443?encryption={value}#Node"),
    )
}

#[allow(clippy::too_many_arguments)]
fn accepted(
    id: &'static str,
    encryption: Option<&str>,
    enabled: bool,
    mode: &str,
    rtt: &str,
    key_count: usize,
    large_key_present: bool,
    padding_count: usize,
    total_padding: usize,
) -> Case {
    Case {
        id,
        input: uri(encryption).into_bytes(),
        expected: Outcome {
            accepted: true,
            classification: "accepted".to_owned(),
            enabled,
            mode: mode.to_owned(),
            rtt: rtt.to_owned(),
            key_count,
            large_key_present,
            padding_count,
            total_padding,
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
            enabled: false,
            mode: "none".to_owned(),
            rtt: "none".to_owned(),
            key_count: 0,
            large_key_present: false,
            padding_count: 0,
            total_padding: 0,
        },
    }
}

fn cases() -> Vec<Case> {
    let key_32 = "A".repeat(43);
    let key_1184 = "A".repeat(1579);
    let max_parts = format!(
        "mlkem768x25519plus.native.1rtt.{}.{}.{KEY}",
        "100-35-35",
        std::iter::repeat_n("0-0-0", 27)
            .collect::<Vec<_>>()
            .join("."),
    );
    let too_many_parts = format!(
        "mlkem768x25519plus.native.1rtt.{}",
        std::iter::repeat_n(KEY, 30).collect::<Vec<_>>().join("."),
    );
    vec![
        accepted("default-none", None, false, "none", "none", 0, false, 0, 0),
        accepted(
            "explicit-none",
            Some("none"),
            false,
            "none",
            "none",
            0,
            false,
            0,
            0,
        ),
        accepted(
            "explicit-empty",
            Some(""),
            false,
            "none",
            "none",
            0,
            false,
            0,
            0,
        ),
        accepted(
            "native-one-rtt",
            Some(&format!("mlkem768x25519plus.native.1rtt.{KEY}")),
            true,
            "native",
            "1rtt",
            1,
            false,
            0,
            0,
        ),
        accepted(
            "native-zero-rtt",
            Some(&format!("mlkem768x25519plus.native.0rtt.{KEY}")),
            true,
            "native",
            "0rtt",
            1,
            false,
            0,
            0,
        ),
        accepted(
            "xorpub",
            Some(&format!("mlkem768x25519plus.xorpub.1rtt.{KEY}")),
            true,
            "xorpub",
            "1rtt",
            1,
            false,
            0,
            0,
        ),
        accepted(
            "random",
            Some(&format!("mlkem768x25519plus.random.0rtt.{KEY}")),
            true,
            "random",
            "0rtt",
            1,
            false,
            0,
            0,
        ),
        accepted(
            "zero-key-32",
            Some(&format!("mlkem768x25519plus.native.1rtt.{key_32}")),
            true,
            "native",
            "1rtt",
            1,
            false,
            0,
            0,
        ),
        accepted(
            "zero-key-1184",
            Some(&format!("mlkem768x25519plus.native.1rtt.{key_1184}")),
            true,
            "native",
            "1rtt",
            1,
            true,
            0,
            0,
        ),
        accepted(
            "two-key-sizes",
            Some(&format!(
                "mlkem768x25519plus.xorpub.0rtt.{key_32}.{key_1184}"
            )),
            true,
            "xorpub",
            "0rtt",
            2,
            true,
            0,
            0,
        ),
        accepted(
            "first-padding-boundary",
            Some(&format!("mlkem768x25519plus.random.1rtt.100-35-35.{KEY}")),
            true,
            "random",
            "1rtt",
            1,
            false,
            1,
            35,
        ),
        accepted(
            "three-padding-ranges",
            Some(&format!(
                "mlkem768x25519plus.random.1rtt.100-35-100.75-0-50.50-0-200.{KEY}"
            )),
            true,
            "random",
            "1rtt",
            1,
            false,
            3,
            300,
        ),
        accepted(
            "padding-after-key",
            Some(&format!("mlkem768x25519plus.native.1rtt.{KEY}.100-35-40")),
            true,
            "native",
            "1rtt",
            1,
            false,
            1,
            40,
        ),
        accepted(
            "two-padding-ranges",
            Some(&format!(
                "mlkem768x25519plus.native.1rtt.100-35-100.50-0-500.{KEY}"
            )),
            true,
            "native",
            "1rtt",
            1,
            false,
            2,
            100,
        ),
        accepted(
            "leading-zero-padding",
            Some(&format!(
                "mlkem768x25519plus.random.1rtt.0100-0035-0035.{KEY}"
            )),
            true,
            "random",
            "1rtt",
            1,
            false,
            1,
            35,
        ),
        accepted(
            "max-parts",
            Some(&max_parts),
            true,
            "native",
            "1rtt",
            1,
            false,
            28,
            35,
        ),
        accepted(
            "mixed-keys-padding",
            Some(&format!(
                "mlkem768x25519plus.random.0rtt.{key_32}.100-35-100.{key_1184}.25-0-50"
            )),
            true,
            "random",
            "0rtt",
            2,
            true,
            2,
            100,
        ),
        rejected(
            "unsupported-simple",
            uri(Some("aes-128-gcm")),
            "unsupported_encryption",
        ),
        rejected(
            "uppercase-none",
            uri(Some("NONE")),
            "unsupported_encryption",
        ),
        rejected(
            "unsupported-mode",
            uri(Some(&format!("mlkem768x25519plus.bad.1rtt.{KEY}"))),
            "unsupported_encryption",
        ),
        rejected(
            "unsupported-rtt",
            uri(Some(&format!("mlkem768x25519plus.native.bad.{KEY}"))),
            "unsupported_encryption",
        ),
        rejected(
            "too-few-parts",
            uri(Some("mlkem768x25519plus.native.1rtt")),
            "unsupported_encryption",
        ),
        rejected(
            "too-many-parts",
            uri(Some(&too_many_parts)),
            "unsupported_encryption",
        ),
        rejected(
            "invalid-format",
            uri(Some("mlkem768x25519plus.native.1rtt.private%2Bvalue")),
            "invalid_encryption_format",
        ),
        rejected(
            "invalid-space",
            uri(Some("mlkem768x25519plus.native.1rtt.private%20value")),
            "invalid_encryption_format",
        ),
        rejected(
            "invalid-padding",
            uri(Some("mlkem768x25519plus.native.1rtt.bad")),
            "invalid_encryption_padding",
        ),
        rejected(
            "padding-extra-segment",
            uri(Some("mlkem768x25519plus.native.1rtt.100-35-35-1")),
            "invalid_encryption_padding",
        ),
        rejected(
            "padding-probability",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.101-35-35.{KEY}"
            ))),
            "encryption_padding_range",
        ),
        rejected(
            "padding-minimum-maximum",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.100-36-35.{KEY}"
            ))),
            "encryption_padding_range",
        ),
        rejected(
            "padding-maximum",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.100-35-65554.{KEY}"
            ))),
            "encryption_padding_range",
        ),
        rejected(
            "first-padding-probability",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.99-35-35.{KEY}"
            ))),
            "encryption_first_padding_too_small",
        ),
        rejected(
            "first-padding-minimum",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.100-34-35.{KEY}"
            ))),
            "encryption_first_padding_too_small",
        ),
        rejected(
            "key-noncanonical",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.{}B",
                "A".repeat(42)
            ))),
            "invalid_encryption_key",
        ),
        rejected(
            "key-31-bytes",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.{}",
                "A".repeat(42)
            ))),
            "invalid_encryption_key",
        ),
        rejected(
            "key-33-bytes",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.{}",
                "A".repeat(44)
            ))),
            "invalid_encryption_key",
        ),
        rejected(
            "key-invalid-length",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.{}",
                "A".repeat(41)
            ))),
            "invalid_encryption_key",
        ),
        rejected(
            "key-required",
            uri(Some("mlkem768x25519plus.native.1rtt.100-35-35")),
            "encryption_key_required",
        ),
        rejected(
            "total-padding",
            uri(Some(&format!(
                "mlkem768x25519plus.random.1rtt.100-35-40000.0-0-0.100-0-30000.{KEY}"
            ))),
            "encryption_total_padding_too_large",
        ),
        rejected(
            "first-padding-after-key",
            uri(Some(&format!(
                "mlkem768x25519plus.native.1rtt.{KEY}.50-0-0"
            ))),
            "encryption_first_padding_too_small",
        ),
        rejected(
            "empty-token",
            uri(Some(&format!("mlkem768x25519plus.native.1rtt..{KEY}"))),
            "invalid_encryption_padding",
        ),
        rejected(
            "too-large",
            uri(Some(&"A".repeat(12 * 1024 + 1))),
            "encryption_too_large",
        ),
        rejected("invalid-utf8", vec![0xff], "invalid_input"),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_query_metadata_bytes(&case.input) {
        Ok(metadata) => metadata.encryption.map_or(
            Outcome {
                accepted: true,
                classification: "accepted".to_owned(),
                enabled: false,
                mode: "none".to_owned(),
                rtt: "none".to_owned(),
                key_count: 0,
                large_key_present: false,
                padding_count: 0,
                total_padding: 0,
            },
            |encryption| Outcome {
                accepted: true,
                classification: "accepted".to_owned(),
                enabled: true,
                mode: encryption.mode.as_str().to_owned(),
                rtt: encryption.rtt.as_str().to_owned(),
                key_count: encryption.key_count,
                large_key_present: encryption.large_key_present,
                padding_count: encryption.padding_count,
                total_padding: encryption.total_padding,
            },
        ),
        Err(error) => Outcome {
            accepted: false,
            classification: error.code().to_owned(),
            enabled: false,
            mode: "none".to_owned(),
            rtt: "none".to_owned(),
            key_count: 0,
            large_key_present: false,
            padding_count: 0,
            total_padding: 0,
        },
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_encryption_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python VLESS Encryption adapter");
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
        suite: "vless-encryption-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("enabled".to_owned(), PublicValue::Bool(outcome.enabled)),
                    ("mode".to_owned(), PublicValue::Text(outcome.mode.clone())),
                    ("rtt".to_owned(), PublicValue::Text(outcome.rtt.clone())),
                    (
                        "key_count".to_owned(),
                        PublicValue::Integer(outcome.key_count as i64),
                    ),
                    (
                        "large_key_present".to_owned(),
                        PublicValue::Bool(outcome.large_key_present),
                    ),
                    (
                        "padding_count".to_owned(),
                        PublicValue::Integer(outcome.padding_count as i64),
                    ),
                    (
                        "total_padding".to_owned(),
                        PublicValue::Integer(outcome.total_padding as i64),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_vless_encryption_match() {
    let cases = cases();
    assert_eq!(cases.len(), 42);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(rust_outcome, case.expected, "Rust {}", case.id);
        assert_eq!(python_outcome, case.expected, "Python {}", case.id);
        for marker in [
            b"private".as_slice(),
            b"mlkem768x25519plus".as_slice(),
            UUID.as_bytes(),
            KEY.as_bytes(),
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
fn encryption_error_catalog_is_fixed_and_safe() {
    let errors = [
        VlessEncryptionError::TooLarge,
        VlessEncryptionError::InvalidFormat,
        VlessEncryptionError::Unsupported,
        VlessEncryptionError::InvalidPadding,
        VlessEncryptionError::PaddingRange,
        VlessEncryptionError::FirstPaddingTooSmall,
        VlessEncryptionError::InvalidKey,
        VlessEncryptionError::KeyRequired,
        VlessEncryptionError::TotalPaddingTooLarge,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("private"));
        assert_eq!(VlessQueryError::Encryption(error).code(), error.code(),);
    }
    assert_eq!(VlessEncryptionMode::Native.as_str(), "native");
}
