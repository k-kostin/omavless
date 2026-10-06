// SPDX-License-Identifier: MIT
//! Inactive fixed conditional transport bound to a parent-owned waitable child.
//! Default builds cannot construct the candidate effect permit. The explicit
//! developer feature admits one separately provisioned pair, never release
//! package authority. Owner confirmation/revision admission remains mandatory.
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
use std::io::{Read, Seek, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

// Passive package-object research and a separately feature-gated developer pair.
#[path = "conditional_package_evidence.rs"]
mod package_evidence;

pub(crate) const MAX_ROWS: usize = 128;
const MAX_SNAPSHOT: usize = 256 * 1024;
const MAX_REPLY: usize = 16 * 1024;
const BUDGET: Duration = Duration::from_secs(3);
#[cfg(feature = "product-image-witness")]
const RETIREMENT_BUDGET: Duration = Duration::from_secs(3);
const PROOF_DRAIN_BUDGET: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Closed,
    Missing,
    Changed,
    Unsupported,
    Unknown,
    RefusedBeforeWrite,
}

/// No default-production constructor, Clone, formatting or deserialization.
/// Developer construction never replaces owner confirmation or release proof.
pub(crate) struct CandidateEffectPermit {
    _private: (),
}
#[cfg(test)]
impl CandidateEffectPermit {
    pub(crate) fn owned_fixture() -> Self {
        Self { _private: () }
    }
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
    #[serde(default)]
    metadata: Option<WireMetadata>,
    #[serde(default)]
    chains: serde_json::Value,
}

#[derive(Deserialize)]
struct WireMetadata {
    #[serde(default)]
    host: serde_json::Value,
    #[serde(default, rename = "destinationIP")]
    ip: serde_json::Value,
    #[serde(default, rename = "destinationPort")]
    port: serde_json::Value,
    #[serde(default)]
    network: serde_json::Value,
}

struct Target {
    id: String,
    token: String,
}

