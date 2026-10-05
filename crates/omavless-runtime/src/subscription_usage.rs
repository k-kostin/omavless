// SPDX-License-Identifier: MIT
//! Explicit, page-private provider assertions. No persistence or health inference.
use crate::subscription_transport::FetchedSubscription;
use omavless_control_protocol::StableErrorCode;
use omavless_domain::subscription_feed::decode_subscription_feed;
use serde_json::{Value, json};

pub const DEADLINE: std::time::Duration = std::time::Duration::from_secs(3);

pub fn projection(fetched: FetchedSubscription, instance: &str) -> Result<Value, StableErrorCode> {
    // Do not accept usage from an arbitrary HTML/config/error page. The same
    // supported-feed decoder as manual refresh must admit this response.
    decode_subscription_feed(fetched.body).map_err(|_| StableErrorCode::SubscriptionUnavailable)?;
    let usage = fetched.usage.map(|usage| {
        json!({
            // Decimal strings preserve the full provider u64 range across clients.
            "uploadBytes": usage.upload_bytes.to_string(),
            "downloadBytes": usage.download_bytes.to_string(),
            "totalBytes": usage.total_bytes.to_string(),
            "expiryUnixSeconds": usage.expiry_unix_seconds.map(|n| n.to_string()),
        })
    });
    Ok(json!({
        "schemaVersion": 1, "scope": "private_provider_reported_usage",
        "instanceId": instance, "availability": if usage.is_some() {"reported"} else {"not_provided"},
        "usage": usage,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_domain::{
        subscription_feed::PrivateSubscriptionBody, subscription_metadata::SubscriptionUsage,
    };
    fn fetched(body: &[u8], usage: Option<SubscriptionUsage>) -> FetchedSubscription {
        FetchedSubscription {
            body: PrivateSubscriptionBody::from_bytes(body.to_vec()).unwrap(),
            usage,
        }
    }
    const PROFILE: &[u8] = b"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example";
    #[test]
    fn projection_is_exact_and_optional_and_never_copies_feed_or_header_text() {
        let result = projection(fetched(PROFILE, None), "fixture-runtime").unwrap();
        assert_eq!(result["availability"], "not_provided");
        assert!(result["usage"].is_null());
        let result = projection(
            fetched(
                PROFILE,
                Some(SubscriptionUsage {
                    upload_bytes: u64::MAX,
                    download_bytes: 2,
                    total_bytes: 0,
                    expiry_unix_seconds: Some(0),
                }),
            ),
            "fixture-runtime",
        )
        .unwrap();
        assert_eq!(result["availability"], "reported");
        assert_eq!(result["usage"]["uploadBytes"], u64::MAX.to_string());
        for input in [
            "192.0.2.1",
            "vless://",
            "Example",
            "11111111",
            "Subscription-Userinfo",
        ] {
            assert!(!result.to_string().contains(input));
        }
    }
    #[test]
    fn unsupported_feed_cannot_supply_a_usage_claim() {
        for body in [
            b"<html>provider-private-error</html>".as_slice(),
            b"proxies:\n - name: private\n".as_slice(),
        ] {
            assert!(projection(fetched(body, None), "fixture-runtime").is_err());
        }
    }
}
