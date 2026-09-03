// SPDX-License-Identifier: MIT

//! Fixed semantic CLI mutation mapping for the canonical runtime.
//!
//! This is deliberately not a raw method/JSON passthrough. Each accepted
//! command maps to one exact v1 method and parameter shape which the runtime
//! validates again. Credential-bearing profile material is never accepted by
//! this boundary; rename text is supplied through bounded stdin rather than
//! process argv.

use crate::desired::RoutingMode;
use crate::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES;
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

impl SemanticRequest {
    #[must_use]
    pub fn into_parts(self) -> (&'static str, Value) {
        (self.method, self.params)
    }
}

fn utf8(arguments: &[OsString]) -> Result<Vec<&str>, SemanticCliError> {
    arguments
        .iter()
        .map(|argument| argument.to_str().ok_or(SemanticCliError::InvalidArgument))
        .collect()
}

fn profile_id(value: &str) -> Result<&str, SemanticCliError> {
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

/// Map fixed user-facing argv plus optional bounded rename stdin to one exact
/// runtime request. Unknown commands, extra arguments and invalid UTF-8 fail
/// before any socket connection is attempted.
pub fn parse_semantic_mutation(
    arguments: &[OsString],
    rename_stdin: Option<&str>,
) -> Result<SemanticRequest, SemanticCliError> {
    let arguments = utf8(arguments)?;
    match arguments.as_slice() {
        ["connect", id] => Ok(SemanticRequest {
            method: "connection.connect",
            params: json!({"profileId": profile_id(id)?}),
        }),
        ["connect", id, requested_mode] => Ok(SemanticRequest {
            method: "connection.connect",
            params: json!({
                "profileId": profile_id(id)?,
                "mode": mode(requested_mode)?.as_str()
            }),
        }),
        ["disconnect"] => Ok(SemanticRequest {
            method: "connection.disconnect",
            params: json!({}),
        }),
        ["profile", "rename", id] => Ok(SemanticRequest {
            method: "profiles.rename",
            params: json!({
                "profileId": profile_id(id)?,
                "name": rename_name(rename_stdin)?
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
                params: json!({"profileId": profile_id(id)?, "enabled": enabled}),
            })
        }
        ["profile", "delete", id] => Ok(SemanticRequest {
            method: "profiles.delete",
            params: json!({"profileId": profile_id(id)?}),
        }),
        [] | ["connect"] | ["profile", ..] => Err(SemanticCliError::InvalidArgument),
        _ => Err(SemanticCliError::InvalidCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
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
            vec!["profile", "favorite", PROFILE, "yes"],
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
}
