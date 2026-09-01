// SPDX-License-Identifier: MIT

use omavless_profile::{Protocol, classify_protocol};
use std::fmt;
use std::net::{IpAddr, Ipv6Addr};

pub const MAX_IMPORT_BYTES: usize = 64 * 1024;
pub const MAX_SUBSCRIPTION_URL_BYTES: usize = 8 * 1024;

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
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let Some((host, suffix)) = rest.split_once(']') else {
            return false;
        };
        if host.parse::<Ipv6Addr>().is_err() {
            return false;
        }
        (host, suffix.strip_prefix(':'))
    } else if authority.matches(':').count() <= 1 {
        authority
            .split_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)))
    } else {
        return false;
    };
    if host.is_empty() || port.is_some_and(|port| port.parse::<u16>().is_err() || port == "0") {
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
}
