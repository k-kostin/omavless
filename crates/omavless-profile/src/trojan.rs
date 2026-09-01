// SPDX-License-Identifier: MIT

//! Canonical bounded Trojan profile adapter for R2.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Number, Value};

use crate::profile_uri::{
    encoded_query, extract_uri, parse_endpoint, percent_decode, query_pairs, quote, sanitize_label,
    split_uri, text_is_bounded,
};
use crate::vless::HostKind;
use crate::vless_canonical::{canonical_json, sha256_hex};
use crate::vless_query::{valid_reality_public_key, valid_reality_short_id};

const SPIDER_NOTE: &str = "Mihomo does not use the Xray-only REALITY spider path (spx); the profile will be imported without it.";
const PQ_NOTE: &str = "REALITY post-quantum key exchange is experimental in Mihomo and depends on the selected client fingerprint; verify this profile on the device.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrojanError {
    InvalidInput,
    MissingLink,
    InvalidLink,
    InvalidAuthority,
    InvalidPassword,
    MissingServerPort,
    InvalidQuery,
    DuplicateField,
    UnsupportedField,
    AliasConflict,
    UnsupportedTransport,
    UnsupportedSecurity,
    InvalidText,
    InvalidAlpn,
    UnsupportedFingerprint,
    InvalidBoolean,
    UnsupportedEncryption,
    UnsupportedFlow,
    UnsupportedHeader,
    TransportFieldConflict,
    RealityWebSocket,
    RealityRequired,
    RealityFieldsRequireReality,
    RealityPublicKey,
    RealityShortId,
    RealityMldsa,
}

impl TrojanError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::MissingLink => "missing_trojan_link",
            Self::InvalidLink => "invalid_link",
            Self::InvalidAuthority => "invalid_authority",
            Self::InvalidPassword => "invalid_password",
            Self::MissingServerPort => "missing_server_port",
            Self::InvalidQuery => "invalid_query",
            Self::DuplicateField => "duplicate_field",
            Self::UnsupportedField => "unsupported_field",
            Self::AliasConflict => "alias_conflict",
            Self::UnsupportedTransport => "unsupported_transport",
            Self::UnsupportedSecurity => "unsupported_security",
            Self::InvalidText => "invalid_text",
            Self::InvalidAlpn => "invalid_alpn",
            Self::UnsupportedFingerprint => "unsupported_fingerprint",
            Self::InvalidBoolean => "invalid_boolean",
            Self::UnsupportedEncryption => "unsupported_encryption",
            Self::UnsupportedFlow => "unsupported_flow",
            Self::UnsupportedHeader => "unsupported_header",
            Self::TransportFieldConflict => "transport_field_conflict",
            Self::RealityWebSocket => "reality_websocket",
            Self::RealityRequired => "reality_required",
            Self::RealityFieldsRequireReality => "reality_fields_require_reality",
            Self::RealityPublicKey => "reality_public_key",
            Self::RealityShortId => "reality_short_id",
            Self::RealityMldsa => "reality_mldsa",
        }
    }
}

impl fmt::Display for TrojanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "Trojan input is invalid",
            Self::MissingLink => "Input does not contain a Trojan link",
            Self::InvalidLink => "Trojan link is invalid",
            Self::InvalidAuthority => "Trojan link has an invalid authority",
            Self::InvalidPassword => "Trojan password has an invalid format",
            Self::MissingServerPort => "Trojan server and port are required",
            Self::InvalidQuery => "Trojan query is invalid",
            Self::DuplicateField => "Trojan link contains duplicate fields",
            Self::UnsupportedField => "Trojan link contains unsupported fields",
            Self::AliasConflict => "Trojan link contains conflicting field aliases",
            Self::UnsupportedTransport => "Trojan transport is not supported",
            Self::UnsupportedSecurity => "Trojan security is not supported",
            Self::InvalidText => "Trojan text field has an invalid format",
            Self::InvalidAlpn => "Trojan ALPN has an invalid format",
            Self::UnsupportedFingerprint => "Trojan client fingerprint is not supported",
            Self::InvalidBoolean => "Trojan boolean option is invalid",
            Self::UnsupportedEncryption => "Trojan encryption metadata is not supported",
            Self::UnsupportedFlow => "Trojan flow metadata is not supported",
            Self::UnsupportedHeader => "Trojan TCP header metadata is not supported",
            Self::TransportFieldConflict => "Trojan transport fields conflict",
            Self::RealityWebSocket => "Trojan Reality is not supported with WebSocket",
            Self::RealityRequired => "Trojan Reality requires a public key and SNI",
            Self::RealityFieldsRequireReality => "Trojan Reality fields require Reality security",
            Self::RealityPublicKey => "Trojan Reality public key is invalid",
            Self::RealityShortId => "Trojan Reality short ID is invalid",
            Self::RealityMldsa => "Trojan Reality ML-DSA is unsupported",
        })
    }
}

