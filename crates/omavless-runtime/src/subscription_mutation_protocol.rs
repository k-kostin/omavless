// SPDX-License-Identifier: MIT

//! Exact v1 subscription-mutation parameter validation and semantic digesting.
//!
//! This parser accepts only the client-owned intent needed before a bounded
//! fetch/store transaction;
//! generated IDs, fetched entries and trusted host observations cannot enter
//! through this boundary. Private names and bearer URLs are neither formatted
//! nor serialized by the resulting request types.

use crate::mutation::{CoordinatorError, MutationDigest, MutationKind, MutationRequest};
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use omavless_control_protocol::validate_request;
use omavless_domain::import::{MAX_SUBSCRIPTION_URL_BYTES, valid_subscription_url};
use omavless_domain::store::valid_record_id;
use serde_json::{Map, Value};

const ADD_FIELDS: &[&str] = &["name", "url", "operationId", "expectedRevision"];
const UPDATE_FIELDS: &[&str] = &[
    "subscriptionId",
    "name",
    "url",
    "operationId",
    "expectedRevision",
];
const DELETE_FIELDS: &[&str] = &["subscriptionId", "operationId", "expectedRevision"];

/// Eighty Unicode scalar values can occupy at most 320 UTF-8 bytes. The
/// normalized name is also checked against the canonical 80-character store
/// limit before this parser produces an intent.
pub const MAX_SUBSCRIPTION_NAME_INPUT_BYTES: usize = 320;
const MAX_SUBSCRIPTION_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionMutationKind {
    Add,
    Update,
    Delete,
}

/// Client-owned subscription intent. Fetch results, generated identifiers,
/// timestamps and active-service observations are supplied later by trusted
/// owner stages, never by this value.
pub(crate) enum SubscriptionMutationIntent {
    Add {
        name: String,
        url: String,
    },
    Update {
        subscription_id: String,
        name: String,
        url: String,
    },
    Delete {
        subscription_id: String,
    },
}

/// One parsed but not executed subscription mutation. It intentionally has no
/// `Debug`, cloning or serialization implementation because names and URLs are
/// private request data.
pub struct SubscriptionMutationRequest {
    intent: SubscriptionMutationIntent,
    operation_id: Option<String>,
    expected_revision: Option<u64>,
    digest: MutationDigest,
}

impl SubscriptionMutationRequest {
    #[must_use]
    pub const fn kind(&self) -> SubscriptionMutationKind {
        match &self.intent {
            SubscriptionMutationIntent::Add { .. } => SubscriptionMutationKind::Add,
            SubscriptionMutationIntent::Update { .. } => SubscriptionMutationKind::Update,
            SubscriptionMutationIntent::Delete { .. } => SubscriptionMutationKind::Delete,
        }
    }

    #[must_use]
    pub(crate) fn remote_url(&self) -> Option<&str> {
        match &self.intent {
            SubscriptionMutationIntent::Add { url, .. }
            | SubscriptionMutationIntent::Update { url, .. } => Some(url),
            SubscriptionMutationIntent::Delete { .. } => None,
        }
    }

    pub(crate) fn external_work_request(&self) -> Result<MutationRequest, CoordinatorError> {
        MutationRequest::new(
            MutationKind::Other,
            self.operation_id.as_deref(),
            self.expected_revision,
            self.digest,
        )
    }

    /// Consume the request at the future serialized owner/fetch boundary.
    /// None of these values belongs in a response, diagnostic or log.
    #[must_use]
    #[allow(
        dead_code,
        reason = "offline until the serialized subscription owner binding is accepted"
    )]
    pub(crate) fn into_parts(
        self,
    ) -> (
        SubscriptionMutationIntent,
        Option<String>,
        Option<u64>,
        MutationDigest,
    ) {
        (
            self.intent,
            self.operation_id,
            self.expected_revision,
            self.digest,
        )
    }
}

