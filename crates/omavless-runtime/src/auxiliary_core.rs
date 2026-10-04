// SPDX-License-Identifier: MIT
//! One explicitly owned disposable core, separate from the requested VPN core.
//!
//! Internal host primitive only: no IPC method, numeric-PID adoption or global
//! process-name exemption. Blocking cleanup must be called OUTSIDE the runtime
//! owner mutex. The child stays waitable until its whole group has been drained.

use crate::core::OwnedCore;
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use nix::unistd::Uid;
use omavless_store::read_private_utf8;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const STOP_BUDGET: Duration = Duration::from_millis(600);
const DRAIN_BUDGET: Duration = Duration::from_millis(900);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryError {
    Busy,
    Cancelled,
    Invalid,
    Cleanup,
}

#[derive(Default)]
struct Signal {
    cancelled: AtomicBool,
    finished: AtomicBool,
}

struct Reservation {
    signal: Arc<Signal>,
    core: Option<TrackedCore>,
}

#[derive(Default)]
struct State {
    reservation: Option<Reservation>,
    inhibited: bool,
    draining: bool,
    failed: bool,
}

/// Only this slot spawns/adopts its own Child. No caller may register a PID.
#[derive(Default)]
pub struct AuxiliarySlot(Mutex<State>);

pub struct AuxiliaryLease {
    slot: Arc<AuxiliarySlot>,
    signal: Arc<Signal>,
}

/// Holds admission closed across a lifecycle transaction, after proven cleanup.
pub struct QuiescentGuard(Arc<AuxiliarySlot>);

impl AuxiliarySlot {
    /// Nonblocking host-side assertion. This does not perform cancellation;
    /// the outer runtime dispatcher must drain outside its owner mutex first.
    pub fn mutation_safe(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|s| !s.failed && !s.draining && s.reservation.is_none())
    }
    pub fn reserve(self: &Arc<Self>) -> Result<AuxiliaryLease, AuxiliaryError> {
        let mut state = self.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
        if state.failed {
            return Err(AuxiliaryError::Cleanup);
        }
        if state.inhibited || state.draining || state.reservation.is_some() {
            return Err(AuxiliaryError::Busy);
        }
        let signal = Arc::<Signal>::default();
        state.reservation = Some(Reservation {
            signal: Arc::clone(&signal),
            core: None,
        });
        Ok(AuxiliaryLease {
            slot: Arc::clone(self),
            signal,
        })
    }

    /// Revoke before waiting, so an admitted worker cannot spawn after the
    /// empty-host check. The owner mutex must NOT be held by the caller.
    pub fn quiesce(self: &Arc<Self>) -> Result<QuiescentGuard, AuxiliaryError> {
        let signal = {
            let mut state = self.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
            if state.failed {
                return Err(AuxiliaryError::Cleanup);
            }
            if state.inhibited {
                return Err(AuxiliaryError::Busy);
            }
            state.inhibited = true;
            state.reservation.as_ref().map(|r| {
                r.signal.cancelled.store(true, Ordering::Release);
                Arc::clone(&r.signal)
            })
        };
        let guard = QuiescentGuard(Arc::clone(self));
        if let Some(signal) = signal {
            self.drain(&signal, true)?;
        }
        Ok(guard)
    }

    /// Exact explicitly owned auxiliary PID for internal observation only.
    /// Unknown processes are never filtered. A failed proof is not an empty slot.
    pub fn verified_pid(&self) -> Result<Option<u32>, AuxiliaryError> {
        let state = self.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
        if state.failed || state.draining {
            return Err(AuxiliaryError::Cleanup);
        }
        state
            .reservation
            .as_ref()
            .and_then(|r| r.core.as_ref())
            .map(TrackedCore::verify)
            .transpose()
    }

    fn drain(&self, signal: &Arc<Signal>, finish: bool) -> Result<(), AuxiliaryError> {
        let deadline = Instant::now() + DRAIN_BUDGET;
        let mut core = loop {
            if signal.finished.load(Ordering::Acquire) {
                return Ok(());
            }
            #[cfg(test)]
            tests::after_unfinished_check();
            let mut state = self.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
            if state.failed {
                return Err(AuxiliaryError::Cleanup);
            }
            let matching = state
                .reservation
                .as_ref()
                .is_some_and(|r| Arc::ptr_eq(&r.signal, signal));
            if !matching {
                return Err(AuxiliaryError::Cancelled);
            }
            if state.draining {
                drop(state);
                if Instant::now() >= deadline {
                    return Err(AuxiliaryError::Cleanup);
                }
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            state.draining = true;
            break state.reservation.as_mut().and_then(|r| r.core.take());
        };
        // No slot or runtime-owner mutex is held while waiting for the child.
        let stopped = core
            .as_mut()
            .is_none_or(|c| c.child.stop(STOP_BUDGET).is_ok());
        let mut state = self.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
        state.draining = false;
        if !stopped {
            state.failed = true;
            if let Some(reservation) = state.reservation.as_mut() {
                reservation.core = core;
            }
            return Err(AuxiliaryError::Cleanup);
        }
        // Cancellation may have arrived while stop_chunk was outside the lock.
        if finish || signal.cancelled.load(Ordering::Acquire) {
            state.reservation = None;
            signal.finished.store(true, Ordering::Release);
        }
        Ok(())
    }
}

