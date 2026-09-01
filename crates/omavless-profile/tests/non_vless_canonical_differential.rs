// SPDX-License-Identifier: MIT

use omavless_profile::hysteria2::{Hysteria2Facts, Hysteria2Profile, parse_hysteria2};
use omavless_profile::trojan::{TrojanFacts, TrojanProfile, parse_trojan};
use omavless_profile::tuic::{TuicFacts, TuicProfile, parse_tuic};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    protocol: String,
    uri: String,
    name: String,
    server_override: Option<String>,
    classification: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    protocol: String,
    host_kind: String,
    port: u16,
    transport: String,
    security: String,
    allow_insecure: bool,
    udp: bool,
    alpn_count: usize,
    port_hopping: bool,
    obfuscation: bool,
    fingerprint_present: bool,
    ech_present: bool,
    reality_pq: bool,
    disable_sni: bool,
    congestion: String,
    udp_relay: String,
    compatibility_note_present: bool,
    identity_fingerprint: String,
    preview_fingerprint: String,
    mihomo_fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonEnvelope {
    cases: Vec<PythonCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonCase {
    id: String,
    #[serde(flatten)]
    outcome: Outcome,
}

#[derive(Serialize)]
struct PythonRequest<'a> {
    cases: Vec<PythonRequestCase<'a>>,
}

#[derive(Serialize)]
struct PythonRequestCase<'a> {
    id: &'a str,
    protocol: &'a str,
    uri: &'a str,
    name: &'a str,
    server_override: Option<&'a str>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cases() -> Vec<Case> {
    let value =
        std::fs::read_to_string(root().join("tests/parity_cases/non-vless-canonical-v1.json"))
            .expect("credential-free non-VLESS corpus");
    serde_json::from_str(&value).expect("bounded corpus")
}

fn empty(classification: &str) -> Outcome {
    Outcome {
        accepted: false,
        classification: classification.to_owned(),
        protocol: "none".to_owned(),
        host_kind: "none".to_owned(),
        port: 0,
        transport: "none".to_owned(),
        security: "none".to_owned(),
        allow_insecure: false,
        udp: false,
        alpn_count: 0,
        port_hopping: false,
        obfuscation: false,
        fingerprint_present: false,
        ech_present: false,
        reality_pq: false,
        disable_sni: false,
        congestion: "none".to_owned(),
        udp_relay: "none".to_owned(),
        compatibility_note_present: false,
        identity_fingerprint: String::new(),
        preview_fingerprint: String::new(),
        mihomo_fingerprint: String::new(),
    }
}

fn trojan_outcome(profile: &TrojanProfile, facts: TrojanFacts, case: &Case) -> Outcome {
    Outcome {
        accepted: true,
        classification: "accepted".to_owned(),
        protocol: "trojan".to_owned(),
        host_kind: facts.host_kind.as_str().to_owned(),
        port: facts.port,
        transport: facts.transport.as_str().to_owned(),
        security: facts.security.as_str().to_owned(),
        allow_insecure: facts.allow_insecure,
        udp: facts.udp,
        alpn_count: facts.alpn_count,
        port_hopping: false,
        obfuscation: false,
        fingerprint_present: facts.fingerprint_present,
        ech_present: false,
        reality_pq: facts.reality_pq,
        disable_sni: false,
        congestion: "none".to_owned(),
        udp_relay: "none".to_owned(),
        compatibility_note_present: facts.compatibility_note_present,
        identity_fingerprint: profile.subscription_identity(),
        preview_fingerprint: profile.preview_fingerprint(),
        mihomo_fingerprint: profile
            .mihomo_render_fingerprint(&case.name, case.server_override.as_deref()),
    }
}

fn hysteria2_outcome(profile: &Hysteria2Profile, facts: Hysteria2Facts, case: &Case) -> Outcome {
    Outcome {
        accepted: true,
        classification: "accepted".to_owned(),
        protocol: "hysteria2".to_owned(),
        host_kind: facts.host_kind.as_str().to_owned(),
        port: facts.port,
        transport: "quic".to_owned(),
        security: "tls".to_owned(),
        allow_insecure: facts.allow_insecure,
        udp: false,
        alpn_count: 0,
        port_hopping: facts.port_hopping,
        obfuscation: facts.obfuscation,
        fingerprint_present: facts.fingerprint_present,
        ech_present: facts.ech_present,
        reality_pq: false,
        disable_sni: false,
        congestion: "none".to_owned(),
        udp_relay: "none".to_owned(),
        compatibility_note_present: false,
        identity_fingerprint: profile.subscription_identity(),
        preview_fingerprint: profile.preview_fingerprint(),
        mihomo_fingerprint: profile
            .mihomo_render_fingerprint(&case.name, case.server_override.as_deref()),
    }
}

