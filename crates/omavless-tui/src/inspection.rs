// SPDX-License-Identifier: MIT
//! Bounded read-only projections. Raw controller objects never reach the view.
use serde_json::Value;

#[derive(Clone, Copy, Default)]
pub struct Capabilities {
    pub profile_details: bool,
    pub traffic: bool,
    pub diagnostics: bool,
    pub subscription_refresh: bool,
    pub profile_probe: bool,
    pub subscription_probe: bool,
    pub refresh_all: bool,
    pub connection_count: bool,
}
impl Capabilities {
    pub fn parse(methods: &[Value]) -> Self {
        let has = |method| methods.iter().any(|value| value == method);
        Self {
            profile_details: has("profiles.details"),
            traffic: has("runtime.traffic"),
            diagnostics: has("diagnostics.summary"),
            subscription_refresh: has("subscriptions.refresh"),
            profile_probe: has("profiles.probe")
                && has("profiles.probe_results")
                && has("operations.get")
                && has("operations.cancel"),
            subscription_probe: has("subscriptions.probe")
                && has("subscriptions.probe_results")
                && has("operations.get")
                && has("operations.cancel"),
            refresh_all: has("subscriptions.refresh_all")
                && has("operations.get")
                && has("operations.cancel"),
            connection_count: has("runtime.connections"),
        }
    }
}

pub fn connection_count(value: &Value) -> Option<u32> {
    let result = &value["result"];
    if value["ok"] != true
        || result["schemaVersion"] != 1
        || result["scope"] != "owned_core_active_connection_count"
        || result["availability"] != "observed"
    {
        return None;
    }
    result["count"]
        .as_u64()
        .filter(|n| *n <= 4096)
        .map(|n| n as u32)
}

/// Saved configuration categories, not an interoperability or health claim.
/// Never retain the private name/server/SNI fields of profiles.details.
#[derive(Clone)]
pub struct ProfileDetails {
    target: crate::client::ProfileTarget,
    pub protocol: Option<&'static str>,
    pub transport: Option<&'static str>,
    pub security: Option<&'static str>,
}
impl ProfileDetails {
    pub fn profile_id(&self) -> &str {
        self.target.as_str()
    }
    pub fn parse(value: &Value, target: crate::client::ProfileTarget) -> Option<Self> {
        if value["ok"] != true || value["result"]["version"] != 1 {
            return None;
        }
        let r = &value["result"];
        let token = |key: &str, allowed: &[&'static str]| {
            let value = r[key].as_str()?;
            allowed.iter().copied().find(|token| *token == value)
        };
        Some(Self {
            target,
            protocol: token("protocol", &["vless", "trojan", "hysteria2", "tuic"]),
            transport: token(
                "transport",
                &["tcp", "ws", "http", "h2", "grpc", "xhttp", "udp"],
            ),
            security: token("security", &["none", "tls", "reality"]),
        })
    }
}

/// Existing runtime log classifications only; zero never means network healthy.
#[derive(Clone)]
pub struct CoreDiagnostics {
    pub dns: u32,
    pub tls: u32,
    pub timeout: u32,
    pub connection: u32,
    pub other: u32,
    pub oversized: u32,
    pub incomplete: bool,
    pub finished: bool,
}
impl CoreDiagnostics {
    pub fn parse(value: &Value) -> Option<Self> {
        if value["scope"] != "latest_owned_core_log_counts" {
            return None;
        }
        let count = |key: &str| u32::try_from(value[key].as_u64()?).ok();
        Some(Self {
            dns: count("dnsErrors")?,
            tls: count("tlsErrors")?,
            timeout: count("timeoutErrors")?,
            connection: count("connectionErrors")?,
            other: count("otherWarnings")?,
            oversized: count("oversizedLines")?,
            incomplete: value["incomplete"].as_bool()? | value["readFailed"].as_bool()?,
            finished: value["finished"].as_bool()?,
        })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Profiles,
    Traffic,
    Details,
    Diagnostics,
    Settings,
    Activity,
    Subscriptions,
    Jobs,
}
impl Page {
    pub fn next(self, reverse: bool) -> Self {
        let pages = [
            Self::Profiles,
            Self::Traffic,
            Self::Details,
            Self::Diagnostics,
            Self::Settings,
            Self::Activity,
            Self::Jobs,
            Self::Subscriptions,
        ];
        let index = pages.iter().position(|p| *p == self).unwrap_or(0);
        pages[(index + if reverse { pages.len() - 1 } else { 1 }) % pages.len()]
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Profiles => "tui.profiles",
            Self::Traffic => "tui.traffic",
            Self::Details => "tui.details",
            Self::Diagnostics => "tui.diagnostics",
            Self::Settings => "tui.settings",
            Self::Activity => "tui.activity",
            Self::Subscriptions => "tui.subscriptions",
            Self::Jobs => "tui.jobs",
        }
    }
}

