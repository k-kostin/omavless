// SPDX-License-Identifier: MIT
//! Explicit standalone editor read, separate from QR/file export.
use crate::mutation_protocol::{MutationProtocolError, exact_fields};
use omavless_control_protocol::validate_request;
use omavless_domain::store::valid_record_id;
use serde_json::Value;

pub struct ProfileEditInputRequest {
    profile_id: String,
}
impl ProfileEditInputRequest {
    pub(crate) fn private_profile_id(&self) -> &str {
        &self.profile_id
    }
}
pub fn parse_profile_edit_input_request(
    request: &Value,
) -> Result<ProfileEditInputRequest, MutationProtocolError> {
    parse_profile_read_request(request, "profiles.edit_input")
}
pub fn parse_profile_details_request(
    request: &Value,
) -> Result<ProfileEditInputRequest, MutationProtocolError> {
    parse_profile_read_request(request, "profiles.details")
}
fn parse_profile_read_request(
    request: &Value,
    method: &str,
) -> Result<ProfileEditInputRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != method {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(params, &["profileId"], &["profileId"]) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let profile_id = params
        .get("profileId")
        .and_then(Value::as_str)
        .filter(|id| valid_record_id(id))
        .ok_or(MutationProtocolError::InvalidArgument)?;
    Ok(ProfileEditInputRequest {
        profile_id: profile_id.to_owned(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const ID: &str = "00000000-0000-4000-8000-000000000001";
    #[test]
    fn profile_details_read_requires_only_valid_record_id() {
        let request = |params| json!({"api":"omavless.control","version":1,"id":"details","method":"profiles.details","params":params});
        assert!(parse_profile_details_request(&request(json!({"profileId":ID}))).is_ok());
        for params in [
            json!({}),
            json!({"profileId":false}),
            json!({"profileId":"private-token"}),
            json!({"profileId":ID,"operationId":"private-token"}),
            json!({"profileId":ID,"path":"private-token"}),
        ] {
            let error = parse_profile_details_request(&request(params))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        let mut wrong = request(json!({"profileId":ID}));
        wrong["method"] = json!("profiles.edit_input");
        assert!(parse_profile_details_request(&wrong).is_err());
    }
    #[test]
    fn profile_editor_read_accepts_only_one_valid_opaque_id() {
        let request = |params| json!({"api":"omavless.control","version":1,"id":"editor","method":"profiles.edit_input","params":params});
        assert!(parse_profile_edit_input_request(&request(json!({"profileId":ID}))).is_ok());
        for params in [
            json!({}),
            json!({"profileId":"private-token"}),
            json!({"profileId":false}),
            json!({"profileId":ID,"purpose":"edit"}),
            json!({"profileId":ID,"path":"private-token"}),
            json!({"profileId":ID,"operationId":"read-is-not-a-mutation"}),
        ] {
            let error = parse_profile_edit_input_request(&request(params))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        let mut wrong = request(json!({"profileId":ID}));
        wrong["method"] = json!("profiles.export");
        assert!(parse_profile_edit_input_request(&wrong).is_err());
    }
}
