// SPDX-License-Identifier: MIT

//! Fixed frontend mutation bridge. Instance fencing is performed by dispatch
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
        "profile-rename" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "profileId",
            "name",
        ],
        "profile-favorite" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "profileId",
            "enabled",
        ],
        "profile-delete" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "profileId",
        ],
        "profile-replace" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "profileId",
            "name",
            "input",
        ],
        "profile-import" => &[
            "instanceId",
            "expectedRevision",
            "operationId",
            "action",
            "name",
            "input",
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
        "mode" => "routing.set_mode",
        "profile-rename" => "profiles.rename",
        "profile-favorite" => "profiles.favorite",
        "profile-delete" => "profiles.delete",
        "profile-replace" => "profiles.replace",
        "profile-import" => "profiles.import",
        _ => return Err(InvalidArgument),
    });
    let mapped = canonical["params"].as_object_mut().ok_or(InvalidArgument)?;
    mapped.remove("instanceId");
    mapped.remove("action");
    if action == "profile-import" {
        crate::profile_import_protocol::parse_profile_import_request(&canonical)?;
    } else if action.starts_with("profile-") {
        crate::profile_mutation_protocol::parse_profile_mutation_request(&canonical)?;
    } else {
        parse_owner_request(&canonical)?;
    }
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

/// Private profile input is bounded before socket connection. The extra byte
/// permits one terminal LF; record IDs have the canonical UUID length.
pub fn cli_input_limit(arguments: &[OsString]) -> Option<usize> {
    if arguments.len() != 5 || arguments[0] != "plugin" {
        return None;
    }
    match arguments[1].to_str()? {
        "profile-rename" => {
            Some(36 + 1 + crate::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES + 1)
        }
        "profile-favorite" => Some(36 + 1 + 3 + 1),
        "profile-delete" => Some(36 + 1),
        "profile-replace" => Some(36 + 1 + crate::semantic_cli::MAX_PROFILE_IMPORT_STDIN_BYTES),
        "profile-import" => Some(crate::semantic_cli::MAX_PROFILE_IMPORT_STDIN_BYTES),
        _ => None,
    }
}

