// SPDX-License-Identifier: MIT
//! Inactive fixed conditional transport bound to a parent-owned waitable child.
//! Production cannot construct the candidate effect permit. Package attestation,
//! owner confirmation/revision admission and production scheduling remain required.
use crate::core::OwnedCore;
use nix::fcntl::{OFlag, open, openat};
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use nix::sys::stat::Mode;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs::{self, File, Metadata};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_ROWS: usize = 128;
const MAX_SNAPSHOT: usize = 256 * 1024;
const MAX_REPLY: usize = 16 * 1024;
const BUDGET: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Closed,
    Missing,
    Changed,
    Unsupported,
    Unknown,
    RefusedBeforeWrite,
}

/// No production constructor, Clone, formatting or deserialization.
/// This is explicitly NOT a normal owner confirmation or package proof.
pub(crate) struct CandidateEffectPermit {
    _private: (),
}

#[derive(Deserialize)]
struct WireSnapshot {
    #[serde(deserialize_with = "nullable_rows")]
    connections: Vec<WireRow>,
}
fn nullable_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<WireRow>, D::Error> {
    Ok(Option::<Vec<WireRow>>::deserialize(d)?.unwrap_or_default())
}
#[derive(Deserialize)]
struct WireRow {
    id: String,
    #[serde(rename = "omavlessCloseToken")]
    token: String,
}

struct Target {
    id: String,
    token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    abi: u8,
    ready: bool,
}

enum Request<'a> {
    Snapshot,
    Capabilities,
    Close(&'a Target),
}
pub(crate) struct BoundTarget {
    target: Target,
    binding: Binding,
    session_identity: Arc<()>,
}
impl std::fmt::Debug for BoundTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoundCloseTarget([private])")
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn targets(raw: &[u8]) -> Result<Vec<Target>, Outcome> {
    if raw.len() > MAX_SNAPSHOT {
        return Err(Outcome::Unsupported);
    }
    // Struct deserialization rejects duplicate known keys before Value can
    // collapse them, including escaped-equivalent ID and token names.
    let snapshot: WireSnapshot = serde_json::from_slice(raw).map_err(|_| Outcome::Unsupported)?;
    let rows = snapshot.connections;
    if rows.len() > MAX_ROWS {
        return Err(Outcome::Unsupported);
    }
    let mut ids = BTreeSet::new();
    let mut tokens = BTreeSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let value = row.token.parse::<u64>().map_err(|_| Outcome::Unsupported)?;
        if !valid_id(&row.id)
            || value == 0
            || value.to_string() != row.token
            || !ids.insert(row.id.clone())
            || !tokens.insert(row.token.clone())
        {
            return Err(Outcome::Unsupported);
        }
        result.push(Target {
            id: row.id,
            token: row.token,
        });
    }
    Ok(result)
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Binding {
    pid: u32,
    uid: u32,
    directory: (u64, u64),
    socket: (u64, u64),
}
fn inode(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}
fn remaining(deadline: Instant) -> Result<Duration, Outcome> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(Outcome::Unknown)
}

