// SPDX-License-Identifier: MIT
//! Bounded read-only projections. Raw controller objects never reach the view.
use serde_json::Value;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Profiles,
    Traffic,
    Details,
    Diagnostics,
}
impl Page {
    pub fn next(self, reverse: bool) -> Self {
        let pages = [
            Self::Profiles,
            Self::Traffic,
            Self::Details,
            Self::Diagnostics,
        ];
        let index = pages.iter().position(|p| *p == self).unwrap_or(0);
        pages[(index + if reverse { 3 } else { 1 }) % pages.len()]
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Profiles => "tui.profiles",
            Self::Traffic => "tui.traffic",
            Self::Details => "tui.details",
            Self::Diagnostics => "tui.diagnostics",
        }
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
