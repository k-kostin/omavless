// SPDX-License-Identifier: MIT
//! Inactive rule-provider work adapter. No IPC, scheduler, state write or owner.
//! Callers must fence current native ownership before each step and again before
//! stamping the store. Successful PUTs are remote effects and cannot be rolled
//! back after cancellation, stale ownership, timeout or another provider failure.

use crate::remote_fetch::RemoteFetchPool;
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use omavless_mihomo::rule_provider::{RuleProviderTarget, refresh_targets};
use omavless_mihomo::{MAX_CONTROLLER_RESPONSE_BYTES, parse_controller_response};
use std::fs;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub const PROVIDER_UPDATE_TIMEOUT: Duration = Duration::from_secs(60);
pub const PROVIDER_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);
pub const PROVIDER_REFRESH_DEADLINE: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRefreshError {
    Unavailable,
    Rejected,
    NoRemoteProviders,
    Cancelled,
    Deadline,
    InvalidState,
}

impl std::fmt::Display for ProviderRefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Private rule updates are unavailable",
            Self::Rejected => "Mihomo rejected rule provider work",
            Self::NoRemoteProviders => "The active configuration has no remote rule providers",
            Self::Cancelled => "Rule provider work was cancelled",
            Self::Deadline => "Rule provider work exceeded its deadline",
            Self::InvalidState => "Rule provider work state is invalid",
        })
    }
}
impl std::error::Error for ProviderRefreshError {}

#[derive(Clone, Default)]
pub struct ProviderRefreshCancellation(Arc<AtomicBool>);
impl ProviderRefreshCancellation {
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Trusted internal transport, never deserialized from a semantic request.
pub trait RuleProviderTransport {
    fn discover(&self, budget: Duration) -> Result<Vec<RuleProviderTarget>, ProviderRefreshError>;
    fn update(
        &self,
        target: &RuleProviderTarget,
        budget: Duration,
    ) -> Result<(), ProviderRefreshError>;
}

/// Runtime-generated fixed native socket only. Contains no TCP/URL transport.
pub struct UnixRuleProviderTransport {
    directory: PathBuf,
    uid: u32,
    identity: Mutex<Option<(u64, u64, i32)>>,
}
impl UnixRuleProviderTransport {
    #[must_use]
    pub fn new(directory: &Path, uid: u32) -> Self {
        Self {
            directory: directory.into(),
            uid,
            identity: Mutex::new(None),
        }
    }

