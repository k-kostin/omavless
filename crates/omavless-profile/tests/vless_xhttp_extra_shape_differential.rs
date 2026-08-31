// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless_xhttp_extra::{
    MAX_XHTTP_EXTRA_BYTES, XhttpExtraError, XhttpExtraFacts, decode_xhttp_extra_bytes,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
    source_empty: bool,
    root_field_count: usize,
    value_count: usize,
    object_count: usize,
    array_count: usize,
    string_count: usize,
    integer_count: usize,
    float_count: usize,
    boolean_count: usize,
    true_count: usize,
    false_count: usize,
    null_count: usize,
    maximum_depth: usize,
    non_ascii_present: bool,
}

impl Outcome {
    fn accepted(facts: XhttpExtraFacts) -> Self {
        Self {
            accepted: true,
            classification: "accepted".to_owned(),
            source_empty: facts.source_empty,
            root_field_count: facts.root_field_count,
            value_count: facts.value_count,
            object_count: facts.object_count,
            array_count: facts.array_count,
            string_count: facts.string_count,
            integer_count: facts.integer_count,
            float_count: facts.float_count,
            boolean_count: facts.boolean_count,
            true_count: facts.true_count,
            false_count: facts.false_count,
            null_count: facts.null_count,
            maximum_depth: facts.maximum_depth,
            non_ascii_present: facts.non_ascii_present,
        }
    }

