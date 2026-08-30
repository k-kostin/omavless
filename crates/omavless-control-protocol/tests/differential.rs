// SPDX-License-Identifier: MIT

use omavless_control_protocol::{
    FrameKind, MAX_ID_LENGTH, MAX_NESTING_DEPTH, MAX_REQUEST_FRAME_BYTES, MAX_REVISION,
    MAX_STRING_BYTES, StableErrorCode, decode_request, decode_response,
};
use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Deserialize)]
struct Corpus {
    requests: Vec<CorpusCase>,
    responses: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
struct CorpusCase {
    name: String,
    value: Value,
}

#[derive(Debug)]
struct DifferentialCase {
    id: String,
    kind: FrameKind,
    frame: Vec<u8>,
    expected: &'static str,
    expected_value: Option<Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AdapterOutcome {
    accepted: bool,
    classification: String,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn frame(value: &Value) -> Vec<u8> {
    let mut encoded = serde_json::to_vec(value).expect("JSON frame");
    encoded.push(b'\n');
    encoded
}

fn base_request() -> Value {
    json!({
        "api": "omavless.control",
        "version": 1,
        "id": "request-1",
        "method": "status.get",
        "params": {},
    })
}

fn base_success() -> Value {
    json!({
        "api": "omavless.control",
        "version": 1,
        "id": "response-1",
        "ok": true,
        "revision": 0,
        "result": {},
    })
}

fn case(id: &str, kind: FrameKind, frame: Vec<u8>, expected: &'static str) -> DifferentialCase {
    DifferentialCase {
        id: id.to_owned(),
        kind,
        frame,
        expected,
        expected_value: None,
    }
}

fn value_case(id: &str, kind: FrameKind, value: Value) -> DifferentialCase {
    DifferentialCase {
        id: id.to_owned(),
        kind,
        frame: frame(&value),
        expected: "accepted",
        expected_value: Some(value),
    }
}

fn set(value: &mut Value, field: &str, replacement: Value) {
    value
        .as_object_mut()
        .expect("object fixture")
        .insert(field.to_owned(), replacement);
}

fn remove(value: &mut Value, field: &str) {
    value.as_object_mut().expect("object fixture").remove(field);
}

fn array_depth(count: usize) -> Value {
    let mut value = Value::String("leaf".to_owned());
    for _ in 0..count {
        value = Value::Array(vec![value]);
    }
    value
}

fn corpus_cases() -> Vec<DifferentialCase> {
    let corpus: Corpus = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/control_protocol_cases/cases.json"))
            .expect("control protocol corpus"),
    )
    .expect("valid corpus");
    let mut cases = Vec::new();
    for item in corpus.requests {
        cases.push(value_case(
            &format!("corpus-request-{}", item.name),
            FrameKind::Request,
            item.value,
        ));
    }
    for item in corpus.responses {
        cases.push(value_case(
            &format!("corpus-response-{}", item.name),
            FrameKind::Response,
            item.value,
        ));
    }
    cases
}

fn differential_cases() -> Vec<DifferentialCase> {
    let mut cases = corpus_cases();

    let mut max_id = base_request();
    set(&mut max_id, "id", Value::String("x".repeat(MAX_ID_LENGTH)));
    cases.push(value_case("request-max-id", FrameKind::Request, max_id));

    let mut max_string = base_request();
    set(
        &mut max_string["params"],
        "value",
        Value::String("x".repeat(MAX_STRING_BYTES)),
    );
    cases.push(value_case(
        "request-max-string",
        FrameKind::Request,
        max_string,
    ));

    let mut max_depth = base_request();
    set(
        &mut max_depth["params"],
        "value",
        array_depth(MAX_NESTING_DEPTH - 2),
    );
    cases.push(value_case(
        "request-max-depth",
        FrameKind::Request,
        max_depth,
    ));

    let mut max_revision = base_success();
    set(&mut max_revision, "revision", json!(MAX_REVISION));
    cases.push(value_case(
        "response-max-revision",
        FrameKind::Response,
        max_revision,
    ));

    for (id, integer) in [
        ("request-min-integer", json!(i64::MIN)),
        ("request-max-integer", json!(u64::MAX)),
    ] {
        let mut request = base_request();
        set(&mut request["params"], "value", integer);
        cases.push(value_case(id, FrameKind::Request, request));
    }

    cases.extend([
        case("request-empty", FrameKind::Request, Vec::new(), "invalid_request"),
        case(
            "request-empty-line",
            FrameKind::Request,
            b"\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-invalid-utf8",
            FrameKind::Request,
            b"{\"api\":\"\xff\"}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-duplicate-top",
            FrameKind::Request,
            b"{\"api\":\"omavless.control\",\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{}}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-duplicate-nested",
            FrameKind::Request,
            b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{\"x\":1,\"x\":2}}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-non-object",
            FrameKind::Request,
            b"[]\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-trailing-json",
            FrameKind::Request,
            b"{}{}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-two-frames",
            FrameKind::Request,
            b"{}\n{}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-non-finite",
            FrameKind::Request,
            b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{\"value\":NaN}}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-positive-integer-overflow",
            FrameKind::Request,
            b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{\"value\":18446744073709551616}}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-negative-integer-overflow",
            FrameKind::Request,
            b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"x\",\"method\":\"status.get\",\"params\":{\"value\":-9223372036854775809}}\n".to_vec(),
            "invalid_request",
        ),
        case(
            "request-private-malformed",
            FrameKind::Request,
            b"{broken:vless://private.invalid?password=secret}\n".to_vec(),
            "invalid_request",
        ),
    ]);

    let mut missing_newline = frame(&base_request());
    missing_newline.pop();
    cases.push(case(
        "request-missing-newline",
        FrameKind::Request,
        missing_newline,
        "invalid_request",
    ));

    let mut unknown = base_request();
    set(&mut unknown, "extra", json!(true));
    cases.push(case(
        "request-unknown-field",
        FrameKind::Request,
        frame(&unknown),
        "invalid_request",
    ));

    let mut wrong_api = base_request();
    set(&mut wrong_api, "api", json!("other.control"));
    cases.push(case(
        "request-wrong-api",
        FrameKind::Request,
        frame(&wrong_api),
        "invalid_request",
    ));

    for (id, version) in [
        ("request-version-two", json!(2)),
        ("request-version-float", json!(1.0)),
        ("request-version-bool", json!(true)),
    ] {
        let mut request = base_request();
        set(&mut request, "version", version);
        cases.push(case(
            id,
            FrameKind::Request,
            frame(&request),
            "unsupported_version",
        ));
    }

    for (id, method) in [
        ("request-empty-method", ""),
        ("request-unicode-method", "статус"),
    ] {
        let mut request = base_request();
        set(&mut request, "method", json!(method));
        cases.push(case(
            id,
            FrameKind::Request,
            frame(&request),
            "invalid_request",
        ));
    }

    let mut missing = base_request();
    remove(&mut missing, "params");
    cases.push(case(
        "request-missing-field",
        FrameKind::Request,
        frame(&missing),
        "invalid_request",
    ));

    for (id, request_id) in [
        ("request-empty-id", "".to_owned()),
        ("request-space-id", "has space".to_owned()),
        ("request-unicode-id", "строка".to_owned()),
        ("request-overlong-id", "x".repeat(MAX_ID_LENGTH + 1)),
    ] {
        let mut request = base_request();
        set(&mut request, "id", Value::String(request_id));
        cases.push(case(
            id,
            FrameKind::Request,
            frame(&request),
            "invalid_request",
        ));
    }

    let mut params_list = base_request();
    set(&mut params_list, "params", json!([]));
    cases.push(case(
        "request-params-list",
        FrameKind::Request,
        frame(&params_list),
        "invalid_argument",
    ));

    let mut operation = base_request();
    set(&mut operation["params"], "operationId", json!("has space"));
    cases.push(case(
        "request-bad-operation-id",
        FrameKind::Request,
        frame(&operation),
        "invalid_request",
    ));
    let mut operation_number = base_request();
    set(&mut operation_number["params"], "operationId", json!(7));
    cases.push(case(
        "request-numeric-operation-id",
        FrameKind::Request,
        frame(&operation_number),
        "invalid_request",
    ));

    for (id, value) in [
        ("request-negative-expected-revision", json!(-1)),
        ("request-boolean-expected-revision", json!(true)),
        ("request-float-expected-revision", json!(1.0)),
        ("request-large-expected-revision", json!(MAX_REVISION + 1)),
    ] {
        let mut revision = base_request();
        set(&mut revision["params"], "expectedRevision", value);
        cases.push(case(
            id,
            FrameKind::Request,
            frame(&revision),
            "invalid_request",
        ));
    }

    let mut excessive_depth = base_request();
    set(
        &mut excessive_depth["params"],
        "value",
        array_depth(MAX_NESTING_DEPTH - 1),
    );
    cases.push(case(
        "request-excessive-depth",
        FrameKind::Request,
        frame(&excessive_depth),
        "invalid_request",
    ));

    let mut oversized_string = base_request();
    set(
        &mut oversized_string["params"],
        "value",
        Value::String("x".repeat(MAX_STRING_BYTES + 1)),
    );
    cases.push(case(
        "request-oversized-string",
        FrameKind::Request,
        frame(&oversized_string),
        "invalid_request",
    ));

    let mut oversized_request = base_request();
    set(
        &mut oversized_request["params"],
        "first",
        Value::String("a".repeat(MAX_STRING_BYTES)),
    );
    set(
        &mut oversized_request["params"],
        "second",
        Value::String("b".repeat(MAX_STRING_BYTES)),
    );
    cases.push(case(
        "request-oversized-frame",
        FrameKind::Request,
        frame(&oversized_request),
        "invalid_request",
    ));
    let mut oversized_raw = vec![b' '; MAX_REQUEST_FRAME_BYTES];
    oversized_raw.push(b'\n');
    cases.push(case(
        "request-oversized-raw",
        FrameKind::Request,
        oversized_raw,
        "invalid_request",
    ));

    let mut response_unknown = base_success();
    set(&mut response_unknown, "extra", json!(true));
    cases.push(case(
        "response-unknown-field",
        FrameKind::Response,
        frame(&response_unknown),
        "invalid_request",
    ));

    let mut response_version = base_success();
    set(&mut response_version, "version", json!(1.0));
    cases.push(case(
        "response-version-float",
        FrameKind::Response,
        frame(&response_version),
        "unsupported_version",
    ));

    for (id, field, value) in [
        ("response-boolean-revision", "revision", json!(true)),
        (
            "response-large-revision",
            "revision",
            json!(MAX_REVISION + 1),
        ),
        ("response-numeric-ok", "ok", json!(1)),
    ] {
        let mut response = base_success();
        set(&mut response, field, value);
        cases.push(case(
            id,
            FrameKind::Response,
            frame(&response),
            "invalid_request",
        ));
    }

    let bad_errors = [
        (
            "response-unknown-error-code",
            json!({"code":"future_code","message":"Safe message","retryable":false}),
        ),
        (
            "response-unsafe-message",
            json!({"code":"conflict","message":"Приватно","retryable":false}),
        ),
        (
            "response-bad-retryable",
            json!({"code":"conflict","message":"Safe message","retryable":"yes"}),
        ),
        (
            "response-bad-details",
            json!({"code":"conflict","message":"Safe message","retryable":false,"details":[]}),
        ),
        (
            "response-missing-error-field",
            json!({"code":"conflict","message":"Safe message"}),
        ),
    ];
    for (id, error) in bad_errors {
        let response = json!({
            "api": "omavless.control",
            "version": 1,
            "id": "error-1",
            "ok": false,
            "revision": 0,
            "error": error,
        });
        cases.push(case(
            id,
            FrameKind::Response,
            frame(&response),
            "invalid_request",
        ));
    }

    let result: serde_json::Map<String, Value> = (0..8)
        .map(|index| {
            (
                index.to_string(),
                Value::String("x".repeat(MAX_STRING_BYTES)),
            )
        })
        .collect();
    let mut oversized_response = base_success();
    set(&mut oversized_response, "result", Value::Object(result));
    cases.push(case(
        "response-oversized-frame",
        FrameKind::Response,
        frame(&oversized_response),
        "invalid_request",
    ));

    cases
}

fn rust_outcome(case: &DifferentialCase) -> AdapterOutcome {
    let result = match case.kind {
        FrameKind::Request => decode_request(&case.frame),
        FrameKind::Response => decode_response(&case.frame),
    };
    match result {
        Ok(value) => {
            if let Some(expected) = &case.expected_value {
                assert_eq!(&value, expected, "Rust semantic value for {}", case.id);
            }
            AdapterOutcome {
                accepted: true,
                classification: "accepted".to_owned(),
            }
        }
        Err(error) => AdapterOutcome {
            accepted: false,
            classification: error.code().as_str().to_owned(),
        },
    }
}

fn python_outcome(case: &DifferentialCase) -> (AdapterOutcome, Vec<u8>, Vec<u8>) {
    let adapter = root().join("tools/control_protocol_parity.py");
    let kind = match case.kind {
        FrameKind::Request => "request",
        FrameKind::Response => "response",
    };
    let mut child = Command::new("python3")
        .arg(adapter)
        .arg(kind)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python parity adapter");
    child
        .stdin
        .take()
        .expect("adapter stdin")
        .write_all(&case.frame)
        .expect("write frame");
    let output = child.wait_with_output().expect("adapter result");
    assert!(output.status.success(), "adapter failed for {}", case.id);
    let outcome = serde_json::from_slice(&output.stdout).expect("sanitized adapter output");
    (outcome, output.stdout, output.stderr)
}

fn report(cases: &[(String, AdapterOutcome)]) -> Report {
    Report {
        api: "omavless.parity".to_owned(),
        version: 1,
        suite: "control-protocol-v1".to_owned(),
        cases: cases
            .iter()
            .map(|(id, outcome)| CaseResult {
                id: id.clone(),
                classification: outcome.classification.clone(),
                fingerprint: None,
                facts: PublicFacts(BTreeMap::from([(
                    "accepted".to_owned(),
                    PublicValue::Bool(outcome.accepted),
                )])),
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_match_the_v1_corpus_and_boundary_matrix() {
    let cases = differential_cases();
    assert_eq!(
        cases.len(),
        57,
        "the differential matrix changes explicitly"
    );
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(
            rust_outcome.classification, case.expected,
            "Rust {}",
            case.id
        );
        assert_eq!(
            python_outcome.classification, case.expected,
            "Python {}",
            case.id
        );
        assert_eq!(rust_outcome.accepted, case.expected == "accepted");
        assert_eq!(python_outcome.accepted, case.expected == "accepted");
        if case.id == "request-private-malformed" {
            assert!(!stdout.windows(8).any(|window| window == b"password"));
            assert!(!stderr.windows(8).any(|window| window == b"password"));
        }
        rust.push((case.id.clone(), rust_outcome));
        python.push((case.id.clone(), python_outcome));
    }

    let summary = compare_reports(&report(&python), &report(&rust)).expect("compatible reports");
    assert!(summary.matched);
    assert_eq!(summary.case_count, cases.len());
    assert_eq!(summary.mismatch_count, 0);
}

#[test]
fn protocol_errors_are_fixed_and_never_echo_raw_input() {
    let marker = b"vless://private.invalid?password=secret";
    let mut frame = b"{broken:".to_vec();
    frame.extend_from_slice(marker);
    frame.extend_from_slice(b"}\n");
    let error = decode_request(&frame).expect_err("malformed private frame");
    assert_eq!(error.code(), StableErrorCode::InvalidRequest);
    assert!(!error.to_string().contains("vless"));
    assert!(!error.to_string().contains("password"));
}
