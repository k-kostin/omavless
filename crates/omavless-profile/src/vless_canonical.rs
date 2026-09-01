// SPDX-License-Identifier: MIT

//! Canonical pure VLESS profile composition for the incremental R2 migration.
//!
//! This module combines the accepted authority, query, REALITY, Encryption,
//! transport and XHTTP slices into one private model. It remains outside the
//! installed Python runtime; Python is still the production owner and oracle.

use std::fmt;
use std::fmt::Write as _;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use serde_json::{Map, Number, Value};

use crate::vless::{HostKind, VlessAuthorityError, parse_vless_authority};
use crate::vless_query::{
    VlessFlow, VlessPacketEncoding, VlessQueryError, VlessSecurity, VlessTransport, XhttpMode,
    parse_vless_query_metadata,
};
use crate::vless_xhttp_extra::{
    XhttpConfiguration, XhttpConfigurationError, XhttpDownloadMode, XhttpDownloadSecurity,
    XhttpValue, parse_xhttp_configuration,
};

const REALITY_SPX_COMPATIBILITY_NOTE: &str = "Mihomo does not use the Xray-only REALITY spider path (spx); the profile will be imported without it.";
const REALITY_PQ_COMPATIBILITY_NOTE: &str = "REALITY post-quantum key exchange is experimental in Mihomo and depends on the selected client fingerprint; verify this profile on the device.";
const VLESS_PROVIDER_METADATA_COMPATIBILITY_NOTE: &str =
    "Provider-only VLESS metadata is not mapped to Mihomo; connectivity settings remain unchanged.";
const VLESS_TRANSPORT_METADATA_COMPATIBILITY_NOTE: &str =
    "A transport mode outside XHTTP is provider metadata and is not mapped to Mihomo.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessCanonicalError {
    Authority(VlessAuthorityError),
    Query(VlessQueryError),
    Xhttp(XhttpConfigurationError),
    XhttpExtraRequiresTransport,
}

impl VlessCanonicalError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Authority(error) => error.code(),
            Self::Query(error) => error.code(),
            Self::Xhttp(error) => error.code(),
            Self::XhttpExtraRequiresTransport => "xhttp_extra_requires_transport",
        }
    }
}

impl fmt::Display for VlessCanonicalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority(error) => error.fmt(formatter),
            Self::Query(error) => error.fmt(formatter),
            Self::Xhttp(error) => error.fmt(formatter),
            Self::XhttpExtraRequiresTransport => {
                formatter.write_str("VLESS XHTTP extra requires the XHTTP transport")
            }
        }
    }
}

impl std::error::Error for VlessCanonicalError {}

impl From<VlessAuthorityError> for VlessCanonicalError {
    fn from(error: VlessAuthorityError) -> Self {
        Self::Authority(error)
    }
}

impl From<VlessQueryError> for VlessCanonicalError {
    fn from(error: VlessQueryError) -> Self {
        Self::Query(error)
    }
}

impl From<XhttpConfigurationError> for VlessCanonicalError {
    fn from(error: XhttpConfigurationError) -> Self {
        Self::Xhttp(error)
    }
}

/// Current redacted import-preview contract. Endpoint and SNI are deliberately
/// visible to the importing user, while reusable credentials remain absent.
pub struct VlessPreview {
    pub version: u8,
    pub protocol: &'static str,
    pub server: String,
    pub port: u16,
    pub transport: &'static str,
    pub security: &'static str,
    pub sni: String,
    pub flow: String,
    pub insecure: bool,
    pub advanced_xhttp: bool,
    pub experimental: bool,
    pub experimental_features: Vec<String>,
    pub compatibility_note: String,
    pub credential_hint: String,
    pub suggested_name: String,
}

