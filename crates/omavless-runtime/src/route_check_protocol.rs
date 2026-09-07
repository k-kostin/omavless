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
