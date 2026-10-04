// SPDX-License-Identifier: MIT
//! Detached fixed-purpose isolated probe execution. Admission, endpoint pinning,
//! job/revision fencing and publication belong to the enclosing runtime owner.
//! This module must never run while that owner's mutex is held.

use crate::auxiliary_core::{AuxiliaryError, AuxiliaryLease};
use nix::fcntl::{OFlag, open, openat};
use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr, connect, socket};
use nix::sys::stat::Mode;
use nix::unistd::Uid;
use nix::unistd::{UnlinkatFlags, unlinkat};
use omavless_mihomo::probe_controller::{MAX_ROUND_TIME, ProbeController, ProbeControllerError};
use omavless_mihomo::probe_plan::{PROBE_URLS, ProbeChunk, ProbePlan, ProbeResult};
use omavless_store::atomic_replace_private;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);
const READY_BUDGET: Duration = Duration::from_secs(10);
pub const MAX_JOB_TIME: Duration = Duration::from_secs(30 * 60);
const OWNER_MARKER: &str = ".omavless-probe-owner";
const OWNER_MAGIC: &str = "omavless-probe-scratch-v1";
const MAX_ORPHANS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeExecutionError {
    Invalid,
    Unavailable,
    Cancelled,
    TimedOut,
    Rejected,
    CleanupRequired,
}
impl std::fmt::Display for ProbeExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "Probe execution input is invalid",
            Self::Unavailable => "Probe execution is unavailable",
            Self::Cancelled => "Probe was cancelled",
            Self::TimedOut => "Probe execution timed out",
            Self::Rejected => "Probe could not complete the end-to-end check",
            Self::CleanupRequired => "Probe cleanup requires manual recovery",
        })
    }
}
impl std::error::Error for ProbeExecutionError {}
type Result<T> = std::result::Result<T, ProbeExecutionError>;

/// No paths, endpoint data or credentials enter progress callbacks. Each event
/// means one accepted group response; completion results remain positional until
/// the admission owner maps them back to its exact private snapshot.
pub fn execute_plan(
    plan: &ProbePlan,
    core: &Path,
    scratch_parent: &Path,
    lease: &AuxiliaryLease,
    deadline: Instant,
    cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<ProbeResult>> {
    let now = Instant::now();
    if deadline <= now || deadline.duration_since(now) > MAX_JOB_TIME {
        return Err(ProbeExecutionError::Invalid);
    }
    let cancelled = || lease.cancelled() || cancelled();
    let mut collector = plan.collector();
    for (index, chunk) in plan.chunks().iter().enumerate() {
        check(deadline, &cancelled)?;
        let mut run = ChunkRun::new(scratch_parent, chunk, lease)?;
        let outcome = (|| {
            check(deadline, &cancelled)?;
            lease
                .spawn(
                    core,
                    &run.scratch.path,
                    &run.scratch.config(),
                    &run.scratch.controller(),
                )
                .map_err(auxiliary_error)?;
            let controller = wait_ready(
                lease,
                &run.scratch.controller(),
                chunk,
                deadline,
                &cancelled,
            )?;
            for round in 0..PROBE_URLS.len() {
                check(deadline, &cancelled)?;
                if !lease.running().map_err(auxiliary_error)? {
                    return Err(ProbeExecutionError::Unavailable);
                }
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .min(MAX_ROUND_TIME);
                let response = controller
                    .round(round, remaining, cancelled)
                    .map_err(controller_error)?;
                check(deadline, &cancelled)?;
                if !lease.running().map_err(auxiliary_error)? {
                    return Err(ProbeExecutionError::Unavailable);
                }
                if collector
                    .record(index, round, response.status, &response.payload)
                    .map_err(|_| ProbeExecutionError::Rejected)?
                {
                    progress(index, round);
                }
            }
            Ok(())
        })();
        // Cleanup is checked even on cancellation, parse failure or rejected
        // configuration. Never publish success while the disposable core lives.
        run.stop()?;
        // Revocation may race with a failed connect/read. Cleanup remains the
        // highest-priority outcome, but a revoked job must not be misclassified
        // as a provider/controller failure merely because I/O failed first.
        if cancelled() {
            return Err(ProbeExecutionError::Cancelled);
        }
        outcome?;
    }
    check(deadline, &cancelled)?;
    collector
        .finish()
        .map_err(|_| ProbeExecutionError::Rejected)
}

fn auxiliary_error(error: AuxiliaryError) -> ProbeExecutionError {
    match error {
        AuxiliaryError::Cancelled => ProbeExecutionError::Cancelled,
        AuxiliaryError::Cleanup => ProbeExecutionError::CleanupRequired,
        AuxiliaryError::Busy | AuxiliaryError::Invalid => ProbeExecutionError::Unavailable,
    }
}
fn controller_error(error: ProbeControllerError) -> ProbeExecutionError {
    match error {
        ProbeControllerError::Cancelled => ProbeExecutionError::Cancelled,
        ProbeControllerError::TimedOut => ProbeExecutionError::TimedOut,
        ProbeControllerError::Unavailable => ProbeExecutionError::Unavailable,
        _ => ProbeExecutionError::Rejected,
    }
}
fn check(deadline: Instant, cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        return Err(ProbeExecutionError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ProbeExecutionError::TimedOut);
    }
    Ok(())
}