impl fmt::Debug for VlessPreview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VlessPreview")
            .field("version", &self.version)
            .field("protocol", &self.protocol)
            .field("port", &self.port)
            .field("transport", &self.transport)
            .field("security", &self.security)
            .field("insecure", &self.insecure)
            .field("advanced_xhttp", &self.advanced_xhttp)
            .field("experimental", &self.experimental)
            .field(
                "experimental_feature_count",
                &self.experimental_features.len(),
            )
            .field(
                "compatibility_note_present",
                &!self.compatibility_note.is_empty(),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessCanonicalFacts {
    pub host_kind: HostKind,
    pub port: u16,
    pub transport: VlessTransport,
    pub security: VlessSecurity,
    pub flow: Option<VlessFlow>,
    pub packet_encoding: Option<VlessPacketEncoding>,
    pub allow_insecure: bool,
    pub encryption_enabled: bool,
    pub reality_pq: bool,
    pub alpn_count: usize,
    pub advanced_xhttp: bool,
    pub xhttp_field_count: usize,
    pub xhttp_mode: Option<XhttpMode>,
    pub experimental_feature_count: usize,
    pub compatibility_note_present: bool,
    pub compatibility_spider: bool,
    pub compatibility_pq: bool,
    pub compatibility_provider_metadata: bool,
    pub compatibility_transport_metadata: bool,
}

/// Private canonical VLESS model. Reusable connection data is intentionally
/// omitted from `Debug`; callers obtain it only through explicit identity or
/// Mihomo-rendering operations.
pub struct VlessCanonicalProfile {
    user_id: String,
    server: String,
    port: u16,
    host_kind: HostKind,
    suggested_name: String,
    transport: VlessTransport,
    security: VlessSecurity,
    encryption: String,
    flow: Option<VlessFlow>,
    packet_encoding: Option<VlessPacketEncoding>,
    server_name: String,
    fingerprint: String,
    public_key: String,
    short_id: String,
    reality_pq: bool,
    path: String,
    host: String,
    service_name: String,
    mode: String,
    xhttp_mode: Option<XhttpMode>,
    xhttp: Option<XhttpConfiguration>,
    alpn: Vec<String>,
    allow_insecure: bool,
    compatibility_note: String,
    experimental_features: Vec<String>,
}

impl fmt::Debug for VlessCanonicalProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VlessCanonicalProfile")
            .field("port", &self.port)
            .field("host_kind", &self.host_kind)
            .field("transport", &self.transport)
            .field("security", &self.security)
            .field("encryption_enabled", &(self.encryption != "none"))
            .field("flow", &self.flow)
            .field("packet_encoding", &self.packet_encoding)
            .field("reality_pq", &self.reality_pq)
            .field("alpn_count", &self.alpn.len())
            .field("advanced_xhttp", &self.advanced_xhttp())
            .field(
                "experimental_feature_count",
                &self.experimental_features.len(),
            )
            .finish_non_exhaustive()
    }
}

pub fn parse_vless_canonical(input: &str) -> Result<VlessCanonicalProfile, VlessCanonicalError> {
    let authority = parse_vless_authority(input)?;
    let query = parse_vless_query_metadata(input)?;
    let private = query.private_projection();

    let (mode, xhttp) = if query.transport == VlessTransport::Xhttp {
        let mode = query.xhttp_mode.unwrap_or(XhttpMode::Default);
        let configuration = parse_xhttp_configuration(
            &private.raw_xhttp_extra,
            download_mode(mode),
            download_security(query.security),
        )?;
        (
            match mode {
                XhttpMode::Default => String::new(),
                _ => mode.as_str().to_owned(),
            },
            Some(configuration),
        )
    } else {
        if !private.raw_xhttp_extra.is_empty() {
            return Err(VlessCanonicalError::XhttpExtraRequiresTransport);
        }
        (String::new(), None)
    };

    let mut notes = Vec::new();
    if query.security == VlessSecurity::Reality && private.spider_x_present {
        push_unique(&mut notes, REALITY_SPX_COMPATIBILITY_NOTE);
    }
    if query.reality_pq {
        push_unique(&mut notes, REALITY_PQ_COMPATIBILITY_NOTE);
    }
    if let Some(configuration) = &xhttp {
        if configuration.download_reality_spider_compatibility() {
            push_unique(&mut notes, REALITY_SPX_COMPATIBILITY_NOTE);
        }
        if configuration.download_reality_pq_compatibility() {
            push_unique(&mut notes, REALITY_PQ_COMPATIBILITY_NOTE);
        }
    }
    if query.provider_metadata_present {
        push_unique(&mut notes, VLESS_PROVIDER_METADATA_COMPATIBILITY_NOTE);
    }
    if query.non_xhttp_mode_metadata {
        push_unique(&mut notes, VLESS_TRANSPORT_METADATA_COMPATIBILITY_NOTE);
    }

    let mut experimental_features = Vec::new();
    if private.encryption != "none" {
        push_unique(&mut experimental_features, "VLESS Encryption");
    }
    if query.reality_pq {
        push_unique(&mut experimental_features, "REALITY PQ");
    }
    if xhttp
        .as_ref()
        .is_some_and(XhttpConfiguration::download_reality_pq)
    {
        push_unique(&mut experimental_features, "REALITY PQ");
    }

    Ok(VlessCanonicalProfile {
        user_id: authority.user_id,
        server: authority.server,
        port: authority.port,
        host_kind: authority.host_kind,
        suggested_name: authority.suggested_name,
        transport: query.transport,
        security: query.security,
        encryption: private.encryption,
        flow: query.flow,
        packet_encoding: query.packet_encoding,
        server_name: private.server_name,
        fingerprint: private.fingerprint,
        public_key: private.public_key,
        short_id: private.short_id,
        reality_pq: query.reality_pq,
        path: private.path,
        host: private.host,
        service_name: private.service_name,
        mode,
        xhttp_mode: query.xhttp_mode,
        xhttp,
        alpn: private.alpn,
        allow_insecure: query.allow_insecure,
        compatibility_note: notes.join(" "),
        experimental_features,
    })
}

