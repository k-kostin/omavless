// SPDX-License-Identifier: MIT

//! R5 runtime foundation: private IPC, lifetime ownership, and read-only
//! semantic dispatch. This checkpoint deliberately cannot start or stop
//! Mihomo and does not replace the Python production owner.

use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg, OFlag};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use nix::unistd::Uid;
use omavless_control_protocol::{
    API_VERSION, FrameKind, MAX_REQUEST_FRAME_BYTES, MAX_RESPONSE_FRAME_BYTES, StableErrorCode,
    decode_request, decode_response, encode_request, encode_response, error_response, make_request,
    negotiate_version, read_unary_frame, success_response, write_unary_frame,
};
use serde_json::{Value, json};
use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod core;
pub mod cutover;
pub mod cutover_transaction;
pub mod desired;
pub mod lifecycle;
pub mod mutation;
pub mod mutation_binding;
pub mod mutation_protocol;
pub mod native_host;
pub mod owner;
pub mod production_observation;
pub mod profile_mutation;
pub mod profile_mutation_protocol;
pub mod store_preflight;
pub mod subscription_mutation;
pub mod subscription_mutation_protocol;
pub mod subscription_refresh;

pub const SOCKET_NAME: &str = "control.sock";
pub const OWNER_LOCK_NAME: &str = "owner.lock";
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError {
    UnsafeRuntimeDirectory,
    AlreadyRunning,
    SocketUnavailable,
    PermissionDenied,
    Protocol,
    Io,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeRuntimeDirectory => "OmaVLESS runtime directory is unsafe",
            Self::AlreadyRunning => "Another OmaVLESS runtime already owns this session",
            Self::SocketUnavailable => "OmaVLESS runtime socket is unavailable",
            Self::PermissionDenied => "OmaVLESS runtime peer is not permitted",
            Self::Protocol => "OmaVLESS control exchange is invalid",
            Self::Io => "OmaVLESS runtime I/O failed",
        })
    }
}

impl std::error::Error for RuntimeError {}

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub directory: PathBuf,
    pub socket: PathBuf,
    pub owner_lock: PathBuf,
}

impl RuntimePaths {
    #[must_use]
    pub fn below(base: &Path) -> Self {
        let directory = base.join("omavless");
        Self {
            socket: directory.join(SOCKET_NAME),
            owner_lock: directory.join(OWNER_LOCK_NAME),
            directory,
        }
    }

    pub fn current() -> Result<Self> {
        let base = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", Uid::current().as_raw())));
        if !base.is_absolute() {
            return Err(RuntimeError::UnsafeRuntimeDirectory);
        }
        Ok(Self::below(&base))
    }
}

fn validate_directory(path: &Path, uid: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RuntimeError::UnsafeRuntimeDirectory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != uid {
        return Err(RuntimeError::UnsafeRuntimeDirectory);
    }
    Ok(())
}

fn prepare_runtime_directory(path: &Path, uid: u32) -> Result<()> {
    let parent = path.parent().ok_or(RuntimeError::UnsafeRuntimeDirectory)?;
    validate_directory(parent, uid)?;
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(RuntimeError::Io),
        }
    }
    validate_directory(path, uid)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| RuntimeError::Io)
}

struct OwnerLock {
    _file: Flock<File>,
}

impl OwnerLock {
    fn acquire(path: &Path, uid: u32) -> Result<Self> {
        if let Ok(metadata) = fs::symlink_metadata(path)
            && (metadata.file_type().is_symlink() || !metadata.is_file() || metadata.uid() != uid)
        {
            return Err(RuntimeError::UnsafeRuntimeDirectory);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(OFlag::O_NOFOLLOW.bits())
            .open(path)
            .map_err(|_| RuntimeError::Io)?;
        let file =
            Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_file, error)| {
                if matches!(error, Errno::EAGAIN) {
                    RuntimeError::AlreadyRunning
                } else {
                    RuntimeError::Io
                }
            })?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| RuntimeError::Io)?;
        Ok(Self { _file: file })
    }
}

pub struct RuntimeServer {
    listener: UnixListener,
    paths: RuntimePaths,
    uid: u32,
    instance_id: String,
    revision: u64,
    _owner: OwnerLock,
}

