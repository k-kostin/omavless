// SPDX-License-Identifier: MIT

//! Canonical bounded Hysteria2 profile adapter for R2.

use std::collections::BTreeMap;
use std::fmt;
use std::net::Ipv6Addr;
use std::str::FromStr;

use base64::Engine as _;
use serde_json::{Map, Number, Value};

use crate::profile_uri::{
    canonical_host, encoded_query, extract_uri, percent_decode_strict, query_pairs, quote,
    sanitize_label, split_uri, text_is_bounded,
};
use crate::vless::HostKind;
use crate::vless_canonical::{canonical_json, sha256_hex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hysteria2Error {
    InvalidInput,
    MissingLink,
    InvalidLink,
    UnsupportedPath,
    InvalidAuthority,
    InvalidAuthentication,
    InvalidHost,
    InvalidPortList,
    InvalidPortRange,
    OverlappingPorts,
    InvalidQuery,
    DuplicateField,
    LocalBandwidth,
    UnsupportedField,
    UnsupportedObfuscation,
    MissingObfuscationPassword,
    MissingObfuscationType,
    InvalidText,
    InvalidInsecure,
    InvalidFingerprint,
    InvalidEch,
}

impl Hysteria2Error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::MissingLink => "missing_hysteria2_link",
            Self::InvalidLink => "invalid_link",
            Self::UnsupportedPath => "unsupported_path",
            Self::InvalidAuthority => "invalid_authority",
            Self::InvalidAuthentication => "invalid_authentication",
            Self::InvalidHost => "invalid_host",
            Self::InvalidPortList => "invalid_port_list",
            Self::InvalidPortRange => "invalid_port_range",
            Self::OverlappingPorts => "overlapping_ports",
            Self::InvalidQuery => "invalid_query",
            Self::DuplicateField => "duplicate_field",
            Self::LocalBandwidth => "local_bandwidth",
            Self::UnsupportedField => "unsupported_field",
            Self::UnsupportedObfuscation => "unsupported_obfuscation",
            Self::MissingObfuscationPassword => "missing_obfuscation_password",
            Self::MissingObfuscationType => "missing_obfuscation_type",
            Self::InvalidText => "invalid_text",
            Self::InvalidInsecure => "invalid_insecure",
            Self::InvalidFingerprint => "invalid_fingerprint",
            Self::InvalidEch => "invalid_ech",
        }
    }
}

impl fmt::Display for Hysteria2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "Hysteria2 input is invalid",
            Self::MissingLink => "Input does not contain a Hysteria2 link",
            Self::InvalidLink => "Hysteria2 link is invalid",
            Self::UnsupportedPath => "Hysteria2 link path is not supported",
            Self::InvalidAuthority => "Hysteria2 link has an invalid authority",
            Self::InvalidAuthentication => "Hysteria2 authentication is invalid",
            Self::InvalidHost => "Hysteria2 server has an invalid format",
            Self::InvalidPortList => "Hysteria2 port list has an invalid format",
            Self::InvalidPortRange => "Hysteria2 port list has an invalid range",
            Self::OverlappingPorts => "Hysteria2 port list contains overlapping ranges",
            Self::InvalidQuery => "Hysteria2 query is invalid",
            Self::DuplicateField => "Hysteria2 link contains duplicate fields",
            Self::LocalBandwidth => "Hysteria2 bandwidth is a local setting",
            Self::UnsupportedField => "Hysteria2 link contains unsupported fields",
            Self::UnsupportedObfuscation => "Hysteria2 obfuscation is unsupported",
            Self::MissingObfuscationPassword => "Hysteria2 obfuscation requires a password",
            Self::MissingObfuscationType => "Hysteria2 obfuscation password requires a type",
            Self::InvalidText => "Hysteria2 text field has an invalid format",
            Self::InvalidInsecure => "Hysteria2 insecure option is invalid",
            Self::InvalidFingerprint => "Hysteria2 certificate fingerprint is invalid",
            Self::InvalidEch => "Hysteria2 ECH config is invalid",
        })
    }
}

impl std::error::Error for Hysteria2Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hysteria2Facts {
    pub host_kind: HostKind,
    pub port: u16,
    pub port_hopping: bool,
    pub obfuscation: bool,
    pub allow_insecure: bool,
    pub fingerprint_present: bool,
    pub ech_present: bool,
}

pub struct Hysteria2Profile {
    authentication: String,
    server: String,
    host_kind: HostKind,
    port: u16,
    ports: String,
    port_hopping: bool,
    obfuscation: String,
    obfuscation_password: String,
    server_name: String,
    allow_insecure: bool,
    fingerprint: String,
    ech: String,
    suggested_name: String,
}

