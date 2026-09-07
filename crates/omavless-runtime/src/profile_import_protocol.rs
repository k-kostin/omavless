// SPDX-License-Identifier: MIT

//! Confirmed new-profile import; private payload cannot be formatted.

use crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES;
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use crate::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES;
use omavless_control_protocol::validate_request;
use serde_json::Value;

pub struct ProfileImportRequest {
    pub(crate) name: String,
    pub(crate) input: String,
    pub(crate) operation_id: Option<String>,
    pub(crate) expected_revision: Option<u64>,
    pub(crate) digest: MutationDigest,
}

pub fn parse_profile_import_request(
    request: &Value,
) -> Result<ProfileImportRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "profiles.import" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(
        params,
        &["name", "input", "operationId", "expectedRevision"],
        &["name", "input"],
    ) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let name = params["name"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= MAX_PROFILE_NAME_INPUT_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let input = params["input"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= MAX_IMPORT_STDIN_BYTES)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let metadata = metadata(params)?;
    if !matches!(
        omavless_domain::import::preview_import(input, &[]),
        Ok(omavless_domain::import::ImportPreview::Profile(_))
    ) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let mut bytes = b"omavless.control/profile-import/v1\0".to_vec();
    append_field(&mut bytes, name);
    append_field(&mut bytes, input);
    match metadata.expected_revision {
        Some(revision) => {
            bytes.push(1);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        None => bytes.push(0),
    }
    Ok(ProfileImportRequest {
        name: name.to_owned(),
        input: input.to_owned(),
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision: metadata.expected_revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(params: Value) -> Value {
        json!({"api": "omavless.control", "version": 1, "id": "import",
               "method": "profiles.import", "params": params})
    }
    #[test]
    fn import_rejects_client_ids_paths_and_unknown_input() {
        for params in [
            json!({}),
            json!({"name": "Name", "input": null}),
            json!({"name": " ", "input": "private-token"}),
            json!({"name": "Name", "input": "private-token", "profileId": "generated"}),
            json!({"name": "Name", "input": "private-token", "oldId": "replace"}),
            json!({"name": "Name", "input": "private-token", "path": "/tmp/input"}),
            json!({"name": "Name", "input": "x".repeat(MAX_IMPORT_STDIN_BYTES+1)}),
            json!({"name": "x".repeat(MAX_PROFILE_NAME_INPUT_BYTES+1), "input": "private-token"}),
        ] {
            let error = parse_profile_import_request(&request(params))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
    }
    #[test]
    fn import_digest_binds_private_intent_and_revision_but_not_request_id() {
        let original = request(
            json!({"name": "Name", "input": "trojan://synthetic-password@203.0.113.1:443", "operationId": "op"}),
        );
        let parsed = parse_profile_import_request(&original).ok().unwrap();
        for (field, value) in [
            ("name", json!("Other")),
            ("input", json!("trojan://other-password@203.0.113.1:443")),
            ("expectedRevision", json!(0)),
        ] {
            let mut changed = original.clone();
            changed["params"][field] = value;
            assert!(parse_profile_import_request(&changed).ok().unwrap().digest != parsed.digest);
        }
        let mut retry = original.clone();
        retry["id"] = json!("retry");
        assert!(parse_profile_import_request(&retry).ok().unwrap().digest == parsed.digest);
    }
}
