// SPDX-License-Identifier: MIT

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

pub const MAX_CUSTOM_RULES: usize = 128;
pub const MAX_CUSTOM_RULE_VALUE_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingError {
    UnsupportedKind,
    UnsupportedAction,
    EmptyValue,
    InvalidDomain,
    InvalidNetwork,
    TooManyRules,
    DuplicateRule,
    MissingRulesSection,
    MultipleRulesSections,
    UnsupportedMode,
    MultipleModes,
}

impl RoutingError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedKind => "unsupported_kind",
            Self::UnsupportedAction => "unsupported_action",
            Self::EmptyValue => "empty_value",
            Self::InvalidDomain => "invalid_domain",
            Self::InvalidNetwork => "invalid_network",
            Self::TooManyRules => "too_many_rules",
            Self::DuplicateRule => "duplicate_rule",
            Self::MissingRulesSection => "missing_rules_section",
            Self::MultipleRulesSections => "multiple_rules_sections",
            Self::UnsupportedMode => "unsupported_mode",
            Self::MultipleModes => "multiple_modes",
        }
    }
}

impl fmt::Display for RoutingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedKind => "Unsupported custom routing match type",
            Self::UnsupportedAction => "Unsupported custom routing action",
            Self::EmptyValue => "Routing value must not be empty",
            Self::InvalidDomain => "Routing domain is invalid",
            Self::InvalidNetwork => "Routing network is invalid",
            Self::TooManyRules => "Too many custom routing rules",
            Self::DuplicateRule => "Custom routing rule is duplicated",
            Self::MissingRulesSection => "Route template has no rules section",
            Self::MultipleRulesSections => "Route template has multiple rules sections",
            Self::UnsupportedMode => "Routing mode is unsupported",
            Self::MultipleModes => "Route template has multiple modes",
        })
    }
}

impl std::error::Error for RoutingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    Domain,
    Suffix,
    IpCidr,
}

impl RuleKind {
    pub fn parse(value: &str) -> Result<Self, RoutingError> {
        match value {
            "domain" => Ok(Self::Domain),
            "suffix" => Ok(Self::Suffix),
            "ipcidr" => Ok(Self::IpCidr),
            _ => Err(RoutingError::UnsupportedKind),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Suffix => "suffix",
            Self::IpCidr => "ipcidr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    Proxy,
    Direct,
    Reject,
}

impl RuleAction {
    pub fn parse(value: &str) -> Result<Self, RoutingError> {
        match value {
            "proxy" => Ok(Self::Proxy),
            "direct" => Ok(Self::Direct),
            "reject" => Ok(Self::Reject),
            _ => Err(RoutingError::UnsupportedAction),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proxy => "proxy",
            Self::Direct => "direct",
            Self::Reject => "reject",
        }
    }

    const fn target(self) -> &'static str {
        match self {
            Self::Proxy => "PROXY",
            Self::Direct => "DIRECT",
            Self::Reject => "REJECT-DROP",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomRule {
    pub kind: RuleKind,
    pub value: String,
    pub action: RuleAction,
}

impl CustomRule {
    pub fn parse(kind: &str, action: &str, value: &str) -> Result<Self, RoutingError> {
        let kind = RuleKind::parse(kind)?;
        let action = RuleAction::parse(action)?;
        let value = canonical_rule_value(kind, value)?;
        Ok(Self {
            kind,
            value,
            action,
        })
    }

    #[must_use]
    pub fn mihomo_line(&self) -> String {
        match self.kind {
            RuleKind::Domain => format!("  - DOMAIN,{},{}\n", self.value, self.action.target()),
            RuleKind::Suffix => format!(
                "  - DOMAIN-SUFFIX,{},{}\n",
                self.value,
                self.action.target()
            ),
            RuleKind::IpCidr => {
                let kind = if self.value.contains(':') {
                    "IP-CIDR6"
                } else {
                    "IP-CIDR"
                };
                format!(
                    "  - {kind},{},{},no-resolve\n",
                    self.value,
                    self.action.target()
                )
            }
        }
    }
}

fn canonical_domain(value: &str) -> Result<String, RoutingError> {
    let value = value.trim().trim_end_matches('.');
    if value.is_empty() || value.len() > MAX_CUSTOM_RULE_VALUE_BYTES {
        return Err(RoutingError::EmptyValue);
    }
    if value
        .chars()
        .any(|character| character <= ' ' || character == '\u{7f}' || "/:?#@".contains(character))
    {
        return Err(RoutingError::InvalidDomain);
    }
    let domain = idna::domain_to_ascii(value)
        .map_err(|_| RoutingError::InvalidDomain)?
        .to_lowercase();
    if domain.len() > 253
        || domain.split('.').count() < 2
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label.as_bytes()[0].is_ascii_alphanumeric()
                || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(RoutingError::InvalidDomain);
    }
    Ok(domain)
}

fn canonical_network(value: &str) -> Result<String, RoutingError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_CUSTOM_RULE_VALUE_BYTES {
        return Err(RoutingError::EmptyValue);
    }
    let (address, prefix) = value.split_once('/').unwrap_or((value, ""));
    if let Ok(address) = Ipv4Addr::from_str(address) {
        let prefix = if prefix.is_empty() {
            32
        } else {
            prefix
                .parse::<u8>()
                .map_err(|_| RoutingError::InvalidNetwork)?
        };
        if prefix > 32 {
            return Err(RoutingError::InvalidNetwork);
        }
        let raw = u32::from(address);
        let mask = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix)
        };
        return Ok(format!("{}/{}", Ipv4Addr::from(raw & mask), prefix));
    }
    if let Ok(address) = Ipv6Addr::from_str(address) {
        let prefix = if prefix.is_empty() {
            128
        } else {
            prefix
                .parse::<u8>()
                .map_err(|_| RoutingError::InvalidNetwork)?
        };
        if prefix > 128 {
            return Err(RoutingError::InvalidNetwork);
        }
        let raw = u128::from(address);
        let mask = if prefix == 0 {
            0
        } else {
            u128::MAX << (128 - prefix)
        };
        return Ok(format!("{}/{}", Ipv6Addr::from(raw & mask), prefix));
    }
    Err(RoutingError::InvalidNetwork)
}

