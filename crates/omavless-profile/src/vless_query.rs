// SPDX-License-Identifier: MIT

//! Strict, bounded VLESS query-envelope and coarse transport metadata parsing.
//!
//! These R2 slices intentionally do not expose REALITY or Encryption keys and
//! do not yet validate XHTTP `extra`, canonical identity, or Mihomo rendering.
//! Query values remain private in memory; the public model exposes only the
//! normalized non-credential semantics accepted by the existing Python path,
//! including REALITY/PQ, Vision-flow, packet-encoding and transport-option
//! facts.

use std::collections::BTreeMap;
use std::fmt;

use crate::MAX_CLASSIFICATION_INPUT_BYTES;
use crate::base64url::decoded_len_if_canonical;
use crate::vless::{
    MAX_VLESS_URI_BYTES, VlessAuthorityError, extract_vless_uri, hex_value, parse_vless_authority,
};
use crate::vless_encryption::{VlessEncryption, VlessEncryptionError, parse_vless_encryption};

const MAX_QUERY_FIELDS: usize = 128;
const MAX_PROVIDER_METADATA_BYTES: usize = 128;

const ALLOWED_FIELDS: &[&str] = &[
    "type",
    "network",
    "security",
    "pbk",
    "publickey",
    "public-key",
    "sni",
    "servername",
    "sid",
    "short-id",
    "spx",
    "spider-x",
    "supportx25519mlkem768",
    "support-x25519mlkem768",
    "mldsa65verify",
    "mldsa65-verify",
    "encryption",
    "flow",
    "packetencoding",
    "packet-encoding",
    "mode",
    "extra",
    "headertype",
    "header-type",
    "fp",
    "fingerprint",
    "client-fingerprint",
    "path",
    "host",
    "servicename",
    "service-name",
    "alpn",
    "allowinsecure",
    "skip-cert-verify",
    "concurrency",
    "x-durev-block",
    "x-durev-prio",
];

const PROVIDER_METADATA_FIELDS: &[&str] = &["concurrency", "x-durev-block", "x-durev-prio"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessTransport {
    Tcp,
    WebSocket,
    Http,
    Http2,
    Grpc,
    Xhttp,
}

impl VlessTransport {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::WebSocket => "ws",
            Self::Http => "http",
            Self::Http2 => "h2",
            Self::Grpc => "grpc",
            Self::Xhttp => "xhttp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessSecurity {
    None,
    Tls,
    Reality,
}

impl VlessSecurity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Tls => "tls",
            Self::Reality => "reality",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpMode {
    Default,
    Auto,
    StreamOne,
    StreamUp,
    PacketUp,
}

impl XhttpMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Auto => "auto",
            Self::StreamOne => "stream-one",
            Self::StreamUp => "stream-up",
            Self::PacketUp => "packet-up",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessFlow {
    Vision,
    VisionUdp443,
}

impl VlessFlow {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vision => "xtls-rprx-vision",
            Self::VisionUdp443 => "xtls-rprx-vision-udp443",
        }
    }

    #[must_use]
    pub const fn mihomo_str(self) -> &'static str {
        "xtls-rprx-vision"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessPacketEncoding {
    Xudp,
    PacketAddr,
}

/// Credential-safe facts about transport-specific query values.
///
/// Host, path, service-name, fingerprint and ALPN text is intentionally not
/// exposed. Exact private values move only with the later canonical profile
/// and rendering slices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessTransportOptions {
    pub path_default: bool,
    pub path_starts_with_slash: bool,
    pub path_non_ascii: bool,
    pub host_present: bool,
    pub host_non_ascii: bool,
    pub service_name_present: bool,
    pub service_name_non_ascii: bool,
    pub fingerprint_present: bool,
    pub fingerprint_non_ascii: bool,
    pub alpn_count: usize,
    pub alpn_non_ascii: bool,
}

