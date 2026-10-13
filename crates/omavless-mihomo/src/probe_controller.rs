// SPDX-License-Identifier: MIT
//! Fixed isolated-probe controller requests. No process, resolver or VPN owner.
//! The caller must retain its supervised auxiliary-child lease throughout I/O.
//! The lease supplies the still-waitable PID; socket metadata alone is not
//! process-lifetime proof. Startup normalizes the authenticated socket to 0600
//! before constructing this adapter. No secret, URL or arbitrary path request
//! can be supplied to the HTTP boundary.

use crate::{
    ControllerResponse, parse_controller_response,
    probe_plan::{PROBE_URLS, ProbeChunk},
};
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use std::fs::{self, Metadata};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const MAX_PROBE_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_ROUND_TIME: Duration = Duration::from_secs(10);
const IO_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeControllerError {
    InvalidInput,
    Unavailable,
    Cancelled,
    TimedOut,
    InvalidResponse,
    ResponseTooLarge,
}

impl std::fmt::Display for ProbeControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Probe controller input is invalid",
            Self::Unavailable => "Private probe controller is unavailable",
            Self::Cancelled => "Probe was cancelled",
            Self::TimedOut => "Probe controller request timed out",
            Self::InvalidResponse => "Probe controller response is invalid",
            Self::ResponseTooLarge => "Probe controller response is too large",
        })
    }
}
impl std::error::Error for ProbeControllerError {}
type Result<T> = std::result::Result<T, ProbeControllerError>;

/// Private local paths are intentionally not Debug/Serialize.
pub struct ProbeController {
    path: PathBuf,
    pid: u32,
    uid: u32,
    parent_identity: (u64, u64),
    socket_identity: (u64, u64),
}

fn identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

impl ProbeController {
    pub fn new(path: &Path, pid: u32, uid: u32) -> Result<Self> {
        if !path.is_absolute() || path.as_os_str().len() > 4096 || pid == 0 {
            return Err(ProbeControllerError::InvalidInput);
        }
        let parent = fs::symlink_metadata(path.parent().ok_or(ProbeControllerError::InvalidInput)?)
            .map_err(|_| ProbeControllerError::Unavailable)?;
        let socket = fs::symlink_metadata(path).map_err(|_| ProbeControllerError::Unavailable)?;
        let result = Self {
            path: path.to_owned(),
            pid,
            uid,
            parent_identity: identity(&parent),
            socket_identity: identity(&socket),
        };
        result.revalidate()?;
        Ok(result)
    }

    fn revalidate(&self) -> Result<()> {
        let parent = fs::symlink_metadata(
            self.path
                .parent()
                .ok_or(ProbeControllerError::Unavailable)?,
        )
        .map_err(|_| ProbeControllerError::Unavailable)?;
        let socket =
            fs::symlink_metadata(&self.path).map_err(|_| ProbeControllerError::Unavailable)?;
        if !parent.is_dir()
            || parent.uid() != self.uid
            || parent.mode() & 0o7777 != 0o700
            || identity(&parent) != self.parent_identity
            || !socket.file_type().is_socket()
            || socket.uid() != self.uid
            || socket.mode() & 0o7777 != 0o600
            || identity(&socket) != self.socket_identity
        {
            return Err(ProbeControllerError::Unavailable);
        }
        Ok(())
    }

    /// Liveness only. The executor must separately check chunk/group readiness.
    pub fn version_ready(&self, budget: Duration, cancelled: impl Fn() -> bool) -> Result<bool> {
        self.exchange("/version", budget, &cancelled)
            .map(|response| response.has_live_version())
    }

    /// `/version` can answer before config loading. Require the exact planned
    /// group members, in config order, before collecting its delay responses.
    pub fn chunk_ready(
        &self,
        chunk: &ProbeChunk,
        budget: Duration,
        cancelled: impl Fn() -> bool,
    ) -> Result<bool> {
        let response = self.exchange("/proxies", budget, &cancelled)?;
        let group = &response.payload["proxies"]["OMAVLESS_TEST"];
        Ok(response.status == 200
            && group["type"].as_str() == Some("Selector")
            && group["all"].as_array().is_some_and(|members| {
                members
                    .iter()
                    .map(serde_json::Value::as_str)
                    .eq(chunk.aliases().map(Some))
            }))
    }

    /// One group request, not one request per member. An HTTP 504 is retained as
    /// an empty completed round by the existing plan collector, never erased by
    /// a generic 2xx-only HTTP adapter. Other statuses are likewise classified by
    /// that collector rather than turned into private raw backend error strings.
    pub fn round(
        &self,
        index: usize,
        budget: Duration,
        cancelled: impl Fn() -> bool,
    ) -> Result<ControllerResponse> {
        self.exchange(&round_path(index)?, budget, &cancelled)
    }

