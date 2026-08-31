// SPDX-License-Identifier: MIT

use omavless_parity::{CaseResult, PublicFacts, PublicValue, Report, compare_reports};
use omavless_profile::vless::HostKind;
use omavless_profile::vless_xhttp_extra::{
    XhttpDownloadError, XhttpDownloadFacts, XhttpDownloadMode, XhttpDownloadSecurity, XhttpRange,
    parse_xhttp_download_settings,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    raw: String,
    main_mode: String,
    main_security: String,
    classification: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    normalized_field_count: usize,
    server_kind: String,
    port_present: bool,
    port: u16,
    tls_present: bool,
    tls: bool,
    servername_kind: String,
    alpn_count: usize,
    alpn_h2: bool,
    alpn_h3: bool,
    alpn_http_1_1: bool,
    fingerprint_present: bool,
    skip_cert_verify_present: bool,
    skip_cert_verify: bool,
    reality_present: bool,
    reality_short_id_present: bool,
    reality_pq_enabled: bool,
    reality_spider_compatibility: bool,
    reality_pq_compatibility: bool,
    path_present: bool,
    host_kind: String,
    header_count: usize,
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
    fn accepted(facts: XhttpDownloadFacts) -> Self {
        Self {
            accepted: true,
            classification: "accepted".to_owned(),
            normalized_field_count: facts.normalized_field_count,
            server_kind: host_kind_code(facts.server_kind).to_owned(),
            port_present: facts.port.is_some(),
            port: facts.port.unwrap_or_default(),
            tls_present: facts.tls.is_some(),
            tls: facts.tls.unwrap_or_default(),
            servername_kind: host_kind_code(facts.servername_kind).to_owned(),
            alpn_count: facts.alpn_count,
            alpn_h2: facts.alpn_h2,
            alpn_h3: facts.alpn_h3,
            alpn_http_1_1: facts.alpn_http_1_1,
            fingerprint_present: facts.fingerprint_present,
            skip_cert_verify_present: facts.skip_cert_verify.is_some(),
            skip_cert_verify: facts.skip_cert_verify.unwrap_or_default(),
            reality_present: facts.reality_present,
            reality_short_id_present: facts.reality_short_id_present,
            reality_pq_enabled: facts.reality_pq_enabled,
            reality_spider_compatibility: facts.reality_spider_compatibility,
            reality_pq_compatibility: facts.reality_pq_compatibility,
            path_present: facts.path_present,
            host_kind: host_kind_code(facts.host_kind).to_owned(),
            header_count: facts.header_count,
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
            server_kind: "none".to_owned(),
            port_present: false,
            port: 0,
            tls_present: false,
            tls: false,
            servername_kind: "none".to_owned(),
            alpn_count: 0,
            alpn_h2: false,
            alpn_h3: false,
            alpn_http_1_1: false,
            fingerprint_present: false,
            skip_cert_verify_present: false,
            skip_cert_verify: false,
            reality_present: false,
            reality_short_id_present: false,
            reality_pq_enabled: false,
            reality_spider_compatibility: false,
            reality_pq_compatibility: false,
            path_present: false,
            host_kind: "none".to_owned(),
            header_count: 0,
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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cases() -> Vec<Case> {
    let source =
        std::fs::read_to_string(root().join("tests/parity_cases/vless-xhttp-download-v1.json"))
            .expect("credential-free XHTTP download corpus");
    serde_json::from_str(&source).expect("bounded XHTTP download corpus")
}

fn mode(value: &str) -> XhttpDownloadMode {
    match value {
        "auto" => XhttpDownloadMode::Auto,
        "stream-up" => XhttpDownloadMode::StreamUp,
        "packet-up" => XhttpDownloadMode::PacketUp,
        "stream-one" => XhttpDownloadMode::StreamOne,
        _ => panic!("invalid public mode in test corpus"),
    }
}

fn security(value: &str) -> XhttpDownloadSecurity {
    match value {
        "none" => XhttpDownloadSecurity::None,
        "tls" => XhttpDownloadSecurity::Tls,
        "reality" => XhttpDownloadSecurity::Reality,
        _ => panic!("invalid public security in test corpus"),
    }
}

fn host_kind_code(value: Option<HostKind>) -> &'static str {
    value.map_or("none", HostKind::as_str)
}

fn range_code(value: Option<XhttpRange>) -> String {
    match value {
        None => "none".to_owned(),
        Some(value) if value.is_single() => value.start.to_string(),
        Some(value) => format!("{}-{}", value.start, value.end),
    }
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_xhttp_download_settings(
        &case.raw,
        mode(&case.main_mode),
        security(&case.main_security),
    ) {
        Ok(settings) => Outcome::accepted(settings.facts()),
        Err(error) => Outcome::rejected(error.code()),
    }
}

fn python_outcome(case: &Case) -> (Outcome, Vec<u8>, Vec<u8>) {
    let request = serde_json::to_vec(&serde_json::json!({
        "raw": case.raw,
        "main_mode": case.main_mode,
        "main_security": case.main_security,
    }))
    .expect("adapter request");
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_xhttp_download_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python XHTTP-download adapter");
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
        suite: "vless-xhttp-download-v1".to_owned(),
        cases: values
            .iter()
            .map(|(id, outcome)| {
                let mut facts = BTreeMap::new();
                facts.insert("accepted".to_owned(), PublicValue::Bool(outcome.accepted));
                facts.insert(
                    "normalized_field_count".to_owned(),
                    PublicValue::Integer(outcome.normalized_field_count as i64),
                );
                for (key, value) in [
                    ("server_kind", &outcome.server_kind),
                    ("servername_kind", &outcome.servername_kind),
                    ("host_kind", &outcome.host_kind),
                    ("max_concurrency", &outcome.max_concurrency),
                    ("max_connections", &outcome.max_connections),
                    ("c_max_reuse_times", &outcome.c_max_reuse_times),
                    ("h_max_request_times", &outcome.h_max_request_times),
                    ("h_max_reusable_secs", &outcome.h_max_reusable_secs),
                ] {
                    facts.insert(key.to_owned(), PublicValue::Text(value.clone()));
                }
                for (key, value) in [
                    ("port_present", outcome.port_present),
                    ("tls_present", outcome.tls_present),
                    ("tls", outcome.tls),
                    ("alpn_h2", outcome.alpn_h2),
                    ("alpn_h3", outcome.alpn_h3),
                    ("alpn_http_1_1", outcome.alpn_http_1_1),
                    ("fingerprint_present", outcome.fingerprint_present),
                    ("skip_cert_verify_present", outcome.skip_cert_verify_present),
                    ("skip_cert_verify", outcome.skip_cert_verify),
                    ("reality_present", outcome.reality_present),
                    ("reality_short_id_present", outcome.reality_short_id_present),
                    ("reality_pq_enabled", outcome.reality_pq_enabled),
                    (
                        "reality_spider_compatibility",
                        outcome.reality_spider_compatibility,
                    ),
                    ("reality_pq_compatibility", outcome.reality_pq_compatibility),
                    ("path_present", outcome.path_present),
                    ("h_keep_alive_present", outcome.h_keep_alive_present),
                ] {
                    facts.insert(key.to_owned(), PublicValue::Bool(value));
                }
                for (key, value) in [
                    ("port", i64::from(outcome.port)),
                    ("alpn_count", outcome.alpn_count as i64),
                    ("header_count", outcome.header_count as i64),
                    ("reuse_field_count", outcome.reuse_field_count as i64),
                    (
                        "h_keep_alive_period",
                        i64::from(outcome.h_keep_alive_period),
                    ),
                ] {
                    facts.insert(key.to_owned(), PublicValue::Integer(value));
                }
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
fn python_and_rust_xhttp_download_settings_match() {
    let cases = cases();
    assert!(cases.len() >= 150);
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
            b"private-spider".as_slice(),
            b"private.invalid".as_slice(),
            b"AAECAwQF".as_slice(),
            b"vless://".as_slice(),
        ] {
            assert!(!stdout.windows(marker.len()).any(|part| part == marker));
            assert!(!stderr.windows(marker.len()).any(|part| part == marker));
        }
        assert!(stdout.len() <= 4096, "bounded stdout for {}", case.id);
        assert!(stderr.len() <= 512, "bounded stderr for {}", case.id);
        rust.push((case.id.clone(), rust_outcome));
        python.push((case.id.clone(), python_outcome));
    }
    let summary = compare_reports(&report(&python), &report(&rust)).expect("compatible reports");
    assert!(summary.matched);
    assert_eq!(summary.case_count, cases.len());
    assert_eq!(summary.mismatch_count, 0);
}

#[test]
fn xhttp_download_errors_and_debug_are_fixed_and_safe() {
    let private = r#"{
        "address":"download.private.invalid",
        "security":"reality",
        "realitySettings":{
            "publicKey":"AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
            "shortId":"a1b2c3d4",
            "serverName":"sni.private.invalid",
            "fingerprint":"firefox",
            "spiderX":"/private-spider"
        },
        "xhttpSettings":{
            "path":"/private-path",
            "host":"host.private.invalid",
            "headers":{"X-Private":"private-secret"}
        }
    }"#;
    let settings = parse_xhttp_download_settings(
        private,
        XhttpDownloadMode::StreamUp,
        XhttpDownloadSecurity::Tls,
    )
    .expect("private normalized settings");
    let debug = format!("{settings:?}");
    for marker in [
        "private-secret",
        "private-spider",
        "private.invalid",
        "AAECAwQF",
        "X-Private",
    ] {
        assert!(!debug.contains(marker));
    }

    let errors = [
        XhttpDownloadError::StreamOne,
        XhttpDownloadError::UnsupportedFields,
        XhttpDownloadError::Sockopt,
        XhttpDownloadError::EndpointFormat,
        XhttpDownloadError::Port,
        XhttpDownloadError::Network,
        XhttpDownloadError::Security,
        XhttpDownloadError::TlsObject,
        XhttpDownloadError::TlsFields,
        XhttpDownloadError::TlsSecurityConflict,
        XhttpDownloadError::TlsShow,
        XhttpDownloadError::AlpnFormat,
        XhttpDownloadError::AlpnValue,
        XhttpDownloadError::RealitySecurityConflict,
        XhttpDownloadError::RealityObject,
        XhttpDownloadError::RealityFields,
        XhttpDownloadError::RealityShow,
        XhttpDownloadError::RealityMldsa,
        XhttpDownloadError::RealityPublicKeyRequired,
        XhttpDownloadError::RealityPublicKey,
        XhttpDownloadError::RealityShortId,
        XhttpDownloadError::RealitySettingsRequired,
        XhttpDownloadError::TransportObject,
        XhttpDownloadError::TransportFields,
        XhttpDownloadError::PathFormat,
        XhttpDownloadError::Mode,
        XhttpDownloadError::ModeMismatch,
        XhttpDownloadError::TransportExtraObject,
        XhttpDownloadError::TransportExtraFields,
        XhttpDownloadError::RecursiveDownload,
        XhttpDownloadError::TransportCompatibilityFormat,
        XhttpDownloadError::TransportMode,
        XhttpDownloadError::IndependentOverride,
        XhttpDownloadError::HeadersConflict,
    ];
    for error in errors {
        assert!(error.to_string().len() <= 128);
        assert!(!error.to_string().contains("private"));
        assert!(!error.code().contains("private"));
    }
}
