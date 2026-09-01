// SPDX-License-Identifier: MIT

use omavless_domain::private_store::{StoreProjection, parse_private_store};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    store: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u64,
    cases: Vec<Case>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Outcome {
    accepted: bool,
    #[serde(default)]
    version: u8,
    #[serde(default)]
    profile_count: usize,
    #[serde(default)]
    subscription_count: usize,
    #[serde(default)]
    protocol_counts: ProtocolCounts,
    #[serde(default)]
    active_present: bool,
    #[serde(default)]
    last_present: bool,
    #[serde(default)]
    routing_preset: String,
    #[serde(default)]
    custom_rule_count: usize,
    #[serde(default)]
    startup_configured: bool,
    #[serde(default)]
    onboarding_complete: bool,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ProtocolCounts {
    vless: usize,
    trojan: usize,
    hysteria2: usize,
    tuic: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonCase {
    id: String,
    #[serde(flatten)]
    outcome: Outcome,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    cases: Vec<PythonCase>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn accepted(projection: StoreProjection) -> Outcome {
    Outcome {
        accepted: true,
        version: projection.version,
        profile_count: projection.profile_count,
        subscription_count: projection.subscription_count,
        protocol_counts: ProtocolCounts {
            vless: projection.vless_count,
            trojan: projection.trojan_count,
            hysteria2: projection.hysteria2_count,
            tuic: projection.tuic_count,
        },
        active_present: projection.active_present,
        last_present: projection.last_present,
        routing_preset: projection.routing_preset,
        custom_rule_count: projection.custom_rule_count,
        startup_configured: projection.startup_configured,
        onboarding_complete: projection.onboarding_complete,
    }
}

fn rejected() -> Outcome {
    Outcome {
        accepted: false,
        version: 0,
        profile_count: 0,
        subscription_count: 0,
        protocol_counts: ProtocolCounts::default(),
        active_present: false,
        last_present: false,
        routing_preset: String::new(),
        custom_rule_count: 0,
        startup_configured: false,
        onboarding_complete: false,
    }
}

#[test]
fn python_and_rust_private_store_projection_match() {
    let raw = std::fs::read_to_string(root().join("tests/parity_cases/private-store-v1.json"))
        .expect("read private store corpus");
    let corpus: Corpus = serde_json::from_str(&raw).expect("valid private store corpus");
    assert_eq!(corpus.version, 1);
    let mut child = Command::new("python3")
        .arg(root().join("tools/private_store_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python private-store oracle");
    child
        .stdin
        .take()
        .expect("oracle stdin")
        .write_all(&serde_json::to_vec(&serde_json::json!({"cases": &corpus.cases})).unwrap())
        .expect("write oracle input");
    let output = child.wait_with_output().expect("oracle output");
    assert!(
        output.status.success(),
        "Python private-store oracle failed"
    );
    let reference: Envelope = serde_json::from_slice(&output.stdout).expect("oracle response");
    assert_eq!(reference.cases.len(), corpus.cases.len());
    for (case, expected) in corpus.cases.iter().zip(reference.cases) {
        assert_eq!(case.id, expected.id);
        let encoded = serde_json::to_string(&case.store).unwrap();
        let rust = parse_private_store(&encoded)
            .map(|store| accepted(store.projection()))
            .unwrap_or_else(|_| rejected());
        assert_eq!(rust, expected.outcome, "differential {}", case.id);
    }
    let public = String::from_utf8_lossy(&output.stdout);
    for private in ["password", "203.0.113", "aaaaaaaaaaaaaaaa"] {
        assert!(!public.contains(private));
    }
}
