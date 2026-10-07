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

pub(crate) const METHODS: &[&str] = &[
    "backup.create",
    "backup.restore",
    "backup.preview",
    "backup.restore_previewed",
];
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PreviewRequest {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
    instance_id: String,
    expected_revision: u64,
}
impl PreviewRequest {
    pub(crate) fn parse(value: &Value) -> Result<Self, ()> {
        let raw = Zeroizing::new(serde_json::to_vec(value).map_err(|_| ())?);
        Self::from_raw(&raw)
    }
    fn from_raw(raw: &[u8]) -> Result<Self, ()> {
        if raw.len() > MAX_INPUT {
            return Err(());
        }
        let request: Self = serde_json::from_slice(raw).map_err(|_| ())?;
        if !valid_private(request.schema, &request.archive.0, &request.passphrase.0)
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
    pub(crate) fn archive(&self) -> &Path {
        Path::new(self.archive.0.as_str())
    }
    pub(crate) fn passphrase(&self) -> &[u8] {
        self.passphrase.0.as_bytes()
    }
}

pub(crate) struct PreviewedRestoreRequest {
    pub(crate) pair: Request,
    pub(crate) expected: [u8; 32],
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewedWire {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
    confirmation: String,
    instance_id: String,
    operation_id: String,
    expected_revision: u64,
    expected_ciphertext_digest: String,
}
impl PreviewedRestoreRequest {
    pub(crate) fn parse(value: &Value) -> Result<Self, ()> {
        let raw = Zeroizing::new(serde_json::to_vec(value).map_err(|_| ())?);
        Self::from_raw(&raw)
    }
    fn from_raw(raw: &[u8]) -> Result<Self, ()> {
        if raw.len() > MAX_INPUT {
            return Err(());
        }
        let wire: PreviewedWire = serde_json::from_slice(raw).map_err(|_| ())?;
        if wire.confirmation != "replace-previewed-current-private-pair"
            || !valid_private(wire.schema, &wire.archive.0, &wire.passphrase.0)
            || wire.instance_id.is_empty()
            || wire.instance_id.len() > 128
            || MutationRequest::new(
                MutationKind::Other,
                Some(&wire.operation_id),
                Some(wire.expected_revision),
                MutationDigest::new([0; 32]),
            )
            .is_err()
        {
            return Err(());
        }
        let expected = parse_ciphertext_digest(&wire.expected_ciphertext_digest)?;
        Ok(Self {
            pair: Request {
                schema: wire.schema,
                archive: wire.archive,
                passphrase: wire.passphrase,
                confirmation: wire.confirmation,
                instance_id: wire.instance_id,
                operation_id: wire.operation_id,
                expected_revision: wire.expected_revision,
            },
            expected,
        })
    }
    pub(crate) fn digest(&self) -> MutationDigest {
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.extend_from_slice(b"omavless/private-pair/previewed-restore/v1\0");
        for field in [
            self.pair.archive.0.as_str(),
            self.pair.passphrase.0.as_str(),
            self.pair.instance_id.as_str(),
        ] {
            crate::mutation_protocol::append_field(&mut bytes, field);
        }
        bytes.extend_from_slice(&self.pair.expected_revision.to_be_bytes());
        bytes.extend_from_slice(&self.expected);
        MutationDigest::from_semantic_bytes(&bytes)
    }
}
fn parse_ciphertext_digest(value: &str) -> Result<[u8; 32], ()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(());
    }
    let mut result = [0; 32];
    for (target, source) in result.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
        let nibble = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
        *target = nibble(source[0]) * 16 + nibble(source[1]);
    }
    Ok(result)
}
pub(crate) fn ciphertext_hex(value: &[u8; 32]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for b in value {
        result.push(char::from(HEX[usize::from(b >> 4)]));
        result.push(char::from(HEX[usize::from(b & 15)]));
    }
    result
}
pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["backup", "create", "--confirm-private-export"]
        || arguments == ["backup", "restore", "--confirm-private-pair"]
}
pub fn preview_arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["backup", "preview"]
        || arguments == ["backup", "restore-previewed", "--confirm-private-pair"]
}
/// Fixed developer-only preview/confirmation selection. Preview output is
/// private local DATA; transport loss never supplies a follow-on grant.
pub fn preview_from_private_input(
    arguments: &[OsString],
    input: impl Read,
) -> Result<String, &'static str> {
    if !preview_arguments_admitted(arguments) {
        return Err("private_pair_arguments_refused");
    }
    let uid = nix::unistd::Uid::current();
    if uid.is_root() || uid != nix::unistd::Uid::effective() {
        return Err("private_pair_refused");
    }
    let mut raw = Zeroizing::new(Vec::new());
    input
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| "private_pair_input_refused")?;
    let preview = arguments[1] == "preview";
    let (instance, revision, params) = if preview {
        let p = PreviewRequest::from_raw(&raw).map_err(|_| "private_pair_input_refused")?;
        let params = serde_json::json!({"schema":p.schema,"archive":p.archive.0.as_str(),"passphrase":p.passphrase.0.as_str(),"instanceId":p.instance_id,"expectedRevision":p.expected_revision});
        (p.instance_id, p.expected_revision, params)
    } else {
        let p =
            PreviewedRestoreRequest::from_raw(&raw).map_err(|_| "private_pair_input_refused")?;
        let pair = p.pair;
        let params = serde_json::json!({"schema":pair.schema,"archive":pair.archive.0.as_str(),"passphrase":pair.passphrase.0.as_str(),"confirmation":pair.confirmation,"instanceId":pair.instance_id,"operationId":pair.operation_id,"expectedRevision":pair.expected_revision,"expectedCiphertextDigest":ciphertext_hex(&p.expected)});
        (pair.instance_id, pair.expected_revision, params)
    };
    let paths = crate::RuntimePaths::current().map_err(|_| "private_pair_refused")?;
    let hello = crate::call(&paths, "system.hello", serde_json::json!({"versions":[1]}))
        .map_err(|_| "private_pair_refused")?;
    if hello["ok"] != true || hello["result"]["instanceId"] != instance {
        return Err("private_pair_refused");
    }
    let response = crate::call_with_timeout(
        &paths,
        if preview {
            "backup.preview"
        } else {
            "backup.restore_previewed"
        },
        params,
        std::time::Duration::from_secs(120),
    )
    .map_err(|_| {
        if preview {
            "private_pair_preview_unavailable"
        } else {
            "private_pair_outcome_unknown"
        }
    })?;
    if !preview {
        classify_reply(&response)?;
        return Ok("private_pair_restore_completed".into());
    }
    let result = &response["result"];
    if response["ok"] != true
        || response["revision"].as_u64() != Some(revision)
        || result.as_object().is_none_or(|o| o.len() != 4)
        || result["scope"] != "privatePair"
        || result["profiles"]
            .as_u64()
            .is_none_or(|n| n > omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES as u64)
        || result["subscriptions"]
            .as_u64()
            .is_none_or(|n| n > omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES as u64)
        || result["ciphertextDigest"]
            .as_str()
            .is_none_or(|v| parse_ciphertext_digest(v).is_err())
    {
        return Err("private_pair_preview_unavailable");
    }
    // Rebuild only the allowlisted count/digest DATA, never print raw response.
    serde_json::to_string(&serde_json::json!({"profiles":result["profiles"],"subscriptions":result["subscriptions"],"ciphertextDigest":result["ciphertextDigest"],"scope":"privatePair","revision":revision}))
        .map_err(|_| "private_pair_preview_unavailable")
}
/// One owned opt-in client request. No generic method/timeout surface and no
/// owner/lease authority here: the existing normal server re-admits the request.
#[cfg(feature = "tui")]
pub fn create_for_tui(
    paths: &crate::RuntimePaths,
    request: omavless_tui::private_backup::Request,
) -> omavless_tui::private_backup::Completion {
    let response = request
        .remaining()
        .ok_or(omavless_tui::model::ReadError::Unavailable)
        .and_then(|_| {
            create_tui_exchange(paths, &request)
                .map_err(|_| omavless_tui::model::ReadError::Unavailable)
        });
    request.settle(response)
}

