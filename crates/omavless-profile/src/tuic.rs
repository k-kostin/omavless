// SPDX-License-Identifier: MIT

//! Canonical bounded TUIC v5 profile adapter for R2.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Number, Value};

use crate::profile_uri::{
    canonical_host, canonical_uuid, encoded_query, extract_uri, parse_endpoint,
    percent_decode_strict, query_pairs, quote, sanitize_label, split_uri, text_is_bounded,
};
use crate::vless::HostKind;
use crate::vless_canonical::{canonical_json, sha256_hex};

const DISABLE_SNI_NOTE: &str = "Disabling SNI also disables certificate verification in Mihomo; use it only when the server explicitly requires an empty SNI.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuicError {
    InvalidInput,
    MissingLink,
    InvalidLink,
    InvalidAuthority,
    InvalidCredential,
    MissingCredential,
    InvalidUuid,
    InvalidPassword,
    MissingServerPort,
    InvalidHost,
    InvalidQuery,
    DuplicateField,
    UnsupportedField,
    UnsupportedCongestion,
    UnsupportedUdpRelay,
    InvalidAlpn,
    InvalidText,
    InvalidBoolean,
    SniConflict,
}

impl TuicError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::MissingLink => "missing_tuic_link",
            Self::InvalidLink => "invalid_link",
            Self::InvalidAuthority => "invalid_authority",
            Self::InvalidCredential => "invalid_credential",
            Self::MissingCredential => "missing_credential",
            Self::InvalidUuid => "invalid_uuid",
            Self::InvalidPassword => "invalid_password",
            Self::MissingServerPort => "missing_server_port",
            Self::InvalidHost => "invalid_host",
            Self::InvalidQuery => "invalid_query",
            Self::DuplicateField => "duplicate_field",
            Self::UnsupportedField => "unsupported_field",
            Self::UnsupportedCongestion => "unsupported_congestion",
            Self::UnsupportedUdpRelay => "unsupported_udp_relay",
            Self::InvalidAlpn => "invalid_alpn",
            Self::InvalidText => "invalid_text",
            Self::InvalidBoolean => "invalid_boolean",
            Self::SniConflict => "sni_conflict",
        }
    }
}

impl fmt::Display for TuicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "TUIC input is invalid",
            Self::MissingLink => "Input does not contain a TUIC link",
            Self::InvalidLink => "TUIC link is invalid",
            Self::InvalidAuthority => "TUIC link has an invalid authority",
            Self::InvalidCredential => "TUIC credential is invalid",
            Self::MissingCredential => "TUIC v5 requires a UUID and password",
            Self::InvalidUuid => "TUIC user id is not a valid UUID",
            Self::InvalidPassword => "TUIC password has an invalid format",
            Self::MissingServerPort => "TUIC server and port are required",
            Self::InvalidHost => "TUIC server has an invalid format",
            Self::InvalidQuery => "TUIC query is invalid",
            Self::DuplicateField => "TUIC link contains duplicate fields",
            Self::UnsupportedField => "TUIC link contains unsupported fields",
            Self::UnsupportedCongestion => "TUIC congestion controller is unsupported",
            Self::UnsupportedUdpRelay => "TUIC UDP relay mode is unsupported",
            Self::InvalidAlpn => "TUIC ALPN has an invalid format",
            Self::InvalidText => "TUIC text field has an invalid format",
            Self::InvalidBoolean => "TUIC boolean option is invalid",
            Self::SniConflict => "TUIC cannot set both SNI and disable SNI",
        })
    }
}

impl std::error::Error for TuicError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuicCongestion {
    Cubic,
    NewReno,
    Bbr,
}

impl TuicCongestion {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cubic => "cubic",
            Self::NewReno => "new_reno",
            Self::Bbr => "bbr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuicUdpRelay {
    Native,
    Quic,
}

impl TuicUdpRelay {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Quic => "quic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuicFacts {
    pub host_kind: HostKind,
    pub port: u16,
    pub congestion: TuicCongestion,
    pub udp_relay: TuicUdpRelay,
    pub alpn_count: usize,
    pub allow_insecure: bool,
    pub disable_sni: bool,
    pub compatibility_note_present: bool,
}

pub struct TuicProfile {
    user_id: String,
    password: String,
    server: String,
    host_kind: HostKind,
    port: u16,
    server_name: String,
    alpn: Vec<String>,
    allow_insecure: bool,
    disable_sni: bool,
    udp_relay: TuicUdpRelay,
    congestion: TuicCongestion,
    suggested_name: String,
}

