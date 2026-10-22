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
pub use omavless_profile::import_reason::{ImportReason, ImportReasons};
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
    Rejected(ImportSummary),
}

impl SubscriptionFeedError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "subscription_feed_too_large",
            Self::InvalidUtf8 => "subscription_feed_invalid_utf8",
            Self::TooManyCandidates => "too_many_subscription_entries",
            Self::NoSupportedProfiles | Self::Rejected(_) => {
                "subscription_contains_no_supported_profiles"
            }
        }
    }
}

impl fmt::Display for SubscriptionFeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Subscription response is too large",
            Self::InvalidUtf8 => "Subscription response is not UTF-8 text",
            Self::TooManyCandidates => "Subscription contains too many supported links",
            Self::NoSupportedProfiles | Self::Rejected(_) => {
                "Subscription contains no supported profiles"
            }
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

/// Safe outcome facts, held only with the original volatile operation result.
/// `skipped` excludes duplicates, consistently for feeds and local bundles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub accepted: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub reasons: ImportReasons,
}
impl ImportSummary {
    #[must_use]
    pub fn rejected(reason: ImportReason) -> Self {
        let mut reasons = ImportReasons::default();
        reasons.record(reason);
        Self {
            accepted: 0,
            duplicates: 0,
            skipped: 1,
            reasons,
        }
    }
    #[must_use]
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let object = value.as_object()?;
        if object.len() != 4 {
            return None;
        }
        let count = |key: &str| {
            usize::try_from(value[key].as_u64()?)
                .ok()
                .filter(|n| *n <= 65536)
        };
        let accepted = count("accepted")?;
        let duplicates = count("duplicates")?;
        let skipped = count("skipped")?;
        if accepted + duplicates + skipped > 65536 {
            return None;
        }
        let reasons = ImportReasons::from_value(&value["skipReasons"], duplicates + skipped)?;
        if value["skipReasons"]["duplicate"].as_u64().unwrap_or(0) != duplicates as u64 {
            return None;
        }
        Some(Self {
            accepted,
            duplicates,
            skipped,
            reasons,
        })
    }
    pub fn merge(&mut self, other: Self) {
        self.accepted += other.accepted;
        self.duplicates += other.duplicates;
        self.skipped += other.skipped;
        self.reasons.merge(other.reasons);
    }
    #[must_use]
    pub fn value(self) -> serde_json::Value {
        serde_json::json!({"accepted":self.accepted,"duplicates":self.duplicates,
            "skipped":self.skipped,"skipReasons":self.reasons.value()})
    }
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
    duplicates: usize,
    reasons: omavless_profile::import_reason::ImportReasons,
}

impl DecodedSubscriptionFeed {
    #[must_use]
    pub fn import_summary(&self) -> ImportSummary {
        ImportSummary {
            accepted: self.entries.len(),
            duplicates: self.duplicates,
            skipped: self.skipped - self.duplicates,
            reasons: self.reasons,
        }
    }
    #[must_use]
    pub fn skip_reasons(&self) -> omavless_profile::import_reason::ImportReasons {
        self.reasons
    }
    pub(crate) fn duplicate_count(&self) -> usize {
        self.duplicates
    }

    pub(crate) fn into_private_uris(self) -> impl Iterator<Item = String> {
        self.entries.into_iter().map(|entry| entry.uri)
    }
    #[must_use]
    pub fn counts(&self) -> SubscriptionFeedCounts {
        SubscriptionFeedCounts {
            accepted: self.entries.len(),
            skipped: self.skipped,
        }
    }

    /// Aggregate size of retained private URI data. Refresh-all orchestration
    /// uses this count to cap memory across feeds without exposing contents.
    #[must_use]
    pub fn private_payload_bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.uri.len()).sum()
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
    candidates_with_scan_work(input, || {})
}

// The observer lets tests bound actual scanner iterations without timing
// assumptions or an unbounded regression run. Production uses a no-op.
fn candidates_with_scan_work(
    input: &str,
    mut scan_step: impl FnMut(),
) -> Result<Vec<&str>, SubscriptionFeedError> {
    let bytes = input.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        scan_step();
        if !bytes[index].is_ascii_alphabetic() {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'+' | b'.' | b'-'))
        {
            scan_step();
            index += 1;
        }
        if bytes.get(index..index + 3) != Some(b"://") {
            // Every alphabetic suffix of this maximal scheme run reaches the
            // same delimiter. None can match :// if the whole run did not.
            // Keep the cursor here instead of rescanning all of its suffixes.
            continue;
        }
        let scheme = &input[start..index];
        let mut end = index + 3;
        while end < bytes.len() {
            scan_step();
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
    decode_candidates(candidates)
}