impl VlessPacketEncoding {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xudp => "xudp",
            Self::PacketAddr => "packetaddr",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VlessQueryMetadata {
    fields: BTreeMap<String, String>,
    pub transport: VlessTransport,
    pub security: VlessSecurity,
    pub allow_insecure: bool,
    pub xhttp_mode: Option<XhttpMode>,
    pub flow: Option<VlessFlow>,
    pub packet_encoding: Option<VlessPacketEncoding>,
    pub encryption: Option<VlessEncryption>,
    pub transport_options: VlessTransportOptions,
    pub reality_pq: bool,
    pub reality_pq_present: bool,
    pub reality_short_id_present: bool,
    pub reality_spider_x_present: bool,
    pub provider_metadata_present: bool,
    pub non_xhttp_mode_metadata: bool,
}

/// Private connection values already validated by [`VlessQueryMetadata`].
///
/// This projection is crate-private so the canonical VLESS adapter can compose
/// the accepted R2 query slices without exposing endpoints, keys, paths or
/// credential-bearing Encryption text in public parity reports.
pub(crate) struct VlessPrivateQuery {
    pub encryption: String,
    pub server_name: String,
    pub fingerprint: String,
    pub public_key: String,
    pub short_id: String,
    pub spider_x_present: bool,
    pub path: String,
    pub host: String,
    pub service_name: String,
    pub raw_xhttp_extra: String,
    pub alpn: Vec<String>,
}

impl VlessQueryMetadata {
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    pub(crate) fn private_projection(&self) -> VlessPrivateQuery {
        let alias = |names: &[&str]| -> String {
            names
                .iter()
                .find_map(|name| self.fields.get(*name))
                .cloned()
                .unwrap_or_default()
        };
        let path = percent_decode_lossy(self.fields.get("path").map_or("/", String::as_str));
        let path = if path.is_empty() {
            "/".to_owned()
        } else {
            path
        };
        let alpn = self
            .fields
            .get("alpn")
            .map_or("", String::as_str)
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
        VlessPrivateQuery {
            encryption: self
                .fields
                .get("encryption")
                .filter(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| "none".to_owned()),
            server_name: alias(&["sni", "servername"]),
            fingerprint: alias(&["fp", "fingerprint", "client-fingerprint"]),
            public_key: alias(&["pbk", "publickey", "public-key"]),
            short_id: alias(&["sid", "short-id"]),
            spider_x_present: ["spx", "spider-x"]
                .iter()
                .find_map(|name| self.fields.get(*name))
                .is_some_and(|value| !value.is_empty()),
            path,
            host: self.fields.get("host").cloned().unwrap_or_default(),
            service_name: alias(&["servicename", "service-name"]),
            raw_xhttp_extra: self.fields.get("extra").cloned().unwrap_or_default(),
            alpn,
        }
    }
}

impl fmt::Debug for VlessQueryMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VlessQueryMetadata")
            .field("transport", &self.transport)
            .field("security", &self.security)
            .field("allow_insecure", &self.allow_insecure)
            .field("xhttp_mode", &self.xhttp_mode)
            .field("flow", &self.flow)
            .field("packet_encoding", &self.packet_encoding)
            .field("encryption_enabled", &self.encryption.is_some())
            .field("transport_options", &self.transport_options)
            .field("reality_pq", &self.reality_pq)
            .field("reality_pq_present", &self.reality_pq_present)
            .field("reality_short_id_present", &self.reality_short_id_present)
            .field("reality_spider_x_present", &self.reality_spider_x_present)
            .field("provider_metadata_present", &self.provider_metadata_present)
            .field("non_xhttp_mode_metadata", &self.non_xhttp_mode_metadata)
            .field("private_field_count", &self.fields.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessQueryError {
    Authority(VlessAuthorityError),
    InvalidQuery,
    DuplicateFields,
    UnsupportedFields,
    InvalidProviderMetadata,
    ConflictingAliases,
    UnsupportedTransport,
    UnsupportedSecurity,
    InvalidBoolean,
    UnsupportedXhttpMode,
    UnsupportedFlow,
    VisionRequiresTcp,
    VisionRequiresSecurity,
    UnsupportedPacketEncoding,
    RealityFieldsRequired,
    InvalidRealityPqBoolean,
    RealityPqRequiresReality,
    RealityMldsaUnsupported,
    InvalidRealityPublicKey,
    InvalidRealityShortId,
    UnsupportedTcpHeader,
    Encryption(VlessEncryptionError),
}

impl VlessQueryError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Authority(error) => error.code(),
            Self::InvalidQuery => "invalid_query",
            Self::DuplicateFields => "duplicate_fields",
            Self::UnsupportedFields => "unsupported_fields",
            Self::InvalidProviderMetadata => "invalid_provider_metadata",
            Self::ConflictingAliases => "conflicting_aliases",
            Self::UnsupportedTransport => "unsupported_transport",
            Self::UnsupportedSecurity => "unsupported_security",
            Self::InvalidBoolean => "invalid_boolean",
            Self::UnsupportedXhttpMode => "unsupported_xhttp_mode",
            Self::UnsupportedFlow => "unsupported_flow",
            Self::VisionRequiresTcp => "vision_requires_tcp",
            Self::VisionRequiresSecurity => "vision_requires_security",
            Self::UnsupportedPacketEncoding => "unsupported_packet_encoding",
            Self::RealityFieldsRequired => "reality_fields_required",
            Self::InvalidRealityPqBoolean => "invalid_reality_pq_boolean",
            Self::RealityPqRequiresReality => "reality_pq_requires_reality",
            Self::RealityMldsaUnsupported => "reality_mldsa_unsupported",
            Self::InvalidRealityPublicKey => "invalid_reality_public_key",
            Self::InvalidRealityShortId => "invalid_reality_short_id",
            Self::UnsupportedTcpHeader => "unsupported_tcp_header",
            Self::Encryption(error) => error.code(),
        }
    }
}

