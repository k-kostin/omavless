// SPDX-License-Identifier: MIT

//! Bounded, credential-private subscription feed decoding.
//!
//! The response body and accepted profile links are bearer-like private data.
//! They deliberately have no `Debug`, `Clone`, display or serialization
//! implementation. Only fixed error codes and bounded counts may cross a
//! public control boundary.

use base64::Engine;
use base64::alphabet;
use base64::engine::{GeneralPurpose, GeneralPurposeConfig};
use omavless_profile::canonical::{CanonicalError, parse_canonical};
use std::collections::BTreeSet;
use std::fmt;

pub const MAX_SUBSCRIPTION_FEED_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_SUBSCRIPTION_CANDIDATES: usize = 1024;

// Python's base64.b64decode(validate=True), used by the current production
// owner, accepts non-zero unused trailing bits. Preserve that legacy decoding
// behavior while keeping alphabet and padding validation strict.
const PYTHON_BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionFeedError {
    TooLarge,
    InvalidUtf8,
    TooManyCandidates,
    NoSupportedProfiles,
}

impl SubscriptionFeedError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "subscription_feed_too_large",
            Self::InvalidUtf8 => "subscription_feed_invalid_utf8",
            Self::TooManyCandidates => "too_many_subscription_entries",
            Self::NoSupportedProfiles => "subscription_contains_no_supported_profiles",
        }
    }
}

impl fmt::Display for SubscriptionFeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Subscription response is too large",
            Self::InvalidUtf8 => "Subscription response is not UTF-8 text",
            Self::TooManyCandidates => "Subscription contains too many supported links",
            Self::NoSupportedProfiles => "Subscription contains no supported profiles",
        })
    }
}

impl std::error::Error for SubscriptionFeedError {}

/// Credential-free facts suitable for parity reports and future responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionFeedCounts {
    pub accepted: usize,
    pub skipped: usize,
}

/// Already-fetched private response bytes. Construction applies the exact
/// body bound before the decoder can retain or inspect the payload. This
/// bearer-like value deliberately has no formatting or cloning support.
pub struct PrivateSubscriptionBody(Vec<u8>);

impl PrivateSubscriptionBody {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SubscriptionFeedError> {
        if bytes.len() > MAX_SUBSCRIPTION_FEED_BYTES {
            return Err(SubscriptionFeedError::TooLarge);
        }
        Ok(Self(bytes))
    }
}

struct PrivateFeedEntry {
    uri: String,
}

/// Validated private feed contents. This type must remain non-formatable.
pub struct DecodedSubscriptionFeed {
    entries: Vec<PrivateFeedEntry>,
    skipped: usize,
}

impl DecodedSubscriptionFeed {
    #[must_use]
    pub fn counts(&self) -> SubscriptionFeedCounts {
        SubscriptionFeedCounts {
            accepted: self.entries.len(),
            skipped: self.skipped,
        }
    }

    /// Bind owner-generated record IDs after parsing. The returned URIs and
    /// IDs stay private and are consumed only by the store mutation boundary.
    pub fn into_private_entries(
        self,
        mut next_record_id: impl FnMut() -> String,
    ) -> Vec<crate::private_store::IncomingSubscriptionProfile> {
        self.entries
            .into_iter()
            .map(|entry| crate::private_store::IncomingSubscriptionProfile {
                uri: entry.uri,
                new_id: next_record_id(),
            })
            .collect()
    }
}

fn supported_scheme(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "vless" | "trojan" | "hysteria2" | "hy2" | "tuic"
    )
}

fn python_whitespace(character: char) -> bool {
    character.is_whitespace() || matches!(character, '\u{1c}'..='\u{1f}')
}

fn candidates(input: &str) -> Result<Vec<&str>, SubscriptionFeedError> {
    let bytes = input.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_alphabetic() {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'+' | b'.' | b'-'))
        {
            index += 1;
        }
        if bytes.get(index..index + 3) != Some(b"://") {
            index = start + 1;
            continue;
        }
        let scheme = &input[start..index];
        let mut end = index + 3;
        while end < bytes.len() {
            let Some(character) = input[end..].chars().next() else {
                break;
            };
            if python_whitespace(character) || matches!(character, '"' | '\'' | '<' | '>') {
                break;
            }
            end += character.len_utf8();
        }
        if supported_scheme(scheme) {
            result.push(&input[start..end]);
            if result.len() > MAX_SUBSCRIPTION_CANDIDATES {
                return Err(SubscriptionFeedError::TooManyCandidates);
            }
        }
        index = end.max(start + 1);
    }
    Ok(result)
}

fn decoded_text(input: &str) -> Option<String> {
    let compact: String = input
        .chars()
        .filter(|character| !python_whitespace(*character))
        .collect();
    if compact.is_empty()
        || !compact.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'+' | b'/' | b'=' | b'-')
        })
    {
        return None;
    }
    let mut normalized = compact.replace('-', "+").replace('_', "/");
    normalized.extend(std::iter::repeat_n('=', (4 - normalized.len() % 4) % 4));
    let decoded = PYTHON_BASE64.decode(normalized).ok()?;
    let text = std::str::from_utf8(
        decoded
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(&decoded),
    )
    .ok()?;
    Some(text.to_owned())
}

