// SPDX-License-Identifier: MIT
//! Fixed client commands, not a second connection state machine.
//! Private targets/receipts deliberately have no Debug/Serialize implementation.
use crate::model::{Mode, ReadError};
use serde_json::{Value, json};
use std::io::Read;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Connect,
    Disconnect,
    Mode,
    RefreshSubscription,
}
impl Kind {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Disconnect => "disconnect",
            Self::Mode => "mode",
            Self::RefreshSubscription => "subscription-refresh",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Connect => "tui.connect",
            Self::Disconnect => "tui.disconnect",
            Self::Mode => "tui.set_mode",
            Self::RefreshSubscription => "tui.refresh_subscription",
        }
    }
}
impl Mode {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Global => "global",
            Self::Direct => "direct",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Rule => "mode.routing",
            Self::Global => "mode.full_vpn",
            Self::Direct => "mode.direct",
        }
    }
}

#[derive(Clone)]
pub struct Command {
    pub(crate) instance: String,
    pub(crate) revision: u64,
    pub kind: Kind,
    pub(crate) profile: String,
    pub(crate) subscription: Option<String>,
    pub name: String,
    pub source: Option<String>,
    pub was_connected: bool,
    pub mode: Mode,
}
#[derive(Clone)]
pub struct Request {
    pub command: Command,
    operation: String,
}
impl Request {
    pub(crate) fn new(command: Command, operation: String) -> Option<Self> {
        if operation.is_empty()
            || operation.len() > 64
            || !operation.bytes().all(|b| (33..=126).contains(&b))
        {
            return None;
        }
        Some(Self { command, operation })
    }
    /// Only the existing fixed plugin.action bridge may consume these params.
    pub fn params(&self) -> Value {
        let c = &self.command;
        let mut p = json!({"instanceId":c.instance,"expectedRevision":c.revision,
            "operationId":self.operation,"action":c.kind.wire()});
        if c.kind == Kind::Connect {
            p["profileId"] = json!(c.profile);
        }
        if matches!(c.kind, Kind::Connect | Kind::Mode) {
            p["mode"] = json!(c.mode.wire());
        }
        if c.kind == Kind::RefreshSubscription {
            p["subscriptionId"] = json!(c.subscription);
        }
        p
    }
    pub fn outcome(&self, response: Result<Value, ReadError>) -> Outcome {
        let Ok(v) = response else {
            return Outcome::Unknown;
        };
        // The real IPC transport already validates framing, request ID and envelope.
        if !v["revision"].as_u64().is_some_and(|r| r <= i64::MAX as u64) {
            return Outcome::Unknown;
        }
        if v["ok"] == true {
            let r = &v["result"];
            let summary = r
                .get("importSummary")
                .and_then(omavless_domain::subscription_feed::ImportSummary::from_value);
            if r.as_object().is_some_and(|o| {
                o.len() == 5
                    || (o.len() == 6
                        && self.command.kind == Kind::RefreshSubscription
                        && summary.is_some())
            }) && r["schemaVersion"] == 1
                && r["applied"] == true
                && r["instanceId"] == self.command.instance
                && r["operationId"] == self.operation
                && r["action"] == self.command.kind.wire()
                && v["revision"]
                    .as_u64()
                    .is_some_and(|r| r >= self.command.revision)
            {
                return summary.map_or(Outcome::Applied, Outcome::ImportApplied);
            }
        } else if v["ok"] == false {
            if self.command.kind == Kind::RefreshSubscription
                && let Some(reasons) = crate::private_import::import_error(&v)
            {
                return Outcome::ImportRejected(reasons);
            }
            // Never render remote message/details or unfamiliar codes.
            return match v["error"]["code"].as_str() {
                Some("dns_pair_required")
                    if self.command.kind == Kind::Connect
                        && v["revision"]
                            .as_u64()
                            .is_some_and(|r| r >= self.command.revision)
                        && v["error"].as_object().is_some_and(|e| e.len() == 3)
                        && v["error"]["retryable"] == false
                        && v["error"]["message"]
                            .as_str()
                            .is_some_and(|m| m.len() <= 512)
                        && v["error"].get("details").is_none() =>
                {
                    Outcome::Rejected("error.dns_pair_required")
                }
                Some("conflict" | "daemon_restarting") => Outcome::Rejected("tui.action_changed"),
                Some("busy") => Outcome::Rejected("tui.action_busy"),
                Some("manual_recovery_required") => Outcome::Rejected("tui.action_recovery"),
                Some("transition_failed_restored") => Outcome::Rejected("tui.action_restored"),
                Some("permission_denied") => Outcome::Rejected("tui.action_denied"),
                Some("invalid_argument" | "not_found" | "capability_unavailable") => {
                    Outcome::Rejected("tui.action_rejected")
                }
                _ => Outcome::Unknown,
            };
        }
        Outcome::Unknown
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Applied,
    ImportApplied(omavless_domain::subscription_feed::ImportSummary),
    ImportRejected(omavless_domain::subscription_feed::ImportReasons),
    Rejected(&'static str),
    Unknown,
}

pub(crate) fn operation_id() -> Option<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    Some(format!("tui-{:032x}", u128::from_ne_bytes(bytes)))
}