impl Drop for QuiescentGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.0.lock() {
            state.inhibited = false;
        }
    }
}

impl AuxiliaryLease {
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.signal.cancelled.load(Ordering::Acquire)
    }

    pub fn spawn(
        &self,
        core: &Path,
        directory: &Path,
        config: &Path,
        controller: &Path,
    ) -> Result<(), AuxiliaryError> {
        let mut state = self.slot.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
        if state.failed {
            return Err(AuxiliaryError::Cleanup);
        }
        if state.inhibited || self.cancelled() {
            return Err(AuxiliaryError::Cancelled);
        }
        if state.draining {
            return Err(AuxiliaryError::Busy);
        }
        let reservation = state
            .reservation
            .as_mut()
            .filter(|r| Arc::ptr_eq(&r.signal, &self.signal))
            .ok_or(AuxiliaryError::Cancelled)?;
        if reservation.core.is_some() {
            return Err(AuxiliaryError::Busy);
        }
        let files = Files::capture(directory, config, controller)?;
        let child = OwnedCore::spawn(core, directory, config, controller)
            .map_err(|_| AuxiliaryError::Invalid)?;
        let pid = child.pid().ok_or(AuxiliaryError::Invalid)?;
        // Keep the child in the slot even if post-spawn proof fails. Cleanup,
        // never a dropped numeric PID or an untracked live child, follows.
        let identity = process_identity(pid);
        reservation.core = Some(TrackedCore {
            child,
            identity,
            files,
        });
        identity.ok_or(AuxiliaryError::Invalid).map(|_| ())
    }

    pub fn pid(&self) -> Result<u32, AuxiliaryError> {
        if self.cancelled() {
            return Err(AuxiliaryError::Cancelled);
        }
        let state = self.slot.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
        if state.failed || state.draining {
            return Err(AuxiliaryError::Cleanup);
        }
        state
            .reservation
            .as_ref()
            .filter(|r| Arc::ptr_eq(&r.signal, &self.signal))
            .and_then(|r| r.core.as_ref())
            .ok_or(AuxiliaryError::Cancelled)?
            .verify()
    }

    pub fn running(&self) -> Result<bool, AuxiliaryError> {
        if self.cancelled() {
            return Ok(false);
        }
        let mut state = self.slot.0.lock().map_err(|_| AuxiliaryError::Cleanup)?;
        if state.failed || state.draining {
            return Err(AuxiliaryError::Cleanup);
        }
        let core = state
            .reservation
            .as_mut()
            .filter(|r| Arc::ptr_eq(&r.signal, &self.signal))
            .and_then(|r| r.core.as_mut())
            .ok_or(AuxiliaryError::Cancelled)?;
        core.verify()?;
        core.child.running().map_err(|_| AuxiliaryError::Cleanup)
    }

    /// Keep the reservation for the next bounded chunk, unless revoked.
    pub fn stop_chunk(&self) -> Result<(), AuxiliaryError> {
        self.slot.drain(&self.signal, false)
    }

    pub fn finish(&self) -> Result<(), AuxiliaryError> {
        self.signal.cancelled.store(true, Ordering::Release);
        self.slot.drain(&self.signal, true)
    }
}