impl fmt::Debug for TuicProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TuicProfile")
            .field("host_kind", &self.host_kind)
            .field("port", &self.port)
            .field("congestion", &self.congestion)
            .field("udp_relay", &self.udp_relay)
            .field("alpn_count", &self.alpn.len())
            .field("allow_insecure", &self.allow_insecure)
            .field("disable_sni", &self.disable_sni)
            .finish_non_exhaustive()
    }
}

fn boolean(query: &BTreeMap<String, String>, name: &str) -> Result<bool, TuicError> {
    match query.get(name).map_or("0", String::as_str) {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(TuicError::InvalidBoolean),
    }
}

fn alpn(value: &str) -> Result<Vec<String>, TuicError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for value in value.split(',').map(str::trim) {
        if value.is_empty()
            || value.len() > 32
            || result.len() >= 8
            || result.iter().any(|item| item == value)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        {
            return Err(TuicError::InvalidAlpn);
        }
        result.push(value.to_owned());
    }
    Ok(result)
}

pub fn parse_tuic(input: &str) -> Result<TuicProfile, TuicError> {
    let uri = extract_uri(input, &["tuic"]).map_err(|_| {
        if input.split_whitespace().any(|token| {
            token
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("tuic://"))
        }) {
            TuicError::InvalidInput
        } else {
            TuicError::MissingLink
        }
    })?;
    let parts = split_uri(uri).map_err(|_| TuicError::InvalidLink)?;
    if !parts.scheme.eq_ignore_ascii_case("tuic") {
        return Err(TuicError::InvalidLink);
    }
    if !matches!(parts.path, "" | "/") || parts.authority.matches('@').count() != 1 {
        return Err(TuicError::InvalidAuthority);
    }
    let (encoded_userinfo, endpoint) = parts
        .authority
        .rsplit_once('@')
        .ok_or(TuicError::InvalidAuthority)?;
    let userinfo =
        percent_decode_strict(encoded_userinfo, false).map_err(|_| TuicError::InvalidCredential)?;
    let (user_id, password) = userinfo
        .split_once(':')
        .ok_or(TuicError::MissingCredential)?;
    let user_id = canonical_uuid(user_id).ok_or(TuicError::InvalidUuid)?;
    if !text_is_bounded(password, 1024, false) {
        return Err(TuicError::InvalidPassword);
    }
    let (raw_server, port, _) =
        parse_endpoint(endpoint, None).map_err(|_| TuicError::MissingServerPort)?;
    let (server, host_kind) = canonical_host(&raw_server).map_err(|_| TuicError::InvalidHost)?;
    let pairs = query_pairs(parts.query, true).map_err(|_| TuicError::InvalidQuery)?;
    let mut query = BTreeMap::new();
    for (key, value) in pairs {
        if query.insert(key.to_lowercase(), value).is_some() {
            return Err(TuicError::DuplicateField);
        }
    }
    const ALLOWED: &[&str] = &[
        "congestion_control",
        "udp_relay_mode",
        "alpn",
        "sni",
        "allow_insecure",
        "disable_sni",
    ];
    if query.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(TuicError::UnsupportedField);
    }
    let congestion = match query
        .get("congestion_control")
        .map_or("cubic", String::as_str)
        .to_lowercase()
        .as_str()
    {
        "" | "cubic" => TuicCongestion::Cubic,
        "new_reno" => TuicCongestion::NewReno,
        "bbr" => TuicCongestion::Bbr,
        _ => return Err(TuicError::UnsupportedCongestion),
    };
    let udp_relay = match query
        .get("udp_relay_mode")
        .map_or("native", String::as_str)
        .to_lowercase()
        .as_str()
    {
        "" | "native" => TuicUdpRelay::Native,
        "quic" => TuicUdpRelay::Quic,
        _ => return Err(TuicError::UnsupportedUdpRelay),
    };
    let alpn = alpn(query.get("alpn").map_or("", String::as_str))?;
    let server_name = query.get("sni").cloned().unwrap_or_default();
    if !text_is_bounded(&server_name, 253, true) {
        return Err(TuicError::InvalidText);
    }
    let allow_insecure = boolean(&query, "allow_insecure")?;
    let disable_sni = boolean(&query, "disable_sni")?;
    if disable_sni && !server_name.is_empty() {
        return Err(TuicError::SniConflict);
    }
    Ok(TuicProfile {
        user_id,
        password: password.to_owned(),
        server,
        host_kind,
        port,
        server_name,
        alpn,
        allow_insecure,
        disable_sni,
        udp_relay,
        congestion,
        suggested_name: sanitize_label(parts.fragment),
    })
}

