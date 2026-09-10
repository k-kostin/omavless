// SPDX-License-Identifier: MIT
use omavless_domain::private_store::parse_private_store;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
const ID: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn reusable_credential_is_absent_from_explicit_details() {
    let store = json!({"version":3,"profiles":[{"id":ID,"name":"Synthetic","protocol":"trojan","uri":"trojan://private-token@server.invalid:443?sni=tls.invalid"}],"subscriptions":[]});
    let parsed = parse_private_store(&store.to_string()).unwrap();
    let value = parsed
        .profile_details(ID)
        .ok()
        .unwrap()
        .into_private_ui_value();
    assert!(!value.to_string().contains("private-token"));
    assert!(!value.to_string().contains(ID));
    assert_eq!(value["server"], "server.invalid:443");
    assert_eq!(value["sni"], "tls.invalid");
}

#[test]
fn explicit_details_match_python_without_credentials_or_invented_tun_address() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for corpus in ["vless-canonical-v1.json", "non-vless-canonical-v1.json"] {
        let values: Vec<Value> = serde_json::from_slice(
            &std::fs::read(root.join("tests/parity_cases").join(corpus)).unwrap(),
        )
        .unwrap();
        for item in values {
            let protocol = item
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or("vless");
            cases.push(json!({"caseId":item["id"],"id":ID,"store":{"version":3,"profiles":[{"id":ID,"name":"Synthetic","protocol":protocol,"uri":item["uri"]}],"subscriptions":[]}}));
        }
    }
    let mut missing = cases[0].clone();
    missing["id"] = json!("00000000-0000-4000-8000-000000000099");
    missing["caseId"] = json!("missing-record");
    cases.push(missing);
    let mut managed = cases[0].clone();
    managed["caseId"] = json!("managed-record");
    managed["store"]["subscriptions"] = json!([{"id":"10000000-0000-4000-8000-000000000001","name":"Synthetic source","url":"https://example.invalid/synthetic-token","updatedAt":1}]);
    managed["store"]["profiles"][0]["subscriptionId"] =
        json!("10000000-0000-4000-8000-000000000001");
    managed["store"]["profiles"][0]["subscriptionKey"] = json!("a".repeat(64));
    cases.push(managed);
    let mut child = Command::new("python3")
        .arg(root.join("tools/profile_details_parity.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&cases).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "details oracle failed");
    let oracle: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(oracle["quicFallbacks"], 12);
    let expected: Vec<Option<String>> = serde_json::from_value(oracle["results"].clone()).unwrap();
    assert_eq!(expected.len(), cases.len());
    assert!(expected.iter().filter(|v| v.is_some()).count() > 30);
    assert!(expected.last().unwrap().is_some());
    for (case, expected) in cases.iter().zip(expected) {
        let actual=parse_private_store(&case["store"].to_string()).ok().and_then(|store|store.profile_details(case["id"].as_str().unwrap()).ok()).map(|details|{
            let value=details.into_private_ui_value();assert_eq!(value.as_object().unwrap().len(),7);
            for key in ["uri","uuid","id","password","key","credentialHint","address","subscriptionId"]{assert!(value.get(key).is_none())}
            assert_eq!(value["name"],"Synthetic");
            let sni=value["sni"].as_str().unwrap();
            let legacy=json!({"version":1,"server":value["server"],"transport":format!("{} / {}",value["transport"].as_str().unwrap(),value["security"].as_str().unwrap()),"sni":if sni.is_empty(){"--"}else{sni}});
            format!("{:x}",Sha256::digest(serde_json::to_vec(&legacy).unwrap()))
        });
        assert!(
            actual == expected,
            "details mismatch: {}",
            case["caseId"].as_str().unwrap()
        );
    }
}
