// SPDX-License-Identifier: MIT
use omavless_mihomo::route_observation::exact_match;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn corrected_python_reference_matches_exact_probe_attribution() {
    let row = json!({"metadata":{"sourceIP":"127.0.0.1","sourcePort":"40000","inboundIP":"127.0.0.1","inboundPort":"7890","destinationPort":"443","network":"tcp","type":"HTTPS","host":"example.com"},"rule":"RuleSet","rulePayload":"safe-public","chains":["synthetic-private","PROXY"]});
    let mut payloads = vec![
        json!({"connections":[]}),
        json!({"connections":[row.clone()]}),
        json!({"connections":[row.clone(),row.clone()]}),
        json!({"connections":null}),
    ];
    for (field, value) in [
        ("sourceIP", "127.0.0.2"),
        ("sourcePort", "40001"),
        ("sourcePort", "040000"),
        ("inboundIP", "0.0.0.0"),
        ("inboundPort", "8888"),
        ("destinationPort", "80"),
        ("type", "HTTP"),
        ("network", "udp"),
        ("host", "other.example"),
    ] {
        let mut unrelated = row.clone();
        unrelated["metadata"][field] = json!(value);
        payloads.push(json!({"connections":[unrelated.clone()]}));
        payloads.push(json!({"connections":[unrelated,row.clone()]}));
    }
    for chains in [
        json!(["DIRECT"]),
        json!(["REJECT"]),
        json!(["REJECT-DROP"]),
        json!(["unknown"]),
        json!(["PROXY", "DIRECT"]),
        json!([]),
        json!([true]),
    ] {
        let mut value = row.clone();
        value["chains"] = chains;
        payloads.push(json!({"connections":[value]}));
    }
    for text in [
        "https://synthetic.invalid/secret",
        "界",
        "one\n two",
        "11111111-1111-4111-8111-111111111111",
    ] {
        let mut value = row.clone();
        value["rulePayload"] = json!(text);
        payloads.push(json!({"connections":[value]}));
    }
    let cases: Vec<Value> = payloads
        .into_iter()
        .map(|payload| json!({"query":"example.com","payload":payload}))
        .collect();
    let mut child = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/route_observation_parity.py"
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "effect-isolated route oracle failed"
    );
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected.len(), cases.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let actual = match exact_match(
            &case["payload"],
            case["query"].as_str().unwrap(),
            40000,
            7890,
            &[],
        ) {
            Ok(value) => {
                json!({"digest":format!("{:x}",Sha256::digest(serde_json::to_vec(&value).unwrap()))})
            }
            Err(_) => json!({"error":true}),
        };
        assert!(actual == expected, "route oracle case {index} diverged");
    }
}
