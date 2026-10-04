// SPDX-License-Identifier: MIT
//! Session-only private usage claims. No Debug, logs, exports or persistence.
use crate::{
    client::{ProfileTarget, Read},
    inspection::Page,
    model::{ReadError, Snapshot},
};
use serde_json::Value;

#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub target: ProfileTarget,
    instance: String,
    revision: u64,
    token: std::time::Instant,
}
impl Request {
    pub fn new(target: ProfileTarget, instance: String, revision: u64) -> Self {
        Self {
            target,
            instance,
            revision,
            token: std::time::Instant::now(),
        }
    }
    pub fn matches(&self, snapshot: &Snapshot) -> bool {
        self.instance == snapshot.metadata.instance_id
            && self.revision == snapshot.revision
            && snapshot
                .metadata
                .subscriptions
                .iter()
                .any(|s| s.id == self.target.as_str())
    }
}

pub enum Status {
    Loading,
    Reported(Usage),
    NotProvided,
    Unavailable,
    Unsupported,
}
pub struct Usage {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expiry_utc: Option<String>,
}

fn decimal(value: &Value) -> Option<u64> {
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
impl Usage {
    pub fn parse(value: &Value, request: &Request) -> Option<Status> {
        let result = &value["result"];
        if value["ok"] != true
            || value["revision"].as_u64()? != request.revision
            || result["instanceId"] != request.instance
            || result["schemaVersion"] != 1
            || result["scope"] != "private_provider_reported_usage"
        {
            return None;
        }
        if result["availability"] == "not_provided" && result["usage"].is_null() {
            return Some(Status::NotProvided);
        }
        if result["availability"] != "reported" {
            return None;
        }
        let usage = &result["usage"];
        let expiry_utc = if usage["expiryUnixSeconds"].is_null() {
            None
        } else {
            let seconds = decimal(&usage["expiryUnixSeconds"])?;
            if seconds > 253_402_300_799 {
                return None;
            }
            (seconds != 0).then(|| utc_date(seconds)).flatten()
        };
        Some(Status::Reported(Self {
            upload: decimal(&usage["uploadBytes"])?,
            download: decimal(&usage["downloadBytes"])?,
            total: decimal(&usage["totalBytes"])?,
            expiry_utc,
        }))
    }
    pub fn remaining(&self) -> Option<u64> {
        (self.total != 0)
            .then_some(self.total)?
            .checked_sub(self.upload.checked_add(self.download)?)
    }
}

/// An unambiguous UTC date/time, never a health/entitlement countdown. Bounded
/// to Gregorian 1970..9999. Zero is deliberately not called "never expires".
pub fn utc_date(seconds: u64) -> Option<String> {
    if seconds == 0 || seconds > 253_402_300_799 {
        return None;
    }
    let leap = |year| year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut days = seconds / 86_400;
    let mut year = 1970;
    while days >= if leap(year) { 366 } else { 365 } {
        days -= if leap(year) { 366 } else { 365 };
        year += 1;
    }
    let months = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while days >= months[month] {
        days -= months[month];
        month += 1;
    }
    Some(format!(
        "{year:04}-{:02}-{:02} {:02}:{:02} UTC",
        month + 1,
        days + 1,
        seconds % 86_400 / 3600,
        seconds % 3600 / 60
    ))
}

/// Existing no-provider page reads are unchanged. Only an explicit request
/// admits one GET; newer metadata/owner facts fence its late private result.
pub fn load(
    read: &mut impl FnMut(Read) -> Result<Value, ReadError>,
    page: Page,
    snapshot: Snapshot,
    request: Option<&Request>,
) -> Result<(Snapshot, Option<Status>), ReadError> {
    let Some(request) = request else {
        return Ok((snapshot, None));
    };
    if page != Page::Subscriptions || !request.matches(&snapshot) {
        return Ok((snapshot, Some(Status::Unavailable)));
    }
    if !snapshot.capabilities.subscription_usage {
        return Ok((snapshot, Some(Status::Unsupported)));
    }
    let value = read(Read::SubscriptionUsage(request.target));
    let latest = crate::client::load_page(read, page)?;
    if !request.matches(&latest) {
        return Err(ReadError::Changed);
    }
    let status = value
        .ok()
        .and_then(|v| Usage::parse(&v, request))
        .unwrap_or(Status::Unavailable);
    Ok((latest, Some(status)))
}
