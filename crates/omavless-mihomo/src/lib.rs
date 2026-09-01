// SPDX-License-Identifier: MIT

//! Bounded Mihomo integration primitives for the staged Rust migration.
//!
//! This crate does not own the VPN lifecycle. It cannot connect, disconnect,
//! change routing, or start a persistent core. It provides only executable
//! discovery, config validation, read-only Unix-controller requests, and safe
//! host readiness facts for later runtime work.

use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub mod diagnostics;
pub mod observation;

pub const MAX_CONTROLLER_RESPONSE_BYTES: usize = 512 * 1024;
pub const MAX_CONTROLLER_HEADER_BYTES: usize = 32 * 1024;
pub const MAX_CONTROLLER_JSON_STRING_BYTES: usize = 64 * 1024;
pub const MAX_CONTROLLER_JSON_DEPTH: usize = 32;
pub const MAX_CORE_PATH_BYTES: usize = 4096;
pub const MAX_SOCKET_PATH_BYTES: usize = 4096;
pub const MAX_PROBE_TARGETS: usize = 64;
pub const MAX_PROBE_ALIAS_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidArgument,
    CoreUnavailable,
    CoreRejected,
    TimedOut,
    ControllerUnavailable,
    ControllerRejected,
    InvalidResponse,
    ResponseTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MihomoError {
    kind: ErrorKind,
}

impl MihomoError {
    #[must_use]
    pub const fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub const fn kind(self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for MihomoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            ErrorKind::InvalidArgument => "Mihomo integration input is invalid",
            ErrorKind::CoreUnavailable => "Mihomo is unavailable",
            ErrorKind::CoreRejected => "Mihomo rejected the configuration",
            ErrorKind::TimedOut => "Mihomo operation timed out",
            ErrorKind::ControllerUnavailable => "Private Mihomo controller is unavailable",
            ErrorKind::ControllerRejected => "Mihomo controller rejected the request",
            ErrorKind::InvalidResponse => "Mihomo controller returned an invalid response",
            ErrorKind::ResponseTooLarge => "Mihomo controller response is too large",
        })
    }
}

impl std::error::Error for MihomoError {}

pub type Result<T> = std::result::Result<T, MihomoError>;

fn valid_path(path: &Path) -> bool {
    let encoded = path.as_os_str().as_encoded_bytes();
    !encoded.is_empty() && encoded.len() <= MAX_CORE_PATH_BYTES && !encoded.contains(&0)
}

fn executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Select the first executable candidate, resolving it to an absolute path.
///
/// Candidate order is policy: explicit override, user-local install, then the
/// caller-provided PATH result. The function never invokes a shell.
pub fn find_core_from_candidates<'a>(
    candidates: impl IntoIterator<Item = &'a Path>,
) -> Result<PathBuf> {
    for candidate in candidates {
        if valid_path(candidate) && executable_file(candidate) {
            return fs::canonicalize(candidate)
                .map_err(|_| MihomoError::new(ErrorKind::CoreUnavailable));
        }
    }
    Err(MihomoError::new(ErrorKind::CoreUnavailable))
}