#[derive(Clone, Copy)]
enum Phase {
    BeforeEffect,
    EffectAttempted,
    Finished(Outcome),
}
struct Reservation {
    identity: Arc<()>,
    phase: Phase,
    cancelled: bool,
    stream: Option<UnixStream>,
}
struct Gate {
    live: bool,
    reservation: Option<Reservation>,
}
impl Gate {
    fn revoke(&mut self) {
        self.live = false;
        if let Some(reservation) = &mut self.reservation {
            if !matches!(reservation.phase, Phase::Finished(_)) {
                reservation.cancelled = true;
            }
            if let Some(stream) = &reservation.stream {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
    }
}
/// Retained non-reusable child identity, not ownership of Child. OwnedCore
/// revokes before signalling or reaping. WNOWAIT is legal only under Live.
pub(crate) struct Lifetime {
    pid: u32,
    gate: Mutex<Gate>,
}
impl Lifetime {
    pub(crate) fn new(pid: u32) -> Self {
        Self {
            pid,
            gate: Mutex::new(Gate {
                live: true,
                reservation: None,
            }),
        }
    }
    pub(crate) fn revoke(&self) {
        // Recover poisoning to revoke during unwind as well. No blocking I/O,
        // callback or child cleanup runs while this fast gate is held.
        let mut gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.revoke();
    }
    fn running(&self, gate: &mut Gate) -> bool {
        if !gate.live {
            return false;
        }
        let Ok(pid) = i32::try_from(self.pid) else {
            gate.revoke();
            return false;
        };
        let running = matches!(
            nix::sys::wait::waitid(
                nix::sys::wait::Id::Pid(nix::unistd::Pid::from_raw(pid)),
                nix::sys::wait::WaitPidFlag::WEXITED
                    | nix::sys::wait::WaitPidFlag::WNOHANG
                    | nix::sys::wait::WaitPidFlag::WNOWAIT,
            ),
            Ok(nix::sys::wait::WaitStatus::StillAlive)
        );
        if !running {
            gate.revoke();
        }
        running
    }
}

/// Cancellation and effect linearization share the retained child gate.
/// A finished definitive receipt wins before later cancellation.
#[derive(Clone)]
pub(crate) struct Cancellation {
    lifetime: Arc<Lifetime>,
    identity: Arc<()>,
}
impl Cancellation {
    pub(crate) fn cancel(&self) {
        let mut gate = self.lifetime.gate.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = &mut gate.reservation
            && Arc::ptr_eq(&r.identity, &self.identity)
            && !matches!(r.phase, Phase::Finished(_))
        {
            r.cancelled = true;
            if let Some(stream) = &r.stream {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
    }
}

/// Owned descriptors and a permanently revocable retained child identity.
/// No owner borrow or owner/migration mutex spans controller I/O.
pub(crate) struct Session {
    lifetime: Arc<Lifetime>,
    path: PathBuf,
    directory: File,
    socket: File,
    binding: Binding,
    identity: Arc<()>,
    deadline: Option<Instant>,
    #[cfg(test)]
    panic_after_write: bool,
    #[cfg(test)]
    before_finish: Option<Arc<std::sync::Barrier>>,
    #[cfg(test)]
    before_write: Option<Arc<std::sync::Barrier>>,
    #[cfg(test)]
    effect_chunk: usize,
    #[cfg(test)]
    after_chunk: Option<Arc<std::sync::Barrier>>,
}

impl Session {
    pub(crate) fn bind(core: &mut OwnedCore, uid: u32) -> Result<Self, Outcome> {
        if !core.running().is_ok_and(|v| v) {
            return Err(Outcome::RefusedBeforeWrite);
        }
        let lifetime = core
            .conditional_lifetime()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        let path = core.controller_path().to_owned();
        let parent = path.parent().ok_or(Outcome::RefusedBeforeWrite)?;
        let directory = File::from(
            open(
                parent,
                OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Outcome::RefusedBeforeWrite)?,
        );
        let socket = File::from(
            openat(
                &directory,
                Path::new(path.file_name().ok_or(Outcome::RefusedBeforeWrite)?),
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Outcome::RefusedBeforeWrite)?,
        );
        let binding = Binding {
            pid: core.pid().ok_or(Outcome::RefusedBeforeWrite)?,
            uid,
            directory: inode(
                &directory
                    .metadata()
                    .map_err(|_| Outcome::RefusedBeforeWrite)?,
            ),
            socket: inode(&socket.metadata().map_err(|_| Outcome::RefusedBeforeWrite)?),
        };
        let mut session = Self {
            lifetime,
            path,
            directory,
            socket,
            binding,
            identity: Arc::new(()),
            deadline: None,
            #[cfg(test)]
            panic_after_write: false,
            #[cfg(test)]
            before_finish: None,
            #[cfg(test)]
            before_write: None,
            #[cfg(test)]
            effect_chunk: usize::MAX,
            #[cfg(test)]
            after_chunk: None,
        };
        {
            let mut gate = session
                .lifetime
                .gate
                .lock()
                .map_err(|_| Outcome::RefusedBeforeWrite)?;
            if !session.lifetime.running(&mut gate) || gate.reservation.is_some() {
                return Err(Outcome::RefusedBeforeWrite);
            }
            gate.reservation = Some(Reservation {
                identity: Arc::clone(&session.identity),
                phase: Phase::BeforeEffect,
                cancelled: false,
                stream: None,
            });
        }
        session.check()?;
        Ok(session)
    }

    fn check(&mut self) -> Result<(), Outcome> {
        let lifetime = Arc::clone(&self.lifetime);
        let mut gate = lifetime
            .gate
            .lock()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        self.check_locked(&mut gate)
    }

    fn check_locked(&self, gate: &mut Gate) -> Result<(), Outcome> {
        let refuse = Outcome::RefusedBeforeWrite;
        if self.lifetime.pid != self.binding.pid
            || !self.lifetime.running(gate)
            || !gate.reservation.as_ref().is_some_and(|r| {
                Arc::ptr_eq(&r.identity, &self.identity)
                    && !r.cancelled
                    && !matches!(r.phase, Phase::Finished(_))
            })
        {
            return Err(refuse);
        }
        let path = &self.path;
        let parent = path.parent().ok_or(refuse)?;
        for metadata in [self.directory.metadata(), fs::symlink_metadata(parent)] {
            let metadata = metadata.map_err(|_| refuse)?;
            if !metadata.is_dir()
                || metadata.uid() != self.binding.uid
                || metadata.mode() & 0o7777 != 0o700
                || inode(&metadata) != self.binding.directory
            {
                return Err(refuse);
            }
        }
        for metadata in [self.socket.metadata(), fs::symlink_metadata(path)] {
            let metadata = metadata.map_err(|_| refuse)?;
            if !metadata.file_type().is_socket()
                || metadata.uid() != self.binding.uid
                || metadata.mode() & 0o7777 != 0o600
                || metadata.nlink() != 1
                || inode(&metadata) != self.binding.socket
            {
                return Err(refuse);
            }
        }
        Ok(())
    }

    fn connected(&mut self) -> Result<UnixStream, Outcome> {
        self.check()?;
        let refuse = Outcome::RefusedBeforeWrite;
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
            None,
        )
        .map_err(|_| refuse)?;
        connect(
            fd.as_raw_fd(),
            &UnixAddr::new(&self.path).map_err(|_| refuse)?,
        )
        .map_err(|_| refuse)?;
        let stream = UnixStream::from(fd);
        let peer = getsockopt(&stream, PeerCredentials).map_err(|_| refuse)?;
        if peer.uid() != self.binding.uid
            || u32::try_from(peer.pid()).ok() != Some(self.binding.pid)
        {
            return Err(refuse);
        }
        self.check()?;
        let lifetime = Arc::clone(&self.lifetime);
        let mut gate = lifetime.gate.lock().map_err(|_| refuse)?;
        self.check_locked(&mut gate)?;
        gate.reservation.as_mut().ok_or(refuse)?.stream =
            Some(stream.try_clone().map_err(|_| refuse)?);
        Ok(stream)
    }

    fn exchange(&mut self, target: Option<&Target>) -> Result<(u16, Vec<u8>), Outcome> {
        self.exchange_request(target.map_or(Request::Snapshot, Request::Close))
    }

    fn exchange_request(&mut self, request: Request<'_>) -> Result<(u16, Vec<u8>), Outcome> {
        let deadline = *self.deadline.get_or_insert_with(|| Instant::now() + BUDGET);
        let mut stream = self.connected()?;
        let effect = matches!(request, Request::Close(_));
        let request = match request {
            Request::Snapshot => {
                "GET /connections HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n".into()
            }
            Request::Capabilities => "GET /connections/conditional-capabilities HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n".into(),
            Request::Close(t) => format!(
                "POST /connections/{}/close-conditional HTTP/1.0\r\nHost: localhost\r\nIf-Match: \"{}\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                t.id, t.token
            ),
        };
        // From the first write attempt onward, partial send/lost response is
        // unknown. Never reconnect/resend, DELETE by ID, close-all or infer a
        // receipt from later absence.
        let mut pending = request.as_bytes();
        while !pending.is_empty() {
            remaining(deadline)?;
            #[cfg(test)]
            if effect && let Some(barrier) = &self.before_write {
                barrier.wait();
                barrier.wait();
            }
            // First effect attempt and every partial chunk are serialized
            // against revocation/cancel. Socket is nonblocking throughout.
            let result = {
                let mut gate = self.lifetime.gate.lock().map_err(|_| Outcome::Unknown)?;
                self.check_locked(&mut gate)?;
                remaining(deadline)?;
                if effect {
                    gate.reservation.as_mut().ok_or(Outcome::Unknown)?.phase =
                        Phase::EffectAttempted;
                }
                #[cfg(test)]
                let pending = if effect {
                    &pending[..pending.len().min(self.effect_chunk)]
                } else {
                    pending
                };
                stream.write(pending)
            };
            let n = match result {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.pause(deadline)?;
                    continue;
                }
                Err(_) => return Err(Outcome::Unknown),
            };
            if n == 0 {
                return Err(Outcome::Unknown);
            }
            pending = &pending[n..];
            #[cfg(test)]
            if effect && let Some(barrier) = self.after_chunk.take() {
                barrier.wait();
                barrier.wait();
            }
        }
        #[cfg(test)]
        if effect && self.panic_after_write {
            panic!("fixed research worker failure");
        }
        let cap = if effect { MAX_REPLY } else { MAX_SNAPSHOT };
        let mut raw = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            remaining(deadline)?;
            self.check()?;
            let n = match stream.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.pause(deadline)?;
                    continue;
                }
                Err(_) => return Err(Outcome::Unknown),
            };
            if n == 0 {
                break;
            }
            if raw.len().saturating_add(n) > cap {
                return Err(Outcome::Unknown);
            }
            raw.extend_from_slice(&chunk[..n]);
        }
        remaining(deadline)?;
        self.check().map_err(|_| Outcome::Unknown)?;
        parse_http(&raw)
    }

    fn pause(&mut self, deadline: Instant) -> Result<(), Outcome> {
        self.check()?;
        std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(2)));
        Ok(())
    }

    pub(crate) fn cancellation(&self) -> Cancellation {
        Cancellation {
            lifetime: Arc::clone(&self.lifetime),
            identity: Arc::clone(&self.identity),
        }
    }

    fn finish(&mut self, candidate: Outcome) -> Outcome {
        let lifetime = Arc::clone(&self.lifetime);
        let mut gate = lifetime.gate.lock().unwrap_or_else(|e| e.into_inner());
        let proved = self.check_locked(&mut gate).is_ok()
            && self
                .deadline
                .is_some_and(|deadline| Instant::now() < deadline);
        let Some(r) = &mut gate.reservation else {
            return Outcome::Unknown;
        };
        if !Arc::ptr_eq(&r.identity, &self.identity) {
            return Outcome::Unknown;
        }
        if let Phase::Finished(outcome) = r.phase {
            return outcome;
        }
        let outcome = if matches!(r.phase, Phase::BeforeEffect) {
            Outcome::RefusedBeforeWrite
        } else if proved && candidate != Outcome::RefusedBeforeWrite {
            candidate
        } else if matches!(r.phase, Phase::EffectAttempted) {
            Outcome::Unknown
        } else {
            Outcome::RefusedBeforeWrite
        };
        r.phase = Phase::Finished(outcome);
        r.stream = None;
        outcome
    }

    pub(crate) fn discover(&mut self) -> Result<Vec<BoundTarget>, Outcome> {
        if !self.ready()? {
            return Err(Outcome::Unsupported);
        }
        let (status, body) = self.exchange(None)?;
        if status != 200 {
            return Err(Outcome::Unsupported);
        }
        Ok(targets(&body)?
            .into_iter()
            .map(|target| BoundTarget {
                target,
                binding: self.binding,
                session_identity: Arc::clone(&self.identity),
            })
            .collect())
    }

    fn ready(&mut self) -> Result<bool, Outcome> {
        let (status, body) = self.exchange_request(Request::Capabilities)?;
        if status != 200 || body.len() > 1024 {
            return Err(Outcome::Unsupported);
        }
        let report: Capabilities =
            serde_json::from_slice(&body).map_err(|_| Outcome::Unsupported)?;
        if report.abi != 1 {
            return Err(Outcome::Unsupported);
        }
        Ok(report.ready)
    }

    pub(crate) fn close(
        &mut self,
        selected: BoundTarget,
        _permit: CandidateEffectPermit,
    ) -> Outcome {
        if self.lifetime.gate.lock().is_ok_and(|gate| {
            gate.reservation
                .as_ref()
                .is_some_and(|r| matches!(r.phase, Phase::Finished(_)))
        }) {
            // A session admits one effect only. A later selection cannot
            // inherit the first action's receipt or renew uncertain work.
            return Outcome::RefusedBeforeWrite;
        }
        let result = self.close_inner(selected);
        #[cfg(test)]
        if let Some(barrier) = &self.before_finish {
            barrier.wait();
            barrier.wait();
        }
        self.finish(result)
    }

    fn close_inner(&mut self, selected: BoundTarget) -> Outcome {
        if selected.binding != self.binding
            || !Arc::ptr_eq(&selected.session_identity, &self.identity)
        {
            return Outcome::RefusedBeforeWrite;
        }
        if !self.ready().is_ok_and(|ready| ready) {
            return Outcome::RefusedBeforeWrite;
        }
        match self.exchange(Some(&selected.target)) {
            Ok((204, body)) if body.is_empty() => Outcome::Closed,
            Ok((404, body)) if body.is_empty() => Outcome::Missing,
            Ok((409, body)) if body.is_empty() => Outcome::Changed,
            Ok((400 | 503, body)) if body.is_empty() => Outcome::Unsupported,
            Err(Outcome::RefusedBeforeWrite) => Outcome::RefusedBeforeWrite,
            _ => Outcome::Unknown,
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancellation().cancel();
        let mut gate = self.lifetime.gate.lock().unwrap_or_else(|e| e.into_inner());
        if gate
            .reservation
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(&r.identity, &self.identity))
        {
            gate.reservation = None;
        }
    }
}