fn wait_ready(
    lease: &AuxiliaryLease,
    socket: &Path,
    chunk: &ProbeChunk,
    job_deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<ProbeController> {
    let deadline = job_deadline.min(Instant::now() + READY_BUDGET);
    let uid = Uid::current().as_raw();
    loop {
        check(deadline, cancelled)?;
        if !lease.running().map_err(auxiliary_error)? {
            return Err(ProbeExecutionError::Unavailable);
        }
        let pid = lease.pid().map_err(auxiliary_error)?;
        if crate::controller_permissions::secure_owned(socket, pid, uid)
            && let Ok(controller) = ProbeController::new(socket, pid, uid)
        {
            let budget = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(250));
            if controller
                .chunk_ready(chunk, budget, cancelled)
                .unwrap_or(false)
            {
                check(deadline, cancelled)?;
                return Ok(controller);
            }
        }
        std::thread::sleep(
            Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

struct Scratch {
    path: PathBuf,
    identity: (u64, u64),
    uid: u32,
}
impl Scratch {
    fn create(parent: &Path, config: &str) -> Result<Self> {
        let uid = Uid::current().as_raw();
        let parent_metadata =
            fs::symlink_metadata(parent).map_err(|_| ProbeExecutionError::Invalid)?;
        if !parent.is_absolute()
            || !parent_metadata.is_dir()
            || parent_metadata.uid() != uid
            || parent_metadata.mode() & 0o7777 != 0o700
        {
            return Err(ProbeExecutionError::Invalid);
        }
        for _ in 0..128 {
            let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("probe-{}-{sequence:x}", std::process::id()));
            if path.join("controller.sock").as_os_str().len() >= 108 {
                return Err(ProbeExecutionError::Invalid);
            }
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(ProbeExecutionError::Unavailable),
                Ok(()) => {}
            }
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| ProbeExecutionError::Unavailable)?;
            let scratch = Self {
                path,
                identity: (metadata.dev(), metadata.ino()),
                uid,
            };
            let marker = format!("{OWNER_MAGIC}\n{}\n", std::process::id());
            if atomic_replace_private(&scratch.path.join(OWNER_MARKER), marker.as_bytes(), uid)
                .is_err()
            {
                scratch.cleanup()?;
                return Err(ProbeExecutionError::Unavailable);
            }
            if atomic_replace_private(&scratch.config(), config.as_bytes(), uid).is_err() {
                scratch.cleanup()?;
                return Err(ProbeExecutionError::Unavailable);
            }
            return Ok(scratch);
        }
        Err(ProbeExecutionError::Unavailable)
    }
    fn config(&self) -> PathBuf {
        self.path.join("config.yaml")
    }
    fn controller(&self) -> PathBuf {
        self.path.join("controller.sock")
    }
    fn cleanup(&self) -> Result<()> {
        let metadata =
            fs::symlink_metadata(&self.path).map_err(|_| ProbeExecutionError::CleanupRequired)?;
        if !metadata.is_dir()
            || metadata.uid() != self.uid
            || metadata.mode() & 0o7777 != 0o700
            || (metadata.dev(), metadata.ino()) != self.identity
        {
            return Err(ProbeExecutionError::CleanupRequired);
        }
        fs::remove_dir_all(&self.path).map_err(|_| ProbeExecutionError::CleanupRequired)
    }
}

