// SPDX-License-Identifier: MIT
//! Fixed completion intent. No caller-provided flags, state, paths or host work.
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, exact_fields, metadata};
use omavless_control_protocol::validate_request;
use serde_json::Value;

pub(crate) struct OnboardingRequest {
    pub operation_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub digest: MutationDigest,
}

pub(crate) fn parse(request: &Value) -> Result<OnboardingRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "onboarding.complete" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, &["operationId", "expectedRevision"], &[]) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let metadata = metadata(params)?;
    let expected_revision = metadata.expected_revision;
    let mut identity = b"omavless/onboarding.complete/v1\0".to_vec();
    identity.extend(
        serde_json::to_vec(&expected_revision)
            .map_err(|_| MutationProtocolError::InvalidArgument)?,
    );
    Ok(OnboardingRequest {
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision,
        digest: MutationDigest::from_semantic_bytes(&identity),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(params: Value) -> Value {
        json!({"api":"omavless.control","version":1,"id":"onboarding",
            "method":"onboarding.complete","params":params})
    }

    #[test]
    fn exact_completion_metadata_and_safe_rejection() {
        assert!(parse(&request(json!({}))).is_ok());
        for params in [
            json!({"complete":false}),
            json!({"path":"private-marker"}),
            json!({"expectedRevision":-1}),
            json!({"expectedRevision":true}),
            json!({"operationId":""}),
            json!({"operationId":"private-marker /"}),
        ] {
            let error = parse(&request(params))
                .err()
                .expect("invalid request accepted");
            assert!(!format!("{error:?} {error}").contains("private-marker"));
        }
        let mut wrong = request(json!({}));
        wrong["method"] = json!("onboarding.reset");
        assert!(matches!(
            parse(&wrong),
            Err(MutationProtocolError::UnknownMethod)
        ));
    }

    #[test]
    fn replay_digest_covers_revision_presence_not_correlation() {
        let base = parse(&request(json!({}))).unwrap();
        let retry = parse(&request(json!({"operationId":"complete-1"}))).unwrap();
        let revision = parse(&request(json!({"expectedRevision":0}))).unwrap();
        assert!(base.digest == retry.digest);
        assert!(base.digest != revision.digest);
    }
}