/// Inactive scheduler seam: reserve under the owner's short scheduling section,
/// then move only the retained transport out. One worker is admitted until its
/// real exit, even when the result handle is dropped. No public method uses it.
#[derive(Default)]
pub(crate) struct Scheduler {
    active: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    fail_next_spawn: bool,
}
struct Slot(Arc<std::sync::atomic::AtomicBool>);
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
pub(crate) struct Worker {
    cancellation: Cancellation,
    result: std::sync::mpsc::Receiver<Outcome>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Scheduler {
    pub(crate) fn start(
        &mut self,
        mut session: Session,
        selected: BoundTarget,
        permit: CandidateEffectPermit,
    ) -> Result<Worker, Outcome> {
        use std::sync::atomic::Ordering;
        if self
            .active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Outcome::RefusedBeforeWrite);
        }
        let slot = Slot(Arc::clone(&self.active));
        let cancellation = session.cancellation();
        let (sender, result) = std::sync::mpsc::sync_channel(1);
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_spawn) {
            drop(slot);
            return Err(Outcome::RefusedBeforeWrite);
        }
        // Discovery time does not give a detached effect a renewable budget.
        // The inherited whole-session deadline can only shorten this work.
        let thread = std::thread::Builder::new()
            .name("omavless-close-research".into())
            .spawn(move || {
                let _slot = slot;
                let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    session.close(selected, permit)
                })) {
                    Ok(outcome) => outcome,
                    Err(_) => session.finish(Outcome::Unknown),
                };
                // Release retained descriptors and this exact reservation
                // before result publication or worker-slot release.
                drop(session);
                let _ = sender.try_send(outcome);
            })
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        Ok(Worker {
            cancellation,
            result,
            thread: Some(thread),
        })
    }
}
impl Worker {
    pub(crate) fn cancel(&self) {
        self.cancellation.cancel();
    }
    pub(crate) fn poll(&mut self) -> Option<Outcome> {
        let result = self.result.try_recv().ok();
        if self
            .thread
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
        result
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        // Socket shutdown wakes the bounded worker. Never join an unfinished
        // worker in Drop or release its slot early to admit replacements.
        if self
            .thread
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

// Keep exact raw JSON for known-key duplicate rejection, unlike Value parsing.
fn parse_http(raw: &[u8]) -> Result<(u16, Vec<u8>), Outcome> {
    let end = raw
        .windows(4)
        .position(|v| v == b"\r\n\r\n")
        .ok_or(Outcome::Unknown)?;
    if end > 8192 {
        return Err(Outcome::Unknown);
    }
    let head = std::str::from_utf8(&raw[..end]).map_err(|_| Outcome::Unknown)?;
    let mut lines = head.split("\r\n");
    let mut status = lines
        .next()
        .ok_or(Outcome::Unknown)?
        .split_ascii_whitespace();
    if !matches!(status.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
        return Err(Outcome::Unknown);
    }
    let code = status
        .next()
        .filter(|v| v.len() == 3 && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| (100..=599).contains(v))
        .ok_or(Outcome::Unknown)?;
    let mut length = None;
    for line in lines {
        let (key, value) = line.split_once(':').ok_or(Outcome::Unknown)?;
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(Outcome::Unknown);
        }
        if key.eq_ignore_ascii_case("transfer-encoding") {
            return Err(Outcome::Unknown);
        }
        if key.eq_ignore_ascii_case("content-length") {
            if length.is_some()
                || value.trim().is_empty()
                || !value.trim().bytes().all(|b| b.is_ascii_digit())
            {
                return Err(Outcome::Unknown);
            }
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| Outcome::Unknown)?,
            );
        }
    }
    let body = &raw[end + 4..];
    if length.is_some_and(|n| n != body.len()) {
        return Err(Outcome::Unknown);
    }
    Ok((code, body.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    const ID: &str = "11111111-1111-4111-8111-111111111111";
    // These tests own temporary subprocess groups and loopback listeners.
    // Serialize their fixture lifetime; core-side concurrent confirmation is
    // independently exercised by the Go race matrix, not this host fixture.
    static FIXTURE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test]
    fn strict_targets_refuse_duplicates_noncanonical_tokens_and_arbitrary_paths() {
        assert!(
            targets(
                &json!({"connections":[{"id":ID,"omavlessCloseToken":"42"}]})
                    .to_string()
                    .into_bytes()
            )
            .is_ok()
        );
        for token in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            "18446744073709551616",
            "1\r\nHeader: x",
        ] {
            assert!(
                targets(
                    json!({"connections":[{"id":ID,"omavlessCloseToken":token}]})
                        .to_string()
                        .as_bytes()
                )
                .is_err()
            );
        }
        for id in ["", "../connections", "11111111-1111-4111-8111-11111111111A"] {
            assert!(
                targets(
                    json!({"connections":[{"id":id,"omavlessCloseToken":"42"}]})
                        .to_string()
                        .as_bytes()
                )
                .is_err()
            );
        }
        for duplicate in [
            format!(r#"{{"connections":[{{"id":"{ID}","id":"{ID}","omavlessCloseToken":"42"}}]}}"#),
            format!(
                r#"{{"connections":[{{"id":"{ID}","omavlessCloseToken":"42","omavlessCloseToken":"43"}}]}}"#
            ),
            r#"{"connections":[],"connections":[]}"#.into(),
        ] {
            assert!(targets(duplicate.as_bytes()).is_err());
        }
        assert!(targets(json!({"connections":[{"id":ID,"omavlessCloseToken":"42"},{"id":ID,"omavlessCloseToken":"43"}]}).to_string().as_bytes()).is_err());
    }
    #[test]
    fn response_framing_rejects_ambiguous_and_partial_receipts() {
        assert_eq!(
            parse_http(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n"),
            Ok((204, vec![]))
        );
        for raw in [
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\nx".as_slice(),
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 204 No Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
            b"HTTP/1.1 204 No Content\r\nContent-Length: +0\r\n\r\n",
            b"HTTP/1.1 +204 No Content\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 0204 No Content\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 204 No Content\r\nContent-Length : 1\r\n\r\n",
        ] {
            assert!(parse_http(raw).is_err());
        }
    }
    #[test]
    fn private_binding_never_formats_controller_identifiers() {
        let target = BoundTarget {
            target: Target {
                id: ID.into(),
                token: "42".into(),
            },
            binding: Binding {
                pid: 1,
                uid: 1,
                directory: (1, 2),
                socket: (1, 3),
            },
            session_identity: Arc::new(()),
        };
        assert_eq!(format!("{target:?}"), "BoundCloseTarget([private])");
        let output = crate::connection_rows::project(crate::connection_rows::extract(
            &json!({"connections":[{"id":ID,"omavlessCloseToken":"42","metadata":{"host":"example.invalid"}}]}),
        ));
        assert!(output["rows"][0].get("omavlessCloseToken").is_none());
        assert!(output["rows"][0].get("id").is_none());
    }

    #[test]
    fn target_bounds_missing_fields_duplicate_tokens_and_escaped_keys_refuse() {
        for raw in [
            b"{}".as_slice(),
            b"{\"connections\":[{}]}",
            br#"{"connections":[],"connect\u0069ons":[]}"#,
        ] {
            assert!(targets(raw).is_err());
        }
        let other = "22222222-2222-4222-8222-222222222222";
        assert!(targets(json!({"connections":[{"id":ID,"omavlessCloseToken":"42"},{"id":other,"omavlessCloseToken":"42"}]}).to_string().as_bytes()).is_err());
        assert!(
            targets(
                json!({"connections":vec![json!({"id":ID,"omavlessCloseToken":"42"});MAX_ROWS+1]})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
        assert!(targets(&vec![b' '; MAX_SNAPSHOT + 1]).is_err());
        assert!(targets(br#"{"connections":null}"#).unwrap().is_empty());
    }

    #[test]
    fn candidate_core_unix_peer_and_exact_close_optin() {
        use std::net::{TcpListener, TcpStream};
        use std::os::unix::fs::PermissionsExt;
        use std::thread;
        let Some(executable) = std::env::var_os("OMAVLESS_TEST_CONDITIONAL_CORE") else {
            return;
        };
        let _fixture = FIXTURE.lock().unwrap();
        let root = crate::test_temp::directory("cc").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.join("mihomo.sock");
        let config = root.join("config.yaml");
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let mixed = reservation.local_addr().unwrap().port();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let target = listener.local_addr().unwrap().port();
        assert_ne!(mixed, target);
        drop(reservation);
        listener.set_nonblocking(true).unwrap();
        let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&accepted);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut children = Vec::new();
            while children.len() < 2 && Instant::now() < deadline {
                let Ok((mut peer, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                children.push(thread::spawn(move || {
                    peer.set_nonblocking(false).unwrap();
                    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let mut data = [0; 4096];
                    while let Ok(n) = peer.read(&mut data) {
                        if n == 0 || peer.write_all(&data[..n]).is_err() {
                            break;
                        }
                    }
                }));
            }
            for child in children {
                child.join().unwrap();
            }
        });
        fs::write(&config,format!("mixed-port: {mixed}\nexternal-controller-unix: {}\nallow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: silent\ntun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n",socket.display())).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        let mut core = OwnedCore::spawn(Path::new(&executable), &root, &config, &socket).unwrap();
        core.wait_ready(Duration::from_secs(10)).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        {
            let mut startup = Session::bind(&mut core, uid).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !startup.ready().unwrap() {
                assert!(
                    Instant::now() < deadline,
                    "synthetic core did not reach Running"
                );
                thread::sleep(Duration::from_millis(5));
            }
        }
        // /version can be served before initialization finishes. Wait for the
        // actual synthetic config/rules, not a sleep or startup traffic retry.
        let loaded = Instant::now() + Duration::from_secs(5);
        loop {
            let config = omavless_mihomo::controller_get(
                &socket,
                omavless_mihomo::ReadOnlyEndpoint::Configs,
                Duration::from_secs(1),
                16 * 1024,
            );
            let rules = omavless_mihomo::controller_get(
                &socket,
                omavless_mihomo::ReadOnlyEndpoint::Rules,
                Duration::from_secs(1),
                16 * 1024,
            );
            if config.is_ok_and(|r| {
                r.status == 200 && r.payload["mode"] == "direct" && r.payload["mixed-port"] == mixed
            }) && rules.is_ok_and(|r| {
                r.status == 200
                    && r.payload["rules"]
                        .as_array()
                        .is_some_and(|r| r.len() == 1 && r[0]["proxy"] == "DIRECT")
            }) {
                break;
            }
            assert!(
                Instant::now() < loaded,
                "synthetic core configuration did not become ready"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut clients = Vec::new();
        for _ in 0..2 {
            let mut stream = TcpStream::connect(("127.0.0.1", mixed)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .write_all(
                    format!(
                        "CONNECT 127.0.0.1:{target} HTTP/1.1\r\nHost: 127.0.0.1:{target}\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") && header.len() < 4096 {
                let mut byte = [0; 1];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            assert!(header.starts_with(b"HTTP/1.1 200 "));
            stream.write_all(b"probe").unwrap();
            let mut bytes = [0; 5];
            stream.read_exact(&mut bytes).unwrap_or_else(|error| {
                panic!(
                    "synthetic initial echo failed: {error:?}; accepted={}",
                    accepted.load(std::sync::atomic::Ordering::Relaxed)
                )
            });
            assert_eq!(&bytes, b"probe");
            clients.push(stream);
        }
        let mut session = Session::bind(&mut core, uid).unwrap();
        let stale = session.discover().unwrap().remove(0);
        drop(session);
        let mut session = Session::bind(&mut core, uid).unwrap();
        assert_eq!(
            session.close(stale, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        drop(session);
        let mut session = Session::bind(&mut core, uid).unwrap();
        let mut discovered = session.discover().unwrap();
        assert_eq!(discovered.len(), 2);
        let (_, raw) = session.exchange(None).unwrap();
        let data: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let selected = discovered.remove(0);
        let port = data["connections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == selected.target.id)
            .unwrap()["metadata"]["sourcePort"]
            .as_str()
            .unwrap()
            .parse::<u16>()
            .unwrap();
        let index = clients
            .iter()
            .position(|s| s.local_addr().unwrap().port() == port)
            .unwrap();
        let wrong = BoundTarget {
            target: Target {
                id: selected.target.id.clone(),
                token: discovered[0].target.token.clone(),
            },
            binding: selected.binding,
            session_identity: Arc::clone(&selected.session_identity),
        };
        assert_eq!(
            session.close(wrong, CandidateEffectPermit { _private: () }),
            Outcome::Changed
        );
        for client in &mut clients {
            client.write_all(b"still").unwrap();
            let mut data = [0; 5];
            client.read_exact(&mut data).unwrap();
            assert_eq!(&data, b"still");
        }
        let selected_id = selected.target.id;
        drop(session);
        let mut session = Session::bind(&mut core, uid).unwrap();
        let selected = session
            .discover()
            .unwrap()
            .into_iter()
            .find(|target| target.target.id == selected_id)
            .unwrap();
        let mut scheduler = Scheduler::default();
        let mut close_worker = scheduler
            .start(session, selected, CandidateEffectPermit { _private: () })
            .unwrap();
        assert_eq!(worker_result(&mut close_worker), Outcome::Closed);
        let deadline = Instant::now() + Duration::from_secs(1);
        while scheduler.active.load(std::sync::atomic::Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut session = Session::bind(&mut core, uid).unwrap();
        let mut byte = [0; 1];
        assert!(matches!(clients[index].read(&mut byte), Ok(0) | Err(_)));
        let other = 1 - index;
        clients[other].write_all(b"alive").unwrap();
        let mut data = [0; 5];
        clients[other].read_exact(&mut data).unwrap();
        assert_eq!(&data, b"alive");
        // A same-user replacement at the controller name is not fresh proof,
        // even while the original pinned child remains alive.
        fs::rename(&socket, root.join("old.sock")).unwrap();
        let replacement = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            session.discover(),
            Err(Outcome::RefusedBeforeWrite)
        ));
        drop(replacement);
        drop(session);
        drop(clients);
        core.stop(Duration::from_secs(3)).unwrap();
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    const PEER_FIXTURE: &str = r#"#!/usr/bin/python3
import argparse, json, os, socket, time
p=argparse.ArgumentParser();p.add_argument('-d');p.add_argument('-f');a=p.parse_args()
root=a.d; variant=json.load(open(a.f))['variant']
s=socket.socket(socket.AF_UNIX);s.bind(root+'/mihomo.sock');os.chmod(root+'/mihomo.sock',0o600);s.listen(4)
while True:
 c,_=s.accept();c.settimeout(3);raw=b''
 while b'\r\n\r\n' not in raw and len(raw)<8192:
  b=c.recv(1024)
  if not b:break
  raw+=b
 if not raw:c.close();continue
 line=raw.split(b'\r\n',1)[0]
 if line.startswith(b'GET /version '):body=b'{"version":"synthetic"}';head=b'HTTP/1.0 200 OK\r\n'
 elif line.startswith(b'GET /connections/conditional-capabilities '):
  if os.path.exists(root+'/stall-capability'):
   open(root+'/capability-entered','wb').close(); deadline=time.monotonic()+10
   while not os.path.exists(root+'/release') and time.monotonic()<deadline:time.sleep(0.002)
  body={'badabi':b'{"abi":2,"ready":true}','suspended':b'{"abi":1,"ready":false}','duplicated':b'{"abi":1,"abi":1,"ready":true}','extra':b'{"abi":1,"ready":true,"unknown":0}'}.get(variant,b'{"abi":1,"ready":true}')
  if os.path.exists(root+'/suspended'):body=b'{"abi":1,"ready":false}'
  head=b'HTTP/1.0 200 OK\r\n'
 elif line.startswith(b'GET /connections '):body=b'{"connections":[{"id":"11111111-1111-4111-8111-111111111111","omavlessCloseToken":"42"}]}';head=b'HTTP/1.0 200 OK\r\n'
 else:
  with open(root+'/effects','ab') as f:f.write(b'1\n')
  if variant=='stall-reply':
   open(root+'/effect-entered','wb').close(); deadline=time.monotonic()+10
   while not os.path.exists(root+'/release') and time.monotonic()<deadline:time.sleep(0.002)
  if variant=='drop':c.close();continue
  if variant=='partial':c.sendall(b'HTTP/1.0 204 No Content\r\nContent-Length: 1\r\n\r\n');c.close();continue
  if variant=='bad-code':body=b'';head=b'HTTP/1.0 +204 No Content\r\n'
  elif variant=='generic404':body=b'404 page not found';head=b'HTTP/1.0 404 Not Found\r\n'
  else:body=b'';head=b'HTTP/1.0 204 No Content\r\n'
 try:c.sendall(head+b'Content-Length: '+str(len(body)).encode()+b'\r\n\r\n'+body)
 except OSError:pass
 c.close()
"#;

    fn peer_fixture(variant: &str) -> (std::path::PathBuf, OwnedCore) {
        use std::os::unix::fs::PermissionsExt;
        let root = crate::test_temp::directory("cp").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let exe = root.join("core.py");
        fs::write(&exe, PEER_FIXTURE).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        let config = root.join("config.json");
        fs::write(&config, json!({"variant":variant}).to_string()).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        let mut core = OwnedCore::spawn(&exe, &root, &config, &root.join("mihomo.sock")).unwrap();
        core.wait_ready(Duration::from_secs(5)).unwrap();
        (root, core)
    }

    fn wait_marker(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path.exists() {
            assert!(Instant::now() < deadline, "fixed fixture barrier timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn worker_result(worker: &mut Worker) -> Outcome {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            if let Some(result) = worker.poll() {
                return result;
            }
            assert!(
                Instant::now() < deadline,
                "bounded research worker did not finish"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn prepared(core: &mut OwnedCore) -> (Session, BoundTarget) {
        let mut session = Session::bind(core, nix::unistd::Uid::current().as_raw()).unwrap();
        let target = session.discover().unwrap().remove(0);
        (session, target)
    }

    #[test]
    fn detached_stalled_capability_and_effect_reply_allow_owner_status_and_urgent_disconnect() {
        use crate::mutation::{
            BeginOutcome, MutationCoordinator, MutationDigest, MutationKind, MutationRequest,
            MutationResult,
        };
        let _fixture = FIXTURE.lock().unwrap();
        for after_effect in [false, true] {
            let (root, mut core) = peer_fixture(if after_effect { "stall-reply" } else { "ok" });
            let (session, target) = prepared(&mut core);
            if !after_effect {
                fs::write(root.join("stall-capability"), b"fixed").unwrap();
            }
            // Compose the actual mutation scheduler with the actual owned child;
            // the detached session contains neither coordinator nor Child.
            let owner = Mutex::new((core, MutationCoordinator::default(), Scheduler::default()));
            let mut worker = owner
                .lock()
                .unwrap()
                .2
                .start(session, target, CandidateEffectPermit { _private: () })
                .unwrap();
            wait_marker(&root.join(if after_effect {
                "effect-entered"
            } else {
                "capability-entered"
            }));
            let begin = Instant::now();
            let mut guard = owner
                .try_lock()
                .expect("transport must release owner during I/O");
            assert_eq!(guard.1.revision(), 0);
            assert!(guard.0.running().unwrap());
            guard
                .1
                .submit(
                    MutationRequest::new(
                        MutationKind::Other,
                        Some("queued"),
                        None,
                        MutationDigest::new([1; 32]),
                    )
                    .unwrap(),
                )
                .unwrap();
            guard
                .1
                .submit(
                    MutationRequest::new(
                        MutationKind::Disconnect,
                        Some("urgent"),
                        None,
                        MutationDigest::new([2; 32]),
                    )
                    .unwrap(),
                )
                .unwrap();
            let BeginOutcome::Started(active) = guard.1.begin_next().unwrap() else {
                panic!("urgent action not admitted");
            };
            assert_eq!(active.kind, MutationKind::Disconnect);
            guard.0.stop(Duration::from_secs(2)).unwrap();
            guard
                .1
                .finish(active.token, MutationResult::Success)
                .unwrap();
            assert_eq!(guard.1.revision(), 1);
            assert!(
                begin.elapsed() < Duration::from_secs(1),
                "disconnect waited for controller deadline"
            );
            drop(guard);
            assert_eq!(
                worker_result(&mut worker),
                if after_effect {
                    Outcome::Unknown
                } else {
                    Outcome::RefusedBeforeWrite
                }
            );
            if after_effect {
                assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
            } else {
                assert!(!root.join("effects").exists());
            }
            drop(worker);
            drop(owner);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn detached_cancel_before_and_after_attempt_never_resends_or_relabels_unknown() {
        let _fixture = FIXTURE.lock().unwrap();
        for after_effect in [false, true] {
            let (root, mut core) = peer_fixture(if after_effect { "stall-reply" } else { "ok" });
            let (session, target) = prepared(&mut core);
            if !after_effect {
                fs::write(root.join("stall-capability"), b"fixed").unwrap();
            }
            let mut scheduler = Scheduler::default();
            let mut worker = scheduler
                .start(session, target, CandidateEffectPermit { _private: () })
                .unwrap();
            wait_marker(&root.join(if after_effect {
                "effect-entered"
            } else {
                "capability-entered"
            }));
            worker.cancel();
            let outcome = worker_result(&mut worker);
            assert_eq!(
                outcome,
                if after_effect {
                    Outcome::Unknown
                } else {
                    Outcome::RefusedBeforeWrite
                }
            );
            fs::write(root.join("release"), b"fixed").unwrap();
            worker.cancel();
            assert!(worker.poll().is_none());
            core.stop(Duration::from_secs(2)).unwrap();
            assert_eq!(
                fs::read(root.join("effects")).unwrap_or_default(),
                if after_effect {
                    b"1\n".to_vec()
                } else {
                    vec![]
                }
            );
            drop(worker);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn detached_receipt_completion_and_cancel_have_one_atomic_winner() {
        let _fixture = FIXTURE.lock().unwrap();
        for cancel_before_finish in [false, true] {
            let (root, mut core) = peer_fixture("ok");
            let (mut session, target) = prepared(&mut core);
            let barrier = Arc::new(std::sync::Barrier::new(2));
            session.before_finish = Some(Arc::clone(&barrier));
            let cancellation = session.cancellation();
            let lifetime = Arc::clone(&session.lifetime);
            let mut scheduler = Scheduler::default();
            let mut worker = scheduler
                .start(session, target, CandidateEffectPermit { _private: () })
                .unwrap();
            barrier.wait(); // complete real empty 204 parsed, final proof pending
            if cancel_before_finish {
                cancellation.cancel();
            }
            barrier.wait();
            let result = worker_result(&mut worker);
            assert_eq!(
                result,
                if cancel_before_finish {
                    Outcome::Unknown
                } else {
                    Outcome::Closed
                }
            );
            cancellation.cancel();
            lifetime.revoke();
            assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
            core.stop(Duration::from_secs(2)).unwrap();
            drop(worker);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn finished_session_cannot_attribute_prior_receipt_to_another_selection() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (mut session, target) = prepared(&mut core);
        let other = BoundTarget {
            target: Target {
                id: "22222222-2222-4222-8222-222222222222".into(),
                token: "43".into(),
            },
            binding: target.binding,
            session_identity: Arc::clone(&target.session_identity),
        };
        assert_eq!(
            session.close(target, CandidateEffectPermit { _private: () }),
            Outcome::Closed
        );
        assert_eq!(
            session.close(other, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
        core.stop(Duration::from_secs(2)).unwrap();
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn attempted_effect_cannot_become_prewrite_refusal_after_transient_path_restoration() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("stall-reply");
        let (mut session, target) = prepared(&mut core);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        session.before_finish = Some(Arc::clone(&barrier));
        let mut scheduler = Scheduler::default();
        let mut worker = scheduler
            .start(session, target, CandidateEffectPermit { _private: () })
            .unwrap();
        wait_marker(&root.join("effect-entered"));
        fs::rename(root.join("mihomo.sock"), root.join("old.sock")).unwrap();
        let replacement = std::os::unix::net::UnixListener::bind(root.join("mihomo.sock")).unwrap();
        fs::set_permissions(root.join("mihomo.sock"), fs::Permissions::from_mode(0o600)).unwrap();
        barrier.wait(); // post-write identity failure observed, finish paused
        drop(replacement);
        fs::remove_file(root.join("mihomo.sock")).unwrap();
        fs::rename(root.join("old.sock"), root.join("mihomo.sock")).unwrap();
        barrier.wait(); // final proof sees the restored exact inode
        assert_eq!(worker_result(&mut worker), Outcome::Unknown);
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
        core.stop(Duration::from_secs(2)).unwrap();
        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parsed_receipt_after_whole_deadline_remains_unknown() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (mut session, target) = prepared(&mut core);
        let deadline = Instant::now() + Duration::from_millis(500);
        session.deadline = Some(deadline);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        session.before_finish = Some(Arc::clone(&barrier));
        let mut scheduler = Scheduler::default();
        let mut worker = scheduler
            .start(session, target, CandidateEffectPermit { _private: () })
            .unwrap();
        barrier.wait();
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(5),
        );
        barrier.wait();
        assert_eq!(worker_result(&mut worker), Outcome::Unknown);
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
        core.stop(Duration::from_secs(2)).unwrap();
        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn actual_partial_write_then_restored_identity_cannot_become_prewrite_refusal() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (mut session, target) = prepared(&mut core);
        let written = Arc::new(std::sync::Barrier::new(2));
        let finishing = Arc::new(std::sync::Barrier::new(2));
        // Test-only chunk size performs a real 16-byte UnixStream write; it
        // does not inject an outcome or replace the actual transport.
        session.effect_chunk = 16;
        session.after_chunk = Some(Arc::clone(&written));
        session.before_finish = Some(Arc::clone(&finishing));
        let mut scheduler = Scheduler::default();
        let mut worker = scheduler
            .start(session, target, CandidateEffectPermit { _private: () })
            .unwrap();
        written.wait();
        fs::rename(root.join("mihomo.sock"), root.join("old.sock")).unwrap();
        let replacement = std::os::unix::net::UnixListener::bind(root.join("mihomo.sock")).unwrap();
        fs::set_permissions(root.join("mihomo.sock"), fs::Permissions::from_mode(0o600)).unwrap();
        written.wait();
        finishing.wait(); // second chunk refused after the real first attempt
        drop(replacement);
        fs::remove_file(root.join("mihomo.sock")).unwrap();
        fs::rename(root.join("old.sock"), root.join("mihomo.sock")).unwrap();
        finishing.wait();
        assert_eq!(worker_result(&mut worker), Outcome::Unknown);
        wait_marker(&root.join("effects"));
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
        assert!(
            core.conditional_lifetime()
                .unwrap()
                .gate
                .lock()
                .unwrap()
                .reservation
                .is_none()
        );
        core.stop(Duration::from_secs(2)).unwrap();
        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn expired_admission_before_write_gate_sends_zero_effect_bytes() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (mut session, target) = prepared(&mut core);
        let deadline = Instant::now() + Duration::from_millis(500);
        session.deadline = Some(deadline);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        session.before_write = Some(Arc::clone(&barrier));
        let mut scheduler = Scheduler::default();
        let mut worker = scheduler
            .start(session, target, CandidateEffectPermit { _private: () })
            .unwrap();
        barrier.wait(); // authenticated stream ready, fast write gate pending
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(5),
        );
        barrier.wait();
        assert_eq!(worker_result(&mut worker), Outcome::RefusedBeforeWrite);
        assert!(!root.join("effects").exists());
        core.stop(Duration::from_secs(2)).unwrap();
        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn owner_observed_child_proof_loss_permanently_revokes_retained_lifetime() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (mut session, target) = prepared(&mut core);
        let pid = nix::unistd::Pid::from_raw(i32::try_from(core.pid().unwrap()).unwrap());
        nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL).unwrap();
        nix::sys::wait::waitpid(pid, None).unwrap();
        assert!(core.running().is_err());
        let mut gate = session.lifetime.gate.lock().unwrap();
        assert!(!gate.live);
        assert!(gate.reservation.as_ref().unwrap().cancelled);
        // Once revoked, observation short-circuits before any numeric PID use.
        assert!(!session.lifetime.running(&mut gate));
        drop(gate);
        assert_eq!(
            session.close(target, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        assert!(!root.join("effects").exists());
        drop(session);
        drop(core);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detached_stop_and_new_child_same_path_cannot_revive_old_identity_or_selection() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (mut old, selected) = prepared(&mut core);
        let pin = Arc::clone(&old.lifetime);
        core.stop(Duration::from_secs(2)).unwrap();
        assert!(!pin.gate.lock().unwrap().live);
        fs::remove_file(root.join("mihomo.sock")).unwrap();
        let mut replacement = OwnedCore::spawn(
            &root.join("core.py"),
            &root,
            &root.join("config.json"),
            &root.join("mihomo.sock"),
        )
        .unwrap();
        replacement.wait_ready(Duration::from_secs(3)).unwrap();
        assert!(old.discover().is_err());
        assert_eq!(
            old.close(selected, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        assert!(!root.join("effects").exists());
        drop(old);
        let (mut session, target) = prepared(&mut replacement);
        assert!(!Arc::ptr_eq(&pin, &session.lifetime));
        assert_eq!(
            session.close(target, CandidateEffectPermit { _private: () }),
            Outcome::Closed
        );
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
        replacement.stop(Duration::from_secs(2)).unwrap();
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detached_unexpected_reap_and_replaced_controller_refuse_without_effects() {
        let _fixture = FIXTURE.lock().unwrap();
        for foreign_reap in [false, true] {
            let (root, mut core) = peer_fixture("ok");
            let (mut session, target) = prepared(&mut core);
            if foreign_reap {
                let pid = nix::unistd::Pid::from_raw(i32::try_from(core.pid().unwrap()).unwrap());
                nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL).unwrap();
                nix::sys::wait::waitpid(pid, None).unwrap();
            } else {
                fs::rename(root.join("mihomo.sock"), root.join("retained.sock")).unwrap();
                let replacement =
                    std::os::unix::net::UnixListener::bind(root.join("mihomo.sock")).unwrap();
                fs::set_permissions(root.join("mihomo.sock"), fs::Permissions::from_mode(0o600))
                    .unwrap();
                assert_eq!(
                    session.close(target, CandidateEffectPermit { _private: () }),
                    Outcome::RefusedBeforeWrite
                );
                drop(replacement);
                drop(session);
                core.stop(Duration::from_secs(2)).unwrap();
                assert!(!root.join("effects").exists());
                fs::remove_dir_all(root).unwrap();
                continue;
            }
            assert_eq!(
                session.close(target, CandidateEffectPermit { _private: () }),
                Outcome::RefusedBeforeWrite
            );
            assert!(
                !session.lifetime.gate.lock().unwrap().live,
                "transport proof loss is sticky before owner stop"
            );
            assert!(!root.join("effects").exists());
            assert!(core.stop(Duration::from_secs(2)).is_err());
            assert!(!session.lifetime.gate.lock().unwrap().live);
            drop(session);
            drop(core);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn detached_whole_deadline_and_owned_core_drop_do_not_wait_for_io_or_retry() {
        let _fixture = FIXTURE.lock().unwrap();
        for teardown in [false, true] {
            let (root, mut core) = peer_fixture("stall-reply");
            let (mut session, target) = prepared(&mut core);
            if !teardown {
                session.deadline = Some(Instant::now() + Duration::from_millis(100));
            }
            let mut scheduler = Scheduler::default();
            let mut worker = scheduler
                .start(session, target, CandidateEffectPermit { _private: () })
                .unwrap();
            wait_marker(&root.join("effect-entered"));
            let begin = Instant::now();
            if teardown {
                drop(core);
                assert!(begin.elapsed() < Duration::from_secs(1));
                assert_eq!(worker_result(&mut worker), Outcome::Unknown);
            } else {
                assert_eq!(worker_result(&mut worker), Outcome::Unknown);
                assert!(begin.elapsed() < Duration::from_secs(1));
                core.stop(Duration::from_secs(2)).unwrap();
            }
            assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
            drop(worker);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn detached_spawn_panic_and_dropped_worker_cleanup_are_bounded_and_exact() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (session, target) = prepared(&mut core);
        let mut scheduler = Scheduler {
            fail_next_spawn: true,
            ..Scheduler::default()
        };
        assert!(
            scheduler
                .start(session, target, CandidateEffectPermit { _private: () })
                .is_err()
        );
        assert!(!scheduler.active.load(std::sync::atomic::Ordering::Acquire));
        assert!(
            core.conditional_lifetime()
                .unwrap()
                .gate
                .lock()
                .unwrap()
                .reservation
                .is_none()
        );
        let (mut session, target) = prepared(&mut core);
        session.panic_after_write = true;
        let mut worker = scheduler
            .start(session, target, CandidateEffectPermit { _private: () })
            .unwrap();
        assert_eq!(worker_result(&mut worker), Outcome::Unknown);
        let deadline = Instant::now() + Duration::from_secs(1);
        while scheduler.active.load(std::sync::atomic::Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        let (session, target) = prepared(&mut core);
        fs::write(root.join("stall-capability"), b"fixed").unwrap();
        let worker = scheduler
            .start(session, target, CandidateEffectPermit { _private: () })
            .unwrap();
        wait_marker(&root.join("capability-entered"));
        let begin = Instant::now();
        drop(worker);
        assert!(begin.elapsed() < Duration::from_millis(100));
        let deadline = Instant::now() + Duration::from_secs(1);
        while scheduler.active.load(std::sync::atomic::Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
        core.stop(Duration::from_secs(2)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_ambiguous_or_suspended_abi_never_gets_an_effect() {
        let _fixture = FIXTURE.lock().unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        for variant in ["badabi", "suspended", "duplicated", "extra"] {
            let (root, mut core) = peer_fixture(variant);
            let mut session = Session::bind(&mut core, uid).unwrap();
            assert!(session.discover().is_err(), "{variant}");
            assert!(!root.join("effects").exists());
            drop(session);
            core.stop(Duration::from_secs(2)).unwrap();
            fs::remove_dir_all(root).unwrap();
        }
        let (root, mut core) = peer_fixture("ok");
        let mut session = Session::bind(&mut core, uid).unwrap();
        let target = session.discover().unwrap().remove(0);
        fs::write(root.join("suspended"), b"synthetic").unwrap();
        assert_eq!(
            session.close(target, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        assert!(!root.join("effects").exists());
        drop(session);
        core.stop(Duration::from_secs(2)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lost_partial_malformed_and_generic_reply_are_unknown_and_never_resent() {
        let _fixture = FIXTURE.lock().unwrap();
        for variant in ["drop", "partial", "bad-code", "generic404"] {
            let (root, mut core) = peer_fixture(variant);
            let mut session =
                Session::bind(&mut core, nix::unistd::Uid::current().as_raw()).unwrap();
            let target = session.discover().unwrap().remove(0);
            assert_eq!(
                session.close(target, CandidateEffectPermit { _private: () }),
                Outcome::Unknown
            );
            assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n");
            drop(session);
            core.stop(Duration::from_secs(3)).unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn stale_session_target_and_dead_waitable_child_send_no_effect() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("success");
        let uid = nix::unistd::Uid::current().as_raw();
        let mut first = Session::bind(&mut core, uid).unwrap();
        let target = first.discover().unwrap().remove(0);
        drop(first);
        let mut second = Session::bind(&mut core, uid).unwrap();
        assert_eq!(
            second.close(target, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        assert!(!root.join("effects").exists());
        drop(second);
        let mut second = Session::bind(&mut core, uid).unwrap();
        let target = second.discover().unwrap().remove(0);
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(i32::try_from(second.binding.pid).unwrap()),
            nix::sys::signal::Signal::SIGTERM,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while core.running().unwrap() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            second.close(target, CandidateEffectPermit { _private: () }),
            Outcome::RefusedBeforeWrite
        );
        assert!(!root.join("effects").exists());
        drop(second);
        core.stop(Duration::from_secs(3)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
