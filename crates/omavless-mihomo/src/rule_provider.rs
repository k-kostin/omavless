// SPDX-License-Identifier: MIT
//! Private controller-discovered rule-provider identities. Never client input.

use crate::{ErrorKind, MihomoError, Result};
use serde_json::Value;

pub const MAX_RULE_PROVIDERS: usize = 256;
pub const MAX_PROVIDER_NAME_BYTES: usize = 256;

/// An update target can only be obtained by validating the complete controller
/// collection. No public string constructor, Debug or serialization exists.
pub struct RuleProviderTarget(String);

impl RuleProviderTarget {
    /// Fixed PUT path, not a public response or a URL. Percent-encode every
    /// UTF-8 byte except RFC 3986 unreserved bytes, matching Python quote.
    #[must_use]
    pub fn update_path(&self) -> String {
        let mut path = String::from("/providers/rules/");
        const HEX: &[u8] = b"0123456789ABCDEF";
        for byte in self.0.bytes() {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                path.push(char::from(byte));
            } else {
                path.push('%');
                path.push(char::from(HEX[usize::from(byte >> 4)]));
                path.push(char::from(HEX[usize::from(byte & 15)]));
            }
        }
        path
    }
}

/// Validate all rows before releasing any HTTP update target. Unknown vehicle
/// kinds, file and inline providers are not update targets. Controller map order
/// is canonicalized; it is not a provider execution-order contract.
pub fn refresh_targets(payload: &Value) -> Result<Vec<RuleProviderTarget>> {
    let invalid = || MihomoError::new(ErrorKind::InvalidResponse);
    let providers = payload
        .get("providers")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    if providers.len() > MAX_RULE_PROVIDERS {
        return Err(invalid());
    }
    let mut result = Vec::new();
    for (name, provider) in providers {
        if name.is_empty()
            || name.len() > MAX_PROVIDER_NAME_BYTES
            || matches!(name.as_str(), "." | "..")
            || name
                .bytes()
                .any(|b| b < 32 || b == 127 || b"/\\?#%".contains(&b))
        {
            return Err(invalid());
        }
        let row = provider.as_object().ok_or_else(invalid)?;
        let vehicle = match row.get("vehicleType") {
            None => "",
            Some(value) => value.as_str().ok_or_else(invalid)?,
        };
        if vehicle.eq_ignore_ascii_case("http") {
            result.push(RuleProviderTarget(name.clone()));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_complete_collection_and_encodes_only_discovered_http_targets() {
        let targets = refresh_targets(&json!({"providers": {
            "space 界:@&": {"vehicleType":"hTtP"},
            "inline": {"vehicleType":"Inline"}, "file": {"vehicleType":"File"},
            "unknown": {}, "future": {"vehicleType":"future"}
        }}))
        .unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].update_path(),
            "/providers/rules/space%20%E7%95%8C%3A%40%26"
        );
        for bad in [
            "", ".", "..", "a/b", "a\\b", "a?b", "a#b", "a%b", "a\nb", "a\u{7f}b",
        ] {
            assert!(refresh_targets(&json!({"providers": {bad: {"vehicleType":"HTTP"}}})).is_err());
        }
        for payload in [
            json!({"providers":null}),
            json!({"providers":[]}),
            json!({"providers":{"a":false}}),
            json!({"providers":{"a":{"vehicleType":null}}}),
        ] {
            assert!(refresh_targets(&payload).is_err());
        }
    }

    #[test]
    fn bounds_are_bytes_and_collection_is_not_partially_accepted() {
        for (name, accepted) in [
            ("a".repeat(256), true),
            ("界".repeat(85), true),
            ("界".repeat(86), false),
            ("a".repeat(257), false),
        ] {
            assert_eq!(
                refresh_targets(&json!({"providers":{name:{"vehicleType":"http"}}})).is_ok(),
                accepted
            );
        }
        let mut rows = serde_json::Map::new();
        for n in 0..256 {
            rows.insert(format!("item-{n}"), json!({"vehicleType":"http"}));
        }
        assert_eq!(
            refresh_targets(&json!({"providers":rows})).unwrap().len(),
            256
        );
        rows.insert("overflow".into(), json!({}));
        assert!(refresh_targets(&json!({"providers":rows})).is_err());
        let error = refresh_targets(&json!({"providers":{"private/password":{}}}))
            .err()
            .unwrap();
        assert!(!format!("{error:?} {error}").contains("private/password"));
    }
}
