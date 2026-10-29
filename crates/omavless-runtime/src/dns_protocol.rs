// SPDX-License-Identifier: MIT
//! Enum-only DNS preference; no URLs, policy, paths or lifecycle commands.
use crate::mutation::MutationDigest;
use crate::mutation_protocol::{MutationProtocolError, exact_fields, metadata};
use omavless_domain::private_store::DnsPreset;
use serde_json::Value;

pub(crate) struct DnsRequest {
    pub preset: DnsPreset,
    pub operation_id: String,
    pub expected_revision: u64,
    pub digest: MutationDigest,
}

pub(crate) fn parse(request: &Value) -> Result<DnsRequest, MutationProtocolError> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "dns.configure" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"]
        .as_object()
        .ok_or(MutationProtocolError::InvalidArgument)?;
    if !exact_fields(
        params,
        &["preset", "operationId", "expectedRevision"],
        &["preset", "operationId", "expectedRevision"],
    ) {
        return Err(MutationProtocolError::InvalidArgument);
    }
    let preset = params["preset"]
        .as_str()
        .and_then(DnsPreset::parse)
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let meta = metadata(params)?;
    let operation_id = meta
        .operation_id
        .ok_or(MutationProtocolError::InvalidArgument)?
        .to_owned();
    let expected_revision = meta
        .expected_revision
        .ok_or(MutationProtocolError::InvalidArgument)?;
    let mut bytes = b"omavless/dns.configure/v1\0".to_vec();
    bytes.extend_from_slice(preset.id().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&expected_revision.to_be_bytes());
    Ok(DnsRequest {
        preset,
        operation_id,
        expected_revision,
        digest: MutationDigest::from_semantic_bytes(&bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request() -> Value {
        omavless_control_protocol::make_request(
            "dns",
            "dns.configure",
            json!({"preset":"google","operationId":"dns-1","expectedRevision":0}),
        )
        .unwrap()
    }
    #[test]
    fn fixed_ids_and_required_revision_metadata() {
        assert!(parse(&request()).is_ok());
        for (key, value) in [
            ("preset", json!("https://private-input.invalid/token")),
            ("preset", Value::Null),
            ("operationId", json!("")),
            ("expectedRevision", json!(-1)),
            ("url", json!("private-input")),
        ] {
            let mut invalid = request();
            invalid["params"][key] = value;
            let error = parse(&invalid).err().unwrap();
            assert!(!format!("{error:?} {error}").contains("private-input"));
        }
        for key in ["preset", "operationId", "expectedRevision"] {
            let mut invalid = request();
            invalid["params"].as_object_mut().unwrap().remove(key);
            assert!(parse(&invalid).is_err());
        }
        let first = request();
        let mut other = first.clone();
        other["params"]["operationId"] = json!("dns-2");
        assert!(parse(&first).unwrap().digest == parse(&other).unwrap().digest);
        other["params"]["preset"] = json!("cloudflare");
        assert!(parse(&first).unwrap().digest != parse(&other).unwrap().digest);
    }
}