/// Identity and bounded display come from the same strictly decoded row.
pub(crate) struct ObservedRow {
    pub(crate) target: BoundTarget,
    pub(crate) display: serde_json::Value,
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
    Configs,
    Proxies,
    Rules,
    Providers,
    Close(&'a Target),
}
pub(crate) struct BoundTarget {
    target: Target,
    binding: Binding,
    session_identity: Arc<()>,
}
impl BoundTarget {
    fn same_selection(&self, other: &Self) -> bool {
        self.binding == other.binding
            && Arc::ptr_eq(&self.session_identity, &other.session_identity)
            && self.target.id == other.target.id
            && self.target.token == other.target.token
    }
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
    Ok(snapshot_rows(raw)?
        .into_iter()
        .map(|(target, _)| target)
        .collect())
}

fn snapshot_rows(raw: &[u8]) -> Result<Vec<(Target, serde_json::Value)>, Outcome> {
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
        let metadata = row.metadata.map(|metadata| {
            serde_json::json!({
                "host":metadata.host,"destinationIP":metadata.ip,
                "destinationPort":metadata.port,"network":metadata.network,
            })
        });
        let display = crate::connection_rows::project(crate::connection_rows::extract(
            &serde_json::json!({"connections":[{"metadata":metadata,"chains":row.chains}]}),
        ));
        if display["availability"] != "observed" {
            return Err(Outcome::Unsupported);
        }
        result.push((
            Target {
                id: row.id,
                token: row.token,
            },
            display["rows"][0].clone(),
        ));
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    inode: (u64, u64),
    size: u64,
    mode: u32,
    owner: u32,
    change: (i64, i64),
    modified: (i64, i64),
}
impl FileIdentity {
    fn capture(metadata: &Metadata) -> Option<Self> {
        (metadata.is_file()
            && metadata.nlink() == 1
            && metadata.len() != 0
            && metadata.len() <= 128 * 1024 * 1024
            && metadata.mode() & 0o022 == 0)
            .then_some(Self {
                inode: inode(metadata),
                size: metadata.len(),
                mode: metadata.mode(),
                owner: metadata.uid(),
                change: (metadata.ctime(), metadata.ctime_nsec()),
                modified: (metadata.mtime(), metadata.mtime_nsec()),
            })
    }
    fn matches(self, file: &File) -> bool {
        file.metadata()
            .ok()
            .and_then(|metadata| Self::capture(&metadata))
            == Some(self)
    }
}

struct ExecutableEvidence {
    image: File,
    source: File,
    image_identity: FileIdentity,
    source_identity: FileIdentity,
    source_path: PathBuf,
    digests: Option<([u8; 32], [u8; 32])>,
}
/// A single current-image observation owned by one original proof flight.
/// No Clone/Debug, stored permission bit, wire constructor or cross-flight reuse.
struct CurrentImage {
    file: File,
    original: Arc<()>,
    pid: u32,
}
#[cfg(feature = "developer-image-witness")]
#[derive(Clone, Copy)]
enum WitnessClass {
    #[cfg(test)]
    Tests,
    InstalledRuntime,
    #[cfg(feature = "product-image-witness")]
    Product,
}
#[cfg(feature = "developer-image-witness")]
enum ImageWitness {
    Pending {
        child: Option<std::os::fd::OwnedFd>,
        class: WitnessClass,
    },
    Bound(omavless_image_witness::Client),
    Refused,
}
#[cfg(feature = "developer-image-witness")]
struct PendingExecutable {
    source: File,
    identity: FileIdentity,
    path: PathBuf,
}
#[cfg(all(test, feature = "developer-image-witness"))]
type ImageProbe = Box<dyn FnMut(Instant) -> Result<File, Outcome> + Send>;
#[cfg(all(test, feature = "developer-image-witness"))]
type ImageFinishProbe = Box<dyn FnMut(Instant) -> Result<(), Outcome> + Send>;
impl ExecutableEvidence {
    fn image(pid: u32) -> Option<File> {
        // Follow this kernel-owned exe link only for the retained Live child.
        // Inaccessible proc evidence (including capabilities) simply refuses.
        open(
            Path::new(&format!("/proc/{pid}/exe")),
            OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
            Mode::empty(),
        )
        .ok()
        .map(File::from)
    }
    fn check_held(&self) -> bool {
        self.image_identity.matches(&self.image)
            && self.source_identity.matches(&self.source)
            && fs::symlink_metadata(&self.source_path)
                .ok()
                .filter(|metadata| !metadata.file_type().is_symlink())
                .and_then(|metadata| FileIdentity::capture(&metadata))
                == Some(self.source_identity)
    }
    fn check(&self, pid: u32) -> bool {
        self.check_held()
            && Self::image(pid).is_some_and(|image| self.image_identity.matches(&image))
    }
    fn check_current(&self, pid: u32, original: &Arc<()>, current: Option<&CurrentImage>) -> bool {
        match current {
            Some(current) => {
                current.pid == pid
                    && Arc::ptr_eq(&current.original, original)
                    && self.check_held()
                    && self.image_identity.matches(&current.file)
                    && current.file.metadata().ok().map(|m| m.gid())
                        == self.image.metadata().ok().map(|m| m.gid())
            }
            None => self.check(pid),
        }
    }
    fn hash(file: &mut File, identity: FileIdentity, deadline: Instant) -> Option<[u8; 32]> {
        use sha2::{Digest, Sha256};
        if !identity.matches(file) || file.rewind().is_err() {
            return None;
        }
        let mut hasher = Sha256::new();
        let mut block = [0; 65536];
        let mut total = 0_u64;
        loop {
            remaining(deadline).ok()?;
            let count = file.read(&mut block).ok()?;
            if count == 0 {
                break;
            }
            total = total.checked_add(count as u64)?;
            if total > identity.size {
                return None;
            }
            hasher.update(&block[..count]);
        }
        (total == identity.size && identity.matches(file) && remaining(deadline).is_ok())
            .then(|| hasher.finalize().into())
    }
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
    #[cfg(feature = "product-image-witness")]
    Retiring,
    Finished(Outcome),
}
impl Phase {
    fn accepts_proof(self) -> bool {
        match self {
            Self::BeforeEffect | Self::EffectAttempted => true,
            Self::Finished(_) => false,
            #[cfg(feature = "product-image-witness")]
            Self::Retiring => false,
        }
    }
}
struct Reservation {
    identity: Arc<()>,
    phase: Phase,
    cancelled: bool,
    stream: Option<UnixStream>,
    proofs: usize,
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
    proof_released: Condvar,
}
impl Lifetime {
    pub(crate) fn new(pid: u32) -> Self {
        Self {
            pid,
            gate: Mutex::new(Gate {
                live: true,
                reservation: None,
            }),
            proof_released: Condvar::new(),
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
    #[cfg(feature = "product-image-witness")]
    pub(crate) fn same_epoch(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity) && Arc::ptr_eq(&self.lifetime, &other.lifetime)
    }
    #[cfg(feature = "product-image-witness")]
    pub(crate) fn epoch_lifetime_available(&self) -> bool {
        self.lifetime.gate.lock().is_ok_and(|gate| gate.live)
    }
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

    /// Owner admission only: drain this exact session's short durable-proof
    /// lease, never a socket wait, thread exit or another session. The CV wait
    /// releases the gate. A timeout leaves cancellation permanent and permits
    /// the ordinary migration-lock admission to return honest Busy.
    pub(crate) fn cancel_and_drain(&self) -> bool {
        self.cancel();
        let deadline = Instant::now() + PROOF_DRAIN_BUDGET;
        let mut gate = self.lifetime.gate.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !gate
                .reservation
                .as_ref()
                .is_some_and(|r| Arc::ptr_eq(&r.identity, &self.identity) && r.proofs != 0)
            {
                return true;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            let (next, _) = self
                .lifetime
                .proof_released
                .wait_timeout(gate, remaining)
                .unwrap_or_else(|e| e.into_inner());
            gate = next;
        }
    }

    #[cfg(test)]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.lifetime
            .gate
            .lock()
            .unwrap()
            .reservation
            .as_ref()
            .is_none_or(|r| !Arc::ptr_eq(&r.identity, &self.identity) || r.cancelled)
    }
}

/// Count begins under the same gate as cancellation, before any durable lease
/// acquisition. Clearing it acknowledges that the REAL lease is already gone.
struct ProofFlight {
    lifetime: Arc<Lifetime>,
    identity: Arc<()>,
    current: Option<CurrentImage>,
}
impl Drop for ProofFlight {
    fn drop(&mut self) {
        // The observation is gone before cancellation sees the flight drained.
        drop(self.current.take());
        let mut gate = self.lifetime.gate.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = &mut gate.reservation
            && Arc::ptr_eq(&r.identity, &self.identity)
        {
            r.proofs = r.proofs.saturating_sub(1);
        }
        self.lifetime.proof_released.notify_all();
    }
}
struct EffectLease {
    lease: Option<crate::cutover::MigrationLock>,
    flight: Option<ProofFlight>,
}
impl Drop for EffectLease {
    fn drop(&mut self) {
        drop(self.lease.take());
        drop(self.flight.take());
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
    effect_proof: Option<crate::native_coordinator::connection_close::EffectProof>,
    confirmation_expiry: Option<Instant>,
    executable: Option<ExecutableEvidence>,
    #[cfg(feature = "developer-image-witness")]
    image_witness: Option<ImageWitness>,
    #[cfg(feature = "developer-image-witness")]
    pending_executable: Option<PendingExecutable>,
    #[cfg(feature = "product-image-witness")]
    product_witness_finished: bool,
    #[cfg(all(test, feature = "developer-image-witness"))]
    image_probe: Option<ImageProbe>,
    #[cfg(all(test, feature = "developer-image-witness"))]
    image_finish_probe: Option<ImageFinishProbe>,
    #[cfg(all(test, feature = "product-image-witness"))]
    retirement_budget: Option<Duration>,
    #[cfg(feature = "developer-conditional-close")]
    developer_pair: Option<package_evidence::developer_pair::Evidence>,
    #[cfg(feature = "developer-conditional-close")]
    qualified_pair: Option<package_evidence::release_pair::Evidence>,
    #[cfg(feature = "developer-conditional-close")]
    qualification_attempted: bool,
    owned_observation: Option<crate::native_host::CloseFacts>,
    expected_display: Option<serde_json::Value>,
    #[cfg(test)]
    proof_pause: Option<(usize, Arc<std::sync::Barrier>)>,
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
            effect_proof: None,
            confirmation_expiry: None,
            executable: None,
            #[cfg(feature = "developer-image-witness")]
            image_witness: None,
            #[cfg(feature = "developer-image-witness")]
            pending_executable: None,
            #[cfg(feature = "product-image-witness")]
            product_witness_finished: false,
            #[cfg(all(test, feature = "developer-image-witness"))]
            image_probe: None,
            #[cfg(all(test, feature = "developer-image-witness"))]
            image_finish_probe: None,
            #[cfg(all(test, feature = "product-image-witness"))]
            retirement_budget: None,
            #[cfg(feature = "developer-conditional-close")]
            developer_pair: None,
            #[cfg(feature = "developer-conditional-close")]
            qualified_pair: None,
            #[cfg(feature = "developer-conditional-close")]
            qualification_attempted: false,
            owned_observation: None,
            expected_display: None,
            #[cfg(test)]
            proof_pause: None,
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
                proofs: 0,
            });
        }
        session.check()?;
        Ok(session)
    }

    fn check(&mut self) -> Result<(), Outcome> {
        #[cfg(feature = "developer-image-witness")]
        if self.image_witness_selected() {
            let flight = self.image_proof_flight()?;
            return self.check_with_flight(&flight);
        }
        #[cfg(feature = "developer-conditional-close")]
        if self.developer_pair.is_some() || self.qualified_pair.is_some() {
            let flight = self.begin_proof_flight()?;
            return self.check_with_flight(&flight);
        }
        let lifetime = Arc::clone(&self.lifetime);
        let mut gate = lifetime
            .gate
            .lock()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        self.check_locked(&mut gate)
    }

    fn begin_proof_flight(&self) -> Result<ProofFlight, Outcome> {
        let mut gate = self
            .lifetime
            .gate
            .lock()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        self.check_locked(&mut gate)?;
        let r = gate
            .reservation
            .as_mut()
            .ok_or(Outcome::RefusedBeforeWrite)?;
        if r.proofs != 0 {
            return Err(Outcome::RefusedBeforeWrite);
        }
        r.proofs = 1;
        Ok(ProofFlight {
            lifetime: Arc::clone(&self.lifetime),
            identity: Arc::clone(&self.identity),
            current: None,
        })
    }

    fn revoke_image(&self) {
        self.lifetime
            .gate
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revoke();
    }

    fn image_witness_selected(&self) -> bool {
        #[cfg(feature = "developer-image-witness")]
        if self.image_witness.is_some() {
            return true;
        }
        #[cfg(all(test, feature = "developer-image-witness"))]
        if self.image_probe.is_some() {
            return true;
        }
        false
    }

    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn install_image_probe_for_test(&mut self, probe: ImageProbe) {
        assert!(self.image_witness.is_none() && self.image_probe.is_none());
        self.image_probe = Some(probe);
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn original_image_for_test(&self) -> File {
        self.executable.as_ref().unwrap().image.try_clone().unwrap()
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn capture_probe_image_for_test(&mut self) {
        let path = fs::read_link(format!("/proc/{}/exe", self.binding.pid)).unwrap();
        self.capture_executable(&path).unwrap();
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn image_gate_available_for_test(&self) -> bool {
        self.lifetime.gate.try_lock().is_ok()
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn image_gate_checker_for_test(&self) -> impl Fn() -> (bool, bool) + Send + 'static {
        let lifetime = Arc::clone(&self.lifetime);
        move || match lifetime.gate.try_lock() {
            Ok(gate) => (
                true,
                gate.reservation
                    .as_ref()
                    .is_some_and(|r| matches!(r.phase, Phase::EffectAttempted)),
            ),
            Err(_) => (false, false),
        }
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn image_deadline_for_test(&mut self, until: Instant) {
        self.deadline = Some(until);
    }
    #[cfg(all(test, feature = "product-image-witness"))]
    pub(crate) fn retirement_budget_for_test(&mut self, budget: Duration) {
        assert!(budget <= RETIREMENT_BUDGET);
        self.retirement_budget = Some(budget);
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn pause_before_finish(&mut self, barrier: Arc<std::sync::Barrier>) {
        self.before_finish = Some(barrier);
    }
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn install_image_finish_probe_for_test(&mut self, probe: ImageFinishProbe) {
        self.image_finish_probe = Some(probe);
    }

    /// Existing counted flight starts before RPC. No owner/gate/migration lease
    /// spans the original private channel. A late valid FD never wins cancel.
    fn image_proof_flight(&mut self) -> Result<ProofFlight, Outcome> {
        #[allow(unused_mut)]
        let mut flight = self.begin_proof_flight()?;
        #[cfg(feature = "developer-image-witness")]
        if self.image_witness_selected() {
            let deadline = *self.deadline.get_or_insert_with(|| Instant::now() + BUDGET);
            let result = (|| {
                remaining(deadline)?;
                #[cfg(test)]
                let file = if let Some(probe) = self.image_probe.as_mut() {
                    Some(probe(deadline)?)
                } else {
                    None
                };
                #[cfg(not(test))]
                let file: Option<File> = None;
                let file = if let Some(file) = file {
                    file
                } else {
                    if let Some(ImageWitness::Pending { child, class }) = &mut self.image_witness {
                        let original = child.take().ok_or(Outcome::RefusedBeforeWrite)?;
                        let class = *class;
                        // Consume before constructor effects; no failed bind is retried.
                        self.image_witness = Some(ImageWitness::Refused);
                        let client = match class {
                            #[cfg(test)]
                            WitnessClass::Tests => {
                                omavless_image_witness::Client::bind_original(original, deadline)
                            }
                            WitnessClass::InstalledRuntime => {
                                omavless_image_witness::Client::bind_original_runtime(
                                    original, deadline,
                                )
                            }
                            #[cfg(feature = "product-image-witness")]
                            WitnessClass::Product => {
                                omavless_image_witness::Client::bind_original_product(
                                    original, deadline,
                                )
                            }
                        }
                        .map_err(|_| Outcome::RefusedBeforeWrite)?;
                        self.image_witness = Some(ImageWitness::Bound(client));
                    }
                    let Some(ImageWitness::Bound(client)) = &mut self.image_witness else {
                        return Err(Outcome::RefusedBeforeWrite);
                    };
                    client
                        .observe(deadline)
                        .map_err(|_| Outcome::RefusedBeforeWrite)?
                };
                flight.current = Some(CurrentImage {
                    file,
                    original: Arc::clone(&self.identity),
                    pid: self.binding.pid,
                });
                let current = flight.current.as_ref().ok_or(Outcome::RefusedBeforeWrite)?;
                if let Some(pending) = &self.pending_executable {
                    if !pending.identity.matches(&pending.source)
                        || !pending.identity.matches(&current.file)
                        || fs::symlink_metadata(&pending.path)
                            .ok()
                            .and_then(|m| FileIdentity::capture(&m))
                            != Some(pending.identity)
                    {
                        return Err(Outcome::RefusedBeforeWrite);
                    }
                } else if !self.executable.as_ref().is_some_and(|image| {
                    image.check_current(self.binding.pid, &self.identity, Some(current))
                }) {
                    return Err(Outcome::RefusedBeforeWrite);
                }
                remaining(deadline)?;
                let mut gate = self
                    .lifetime
                    .gate
                    .lock()
                    .map_err(|_| Outcome::RefusedBeforeWrite)?;
                self.check_locked(&mut gate)
            })();
            if result.is_err() {
                self.revoke_image();
                return Err(Outcome::RefusedBeforeWrite);
            }
        }
        Ok(flight)
    }

    fn check_with_flight(&self, flight: &ProofFlight) -> Result<(), Outcome> {
        #[cfg(feature = "developer-conditional-close")]
        self.check_developer_pair(flight.current.as_ref())?;
        #[cfg(feature = "developer-conditional-close")]
        self.check_qualified_pair(flight.current.as_ref())?;
        let mut gate = self
            .lifetime
            .gate
            .lock()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        self.check_locked_current(&mut gate, flight.current.as_ref())
    }

    fn check_locked_current(
        &self,
        gate: &mut Gate,
        current: Option<&CurrentImage>,
    ) -> Result<(), Outcome> {
        self.check_locked(gate)?;
        #[cfg(feature = "developer-image-witness")]
        if self.image_witness_selected()
            && (!current.is_some_and(|c| {
                c.pid == self.binding.pid && Arc::ptr_eq(&c.original, &self.identity)
            }) || self.executable.as_ref().is_some_and(|image| {
                !image.check_current(self.binding.pid, &self.identity, current)
            }))
        {
            gate.revoke();
            return Err(Outcome::RefusedBeforeWrite);
        }
        #[cfg(not(feature = "developer-image-witness"))]
        let _ = current;
        Ok(())
    }

    #[cfg(feature = "developer-conditional-close")]
    fn check_developer_pair(&self, current: Option<&CurrentImage>) -> Result<(), Outcome> {
        let Some(pair) = &self.developer_pair else {
            return Ok(());
        };
        let valid = self.executable.as_ref().is_some_and(|image| {
            pair.check(&self.identity, image, self.binding.pid, current)
                .is_ok()
        });
        if !valid {
            self.lifetime
                .gate
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .revoke();
            return Err(Outcome::RefusedBeforeWrite);
        }
        Ok(())
    }

    fn effect_lease(&mut self, finalizing: bool) -> Result<Option<EffectLease>, Outcome> {
        if self.effect_proof.is_none() {
            #[cfg(feature = "developer-image-witness")]
            if self.image_witness_selected() {
                self.revoke_image();
                return Err(Outcome::RefusedBeforeWrite);
            }
            return Ok(None);
        }
        let flight = self.image_proof_flight()?;
        // New developer-object pathname/descriptor work uses this same flight,
        // outside the urgent revoke gate, before EVERY effect chunk/finish.
        #[cfg(feature = "developer-conditional-close")]
        self.check_developer_pair(flight.current.as_ref())?;
        #[cfg(feature = "developer-conditional-close")]
        self.check_qualified_pair(flight.current.as_ref())?;
        #[cfg(feature = "developer-image-witness")]
        if finalizing && self.image_witness_selected() {
            let deadline = self.deadline.ok_or(Outcome::RefusedBeforeWrite)?;
            if self.finish_image_witness(deadline).is_err() {
                self.revoke_image();
                return Err(Outcome::RefusedBeforeWrite);
            }
            #[cfg(feature = "product-image-witness")]
            {
                // Not authority by itself: the SAME original worker combines
                // this with definitive phase + zero flights and publishes only
                // AFTER this whole Session's fields have dropped.
                self.product_witness_finished = true;
            }
        }
        #[cfg(not(feature = "developer-image-witness"))]
        let _ = finalizing;
        // RPC/Finish completion may have lost cancellation or expiry. Never
        // acquire even a durable lease for that late positive observation.
        #[cfg(feature = "developer-image-witness")]
        if self.image_witness_selected() {
            remaining(self.deadline.ok_or(Outcome::RefusedBeforeWrite)?)?;
            let mut gate = self
                .lifetime
                .gate
                .lock()
                .map_err(|_| Outcome::RefusedBeforeWrite)?;
            self.check_locked_current(&mut gate, flight.current.as_ref())?;
        }
        // Disk reads and try-lock are OUTSIDE the lifetime gate. On error,
        // proof.lease drops its own acquired lease before `flight` unwinds.
        let lease = self
            .effect_proof
            .as_ref()
            .ok_or(Outcome::RefusedBeforeWrite)?
            .lease()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        let guarded = EffectLease {
            lease: Some(lease),
            flight: Some(flight),
        };
        #[cfg(test)]
        if let Some((skip, _)) = self.proof_pause.as_mut() {
            if *skip != 0 {
                *skip -= 1;
            } else if let Some((_, barrier)) = self.proof_pause.take() {
                barrier.wait();
                barrier.wait();
            }
        }
        Ok(Some(guarded))
    }

    #[cfg(test)]
    pub(crate) fn pause_proof_after(&mut self, skip: usize, barrier: Arc<std::sync::Barrier>) {
        self.proof_pause = Some((skip, barrier));
    }
    #[cfg(test)]
    pub(crate) fn partial_effect_chunks(&mut self, bytes: usize) {
        self.effect_chunk = bytes;
    }

    fn check_locked(&self, gate: &mut Gate) -> Result<(), Outcome> {
        let refuse = Outcome::RefusedBeforeWrite;
        if self.lifetime.pid != self.binding.pid
            || !self.lifetime.running(gate)
            || !gate.reservation.as_ref().is_some_and(|r| {
                Arc::ptr_eq(&r.identity, &self.identity) && !r.cancelled && r.phase.accepts_proof()
            })
        {
            return Err(refuse);
        }
        self.check_original_files(gate)
    }

    // Local original checks shared by proof and terminal-only retirement. No
    // helper RPC/current-image capture, effect permission or deadline renewal.
    fn check_original_files(&self, gate: &mut Gate) -> Result<(), Outcome> {
        let refuse = Outcome::RefusedBeforeWrite;
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
        #[allow(unused_mut)]
        let mut current_needed = true;
        #[cfg(feature = "developer-image-witness")]
        if self.image_witness_selected() {
            current_needed = false;
            if self.pending_executable.as_ref().is_some_and(|pending| {
                !pending.identity.matches(&pending.source)
                    || fs::symlink_metadata(&pending.path)
                        .ok()
                        .filter(|m| !m.file_type().is_symlink())
                        .and_then(|m| FileIdentity::capture(&m))
                        != Some(pending.identity)
            }) {
                gate.revoke();
                return Err(refuse);
            }
        }
        if self.executable.as_ref().is_some_and(|evidence| {
            if current_needed {
                !evidence.check(self.binding.pid)
            } else {
                !evidence.check_held()
            }
        }) {
            gate.revoke();
            return Err(refuse);
        }
        #[cfg(feature = "developer-conditional-close")]
        if let Some(pair) = &self.developer_pair
            && !pair.belongs_to(&self.identity)
        {
            gate.revoke();
            return Err(refuse);
        }
        #[cfg(feature = "developer-conditional-close")]
        if let Some(pair) = &self.qualified_pair
            && !pair.belongs_to(&self.identity)
        {
            gate.revoke();
            return Err(refuse);
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
            Request::Configs => "GET /configs HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n".into(),
            Request::Proxies => "GET /proxies HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n".into(),
            Request::Rules => "GET /rules HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n".into(),
            Request::Providers => "GET /providers/rules HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n".into(),
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
            // Durable proof is rechecked outside the child gate for EVERY
            // chunk. Retain a nonblocking migration lease for exactly one
            // syscall, never a readiness wait or owner callback.
            let effect_lease = if effect {
                self.effect_lease(false)?
            } else {
                #[cfg(feature = "developer-image-witness")]
                if self.image_witness_selected() {
                    self.check()?;
                }
                None
            };
            let result = {
                let mut gate = self.lifetime.gate.lock().map_err(|_| Outcome::Unknown)?;
                if effect {
                    self.check_locked_current(
                        &mut gate,
                        effect_lease
                            .as_ref()
                            .and_then(|lease| lease.flight.as_ref())
                            .and_then(|flight| flight.current.as_ref()),
                    )?;
                } else {
                    self.check_locked(&mut gate)?;
                }
                remaining(deadline)?;
                if effect
                    && self
                        .confirmation_expiry
                        .is_some_and(|expiry| Instant::now() >= expiry)
                {
                    return Err(Outcome::RefusedBeforeWrite);
                }
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
            drop(effect_lease);
            let n = match result {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if effect {
                        return Err(Outcome::Unknown);
                    }
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

    #[cfg(feature = "developer-image-witness")]
    fn finish_image_witness(&mut self, deadline: Instant) -> Result<(), Outcome> {
        #[cfg(test)]
        if let Some(probe) = &mut self.image_finish_probe {
            return probe(deadline);
        }
        if let Some(ImageWitness::Bound(client)) = &mut self.image_witness {
            client
                .finish(deadline)
                .map_err(|_| Outcome::RefusedBeforeWrite)
        } else {
            #[cfg(test)]
            return Ok(());
            #[cfg(not(test))]
            return Err(Outcome::RefusedBeforeWrite);
        }
    }

    /// Detached terminal-only completion of this SAME before-effect session.
    /// Clock expiry alone may be retired; poison/cancel/authorization/attempt
    /// may not. The old mutation deadline is never changed or used as authority.
    /// No owner/migration lock may be held by the caller.
    #[cfg(feature = "product-image-witness")]
    pub(crate) fn retire_before_effect(mut self) -> Result<CloseEpochRetirement, Outcome> {
        let original = self.cancellation();
        let budget = RETIREMENT_BUDGET;
        #[cfg(test)]
        let budget = self.retirement_budget.unwrap_or(budget);
        let deadline = Instant::now() + budget;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if self.effect_proof.is_some()
                || !self.image_witness_selected()
                || self.deadline.is_none()
                || self.executable.is_none()
            {
                return Err(Outcome::RefusedBeforeWrite);
            }
            remaining(deadline)?;
            {
                let mut gate = self
                    .lifetime
                    .gate
                    .lock()
                    .map_err(|_| Outcome::RefusedBeforeWrite)?;
                self.check_locked(&mut gate)?;
                if !gate
                    .reservation
                    .as_ref()
                    .is_some_and(|r| matches!(r.phase, Phase::BeforeEffect) && r.proofs == 0)
                {
                    return Err(Outcome::RefusedBeforeWrite);
                }
                gate.reservation.as_mut().unwrap().phase = Phase::Retiring;
            }
            // Consume before the fallible terminal RPC. No ordinary proof,
            // authorization or effect method accepts Retiring.
            remaining(deadline)?;
            {
                let mut gate = self
                    .lifetime
                    .gate
                    .lock()
                    .map_err(|_| Outcome::RefusedBeforeWrite)?;
                self.check_retirement_locked(&mut gate)?;
            }
            self.finish_image_witness(deadline)?;
            remaining(deadline)?;
            {
                let mut gate = self
                    .lifetime
                    .gate
                    .lock()
                    .map_err(|_| Outcome::RefusedBeforeWrite)?;
                self.check_retirement_locked(&mut gate)?;
                let reservation = gate
                    .reservation
                    .as_mut()
                    .ok_or(Outcome::RefusedBeforeWrite)?;
                reservation.cancelled = true;
                reservation.phase = Phase::Finished(Outcome::RefusedBeforeWrite);
                reservation.stream = None;
            }
            let gate = self
                .lifetime
                .gate
                .lock()
                .map_err(|_| Outcome::RefusedBeforeWrite)?;
            if !gate.live
                || !gate.reservation.as_ref().is_some_and(|r| {
                    Arc::ptr_eq(&r.identity, &self.identity)
                        && matches!(r.phase, Phase::Finished(Outcome::RefusedBeforeWrite))
                        && r.cancelled
                        && r.proofs == 0
                })
            {
                return Err(Outcome::RefusedBeforeWrite);
            }
            Ok(())
        }));
        if !matches!(result, Ok(Ok(()))) {
            self.revoke_image();
            return Err(Outcome::RefusedBeforeWrite);
        }
        drop(self); // ALL original session/channel/source fields, not ACK alone
        if remaining(deadline).is_err() {
            original.lifetime.revoke();
            return Err(Outcome::RefusedBeforeWrite);
        }
        Ok(CloseEpochRetirement { original, deadline })
    }

    #[cfg(feature = "product-image-witness")]
    fn check_retirement_locked(&self, gate: &mut Gate) -> Result<(), Outcome> {
        if self.lifetime.pid != self.binding.pid
            || !self.lifetime.running(gate)
            || !gate.reservation.as_ref().is_some_and(|r| {
                Arc::ptr_eq(&r.identity, &self.identity)
                    && matches!(r.phase, Phase::Retiring)
                    && !r.cancelled
                    && r.proofs == 0
            })
        {
            return Err(Outcome::RefusedBeforeWrite);
        }
        self.check_original_files(gate)
    }

    #[cfg(feature = "product-image-witness")]
    pub(crate) fn refuse_retirement(&self) {
        self.revoke_image();
    }

    fn finish(&mut self, candidate: Outcome) -> Outcome {
        // Definitive acceptance also proves the durable owner receipt outside
        // the gate, with its nonblocking lease retained through terminalization.
        // Refusal/uncertainty never needs positive durable authority. In
        // particular, cancellation must not reacquire a migration lease and
        // contend with the urgent owner operation that just revoked us.
        let effect_lease = if matches!(candidate, Outcome::Unknown | Outcome::RefusedBeforeWrite) {
            Ok(None)
        } else {
            self.effect_lease(true)
        };
        let lifetime = Arc::clone(&self.lifetime);
        let mut gate = lifetime.gate.lock().unwrap_or_else(|e| e.into_inner());
        let current = effect_lease
            .as_ref()
            .ok()
            .and_then(|lease| lease.as_ref())
            .and_then(|lease| lease.flight.as_ref())
            .and_then(|flight| flight.current.as_ref());
        let proved = self.check_locked_current(&mut gate, current).is_ok()
            && effect_lease.is_ok()
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
        Ok(self
            .discover_rows()?
            .into_iter()
            .map(|row| row.target)
            .collect())
    }

    pub(crate) fn discover_rows(&mut self) -> Result<Vec<ObservedRow>, Outcome> {
        if !self.ready()? {
            return Err(Outcome::Unsupported);
        }
        let (status, body) = self.exchange(None)?;
        if status != 200 {
            return Err(Outcome::Unsupported);
        }
        Ok(snapshot_rows(&body)?
            .into_iter()
            .map(|(target, display)| ObservedRow {
                target: BoundTarget {
                    target,
                    binding: self.binding,
                    session_identity: Arc::clone(&self.identity),
                },
                display,
            })
            .collect())
    }

    pub(crate) fn authorize_effect(
        &mut self,
        proof: crate::native_coordinator::connection_close::EffectProof,
        expiry: Instant,
        display: serde_json::Value,
    ) {
        self.effect_proof = Some(proof);
        self.confirmation_expiry = Some(expiry);
        self.expected_display = Some(display);
        // Separate one-shot effect budget; never renew the snapshot expiry.
        self.deadline = Some(Instant::now() + BUDGET);
    }

    pub(crate) fn proves_live(&mut self) -> bool {
        self.check().is_ok()
    }

    /// Confirmation already holds the scheduler lease. Check only original
    /// lifetime facts here; full developer object checks run in the worker's
    /// outside-gate ProofFlight before any effect and definitive completion.
    pub(crate) fn proves_live_for_scheduling(&self) -> bool {
        self.lifetime
            .gate
            .lock()
            .is_ok_and(|mut gate| self.check_locked(&mut gate).is_ok())
    }
    pub(crate) fn attach_observation(&mut self, facts: crate::native_host::CloseFacts) {
        self.owned_observation = Some(facts);
    }

    pub(crate) fn capture_executable(&mut self, source_path: &Path) -> Result<(), Outcome> {
        let mut gate = self
            .lifetime
            .gate
            .lock()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        self.check_locked(&mut gate)?;
        let image =
            ExecutableEvidence::image(self.binding.pid).ok_or(Outcome::RefusedBeforeWrite)?;
        let source = File::from(
            open(
                source_path,
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
                Mode::empty(),
            )
            .map_err(|_| Outcome::RefusedBeforeWrite)?,
        );
        let image_identity =
            FileIdentity::capture(&image.metadata().map_err(|_| Outcome::RefusedBeforeWrite)?)
                .ok_or(Outcome::RefusedBeforeWrite)?;
        let source_identity =
            FileIdentity::capture(&source.metadata().map_err(|_| Outcome::RefusedBeforeWrite)?)
                .ok_or(Outcome::RefusedBeforeWrite)?;
        self.executable = Some(ExecutableEvidence {
            image,
            source,
            image_identity,
            source_identity,
            source_path: source_path.to_owned(),
            digests: None,
        });
        self.check_locked(&mut gate)
    }

    /// Explicit fixed developer capture only. Keep original source/child handle
    /// while under owner admission; Bind/Observe are deferred to detached work.
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn capture_executable_via_witness(
        &mut self,
        source_path: &Path,
    ) -> Result<(), Outcome> {
        self.capture_executable_via_class(source_path, WitnessClass::Tests)
    }

    /// Explicit installed-development selection only; no direct fallback.
    /// This captures local originals. Bind/Observe stay in detached flights.
    #[cfg(feature = "developer-image-witness")]
    pub(crate) fn capture_executable_via_runtime_witness(
        &mut self,
        source_path: &Path,
    ) -> Result<(), Outcome> {
        self.capture_executable_via_class(source_path, WitnessClass::InstalledRuntime)
    }

    #[cfg(feature = "product-image-witness")]
    pub(crate) fn capture_executable_via_product_witness(
        &mut self,
        source_path: &Path,
    ) -> Result<(), Outcome> {
        self.capture_executable_via_class(source_path, WitnessClass::Product)
    }

    #[cfg(feature = "developer-image-witness")]
    fn capture_executable_via_class(
        &mut self,
        source_path: &Path,
        class: WitnessClass,
    ) -> Result<(), Outcome> {
        let result = (|| {
            if source_path != Path::new(crate::managed_pair::RELEASE_CORE)
                || self.image_witness.is_some()
                || self.executable.is_some()
            {
                return Err(Outcome::RefusedBeforeWrite);
            }
            let mut gate = self
                .lifetime
                .gate
                .lock()
                .map_err(|_| Outcome::RefusedBeforeWrite)?;
            self.check_locked(&mut gate)?;
            let source = File::from(
                open(
                    source_path,
                    OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
                    Mode::empty(),
                )
                .map_err(|_| Outcome::RefusedBeforeWrite)?,
            );
            let m = source.metadata().map_err(|_| Outcome::RefusedBeforeWrite)?;
            let identity = FileIdentity::capture(&m).ok_or(Outcome::RefusedBeforeWrite)?;
            if m.uid() != 0 || m.gid() != 0 || m.mode() & 0o7777 != 0o755 {
                return Err(Outcome::RefusedBeforeWrite);
            }
            let pid = rustix::process::Pid::from_raw(
                i32::try_from(self.binding.pid).map_err(|_| Outcome::RefusedBeforeWrite)?,
            )
            .ok_or(Outcome::RefusedBeforeWrite)?;
            let child = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::NONBLOCK)
                .map_err(|_| Outcome::RefusedBeforeWrite)?;
            self.check_locked(&mut gate)?;
            self.pending_executable = Some(PendingExecutable {
                source,
                identity,
                path: source_path.to_owned(),
            });
            self.image_witness = Some(ImageWitness::Pending {
                child: Some(child),
                class,
            });
            Ok(())
        })();
        if result.is_err() {
            self.revoke_image();
        }
        result
    }

    #[cfg(feature = "developer-image-witness")]
    fn prepare_witness_image(&mut self) -> Result<(), Outcome> {
        if self.pending_executable.is_none() {
            return Ok(());
        }
        let result = (|| {
            let flight = self.image_proof_flight()?;
            let current = flight.current.as_ref().ok_or(Outcome::RefusedBeforeWrite)?;
            let image = current
                .file
                .try_clone()
                .map_err(|_| Outcome::RefusedBeforeWrite)?;
            let pending = self
                .pending_executable
                .take()
                .ok_or(Outcome::RefusedBeforeWrite)?;
            self.executable = Some(ExecutableEvidence {
                image,
                source: pending.source,
                image_identity: pending.identity,
                source_identity: pending.identity,
                source_path: pending.path,
                digests: None,
            });
            self.check_with_flight(&flight)
        })();
        if result.is_err() {
            self.revoke_image();
        }
        result
    }

    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn revoke_prepared_witness(&self) {
        self.revoke_image();
    }

    /// Initial developer adoption only, NOT operation/current-catalog proof.
    /// Full fresh observation preceded preparation and will run again detached.
    #[cfg(all(test, feature = "developer-image-witness"))]
    pub(crate) fn prepared_witness_origin(&self, core: &mut OwnedCore) -> bool {
        matches!(self.image_witness, Some(ImageWitness::Bound(_)))
            && self.pending_executable.is_none()
            && self.executable.is_some()
            && self
                .qualified_pair
                .as_ref()
                .is_some_and(|pair| pair.belongs_to(&self.identity))
            && self
                .deadline
                .is_some_and(|deadline| Instant::now() < deadline)
            && core.pid() == Some(self.binding.pid)
            && core
                .conditional_lifetime()
                .is_ok_and(|original| Arc::ptr_eq(&original, &self.lifetime))
            && self.proves_live_for_scheduling()
    }

    /// Passive only. Hash actual retained bytes OUTSIDE owner/child gates;
    /// neither these digests nor ABI readiness mint package authority.
    pub(crate) fn prepare_executable(&mut self) -> Result<(), Outcome> {
        #[cfg(feature = "developer-image-witness")]
        self.prepare_witness_image()?;
        self.check()?;
        let deadline = *self.deadline.get_or_insert_with(|| Instant::now() + BUDGET);
        remaining(deadline)?;
        let evidence = self
            .executable
            .as_mut()
            .ok_or(Outcome::RefusedBeforeWrite)?;
        if evidence.digests.is_none() {
            let image =
                ExecutableEvidence::hash(&mut evidence.image, evidence.image_identity, deadline)
                    .ok_or(Outcome::RefusedBeforeWrite)?;
            let source =
                ExecutableEvidence::hash(&mut evidence.source, evidence.source_identity, deadline)
                    .ok_or(Outcome::RefusedBeforeWrite)?;
            evidence.digests = Some((image, source));
        }
        self.check()
    }

    /// Opt-in developer object admission runs with discovery outside the owner
    /// mutex. Ordinary core paths remain non-authorizing even in this build.
    #[cfg(feature = "developer-conditional-close")]
    pub(crate) fn prepare_developer_pair(&mut self) -> Result<(), Outcome> {
        let fixed = Path::new(package_evidence::developer_pair::DIRECTORY).join("mihomo");
        if self
            .executable
            .as_ref()
            .is_none_or(|image| image.source_path != fixed)
        {
            return Ok(());
        }
        if self.developer_pair.is_none() {
            self.developer_pair = Some(
                package_evidence::developer_pair::Evidence::capture(self)
                    .map_err(|_| Outcome::RefusedBeforeWrite)?,
            );
        }
        self.check()
    }

    #[cfg(feature = "developer-conditional-close")]
    pub(crate) fn developer_pair_permit(&mut self) -> Option<CandidateEffectPermit> {
        self.developer_pair.as_ref()?;
        // Confirmation holds the shared scheduler lease. Do not introduce
        // developer package I/O there; the worker rechecks it under ProofFlight
        // before the first and every subsequent write and definitive finish.
        let mut gate = self.lifetime.gate.lock().ok()?;
        self.check_locked(&mut gate).ok()?;
        Some(CandidateEffectPermit { _private: () })
    }

    #[cfg(feature = "developer-conditional-close")]
    pub(crate) fn prepare_qualified_pair(&mut self, config: &Path) -> Result<(), Outcome> {
        if self
            .executable
            .as_ref()
            .is_none_or(|image| image.source_path != Path::new(crate::managed_pair::RELEASE_CORE))
        {
            return Ok(());
        }
        if !self.qualification_attempted {
            self.qualification_attempted = true; // no restore/retry of failed original qualification
            match package_evidence::release_pair::Evidence::capture(self, config) {
                Ok(evidence) => self.qualified_pair = evidence,
                Err(_) => {
                    self.lifetime
                        .gate
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .revoke();
                    return Err(Outcome::RefusedBeforeWrite);
                }
            }
        }
        self.check()
    }
    #[cfg(feature = "developer-conditional-close")]
    fn check_qualified_pair(&self, current: Option<&CurrentImage>) -> Result<(), Outcome> {
        let Some(pair) = &self.qualified_pair else {
            return Ok(());
        };
        if self.executable.as_ref().is_none_or(|image| {
            pair.check(&self.identity, image, self.binding.pid, current)
                .is_err()
        }) {
            self.lifetime
                .gate
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .revoke();
            return Err(Outcome::RefusedBeforeWrite);
        }
        Ok(())
    }
    #[cfg(feature = "developer-conditional-close")]
    pub(crate) fn qualified_pair_permit(
        &mut self,
    ) -> Result<Option<CandidateEffectPermit>, Outcome> {
        if self.qualified_pair.is_none() {
            return Ok(None);
        }
        // No package I/O or fresh constructor in the scheduler lease. The
        // SAME worker ProofFlight checks originals before every write/finish.
        let mut gate = self
            .lifetime
            .gate
            .lock()
            .map_err(|_| Outcome::RefusedBeforeWrite)?;
        self.check_locked(&mut gate)?;
        Ok(Some(CandidateEffectPermit { _private: () }))
    }

    pub(crate) fn read_fixed(
        &mut self,
        endpoint: omavless_mihomo::ReadOnlyEndpoint,
    ) -> Option<serde_json::Value> {
        let request = match endpoint {
            omavless_mihomo::ReadOnlyEndpoint::Configs => Request::Configs,
            omavless_mihomo::ReadOnlyEndpoint::Proxies => Request::Proxies,
            omavless_mihomo::ReadOnlyEndpoint::Rules => Request::Rules,
            omavless_mihomo::ReadOnlyEndpoint::RuleProviders => Request::Providers,
            _ => return None,
        };
        let (status, raw) = self.exchange_request(request).ok()?;
        (status == 200)
            .then(|| serde_json::from_slice(&raw).ok())
            .flatten()
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
        if let Some(facts) = self.owned_observation.take() {
            if facts.observe(self).is_err() {
                return Outcome::RefusedBeforeWrite;
            }
            let rows = match self.discover_rows() {
                Ok(rows) => rows,
                Err(_) => return Outcome::RefusedBeforeWrite,
            };
            if !rows.iter().any(|row| {
                selected.same_selection(&row.target)
                    && self.expected_display.as_ref() == Some(&row.display)
            }) {
                return Outcome::RefusedBeforeWrite;
            }
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
/// then move only the retained transport out. Capacity remains reserved until
/// all transport/proof cleanup is complete, before result publication. No
/// default-product method uses it. The opt-in development socket workspace
/// reuses the same owner scheduling path rather than creating another one.
#[derive(Default)]
pub(crate) struct Scheduler {
    active: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    fail_next_spawn: bool,
    #[cfg(test)]
    after_publish: Option<Arc<std::sync::Barrier>>,
}
struct Slot(Arc<std::sync::atomic::AtomicBool>);
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
pub(crate) struct Worker {
    cancellation: Cancellation,
    result: std::sync::mpsc::Receiver<WorkerResult>,
    thread: Option<std::thread::JoinHandle<()>>,
}
struct WorkerResult {
    outcome: Outcome,
    #[cfg(feature = "product-image-witness")]
    product_retired: bool,
}

/// Constructed only from this original worker's after-Drop publication. No
/// Clone, wire/boolean constructor or caller-selected session identity.
#[cfg(feature = "product-image-witness")]
pub struct CloseEpochCompletion {
    outcome: Outcome,
    original: Cancellation,
    retired: bool,
}
#[cfg(feature = "product-image-witness")]
pub struct CloseEpochRetirement {
    original: Cancellation,
    deadline: Instant,
}
#[cfg(feature = "product-image-witness")]
impl CloseEpochRetirement {
    pub(crate) fn admits(&self, expected: &Cancellation) -> bool {
        remaining(self.deadline).is_ok()
            && Arc::ptr_eq(&self.original.identity, &expected.identity)
            && Arc::ptr_eq(&self.original.lifetime, &expected.lifetime)
            && self.original.lifetime.gate.lock().is_ok_and(|gate| {
                gate.live && gate.reservation.is_none() && remaining(self.deadline).is_ok()
            })
    }
    pub(crate) fn retirement_deadline(&self) -> Instant {
        self.deadline
    }
}
#[cfg(feature = "product-image-witness")]
impl CloseEpochCompletion {
    pub(crate) fn outcome(&self) -> Outcome {
        self.outcome
    }
    pub(crate) fn admits(&self, expected: &Cancellation) -> bool {
        self.retired
            && self.outcome == Outcome::Closed
            && Arc::ptr_eq(&self.original.identity, &expected.identity)
            && Arc::ptr_eq(&self.original.lifetime, &expected.lifetime)
            && self.original.lifetime.gate.lock().is_ok_and(|gate| {
                // Original session/stream fields and counted flights have gone;
                // observed lifetime poison may never be renewed.
                gate.live && gate.reservation.is_none()
            })
    }
}

#[cfg(feature = "product-image-witness")]
impl Session {
    fn product_retirement_ready(&self, outcome: Outcome) -> bool {
        self.product_witness_finished
            && outcome == Outcome::Closed
            && self.lifetime.gate.lock().is_ok_and(|gate| {
                gate.live
                    && gate.reservation.as_ref().is_some_and(|r| {
                        Arc::ptr_eq(&r.identity, &self.identity)
                            && matches!(r.phase, Phase::Finished(Outcome::Closed))
                            && r.proofs == 0
                    })
            })
    }
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
        let after_publish = self.after_publish.take();
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
                let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    session.close(selected, permit)
                })) {
                    Ok(outcome) => outcome,
                    Err(_) => session.finish(Outcome::Unknown),
                };
                #[cfg(feature = "product-image-witness")]
                let product_retired = session.product_retirement_ready(outcome);
                // Release retained descriptors and this exact reservation
                // before result publication or worker-slot release.
                drop(session);
                drop(slot);
                let _ = sender.try_send(WorkerResult {
                    outcome,
                    #[cfg(feature = "product-image-witness")]
                    product_retired,
                });
                #[cfg(test)]
                if let Some(barrier) = after_publish {
                    barrier.wait();
                    barrier.wait();
                }
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
    #[cfg(any(test, not(feature = "product-image-witness")))]
    pub(crate) fn poll(&mut self) -> Option<Outcome> {
        self.poll_original().map(|result| result.outcome)
    }
    #[cfg(feature = "product-image-witness")]
    pub(crate) fn poll_epoch(&mut self) -> Option<CloseEpochCompletion> {
        self.poll_original().map(|result| CloseEpochCompletion {
            outcome: result.outcome,
            original: self.cancellation.clone(),
            retired: result.product_retired,
        })
    }
    fn poll_original(&mut self) -> Option<WorkerResult> {
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
    fn strict_same_row_projection_rejects_all_known_duplicate_keys_before_value_collapse() {
        let valid = format!(
            r#"{{"connections":[{{"id":"{ID}","omavlessCloseToken":"42","metadata":{{"host":"same.invalid","destinationIP":"192.0.2.1","destinationPort":"443","network":"tcp"}},"chains":["DIRECT"]}}]}}"#
        );
        let rows = snapshot_rows(valid.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        for (needle, duplicate) in [
            ("\"metadata\":{", "\"metadata\":{},\"metadata\":{"),
            ("\"chains\":[", "\"chains\":[],\"chains\":["),
            ("\"host\":", "\"host\":\"foreign.invalid\",\"ho\\u0073t\":"),
            (
                "\"destinationIP\":",
                "\"destinationIP\":\"192.0.2.2\",\"destinationIP\":",
            ),
            (
                "\"destinationPort\":",
                "\"destinationPort\":\"80\",\"destinationPort\":",
            ),
            ("\"network\":", "\"network\":\"udp\",\"network\":"),
        ] {
            assert!(
                snapshot_rows(valid.replace(needle, duplicate).as_bytes()).is_err(),
                "{needle}"
            );
        }
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
    fn published_receipt_releases_session_and_capacity_before_thread_exit() {
        let _fixture = FIXTURE.lock().unwrap();
        let (root, mut core) = peer_fixture("ok");
        let (session, target) = prepared(&mut core);
        let published = Arc::new(std::sync::Barrier::new(2));
        let mut scheduler = Scheduler {
            after_publish: Some(Arc::clone(&published)),
            ..Scheduler::default()
        };
        let mut first = scheduler
            .start(session, target, CandidateEffectPermit::owned_fixture())
            .unwrap();
        published.wait(); // result sent, old thread deliberately still alive
        assert_eq!(first.poll(), Some(Outcome::Closed));
        assert!(!first.thread.as_ref().unwrap().is_finished());
        let (session, target) = prepared(&mut core);
        let mut successor = scheduler
            .start(session, target, CandidateEffectPermit::owned_fixture())
            .unwrap();
        assert_eq!(worker_result(&mut successor), Outcome::Closed);
        published.wait();
        assert_eq!(fs::read(root.join("effects")).unwrap(), b"1\n1\n");
        core.stop(Duration::from_secs(2)).unwrap();
        drop(first);
        drop(successor);
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
