// SPDX-License-Identifier: MIT
//! Bounded read-only projections. Raw controller objects never reach the view.
use serde_json::Value;

#[derive(Clone, Copy, Default)]
pub struct Capabilities {
    pub profile_details: bool,
    pub traffic: bool,
    pub diagnostics: bool,
    pub support: bool,
    pub rules: bool,
    pub providers: bool,
    pub custom_rules: bool,
    pub subscription_refresh: bool,
    pub subscription_usage: bool,
    pub profile_probe: bool,
    pub subscription_probe: bool,
    pub refresh_all: bool,
    pub connection_count: bool,
    pub connection_overview: bool,
    pub connection_rows: bool,
}
impl Capabilities {
    pub fn parse(methods: &[Value]) -> Self {
        let has = |method| methods.iter().any(|value| value == method);
        Self {
            profile_details: has("profiles.details"),
            traffic: has("runtime.traffic"),
            diagnostics: has("diagnostics.summary"),
            support: has("diagnostics.export"),
            rules: has("diagnostics.rules"),
            providers: has("diagnostics.providers"),
            custom_rules: has("routing.custom_rules.list"),
            subscription_refresh: has("subscriptions.refresh"),
            subscription_usage: has("subscriptions.usage"),
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
            connection_overview: has("runtime.connection_overview"),
            connection_rows: has("runtime.connection_rows"),
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

#[derive(Clone, Copy)]
pub struct ConnectionOverview {
    pub total: u32,
    pub tcp: u32,
    pub udp: u32,
    pub other_network: u32,
    pub direct: u32,
    pub blocked: u32,
    pub vpn: u32,
    pub unclassified: u32,
}

#[derive(Clone)]
pub struct ConnectionRow {
    pub host: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub network: &'static str,
    pub route: &'static str,
}

#[derive(Clone)]
pub struct ConnectionRows {
    pub total: u32,
    pub truncated: bool,
    pub rows: Vec<ConnectionRow>,
}

impl ConnectionRows {
    pub fn parse(value: &Value) -> Option<Self> {
        let result = &value["result"];
        if value["ok"] != true
            || result["schemaVersion"] != 1
            || result["scope"] != "owned_core_private_connection_rows"
            || result["availability"] != "observed"
        {
            return None;
        }
        let total = u32::try_from(result["total"].as_u64().filter(|n| *n <= 4096)?).ok()?;
        let rows = result["rows"].as_array().filter(|rows| rows.len() <= 128)?;
        if result["shown"].as_u64()? != rows.len() as u64
            || (total as usize) < rows.len()
            || result["truncated"].as_bool()? != (total as usize > rows.len())
        {
            return None;
        }
        let token = |value: &Value, allowed: &[&'static str]| {
            let value = value.as_str()?;
            allowed.iter().copied().find(|token| *token == value)
        };
        let mut parsed = Vec::with_capacity(rows.len());
        for row in rows {
            let host = row["host"]
                .as_str()
                .filter(|host| {
                    !host.is_empty()
                        && host.len() <= 120
                        && host
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
                })
                .map(str::to_owned);
            let ip = row["ip"]
                .as_str()
                .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
                .map(|ip| ip.to_string());
            let port = row["port"]
                .as_u64()
                .and_then(|port| u16::try_from(port).ok())
                .filter(|port| *port != 0);
            parsed.push(ConnectionRow {
                host,
                ip,
                port,
                network: token(&row["network"], &["tcp", "udp", "other"])?,
                route: token(&row["route"], &["direct", "blocked", "vpn", "unclassified"])?,
            });
        }
        Some(Self {
            total,
            truncated: result["truncated"].as_bool()?,
            rows: parsed,
        })
    }
}

impl ConnectionOverview {
    pub fn parse(value: &Value) -> Option<Self> {
        let result = &value["result"];
        if value["ok"] != true
            || result["schemaVersion"] != 1
            || result["scope"] != "owned_core_connection_categories"
            || result["availability"] != "observed"
        {
            return None;
        }
        let count = |path: &Value| u32::try_from(path.as_u64().filter(|n| *n <= 4096)?).ok();
        let overview = Self {
            total: count(&result["total"])?,
            tcp: count(&result["network"]["tcp"])?,
            udp: count(&result["network"]["udp"])?,
            other_network: count(&result["network"]["other"])?,
            direct: count(&result["outcome"]["direct"])?,
            blocked: count(&result["outcome"]["blocked"])?,
            vpn: count(&result["outcome"]["vpn"])?,
            unclassified: count(&result["outcome"]["unclassified"])?,
        };
        if overview.tcp + overview.udp + overview.other_network != overview.total
            || overview.direct + overview.blocked + overview.vpn + overview.unclassified
                != overview.total
        {
            return None;
        }
        Some(overview)
    }
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

#[derive(Clone)]
pub struct CoreLogHint {
    pub sequence: u32,
    pub category: &'static str,
}

#[derive(Clone)]
pub struct CoreLogHints {
    pub items: Vec<CoreLogHint>,
    pub incomplete: bool,
}

impl CoreLogHints {
    pub fn parse(value: &Value) -> Option<Self> {
        if value["schemaVersion"] != 1
            || value["scope"] != "latest_owned_core_log_categories"
            || value["availability"] != "observed"
            || value["interpretation"] != "log_hints_not_health"
        {
            return None;
        }
        let raw = value["items"]
            .as_array()
            .filter(|items| items.len() <= 24)?;
        let mut items = Vec::with_capacity(raw.len());
        let mut previous = 0;
        for item in raw {
            let sequence = u32::try_from(item["sequence"].as_u64()?).ok()?;
            if sequence <= previous {
                return None;
            }
            previous = sequence;
            let category = match item["category"].as_str()? {
                "dns" => "dns",
                "tls" => "tls",
                "timeout" => "timeout",
                "connection" => "connection",
                "other" => "other",
                "oversized" => "oversized",
                _ => return None,
            };
            items.push(CoreLogHint { sequence, category });
        }
        Some(Self {
            items,
            incomplete: value["incomplete"].as_bool()?,
        })
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Profiles,
    Traffic,
    Connections,
    Details,
    Diagnostics,
    Host,
    Rules,
    Providers,
    CustomRules,
    RouteCheck,
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
            Self::Connections,
            Self::Diagnostics,
            Self::Settings,
            Self::Activity,
            Self::Host,
            Self::Rules,
            Self::Providers,
            Self::CustomRules,
            Self::RouteCheck,
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
            Self::Connections => "tui.connections",
            Self::Details => "tui.details",
            Self::Diagnostics => "tui.diagnostics",
            Self::Host => "tui.host",
            Self::Rules => "tui.rules",
            Self::Providers => "tui.providers",
            Self::CustomRules => "tui.custom_rules",
            Self::RouteCheck => "tui.route_check",
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

/// Private, bounded controller projections. These are display data, never
/// shareable diagnostics: rule payloads can name user-selected destinations.
#[derive(Clone)]
pub struct Rule {
    pub kind: String,
    pub payload: String,
    pub target: &'static str,
}

#[derive(Clone)]
pub struct Rules {
    pub total: u64,
    pub truncated: bool,
    pub items: Vec<Rule>,
}

#[derive(Clone)]
pub struct Provider {
    pub name: String,
    pub behavior: String,
    pub updated_at: String,
    pub rule_count: Option<u64>,
    pub status: &'static str,
    pub refreshable: bool,
}

#[derive(Clone)]
pub struct Providers {
    pub total: u64,
    pub truncated: bool,
    pub items: Vec<Provider>,
}

/// Private editor payload. The opaque ID is validated then discarded because
/// this page cannot edit or delete a rule. Never log or serialize these rows.
#[derive(Clone)]
pub struct CustomRule {
    pub kind: &'static str,
    pub action: &'static str,
    pub value: String,
}

#[derive(Clone)]
pub struct CustomRules {
    pub items: Vec<CustomRule>,
}

impl CustomRules {
    pub fn parse(value: &Value) -> Option<Self> {
        if value["ok"] != true || value["result"]["version"] != 1 {
            return None;
        }
        let raw = value["result"]["rules"].as_array()?;
        if raw.len() > 128 {
            return None;
        }
        let mut items = Vec::with_capacity(raw.len());
        for item in raw {
            let id = item["id"].as_str()?;
            if !crate::model::opaque(id) {
                return None;
            }
            let kind = match item["kind"].as_str()? {
                "domain" => "domain",
                "suffix" => "suffix",
                "ipcidr" => "ipcidr",
                _ => return None,
            };
            let action = match item["action"].as_str()? {
                "proxy" => "PROXY",
                "direct" => "DIRECT",
                "reject" => "REJECT",
                _ => return None,
            };
            let input = item["value"].as_str()?;
            if input.is_empty() || input.len() > 1024 {
                return None;
            }
            items.push(CustomRule {
                kind,
                action,
                value: crate::model::display(input, 1024),
            });
        }
        Some(Self { items })
    }
}

#[derive(Clone)]
pub struct HostSupport {
    pub core_installed: Option<bool>,
    pub core_capabilities: Option<bool>,
    pub tun_device: Option<bool>,
    pub runtime_unit_loaded: Option<bool>,
    pub runtime_unit_active: Option<bool>,
    pub runtime_unit_enabled: Option<bool>,
    pub store_present: Option<bool>,
    pub template_present: Option<bool>,
    pub generated_config_present: Option<bool>,
    pub package_runtime_unit_present: Option<bool>,
    pub package_login_unit_present: Option<bool>,
}

fn nullable_bool(value: &Value) -> Option<Option<bool>> {
    if value.is_null() {
        Some(None)
    } else {
        value.as_bool().map(Some)
    }
}

impl HostSupport {
    pub fn parse(value: &Value) -> Option<Self> {
        let result = &value["result"];
        if value["ok"] != true
            || result["schemaVersion"] != 3
            || result["scope"] != "native_support"
        {
            return None;
        }
        let host = result["host"].as_object()?;
        for key in [
            "core",
            "runtimeService",
            "loginService",
            "files",
            "configuredPolicy",
        ] {
            if !host.contains_key(key) {
                return None;
            }
        }
        let core = &result["host"]["core"];
        let core_installed = if core.is_null() {
            None
        } else {
            core.as_object()?;
            let installed = core["installed"].as_bool()?;
            let capabilities = core
                .get("fileNetworkCapabilities")
                .and_then(nullable_bool)?;
            if !installed && capabilities != Some(false) {
                return None;
            }
            Some(installed)
        };
        let core_capabilities = if core.is_null() {
            None
        } else {
            core.get("fileNetworkCapabilities")
                .and_then(nullable_bool)?
        };
        let tun_device = if core.is_null() {
            None
        } else {
            core.get("tunDevicePresent").and_then(nullable_bool)?
        };
        let service = &result["host"]["runtimeService"];
        let (runtime_unit_loaded, runtime_unit_active, runtime_unit_enabled) = if service.is_null()
        {
            (None, None, None)
        } else {
            service.as_object()?;
            let loaded = service["loaded"].as_bool()?;
            let active = service["active"].as_bool()?;
            let enabled = service["enabled"].as_bool()?;
            let owns = service["ownsCurrentProcess"].as_bool()?;
            if (!loaded && (active || enabled)) || (owns && !active) {
                return None;
            }
            (Some(loaded), Some(active), Some(enabled))
        };
        let login = &result["host"]["loginService"];
        if !login.is_null() {
            login.as_object()?;
            let loaded = login["loaded"].as_bool()?;
            let active = login["active"].as_bool()?;
            let enabled = login["enabled"].as_bool()?;
            if (!loaded && (active || enabled)) || !login["ownsCurrentProcess"].is_null() {
                return None;
            }
        }
        let files = &result["host"]["files"];
        if !files.is_null() {
            files.as_object()?;
        }
        let file = |key: &str| {
            if files.is_null() {
                Some(None)
            } else {
                files.get(key).and_then(nullable_bool)
            }
        };
        let store_present = file("store")?;
        let template_present = file("template")?;
        let generated_config_present = file("generatedConfig")?;
        let package_runtime_unit_present = file("runtimeUnit")?;
        let package_login_unit_present = file("loginUnit")?;
        let policy = &result["host"]["configuredPolicy"];
        if !policy.is_null()
            && (!["template", "active_config"].contains(&policy["basis"].as_str()?)
                || policy["rules"].as_u64().filter(|n| *n <= 100_000).is_none()
                || policy["providers"]
                    .as_u64()
                    .filter(|n| *n <= 1024)
                    .is_none())
        {
            return None;
        }
        let coverage = &result["coverage"];
        coverage.as_object()?;
        if coverage["coreSetupVerified"].as_bool()?
            != (core_installed.is_some() && core_capabilities.is_some() && tun_device.is_some())
            || coverage["serviceEnablementVerified"].as_bool()?
                != (!service.is_null() && !login.is_null())
            || coverage["fileReadiness"].as_bool()?
                != (!files.is_null()
                    && [
                        store_present,
                        template_present,
                        generated_config_present,
                        package_runtime_unit_present,
                        package_login_unit_present,
                    ]
                    .iter()
                    .all(Option::is_some))
            || coverage["loadedPolicyCounts"].as_bool()?
        {
            return None;
        }
        Some(Self {
            core_installed,
            core_capabilities,
            tun_device,
            runtime_unit_loaded,
            runtime_unit_active,
            runtime_unit_enabled,
            store_present,
            template_present,
            generated_config_present,
            package_runtime_unit_present,
            package_login_unit_present,
        })
    }
}

fn bounded_text(value: &Value, max: usize) -> Option<String> {
    let text = value.as_str()?;
    (text.len() <= max).then(|| crate::model::display(text, max))
}

fn rows<'a>(
    value: &'a Value,
    key: &str,
    maximum: usize,
    total_max: u64,
) -> Option<(u64, bool, &'a [Value])> {
    if value["ok"] != true || value["result"]["version"] != 1 {
        return None;
    }
    let projection = &value["result"][key];
    let total = projection["total"].as_u64().filter(|n| *n <= total_max)?;
    let items = projection["items"].as_array()?;
    let shown = projection["shown"].as_u64()?;
    let truncated = projection["truncated"].as_bool()?;
    if items.len() > maximum
        || shown != items.len() as u64
        || shown > total
        || truncated != (shown < total)
    {
        return None;
    }
    Some((total, truncated, items))
}

impl Rules {
    pub fn parse(value: &Value) -> Option<Self> {
        // The core can load up to 65,536 rules, while the IPC projection
        // intentionally shows at most 2,048 of them.
        let (total, truncated, raw) = rows(value, "rules", 2048, 65_536)?;
        let mut items = Vec::with_capacity(raw.len());
        for item in raw {
            let target = match item["target"].as_str()? {
                "DIRECT" => "DIRECT",
                "REJECT" => "REJECT",
                "VPN" => "VPN",
                _ => return None,
            };
            items.push(Rule {
                kind: bounded_text(&item["type"], 80)?,
                payload: bounded_text(&item["payload"], 512)?,
                target,
            });
        }
        Some(Self {
            total,
            truncated,
            items,
        })
    }
}

impl Providers {
    pub fn parse(value: &Value) -> Option<Self> {
        let (total, truncated, raw) = rows(value, "providers", 256, 256)?;
        let mut items = Vec::with_capacity(raw.len());
        for item in raw {
            let count = item["ruleCount"]
                .as_i64()
                .filter(|n| (-1..=1_000_000_000).contains(n))?;
            let status = match item["status"].as_str()? {
                "unknown" if count == -1 => "unknown",
                "empty" if count == 0 => "empty",
                "loaded" if count > 0 => "loaded",
                _ => return None,
            };
            items.push(Provider {
                name: bounded_text(&item["name"], 160)?,
                behavior: bounded_text(&item["behavior"], 80)?,
                updated_at: bounded_text(&item["updatedAt"], 80)?,
                rule_count: (count >= 0).then_some(count as u64),
                status,
                refreshable: item["refreshable"].as_bool()?,
            });
        }
        Some(Self {
            total,
            truncated,
            items,
        })
    }
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