fn tuic_outcome(profile: &TuicProfile, facts: TuicFacts, case: &Case) -> Outcome {
    Outcome {
        accepted: true,
        classification: "accepted".to_owned(),
        protocol: "tuic".to_owned(),
        host_kind: facts.host_kind.as_str().to_owned(),
        port: facts.port,
        transport: "quic".to_owned(),
        security: "tls".to_owned(),
        allow_insecure: facts.allow_insecure,
        udp: false,
        alpn_count: facts.alpn_count,
        port_hopping: false,
        obfuscation: false,
        fingerprint_present: false,
        ech_present: false,
        reality_pq: false,
        disable_sni: facts.disable_sni,
        congestion: facts.congestion.as_str().to_owned(),
        udp_relay: facts.udp_relay.as_str().to_owned(),
        compatibility_note_present: facts.compatibility_note_present,
        identity_fingerprint: profile.subscription_identity(),
        preview_fingerprint: profile.preview_fingerprint(),
        mihomo_fingerprint: profile
            .mihomo_render_fingerprint(&case.name, case.server_override.as_deref()),
    }
}

fn rust_outcome(case: &Case) -> Outcome {
    match case.protocol.as_str() {
        "trojan" => match parse_trojan(&case.uri) {
            Ok(profile) => trojan_outcome(&profile, profile.facts(), case),
            Err(error) => empty(error.code()),
        },
        "hysteria2" => match parse_hysteria2(&case.uri) {
            Ok(profile) => hysteria2_outcome(&profile, profile.facts(), case),
            Err(error) => empty(error.code()),
        },
        "tuic" => match parse_tuic(&case.uri) {
            Ok(profile) => tuic_outcome(&profile, profile.facts(), case),
            Err(error) => empty(error.code()),
        },
        _ => empty("internal_error"),
    }
}

fn python_outcomes(cases: &[Case]) -> (Vec<PythonCase>, Vec<u8>, Vec<u8>) {
    let request = PythonRequest {
        cases: cases
            .iter()
            .map(|case| PythonRequestCase {
                id: &case.id,
                protocol: &case.protocol,
                uri: &case.uri,
                name: &case.name,
                server_override: case.server_override.as_deref(),
            })
            .collect(),
    };
    let request = serde_json::to_vec(&request).expect("adapter request");
    let mut child = Command::new("python3")
        .arg(root().join("tools/non_vless_canonical_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python non-VLESS adapter");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&request)
        .expect("write request");
    let output = child.wait_with_output().expect("adapter output");
    assert!(
        output.status.success(),
        "Python adapter failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: PythonEnvelope =
        serde_json::from_slice(&output.stdout).expect("sanitized Python output");
    (envelope.cases, output.stdout, output.stderr)
}

#[test]
fn python_and_rust_non_vless_canonical_semantics_match() {
    let cases = cases();
    assert_eq!(cases.len(), 58, "accepted R2 non-VLESS corpus");
    let (python, stdout, stderr) = python_outcomes(&cases);
    assert_eq!(python.len(), cases.len());
    assert!(stdout.len() <= 192 * 1024);
    assert!(stderr.len() <= 1024);
    for marker in [
        "trojan://",
        "hysteria2://",
        "hy2://",
        "tuic://",
        "example.invalid",
        "s3cr",
        "obfs secret",
        "22222222-2222-4222-8222-222222222222",
    ] {
        assert!(!String::from_utf8_lossy(&stdout).contains(marker));
        assert!(!String::from_utf8_lossy(&stderr).contains(marker));
    }
    for (case, python_case) in cases.iter().zip(&python) {
        assert_eq!(case.id, python_case.id);
        let rust = rust_outcome(case);
        assert_eq!(
            python_case.outcome.classification, case.classification,
            "Python classification {}",
            case.id
        );
        assert_eq!(
            rust.classification, case.classification,
            "Rust classification {}",
            case.id
        );
        assert_eq!(rust, python_case.outcome, "differential {}", case.id);
    }
    let identity = |id: &str| {
        python
            .iter()
            .find(|case| case.id == id)
            .expect("identity case")
            .outcome
            .identity_fingerprint
            .as_str()
    };
    assert_eq!(identity("trojan-identity-a"), identity("trojan-identity-b"));
    assert_eq!(identity("hy2-identity-a"), identity("hy2-identity-b"));
    assert_eq!(identity("tuic-identity-a"), identity("tuic-identity-b"));
}

#[test]
fn non_vless_errors_and_debug_are_credential_safe() {
    for (input, marker) in [
        (
            "trojan://private-secret@example.invalid:443?type=private",
            "private-secret",
        ),
        (
            "hy2://private-secret@example.invalid:443?obfs=private",
            "private-secret",
        ),
        (
            "tuic://private-secret@example.invalid:443",
            "private-secret",
        ),
    ] {
        let message = if input.starts_with("trojan") {
            parse_trojan(input).expect_err("invalid").to_string()
        } else if input.starts_with("hy2") {
            parse_hysteria2(input).expect_err("invalid").to_string()
        } else {
            parse_tuic(input).expect_err("invalid").to_string()
        };
        assert!(message.len() <= 160);
        assert!(!message.contains(marker));
        assert!(!message.contains("example.invalid"));
    }
}
