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
    uid: u32,
    _file: Flock<File>,
}

impl MigrationLock {
    pub fn acquire(paths: &CutoverPaths, uid: u32) -> Result<Self, CutoverError> {
        let metadata = fs::symlink_metadata(&paths.runtime_base)
            .map_err(|_| CutoverError::UnsafeRuntimeDirectory)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != uid
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(CutoverError::UnsafeRuntimeDirectory);
        }
        if let Ok(metadata) = fs::symlink_metadata(&paths.operation_lock)
            && (metadata.file_type().is_symlink() || !metadata.is_file() || metadata.uid() != uid)
        {
            return Err(CutoverError::UnsafeRuntimeDirectory);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(OFlag::O_NOFOLLOW.bits())
            .open(&paths.operation_lock)
            .map_err(|_| CutoverError::Io)?;
        let file =
            Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_file, error)| {
                if matches!(error, Errno::EAGAIN) {
                    CutoverError::Busy
                } else {
                    CutoverError::Io
                }
            })?;
        fs::set_permissions(&paths.operation_lock, fs::Permissions::from_mode(0o600))
            .map_err(|_| CutoverError::Io)?;
        Ok(Self {
            path: paths.operation_lock.clone(),
            uid,
            _file: file,
        })
    }
}

pub fn read_marker(paths: &CutoverPaths, uid: u32) -> Result<OwnershipMarker, CutoverError> {
    prepare_state_directory(&paths.state_directory, uid)?;
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
    if lock.uid != uid || lock.path != paths.operation_lock {
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
