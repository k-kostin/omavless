// SPDX-License-Identifier: MIT

//! Canonical pure profile-domain types for the incremental Rust migration.
//!
//! R2a owns protocol classification; later bounded slices own VLESS authority,
//! public preview and query-metadata semantics. None of these slices accesses
//! the private store, renders Mihomo configuration, or enters the production
//! runtime path.

use std::fmt;

mod base64url;
pub mod vless;
pub mod vless_encryption;
pub mod vless_query;

pub const MAX_CLASSIFICATION_INPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Protocol {
    Vless,
    Trojan,
    Hysteria2,
    Tuic,
}

impl Protocol {
    pub const ALL: [Self; 4] = [Self::Vless, Self::Trojan, Self::Hysteria2, Self::Tuic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vless => "vless",
            Self::Trojan => "trojan",
            Self::Hysteria2 => "hysteria2",
            Self::Tuic => "tuic",
        }
    }

    #[must_use]
    pub fn from_scheme(scheme: &str) -> Option<Self> {
        if scheme.eq_ignore_ascii_case("vless") {
            Some(Self::Vless)
        } else if scheme.eq_ignore_ascii_case("trojan") {
            Some(Self::Trojan)
        } else if scheme.eq_ignore_ascii_case("hysteria2") || scheme.eq_ignore_ascii_case("hy2") {
            Some(Self::Hysteria2)
        } else if scheme.eq_ignore_ascii_case("tuic") {
            Some(Self::Tuic)
        } else {
            None
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassificationError {
    InvalidInput,
    UnsupportedProtocol,
    MissingProfileLink,
}

impl ClassificationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::UnsupportedProtocol => "unsupported_protocol",
            Self::MissingProfileLink => "missing_profile_link",
        }
    }
}

impl fmt::Display for ClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "Profile input is invalid",
            Self::UnsupportedProtocol => "That profile protocol is not supported",
            Self::MissingProfileLink => "Input does not contain a supported profile link",
        })
    }
}

impl std::error::Error for ClassificationError {}

fn uri_scheme(token: &str) -> Option<&str> {
    let bytes = token.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    let mut end = 1;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'+' | b'.' | b'-'))
    {
        end += 1;
    }
    if bytes.get(end..end + 3) == Some(b"://") {
        Some(&token[..end])
    } else {
        None
    }
}

pub fn classify_protocol(input: &str) -> Result<Protocol, ClassificationError> {
    if input.len() > MAX_CLASSIFICATION_INPUT_BYTES {
        return Err(ClassificationError::InvalidInput);
    }
    let mut saw_link = false;
    for token in input.split_whitespace() {
        let Some(scheme) = uri_scheme(token) else {
            continue;
        };
        saw_link = true;
        if let Some(protocol) = Protocol::from_scheme(scheme) {
            return Ok(protocol);
        }
    }
    if saw_link {
        Err(ClassificationError::UnsupportedProtocol)
    } else {
        Err(ClassificationError::MissingProfileLink)
    }
}

pub fn classify_protocol_bytes(input: &[u8]) -> Result<Protocol, ClassificationError> {
    if input.len() > MAX_CLASSIFICATION_INPUT_BYTES {
        return Err(ClassificationError::InvalidInput);
    }
    let input = std::str::from_utf8(input).map_err(|_| ClassificationError::InvalidInput)?;
    classify_protocol(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_protocols_and_aliases_are_stable() {
        let cases = [
            ("vless://opaque.invalid", Protocol::Vless),
            ("TROJAN://opaque.invalid", Protocol::Trojan),
            ("hysteria2://opaque.invalid", Protocol::Hysteria2),
            ("hy2://opaque.invalid", Protocol::Hysteria2),
            ("tuic://opaque.invalid", Protocol::Tuic),
        ];
        for (input, expected) in cases {
            assert_eq!(classify_protocol(input), Ok(expected));
        }
        assert_eq!(
            Protocol::ALL.map(Protocol::as_str),
            ["vless", "trojan", "hysteria2", "tuic"]
        );
    }

    #[test]
    fn first_supported_link_matches_the_reference_semantics() {
        assert_eq!(
            classify_protocol("https://docs.invalid vless://opaque.invalid trojan://later.invalid"),
            Ok(Protocol::Vless)
        );
        assert_eq!(
            classify_protocol("unknown://private.invalid hy2://opaque.invalid"),
            Ok(Protocol::Hysteria2)
        );
    }

    #[test]
    fn errors_are_fixed_and_credential_safe() {
        let unsupported = classify_protocol("unknown://private-secret.invalid")
            .expect_err("unsupported protocol");
        assert_eq!(unsupported, ClassificationError::UnsupportedProtocol);
        assert!(!unsupported.to_string().contains("private-secret"));
        assert_eq!(
            classify_protocol("ordinary text"),
            Err(ClassificationError::MissingProfileLink)
        );
        assert_eq!(
            classify_protocol_bytes(&[0xff]),
            Err(ClassificationError::InvalidInput)
        );
    }

    #[test]
    fn input_bound_is_exact() {
        assert_eq!(
            classify_protocol(&"x".repeat(MAX_CLASSIFICATION_INPUT_BYTES)),
            Err(ClassificationError::MissingProfileLink)
        );
        assert_eq!(
            classify_protocol(&"x".repeat(MAX_CLASSIFICATION_INPUT_BYTES + 1)),
            Err(ClassificationError::InvalidInput)
        );
    }
}
