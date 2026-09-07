// SPDX-License-Identifier: MIT

//! Explicit credential release; no arbitrary file or program destination.

use crate::mutation_protocol::{MutationProtocolError, exact_fields};
use omavless_control_protocol::validate_request;
use omavless_domain::store::valid_record_id;
use serde_json::Value;

pub struct ProfileExportRequest {
    profile_id: String,
}

impl ProfileExportRequest {
    pub(crate) fn private_profile_id(&self) -> &str {
        &self.profile_id
    }
}

pub fn parse_profile_export_request(
    request: &Value,
) -> Result<ProfileExportRequest, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "profiles.export" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let fields = &["profileId", "purpose"];
    if !exact_fields(params, fields, fields)
        || !matches!(
            params.get("purpose").and_then(Value::as_str),
            Some("qr" | "file")
        )
    {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let profile_id = params
        .get("profileId")
        .and_then(Value::as_str)
        .filter(|id| valid_record_id(id))
        .ok_or(MutationProtocolError::InvalidArgument)?;
    Ok(ProfileExportRequest {
        profile_id: profile_id.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const ID: &str = "00000000-0000-4000-8000-000000000001";
    fn request(params: Value) -> Value {
        json!({"api":"omavless.control","version":1,"id":"export","method":"profiles.export","params":params})
    }
    #[test]
    fn only_explicit_qr_or_file_release_is_accepted() {
        for purpose in ["qr", "file"] {
            assert!(
                parse_profile_export_request(&request(json!({"profileId":ID,"purpose":purpose})))
                    .is_ok()
            );
        }
        for params in [
            json!({}),
            json!({"profileId":ID}),
            json!({"profileId":ID,"purpose":"edit"}),
            json!({"profileId":ID,"purpose":true}),
            json!({"profileId":"private-token","purpose":"file"}),
            json!({"profileId":ID,"purpose":"file","path":"private-token"}),
            json!({"profileId":ID,"purpose":"qr","operationId":"private-token"}),
        ] {
            let error = parse_profile_export_request(&request(params))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
    }
}
