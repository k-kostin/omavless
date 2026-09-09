// SPDX-License-Identifier: MIT

//! Fixed semantic CLI request mapping for the canonical runtime.
//!
//! This is deliberately not a raw method/JSON passthrough. Each accepted
//! command maps to one exact v1 method and parameter shape which the runtime
//! validates again. Private import, rename and subscription input is supplied
//! through bounded stdin rather than process argv. Explicit previews/editor
//! reads return private local UI data, never shareable diagnostics.

use crate::desired::RoutingMode;
use crate::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES;
use crate::subscription_mutation_protocol::MAX_SUBSCRIPTION_NAME_INPUT_BYTES;
use omavless_domain::import::{MAX_SUBSCRIPTION_URL_BYTES, valid_subscription_url};
use omavless_domain::store::valid_record_id;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticCliError {
    InvalidCommand,
    InvalidArgument,
    MissingInput,
    InputTooLarge,
}

pub const MAX_SUBSCRIPTION_STDIN_BYTES: usize =
    MAX_SUBSCRIPTION_NAME_INPUT_BYTES + 1 + MAX_SUBSCRIPTION_URL_BYTES + 1;
pub const MAX_PROFILE_IMPORT_STDIN_BYTES: usize =
    MAX_PROFILE_NAME_INPUT_BYTES + 1 + crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES;

impl fmt::Display for SemanticCliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCommand => "OmaVLESS semantic command is invalid",
            Self::InvalidArgument => "OmaVLESS semantic command argument is invalid",
            Self::MissingInput => "OmaVLESS semantic command input is missing",
            Self::InputTooLarge => "OmaVLESS semantic command input is too large",
        })
    }
}

impl std::error::Error for SemanticCliError {}

/// One fixed runtime request. This type intentionally has no `Debug` or
/// serialization implementation because its params may contain a private
/// profile label supplied over stdin.
pub struct SemanticRequest {
    method: &'static str,
    params: Value,
}

/// Map fixed reads, including explicitly sensitive editor/export requests.
/// `None` means the argv belongs to another semantic command family; malformed
/// UTF-8 still fails before any socket connection.
pub fn parse_semantic_read(
    arguments: &[OsString],
) -> Result<Option<SemanticRequest>, SemanticCliError> {
    let arguments = utf8(arguments)?;
    Ok(match arguments.as_slice() {
        ["plugin", "snapshot"] => Some(SemanticRequest {
            method: "ui.snapshot",
            params: json!({}),
        }),
        ["diagnostics", "export"] => Some(SemanticRequest {
            method: "diagnostics.export",
            params: json!({}),
        }),
        ["diagnostics", kind @ ("summary" | "rules" | "providers")] => Some(SemanticRequest {
            method: match *kind {
                "summary" => "diagnostics.summary",
                "rules" => "diagnostics.rules",
                _ => "diagnostics.providers",
            },
            params: json!({}),
        }),
        ["routing", "rules"] => Some(SemanticRequest {
            method: "routing.custom_rules.list",
            params: json!({}),
        }),
        ["operation", "get", instance, operation] => {
            Some(long_request("operations.get", instance, operation, None)?)
        }
        ["profile", "list"] => Some(SemanticRequest {
            method: "profiles.list",
            params: json!({}),
        }),
        ["profile", "edit-input", id] => Some(SemanticRequest {
            method: "profiles.edit_input",
            params: json!({"profileId":record_id(id)?}),
        }),
        ["profile", "export", id, purpose @ ("qr" | "file")] => Some(SemanticRequest {
            method: "profiles.export",
            params: json!({"profileId":record_id(id)?, "purpose":purpose}),
        }),
        ["subscription", "list"] => Some(SemanticRequest {
            method: "subscriptions.list",
            params: json!({}),
        }),
        ["subscription", "edit-input", id] => Some(SemanticRequest {
            method: "subscriptions.edit_input",
            params: json!({"subscriptionId": record_id(id)?}),
        }),
        _ => None,
    })
}