impl fmt::Display for VlessQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Authority(error) => return error.fmt(formatter),
            Self::InvalidQuery => "VLESS query is invalid",
            Self::DuplicateFields => "VLESS link contains duplicate fields",
            Self::UnsupportedFields => "VLESS link contains unsupported fields",
            Self::InvalidProviderMetadata => "VLESS provider metadata has an invalid format",
            Self::ConflictingAliases => "VLESS link contains conflicting field aliases",
            Self::UnsupportedTransport => "VLESS transport is unsupported",
            Self::UnsupportedSecurity => "VLESS security is unsupported",
            Self::InvalidBoolean => "VLESS boolean query field must be true or false",
            Self::UnsupportedXhttpMode => "VLESS XHTTP mode is unsupported",
            Self::UnsupportedFlow => "VLESS flow is unsupported",
            Self::VisionRequiresTcp => "VLESS Vision flow requires the TCP transport",
            Self::VisionRequiresSecurity => "VLESS Vision flow requires TLS or Reality security",
            Self::UnsupportedPacketEncoding => "VLESS packet encoding is unsupported",
            Self::RealityFieldsRequired => "VLESS Reality requires a public key and server name",
            Self::InvalidRealityPqBoolean => {
                "VLESS Reality post-quantum flag must be true or false"
            }
            Self::RealityPqRequiresReality => {
                "VLESS Reality post-quantum flag requires Reality security"
            }
            Self::RealityMldsaUnsupported => "VLESS Reality ML-DSA verification is unsupported",
            Self::InvalidRealityPublicKey => "VLESS Reality public key has an invalid format",
            Self::InvalidRealityShortId => "VLESS Reality short ID has an invalid format",
            Self::UnsupportedTcpHeader => "VLESS TCP header type is unsupported",
            Self::Encryption(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for VlessQueryError {}

impl From<VlessAuthorityError> for VlessQueryError {
    fn from(error: VlessAuthorityError) -> Self {
        Self::Authority(error)
    }
}

impl From<VlessEncryptionError> for VlessQueryError {
    fn from(error: VlessEncryptionError) -> Self {
        Self::Encryption(error)
    }
}

fn query_text(uri: &str) -> &str {
    let fragment = uri.find('#').unwrap_or(uri.len());
    let Some(question) = uri[..fragment].find('?') else {
        return "";
    };
    &uri[question + 1..fragment]
}

fn form_decode(value: &str) -> Result<String, VlessQueryError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(if bytes[index] == b'+' {
                b' '
            } else {
                bytes[index]
            });
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| VlessQueryError::InvalidQuery)
}

