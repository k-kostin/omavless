// SPDX-License-Identifier: MIT
//! Fixed statistics commands. No paths, devices, profile or provider input.
use super::worker::Control;
use crate::{
    mutation::MutationDigest,
    mutation_protocol::{MutationProtocolError, exact_fields, metadata},
};
use omavless_control_protocol::MAX_REVISION;
use serde_json::Value;

pub(crate) const METHODS: &[&str] = &["stats.get", "stats.configure", "stats.reset"];

pub(crate) struct Request {
    pub(crate) instance: String,
    pub(crate) operation: String,
    pub(crate) owner_revision: u64,
    pub(crate) stats_revision: u64,
    pub(crate) action: Control,
    pub(crate) digest: MutationDigest,
}

pub(crate) fn parse(request: &Value) -> Result<Request, MutationProtocolError> {
    let invalid = MutationProtocolError::InvalidArgument;
    omavless_control_protocol::validate_request(request).map_err(|_| invalid)?;
    let params = request["params"].as_object().ok_or(invalid)?;
    let fields = [
        "instanceId",
        "operationId",
        "expectedRevision",
        "statisticsRevision",
    ];
    let (action, required) = match request["method"].as_str() {
        Some("stats.configure") => (
            Control::Configure(
                params
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .ok_or(invalid)?,
            ),
            [fields.as_slice(), &["enabled"]].concat(),
        ),
        Some("stats.reset") => (Control::Reset, fields.to_vec()),
        _ => return Err(MutationProtocolError::UnknownMethod),
    };
    if !exact_fields(params, &required, &required) {
        return Err(invalid);
    }
    let meta = metadata(params)?;
    let operation = meta.operation_id.ok_or(invalid)?.to_owned();
    let owner_revision = meta.expected_revision.ok_or(invalid)?;
    let stats_revision = params["statisticsRevision"]
        .as_u64()
        .filter(|r| *r <= MAX_REVISION)
        .ok_or(invalid)?;
    let instance = params["instanceId"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| (0x21..=0x7e).contains(&b)))
        .ok_or(invalid)?
        .to_owned();
    let mut semantic = b"omavless/recorded-traffic-control/v1\0".to_vec();
    semantic.push(match action {
        Control::Configure(false) => 0,
        Control::Configure(true) => 1,
        Control::Reset => 2,
    });
    semantic.extend_from_slice(&owner_revision.to_be_bytes());
    semantic.extend_from_slice(&stats_revision.to_be_bytes());
    Ok(Request {
        instance,
        operation,
        owner_revision,
        stats_revision,
        action,
        digest: MutationDigest::from_semantic_bytes(&semantic),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn exact_semantic_shape_and_digest_reject_untrusted_or_ambiguous_input() {
        let params = json!({"instanceId":"instance","operationId":"one","expectedRevision":3,"statisticsRevision":2,"enabled":true});
        let make = |p| json!({"api":"omavless.control","version":1,"id":"stats","method":"stats.configure","params":p});
        let first = parse(&make(params.clone())).unwrap();
        let mut same = params.clone();
        same["operationId"] = "two".into();
        assert!(first.digest == parse(&make(same)).unwrap().digest);
        for (key, value) in [
            ("enabled", json!(1)),
            ("path", json!("private")),
            ("statisticsRevision", json!(-1)),
            ("operationId", json!("")),
            ("instanceId", json!("")),
        ] {
            let mut invalid = params.clone();
            invalid[key] = value;
            assert!(parse(&make(invalid)).is_err());
        }
        for key in [
            "instanceId",
            "operationId",
            "expectedRevision",
            "statisticsRevision",
            "enabled",
        ] {
            let mut invalid = params.clone();
            invalid.as_object_mut().unwrap().remove(key);
            assert!(parse(&make(invalid)).is_err());
        }
        let ordinary = b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"s\",\"method\":\"stats.configure\",\"params\":{\"enabled\":true}}\n";
        let duplicate = b"{\"api\":\"omavless.control\",\"version\":1,\"id\":\"s\",\"method\":\"stats.configure\",\"params\":{\"enabled\":true,\"enabled\":false}}\n";
        assert!(omavless_control_protocol::decode_request(ordinary).is_ok());
        assert!(omavless_control_protocol::decode_request(duplicate).is_err());
    }
}