impl TuicProfile {
    #[must_use]
    pub fn facts(&self) -> TuicFacts {
        TuicFacts {
            host_kind: self.host_kind,
            port: self.port,
            congestion: self.congestion,
            udp_relay: self.udp_relay,
            alpn_count: self.alpn.len(),
            allow_insecure: self.allow_insecure,
            disable_sni: self.disable_sni,
            compatibility_note_present: self.disable_sni,
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        let host = if self.host_kind == HostKind::Ipv6 {
            format!("[{}]", self.server)
        } else {
            self.server.clone()
        };
        let userinfo = quote(&format!("{}:{}", self.user_id, self.password), false);
        let pairs = vec![
            (
                "congestion_control".to_owned(),
                self.congestion.as_str().to_owned(),
            ),
            (
                "udp_relay_mode".to_owned(),
                self.udp_relay.as_str().to_owned(),
            ),
            ("alpn".to_owned(), self.alpn.join(",")),
            ("sni".to_owned(), self.server_name.clone()),
            (
                "allow_insecure".to_owned(),
                if self.allow_insecure { "1" } else { "0" }.to_owned(),
            ),
            (
                "disable_sni".to_owned(),
                if self.disable_sni { "1" } else { "0" }.to_owned(),
            ),
        ];
        sha256_hex(
            format!(
                "tuic://{userinfo}@{host}:{}?{}",
                self.port,
                encoded_query(pairs)
            )
            .as_bytes(),
        )
    }

    fn preview_value(&self) -> Value {
        let mut value = Map::new();
        value.insert("version".to_owned(), Value::Number(Number::from(1)));
        value.insert("protocol".to_owned(), Value::String("tuic".to_owned()));
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
        value.insert(
            "insecure".to_owned(),
            Value::Bool(self.allow_insecure || self.disable_sni),
        );
        value.insert("advancedXhttp".to_owned(), Value::Bool(false));
        value.insert("experimental".to_owned(), Value::Bool(true));
        value.insert(
            "experimentalFeatures".to_owned(),
            Value::Array(vec![Value::String("TUIC v5".to_owned())]),
        );
        value.insert(
            "compatibilityNote".to_owned(),
            Value::String(if self.disable_sni {
                DISABLE_SNI_NOTE.to_owned()
            } else {
                String::new()
            }),
        );
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
        value.insert("type".to_owned(), Value::String("tuic".to_owned()));
        value.insert(
            "server".to_owned(),
            Value::String(server_override.unwrap_or(&self.server).to_owned()),
        );
        value.insert("port".to_owned(), Value::Number(Number::from(self.port)));
        value.insert("uuid".to_owned(), Value::String(self.user_id.clone()));
        value.insert("password".to_owned(), Value::String(self.password.clone()));
        value.insert(
            "udp-relay-mode".to_owned(),
            Value::String(self.udp_relay.as_str().to_owned()),
        );
        value.insert(
            "congestion-controller".to_owned(),
            Value::String(self.congestion.as_str().to_owned()),
        );
        if !self.alpn.is_empty() {
            value.insert(
                "alpn".to_owned(),
                Value::Array(self.alpn.iter().cloned().map(Value::String).collect()),
            );
        }
        if !self.server_name.is_empty() {
            value.insert("sni".to_owned(), Value::String(self.server_name.clone()));
        }
        if self.allow_insecure {
            value.insert("skip-cert-verify".to_owned(), Value::Bool(true));
        }
        if self.disable_sni {
            value.insert("disable-sni".to_owned(), Value::Bool(true));
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

    #[must_use]
    pub fn render_mihomo_proxy(&self, name: &str, server_override: Option<&str>) -> String {
        format!(
            "- {}",
            serde_json::to_string(&self.mihomo_value(name, server_override))
                .expect("validated profile JSON is serializable")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_encoded_credentials_and_keeps_debug_private() {
        let profile = parse_tuic("tuic://33333333-3333-4333-8333-333333333333%3Ap%40ss%3Aword@[2001:db8::2]:443/?alpn=h3#Private").expect("TUIC");
        assert_eq!(profile.facts().host_kind, HostKind::Ipv6);
        let debug = format!("{profile:?}");
        for marker in ["33333333", "p@ss", "2001:db8", "Private"] {
            assert!(!debug.contains(marker));
        }
    }

    #[test]
    fn rejects_v4_and_private_errors_are_safe() {
        let error = parse_tuic("tuic://private-token@example.invalid:443").expect_err("v4");
        assert_eq!(error.code(), "missing_credential");
        assert!(!error.to_string().contains("private"));
    }
}