impl fmt::Debug for Hysteria2Profile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Hysteria2Profile")
            .field("host_kind", &self.host_kind)
            .field("port", &self.port)
            .field("port_hopping", &self.port_hopping)
            .field("obfuscation_present", &!self.obfuscation.is_empty())
            .field("allow_insecure", &self.allow_insecure)
            .field("fingerprint_present", &!self.fingerprint.is_empty())
            .field("ech_present", &!self.ech.is_empty())
            .finish_non_exhaustive()
    }
}

fn parse_ports(value: &str) -> Result<(String, u16, bool), Hysteria2Error> {
    if value.is_empty() || value.len() > 2048 || !value.is_ascii() {
        return Err(Hysteria2Error::InvalidPortList);
    }
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 256 || parts.iter().any(|part| part.is_empty()) {
        return Err(Hysteria2Error::InvalidPortList);
    }
    let mut ranges = Vec::new();
    let mut canonical = Vec::new();
    for part in parts {
        let pieces = part.split('-').collect::<Vec<_>>();
        if pieces.len() > 2
            || pieces.iter().any(|piece| {
                piece.is_empty()
                    || piece.len() > 5
                    || !piece.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(Hysteria2Error::InvalidPortList);
        }
        let start = pieces[0]
            .parse::<u16>()
            .map_err(|_| Hysteria2Error::InvalidPortRange)?;
        let end = if pieces.len() == 2 {
            pieces[1]
                .parse::<u16>()
                .map_err(|_| Hysteria2Error::InvalidPortRange)?
        } else {
            start
        };
        if start == 0 || end == 0 || start > end {
            return Err(Hysteria2Error::InvalidPortRange);
        }
        if ranges
            .iter()
            .any(|(previous_start, previous_end)| start <= *previous_end && end >= *previous_start)
        {
            return Err(Hysteria2Error::OverlappingPorts);
        }
        ranges.push((start, end));
        canonical.push(if start == end {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
    }
    let hopping = ranges.len() > 1 || ranges[0].0 != ranges[0].1;
    Ok((canonical.join(","), ranges[0].0, hopping))
}

fn authority(value: &str) -> Result<(String, String, HostKind, String, u16, bool), Hysteria2Error> {
    if value.is_empty() || value.matches('@').count() > 1 {
        return Err(Hysteria2Error::InvalidAuthority);
    }
    let (encoded_auth, endpoint) = value.rsplit_once('@').unwrap_or(("", value));
    let authentication = percent_decode_strict(encoded_auth, false)
        .map_err(|_| Hysteria2Error::InvalidAuthentication)?;
    if !text_is_bounded(&authentication, 1024, true) {
        return Err(Hysteria2Error::InvalidAuthentication);
    }
    let (host_text, port_text) = if let Some(bracketed) = endpoint.strip_prefix('[') {
        let close = bracketed
            .find(']')
            .ok_or(Hysteria2Error::InvalidAuthority)?;
        let host = &bracketed[..close];
        Ipv6Addr::from_str(host).map_err(|_| Hysteria2Error::InvalidAuthority)?;
        let suffix = &bracketed[close + 1..];
        let port = if suffix.is_empty() {
            "443"
        } else {
            suffix
                .strip_prefix(':')
                .ok_or(Hysteria2Error::InvalidAuthority)?
        };
        (host, port)
    } else if endpoint.matches(':').count() > 1 {
        return Err(Hysteria2Error::InvalidAuthority);
    } else {
        endpoint.rsplit_once(':').unwrap_or((endpoint, "443"))
    };
    let (server, host_kind) = canonical_host(host_text).map_err(|_| Hysteria2Error::InvalidHost)?;
    let (ports, port, hopping) = parse_ports(port_text)?;
    Ok((authentication, server, host_kind, ports, port, hopping))
}

pub fn parse_hysteria2(input: &str) -> Result<Hysteria2Profile, Hysteria2Error> {
    let uri = extract_uri(input, &["hysteria2", "hy2"]).map_err(|_| {
        if input.split_whitespace().any(|token| {
            token
                .get(..6)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("hy2://"))
                || token
                    .get(..12)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("hysteria2://"))
        }) {
            Hysteria2Error::InvalidInput
        } else {
            Hysteria2Error::MissingLink
        }
    })?;
    let parts = split_uri(uri).map_err(|_| Hysteria2Error::InvalidLink)?;
    if !parts.scheme.eq_ignore_ascii_case("hysteria2") && !parts.scheme.eq_ignore_ascii_case("hy2")
    {
        return Err(Hysteria2Error::InvalidLink);
    }
    if !matches!(parts.path, "" | "/") {
        return Err(Hysteria2Error::UnsupportedPath);
    }
    let (authentication, server, host_kind, ports, port, port_hopping) =
        authority(parts.authority)?;
    let pairs = query_pairs(parts.query, true).map_err(|_| Hysteria2Error::InvalidQuery)?;
    let mut query = BTreeMap::new();
    for (key, value) in pairs {
        if query.insert(key.to_lowercase(), value).is_some() {
            return Err(Hysteria2Error::DuplicateField);
        }
    }
    const BANDWIDTH: &[&str] = &["up", "down", "upmbps", "downmbps", "bandwidth"];
    if query.keys().any(|key| BANDWIDTH.contains(&key.as_str())) {
        return Err(Hysteria2Error::LocalBandwidth);
    }
    const ALLOWED: &[&str] = &[
        "obfs",
        "obfs-password",
        "sni",
        "insecure",
        "pinsha256",
        "ech",
    ];
    if query.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(Hysteria2Error::UnsupportedField);
    }
    let obfuscation = query.get("obfs").map_or("", String::as_str).to_lowercase();
    let obfuscation_password = query.get("obfs-password").cloned().unwrap_or_default();
    if !text_is_bounded(&obfuscation_password, 1024, true) {
        return Err(Hysteria2Error::InvalidText);
    }
    if !matches!(obfuscation.as_str(), "" | "salamander" | "gecko") {
        return Err(Hysteria2Error::UnsupportedObfuscation);
    }
    if !obfuscation.is_empty() && obfuscation_password.is_empty() {
        return Err(Hysteria2Error::MissingObfuscationPassword);
    }
    if obfuscation.is_empty() && !obfuscation_password.is_empty() {
        return Err(Hysteria2Error::MissingObfuscationType);
    }
    let server_name = query.get("sni").cloned().unwrap_or_default();
    if !text_is_bounded(&server_name, 253, true) {
        return Err(Hysteria2Error::InvalidText);
    }
    let allow_insecure = match query.get("insecure").map_or("0", String::as_str) {
        "0" => false,
        "1" => true,
        _ => return Err(Hysteria2Error::InvalidInsecure),
    };
    let fingerprint = query
        .get("pinsha256")
        .map_or_else(String::new, |value| value.replace(':', "").to_lowercase());
    if !fingerprint.is_empty()
        && (fingerprint.len() != 64 || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(Hysteria2Error::InvalidFingerprint);
    }
    let ech = query.get("ech").cloned().unwrap_or_default();
    if !ech.is_empty() {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&ech)
            .map_err(|_| Hysteria2Error::InvalidEch)?;
        if decoded.is_empty() || decoded.len() > 8 * 1024 {
            return Err(Hysteria2Error::InvalidEch);
        }
    }
    Ok(Hysteria2Profile {
        authentication,
        server,
        host_kind,
        port,
        ports,
        port_hopping,
        obfuscation,
        obfuscation_password,
        server_name,
        allow_insecure,
        fingerprint,
        ech,
        suggested_name: sanitize_label(parts.fragment),
    })
}