fn parse_candidate(uri: &str) -> Result<(String, String), CanonicalError> {
    let canonical = parse_canonical(uri)?;
    Ok((uri.to_owned(), canonical.subscription_identity()))
}

/// Decode one already bounded transport body using the legacy raw-or-base64
/// semantics. Unsupported schemes are ignored. Invalid supported links and
/// canonical duplicates contribute only to the bounded skipped count.
pub fn decode_subscription_feed(
    body: PrivateSubscriptionBody,
) -> Result<DecodedSubscriptionFeed, SubscriptionFeedError> {
    let body = body.0;
    let body = body.as_slice();
    let body = body.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(body);
    let text = std::str::from_utf8(body).map_err(|_| SubscriptionFeedError::InvalidUtf8)?;
    let raw_candidates = candidates(text)?;
    let decoded;
    let candidates = if raw_candidates.is_empty() {
        decoded = decoded_text(text);
        match decoded.as_deref() {
            Some(decoded) => candidates(decoded)?,
            None => Vec::new(),
        }
    } else {
        raw_candidates
    };
    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(candidates.len());
    let mut skipped = 0;
    for candidate in candidates {
        match parse_candidate(candidate) {
            Ok((uri, identity)) if seen.insert(identity.clone()) => {
                entries.push(PrivateFeedEntry { uri });
            }
            Ok(_) | Err(_) => skipped += 1,
        }
    }
    if entries.is_empty() {
        return Err(SubscriptionFeedError::NoSupportedProfiles);
    }
    Ok(DecodedSubscriptionFeed { entries, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

    const URI: &str =
        "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example";

    #[test]
    fn raw_base64_and_urlsafe_bodies_decode_identically() {
        for body in [
            URI.as_bytes().to_vec(),
            STANDARD.encode(URI).into_bytes(),
            URL_SAFE_NO_PAD.encode(URI).into_bytes(),
        ] {
            assert_eq!(
                decode_subscription_feed(PrivateSubscriptionBody::from_bytes(body).unwrap())
                    .unwrap()
                    .counts(),
                SubscriptionFeedCounts {
                    accepted: 1,
                    skipped: 0,
                }
            );
        }
        let unicode_uri = URI.replace("#Example", "#🚀");
        let standard = STANDARD.encode(&unicode_uri);
        let urlsafe = URL_SAFE_NO_PAD.encode(&unicode_uri);
        assert_ne!(standard, urlsafe);
        assert_eq!(
            decode_subscription_feed(
                PrivateSubscriptionBody::from_bytes(urlsafe.into_bytes()).unwrap(),
            )
            .unwrap()
            .counts(),
            SubscriptionFeedCounts {
                accepted: 1,
                skipped: 0,
            }
        );
    }

    #[test]
    fn bom_duplicate_invalid_and_unsupported_are_bounded() {
        let body = format!("\u{feff}{URI}\n{URI}\nvless://broken\nss://ignored");
        assert_eq!(
            decode_subscription_feed(
                PrivateSubscriptionBody::from_bytes(body.into_bytes()).unwrap(),
            )
            .unwrap()
            .counts(),
            SubscriptionFeedCounts {
                accepted: 1,
                skipped: 2,
            }
        );
    }

    #[test]
    fn exact_body_and_candidate_bounds_are_enforced() {
        assert_eq!(
            PrivateSubscriptionBody::from_bytes(vec![b'x'; MAX_SUBSCRIPTION_FEED_BYTES + 1])
                .err()
                .unwrap(),
            SubscriptionFeedError::TooLarge
        );
        let body = std::iter::repeat_n(URI, MAX_SUBSCRIPTION_CANDIDATES + 1)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            decode_subscription_feed(
                PrivateSubscriptionBody::from_bytes(body.into_bytes()).unwrap(),
            )
            .err()
            .unwrap(),
            SubscriptionFeedError::TooManyCandidates
        );
        let maximum = std::iter::repeat_n(URI, MAX_SUBSCRIPTION_CANDIDATES)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            decode_subscription_feed(
                PrivateSubscriptionBody::from_bytes(maximum.into_bytes()).unwrap(),
            )
            .unwrap()
            .counts(),
            SubscriptionFeedCounts {
                accepted: 1,
                skipped: MAX_SUBSCRIPTION_CANDIDATES - 1,
            }
        );
    }

    #[test]
    fn errors_and_public_facts_never_echo_private_input() {
        for body in [b"\xff".as_slice(), b"private-provider-password"] {
            let error = decode_subscription_feed(
                PrivateSubscriptionBody::from_bytes(body.to_vec()).unwrap(),
            )
            .err()
            .unwrap();
            let public = format!("{error:?} {error}");
            assert!(!public.contains("private-provider"));
            assert!(!public.contains("password"));
        }
    }
}