impl RuntimeServer {
    pub fn bind(paths: RuntimePaths) -> Result<Self> {
        let uid = Uid::current().as_raw();
        prepare_runtime_directory(&paths.directory, uid)?;
        let owner = OwnerLock::acquire(&paths.owner_lock, uid)?;
        if let Ok(metadata) = fs::symlink_metadata(&paths.socket) {
            if !metadata.file_type().is_socket() || metadata.uid() != uid {
                return Err(RuntimeError::UnsafeRuntimeDirectory);
            }
            fs::remove_file(&paths.socket).map_err(|_| RuntimeError::Io)?;
        }
        let listener =
            UnixListener::bind(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600))
            .map_err(|_| RuntimeError::Io)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RuntimeError::Io)?
            .as_nanos();
        Ok(Self {
            listener,
            paths,
            uid,
            instance_id: format!("{:x}-{nonce:x}", std::process::id()),
            revision: 0,
            _owner: owner,
        })
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.paths.socket
    }

    pub fn serve(self, maximum_connections: Option<usize>) -> Result<()> {
        for (handled, incoming) in self.listener.incoming().enumerate() {
            if let Ok(mut stream) = incoming {
                // A malformed, slow, disconnected, or unauthorized client is
                // isolated to its bounded unary exchange. Client failure must
                // never terminate the canonical runtime process.
                let _ = self.handle(&mut stream);
            }
            if maximum_connections.is_some_and(|maximum| handled + 1 >= maximum) {
                break;
            }
        }
        Ok(())
    }

    pub fn serve_until(self, stop: &AtomicBool) -> Result<()> {
        self.listener
            .set_nonblocking(true)
            .map_err(|_| RuntimeError::Io)?;
        while !stop.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((mut stream, _address)) => {
                    let _ = self.handle(&mut stream);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return Err(RuntimeError::Io),
            }
        }
        Ok(())
    }

    fn handle(&self, stream: &mut UnixStream) -> Result<()> {
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(|_| RuntimeError::Io)?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(|_| RuntimeError::Io)?;
        let credentials =
            getsockopt(&*stream, PeerCredentials).map_err(|_| RuntimeError::PermissionDenied)?;
        if credentials.uid() != self.uid {
            return Err(RuntimeError::PermissionDenied);
        }
        let response = match read_unary_frame(stream, FrameKind::Request)
            .and_then(|frame| decode_request(&frame))
        {
            Ok(request) => dispatch(&request, &self.instance_id, self.revision),
            Err(error) => error_response(
                "invalid",
                self.revision,
                error.code(),
                false,
                (error.code() == StableErrorCode::UnsupportedVersion)
                    .then(|| json!({"supported": [API_VERSION]})),
            ),
        }
        .map_err(|_| RuntimeError::Protocol)?;
        let frame = encode_response(&response).map_err(|_| RuntimeError::Protocol)?;
        write_unary_frame(stream, &frame, FrameKind::Response).map_err(|_| RuntimeError::Io)
    }
}

impl Drop for RuntimeServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.paths.socket);
    }
}

fn empty_params(request: &Value) -> bool {
    request["params"]
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
}

fn dispatch(
    request: &Value,
    instance_id: &str,
    revision: u64,
) -> std::result::Result<Value, omavless_control_protocol::ProtocolError> {
    let id = request["id"].as_str().unwrap_or("invalid");
    let method = request["method"].as_str().unwrap_or_default();
    let result = match method {
        "system.hello" => {
            let params = request["params"].as_object();
            let versions = params.and_then(|value| value.get("versions"));
            if params.is_none_or(|value| value.len() != 1) || versions.is_none() {
                return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
            }
            if let Err(error) = negotiate_version(versions.unwrap_or(&Value::Null)) {
                return error_response(
                    id,
                    revision,
                    error.code(),
                    false,
                    (error.code() == StableErrorCode::UnsupportedVersion)
                        .then(|| json!({"supported": [API_VERSION]})),
                );
            }
            json!({
                "instanceId": instance_id,
                "version": API_VERSION,
                "versions": [API_VERSION],
                "limits": {
                    "requestFrameBytes": MAX_REQUEST_FRAME_BYTES,
                    "responseFrameBytes": MAX_RESPONSE_FRAME_BYTES
                },
                "runtimeOwnership": false
            })
        }
        "status.get" if empty_params(request) => json!({
            "desired": "disconnected",
            "actual": "disconnected",
            "activeProfileId": "",
            "mode": "rule",
            "transition": null,
            "runtimeOwnership": false
        }),
        "capabilities.get" if empty_params(request) => json!({
            "runtimeOwnership": false,
            "mutations": false,
            "methods": ["system.hello", "status.get", "capabilities.get"]
        }),
        "status.get" | "capabilities.get" => {
            return error_response(id, revision, StableErrorCode::InvalidArgument, false, None);
        }
        _ => return error_response(id, revision, StableErrorCode::UnknownMethod, false, None),
    };
    success_response(id, revision, result)
}