pub fn parse_vless_canonical_bytes(
    input: &[u8],
) -> Result<VlessCanonicalProfile, VlessCanonicalError> {
    let input = std::str::from_utf8(input)
        .map_err(|_| VlessCanonicalError::Authority(VlessAuthorityError::InvalidInput))?;
    parse_vless_canonical(input)
}

impl VlessCanonicalProfile {
    #[must_use]
    pub fn facts(&self) -> VlessCanonicalFacts {
        let xhttp_field_count = self
            .xhttp
            .as_ref()
            .map_or(0, |configuration| configuration.normalized_entries().len());
        VlessCanonicalFacts {
            host_kind: self.host_kind,
            port: self.port,
            transport: self.transport,
            security: self.security,
            flow: self.flow,
            packet_encoding: self.packet_encoding,
            allow_insecure: self.allow_insecure,
            encryption_enabled: self.encryption != "none",
            reality_pq: self.reality_pq,
            alpn_count: self.alpn.len(),
            advanced_xhttp: self.advanced_xhttp(),
            xhttp_field_count,
            xhttp_mode: self.xhttp_mode,
            experimental_feature_count: self.experimental_features.len(),
            compatibility_note_present: !self.compatibility_note.is_empty(),
            compatibility_spider: self
                .compatibility_note
                .contains(REALITY_SPX_COMPATIBILITY_NOTE),
            compatibility_pq: self
                .compatibility_note
                .contains(REALITY_PQ_COMPATIBILITY_NOTE),
            compatibility_provider_metadata: self
                .compatibility_note
                .contains(VLESS_PROVIDER_METADATA_COMPATIBILITY_NOTE),
            compatibility_transport_metadata: self
                .compatibility_note
                .contains(VLESS_TRANSPORT_METADATA_COMPATIBILITY_NOTE),
        }
    }

    #[must_use]
    pub fn preview(&self) -> VlessPreview {
        let suffix = self
            .user_id
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        let suggested_name = self
            .suggested_name
            .chars()
            .filter(|character| !matches!(*character as u32, 0x00..=0x1f | 0x7f))
            .collect::<String>()
            .trim()
            .chars()
            .take(80)
            .collect();
        VlessPreview {
            version: 1,
            protocol: "vless",
            server: self.server.chars().take(253).collect(),
            port: self.port,
            transport: self.transport.as_str(),
            security: self.security.as_str(),
            sni: self.server_name.chars().take(253).collect(),
            flow: self
                .flow
                .map_or("", VlessFlow::as_str)
                .chars()
                .take(64)
                .collect(),
            insecure: self.allow_insecure,
            advanced_xhttp: self.advanced_xhttp(),
            experimental: !self.experimental_features.is_empty(),
            experimental_features: self.experimental_features.clone(),
            compatibility_note: self.compatibility_note.clone(),
            credential_hint: format!("••••{suffix}"),
            suggested_name,
        }
    }

