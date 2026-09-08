// SPDX-License-Identifier: MIT
//! Exact attribution of a held-open, fixed HTTPS CONNECT probe. Global rule
//! counters and destination-only matches are never evidence for this probe.
use crate::diagnostics::{bounded_controller_text, private_fragment_budget};
use crate::{ErrorKind, MihomoError, Result};
use serde_json::{Value, json};
use std::net::IpAddr;

pub const MAX_CONNECTIONS: usize = 2048;

fn invalid() -> MihomoError {
    MihomoError::new(ErrorKind::InvalidResponse)
}

fn port(value: &Value) -> Option<u16> {
    let text = value.as_str()?;
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse::<u16>()
        .ok()
        .filter(|port| *port != 0 && port.to_string() == text)
}

/// The query must already have passed the domain layer's canonical validator.
/// Only fixed policy categories survive; unknown chains are not assumed VPN.
pub fn exact_match(
    payload: &Value,
    query: &str,
    source_port: u16,
    mixed_port: u16,
    private: &[String],
) -> Result<Option<Value>> {
    if source_port == 0 || mixed_port == 0 || !private_fragment_budget(private) {
        return Err(invalid());
    }
    // Mihomo serializes an empty Go connection slice as explicit null. This
    // means no observation yet, never a policy result; callers keep the deadline.
    if payload.get("connections") == Some(&Value::Null) {
        return Ok(None);
    }
    let rows = payload
        .get("connections")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if rows.len() > MAX_CONNECTIONS {
        return Err(invalid());
    }
    let mut matched = None;
    for row in rows {
        let Some(metadata) = row.get("metadata").and_then(Value::as_object) else {
            continue;
        };
        if metadata.get("sourceIP").and_then(Value::as_str) != Some("127.0.0.1")
            || metadata.get("inboundIP").and_then(Value::as_str) != Some("127.0.0.1")
            || metadata.get("network").and_then(Value::as_str) != Some("tcp")
            || metadata.get("type").and_then(Value::as_str) != Some("HTTPS")
            || metadata.get("sourcePort").and_then(port) != Some(source_port)
            || metadata.get("inboundPort").and_then(port) != Some(mixed_port)
            || metadata.get("destinationPort").and_then(port) != Some(443)
        {
            continue;
        }
        let destination_matches = if let Ok(ip) = query.parse::<IpAddr>() {
            metadata
                .get("destinationIP")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<IpAddr>().ok())
                == Some(ip)
        } else {
            metadata
                .get("host")
                .and_then(Value::as_str)
                .is_some_and(|host| host.eq_ignore_ascii_case(query))
        };
        if !destination_matches {
            continue;
        }
        if matched.is_some() {
            return Err(invalid());
        }
        let chains = row
            .get("chains")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        if chains.is_empty()
            || chains.len() > 16
            || chains
                .iter()
                .any(|v| v.as_str().is_none_or(|s| s.is_empty() || s.len() > 256))
        {
            return Err(invalid());
        }
        // Only a singleton terminal DIRECT/REJECT, or the generated PROXY
        // selector with no conflicting terminal, is an identified policy.
        let (target, outcome) = if chains.len() == 1 && chains[0] == "DIRECT" {
            ("DIRECT", "direct")
        } else if chains.len() == 1 && matches!(chains[0].as_str(), Some("REJECT" | "REJECT-DROP"))
        {
            ("REJECT", "block")
        } else if chains.iter().any(|v| v == "PROXY")
            && !chains
                .iter()
                .any(|v| matches!(v.as_str(), Some("DIRECT" | "REJECT" | "REJECT-DROP")))
        {
            ("PROXY", "vpn")
        } else {
            return Err(invalid());
        };
        let kind = row
            .get("rule")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let rule_payload = row
            .get("rulePayload")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        if kind.is_empty() {
            return Err(invalid());
        }
        matched = Some(
            json!({"outcome":outcome,"ruleType":bounded_controller_text(kind,80,private),
            "rulePayload":bounded_controller_text(rule_payload,256,private),"target":target,"source":"live"}),
        );
    }
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> Value {
        json!({"metadata":{"sourceIP":"127.0.0.1","sourcePort":"40000","inboundIP":"127.0.0.1","inboundPort":"7890","destinationPort":"443","host":"example.com","network":"tcp","type":"HTTPS"},"chains":["private node","PROXY"],"rule":"RuleSet","rulePayload":"public-rules"})
    }
    fn check(rows: Vec<Value>) -> Result<Option<Value>> {
        exact_match(
            &json!({"connections":rows}),
            "example.com",
            40000,
            7890,
            &["private node".into()],
        )
    }
    #[test]
    fn matches_only_held_probe_tuple() {
        let expected = check(vec![row()]).unwrap().unwrap();
        assert_eq!(expected["target"], "PROXY");
        assert!(!expected.to_string().contains("private node"));
        for (field, value) in [
            ("sourcePort", "40001"),
            ("sourceIP", "127.0.0.2"),
            ("inboundIP", "0.0.0.0"),
            ("inboundPort", "7891"),
            ("destinationPort", "80"),
            ("network", "udp"),
            ("type", "HTTP"),
            ("host", "other.example"),
        ] {
            let mut unrelated = row();
            unrelated["metadata"][field] = json!(value);
            assert!(check(vec![unrelated.clone()]).unwrap().is_none(), "{field}");
            assert_eq!(check(vec![unrelated, row()]).unwrap().unwrap(), expected);
        }
    }
    #[test]
    fn ambiguous_or_unclassified_evidence_fails_closed() {
        assert!(check(vec![row(), row()]).is_err());
        for chain in [
            json!([]),
            json!(["unknown"]),
            json!(["DIRECT", "PROXY"]),
            json!([false]),
        ] {
            let mut value = row();
            value["chains"] = chain;
            assert!(check(vec![value]).is_err());
        }
        assert!(exact_match(&json!({}), "example.com", 40000, 7890, &[]).is_err());
        assert!(
            exact_match(
                &json!({"connections":false}),
                "example.com",
                40000,
                7890,
                &[]
            )
            .is_err()
        );
        assert!(check(vec![row(); MAX_CONNECTIONS + 1]).is_err());
    }
    #[test]
    fn explicit_nil_connection_list_is_only_no_observation() {
        assert!(
            exact_match(
                &json!({"connections":null}),
                "example.com",
                40000,
                7890,
                &[]
            )
            .unwrap()
            .is_none()
        );
        assert!(check(vec![]).unwrap().is_none());
    }
    #[test]
    fn categories_and_private_payload_redaction() {
        for (chain, target) in [
            ("DIRECT", "DIRECT"),
            ("REJECT", "REJECT"),
            ("REJECT-DROP", "REJECT"),
        ] {
            let mut value = row();
            value["chains"] = json!([chain]);
            value["rulePayload"] = json!("private node");
            let result = check(vec![value]).unwrap().unwrap();
            assert_eq!(result["target"], target);
            assert!(!result.to_string().contains("private node"));
        }
    }

    #[test]
    fn ip_family_and_output_bounds_are_explicit() {
        let mut value = row();
        value["metadata"]["host"] = json!("");
        value["metadata"]["destinationIP"] = json!("::ffff:192.0.2.1");
        value["rulePayload"] = json!("界".repeat(300));
        value["rule"] = json!("x".repeat(100));
        let payload = json!({"connections":[value.clone()]});
        let result = exact_match(&payload, "::ffff:c000:201", 40000, 7890, &[])
            .unwrap()
            .unwrap();
        assert!(result["rulePayload"].as_str().unwrap().len() <= 256);
        assert!(result["ruleType"].as_str().unwrap().len() <= 80);
        assert!(
            exact_match(&payload, "192.0.2.1", 40000, 7890, &[])
                .unwrap()
                .is_none()
        );
        assert!(
            exact_match(
                &payload,
                "::ffff:c000:201",
                40000,
                7890,
                &vec!["secret".into(); 513]
            )
            .is_err()
        );
        value["chains"] = json!(vec!["PROXY"; 17]);
        assert!(
            exact_match(
                &json!({"connections":[value]}),
                "::ffff:c000:201",
                40000,
                7890,
                &[]
            )
            .is_err()
        );
    }
}