/// Startup-only bounded crash cleanup. The caller holds the canonical owner
/// lock and committed ownership/migration lease, before starting a successor
/// core. `no_live_core` must prove complete process/TUN absence, not a tolerant
/// diagnostic count. There is no name-only recursive deletion or PID adoption.
/// Unknown members, live/reused owner PIDs, responsive sockets or changed inodes
/// refuse cleanup. Nothing is read from a profile/store file.
pub fn cleanup_orphans(parent: &Path, no_live_core: impl Fn() -> bool) -> Result<usize> {
    let uid = Uid::current().as_raw();
    let failure = || ProbeExecutionError::CleanupRequired;
    if !parent.is_absolute() {
        return Err(failure());
    }
    let directory = File::from(
        open(
            parent,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| failure())?,
    );
    let parent_metadata = directory.metadata().map_err(|_| failure())?;
    if !private_directory_metadata(&parent_metadata, uid) {
        return Err(failure());
    }
    let mut candidates = Vec::new();
    for (index, entry) in fs::read_dir(fd_path(&directory))
        .map_err(|_| failure())?
        .enumerate()
    {
        if index >= 512 {
            return Err(failure());
        }
        let entry = entry.map_err(|_| failure())?;
        let name = entry.file_name();
        if !name.as_encoded_bytes().starts_with(b"probe-") {
            continue;
        }
        let pid = orphan_name_pid(&name).ok_or_else(failure)?;
        if candidates.len() >= MAX_ORPHANS {
            return Err(failure());
        }
        candidates.push((name, pid));
    }
    if candidates.is_empty() {
        return Ok(0);
    }
    if !no_live_core() {
        return Err(failure());
    }
    // Validate the complete set first. One unsafe candidate prevents deletion
    // of every other candidate; a safe prefix is not partial acceptance.
    let mut prepared = Vec::new();
    for (name, pid) in candidates {
        match fs::symlink_metadata(format!("/proc/{pid}")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(failure()),
        }
        let held = File::from(
            openat(
                &directory,
                Path::new(&name),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| failure())?,
        );
        let metadata = held.metadata().map_err(|_| failure())?;
        if !private_directory_metadata(&metadata, uid) {
            return Err(failure());
        }
        let mut members = Vec::new();
        let mut marker_present = false;
        for (index, entry) in fs::read_dir(fd_path(&held))
            .map_err(|_| failure())?
            .enumerate()
        {
            if index >= 4 {
                return Err(failure());
            }
            let entry = entry.map_err(|_| failure())?;
            let member = entry.file_name();
            let file = File::from(
                openat(
                    &held,
                    Path::new(&member),
                    OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| failure())?,
            );
            let facts = file.metadata().map_err(|_| failure())?;
            let regular = facts.is_file() && facts.nlink() == 1 && facts.uid() == uid;
            let valid = match member.to_str() {
                Some(OWNER_MARKER) => {
                    marker_present = true;
                    if !regular || facts.mode() & 0o7777 != 0o600 || facts.len() > 128 {
                        return Err(failure());
                    }
                    let input = File::from(
                        openat(
                            &held,
                            Path::new(OWNER_MARKER),
                            OFlag::O_RDONLY
                                | OFlag::O_NONBLOCK
                                | OFlag::O_NOFOLLOW
                                | OFlag::O_CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(|_| failure())?,
                    );
                    if !input.metadata().is_ok_and(|now| same_file(&now, &facts)) {
                        return Err(failure());
                    }
                    let mut bytes = Vec::new();
                    input
                        .take(129)
                        .read_to_end(&mut bytes)
                        .map_err(|_| failure())?;
                    regular
                        && facts.mode() & 0o7777 == 0o600
                        && facts.len() <= 128
                        && bytes == format!("{OWNER_MAGIC}\n{pid}\n").as_bytes()
                }
                Some("config.yaml") => {
                    regular
                        && facts.mode() & 0o7777 == 0o600
                        && facts.len() <= omavless_mihomo::probe_plan::MAX_CONFIG_BYTES as u64
                }
                Some("cache.db") => {
                    regular
                        && matches!(facts.mode() & 0o7777, 0o600 | 0o644)
                        && facts.len() <= 32 * 1024 * 1024
                }
                Some("controller.sock") => {
                    if !facts.file_type().is_socket()
                        || facts.uid() != uid
                        || !matches!(facts.mode() & 0o7777, 0o600 | 0o666)
                    {
                        return Err(failure());
                    }
                    let descriptor = socket(
                        AddressFamily::Unix,
                        SockType::Stream,
                        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
                        None,
                    )
                    .map_err(|_| failure())?;
                    let address = UnixAddr::new(&fd_path(&held).join("controller.sock"))
                        .map_err(|_| failure())?;
                    matches!(
                        connect(descriptor.as_raw_fd(), &address),
                        Err(nix::errno::Errno::ECONNREFUSED)
                    )
                }
                _ => false,
            };
            if !valid {
                return Err(failure());
            }
            members.push((member, facts));
        }
        if !marker_present {
            return Err(failure());
        }
        // Keep ownership proof until credential-bearing members are gone. A
        // crash during cleanup can then retry the remaining fixed members.
        members.sort_by_key(|(name, _)| name == OWNER_MARKER);
        prepared.push((name, held, metadata, members));
    }
    if !no_live_core()
        || !fs::symlink_metadata(parent).is_ok_and(|now| same_file(&now, &parent_metadata))
    {
        return Err(failure());
    }
    // First revalidate every member, then unlink only fixed names via held
    // directory descriptors. A replaced directory entry cannot redirect writes.
    for (name, held, metadata, members) in &prepared {
        let current = File::from(
            openat(
                &directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| failure())?,
        );
        if !current
            .metadata()
            .is_ok_and(|now| same_file(&now, metadata))
        {
            return Err(failure());
        }
        for (member, expected) in members {
            let current = File::from(
                openat(
                    held,
                    Path::new(member),
                    OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| failure())?,
            );
            if !current
                .metadata()
                .is_ok_and(|now| same_file(&now, expected))
            {
                return Err(failure());
            }
        }
    }
    let count = prepared.len();
    for (name, held, metadata, members) in prepared {
        for (member, expected) in members {
            let current = File::from(
                openat(
                    &held,
                    Path::new(&member),
                    OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| failure())?,
            );
            if !current
                .metadata()
                .is_ok_and(|now| same_file(&now, &expected))
            {
                return Err(failure());
            }
            unlinkat(&held, Path::new(&member), UnlinkatFlags::NoRemoveDir)
                .map_err(|_| failure())?;
        }
        let current = File::from(
            openat(
                &directory,
                Path::new(&name),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| failure())?,
        );
        if !current
            .metadata()
            .is_ok_and(|now| same_file(&now, &metadata))
        {
            return Err(failure());
        }
        unlinkat(&directory, Path::new(&name), UnlinkatFlags::RemoveDir).map_err(|_| failure())?;
    }
    Ok(count)
}

fn fd_path(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
fn private_directory_metadata(metadata: &Metadata, uid: u32) -> bool {
    metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o700
}
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    (a.dev(), a.ino(), a.uid(), a.mode(), a.nlink())
        == (b.dev(), b.ino(), b.uid(), b.mode(), b.nlink())
        && ((a.is_dir() && b.is_dir()) || a.len() == b.len())
}
fn orphan_name_pid(name: &std::ffi::OsStr) -> Option<u32> {
    let (pid, sequence) = name.to_str()?.strip_prefix("probe-")?.split_once('-')?;
    let number = pid.parse::<u32>().ok()?;
    let serial = u64::from_str_radix(sequence, 16).ok()?;
    (number > 1
        && number <= i32::MAX as u32
        && pid == number.to_string()
        && sequence == format!("{serial:x}"))
    .then_some(number)
}

struct ChunkRun<'a> {
    scratch: Scratch,
    lease: &'a AuxiliaryLease,
    cleaned: bool,
}
impl<'a> ChunkRun<'a> {
    fn new(parent: &Path, chunk: &ProbeChunk, lease: &'a AuxiliaryLease) -> Result<Self> {
        Ok(Self {
            scratch: Scratch::create(parent, chunk.private_config())?,
            lease,
            cleaned: false,
        })
    }
    fn stop(&mut self) -> Result<()> {
        self.lease
            .stop_chunk()
            .map_err(|_| ProbeExecutionError::CleanupRequired)?;
        self.scratch.cleanup()?;
        self.cleaned = true;
        Ok(())
    }
}
impl Drop for ChunkRun<'_> {
    fn drop(&mut self) {
        if !self.cleaned && self.lease.finish().is_ok() {
            let _ = self.scratch.cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auxiliary_core::AuxiliarySlot;
    use omavless_mihomo::probe_plan::PinnedProfile;
    use omavless_profile::canonical::parse_canonical;
    use std::io::{Read, Write};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::sync::Arc;

    /// Invoked only as a supervised subprocess by the surrounding tests. No
    /// production CLI or network fixture is required; real Unix peer PID applies.
    #[test]
    fn synthetic_controller_child() {
        let Some(root) = std::env::var_os("OMAVLESS_TEST_PROBE_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let config = fs::read_to_string(root.join("config.yaml")).unwrap();
        let aliases: Vec<_> = config
            .lines()
            .filter_map(|line| {
                line.strip_prefix("    - \"")
                    .and_then(|line| line.strip_suffix('"'))
            })
            .collect();
        let listener = UnixListener::bind(root.join("controller.sock")).unwrap();
        fs::set_permissions(
            root.join("controller.sock"),
            fs::Permissions::from_mode(0o666),
        )
        .unwrap();
        let mode = std::env::var("OMAVLESS_TEST_PROBE_MODE").unwrap();
        for connection in listener.incoming() {
            let mut stream = connection.unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0_u8];
            while request.len() < 4096 && !request.ends_with(b"\r\n\r\n") {
                if stream.read(&mut byte).unwrap_or(0) == 0 {
                    break;
                }
                request.push(byte[0]);
            }
            if request.is_empty() {
                continue;
            }
            let request = String::from_utf8(request).unwrap();
            let body = if request.starts_with("GET /proxies ") {
                serde_json::to_vec(&serde_json::json!({"proxies":{"OMAVLESS_TEST":{"type":"Selector","all":aliases}}})).unwrap()
            } else if mode == "malformed" {
                b"private-response-fragment".to_vec()
            } else {
                serde_json::to_vec(
                    &aliases
                        .iter()
                        .map(|alias| (*alias, 42))
                        .collect::<std::collections::BTreeMap<_, _>>(),
                )
                .unwrap()
            };
            if mode == "silent" && request.starts_with("GET /group/") {
                std::thread::sleep(Duration::from_secs(60));
            }
            let _ = write!(
                stream,
                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(&body);
        }
    }

    struct Fixture {
        root: PathBuf,
        scratch: PathBuf,
        core: PathBuf,
    }
    impl Fixture {
        fn new(mode: &str) -> Self {
            let root = crate::test_temp::directory("probe-exec").unwrap();
            let scratch = root.join("scratch");
            fs::DirBuilder::new().mode(0o700).create(&scratch).unwrap();
            let core = root.join("core");
            let executable = std::env::current_exe().unwrap();
            assert!(!executable.as_os_str().as_encoded_bytes().contains(&b'\''));
            fs::write(&core, format!("#!/bin/sh\nOMAVLESS_TEST_PROBE_ROOT=\"$2\" OMAVLESS_TEST_PROBE_MODE='{mode}' exec '{}' --exact probe_executor::tests::synthetic_controller_child --test-threads=1\n", executable.display())).unwrap();
            fs::set_permissions(&core, fs::Permissions::from_mode(0o700)).unwrap();
            Self {
                root,
                scratch,
                core,
            }
        }
        fn empty(&self) {
            assert_eq!(fs::read_dir(&self.scratch).unwrap().count(), 0);
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn plan() -> ProbePlan {
        let profile =
            parse_canonical("trojan://synthetic@example.invalid:443?sni=cdn.example.invalid")
                .unwrap();
        ProbePlan::new(&[PinnedProfile {
            profile: &profile,
            addresses: &["192.0.2.1".parse().unwrap()],
        }])
        .unwrap()
    }

    #[test]
    fn supervised_three_round_success_reaps_before_return_and_retains_reservation() {
        let fixture = Fixture::new("success");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let mut progress = Vec::new();
        let results = execute_plan(
            &plan(),
            &fixture.core,
            &fixture.scratch,
            &lease,
            Instant::now() + Duration::from_secs(5),
            || false,
            |chunk, round| progress.push((chunk, round)),
        )
        .unwrap();
        assert_eq!(progress, [(0, 0), (0, 1), (0, 2)]);
        assert_eq!(
            results,
            [ProbeResult {
                resolved: true,
                reachable: true,
                latency_ms: 42
            }]
        );
        assert_eq!(slot.verified_pid().unwrap(), None);
        fixture.empty();
        assert!(slot.reserve().is_err());
        lease.finish().unwrap();
        assert!(slot.reserve().is_ok());
    }

    #[test]
    fn cancelled_live_round_reaps_and_restores_scratch_without_waiting_for_server() {
        let fixture = Fixture::new("silent");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let start = Instant::now();
        let result = execute_plan(
            &plan(),
            &fixture.core,
            &fixture.scratch,
            &lease,
            start + Duration::from_secs(5),
            || start.elapsed() > Duration::from_millis(250),
            |_, _| {},
        );
        assert_eq!(result.unwrap_err(), ProbeExecutionError::Cancelled);
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(slot.verified_pid().unwrap(), None);
        fixture.empty();
    }

    #[test]
    fn invalid_controller_response_reaps_and_safe_error_has_no_fragment() {
        let fixture = Fixture::new("malformed");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let error = execute_plan(
            &plan(),
            &fixture.core,
            &fixture.scratch,
            &lease,
            Instant::now() + Duration::from_secs(5),
            || false,
            |_, _| {},
        )
        .unwrap_err();
        assert_eq!(error, ProbeExecutionError::Rejected);
        assert!(!error.to_string().contains("fragment"));
        assert_eq!(slot.verified_pid().unwrap(), None);
        fixture.empty();
    }

    #[test]
    fn cancellation_observed_after_cleanup_wins_over_io_failure() {
        use std::sync::atomic::AtomicBool;
        let fixture = Fixture::new("malformed");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let observed_child = AtomicBool::new(false);
        let outcome = execute_plan(
            &plan(),
            &fixture.core,
            &fixture.scratch,
            &lease,
            Instant::now() + Duration::from_secs(5),
            || {
                let present = slot.verified_pid().unwrap().is_some();
                if present {
                    observed_child.store(true, Ordering::Release);
                }
                observed_child.load(Ordering::Acquire) && !present
            },
            |_, _| {},
        );
        assert_eq!(outcome.unwrap_err(), ProbeExecutionError::Cancelled);
        assert!(observed_child.load(Ordering::Acquire));
        fixture.empty();
    }

    #[test]
    fn panicked_progress_reaps_child_and_cleans_private_scratch() {
        let fixture = Fixture::new("success");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let result = std::panic::catch_unwind(|| {
            execute_plan(
                &plan(),
                &fixture.core,
                &fixture.scratch,
                &lease,
                Instant::now() + Duration::from_secs(5),
                || false,
                |_, _| panic!("synthetic callback fault"),
            )
        });
        assert!(result.is_err());
        assert_eq!(slot.verified_pid().unwrap(), None);
        fixture.empty();
        assert!(slot.reserve().is_ok());
    }

    #[test]
    fn empty_or_precancelled_plan_has_no_files_or_process_effects() {
        let fixture = Fixture::new("success");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        assert!(
            execute_plan(
                &ProbePlan::new(&[]).unwrap(),
                &fixture.core,
                &fixture.scratch,
                &lease,
                Instant::now() + Duration::from_secs(5),
                || false,
                |_, _| panic!("no work")
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            execute_plan(
                &plan(),
                &fixture.core,
                &fixture.scratch,
                &lease,
                Instant::now() + Duration::from_secs(5),
                || true,
                |_, _| {}
            )
            .unwrap_err(),
            ProbeExecutionError::Cancelled
        );
        fixture.empty();
        assert_eq!(slot.verified_pid().unwrap(), None);
    }

    #[test]
    fn scratch_refuses_unsafe_parent_and_does_not_remove_replacement() {
        let fixture = Fixture::new("success");
        fs::set_permissions(&fixture.scratch, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Scratch::create(&fixture.scratch, "synthetic").is_err());
        fs::set_permissions(&fixture.scratch, fs::Permissions::from_mode(0o700)).unwrap();
        let link = fixture.root.join("linked");
        symlink(&fixture.scratch, &link).unwrap();
        assert!(Scratch::create(&link, "synthetic").is_err());
        let scratch = Scratch::create(&fixture.scratch, "synthetic").unwrap();
        assert_eq!(
            fs::metadata(scratch.config()).unwrap().mode() & 0o7777,
            0o600
        );
        let original = fixture.root.join("preserved");
        fs::rename(&scratch.path, &original).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&scratch.path)
            .unwrap();
        fs::write(scratch.path.join("sentinel"), "unchanged").unwrap();
        assert_eq!(
            scratch.cleanup().unwrap_err(),
            ProbeExecutionError::CleanupRequired
        );
        assert!(scratch.path.join("sentinel").exists());
        assert!(original.join("config.yaml").exists());
    }

    #[test]
    fn multiple_chunks_run_sequentially_without_a_second_owned_core() {
        let fixture = Fixture::new("success");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let profile =
            parse_canonical("trojan://synthetic@example.invalid:443?sni=cdn.example.invalid")
                .unwrap();
        let addresses = ["192.0.2.1".parse().unwrap()];
        let profiles: Vec<_> = (0..65)
            .map(|_| PinnedProfile {
                profile: &profile,
                addresses: &addresses,
            })
            .collect();
        let plan = ProbePlan::new(&profiles).unwrap();
        let mut rounds = Vec::new();
        let results = execute_plan(
            &plan,
            &fixture.core,
            &fixture.scratch,
            &lease,
            Instant::now() + Duration::from_secs(5),
            || false,
            |chunk, round| {
                assert!(slot.verified_pid().unwrap().is_some());
                assert_eq!(fs::read_dir(&fixture.scratch).unwrap().count(), 1);
                rounds.push((chunk, round));
            },
        )
        .unwrap();
        assert_eq!(rounds, [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]);
        assert_eq!(results.len(), 65);
        assert!(results.iter().all(|r| r.reachable && r.latency_ms == 42));
        fixture.empty();
        assert_eq!(slot.verified_pid().unwrap(), None);
    }

    #[test]
    fn urgent_quiesce_revokes_inflight_work_before_next_round() {
        let fixture = Fixture::new("success");
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        let (progress_tx, progress_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let fixture_ref = &fixture;
            let lease_ref = &lease;
            let worker = scope.spawn(move || {
                execute_plan(
                    &plan(),
                    &fixture_ref.core,
                    &fixture_ref.scratch,
                    lease_ref,
                    Instant::now() + Duration::from_secs(5),
                    || false,
                    |_, _| {
                        progress_tx.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                    },
                )
            });
            progress_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let guard = slot.quiesce().unwrap();
            assert!(slot.reserve().is_err());
            assert_eq!(slot.verified_pid().unwrap(), None);
            release_tx.send(()).unwrap();
            assert_eq!(
                worker.join().unwrap().unwrap_err(),
                ProbeExecutionError::Cancelled
            );
            fixture.empty();
            drop(guard);
            assert!(slot.reserve().is_ok());
        });
    }

    fn orphan_fixture(parent: &Path, sequence: u64) -> PathBuf {
        // Beyond Linux pid_max, but within the accepted signed PID domain.
        let pid = i32::MAX as u32;
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        let path = parent.join(format!("probe-{pid}-{sequence:x}"));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        for (name, data, mode) in [
            (OWNER_MARKER, format!("{OWNER_MAGIC}\n{pid}\n"), 0o600),
            ("config.yaml", "synthetic-private-config".to_owned(), 0o600),
            ("cache.db", "synthetic-cache".to_owned(), 0o644),
        ] {
            fs::write(path.join(name), data).unwrap();
            fs::set_permissions(path.join(name), fs::Permissions::from_mode(mode)).unwrap();
        }
        path
    }

    fn stop_fixture_listener(listener: UnixListener) {
        // A parallel process-spawning test can briefly inherit this descriptor
        // between fork and exec, even with CLOEXEC. Dropping our fd alone is not
        // proof that the endpoint has stopped listening. Shut down the test-only
        // endpoint for every descriptor copy; production cleanup remains strict
        // and must still refuse a listener that could be live.
        nix::sys::socket::shutdown(listener.as_raw_fd(), nix::sys::socket::Shutdown::Both).unwrap();
        drop(listener);
    }

    #[test]
    fn orphan_cleanup_removes_only_marked_dead_private_fixed_members() {
        let fixture = Fixture::new("success");
        let orphan = orphan_fixture(&fixture.scratch, 0);
        let socket = UnixListener::bind(orphan.join("controller.sock")).unwrap();
        fs::set_permissions(
            orphan.join("controller.sock"),
            fs::Permissions::from_mode(0o666),
        )
        .unwrap();
        stop_fixture_listener(socket);
        fs::write(fixture.scratch.join("owner.lock"), "unrelated").unwrap();
        assert_eq!(cleanup_orphans(&fixture.scratch, || true).unwrap(), 1);
        assert!(!orphan.exists());
        assert_eq!(
            fs::read(fixture.scratch.join("owner.lock")).unwrap(),
            b"unrelated"
        );
        assert_eq!(
            cleanup_orphans(&fixture.scratch, || panic!(
                "no orphan means no host effects"
            ))
            .unwrap(),
            0
        );
    }

    #[test]
    fn orphan_cleanup_never_deletes_live_or_unproven_scratch() {
        let fixture = Fixture::new("success");
        let scratch = Scratch::create(&fixture.scratch, "synthetic").unwrap();
        assert!(cleanup_orphans(&fixture.scratch, || true).is_err());
        assert!(scratch.config().exists());
        scratch.cleanup().unwrap();
        let orphan = orphan_fixture(&fixture.scratch, 0);
        assert!(cleanup_orphans(&fixture.scratch, || false).is_err());
        let listener = UnixListener::bind(orphan.join("controller.sock")).unwrap();
        fs::set_permissions(
            orphan.join("controller.sock"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert!(cleanup_orphans(&fixture.scratch, || true).is_err());
        assert!(orphan.join("config.yaml").exists());
        // Deterministically model an outstanding descriptor copy: dropping the
        // original must not authorize deleting a still-listening orphan.
        let held_copy = listener.try_clone().unwrap();
        drop(listener);
        assert!(cleanup_orphans(&fixture.scratch, || true).is_err());
        assert!(orphan.join("config.yaml").exists());
        stop_fixture_listener(held_copy);
        assert_eq!(cleanup_orphans(&fixture.scratch, || true).unwrap(), 1);
    }

    #[test]
    fn orphan_cleanup_unknown_member_or_unmarked_directory_refuses_entire_set() {
        for bad in [
            "extra",
            "missing-marker",
            "bad-marker",
            "symlink",
            "wide-mode",
            "oversized",
        ] {
            let fixture = Fixture::new("success");
            let first = orphan_fixture(&fixture.scratch, 0);
            let second = orphan_fixture(&fixture.scratch, 1);
            match bad {
                "extra" => fs::write(second.join("unexpected"), "preserve").unwrap(),
                "missing-marker" => fs::remove_file(second.join(OWNER_MARKER)).unwrap(),
                "bad-marker" => fs::write(second.join(OWNER_MARKER), "unrecognized").unwrap(),
                "symlink" => {
                    fs::remove_file(second.join("cache.db")).unwrap();
                    symlink(first.join("cache.db"), second.join("cache.db")).unwrap();
                }
                "wide-mode" => fs::set_permissions(
                    second.join("config.yaml"),
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap(),
                "oversized" => fs::OpenOptions::new()
                    .write(true)
                    .open(second.join("cache.db"))
                    .unwrap()
                    .set_len(32 * 1024 * 1024 + 1)
                    .unwrap(),
                _ => unreachable!(),
            }
            assert_eq!(
                cleanup_orphans(&fixture.scratch, || true).unwrap_err(),
                ProbeExecutionError::CleanupRequired
            );
            assert!(first.join("config.yaml").exists());
            assert!(second.join("config.yaml").exists());
        }
    }

    #[test]
    fn orphan_cleanup_bounds_and_replacement_refuse_without_prefix_deletion() {
        let fixture = Fixture::new("success");
        for sequence in 0..=MAX_ORPHANS as u64 {
            orphan_fixture(&fixture.scratch, sequence);
        }
        assert!(cleanup_orphans(&fixture.scratch, || true).is_err());
        assert_eq!(
            fs::read_dir(&fixture.scratch).unwrap().count(),
            MAX_ORPHANS + 1
        );
        let replacement = Fixture::new("success");
        let orphan = orphan_fixture(&replacement.scratch, 0);
        let calls = AtomicU64::new(0);
        assert!(
            cleanup_orphans(&replacement.scratch, || {
                if calls.fetch_add(1, Ordering::Relaxed) == 1 {
                    fs::rename(&orphan, replacement.root.join("preserved")).unwrap();
                    fs::DirBuilder::new().mode(0o700).create(&orphan).unwrap();
                    fs::write(orphan.join("sentinel"), "unchanged").unwrap();
                }
                true
            })
            .is_err()
        );
        assert!(orphan.join("sentinel").exists());
        assert!(replacement.root.join("preserved/config.yaml").exists());
    }
}
