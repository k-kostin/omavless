// SPDX-License-Identifier: MIT

use crate::{ErrorKind, MihomoError, Result};
use serde_json::Value;
use std::time::Instant;

pub const MAX_RULES: usize = 2048;
pub const MAX_PROVIDERS: usize = 256;

pub fn private_fragment_budget(private: &[String]) -> bool {
    private.len() <= 512 && private.iter().map(String::len).sum::<usize>() <= 64 * 1024
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSummary {
    pub kind: String,
    pub payload: String,
    pub target: &'static str,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSummary {
    pub name: String,
    pub behavior: String,
    pub updated_at: String,
    pub rule_count: i64,
    pub status: &'static str,
    pub refreshable: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficSample {
    pub upload_bytes_per_second: u64,
    pub download_bytes_per_second: u64,
}

fn invalid() -> MihomoError {
    MihomoError::new(ErrorKind::InvalidResponse)
}

pub fn bounded_controller_text(value: &str, maximum: usize, private: &[String]) -> String {
    // Never omit secrets to save work: conservatively redact the entire field.
    if value.len() > 8192 || !private_fragment_budget(private) {
        return "[redacted]".into();
    }
    let mut text = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.contains("://") {
        return "[redacted]".into();
    }
    for fragment in private {
        if !fragment.is_empty() && text.contains(fragment) {
            text = text.replace(fragment, "[private]");
        }
    }
    text = redact_uuids(&text);
    if text.len() <= maximum {
        return text;
    }
    let mut end = maximum.saturating_sub(3).min(text.len());
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}…", text[..end].trim_end())
}

fn route_target(value: &str) -> &'static str {
    let upper = value.to_ascii_uppercase();
    if upper == "DIRECT" {
        "DIRECT"
    } else if upper.starts_with("REJECT") {
        "REJECT"
    } else {
        "VPN"
    }
}

pub fn loaded_rules(payload: &Value, private: &[String]) -> Result<Vec<RuleSummary>> {
    loaded_rules_before(payload, private, None)
}

fn loaded_rules_before(
    payload: &Value,
    private: &[String],
    deadline: Option<Instant>,
) -> Result<Vec<RuleSummary>> {
    let rules = payload
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if rules.len() > 65_536 {
        return Err(invalid());
    }
    rules
        .iter()
        .take(MAX_RULES)
        .map(|item| {
            if deadline.is_some_and(|end| Instant::now() >= end) {
                return Err(MihomoError::new(ErrorKind::TimedOut));
            }
            let object = item.as_object().ok_or_else(invalid)?;
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            let payload = object.get("payload").and_then(Value::as_str).unwrap_or("");
            let proxy = object.get("proxy").and_then(Value::as_str).unwrap_or("");
            if object.get("type").is_some_and(|v| !v.is_string())
                || object.get("payload").is_some_and(|v| !v.is_string())
                || object.get("proxy").is_some_and(|v| !v.is_string())
            {
                return Err(invalid());
            }
            Ok(RuleSummary {
                kind: bounded_controller_text(kind, 80, private),
                payload: bounded_controller_text(payload, 512, private),
                target: route_target(proxy),
            })
        })
        .collect()
}

fn provider_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value != "."
        && value != ".."
        && !value
            .chars()
            .any(|c| c.is_control() || "/\\?#%".contains(c))
}

pub fn loaded_providers(payload: &Value, private: &[String]) -> Result<Vec<ProviderSummary>> {
    loaded_providers_before(payload, private, None)
}

fn loaded_providers_before(
    payload: &Value,
    private: &[String],
    deadline: Option<Instant>,
) -> Result<Vec<ProviderSummary>> {
    let providers = payload
        .get("providers")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    if providers.len() > MAX_PROVIDERS {
        return Err(invalid());
    }
    providers
        .iter()
        .map(|(name, value)| {
            if deadline.is_some_and(|end| Instant::now() >= end) {
                return Err(MihomoError::new(ErrorKind::TimedOut));
            }
            if !provider_name(name) {
                return Err(invalid());
            }
            let object = value.as_object().ok_or_else(invalid)?;
            let behavior = object.get("behavior").and_then(Value::as_str).unwrap_or("");
            let vehicle = object
                .get("vehicleType")
                .and_then(Value::as_str)
                .unwrap_or("");
            let updated_at = match object.get("updatedAt") {
                None => String::new(),
                Some(Value::String(text)) => text.clone(),
                Some(Value::Number(number)) => number.to_string(),
                _ => return Err(invalid()),
            };
            if object.get("behavior").is_some_and(|v| !v.is_string())
                || object.get("vehicleType").is_some_and(|v| !v.is_string())
            {
                return Err(invalid());
            }
            let count = match object.get("ruleCount") {
                None | Some(Value::Null) => -1,
                Some(v) => v
                    .as_i64()
                    .filter(|n| (0..=1_000_000_000).contains(n))
                    .ok_or_else(invalid)?,
            };
            Ok(ProviderSummary {
                name: bounded_controller_text(name, 160, private),
                behavior: bounded_controller_text(behavior, 80, private),
                updated_at: bounded_controller_text(&updated_at, 80, private),
                rule_count: count,
                status: if count < 0 {
                    "unknown"
                } else if count == 0 {
                    "empty"
                } else {
                    "loaded"
                },
                refreshable: vehicle.eq_ignore_ascii_case("http"),
            })
        })
        .collect()
}