fn parse_fields(query: &str) -> Result<BTreeMap<String, String>, VlessQueryError> {
    if !query.is_empty() && query.split('&').count() > MAX_QUERY_FIELDS {
        return Err(VlessQueryError::InvalidQuery);
    }
    let mut fields = BTreeMap::new();
    for item in query.split('&').filter(|item| !item.is_empty()) {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        let key = form_decode(key)?.to_lowercase();
        let value = form_decode(value)?;
        if fields.insert(key, value).is_some() {
            return Err(VlessQueryError::DuplicateFields);
        }
    }
    if fields
        .keys()
        .any(|key| !ALLOWED_FIELDS.contains(&key.as_str()))
    {
        return Err(VlessQueryError::UnsupportedFields);
    }
    for name in PROVIDER_METADATA_FIELDS {
        if let Some(value) = fields.get(*name)
            && (value.len() > MAX_PROVIDER_METADATA_BYTES
                || value
                    .chars()
                    .any(|character| matches!(character as u32, 0x00..=0x1f | 0x7f)))
        {
            return Err(VlessQueryError::InvalidProviderMetadata);
        }
    }
    Ok(fields)
}

fn first_alias<'a>(
    fields: &'a BTreeMap<String, String>,
    names: &[&str],
) -> Result<Option<&'a str>, VlessQueryError> {
    let mut found = None;
    for name in names {
        if let Some(value) = fields.get(*name) {
            if found.is_some() {
                return Err(VlessQueryError::ConflictingAliases);
            }
            found = Some(value.as_str());
        }
    }
    Ok(found)
}

fn transport(fields: &BTreeMap<String, String>) -> Result<VlessTransport, VlessQueryError> {
    let raw = first_alias(fields, &["type", "network"])?
        .unwrap_or("tcp")
        .to_lowercase();
    match raw.as_str() {
        "" | "tcp" | "raw" => Ok(VlessTransport::Tcp),
        "ws" => Ok(VlessTransport::WebSocket),
        "http" => Ok(VlessTransport::Http),
        "h2" => Ok(VlessTransport::Http2),
        "grpc" => Ok(VlessTransport::Grpc),
        "xhttp" => Ok(VlessTransport::Xhttp),
        _ => Err(VlessQueryError::UnsupportedTransport),
    }
}

fn security(fields: &BTreeMap<String, String>) -> Result<VlessSecurity, VlessQueryError> {
    let raw = fields
        .get("security")
        .map_or("none", String::as_str)
        .to_lowercase();
    match raw.as_str() {
        "" | "none" => Ok(VlessSecurity::None),
        "tls" => Ok(VlessSecurity::Tls),
        "reality" => Ok(VlessSecurity::Reality),
        _ => Err(VlessQueryError::UnsupportedSecurity),
    }
}

fn boolean_alias(
    fields: &BTreeMap<String, String>,
    names: &[&str],
) -> Result<bool, VlessQueryError> {
    let raw = first_alias(fields, names)?.unwrap_or("").to_lowercase();
    match raw.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "" | "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(VlessQueryError::InvalidBoolean),
    }
}

fn xhttp_mode(
    fields: &BTreeMap<String, String>,
    transport: VlessTransport,
) -> Result<(Option<XhttpMode>, bool), VlessQueryError> {
    let raw = fields.get("mode").map_or("", String::as_str);
    if transport != VlessTransport::Xhttp {
        return Ok((None, !raw.is_empty()));
    }
    let mode = match raw.to_lowercase().as_str() {
        "" => XhttpMode::Default,
        "auto" => XhttpMode::Auto,
        "stream-one" => XhttpMode::StreamOne,
        "stream-up" => XhttpMode::StreamUp,
        "packet-up" => XhttpMode::PacketUp,
        _ => return Err(VlessQueryError::UnsupportedXhttpMode),
    };
    Ok((Some(mode), false))
}

