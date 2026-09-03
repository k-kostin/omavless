// SPDX-License-Identifier: MIT

//! Exact v1 profile-mutation parameter validation and semantic digesting.
//!
//! This parser is reachable only through ownership-gated runtime dispatch. It
//! produces a private domain mutation plus bounded scheduling metadata; none
//! of those values can be formatted or serialized.

use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use omavless_control_protocol::validate_request;
use omavless_domain::private_store::ProfileMutation;
use omavless_domain::store::valid_record_id;
use serde_json::{Map, Value};

const RENAME_FIELDS: &[&str] = &["profileId", "name", "operationId", "expectedRevision"];
const FAVORITE_FIELDS: &[&str] = &["profileId", "enabled", "operationId", "expectedRevision"];
const DELETE_FIELDS: &[&str] = &["profileId", "operationId", "expectedRevision"];

/// Eighty Unicode scalar values can occupy at most 320 UTF-8 bytes. The
/// domain layer performs canonical trimming/control removal and enforces the
/// final 80-character limit before any store write.
pub const MAX_PROFILE_NAME_INPUT_BYTES: usize = 320;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMutationKind {
    Rename,
    Favorite,
    Delete,
}

/// One parsed but not executed profile mutation. Private payload fields are
/// intentionally inaccessible except through the consuming owner boundary.
pub struct ProfileMutationRequest {
    mutation: ProfileMutation,
    operation_id: Option<String>,
    expected_revision: Option<u64>,
    digest: MutationDigest,
}

impl ProfileMutationRequest {
    #[must_use]
    pub const fn kind(&self) -> ProfileMutationKind {
        match &self.mutation {
            ProfileMutation::Rename { .. } => ProfileMutationKind::Rename,
            ProfileMutation::Favorite { .. } => ProfileMutationKind::Favorite,
            ProfileMutation::Delete { .. } => ProfileMutationKind::Delete,
        }
    }

    /// Consume the request at the future serialized owner boundary. None of
    /// these private values belongs in a control response or diagnostic log.
    #[must_use]
    #[allow(
        dead_code,
        reason = "offline until the serialized profile owner binding is accepted"
    )]
    pub(crate) fn into_parts(
        self,
    ) -> (ProfileMutation, Option<String>, Option<u64>, MutationDigest) {
        (
            self.mutation,
            self.operation_id,
            self.expected_revision,
            self.digest,
        )
    }
}

fn profile_id(params: &Map<String, Value>) -> Result<&str, MutationProtocolError> {
    params
        .get("profileId")
        .and_then(Value::as_str)
        .filter(|value| valid_record_id(value))
        .ok_or(MutationProtocolError::InvalidArgument)
}

