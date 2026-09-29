// SPDX-License-Identifier: MIT

//! Inactive, pure private-store candidate for provider-reported usage.
//!
//! No production caller writes or reads this extension. A future refresh must
//! compose it with the already validated feed into ONE atomic store payload;
//! calling it after a committed refresh would create a stale-data window.

use crate::private_store::{MAX_PRIVATE_STORE_BYTES, parse_private_store};
use crate::subscription_metadata::{MAX_EXPIRY_UNIX_SECONDS, SubscriptionUsage};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;

const FIELD: &str = "providerUsageV1";
const DIGEST_DOMAIN: &[u8] = b"omavless.provider-usage-url.v1\0";

/// Validate member identity before `serde_json::Value` can collapse repeated
/// keys. Deserialization supplies decoded keys, so JSON escapes and Unicode
/// spelling variants cannot bypass the comparison. This is deliberately local
/// to the inactive optional-usage model; the legacy store reader is unchanged.
struct NoDuplicateMembers;

impl<'de> Deserialize<'de> for NoDuplicateMembers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NoDuplicateMembersVisitor)
    }
}

struct NoDuplicateMembersVisitor;

impl<'de> Visitor<'de> for NoDuplicateMembersVisitor {
    type Value = NoDuplicateMembers;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate members")
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_string<E: serde::de::Error>(self, _: String) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateMembers)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        while sequence.next_element::<NoDuplicateMembers>()?.is_some() {}
        Ok(NoDuplicateMembers)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(serde::de::Error::custom("duplicate_json_member"));
            }
            map.next_value::<NoDuplicateMembers>()?;
        }
        Ok(NoDuplicateMembers)
    }
}

fn reject_duplicate_members(input: &str) -> Result<(), UsageStoreError> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(UsageStoreError::InvalidStore);
    }
    serde_json::from_str::<NoDuplicateMembers>(input)
        .map(|_| ())
        .map_err(|_| UsageStoreError::InvalidStore)
}

/// Fixed error categories; no stored URL, metadata or profile identity is
/// retained by the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageStoreError {
    InvalidStore,
    SubscriptionChanged,
    InvalidObservation,
    TooLarge,
}

/// Private candidate bytes for the existing atomic store transaction only.
/// This type deliberately has no Debug, Clone, Display or serialization.
pub struct PrivateUsageCandidate(Vec<u8>);

impl PrivateUsageCandidate {
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.0
    }
}

/// Provider assertion bound to one subscription URL and exact successful
/// refresh token. It is not VPN health or proof that an account is active.
/// Intentionally cannot be formatted or serialized generically.
pub struct PrivateUsageClaim {
    usage: SubscriptionUsage,
    observed_at_unix_seconds: u64,
}

impl PrivateUsageClaim {
    #[must_use]
    pub const fn usage(&self) -> SubscriptionUsage {
        self.usage
    }

    #[must_use]
    pub const fn observed_at_unix_seconds(&self) -> u64 {
        self.observed_at_unix_seconds
    }
}

fn digest_url(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update(url.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn subscription_record<'a>(root: &'a Value, id: &str) -> Option<&'a Map<String, Value>> {
    root.get("subscriptions")?
        .as_array()?
        .iter()
        .find(|value| value.get("id").and_then(Value::as_str) == Some(id))?
        .as_object()
}