#[cfg(feature = "tui")]
fn create_tui_exchange(
    paths: &crate::RuntimePaths,
    request: &omavless_tui::private_backup::Request,
) -> crate::Result<Value> {
    private_tui_exchange(
        paths,
        "backup.create",
        || request.params(),
        request.deadline(),
    )
}

/// Closed Restore client request: preview or preview-bound Restore only. No
/// raw RPC/method/timeout entry point is exposed to user input.
#[cfg(feature = "tui")]
pub fn restore_for_tui(
    paths: &crate::RuntimePaths,
    request: omavless_tui::private_restore::Request,
) -> omavless_tui::private_restore::Completion {
    let response = request
        .remaining()
        .ok_or(omavless_tui::model::ReadError::Unavailable)
        .and_then(|_| {
            private_tui_exchange(
                paths,
                request.method(),
                || request.params(),
                request.deadline(),
            )
            .map_err(|_| omavless_tui::model::ReadError::Unavailable)
        });
    request.settle(response)
}

#[cfg(feature = "tui")]
fn private_tui_exchange(
    paths: &crate::RuntimePaths,
    method: &'static str,
    params: impl FnOnce() -> Value,
    end: std::time::Instant,
) -> crate::Result<Value> {
    use crate::RuntimeError;
    use nix::sys::socket::{
        AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
        sockopt::PeerCredentials,
    };
    use std::os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt},
            net::UnixStream,
        },
    };
    // All callers are fixed typed adapters above. This private allowlist also
    // prevents a later internal caller from silently broadening the transport.
    if !matches!(
        method,
        "backup.create" | "backup.preview" | "backup.restore_previewed"
    ) {
        return Err(RuntimeError::Protocol);
    }
    let live = || {
        end.checked_duration_since(std::time::Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(RuntimeError::Io)
    };
    live()?;
    let uid = nix::unistd::Uid::current().as_raw();
    crate::validate_client_directory(&paths.directory, uid)?;
    let node =
        std::fs::symlink_metadata(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
    if !node.file_type().is_socket() || node.uid() != uid || node.mode() & 0o7777 != 0o600 {
        return Err(RuntimeError::PermissionDenied);
    }
    live()?;
    // One nonblocking connect. A full backlog/unknown connection refuses;
    // there is no connect retry, query, fallback or renewed effect budget.
    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        None,
    )
    .map_err(|_| RuntimeError::Io)?;
    live()?;
    connect(
        fd.as_raw_fd(),
        &UnixAddr::new(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?,
    )
    .map_err(|_| RuntimeError::SocketUnavailable)?;
    live()?;
    let stream = UnixStream::from(fd);
    if getsockopt(&stream, PeerCredentials)
        .map_err(|_| RuntimeError::PermissionDenied)?
        .uid()
        != uid
    {
        return Err(RuntimeError::PermissionDenied);
    }
    live()?;
    stream
        .set_nonblocking(false)
        .map_err(|_| RuntimeError::Io)?;
    let id = format!("cli-{}", std::process::id());
    let mut message = omavless_control_protocol::make_request(&id, method, params())
        .map_err(|_| RuntimeError::Protocol)?;
    let encoded = omavless_control_protocol::encode_request(&message);
    crate::developer_current_restore::wipe_request(&mut message);
    let frame = Zeroizing::new(encoded.map_err(|_| RuntimeError::Protocol)?);
    let mut io = TuiDeadlineIo { stream, end };
    omavless_control_protocol::write_unary_frame(
        &mut io,
        &frame,
        omavless_control_protocol::FrameKind::Request,
    )
    .map_err(|_| RuntimeError::Io)?;
    live()?;
    io.stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| RuntimeError::Io)?;
    live()?;
    let response = omavless_control_protocol::read_unary_frame(
        &mut io,
        omavless_control_protocol::FrameKind::Response,
    )
    .and_then(|raw| omavless_control_protocol::decode_response(&raw))
    .map_err(|_| RuntimeError::Protocol)?;
    live()?;
    if response["id"].as_str() != Some(id.as_str()) {
        return Err(RuntimeError::Protocol);
    }
    Ok(response)
}

