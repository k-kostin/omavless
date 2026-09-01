// SPDX-License-Identifier: MIT

use crate::{ErrorKind, MihomoError, Result};
use serde_json::Value;

pub const MAX_RULES: usize = 2048;
pub const MAX_PROVIDERS: usize = 256;

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
    } else if upper.contains("REJECT") {
        "REJECT"
    } else {
        "VPN"
    }
}

pub fn loaded_rules(payload: &Value, private: &[String]) -> Result<Vec<RuleSummary>> {
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
            if !provider_name(name) {
                return Err(invalid());
            }
            let object = value.as_object().ok_or_else(invalid)?;
            let behavior = object.get("behavior").and_then(Value::as_str).unwrap_or("");
            let vehicle = object
                .get("vehicleType")
                .and_then(Value::as_str)
                .unwrap_or("");
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
}