    #[must_use]
    pub fn subscription_identity(&self) -> String {
        sha256_hex(canonical_json(&self.identity_value()).as_bytes())
    }

    #[must_use]
    pub fn preview_fingerprint(&self) -> String {
        sha256_hex(canonical_json(&self.preview_value()).as_bytes())
    }

    #[must_use]
    pub fn mihomo_render_fingerprint(&self, name: &str, server_override: Option<&str>) -> String {
        let semantics = entries_to_json(&self.mihomo_entries(name, server_override));
        sha256_hex(canonical_json(&semantics).as_bytes())
    }

    #[must_use]
    pub fn parity_fingerprint(&self, name: &str, server_override: Option<&str>) -> String {
        let combined = format!(
            "{}\n{}\n{}",
            self.subscription_identity(),
            self.preview_fingerprint(),
            self.mihomo_render_fingerprint(name, server_override),
        );
        sha256_hex(combined.as_bytes())
    }

    #[must_use]
    pub fn render_mihomo_proxy(&self, name: &str, server_override: Option<&str>) -> String {
        let entries = self.mihomo_entries(name, server_override);
        let Some((_, first)) = entries.first() else {
            return String::new();
        };
        let mut lines = vec![format!("- name: {}", yaml_scalar(first))];
        append_yaml_mapping(&mut lines, &entries[1..], 2, false);
        lines.join("\n")
    }

    fn advanced_xhttp(&self) -> bool {
        self.xhttp
            .as_ref()
            .is_some_and(|configuration| !configuration.is_empty())
    }

    fn xhttp_value(&self) -> Value {
        self.xhttp.as_ref().map_or_else(
            || Value::Object(Map::new()),
            |configuration| entries_to_json(&configuration.normalized_entries()),
        )
    }

    fn identity_value(&self) -> Value {
        let mut values = Map::new();
        values.insert("protocol".to_owned(), Value::String("vless".to_owned()));
        values.insert(
            "uuid".to_owned(),
            Value::String(self.user_id.to_lowercase()),
        );
        values.insert(
            "server".to_owned(),
            Value::String(normalize_identity_server(&self.server)),
        );
        values.insert("port".to_owned(), Value::Number(Number::from(self.port)));
        values.insert(
            "network".to_owned(),
            Value::String(self.transport.as_str().to_owned()),
        );
        values.insert(
            "security".to_owned(),
            Value::String(self.security.as_str().to_owned()),
        );
        values.insert(
            "encryption".to_owned(),
            Value::String(self.encryption.clone()),
        );
        values.insert(
            "flow".to_owned(),
            Value::String(self.flow.map_or("", VlessFlow::as_str).to_owned()),
        );
        values.insert(
            "servername".to_owned(),
            Value::String(
                self.server_name
                    .to_lowercase()
                    .trim_end_matches('.')
                    .to_owned(),
            ),
        );
        values.insert(
            "fingerprint".to_owned(),
            Value::String(self.fingerprint.to_lowercase()),
        );
        values.insert(
            "publicKey".to_owned(),
            Value::String(self.public_key.clone()),
        );
        values.insert(
            "shortId".to_owned(),
            Value::String(self.short_id.to_lowercase()),
        );
        values.insert("realityPq".to_owned(), Value::Bool(self.reality_pq));
        values.insert("path".to_owned(), Value::String(self.path.clone()));
        values.insert("host".to_owned(), Value::String(self.host.clone()));
        values.insert(
            "serviceName".to_owned(),
            Value::String(self.service_name.clone()),
        );
        values.insert("mode".to_owned(), Value::String(self.mode.clone()));
        values.insert("xhttpExtra".to_owned(), self.xhttp_value());
        values.insert(
            "alpn".to_owned(),
            Value::Array(self.alpn.iter().cloned().map(Value::String).collect()),
        );
        values.insert("allowInsecure".to_owned(), Value::Bool(self.allow_insecure));
        values.insert(
            "packetEncoding".to_owned(),
            Value::String(
                self.packet_encoding
                    .map_or("", VlessPacketEncoding::as_str)
                    .to_owned(),
            ),
        );
        Value::Object(values)
    }