fn redact_uuids(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut output = String::new();
    let mut copied = 0;
    let mut start = 0;
    while start + 36 <= bytes.len() {
        let candidate = &bytes[start..start + 36];
        let boundary = |byte: u8| !byte.is_ascii_alphanumeric() && byte != b'_';
        let valid = (start == 0 || boundary(bytes[start - 1]))
            && (start + 36 == bytes.len() || boundary(bytes[start + 36]))
            && candidate.iter().enumerate().all(|(i, byte)| {
                if [8, 13, 18, 23].contains(&i) {
                    *byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            })
            && (b'1'..=b'5').contains(&candidate[14])
            && b"89abAB".contains(&candidate[19]);
        if valid {
            output.push_str(&text[copied..start]);
            output.push_str("[private]");
            start += 36;
            copied = start;
        } else {
            start += 1;
        }
    }
    output.push_str(&text[copied..]);
    output
}

/// Native v1 response budget is smaller than the legacy 384-KiB UI envelope.
/// Truncate rows, never individual JSON or metadata, with an explicit marker.
pub fn rules_projection(payload: &Value, private: &[String]) -> Result<Value> {
    rules_projection_before(payload, private, None)
}

pub fn rules_projection_before(
    payload: &Value,
    private: &[String],
    deadline: Option<Instant>,
) -> Result<Value> {
    let total = payload
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?
        .len();
    let rows = loaded_rules_before(payload, private, deadline)?
        .into_iter()
        .map(|row| serde_json::json!({"type":row.kind,"payload":row.payload,"target":row.target}));
    Ok(budgeted_rows(rows, total, 160 * 1024))
}

pub fn providers_projection(payload: &Value, private: &[String]) -> Result<Value> {
    providers_projection_before(payload, private, None)
}

pub fn providers_projection_before(
    payload: &Value,
    private: &[String],
    deadline: Option<Instant>,
) -> Result<Value> {
    let total = payload
        .get("providers")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?
        .len();
    let rows = loaded_providers_before(payload, private, deadline)?
        .into_iter()
        .map(|row| {
            serde_json::json!({"name":row.name,"behavior":row.behavior,"updatedAt":row.updated_at,
            "ruleCount":row.rule_count,"status":row.status,"refreshable":row.refreshable})
        });
    Ok(budgeted_rows(rows, total, 64 * 1024))
}

fn budgeted_rows(rows: impl Iterator<Item = Value>, total: usize, limit: usize) -> Value {
    let mut items = Vec::new();
    let mut bytes = 64;
    for row in rows {
        let size = row.to_string().len() + 1;
        if bytes + size > limit {
            break;
        }
        bytes += size;
        items.push(row);
    }
    serde_json::json!({"total":total,"shown":items.len(),"truncated":items.len()<total,"items":items})
}

pub fn traffic_sample(payload: &Value) -> Result<TrafficSample> {
    let object = payload.as_object().ok_or_else(invalid)?;
    let up = object
        .get("up")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    let down = object
        .get("down")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    Ok(TrafficSample {
        upload_bytes_per_second: up.min(1_000_000_000_000),
        download_bytes_per_second: down.min(1_000_000_000_000),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn diagnostics_are_bounded_redacted_and_categorical() {
        let private = vec!["private-name".into()];
        let rows=loaded_rules(&json!({"rules":[{"type":"DOMAIN","payload":"private-name.example","proxy":"secret-group"},{"type":"MATCH","payload":"vless://secret","proxy":"REJECT-DROP"}]}),&private).unwrap();
        assert!(rows[0].payload.contains("[private]"));
        assert_eq!(rows[0].target, "VPN");
        assert_eq!(rows[1].payload, "[redacted]");
        assert_eq!(rows[1].target, "REJECT");
    }
    #[test]
    fn providers_and_traffic_fail_closed() {
        let providers=loaded_providers(&json!({"providers":{"safe":{"behavior":"domain","vehicleType":"HTTP","ruleCount":12}}}),&[]).unwrap();
        assert!(providers[0].refreshable);
        assert_eq!(providers[0].status, "loaded");
        assert_eq!(
            traffic_sample(&json!({"up":12,"down":34}))
                .unwrap()
                .download_bytes_per_second,
            34
        );
        assert!(traffic_sample(&json!({"up":-1,"down":0})).is_err());
    }

    #[test]
    fn redaction_work_is_bounded_without_dropping_private_fragments() {
        assert!(private_fragment_budget(&vec!["x".repeat(128); 512]));
        for private in [vec!["private".into(); 513], vec!["x".repeat(65537)]] {
            assert!(!private_fragment_budget(&private));
            assert_eq!(
                bounded_controller_text("private data", 512, &private),
                "[redacted]"
            );
        }
        assert_eq!(
            bounded_controller_text(&"x".repeat(8193), 512, &[]),
            "[redacted]"
        );
        let expired = Some(Instant::now());
        assert!(rules_projection_before(&json!({"rules":[{}]}), &[], expired).is_err());
        assert!(
            providers_projection_before(&json!({"providers":{"safe":{}}}), &[], expired).is_err()
        );
    }
}