impl SemanticRequest {
    #[must_use]
    pub fn into_parts(self) -> (&'static str, Value) {
        (self.method, self.params)
    }
}

pub fn parse_semantic_route_check(
    arguments: &[OsString],
    input: &str,
) -> Result<SemanticRequest, SemanticCliError> {
    if utf8(arguments)?.as_slice() != ["routing", "check"] {
        return Err(SemanticCliError::InvalidCommand);
    }
    omavless_domain::route_check::canonical_query(input)
        .map_err(|_| SemanticCliError::InvalidArgument)?;
    Ok(SemanticRequest {
        method: "routing.check",
        params: json!({"query":input}),
    })
}

/// Explicit private input, never a raw method or caller-provided JSON envelope.
pub fn parse_semantic_import_preview(
    arguments: &[OsString],
    stdin: Option<&str>,
) -> Result<SemanticRequest, SemanticCliError> {
    if utf8(arguments)?.as_slice() != ["import", "preview"] {
        return Err(SemanticCliError::InvalidCommand);
    }
    let input = stdin.ok_or(SemanticCliError::MissingInput)?;
    if input.trim().is_empty() {
        return Err(SemanticCliError::MissingInput);
    }
    if input.len() > crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES {
        return Err(SemanticCliError::InputTooLarge);
    }
    Ok(SemanticRequest {
        method: "imports.classify",
        params: json!({"input": input}),
    })
}

/// Confirmed name on the first stdin line, profile link in the remaining text.
pub fn parse_semantic_profile_import(
    arguments: &[OsString],
    stdin: Option<&str>,
) -> Result<SemanticRequest, SemanticCliError> {
    if utf8(arguments)?.as_slice() != ["profile", "import"] {
        return Err(SemanticCliError::InvalidCommand);
    }
    let input = stdin.ok_or(SemanticCliError::MissingInput)?;
    if input.len() > MAX_PROFILE_IMPORT_STDIN_BYTES {
        return Err(SemanticCliError::InputTooLarge);
    }
    let (name, profile_input) = input
        .split_once('\n')
        .ok_or(SemanticCliError::InvalidArgument)?;
    if name.trim().is_empty()
        || name.len() > MAX_PROFILE_NAME_INPUT_BYTES
        || profile_input.trim().is_empty()
        || profile_input.len() > crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES
    {
        return Err(SemanticCliError::InvalidArgument);
    }
    Ok(SemanticRequest {
        method: "profiles.import",
        params: json!({"name": name, "input": profile_input}),
    })
}

pub fn parse_semantic_profile_replace(
    arguments: &[OsString],
    stdin: Option<&str>,
) -> Result<SemanticRequest, SemanticCliError> {
    let values = utf8(arguments)?;
    let ["profile", "replace", id] = values.as_slice() else {
        return Err(SemanticCliError::InvalidCommand);
    };
    let id = record_id(id)?;
    let mut request = parse_semantic_profile_import(&["profile".into(), "import".into()], stdin)?;
    request.method = "profiles.replace";
    request.params["profileId"] = json!(id);
    Ok(request)
}

// Correlation IDs are non-credential identifiers supplied by hello/the caller.
// Reuse the exact protocol parser; never accept a generic method or JSON argv.
fn long_request(
    method: &'static str,
    instance: &str,
    operation: &str,
    revision: Option<&str>,
) -> Result<SemanticRequest, SemanticCliError> {
    use crate::long_operation_protocol::{
        parse_operation_cancel, parse_operation_get, parse_provider_refresh_start,
        parse_refresh_all_start,
    };
    let mut params = json!({"instanceId": instance, "operationId": operation});
    if let Some(revision) = revision {
        params["expectedRevision"] = json!(
            revision
                .parse::<u64>()
                .map_err(|_| SemanticCliError::InvalidArgument)?
        );
    }
    let request = omavless_control_protocol::make_request("semantic-cli", method, params.clone())
        .map_err(|_| SemanticCliError::InvalidArgument)?;
    match method {
        "subscriptions.refresh_all" => parse_refresh_all_start(&request).map(|_| ()),
        "routing.refresh_providers" => parse_provider_refresh_start(&request).map(|_| ()),
        "operations.get" => parse_operation_get(&request).map(|_| ()),
        "operations.cancel" => parse_operation_cancel(&request).map(|_| ()),
        _ => return Err(SemanticCliError::InvalidCommand),
    }
    .map_err(|_| SemanticCliError::InvalidArgument)?;
    Ok(SemanticRequest { method, params })
}