#[cfg(feature = "tui")]
struct TuiDeadlineIo {
    stream: std::os::unix::net::UnixStream,
    end: std::time::Instant,
}
#[cfg(feature = "tui")]
impl TuiDeadlineIo {
    fn remaining(&self) -> std::io::Result<std::time::Duration> {
        self.end
            .checked_duration_since(std::time::Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| std::io::ErrorKind::TimedOut.into())
    }
}
#[cfg(feature = "tui")]
impl std::io::Read for TuiDeadlineIo {
    fn read(&mut self, body: &mut [u8]) -> std::io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        self.remaining()?;
        let count = std::io::Read::read(&mut self.stream, body)?;
        self.remaining()?;
        Ok(count)
    }
}
#[cfg(feature = "tui")]
impl std::io::Write for TuiDeadlineIo {
    fn write(&mut self, body: &[u8]) -> std::io::Result<usize> {
        self.stream
            .set_write_timeout(Some(self.remaining()?.min(crate::IO_TIMEOUT)))?;
        self.remaining()?;
        let count = std::io::Write::write(&mut self.stream, body)?;
        self.remaining()?;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.remaining()?;
        std::io::Write::flush(&mut self.stream)?;
        self.remaining()?;
        Ok(())
    }
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
    #[test]
    fn preview_schema_is_separate_nonmutating_and_digest_strict() {
        let preview = serde_json::json!({"schema":1,"archive":"/private/synthetic.ovb","passphrase":"synthetic-only-secret","instanceId":"same","expectedRevision":0});
        assert!(PreviewRequest::parse(&preview).is_ok());
        for name in [
            "operationId",
            "confirmation",
            "expectedCiphertextDigest",
            "unknown",
        ] {
            let mut v = preview.clone();
            v[name] = "extra".into();
            assert!(PreviewRequest::parse(&v).is_err());
        }
        let duplicate = serde_json::to_string(&preview)
            .unwrap()
            .replace("\"schema\":1", "\"schema\":1,\"schema\":1");
        assert!(PreviewRequest::from_raw(duplicate.as_bytes()).is_err());
        let bound = serde_json::json!({"schema":1,"archive":"/private/synthetic.ovb","passphrase":"synthetic-only-secret","instanceId":"same","expectedRevision":0,"operationId":"new-op","confirmation":"replace-previewed-current-private-pair","expectedCiphertextDigest":"a".repeat(64)});
        let request = PreviewedRestoreRequest::parse(&bound).unwrap();
        let duplicate = serde_json::to_string(&bound)
            .unwrap()
            .replace("\"schema\":1", "\"schema\":1,\"schema\":1");
        assert!(PreviewedRestoreRequest::from_raw(duplicate.as_bytes()).is_err());
        assert!(request.digest() != request.pair.digest(Action::Restore));
        let mut other = bound.clone();
        other["expectedCiphertextDigest"] = "b".repeat(64).into();
        assert!(request.digest() != PreviewedRestoreRequest::parse(&other).unwrap().digest());
        for value in [
            "A".repeat(64),
            "g".repeat(64),
            "0".repeat(63),
            "0".repeat(65),
        ] {
            let mut v = bound.clone();
            v["expectedCiphertextDigest"] = value.into();
            assert!(PreviewedRestoreRequest::parse(&v).is_err());
        }
        assert!(Request::parse("backup.restore", &bound).is_err());
        let mut old = bound;
        old.as_object_mut()
            .unwrap()
            .remove("expectedCiphertextDigest");
        old["confirmation"] = "replace-current-private-pair".into();
        assert!(Request::parse("backup.restore", &old).is_ok());
        assert!(PreviewedRestoreRequest::parse(&old).is_err());
        assert!(preview_arguments_admitted(&[
            "backup".into(),
            "preview".into()
        ]));
        assert!(!arguments_admitted(&["backup".into(), "preview".into()]));
        assert_eq!(
            parse_ciphertext_digest(&ciphertext_hex(&[0xa5; 32])).unwrap(),
            [0xa5; 32]
        );
    }
    #[cfg(feature = "tui")]
    #[test]
    fn restore_tui_transport_is_closed_and_expiry_precedes_secret_serialization() {
        let paths = crate::RuntimePaths::below(Path::new("/public/nonexistent-synthetic-runtime"));
        for method in [
            "backup.create",
            "backup.preview",
            "backup.restore_previewed",
        ] {
            assert_eq!(
                private_tui_exchange(
                    &paths,
                    method,
                    || panic!("no secret params after expiry"),
                    std::time::Instant::now()
                ),
                Err(crate::RuntimeError::Io)
            );
        }
        assert_eq!(
            private_tui_exchange(
                &paths,
                "profiles.delete",
                || panic!("closed selector must refuse"),
                std::time::Instant::now() + std::time::Duration::from_secs(120)
            ),
            Err(crate::RuntimeError::Protocol)
        );
    }
    #[cfg(feature = "tui")]
    #[test]
    fn restore_tui_real_preview_transport_false_factory_has_no_followon_restore() {
        use omavless_tui::private_restore::{State, Workspace};
        let base = crate::test_temp::directory("r-preview").unwrap();
        let paths = crate::RuntimePaths::below(&base);
        let server = crate::RuntimeServer::bind(paths.clone()).unwrap();
        let mut editor = Workspace::new(&server.instance_id, 0).unwrap();
        for ch in "/private/public-fixture.ovb".chars() {
            assert!(editor.push(ch));
        }
        editor.next_field();
        for ch in "synthetic-restore-secret".chars() {
            assert!(editor.push(ch));
        }
        let request = editor.begin_preview().unwrap();
        assert!(PreviewRequest::parse(&request.params()).is_ok());
        let worker = std::thread::spawn(move || server.serve(Some(1)).unwrap());
        editor.accept(restore_for_tui(&paths, request));
        worker.join().unwrap();
        assert_eq!(editor.state(), State::PreviewUnavailable);
        assert!(editor.submit("no-followon".into()).is_none());
        assert!(editor.begin_preview().is_none());
        assert!(!base.join("profiles.json").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
    #[cfg(feature = "tui")]
    #[test]
    fn restore_tui_synthetic_preview_data_does_not_grant_real_server_authority() {
        use omavless_tui::private_restore::{State, Workspace};
        let base = crate::test_temp::directory("r-submit").unwrap();
        let paths = crate::RuntimePaths::below(&base);
        let server = crate::RuntimeServer::bind(paths.clone()).unwrap();
        let mut editor = Workspace::new(&server.instance_id, 0).unwrap();
        for ch in "/private/public-fixture.ovb".chars() {
            assert!(editor.push(ch));
        }
        editor.next_field();
        for ch in "synthetic-restore-secret".chars() {
            assert!(editor.push(ch));
        }
        // Local positive DATA-only rendering fixture. The real runtime has NO
        // current owner and must reject this correlated request before entry.
        let preview = editor.begin_preview().unwrap();
        editor.accept(preview.settle(Ok(serde_json::json!({"ok":true,"revision":0,"result":{
            "profiles":0,"subscriptions":0,"scope":"privatePair","ciphertextDigest":"a".repeat(64)
        }}))));
        assert_eq!(editor.state(), State::Confirming);
        let request = editor.submit("r-submit-once".into()).unwrap();
        assert!(PreviewedRestoreRequest::parse(&request.params()).is_ok());
        assert_eq!(request.params()["expectedCiphertextDigest"], "a".repeat(64));
        let worker = std::thread::spawn(move || server.serve(Some(1)).unwrap());
        editor.accept(restore_for_tui(&paths, request));
        worker.join().unwrap();
        assert_eq!(editor.state(), State::Denied);
        assert!(editor.submit("no-retry".into()).is_none());
        assert!(!base.join("profiles.json").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
    #[cfg(feature = "tui")]
    #[test]
    fn backup_tui_expired_transport_refuses_before_write_and_does_not_touch_peer() {
        use std::io::{Read, Write};
        let (stream, mut peer) = std::os::unix::net::UnixStream::pair().unwrap();
        peer.set_nonblocking(true).unwrap();
        let mut io = TuiDeadlineIo {
            stream,
            end: std::time::Instant::now(),
        };
        assert_eq!(
            io.write(b"synthetic-only-frame").unwrap_err().kind(),
            std::io::ErrorKind::TimedOut
        );
        assert_eq!(io.flush().unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(
            peer.read(&mut [0u8; 32]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[cfg(feature = "tui")]
    #[test]
    fn backup_tui_possible_write_then_timeout_keeps_unknown_without_cancel_or_retry() {
        use omavless_tui::private_backup::{State, Workspace};
        use std::io::{Read, Write};
        let mut w = Workspace::new("synthetic", 0).unwrap();
        for ch in "/private/a.ovb".chars() {
            w.push(ch);
        }
        w.next_field();
        for ch in "synthetic-secret".chars() {
            w.push(ch);
        }
        w.next_field();
        for ch in "synthetic-secret".chars() {
            w.push(ch);
        }
        assert!(w.confirm());
        let request = w.submit("original".into()).unwrap();
        let (stream, mut peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut io = TuiDeadlineIo {
            stream,
            end: request.deadline(),
        };
        assert_eq!(io.end, request.deadline());
        io.write_all(b"synthetic-only-frame").unwrap();
        let mut raw = [0u8; 20];
        peer.read_exact(&mut raw).unwrap();
        assert_eq!(&raw, b"synthetic-only-frame");
        // Synthetic expired clock tests the SAME production guard after an
        // actual kernel write; not a live engine or 120s wall-clock acceptance.
        io.end = std::time::Instant::now();
        assert_eq!(
            io.read(&mut [0u8; 1]).unwrap_err().kind(),
            std::io::ErrorKind::TimedOut
        );
        w.accept(request.settle(Err(omavless_tui::model::ReadError::Unavailable)));
        assert_eq!(w.state(), State::Unknown);
        assert!(w.submit("retry".into()).is_none());
        peer.set_nonblocking(true).unwrap();
        assert_eq!(
            peer.read(&mut [0u8; 1]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[cfg(feature = "tui")]
    #[test]
    fn backup_tui_owned_request_uses_real_transport_and_no_current_factory_denies() {
        use omavless_tui::private_backup::{State, Workspace};
        let base = crate::test_temp::directory("ui-client").unwrap();
        let paths = crate::RuntimePaths::below(&base);
        let server = crate::RuntimeServer::bind(paths.clone()).unwrap();
        let mut editor = Workspace::new(&server.instance_id, 0).unwrap();
        for ch in "/private/public-fixture.ovb".chars() {
            assert!(editor.push(ch));
        }
        editor.next_field();
        for ch in "synthetic-backup-secret".chars() {
            assert!(editor.push(ch));
        }
        editor.next_field();
        for ch in "synthetic-backup-secret".chars() {
            assert!(editor.push(ch));
        }
        assert!(editor.confirm());
        let request = editor.submit("ui-client-once".into()).unwrap();
        // Real bind/peer/framing/default handler, not a genuine-current stand-in.
        let worker = std::thread::spawn(move || server.serve(Some(1)).unwrap());
        editor.accept(create_for_tui(&paths, request));
        worker.join().unwrap();
        assert_eq!(editor.state(), State::Denied);
        assert!(editor.submit("fresh-id".into()).is_none());
        assert!(!base.join("profiles.json").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
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