impl std::error::Error for TrojanError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrojanTransport {
    Tcp,
    WebSocket,
    Grpc,
}

impl TrojanTransport {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::WebSocket => "ws",
            Self::Grpc => "grpc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrojanSecurity {
    Tls,
    Reality,
}

impl TrojanSecurity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::Reality => "reality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrojanFacts {
    pub host_kind: HostKind,
    pub port: u16,
    pub transport: TrojanTransport,
    pub security: TrojanSecurity,
    pub allow_insecure: bool,
    pub udp: bool,
    pub alpn_count: usize,
    pub fingerprint_present: bool,
    pub reality_pq: bool,
    pub compatibility_note_present: bool,
}

pub struct TrojanProfile {
    password: String,
    server: String,
    port: u16,
    host_kind: HostKind,
    transport: TrojanTransport,
    security: TrojanSecurity,
    server_name: String,
    alpn: Vec<String>,
    fingerprint: String,
    allow_insecure: bool,
    udp: bool,
    host: String,
    path: String,
    service_name: String,
    public_key: String,
    short_id: String,
    reality_pq: bool,
    compatibility_note: String,
    suggested_name: String,
    identity_pairs: Vec<(String, String)>,
}

impl fmt::Debug for TrojanProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TrojanProfile")
            .field("host_kind", &self.host_kind)
            .field("port", &self.port)
            .field("transport", &self.transport)
            .field("security", &self.security)
            .field("allow_insecure", &self.allow_insecure)
            .field("udp", &self.udp)
            .field("alpn_count", &self.alpn.len())
            .field("reality_pq", &self.reality_pq)
            .finish_non_exhaustive()
    }
}

fn alias(
    query: &BTreeMap<String, String>,
    names: &[&str],
    default: &str,
) -> Result<String, TrojanError> {
    let values = names
        .iter()
        .filter_map(|name| query.get(&name.to_lowercase()))
        .collect::<Vec<_>>();
    if values.windows(2).any(|items| items[0] != items[1]) {
        return Err(TrojanError::AliasConflict);
    }
    Ok(values
        .first()
        .map_or(default, |value| value.as_str())
        .to_owned())
}

fn boolean(
    query: &BTreeMap<String, String>,
    names: &[&str],
    default: bool,
) -> Result<bool, TrojanError> {
    let value = alias(query, names, "")?;
    if value.is_empty() {
        return Ok(default);
    }
    match value.to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(TrojanError::InvalidBoolean),
    }
}

fn alpn(value: &str) -> Result<Vec<String>, TrojanError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for value in value.split(',').map(str::trim) {
        if value.is_empty()
            || value.len() > 32
            || values.len() >= 8
            || values.iter().any(|existing| existing == value)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        {
            return Err(TrojanError::InvalidAlpn);
        }
        values.push(value.to_owned());
    }
    Ok(values)
}

