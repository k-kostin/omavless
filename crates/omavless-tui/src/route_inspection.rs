// SPDX-License-Identifier: MIT
//! One-shot private routing checks. No query/result enters Debug or activity.
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Target {
    bytes: [u8; 256],
    len: u16,
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RouteTarget([private])")
    }
}

impl Target {
    pub fn new(input: &str) -> Option<Self> {
        let canonical = omavless_domain::route_check::canonical_query(input).ok()?;
        if canonical.is_empty() || canonical.len() > 256 || !canonical.is_ascii() {
            return None;
        }
        let mut bytes = [0; 256];
        bytes[..canonical.len()].copy_from_slice(canonical.as_bytes());
        Some(Self {
            bytes,
            len: canonical.len() as u16,
        })
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or_default()
    }
}

/// One explicit user request, bound to the currently observed owner before
/// any private query can be dispatched. Never serialized or debug-printed.
#[derive(Clone)]
pub struct Request {
    target: Target,
    instance: String,
    revision: u64,
}

impl Request {
    pub fn new(target: Target, instance: String, revision: u64) -> Self {
        Self {
            target,
            instance,
            revision,
        }
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn matches(&self, instance: &str, revision: u64) -> bool {
        self.instance == instance && self.revision == revision
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Vpn,
    Direct,
    Block,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Mode,
    Custom,
    Live,
    Disconnected,
}

pub enum Status {
    Observed(Result),
    Unavailable,
    Unsupported,
    InvalidInput,
}

/// UI-private response. The source rule and destination can contain private
/// profile-derived text, so this type must not implement Debug/Serialize.
pub struct Result {
    pub query: String,
    pub outcome: Outcome,
    pub source: Source,
    pub rule_type: String,
    pub rule_payload: String,
}

impl Result {
    pub fn parse(value: &Value, target: Target, expected_revision: u64) -> Option<Self> {
        if value["ok"] != true || value["revision"].as_u64()? != expected_revision {
            return None;
        }
        let result = &value["result"];
        if result["version"] != 1 || result["query"].as_str()? != target.as_str() {
            return None;
        }
        let outcome = match result["outcome"].as_str()? {
            "vpn" => Outcome::Vpn,
            "direct" => Outcome::Direct,
            "block" => Outcome::Block,
            "unknown" => Outcome::Unknown,
            _ => return None,
        };
        let source = match result["source"].as_str()? {
            "mode" => Source::Mode,
            "custom" => Source::Custom,
            "live" => Source::Live,
            "disconnected" => Source::Disconnected,
            _ => return None,
        };
        if (outcome == Outcome::Unknown) != (source == Source::Disconnected) {
            return None;
        }
        let rule_type = bounded(result["ruleType"].as_str()?, 80)?;
        let rule_payload = bounded(result["rulePayload"].as_str()?, 256)?;
        let expected_target = match outcome {
            Outcome::Vpn => "PROXY",
            Outcome::Direct => "DIRECT",
            Outcome::Block => {
                let target = result["target"].as_str()?;
                if !matches!(target, "REJECT" | "REJECT-DROP") {
                    return None;
                }
                target
            }
            Outcome::Unknown => "",
        };
        if result["target"] != expected_target {
            return None;
        }
        Some(Self {
            query: target.as_str().to_owned(),
            outcome,
            source,
            rule_type,
            rule_payload,
        })
    }
}

fn bounded(value: &str, max: usize) -> Option<String> {
    if value.len() > max || value.chars().any(char::is_control) {
        return None;
    }
    Some(crate::model::display(value, max))
}