/// Discover Mihomo with the same precedence as the current Python runtime.
pub fn discover_core(home: &Path, path_lookup: Option<&Path>) -> Result<PathBuf> {
    if !valid_path(home) {
        return Err(MihomoError::new(ErrorKind::InvalidArgument));
    }
    let override_path = env::var_os("OMAVLESS_MIHOMO").filter(|value| !value.is_empty());
    let user_local = home.join(".local/bin/mihomo");
    let mut candidates = Vec::with_capacity(3);
    if let Some(path) = override_path {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(user_local);
    if let Some(path) = path_lookup {
        candidates.push(path.to_path_buf());
    }
    find_core_from_candidates(candidates.iter().map(PathBuf::as_path))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationOutcome {
    pub elapsed: Duration,
}

/// Run Mihomo's built-in config validator with fixed argv and bounded time.
/// Output is discarded so a rejected private configuration cannot reach a
/// public diagnostic channel.
pub fn validate_config(
    core: &Path,
    data_dir: &Path,
    config: &Path,
    timeout: Duration,
) -> Result<ValidationOutcome> {
    if !valid_path(core)
        || !executable_file(core)
        || !valid_path(data_dir)
        || !fs::metadata(data_dir).is_ok_and(|metadata| metadata.is_dir())
        || !valid_path(config)
        || !fs::metadata(config).is_ok_and(|metadata| metadata.is_file())
        || timeout.is_zero()
        || timeout > Duration::from_secs(120)
    {
        return Err(MihomoError::new(ErrorKind::InvalidArgument));
    }
    let started = Instant::now();
    let mut child = Command::new(core)
        .arg("-t")
        .arg("-d")
        .arg(data_dir)
        .arg("-f")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| MihomoError::new(ErrorKind::CoreUnavailable))?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return Ok(ValidationOutcome {
                    elapsed: started.elapsed(),
                });
            }
            Ok(Some(_)) => return Err(MihomoError::new(ErrorKind::CoreRejected)),
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MihomoError::new(ErrorKind::TimedOut));
            }
            Err(_) => return Err(MihomoError::new(ErrorKind::CoreUnavailable)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyEndpoint {
    Version,
    Configs,
    Proxies,
    Rules,
    RuleProviders,
    Connections,
}

impl ReadOnlyEndpoint {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Version => "/version",
            Self::Configs => "/configs",
            Self::Proxies => "/proxies",
            Self::Rules => "/rules",
            Self::RuleProviders => "/providers/rules",
            Self::Connections => "/connections",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControllerResponse {
    pub status: u16,
    pub payload: Value,
}

fn find_header_end(response: &[u8]) -> Option<usize> {
    response.windows(4).position(|window| window == b"\r\n\r\n")
}

fn bounded_json(value: &Value, depth: usize) -> bool {
    if depth > MAX_CONTROLLER_JSON_DEPTH {
        return false;
    }
    match value {
        Value::String(text) => text.len() <= MAX_CONTROLLER_JSON_STRING_BYTES,
        Value::Array(items) => items
            .iter()
            .all(|item| bounded_json(item, depth.saturating_add(1))),
        Value::Object(object) => object.iter().all(|(key, item)| {
            key.len() <= MAX_CONTROLLER_JSON_STRING_BYTES
                && bounded_json(item, depth.saturating_add(1))
        }),
        _ => true,
    }
}

/// Parse one bounded HTTP/1.x JSON response without exposing its body in errors.
pub fn parse_controller_response(response: &[u8]) -> Result<ControllerResponse> {
    if response.len() > MAX_CONTROLLER_RESPONSE_BYTES {
        return Err(MihomoError::new(ErrorKind::ResponseTooLarge));
    }
    let header_end =
        find_header_end(response).ok_or_else(|| MihomoError::new(ErrorKind::InvalidResponse))?;
    if header_end + 4 > MAX_CONTROLLER_HEADER_BYTES {
        return Err(MihomoError::new(ErrorKind::InvalidResponse));
    }
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|_| MihomoError::new(ErrorKind::InvalidResponse))?;
    let mut lines = header.split("\r\n");
    let mut status_parts = lines
        .next()
        .ok_or_else(|| MihomoError::new(ErrorKind::InvalidResponse))?
        .split_ascii_whitespace();
    let protocol = status_parts
        .next()
        .ok_or_else(|| MihomoError::new(ErrorKind::InvalidResponse))?;
    if !matches!(protocol, "HTTP/1.0" | "HTTP/1.1") {
        return Err(MihomoError::new(ErrorKind::InvalidResponse));
    }
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| (100..=599).contains(value))
        .ok_or_else(|| MihomoError::new(ErrorKind::InvalidResponse))?;
    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| MihomoError::new(ErrorKind::InvalidResponse))?;
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(MihomoError::new(ErrorKind::InvalidResponse));
            }
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| MihomoError::new(ErrorKind::InvalidResponse))?,
            );
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
        {
            chunked = true;
        }
    }
    if chunked {
        return Err(MihomoError::new(ErrorKind::InvalidResponse));
    }
    let body = &response[header_end + 4..];
    if content_length.is_some_and(|length| length != body.len()) {
        return Err(MihomoError::new(ErrorKind::InvalidResponse));
    }
    let payload = if body.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_slice(body).map_err(|_| MihomoError::new(ErrorKind::InvalidResponse))?
    };
    if !bounded_json(&payload, 0) {
        return Err(MihomoError::new(ErrorKind::InvalidResponse));
    }
    Ok(ControllerResponse { status, payload })
}