fn semantic_digest(
    method: &str,
    profile_id: &str,
    variant_payload: &[u8],
    expected_revision: Option<u64>,
) -> MutationDigest {
    let mut bytes = Vec::with_capacity(160 + variant_payload.len());
    bytes.extend_from_slice(b"omavless.control/profile-mutation/v1\0");
    append_field(&mut bytes, method);
    append_field(&mut bytes, profile_id);
    bytes.extend_from_slice(&(variant_payload.len() as u32).to_be_bytes());
    bytes.extend_from_slice(variant_payload);
    match expected_revision {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    MutationDigest::from_semantic_bytes(&bytes)
}

/// Parse one bounded control request into an offline profile mutation.
///
/// The caller must already have decoded the request with the duplicate-key,
/// UTF-8, framing, depth and global string limits from the control protocol.
pub fn parse_profile_mutation_request(
    request: &Value,
) -> Result<ProfileMutationRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    let method = request["method"]
        .as_str()
        .ok_or(MutationProtocolError::InvalidRequest)?;
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let metadata = metadata(params)?;

    let (mutation, variant_payload) = match method {
        "profiles.rename" => {
            if !exact_fields(params, RENAME_FIELDS, &["profileId", "name"]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let profile_id = profile_id(params)?;
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && value.len() <= MAX_PROFILE_NAME_INPUT_BYTES)
                .ok_or(MutationProtocolError::InvalidArgument)?;
            let mut payload = Vec::with_capacity(name.len() + 5);
            payload.push(1);
            append_field(&mut payload, name);
            (
                ProfileMutation::Rename {
                    profile_id: profile_id.to_owned(),
                    new_name: name.to_owned(),
                },
                payload,
            )
        }
        "profiles.favorite" => {
            if !exact_fields(params, FAVORITE_FIELDS, &["profileId", "enabled"]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let profile_id = profile_id(params)?;
            let enabled = params
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(MutationProtocolError::InvalidArgument)?;
            (
                ProfileMutation::Favorite {
                    profile_id: profile_id.to_owned(),
                    enabled,
                },
                vec![2, u8::from(enabled)],
            )
        }
        "profiles.delete" => {
            if !exact_fields(params, DELETE_FIELDS, &["profileId"]) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let profile_id = profile_id(params)?;
            (
                ProfileMutation::Delete {
                    profile_id: profile_id.to_owned(),
                },
                vec![3],
            )
        }
        _ => return Err(MutationProtocolError::UnknownMethod),
    };

    let digest = semantic_digest(
        method,
        profile_id(params)?,
        &variant_payload,
        metadata.expected_revision,
    );
    Ok(ProfileMutationRequest {
        mutation,
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision: metadata.expected_revision,
        digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omavless_control_protocol::MAX_REVISION;
    use serde_json::json;

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000000001";

    fn request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "request-1",
            "method": method,
            "params": params,
        })
    }

    fn parsed(method: &str, params: Value) -> ProfileMutationRequest {
        parse_profile_mutation_request(&request(method, params)).unwrap()
    }

    #[test]
    fn exact_profile_mutation_shapes_are_accepted() {
        assert_eq!(
            parsed(
                "profiles.rename",
                json!({"profileId": PROFILE_ID, "name": "Renamed"})
            )
            .kind(),
            ProfileMutationKind::Rename
        );
        assert_eq!(
            parsed(
                "profiles.favorite",
                json!({
                    "profileId": PROFILE_ID,
                    "enabled": true,
                    "operationId": "operation-1",
                    "expectedRevision": MAX_REVISION
                })
            )
            .kind(),
            ProfileMutationKind::Favorite
        );
        assert_eq!(
            parsed("profiles.delete", json!({"profileId": PROFILE_ID})).kind(),
            ProfileMutationKind::Delete
        );
    }

    #[test]
    fn exact_private_payload_and_metadata_reach_the_domain_mutation() {
        let (mutation, operation_id, expected_revision, _) = parsed(
            "profiles.rename",
            json!({
                "profileId": PROFILE_ID,
                "name": "Renamed",
                "operationId": "operation-1",
                "expectedRevision": 9
            }),
        )
        .into_parts();
        match mutation {
            ProfileMutation::Rename {
                profile_id,
                new_name,
            } => {
                assert_eq!(profile_id, PROFILE_ID);
                assert_eq!(new_name, "Renamed");
            }
            _ => panic!("rename request mapped to a different mutation kind"),
        }
        assert_eq!(operation_id.as_deref(), Some("operation-1"));
        assert_eq!(expected_revision, Some(9));

        let (mutation, _, _, _) = parsed(
            "profiles.favorite",
            json!({"profileId": PROFILE_ID, "enabled": false}),
        )
        .into_parts();
        match mutation {
            ProfileMutation::Favorite {
                profile_id,
                enabled,
            } => {
                assert_eq!(profile_id, PROFILE_ID);
                assert!(!enabled);
            }
            _ => panic!("favorite request mapped to a different mutation kind"),
        }

        let (mutation, _, _, _) =
            parsed("profiles.delete", json!({"profileId": PROFILE_ID})).into_parts();
        match mutation {
            ProfileMutation::Delete { profile_id } => assert_eq!(profile_id, PROFILE_ID),
            _ => panic!("delete request mapped to a different mutation kind"),
        }
    }

    #[test]
    fn unknown_fields_and_wrong_argument_types_fail_closed() {
        let invalid = [
            ("profiles.rename", json!({"profileId": PROFILE_ID})),
            (
                "profiles.rename",
                json!({"profileId": PROFILE_ID, "name": false}),
            ),
            (
                "profiles.rename",
                json!({"profileId": PROFILE_ID, "name": "Name", "uri": "secret"}),
            ),
            (
                "profiles.favorite",
                json!({"profileId": PROFILE_ID, "enabled": "true"}),
            ),
            (
                "profiles.delete",
                json!({"profileId": PROFILE_ID, "enabled": true}),
            ),
            ("profiles.delete", json!({"profileId": "not-a-record-id"})),
        ];
        for (method, params) in invalid {
            assert!(matches!(
                parse_profile_mutation_request(&request(method, params)),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
        assert!(matches!(
            parse_profile_mutation_request(&request("profiles.create", json!({}))),
            Err(MutationProtocolError::UnknownMethod)
        ));
        for params in [
            json!({"profileId": PROFILE_ID, "operationId": 1}),
            json!({"profileId": PROFILE_ID, "expectedRevision": "1"}),
        ] {
            assert!(matches!(
                parse_profile_mutation_request(&request("profiles.delete", params)),
                Err(MutationProtocolError::InvalidRequest)
            ));
        }
    }

    #[test]
    fn rename_input_is_nonempty_and_method_bounded() {
        for name in [String::new(), "x".repeat(MAX_PROFILE_NAME_INPUT_BYTES + 1)] {
            assert!(matches!(
                parse_profile_mutation_request(&request(
                    "profiles.rename",
                    json!({"profileId": PROFILE_ID, "name": name})
                )),
                Err(MutationProtocolError::InvalidArgument)
            ));
        }
        assert!(
            parse_profile_mutation_request(&request(
                "profiles.rename",
                json!({
                    "profileId": PROFILE_ID,
                    "name": "🛡".repeat(MAX_PROFILE_NAME_INPUT_BYTES / 4)
                })
            ))
            .is_ok()
        );
    }

    #[test]
    fn digest_ignores_operation_id_and_covers_every_semantic_input() {
        fn digest(method: &str, params: Value) -> MutationDigest {
            parsed(method, params).into_parts().3
        }
        let baseline = digest(
            "profiles.rename",
            json!({
                "profileId": PROFILE_ID,
                "name": "One",
                "operationId": "operation-1",
                "expectedRevision": 4
            }),
        );
        assert!(
            baseline
                == digest(
                    "profiles.rename",
                    json!({
                        "profileId": PROFILE_ID,
                        "name": "One",
                        "operationId": "different-operation",
                        "expectedRevision": 4
                    })
                )
        );
        for changed in [
            digest(
                "profiles.rename",
                json!({"profileId": PROFILE_ID, "name": "Two", "expectedRevision": 4}),
            ),
            digest(
                "profiles.rename",
                json!({
                    "profileId": "00000000-0000-4000-8000-000000000002",
                    "name": "One",
                    "expectedRevision": 4
                }),
            ),
            digest(
                "profiles.rename",
                json!({"profileId": PROFILE_ID, "name": "One", "expectedRevision": 5}),
            ),
            digest(
                "profiles.rename",
                json!({"profileId": PROFILE_ID, "name": "One"}),
            ),
            digest(
                "profiles.favorite",
                json!({"profileId": PROFILE_ID, "enabled": true, "expectedRevision": 4}),
            ),
            digest(
                "profiles.favorite",
                json!({"profileId": PROFILE_ID, "enabled": false, "expectedRevision": 4}),
            ),
            digest(
                "profiles.delete",
                json!({"profileId": PROFILE_ID, "expectedRevision": 4}),
            ),
        ] {
            assert!(baseline != changed);
        }

        let favorite_true = digest(
            "profiles.favorite",
            json!({"profileId": PROFILE_ID, "enabled": true, "expectedRevision": 4}),
        );
        let favorite_false = digest(
            "profiles.favorite",
            json!({"profileId": PROFILE_ID, "enabled": false, "expectedRevision": 4}),
        );
        assert!(favorite_true != favorite_false);

        let delete_baseline = digest(
            "profiles.delete",
            json!({"profileId": PROFILE_ID, "expectedRevision": 4}),
        );
        assert!(
            delete_baseline
                != digest(
                    "profiles.delete",
                    json!({
                        "profileId": "00000000-0000-4000-8000-000000000002",
                        "expectedRevision": 4
                    })
                )
        );
        assert!(
            delete_baseline
                != digest(
                    "profiles.delete",
                    json!({"profileId": PROFILE_ID, "expectedRevision": 5})
                )
        );
    }

    #[test]
    fn public_errors_never_echo_private_values() {
        let private = "private.example/password";
        let error = parse_profile_mutation_request(&request(
            "profiles.rename",
            json!({"profileId": PROFILE_ID, "name": "Name", "uri": private}),
        ))
        .err()
        .expect("private extra field must be rejected");
        let public = format!("{error:?} {error}");
        assert!(!public.contains(private));
        assert!(!public.contains("password"));
        assert!(!public.contains("private.example"));
    }
}