/// Read only a well-formed claim bound to the record's *current* URL and
/// refresh token. Unknown/corrupt optional metadata is absent rather than a
/// fatal store error. A deleted record or legacy store has no claim.
pub fn read_provider_usage(
    input: &str,
    subscription_id: &str,
) -> Result<Option<PrivateUsageClaim>, UsageStoreError> {
    reject_duplicate_members(input)?;
    parse_private_store(input).map_err(|_| UsageStoreError::InvalidStore)?;
    let root: Value = serde_json::from_str(input).map_err(|_| UsageStoreError::InvalidStore)?;
    let Some(record) = subscription_record(&root, subscription_id) else {
        return Ok(None);
    };
    let Some(value) = record.get(FIELD).and_then(Value::as_object) else {
        return Ok(None);
    };
    let url = record
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let token = record.get("updatedAt").and_then(Value::as_u64).unwrap_or(0);
    let observed = value.get("observedAt").and_then(Value::as_u64);
    let expiry = match value.get("expire") {
        None => Some(None),
        Some(value) => value.as_u64().map(Some),
    };
    let expected_len = if value.contains_key("expire") { 8 } else { 7 };
    if value.len() != expected_len
        || value.get("version").and_then(Value::as_u64) != Some(1)
        || value.get("urlSha256").and_then(Value::as_str) != Some(digest_url(url).as_str())
        || value.get("refreshToken").and_then(Value::as_u64) != Some(token)
        || token == 0
        || !observed.is_some_and(|time| time > 0 && time <= MAX_EXPIRY_UNIX_SECONDS)
        || expiry.is_none()
        || expiry
            .flatten()
            .is_some_and(|time| time > MAX_EXPIRY_UNIX_SECONDS)
    {
        return Ok(None);
    }
    let (Some(upload), Some(download), Some(total)) = (
        value.get("upload").and_then(Value::as_u64),
        value.get("download").and_then(Value::as_u64),
        value.get("total").and_then(Value::as_u64),
    ) else {
        return Ok(None);
    };
    Ok(Some(PrivateUsageClaim {
        usage: SubscriptionUsage {
            upload_bytes: upload,
            download_bytes: download,
            total_bytes: total,
            expiry_unix_seconds: expiry.flatten(),
        },
        observed_at_unix_seconds: observed.unwrap_or_default(),
    }))
}

