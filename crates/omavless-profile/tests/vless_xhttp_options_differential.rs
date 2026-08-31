// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless_xhttp_extra::{
    XhttpOptionsError, XhttpOptionsFacts, XhttpRange, parse_xhttp_options,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug)]
struct Case {
    id: &'static str,
    raw: String,
    classification: &'static str,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    normalized_field_count: usize,
    header_count: usize,
    x_padding_bytes: String,
    uplink_chunk_size: String,
    sc_max_each_post_bytes: String,
    sc_min_posts_interval_ms: String,
    x_padding_obfs_mode: bool,
    no_grpc_header: bool,
    x_padding_placement: String,
    x_padding_method: String,
    uplink_http_method: String,
    seq_placement: String,
    uplink_data_placement: String,
    x_padding_key_present: bool,
    x_padding_header_present: bool,
    seq_key_present: bool,
    uplink_data_key_present: bool,
    session_placement: String,
    session_key_present: bool,
    session_table_present: bool,
    session_length: String,
    reuse_field_count: usize,
    max_concurrency: String,
    max_connections: String,
    c_max_reuse_times: String,
    h_max_request_times: String,
    h_max_reusable_secs: String,
    h_keep_alive_present: bool,
    h_keep_alive_period: i32,
}

impl Outcome {
    fn accepted(facts: XhttpOptionsFacts) -> Self {
        Self {
            accepted: true,
            classification: "accepted".to_owned(),
            normalized_field_count: facts.normalized_field_count,
            header_count: facts.header_count,
            x_padding_bytes: range_code(facts.x_padding_bytes),
            uplink_chunk_size: range_code(facts.uplink_chunk_size),
            sc_max_each_post_bytes: range_code(facts.sc_max_each_post_bytes),
            sc_min_posts_interval_ms: range_code(facts.sc_min_posts_interval_ms),
            x_padding_obfs_mode: facts.x_padding_obfs_mode,
            no_grpc_header: facts.no_grpc_header,
            x_padding_placement: facts
                .x_padding_placement
                .map_or("none", |value| value.code())
                .to_owned(),
            x_padding_method: facts
                .x_padding_method
                .map_or("none", |value| value.code())
                .to_owned(),
            uplink_http_method: facts
                .uplink_http_method
                .map_or("none", |value| value.code())
                .to_owned(),
            seq_placement: facts
                .seq_placement
                .map_or("none", |value| value.code())
                .to_owned(),
            uplink_data_placement: facts
                .uplink_data_placement
                .map_or("none", |value| value.code())
                .to_owned(),
            x_padding_key_present: facts.x_padding_key_present,
            x_padding_header_present: facts.x_padding_header_present,
            seq_key_present: facts.seq_key_present,
            uplink_data_key_present: facts.uplink_data_key_present,
            session_placement: facts
                .session_placement
                .map_or("none", |value| value.code())
                .to_owned(),
            session_key_present: facts.session_key_present,
            session_table_present: facts.session_table_present,
            session_length: range_code(facts.session_length),
            reuse_field_count: facts.reuse_field_count,
            max_concurrency: range_code(facts.max_concurrency),
            max_connections: range_code(facts.max_connections),
            c_max_reuse_times: range_code(facts.c_max_reuse_times),
            h_max_request_times: range_code(facts.h_max_request_times),
            h_max_reusable_secs: range_code(facts.h_max_reusable_secs),
            h_keep_alive_present: facts.h_keep_alive_period.is_some(),
            h_keep_alive_period: facts.h_keep_alive_period.unwrap_or_default(),
        }
    }

    fn rejected(classification: &str) -> Self {
        Self {
            accepted: false,
            classification: classification.to_owned(),
            normalized_field_count: 0,
            header_count: 0,
            x_padding_bytes: "none".to_owned(),
            uplink_chunk_size: "none".to_owned(),
            sc_max_each_post_bytes: "none".to_owned(),
            sc_min_posts_interval_ms: "none".to_owned(),
            x_padding_obfs_mode: false,
            no_grpc_header: false,
            x_padding_placement: "none".to_owned(),
            x_padding_method: "none".to_owned(),
            uplink_http_method: "none".to_owned(),
            seq_placement: "none".to_owned(),
            uplink_data_placement: "none".to_owned(),
            x_padding_key_present: false,
            x_padding_header_present: false,
            seq_key_present: false,
            uplink_data_key_present: false,
            session_placement: "none".to_owned(),
            session_key_present: false,
            session_table_present: false,
            session_length: "none".to_owned(),
            reuse_field_count: 0,
            max_concurrency: "none".to_owned(),
            max_connections: "none".to_owned(),
            c_max_reuse_times: "none".to_owned(),
            h_max_request_times: "none".to_owned(),
            h_max_reusable_secs: "none".to_owned(),
            h_keep_alive_present: false,
            h_keep_alive_period: 0,
        }
    }
}

