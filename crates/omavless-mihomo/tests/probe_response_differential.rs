// SPDX-License-Identifier: MIT

use omavless_mihomo::merge_probe_response;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    status: u16,
    payload: Value,
    samples: BTreeMap<String, Vec<u32>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    version: u64,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    id: String,
    accepted: bool,
    samples: BTreeMap<String, Vec<u32>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    cases: Vec<Outcome>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn python_and_rust_probe_response_semantics_match() {
    let raw = fs_err_free_read(root().join("tests/parity_cases/mihomo-probe-response-v1.json"));
    let corpus: Corpus = serde_json::from_str(&raw).expect("valid probe corpus");
    assert_eq!(corpus.version, 1);
    assert_eq!(corpus.cases.len(), 14);
    let mut child = Command::new("python3")
        .arg(root().join("tools/mihomo_probe_response_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Python oracle");
    child
        .stdin
        .take()
        .expect("oracle stdin")
        .write_all(
            &serde_json::to_vec(&serde_json::json!({"cases": &corpus.cases})).expect("request"),
        )
        .expect("write oracle input");
    let output = child.wait_with_output().expect("oracle output");
    assert!(output.status.success(), "Python oracle failed");
    let reference: Envelope = serde_json::from_slice(&output.stdout).expect("oracle envelope");
    assert_eq!(reference.cases.len(), corpus.cases.len());
    for (case, expected) in corpus.cases.iter().zip(reference.cases) {
        assert_eq!(case.id, expected.id);
        let mut samples = case.samples.clone();
        let accepted = merge_probe_response(&mut samples, case.status, &case.payload);
        assert_eq!(accepted, expected.accepted, "{} acceptance", case.id);
        assert_eq!(samples, expected.samples, "{} samples", case.id);
    }
}

fn fs_err_free_read(path: PathBuf) -> String {
    std::fs::read_to_string(path).expect("read probe corpus")
}