    fn rejected(classification: &str) -> Self {
        Self {
            accepted: false,
            classification: classification.to_owned(),
            source_empty: false,
            root_field_count: 0,
            value_count: 0,
            object_count: 0,
            array_count: 0,
            string_count: 0,
            integer_count: 0,
            float_count: 0,
            boolean_count: 0,
            true_count: 0,
            false_count: 0,
            null_count: 0,
            maximum_depth: 0,
            non_ascii_present: false,
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn accepted(id: &'static str, input: impl Into<Vec<u8>>, facts: XhttpExtraFacts) -> Case {
    Case {
        id,
        input: input.into(),
        expected: Outcome::accepted(facts),
    }
}

fn rejected(id: &'static str, input: impl Into<Vec<u8>>, classification: &str) -> Case {
    Case {
        id,
        input: input.into(),
        expected: Outcome::rejected(classification),
    }
}

fn object_with_integer_fields(count: usize) -> String {
    let fields = (0..count)
        .map(|index| format!(r#""k{index}":0"#))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn nested_object(depth: usize) -> String {
    let mut value = "{}".to_owned();
    for _ in 0..depth {
        value = format!(r#"{{"a":{value}}}"#);
    }
    value
}

fn cases() -> Vec<Case> {
    let simple = XhttpExtraFacts {
        root_field_count: 6,
        value_count: 7,
        object_count: 1,
        string_count: 1,
        integer_count: 1,
        float_count: 1,
        boolean_count: 2,
        true_count: 1,
        false_count: 1,
        null_count: 1,
        maximum_depth: 1,
        ..XhttpExtraFacts::default()
    };
    let one_integer = XhttpExtraFacts {
        root_field_count: 1,
        value_count: 2,
        object_count: 1,
        integer_count: 1,
        maximum_depth: 1,
        ..XhttpExtraFacts::default()
    };
    let one_float = XhttpExtraFacts {
        root_field_count: 1,
        value_count: 2,
        object_count: 1,
        float_count: 1,
        maximum_depth: 1,
        ..XhttpExtraFacts::default()
    };
    let one_ascii_string = XhttpExtraFacts {
        root_field_count: 1,
        value_count: 2,
        object_count: 1,
        string_count: 1,
        maximum_depth: 1,
        ..XhttpExtraFacts::default()
    };
    let one_unicode_string = XhttpExtraFacts {
        non_ascii_present: true,
        ..one_ascii_string
    };

    let exact_size = format!("{}{{}}", " ".repeat(MAX_XHTTP_EXTRA_BYTES - 2));
    let legal_items = object_with_integer_fields(159);
    let too_many_items = object_with_integer_fields(160);
    let non_object_too_many = format!("[{}]", ["0"; 160].join(","));
    let too_many_before_string = format!(
        "{{{},\"last\":\"{}\"}}",
        object_with_integer_fields(159)
            .trim_start_matches('{')
            .trim_end_matches('}'),
        "x".repeat(2049)
    );
    let oversized_key_before_deep = format!(r#"{{"{}":{}}}"#, "k".repeat(129), nested_object(9));
    let duplicate_before_oversized = format!(r#"{{"a":"{}","a":0}}"#, "x".repeat(2049));
    let unicode_oversized_key = format!("{}abc", "é".repeat(63));

    vec![
        accepted(
            "empty-input",
            Vec::new(),
            XhttpExtraFacts {
                source_empty: true,
                value_count: 1,
                object_count: 1,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "empty-object",
            b"{}".to_vec(),
            XhttpExtraFacts {
                value_count: 1,
                object_count: 1,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "whitespace-object",
            b" \t{}\r\n".to_vec(),
            XhttpExtraFacts {
                value_count: 1,
                object_count: 1,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "simple-scalars",
            br#"{"string":"text","integer":1,"float":1.5,"true":true,"false":false,"null":null}"#.to_vec(),
            simple,
        ),
        accepted(
            "nested-mixed",
            r#"{"a":[1,{"b":"é"},[]]}"#.as_bytes().to_vec(),
            XhttpExtraFacts {
                root_field_count: 1,
                value_count: 6,
                object_count: 2,
                array_count: 2,
                string_count: 1,
                integer_count: 1,
                maximum_depth: 3,
                non_ascii_present: true,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "empty-containers",
            br#"{"array":[],"object":{}}"#.to_vec(),
            XhttpExtraFacts {
                root_field_count: 2,
                value_count: 3,
                object_count: 2,
                array_count: 1,
                maximum_depth: 1,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "legal-depth-eight",
            nested_object(8),
            XhttpExtraFacts {
                root_field_count: 1,
                value_count: 9,
                object_count: 9,
                maximum_depth: 8,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "legal-item-count",
            legal_items,
            XhttpExtraFacts {
                root_field_count: 159,
                value_count: 160,
                object_count: 1,
                integer_count: 159,
                maximum_depth: 1,
                ..XhttpExtraFacts::default()
            },
        ),
        accepted(
            "ascii-key-128-bytes",
            format!(r#"{{"{}":0}}"#, "k".repeat(128)),
            one_integer,
        ),
        accepted(
            "unicode-key-128-bytes",
            format!(r#"{{"{}":0}}"#, "é".repeat(64)),
            XhttpExtraFacts {
                non_ascii_present: true,
                ..one_integer
            },
        ),
        accepted(
            "ascii-string-2048-bytes",
            format!(r#"{{"x":"{}"}}"#, "x".repeat(2048)),
            one_ascii_string,
        ),
        accepted(
            "unicode-string-2048-bytes",
            format!(r#"{{"x":"{}"}}"#, "é".repeat(1024)),
            one_unicode_string,
        ),
        accepted(
            "escaped-unicode-string-2048-bytes",
            format!(r#"{{"x":"{}"}}"#, r"\u00e9".repeat(1024)),
            one_unicode_string,
        ),
        accepted(
            "escaped-surrogate-pair",
            br#"{"x":"\ud83d\ude00"}"#.to_vec(),
            one_unicode_string,
        ),
        accepted(
            "escaped-control-characters",
            br#"{"x":"\u0000\n\t"}"#.to_vec(),
            one_ascii_string,
        ),
        accepted(
            "raw-unicode-string",
            r#"{"x":"тест"}"#.as_bytes().to_vec(),
            one_unicode_string,
        ),
        accepted("integer-zero", br#"{"x":0}"#.to_vec(), one_integer),
        accepted("negative-zero-integer", br#"{"x":-0}"#.to_vec(), one_integer),
        accepted(
            "arbitrary-precision-integer",
            br#"{"x":1234567890123456789012345678901234567890}"#.to_vec(),
            one_integer,
        ),
        accepted("finite-float", br#"{"x":1.25e2}"#.to_vec(), one_float),
        accepted("float-zero", br#"{"x":0.0}"#.to_vec(), one_float),
        accepted("overflow-float", br#"{"x":1e400}"#.to_vec(), one_float),
        accepted("nan-extension", br#"{"x":NaN}"#.to_vec(), one_float),
        accepted(
            "positive-infinity-extension",
            br#"{"x":Infinity}"#.to_vec(),
            one_float,
        ),
        accepted(
            "negative-infinity-extension",
            br#"{"x":-Infinity}"#.to_vec(),
            one_float,
        ),
        accepted(
            "exact-raw-size",
            exact_size,
            XhttpExtraFacts {
                value_count: 1,
                object_count: 1,
                ..XhttpExtraFacts::default()
            },
        ),
        rejected(
            "invalid-utf8",
            vec![b'{', b'"', b'x', b'"', b':', 0xff, b'}'],
            "invalid_utf8",
        ),
        rejected("whitespace-only", b" \t\r\n".to_vec(), "invalid_json"),
        rejected("malformed-object", br#"{"x":}"#.to_vec(), "invalid_json"),
        rejected("trailing-json", b"{} trailing".to_vec(), "invalid_json"),
        rejected("non-object-null", b"null".to_vec(), "non_object_root"),
        rejected("non-object-array", b"[1]".to_vec(), "non_object_root"),
        rejected(
            "non-object-oversized-string",
            format!(r#""{}""#, "x".repeat(2049)),
            "oversized_string",
        ),
        rejected(
            "duplicate-top-level-key",
            br#"{"a":1,"a":2}"#.to_vec(),
            "duplicate_fields",
        ),
        rejected(
            "duplicate-nested-key",
            br#"{"outer":{"a":1,"a":2}}"#.to_vec(),
            "duplicate_fields",
        ),
        rejected(
            "duplicate-decoded-key",
            br#"{"a":1,"\u0061":2}"#.to_vec(),
            "duplicate_fields",
        ),
        rejected(
            "duplicate-before-trailing-json",
            br#"{"a":1,"a":2} trailing"#.to_vec(),
            "duplicate_fields",
        ),
        rejected(
            "malformed-before-duplicate-hook",
            br#"{"a":1,"a":}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "duplicate-before-shape-validation",
            duplicate_before_oversized,
            "duplicate_fields",
        ),
        rejected(
            "raw-size-overflow",
            vec![b' '; MAX_XHTTP_EXTRA_BYTES + 1],
            "too_large",
        ),
        rejected("depth-nine", nested_object(9), "too_deep"),
        rejected("item-count-161", too_many_items, "too_many_values"),
        rejected(
            "non-object-item-count-161",
            non_object_too_many,
            "too_many_values",
        ),
        rejected(
            "ascii-key-129-bytes",
            format!(r#"{{"{}":0}}"#, "k".repeat(129)),
            "oversized_field_name",
        ),
        rejected(
            "unicode-key-129-bytes",
            format!(r#"{{"{unicode_oversized_key}":0}}"#),
            "oversized_field_name",
        ),
        rejected(
            "ascii-string-2049-bytes",
            format!(r#"{{"x":"{}"}}"#, "x".repeat(2049)),
            "oversized_string",
        ),
        rejected(
            "unicode-string-2049-bytes",
            format!(r#"{{"x":"{}a"}}"#, "é".repeat(1024)),
            "oversized_string",
        ),
        rejected(
            "escaped-unicode-string-2050-bytes",
            format!(r#"{{"x":"{}"}}"#, r"\u00e9".repeat(1025)),
            "oversized_string",
        ),
        rejected(
            "lone-high-surrogate",
            br#"{"x":"\ud800"}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "lone-low-surrogate",
            br#"{"x":"\udc00"}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "invalid-surrogate-pair",
            br#"{"x":"\ud800\u0041"}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "raw-control-character",
            b"{\"x\":\"a\nb\"}".to_vec(),
            "invalid_json",
        ),
        rejected("leading-zero-number", br#"{"x":01}"#.to_vec(), "invalid_json"),
        rejected("plus-number", br#"{"x":+1}"#.to_vec(), "invalid_json"),
        rejected("lowercase-nan", br#"{"x":nan}"#.to_vec(), "invalid_json"),
        rejected(
            "incomplete-exponent",
            br#"{"x":1e}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "utf8-bom",
            "\u{feff}{}".as_bytes().to_vec(),
            "invalid_json",
        ),
        rejected(
            "oversized-key-before-deep-value",
            oversized_key_before_deep,
            "oversized_field_name",
        ),
        rejected(
            "too-many-before-oversized-string",
            too_many_before_string,
            "too_many_values",
        ),
        rejected(
            "invalid-string-escape",
            br#"{"x":"\q"}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "invalid-unicode-escape",
            br#"{"x":"\u12xz"}"#.to_vec(),
            "invalid_json",
        ),
        rejected(
            "credential-looking-malformed-json",
            br#"{"password":"private-secret","uri":"vless://secret.example","token":"token-value",}"#.to_vec(),
            "invalid_json",
        ),
    ]
}

fn rust_outcome(case: &Case) -> Outcome {
    match decode_xhttp_extra_bytes(&case.input) {
        Ok(document) => Outcome::accepted(document.facts()),
        Err(error) => Outcome::rejected(error.code()),
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_xhttp_extra_shape_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python XHTTP-extra shape adapter");
    {
        let mut stdin = child.stdin.take().expect("adapter stdin");
        stdin.write_all(&case.input).expect("write adapter input");
    }
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
        suite: "vless-xhttp-extra-shape-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([
                    ("accepted".to_owned(), PublicValue::Bool(outcome.accepted)),
                    (
                        "source_empty".to_owned(),
                        PublicValue::Bool(outcome.source_empty),
                    ),
                    (
                        "root_field_count".to_owned(),
                        PublicValue::Integer(outcome.root_field_count as i64),
                    ),
                    (
                        "value_count".to_owned(),
                        PublicValue::Integer(outcome.value_count as i64),
                    ),
                    (
                        "object_count".to_owned(),
                        PublicValue::Integer(outcome.object_count as i64),
                    ),
                    (
                        "array_count".to_owned(),
                        PublicValue::Integer(outcome.array_count as i64),
                    ),
                    (
                        "string_count".to_owned(),
                        PublicValue::Integer(outcome.string_count as i64),
                    ),
                    (
                        "integer_count".to_owned(),
                        PublicValue::Integer(outcome.integer_count as i64),
                    ),
                    (
                        "float_count".to_owned(),
                        PublicValue::Integer(outcome.float_count as i64),
                    ),
                    (
                        "boolean_count".to_owned(),
                        PublicValue::Integer(outcome.boolean_count as i64),
                    ),
                    (
                        "true_count".to_owned(),
                        PublicValue::Integer(outcome.true_count as i64),
                    ),
                    (
                        "false_count".to_owned(),
                        PublicValue::Integer(outcome.false_count as i64),
                    ),
                    (
                        "null_count".to_owned(),
                        PublicValue::Integer(outcome.null_count as i64),
                    ),
                    (
                        "maximum_depth".to_owned(),
                        PublicValue::Integer(outcome.maximum_depth as i64),
                    ),
                    (
                        "non_ascii_present".to_owned(),
                        PublicValue::Bool(outcome.non_ascii_present),
                    ),
                ])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_xhttp_extra_shape_match() {
    let cases = cases();
    assert_eq!(cases.len(), 62);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(rust_outcome, case.expected, "Rust {}", case.id);
        assert_eq!(python_outcome, case.expected, "Python {}", case.id);
        for marker in [
            b"private-secret".as_slice(),
            b"vless://".as_slice(),
            b"secret.example".as_slice(),
            b"token-value".as_slice(),
        ] {
            assert!(!stdout.windows(marker.len()).any(|part| part == marker));
            assert!(!stderr.windows(marker.len()).any(|part| part == marker));
        }
        assert!(stdout.len() <= 1024, "bounded stdout for {}", case.id);
        assert!(stderr.len() <= 256, "bounded stderr for {}", case.id);
        rust.push((case.id.to_owned(), rust_outcome));
        python.push((case.id.to_owned(), python_outcome));
    }
    let summary = compare_reports(&report(&python), &report(&rust)).expect("compatible reports");
    assert!(summary.matched);
    assert_eq!(summary.case_count, cases.len());
    assert_eq!(summary.mismatch_count, 0);
}

#[test]
fn xhttp_extra_error_catalog_is_fixed_and_safe() {
    let errors = [
        XhttpExtraError::InvalidUtf8,
        XhttpExtraError::TooLarge,
        XhttpExtraError::InvalidJson,
        XhttpExtraError::DuplicateFields,
        XhttpExtraError::TooDeep,
        XhttpExtraError::TooManyValues,
        XhttpExtraError::OversizedFieldName,
        XhttpExtraError::OversizedString,
        XhttpExtraError::NonObjectRoot,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 80);
        assert!(!error.to_string().contains("private"));
        assert!(!error.code().contains("private"));
    }
}