    fn preview_value(&self) -> Value {
        let preview = self.preview();
        let mut values = Map::new();
        values.insert(
            "version".to_owned(),
            Value::Number(Number::from(preview.version)),
        );
        values.insert(
            "protocol".to_owned(),
            Value::String(preview.protocol.to_owned()),
        );
        values.insert("server".to_owned(), Value::String(preview.server));
        values.insert("port".to_owned(), Value::Number(Number::from(preview.port)));
        values.insert(
            "transport".to_owned(),
            Value::String(preview.transport.to_owned()),
        );
        values.insert(
            "security".to_owned(),
            Value::String(preview.security.to_owned()),
        );
        values.insert("sni".to_owned(), Value::String(preview.sni));
        values.insert("flow".to_owned(), Value::String(preview.flow));
        values.insert("insecure".to_owned(), Value::Bool(preview.insecure));
        values.insert(
            "advancedXhttp".to_owned(),
            Value::Bool(preview.advanced_xhttp),
        );
        values.insert("experimental".to_owned(), Value::Bool(preview.experimental));
        values.insert(
            "experimentalFeatures".to_owned(),
            Value::Array(
                preview
                    .experimental_features
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
        values.insert(
            "compatibilityNote".to_owned(),
            Value::String(preview.compatibility_note),
        );
        values.insert(
            "credentialHint".to_owned(),
            Value::String(preview.credential_hint),
        );
        values.insert(
            "suggestedName".to_owned(),
            Value::String(preview.suggested_name),
        );
        Value::Object(values)
    }

    fn mihomo_entries(
        &self,
        name: &str,
        server_override: Option<&str>,
    ) -> Vec<(String, XhttpValue)> {
        let server = server_override
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.server);
        let mut entries = vec![
            ("name".to_owned(), XhttpValue::String(name.to_owned())),
            ("type".to_owned(), XhttpValue::String("vless".to_owned())),
            ("server".to_owned(), XhttpValue::String(server.to_owned())),
            ("port".to_owned(), XhttpValue::Integer(i64::from(self.port))),
            ("uuid".to_owned(), XhttpValue::String(self.user_id.clone())),
            ("udp".to_owned(), XhttpValue::Boolean(true)),
            (
                "network".to_owned(),
                XhttpValue::String(self.transport.as_str().to_owned()),
            ),
            (
                "encryption".to_owned(),
                XhttpValue::String(self.encryption.clone()),
            ),
        ];
        if let Some(flow) = self.flow {
            entries.push((
                "flow".to_owned(),
                XhttpValue::String(flow.mihomo_str().to_owned()),
            ));
        }
        if let Some(packet_encoding) = self.packet_encoding {
            entries.push((
                "packet-encoding".to_owned(),
                XhttpValue::String(packet_encoding.as_str().to_owned()),
            ));
        }
        if matches!(self.security, VlessSecurity::Tls | VlessSecurity::Reality) {
            entries.push(("tls".to_owned(), XhttpValue::Boolean(true)));
            if !self.server_name.is_empty() {
                entries.push((
                    "servername".to_owned(),
                    XhttpValue::String(self.server_name.clone()),
                ));
            }
            if !self.fingerprint.is_empty() {
                entries.push((
                    "client-fingerprint".to_owned(),
                    XhttpValue::String(self.fingerprint.clone()),
                ));
            }
            if !self.alpn.is_empty() {
                entries.push((
                    "alpn".to_owned(),
                    XhttpValue::Array(self.alpn.iter().cloned().map(XhttpValue::String).collect()),
                ));
            }
            if self.allow_insecure {
                entries.push(("skip-cert-verify".to_owned(), XhttpValue::Boolean(true)));
            }
        }
        if self.security == VlessSecurity::Reality {
            let mut reality = vec![(
                "public-key".to_owned(),
                XhttpValue::String(self.public_key.clone()),
            )];
            if !self.short_id.is_empty() {
                reality.push((
                    "short-id".to_owned(),
                    XhttpValue::String(self.short_id.clone()),
                ));
            }
            if self.reality_pq {
                reality.push((
                    "support-x25519mlkem768".to_owned(),
                    XhttpValue::Boolean(true),
                ));
            }
            entries.push(("reality-opts".to_owned(), XhttpValue::Object(reality)));
        }
        match self.transport {
            VlessTransport::Tcp => {}
            VlessTransport::WebSocket => {
                let mut options = vec![("path".to_owned(), XhttpValue::String(self.path.clone()))];
                if !self.host.is_empty() {
                    options.push((
                        "headers".to_owned(),
                        XhttpValue::Object(vec![(
                            "Host".to_owned(),
                            XhttpValue::String(self.host.clone()),
                        )]),
                    ));
                }
                entries.push(("ws-opts".to_owned(), XhttpValue::Object(options)));
            }
            VlessTransport::Grpc => {
                entries.push((
                    "grpc-opts".to_owned(),
                    XhttpValue::Object(vec![(
                        "grpc-service-name".to_owned(),
                        XhttpValue::String(self.service_name.clone()),
                    )]),
                ));
            }
            VlessTransport::Http2 => {
                let mut options = Vec::new();
                if !self.host.is_empty() {
                    options.push((
                        "host".to_owned(),
                        XhttpValue::Array(vec![XhttpValue::String(self.host.clone())]),
                    ));
                }
                options.push(("path".to_owned(), XhttpValue::String(self.path.clone())));
                entries.push(("h2-opts".to_owned(), XhttpValue::Object(options)));
            }
            VlessTransport::Http => {
                let mut options = vec![(
                    "path".to_owned(),
                    XhttpValue::Array(vec![XhttpValue::String(self.path.clone())]),
                )];
                if !self.host.is_empty() {
                    options.push((
                        "headers".to_owned(),
                        XhttpValue::Object(vec![(
                            "Host".to_owned(),
                            XhttpValue::Array(vec![XhttpValue::String(self.host.clone())]),
                        )]),
                    ));
                }
                entries.push(("http-opts".to_owned(), XhttpValue::Object(options)));
            }
            VlessTransport::Xhttp => {
                let mut options = vec![("path".to_owned(), XhttpValue::String(self.path.clone()))];
                if !self.host.is_empty() {
                    options.push(("host".to_owned(), XhttpValue::String(self.host.clone())));
                }
                if !self.mode.is_empty() {
                    options.push(("mode".to_owned(), XhttpValue::String(self.mode.clone())));
                }
                if let Some(configuration) = &self.xhttp {
                    options.extend(configuration.normalized_entries());
                }
                entries.push(("xhttp-opts".to_owned(), XhttpValue::Object(options)));
            }
        }
        entries
    }
}

fn download_mode(mode: XhttpMode) -> XhttpDownloadMode {
    match mode {
        XhttpMode::Default | XhttpMode::Auto => XhttpDownloadMode::Auto,
        XhttpMode::StreamOne => XhttpDownloadMode::StreamOne,
        XhttpMode::StreamUp => XhttpDownloadMode::StreamUp,
        XhttpMode::PacketUp => XhttpDownloadMode::PacketUp,
    }
}

fn download_security(security: VlessSecurity) -> XhttpDownloadSecurity {
    match security {
        VlessSecurity::None => XhttpDownloadSecurity::None,
        VlessSecurity::Tls => XhttpDownloadSecurity::Tls,
        VlessSecurity::Reality => XhttpDownloadSecurity::Reality,
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_owned());
    }
}

