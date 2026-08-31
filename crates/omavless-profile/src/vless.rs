// SPDX-License-Identifier: MIT

//! Bounded VLESS authority parsing for the incremental R2 migration.
//!
//! This module deliberately stops before query/transport, REALITY, XHTTP,
//! identity and Mihomo configuration semantics. Query-envelope and coarse
//! transport/security metadata live in the adjacent `vless_query` module.
//! Credentials and endpoint text remain available only in the in-memory model;
//! differential reports use the coarse [`VlessAuthorityFacts`] projection.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::MAX_CLASSIFICATION_INPUT_BYTES;

pub const MAX_VLESS_URI_BYTES: usize = 16 * 1024;
const PREVIEW_SERVER_CHARS: usize = 253;
const PREVIEW_LABEL_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlessAuthorityError {
    InvalidInput,
    MissingLink,
    InvalidLink,
    PasswordNotAllowed,
    InvalidUserId,
    MissingServerPort,
}

impl VlessAuthorityError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::MissingLink => "missing_vless_link",
            Self::InvalidLink => "invalid_link",
            Self::PasswordNotAllowed => "password_not_allowed",
            Self::InvalidUserId => "invalid_user_id",
            Self::MissingServerPort => "missing_server_port",
        }
    }
}

impl fmt::Display for VlessAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "VLESS input is invalid",
            Self::MissingLink => "Input does not contain a VLESS link",
            Self::InvalidLink => "VLESS link is invalid",
            Self::PasswordNotAllowed => "VLESS link must not contain a password field",
            Self::InvalidUserId => "VLESS user id is not a valid UUID",
            Self::MissingServerPort => "VLESS server and port are required",
        })
    }
}

impl std::error::Error for VlessAuthorityError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    Dns,
    Ipv4,
    Ipv6,
}

impl HostKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::Ipv4 => "ipv4",
            Self::Ipv6 => "ipv6",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessAuthority {
    pub user_id: String,
    pub server: String,
    pub port: u16,
    pub suggested_name: String,
    pub host_kind: HostKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessAuthorityPreview {
    pub server: String,
    pub port: u16,
    pub credential_hint: String,
    pub suggested_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessAuthorityFacts {
    pub host_kind: HostKind,
    pub standard_https_port: bool,
    pub label_kind: LabelKind,
    pub label_sanitized: bool,
    pub label_truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    None,
    Ascii,
    Unicode,
}

impl LabelKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ascii => "ascii",
            Self::Unicode => "unicode",
        }
    }
}

impl VlessAuthority {
    #[must_use]
    pub fn preview(&self) -> VlessAuthorityPreview {
        let suggested_name = sanitize_label(&self.suggested_name)
            .chars()
            .take(PREVIEW_LABEL_CHARS)
            .collect();
        let credential_suffix: String = self
            .user_id
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        VlessAuthorityPreview {
            server: self.server.chars().take(PREVIEW_SERVER_CHARS).collect(),
            port: self.port,
            credential_hint: format!("••••{credential_suffix}"),
            suggested_name,
        }
    }

    #[must_use]
    pub fn public_facts(&self) -> VlessAuthorityFacts {
        let sanitized = sanitize_label(&self.suggested_name);
        let label_kind = if sanitized.is_empty() {
            LabelKind::None
        } else if sanitized.is_ascii() {
            LabelKind::Ascii
        } else {
            LabelKind::Unicode
        };
        VlessAuthorityFacts {
            host_kind: self.host_kind,
            standard_https_port: self.port == 443,
            label_kind,
            label_sanitized: sanitized != self.suggested_name,
            label_truncated: sanitized.chars().count() > PREVIEW_LABEL_CHARS,
        }
    }
}

fn sanitize_label(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(*character as u32, 0x00..=0x1f | 0x7f))
        .collect::<String>()
        .trim()
        .to_owned()
}