fn range_code(value: Option<XhttpRange>) -> String {
    match value {
        None => "none".to_owned(),
        Some(value) if value.is_single() => value.start.to_string(),
        Some(value) => format!("{}-{}", value.start, value.end),
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn case(id: &'static str, raw: impl Into<String>, classification: &'static str) -> Case {
    Case {
        id,
        raw: raw.into(),
        classification,
    }
}

fn header_object(count: usize) -> String {
    let fields = (0..count)
        .map(|index| format!(r#""X-Test-{index}":"value-{index}""#))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"headers":{{{fields}}}}}"#)
}

fn cases() -> Vec<Case> {
    let mut values = vec![
        case("empty-input", "", "accepted"),
        case("empty-object", "{}", "accepted"),
        case(
            "compatibility-defaults",
            r#"{"host":"","path":null,"mode":"auto","extra":null,"noSSEHeader":false,"scMaxBufferedPosts":0,"scStreamUpServerSecs":"0","serverMaxHeaderBytes":"","downloadSettings":null}"#,
            "accepted",
        ),
        case(
            "python-equality-defaults",
            r#"{"noSSEHeader":0.0,"scMaxBufferedPosts":false,"scStreamUpServerSecs":-0.0,"serverMaxHeaderBytes":0.0}"#,
            "accepted",
        ),
        case(
            "ignored-private-compatibility-values",
            r#"{"host":"private-host.invalid","path":"/private-secret","mode":"stream-up"}"#,
            "accepted",
        ),
        case("headers-null", r#"{"headers":null}"#, "accepted"),
        case("headers-empty", r#"{"headers":{}}"#, "accepted"),
        case(
            "headers-one",
            r#"{"headers":{"X-Test":"private-secret"}}"#,
            "accepted",
        ),
        case(
            "header-boundaries",
            format!(
                r#"{{"headers":{{"{}":"{}"}}}}"#,
                "N".repeat(64),
                "V".repeat(1024)
            ),
            "accepted",
        ),
        case(
            "ranges-integers",
            r#"{"xPaddingBytes":1,"uplinkChunkSize":2,"scMaxEachPostBytes":3,"scMinPostsIntervalMs":4}"#,
            "accepted",
        ),
        case(
            "ranges-strings",
            r#"{"xPaddingBytes":"1-2","uplinkChunkSize":"000","scMaxEachPostBytes":"4-4","scMinPostsIntervalMs":"5"}"#,
            "accepted",
        ),
        case("range-max", r#"{"xPaddingBytes":2147483647}"#, "accepted"),
        case(
            "range-empty-python-values",
            r#"{"xPaddingBytes":false,"uplinkChunkSize":0.0,"scMaxEachPostBytes":-0,"scMinPostsIntervalMs":"0"}"#,
            "accepted",
        ),
        case(
            "booleans-true",
            r#"{"xPaddingObfsMode":true,"noGRPCHeader":true}"#,
            "accepted",
        ),
        case(
            "booleans-false",
            r#"{"xPaddingObfsMode":false,"noGRPCHeader":false}"#,
            "accepted",
        ),
        case(
            "enum-all",
            r#"{"xPaddingPlacement":"queryInHeader","xPaddingMethod":"repeat-x","uplinkHTTPMethod":"patch","seqPlacement":"header","uplinkDataPlacement":"body","sessionPlacement":"header"}"#,
            "accepted",
        ),
        case(
            "http-method-post",
            r#"{"uplinkHTTPMethod":"post"}"#,
            "accepted",
        ),
        case(
            "http-method-put",
            r#"{"uplinkHTTPMethod":"PuT"}"#,
            "accepted",
        ),
        case(
            "http-method-delete",
            r#"{"uplinkHTTPMethod":"delete"}"#,
            "accepted",
        ),
        case(
            "tokens",
            r#"{"xPaddingKey":"pad-key","xPaddingHeader":"X-Pad","seqKey":"seq_key","uplinkDataKey":"data.key"}"#,
            "accepted",
        ),
        case(
            "token-boundary",
            format!(r#"{{"xPaddingKey":"{}"}}"#, "a".repeat(64)),
            "accepted",
        ),
        case(
            "session-id-aliases",
            r#"{"sessionIDPlacement":"query","sessionIDKey":"session-key","sessionIDTable":"session.table","sessionIDLength":"8-16","seqPlacement":"query"}"#,
            "accepted",
        ),
        case(
            "session-legacy-aliases",
            r#"{"sessionPlacement":"cookie","sessionKey":"session-key","sessionTable":"session.table","sessionLength":12,"seqPlacement":"header"}"#,
            "accepted",
        ),
        case(
            "session-equal-aliases",
            r#"{"sessionIDPlacement":"header","sessionPlacement":"header","sessionIDKey":"key","sessionKey":"key","sessionIDTable":"table","sessionTable":"table","sessionIDLength":8,"sessionLength":8,"seqPlacement":"header"}"#,
            "accepted",
        ),
        case(
            "session-default-path",
            r#"{"sessionPlacement":"path","seqPlacement":"path"}"#,
            "accepted",
        ),
        case(
            "session-length-zero-range-string",
            r#"{"sessionLength":"0-0"}"#,
            "accepted",
        ),
        case(
            "session-table-boundary",
            format!(r#"{{"sessionTable":"{}"}}"#, "t".repeat(128)),
            "accepted",
        ),
        case("xmux-null", r#"{"xmux":null}"#, "accepted"),
        case("xmux-empty", r#"{"xmux":{}}"#, "accepted"),
        case(
            "xmux-concurrency",
            r#"{"xmux":{"maxConcurrency":"1-4","cMaxReuseTimes":5,"hMaxRequestTimes":"6","hMaxReusableSecs":"7-8","hKeepAlivePeriod":9}}"#,
            "accepted",
        ),
        case(
            "xmux-connections",
            r#"{"xmux":{"maxConnections":3}}"#,
            "accepted",
        ),
        case(
            "xmux-keepalive-minus-one",
            r#"{"xmux":{"hKeepAlivePeriod":-1}}"#,
            "accepted",
        ),
        case(
            "xmux-keepalive-max",
            r#"{"xmux":{"hKeepAlivePeriod":86400}}"#,
            "accepted",
        ),
        case(
            "xmux-keepalive-zero-alone",
            r#"{"xmux":{"hKeepAlivePeriod":0}}"#,
            "accepted",
        ),
        case(
            "xmux-keepalive-zero-retained",
            r#"{"xmux":{"maxConcurrency":1,"hKeepAlivePeriod":0}}"#,
            "accepted",
        ),
        case(
            "combined-private-values",
            r#"{"headers":{"X-Private":"private-secret"},"xPaddingBytes":"1-2","xPaddingObfsMode":true,"xPaddingKey":"private-token","xPaddingHeader":"X-Private","xPaddingPlacement":"header","xPaddingMethod":"tokenish","uplinkHTTPMethod":"POST","seqPlacement":"query","seqKey":"private-seq","uplinkDataPlacement":"cookie","uplinkDataKey":"private-data","uplinkChunkSize":3,"noGRPCHeader":true,"scMaxEachPostBytes":4,"scMinPostsIntervalMs":5,"sessionPlacement":"query","sessionKey":"private-session","sessionTable":"private-table","sessionLength":6,"xmux":{"maxConcurrency":7,"hKeepAlivePeriod":8}}"#,
            "accepted",
        ),
        case(
            "unknown-field",
            r#"{"unknown":"private-secret"}"#,
            "unsupported_fields",
        ),
        case(
            "compat-host-type",
            r#"{"host":1}"#,
            "compatibility_field_format",
        ),
        case(
            "compat-path-type",
            r#"{"path":false}"#,
            "compatibility_field_format",
        ),
        case(
            "compat-mode-invalid",
            r#"{"mode":"invalid"}"#,
            "compatibility_mode",
        ),
        case(
            "compat-mode-type",
            r#"{"mode":false}"#,
            "compatibility_mode",
        ),
        case(
            "recursive-extra-object",
            r#"{"extra":{}}"#,
            "recursive_extra",
        ),
        case(
            "recursive-extra-false",
            r#"{"extra":false}"#,
            "recursive_extra",
        ),
        case(
            "server-no-sse",
            r#"{"noSSEHeader":true}"#,
            "server_only_field",
        ),
        case(
            "server-buffered-posts",
            r#"{"scMaxBufferedPosts":1}"#,
            "server_only_field",
        ),
        case(
            "server-stream-seconds",
            r#"{"scStreamUpServerSecs":"1"}"#,
            "server_only_field",
        ),
        case(
            "server-header-bytes",
            r#"{"serverMaxHeaderBytes":1}"#,
            "server_only_field",
        ),
        case(
            "download-object",
            r#"{"downloadSettings":{}}"#,
            "download_settings_outside_slice",
        ),
        case(
            "download-false",
            r#"{"downloadSettings":false}"#,
            "download_settings_outside_slice",
        ),
        case("headers-array", r#"{"headers":[]}"#, "headers_format"),
        case("headers-too-many", header_object(33), "headers_format"),
        case(
            "header-host",
            r#"{"headers":{"Host":"example.invalid"}}"#,
            "header_name",
        ),
        case(
            "header-name-space",
            r#"{"headers":{"X Test":"value"}}"#,
            "header_name",
        ),
        case(
            "header-name-unicode",
            r#"{"headers":{"X-é":"value"}}"#,
            "header_name",
        ),
        case(
            "header-name-too-long",
            format!(r#"{{"headers":{{"{}":"value"}}}}"#, "N".repeat(65)),
            "header_name",
        ),
        case(
            "header-value-type",
            r#"{"headers":{"X-Test":1}}"#,
            "header_value",
        ),
        case(
            "header-value-unicode",
            r#"{"headers":{"X-Test":"é"}}"#,
            "header_value",
        ),
        case(
            "header-value-control",
            "{\"headers\":{\"X-Test\":\"a\\nb\"}}",
            "header_value",
        ),
        case(
            "header-value-too-long",
            format!(r#"{{"headers":{{"X-Test":"{}"}}}}"#, "v".repeat(1025)),
            "header_value",
        ),
        case("range-bool-true", r#"{"xPaddingBytes":true}"#, "range_type"),
        case("range-float", r#"{"xPaddingBytes":1.0}"#, "range_type"),
        case("range-object", r#"{"xPaddingBytes":{}}"#, "range_type"),
        case(
            "range-string-malformed",
            r#"{"xPaddingBytes":"1--2"}"#,
            "range_type",
        ),
        case(
            "range-string-negative",
            r#"{"xPaddingBytes":"-1"}"#,
            "range_type",
        ),
        case(
            "range-negative-int",
            r#"{"xPaddingBytes":-1}"#,
            "range_bounds",
        ),
        case(
            "range-reversed",
            r#"{"xPaddingBytes":"2-1"}"#,
            "range_bounds",
        ),
        case(
            "range-over-max",
            r#"{"xPaddingBytes":2147483648}"#,
            "range_bounds",
        ),
        case(
            "range-huge",
            r#"{"xPaddingBytes":123456789012345678901234567890}"#,
            "range_bounds",
        ),
        case("boolean-int", r#"{"xPaddingObfsMode":0}"#, "boolean_type"),
        case(
            "boolean-string",
            r#"{"noGRPCHeader":"false"}"#,
            "boolean_type",
        ),
        case(
            "enum-padding-placement",
            r#"{"xPaddingPlacement":"path"}"#,
            "enum_value",
        ),
        case(
            "enum-padding-method",
            r#"{"xPaddingMethod":"repeat"}"#,
            "enum_value",
        ),
        case(
            "enum-http-method",
            r#"{"uplinkHTTPMethod":"GET"}"#,
            "enum_value",
        ),
        case(
            "enum-seq-placement",
            r#"{"seqPlacement":"auto"}"#,
            "enum_value",
        ),
        case(
            "enum-data-placement",
            r#"{"uplinkDataPlacement":"path"}"#,
            "enum_value",
        ),
        case("enum-type", r#"{"xPaddingPlacement":1}"#, "enum_value"),
        case(
            "token-space",
            r#"{"xPaddingKey":"private secret"}"#,
            "token_format",
        ),
        case("token-unicode", r#"{"seqKey":"секрет"}"#, "token_format"),
        case("token-type", r#"{"uplinkDataKey":1}"#, "token_format"),
        case(
            "token-too-long",
            format!(r#"{{"xPaddingHeader":"{}"}}"#, "x".repeat(65)),
            "token_format",
        ),
        case(
            "session-table-space",
            r#"{"sessionTable":"private table"}"#,
            "token_format",
        ),
        case(
            "session-table-too-long",
            format!(r#"{{"sessionTable":"{}"}}"#, "t".repeat(129)),
            "token_format",
        ),
        case(
            "session-placement-invalid",
            r#"{"sessionPlacement":"auto"}"#,
            "session_placement",
        ),
        case(
            "session-placement-type",
            r#"{"sessionPlacement":1}"#,
            "session_placement",
        ),
        case(
            "session-alias-conflict-placement",
            r#"{"sessionIDPlacement":"query","sessionPlacement":"header"}"#,
            "alias_conflict",
        ),
        case(
            "session-alias-conflict-key",
            r#"{"sessionIDKey":"a","sessionKey":"b"}"#,
            "alias_conflict",
        ),
        case(
            "session-alias-conflict-table",
            r#"{"sessionIDTable":"a","sessionTable":"b"}"#,
            "alias_conflict",
        ),
        case(
            "session-alias-conflict-length",
            r#"{"sessionIDLength":1,"sessionLength":2}"#,
            "alias_conflict",
        ),
        case(
            "session-alias-python-equal-then-type",
            r#"{"sessionIDLength":true,"sessionLength":1}"#,
            "range_type",
        ),
        case(
            "session-sequence-conflict-default",
            r#"{"seqPlacement":"query"}"#,
            "session_sequence_conflict",
        ),
        case(
            "session-sequence-conflict-explicit",
            r#"{"sessionPlacement":"path","seqPlacement":"header"}"#,
            "session_sequence_conflict",
        ),
        case("xmux-array", r#"{"xmux":[]}"#, "xmux_object"),
        case("xmux-unknown", r#"{"xmux":{"unknown":1}}"#, "xmux_fields"),
        case(
            "xmux-exclusive",
            r#"{"xmux":{"maxConcurrency":1,"maxConnections":1}}"#,
            "xmux_exclusive",
        ),
        case(
            "xmux-keepalive-bool",
            r#"{"xmux":{"hKeepAlivePeriod":false}}"#,
            "xmux_keep_alive",
        ),
        case(
            "xmux-keepalive-float",
            r#"{"xmux":{"hKeepAlivePeriod":0.0}}"#,
            "xmux_keep_alive",
        ),
        case(
            "xmux-keepalive-low",
            r#"{"xmux":{"hKeepAlivePeriod":-2}}"#,
            "xmux_keep_alive",
        ),
        case(
            "xmux-keepalive-high",
            r#"{"xmux":{"hKeepAlivePeriod":86401}}"#,
            "xmux_keep_alive",
        ),
        case(
            "xmux-keepalive-huge",
            r#"{"xmux":{"hKeepAlivePeriod":123456789012345678901234567890}}"#,
            "xmux_keep_alive",
        ),
        case(
            "ordering-unknown-before-compat",
            r#"{"unknown":1,"host":1}"#,
            "unsupported_fields",
        ),
        case(
            "ordering-compat-before-recursive",
            r#"{"host":1,"extra":{}}"#,
            "compatibility_field_format",
        ),
        case(
            "ordering-recursive-before-server",
            r#"{"extra":{},"noSSEHeader":true}"#,
            "recursive_extra",
        ),
        case(
            "ordering-server-before-download",
            r#"{"noSSEHeader":true,"downloadSettings":{}}"#,
            "server_only_field",
        ),
        case(
            "ordering-download-before-headers",
            r#"{"downloadSettings":{},"headers":[]}"#,
            "download_settings_outside_slice",
        ),
        case(
            "ordering-headers-before-range",
            r#"{"headers":[],"xPaddingBytes":true}"#,
            "headers_format",
        ),
        case(
            "ordering-range-before-boolean",
            r#"{"xPaddingBytes":true,"xPaddingObfsMode":0}"#,
            "range_type",
        ),
        case(
            "ordering-boolean-before-enum",
            r#"{"xPaddingObfsMode":0,"xPaddingPlacement":"bad"}"#,
            "boolean_type",
        ),
        case(
            "ordering-enum-before-token",
            r#"{"xPaddingPlacement":"bad","xPaddingKey":"bad value"}"#,
            "enum_value",
        ),
        case(
            "ordering-token-before-alias",
            r#"{"xPaddingKey":"bad value","sessionIDKey":"a","sessionKey":"b"}"#,
            "token_format",
        ),
        case(
            "ordering-alias-before-sequence",
            r#"{"sessionIDPlacement":"query","sessionPlacement":"header","seqPlacement":"query"}"#,
            "alias_conflict",
        ),
        case(
            "ordering-sequence-before-xmux",
            r#"{"seqPlacement":"query","xmux":[]}"#,
            "session_sequence_conflict",
        ),
        case(
            "malformed-json",
            r#"{"xPaddingKey":"private-secret",}"#,
            "invalid_json",
        ),
        case("non-object-root", "[]", "non_object_root"),
    ];
    values.push(case("headers-max", header_object(32), "accepted"));
    values.push(case(
        "raw-size-overflow",
        " ".repeat(12 * 1024 + 1),
        "too_large",
    ));
    values
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_xhttp_options(&case.raw) {
        Ok(options) => Outcome::accepted(options.facts()),
        Err(error) => Outcome::rejected(error.code()),
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let request =
        serde_json::to_vec(&serde_json::json!({"raw": case.raw})).expect("adapter request");
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_xhttp_options_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python XHTTP-options adapter");
    {
        let mut stdin = child.stdin.take().expect("adapter stdin");
        stdin.write_all(&request).expect("write adapter input");
    }
    let output = child.wait_with_output().expect("adapter output");
    assert!(
        output.status.success(),
        "Python adapter failed for {}: {}",
        case.id,
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome = serde_json::from_slice(&output.stdout).expect("sanitized adapter JSON");
    (outcome, output.stdout, output.stderr)
}

fn report(values: &[(String, Outcome)]) -> Report {
    Report {
        api: "omavless.parity".to_owned(),
        version: 1,
        suite: "vless-xhttp-options-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| {
                let mut facts = BTreeMap::new();
                facts.insert("accepted".to_owned(), PublicValue::Bool(outcome.accepted));
                facts.insert(
                    "normalized_field_count".to_owned(),
                    PublicValue::Integer(outcome.normalized_field_count as i64),
                );
                facts.insert(
                    "header_count".to_owned(),
                    PublicValue::Integer(outcome.header_count as i64),
                );
                for (key, value) in [
                    ("x_padding_bytes", &outcome.x_padding_bytes),
                    ("uplink_chunk_size", &outcome.uplink_chunk_size),
                    ("sc_max_each_post_bytes", &outcome.sc_max_each_post_bytes),
                    (
                        "sc_min_posts_interval_ms",
                        &outcome.sc_min_posts_interval_ms,
                    ),
                    ("x_padding_placement", &outcome.x_padding_placement),
                    ("x_padding_method", &outcome.x_padding_method),
                    ("uplink_http_method", &outcome.uplink_http_method),
                    ("seq_placement", &outcome.seq_placement),
                    ("uplink_data_placement", &outcome.uplink_data_placement),
                    ("session_placement", &outcome.session_placement),
                    ("session_length", &outcome.session_length),
                    ("max_concurrency", &outcome.max_concurrency),
                    ("max_connections", &outcome.max_connections),
                    ("c_max_reuse_times", &outcome.c_max_reuse_times),
                    ("h_max_request_times", &outcome.h_max_request_times),
                    ("h_max_reusable_secs", &outcome.h_max_reusable_secs),
                ] {
                    facts.insert(key.to_owned(), PublicValue::Text(value.clone()));
                }
                for (key, value) in [
                    ("x_padding_obfs_mode", outcome.x_padding_obfs_mode),
                    ("no_grpc_header", outcome.no_grpc_header),
                    ("x_padding_key_present", outcome.x_padding_key_present),
                    ("x_padding_header_present", outcome.x_padding_header_present),
                    ("seq_key_present", outcome.seq_key_present),
                    ("uplink_data_key_present", outcome.uplink_data_key_present),
                    ("session_key_present", outcome.session_key_present),
                    ("session_table_present", outcome.session_table_present),
                    ("h_keep_alive_present", outcome.h_keep_alive_present),
                ] {
                    facts.insert(key.to_owned(), PublicValue::Bool(value));
                }
                facts.insert(
                    "reuse_field_count".to_owned(),
                    PublicValue::Integer(outcome.reuse_field_count as i64),
                );
                facts.insert(
                    "h_keep_alive_period".to_owned(),
                    PublicValue::Integer(i64::from(outcome.h_keep_alive_period)),
                );
                CaseResult {
                    id: id.clone(),
                    classification: outcome.classification.clone(),
                    fingerprint: None,
                    facts: PublicFacts(facts),
                }
            })
            .collect(),
    }
}

#[test]
fn python_and_rust_xhttp_options_match() {
    let cases = cases();
    assert!(cases.len() >= 100);
    let mut rust = Vec::new();
    let mut python = Vec::new();
    for case in &cases {
        let rust_outcome = rust_outcome(case);
        let (python_outcome, stdout, stderr) = python_outcome(case);
        assert_eq!(
            rust_outcome.classification, case.classification,
            "Rust classification {}",
            case.id
        );
        assert_eq!(
            python_outcome.classification, case.classification,
            "Python classification {}",
            case.id
        );
        assert_eq!(rust_outcome, python_outcome, "differential {}", case.id);
        for marker in [
            b"private-secret".as_slice(),
            b"private-token".as_slice(),
            b"private-host.invalid".as_slice(),
            b"vless://".as_slice(),
        ] {
            assert!(!stdout.windows(marker.len()).any(|part| part == marker));
            assert!(!stderr.windows(marker.len()).any(|part| part == marker));
        }
        assert!(stdout.len() <= 4096, "bounded stdout for {}", case.id);
        assert!(stderr.len() <= 512, "bounded stderr for {}", case.id);
        rust.push((case.id.to_owned(), rust_outcome));
        python.push((case.id.to_owned(), python_outcome));
    }
    let summary = compare_reports(&report(&python), &report(&rust)).expect("compatible reports");
    assert!(summary.matched);
    assert_eq!(summary.case_count, cases.len());
    assert_eq!(summary.mismatch_count, 0);
}

#[test]
fn xhttp_options_errors_and_debug_are_fixed_and_safe() {
    let options = parse_xhttp_options(
        r#"{"headers":{"X-Private":"private-secret"},"xPaddingKey":"private-token","sessionTable":"private-table"}"#,
    )
    .expect("private normalized options");
    let debug = format!("{options:?}");
    for marker in [
        "private-secret",
        "private-token",
        "private-table",
        "X-Private",
    ] {
        assert!(!debug.contains(marker));
    }

    let errors = [
        XhttpOptionsError::UnsupportedFields,
        XhttpOptionsError::CompatibilityFieldFormat,
        XhttpOptionsError::CompatibilityMode,
        XhttpOptionsError::RecursiveExtra,
        XhttpOptionsError::ServerOnlyField,
        XhttpOptionsError::DownloadSettingsOutsideSlice,
        XhttpOptionsError::HeadersFormat,
        XhttpOptionsError::HeaderName,
        XhttpOptionsError::HeaderValue,
        XhttpOptionsError::RangeType,
        XhttpOptionsError::RangeBounds,
        XhttpOptionsError::BooleanType,
        XhttpOptionsError::TokenFormat,
        XhttpOptionsError::EnumValue,
        XhttpOptionsError::AliasConflict,
        XhttpOptionsError::SessionPlacement,
        XhttpOptionsError::SessionSequenceConflict,
        XhttpOptionsError::XmuxObject,
        XhttpOptionsError::XmuxFields,
        XhttpOptionsError::XmuxExclusive,
        XhttpOptionsError::XmuxKeepAlive,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 96);
        assert!(!error.to_string().contains("private"));
        assert!(!error.code().contains("private"));
    }
}
