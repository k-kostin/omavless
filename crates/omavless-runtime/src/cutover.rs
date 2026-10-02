// SPDX-License-Identifier: MIT

//! Durable one-owner cutover primitives for the staged R5 migration.
//!
//! This module deliberately exposes no CLI, socket method, service control, or
//! production marker write. It defines the shared legacy/Rust operation lock,
//! a bounded private ownership marker, and fail-closed transition decisions so
//! a later cutover transaction cannot accidentally create two lifecycle owners.

use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg, OFlag};
use omavless_store::{StoreIoError, atomic_replace_private, read_private_utf8};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const OWNERSHIP_SCHEMA_VERSION: u8 = 1;
pub const MAX_OWNERSHIP_MARKER_BYTES: u64 = 1024;
pub const OWNERSHIP_MARKER_NAME: &str = "ownership.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OwnershipPhase {
    Legacy,
    CutoverPreparing,
    Rust,
    RollbackPreparing,
}

impl OwnershipPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::CutoverPreparing => "cutoverPreparing",
            Self::Rust => "rust",
            Self::RollbackPreparing => "rollbackPreparing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnershipMarker {
    schema_version: u8,
    generation: u64,
    phase: OwnershipPhase,
}

impl Default for OwnershipMarker {
    fn default() -> Self {
        Self {
            schema_version: OWNERSHIP_SCHEMA_VERSION,
            generation: 0,
            phase: OwnershipPhase::Legacy,
        }
    }
}

impl OwnershipMarker {
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn phase(&self) -> OwnershipPhase {
        self.phase
    }

    fn successor(&self, phase: OwnershipPhase) -> Result<Self, CutoverError> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(CutoverError::InvalidTransition)?;
        Ok(Self {
            schema_version: OWNERSHIP_SCHEMA_VERSION,
            generation,
            phase,
        })
    }

    fn validate(&self) -> Result<(), CutoverError> {
        if self.schema_version != OWNERSHIP_SCHEMA_VERSION {
            return Err(CutoverError::InvalidMarker);
        }
        Ok(())
    }
}

/// Credential-free generation fence for one explicit transition candidate.
///
/// The target Rust generation is derived exactly once from the durable
/// `cutoverPreparing` marker. Callers never perform transition generation
/// arithmetic independently, and overflow fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionBootstrap {
    preparing_generation: u64,
    rust_generation: u64,
}

impl TransitionBootstrap {
    pub fn from_preparing(marker: &OwnershipMarker) -> Result<Self, CutoverError> {
        if marker.phase() != OwnershipPhase::CutoverPreparing {
            return Err(CutoverError::PreconditionsFailed);
        }
        let rust_generation = marker
            .generation()
            .checked_add(1)
            .ok_or(CutoverError::InvalidTransition)?;
        Ok(Self {
            preparing_generation: marker.generation(),
            rust_generation,
        })
    }

    #[must_use]
    pub const fn preparing_generation(self) -> u64 {
        self.preparing_generation
    }

    #[must_use]
    pub const fn rust_generation(self) -> u64 {
        self.rust_generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoverPaths {
    pub runtime_base: PathBuf,
    pub operation_lock: PathBuf,
    pub state_directory: PathBuf,
    pub ownership_marker: PathBuf,
}

impl CutoverPaths {
    #[must_use]
    pub fn below(runtime_base: &Path, state_base: &Path, uid: u32) -> Self {
        let state_directory = state_base.join("omavless");
        Self {
            runtime_base: runtime_base.to_owned(),
            operation_lock: runtime_base.join(format!("omavless.{uid}.lock")),
            ownership_marker: state_directory.join(OWNERSHIP_MARKER_NAME),
            state_directory,
        }
    }

    pub fn current(uid: u32) -> Result<Self, CutoverError> {
        let runtime_base = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")));
        let state_base = match env::var_os("XDG_STATE_HOME") {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(env::var_os("HOME").ok_or(CutoverError::UnsafeStateDirectory)?)
                .join(".local/state"),
        };
        if !runtime_base.is_absolute() {
            return Err(CutoverError::UnsafeRuntimeDirectory);
        }
        if !state_base.is_absolute() {
            return Err(CutoverError::UnsafeStateDirectory);
        }
        Ok(Self::below(&runtime_base, &state_base, uid))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverError {
    UnsafeRuntimeDirectory,
    UnsafeStateDirectory,
    Busy,
    InvalidMarker,
    MarkerTooLarge,
    InvalidTransition,
    PreconditionsFailed,
    Io,
}

impl fmt::Display for CutoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeRuntimeDirectory => "OmaVLESS migration runtime directory is unsafe",
            Self::UnsafeStateDirectory => "OmaVLESS migration state directory is unsafe",
            Self::Busy => "Another OmaVLESS operation owns the migration lock",
            Self::InvalidMarker => "OmaVLESS ownership marker is invalid",
            Self::MarkerTooLarge => "OmaVLESS ownership marker is too large",
            Self::InvalidTransition => "OmaVLESS ownership transition is invalid",
            Self::PreconditionsFailed => "OmaVLESS ownership preconditions are not satisfied",
            Self::Io => "OmaVLESS ownership state I/O failed",
        })
    }
}

impl std::error::Error for CutoverError {}

impl From<StoreIoError> for CutoverError {
    fn from(value: StoreIoError) -> Self {
        match value {
            StoreIoError::UnsafePath | StoreIoError::WrongOwner => Self::UnsafeStateDirectory,
            StoreIoError::TooLarge => Self::MarkerTooLarge,
            StoreIoError::InvalidUtf8 => Self::InvalidMarker,
            StoreIoError::Io => Self::Io,
        }
    }
}

