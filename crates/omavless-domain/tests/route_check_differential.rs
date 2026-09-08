// SPDX-License-Identifier: MIT
use omavless_domain::route_check::check_fast_paths;
use omavless_domain::routing::CustomRule;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn actual_python_route_fastpaths_match_without_live_observation() {
    let rules = json!([
        {"kind":"domain","value":"exact.example","action":"reject"},
        {"kind":"suffix","value":"suffix.example","action":"direct"},
        {"kind":"suffix","value":"deep.suffix.example","action":"proxy"},
        {"kind":"ipcidr","value":"192.0.2.0/24","action":"direct"},
        {"kind":"ipcidr","value":"2001:db8::/32","action":"proxy"}
    ]);
    let parsed: Vec<_> = rules
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| {
            CustomRule::parse(
                rule["kind"].as_str().unwrap(),
                rule["action"].as_str().unwrap(),
                rule["value"].as_str().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let mut cases = Vec::new();
    for mode in ["global", "direct", "rule"] {
        for connected in [false, true] {
            for query in [
                "EXACT.EXAMPLE.",
                "suffix.example",
                "child.suffix.example",
                "not-suffix.example",
                "192.0.2.42",
                "192.0.3.1",
                "2001:DB8::42",
                "::ffff:192.0.2.1",
                "täst.example",
                "ordinary.example",
                "https://private.invalid/key",
                "localhost",
                "",
            ] {
                cases.push(json!({"rules":rules,"mode":mode,"connected":connected,"query":query}));
            }
        }
    }
    let mut child = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/route_check_parity.py"
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
    assert!(output.status.success(), "route oracle failed");
    let reference: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reference.len(), cases.len());
    for (index, (case, reference)) in cases.iter().zip(reference).enumerate() {
        let actual = match check_fast_paths(
            case["mode"].as_str().unwrap(),
            case["connected"].as_bool().unwrap(),
            &parsed,
            case["query"].as_str().unwrap(),
        ) {
            Ok(Some(result)) => {
                let value = result.private_ui_value();
                let mapped = value["query"]
                    .as_str()
                    .unwrap()
                    .parse::<std::net::Ipv6Addr>()
                    .is_ok_and(|address| address.to_ipv4_mapped().is_some());
                json!({"kind":"result","digest":format!("{:x}",Sha256::digest(serde_json::to_vec(&value).unwrap())),
                    "normalization":if mapped {"mapped_ipv6_display"} else {"none"}})
            }
            Ok(None) => json!({"kind":"live_required"}),
            Err(_) => json!({"kind":"invalid"}),
        };
        assert!(
            actual == reference,
            "route parity mismatch at public case {index}"
        );
    }
}