/// Saved metadata age, not last network attempt or a health assertion. The
/// owner's millisecond timestamp is also a monotonic commit token, so a future
/// value (clock rollback included) must not be rendered as "just updated".
pub fn saved_age(updated_at: Option<u64>, now_ms: u64) -> (&'static str, Option<u64>) {
    let Some(age) = updated_at
        .filter(|n| *n > 0)
        .and_then(|n| now_ms.checked_sub(n))
    else {
        return ("tui.metric_unavailable", None);
    };
    if age < 60_000 {
        ("tui.age_recent", None)
    } else if age < 3_600_000 {
        ("tui.age_minutes", Some(age / 60_000))
    } else if age < 86_400_000 {
        ("tui.age_hours", Some(age / 3_600_000))
    } else {
        ("tui.age_days", Some(age / 86_400_000))
    }
}

#[derive(Clone)]
pub struct Traffic {
    identity: String,
    pub upload: u64,
    pub download: u64,
    time: u64,
}
const MAX_INTEGER: u64 = 9_007_199_254_740_991;
fn integer(value: &Value) -> Option<u64> {
    value.as_u64().filter(|n| *n <= MAX_INTEGER)
}
impl Traffic {
    pub fn parse(value: &Value) -> Option<Self> {
        let r = &value["result"];
        if value["ok"] != true
            || r["schemaVersion"] != 1
            || r["scope"] != "controller_attributed_tun_counters"
            || r["availability"] != "observed"
        {
            return None;
        }
        let s = &r["sample"];
        let identity = s["identity"].as_str()?;
        if identity.len() != 64 || !identity.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        // Match the accepted QML/sysfs convention: RX download, TX upload.
        Some(Self {
            identity: identity.into(),
            upload: integer(&s["txBytes"])?,
            download: integer(&s["rxBytes"])?,
            time: integer(&s["sampledAtMs"])?,
        })
    }
    pub fn rates(&self, previous: &Self) -> Option<(u64, u64)> {
        let elapsed = self.time.checked_sub(previous.time)?;
        if self.identity != previous.identity || !(250..=10_000).contains(&elapsed) {
            return None;
        }
        let rate = |new: u64, old: u64| {
            let bytes = u128::from(new.checked_sub(old)?);
            u64::try_from(bytes * 1000 / u128::from(elapsed)).ok()
        };
        Some((
            rate(self.upload, previous.upload)?,
            rate(self.download, previous.download)?,
        ))
    }
}

#[derive(Clone)]
pub struct Diagnostics {
    pub rules: u64,
    pub providers: u64,
}
impl Diagnostics {
    pub fn parse(value: &Value) -> Option<Self> {
        if value["ok"] != true || value["result"]["version"] != 1 {
            return None;
        }
        // Deliberately ignore names, rules, endpoints and raw error messages.
        Some(Self {
            rules: value["result"]["rules"]["total"]
                .as_u64()
                .filter(|n| *n <= 65_536)?,
            providers: value["result"]["providers"]["total"]
                .as_u64()
                .filter(|n| *n <= 256)?,
        })
    }
}

pub fn bytes(value: u64) -> String {
    // Integral IEC formatting is locale-neutral; no hardcoded decimal separator.
    for (unit, scale) in [("GiB", 1_u64 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)] {
        if value >= scale {
            return format!("{} {unit}", value / scale);
        }
    }
    format!("{value} B")
}
