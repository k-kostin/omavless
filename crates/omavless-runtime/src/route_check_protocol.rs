// SPDX-License-Identifier: MIT
use crate::mutation_protocol::MutationProtocolError;
use omavless_control_protocol::validate_request;
use serde_json::Value;

pub(crate) fn query(request: &Value) -> Result<&str, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "routing.check" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if params.len() != 1 {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let query = params
        .get("query")
        .and_then(Value::as_str)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    omavless_domain::route_check::canonical_query(query)
        .map_err(|_| MutationProtocolError::InvalidArgument)?;
    Ok(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn route_query_envelope_is_exact_bounded_and_credential_safe() {
        let make = |params| json!({"api":"omavless.control","version":1,"id":"route-check","method":"routing.check","params":params});
        assert_eq!(
            query(&make(json!({"query":"example.invalid"}))).unwrap(),
            "example.invalid"
        );
        for params in [
            json!({}),
            json!({"query":false}),
            json!({"query":[]}),
            json!({"query":"example.invalid","operationId":"route-op"}),
            json!({"query":"example.invalid","expectedRevision":0}),
            json!({"query":"example.invalid","connected":false}),
            json!({"query":"example.invalid","rules":[]}),
            json!({"query":"x".repeat(1025)}),
            json!({"query":"https://private-token.invalid/key"}),
            json!({"query":"fe80::1%private-token"}),
        ] {
            let error = query(&make(params)).unwrap_err();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        let mut wrong = make(json!({"query":"example.invalid"}));
        wrong["method"] = json!("routing.other");
        assert_eq!(query(&wrong), Err(MutationProtocolError::UnknownMethod));
    }
}