/// Read one bounded response from a private Unix controller. The endpoint is
/// an enum so callers cannot turn this into an arbitrary HTTP client.
pub fn controller_get(
    socket_path: &Path,
    endpoint: ReadOnlyEndpoint,
    timeout: Duration,
    max_response_bytes: usize,
) -> Result<ControllerResponse> {
    let encoded_path = socket_path.as_os_str().as_encoded_bytes();
    if encoded_path.is_empty()
        || encoded_path.len() > MAX_SOCKET_PATH_BYTES
        || encoded_path.contains(&0)
        || timeout.is_zero()
        || timeout > Duration::from_secs(120)
        || !(1..=MAX_CONTROLLER_RESPONSE_BYTES).contains(&max_response_bytes)
    {
        return Err(MihomoError::new(ErrorKind::InvalidArgument));
    }
    if fs::symlink_metadata(socket_path).is_ok_and(|metadata| !metadata.file_type().is_socket()) {
        return Err(MihomoError::new(ErrorKind::ControllerUnavailable));
    }
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|_| MihomoError::new(ErrorKind::ControllerUnavailable))?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|_| MihomoError::new(ErrorKind::ControllerUnavailable))?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        endpoint.path()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| MihomoError::new(ErrorKind::ControllerUnavailable))?;
    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = stream
            .read(&mut chunk)
            .map_err(|_| MihomoError::new(ErrorKind::ControllerUnavailable))?;
        if count == 0 {
            break;
        }
        if response.len().saturating_add(count) > max_response_bytes {
            return Err(MihomoError::new(ErrorKind::ResponseTooLarge));
        }
        response.extend_from_slice(&chunk[..count]);
    }
    let parsed = parse_controller_response(&response)?;
    if !(200..300).contains(&parsed.status) {
        return Err(MihomoError::new(ErrorKind::ControllerRejected));
    }
    Ok(parsed)
}

