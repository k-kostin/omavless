// SPDX-License-Identifier: MIT

use omavless_domain::routing::{CustomRule, RoutingError, inject_custom_rules, template_with_mode};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    kind: String,
    action: String,
    value: String,
    template: String,
    mode: String,
    classification: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Outcome {
    accepted: bool,
    classification: String,
    canonical_fingerprint: String,
    rule_fingerprint: String,
    template_fingerprint: String,
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
struct PythonEnvelope {
    cases: Vec<PythonCase>,
}

#[derive(Serialize)]
struct Request<'a> {
    cases: Vec<RequestCase<'a>>,
}

#[derive(Serialize)]
struct RequestCase<'a> {
    id: &'a str,
    kind: &'a str,
    action: &'a str,
    value: &'a str,
    template: &'a str,
    mode: &'a str,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha256(input: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = (input.len() as u64) * 8;
    let mut data = input.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = INITIAL;
    for chunk in data.as_chunks::<64>().0 {
        let mut words = [0u32; 64];
        for (i, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = words[i - 15].rotate_right(7)
                ^ words[i - 15].rotate_right(18)
                ^ (words[i - 15] >> 3);
            let s1 = words[i - 2].rotate_right(17)
                ^ words[i - 2].rotate_right(19)
                ^ (words[i - 2] >> 10);
            words[i] = words[i - 16]
                .wrapping_add(s0)
                .wrapping_add(words[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (
            state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
        );
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(words[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (i, v) in [a, b, c, d, e, f, g, h].into_iter().enumerate() {
            state[i] = state[i].wrapping_add(v);
        }
    }
    state.iter().map(|v| format!("{v:08x}")).collect()
}

fn rejected(error: RoutingError) -> Outcome {
    Outcome {
        accepted: false,
        classification: error.code().to_owned(),
        canonical_fingerprint: String::new(),
        rule_fingerprint: String::new(),
        template_fingerprint: String::new(),
    }
}

fn rust_outcome(case: &Case) -> Outcome {
    let rule = match CustomRule::parse(&case.kind, &case.action, &case.value) {
        Ok(value) => value,
        Err(error) => return rejected(error),
    };
    let injected = match inject_custom_rules(&case.template, std::slice::from_ref(&rule)) {
        Ok(value) => value,
        Err(error) => return rejected(error),
    };
    let rewritten = match template_with_mode(&injected, &case.mode) {
        Ok(value) => value,
        Err(error) => return rejected(error),
    };
    Outcome {
        accepted: true,
        classification: "accepted".to_owned(),
        canonical_fingerprint: sha256(rule.value.as_bytes()),
        rule_fingerprint: sha256(rule.mihomo_line().as_bytes()),
        template_fingerprint: sha256(rewritten.as_bytes()),
    }
}

#[test]
fn python_and_rust_routing_semantics_match() {
    let raw =
        std::fs::read_to_string(root().join("tests/parity_cases/routing-domain-v1.json")).unwrap();
    let cases: Vec<Case> = serde_json::from_str(&raw).unwrap();
    assert_eq!(cases.len(), 19);
    let request = Request {
        cases: cases
            .iter()
            .map(|c| RequestCase {
                id: &c.id,
                kind: &c.kind,
                action: &c.action,
                value: &c.value,
                template: &c.template,
                mode: &c.mode,
            })
            .collect(),
    };
    let mut child = Command::new("python3")
        .arg(root().join("tools/routing_domain_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&request).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let python: PythonEnvelope = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(python.cases.len(), cases.len());
    for marker in ["private.example", "täst.invalid", "192.0.2.42"] {
        assert!(!String::from_utf8_lossy(&output.stdout).contains(marker));
    }
    for (case, reference) in cases.iter().zip(&python.cases) {
        assert_eq!(case.id, reference.id);
        assert_eq!(
            reference.outcome.classification, case.classification,
            "Python {}",
            case.id
        );
        let rust = rust_outcome(case);
        assert_eq!(rust.classification, case.classification, "Rust {}", case.id);
        assert_eq!(rust, reference.outcome, "differential {}", case.id);
    }
}
