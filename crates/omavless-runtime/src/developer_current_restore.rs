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
pub(crate) const MAX_INPUT: usize = 32768;
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
            && self.confirmation == CONFIRM
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
}
pub(crate) fn wipe_request(request: &mut Value) {
    if request["method"] == METHOD
        && let Some(params) = request["params"].as_object_mut()
        && let Some(Value::String(secret)) = params.remove("passphrase")
    {
        drop(Zeroizing::new(secret));
    }
}
pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["developer", "restore-current", "--confirm-private-pair"]
}
/// A lost reply is UNKNOWN, never an automatic retry. No request data is echoed.
pub fn from_private_input(input: impl Read) -> Result<(), &'static str> {
    let uid = nix::unistd::Uid::current();
    if uid.is_root() || uid != nix::unistd::Uid::effective() {
        return Err("developer_current_restore_refused");
    }
    let mut raw = Zeroizing::new(Vec::new());
    input
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| "developer_current_restore_input_refused")?;
    let request = parse_input(&raw).map_err(|_| "developer_current_restore_input_refused")?;
    let paths = crate::RuntimePaths::current().map_err(|_| "developer_current_restore_refused")?;
    let hello = crate::call(&paths, "system.hello", serde_json::json!({"versions":[1]}))
        .map_err(|_| "developer_current_restore_refused")?;
    let instance = hello["result"]["instanceId"]
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or("developer_current_restore_refused")?;
    if hello["ok"] != true || hello["result"]["runtimeOwnership"] != true {
        return Err("developer_current_restore_refused");
    }
    let status = crate::call(&paths, "status.get", serde_json::json!({}))
        .map_err(|_| "developer_current_restore_refused")?;
    if status["ok"] != true
        || status["result"]["runtimeOwnership"] != true
        || status["result"]["desired"] != "disconnected"
        || status["result"]["actual"] != "disconnected"
    {
        return Err("developer_current_restore_refused");
    }
    let revision = status["revision"]
        .as_u64()
        .ok_or("developer_current_restore_refused")?;
    let params = serde_json::json!({"schema":1,"confirmation":CONFIRM,"archive":request.archive.0.as_str(),"passphrase":request.passphrase.0.as_str(),"instanceId":instance,"expectedRevision":revision});
    let response =
        crate::call_with_timeout(&paths, METHOD, params, std::time::Duration::from_secs(120))
            .map_err(|_| "developer_current_restore_outcome_unknown")?;
    if response["ok"] != true || response["result"]["completed"] != true {
        return Err("developer_current_restore_refused");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checked(frame: &[u8]) -> Result<(), ()> {
        if frame.len() > MAX_INPUT {
            return Err(());
        }
        let value = crate::decode_request(frame).map_err(|_| ())?;
        Request::parse(&value["params"]).map(|_| ())
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
}