    fn exchange(
        &self,
        path: &str,
        budget: Duration,
        cancelled: &impl Fn() -> bool,
    ) -> Result<ControllerResponse> {
        if budget.is_zero() || budget > MAX_ROUND_TIME {
            return Err(ProbeControllerError::InvalidInput);
        }
        let deadline = Instant::now() + budget;
        let remaining = || {
            if cancelled() {
                return Err(ProbeControllerError::Cancelled);
            }
            deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(ProbeControllerError::TimedOut)
        };
        remaining()?;
        self.revalidate()?;
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            None,
        )
        .map_err(|_| ProbeControllerError::Unavailable)?;
        let address = UnixAddr::new(&self.path).map_err(|_| ProbeControllerError::InvalidInput)?;
        connect(fd.as_raw_fd(), &address).map_err(|_| ProbeControllerError::Unavailable)?;
        let peer =
            getsockopt(&fd, PeerCredentials).map_err(|_| ProbeControllerError::Unavailable)?;
        if peer.uid() != self.uid || u32::try_from(peer.pid()).ok() != Some(self.pid) {
            return Err(ProbeControllerError::Unavailable);
        }
        self.revalidate()?;
        let mut stream = UnixStream::from(fd);
        stream
            .set_nonblocking(false)
            .map_err(|_| ProbeControllerError::Unavailable)?;
        let request = format!(
            "GET {path} HTTP/1.0\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
        );
        let mut pending = request.as_bytes();
        while !pending.is_empty() {
            stream
                .set_write_timeout(Some(remaining()?.min(IO_SLICE)))
                .map_err(|_| ProbeControllerError::Unavailable)?;
            match stream.write(pending) {
                Ok(0) => return Err(ProbeControllerError::Unavailable),
                Ok(count) => pending = &pending[count..],
                Err(error) if retryable(&error) => continue,
                Err(_) => return Err(ProbeControllerError::Unavailable),
            }
        }
        let mut response = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            stream
                .set_read_timeout(Some(remaining()?.min(IO_SLICE)))
                .map_err(|_| ProbeControllerError::Unavailable)?;
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    if response.len().saturating_add(count) > MAX_PROBE_RESPONSE_BYTES {
                        return Err(ProbeControllerError::ResponseTooLarge);
                    }
                    response.extend_from_slice(&buffer[..count]);
                }
                Err(error) if retryable(&error) => continue,
                Err(_) => return Err(ProbeControllerError::Unavailable),
            }
        }
        remaining()?;
        self.revalidate()?;
        parse_controller_response(&response).map_err(|_| ProbeControllerError::InvalidResponse)
    }
}

fn retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
    )
}