    fn connect_pinned(&self, require_existing: bool) -> Result<UnixStream, ProviderRefreshError> {
        let unavailable = ProviderRefreshError::Unavailable;
        let path = self.directory.join("mihomo.sock");
        let directory = fs::symlink_metadata(&self.directory).map_err(|_| unavailable)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| unavailable)?;
        if !directory.is_dir()
            || directory.file_type().is_symlink()
            || directory.uid() != self.uid
            || directory.mode() & 0o7777 != 0o700
            || !metadata.file_type().is_socket()
            || metadata.uid() != self.uid
            || metadata.mode() & 0o7777 != 0o600
        {
            return Err(unavailable);
        }
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
            None,
        )
        .map_err(|_| unavailable)?;
        connect(
            fd.as_raw_fd(),
            &UnixAddr::new(&path).map_err(|_| unavailable)?,
        )
        .map_err(|_| unavailable)?;
        let stream = UnixStream::from(fd);
        let peer = getsockopt(&stream, PeerCredentials).map_err(|_| unavailable)?;
        let after = fs::symlink_metadata(&path).map_err(|_| unavailable)?;
        if peer.uid() != self.uid || after.dev() != metadata.dev() || after.ino() != metadata.ino()
        {
            return Err(unavailable);
        }
        let observed = (metadata.dev(), metadata.ino(), peer.pid());
        let mut pinned = self.identity.lock().map_err(|_| unavailable)?;
        if pinned.is_some_and(|expected| expected != observed) {
            return Err(unavailable);
        }
        if require_existing && pinned.is_none() {
            return Err(unavailable);
        }
        *pinned = Some(observed);
        drop(pinned);
        Ok(stream)
    }

    /// Short nonblocking socket/peer proof only, no controller request or read.
    /// Safe to call under the final serialized stamp lease.
    pub(crate) fn verify_identity(&self) -> Result<(), ProviderRefreshError> {
        self.connect_pinned(true).map(drop)
    }

    fn exchange(
        &self,
        target: Option<&RuleProviderTarget>,
        budget: Duration,
    ) -> Result<serde_json::Value, ProviderRefreshError> {
        let unavailable = ProviderRefreshError::Unavailable;
        if budget.is_zero() || budget > PROVIDER_UPDATE_TIMEOUT {
            return Err(ProviderRefreshError::Deadline);
        }
        let deadline = Instant::now() + budget;
        let remaining = || {
            deadline
                .checked_duration_since(Instant::now())
                .filter(|d| !d.is_zero())
                .ok_or(ProviderRefreshError::Deadline)
        };
        let mut stream = self.connect_pinned(target.is_some())?;
        stream.set_nonblocking(false).map_err(|_| unavailable)?;
        let (method, endpoint) = match target {
            Some(target) => ("PUT", target.update_path()),
            None => ("GET", String::from("/providers/rules")),
        };
        let request = format!(
            "{method} {endpoint} HTTP/1.0\r\nHost: localhost\r\nAccept: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let mut pending = request.as_bytes();
        while !pending.is_empty() {
            stream
                .set_write_timeout(Some(remaining()?))
                .map_err(|_| unavailable)?;
            let written = stream.write(pending).map_err(|_| unavailable)?;
            if written == 0 {
                return Err(unavailable);
            }
            pending = &pending[written..];
        }
        let cap = if target.is_some() {
            32 * 1024
        } else {
            MAX_CONTROLLER_RESPONSE_BYTES
        };
        let mut response = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            stream
                .set_read_timeout(Some(remaining()?))
                .map_err(|_| unavailable)?;
            let count = stream
                .read(&mut chunk)
                .map_err(|_| ProviderRefreshError::Rejected)?;
            if count == 0 {
                break;
            }
            if response.len().saturating_add(count) > cap {
                return Err(ProviderRefreshError::Rejected);
            }
            response.extend_from_slice(&chunk[..count]);
        }
        remaining()?;
        let response =
            parse_controller_response(&response).map_err(|_| ProviderRefreshError::Rejected)?;
        if response.status != if target.is_some() { 204 } else { 200 } {
            return Err(ProviderRefreshError::Rejected);
        }
        Ok(response.payload)
    }
}
impl RuleProviderTransport for UnixRuleProviderTransport {
    fn discover(&self, budget: Duration) -> Result<Vec<RuleProviderTarget>, ProviderRefreshError> {
        refresh_targets(&self.exchange(None, budget.min(PROVIDER_DISCOVERY_TIMEOUT))?)
            .map_err(|_| ProviderRefreshError::Rejected)
    }
    fn update(
        &self,
        target: &RuleProviderTarget,
        budget: Duration,
    ) -> Result<(), ProviderRefreshError> {
        self.exchange(Some(target), budget).map(|_| ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRefreshStep {
    Busy,
    Advanced,
    Ready,
}

/// Incremental bounded work, not a second operation registry. All public state
/// is counts only. Private targets cannot be constructed from a client string.
pub struct ProviderRefreshWork {
    targets: Option<Vec<RuleProviderTarget>>,
    completed: usize,
    failed: usize,
    terminal: bool,
    deadline: Instant,
    cancellation: ProviderRefreshCancellation,
}
impl ProviderRefreshWork {
    pub(crate) fn discovered(
        targets: Vec<RuleProviderTarget>,
        cancellation: ProviderRefreshCancellation,
    ) -> Self {
        Self {
            targets: Some(targets),
            ..Self::new(cancellation)
        }
    }
    #[must_use]
    pub fn new(cancellation: ProviderRefreshCancellation) -> Self {
        Self {
            targets: None,
            completed: 0,
            failed: 0,
            terminal: false,
            deadline: Instant::now() + PROVIDER_REFRESH_DEADLINE,
            cancellation,
        }
    }
    #[must_use]
    pub fn progress(&self) -> (usize, usize, usize) {
        (
            self.completed,
            self.targets.as_ref().map_or(0, Vec::len),
            self.failed,
        )
    }
    fn budget(&self) -> Result<Duration, ProviderRefreshError> {
        if self.cancellation.0.load(Ordering::Acquire) {
            return Err(ProviderRefreshError::Cancelled);
        }
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(ProviderRefreshError::Deadline)
    }
    pub fn step<T: RuleProviderTransport>(
        &mut self,
        transport: &T,
        pool: &RemoteFetchPool,
    ) -> Result<ProviderRefreshStep, ProviderRefreshError> {
        if self.terminal {
            return Err(ProviderRefreshError::InvalidState);
        }
        let result = self.step_inner(transport, pool);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }
    fn step_inner<T: RuleProviderTransport>(
        &mut self,
        transport: &T,
        pool: &RemoteFetchPool,
    ) -> Result<ProviderRefreshStep, ProviderRefreshError> {
        self.budget()?;
        let Some(_permit) = pool.try_acquire() else {
            return Ok(ProviderRefreshStep::Busy);
        };
        if self.targets.is_none() {
            let found = transport.discover(self.budget()?.min(PROVIDER_DISCOVERY_TIMEOUT));
            self.budget()?;
            let targets = found?;
            if targets.is_empty() {
                return Err(ProviderRefreshError::NoRemoteProviders);
            }
            self.targets = Some(targets);
            return Ok(ProviderRefreshStep::Advanced);
        }
        let targets = self
            .targets
            .as_ref()
            .ok_or(ProviderRefreshError::InvalidState)?;
        if self.completed == targets.len() {
            return if self.failed == 0 {
                Ok(ProviderRefreshStep::Ready)
            } else {
                Err(ProviderRefreshError::Rejected)
            };
        }
        let result = transport.update(
            &targets[self.completed],
            self.budget()?.min(PROVIDER_UPDATE_TIMEOUT),
        );
        self.budget()?;
        self.completed += 1;
        self.failed += usize::from(result.is_err());
        if self.completed == targets.len() {
            if self.failed == 0 {
                Ok(ProviderRefreshStep::Ready)
            } else {
                Err(ProviderRefreshError::Rejected)
            }
        } else {
            Ok(ProviderRefreshStep::Advanced)
        }
    }
    /// Returns only all-success count, not authority to stamp rulesUpdatedAt.
    /// The one owner still must close cancellation and revalidate its revision,
    /// generation, desired state, active config and store before atomic commit.
    pub fn finish(self) -> Result<usize, ProviderRefreshError> {
        self.budget()?;
        let total = self.targets.as_ref().map_or(0, Vec::len);
        if self.terminal || total == 0 || self.completed != total || self.failed != 0 {
            return Err(ProviderRefreshError::InvalidState);
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::thread;

    struct Fake {
        calls: Cell<usize>,
        fail: Option<usize>,
        cancel: Option<ProviderRefreshCancellation>,
        budgets: RefCell<Vec<Duration>>,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                calls: Cell::new(0),
                fail: None,
                cancel: None,
                budgets: RefCell::new(Vec::new()),
            }
        }
    }
    impl RuleProviderTransport for Fake {
        fn discover(
            &self,
            budget: Duration,
        ) -> Result<Vec<RuleProviderTarget>, ProviderRefreshError> {
            assert!(budget <= PROVIDER_DISCOVERY_TIMEOUT);
            refresh_targets(&json!({"providers":{"first":{"vehicleType":"HTTP"},"second":{"vehicleType":"http"}}})).map_err(|_| ProviderRefreshError::Rejected)
        }
        fn update(
            &self,
            _: &RuleProviderTarget,
            budget: Duration,
        ) -> Result<(), ProviderRefreshError> {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            self.budgets.borrow_mut().push(budget);
            if let Some(cancel) = &self.cancel {
                cancel.request();
            }
            if self.fail == Some(call) {
                Err(ProviderRefreshError::Rejected)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn sequential_updates_share_permits_and_only_complete_success_finishes() {
        let pool = RemoteFetchPool::default();
        let mut permits: Vec<_> = (0..4).map(|_| pool.try_acquire().unwrap()).collect();
        let transport = Fake::default();
        let mut work = ProviderRefreshWork::new(ProviderRefreshCancellation::default());
        assert_eq!(work.step(&transport, &pool), Ok(ProviderRefreshStep::Busy));
        assert_eq!(work.progress(), (0, 0, 0));
        permits.pop();
        assert_eq!(
            work.step(&transport, &pool),
            Ok(ProviderRefreshStep::Advanced)
        );
        assert_eq!(work.progress(), (0, 2, 0));
        assert_eq!(
            work.step(&transport, &pool),
            Ok(ProviderRefreshStep::Advanced)
        );
        assert_eq!(work.step(&transport, &pool), Ok(ProviderRefreshStep::Ready));
        assert_eq!(work.step(&transport, &pool), Ok(ProviderRefreshStep::Ready));
        assert_eq!(transport.calls.get(), 2);
        assert!(
            transport
                .budgets
                .borrow()
                .iter()
                .all(|d| *d <= PROVIDER_UPDATE_TIMEOUT)
        );
        assert_eq!(work.finish(), Ok(2));
    }

    #[test]
    fn partial_failure_attempts_remaining_targets_but_never_authorizes_stamp() {
        let pool = RemoteFetchPool::default();
        let transport = Fake {
            fail: Some(1),
            ..Fake::default()
        };
        let mut work = ProviderRefreshWork::new(ProviderRefreshCancellation::default());
        work.step(&transport, &pool).unwrap();
        assert_eq!(
            work.step(&transport, &pool),
            Ok(ProviderRefreshStep::Advanced)
        );
        assert_eq!(
            work.step(&transport, &pool),
            Err(ProviderRefreshError::Rejected)
        );
        assert_eq!(work.progress(), (2, 2, 1));
        assert_eq!(
            work.step(&transport, &pool),
            Err(ProviderRefreshError::InvalidState)
        );
        assert!(work.finish().is_err());
        assert_eq!(transport.calls.get(), 2);
        assert!(pool.try_acquire().is_some());
    }

    #[test]
    fn actual_python_refresh_all_success_and_partial_effect_semantics_match() {
        use std::process::{Command, Stdio};
        let cases = vec![vec![204, 204], vec![500, 204], vec![204, 500]];
        let mut child = Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/rule_provider_refresh_parity.py"
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&cases).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "synthetic refresh oracle failed");
        let expected: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(expected.len(), cases.len());
        for (index, (statuses, expected)) in cases.iter().zip(expected).enumerate() {
            let transport = Fake {
                fail: statuses
                    .iter()
                    .position(|status| *status != 204)
                    .map(|index| index + 1),
                ..Fake::default()
            };
            let pool = RemoteFetchPool::default();
            let mut work = ProviderRefreshWork::new(ProviderRefreshCancellation::default());
            for _ in 0..3 {
                let _ = work.step(&transport, &pool);
            }
            let finished = work.finish();
            let actual = json!({"ok":finished.is_ok(), "updated":finished.unwrap_or(0),"attempted":transport.calls.get(),"stamp":finished.is_ok()});
            assert!(
                actual == expected,
                "refresh parity mismatch at public case {index}"
            );
        }
    }

    #[test]
    fn cancellation_wins_over_inflight_failure_and_prevents_future_effects() {
        let pool = RemoteFetchPool::default();
        let cancel = ProviderRefreshCancellation::default();
        let mut work = ProviderRefreshWork::new(cancel.clone());
        let transport = Fake {
            fail: Some(1),
            cancel: Some(cancel),
            ..Fake::default()
        };
        work.step(&transport, &pool).unwrap();
        assert_eq!(
            work.step(&transport, &pool),
            Err(ProviderRefreshError::Cancelled)
        );
        assert_eq!(transport.calls.get(), 1);
        assert!(work.finish().is_err());
        let cancel = ProviderRefreshCancellation::default();
        cancel.request();
        let mut work = ProviderRefreshWork::new(cancel);
        assert_eq!(
            work.step(&transport, &pool),
            Err(ProviderRefreshError::Cancelled)
        );
        assert_eq!(transport.calls.get(), 1);
    }

    #[test]
    fn late_cancel_and_deadline_cannot_publish_success() {
        let pool = RemoteFetchPool::default();
        let transport = Fake::default();
        let cancel = ProviderRefreshCancellation::default();
        let mut work = ProviderRefreshWork::new(cancel.clone());
        for _ in 0..3 {
            work.step(&transport, &pool).unwrap();
        }
        cancel.request();
        assert_eq!(work.finish(), Err(ProviderRefreshError::Cancelled));
        let mut work = ProviderRefreshWork::new(ProviderRefreshCancellation::default());
        work.deadline = Instant::now();
        assert_eq!(
            work.step(&transport, &pool),
            Err(ProviderRefreshError::Deadline)
        );
    }

    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ov-refresh-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    fn read_request(stream: &mut UnixStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            assert!(request.len() < 4096);
        }
        request
    }
    #[test]
    fn replaced_controller_receives_no_put_from_old_discovered_job() {
        let directory = directory();
        let path = directory.join("mihomo.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let transport =
            UnixRuleProviderTransport::new(&directory, nix::unistd::Uid::current().as_raw());
        let first = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n{\"providers\":{\"synthetic\":{\"vehicleType\":\"HTTP\"}}}").unwrap();
        });
        let targets = transport.discover(Duration::from_secs(1)).unwrap();
        first.join().unwrap();
        // Retain the first inode at another pathname so reuse cannot obscure
        // this adversarial replacement test. The new listener is the successor.
        fs::rename(&path, directory.join("old.sock")).unwrap();
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let second = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte).unwrap(),
                0,
                "old job wrote to successor controller"
            );
        });
        assert_eq!(
            transport.update(&targets[0], Duration::from_secs(1)),
            Err(ProviderRefreshError::Unavailable)
        );
        second.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn private_unix_discovery_and_fixed_put_reject_modes_status_and_slow_body() {
        let directory = directory();
        let path = directory.join("mihomo.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        let transport = UnixRuleProviderTransport::new(&directory, uid);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(transport.discover(Duration::from_secs(1)).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            UnixRuleProviderTransport::new(&directory, uid + 1)
                .discover(Duration::from_secs(1))
                .is_err()
        );
        let worker = thread::spawn(move || {
            for index in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                if index == 0 {
                    assert!(request.starts_with(b"GET /providers/rules HTTP/1.0\r\n"));
                    stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n{\"providers\":{\"space name\":{\"vehicleType\":\"HTTP\"}}}").unwrap();
                } else {
                    assert!(request.starts_with(b"PUT /providers/rules/space%20name HTTP/1.0\r\n"));
                    if index == 1 {
                        stream
                            .write_all(b"HTTP/1.0 204 No Content\r\nContent-Length: 0\r\n\r\n")
                            .unwrap();
                    } else if index == 2 {
                        stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n{}").unwrap();
                    } else {
                        for _ in 0..20 {
                            if stream.write_all(b"x").is_err() {
                                break;
                            }
                            thread::sleep(Duration::from_millis(20));
                        }
                    }
                }
            }
        });
        let targets = transport.discover(Duration::from_secs(1)).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(
            transport.update(&targets[0], Duration::from_secs(1)),
            Ok(())
        );
        assert_eq!(
            transport.update(&targets[0], Duration::from_secs(1)),
            Err(ProviderRefreshError::Rejected)
        );
        let started = Instant::now();
        assert!(
            transport
                .update(&targets[0], Duration::from_millis(60))
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_millis(300));
        worker.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn installed_mihomo_refreshes_synthetic_loopback_provider_without_tun_or_tcp_controller() {
        let Some(core) = std::env::var_os("OMAVLESS_TEST_MIHOMO").map(PathBuf::from) else {
            return;
        };
        let directory = directory();
        let socket = directory.join("mihomo.sock");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let changed = Arc::new(AtomicBool::new(false));
        let quit = Arc::new(AtomicBool::new(false));
        let server_changed = changed.clone();
        let server_quit = quit.clone();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !server_quit.load(Ordering::Acquire) && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    request.push(byte[0]);
                    assert!(request.len() < 8192);
                }
                assert!(request.starts_with(b"GET /rules.yaml HTTP/1.1\r\n"));
                let body = if server_changed.load(Ordering::Acquire) {
                    "payload:\n  - first.example.invalid\n  - second.example.invalid\n"
                } else {
                    "payload:\n  - first.example.invalid\n"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let config = directory.join("config.yaml");
        fs::write(&config, format!("mixed-port: 0\nport: 0\nsocks-port: 0\nallow-lan: false\nmode: rule\nlog-level: silent\nexternal-controller-unix: {}\ntun:\n  enable: false\ndns:\n  enable: false\nproxies: []\nproxy-groups: []\nrule-providers:\n  synthetic:\n    type: http\n    behavior: domain\n    format: yaml\n    url: http://127.0.0.1:{port}/rules.yaml\n    path: ./synthetic.yaml\n    interval: 86400\nrules:\n  - RULE-SET,synthetic,DIRECT\n  - MATCH,DIRECT\n", socket.display())).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        omavless_mihomo::validate_config(&core, &directory, &config, Duration::from_secs(10))
            .unwrap();
        let copy = directory.join("mihomo");
        fs::copy(&core, &copy).unwrap();
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(fs::read(&copy).unwrap() == fs::read(&core).unwrap());
        let mut owned = crate::core::OwnedCore::spawn(&copy, &directory, &config, &socket).unwrap();
        owned.wait_ready(Duration::from_secs(10)).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let transport =
            UnixRuleProviderTransport::new(&directory, nix::unistd::Uid::current().as_raw());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let payload = transport.exchange(None, Duration::from_secs(1)).unwrap();
            if payload["providers"]["synthetic"]["ruleCount"] == 1 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "synthetic provider did not initialize"
            );
            thread::sleep(Duration::from_millis(20));
        }
        changed.store(true, Ordering::Release);
        let mut work = ProviderRefreshWork::new(ProviderRefreshCancellation::default());
        let pool = RemoteFetchPool::default();
        assert_eq!(
            work.step(&transport, &pool),
            Ok(ProviderRefreshStep::Advanced)
        );
        assert_eq!(work.step(&transport, &pool), Ok(ProviderRefreshStep::Ready));
        assert_eq!(work.finish(), Ok(1));
        let payload = transport.exchange(None, Duration::from_secs(1)).unwrap();
        assert_eq!(payload["providers"]["synthetic"]["ruleCount"], 2);
        let targets: Vec<String> = fs::read_dir(format!("/proc/{}/fd", owned.pid().unwrap()))
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read_link(entry.path()).ok())
            .filter_map(|path| path.to_str().map(str::to_owned))
            .collect();
        assert!(!targets.iter().any(|path| path == "/dev/net/tun"));
        let sockets: Vec<_> = targets
            .iter()
            .filter_map(|target| {
                target
                    .strip_prefix("socket:[")
                    .and_then(|value| value.strip_suffix(']'))
            })
            .collect();
        for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
            for line in fs::read_to_string(table).unwrap().lines().skip(1) {
                let fields: Vec<_> = line.split_whitespace().collect();
                assert!(
                    !(fields.get(3) == Some(&"0A")
                        && fields.get(9).is_some_and(|inode| sockets.contains(inode))),
                    "isolated core unexpectedly owns TCP listener"
                );
            }
        }
        owned.stop(Duration::from_secs(5)).unwrap();
        assert!(owned.pid().is_none());
        quit.store(true, Ordering::Release);
        server.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }
}