/// Merge one Mihomo group-delay response using the current bounded Python
/// semantics. A 504 all-timeout response is a successfully completed sample
/// with no delays. Unknown aliases and invalid delay values are ignored.
pub fn merge_probe_response(
    samples: &mut BTreeMap<String, Vec<u32>>,
    status: u16,
    payload: &Value,
) -> bool {
    if samples.len() > MAX_PROBE_TARGETS
        || samples
            .keys()
            .any(|alias| alias.is_empty() || alias.len() > MAX_PROBE_ALIAS_BYTES)
    {
        return false;
    }
    let Some(object) = payload.as_object() else {
        return false;
    };
    if status == 504
        && object.get("message").and_then(Value::as_str) == Some("get delay: all proxies timeout")
    {
        return true;
    }
    if status != 200 {
        return false;
    }
    for (alias, value) in object {
        let delay = match value {
            Value::Bool(true) => Some(1),
            Value::Number(number) => number.as_u64().and_then(|value| u32::try_from(value).ok()),
            _ => None,
        };
        if let (Some(target), Some(delay)) = (samples.get_mut(alias), delay)
            && (1..=60_000).contains(&delay)
        {
            target.push(delay);
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    Arch,
    NixOs,
    Other,
}

/// Classify only supported packaging families from bounded `/etc/os-release`
/// fields. The caller remains responsible for reading and bounding the file.
#[must_use]
pub fn classify_host_platform(id: &str, id_like: &str) -> HostPlatform {
    let values = std::iter::once(id).chain(id_like.split_ascii_whitespace());
    let mut arch = false;
    let mut nixos = false;
    for value in values {
        let normalized = value.trim_matches('"').to_ascii_lowercase();
        arch |= normalized == "arch";
        nixos |= normalized == "nixos";
    }
    if nixos {
        HostPlatform::NixOs
    } else if arch {
        HostPlatform::Arch
    } else {
        HostPlatform::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeObservation {
    pub user_service_active: bool,
    pub supervisor_count: u16,
    pub core_count: u16,
    pub tun_count: u16,
    pub private_controller_live: bool,
    pub tcp_controller_count: u16,
}

impl RuntimeObservation {
    #[must_use]
    pub const fn healthy_connected(self) -> bool {
        self.user_service_active
            && self.supervisor_count == 1
            && self.core_count == 1
            && self.tun_count == 1
            && self.private_controller_live
            && self.tcp_controller_count == 0
    }

    #[must_use]
    pub const fn clean_disconnected(self) -> bool {
        !self.user_service_active
            && self.supervisor_count == 0
            && self.core_count == 0
            && self.tun_count == 0
            && !self.private_controller_live
            && self.tcp_controller_count == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostReadiness {
    pub core_installed: bool,
    pub tun_ready: bool,
    pub runtime_dir_private: bool,
    pub user_service_manager: bool,
}

impl HostReadiness {
    #[must_use]
    pub const fn ready_for_full_vpn(self) -> bool {
        self.core_installed
            && self.tun_ready
            && self.runtime_dir_private
            && self.user_service_manager
    }
}

/// Project already-collected facts into a credential-free readiness model.
#[must_use]
pub fn host_readiness(
    core_installed: bool,
    capabilities: &str,
    runtime_dir_mode: Option<u32>,
    user_service_manager: bool,
) -> HostReadiness {
    let tun_ready = capabilities.contains("cap_net_admin")
        && capabilities.contains("cap_net_raw")
        && capabilities.contains("cap_net_bind_service");
    HostReadiness {
        core_installed,
        tun_ready,
        runtime_dir_private: matches!(runtime_dir_mode, Some(mode) if mode & 0o077 == 0),
        user_service_manager,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::mpsc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn response(status: &str, body: &[u8]) -> Vec<u8> {
        let mut result = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        result.extend_from_slice(body);
        result
    }

    fn temporary_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path =
            env::temp_dir().join(format!("omavless-{label}-{}-{unique}", std::process::id()));
        fs::create_dir(&path).expect("temporary root");
        path
    }

    #[test]
    fn controller_response_is_bounded_and_safe() {
        let parsed = parse_controller_response(&response("200 OK", br#"{"version":"1.2.3"}"#))
            .expect("valid response");
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.payload["version"], "1.2.3");

        let private = b"vless://secret-uuid@private.example:443?password=secret";
        for invalid in [
            private.to_vec(),
            response("200 OK", private),
            b"HTTP/2 200\r\n\r\n{}".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n{}".to_vec(),
            b"HTTP/1.1 nope\r\n\r\n{}".to_vec(),
        ] {
            let error = parse_controller_response(&invalid).expect_err("invalid response");
            assert!(!error.to_string().contains("secret"));
            assert!(!error.to_string().contains("private.example"));
        }
    }

    #[test]
    fn response_rejects_deep_or_long_json_and_large_frames() {
        let deep = format!("{}0{}", "[".repeat(34), "]".repeat(34));
        assert_eq!(
            parse_controller_response(&response("200 OK", deep.as_bytes()))
                .expect_err("deep response")
                .kind(),
            ErrorKind::InvalidResponse
        );
        let long = serde_json::to_vec(&serde_json::json!({
            "value": "x".repeat(MAX_CONTROLLER_JSON_STRING_BYTES + 1)
        }))
        .expect("JSON");
        assert_eq!(
            parse_controller_response(&response("200 OK", &long))
                .expect_err("long response")
                .kind(),
            ErrorKind::InvalidResponse
        );
        assert_eq!(
            parse_controller_response(&vec![b'x'; MAX_CONTROLLER_RESPONSE_BYTES + 1])
                .expect_err("large response")
                .kind(),
            ErrorKind::ResponseTooLarge
        );
    }

    #[test]
    fn unix_client_uses_fixed_read_only_request() {
        let root = temporary_root("controller");
        let socket = root.join("controller.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("listener");
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 512];
            let count = stream.read(&mut request).expect("request");
            sender
                .send(request[..count].to_vec())
                .expect("send request");
            stream
                .write_all(&response("200 OK", br#"{"version":"test"}"#))
                .expect("response");
        });
        let parsed = controller_get(
            &socket,
            ReadOnlyEndpoint::Version,
            Duration::from_secs(2),
            4096,
        )
        .expect("controller response");
        assert_eq!(parsed.payload["version"], "test");
        let request = receiver.recv().expect("request capture");
        assert!(request.starts_with(b"GET /version HTTP/1.1\r\n"));
        worker.join().expect("worker");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn discovery_obeys_candidate_order() {
        let root = temporary_root("discovery");
        let first = root.join("first");
        let second = root.join("second");
        fs::write(&first, b"#!/bin/sh\nexit 0\n").expect("first");
        fs::write(&second, b"#!/bin/sh\nexit 0\n").expect("second");
        fs::set_permissions(&first, fs::Permissions::from_mode(0o700)).expect("mode");
        fs::set_permissions(&second, fs::Permissions::from_mode(0o700)).expect("mode");
        assert_eq!(
            find_core_from_candidates([first.as_path(), second.as_path()]).expect("core"),
            fs::canonicalize(&first).expect("canonical")
        );
        fs::set_permissions(&first, fs::Permissions::from_mode(0o600)).expect("mode");
        assert_eq!(
            find_core_from_candidates([first.as_path(), second.as_path()]).expect("core"),
            fs::canonicalize(&second).expect("canonical")
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn validation_has_fixed_argv_and_safe_errors() {
        let root = temporary_root("validation");
        let config = root.join("config with spaces.yaml");
        fs::write(&config, b"mixed-port: 0\n").expect("config");
        let core = root.join("fake core");
        fs::write(
            &core,
            b"#!/bin/sh\n[ \"$1\" = '-t' ] && [ \"$2\" = '-d' ] && [ -d \"$3\" ] && [ \"$4\" = '-f' ] && [ -f \"$5\" ]\n",
        )
        .expect("core");
        fs::set_permissions(&core, fs::Permissions::from_mode(0o700)).expect("mode");
        validate_config(&core, &root, &config, Duration::from_secs(2)).expect("valid config");

        fs::write(&core, b"#!/bin/sh\necho 'password=private' >&2\nexit 1\n").expect("core");
        let error = validate_config(&core, &root, &config, Duration::from_secs(2))
            .expect_err("rejected config");
        assert_eq!(error.kind(), ErrorKind::CoreRejected);
        assert!(!error.to_string().contains("private"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn readiness_requires_all_private_full_vpn_facts() {
        let ready = host_readiness(
            true,
            "cap_net_admin,cap_net_raw,cap_net_bind_service=ep",
            Some(0o700),
            true,
        );
        assert!(ready.ready_for_full_vpn());
        assert!(!host_readiness(true, "cap_net_admin", Some(0o755), true).ready_for_full_vpn());
    }

    #[test]
    fn platform_and_runtime_observation_are_explicit() {
        assert_eq!(classify_host_platform("arch", ""), HostPlatform::Arch);
        assert_eq!(
            classify_host_platform("omarchy", "arch"),
            HostPlatform::Arch
        );
        assert_eq!(classify_host_platform("nixos", ""), HostPlatform::NixOs);
        assert_eq!(
            classify_host_platform("ubuntu", "debian"),
            HostPlatform::Other
        );

        let connected = RuntimeObservation {
            user_service_active: true,
            supervisor_count: 1,
            core_count: 1,
            tun_count: 1,
            private_controller_live: true,
            tcp_controller_count: 0,
        };
        assert!(connected.healthy_connected());
        assert!(
            !RuntimeObservation {
                core_count: 2,
                ..connected
            }
            .healthy_connected()
        );
        assert!(
            RuntimeObservation {
                user_service_active: false,
                supervisor_count: 0,
                core_count: 0,
                tun_count: 0,
                private_controller_live: false,
                tcp_controller_count: 0,
            }
            .clean_disconnected()
        );
    }
}
