// SPDX-License-Identifier: MIT

use omavless_profile::vless_canonical::{
    VlessCanonicalFacts, VlessCanonicalProfile, parse_vless_canonical,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
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
    host_kind: String,
    port: u16,
    standard_https_port: bool,
    transport: String,
    security: String,
    flow: String,
    packet_encoding: String,
    xhttp_mode: String,
    allow_insecure: bool,
    encryption_enabled: bool,
    reality_pq: bool,
    alpn_count: usize,
    advanced_xhttp: bool,
    xhttp_field_count: usize,
    experimental_feature_count: usize,
    compatibility_note_present: bool,
    compatibility_spider: bool,
    compatibility_pq: bool,
    compatibility_provider_metadata: bool,
    compatibility_transport_metadata: bool,
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
    uri: &'a str,
    name: &'a str,
    server_override: Option<&'a str>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cases() -> Vec<Case> {
    let source = std::fs::read_to_string(root().join("tests/parity_cases/vless-canonical-v1.json"))
        .expect("credential-free canonical VLESS corpus");
    serde_json::from_str(&source).expect("bounded canonical VLESS corpus")
}

fn empty_outcome(classification: &str) -> Outcome {
    Outcome {
        accepted: false,
        classification: classification.to_owned(),
        host_kind: "none".to_owned(),
        port: 0,
        standard_https_port: false,
        transport: "none".to_owned(),
        security: "none".to_owned(),
        flow: "none".to_owned(),
        packet_encoding: "none".to_owned(),
        xhttp_mode: "none".to_owned(),
        allow_insecure: false,
        encryption_enabled: false,
        reality_pq: false,
        alpn_count: 0,
        advanced_xhttp: false,
        xhttp_field_count: 0,
        experimental_feature_count: 0,
        compatibility_note_present: false,
        compatibility_spider: false,
        compatibility_pq: false,
        compatibility_provider_metadata: false,
        compatibility_transport_metadata: false,
        identity_fingerprint: String::new(),
        preview_fingerprint: String::new(),
        mihomo_fingerprint: String::new(),
    }
}

fn accepted_outcome(
    profile: &VlessCanonicalProfile,
    facts: VlessCanonicalFacts,
    name: &str,
    server_override: Option<&str>,
) -> Outcome {
    Outcome {
        accepted: true,
        classification: "accepted".to_owned(),
        host_kind: facts.host_kind.as_str().to_owned(),
        port: facts.port,
        standard_https_port: facts.port == 443,
        transport: facts.transport.as_str().to_owned(),
        security: facts.security.as_str().to_owned(),
        flow: facts.flow.map_or("none", |value| value.as_str()).to_owned(),
        packet_encoding: facts
            .packet_encoding
            .map_or("none", |value| value.as_str())
            .to_owned(),
        xhttp_mode: facts
            .xhttp_mode
            .map_or("none", |value| value.as_str())
            .to_owned(),
        allow_insecure: facts.allow_insecure,
        encryption_enabled: facts.encryption_enabled,
        reality_pq: facts.reality_pq,
        alpn_count: facts.alpn_count,
        advanced_xhttp: facts.advanced_xhttp,
        xhttp_field_count: facts.xhttp_field_count,
        experimental_feature_count: facts.experimental_feature_count,
        compatibility_note_present: facts.compatibility_note_present,
        compatibility_spider: facts.compatibility_spider,
        compatibility_pq: facts.compatibility_pq,
        compatibility_provider_metadata: facts.compatibility_provider_metadata,
        compatibility_transport_metadata: facts.compatibility_transport_metadata,
        identity_fingerprint: profile.subscription_identity(),
        preview_fingerprint: profile.preview_fingerprint(),
        mihomo_fingerprint: profile.mihomo_render_fingerprint(name, server_override),
    }
}

