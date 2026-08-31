// SPDX-License-Identifier: MIT

//! Strict, bounded VLESS query-envelope and coarse transport metadata parsing.
//!
//! These R2 slices intentionally do not validate or expose REALITY keys,
//! VLESS Encryption, XHTTP `extra`, canonical identity, or Mihomo rendering.
//! Query values remain private in memory; the public model exposes only the
//! normalized non-credential semantics accepted by the existing Python path,
//! including Vision-flow and packet-encoding vocabulary.

use std::collections::BTreeMap;
use std::fmt;

use crate::MAX_CLASSIFICATION_INPUT_BYTES;
use crate::vless::{
    MAX_VLESS_URI_BYTES, VlessAuthorityError, extract_vless_uri, hex_value, parse_vless_authority,
};

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

impl VlessPacketEncoding {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xudp => "xudp",
            Self::PacketAddr => "packetaddr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessQueryMetadata {
    fields: BTreeMap<String, String>,
    pub transport: VlessTransport,
    pub security: VlessSecurity,
    pub allow_insecure: bool,
    pub xhttp_mode: Option<XhttpMode>,
    pub flow: Option<VlessFlow>,
    pub packet_encoding: Option<VlessPacketEncoding>,
    pub provider_metadata_present: bool,
    pub non_xhttp_mode_metadata: bool,
}

impl VlessQueryMetadata {
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
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
        })
    }
}

impl std::error::Error for VlessQueryError {}

impl From<VlessAuthorityError> for VlessQueryError {
    fn from(error: VlessAuthorityError) -> Self {
        Self::Authority(error)
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
    let allow_insecure = boolean_alias(&fields, &["allowinsecure", "skip-cert-verify"])?;
    let (xhttp_mode, non_xhttp_mode_metadata) = xhttp_mode(&fields, transport)?;
    let flow = flow(&fields, transport, security)?;
    let packet_encoding = packet_encoding(&fields)?;
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