fn normalize_identity_server(server: &str) -> String {
    if let Ok(address) = Ipv4Addr::from_str(server) {
        address.to_string()
    } else if let Ok(address) = Ipv6Addr::from_str(server) {
        address.to_string()
    } else {
        server.to_lowercase().trim_end_matches('.').to_owned()
    }
}

fn entries_to_json(entries: &[(String, XhttpValue)]) -> Value {
    let mut values = Map::new();
    for (name, value) in entries {
        values.insert(name.clone(), xhttp_value_to_json(value));
    }
    Value::Object(values)
}

fn xhttp_value_to_json(value: &XhttpValue) -> Value {
    match value {
        XhttpValue::Boolean(value) => Value::Bool(*value),
        XhttpValue::Integer(value) => Value::Number(Number::from(*value)),
        XhttpValue::String(value) => Value::String(value.clone()),
        XhttpValue::Array(values) => Value::Array(values.iter().map(xhttp_value_to_json).collect()),
        XhttpValue::Object(values) => entries_to_json(values),
    }
}

pub(crate) fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value serialization is infallible")
}

fn yaml_scalar(value: &XhttpValue) -> String {
    match value {
        XhttpValue::Boolean(value) => value.to_string(),
        XhttpValue::Integer(value) => value.to_string(),
        XhttpValue::String(value) => {
            serde_json::to_string(value).expect("string serialization is infallible")
        }
        XhttpValue::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(yaml_scalar)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        XhttpValue::Object(values) if values.is_empty() => "{}".to_owned(),
        XhttpValue::Object(_) => String::new(),
    }
}