fn round_path(index: usize) -> Result<String> {
    let url = PROBE_URLS
        .get(index)
        .ok_or(ProbeControllerError::InvalidInput)?;
    let mut encoded = String::new();
    for byte in url.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("String formatting cannot fail");
        }
    }
    Ok(format!(
        "/group/OMAVLESS_TEST/delay?url={encoded}&timeout=5000&expected=200-299"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;

    fn fixture() -> (PathBuf, UnixListener, ProbeController) {
        let root = crate::test_temp::directory_under(
            &crate::test_temp::home_parent().unwrap(),
            "probe-controller",
        )
        .unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("controller.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        let controller = ProbeController::new(&path, std::process::id(), uid).unwrap();
        (root, listener, controller)
    }
    fn request(stream: &mut UnixStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut byte = [0];
        while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 4096 {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                break;
            }
            bytes.push(byte[0]);
        }
        String::from_utf8(bytes).unwrap()
    }
    fn response(body: &[u8], status: u16) -> Vec<u8> {
        let mut bytes = format!(
            "HTTP/1.0 {status} synthetic\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn exactly_three_fixed_group_rounds_keep_504_and_version_separate() {
        let (root, listener, controller) = fixture();
        let worker = thread::spawn(move || {
            for index in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let path = if index == 0 {
                    "/version".to_owned()
                } else {
                    round_path(index - 1).unwrap()
                };
                assert!(request(&mut stream).starts_with(&format!("GET {path} HTTP/1.0\r\n")));
                let body = if index == 0 {
                    br#"{"version":"synthetic"}"#.as_slice()
                } else {
                    br#"{}"#.as_slice()
                };
                stream
                    .write_all(&response(body, if index == 3 { 504 } else { 200 }))
                    .unwrap();
            }
        });
        assert!(
            controller
                .version_ready(Duration::from_secs(1), || false)
                .unwrap()
        );
        for index in 0..3 {
            let result = controller
                .round(index, Duration::from_secs(1), || false)
                .unwrap();
            assert_eq!(result.status, if index == 2 { 504 } else { 200 });
        }
        assert!(
            controller
                .round(3, Duration::from_secs(1), || false)
                .is_err()
        );
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn configured_group_requires_exact_plan_not_just_version_liveness() {
        use crate::probe_plan::{PinnedProfile, ProbePlan};
        use omavless_profile::canonical::parse_canonical;
        use serde_json::json;
        let profile =
            parse_canonical("trojan://synthetic@example.invalid:443?sni=cdn.example.invalid")
                .unwrap();
        let plan = ProbePlan::new(&[PinnedProfile {
            profile: &profile,
            addresses: &["192.0.2.1".parse().unwrap()],
        }])
        .unwrap();
        let (root, listener, controller) = fixture();
        let worker = thread::spawn(move || {
            for members in [
                json!(null),
                json!([]),
                json!(["p0000a0", "extra"]),
                json!(["p0000a0"]),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                assert!(request(&mut stream).starts_with("GET /proxies HTTP/1.0\r\n"));
                stream
                    .write_all(&response(
                        &serde_json::to_vec(
                            &json!({"proxies":{"OMAVLESS_TEST":{"type":"Selector","all":members}}}),
                        )
                        .unwrap(),
                        200,
                    ))
                    .unwrap();
            }
        });
        for expected in [false, false, false, true] {
            assert_eq!(
                controller
                    .chunk_ready(&plan.chunks()[0], Duration::from_secs(1), || false)
                    .unwrap(),
                expected
            );
        }
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn path_replacement_during_io_discards_successful_response() {
        let (root, listener, controller) = fixture();
        let path = controller.path.clone();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            request(&mut stream);
            fs::remove_file(&path).unwrap();
            let _replacement = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            stream.write_all(&response(br#"{}"#, 200)).unwrap();
        });
        assert_eq!(
            controller
                .round(0, Duration::from_secs(1), || false)
                .unwrap_err(),
            ProbeControllerError::Unavailable
        );
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_deadlines_and_uid_are_refused_without_io() {
        let (root, _listener, controller) = fixture();
        for duration in [Duration::ZERO, MAX_ROUND_TIME + Duration::from_millis(1)] {
            assert_eq!(
                controller.round(0, duration, || false).unwrap_err(),
                ProbeControllerError::InvalidInput
            );
        }
        assert!(
            ProbeController::new(
                &controller.path,
                controller.pid,
                controller.uid.wrapping_add(1)
            )
            .is_err()
        );
        assert!(ProbeController::new(&controller.path, 0, controller.uid).is_err());
        assert!(
            ProbeController::new(Path::new("relative.sock"), controller.pid, controller.uid)
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incorrect_peer_gets_no_request_bytes() {
        let (root, listener, mut controller) = fixture();
        controller.pid = controller.pid.saturating_add(1);
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            assert!(request(&mut stream).is_empty());
        });
        assert_eq!(
            controller
                .round(0, Duration::from_secs(1), || false)
                .unwrap_err(),
            ProbeControllerError::Unavailable
        );
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_modes_symlinks_and_replacement_are_refused() {
        let (root, _listener, controller) = fixture();
        fs::set_permissions(&controller.path, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(controller.revalidate().is_err());
        fs::set_permissions(&controller.path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(controller.revalidate().is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let link = root.join("link");
        symlink(&controller.path, &link).unwrap();
        assert!(ProbeController::new(&link, controller.pid, controller.uid).is_err());
        fs::remove_file(&controller.path).unwrap();
        let _replacement = UnixListener::bind(&controller.path).unwrap();
        fs::set_permissions(&controller.path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(controller.revalidate().is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_before_connect_and_during_silent_response_is_bounded() {
        let (root, listener, controller) = fixture();
        assert_eq!(
            controller
                .round(0, Duration::from_secs(1), || true)
                .unwrap_err(),
            ProbeControllerError::Cancelled
        );
        let flag = Arc::new(AtomicBool::new(false));
        let worker_flag = Arc::clone(&flag);
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            assert!(!request(&mut stream).is_empty());
            worker_flag.store(true, Ordering::Release);
            thread::sleep(Duration::from_millis(200));
        });
        let start = Instant::now();
        assert_eq!(
            controller
                .round(0, Duration::from_secs(1), || flag.load(Ordering::Acquire))
                .unwrap_err(),
            ProbeControllerError::Cancelled
        );
        assert!(start.elapsed() < Duration::from_millis(180));
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trickle_cannot_extend_whole_deadline() {
        let (root, listener, controller) = fixture();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            request(&mut stream);
            for byte in response(br#"{}"#, 200) {
                if stream.write_all(&[byte]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let start = Instant::now();
        assert_eq!(
            controller
                .round(0, Duration::from_millis(90), || false)
                .unwrap_err(),
            ProbeControllerError::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(250));
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_and_malformed_private_response_errors_never_echo_content() {
        for (body, expected) in [
            (
                vec![b'x'; MAX_PROBE_RESPONSE_BYTES],
                ProbeControllerError::ResponseTooLarge,
            ),
            (
                b"private-password://synthetic-secret".to_vec(),
                ProbeControllerError::InvalidResponse,
            ),
        ] {
            let (root, listener, controller) = fixture();
            let worker = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                request(&mut stream);
                let _ = stream.write_all(&response(&body, 200));
            });
            let error = controller
                .round(0, Duration::from_secs(1), || false)
                .unwrap_err();
            assert_eq!(error, expected);
            assert!(!error.to_string().contains("secret"));
            assert!(!format!("{error:?}").contains("private-password"));
            worker.join().unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }
}