fn owned_directory(path: &Path, uid: u32, private: bool) -> Result<(), CutoverError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| CutoverError::UnsafeStateDirectory)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != uid
        || (private && metadata.permissions().mode() & 0o077 != 0)
    {
        return Err(CutoverError::UnsafeStateDirectory);
    }
    Ok(())
}

fn prepare_state_directory(path: &Path, uid: u32) -> Result<(), CutoverError> {
    let parent = path.parent().ok_or(CutoverError::UnsafeStateDirectory)?;
    // XDG_STATE_HOME itself may be a conventional same-user 0755 directory;
    // the OmaVLESS child and marker remain private.
    owned_directory(parent, uid, false)?;
    if !path.exists() {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| CutoverError::Io)?;
    }
    owned_directory(path, uid, true)
}

pub struct MigrationLock {
    path: PathBuf,
    runtime_path: PathBuf,
    runtime: File,
    runtime_identity: fs::Metadata,
    uid: u32,
    _file: Flock<File>,
}

#[derive(Clone, Copy)]
enum LockOpen {
    Existing,
    CreateIfAbsent,
    AbsentOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockCheckpoint {
    RuntimeOpened,
    PriorChecked,
    Opened,
    Locked,
    PermissionsSet,
    Validated,
}

fn safe_runtime(metadata: &fs::Metadata, uid: u32) -> bool {
    metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o700
}

fn safe_lock(metadata: &fs::Metadata, uid: u32) -> bool {
    metadata.is_file()
        && metadata.uid() == uid
        && metadata.mode() & 0o7777 == 0o600
        && metadata.nlink() == 1
}

fn same_inode(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

fn acceptable_open_lock(metadata: &fs::Metadata, uid: u32, mode: LockOpen) -> bool {
    safe_lock(metadata, uid)
        || matches!(mode, LockOpen::CreateIfAbsent)
            && metadata.is_file()
            && metadata.uid() == uid
            && metadata.nlink() == 1
            && metadata.mode() & 0o7777 == 0o644
}

impl MigrationLock {
    /// Read-only acquisition: never create or repair a missing/unsafe lock.
    pub(crate) fn acquire_existing(paths: &CutoverPaths, uid: u32) -> Result<Self, CutoverError> {
        Self::acquire_checked(paths, uid, LockOpen::Existing, |_| true)
    }

    pub fn acquire(paths: &CutoverPaths, uid: u32) -> Result<Self, CutoverError> {
        Self::acquire_checked(paths, uid, LockOpen::CreateIfAbsent, |_| true)
    }

    /// Recovery diagnostic only: never take, repair or retry an existing name.
    #[allow(dead_code)]
    pub(crate) fn acquire_absent(paths: &CutoverPaths, uid: u32) -> Result<Self, CutoverError> {
        Self::acquire_checked(paths, uid, LockOpen::AbsentOnly, |_| true)
    }

    fn acquire_checked(
        paths: &CutoverPaths,
        uid: u32,
        mode: LockOpen,
        mut hook: impl FnMut(LockCheckpoint) -> bool,
    ) -> Result<Self, CutoverError> {
        use nix::fcntl::openat;
        use nix::sys::stat::Mode;
        let refuse = CutoverError::UnsafeRuntimeDirectory;
        let name = format!("omavless.{uid}.lock");
        if paths.operation_lock != paths.runtime_base.join(&name) {
            return Err(refuse);
        }
        let before = fs::symlink_metadata(&paths.runtime_base).map_err(|_| refuse)?;
        if !safe_runtime(&before, uid) {
            return Err(refuse);
        }
        let runtime = OpenOptions::new()
            .read(true)
            .custom_flags((OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC).bits())
            .open(&paths.runtime_base)
            .map_err(|_| refuse)?;
        let runtime_identity = runtime.metadata().map_err(|_| refuse)?;
        if !safe_runtime(&runtime_identity, uid)
            || !same_inode(&before, &runtime_identity)
            || !hook(LockCheckpoint::RuntimeOpened)
        {
            return Err(refuse);
        }

        // Absence is only ENOENT. An intervening creator gets EEXIST: never
        // reopen or repair that new inode as an implicit retry.
        let prior = match fs::symlink_metadata(&paths.operation_lock) {
            Ok(value)
                if !matches!(mode, LockOpen::AbsentOnly)
                    && acceptable_open_lock(&value, uid, mode) =>
            {
                Some(value)
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && !matches!(mode, LockOpen::Existing) =>
            {
                None
            }
            _ => return Err(refuse),
        };
        if !hook(LockCheckpoint::PriorChecked) {
            return Err(refuse);
        }
        let mut flags = OFlag::O_RDWR | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK;
        if prior.is_none() {
            flags |= OFlag::O_CREAT | OFlag::O_EXCL;
        }
        let file = File::from(
            openat(
                &runtime,
                Path::new(&name),
                flags,
                Mode::from_bits_truncate(0o600),
            )
            .map_err(|error| {
                // A concurrent normal creator won the name. Report contention
                // without opening, repairing or acquiring its new inode.
                if matches!(mode, LockOpen::CreateIfAbsent)
                    && prior.is_none()
                    && error == nix::errno::Errno::EEXIST
                {
                    CutoverError::Busy
                } else {
                    refuse
                }
            })?,
        );
        let opened = file.metadata().map_err(|_| refuse)?;
        if !opened.is_file()
            || opened.uid() != uid
            || opened.nlink() != 1
            || prior.as_ref().is_some_and(|p| !same_inode(p, &opened))
            || !hook(LockCheckpoint::Opened)
        {
            return Err(refuse);
        }
        let file = Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
            if matches!(error, Errno::EAGAIN) {
                CutoverError::Busy
            } else {
                CutoverError::Io
            }
        })?;
        if !hook(LockCheckpoint::Locked) {
            return Err(refuse);
        }
        let current = fs::symlink_metadata(&paths.operation_lock).map_err(|_| refuse)?;
        let held = file.metadata().map_err(|_| refuse)?;
        let current_runtime = fs::symlink_metadata(&paths.runtime_base).map_err(|_| refuse)?;
        if !current.is_file()
            || current.uid() != uid
            || current.nlink() != 1
            || !held.is_file()
            || held.uid() != uid
            || held.nlink() != 1
            || !same_inode(&opened, &current)
            || !same_inode(&opened, &held)
            || current.mode() != opened.mode()
            || held.mode() != opened.mode()
            || !safe_runtime(&current_runtime, uid)
            || !same_inode(&runtime_identity, &current_runtime)
            || prior.is_some()
                && (!acceptable_open_lock(&current, uid, mode)
                    || !acceptable_open_lock(&held, uid, mode))
        {
            return Err(refuse);
        }
        // Restrictive umask may remove owner bits on our new inode. Frozen
        // Python used open('a'), so normal acquisition also tightens exactly
        // legacy 0644 only after exclusive flock and the full identity check.
        // Existing-only inspection never repairs, and no pathname is chmodded.
        if prior.is_none() || opened.mode() & 0o7777 == 0o644 {
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| CutoverError::Io)?;
        }
        if !hook(LockCheckpoint::PermissionsSet) {
            return Err(refuse);
        }
        let lease = Self {
            path: paths.operation_lock.clone(),
            runtime_path: paths.runtime_base.clone(),
            runtime,
            runtime_identity,
            uid,
            _file: file,
        };
        if !lease.authorizes(paths, uid)
            || !hook(LockCheckpoint::Validated)
            || !lease.authorizes(paths, uid)
        {
            return Err(refuse);
        }
        Ok(lease)
    }