fn rust_outcome(case: &Case) -> Outcome {
    match parse_vless_canonical(&case.uri) {
        Ok(profile) => accepted_outcome(
            &profile,
            profile.facts(),
            &case.name,
            case.server_override.as_deref(),
        ),
        Err(error) => empty_outcome(error.code()),
    }
}

fn python_outcomes(cases: &[Case]) -> (Vec<PythonCase>, Vec<u8>, Vec<u8>) {
    let request = PythonRequest {
        cases: cases
            .iter()
            .map(|case| PythonRequestCase {
                id: &case.id,
                uri: &case.uri,
                name: &case.name,
                server_override: case.server_override.as_deref(),
            })
            .collect(),
    };
    let request = serde_json::to_vec(&request).expect("adapter request");
    let mut child = Command::new("python3")
        .arg(root().join("tools/vless_canonical_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python canonical VLESS adapter");
    child
        .stdin
        .take()
        .expect("adapter stdin")
        .write_all(&request)
        .expect("write adapter input");
    let output = child.wait_with_output().expect("adapter output");
    assert!(
        output.status.success(),
        "Python canonical adapter failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: PythonEnvelope =
        serde_json::from_slice(&output.stdout).expect("sanitized adapter JSON");
    (envelope.cases, output.stdout, output.stderr)
}

#[test]
fn python_and_rust_canonical_vless_semantics_match() {
    let cases = cases();
    assert_eq!(cases.len(), 49, "accepted pairwise/boundary corpus");
    let (python, stdout, stderr) = python_outcomes(&cases);
    assert_eq!(python.len(), cases.len());
    assert!(stdout.len() <= 128 * 1024, "bounded adapter stdout");
    assert!(stderr.len() <= 1024, "bounded adapter stderr");
    for marker in [
        "vless://",
        "example.invalid",
        "11111111-1111-4111-8111-111111111111",
        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
        "synthetic",
    ] {
        assert!(!String::from_utf8_lossy(&stdout).contains(marker));
        assert!(!String::from_utf8_lossy(&stderr).contains(marker));
    }

    for (case, python_case) in cases.iter().zip(&python) {
        assert_eq!(case.id, python_case.id);
        let rust = rust_outcome(case);
        assert_eq!(
            rust.classification, case.classification,
            "Rust classification {}",
            case.id
        );
        assert_eq!(
            python_case.outcome.classification, case.classification,
            "Python classification {}",
            case.id
        );
        assert_eq!(rust, python_case.outcome, "differential {}", case.id);
    }

    let identity = |id: &str| {
        python
            .iter()
            .find(|value| value.id == id)
            .expect("identity case")
            .outcome
            .identity_fingerprint
            .as_str()
    };
    assert_eq!(identity("identity-base"), identity("identity-reordered"));
    assert_eq!(identity("identity-base"), identity("identity-provider"));
    assert_ne!(identity("identity-base"), identity("tls-full"));
}

#[test]
fn canonical_errors_and_private_debug_are_credential_safe() {
    let errors = [
        "ordinary text",
        "vless://not-a-uuid@example.invalid:443",
        "vless://11111111-1111-4111-8111-111111111111@example.invalid:443?type=kcp",
    ];
    for input in errors {
        let error = parse_vless_canonical(input).expect_err("invalid profile");
        assert!(error.code().len() <= 64);
        assert!(error.to_string().len() <= 160);
        for marker in ["not-a-uuid", "example.invalid", "kcp"] {
            assert!(!error.to_string().contains(marker));
        }
    }

    let profile = parse_vless_canonical(
        "vless://11111111-1111-4111-8111-111111111111@server.private.invalid:443?type=ws&security=tls&sni=sni.private.invalid&host=host.private.invalid&path=%2Fprivate&fp=chrome#Private",
    )
    .expect("private profile");
    let debug = format!("{profile:?}");
    for marker in [
        "11111111-1111-4111-8111-111111111111",
        "private.invalid",
        "/private",
        "Private",
    ] {
        assert!(!debug.contains(marker));
    }
}