pub fn parse_trojan(input: &str) -> Result<TrojanProfile, TrojanError> {
    let uri = extract_uri(input, &["trojan"]).map_err(|_| {
        if input.split_whitespace().any(|token| {
            token
                .get(..9)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("trojan://"))
        }) {
            TrojanError::InvalidInput
        } else {
            TrojanError::MissingLink
        }
    })?;
    let parts = split_uri(uri).map_err(|_| TrojanError::InvalidLink)?;
    if !parts.scheme.eq_ignore_ascii_case("trojan") {
        return Err(TrojanError::InvalidLink);
    }
    if !matches!(parts.path, "" | "/") {
        return Err(TrojanError::InvalidAuthority);
    }
    let (encoded_password, endpoint) = parts
        .authority
        .rsplit_once('@')
        .ok_or(TrojanError::InvalidPassword)?;
    if encoded_password.contains(':') || endpoint.contains('@') {
        return Err(TrojanError::InvalidAuthority);
    }
    let password = percent_decode(encoded_password);
    if !text_is_bounded(&password, 1024, false) {
        return Err(TrojanError::InvalidPassword);
    }
    let (server, port, host_kind) =
        parse_endpoint(endpoint, None).map_err(|_| TrojanError::MissingServerPort)?;
    let identity_pairs = query_pairs(parts.query, false).map_err(|_| TrojanError::InvalidQuery)?;
    let mut query = BTreeMap::new();
    for (key, value) in &identity_pairs {
        if query.insert(key.to_lowercase(), value.clone()).is_some() {
            return Err(TrojanError::DuplicateField);
        }
    }
    const ALLOWED: &[&str] = &[
        "type",
        "network",
        "security",
        "sni",
        "servername",
        "peer",
        "alpn",
        "fp",
        "fingerprint",
        "client-fingerprint",
        "allowinsecure",
        "skip-cert-verify",
        "udp",
        "host",
        "path",
        "servicename",
        "service-name",
        "mode",
        "headertype",
        "header-type",
        "encryption",
        "flow",
        "pbk",
        "publickey",
        "public-key",
        "sid",
        "short-id",
        "spx",
        "spider-x",
        "mldsa65verify",
        "mldsa65-verify",
        "supportx25519mlkem768",
        "support-x25519mlkem768",
    ];
    if query.keys().any(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(TrojanError::UnsupportedField);
    }
    let transport = match alias(&query, &["type", "network"], "tcp")?
        .to_lowercase()
        .as_str()
    {
        "" | "tcp" | "raw" => TrojanTransport::Tcp,
        "ws" => TrojanTransport::WebSocket,
        "grpc" => TrojanTransport::Grpc,
        _ => return Err(TrojanError::UnsupportedTransport),
    };
    let security = match alias(&query, &["security"], "tls")?.to_lowercase().as_str() {
        "" | "tls" => TrojanSecurity::Tls,
        "reality" => TrojanSecurity::Reality,
        _ => return Err(TrojanError::UnsupportedSecurity),
    };
    let server_name = alias(&query, &["sni", "servername", "peer"], "")?;
    let host = alias(&query, &["host"], "")?;
    let mut path = alias(&query, &["path"], "")?;
    let service_name = alias(&query, &["serviceName", "service-name"], "")?;
    for (value, limit) in [
        (&server_name, 253),
        (&host, 253),
        (&path, 2048),
        (&service_name, 1024),
    ] {
        if !text_is_bounded(value, limit, true) {
            return Err(TrojanError::InvalidText);
        }
    }
    let alpn = alpn(&alias(&query, &["alpn"], "")?)?;
    let fingerprint = match alias(&query, &["fp", "fingerprint", "client-fingerprint"], "")?
        .to_lowercase()
        .as_str()
    {
        "" => String::new(),
        "chrome" | "firefox" | "safari" | "android" | "edge" | "360" | "qq" | "random" => {
            alias(&query, &["fp", "fingerprint", "client-fingerprint"], "")?.to_lowercase()
        }
        "ios" => "iOS".to_owned(),
        _ => return Err(TrojanError::UnsupportedFingerprint),
    };
    let allow_insecure = boolean(&query, &["allowInsecure", "skip-cert-verify"], false)?;
    let udp = boolean(&query, &["udp"], true)?;
    if !matches!(alias(&query, &["encryption"], "")?.as_str(), "" | "none") {
        return Err(TrojanError::UnsupportedEncryption);
    }
    if !alias(&query, &["flow"], "")?.is_empty() {
        return Err(TrojanError::UnsupportedFlow);
    }
    if !matches!(
        alias(&query, &["headerType", "header-type"], "")?
            .to_lowercase()
            .as_str(),
        "" | "none"
    ) {
        return Err(TrojanError::UnsupportedHeader);
    }
    let mode = alias(&query, &["mode"], "")?.to_lowercase();
    match transport {
        TrojanTransport::WebSocket => {
            if !service_name.is_empty() || !mode.is_empty() {
                return Err(TrojanError::TransportFieldConflict);
            }
            if path.is_empty() {
                path = "/".to_owned();
            }
        }
        TrojanTransport::Grpc => {
            if !host.is_empty() || !path.is_empty() || !matches!(mode.as_str(), "" | "gun") {
                return Err(TrojanError::TransportFieldConflict);
            }
        }
        TrojanTransport::Tcp => {
            if !host.is_empty() || !path.is_empty() || !service_name.is_empty() || !mode.is_empty()
            {
                return Err(TrojanError::TransportFieldConflict);
            }
        }
    }
    let public_key = alias(&query, &["pbk", "publicKey", "public-key"], "")?;
    let short_id = alias(&query, &["sid", "short-id"], "")?;
    let spider = alias(&query, &["spx", "spider-x"], "")?;
    let mldsa = alias(&query, &["mldsa65Verify", "mldsa65-verify"], "")?;
    let pq_present =
        query.contains_key("supportx25519mlkem768") || query.contains_key("support-x25519mlkem768");
    let reality_pq = boolean(
        &query,
        &["supportX25519MLKEM768", "support-x25519mlkem768"],
        false,
    )?;
    let reality_metadata = !public_key.is_empty()
        || !short_id.is_empty()
        || !spider.is_empty()
        || !mldsa.is_empty()
        || pq_present;
    if security == TrojanSecurity::Reality {
        if transport == TrojanTransport::WebSocket {
            return Err(TrojanError::RealityWebSocket);
        }
        if public_key.is_empty() || server_name.is_empty() {
            return Err(TrojanError::RealityRequired);
        }
        if !mldsa.is_empty() {
            return Err(TrojanError::RealityMldsa);
        }
        if !valid_reality_public_key(&public_key) {
            return Err(TrojanError::RealityPublicKey);
        }
        if !valid_reality_short_id(&short_id) {
            return Err(TrojanError::RealityShortId);
        }
    } else if reality_metadata {
        return Err(TrojanError::RealityFieldsRequireReality);
    }
    let compatibility_note = match (!spider.is_empty(), reality_pq) {
        (true, true) => format!("{SPIDER_NOTE} {PQ_NOTE}"),
        (true, false) => SPIDER_NOTE.to_owned(),
        (false, true) => PQ_NOTE.to_owned(),
        (false, false) => String::new(),
    };
    Ok(TrojanProfile {
        password,
        server,
        port,
        host_kind,
        transport,
        security,
        server_name,
        alpn,
        fingerprint,
        allow_insecure,
        udp,
        host,
        path,
        service_name,
        public_key,
        short_id,
        reality_pq,
        compatibility_note,
        suggested_name: sanitize_label(parts.fragment),
        identity_pairs,
    })
}