    /// A held flock on an unlinked inode does not serialize a replacement
    /// pathname. Every authority check must prove the fixed name still reaches
    /// this held inode within the originally pinned runtime directory.
    /// This is a point-in-time check, not an atomic filesystem/effect boundary:
    /// callers retain their per-step gates and trust cooperating same-UID
    /// processes not to unlink/replace the lease between a check and an effect.
    /// Arbitrary hostile same-UID replacement-and-restoration is not prevented.
    #[must_use]
    pub(crate) fn authorizes(&self, paths: &CutoverPaths, uid: u32) -> bool {
        if self.uid != uid
            || self.path != paths.operation_lock
            || self.runtime_path != paths.runtime_base
        {
            return false;
        }
        let Ok(runtime) = fs::symlink_metadata(&self.runtime_path) else {
            return false;
        };
        let Ok(held_runtime) = self.runtime.metadata() else {
            return false;
        };
        let Ok(current) = fs::symlink_metadata(&self.path) else {
            return false;
        };
        let Ok(held) = self._file.metadata() else {
            return false;
        };
        safe_runtime(&runtime, uid)
            && safe_runtime(&held_runtime, uid)
            && same_inode(&self.runtime_identity, &runtime)
            && same_inode(&self.runtime_identity, &held_runtime)
            && safe_lock(&current, uid)
            && safe_lock(&held, uid)
            && same_inode(&current, &held)
    }
}

pub fn read_marker(paths: &CutoverPaths, uid: u32) -> Result<OwnershipMarker, CutoverError> {
    prepare_state_directory(&paths.state_directory, uid)?;
    read_marker_existing(paths, uid)
}

/// Shared canonical marker decoder without directory creation or repair.
/// Read-only callers must separately validate the existing parent hierarchy.
pub(crate) fn read_marker_existing(
    paths: &CutoverPaths,
    uid: u32,
) -> Result<OwnershipMarker, CutoverError> {
    let metadata = match fs::symlink_metadata(&paths.ownership_marker) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OwnershipMarker::default());
        }
        Ok(metadata) => metadata,
        Err(_) => return Err(CutoverError::Io),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(CutoverError::UnsafeStateDirectory);
    }
    if metadata.len() > MAX_OWNERSHIP_MARKER_BYTES {
        return Err(CutoverError::MarkerTooLarge);
    }
    let raw = read_private_utf8(&paths.ownership_marker, uid)?;
    if raw.len() as u64 > MAX_OWNERSHIP_MARKER_BYTES {
        return Err(CutoverError::MarkerTooLarge);
    }
    let marker: OwnershipMarker =
        serde_json::from_str(&raw).map_err(|_| CutoverError::InvalidMarker)?;
    marker.validate()?;
    Ok(marker)
}

fn legal_successor(current: OwnershipPhase, next: OwnershipPhase) -> bool {
    matches!(
        (current, next),
        (OwnershipPhase::Legacy, OwnershipPhase::CutoverPreparing)
            | (OwnershipPhase::CutoverPreparing, OwnershipPhase::Rust)
            | (OwnershipPhase::CutoverPreparing, OwnershipPhase::Legacy)
            | (OwnershipPhase::Rust, OwnershipPhase::RollbackPreparing)
            | (OwnershipPhase::RollbackPreparing, OwnershipPhase::Legacy)
            | (OwnershipPhase::RollbackPreparing, OwnershipPhase::Rust)
    )
}