fn append_yaml_mapping(
    lines: &mut Vec<String>,
    entries: &[(String, XhttpValue)],
    indent: usize,
    quote_keys: bool,
) {
    let prefix = " ".repeat(indent);
    for (name, value) in entries {
        let rendered_name = if quote_keys {
            serde_json::to_string(name).expect("mapping key serialization is infallible")
        } else {
            name.clone()
        };
        match value {
            XhttpValue::Object(values) if values.is_empty() => {
                lines.push(format!("{prefix}{rendered_name}: {{}}"));
            }
            XhttpValue::Object(values) => {
                lines.push(format!("{prefix}{rendered_name}:"));
                append_yaml_mapping(lines, values, indent + 2, name == "headers");
            }
            _ => lines.push(format!("{prefix}{rendered_name}: {}", yaml_scalar(value))),
        }
    }
}

pub(crate) fn sha256_hex(input: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_length = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = INITIAL;
    for chunk in padded.as_chunks::<64>().0 {
        let mut schedule = [0_u32; 64];
        let mut index = 0;
        while index < 16 {
            let offset = index * 4;
            schedule[index] = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
            index += 1;
        }
        while index < 64 {
            let small_zero = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let small_one = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(small_zero)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(small_one);
            index += 1;
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        index = 0;
        while index < 64 {
            let big_one = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary_one = h
                .wrapping_add(big_one)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(schedule[index]);
            let big_zero = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary_two = big_zero.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary_one);
            d = c;
            c = b;
            b = a;
            a = temporary_one.wrapping_add(temporary_two);
            index += 1;
        }
        for (word, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *word = word.wrapping_add(value);
        }
    }

    let mut output = String::with_capacity(64);
    for word in state {
        write!(&mut output, "{word:08x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";
    const REALITY_KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";

    #[test]
    fn sha256_vectors_are_stable() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn private_debug_is_redacted_and_preview_is_bounded() {
        let uri = format!(
            "vless://{UUID}@server.private.invalid:443?type=tcp&security=reality&pbk={REALITY_KEY}&sni=sni.private.invalid&sid=a1b2&spx=%2Fprivate&fp=chrome#%00%20{}",
            "x".repeat(90)
        );
        let profile = parse_vless_canonical(&uri).expect("canonical profile");
        let debug = format!("{profile:?}");
        for marker in ["private.invalid", REALITY_KEY, UUID, "a1b2"] {
            assert!(!debug.contains(marker));
        }
        let preview = profile.preview();
        assert_eq!(preview.credential_hint, "••••1111");
        assert_eq!(preview.suggested_name.chars().count(), 80);
        assert!(preview.compatibility_note.contains("spider path"));
    }

    #[test]
    fn renderer_contains_private_values_only_on_explicit_request() {
        let uri = format!(
            "vless://{UUID}@server.private.invalid:443?type=ws&security=tls&sni=sni.private.invalid&host=host.private.invalid&path=%2Fprivate&fp=chrome"
        );
        let profile = parse_vless_canonical(&uri).expect("canonical profile");
        let yaml = profile.render_mihomo_proxy("Private Node", None);
        assert!(yaml.contains("server.private.invalid"));
        assert!(yaml.contains("host.private.invalid"));
        assert!(yaml.contains("uuid:"));
        assert!(!format!("{profile:?}").contains("private.invalid"));
    }
}