pub fn canonical_rule_value(kind: RuleKind, value: &str) -> Result<String, RoutingError> {
    match kind {
        RuleKind::Domain => canonical_domain(value),
        RuleKind::Suffix => {
            let value = value.trim();
            let value = value.strip_prefix("*.").unwrap_or(value);
            canonical_domain(value.strip_prefix('.').unwrap_or(value))
        }
        RuleKind::IpCidr => canonical_network(value),
    }
}

pub fn validate_rules(rules: &[CustomRule]) -> Result<(), RoutingError> {
    if rules.len() > MAX_CUSTOM_RULES {
        return Err(RoutingError::TooManyRules);
    }
    for (index, rule) in rules.iter().enumerate() {
        if rules[..index]
            .iter()
            .any(|other| other.kind == rule.kind && other.value == rule.value)
        {
            return Err(RoutingError::DuplicateRule);
        }
    }
    Ok(())
}

fn is_top_level_key(line: &str, key: &str) -> bool {
    if line.starts_with([' ', '\t']) {
        return false;
    }
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let Some(after) = trimmed.strip_prefix(key) else {
        return false;
    };
    let Some(after) = after.strip_prefix(':') else {
        return false;
    };
    after.trim().is_empty() || after.trim_start().starts_with('#')
}

pub fn inject_custom_rules(template: &str, rules: &[CustomRule]) -> Result<String, RoutingError> {
    validate_rules(rules)?;
    if rules.is_empty() {
        return Ok(template.to_owned());
    }
    let lines = template.split_inclusive('\n').collect::<Vec<_>>();
    let matches = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is_top_level_key(line, "rules"))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let index = match matches.as_slice() {
        [] => return Err(RoutingError::MissingRulesSection),
        [index] => *index,
        _ => return Err(RoutingError::MultipleRulesSections),
    };
    let mut output = String::with_capacity(template.len() + rules.len() * 80);
    for (line_index, line) in lines.iter().enumerate() {
        output.push_str(line);
        if line_index == index {
            output.push_str("  # OmaVLESS custom rules — evaluated before the selected preset\n");
            for rule in rules {
                output.push_str(&rule.mihomo_line());
            }
        }
    }
    Ok(output)
}

pub fn template_with_mode(template: &str, mode: &str) -> Result<String, RoutingError> {
    if !matches!(mode, "rule" | "global" | "direct") {
        return Err(RoutingError::UnsupportedMode);
    }
    let lines = template.split_inclusive('\n').collect::<Vec<_>>();
    let matches = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            !line.starts_with([' ', '\t']) && line.trim_start().starts_with("mode:")
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(format!("mode: {mode}\n{template}")),
        [index] => {
            let line = lines[*index].trim_end_matches(['\r', '\n']);
            let comment = line
                .split_once('#')
                .map_or("", |(_, comment)| comment.trim());
            let newline = if lines[*index].ends_with("\r\n") {
                "\r\n"
            } else if lines[*index].ends_with('\n') {
                "\n"
            } else {
                ""
            };
            let replacement = if comment.is_empty() {
                format!("mode: {mode}{newline}")
            } else {
                format!("mode: {mode} # {comment}{newline}")
            };
            let mut output = String::new();
            for (line_index, line) in lines.iter().enumerate() {
                if line_index == *index {
                    output.push_str(&replacement);
                } else {
                    output.push_str(line);
                }
            }
            Ok(output)
        }
        _ => Err(RoutingError::MultipleModes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_domains_suffixes_and_networks() {
        assert_eq!(
            CustomRule::parse("domain", "proxy", " EXAMPLE.COM. ")
                .unwrap()
                .value,
            "example.com"
        );
        assert_eq!(
            CustomRule::parse("suffix", "direct", "*.täst.invalid")
                .unwrap()
                .value,
            "xn--tst-qla.invalid"
        );
        assert_eq!(
            CustomRule::parse("ipcidr", "reject", "192.0.2.42/24")
                .unwrap()
                .value,
            "192.0.2.0/24"
        );
        assert_eq!(
            CustomRule::parse("ipcidr", "reject", "2001:db8::42/64")
                .unwrap()
                .value,
            "2001:db8::/64"
        );
    }

    #[test]
    fn injects_before_existing_rules_and_rewrites_one_mode() {
        let rules = [CustomRule::parse("domain", "proxy", "example.invalid").unwrap()];
        let rendered =
            inject_custom_rules("mode: rule\nrules:\n  - MATCH,DIRECT\n", &rules).unwrap();
        assert!(
            rendered.find("DOMAIN,example.invalid,PROXY").unwrap()
                < rendered.find("MATCH,DIRECT").unwrap()
        );
        assert!(
            template_with_mode(&rendered, "global")
                .unwrap()
                .starts_with("mode: global\n")
        );
    }
}