impl Drop for AuxiliaryLease {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

struct TrackedCore {
    child: OwnedCore,
    identity: Option<(u32, u64)>,
    files: Files,
}
impl TrackedCore {
    fn verify(&self) -> Result<u32, AuxiliaryError> {
        let pid = self.child.pid().ok_or(AuxiliaryError::Invalid)?;
        if self.identity.is_none()
            || process_identity(pid) != self.identity
            || !self.files.matches(pid)
        {
            return Err(AuxiliaryError::Invalid);
        }
        Ok(pid)
    }
}

struct Files {
    directory: PathBuf,
    config: PathBuf,
    controller: PathBuf,
    directory_id: (u64, u64),
    config_id: (u64, u64),
    digest: [u8; 32],
    socket_id: Mutex<Option<(u64, u64)>>,
}
impl Files {
    fn capture(directory: &Path, config: &Path, controller: &Path) -> Result<Self, AuxiliaryError> {
        let uid = Uid::current().as_raw();
        if !directory.is_absolute()
            || config.parent() != Some(directory)
            || controller.parent() != Some(directory)
            || config == controller
        {
            return Err(AuxiliaryError::Invalid);
        }
        let dir = fs::symlink_metadata(directory).map_err(|_| AuxiliaryError::Invalid)?;
        let file = fs::symlink_metadata(config).map_err(|_| AuxiliaryError::Invalid)?;
        if !dir.is_dir()
            || dir.uid() != uid
            || dir.mode() & 0o7777 != 0o700
            || !file.is_file()
            || file.uid() != uid
            || file.mode() & 0o7777 != 0o600
        {
            return Err(AuxiliaryError::Invalid);
        }
        if fs::symlink_metadata(controller).is_ok()
            || fs::symlink_metadata(controller)
                .is_err_and(|e| e.kind() != std::io::ErrorKind::NotFound)
        {
            return Err(AuxiliaryError::Invalid);
        }
        let payload = read_private_utf8(config, uid).map_err(|_| AuxiliaryError::Invalid)?;
        Ok(Self {
            directory: directory.to_owned(),
            config: config.to_owned(),
            controller: controller.to_owned(),
            directory_id: (dir.dev(), dir.ino()),
            config_id: (file.dev(), file.ino()),
            digest: Sha256::digest(payload.as_bytes()).into(),
            socket_id: Mutex::new(None),
        })
    }
    fn matches(&self, pid: u32) -> bool {
        let uid = Uid::current().as_raw();
        let directory_ok = fs::symlink_metadata(&self.directory).is_ok_and(|m| {
            m.is_dir()
                && m.uid() == uid
                && m.mode() & 0o7777 == 0o700
                && (m.dev(), m.ino()) == self.directory_id
        });
        let config_ok = fs::symlink_metadata(&self.config).is_ok_and(|m| {
            m.is_file()
                && m.uid() == uid
                && m.mode() & 0o7777 == 0o600
                && (m.dev(), m.ino()) == self.config_id
        });
        let controller_ok = self.controller_matches(pid, uid);
        directory_ok
            && config_ok
            && controller_ok
            && read_private_utf8(&self.config, uid)
                .is_ok_and(|s| <[u8; 32]>::from(Sha256::digest(s.as_bytes())) == self.digest)
    }