pub fn call(paths: &RuntimePaths, method: &str, params: Value) -> Result<Value> {
    let mut stream =
        UnixStream::connect(&paths.socket).map_err(|_| RuntimeError::SocketUnavailable)?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| RuntimeError::Io)?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| RuntimeError::Io)?;
    let id = format!("cli-{}", std::process::id());
    let request = make_request(&id, method, params).map_err(|_| RuntimeError::Protocol)?;
    let frame = encode_request(&request).map_err(|_| RuntimeError::Protocol)?;
    write_unary_frame(&mut stream, &frame, FrameKind::Request).map_err(|_| RuntimeError::Io)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| RuntimeError::Io)?;
    let response = read_unary_frame(&mut stream, FrameKind::Response)
        .and_then(|frame| decode_response(&frame))
        .map_err(|_| RuntimeError::Protocol)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn temporary_base(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = env::temp_dir().join(format!(
            "omavless-runtime-{label}-{}-{nonce}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(&base).unwrap();
        base
    }

    #[test]
    fn socket_and_owner_are_private_and_singleton() {
        let base = temporary_base("owner");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        assert_eq!(
            fs::metadata(&paths.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&paths.socket).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(matches!(
            RuntimeServer::bind(paths.clone()),
            Err(RuntimeError::AlreadyRunning)
        ));
        drop(server);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn read_only_unary_api_round_trips() {
        let base = temporary_base("api");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        let worker = thread::spawn(move || server.serve(Some(3)).unwrap());
        for (method, params) in [
            ("system.hello", json!({"versions": [1]})),
            ("status.get", json!({})),
            ("capabilities.get", json!({})),
        ] {
            let response = call(&paths, method, params).unwrap();
            assert_eq!(response["ok"], true);
            assert_eq!(response["result"]["runtimeOwnership"], false);
        }
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn errors_are_stable_and_do_not_echo_private_input() {
        let request = make_request("safe", "private.example/password", json!({})).unwrap();
        let response = dispatch(&request, "instance", 0).unwrap();
        let rendered = serde_json::to_string(&response).unwrap();
        assert_eq!(response["error"]["code"], "unknown_method");
        assert!(!rendered.contains("private.example"));
        assert!(!rendered.contains("password"));
    }

    #[test]
    fn malformed_client_does_not_terminate_runtime() {
        use std::io::{Read, Write};

        let base = temporary_base("malformed");
        let paths = RuntimePaths::below(&base);
        let server = RuntimeServer::bind(paths.clone()).unwrap();
        let worker = thread::spawn(move || server.serve(Some(2)).unwrap());

        let mut invalid = UnixStream::connect(&paths.socket).unwrap();
        invalid.write_all(b"private.example/password\n").unwrap();
        invalid.shutdown(std::net::Shutdown::Write).unwrap();
        let mut safe_error = String::new();
        invalid.read_to_string(&mut safe_error).unwrap();
        assert!(safe_error.contains("invalid_request"));
        assert!(!safe_error.contains("private.example"));
        assert!(!safe_error.contains("password"));

        let response = call(&paths, "status.get", json!({})).unwrap();
        assert_eq!(response["ok"], true);
        worker.join().unwrap();
        fs::remove_dir_all(base).unwrap();
    }
}
