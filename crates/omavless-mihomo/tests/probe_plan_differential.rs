// SPDX-License-Identifier: MIT
use omavless_mihomo::probe_plan::{PROBE_URLS, PinnedProfile, ProbePlan};
use omavless_profile::canonical::{CanonicalProfile, parse_canonical};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    net::IpAddr,
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn canonical_config_and_median_match_actual_python() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for filename in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let corpus: Vec<Value> = serde_json::from_str(
            &std::fs::read_to_string(root.join("tests/parity_cases").join(filename)).unwrap(),
        )
        .unwrap();
        for case in corpus
            .iter()
            .filter(|case| case["classification"] == "accepted")
        {
            for addresses in [
                json!([]),
                json!(["192.0.2.5"]),
                json!(["192.0.2.5", "2001:db8::5"]),
            ] {
                cases.push(json!({"uri":case["uri"], "addresses": addresses,
                    "responses":[[200,{"p0000a0":10,"p0000a1":11}], [504,{"message":"get delay: all proxies timeout"}], [200,{}]]}));
            }
        }
    }
    let mut child = Command::new("python3")
        .arg(root.join("tools/probe_plan_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"cases": cases})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "safe oracle failed");
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected["urls"], json!(PROBE_URLS));
    assert_eq!(expected["cases"].as_array().unwrap().len(), cases.len());
    for (index, case) in cases.iter().enumerate() {
        let profile = parse_canonical(case["uri"].as_str().unwrap()).unwrap();
        let addresses: Vec<IpAddr> = case["addresses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect();
        let plan = ProbePlan::new(&[PinnedProfile {
            profile: &profile,
            addresses: &addresses,
        }])
        .unwrap();
        let config = plan
            .chunks()
            .first()
            .map_or("", |chunk| chunk.private_config());
        let proxies: Vec<_> = addresses
            .iter()
            .enumerate()
            .map(|(i, address)| {
                profile.render_mihomo_proxy(&format!("p0000a{i}"), Some(&address.to_string()))
            })
            .collect();
        let skeleton = if proxies.is_empty() {
            config.to_owned()
        } else {
            let rendered = proxies.join("\n");
            assert!(
                config.contains(&rendered),
                "canonical renderer reused case {index}"
            );
            config.replacen(&rendered, "<canonical-proxies>", 1)
        };
        let fingerprints: Vec<_> = addresses
            .iter()
            .enumerate()
            .map(|(i, address)| {
                let alias = format!("p0000a{i}");
                let address = address.to_string();
                match &profile {
                    CanonicalProfile::Vless(p) => {
                        p.mihomo_render_fingerprint(&alias, Some(&address))
                    }
                    CanonicalProfile::Trojan(p) => {
                        p.mihomo_render_fingerprint(&alias, Some(&address))
                    }
                    CanonicalProfile::Hysteria2(p) => {
                        p.mihomo_render_fingerprint(&alias, Some(&address))
                    }
                    CanonicalProfile::Tuic(p) => {
                        p.mihomo_render_fingerprint(&alias, Some(&address))
                    }
                }
            })
            .collect();
        assert_eq!(
            json!(fingerprints),
            expected["cases"][index]["proxyHashes"],
            "canonical proxy case {index}"
        );
        let fingerprint = format!("{:x}", Sha256::digest(skeleton.as_bytes()));
        assert_eq!(
            json!(fingerprint),
            expected["cases"][index]["configHash"],
            "config case {index}"
        );
        let mut collector = plan.collector();
        if !addresses.is_empty() {
            for (round, response) in case["responses"].as_array().unwrap().iter().enumerate() {
                collector
                    .record(0, round, response[0].as_u64().unwrap() as u16, &response[1])
                    .unwrap();
            }
        }
        let result = collector.finish().unwrap()[0];
        assert_eq!(
            json!({"resolved":result.resolved,"reachable":result.reachable,"latencyMs":result.latency_ms}),
            expected["cases"][index]["result"],
            "result case {index}"
        );
    }
}
