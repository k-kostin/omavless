// SPDX-License-Identifier: MIT
//! Fixed local bundle confirmation. The preview hash binds data, never authority.

use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) struct BundleRequest {
    pub input: String,
    pub fingerprint: String,
    pub operation_id: String,
    pub revision: u64,
    pub digest: MutationDigest,
}

pub(crate) fn fingerprint(store: &str, input: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"omavless/local-bundle-preview/v1\0");
    hash.update((store.len() as u64).to_be_bytes());
    hash.update(store.as_bytes());
    hash.update((input.len() as u64).to_be_bytes());
    hash.update(input.as_bytes());
    format!("{:x}", hash.finalize())
}

pub(crate) fn parse(request: &Value) -> Result<BundleRequest, MutationProtocolError> {
    use MutationProtocolError::InvalidArgument;
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "profiles.importBundle" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"].as_object().ok_or(InvalidArgument)?;
    let fields = &[
        "input",
        "previewFingerprint",
        "operationId",
        "expectedRevision",
    ];
    if !exact_fields(params, fields, fields) {
        return Err(InvalidArgument);
    }
    let input = params["input"]
        .as_str()
        .filter(|s| {
            !s.trim().is_empty() && s.len() <= crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES
        })
        .ok_or(InvalidArgument)?;
    let fingerprint = params["previewFingerprint"]
        .as_str()
        .filter(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or(InvalidArgument)?;
    let metadata = metadata(params)?;
    let operation_id = metadata.operation_id.ok_or(InvalidArgument)?;
    let revision = metadata.expected_revision.ok_or(InvalidArgument)?;
    let mut bytes = b"omavless.control/profile-bundle-import/v1\0".to_vec();
    append_field(&mut bytes, input);
    append_field(&mut bytes, fingerprint);
    bytes.extend_from_slice(&revision.to_be_bytes());
    Ok(BundleRequest {
        input: input.into(),
        fingerprint: fingerprint.into(),
        operation_id: operation_id.into(),
        revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn binding_changes_for_exact_store_or_input_changes() {
        assert_ne!(
            fingerprint("store", "input"),
            fingerprint("store\n", "input")
        );
        assert_ne!(
            fingerprint("store", "input"),
            fingerprint("store", "input\n")
        );
    }
    #[test]
    fn confirmation_requires_all_fixed_fields_and_bounded_private_input() {
        let params = json!({"input":"synthetic", "previewFingerprint":"a".repeat(64),
            "operationId":"bundle", "expectedRevision":0});
        let request = |params| {
            json!({"api":"omavless.control", "version":1, "id":"bundle",
            "method":"profiles.importBundle", "params":params})
        };
        assert!(parse(&request(params.clone())).is_ok());
        for field in [
            "input",
            "previewFingerprint",
            "operationId",
            "expectedRevision",
        ] {
            let mut changed = params.clone();
            changed.as_object_mut().unwrap().remove(field);
            assert!(parse(&request(changed)).is_err());
        }
        for (field, value) in [
            ("path", json!("/synthetic")),
            ("input", json!("x".repeat(32769))),
            ("previewFingerprint", json!("A".repeat(64))),
        ] {
            let mut changed = params.clone();
            changed[field] = value;
            assert!(parse(&request(changed)).is_err());
        }
    }
}