/// Compose optional metadata with the already refreshed, not-yet-committed
/// private store candidate. `None` physically clears a previous claim. This
/// function does not perform the refresh, I/O, or any lifecycle action.
pub fn bind_usage_to_refresh_candidate(
    candidate: &str,
    subscription_id: &str,
    expected_url: &str,
    expected_refresh_token: u64,
    observed_at_unix_seconds: u64,
    usage: Option<SubscriptionUsage>,
) -> Result<PrivateUsageCandidate, UsageStoreError> {
    reject_duplicate_members(candidate)?;
    parse_private_store(candidate).map_err(|_| UsageStoreError::InvalidStore)?;
    let mut document: Value =
        serde_json::from_str(candidate).map_err(|_| UsageStoreError::InvalidStore)?;
    let record = document
        .get_mut("subscriptions")
        .and_then(Value::as_array_mut)
        .and_then(|items| {
            items
                .iter_mut()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(subscription_id))
        })
        .and_then(Value::as_object_mut)
        .ok_or(UsageStoreError::SubscriptionChanged)?;
    if record.get("url").and_then(Value::as_str) != Some(expected_url)
        || record.get("updatedAt").and_then(Value::as_u64) != Some(expected_refresh_token)
        || expected_refresh_token == 0
    {
        return Err(UsageStoreError::SubscriptionChanged);
    }
    if let Some(usage) = usage {
        if observed_at_unix_seconds == 0
            || observed_at_unix_seconds > MAX_EXPIRY_UNIX_SECONDS
            || usage
                .expiry_unix_seconds
                .is_some_and(|time| time > MAX_EXPIRY_UNIX_SECONDS)
        {
            return Err(UsageStoreError::InvalidObservation);
        }
        let mut claim = json!({
            "version": 1,
            "urlSha256": digest_url(expected_url),
            "refreshToken": expected_refresh_token,
            "observedAt": observed_at_unix_seconds,
            "upload": usage.upload_bytes,
            "download": usage.download_bytes,
            "total": usage.total_bytes,
        });
        if let Some(expiry) = usage.expiry_unix_seconds {
            claim["expire"] = Value::from(expiry);
        }
        record.insert(FIELD.to_owned(), claim);
    } else {
        record.remove(FIELD);
    }
    let mut bytes =
        serde_json::to_vec_pretty(&document).map_err(|_| UsageStoreError::InvalidStore)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(UsageStoreError::TooLarge);
    }
    Ok(PrivateUsageCandidate(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_store::{apply_subscription_refresh, prepare_subscription_refresh};

    const ID: &str = "20000000-0000-0000-0000-000000000001";
    const URL: &str = "https://example.invalid/synthetic";

    fn store(version: u8) -> String {
        json!({
            "version": version,
            "profiles": [],
            "subscriptions": [{
                "id": ID, "name": "Synthetic", "url": URL, "updatedAt": 1,
                "extra": {"keep": true}
            }],
            "unrelated": {"keep": [1, 2]}
        })
        .to_string()
    }

    fn usage() -> SubscriptionUsage {
        SubscriptionUsage {
            upload_bytes: 12,
            download_bytes: 34,
            total_bytes: 100,
            expiry_unix_seconds: Some(1_893_456_000),
        }
    }

    fn bound(input: &str) -> String {
        String::from_utf8(
            bind_usage_to_refresh_candidate(input, ID, URL, 1, 1_800_000_000, Some(usage()))
                .unwrap()
                .payload()
                .to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn old_stores_and_unknown_fields_remain_compatible() {
        for version in [1, 2, 3] {
            let original = store(version);
            assert!(read_provider_usage(&original, ID).unwrap().is_none());
            let written = bound(&original);
            let value: Value = serde_json::from_str(&written).unwrap();
            assert_eq!(value["version"], version);
            assert_eq!(value["unrelated"], json!({"keep": [1, 2]}));
            assert_eq!(value["subscriptions"][0]["extra"], json!({"keep": true}));
            let claim = read_provider_usage(&written, ID).unwrap().unwrap();
            assert_eq!(claim.observed_at_unix_seconds(), 1_800_000_000);
            assert_eq!(claim.usage().upload_bytes, 12);
            assert_eq!(
                parse_private_store(&original).unwrap().projection(),
                parse_private_store(&written).unwrap().projection(),
                "ordinary/shareable store facts must not gain usage values"
            );
            assert_eq!(
                parse_private_store(&original).unwrap().support_projection(),
                parse_private_store(&written).unwrap().support_projection(),
                "shareable diagnostics must not gain usage values"
            );
        }
    }

    #[test]
    fn url_token_and_delete_revoke_even_a_field_preserved_by_old_code() {
        let written = bound(&store(3));
        let mut document: Value = serde_json::from_str(&written).unwrap();
        document["subscriptions"][0]["name"] = Value::from("Renamed");
        assert!(
            read_provider_usage(&document.to_string(), ID)
                .unwrap()
                .is_some()
        );
        document["subscriptions"][0]["url"] = Value::from("https://replacement.invalid/path");
        assert!(
            read_provider_usage(&document.to_string(), ID)
                .unwrap()
                .is_none()
        );
        document["subscriptions"][0]["url"] = Value::from(URL);
        document["subscriptions"][0]["updatedAt"] = Value::from(2);
        assert!(
            read_provider_usage(&document.to_string(), ID)
                .unwrap()
                .is_none()
        );
        document["subscriptions"] = json!([]);
        assert!(
            read_provider_usage(&document.to_string(), ID)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn absent_optional_header_clears_claim_in_same_candidate() {
        let prior = bound(&store(3));
        assert!(matches!(
            bind_usage_to_refresh_candidate(&prior, ID, URL, 2, 0, None),
            Err(UsageStoreError::SubscriptionChanged)
        ));
        assert!(read_provider_usage(&prior, ID).unwrap().is_some());
        let cleared = bind_usage_to_refresh_candidate(&prior, ID, URL, 1, 0, None).unwrap();
        let value: Value = serde_json::from_slice(cleared.payload()).unwrap();
        assert!(value["subscriptions"][0].get(FIELD).is_none());
        assert!(
            read_provider_usage(std::str::from_utf8(cleared.payload()).unwrap(), ID)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn composes_only_with_validated_not_yet_committed_refresh_payload() {
        let original = store(3);
        let snapshot = prepare_subscription_refresh(&original, ID).unwrap();
        let (refreshed, _) = apply_subscription_refresh(&original, snapshot, vec![], 9, 0).unwrap();
        let refreshed_text = std::str::from_utf8(refreshed.payload()).unwrap();
        let candidate = bind_usage_to_refresh_candidate(
            refreshed_text,
            ID,
            URL,
            9,
            1_800_000_000,
            Some(usage()),
        )
        .unwrap();
        let candidate_value: Value = serde_json::from_slice(candidate.payload()).unwrap();
        assert_eq!(candidate_value["unrelated"], json!({"keep": [1, 2]}));
        assert_eq!(
            candidate_value["subscriptions"][0]["extra"],
            json!({"keep": true})
        );
        assert!(read_provider_usage(&original, ID).unwrap().is_none());
        assert_eq!(
            read_provider_usage(std::str::from_utf8(candidate.payload()).unwrap(), ID)
                .unwrap()
                .unwrap()
                .usage()
                .total_bytes,
            100
        );
        assert!(matches!(
            bind_usage_to_refresh_candidate(
                refreshed_text,
                ID,
                URL,
                8,
                1_800_000_000,
                Some(usage())
            ),
            Err(UsageStoreError::SubscriptionChanged)
        ));
        assert_eq!(original, store(3), "candidate failure never mutates input");
        let mut concurrent: Value = serde_json::from_str(&original).unwrap();
        concurrent["subscriptions"][0]["updatedAt"] = Value::from(3);
        let stale_snapshot = prepare_subscription_refresh(&original, ID).unwrap();
        assert!(matches!(
            apply_subscription_refresh(&concurrent.to_string(), stale_snapshot, vec![], 9, 0),
            Err(crate::private_store::PrivateStoreError::SubscriptionChanged)
        ));
    }

    #[test]
    fn malformed_optional_claim_is_not_a_fatal_store_error() {
        let mut document: Value = serde_json::from_str(&bound(&store(3))).unwrap();
        for bad in [
            json!(null),
            json!({"version": 1}),
            json!({"upload": "secret"}),
        ] {
            document["subscriptions"][0][FIELD] = bad;
            assert!(parse_private_store(&document.to_string()).is_ok());
            assert!(
                read_provider_usage(&document.to_string(), ID)
                    .unwrap()
                    .is_none()
            );
        }
        document["subscriptions"][0][FIELD] = json!({
            "version": 1, "urlSha256": digest_url(URL), "refreshToken": 1,
            "observedAt": 1_800_000_000, "upload": 1, "download": 2,
            "total": 3, "expire": "invalid"
        });
        assert!(
            read_provider_usage(&document.to_string(), ID)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn repeated_raw_members_are_rejected_before_value_can_collapse_them() {
        let original = store(3);
        let nested = original.replacen("\"keep\":true", "\"keep\":true,\"keep\":false", 1);
        let in_array = original.replacen("[1,2]", "[{\"item\":1,\"item\":2}]", 1);
        let escaped = original.replacen("\"version\":3", "\"version\":3,\"\\u0076ersion\":3", 1);
        let unicode = format!(
            "{},\"é\":1,\"\\u00e9\":2}}",
            &original[..original.len() - 1]
        );
        let valid_metadata = bound(&original);
        let metadata =
            valid_metadata.replacen("\"upload\": 12", "\"upload\": 12, \"upload\": 13", 1);
        assert_ne!(nested, original);
        assert_ne!(in_array, original);
        assert_ne!(escaped, original);
        assert_ne!(metadata, valid_metadata);
        for input in [&nested, &in_array, &escaped, &unicode, &metadata] {
            assert!(
                parse_private_store(input).is_ok(),
                "legacy parser is unchanged"
            );
            assert!(matches!(
                read_provider_usage(input, ID),
                Err(UsageStoreError::InvalidStore)
            ));
            assert!(matches!(
                bind_usage_to_refresh_candidate(input, ID, URL, 1, 0, None),
                Err(UsageStoreError::InvalidStore)
            ));
        }
        assert!(read_provider_usage(&original, ID).unwrap().is_none());
        assert!(
            read_provider_usage(&bound(&original), ID)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn raw_duplicate_check_keeps_old_store_shape_and_bounds_private_errors() {
        for version in [1, 2, 3] {
            let original = store(version);
            let written = bound(&original);
            assert!(read_provider_usage(&written, ID).unwrap().is_some());
            let duplicate = format!(
                "{},\"private-marker\":1,\"private-marker\":2}}",
                &original[..original.len() - 1]
            );
            let error = read_provider_usage(&duplicate, ID).err().unwrap();
            assert_eq!(error, UsageStoreError::InvalidStore);
            assert!(!format!("{error:?}").contains("private-marker"));
            assert!(matches!(
                bind_usage_to_refresh_candidate(&duplicate, ID, URL, 1, 0, None),
                Err(UsageStoreError::InvalidStore)
            ));
        }
        let too_large = format!("{}{}", store(3), " ".repeat(MAX_PRIVATE_STORE_BYTES));
        assert!(matches!(
            read_provider_usage(&too_large, ID),
            Err(UsageStoreError::InvalidStore)
        ));
        let mut mixed: Value = serde_json::from_str(&store(3)).unwrap();
        mixed["unknown"] = json!([null, true, false, 1.25, "é", {"nested": []}]);
        let mixed = mixed.to_string();
        assert!(read_provider_usage(&mixed, ID).unwrap().is_none());
        assert!(bind_usage_to_refresh_candidate(&mixed, ID, URL, 1, 0, None).is_ok());
    }

    #[test]
    fn invalid_inputs_fail_without_private_echo_or_partial_payload() {
        let original = store(3);
        for (url, token, observed) in [
            ("https://wrong.invalid/path", 1, 1_800_000_000),
            (URL, 2, 1_800_000_000),
            (URL, 1, 0),
            (URL, 1, MAX_EXPIRY_UNIX_SECONDS + 1),
        ] {
            let error = match bind_usage_to_refresh_candidate(
                &original,
                ID,
                url,
                token,
                observed,
                Some(usage()),
            ) {
                Ok(_) => panic!("unsafe usage candidate succeeded"),
                Err(error) => error,
            };
            let message = format!("{error:?}");
            assert!(!message.contains("wrong.invalid"));
            assert!(!message.contains("example.invalid"));
        }
        assert!(matches!(
            bind_usage_to_refresh_candidate(
                &original,
                ID,
                URL,
                1,
                1_800_000_000,
                Some(SubscriptionUsage {
                    expiry_unix_seconds: Some(MAX_EXPIRY_UNIX_SECONDS + 1),
                    ..usage()
                })
            ),
            Err(UsageStoreError::InvalidObservation)
        ));
        assert_eq!(original, store(3));
    }

    #[test]
    fn candidate_near_store_limit_refuses_growth_without_truncation() {
        let mut document: Value = serde_json::from_str(&store(3)).unwrap();
        let baseline = document.to_string().len();
        document["padding"] = Value::from("x".repeat(MAX_PRIVATE_STORE_BYTES - baseline - 256));
        let original = document.to_string();
        assert!(original.len() < MAX_PRIVATE_STORE_BYTES);
        assert!(parse_private_store(&original).is_ok());
        assert!(matches!(
            bind_usage_to_refresh_candidate(&original, ID, URL, 1, 1_800_000_000, Some(usage())),
            Err(UsageStoreError::TooLarge)
        ));
        assert!(parse_private_store(&original).is_ok());
    }
}