pub fn write_marker_locked(
    paths: &CutoverPaths,
    uid: u32,
    lock: &MigrationLock,
    expected: &OwnershipMarker,
    next: &OwnershipMarker,
) -> Result<(), CutoverError> {
    if !lock.authorizes(paths, uid) {
        return Err(CutoverError::InvalidTransition);
    }
    let current = read_marker(paths, uid)?;
    let expected_generation = current
        .generation
        .checked_add(1)
        .ok_or(CutoverError::InvalidTransition)?;
    if &current != expected
        || next.schema_version != OWNERSHIP_SCHEMA_VERSION
        || next.generation != expected_generation
        || !legal_successor(current.phase, next.phase)
    {
        return Err(CutoverError::InvalidTransition);
    }
    let mut payload = serde_json::to_vec(next).map_err(|_| CutoverError::InvalidMarker)?;
    payload.push(b'\n');
    if payload.len() as u64 > MAX_OWNERSHIP_MARKER_BYTES {
        return Err(CutoverError::MarkerTooLarge);
    }
    if !lock.authorizes(paths, uid) {
        return Err(CutoverError::InvalidTransition);
    }
    atomic_replace_private(&paths.ownership_marker, &payload, uid).map_err(Into::into)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipObservation {
    pub legacy_owner_active: bool,
    pub rust_owner_active: bool,
    pub legacy_controller_ready: bool,
    pub rust_controller_ready: bool,
    pub core_count: u8,
    pub tun_count: u8,
    pub active_profile_matches: bool,
}

impl OwnershipObservation {
    #[must_use]
    pub const fn disconnected() -> Self {
        Self {
            legacy_owner_active: false,
            rust_owner_active: false,
            legacy_controller_ready: false,
            rust_controller_ready: false,
            core_count: 0,
            tun_count: 0,
            active_profile_matches: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverBlocker {
    MarkerNotLegacy,
    RustLifecycleAlreadyActive,
    InconsistentHostState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverReadiness {
    ReadyDisconnected,
    ReadyToAdopt,
    Blocked(CutoverBlocker),
}

impl CutoverReadiness {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadyDisconnected => "ready_disconnected",
            Self::ReadyToAdopt => "ready_to_adopt",
            Self::Blocked(CutoverBlocker::MarkerNotLegacy) => "blocked_marker_not_legacy",
            Self::Blocked(CutoverBlocker::RustLifecycleAlreadyActive) => {
                "blocked_rust_lifecycle_active"
            }
            Self::Blocked(CutoverBlocker::InconsistentHostState) => {
                "blocked_inconsistent_host_state"
            }
        }
    }
}

fn settled_disconnected(observation: OwnershipObservation) -> bool {
    !observation.legacy_owner_active
        && !observation.rust_owner_active
        && !observation.legacy_controller_ready
        && !observation.rust_controller_ready
        && observation.core_count == 0
        && observation.tun_count == 0
}

fn settled_legacy_connected(observation: OwnershipObservation) -> bool {
    observation.legacy_owner_active
        && !observation.rust_owner_active
        && observation.legacy_controller_ready
        && !observation.rust_controller_ready
        && observation.core_count == 1
        && observation.tun_count == 1
        && observation.active_profile_matches
}

fn settled_rust(observation: OwnershipObservation) -> bool {
    if !observation.rust_owner_active || observation.legacy_owner_active {
        return false;
    }
    let disconnected = !observation.legacy_controller_ready
        && !observation.rust_controller_ready
        && observation.core_count == 0
        && observation.tun_count == 0;
    let connected = !observation.legacy_controller_ready
        && observation.rust_controller_ready
        && observation.core_count == 1
        && observation.tun_count == 1
        && observation.active_profile_matches;
    disconnected || connected
}

#[must_use]
pub fn evaluate_cutover(
    marker: &OwnershipMarker,
    observation: OwnershipObservation,
) -> CutoverReadiness {
    if marker.phase != OwnershipPhase::Legacy {
        return CutoverReadiness::Blocked(CutoverBlocker::MarkerNotLegacy);
    }
    if observation.rust_owner_active || observation.rust_controller_ready {
        return CutoverReadiness::Blocked(CutoverBlocker::RustLifecycleAlreadyActive);
    }
    if settled_disconnected(observation) {
        CutoverReadiness::ReadyDisconnected
    } else if settled_legacy_connected(observation) {
        CutoverReadiness::ReadyToAdopt
    } else {
        CutoverReadiness::Blocked(CutoverBlocker::InconsistentHostState)
    }
}

pub fn begin_cutover(
    marker: &OwnershipMarker,
    readiness: CutoverReadiness,
) -> Result<OwnershipMarker, CutoverError> {
    if marker.phase != OwnershipPhase::Legacy || matches!(readiness, CutoverReadiness::Blocked(_)) {
        return Err(CutoverError::PreconditionsFailed);
    }
    marker.successor(OwnershipPhase::CutoverPreparing)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustCommitEvidence {
    pub hello_verified: bool,
    pub status_verified: bool,
    pub plugin_bridge_switched: bool,
    pub observation: OwnershipObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyCommitEvidence {
    pub plugin_bridge_legacy: bool,
    pub observation: OwnershipObservation,
}

pub fn commit_cutover(
    marker: &OwnershipMarker,
    evidence: RustCommitEvidence,
) -> Result<OwnershipMarker, CutoverError> {
    if marker.phase != OwnershipPhase::CutoverPreparing
        || !evidence.hello_verified
        || !evidence.status_verified
        || !evidence.plugin_bridge_switched
        || !settled_rust(evidence.observation)
    {
        return Err(CutoverError::PreconditionsFailed);
    }
    marker.successor(OwnershipPhase::Rust)
}

pub fn abort_cutover(
    marker: &OwnershipMarker,
    evidence: LegacyCommitEvidence,
) -> Result<OwnershipMarker, CutoverError> {
    if marker.phase != OwnershipPhase::CutoverPreparing
        || !evidence.plugin_bridge_legacy
        || !(settled_disconnected(evidence.observation)
            || settled_legacy_connected(evidence.observation))
    {
        return Err(CutoverError::PreconditionsFailed);
    }
    marker.successor(OwnershipPhase::Legacy)
}

pub fn begin_rollback(marker: &OwnershipMarker) -> Result<OwnershipMarker, CutoverError> {
    if marker.phase != OwnershipPhase::Rust {
        return Err(CutoverError::PreconditionsFailed);
    }
    marker.successor(OwnershipPhase::RollbackPreparing)
}

pub fn commit_rollback(
    marker: &OwnershipMarker,
    evidence: LegacyCommitEvidence,
) -> Result<OwnershipMarker, CutoverError> {
    if marker.phase != OwnershipPhase::RollbackPreparing
        || !evidence.plugin_bridge_legacy
        || !(settled_disconnected(evidence.observation)
            || settled_legacy_connected(evidence.observation))
    {
        return Err(CutoverError::PreconditionsFailed);
    }
    marker.successor(OwnershipPhase::Legacy)
}

pub fn abort_rollback(
    marker: &OwnershipMarker,
    evidence: RustCommitEvidence,
) -> Result<OwnershipMarker, CutoverError> {
    if marker.phase != OwnershipPhase::RollbackPreparing
        || !evidence.hello_verified
        || !evidence.status_verified
        || !evidence.plugin_bridge_switched
        || !settled_rust(evidence.observation)
    {
        return Err(CutoverError::PreconditionsFailed);
    }
    marker.successor(OwnershipPhase::Rust)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn roots(label: &str) -> (PathBuf, PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "omavless-cutover-{label}-{}-{nonce}",
            std::process::id()
        ));
        let runtime = root.join("runtime");
        let state = root.join("state");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&runtime).unwrap();
        fs::create_dir(&state).unwrap();
        for path in [&root, &runtime, &state] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        (runtime, state, uid)
    }

    fn legacy_connected() -> OwnershipObservation {
        OwnershipObservation {
            legacy_owner_active: true,
            rust_owner_active: false,
            legacy_controller_ready: true,
            rust_controller_ready: false,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        }
    }

    #[test]
    fn transition_bootstrap_requires_preparing_and_derives_exact_successor() {
        let legacy = OwnershipMarker::default();
        assert_eq!(
            TransitionBootstrap::from_preparing(&legacy),
            Err(CutoverError::PreconditionsFailed)
        );

        let preparing = legacy.successor(OwnershipPhase::CutoverPreparing).unwrap();
        let bootstrap = TransitionBootstrap::from_preparing(&preparing).unwrap();
        assert_eq!(bootstrap.preparing_generation(), 1);
        assert_eq!(bootstrap.rust_generation(), 2);

        let exhausted = OwnershipMarker {
            schema_version: OWNERSHIP_SCHEMA_VERSION,
            generation: u64::MAX,
            phase: OwnershipPhase::CutoverPreparing,
        };
        assert_eq!(
            TransitionBootstrap::from_preparing(&exhausted),
            Err(CutoverError::InvalidTransition)
        );
    }

    fn rust_connected() -> OwnershipObservation {
        OwnershipObservation {
            legacy_owner_active: false,
            rust_owner_active: true,
            legacy_controller_ready: false,
            rust_controller_ready: true,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        }
    }

    #[test]
    fn shared_lock_path_is_private_and_singleton() {
        let (runtime, state, uid) = roots("lock");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        assert_eq!(
            paths.operation_lock.file_name().unwrap().to_str().unwrap(),
            format!("omavless.{uid}.lock")
        );
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        assert_eq!(
            fs::metadata(&paths.operation_lock)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(matches!(
            MigrationLock::acquire(&paths, uid),
            Err(CutoverError::Busy)
        ));
        drop(lock);
        assert!(MigrationLock::acquire(&paths, uid).is_ok());
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn recovery_inspection_lock_never_creates_or_repairs_unsafe_inode() {
        let (runtime, state, uid) = roots("inspect-lock");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        assert!(matches!(
            MigrationLock::acquire_existing(&paths, uid),
            Err(CutoverError::UnsafeRuntimeDirectory)
        ));
        assert!(!paths.operation_lock.exists());
        let held = MigrationLock::acquire(&paths, uid).unwrap();
        assert!(matches!(
            MigrationLock::acquire_existing(&paths, uid),
            Err(CutoverError::Busy)
        ));
        drop(held);
        drop(MigrationLock::acquire_existing(&paths, uid).unwrap());

        fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            MigrationLock::acquire_existing(&paths, uid),
            Err(CutoverError::UnsafeRuntimeDirectory)
        ));
        assert_eq!(
            fs::metadata(&paths.operation_lock).unwrap().mode() & 0o777,
            0o644
        );
        fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = runtime.join("synthetic-hardlink");
        fs::hard_link(&paths.operation_lock, &alias).unwrap();
        assert!(matches!(
            MigrationLock::acquire_existing(&paths, uid),
            Err(CutoverError::UnsafeRuntimeDirectory)
        ));
        fs::remove_file(&alias).unwrap();
        fs::remove_file(&paths.operation_lock).unwrap();
        symlink(&alias, &paths.operation_lock).unwrap();
        assert!(matches!(
            MigrationLock::acquire_existing(&paths, uid),
            Err(CutoverError::UnsafeRuntimeDirectory)
        ));
        assert!(
            fs::symlink_metadata(&paths.operation_lock)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn migration_lease_normal_create_race_is_busy_without_touching_winner() {
        for normal in [false, true] {
            let (runtime, state, uid) = roots("lease-create-race");
            let paths = CutoverPaths::below(&runtime, &state, uid);
            let mut winner = None;
            let result = MigrationLock::acquire_checked(
                &paths,
                uid,
                if normal {
                    LockOpen::CreateIfAbsent
                } else {
                    LockOpen::AbsentOnly
                },
                |point| {
                    if point == LockCheckpoint::PriorChecked {
                        fs::write(&paths.operation_lock, b"concurrent synthetic winner").unwrap();
                        fs::set_permissions(
                            &paths.operation_lock,
                            fs::Permissions::from_mode(0o644),
                        )
                        .unwrap();
                        winner = Some(fs::metadata(&paths.operation_lock).unwrap());
                    }
                    true
                },
            );
            assert!(matches!(result, Err(error) if error == if normal {
                CutoverError::Busy
            } else {
                CutoverError::UnsafeRuntimeDirectory
            }));
            let before = winner.unwrap();
            let after = fs::metadata(&paths.operation_lock).unwrap();
            assert!(same_inode(&before, &after));
            assert_eq!(before.mode(), after.mode());
            assert_eq!(
                (before.ctime(), before.ctime_nsec()),
                (after.ctime(), after.ctime_nsec())
            );
            assert_eq!(
                fs::read(&paths.operation_lock).unwrap(),
                b"concurrent synthetic winner"
            );
            fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
        }
    }

    #[test]
    fn migration_lease_absent_only_never_takes_existing_or_raced_name() {
        let (runtime, state, uid) = roots("lease-absent-only");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        let held = MigrationLock::acquire_absent(&paths, uid).unwrap();
        assert!(held.authorizes(&paths, uid));
        assert!(MigrationLock::acquire_absent(&paths, uid).is_err());
        drop(held);
        let identity = fs::metadata(&paths.operation_lock).unwrap();
        assert!(MigrationLock::acquire_absent(&paths, uid).is_err());
        assert!(same_inode(
            &identity,
            &fs::metadata(&paths.operation_lock).unwrap()
        ));
        fs::remove_file(&paths.operation_lock).unwrap();
        let result = MigrationLock::acquire_checked(&paths, uid, LockOpen::AbsentOnly, |point| {
            if point == LockCheckpoint::PriorChecked {
                fs::write(&paths.operation_lock, b"raced synthetic lock").unwrap();
                fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o600))
                    .unwrap();
            }
            true
        });
        assert!(result.is_err());
        assert_eq!(
            fs::read(&paths.operation_lock).unwrap(),
            b"raced synthetic lock"
        );
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn migration_lease_rejects_unlinked_and_replaced_lock_even_with_second_flock() {
        for existing in [false, true] {
            let (runtime, state, uid) = roots("stale-lease");
            let paths = CutoverPaths::below(&runtime, &state, uid);
            let initial = MigrationLock::acquire(&paths, uid).unwrap();
            drop(initial);
            let old = if existing {
                MigrationLock::acquire_existing(&paths, uid)
            } else {
                MigrationLock::acquire(&paths, uid)
            }
            .unwrap();
            assert!(old.authorizes(&paths, uid));
            fs::remove_file(&paths.operation_lock).unwrap();
            assert!(!old.authorizes(&paths, uid));
            let new = MigrationLock::acquire(&paths, uid).unwrap();
            assert!(!old.authorizes(&paths, uid));
            assert!(new.authorizes(&paths, uid));
            assert!(matches!(
                MigrationLock::acquire_existing(&paths, uid),
                Err(CutoverError::Busy)
            ));
            drop(old);
            drop(new);
            fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
        }
    }

    #[test]
    fn migration_lease_stale_holder_cannot_write_owner_marker() {
        let (runtime, state, uid) = roots("lease-marker");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        let old = MigrationLock::acquire(&paths, uid).unwrap();
        let legacy = read_marker(&paths, uid).unwrap();
        let preparing = legacy.successor(OwnershipPhase::CutoverPreparing).unwrap();
        fs::remove_file(&paths.operation_lock).unwrap();
        let new = MigrationLock::acquire(&paths, uid).unwrap();
        assert_eq!(
            write_marker_locked(&paths, uid, &old, &legacy, &preparing),
            Err(CutoverError::InvalidTransition)
        );
        assert!(!paths.ownership_marker.exists());
        write_marker_locked(&paths, uid, &new, &legacy, &preparing).unwrap();
        let committed = preparing.successor(OwnershipPhase::Rust).unwrap();
        let before = fs::read(&paths.ownership_marker).unwrap();
        assert_eq!(
            write_marker_locked(&paths, uid, &old, &preparing, &committed),
            Err(CutoverError::InvalidTransition)
        );
        assert_eq!(fs::read(&paths.ownership_marker).unwrap(), before);
        write_marker_locked(&paths, uid, &new, &preparing, &committed).unwrap();
        drop(old);
        drop(new);
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn migration_lease_constructor_substitution_never_repairs_foreign_path() {
        for existing in [false, true] {
            for point in [
                LockCheckpoint::PriorChecked,
                LockCheckpoint::Opened,
                LockCheckpoint::Locked,
                LockCheckpoint::PermissionsSet,
                LockCheckpoint::Validated,
            ] {
                for foreign_mode in [0o600, 0o644] {
                    let (runtime, state, uid) = roots("lease-substitution");
                    let paths = CutoverPaths::below(&runtime, &state, uid);
                    if existing {
                        drop(MigrationLock::acquire(&paths, uid).unwrap());
                    }
                    let result = MigrationLock::acquire_checked(
                        &paths,
                        uid,
                        if existing {
                            LockOpen::Existing
                        } else {
                            LockOpen::CreateIfAbsent
                        },
                        |checkpoint| {
                            if checkpoint == point {
                                let replacement = runtime.join("replacement");
                                fs::write(&replacement, b"foreign synthetic lock").unwrap();
                                fs::set_permissions(
                                    &replacement,
                                    fs::Permissions::from_mode(foreign_mode),
                                )
                                .unwrap();
                                fs::rename(replacement, &paths.operation_lock).unwrap();
                            }
                            true
                        },
                    );
                    assert!(result.is_err(), "existing={existing} {point:?}");
                    assert_eq!(
                        fs::read(&paths.operation_lock).unwrap(),
                        b"foreign synthetic lock"
                    );
                    assert_eq!(
                        fs::metadata(&paths.operation_lock).unwrap().mode() & 0o7777,
                        foreign_mode
                    );
                    fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
                }
            }
        }
    }

    #[test]
    fn migration_lease_parent_replacement_and_unsafe_names_refuse() {
        for existing in [false, true] {
            for point in [
                LockCheckpoint::RuntimeOpened,
                LockCheckpoint::Opened,
                LockCheckpoint::Validated,
            ] {
                let (runtime, state, uid) = roots("lease-parent");
                let paths = CutoverPaths::below(&runtime, &state, uid);
                if existing {
                    drop(MigrationLock::acquire(&paths, uid).unwrap());
                }
                let result = MigrationLock::acquire_checked(
                    &paths,
                    uid,
                    if existing {
                        LockOpen::Existing
                    } else {
                        LockOpen::CreateIfAbsent
                    },
                    |checkpoint| {
                        if checkpoint == point {
                            let saved = runtime.with_extension("saved");
                            fs::rename(&runtime, &saved).unwrap();
                            fs::create_dir(&runtime).unwrap();
                            fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
                                .unwrap();
                            if saved.join(format!("omavless.{uid}.lock")).exists() {
                                fs::rename(
                                    saved.join(format!("omavless.{uid}.lock")),
                                    &paths.operation_lock,
                                )
                                .unwrap();
                            }
                        }
                        true
                    },
                );
                assert!(result.is_err(), "existing={existing} {point:?}");
                fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
            }
        }
        for kind in ["mode", "hardlink", "symlink", "fifo"] {
            let (runtime, state, uid) = roots("lease-unsafe");
            let paths = CutoverPaths::below(&runtime, &state, uid);
            drop(MigrationLock::acquire(&paths, uid).unwrap());
            match kind {
                "mode" => {
                    fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o666))
                        .unwrap()
                }
                "hardlink" => fs::hard_link(&paths.operation_lock, runtime.join("alias")).unwrap(),
                "symlink" => {
                    fs::remove_file(&paths.operation_lock).unwrap();
                    symlink("missing", &paths.operation_lock).unwrap();
                }
                _ => {
                    fs::remove_file(&paths.operation_lock).unwrap();
                    nix::unistd::mkfifo(
                        &paths.operation_lock,
                        nix::sys::stat::Mode::from_bits_truncate(0o600),
                    )
                    .unwrap();
                }
            }
            assert!(MigrationLock::acquire(&paths, uid).is_err(), "{kind}");
            assert!(
                MigrationLock::acquire_existing(&paths, uid).is_err(),
                "{kind}"
            );
            fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
        }
    }

    #[test]
    fn migration_lease_legacy_0644_busy_is_not_repaired_until_exclusive_flock() {
        let (runtime, state, uid) = roots("lease-legacy-mode");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        let old = MigrationLock::acquire(&paths, uid).unwrap();
        fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o644)).unwrap();
        let identity = fs::metadata(&paths.operation_lock).unwrap();
        assert!(matches!(
            MigrationLock::acquire(&paths, uid),
            Err(CutoverError::Busy)
        ));
        assert_eq!(
            fs::metadata(&paths.operation_lock).unwrap().mode() & 0o7777,
            0o644
        );
        assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
        drop(old);
        let migrated = MigrationLock::acquire(&paths, uid).unwrap();
        assert!(migrated.authorizes(&paths, uid));
        let now = fs::metadata(&paths.operation_lock).unwrap();
        assert!(same_inode(&identity, &now));
        assert_eq!(now.mode() & 0o7777, 0o600);
        drop(migrated);
        drop(MigrationLock::acquire_existing(&paths, uid).unwrap());
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn migration_lease_authority_rechecks_parent_mode_links_and_foreign_binding() {
        for kind in [
            "parent",
            "mode",
            "hardlink",
            "symlink",
            "foreign-path",
            "foreign-uid",
        ] {
            let (runtime, state, uid) = roots("lease-authority");
            let paths = CutoverPaths::below(&runtime, &state, uid);
            let held = MigrationLock::acquire(&paths, uid).unwrap();
            let mut check_paths = paths.clone();
            let mut check_uid = uid;
            match kind {
                "parent" => {
                    let saved = runtime.with_extension("saved");
                    fs::rename(&runtime, &saved).unwrap();
                    fs::create_dir(&runtime).unwrap();
                    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
                    fs::rename(
                        saved.join(format!("omavless.{uid}.lock")),
                        &paths.operation_lock,
                    )
                    .unwrap();
                }
                "mode" => {
                    fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o644))
                        .unwrap()
                }
                "hardlink" => fs::hard_link(&paths.operation_lock, runtime.join("alias")).unwrap(),
                "symlink" => {
                    let saved = runtime.join("saved-lock");
                    fs::rename(&paths.operation_lock, &saved).unwrap();
                    symlink(&saved, &paths.operation_lock).unwrap();
                }
                "foreign-path" => check_paths.operation_lock = runtime.join("other.lock"),
                _ => check_uid = uid.wrapping_add(1),
            }
            assert!(!held.authorizes(&check_paths, check_uid), "{kind}");
            drop(held);
            fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
        }
    }

    #[test]
    fn absent_marker_defaults_legacy_and_transitions_are_private() {
        let (runtime, state, uid) = roots("marker");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap();
        let paths = CutoverPaths::below(&runtime, &state, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        let legacy = read_marker(&paths, uid).unwrap();
        assert_eq!(legacy.phase(), OwnershipPhase::Legacy);
        assert_eq!(legacy.generation(), 0);
        let preparing = begin_cutover(
            &legacy,
            evaluate_cutover(&legacy, OwnershipObservation::disconnected()),
        )
        .unwrap();
        write_marker_locked(&paths, uid, &lock, &legacy, &preparing).unwrap();
        assert_eq!(read_marker(&paths, uid).unwrap(), preparing);
        assert_eq!(
            fs::metadata(&paths.state_directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&paths.ownership_marker)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn stale_and_illegal_marker_writes_are_rejected() {
        let (runtime, state, uid) = roots("stale");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        let legacy = read_marker(&paths, uid).unwrap();
        let preparing = begin_cutover(&legacy, CutoverReadiness::ReadyDisconnected).unwrap();
        write_marker_locked(&paths, uid, &lock, &legacy, &preparing).unwrap();
        assert_eq!(
            write_marker_locked(&paths, uid, &lock, &legacy, &preparing),
            Err(CutoverError::InvalidTransition)
        );
        let rust = preparing.successor(OwnershipPhase::Rust).unwrap();
        let other_runtime = runtime.parent().unwrap().join("other-runtime");
        fs::create_dir(&other_runtime).unwrap();
        fs::set_permissions(&other_runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let wrong_paths = CutoverPaths::below(&other_runtime, &state, uid);
        assert_eq!(
            write_marker_locked(&wrong_paths, uid, &lock, &preparing, &rust),
            Err(CutoverError::InvalidTransition)
        );
        let exhausted: OwnershipMarker = serde_json::from_str(&format!(
            r#"{{"schemaVersion":1,"generation":{},"phase":"legacy"}}"#,
            u64::MAX
        ))
        .unwrap();
        assert_eq!(
            begin_cutover(&exhausted, CutoverReadiness::ReadyDisconnected),
            Err(CutoverError::InvalidTransition)
        );
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_duplicate_oversized_and_symlinked_markers_fail_closed() {
        let (runtime, state, uid) = roots("invalid");
        let paths = CutoverPaths::below(&runtime, &state, uid);
        fs::create_dir(&paths.state_directory).unwrap();
        fs::set_permissions(&paths.state_directory, fs::Permissions::from_mode(0o700)).unwrap();
        for payload in [
            r#"{"schemaVersion":1,"generation":0,"phase":"legacy","phase":"rust"}"#,
            r#"{"schemaVersion":1,"generation":0,"phase":"legacy","extra":true}"#,
            r#"{"schemaVersion":2,"generation":0,"phase":"legacy"}"#,
        ] {
            fs::write(&paths.ownership_marker, payload).unwrap();
            fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o600))
                .unwrap();
            assert!(matches!(
                read_marker(&paths, uid),
                Err(CutoverError::InvalidMarker)
            ));
        }
        fs::write(
            &paths.ownership_marker,
            vec![b'x'; MAX_OWNERSHIP_MARKER_BYTES as usize + 1],
        )
        .unwrap();
        assert_eq!(read_marker(&paths, uid), Err(CutoverError::MarkerTooLarge));
        fs::remove_file(&paths.ownership_marker).unwrap();
        let target = state.join("private-target");
        fs::write(&target, "unchanged").unwrap();
        symlink(&target, &paths.ownership_marker).unwrap();
        assert_eq!(
            read_marker(&paths, uid),
            Err(CutoverError::UnsafeStateDirectory)
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "unchanged");
        fs::remove_dir_all(runtime.parent().unwrap()).unwrap();
    }

    #[test]
    fn preflight_accepts_only_empty_or_one_healthy_legacy_owner() {
        let marker = OwnershipMarker::default();
        assert_eq!(
            evaluate_cutover(&marker, OwnershipObservation::disconnected()),
            CutoverReadiness::ReadyDisconnected
        );
        assert_eq!(
            evaluate_cutover(&marker, legacy_connected()),
            CutoverReadiness::ReadyToAdopt
        );
        let mut duplicate = legacy_connected();
        duplicate.rust_owner_active = true;
        assert_eq!(
            evaluate_cutover(&marker, duplicate),
            CutoverReadiness::Blocked(CutoverBlocker::RustLifecycleAlreadyActive)
        );
        let mut partial = legacy_connected();
        partial.legacy_controller_ready = false;
        assert_eq!(
            evaluate_cutover(&marker, partial),
            CutoverReadiness::Blocked(CutoverBlocker::InconsistentHostState)
        );
    }

    #[test]
    fn cutover_and_rollback_require_verified_settled_ownership() {
        let legacy = OwnershipMarker::default();
        let preparing = begin_cutover(&legacy, CutoverReadiness::ReadyToAdopt).unwrap();
        let evidence = RustCommitEvidence {
            hello_verified: true,
            status_verified: true,
            plugin_bridge_switched: true,
            observation: rust_connected(),
        };
        let rust = commit_cutover(&preparing, evidence).unwrap();
        assert_eq!(rust.phase(), OwnershipPhase::Rust);
        let rollback = begin_rollback(&rust).unwrap();
        let legacy_again = commit_rollback(
            &rollback,
            LegacyCommitEvidence {
                plugin_bridge_legacy: true,
                observation: legacy_connected(),
            },
        )
        .unwrap();
        assert_eq!(legacy_again.phase(), OwnershipPhase::Legacy);
        assert_eq!(legacy_again.generation(), 4);

        let mut broken = evidence;
        broken.observation.legacy_owner_active = true;
        assert_eq!(
            commit_cutover(&preparing, broken),
            Err(CutoverError::PreconditionsFailed)
        );
        assert_eq!(
            abort_rollback(&rollback, broken),
            Err(CutoverError::PreconditionsFailed)
        );
    }

    #[test]
    fn errors_never_include_private_paths_or_values() {
        let marker = "/private.example/password";
        let error = CutoverPaths::current(u32::MAX)
            .and_then(|paths| read_marker(&paths, u32::MAX))
            .unwrap_err();
        let output = format!("{error:?} {error}");
        assert!(!output.contains(marker));
        assert!(!output.contains("private.example"));
        assert!(!output.contains("password"));
    }
}