impl Hysteria2Profile {
    #[must_use]
    pub fn facts(&self) -> Hysteria2Facts {
        Hysteria2Facts {
            host_kind: self.host_kind,
            port: self.port,
            port_hopping: self.port_hopping,
            obfuscation: !self.obfuscation.is_empty(),
            allow_insecure: self.allow_insecure,
            fingerprint_present: !self.fingerprint.is_empty(),
            ech_present: !self.ech.is_empty(),
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        let host = if self.host_kind == HostKind::Ipv6 {
            format!("[{}]", self.server)
        } else {
            self.server.clone()
        };
        let mut pairs = Vec::new();
        for (key, value) in [
            ("obfs", self.obfuscation.as_str()),
            ("obfs-password", self.obfuscation_password.as_str()),
            ("sni", self.server_name.as_str()),
            ("insecure", if self.allow_insecure { "1" } else { "" }),
            ("pinsha256", self.fingerprint.as_str()),
            ("ech", self.ech.as_str()),
        ] {
            if !value.is_empty() {
                pairs.push((key.to_owned(), value.to_owned()));
            }
        }
        let auth = if self.authentication.is_empty() {
            String::new()
        } else {
            format!("{}@", quote(&self.authentication, false))
        };
        let query = encoded_query(pairs);
        let canonical = format!(
            "hysteria2://{auth}{host}:{}{}{}",
            self.ports,
            if query.is_empty() { "" } else { "?" },
            query,
        );
        sha256_hex(canonical.as_bytes())
    }

    fn preview_value(&self) -> Value {
        let mut value = Map::new();
        value.insert("version".to_owned(), Value::Number(Number::from(1)));
        value.insert("protocol".to_owned(), Value::String("hysteria2".to_owned()));
        value.insert(
            "server".to_owned(),
            Value::String(self.server.chars().take(253).collect()),
        );
        value.insert("port".to_owned(), Value::Number(Number::from(self.port)));
        value.insert("transport".to_owned(), Value::String("quic".to_owned()));
        value.insert("security".to_owned(), Value::String("tls".to_owned()));
        value.insert(
            "sni".to_owned(),
            Value::String(self.server_name.chars().take(253).collect()),
        );
        value.insert("flow".to_owned(), Value::String(String::new()));
        value.insert("insecure".to_owned(), Value::Bool(self.allow_insecure));
        value.insert("advancedXhttp".to_owned(), Value::Bool(false));
        value.insert("experimental".to_owned(), Value::Bool(true));
        value.insert(
            "experimentalFeatures".to_owned(),
            Value::Array(vec![Value::String("Hysteria2".to_owned())]),
        );
        value.insert("compatibilityNote".to_owned(), Value::String(String::new()));
        value.insert(
            "credentialHint".to_owned(),
            Value::String("••••".to_owned()),
        );
        value.insert(
            "suggestedName".to_owned(),
            Value::String(self.suggested_name.clone()),
        );
        Value::Object(value)
    }

    fn mihomo_value(&self, name: &str, server_override: Option<&str>) -> Value {
        let mut value = Map::new();
        value.insert("name".to_owned(), Value::String(name.to_owned()));
        value.insert("type".to_owned(), Value::String("hysteria2".to_owned()));
        value.insert(
            "server".to_owned(),
            Value::String(server_override.unwrap_or(&self.server).to_owned()),
        );
        value.insert("port".to_owned(), Value::Number(Number::from(self.port)));
        value.insert(
            "password".to_owned(),
            Value::String(self.authentication.clone()),
        );
        if self.port_hopping {
            value.insert("ports".to_owned(), Value::String(self.ports.clone()));
        }
        if !self.obfuscation.is_empty() {
            value.insert("obfs".to_owned(), Value::String(self.obfuscation.clone()));
            value.insert(
                "obfs-password".to_owned(),
                Value::String(self.obfuscation_password.clone()),
            );
        }
        if !self.server_name.is_empty() {
            value.insert("sni".to_owned(), Value::String(self.server_name.clone()));
        }
        if self.allow_insecure {
            value.insert("skip-cert-verify".to_owned(), Value::Bool(true));
        }
        if !self.fingerprint.is_empty() {
            value.insert(
                "fingerprint".to_owned(),
                Value::String(self.fingerprint.clone()),
            );
        }
        if !self.ech.is_empty() {
            value.insert(
                "ech-opts".to_owned(),
                Value::Object(Map::from_iter([
                    ("enable".to_owned(), Value::Bool(true)),
                    ("config".to_owned(), Value::String(self.ech.clone())),
                ])),
            );
        }
        Value::Object(value)
    }

    #[must_use]
    pub fn preview_fingerprint(&self) -> String {
        sha256_hex(canonical_json(&self.preview_value()).as_bytes())
    }

    #[must_use]
    pub fn mihomo_render_fingerprint(&self, name: &str, server_override: Option<&str>) -> String {
        sha256_hex(canonical_json(&self.mihomo_value(name, server_override)).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_port_hopping_and_keeps_debug_private() {
        let profile = parse_hysteria2("hy2://user%3Asecret@[2001:db8::1]:443,5000-5002/?obfs=gecko&obfs-password=private&sni=cdn.example.invalid&ech=AQIDBA%3D%3D#Private").expect("profile");
        assert!(profile.facts().port_hopping);
        let debug = format!("{profile:?}");
        for marker in ["secret", "private", "example.invalid", "AQID"] {
            assert!(!debug.contains(marker));
        }
    }

    #[test]
    fn rejects_overlapping_ports_safely() {
        let error = parse_hysteria2("hy2://secret@example.invalid:443,443").expect_err("overlap");
        assert_eq!(error.code(), "overlapping_ports");
        assert!(!error.to_string().contains("secret"));
    }
}
