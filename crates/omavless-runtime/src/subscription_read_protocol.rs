// SPDX-License-Identifier: MIT

//! Exact v1 parsing for explicit subscription editor reads.
//!
//! The returned opaque record ID is private local metadata and deliberately
//! cannot be formatted or serialized through this type.

use crate::mutation_protocol::{MutationProtocolError, exact_fields};
use omavless_control_protocol::validate_request;
use omavless_domain::store::valid_record_id;
use serde_json::Value;

const EDIT_INPUT_FIELDS: &[&str] = &["subscriptionId"];

pub struct SubscriptionEditInputRequest {
    subscription_id: String,
}

impl SubscriptionEditInputRequest {
    #[must_use]
    pub(crate) fn private_subscription_id(&self) -> &str {
        &self.subscription_id
    }
}

pub fn parse_subscription_edit_input_request(
    request: &Value,
) -> Result<SubscriptionEditInputRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "subscriptions.edit_input" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, EDIT_INPUT_FIELDS, EDIT_INPUT_FIELDS) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let subscription_id = params
        .get("subscriptionId")
        .and_then(Value::as_str)
        .filter(|value| valid_record_id(value))
        .ok_or(MutationProtocolError::InvalidArgument)?;
    Ok(SubscriptionEditInputRequest {
        subscription_id: subscription_id.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";

    fn request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "edit-input-request",
            "method": method,
            "params": params,
        })
    }

    #[test]
    fn exact_editor_request_is_accepted() {
        let parsed = parse_subscription_edit_input_request(&request(
            "subscriptions.edit_input",
            json!({"subscriptionId": SUBSCRIPTION}),
        ))
        .unwrap();
        assert_eq!(parsed.private_subscription_id(), SUBSCRIPTION);
    }

    #[test]
    fn unknown_fields_methods_and_invalid_ids_fail_without_echo() {
        for candidate in [
            request(
                "subscriptions.list",
                json!({"subscriptionId": SUBSCRIPTION}),
            ),
            request("subscriptions.edit_input", json!({})),
            request(
                "subscriptions.edit_input",
                json!({"subscriptionId": SUBSCRIPTION, "operationId": "not-a-read-field"}),
            ),
            request(
                "subscriptions.edit_input",
                json!({"subscriptionId": "private.example/password"}),
            ),
        ] {
            let error = parse_subscription_edit_input_request(&candidate)
                .err()
                .unwrap();
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains("private.example"));
            assert!(!rendered.contains("password"));
        }
    }
}