fn flow(
    fields: &BTreeMap<String, String>,
    transport: VlessTransport,
    security: VlessSecurity,
) -> Result<Option<VlessFlow>, VlessQueryError> {
    let raw = fields.get("flow").map_or("", String::as_str).to_lowercase();
    let flow = match raw.as_str() {
        "" => return Ok(None),
        "xtls-rprx-vision" => VlessFlow::Vision,
        "xtls-rprx-vision-udp443" => VlessFlow::VisionUdp443,
        _ => return Err(VlessQueryError::UnsupportedFlow),
    };
    if transport != VlessTransport::Tcp {
        return Err(VlessQueryError::VisionRequiresTcp);
    }
    if !matches!(security, VlessSecurity::Tls | VlessSecurity::Reality) {
        return Err(VlessQueryError::VisionRequiresSecurity);
    }
    Ok(Some(flow))
}

fn packet_encoding(
    fields: &BTreeMap<String, String>,
) -> Result<Option<VlessPacketEncoding>, VlessQueryError> {
    let raw = first_alias(fields, &["packetencoding", "packet-encoding"])?
        .unwrap_or("")
        .to_lowercase();
    match raw.as_str() {
        "" => Ok(None),
        "xudp" => Ok(Some(VlessPacketEncoding::Xudp)),
        "packetaddr" => Ok(Some(VlessPacketEncoding::PacketAddr)),
        _ => Err(VlessQueryError::UnsupportedPacketEncoding),
    }
}