fn utf8(arguments: &[OsString]) -> Result<Vec<&str>, SemanticCliError> {
    arguments
        .iter()
        .map(|argument| argument.to_str().ok_or(SemanticCliError::InvalidArgument))
        .collect()
}

fn record_id(value: &str) -> Result<&str, SemanticCliError> {
    valid_record_id(value)
        .then_some(value)
        .ok_or(SemanticCliError::InvalidArgument)
}

fn mode(value: &str) -> Result<RoutingMode, SemanticCliError> {
    match value {
        "rule" => Ok(RoutingMode::Rule),
        "global" => Ok(RoutingMode::Global),
        "direct" => Ok(RoutingMode::Direct),
        _ => Err(SemanticCliError::InvalidArgument),
    }
}

fn rename_name(stdin: Option<&str>) -> Result<&str, SemanticCliError> {
    let name = stdin.ok_or(SemanticCliError::MissingInput)?;
    if name.is_empty() {
        return Err(SemanticCliError::MissingInput);
    }
    if name.len() > MAX_PROFILE_NAME_INPUT_BYTES {
        return Err(SemanticCliError::InputTooLarge);
    }
    Ok(name)
}

fn subscription_input(stdin: Option<&str>) -> Result<(&str, &str), SemanticCliError> {
    let input = stdin.ok_or(SemanticCliError::MissingInput)?;
    if input.is_empty() || input.len() > MAX_SUBSCRIPTION_STDIN_BYTES || input.contains('\r') {
        return Err(SemanticCliError::InvalidArgument);
    }
    let input = input.strip_suffix('\n').unwrap_or(input);
    let Some((name, url)) = input.split_once('\n') else {
        return Err(SemanticCliError::InvalidArgument);
    };
    if name.is_empty()
        || name.len() > MAX_SUBSCRIPTION_NAME_INPUT_BYTES
        || url.is_empty()
        || url.len() > MAX_SUBSCRIPTION_URL_BYTES
        || url.contains('\n')
        || !valid_subscription_url(url)
    {
        return Err(SemanticCliError::InvalidArgument);
    }
    Ok((name, url))
}