pub(crate) fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(value: &str) -> String {
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

fn valid_uuid(value: &str) -> bool {
    let value = value.strip_prefix("urn:uuid:").unwrap_or(value);
    let value = value
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(value);
    let mut hexadecimal = 0;
    for character in value.bytes() {
        if character == b'-' {
            continue;
        }
        if !character.is_ascii_hexdigit() {
            return false;
        }
        hexadecimal += 1;
    }
    hexadecimal == 32
}

pub(crate) fn extract_vless_uri(input: &str) -> Result<&str, VlessAuthorityError> {
    for token in input.split_whitespace() {
        if token
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("vless://"))
        {
            if token.len() > MAX_VLESS_URI_BYTES {
                return Err(VlessAuthorityError::InvalidInput);
            }
            return Ok(token);
        }
    }
    Err(VlessAuthorityError::MissingLink)
}

fn parse_host_port(authority: &str) -> Result<(String, u16, HostKind), VlessAuthorityError> {
    if authority.is_empty() {
        return Err(VlessAuthorityError::MissingServerPort);
    }
    let (server, port_text, host_kind) = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some(close) = bracketed.find(']') else {
            return Err(VlessAuthorityError::InvalidLink);
        };
        let server = &bracketed[..close];
        let suffix = &bracketed[close + 1..];
        let Some(port) = suffix.strip_prefix(':') else {
            return Err(VlessAuthorityError::MissingServerPort);
        };
        if Ipv6Addr::from_str(server).is_err() {
            return Err(VlessAuthorityError::InvalidLink);
        }
        (server.to_lowercase(), port, HostKind::Ipv6)
    } else {
        let Some((server, port)) = authority.rsplit_once(':') else {
            return Err(VlessAuthorityError::MissingServerPort);
        };
        if server.contains(':') {
            return Err(VlessAuthorityError::InvalidLink);
        }
        let server = server.to_lowercase();
        let kind = if Ipv4Addr::from_str(&server).is_ok() {
            HostKind::Ipv4
        } else {
            HostKind::Dns
        };
        (server, port, kind)
    };
    if server.is_empty() || port_text.is_empty() {
        return Err(VlessAuthorityError::MissingServerPort);
    }
    let port_value: u32 = port_text
        .parse()
        .map_err(|_| VlessAuthorityError::InvalidLink)?;
    if port_value > u16::MAX.into() {
        return Err(VlessAuthorityError::InvalidLink);
    }
    if port_value == 0 {
        return Err(VlessAuthorityError::MissingServerPort);
    }
    Ok((server, port_value as u16, host_kind))
}

pub fn parse_vless_authority(input: &str) -> Result<VlessAuthority, VlessAuthorityError> {
    let uri = extract_vless_uri(input)?;
    let remainder = &uri[8..];
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    let fragment = remainder
        .split_once('#')
        .map_or("", |(_, fragment)| fragment);
    let Some((raw_user, raw_server)) = authority.rsplit_once('@') else {
        return Err(VlessAuthorityError::InvalidUserId);
    };
    if raw_user.contains(':') {
        return Err(VlessAuthorityError::PasswordNotAllowed);
    }
    let user_id = percent_decode(raw_user);
    if !valid_uuid(&user_id) {
        return Err(VlessAuthorityError::InvalidUserId);
    }
    let (server, port, host_kind) = parse_host_port(raw_server)?;
    Ok(VlessAuthority {
        user_id,
        server,
        port,
        suggested_name: percent_decode(fragment).trim().to_owned(),
        host_kind,
    })
}