    fn controller_matches(&self, pid: u32, uid: u32) -> bool {
        let Ok(mut pinned) = self.socket_id.lock() else {
            return false;
        };
        let before = match fs::symlink_metadata(&self.controller) {
            Ok(m) => m,
            Err(e) => return e.kind() == std::io::ErrorKind::NotFound && pinned.is_none(),
        };
        let identity = (before.dev(), before.ino());
        if !before.file_type().is_socket()
            || before.uid() != uid
            || !matches!(before.mode() & 0o7777, 0o600 | 0o666)
            || pinned.is_some_and(|previous| previous != identity)
        {
            return false;
        }
        let authenticated = || -> Option<()> {
            let fd = socket(
                AddressFamily::Unix,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
                None,
            )
            .ok()?;
            connect(fd.as_raw_fd(), &UnixAddr::new(&self.controller).ok()?).ok()?;
            let stream = UnixStream::from(fd);
            let peer = getsockopt(&stream, PeerCredentials).ok()?;
            let after = fs::symlink_metadata(&self.controller).ok()?;
            (peer.uid() == uid
                && u32::try_from(peer.pid()).ok()? == pid
                && (after.dev(), after.ino()) == identity)
                .then_some(())
        };
        if authenticated().is_none() {
            return false;
        }
        *pinned = Some(identity);
        true
    }
}

/// PID reuse and parent changes fail closed; no comm/cmdline value is emitted.
fn process_identity(pid: u32) -> Option<(u32, u64)> {
    let mut raw = Vec::new();
    File::open(format!("/proc/{pid}/stat"))
        .ok()?
        .take(4097)
        .read_to_end(&mut raw)
        .ok()?;
    if raw.len() > 4096 {
        return None;
    }
    let close = raw.iter().rposition(|b| *b == b')')?;
    let suffix = std::str::from_utf8(&raw[close + 1..]).ok()?;
    let fields: Vec<_> = suffix.split_ascii_whitespace().collect();
    let parent = fields.get(1)?.parse::<u32>().ok()?;
    let group = fields.get(2)?.parse::<u32>().ok()?;
    let start = fields.get(19)?.parse::<u64>().ok()?;
    (parent == std::process::id() && group == pid && start != 0).then_some((parent, start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::os::unix::fs::{PermissionsExt, symlink};

    thread_local! {
        static AFTER_UNFINISHED: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    }

    pub(super) fn after_unfinished_check() {
        let hook = AFTER_UNFINISHED.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }

    struct ChunkWorker {
        thread: Option<std::thread::JoinHandle<Result<(), AuxiliaryError>>>,
        resume: Option<std::sync::mpsc::SyncSender<()>>,
    }

    impl ChunkWorker {
        fn resume(&mut self) {
            if let Some(resume) = self.resume.take() {
                let _ = resume.send(());
            }
        }
        fn join(mut self) -> Result<(), AuxiliaryError> {
            self.resume();
            self.thread.take().unwrap().join().unwrap()
        }
    }

    impl Drop for ChunkWorker {
        fn drop(&mut self) {
            // An assertion/handshake failure must still unblock and reap the
            // worker before Fixture removes its private child inputs.
            self.resume();
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    struct Fixture {
        root: PathBuf,
        tool: PathBuf,
        config: PathBuf,
        controller: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = crate::test_temp::directory("aux-core").unwrap();
            let tool = root.join("synthetic-core");
            fs::write(&tool, "#!/bin/sh\nexec /usr/bin/sleep 60\n").unwrap();
            fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
            let config = root.join("config.yaml");
            fs::write(&config, "mode: rule\n").unwrap();
            fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
            let controller = root.join("controller.sock");
            // Match existing executable fixture publication gate on overlayfs.
            std::thread::sleep(Duration::from_millis(20));
            Self {
                root,
                tool,
                config,
                controller,
            }
        }
        fn spawn(&self, lease: &AuxiliaryLease) -> Result<(), AuxiliaryError> {
            lease.spawn(&self.tool, &self.root, &self.config, &self.controller)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn admission_is_exclusive_and_quiesce_revokes_before_spawn() {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let old = slot.reserve().unwrap();
        assert!(matches!(slot.reserve(), Err(AuxiliaryError::Busy)));
        let guard = slot.quiesce().unwrap();
        assert!(old.cancelled());
        assert_eq!(fixture.spawn(&old), Err(AuxiliaryError::Cancelled));
        assert!(matches!(slot.reserve(), Err(AuxiliaryError::Busy)));
        drop(guard);
        let new = slot.reserve().unwrap();
        drop(old);
        fixture.spawn(&new).unwrap();
        assert!(new.running().unwrap());
        new.finish().unwrap();
        assert_eq!(slot.verified_pid(), Ok(None));
    }

    #[test]
    fn identity_is_owned_child_only_and_quiesce_reaps_before_reopening() {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        fixture.spawn(&lease).unwrap();
        let pid = lease.pid().unwrap();
        assert_eq!(slot.verified_pid(), Ok(Some(pid)));
        assert!(process_identity(std::process::id()).is_none());
        let guard = slot.quiesce().unwrap();
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        assert_eq!(slot.verified_pid(), Ok(None));
        assert!(!lease.running().unwrap());
        lease.finish().unwrap();
        drop(guard);
        assert!(slot.reserve().is_ok());
    }

    #[test]
    fn chunks_keep_reservation_and_drop_cleans_last_child() {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        fixture.spawn(&lease).unwrap();
        let first = lease.pid().unwrap();
        lease.stop_chunk().unwrap();
        assert!(!Path::new(&format!("/proc/{first}")).exists());
        assert!(matches!(slot.reserve(), Err(AuxiliaryError::Busy)));
        fixture.spawn(&lease).unwrap();
        let second = lease.pid().unwrap();
        drop(lease);
        assert!(!Path::new(&format!("/proc/{second}")).exists());
        assert!(slot.reserve().is_ok());
    }

    #[test]
    fn changed_or_replaced_private_config_is_not_exempted() {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        fixture.spawn(&lease).unwrap();
        fs::write(&fixture.config, "mode: global\n").unwrap();
        assert_eq!(slot.verified_pid(), Err(AuxiliaryError::Invalid));
        fs::write(&fixture.config, "mode: rule\n").unwrap();
        assert!(slot.verified_pid().unwrap().is_some());
        fs::rename(&fixture.config, fixture.root.join("old.yaml")).unwrap();
        fs::write(&fixture.config, "mode: rule\n").unwrap();
        fs::set_permissions(&fixture.config, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(slot.verified_pid(), Err(AuxiliaryError::Invalid));
        // Failed observation must not prevent killing the still-owned child.
        lease.finish().unwrap();
    }

    #[test]
    fn unsafe_paths_and_existing_controller_refuse_before_spawn() {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        fs::set_permissions(&fixture.config, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(fixture.spawn(&lease), Err(AuxiliaryError::Invalid));
        fs::set_permissions(&fixture.config, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&fixture.config, &fixture.controller).unwrap();
        assert_eq!(fixture.spawn(&lease), Err(AuxiliaryError::Invalid));
        fs::remove_file(&fixture.controller).unwrap();
        assert_eq!(slot.verified_pid(), Ok(None));
    }

    #[test]
    fn replaced_controller_and_wrong_process_start_fail_closed() {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = slot.reserve().unwrap();
        fixture.spawn(&lease).unwrap();
        fs::write(&fixture.controller, "not a socket").unwrap();
        assert_eq!(slot.verified_pid(), Err(AuxiliaryError::Invalid));
        fs::remove_file(&fixture.controller).unwrap();
        let listener = std::os::unix::net::UnixListener::bind(&fixture.controller).unwrap();
        fs::set_permissions(&fixture.controller, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(slot.verified_pid(), Err(AuxiliaryError::Invalid));
        drop(listener);
        fs::remove_file(&fixture.controller).unwrap();
        {
            let mut state = slot.0.lock().unwrap();
            let core = state.reservation.as_mut().unwrap().core.as_mut().unwrap();
            core.identity.as_mut().unwrap().1 += 1;
        }
        assert_eq!(slot.verified_pid(), Err(AuxiliaryError::Invalid));
        lease.finish().unwrap();
    }

    #[test]
    fn revoke_during_chunk_cleanup_never_reopens_stale_lease() {
        revoke_during_chunk_cleanup(false);
    }

    #[test]
    fn quiesce_between_unfinished_read_and_slot_lock_returns_cancelled_safely() {
        revoke_during_chunk_cleanup(true);
    }

    fn revoke_during_chunk_cleanup(force_interleave: bool) {
        let fixture = Fixture::new();
        let slot = Arc::<AuxiliarySlot>::default();
        let lease = Arc::new(slot.reserve().unwrap());
        fixture.spawn(&lease).unwrap();
        let original_pid = lease.pid().unwrap();
        let worker_lease = Arc::clone(&lease);
        let (reached_tx, reached_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            if force_interleave {
                AFTER_UNFINISHED.with(|slot| {
                    *slot.borrow_mut() = Some(Box::new(move || {
                        reached_tx.send(()).unwrap();
                        resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    }));
                });
            }
            worker_lease.stop_chunk()
        });
        let mut worker = ChunkWorker {
            thread: Some(thread),
            resume: Some(resume_tx),
        };
        if force_interleave {
            reached_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(!lease.signal.finished.load(Ordering::Acquire));
        }
        let guard = slot.quiesce().unwrap();
        assert!(lease.signal.finished.load(Ordering::Acquire));
        assert!(!Path::new(&format!("/proc/{original_pid}")).exists());
        assert_eq!(slot.verified_pid(), Ok(None));
        assert!(matches!(slot.reserve(), Err(AuxiliaryError::Busy)));
        if force_interleave {
            worker.resume();
        }
        let outcome = worker.join();
        assert!(lease.cancelled());
        assert_eq!(fixture.spawn(&lease), Err(AuxiliaryError::Cancelled));
        drop(guard);
        let next = slot.reserve().unwrap();
        fixture.spawn(&next).unwrap();
        let successor_pid = next.pid().unwrap();
        assert_eq!(fixture.spawn(&lease), Err(AuxiliaryError::Cancelled));
        drop(lease);
        assert_eq!(slot.verified_pid(), Ok(Some(successor_pid)));
        assert!(next.running().unwrap());
        next.finish().unwrap();
        assert!(!Path::new(&format!("/proc/{successor_pid}")).exists());
        assert_eq!(slot.verified_pid(), Ok(None));
        // Quiesce may finish after the worker's atomic read but before its
        // reservation check. Only successful cleanup or that stale-identity
        // refusal is valid; Cleanup/Busy/Invalid must never be hidden.
        assert!(matches!(outcome, Ok(()) | Err(AuxiliaryError::Cancelled)));
        if force_interleave {
            assert_eq!(outcome, Err(AuxiliaryError::Cancelled));
        }
    }

    #[test]
    fn poisoned_or_failed_slots_never_admit_or_report_empty() {
        let slot = Arc::<AuxiliarySlot>::default();
        slot.0.lock().unwrap().failed = true;
        assert!(matches!(slot.reserve(), Err(AuxiliaryError::Cleanup)));
        assert!(matches!(slot.quiesce(), Err(AuxiliaryError::Cleanup)));
        assert_eq!(slot.verified_pid(), Err(AuxiliaryError::Cleanup));
        let slot = Arc::<AuxiliarySlot>::default();
        let poisoned = Arc::clone(&slot);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned.0.lock().unwrap();
            panic!("synthetic");
        })
        .join();
        assert!(matches!(slot.reserve(), Err(AuxiliaryError::Cleanup)));
        assert!(matches!(slot.quiesce(), Err(AuxiliaryError::Cleanup)));
    }
}
