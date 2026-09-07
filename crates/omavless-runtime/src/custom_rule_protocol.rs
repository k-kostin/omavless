// SPDX-License-Identifier: MIT
//! Exact private routing mutations; no caller-generated IDs or host inputs.
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, append_field, exact_fields, metadata};
use omavless_domain::private_store::CustomRuleMutation;
use omavless_domain::routing::{CustomRule, MAX_CUSTOM_RULE_VALUE_BYTES};
use serde_json::Value;

pub(crate) struct CustomRuleRequest {
    pub mutation: CustomRuleMutation,
    pub operation_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub digest: MutationDigest,
}

pub(crate) fn parse(request: &Value) -> Result<CustomRuleRequest, MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let metadata = metadata(params)?;
    let method = request["method"]
        .as_str()
        .ok_or(MutationProtocolError::InvalidRequest)?;
    let mut bytes = b"omavless.control/custom-rule/v1\0".to_vec();
    append_field(&mut bytes, method);
    let mutation = match method {
        "routing.custom_rules.add" => {
            if !exact_fields(
                params,
                &["kind", "action", "value", "operationId", "expectedRevision"],
                &["kind", "action", "value"],
            ) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let text = |key| {
                params
                    .get(key)
                    .and_then(Value::as_str)
                    .ok_or(MutationProtocolError::InvalidArgument)
            };
            let (kind, action, value) = (text("kind")?, text("action")?, text("value")?);
            if value.len() > MAX_CUSTOM_RULE_VALUE_BYTES {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let rule = CustomRule::parse(kind, action, value)
                .map_err(|_| MutationProtocolError::InvalidArgument)?;
            for field in [kind, action, rule.value.as_str()] {
                append_field(&mut bytes, field);
            }
            CustomRuleMutation::Add {
                kind: kind.into(),
                action: action.into(),
                value: rule.value,
            }
        }
        "routing.custom_rules.delete" => {
            if !exact_fields(
                params,
                &["ruleId", "operationId", "expectedRevision"],
                &["ruleId"],
            ) {
                return Err(MutationProtocolError::InvalidArgument);
            }
            let id = params["ruleId"]
                .as_str()
                .filter(|id| omavless_domain::store::valid_record_id(id))
                .ok_or(MutationProtocolError::InvalidArgument)?;
            append_field(&mut bytes, id);
            CustomRuleMutation::Delete { rule_id: id.into() }
        }
        _ => return Err(MutationProtocolError::UnknownMethod),
    };
    match metadata.expected_revision {
        Some(revision) => {
            bytes.push(1);
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        None => bytes.push(0),
    }
    Ok(CustomRuleRequest {
        mutation,
        operation_id: metadata.operation_id.map(str::to_owned),
        expected_revision: metadata.expected_revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(method: &str, params: Value) -> Value {
        json!({"api":"omavless.control","version":1,"id":"rules","method":method,"params":params})
    }
    #[test]
    fn custom_rule_exact_shapes_bounds_and_private_errors() {
        let params = json!({"kind":"domain","action":"direct","value":"example.invalid"});
        assert!(parse(&request("routing.custom_rules.add", params.clone())).is_ok());
        for (key, value) in [
            ("kind", json!(false)),
            ("action", json!("shell")),
            ("value", json!("private-token/password")),
            ("value", json!("x".repeat(1025))),
            ("path", json!("private-token")),
            ("ruleId", json!("00000000-0000-4000-8000-000000000001")),
            ("expectedRevision", json!(-1)),
            ("operationId", json!("")),
        ] {
            let mut invalid = params.clone();
            invalid[key] = value;
            let error = parse(&request("routing.custom_rules.add", invalid))
                .err()
                .unwrap();
            assert!(!format!("{error:?} {error}").contains("private-token"));
        }
        for params in [
            json!({}),
            json!({"ruleId":"invalid"}),
            json!({"ruleId":"00000000-0000-4000-8000-000000000001","value":"private-token"}),
        ] {
            assert!(parse(&request("routing.custom_rules.delete", params)).is_err());
        }
    }
    #[test]
    fn custom_rule_digest_normalizes_and_covers_all_intent() {
        let baseline = json!({"kind":"domain","action":"direct","value":"example.invalid","operationId":"one"});
        let digest = parse(&request("routing.custom_rules.add", baseline.clone()))
            .unwrap()
            .digest;
        let mut equivalent = baseline.clone();
        equivalent["operationId"] = json!("two");
        equivalent["value"] = json!(" EXAMPLE.INVALID. ");
        assert!(
            parse(&request("routing.custom_rules.add", equivalent))
                .unwrap()
                .digest
                == digest
        );
        for (key, value) in [
            ("kind", json!("suffix")),
            ("action", json!("reject")),
            ("value", json!("other.example.invalid")),
            ("expectedRevision", json!(0)),
        ] {
            let mut changed = baseline.clone();
            changed[key] = value;
            assert!(
                parse(&request("routing.custom_rules.add", changed))
                    .unwrap()
                    .digest
                    != digest
            );
        }
    }
}
