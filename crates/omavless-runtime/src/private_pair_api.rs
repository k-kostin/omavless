// SPDX-License-Identifier: MIT
//! Opt-in normal private-pair API. Not whole-settings backup, nor authority in data.
use crate::developer_current_restore::{MAX_INPUT, PrivateText, valid_private};
use crate::mutation::{MutationDigest, MutationKind, MutationRequest};
use serde::Deserialize;
use serde_json::Value;
use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use zeroize::Zeroizing;

pub(crate) const METHODS: &[&str] = &["backup.create", "backup.restore"];
pub(crate) fn is_method(method: &str) -> bool {
    METHODS.contains(&method)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Create,
    Restore,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
    confirmation: String,
    instance_id: String,
    operation_id: String,
    expected_revision: u64,
}
impl Request {
    pub(crate) fn parse(method: &str, value: &Value) -> Result<(Self, Action), ()> {
        let raw = Zeroizing::new(serde_json::to_vec(value).map_err(|_| ())?);
        Self::from_raw(method, &raw)
    }
    fn from_raw(method: &str, raw: &[u8]) -> Result<(Self, Action), ()> {
        if raw.len() > MAX_INPUT {
            return Err(());
        }
        let request: Self = serde_json::from_slice(raw).map_err(|_| ())?;
        let action = match (method, request.confirmation.as_str()) {
            ("backup.create", "export-current-private-pair") => Action::Create,
            ("backup.restore", "replace-current-private-pair") => Action::Restore,
            _ => return Err(()),
        };
        if !valid_private(request.schema, &request.archive.0, &request.passphrase.0)
            || request.instance_id.is_empty()
            || request.instance_id.len() > 128
            || MutationRequest::new(
                MutationKind::Other,
                Some(&request.operation_id),
                Some(request.expected_revision),
                MutationDigest::new([0; 32]),
            )
            .is_err()
        {
            return Err(());
        }
        Ok((request, action))
    }
    pub(crate) fn instance(&self) -> &str {
        &self.instance_id
    }
    pub(crate) fn revision(&self) -> u64 {
        self.expected_revision
    }
    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub(crate) fn archive(&self) -> &Path {
        Path::new(self.archive.0.as_str())
    }
    pub(crate) fn passphrase(&self) -> &[u8] {
        self.passphrase.0.as_bytes()
    }
    pub(crate) fn digest(&self, action: Action) -> MutationDigest {
        // Only a fixed digest survives. No secret-bearing Debug, cache body or
        // ordinary log. JSON temporaries are not all guaranteed zeroized.
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.extend_from_slice(b"omavless/private-pair/v1\0");
        bytes.push(u8::from(action == Action::Restore));
        for field in [
            self.archive.0.as_str(),
            self.passphrase.0.as_str(),
            self.instance_id.as_str(),
        ] {
            crate::mutation_protocol::append_field(&mut bytes, field);
        }
        bytes.extend_from_slice(&self.expected_revision.to_be_bytes());
        MutationDigest::from_semantic_bytes(&bytes)
    }
}
pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["backup", "create", "--confirm-private-export"]
        || arguments == ["backup", "restore", "--confirm-private-pair"]
}
/// Fixed semantic operation with private correlated stdin; never a fresh ID,
/// revision refresh or automatic resend after uncertainty.
pub fn from_private_input(
    arguments: &[OsString],
    input: impl Read,
) -> Result<&'static str, &'static str> {
    if !arguments_admitted(arguments) {
        return Err("private_pair_arguments_refused");
    }
    let uid = nix::unistd::Uid::current();
    if uid.is_root() || uid != nix::unistd::Uid::effective() {
        return Err("private_pair_refused");
    }
    let method = if arguments[1] == "create" {
        "backup.create"
    } else {
        "backup.restore"
    };
    let mut raw = Zeroizing::new(Vec::new());
    input
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| "private_pair_input_refused")?;
    let (request, _) = Request::from_raw(method, &raw).map_err(|_| "private_pair_input_refused")?;
    let paths = crate::RuntimePaths::current().map_err(|_| "private_pair_refused")?;
    let hello = crate::call(&paths, "system.hello", serde_json::json!({"versions":[1]}))
        .map_err(|_| "private_pair_refused")?;
    if hello["ok"] != true || hello["result"]["instanceId"] != request.instance() {
        return Err("private_pair_refused");
    }
    let params = serde_json::json!({"schema":request.schema,"archive":request.archive.0.as_str(),"passphrase":request.passphrase.0.as_str(),"confirmation":request.confirmation,"instanceId":request.instance_id,"operationId":request.operation_id,"expectedRevision":request.expected_revision});
    let response =
        crate::call_with_timeout(&paths, method, params, std::time::Duration::from_secs(120))
            .map_err(|_| "private_pair_outcome_unknown")?;
    classify_reply(&response)?;
    Ok(if method == "backup.create" {
        "private_pair_backup_completed"
    } else {
        "private_pair_restore_completed"
    })
}
fn classify_reply(response: &Value) -> Result<(), &'static str> {
    if response["ok"] == true
        && response["result"]["completed"] == true
        && response["result"]["scope"] == "privatePair"
        && response["result"]["replayed"].is_boolean()
        && response["revision"]
            .as_u64()
            .is_some_and(|n| n <= omavless_control_protocol::MAX_REVISION)
    {
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
        return Err("private_pair_refused");
    }
    Err("private_pair_outcome_unknown")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Value {
        serde_json::json!({"schema":1,"archive":"/private/synthetic.ovb","passphrase":"synthetic-only-passphrase","confirmation":"export-current-private-pair","instanceId":"current-instance","operationId":"pair-1","expectedRevision":4})
    }
    #[test]
    fn normal_pair_exact_schema_and_method_specific_confirmation() {
        assert!(Request::parse("backup.create", &input()).is_ok());
        assert!(Request::parse("backup.restore", &input()).is_err());
        for field in [
            "schema",
            "archive",
            "passphrase",
            "confirmation",
            "instanceId",
            "operationId",
            "expectedRevision",
        ] {
            let mut value = input();
            value.as_object_mut().unwrap().remove(field);
            assert!(Request::parse("backup.create", &value).is_err());
        }
        let mut value = input();
        value["grant"] = true.into();
        assert!(Request::parse("backup.create", &value).is_err());
        for id in ["", "with space", "line\n"] {
            let mut value = input();
            value["operationId"] = id.into();
            assert!(Request::parse("backup.create", &value).is_err());
        }
    }
    #[test]
    fn normal_pair_cli_has_closed_nonsecret_arguments_and_strict_private_stdin() {
        let args = |items: &[&str]| items.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(arguments_admitted(&args(&[
            "backup",
            "create",
            "--confirm-private-export"
        ])));
        assert!(arguments_admitted(&args(&[
            "backup",
            "restore",
            "--confirm-private-pair"
        ])));
        for items in [
            vec!["backup", "create"],
            vec!["backup", "restore", "--confirm-private-export"],
            vec![
                "backup",
                "create",
                "--confirm-private-export",
                "private-passphrase",
            ],
        ] {
            assert!(!arguments_admitted(&args(&items)));
        }
        let raw = serde_json::to_string(&input()).unwrap();
        assert!(Request::from_raw("backup.create", raw.as_bytes()).is_ok());
        let duplicate = raw.replace("\"schema\":1", "\"schema\":1,\"schema\":1");
        assert!(Request::from_raw("backup.create", duplicate.as_bytes()).is_err());
        assert!(Request::from_raw("backup.create", &[b' '; MAX_INPUT + 1]).is_err());
        assert!(Request::from_raw("backup.create", &[0xff]).is_err());
    }
    #[test]
    fn normal_pair_cli_does_not_turn_unknown_transport_or_backend_into_denial() {
        let success = serde_json::json!({"ok":true,"revision":1,"result":{"completed":true,"replayed":false,"scope":"privatePair"}});
        assert!(classify_reply(&success).is_ok());
        for code in [
            "manual_recovery_required",
            "internal_error",
            "future_unknown",
        ] {
            assert_eq!(
                classify_reply(&serde_json::json!({"ok":false,"error":{"code":code}})),
                Err("private_pair_outcome_unknown")
            );
        }
        for code in [
            "busy",
            "capability_unavailable",
            "conflict",
            "daemon_restarting",
            "invalid_argument",
        ] {
            assert_eq!(
                classify_reply(&serde_json::json!({"ok":false,"error":{"code":code}})),
                Err("private_pair_refused")
            );
        }
        let mut incomplete = success.clone();
        incomplete["result"]
            .as_object_mut()
            .unwrap()
            .remove("replayed");
        assert_eq!(
            classify_reply(&incomplete),
            Err("private_pair_outcome_unknown")
        );
    }
    #[test]
    fn normal_pair_digest_binds_private_data_action_instance_and_revision() {
        let (r, a) = Request::parse("backup.create", &input()).unwrap();
        let digest = r.digest(a);
        for (field, value) in [
            ("archive", Value::from("/private/other.ovb")),
            ("passphrase", Value::from("other-synthetic-passphrase")),
            ("instanceId", Value::from("other-instance")),
            ("expectedRevision", Value::from(5)),
        ] {
            let mut input = input();
            input[field] = value;
            let (r, a) = Request::parse("backup.create", &input).unwrap();
            assert!(digest != r.digest(a));
        }
        let mut input = input();
        input["confirmation"] = "replace-current-private-pair".into();
        let (r, a) = Request::parse("backup.restore", &input).unwrap();
        assert!(digest != r.digest(a));
    }
}
