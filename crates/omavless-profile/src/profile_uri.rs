// SPDX-License-Identifier: MIT

//! Shared bounded URI helpers for pure R2 profile adapters.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::vless::HostKind;

pub(crate) const MAX_PROFILE_URI_BYTES: usize = 16 * 1024;
pub(crate) const MAX_QUERY_FIELDS: usize = 128;

pub(crate) struct UriParts<'a> {
    pub scheme: &'a str,
    pub authority: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub fragment: &'a str,
}

pub(crate) fn extract_uri<'a>(input: &'a str, schemes: &[&str]) -> Result<&'a str, ()> {
    for token in input.split_whitespace() {
        let Some((scheme, _)) = token.split_once("://") else {
            continue;
        };
        if schemes
            .iter()
            .any(|expected| scheme.eq_ignore_ascii_case(expected))
        {
            return (token.len() <= MAX_PROFILE_URI_BYTES)
                .then_some(token)
                .ok_or(());
        }
    }
    Err(())
}

pub(crate) fn split_uri(value: &str) -> Result<UriParts<'_>, ()> {
    let (scheme, remainder) = value.split_once("://").ok_or(())?;
    let (before_fragment, fragment) = remainder.split_once('#').unwrap_or((remainder, ""));
    let (before_query, query) = before_fragment
        .split_once('?')
        .unwrap_or((before_fragment, ""));
    let authority_end = before_query.find('/').unwrap_or(before_query.len());
    Ok(UriParts {
        scheme,
        authority: &before_query[..authority_end],
        path: &before_query[authority_end..],
        query,
        fragment,
    })
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_decode_bytes(value: &str, plus_as_space: bool) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2]))
        {
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(if plus_as_space && bytes[index] == b'+' {
                b' '
            } else {
                bytes[index]
            });
            index += 1;
        }
    }
    decoded
}

pub(crate) fn percent_decode(value: &str) -> String {
    String::from_utf8_lossy(&percent_decode_bytes(value, false)).into_owned()
}

pub(crate) fn percent_decode_strict(value: &str, plus_as_space: bool) -> Result<String, ()> {
    String::from_utf8(percent_decode_bytes(value, plus_as_space)).map_err(|_| ())
}

pub(crate) fn query_pairs(value: &str, strict_utf8: bool) -> Result<Vec<(String, String)>, ()> {
    let fields = value.split('&').filter(|field| !field.is_empty());
    let mut result = Vec::new();
    for field in fields {
        if result.len() >= MAX_QUERY_FIELDS {
            return Err(());
        }
        let (key, value) = field.split_once('=').unwrap_or((field, ""));
        let decode = |item: &str| {
            if strict_utf8 {
                percent_decode_strict(item, true)
            } else {
                Ok(String::from_utf8_lossy(&percent_decode_bytes(item, true)).into_owned())
            }
        };
        result.push((decode(key)?, decode(value)?));
    }
    Ok(result)
}

pub(crate) fn parse_endpoint(
    value: &str,
    default_port: Option<u16>,
) -> Result<(String, u16, HostKind), ()> {
    let (host, port_text, kind) = if let Some(bracketed) = value.strip_prefix('[') {
        let close = bracketed.find(']').ok_or(())?;
        let host = &bracketed[..close];
        let suffix = &bracketed[close + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            Some(suffix.strip_prefix(':').ok_or(())?)
        };
        let address = Ipv6Addr::from_str(host).map_err(|_| ())?;
        (address.to_string(), port, HostKind::Ipv6)
    } else {
        let (host, port) = value
            .rsplit_once(':')
            .map_or((value, None), |(host, port)| {
                if host.contains(':') {
                    (value, None)
                } else {
                    (host, Some(port))
                }
            });
        if host.contains(':') {
            return Err(());
        }
        let (host, kind) = if let Ok(address) = Ipv4Addr::from_str(host) {
            (address.to_string(), HostKind::Ipv4)
        } else {
            (host.to_lowercase(), HostKind::Dns)
        };
        (host, port, kind)
    };
    if host.is_empty() {
        return Err(());
    }
    let port = match port_text {
        Some(value) => value
            .parse::<u16>()
            .ok()
            .filter(|value| *value != 0)
            .ok_or(())?,
        None => default_port.ok_or(())?,
    };
    Ok((host, port, kind))
}

pub(crate) fn canonical_host(value: &str) -> Result<(String, HostKind), ()> {
    if value.is_empty()
        || value.len() > 1024
        || value.chars().any(|character| {
            character <= ' ' || character == '\u{7f}' || "/?#@".contains(character)
        })
    {
        return Err(());
    }
    if let Ok(address) = Ipv4Addr::from_str(value) {
        return Ok((address.to_string(), HostKind::Ipv4));
    }
    if let Ok(address) = Ipv6Addr::from_str(value) {
        return Ok((address.to_string(), HostKind::Ipv6));
    }
    let ascii = idna::domain_to_ascii(value.trim_end_matches('.')).map_err(|_| ())?;
    let host = ascii.to_lowercase();
    if host.is_empty()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label.as_bytes()[0].is_ascii_alphanumeric()
                || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(());
    }
    Ok((host, HostKind::Dns))
}

pub(crate) fn sanitize_label(value: &str) -> String {
    percent_decode(value)
        .chars()
        .filter(|character| !matches!(*character as u32, 0x00..=0x1f | 0x7f))
        .collect::<String>()
        .trim()
        .chars()
        .take(80)
        .collect()
}

pub(crate) fn quote(value: &str, plus_for_space: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~') {
            result.push(char::from(*byte));
        } else if plus_for_space && *byte == b' ' {
            result.push('+');
        } else {
            result.push('%');
            result.push(char::from(HEX[usize::from(byte >> 4)]));
            result.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    result
}

pub(crate) fn encoded_query(mut pairs: Vec<(String, String)>) -> String {
    pairs.sort();
    pairs
        .into_iter()
        .map(|(key, value)| format!("{}={}", quote(&key, true), quote(&value, true)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(crate) fn text_is_bounded(value: &str, limit: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.len() <= limit
        && !value
            .chars()
            .any(|character| matches!(character as u32, 0x00..=0x1f | 0x7f))
}

pub(crate) fn canonical_uuid(value: &str) -> Option<String> {
    let value = value
        .strip_prefix('{')
        .and_then(|item| item.strip_suffix('}'))
        .unwrap_or(value);
    let digits = value
        .chars()
        .filter(|character| *character != '-')
        .collect::<String>();
    if digits.len() != 32 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let digits = digits.to_lowercase();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &digits[..8],
        &digits[8..12],
        &digits[12..16],
        &digits[16..20],
        &digits[20..]
    ))
}
