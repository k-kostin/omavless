// SPDX-License-Identifier: MIT

//! Fixed frontend lifecycle bridge. Instance fencing is performed by dispatch
//! before passing the canonical request to the existing mutation coordinator.

use crate::mutation_protocol::{MutationProtocolError, exact_fields, parse_owner_request};
use serde_json::{Value, json};
use std::ffi::OsString;

pub(crate) struct Action {
    pub instance: String,
    pub operation: String,
    pub action: String,
    pub canonical: Value,
}

pub(crate) fn parse(request: &Value) -> Result<Action, MutationProtocolError> {
    use MutationProtocolError::InvalidArgument;
    omavless_control_protocol::validate_request(request)
        .map_err(|_| MutationProtocolError::InvalidRequest)?;
    if request["method"] != "plugin.action" {
        return Err(MutationProtocolError::UnknownMethod);
    }
    let params = request["params"].as_object().ok_or(InvalidArgument)?;
    let action = params
        .get("action")
        .and_then(Value::as_str)
        .ok_or(InvalidArgument)?;
    let fields: &[&str] = match action {
        "connect" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "profileId",
            "mode",
        ],
        "disconnect" => &["instanceId", "expectedRevision", "operationId", "action"],
        "mode" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "mode",
        ],
        _ => return Err(InvalidArgument),
    };
    if !exact_fields(params, fields, fields) {
        return Err(InvalidArgument);
    }
    let instance = params["instanceId"]
        .as_str()
        .filter(|value| {
            !value.is_empty()
                && value.len() <= crate::long_operation_protocol::MAX_INSTANCE_ID_BYTES
                && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
        .ok_or(InvalidArgument)?;
    let mut canonical = request.clone();
    canonical["method"] = json!(match action {
        "connect" => "connection.connect",
        "disconnect" => "connection.disconnect",
        _ => "routing.set_mode",
    });
    let mapped = canonical["params"].as_object_mut().ok_or(InvalidArgument)?;
    mapped.remove("instanceId");
    mapped.remove("action");
    parse_owner_request(&canonical)?;
    Ok(Action {
        instance: instance.to_owned(),
        operation: params["operationId"]
            .as_str()
            .ok_or(InvalidArgument)?
            .to_owned(),
        action: action.to_owned(),
        canonical,
    })
}

/// Parse only the three fixed action commands. No stdin, generic JSON or method
/// names are accepted. The returned parameters still require server fencing.
pub fn cli_params(
    arguments: &[OsString],
) -> Result<Option<Value>, crate::semantic_cli::SemanticCliError> {
    use crate::semantic_cli::SemanticCliError::InvalidArgument;
    if arguments.first().is_none_or(|arg| arg != "plugin")
        || arguments
            .get(1)
            .is_none_or(|arg| !["connect", "disconnect", "mode"].iter().any(|v| arg == v))
    {
        return Ok(None);
    }
    let args: Vec<&str> = arguments
        .iter()
        .map(|arg| arg.to_str().ok_or(InvalidArgument))
        .collect::<Result<_, _>>()?;
    let (action, instance, revision, operation, profile, mode) = match args.as_slice() {
        [
            "plugin",
            "connect",
            instance,
            revision,
            operation,
            profile,
            mode,
        ] => (
            "connect",
            *instance,
            *revision,
            *operation,
            Some(*profile),
            Some(*mode),
        ),
        ["plugin", "disconnect", instance, revision, operation] => {
            ("disconnect", *instance, *revision, *operation, None, None)
        }
        ["plugin", "mode", instance, revision, operation, mode] => {
            ("mode", *instance, *revision, *operation, None, Some(*mode))
        }
        _ => return Err(InvalidArgument),
    };
    if revision.is_empty() || !revision.bytes().all(|b| b.is_ascii_digit()) {
        return Err(InvalidArgument);
    }
    let revision = revision.parse::<u64>().map_err(|_| InvalidArgument)?;
    let mut params = json!({"action":action,"instanceId":instance,"expectedRevision":revision,"operationId":operation});
    if let Some(profile) = profile {
        params["profileId"] = json!(profile);
    }
    if let Some(mode) = mode {
        params["mode"] = json!(mode);
    }
    let request =
        omavless_control_protocol::make_request("plugin-cli", "plugin.action", params.clone())
            .map_err(|_| InvalidArgument)?;
    parse(&request).map_err(|_| InvalidArgument)?;
    Ok(Some(params))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(params: Value) -> Value {
        // Invalid cases must reach the parser rather than the checked builder.
        json!({"api":"omavless.control","version":1,"id":"test","method":"plugin.action","params":params})
    }
    #[test]
    fn exact_actions_reuse_canonical_validation() {
        for (action, method, extra) in [
            (
                "connect",
                "connection.connect",
                json!({"profileId":"00000000-0000-4000-8000-000000000001","mode":"rule"}),
            ),
            ("disconnect", "connection.disconnect", json!({})),
            ("mode", "routing.set_mode", json!({"mode":"global"})),
        ] {
            let mut params = json!({"instanceId":"instance-1","expectedRevision":0,"operationId":"operation-1","action":action});
            params
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let parsed = parse(&request(params.clone())).unwrap();
            assert_eq!(parsed.canonical["method"], method);
            assert!(parsed.canonical["params"].get("instanceId").is_none());
            for required in params.as_object().unwrap().keys() {
                let mut missing = params.clone();
                missing.as_object_mut().unwrap().remove(required);
                assert!(parse(&request(missing)).is_err());
            }
            params["privateExtra"] = json!("not echoed");
            assert!(parse(&request(params)).is_err());
        }
    }
    #[test]
    fn invalid_metadata_and_cli_bounds_are_rejected() {
        let valid = json!({"action":"disconnect","instanceId":"instance-1","operationId":"op-1","expectedRevision":0});
        for (key, value) in [
            ("instanceId", json!("")),
            ("instanceId", json!("x".repeat(129))),
            ("instanceId", json!("bad\nvalue")),
            ("operationId", json!("")),
            ("operationId", json!("x".repeat(65))),
            ("expectedRevision", json!(-1)),
            ("expectedRevision", json!(u64::MAX)),
            ("expectedRevision", Value::Null),
        ] {
            let mut params = valid.clone();
            params[key] = value;
            assert!(parse(&request(params)).is_err());
        }
        let args = |items: &[&str]| items.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(
            cli_params(&args(&["plugin", "disconnect", "instance-1", "0", "op-1"])).unwrap(),
            Some(valid)
        );
        for command in [
            vec!["plugin", "disconnect"],
            vec!["plugin", "disconnect", "i", "+1", "o"],
            vec!["plugin", "disconnect", "i", "0", "o", "extra"],
            vec!["plugin", "mode", "i", "0", "o", "bad"],
        ] {
            assert!(cli_params(&args(&command)).is_err());
        }
    }
}
