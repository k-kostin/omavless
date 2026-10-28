// SPDX-License-Identifier: MIT
//! Fixed same-user daily settings. No URL, clock, path or Connect input.
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, exact_fields, metadata};
use omavless_control_protocol::validate_request;
use omavless_domain::store::valid_record_id;
use serde_json::Value;

pub(crate) struct Configure {
    pub id: String,
    pub enabled: bool,
    pub operation: String,
    pub revision: u64,
    pub digest: MutationDigest,
}
pub(crate) fn parse(request: &Value) -> Result<Configure, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "subscriptions.schedule.configure" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let fields = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let expected = &[
        "subscriptionId",
        "enabled",
        "operationId",
        "expectedRevision",
    ];
    if !exact_fields(fields, expected, expected) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let id = subscription_id(&fields["subscriptionId"])?;
    let enabled = fields["enabled"]
        .as_bool()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let meta = metadata(fields)?;
    let operation = meta
        .operation_id
        .ok_or(MutationProtocolError::InvalidArgument)?
        .to_owned();
    let revision = meta
        .expected_revision
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let mut bytes = b"omavless/subscriptions.schedule.configure/v1\0".to_vec();
    let mut identity = fields.clone();
    identity.remove("operationId");
    bytes
        .extend(serde_json::to_vec(&identity).map_err(|_| MutationProtocolError::InvalidArgument)?);
    Ok(Configure {
        id,
        enabled,
        operation,
        revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}
fn subscription_id(value: &Value) -> Result<String, MutationProtocolError> {
    value
        .as_str()
        .filter(|s| valid_record_id(s))
        .map(str::to_owned)
        .ok_or(MutationProtocolError::InvalidArgument)
}
pub(crate) fn parse_get(request: &Value) -> Result<String, MutationProtocolError> {
    validate_request(request).map_err(|_| MutationProtocolError::InvalidRequest)?;
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if request["method"] != "subscriptions.schedule.get"
        || !exact_fields(params, &["subscriptionId"], &["subscriptionId"])
    {
        return Err(MutationProtocolError::InvalidArgument);
    }
    subscription_id(&params["subscriptionId"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const ID: &str = "10000000-0000-4000-8000-000000000001";
    fn request() -> Value {
        omavless_control_protocol::make_request("daily", "subscriptions.schedule.configure", json!({"subscriptionId":ID,"enabled":true,"operationId":"settings-one","expectedRevision":0})).unwrap()
    }
    #[test]
    fn daily_schedule_fixed_shape_digest_and_private_refusals() {
        let original = request();
        let mut repeated = original.clone();
        repeated["params"]["operationId"] = json!("settings-two");
        assert!(parse(&original).unwrap().digest == parse(&repeated).unwrap().digest);
        repeated["params"]["enabled"] = json!(false);
        assert!(parse(&original).unwrap().digest != parse(&repeated).unwrap().digest);
        for (field, value) in [
            ("subscriptionId", json!("private-invalid")),
            ("enabled", json!("true")),
            ("operationId", json!("")),
            ("expectedRevision", json!(-1)),
            ("url", json!("https://private.invalid/secret")),
            ("nextDueAt", json!(0)),
            ("intervalSeconds", json!(1)),
        ] {
            let mut invalid = original.clone();
            invalid["params"][field] = value;
            let error = parse(&invalid).err().expect("invalid schedule accepted");
            assert!(!format!("{error:?} {error}").contains("private"));
        }
        for field in [
            "subscriptionId",
            "enabled",
            "operationId",
            "expectedRevision",
        ] {
            let mut invalid = original.clone();
            invalid["params"].as_object_mut().unwrap().remove(field);
            assert!(parse(&invalid).is_err());
        }
        let mut read = omavless_control_protocol::make_request(
            "read",
            "subscriptions.schedule.get",
            json!({"subscriptionId":ID}),
        )
        .unwrap();
        assert_eq!(parse_get(&read).unwrap(), ID);
        read["params"]["url"] = json!("private");
        assert!(parse_get(&read).is_err());
    }
}
