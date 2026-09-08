// SPDX-License-Identifier: MIT
//! Pure current-routing fast paths. No lookup, probe or guessed live outcome.
use crate::routing::{CustomRule, RoutingError, RuleAction, RuleKind, canonical_rule_value};
use serde_json::{Value, json};
use std::net::IpAddr;

/// Explicit private UI response: query/rule destination must never be logged.
pub struct PrivateRouteCheck(Value);
impl PrivateRouteCheck {
    pub fn private_ui_value(self) -> Value {
        self.0
    }
}

fn ip_text(address: IpAddr) -> String {
    match address {
        IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some() => {
            let segments = ip.segments();
            format!("::ffff:{:x}:{:x}", segments[6], segments[7])
        }
        _ => address.to_string(),
    }
}

pub fn canonical_query(input: &str) -> Result<String, RoutingError> {
    if input.len() > crate::routing::MAX_CUSTOM_RULE_VALUE_BYTES {
        return Err(RoutingError::InvalidDomain);
    }
    match input.trim().parse::<IpAddr>() {
        Ok(address) => Ok(ip_text(address)),
        Err(_) => canonical_rule_value(RuleKind::Domain, input),
    }
}

fn contains(network: &str, query: IpAddr) -> bool {
    let Some((address, prefix)) = network.split_once('/') else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u32>() else {
        return false;
    };
    match (address.parse::<IpAddr>(), query) {
        (Ok(IpAddr::V4(network)), IpAddr::V4(query)) if prefix <= 32 => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            u32::from(network) & mask == u32::from(query) & mask
        }
        (Ok(IpAddr::V6(network)), IpAddr::V6(query)) if prefix <= 128 => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            u128::from(network) & mask == u128::from(query) & mask
        }
        _ => false,
    }
}

/// `None` means actual live rule observation is required, not "unknown".
pub fn check_fast_paths(
    mode: &str,
    connected: bool,
    rules: &[CustomRule],
    input: &str,
) -> Result<Option<PrivateRouteCheck>, RoutingError> {
    let query = canonical_query(input)?;
    let ip = query.parse::<IpAddr>().ok();
    let mut result = json!({"version":1,"query":query});
    let outcome = match mode {
        "global" => {
            json!({"outcome":"vpn","ruleType":"MODE","rulePayload":"global","target":"PROXY","source":"mode"})
        }
        "direct" => {
            json!({"outcome":"direct","ruleType":"MODE","rulePayload":"direct","target":"DIRECT","source":"mode"})
        }
        "rule" => {
            let matched = rules.iter().find(|rule| match rule.kind {
                RuleKind::Domain => ip.is_none() && query == rule.value,
                RuleKind::Suffix => {
                    ip.is_none()
                        && (query == rule.value || query.ends_with(&format!(".{}", rule.value)))
                }
                RuleKind::IpCidr => ip.is_some_and(|address| contains(&rule.value, address)),
            });
            if let Some(rule) = matched {
                let outcome = match rule.action {
                    RuleAction::Proxy => "vpn",
                    RuleAction::Direct => "direct",
                    RuleAction::Reject => "block",
                };
                let target = match rule.action {
                    RuleAction::Proxy => "PROXY",
                    RuleAction::Direct => "DIRECT",
                    RuleAction::Reject => "REJECT-DROP",
                };
                json!({"outcome":outcome,"ruleType":rule.kind.as_str().to_uppercase(),"rulePayload":rule.value,"target":target,"source":"custom"})
            } else if connected {
                return Ok(None);
            } else {
                json!({"outcome":"unknown","ruleType":"RULE-SET","rulePayload":"","target":"","source":"disconnected"})
            }
        }
        _ => return Err(RoutingError::UnsupportedMode),
    };
    result
        .as_object_mut()
        .unwrap()
        .extend(outcome.as_object().unwrap().clone());
    Ok(Some(PrivateRouteCheck(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_custom_matches_and_ip_family_boundaries_do_not_probe() {
        let rules = [
            CustomRule::parse("suffix", "direct", "example.invalid").unwrap(),
            CustomRule::parse("domain", "reject", "deep.example.invalid").unwrap(),
            CustomRule::parse("ipcidr", "reject", "0.0.0.0/0").unwrap(),
            CustomRule::parse("ipcidr", "proxy", "::/0").unwrap(),
        ];
        let result = |input| {
            check_fast_paths("rule", true, &rules, input)
                .unwrap()
                .unwrap()
                .private_ui_value()
        };
        assert_eq!(result("DEEP.EXAMPLE.INVALID.")["outcome"], "direct");
        assert_eq!(result("192.0.2.255")["outcome"], "block");
        assert_eq!(result("::ffff:192.0.2.1")["outcome"], "vpn");
        assert!(
            check_fast_paths("rule", true, &rules, "other.invalid")
                .unwrap()
                .is_none()
        );
        assert!(check_fast_paths("other", true, &rules, "example.invalid").is_err());
    }

    #[test]
    fn ipv6_mapped_and_network_boundaries_match_python_spelling() {
        assert_eq!(
            canonical_query("::ffff:192.0.2.1").unwrap(),
            "::ffff:c000:201"
        );
        assert!(contains(
            "2001:db8::/32",
            IpAddr::V6("2001:db8::1".parse::<std::net::Ipv6Addr>().unwrap())
        ));
        assert!(!contains("192.0.2.0/24", "192.0.3.1".parse().unwrap()));
        assert!(canonical_query("fe80::1%eth0").is_err());
    }
}