fn subscription_id(params: &Map<String, Value>) -> Result<&str, MutationProtocolError> {
    params
        .get("subscriptionId")
        .and_then(Value::as_str)
        .filter(|value| valid_record_id(value))
        .ok_or(MutationProtocolError::InvalidArgument)
}

fn normalized_name(params: &Map<String, Value>) -> Result<String, MutationProtocolError> {
    let value = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= MAX_SUBSCRIPTION_NAME_INPUT_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let value = value
        .chars()
        .filter(|character| !matches!(*character as u32, 0..=31 | 127))
        .collect::<String>();
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_SUBSCRIPTION_NAME_CHARS {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(value.to_owned())
}

fn normalized_url(params: &Map<String, Value>) -> Result<String, MutationProtocolError> {
    let value = params
        .get("url")
        .and_then(Value::as_str)
        .filter(|value| value.len() <= MAX_SUBSCRIPTION_URL_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?
        .trim();
    if !valid_subscription_url(value) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(value.to_owned())
}

fn semantic_digest(
    method: &str,
    subscription_id: Option<&str>,
    name: Option<&str>,
    url: Option<&str>,
    expected_revision: Option<u64>,
) -> MutationDigest {
    let mut bytes = Vec::with_capacity(
        160 + subscription_id.map_or(0, str::len)
            + name.map_or(0, str::len)
            + url.map_or(0, str::len),
    );
    bytes.extend_from_slice(b"omavless.control/subscription-mutation/v1\0");
    append_field(&mut bytes, method);
    for value in [subscription_id, name, url] {
        match value {
            Some(value) => {
                bytes.push(1);
                append_field(&mut bytes, value);
            }
            None => bytes.push(0),
        }
    }
    match expected_revision {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    MutationDigest::from_semantic_bytes(&bytes)
}

fn intent_digest(
    method: &str,
    intent: &SubscriptionMutationIntent,
    expected_revision: Option<u64>,
) -> MutationDigest {
    match intent {
        SubscriptionMutationIntent::Add { name, url } => {
            semantic_digest(method, None, Some(name), Some(url), expected_revision)
        }
        SubscriptionMutationIntent::Update {
            subscription_id,
            name,
            url,
        } => semantic_digest(
            method,
            Some(subscription_id),
            Some(name),
            Some(url),
            expected_revision,
        ),
        SubscriptionMutationIntent::Delete { subscription_id } => {
            semantic_digest(method, Some(subscription_id), None, None, expected_revision)
        }
    }
}

/// Parse one bounded control request into an offline subscription intent.
///
/// The caller must already have decoded the request with the duplicate-key,
/// UTF-8, framing, depth and global string limits from the control protocol.
pub fn parse_subscription_mutation_request(
    request: &Value,
) -> Result<SubscriptionMutationRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    let method = request["method"]
        .as_str()
        .ok_or(MutationProtocolError::InvalidRequest)?;
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let metadata = metadata(params)?;

    let intent = match method {
        "subscriptions.add" => {
            if !exact_fields(params, ADD_FIELDS, &["name", "url"]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let name = normalized_name(params)?;
            let url = normalized_url(params)?;
            SubscriptionMutationIntent::Add { name, url }
        }
        "subscriptions.update" => {
            if !exact_fields(params, UPDATE_FIELDS, &["subscriptionId", "name", "url"]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let subscription_id = subscription_id(params)?.to_owned();
            let name = normalized_name(params)?;
            let url = normalized_url(params)?;
            SubscriptionMutationIntent::Update {
                subscription_id,
                name,
                url,
            }
        }
        "subscriptions.delete" => {
            if !exact_fields(params, DELETE_FIELDS, &["subscriptionId"]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let subscription_id = subscription_id(params)?.to_owned();
            SubscriptionMutationIntent::Delete { subscription_id }
        }
        _ => return Err(MutationProtocolError::UnknownMethod),
    };

    let digest = intent_digest(method, &intent, metadata.expected_revision);
    Ok(SubscriptionMutationRequest {
        intent,
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision: metadata.expected_revision,
        digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_mutation_protocol::parse_profile_mutation_request;
    use omavless_control_protocol::MAX_REVISION;
    use serde_json::json;

    const SUBSCRIPTION_ID: &str = "00000000-0000-4000-8000-000000000001";
    const URL: &str = "https://example.test/subscription";

    fn request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "request-1",
            "method": method,
            "params": params,
        })
    }

    fn parsed(method: &str, params: Value) -> SubscriptionMutationRequest {
        parse_subscription_mutation_request(&request(method, params)).unwrap()
    }

    fn digest(method: &str, params: Value) -> MutationDigest {
        parsed(method, params).into_parts().3
    }

    #[test]
    fn exact_subscription_mutation_shapes_are_accepted() {
        assert_eq!(
            parsed("subscriptions.add", json!({"name": "Provider", "url": URL})).kind(),
            SubscriptionMutationKind::Add
        );
        assert_eq!(
            parsed(
                "subscriptions.update",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "name": "Provider",
                    "url": URL,
                    "operationId": "operation-1",
                    "expectedRevision": MAX_REVISION
                })
            )
            .kind(),
            SubscriptionMutationKind::Update
        );
        assert_eq!(
            parsed(
                "subscriptions.delete",
                json!({"subscriptionId": SUBSCRIPTION_ID})
            )
            .kind(),
            SubscriptionMutationKind::Delete
        );
    }

    #[test]
    fn private_intents_contain_only_client_owned_semantics() {
        let (intent, operation_id, expected_revision, _) = parsed(
            "subscriptions.add",
            json!({
                "name": "  Provider  ",
                "url": format!("  {URL}  "),
                "operationId": "operation-1",
                "expectedRevision": 9
            }),
        )
        .into_parts();
        match intent {
            SubscriptionMutationIntent::Add { name, url } => {
                assert_eq!(name, "Provider");
                assert_eq!(url, URL);
            }
            _ => panic!("add request mapped to a different intent kind"),
        }
        assert_eq!(operation_id.as_deref(), Some("operation-1"));
        assert_eq!(expected_revision, Some(9));

        let (intent, _, _, _) = parsed(
            "subscriptions.update",
            json!({"subscriptionId": SUBSCRIPTION_ID, "name": "Provider", "url": URL}),
        )
        .into_parts();
        match intent {
            SubscriptionMutationIntent::Update {
                subscription_id,
                name,
                url,
            } => {
                assert_eq!(subscription_id, SUBSCRIPTION_ID);
                assert_eq!(name, "Provider");
                assert_eq!(url, URL);
            }
            _ => panic!("update request mapped to a different intent kind"),
        }

        let (intent, _, _, _) = parsed(
            "subscriptions.delete",
            json!({"subscriptionId": SUBSCRIPTION_ID}),
        )
        .into_parts();
        match intent {
            SubscriptionMutationIntent::Delete { subscription_id } => {
                assert_eq!(subscription_id, SUBSCRIPTION_ID);
            }
            _ => panic!("delete request mapped to a different intent kind"),
        }
    }

    #[test]
    fn forbidden_owner_fetch_and_host_fields_fail_closed() {
        let forbidden = [
            ("subscriptions.add", "subscriptionId"),
            ("subscriptions.add", "newId"),
            ("subscriptions.add", "entries"),
            ("subscriptions.add", "updatedAt"),
            ("subscriptions.update", "activeService"),
            ("subscriptions.update", "path"),
            ("subscriptions.delete", "command"),
        ];
        for (method, field) in forbidden {
            let mut params = match method {
                "subscriptions.add" => json!({"name": "Provider", "url": URL}),
                "subscriptions.update" => json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "name": "Provider",
                    "url": URL
                }),
                _ => json!({"subscriptionId": SUBSCRIPTION_ID}),
            };
            params
                .as_object_mut()
                .unwrap()
                .insert(field.to_owned(), json!("private"));
            assert!(matches!(
                parse_subscription_mutation_request(&request(method, params)),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
    }

    #[test]
    fn missing_and_wrongly_typed_fields_fail_closed() {
        let invalid = [
            ("subscriptions.add", json!({"url": URL})),
            ("subscriptions.add", json!({"name": true, "url": URL})),
            (
                "subscriptions.add",
                json!({"name": "Provider", "url": false}),
            ),
            (
                "subscriptions.update",
                json!({"subscriptionId": SUBSCRIPTION_ID, "name": "Provider"}),
            ),
            (
                "subscriptions.update",
                json!({"subscriptionId": false, "name": "Provider", "url": URL}),
            ),
            ("subscriptions.delete", json!({})),
            ("subscriptions.delete", json!({"subscriptionId": 1})),
        ];
        for (method, params) in invalid {
            assert!(matches!(
                parse_subscription_mutation_request(&request(method, params)),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
        assert!(matches!(
            parse_subscription_mutation_request(&request("subscriptions.refresh", json!({}))),
            Err(MutationProtocolError::UnknownMethod)
        ));
    }

    #[test]
    fn request_envelope_and_metadata_are_revalidated() {
        let mut invalid_api = request(
            "subscriptions.delete",
            json!({"subscriptionId": SUBSCRIPTION_ID}),
        );
        invalid_api["api"] = json!("other.control");
        assert!(matches!(
            parse_subscription_mutation_request(&invalid_api),
            Err(MutationProtocolError::InvalidRequest)
        ));
        for params in [
            json!({"subscriptionId": SUBSCRIPTION_ID, "operationId": ""}),
            json!({"subscriptionId": SUBSCRIPTION_ID, "operationId": "contains space"}),
            json!({"subscriptionId": SUBSCRIPTION_ID, "operationId": "x".repeat(65)}),
            json!({"subscriptionId": SUBSCRIPTION_ID, "expectedRevision": "1"}),
            json!({"subscriptionId": SUBSCRIPTION_ID, "expectedRevision": MAX_REVISION + 1}),
        ] {
            assert!(matches!(
                parse_subscription_mutation_request(&request("subscriptions.delete", params)),
                Err(MutationProtocolError::InvalidRequest)
            ));
        }
    }

    #[test]
    fn names_are_normalized_and_bounded_before_intent_creation() {
        for name in [
            String::new(),
            " \u{0}\u{1}\t ".to_owned(),
            "x".repeat(MAX_SUBSCRIPTION_NAME_INPUT_BYTES + 1),
            "x".repeat(MAX_SUBSCRIPTION_NAME_CHARS + 1),
        ] {
            assert!(matches!(
                parse_subscription_mutation_request(&request(
                    "subscriptions.add",
                    json!({"name": name, "url": URL})
                )),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
        assert!(
            parse_subscription_mutation_request(&request(
                "subscriptions.add",
                json!({"name": "🛡".repeat(MAX_SUBSCRIPTION_NAME_CHARS), "url": URL})
            ))
            .is_ok()
        );
    }

    #[test]
    fn urls_use_the_canonical_bounded_subscription_validator() {
        let legal_prefix = "https://example.test/";
        let max_legal = format!(
            "{legal_prefix}{}",
            "x".repeat(MAX_SUBSCRIPTION_URL_BYTES - legal_prefix.len())
        );
        for url in [URL.to_owned(), "http://localhost/sub".to_owned(), max_legal] {
            assert!(
                parse_subscription_mutation_request(&request(
                    "subscriptions.add",
                    json!({"name": "Provider", "url": url})
                ))
                .is_ok()
            );
        }
        for url in [
            "http://example.test/sub".to_owned(),
            "https://user@example.test/sub".to_owned(),
            "https://example.test/sub#fragment".to_owned(),
            "https://éxample.test/sub".to_owned(),
            "https://example.test:0/sub".to_owned(),
            "file:///tmp/sub".to_owned(),
            format!(
                "https://example.test/{}",
                "x".repeat(MAX_SUBSCRIPTION_URL_BYTES)
            ),
        ] {
            assert!(matches!(
                parse_subscription_mutation_request(&request(
                    "subscriptions.add",
                    json!({"name": "Provider", "url": url})
                )),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
    }

    #[test]
    fn subscription_ids_use_the_existing_internal_record_syntax() {
        for id in ["", "not-an-id", "00000000-0000-0000-0000-00000000000g"] {
            assert!(matches!(
                parse_subscription_mutation_request(&request(
                    "subscriptions.delete",
                    json!({"subscriptionId": id})
                )),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
        assert!(
            parse_subscription_mutation_request(&request(
                "subscriptions.delete",
                json!({"subscriptionId": SUBSCRIPTION_ID})
            ))
            .is_ok()
        );
    }

    #[test]
    fn digest_excludes_operation_id_and_covers_every_semantic_input() {
        let baseline = digest(
            "subscriptions.update",
            json!({
                "subscriptionId": SUBSCRIPTION_ID,
                "name": "Provider",
                "url": URL,
                "operationId": "operation-1",
                "expectedRevision": 4
            }),
        );
        assert!(
            baseline
                == digest(
                    "subscriptions.update",
                    json!({
                        "subscriptionId": SUBSCRIPTION_ID,
                        "name": "Provider",
                        "url": URL,
                        "operationId": "different-operation",
                        "expectedRevision": 4
                    })
                )
        );
        for changed in [
            digest(
                "subscriptions.update",
                json!({
                    "subscriptionId": "00000000-0000-4000-8000-000000000002",
                    "name": "Provider",
                    "url": URL,
                    "expectedRevision": 4
                }),
            ),
            digest(
                "subscriptions.update",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "name": "Other",
                    "url": URL,
                    "expectedRevision": 4
                }),
            ),
            digest(
                "subscriptions.update",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "name": "Provider",
                    "url": "https://example.test/other",
                    "expectedRevision": 4
                }),
            ),
            digest(
                "subscriptions.update",
                json!({
                    "subscriptionId": SUBSCRIPTION_ID,
                    "name": "Provider",
                    "url": URL,
                    "expectedRevision": 5
                }),
            ),
            digest(
                "subscriptions.update",
                json!({"subscriptionId": SUBSCRIPTION_ID, "name": "Provider", "url": URL}),
            ),
            digest(
                "subscriptions.add",
                json!({"name": "Provider", "url": URL, "expectedRevision": 4}),
            ),
            digest(
                "subscriptions.delete",
                json!({"subscriptionId": SUBSCRIPTION_ID, "expectedRevision": 4}),
            ),
        ] {
            assert!(baseline != changed);
        }
    }

    #[test]
    fn normalized_equivalent_values_have_the_same_digest() {
        assert!(
            digest("subscriptions.add", json!({"name": "Provider", "url": URL}))
                == digest(
                    "subscriptions.add",
                    json!({"name": "  Provider  ", "url": format!("  {URL}  ")})
                )
        );
    }

    #[test]
    fn digest_is_domain_separated_from_profile_mutations() {
        let subscription = digest(
            "subscriptions.delete",
            json!({"subscriptionId": SUBSCRIPTION_ID, "expectedRevision": 4}),
        );
        let profile = parse_profile_mutation_request(&request(
            "profiles.delete",
            json!({"profileId": SUBSCRIPTION_ID, "expectedRevision": 4}),
        ))
        .unwrap()
        .into_parts()
        .3;
        assert!(subscription != profile);
    }

    #[test]
    fn public_errors_never_echo_private_values() {
        let private = "https://user:password@private.example/subscription";
        let error = parse_subscription_mutation_request(&request(
            "subscriptions.add",
            json!({"name": "Private provider", "url": private}),
        ))
        .err()
        .expect("credential-bearing URL must be rejected");
        let public = format!("{error:?} {error}");
        assert!(!public.contains(private));
        assert!(!public.contains("password"));
        assert!(!public.contains("private.example"));
        assert!(!public.contains("Private provider"));
    }
}