/// Map fixed user-facing argv plus optional bounded private stdin to one exact
/// runtime request. Unknown commands, extra arguments and invalid UTF-8 fail
/// before any socket connection is attempted.
pub fn parse_semantic_mutation(
    arguments: &[OsString],
    private_stdin: Option<&str>,
) -> Result<SemanticRequest, SemanticCliError> {
    let arguments = utf8(arguments)?;
    match arguments.as_slice() {
        ["onboarding", "complete"] => Ok(SemanticRequest {
            method: "onboarding.complete",
            params: json!({}),
        }),
        ["routing", "preset", preset] | ["routing", "preset", preset, "keep-mode"] => {
            if !matches!(
                *preset,
                "roscomvpn-default" | "china-cn-direct" | "iran-ir-direct"
            ) {
                return Err(SemanticCliError::InvalidArgument);
            }
            Ok(SemanticRequest {
                method: "routing.set_preset",
                params: json!({"preset":preset,"keepMode":arguments.len()==4}),
            })
        }
        ["routing", "rule-add", kind, action] => {
            let value = private_stdin.ok_or(SemanticCliError::MissingInput)?;
            if value.len() > omavless_domain::routing::MAX_CUSTOM_RULE_VALUE_BYTES {
                return Err(SemanticCliError::InputTooLarge);
            }
            omavless_domain::routing::CustomRule::parse(kind, action, value)
                .map_err(|_| SemanticCliError::InvalidArgument)?;
            Ok(SemanticRequest {
                method: "routing.custom_rules.add",
                params: json!({"kind":kind,"action":action,"value":value}),
            })
        }
        ["routing", "rule-delete", id] => Ok(SemanticRequest {
            method: "routing.custom_rules.delete",
            params: json!({"ruleId":record_id(id)?}),
        }),
        ["subscription", "refresh-all", instance, operation] => {
            long_request("subscriptions.refresh_all", instance, operation, None)
        }
        ["routing", "refresh-providers", instance, operation] => {
            long_request("routing.refresh_providers", instance, operation, None)
        }
        [
            "routing",
            "refresh-providers",
            instance,
            operation,
            revision,
        ] => long_request(
            "routing.refresh_providers",
            instance,
            operation,
            Some(revision),
        ),
        ["subscription", "refresh-all", instance, operation, revision] => long_request(
            "subscriptions.refresh_all",
            instance,
            operation,
            Some(revision),
        ),
        ["operation", "cancel", instance, operation] => {
            long_request("operations.cancel", instance, operation, None)
        }
        ["connect", id] => Ok(SemanticRequest {
            method: "connection.connect",
            params: json!({"profileId": record_id(id)?}),
        }),
        ["connect", id, requested_mode] => Ok(SemanticRequest {
            method: "connection.connect",
            params: json!({
                "profileId": record_id(id)?,
                "mode": mode(requested_mode)?.as_str()
            }),
        }),
        ["disconnect"] => Ok(SemanticRequest {
            method: "connection.disconnect",
            params: json!({}),
        }),
        ["mode", requested_mode] => Ok(SemanticRequest {
            method: "routing.set_mode",
            params: json!({"mode": mode(requested_mode)?.as_str()}),
        }),
        ["profile", "rename", id] => Ok(SemanticRequest {
            method: "profiles.rename",
            params: json!({
                "profileId": record_id(id)?,
                "name": rename_name(private_stdin)?
            }),
        }),
        ["profile", "favorite", id, enabled] => {
            let enabled = match *enabled {
                "on" => true,
                "off" => false,
                _ => return Err(SemanticCliError::InvalidArgument),
            };
            Ok(SemanticRequest {
                method: "profiles.favorite",
                params: json!({"profileId": record_id(id)?, "enabled": enabled}),
            })
        }
        ["profile", "delete", id] => Ok(SemanticRequest {
            method: "profiles.delete",
            params: json!({"profileId": record_id(id)?}),
        }),
        ["subscription", "add"] => {
            let (name, url) = subscription_input(private_stdin)?;
            Ok(SemanticRequest {
                method: "subscriptions.add",
                params: json!({"name": name, "url": url}),
            })
        }
        ["subscription", "update", id] => {
            let (name, url) = subscription_input(private_stdin)?;
            Ok(SemanticRequest {
                method: "subscriptions.update",
                params: json!({
                    "subscriptionId": record_id(id)?,
                    "name": name,
                    "url": url
                }),
            })
        }
        ["subscription", "delete", id] => Ok(SemanticRequest {
            method: "subscriptions.delete",
            params: json!({"subscriptionId": record_id(id)?}),
        }),
        ["subscription", "refresh", id] => Ok(SemanticRequest {
            method: "subscriptions.refresh",
            params: json!({"subscriptionId": record_id(id)?}),
        }),
        [] | ["connect"] | ["mode"] | ["profile", ..] | ["subscription", ..] => {
            Err(SemanticCliError::InvalidArgument)
        }
        _ => Err(SemanticCliError::InvalidCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn diagnostics_cli_has_only_fixed_empty_parameter_reads() {
        for kind in ["summary", "rules", "providers", "export"] {
            let request = parse_semantic_read(&args(&["diagnostics", kind]))
                .unwrap()
                .unwrap();
            let (method, params) = request.into_parts();
            assert_eq!(method, format!("diagnostics.{kind}"));
            assert_eq!(params, json!({}));
        }
        for arguments in [
            &["diagnostics"][..],
            &["diagnostics", "rules", "private-token"],
            &["diagnostics", "controller-forward"],
        ] {
            assert!(parse_semantic_read(&args(arguments)).unwrap().is_none());
        }
    }

    #[test]
    fn route_check_cli_keeps_private_query_out_of_arguments() {
        let (method, params) =
            parse_semantic_route_check(&args(&["routing", "check"]), " Private.Example.\n")
                .unwrap()
                .into_parts();
        assert_eq!(method, "routing.check");
        assert_eq!(params, json!({"query":" Private.Example.\n"}));
        for arguments in [
            &["routing", "check", "private.example"][..],
            &["route-check"],
        ] {
            let error = match parse_semantic_route_check(&args(arguments), "private.example") {
                Ok(_) => panic!("bad argv accepted"),
                Err(error) => error,
            };
            assert!(!error.to_string().contains("private.example"));
        }
        for query in [
            "https://private.example/token".to_owned(),
            "x".repeat(1025),
            "".to_owned(),
        ] {
            assert!(parse_semantic_route_check(&args(&["routing", "check"]), &query).is_err());
        }
    }

    #[test]
    fn plugin_snapshot_cli_is_an_exact_read_without_stdin_or_params() {
        let request = parse_semantic_read(&args(&["plugin", "snapshot"]))
            .unwrap()
            .unwrap();
        let (method, params) = request.into_parts();
        assert_eq!(method, "ui.snapshot");
        assert_eq!(params, json!({}));
        for arguments in [
            &["plugin"][..],
            &["plugin", "snapshot", "private-token"],
            &["plugin", "raw"],
        ] {
            assert!(parse_semantic_read(&args(arguments)).unwrap().is_none());
        }
    }

    #[test]
    fn import_preview_mapping_is_fixed_bounded_and_preserves_private_input() {
        use crate::import_read_protocol::MAX_IMPORT_STDIN_BYTES;
        for input in [
            "https://example.invalid/token\n".to_owned(),
            "x".repeat(MAX_IMPORT_STDIN_BYTES),
        ] {
            let request =
                parse_semantic_import_preview(&args(&["import", "preview"]), Some(&input))
                    .ok()
                    .unwrap();
            let (method, params) = request.into_parts();
            assert_eq!(method, "imports.classify");
            assert!(params == json!({"input": input}));
        }
        for input in [None, Some(""), Some(" \n")] {
            assert!(parse_semantic_import_preview(&args(&["import", "preview"]), input).is_err());
        }
        assert!(
            parse_semantic_import_preview(
                &args(&["import", "preview"]),
                Some(&"x".repeat(MAX_IMPORT_STDIN_BYTES + 1))
            )
            .is_err()
        );
        assert!(
            parse_semantic_import_preview(&args(&["import", "preview", "secret"]), Some("secret"))
                .is_err()
        );
    }

    #[test]
    fn confirmed_profile_import_uses_only_bounded_stdin_name_and_link() {
        let input = "Confirmed\ntrojan://synthetic-password@203.0.113.1:443\n";
        let (method, params) =
            parse_semantic_profile_import(&args(&["profile", "import"]), Some(input))
                .ok()
                .unwrap()
                .into_parts();
        assert_eq!(method, "profiles.import");
        assert_eq!(params.as_object().unwrap().len(), 2);
        assert!(params["input"] == "trojan://synthetic-password@203.0.113.1:443\n");
        for input in [
            None,
            Some("name-only"),
            Some(" \nprivate-token"),
            Some("Name\n"),
        ] {
            assert!(parse_semantic_profile_import(&args(&["profile", "import"]), input).is_err());
        }
        assert!(
            parse_semantic_profile_import(
                &args(&["profile", "import", "private-token"]),
                Some(input)
            )
            .is_err()
        );
        assert!(
            parse_semantic_profile_import(
                &args(&["profile", "import"]),
                Some(&"x".repeat(MAX_PROFILE_IMPORT_STDIN_BYTES + 1))
            )
            .is_err()
        );
    }

    #[test]
    fn replacement_uses_same_private_input_boundary_and_explicit_opaque_target() {
        let input = "Confirmed\ntrojan://synthetic-password@203.0.113.1:443\n";
        let (method, params) =
            parse_semantic_profile_replace(&args(&["profile", "replace", PROFILE]), Some(input))
                .ok()
                .unwrap()
                .into_parts();
        assert_eq!(method, "profiles.replace");
        assert_eq!(params["profileId"], PROFILE);
        assert_eq!(params.as_object().unwrap().len(), 3);
        assert!(
            parse_semantic_profile_replace(
                &args(&["profile", "replace", "private-token"]),
                Some(input)
            )
            .is_err()
        );
        assert!(
            parse_semantic_profile_replace(
                &args(&["profile", "replace", PROFILE, "extra"]),
                Some(input)
            )
            .is_err()
        );
        assert!(
            parse_semantic_profile_replace(
                &args(&["profile", "replace", PROFILE]),
                Some("no newline")
            )
            .is_err()
        );
    }

    fn parsed(values: &[&str], stdin: Option<&str>) -> (&'static str, Value) {
        parse_semantic_mutation(&args(values), stdin)
            .unwrap()
            .into_parts()
    }

    fn rejected(values: &[&str], stdin: Option<&str>) -> SemanticCliError {
        match parse_semantic_mutation(&args(values), stdin) {
            Ok(_) => panic!("invalid semantic command was accepted"),
            Err(error) => error,
        }
    }

    #[test]
    fn connection_commands_map_to_exact_runtime_shapes() {
        assert_eq!(
            parsed(&["connect", PROFILE], None),
            ("connection.connect", json!({"profileId": PROFILE}))
        );
        assert_eq!(
            parsed(&["connect", PROFILE, "global"], None),
            (
                "connection.connect",
                json!({"profileId": PROFILE, "mode": "global"})
            )
        );
        assert_eq!(
            parsed(&["disconnect"], None),
            ("connection.disconnect", json!({}))
        );
        assert_eq!(
            parsed(&["mode", "direct"], None),
            ("routing.set_mode", json!({"mode": "direct"}))
        );
    }

    #[test]
    fn profile_commands_map_to_exact_runtime_shapes() {
        assert_eq!(
            parsed(&["profile", "rename", PROFILE], Some("Renamed")),
            (
                "profiles.rename",
                json!({"profileId": PROFILE, "name": "Renamed"})
            )
        );
        assert_eq!(
            parsed(&["profile", "favorite", PROFILE, "on"], None),
            (
                "profiles.favorite",
                json!({"profileId": PROFILE, "enabled": true})
            )
        );
        assert_eq!(
            parsed(&["profile", "delete", PROFILE], None),
            ("profiles.delete", json!({"profileId": PROFILE}))
        );
        assert_eq!(
            parsed(&["subscription", "delete", SUBSCRIPTION], None),
            (
                "subscriptions.delete",
                json!({"subscriptionId": SUBSCRIPTION})
            )
        );
        assert_eq!(
            parsed(&["subscription", "refresh", SUBSCRIPTION], None),
            (
                "subscriptions.refresh",
                json!({"subscriptionId": SUBSCRIPTION})
            )
        );
    }

    #[test]
    fn subscription_remote_commands_use_one_exact_private_stdin_shape() {
        let input = "Private source\nhttps://provider.invalid/subscription-token\n";
        assert_eq!(
            parsed(&["subscription", "add"], Some(input)),
            (
                "subscriptions.add",
                json!({
                    "name": "Private source",
                    "url": "https://provider.invalid/subscription-token"
                })
            )
        );
        assert_eq!(
            parsed(&["subscription", "update", SUBSCRIPTION], Some(input)),
            (
                "subscriptions.update",
                json!({
                    "subscriptionId": SUBSCRIPTION,
                    "name": "Private source",
                    "url": "https://provider.invalid/subscription-token"
                })
            )
        );
        for private_input in [
            None,
            Some("only-one-line"),
            Some("Source\nhttp://remote.invalid/private"),
            Some("Source\r\nhttps://provider.invalid/private"),
            Some("Source\nhttps://provider.invalid/private\nextra"),
        ] {
            let error = rejected(&["subscription", "add"], private_input);
            let rendered = format!("{error:?} {error}");
            for private in ["remote.invalid", "provider.invalid", "private"] {
                assert!(!rendered.contains(private));
            }
        }
    }

    #[test]
    fn read_commands_map_to_exact_runtime_shapes() {
        assert_eq!(
            parse_semantic_read(&args(&["profile", "edit-input", PROFILE]))
                .unwrap()
                .unwrap()
                .into_parts(),
            ("profiles.edit_input", json!({"profileId":PROFILE}))
        );
        assert!(
            parse_semantic_read(&args(&["profile", "edit-input", PROFILE, "extra"]))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            parse_semantic_read(&args(&["profile", "list"]))
                .unwrap()
                .unwrap()
                .into_parts(),
            ("profiles.list", json!({}))
        );
        assert_eq!(
            parse_semantic_read(&args(&["subscription", "list"]))
                .unwrap()
                .unwrap()
                .into_parts(),
            ("subscriptions.list", json!({}))
        );
        assert_eq!(
            parse_semantic_read(&args(&["subscription", "edit-input", SUBSCRIPTION]))
                .unwrap()
                .unwrap()
                .into_parts(),
            (
                "subscriptions.edit_input",
                json!({"subscriptionId": SUBSCRIPTION})
            )
        );
        assert!(
            parse_semantic_read(&args(&["profile", "list", "extra"]))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn unknown_extra_and_raw_method_commands_fail_before_dispatch() {
        for values in [
            vec!["request", "connection.connect"],
            vec!["connect", PROFILE, "rule", "extra"],
            vec!["profile", "delete", PROFILE, "extra"],
            vec!["systemctl", "--user", "stop"],
        ] {
            assert!(parse_semantic_mutation(&args(&values), None).is_err());
        }
    }

    #[test]
    fn invalid_ids_modes_flags_and_utf8_are_rejected_safely() {
        for values in [
            vec!["connect", "private.example/password"],
            vec!["connect", PROFILE, "unsafe-mode"],
            vec!["mode", "unsafe-mode"],
            vec!["profile", "favorite", PROFILE, "yes"],
            vec!["subscription", "delete", "private.example/password"],
            vec!["subscription", "refresh", "private.example/password"],
        ] {
            let error = rejected(&values, None);
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains("private.example"));
            assert!(!rendered.contains("password"));
            assert!(!rendered.contains("unsafe-mode"));
        }
        use std::os::unix::ffi::OsStringExt;
        let invalid = vec![OsString::from_vec(vec![0xff])];
        let error = match parse_semantic_mutation(&invalid, None) {
            Ok(_) => panic!("invalid UTF-8 argument was accepted"),
            Err(error) => error,
        };
        assert_eq!(error, SemanticCliError::InvalidArgument);
    }

    #[test]
    fn rename_input_is_required_bounded_and_never_formatted() {
        assert_eq!(
            rejected(&["profile", "rename", PROFILE], None),
            SemanticCliError::MissingInput
        );
        let private = "private.example/password".repeat(20);
        let error = rejected(&["profile", "rename", PROFILE], Some(&private));
        assert_eq!(error, SemanticCliError::InputTooLarge);
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("private.example"));
        assert!(!rendered.contains("password"));
    }
    #[test]
    fn profile_export_requires_fixed_purpose_and_never_accepts_a_destination() {
        for purpose in ["file", "qr"] {
            let args = [
                "profile",
                "export",
                "00000000-0000-4000-8000-000000000001",
                purpose,
            ]
            .map(OsString::from);
            let (method, params) = parse_semantic_read(&args).unwrap().unwrap().into_parts();
            assert_eq!(method, "profiles.export");
            assert_eq!(params["purpose"], purpose);
            assert_eq!(params.as_object().unwrap().len(), 2);
        }
        for args in [
            vec!["profile", "export"],
            vec![
                "profile",
                "export",
                "00000000-0000-4000-8000-000000000001",
                "edit",
            ],
            vec![
                "profile",
                "export",
                "00000000-0000-4000-8000-000000000001",
                "file",
                "/tmp/private-token",
            ],
        ] {
            let args: Vec<_> = args.into_iter().map(OsString::from).collect();
            assert!(parse_semantic_read(&args).unwrap().is_none());
        }
    }
    #[test]
    fn onboarding_command_has_no_reset_flags_or_private_input() {
        let (method, params) = parse_semantic_mutation(&args(&["onboarding", "complete"]), None)
            .unwrap()
            .into_parts();
        assert_eq!(method, "onboarding.complete");
        assert_eq!(params, json!({}));
        for arguments in [
            &["onboarding"][..],
            &["onboarding", "reset"],
            &["onboarding", "complete", "private-token"],
        ] {
            let error = parse_semantic_mutation(&args(arguments), None)
                .err()
                .expect("bad argv accepted");
            assert!(!error.to_string().contains("private-token"));
        }
    }
}

#[cfg(test)]
mod long_operation_tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn maps_only_fixed_long_commands_and_validated_metadata() {
        let start = parse_semantic_mutation(
            &args(&["subscription", "refresh-all", "instance-1", "op-1", "7"]),
            None,
        )
        .unwrap()
        .into_parts();
        assert_eq!(start.0, "subscriptions.refresh_all");
        assert_eq!(start.1["expectedRevision"], 7);
        let get = parse_semantic_read(&args(&["operation", "get", "instance-1", "op-1"]))
            .unwrap()
            .unwrap()
            .into_parts();
        assert_eq!(get.0, "operations.get");
        let cancel =
            parse_semantic_mutation(&args(&["operation", "cancel", "instance-1", "op-1"]), None)
                .unwrap()
                .into_parts();
        assert_eq!(cancel.0, "operations.cancel");
        for bad in ["", "has space", "private\nvalue"] {
            assert!(
                parse_semantic_mutation(&args(&["subscription", "refresh-all", bad, "op-1"]), None)
                    .is_err()
            );
            assert!(parse_semantic_read(&args(&["operation", "get", "instance-1", bad])).is_err());
        }
        assert!(
            parse_semantic_mutation(
                &args(&["subscription", "refresh-all", "instance-1", "op-1", "-1"]),
                None
            )
            .is_err()
        );
        assert!(parse_semantic_mutation(&args(&["operations.cancel", "{}"]), None).is_err());
    }

    #[test]
    fn provider_refresh_cli_accepts_only_fixed_metadata() {
        for (argv, expected) in [
            (
                vec!["routing", "refresh-providers", "instance-1", "op-1"],
                json!({"instanceId":"instance-1","operationId":"op-1"}),
            ),
            (
                vec!["routing", "refresh-providers", "instance-1", "op-1", "7"],
                json!({"instanceId":"instance-1","operationId":"op-1","expectedRevision":7}),
            ),
        ] {
            let (method, params) = parse_semantic_mutation(&args(&argv), None)
                .unwrap()
                .into_parts();
            assert_eq!(method, "routing.refresh_providers");
            assert_eq!(params, expected);
        }
        for argv in [
            vec!["routing", "refresh-providers"],
            vec![
                "routing",
                "refresh-providers",
                "instance-1",
                "private token",
            ],
            vec!["routing", "refresh-providers", "instance-1", "op-1", "-1"],
            vec![
                "routing",
                "refresh-providers",
                "instance-1",
                "op-1",
                "7",
                "private-token",
            ],
        ] {
            let error = parse_semantic_mutation(&args(&argv), None)
                .err()
                .expect("invalid argv accepted");
            assert!(!error.to_string().contains("private"));
        }
    }
}
