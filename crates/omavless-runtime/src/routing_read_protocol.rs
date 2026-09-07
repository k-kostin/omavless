// SPDX-License-Identifier: MIT
//! Exact bounded read for private custom-rule editor data.
use crate::mutation_protocol::MutationProtocolError;
use omavless_control_protocol::validate_request;
use serde_json::Value;

pub fn validate_custom_rules_request(request: &Value) -> Result<(), MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "routing.custom_rules.list" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"]
        .as_object()
        .is_some_and(|params| params.is_empty())
    {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn custom_rules_read_rejects_all_parameters_without_echo() {
        let request = |params| json!({"api":"omavless.control","version":1,"id":"rules","method":"routing.custom_rules.list","params":params});
        assert!(validate_custom_rules_request(&request(json!({}))).is_ok());
        for params in [
            json!([]),
            json!({"path":"private-token"}),
            json!({"limit":1}),
            json!({"operationId":"private-token"}),
            json!({"expectedRevision":0}),
        ] {
            let error = validate_custom_rules_request(&request(params)).unwrap_err();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        let mut wrong = request(json!({}));
        wrong["method"] = json!("routing.custom_rules.add");
        assert!(validate_custom_rules_request(&wrong).is_err());
    }
}
