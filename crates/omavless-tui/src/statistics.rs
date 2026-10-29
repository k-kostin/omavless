// SPDX-License-Identifier: MIT
//! Private recorded totals and fixed statistics controls, separate from network
//! actions and ephemeral live charts. No state file or sampler in this client.
use crate::{
    i18n::Locale,
    model::{ReadError, Snapshot, Status},
};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct Data {
    pub enabled: bool,
    pub since: u64,
    pub received: u64,
    pub sent: u64,
    pub partial: bool,
    pub overflow: bool,
    pub revision: u64,
}
fn integer(value: &Value) -> Option<u64> {
    let text = value.as_str()?;
    if text.is_empty()
        || text.len() > 20
        || !text.bytes().all(|b| b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return None;
    }
    text.parse().ok()
}
impl Data {
    pub fn parse(value: &Value, instance: &str, owner_revision: u64) -> Option<Self> {
        let r = &value["result"];
        if value["ok"] != true
            || value["revision"].as_u64()? != owner_revision
            || r.as_object()?.len() != 10
            || r["schemaVersion"] != 1
            || r["scope"] != "private_recorded_tun_bytes"
            || r["instanceId"] != instance
        {
            return None;
        }
        let overflow = match r["availability"].as_str()? {
            "recorded" => false,
            "overflow" => true,
            _ => return None,
        };
        let data = Self {
            enabled: r["enabled"].as_bool()?,
            since: r["periodStartedAtUtc"].as_u64()?,
            received: integer(&r["receivedBytes"])?,
            sent: integer(&r["sentBytes"])?,
            partial: r["partial"].as_bool()?,
            overflow,
            revision: r["controlRevision"].as_u64()?,
        };
        if data.since > 253_402_300_799
            || data.revision > 9_007_199_254_740_991
            || (data.since == 0
                && (data.enabled || data.partial || data.received != 0 || data.sent != 0))
            || (overflow && !data.partial)
        {
            return None;
        }
        Some(data)
    }
}

pub fn grouped(value: u64, locale: Locale) -> String {
    let text = value.to_string();
    let separator = if locale == Locale::Ru { '\u{a0}' } else { ',' };
    let mut output = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i > 0 && (text.len() - i).is_multiple_of(3) {
            output.push(separator);
        }
        output.push(ch);
    }
    output
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Set(bool),
    Reset,
}
#[derive(Clone)]
pub struct Review {
    instance: String,
    owner_revision: u64,
    stats_revision: u64,
    since: u64,
    enabled: bool,
    pub control: Control,
}
impl Review {
    pub fn new(snapshot: &Snapshot, control: Control) -> Option<Self> {
        let data = snapshot.statistics.as_ref()?;
        if !snapshot.capabilities.statistics
            || snapshot.status() != Status::Disconnected
            || snapshot.metadata.desired.connected
            || (control == Control::Reset && data.since == 0)
        {
            return None;
        }
        Some(Self {
            instance: snapshot.metadata.instance_id.clone(),
            owner_revision: snapshot.revision,
            stats_revision: data.revision,
            since: data.since,
            enabled: data.enabled,
            control,
        })
    }
    pub fn matches(&self, snapshot: &Snapshot) -> bool {
        self.instance == snapshot.metadata.instance_id
            && self.owner_revision == snapshot.revision
            && snapshot.status() == Status::Disconnected
            && !snapshot.metadata.desired.connected
            && snapshot
                .statistics
                .as_ref()
                .is_some_and(|d| d.revision == self.stats_revision && d.since == self.since)
    }
    pub fn key(&self) -> &'static str {
        match self.control {
            Control::Reset => "stats.reset_notice",
            Control::Set(true) => "stats.enable",
            Control::Set(false) => "stats.disable",
        }
    }
}

#[derive(Clone)]
pub struct Request {
    review: Review,
    operation: String,
}
impl Request {
    pub fn new(review: Review, operation: String) -> Option<Self> {
        if operation.is_empty()
            || operation.len() > 64
            || !operation.bytes().all(|b| (33..=126).contains(&b))
        {
            return None;
        }
        Some(Self { review, operation })
    }
    pub fn method(&self) -> &'static str {
        if self.review.control == Control::Reset {
            "stats.reset"
        } else {
            "stats.configure"
        }
    }
    pub fn params(&self) -> Value {
        let r = &self.review;
        let mut params = json!({"instanceId":r.instance,"expectedRevision":r.owner_revision,"statisticsRevision":r.stats_revision,"operationId":self.operation});
        if let Control::Set(enabled) = r.control {
            params["enabled"] = enabled.into();
        }
        params
    }
    pub fn same(&self, other: &Self) -> bool {
        self.operation == other.operation && self.review.instance == other.review.instance
    }
    pub fn not_sent(self) -> Completion {
        Completion {
            request: self,
            unknown: false,
            error: "stats.unavailable",
        }
    }
    pub fn settle(self, response: Result<Value, ReadError>) -> Completion {
        let mut unknown = true;
        let mut error = "stats.unconfirmed";
        if let Ok(value) = response {
            if !value["revision"]
                .as_u64()
                .is_some_and(|r| r <= 9_007_199_254_740_991)
            {
                return Completion {
                    request: self,
                    unknown: true,
                    error: "stats.unconfirmed",
                };
            }
            let mut read_shape = value.clone();
            if let Some(result) = read_shape["result"].as_object_mut() {
                result.remove("operationId");
            }
            let verified = Data::parse(
                &read_shape,
                &self.review.instance,
                self.review.owner_revision,
            );
            if value["ok"] == true
                && value["result"]["operationId"] == self.operation
                && verified.is_some_and(|data| {
                    let revision_matches = data.revision == self.review.stats_revision
                        || self.review.stats_revision.checked_add(1) == Some(data.revision);
                    revision_matches
                        && match self.review.control {
                            Control::Set(enabled) => data.enabled == enabled,
                            Control::Reset => {
                                data.enabled == self.review.enabled
                                    && data.since > 0
                                    && data.received == 0
                                    && data.sent == 0
                                    && !data.partial
                                    && self.review.stats_revision.checked_add(1)
                                        == Some(data.revision)
                            }
                        }
                })
            {
                unknown = false;
                error = "";
            } else if value["ok"] == false {
                error = match value["error"]["code"].as_str() {
                    Some("busy") => "error.busy",
                    Some("conflict") => "error.conflict",
                    Some("daemon_restarting") => "error.daemon_restarting",
                    Some("capability_unavailable") => "error.capability_unavailable",
                    Some("invalid_argument") => "error.invalid_argument",
                    _ => "stats.unconfirmed",
                };
                unknown = error == "stats.unconfirmed";
            }
        }
        Completion {
            request: self,
            unknown,
            error,
        }
    }
}
pub struct Completion {
    pub request: Request,
    pub unknown: bool,
    pub error: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_bounds_and_grouping_do_not_round_private_totals() {
        assert_eq!(integer(&json!("18446744073709551615")), Some(u64::MAX));
        for value in [
            json!("18446744073709551616"),
            json!("01"),
            json!(1),
            json!("1e3"),
            json!("-1"),
        ] {
            assert!(integer(&value).is_none());
        }
        assert_eq!(grouped(u64::MAX, Locale::En), "18,446,744,073,709,551,615");
        assert_eq!(grouped(1000, Locale::Ru), "1\u{a0}000");
    }
}