pub fn parse_vless_authority_bytes(input: &[u8]) -> Result<VlessAuthority, VlessAuthorityError> {
    if input.len() > MAX_CLASSIFICATION_INPUT_BYTES {
        return Err(VlessAuthorityError::InvalidInput);
    }
    let input = std::str::from_utf8(input).map_err(|_| VlessAuthorityError::InvalidInput)?;
    parse_vless_authority(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";

    fn uri(authority: &str, fragment: &str) -> String {
        format!("vless://{UUID}@{authority}?security=none#{fragment}")
    }

    #[test]
    fn parses_dns_ipv4_ipv6_and_public_preview() {
        let dns = parse_vless_authority(&uri("EXAMPLE.INVALID:443", "A%20B")).expect("dns");
        assert_eq!(dns.server, "example.invalid");
        assert_eq!(dns.host_kind, HostKind::Dns);
        assert_eq!(dns.preview().suggested_name, "A B");
        assert_eq!(dns.preview().credential_hint, "••••1111");
        assert_eq!(dns.public_facts().label_kind, LabelKind::Ascii);

        let ipv4 = parse_vless_authority(&uri("192.0.2.1:8443", "")).expect("ipv4");
        assert_eq!(ipv4.host_kind, HostKind::Ipv4);
        assert!(!ipv4.public_facts().standard_https_port);

        let ipv6 = parse_vless_authority(&uri("[2001:db8::1]:443", "Метка")).expect("ipv6");
        assert_eq!(ipv6.server, "2001:db8::1");
        assert_eq!(ipv6.host_kind, HostKind::Ipv6);
        assert_eq!(ipv6.public_facts().label_kind, LabelKind::Unicode);
    }

    #[test]
    fn preserves_python_uuid_equivalent_spellings() {
        for user_id in [
            UUID.to_owned(),
            UUID.replace('-', ""),
            format!("{{{UUID}}}"),
        ] {
            let value = format!("vless://{user_id}@example.invalid:443");
            assert_eq!(
                parse_vless_authority(&value).expect("UUID").user_id,
                user_id
            );
        }
    }

    #[test]
    fn label_decoding_sanitization_and_truncation_are_bounded() {
        let value = parse_vless_authority(&uri(
            "example.invalid:443",
            &format!("%00%20{}%20", "x".repeat(90)),
        ))
        .expect("label");
        let facts = value.public_facts();
        assert!(facts.label_sanitized);
        assert!(facts.label_truncated);
        assert_eq!(value.preview().suggested_name.chars().count(), 80);

        let replacement = parse_vless_authority(&uri("example.invalid:443", "bad%C3"))
            .expect("lossy percent decoding");
        assert_eq!(replacement.suggested_name, "bad�");
    }

    #[test]
    fn invalid_authorities_have_fixed_safe_errors() {
        let private = "private-marker.invalid";
        let cases = [
            (
                format!("vless://not-a-uuid@{private}:443"),
                VlessAuthorityError::InvalidUserId,
            ),
            (
                format!("vless://{UUID}:password@{private}:443"),
                VlessAuthorityError::PasswordNotAllowed,
            ),
            (
                format!("vless://{UUID}@{private}"),
                VlessAuthorityError::MissingServerPort,
            ),
            (
                format!("vless://{UUID}@{private}:bad"),
                VlessAuthorityError::InvalidLink,
            ),
            (
                format!("vless://{UUID}@[not-ipv6]:443"),
                VlessAuthorityError::InvalidLink,
            ),
        ];
        for (input, expected) in cases {
            let error = parse_vless_authority(&input).expect_err("invalid authority");
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(private));
            assert!(error.to_string().len() <= 80);
        }
    }

    #[test]
    fn uri_and_input_bounds_are_exact() {
        let prefix = format!("vless://{UUID}@example.invalid:443#");
        let legal = prefix.clone() + &"x".repeat(MAX_VLESS_URI_BYTES - prefix.len());
        assert!(parse_vless_authority(&legal).is_ok());
        assert_eq!(legal.len(), MAX_VLESS_URI_BYTES);
        let oversized = legal + "x";
        assert_eq!(
            parse_vless_authority(&oversized),
            Err(VlessAuthorityError::InvalidInput)
        );
        assert_eq!(
            parse_vless_authority_bytes(&[0xff]),
            Err(VlessAuthorityError::InvalidInput)
        );
    }
}