fn decode_candidates(
    candidates: Vec<&str>,
) -> Result<DecodedSubscriptionFeed, SubscriptionFeedError> {
    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(candidates.len());
    let mut skipped = 0;
    let mut duplicates = 0;
    let mut reasons = omavless_profile::import_reason::ImportReasons::default();
    for candidate in candidates {
        match parse_candidate(candidate) {
            Ok((uri, identity)) if seen.insert(identity.clone()) => {
                entries.push(PrivateFeedEntry { uri });
            }
            Ok(_) => {
                skipped += 1;
                duplicates += 1;
                reasons.record(omavless_profile::import_reason::ImportReason::Duplicate);
            }
            Err(error) => {
                skipped += 1;
                reasons
                    .record(omavless_profile::import_reason::ImportReason::from_canonical(error));
            }
        }
    }
    if entries.is_empty() {
        if skipped > 0 {
            return Err(SubscriptionFeedError::Rejected(ImportSummary {
                accepted: 0,
                duplicates,
                skipped: skipped - duplicates,
                reasons,
            }));
        }
        return Err(SubscriptionFeedError::NoSupportedProfiles);
    }
    Ok(DecodedSubscriptionFeed {
        entries,
        skipped,
        duplicates,
        reasons,
    })
}

/// Local input is a list of complete links, not a provider document in which
/// links can be embedded. Reuse the feed's Base64 and canonical identity rules,
/// but refuse prose, JSON/YAML and mixed subscription URLs as a whole.
pub fn decode_local_profile_lines(
    input: &str,
) -> Result<DecodedSubscriptionFeed, SubscriptionFeedError> {
    if input.len() > 32 * 1024 {
        return Err(SubscriptionFeedError::TooLarge);
    }
    let raw = input.trim_start_matches('\u{feff}').trim();
    let decoded;
    let text = if raw.contains("://") {
        raw
    } else {
        decoded = decoded_text(raw).ok_or(SubscriptionFeedError::NoSupportedProfiles)?;
        &decoded
    };
    let mut candidates = Vec::new();
    let mut unsupported = 0;
    let mut count = 0;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        count += 1;
        if count > MAX_SUBSCRIPTION_CANDIDATES {
            return Err(SubscriptionFeedError::TooManyCandidates);
        }
        let (scheme, _) = line
            .split_once("://")
            .ok_or(SubscriptionFeedError::NoSupportedProfiles)?;
        if !scheme
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic())
            || !scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
            || scheme.eq_ignore_ascii_case("http")
            || scheme.eq_ignore_ascii_case("https")
            || line
                .split('#')
                .next()
                .unwrap_or(line)
                .chars()
                .any(char::is_whitespace)
            || line.chars().any(char::is_control)
        {
            return Err(SubscriptionFeedError::NoSupportedProfiles);
        }
        // A second whitespace-delimited link is ambiguous, but :// inside a
        // path or display label is not a second item. Canonical parsers accept
        // encoded fragments; encode only raw label whitespace so their ordinary
        // single-link extraction does not truncate a multi-word display name.
        if line.split_whitespace().skip(1).any(|part| {
            part.split_once("://").is_some_and(|(scheme, _)| {
                supported_scheme(scheme)
                    || scheme.eq_ignore_ascii_case("http")
                    || scheme.eq_ignore_ascii_case("https")
            })
        }) {
            return Err(SubscriptionFeedError::NoSupportedProfiles);
        }
        if supported_scheme(scheme) {
            let normalized = match line.split_once('#') {
                Some((link, label)) => {
                    let mut result = format!("{link}#");
                    for character in label.chars() {
                        if character.is_whitespace() {
                            use std::fmt::Write;
                            for byte in character.encode_utf8(&mut [0; 4]).bytes() {
                                write!(result, "%{byte:02X}").expect("String write");
                            }
                        } else {
                            result.push(character);
                        }
                    }
                    result
                }
                None => line.to_owned(),
            };
            candidates.push(normalized);
        } else {
            unsupported += 1;
        }
    }
    if candidates.is_empty() && unsupported > 0 {
        let mut reasons = ImportReasons::default();
        for _ in 0..unsupported {
            reasons.record(ImportReason::UnsupportedProtocol);
        }
        return Err(SubscriptionFeedError::Rejected(ImportSummary {
            accepted: 0,
            duplicates: 0,
            skipped: unsupported,
            reasons,
        }));
    }
    let mut feed = decode_candidates(candidates.iter().map(String::as_str).collect())?;
    feed.skipped += unsupported;
    for _ in 0..unsupported {
        feed.reasons
            .record(omavless_profile::import_reason::ImportReason::UnsupportedProtocol);
    }
    Ok(feed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};

    const URI: &str =
        "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example";

    fn candidates_with_linear_budget(input: &str) -> Result<Vec<&str>, SubscriptionFeedError> {
        let mut remaining = input.len().saturating_mul(2).saturating_add(1);
        candidates_with_scan_work(input, || {
            remaining = remaining
                .checked_sub(1)
                .expect("subscription scanner exceeded linear work budget");
        })
    }

    #[test]
    fn delimiter_free_scheme_runs_use_linear_work_at_the_body_limit() {
        // A restored suffix restart fails after O(n) observed steps rather
        // than spending quadratic time. No wall-clock threshold is involved.
        for size in [32, 4096, MAX_SUBSCRIPTION_FEED_BYTES] {
            let alphabetic = "a".repeat(size);
            assert!(
                candidates_with_linear_budget(&alphabetic)
                    .unwrap()
                    .is_empty()
            );
            let mixed = "a1+.-".repeat(size / 5);
            assert!(candidates_with_linear_budget(&mixed).unwrap().is_empty());
        }
        // Exercise the public raw/base64 path only after the same-sized
        // candidate scan has met its deterministic work bound above.
        let body =
            PrivateSubscriptionBody::from_bytes(vec![b'a'; MAX_SUBSCRIPTION_FEED_BYTES]).unwrap();
        assert_eq!(
            decode_subscription_feed(body).err().unwrap(),
            SubscriptionFeedError::NoSupportedProfiles
        );
    }

    #[test]
    fn failed_runs_preserve_delimiters_and_supported_scheme_boundaries() {
        for separator in [
            " ", "\u{1c}", "\u{2003}", "\u{feff}", "_", "/", ":", "\"", "'", "<", ">",
        ] {
            let text = format!("not-a-scheme{separator}{URI}");
            assert_eq!(candidates_with_linear_budget(&text).unwrap(), vec![URI]);
        }
        // A supported suffix inside a recognized unsupported scheme is not
        // a new candidate. Nor do links nested in its payload become entries.
        for text in [
            format!("prefix{URI}"),
            format!("unknown+{URI}"),
            format!("https://example.invalid/{URI}"),
        ] {
            assert!(candidates_with_linear_budget(&text).unwrap().is_empty());
        }
        for scheme in ["vless", "VLESS", "trojan", "hysteria2", "hy2", "tuic"] {
            let text = format!("{scheme}://synthetic");
            assert_eq!(
                candidates_with_linear_budget(&text).unwrap(),
                vec![text.as_str()]
            );
        }
        let text = format!("123+.-{URI}");
        assert_eq!(candidates_with_linear_budget(&text).unwrap(), vec![URI]);
    }

    #[test]
    fn long_failed_runs_preserve_raw_and_encoded_profile_feeds() {
        let text = format!("{}\u{2003}{URI}", "a".repeat(4096));
        assert_eq!(candidates_with_linear_budget(&text).unwrap(), vec![URI]);
        for encoded in [
            text.as_bytes().to_vec(),
            STANDARD.encode(&text).into_bytes(),
            URL_SAFE_NO_PAD.encode(&text).into_bytes(),
        ] {
            // The raw encoding itself can also contain long scheme runs.
            candidates_with_linear_budget(std::str::from_utf8(&encoded).unwrap()).unwrap();
            assert_eq!(
                decode_subscription_feed(PrivateSubscriptionBody::from_bytes(encoded).unwrap())
                    .unwrap()
                    .counts(),
                SubscriptionFeedCounts {
                    accepted: 1,
                    skipped: 0,
                }
            );
        }
    }

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
    fn mixed_feed_explains_each_rejection_without_relaxing_acceptance() {
        let raw = format!(
            "{URI}\n{URI}\n{}\n{}\nvless://broken",
            URI.replace("#Example", "&fm=secret#Example"),
            URI.replace("type=tcp", "type=kcp")
        );
        for input in [raw.clone(), STANDARD.encode(&raw)] {
            let feed = decode_subscription_feed(
                PrivateSubscriptionBody::from_bytes(input.into_bytes()).unwrap(),
            )
            .unwrap();
            assert_eq!(
                feed.counts(),
                SubscriptionFeedCounts {
                    accepted: 1,
                    skipped: 4
                }
            );
            assert_eq!(
                feed.skip_reasons().value(),
                serde_json::json!({
                    "duplicate": 1, "unsupported_field": 1, "unsupported_transport": 1, "invalid_profile": 1
                })
            );
        }
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