fn percent_decode_lossy(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn transport_options(
    fields: &BTreeMap<String, String>,
    transport: VlessTransport,
) -> Result<VlessTransportOptions, VlessQueryError> {
    let header_type = first_alias(fields, &["headertype", "header-type"])?
        .unwrap_or("")
        .to_lowercase();
    if transport == VlessTransport::Tcp && !matches!(header_type.as_str(), "" | "none") {
        return Err(VlessQueryError::UnsupportedTcpHeader);
    }

    let fingerprint =
        first_alias(fields, &["fp", "fingerprint", "client-fingerprint"])?.unwrap_or("");
    let service_name = first_alias(fields, &["servicename", "service-name"])?.unwrap_or("");
    let host = fields.get("host").map_or("", String::as_str);
    // Python's parse_qsl performs the first form decode and parse_vless then
    // applies urllib.parse.unquote once more to path. Preserve that established
    // behavior, including lossy replacement for invalid bytes produced only by
    // the second decode and leaving '+' untouched on that second pass.
    let path = percent_decode_lossy(fields.get("path").map_or("/", String::as_str));
    let path = if path.is_empty() { "/" } else { &path };
    let alpn = fields
        .get("alpn")
        .map_or("", String::as_str)
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    Ok(VlessTransportOptions {
        path_default: path == "/",
        path_starts_with_slash: path.starts_with('/'),
        path_non_ascii: !path.is_ascii(),
        host_present: !host.is_empty(),
        host_non_ascii: !host.is_ascii(),
        service_name_present: !service_name.is_empty(),
        service_name_non_ascii: !service_name.is_ascii(),
        fingerprint_present: !fingerprint.is_empty(),
        fingerprint_non_ascii: !fingerprint.is_ascii(),
        alpn_count: alpn.len(),
        alpn_non_ascii: alpn.iter().any(|part| !part.is_ascii()),
    })
}

fn strict_boolean_alias(
    fields: &BTreeMap<String, String>,
    names: &[&str],
) -> Result<(bool, bool), VlessQueryError> {
    let present = names.iter().any(|name| fields.contains_key(*name));
    if !present {
        return Ok((false, false));
    }
    let raw = first_alias(fields, names)?.unwrap_or("").to_lowercase();
    match raw.as_str() {
        "1" | "true" | "yes" | "on" => Ok((true, true)),
        "" | "0" | "false" | "no" | "off" => Ok((false, true)),
        _ => Err(VlessQueryError::InvalidRealityPqBoolean),
    }
}

pub(crate) fn valid_reality_public_key(value: &str) -> bool {
    decoded_len_if_canonical(value) == Some(32)
}

pub(crate) fn valid_reality_short_id(value: &str) -> bool {
    value.len() <= 16
        && value.len().is_multiple_of(2)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RealityMetadata {
    pq: bool,
    pq_present: bool,
    short_id_present: bool,
    spider_x_present: bool,
}

fn reality_metadata(
    fields: &BTreeMap<String, String>,
    security: VlessSecurity,
) -> Result<RealityMetadata, VlessQueryError> {
    let public_key = first_alias(fields, &["pbk", "publickey", "public-key"])?.unwrap_or("");
    let server_name = first_alias(fields, &["sni", "servername"])?.unwrap_or("");
    if security == VlessSecurity::Reality && (public_key.is_empty() || server_name.is_empty()) {
        return Err(VlessQueryError::RealityFieldsRequired);
    }
    let short_id = first_alias(fields, &["sid", "short-id"])?.unwrap_or("");
    let spider_x = first_alias(fields, &["spx", "spider-x"])?.unwrap_or("");
    let (pq, pq_present) =
        strict_boolean_alias(fields, &["supportx25519mlkem768", "support-x25519mlkem768"])?;
    if pq_present && security != VlessSecurity::Reality {
        return Err(VlessQueryError::RealityPqRequiresReality);
    }
    let mldsa = first_alias(fields, &["mldsa65verify", "mldsa65-verify"])?.unwrap_or("");
    if !mldsa.is_empty() {
        return Err(VlessQueryError::RealityMldsaUnsupported);
    }
    if security == VlessSecurity::Reality {
        if !valid_reality_public_key(public_key) {
            return Err(VlessQueryError::InvalidRealityPublicKey);
        }
        if !valid_reality_short_id(short_id) {
            return Err(VlessQueryError::InvalidRealityShortId);
        }
    }
    Ok(RealityMetadata {
        pq,
        pq_present,
        short_id_present: !short_id.is_empty(),
        spider_x_present: !spider_x.is_empty(),
    })
}

pub fn parse_vless_query_metadata(input: &str) -> Result<VlessQueryMetadata, VlessQueryError> {
    if input.len() > MAX_CLASSIFICATION_INPUT_BYTES {
        return Err(VlessQueryError::Authority(
            VlessAuthorityError::InvalidInput,
        ));
    }
    parse_vless_authority(input)?;
    let uri = extract_vless_uri(input)?;
    if uri.len() > MAX_VLESS_URI_BYTES {
        return Err(VlessQueryError::Authority(
            VlessAuthorityError::InvalidInput,
        ));
    }
    let fields = parse_fields(query_text(uri))?;
    let transport = transport(&fields)?;
    let security = security(&fields)?;
    let reality = reality_metadata(&fields, security)?;
    let encryption =
        parse_vless_encryption(fields.get("encryption").map_or("none", String::as_str))?;
    let allow_insecure = boolean_alias(&fields, &["allowinsecure", "skip-cert-verify"])?;
    let (xhttp_mode, non_xhttp_mode_metadata) = xhttp_mode(&fields, transport)?;
    let flow = flow(&fields, transport, security)?;
    let packet_encoding = packet_encoding(&fields)?;
    let transport_options = transport_options(&fields, transport)?;
    let provider_metadata_present = PROVIDER_METADATA_FIELDS
        .iter()
        .any(|name| fields.contains_key(*name));
    Ok(VlessQueryMetadata {
        fields,
        transport,
        security,
        allow_insecure,
        xhttp_mode,
        flow,
        packet_encoding,
        encryption,
        transport_options,
        reality_pq: reality.pq,
        reality_pq_present: reality.pq_present,
        reality_short_id_present: reality.short_id_present,
        reality_spider_x_present: reality.spider_x_present,
        provider_metadata_present,
        non_xhttp_mode_metadata,
    })
}

pub fn parse_vless_query_metadata_bytes(
    input: &[u8],
) -> Result<VlessQueryMetadata, VlessQueryError> {
    if input.len() > MAX_CLASSIFICATION_INPUT_BYTES {
        return Err(VlessQueryError::Authority(
            VlessAuthorityError::InvalidInput,
        ));
    }
    let input = std::str::from_utf8(input)
        .map_err(|_| VlessQueryError::Authority(VlessAuthorityError::InvalidInput))?;
    parse_vless_query_metadata(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";

    fn uri(query: &str) -> String {
        format!("vless://{UUID}@example.invalid:443?{query}#Node")
    }

    #[test]
    fn normalizes_transport_security_boolean_and_modes() {
        let raw = parse_vless_query_metadata(&uri(
            "type=raw&security=TLS&allowInsecure=yes&mode=provider-note",
        ))
        .expect("raw TCP metadata");
        assert_eq!(raw.transport, VlessTransport::Tcp);
        assert_eq!(raw.security, VlessSecurity::Tls);
        assert!(raw.allow_insecure);
        assert!(raw.non_xhttp_mode_metadata);
        assert_eq!(raw.xhttp_mode, None);

        let xhttp = parse_vless_query_metadata(&uri("type=xhttp&security=none&mode=stream-up"))
            .expect("XHTTP mode");
        assert_eq!(xhttp.xhttp_mode, Some(XhttpMode::StreamUp));
        assert!(!xhttp.non_xhttp_mode_metadata);
    }

    #[test]
    fn normalizes_vision_flow_and_packet_encoding() {
        let parsed = parse_vless_query_metadata(&uri(
            "type=tcp&security=tls&flow=XTLS-RPRX-VISION-UDP443&packetEncoding=XUDP",
        ))
        .expect("Vision and packet metadata");
        assert_eq!(parsed.flow, Some(VlessFlow::VisionUdp443));
        assert_eq!(
            parsed.flow.map(VlessFlow::mihomo_str),
            Some("xtls-rprx-vision")
        );
        assert_eq!(parsed.packet_encoding, Some(VlessPacketEncoding::Xudp));

        let packet_addr = parse_vless_query_metadata(&uri("packet-encoding=packetaddr"))
            .expect("packetaddr metadata");
        assert_eq!(
            packet_addr.packet_encoding,
            Some(VlessPacketEncoding::PacketAddr)
        );
    }

    #[test]
    fn validates_reality_keys_short_ids_and_post_quantum_flags() {
        let parsed = parse_vless_query_metadata(&uri(
            "security=reality&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=0123456789abcdef&spx=%2F&supportX25519MLKEM768=true",
        ))
        .expect("Reality metadata");
        assert!(parsed.reality_pq);
        assert!(parsed.reality_pq_present);
        assert!(parsed.reality_short_id_present);
        assert!(parsed.reality_spider_x_present);

        let disabled = parse_vless_query_metadata(&uri(
            "security=reality&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&support-x25519mlkem768=false",
        ))
        .expect("disabled Reality PQ metadata");
        assert!(!disabled.reality_pq);
        assert!(disabled.reality_pq_present);

        for final_character in "AEIMQUYcgkosw048".chars() {
            let key = format!("{}{}", "A".repeat(42), final_character);
            parse_vless_query_metadata(&uri(&format!(
                "security=reality&sni=example.invalid&pbk={key}"
            )))
            .expect("every canonical 32-byte Base64 tail");
        }
    }

    #[test]
    fn rejects_invalid_reality_metadata_without_echoing_values() {
        let private = "private-marker";
        let cases = [
            (
                uri("security=reality"),
                VlessQueryError::RealityFieldsRequired,
            ),
            (
                uri(&format!(
                    "security=reality&sni=example.invalid&pbk={private}"
                )),
                VlessQueryError::InvalidRealityPublicKey,
            ),
            (
                uri(
                    "security=reality&sni=example.invalid&pbk=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                ),
                VlessQueryError::InvalidRealityPublicKey,
            ),
            (
                uri(
                    "security=reality&sni=example.invalid&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=abc",
                ),
                VlessQueryError::InvalidRealityShortId,
            ),
            (
                uri(&format!("mldsa65Verify={private}")),
                VlessQueryError::RealityMldsaUnsupported,
            ),
            (
                uri("supportX25519MLKEM768=maybe"),
                VlessQueryError::InvalidRealityPqBoolean,
            ),
            (
                uri("security=tls&supportX25519MLKEM768=false"),
                VlessQueryError::RealityPqRequiresReality,
            ),
        ];
        for (input, expected) in cases {
            let error = parse_vless_query_metadata(&input).expect_err("invalid Reality metadata");
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(private));
            assert!(error.to_string().len() <= 80);
        }
    }

    #[test]
    fn rejects_invalid_vision_and_packet_semantics_without_echoing_values() {
        let private = "private-marker";
        let cases = [
            (
                uri(&format!("flow={private}")),
                VlessQueryError::UnsupportedFlow,
            ),
            (
                uri("type=ws&security=tls&flow=xtls-rprx-vision"),
                VlessQueryError::VisionRequiresTcp,
            ),
            (
                uri("type=tcp&security=none&flow=xtls-rprx-vision"),
                VlessQueryError::VisionRequiresSecurity,
            ),
            (
                uri(&format!("packetEncoding={private}")),
                VlessQueryError::UnsupportedPacketEncoding,
            ),
        ];
        for (input, expected) in cases {
            let error = parse_vless_query_metadata(&input).expect_err("invalid flow metadata");
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(private));
            assert!(error.to_string().len() <= 80);
        }
    }

    #[test]
    fn form_decoding_aliases_and_provider_metadata_match_contract() {
        let parsed = parse_vless_query_metadata(&uri(
            "type=ws&host=cdn%2Eexample%2Einvalid&concurrency=two+streams",
        ))
        .expect("form decoding");
        assert_eq!(parsed.transport, VlessTransport::WebSocket);
        assert!(parsed.provider_metadata_present);
        assert_eq!(parsed.field_count(), 3);

        assert_eq!(
            parse_vless_query_metadata(&uri("type=tcp&network=tcp")),
            Err(VlessQueryError::ConflictingAliases)
        );
        assert_eq!(
            parse_vless_query_metadata(&uri("ALLOWINSECURE=1&allowinsecure=0")),
            Err(VlessQueryError::DuplicateFields)
        );
    }

    #[test]
    fn rejects_invalid_fields_without_echoing_values() {
        let private = "private-marker";
        let cases = [
            (
                uri(&format!("unknown={private}")),
                VlessQueryError::UnsupportedFields,
            ),
            (
                uri(&format!("type={private}")),
                VlessQueryError::UnsupportedTransport,
            ),
            (
                uri(&format!("security={private}")),
                VlessQueryError::UnsupportedSecurity,
            ),
            (
                uri(&format!("allowInsecure={private}")),
                VlessQueryError::InvalidBoolean,
            ),
            (
                uri(&format!("type=xhttp&mode={private}")),
                VlessQueryError::UnsupportedXhttpMode,
            ),
        ];
        for (input, expected) in cases {
            let error = parse_vless_query_metadata(&input).expect_err("invalid query metadata");
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(private));
            assert!(error.to_string().len() <= 80);
        }
    }

    #[test]
    fn enforces_query_field_utf8_and_count_bounds() {
        let too_many = (0..129)
            .map(|index| format!("host=value{index}"))
            .collect::<Vec<_>>()
            .join("&");
        assert_eq!(
            parse_vless_query_metadata(&uri(&too_many)),
            Err(VlessQueryError::InvalidQuery)
        );
        assert_eq!(
            parse_vless_query_metadata(&uri("host=%C3")),
            Err(VlessQueryError::InvalidQuery)
        );
        assert_eq!(
            parse_vless_query_metadata_bytes(&[0xff]),
            Err(VlessQueryError::Authority(
                VlessAuthorityError::InvalidInput
            ))
        );
    }
}