impl TrojanProfile {
    #[must_use]
    pub fn facts(&self) -> TrojanFacts {
        TrojanFacts {
            host_kind: self.host_kind,
            port: self.port,
            transport: self.transport,
            security: self.security,
            allow_insecure: self.allow_insecure,
            udp: self.udp,
            alpn_count: self.alpn.len(),
            fingerprint_present: !self.fingerprint.is_empty(),
            reality_pq: self.reality_pq,
            compatibility_note_present: !self.compatibility_note.is_empty(),
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        let host = if self.host_kind == HostKind::Ipv6 {
            format!("[{}]", self.server)
        } else {
            self.server.to_lowercase()
        };
        let query = encoded_query(self.identity_pairs.clone());
        let canonical = format!(
            "trojan://{}@{}:{}{}{}",
            quote(&self.password, false),
            host,
            self.port,
            if query.is_empty() { "" } else { "?" },
            query,
        );
        sha256_hex(canonical.as_bytes())
    }

    fn preview_value(&self) -> Value {
        let mut value = Map::new();
        value.insert("version".to_owned(), Value::Number(Number::from(1)));
        value.insert("protocol".to_owned(), Value::String("trojan".to_owned()));
        value.insert(
            "server".to_owned(),
            Value::String(self.server.chars().take(253).collect()),
        );
        value.insert("port".to_owned(), Value::Number(Number::from(self.port)));
        value.insert(
            "transport".to_owned(),
            Value::String(self.transport.as_str().to_owned()),
        );
        value.insert(
            "security".to_owned(),
            Value::String(self.security.as_str().to_owned()),
        );
        value.insert(
            "sni".to_owned(),
            Value::String(self.server_name.chars().take(253).collect()),
        );
        value.insert("flow".to_owned(), Value::String(String::new()));
        value.insert("insecure".to_owned(), Value::Bool(self.allow_insecure));
        value.insert("advancedXhttp".to_owned(), Value::Bool(false));
        value.insert("experimental".to_owned(), Value::Bool(true));
        let mut features = vec![Value::String("Trojan".to_owned())];
        if self.reality_pq {
            features.push(Value::String("REALITY PQ".to_owned()));
        }
        value.insert("experimentalFeatures".to_owned(), Value::Array(features));
        value.insert(
            "compatibilityNote".to_owned(),
            Value::String(self.compatibility_note.clone()),
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
        value.insert("type".to_owned(), Value::String("trojan".to_owned()));
        value.insert(
            "server".to_owned(),
            Value::String(server_override.unwrap_or(&self.server).to_owned()),
        );
        value.insert("port".to_owned(), Value::Number(Number::from(self.port)));
        value.insert("password".to_owned(), Value::String(self.password.clone()));
        value.insert("udp".to_owned(), Value::Bool(self.udp));
        value.insert(
            "network".to_owned(),
            Value::String(self.transport.as_str().to_owned()),
        );
        if !self.server_name.is_empty() {
            value.insert("sni".to_owned(), Value::String(self.server_name.clone()));
        }
        if !self.alpn.is_empty() {
            value.insert(
                "alpn".to_owned(),
                Value::Array(self.alpn.iter().cloned().map(Value::String).collect()),
            );
        }
        if !self.fingerprint.is_empty() {
            value.insert(
                "client-fingerprint".to_owned(),
                Value::String(self.fingerprint.clone()),
            );
        }
        if self.allow_insecure {
            value.insert("skip-cert-verify".to_owned(), Value::Bool(true));
        }
        if self.security == TrojanSecurity::Reality {
            let mut reality = Map::new();
            reality.insert(
                "public-key".to_owned(),
                Value::String(self.public_key.clone()),
            );
            if !self.short_id.is_empty() {
                reality.insert("short-id".to_owned(), Value::String(self.short_id.clone()));
            }
            if self.reality_pq {
                reality.insert("support-x25519mlkem768".to_owned(), Value::Bool(true));
            }
            value.insert("reality-opts".to_owned(), Value::Object(reality));
        }
        match self.transport {
            TrojanTransport::WebSocket => {
                let mut options = Map::new();
                options.insert("path".to_owned(), Value::String(self.path.clone()));
                if !self.host.is_empty() {
                    options.insert(
                        "headers".to_owned(),
                        Value::Object(Map::from_iter([(
                            "Host".to_owned(),
                            Value::String(self.host.clone()),
                        )])),
                    );
                }
                value.insert("ws-opts".to_owned(), Value::Object(options));
            }
            TrojanTransport::Grpc => {
                value.insert(
                    "grpc-opts".to_owned(),
                    Value::Object(Map::from_iter([(
                        "grpc-service-name".to_owned(),
                        Value::String(self.service_name.clone()),
                    )])),
                );
            }
            TrojanTransport::Tcp => {}
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
    fn parses_tls_ws_and_reality_without_debug_leaks() {
        let tls = parse_trojan("trojan://s3cr%3At%40value@example.invalid:443?type=ws&security=tls&sni=cdn.example.invalid&host=edge.example.invalid&path=%2Fsocket&alpn=h2%2Chttp%2F1.1#Private").expect("TLS");
        assert_eq!(tls.facts().transport, TrojanTransport::WebSocket);
        assert_eq!(tls.facts().alpn_count, 2);
        let debug = format!("{tls:?}");
        for secret in ["s3cr", "example.invalid", "/socket", "Private"] {
            assert!(!debug.contains(secret));
        }

        let reality = parse_trojan("trojan://secret@example.invalid:443?type=grpc&security=reality&sni=reality.example.invalid&serviceName=edge&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=0123456789abcdef").expect("Reality");
        assert_eq!(reality.facts().security, TrojanSecurity::Reality);
    }

    #[test]
    fn errors_never_echo_private_input() {
        let error =
            parse_trojan("trojan://private-secret@example.invalid:443?type=private-transport")
                .expect_err("invalid transport");
        assert_eq!(error.code(), "unsupported_transport");
        assert!(!error.to_string().contains("private"));
    }
}