/// Parse only fixed actions, never generic JSON/method names. Profile inputs
/// travel through bounded stdin, not argv. Server fencing remains mandatory.
pub fn cli_params(
    arguments: &[OsString],
    private_stdin: Option<&str>,
) -> Result<Option<Value>, crate::semantic_cli::SemanticCliError> {
    use crate::semantic_cli::SemanticCliError::InvalidArgument;
    if arguments.first().is_none_or(|arg| arg != "plugin")
        || arguments.get(1).is_none_or(|arg| {
            ![
                "connect",
                "disconnect",
                "mode",
                "profile-rename",
                "profile-favorite",
                "profile-delete",
                "profile-replace",
                "profile-import",
            ]
            .iter()
            .any(|v| arg == v)
        })
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
        [
            "plugin",
            action @ ("profile-rename" | "profile-favorite" | "profile-delete" | "profile-import" | "profile-replace"),
            instance,
            revision,
            operation,
        ] => (*action, *instance, *revision, *operation, None, None),
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
    if action == "profile-replace" {
        let input = private_stdin.ok_or(crate::semantic_cli::SemanticCliError::MissingInput)?;
        if input.len() > cli_input_limit(arguments).ok_or(InvalidArgument)? {
            return Err(crate::semantic_cli::SemanticCliError::InputTooLarge);
        }
        let (id, replacement) = input.split_once('\n').ok_or(InvalidArgument)?;
        let (_, replaced) = crate::semantic_cli::parse_semantic_profile_replace(
            &["profile".into(), "replace".into(), id.into()],
            Some(replacement),
        )?
        .into_parts();
        params
            .as_object_mut()
            .ok_or(InvalidArgument)?
            .extend(replaced.as_object().ok_or(InvalidArgument)?.clone());
    } else     if action == "profile-import" {
        let (_, imported) = crate::semantic_cli::parse_semantic_profile_import(
            &["profile".into(), "import".into()],
            private_stdin,
        )?
        .into_parts();
        params
            .as_object_mut()
            .ok_or(InvalidArgument)?
            .extend(imported.as_object().ok_or(InvalidArgument)?.clone());
    } else if action.starts_with("profile-") {
        let input = private_stdin.ok_or(crate::semantic_cli::SemanticCliError::MissingInput)?;
        if input.len() > cli_input_limit(arguments).ok_or(InvalidArgument)? {
            return Err(crate::semantic_cli::SemanticCliError::InputTooLarge);
        }
        let input = input.strip_suffix('\n').unwrap_or(input);
        if input.contains('\r') || input.contains('\0') {
            return Err(InvalidArgument);
        }
        let (id, value) = if action == "profile-delete" {
            (input, None)
        } else {
            let (id, value) = input.split_once('\n').ok_or(InvalidArgument)?;
            (id, Some(value))
        };
        if id.contains('\n') || value.is_some_and(|value| value.contains('\n')) {
            return Err(InvalidArgument);
        }
        params["profileId"] = json!(id);
        if action == "profile-rename" {
            params["name"] = json!(value.ok_or(InvalidArgument)?);
        } else if action == "profile-favorite" {
            params["enabled"] = json!(match value {
                Some("on") => true,
                Some("off") => false,
                _ => return Err(InvalidArgument),
            });
        }
    } else if private_stdin.is_some() {
        return Err(InvalidArgument);
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
            (
                "profile-replace",
                "profiles.replace",
                json!({"profileId":"00000000-0000-4000-8000-000000000001","name":"Replaced","input":"trojan://synthetic-password@203.0.113.1:443"}),
            ),
            (
                "profile-import",
                "profiles.import",
                json!({"name":"Imported", "input":"trojan://synthetic-password@203.0.113.1:443"}),
            ),
            (
                "profile-rename",
                "profiles.rename",
                json!({"profileId":"00000000-0000-4000-8000-000000000001","name":"Private label"}),
            ),
            (
                "profile-favorite",
                "profiles.favorite",
                json!({"profileId":"00000000-0000-4000-8000-000000000001","enabled":true}),
            ),
            (
                "profile-delete",
                "profiles.delete",
                json!({"profileId":"00000000-0000-4000-8000-000000000001"}),
            ),
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
            cli_params(
                &args(&["plugin", "disconnect", "instance-1", "0", "op-1"]),
                None
            )
            .unwrap(),
            Some(valid)
        );
        for command in [
            vec!["plugin", "disconnect"],
            vec!["plugin", "disconnect", "i", "+1", "o"],
            vec!["plugin", "disconnect", "i", "0", "o", "extra"],
            vec!["plugin", "mode", "i", "0", "o", "bad"],
        ] {
            assert!(cli_params(&args(&command), None).is_err());
        }
    }

    #[test]
    fn private_profile_cli_has_exact_line_and_argument_grammar() {
        let id = "00000000-0000-4000-8000-000000000001";
        for (action, input, extra) in [
            (
                "profile-rename",
                format!("{id}\nPrivate name"),
                json!({"name":"Private name"}),
            ),
            (
                "profile-favorite",
                format!("{id}\non"),
                json!({"enabled":true}),
            ),
            (
                "profile-favorite",
                format!("{id}\noff"),
                json!({"enabled":false}),
            ),
            ("profile-delete", id.to_owned(), json!({})),
        ] {
            let args: Vec<_> = ["plugin", action, "instance", "7", "operation"]
                .map(OsString::from)
                .into();
            for input in [input.clone(), format!("{input}\n")] {
                let parsed = cli_params(&args, Some(&input)).unwrap().unwrap();
                assert_eq!(parsed["profileId"], id);
                for (key, value) in extra.as_object().unwrap() {
                    assert_eq!(parsed[key], *value);
                }
            }
            assert!(cli_params(&args, None).is_err());
            for invalid in [
                String::new(),
                "private-token".into(),
                format!("{input}\nextra"),
                format!("{input}\n\n"),
                format!("{input}\r"),
                format!("{input}\0"),
            ] {
                assert!(cli_params(&args, Some(&invalid)).is_err());
            }
            let mut extra_arg = args.clone();
            extra_arg.push(OsString::from("private-token"));
            assert_eq!(cli_input_limit(&extra_arg), None);
            assert!(cli_params(&extra_arg, Some(&input)).is_err());
        }
        let args: Vec<_> = ["plugin", "profile-rename", "instance", "7", "operation"]
            .map(OsString::from)
            .into();
        assert!(cli_params(&args, Some(&format!("{id}\n{}", "x".repeat(320)))).is_ok());
        assert!(cli_params(&args, Some(&format!("{id}\n{}", "x".repeat(321)))).is_err());
        assert!(cli_params(&args, Some(&format!("{id}\n{}", "🛡".repeat(81)))).is_err());
        let args: Vec<_> = ["plugin", "profile-favorite", "instance", "7", "operation"]
            .map(OsString::from)
            .into();
        for value in ["true", "false", "1", "ON", "off "] {
            assert!(cli_params(&args, Some(&format!("{id}\n{value}"))).is_err());
        }
    }

    #[test]
    fn replacement_cli_reuses_canonical_framing_and_bounds() {
        let args: Vec<_> = ["plugin", "profile-replace", "instance", "7", "operation"]
            .map(OsString::from)
            .into();
        let id = "00000000-0000-4000-8000-000000000001";
        let link = "trojan://synthetic-password@203.0.113.1:443";
        let input = format!("{id}\nPrivate label\n{link}\n");
        let params = cli_params(&args, Some(&input)).unwrap().unwrap();
        assert_eq!(params["profileId"], id);
        assert_eq!(params["name"], "Private label");
        assert_eq!(params["input"], format!("{link}\n"));
        assert_eq!(cli_input_limit(&args), Some(33126));
        assert!(cli_params(&args, None).is_err());
        for invalid in [
            String::new(),
            id.into(),
            format!("{id}\nName"),
            format!("{id}\n\n{link}"),
            format!("bad-id\nName\n{link}"),
            format!("{id}\n{}\n{link}", "x".repeat(321)),
            format!("{id}\nName\n{}", "x".repeat(32769)),
        ] {
            assert!(cli_params(&args, Some(&invalid)).is_err());
        }
        let mut extra = args;
        extra.push("private-token".into());
        assert_eq!(cli_input_limit(&extra), None);
        assert!(cli_params(&extra, Some(&input)).is_err());
        for key in ["path", "oldId", "enabled", "command"] {
            let mut changed = params.clone();
            changed[key] = json!("private-token");
            assert!(parse(&request(changed)).is_err());
        }
    }

    #[test]
    fn import_action_reuses_private_parser_without_replacement_or_subscription() {
        let args: Vec<_> = ["plugin", "profile-import", "instance", "7", "operation"]
            .map(OsString::from)
            .into();
        let link = "trojan://synthetic-password@203.0.113.1:443";
        let input = format!("Private name\n{link}\n");
        let params = cli_params(&args, Some(&input)).unwrap().unwrap();
        assert_eq!(params["name"], "Private name");
        assert_eq!(params["input"], format!("{link}\n"));
        assert_eq!(
            cli_input_limit(&args),
            Some(crate::semantic_cli::MAX_PROFILE_IMPORT_STDIN_BYTES)
        );
        for invalid in [
            String::new(),
            "Name".into(),
            format!("\n{link}"),
            format!("{}\n{link}", "x".repeat(321)),
            "Name\nhttps://private.example/subscription-token".into(),
            "Name\nprivate-token".into(),
            format!("Name\n{link}\n{link}"),
            format!(
                "Name\n{}",
                "x".repeat(crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES + 1)
            ),
        ] {
            let error = cli_params(&args, Some(&invalid)).unwrap_err();
            assert!(!error.to_string().contains("private-token"));
        }
        assert!(cli_params(&args, None).is_err());
        let mut extra = args.clone();
        extra.push("private-token".into());
        assert!(cli_params(&extra, Some(&input)).is_err());
        assert_eq!(cli_input_limit(&extra), None);
        for key in ["profileId", "oldId", "path", "enabled"] {
            let mut changed = params.clone();
            changed[key] = json!("private-token");
            assert!(parse(&request(changed)).is_err());
        }
    }

    #[test]
    fn profile_actions_reject_wrong_types_and_arbitrary_mutations() {
        let valid = json!({"action":"profile-favorite","instanceId":"instance","operationId":"op","expectedRevision":0,"profileId":"00000000-0000-4000-8000-000000000001","enabled":true});
        for (key, value) in [
            ("enabled", json!("on")),
            ("profileId", json!("private-token")),
            ("action", json!("profiles.favorite")),
            ("action", json!("profile-replace")),
            ("operationId", json!(null)),
            ("expectedRevision", json!("0")),
        ] {
            let mut params = valid.clone();
            params[key] = value;
            assert!(parse(&request(params)).is_err());
        }
    }
}
