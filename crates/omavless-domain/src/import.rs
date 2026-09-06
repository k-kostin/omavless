// SPDX-License-Identifier: MIT

use omavless_profile::canonical::{CanonicalError, CanonicalProfile, parse_canonical};
use omavless_profile::{Protocol, classify_protocol};
use std::fmt;
use std::net::{IpAddr, Ipv6Addr};

pub const MAX_IMPORT_BYTES: usize = 64 * 1024;
pub const MAX_SUBSCRIPTION_URL_BYTES: usize = 8 * 1024;

/// Parsed confirmation input, not permission to import or fetch anything.
/// Debug deliberately uses the canonical profile's redacted representation.
#[derive(Debug)]
pub enum ImportPreview {
    Profile(CanonicalProfile),
    Subscription { duplicate: bool },
}

impl ImportPreview {
    /// Existing version-1 UI shape. Profile metadata is PRIVATE (names,
    /// endpoint, SNI and a masked hint); it is not a diagnostics projection.
    #[must_use]
    pub fn private_ui_value(&self) -> serde_json::Value {
        match self {
            Self::Profile(profile) => serde_json::json!({
                "version": 1, "kind": "profile",
                "profile": profile.private_preview(),
            }),
            Self::Subscription { duplicate } => serde_json::json!({
                "version": 1, "kind": "subscription",
                "suggestedName": "Subscription", "duplicate": duplicate,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPreviewError {
    Input(ImportError),
    Profile(CanonicalError),
}

impl fmt::Display for ImportPreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Profile(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ImportPreviewError {}

/// Same pure boundary for clipboard text and already bounded file contents.
/// The caller supplies its authoritative store snapshot for duplicate checks.
/// No filesystem, network, store mutation, or lifecycle effect.
pub fn preview_import(
    input: &str,
    existing_urls: &[String],
) -> Result<ImportPreview, ImportPreviewError> {
    match classify_import(input, existing_urls).map_err(ImportPreviewError::Input)? {
        ImportKind::Profile(_) => parse_canonical(input.trim())
            .map(ImportPreview::Profile)
            .map_err(ImportPreviewError::Profile),
        ImportKind::Subscription { duplicate } => Ok(ImportPreview::Subscription { duplicate }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Profile(Protocol),
    Subscription { duplicate: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    Empty,
    Ambiguous,
    InvalidSubscription,
}
impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "Import input must not be empty",
            Self::Ambiguous => "Import one profile link or subscription URL at a time",
            Self::InvalidSubscription => {
                "Input is not a supported profile link or valid subscription URL"
            }
        })
    }
}
impl std::error::Error for ImportError {}

fn authority(url: &str) -> Option<(&str, &str)> {
    let (scheme, remainder) = url.split_once("://")?;
    let end = remainder.find(['/', '?']).unwrap_or(remainder.len());
    Some((scheme, &remainder[..end]))
}

fn bracket_host(value: &str) -> bool {
    if let Some((version, address)) = value.split_once('.')
        && version.strip_prefix(['v', 'V']).is_some_and(|version| {
            !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return !address.is_empty()
            && address.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'-' | b'.'
                            | b'_'
                            | b'~'
                            | b'!'
                            | b'$'
                            | b'&'
                            | b'\''
                            | b'('
                            | b')'
                            | b'*'
                            | b'+'
                            | b','
                            | b';'
                            | b'='
                            | b':'
                    )
            });
    }
    if let Some((address, zone)) = value.split_once('%') {
        return !zone.is_empty() && !zone.contains('%') && address.parse::<Ipv6Addr>().is_ok();
    }
    value.parse::<Ipv6Addr>().is_ok()
}

fn valid_port(value: Option<&str>) -> bool {
    value.is_none_or(|value| value.is_empty() || value.parse::<u16>().is_ok_and(|port| port > 0))
}

pub fn valid_subscription_url(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_SUBSCRIPTION_URL_BYTES
        || value.bytes().any(|byte| byte <= b' ' || byte == 0x7f)
        || value.contains('#')
    {
        return false;
    }
    let Some((scheme, authority)) = authority(value) else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return false;
    }
    // The legacy Python fetch path cannot reliably send raw Unicode
    // authorities. Require their ASCII/punycode spelling at this native
    // boundary and avoid Unicode NFKC delimiter ambiguities.
    if authority.is_empty() || !authority.is_ascii() || authority.contains('@') {
        return false;
    }
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let Some((host, suffix)) = rest.split_once(']') else {
            return false;
        };
        if !bracket_host(host) {
            return false;
        }
        let port = if suffix.is_empty() {
            None
        } else {
            let Some(port) = suffix.strip_prefix(':') else {
                return false;
            };
            Some(port)
        };
        (host, port)
    } else if authority.matches(':').count() <= 1 {
        if authority.contains(['[', ']']) {
            return false;
        }
        authority
            .split_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)))
    } else {
        return false;
    };
    if host.is_empty() || !valid_port(port) {
        return false;
    }
    if scheme.eq_ignore_ascii_case("http") {
        let lower = host.to_ascii_lowercase();
        let loopback = lower == "localhost"
            || lower.ends_with(".localhost")
            || host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if !loopback {
            return false;
        }
    }
    true
}

pub fn classify_import(input: &str, existing_urls: &[String]) -> Result<ImportKind, ImportError> {
    if input.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::InvalidSubscription);
    }
    let value = input.trim();
    if value.is_empty() {
        return Err(ImportError::Empty);
    }
    let tokens: Vec<_> = value.split_whitespace().collect();
    let supported: Vec<_> = tokens
        .iter()
        .filter_map(|token| classify_protocol(token).ok())
        .collect();
    if let [protocol] = supported.as_slice() {
        return if tokens.len() == 1 {
            Ok(ImportKind::Profile(*protocol))
        } else {
            Err(ImportError::Ambiguous)
        };
    }
    if !supported.is_empty() || tokens.len() != 1 {
        return Err(ImportError::Ambiguous);
    }
    if !valid_subscription_url(value) {
        return Err(ImportError::InvalidSubscription);
    }
    Ok(ImportKind::Subscription {
        duplicate: existing_urls.iter().any(|url| url == value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_and_strict_subscription_paths_are_distinct() {
        assert_eq!(
            classify_import("vless://opaque.invalid", &[]),
            Ok(ImportKind::Profile(Protocol::Vless))
        );
        assert_eq!(
            classify_import(
                "https://example.invalid/sub",
                &["https://example.invalid/sub".into()]
            ),
            Ok(ImportKind::Subscription { duplicate: true })
        );
        assert!(!valid_subscription_url("http://example.invalid/sub"));
        assert!(valid_subscription_url("http://127.0.0.1:8080/sub"));
        assert_eq!(
            classify_import("vless://one trojan://two", &[]),
            Err(ImportError::Ambiguous)
        );
    }
    #[test]
    fn errors_never_echo_private_input() {
        let error = classify_import("https://user:secret@private.invalid/sub", &[]).unwrap_err();
        assert!(!error.to_string().contains("secret"));
        assert!(!error.to_string().contains("private.invalid"));
    }

    #[test]
    fn native_subscription_authorities_are_ascii_and_ipvfuture_is_rfc_bounded() {
        assert!(valid_subscription_url(
            "https://[v1.a:b-c._~!$&'()*+,;=]/sub"
        ));
        assert!(!valid_subscription_url("https://[v1.a%b]/sub"));
        assert!(!valid_subscription_url("https://example.invalid／sub"));
    }
}
