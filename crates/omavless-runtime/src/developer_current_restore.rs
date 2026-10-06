// SPDX-License-Identifier: MIT
//! Private developer selector only; never advertised or compiled by default.
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Component, Path};
use zeroize::Zeroizing;

pub(crate) const METHOD: &str = "developer.restore_current";
const CONFIRM: &str = "replace-current-private-pair";
pub(crate) const BACKUP_METHOD: &str = "developer.backup_current";
const BACKUP_CONFIRM: &str = "export-current-private-pair";
pub(crate) const PAUSE_METHOD: &str = "developer.pause_current_intent";
const PAUSE_CONFIRM: &str = "pause-current-private-intent";
pub(crate) const ABORT_METHOD: &str = "developer.abort_current_intent";
const ABORT_CONFIRM: &str = "abort-current-private-intent";
pub(crate) fn private_method(method: &str) -> bool {
    matches!(method, METHOD | BACKUP_METHOD | PAUSE_METHOD | ABORT_METHOD)
}
pub(crate) const MAX_INPUT: usize = 32768;
#[derive(Debug, PartialEq, Eq)]
enum ReplyDisposition {
    RefusedBeforeEffect,
    Unknown,
}
fn classify_response(response: &Value) -> Result<(), ReplyDisposition> {
    classify_response_field(response, "completed")
}
fn classify_response_field(response: &Value, field: &str) -> Result<(), ReplyDisposition> {
    if response["ok"] == true && response["result"][field] == true {
        return Ok(());
    }
    if response["ok"] == false
        && matches!(
            response["error"]["code"].as_str(),
            Some(
                "invalid_request"
                    | "invalid_argument"
                    | "busy"
                    | "conflict"
                    | "daemon_restarting"
                    | "capability_unavailable"
                    | "unknown_method"
            )
        )
    {
        return Err(ReplyDisposition::RefusedBeforeEffect);
    }
    // Backend manual recovery/publication ambiguity, malformed reply and any
    // unrecognized outcome may follow effects. No retry or no-effect claim.
    Err(ReplyDisposition::Unknown)
}
struct PrivateText(Zeroizing<String>);
impl<'de> Deserialize<'de> for PrivateText {
    fn deserialize<D: Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
        String::deserialize(input).map(|text| Self(Zeroizing::new(text)))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
    confirmation: String,
    instance_id: String,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AbortRequest {
    schema: u8,
    confirmation: String,
    instance_id: String,
    expected_revision: u64,
}
impl AbortRequest {
    pub(crate) fn parse(value: &Value) -> Result<Self, ()> {
        let input = serde_json::to_vec(value).map_err(|_| ())?;
        if input.len() > MAX_INPUT {
            return Err(());
        }
        let request: Self = serde_json::from_slice(&input).map_err(|_| ())?;
        if request.schema != 1
            || request.confirmation != ABORT_CONFIRM
            || request.instance_id.is_empty()
            || request.instance_id.len() > 128
            || request.expected_revision > omavless_control_protocol::MAX_REVISION
        {
            return Err(());
        }
        Ok(request)
    }
    pub(crate) fn instance(&self) -> &str {
        &self.instance_id
    }
    pub(crate) fn revision(&self) -> u64 {
        self.expected_revision
    }
}
fn valid_private(schema: u8, archive: &str, passphrase: &str) -> bool {
    schema == 1
        && !archive.is_empty()
        && archive.len() <= 4096
        && Path::new(archive).is_absolute()
        && !archive.ends_with('/')
        && !archive.contains("//")
        && !archive.contains("/./")
        && !archive.ends_with("/.")
        && !archive.chars().any(char::is_control)
        && !Path::new(archive)
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        && (12..=1024).contains(&passphrase.len())
}
fn parse_input(raw: &[u8]) -> Result<Input, ()> {
    if raw.len() > MAX_INPUT {
        return Err(());
    }
    let request: Input = serde_json::from_slice(raw).map_err(|_| ())?;
    if !valid_private(request.schema, &request.archive.0, &request.passphrase.0) {
        return Err(());
    }
    Ok(request)
}
impl Request {
    pub(crate) fn parse(value: &Value) -> Result<Self, ()> {
        // Bounded by the already checked wire frame. The additional owned
        // secret is Zeroizing; JSON library temporaries are not all guaranteed.
        let input = Zeroizing::new(serde_json::to_vec(value).map_err(|_| ())?);
        if input.len() > MAX_INPUT {
            return Err(());
        }
        let request: Self = serde_json::from_slice(&input).map_err(|_| ())?;
        if !request.valid() {
            return Err(());
        }
        Ok(request)
    }
    fn valid(&self) -> bool {
        valid_private(self.schema, &self.archive.0, &self.passphrase.0)
            && matches!(
                self.confirmation.as_str(),
                CONFIRM | BACKUP_CONFIRM | PAUSE_CONFIRM
            )
            && !self.instance_id.is_empty()
            && self.instance_id.len() <= 128
            && self.expected_revision <= omavless_control_protocol::MAX_REVISION
    }
    pub(crate) fn instance(&self) -> &str {
        &self.instance_id
    }
    pub(crate) fn revision(&self) -> u64 {
        self.expected_revision
    }
    pub(crate) fn archive(&self) -> &Path {
        Path::new(self.archive.0.as_str())
    }
    pub(crate) fn passphrase(&self) -> &[u8] {
        self.passphrase.0.as_bytes()
    }
    pub(crate) fn matches_method(&self, method: &str) -> bool {
        matches!(
            (method, self.confirmation.as_str()),
            (METHOD, CONFIRM) | (BACKUP_METHOD, BACKUP_CONFIRM) | (PAUSE_METHOD, PAUSE_CONFIRM)
        )
    }
}
pub(crate) fn wipe_request(request: &mut Value) {
    if request["method"].as_str().is_some_and(private_method)
        && let Some(params) = request["params"].as_object_mut()
        && let Some(Value::String(secret)) = params.remove("passphrase")
    {
        drop(Zeroizing::new(secret));
    }
}
pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["developer", "restore-current", "--confirm-private-pair"]
        || arguments == ["developer", "backup-current", "--confirm-private-export"]
        || arguments
            == [
                "developer",
                "pause-current-intent",
                "--confirm-private-intent-pause",
            ]
        || arguments
            == [
                "developer",
                "abort-current-intent",
                "--confirm-private-intent-abort",
            ]
}
/// A lost reply is UNKNOWN, never an automatic retry. No request data is echoed.
pub fn from_private_input(
    arguments: &[OsString],
    input: impl Read,
) -> Result<&'static str, &'static str> {
    if !arguments_admitted(arguments) {
        return Err("developer_current_arguments_refused");
    }
    let abort = arguments[1] == "abort-current-intent";
    let (method, confirmation, marker) = if abort {
        (ABORT_METHOD, ABORT_CONFIRM, "t4_current_intent_aborted")
    } else if arguments[1] == "pause-current-intent" {
        (PAUSE_METHOD, PAUSE_CONFIRM, "t4_current_intent_paused")
    } else if arguments[1] == "backup-current" {
        (BACKUP_METHOD, BACKUP_CONFIRM, "t4_current_backup_created")
    } else {
        (METHOD, CONFIRM, "t4_current_pair_completed")
    };
    let uid = nix::unistd::Uid::current();
    if uid.is_root() || uid != nix::unistd::Uid::effective() {
        return Err("developer_current_restore_refused");
    }
    let mut raw = Zeroizing::new(Vec::new());
    input
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| "developer_current_restore_input_refused")?;
    if raw.len() > MAX_INPUT {
        return Err("developer_current_restore_input_refused");
    }
    let request = if abort {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ResumeInput {
            schema: u8,
        }
        let value: ResumeInput =
            serde_json::from_slice(&raw).map_err(|_| "developer_current_restore_input_refused")?;
        if value.schema != 1 {
            return Err("developer_current_restore_input_refused");
        }
        None
    } else {
        Some(parse_input(&raw).map_err(|_| "developer_current_restore_input_refused")?)
    };
    let paths = crate::RuntimePaths::current().map_err(|_| "developer_current_restore_refused")?;
    let hello = crate::call(&paths, "system.hello", serde_json::json!({"versions":[1]}))
        .map_err(|_| "developer_current_restore_refused")?;
    let instance = hello["result"]["instanceId"]
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or("developer_current_restore_refused")?;
    if hello["ok"] != true || (!abort && hello["result"]["runtimeOwnership"] != true) {
        return Err("developer_current_restore_refused");
    }
    let status = if abort {
        hello.clone()
    } else {
        crate::call(&paths, "status.get", serde_json::json!({}))
            .map_err(|_| "developer_current_restore_refused")?
    };
    if !abort
        && (status["ok"] != true
            || status["result"]["runtimeOwnership"] != true
            || status["result"]["desired"] != "disconnected"
            || status["result"]["actual"] != "disconnected")
    {
        return Err("developer_current_restore_refused");
    }
    let revision = status["revision"]
        .as_u64()
        .ok_or("developer_current_restore_refused")?;
    let params = if let Some(request) = request {
        serde_json::json!({"schema":1,"confirmation":confirmation,"archive":request.archive.0.as_str(),"passphrase":request.passphrase.0.as_str(),"instanceId":instance,"expectedRevision":revision})
    } else {
        serde_json::json!({"schema":1,"confirmation":confirmation,"instanceId":instance,"expectedRevision":revision})
    };
    let response =
        crate::call_with_timeout(&paths, method, params, std::time::Duration::from_secs(120))
            .map_err(|_| "developer_current_restore_outcome_unknown")?;
    let disposition = if method == PAUSE_METHOD {
        classify_response_field(&response, "intentPaused")
    } else {
        classify_response(&response)
    };
    disposition.map_err(|outcome| match outcome {
        ReplyDisposition::RefusedBeforeEffect => "developer_current_restore_refused",
        ReplyDisposition::Unknown => "developer_current_restore_outcome_unknown",
    })?;
    Ok(marker)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intent_resume_is_closed_context_only_and_pause_is_not_completion() {
        let good = serde_json::json!({"schema":1,"confirmation":ABORT_CONFIRM,
            "instanceId":"public-instance","expectedRevision":0});
        assert!(AbortRequest::parse(&good).is_ok());
        let pause_wire = wire()
            .replace(METHOD, PAUSE_METHOD)
            .replace(CONFIRM, PAUSE_CONFIRM);
        assert!(checked(pause_wire.as_bytes()).is_ok());
        assert!(checked(pause_wire.replace(PAUSE_CONFIRM, CONFIRM).as_bytes()).is_err());
        let mut resume_wire = crate::make_request("resume", ABORT_METHOD, good.clone()).unwrap();
        let encoded = crate::encode_request(&resume_wire).unwrap();
        let duplicate = String::from_utf8(encoded)
            .unwrap()
            .replace("\"schema\":1", "\"schema\":1,\"schema\":1");
        assert!(crate::decode_request(duplicate.as_bytes()).is_err());
        resume_wire["params"]["phase"] = "any".into();
        assert!(AbortRequest::parse(&resume_wire["params"]).is_err());
        for (key, value) in [
            ("archive", serde_json::json!("/public/a")),
            ("passphrase", serde_json::json!("synthetic password")),
            ("schema", serde_json::json!(true)),
            ("confirmation", serde_json::json!(PAUSE_CONFIRM)),
            ("instanceId", serde_json::json!("")),
        ] {
            let mut bad = good.clone();
            bad[key] = value;
            assert!(AbortRequest::parse(&bad).is_err());
        }
        let paused = serde_json::json!({"ok":true,"result":{"intentPaused":true}});
        assert_eq!(classify_response_field(&paused, "intentPaused"), Ok(()));
        assert_eq!(classify_response(&paused), Err(ReplyDisposition::Unknown));
        assert_eq!(
            classify_response_field(
                &serde_json::json!({"ok":true,"result":{"completed":true}}),
                "intentPaused"
            ),
            Err(ReplyDisposition::Unknown)
        );
    }
    fn checked(frame: &[u8]) -> Result<(), ()> {
        if frame.len() > MAX_INPUT {
            return Err(());
        }
        let value = crate::decode_request(frame).map_err(|_| ())?;
        let request = Request::parse(&value["params"])?;
        if !request.matches_method(value["method"].as_str().unwrap_or("")) {
            return Err(());
        }
        Ok(())
    }
    fn wire() -> String {
        let mut raw: String = r#"{"api":"omavless.control","version":1,"id":"public","method":"developer.restore_current","params":{"schema":1,"archive":"/public/input.ovb","passphrase":"synthetic password","confirmation":"replace-current-private-pair","instanceId":"public-instance","expectedRevision":0}}"#.into();
        raw.push('\n');
        raw
    }
    #[test]
    fn strict_private_wire_refuses_duplicates_unknown_types_utf8_and_bounds() {
        let good = wire();
        assert!(checked(good.as_bytes()).is_ok());
        for changed in [
            good.replace("\"schema\":1", "\"schema\":1,\"schema\":1"),
            good.replace("\"schema\":1", "\"schema\":true"),
            good.replace("\"schema\":1", "\"unknown\":1,\"schema\":1"),
            good.replace("\"version\":1", "\"version\":1,\"version\":1"),
            good.replace("replace-current-private-pair", "not-confirmed"),
            good.replace("/public/input.ovb", "relative"),
            good.replace("/public/input.ovb", "/public/../input.ovb"),
            good.replace("/public/input.ovb", "/public//input.ovb"),
            good.replace("/public/input.ovb", "/public/./input.ovb"),
        ] {
            assert!(checked(changed.as_bytes()).is_err());
        }
        assert!(checked(&[0xff]).is_err());
        assert!(checked(&vec![b' '; MAX_INPUT + 1]).is_err());
        assert!(arguments_admitted(&[
            "developer".into(),
            "restore-current".into(),
            "--confirm-private-pair".into()
        ]));
        assert!(!arguments_admitted(&[
            "developer".into(),
            "restore-current".into()
        ]));
    }
    #[test]
    fn private_secret_wipe_does_not_change_other_methods() {
        let mut request: Value = serde_json::from_str(&wire()).unwrap();
        wipe_request(&mut request);
        assert!(request["params"].get("passphrase").is_none());
        request["method"] = "other".into();
        request["params"]["passphrase"] = "public-synthetic".into();
        wipe_request(&mut request);
        assert_eq!(request["params"]["passphrase"], "public-synthetic");
    }
    #[test]
    fn private_stdin_exact_shape_refuses_before_any_client_io() {
        let good =
            r#"{"schema":1,"archive":"/public/input.ovb","passphrase":"synthetic password"}"#;
        assert!(parse_input(good.as_bytes()).is_ok());
        for bad in [
            good.replace("\"schema\":1", "\"schema\":1,\"schema\":1"),
            good.replace("\"schema\":1", "\"schema\":true"),
            good.replace("\"schema\":1", "\"schema\":1,\"path\":\"other\""),
            good.replace("/public/input.ovb", "/public/./input.ovb"),
            good.replace("synthetic password", "short"),
        ] {
            assert!(parse_input(bad.as_bytes()).is_err());
        }
        assert!(parse_input(&[0xff]).is_err());
        assert!(parse_input(&vec![b' '; MAX_INPUT + 1]).is_err());
    }
    #[test]
    fn private_backup_and_restore_confirmations_are_not_interchangeable() {
        let restore = wire();
        let backup = restore
            .replace(METHOD, BACKUP_METHOD)
            .replace(CONFIRM, BACKUP_CONFIRM);
        assert!(checked(backup.as_bytes()).is_ok());
        assert!(checked(restore.replace(CONFIRM, BACKUP_CONFIRM).as_bytes()).is_err());
        assert!(checked(restore.replace(METHOD, BACKUP_METHOD).as_bytes()).is_err());
        assert!(arguments_admitted(&[
            "developer".into(),
            "backup-current".into(),
            "--confirm-private-export".into()
        ]));
        assert!(!arguments_admitted(&[
            "developer".into(),
            "backup-current".into(),
            "--confirm-private-pair".into()
        ]));
    }
    #[test]
    fn private_effect_reply_never_labels_manual_recovery_as_no_effect() {
        assert_eq!(
            classify_response(&serde_json::json!({"ok":true,"result":{"completed":true}})),
            Ok(())
        );
        for code in [
            "invalid_request",
            "invalid_argument",
            "busy",
            "conflict",
            "daemon_restarting",
            "capability_unavailable",
            "unknown_method",
        ] {
            assert_eq!(
                classify_response(&serde_json::json!({"ok":false,"error":{"code":code}})),
                Err(ReplyDisposition::RefusedBeforeEffect)
            );
        }
        for reply in [
            serde_json::json!({"ok":false,"error":{"code":"manual_recovery_required"}}),
            serde_json::json!({"ok":false,"error":{"code":"internal_error"}}),
            serde_json::json!({"ok":true,"result":{"completed":false}}),
            serde_json::json!({}),
        ] {
            assert_eq!(classify_response(&reply), Err(ReplyDisposition::Unknown));
        }
    }
}
