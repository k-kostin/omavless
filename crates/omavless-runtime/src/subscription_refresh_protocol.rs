// SPDX-License-Identifier: MIT

//! Exact v1 single-subscription refresh request validation.
//!
//! The request names only an opaque local record. The provider URL is loaded
//! by the ownership-gated runtime from its private store and never crosses the
//! ordinary control response boundary.

use crate::mutation::{CoordinatorError, MutationDigest, MutationKind, MutationRequest};
use crate::mutation_protocol::{MutationProtocolError, exact_fields, metadata};
use crate::subscription_mutation_protocol::semantic_digest;
use omavless_control_protocol::validate_request;
use omavless_domain::store::valid_record_id;
use serde_json::Value;

const REFRESH_FIELDS: &[&str] = &["subscriptionId", "operationId", "expectedRevision"];

/// Parsed private refresh intent. The local record ID is deliberately not
/// formattable or serializable.
pub struct SubscriptionRefreshRequest {
    subscription_id: String,
    operation_id: Option<String>,
    expected_revision: Option<u64>,
    digest: MutationDigest,
}

impl SubscriptionRefreshRequest {
    #[must_use]
    pub(crate) fn private_subscription_id(&self) -> &str {
        &self.subscription_id
    }

    pub(crate) fn external_work_request(&self) -> Result<MutationRequest, CoordinatorError> {
        MutationRequest::new(
            MutationKind::Other,
            self.operation_id.as_deref(),
            self.expected_revision,
            self.digest,
        )
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (String, Option<String>, Option<u64>, MutationDigest) {
        (
            self.subscription_id,
            self.operation_id,
            self.expected_revision,
            self.digest,
        )
    }
}

/// Parse one `subscriptions.refresh` request without reading the private
/// store, fetching a provider, or reserving a mutation slot.
pub fn parse_subscription_refresh_request(
    request: &Value,
) -> Result<SubscriptionRefreshRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "subscriptions.refresh" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, REFRESH_FIELDS, &["subscriptionId"]) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let metadata = metadata(params)?;
    let subscription_id = params
        .get("subscriptionId")
        .and_then(Value::as_str)
        .filter(|value| valid_record_id(value))
        .ok_or(MutationProtocolError::InvalidArgument)?;
    Ok(SubscriptionRefreshRequest {
        subscription_id: subscription_id.to_owned(),
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision: metadata.expected_revision,
        digest: semantic_digest(
            "subscriptions.refresh",
            Some(subscription_id),
            None,
            None,
            metadata.expected_revision,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_control_protocol::MAX_REVISION;
    use serde_json::json;

    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";

    fn request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "refresh-request",
            "method": method,
            "params": params,
        })
    }

    #[test]
    fn exact_refresh_shape_and_metadata_are_accepted() {
        let parsed = parse_subscription_refresh_request(&request(
            "subscriptions.refresh",
            json!({
                "subscriptionId": SUBSCRIPTION,
                "operationId": "refresh-operation",
                "expectedRevision": 9,
            }),
        ))
        .unwrap();
        assert_eq!(parsed.private_subscription_id(), SUBSCRIPTION);
        let (subscription_id, operation_id, expected_revision, _) = parsed.into_parts();
        assert_eq!(subscription_id, SUBSCRIPTION);
        assert_eq!(operation_id.as_deref(), Some("refresh-operation"));
        assert_eq!(expected_revision, Some(9));
    }

    #[test]
    fn unknown_methods_fields_and_invalid_ids_fail_closed() {
        for candidate in [
            request("subscriptions.refresh_all", json!({})),
            request("subscriptions.refresh", json!({})),
            request(
                "subscriptions.refresh",
                json!({"subscriptionId": SUBSCRIPTION, "url": "https://private.invalid"}),
            ),
            request("subscriptions.refresh", json!({"subscriptionId": false})),
            request(
                "subscriptions.refresh",
                json!({"subscriptionId": "private.example/password"}),
            ),
        ] {
            assert!(parse_subscription_refresh_request(&candidate).is_err());
        }
    }

    #[test]
    fn metadata_bounds_and_digest_identity_are_exact() {
        for params in [
            json!({"subscriptionId": SUBSCRIPTION, "operationId": ""}),
            json!({"subscriptionId": SUBSCRIPTION, "operationId": "contains space"}),
            json!({"subscriptionId": SUBSCRIPTION, "operationId": "x".repeat(65)}),
            json!({"subscriptionId": SUBSCRIPTION, "expectedRevision": "1"}),
            json!({"subscriptionId": SUBSCRIPTION, "expectedRevision": MAX_REVISION + 1}),
        ] {
            assert!(
                parse_subscription_refresh_request(&request("subscriptions.refresh", params))
                    .is_err()
            );
        }

        let (_, _, _, first) = parse_subscription_refresh_request(&request(
            "subscriptions.refresh",
            json!({"subscriptionId": SUBSCRIPTION, "expectedRevision": 4}),
        ))
        .unwrap()
        .into_parts();
        let (_, _, _, retry) = parse_subscription_refresh_request(&request(
            "subscriptions.refresh",
            json!({
                "subscriptionId": SUBSCRIPTION,
                "operationId": "retry-id",
                "expectedRevision": 4,
            }),
        ))
        .unwrap()
        .into_parts();
        assert!(first == retry);

        let (_, _, _, changed) = parse_subscription_refresh_request(&request(
            "subscriptions.refresh",
            json!({"subscriptionId": SUBSCRIPTION, "expectedRevision": 5}),
        ))
        .unwrap()
        .into_parts();
        assert!(first != changed);
    }

    #[test]
    fn errors_never_echo_private_values() {
        let private = "private.example/password";
        let error = parse_subscription_refresh_request(&request(
            "subscriptions.refresh",
            json!({"subscriptionId": private}),
        ))
        .err()
        .unwrap();
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("private.example"));
        assert!(!rendered.contains("password"));
    }
}
