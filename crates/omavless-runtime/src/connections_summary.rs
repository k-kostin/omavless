// SPDX-License-Identifier: MIT
//! Count-only controller projection. Never expose individual connection metadata.
use crate::mutation_protocol::MutationProtocolError;
use serde_json::{Value, json};

pub(crate) const MAX_CONNECTIONS: usize = 4096;

pub(crate) fn validate(request: &Value) -> Result<(), MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "runtime.connections" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    if !request["params"].as_object().is_some_and(|p| p.is_empty()) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    Ok(())
}

pub(crate) fn count(payload: &Value) -> Option<u32> {
    match payload.get("connections")? {
        Value::Null => Some(0),
        Value::Array(rows)
            if rows.len() <= MAX_CONNECTIONS && rows.iter().all(Value::is_object) =>
        {
            u32::try_from(rows.len()).ok()
        }
        _ => None,
    }
}

pub(crate) fn project(count: Option<u32>) -> Value {
    let count = count.filter(|n| *n <= MAX_CONNECTIONS as u32);
    json!({"schemaVersion":1,"scope":"owned_core_active_connection_count",
        "availability":if count.is_some(){"observed"}else{"unavailable"},"count":count})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_only_objects_within_bound_and_null_means_empty() {
        assert_eq!(count(&json!({"connections":null})), Some(0));
        assert_eq!(count(&json!({"connections":[]})), Some(0));
        assert_eq!(
            count(&json!({"connections":vec![json!({});MAX_CONNECTIONS]})),
            Some(4096)
        );
        for bad in [
            json!({}),
            json!({"connections":false}),
            json!({"connections":[null]}),
            json!({"connections":vec![json!({});MAX_CONNECTIONS+1]}),
        ] {
            assert_eq!(count(&bad), None);
        }
    }
    #[test]
    fn private_rows_and_unknown_availability_are_not_public() {
        let payload = json!({"connections":[{"metadata":{"host":"private.invalid","process":"secret"},"chains":["private-profile"]}],"downloadTotal":999});
        let value = project(count(&payload));
        assert_eq!(value["count"], 1);
        assert!(value.to_string().len() < 160);
        for secret in ["private", "secret", "chains", "downloadTotal"] {
            assert!(!value.to_string().contains(secret));
        }
        for unavailable in [None, Some(4097), Some(u32::MAX)] {
            let value = project(unavailable);
            assert!(value["count"].is_null());
            assert_eq!(value["availability"], "unavailable");
        }
    }
    #[test]
    fn no_caller_controller_path_filter_or_endpoint() {
        for params in [
            json!({}),
            json!({"path":"/connections"}),
            json!({"profileId":"private"}),
            json!([]),
        ] {
            let request = json!({"api":"omavless.control","version":1,"id":"test","method":"runtime.connections","params":params});
            assert_eq!(validate(&request).is_ok(), params == json!({}));
        }
    }
}
