// SPDX-License-Identifier: MIT

//! Generation-fenced target state for the Omarchy compatibility bridge.
//!
//! This module does not expose a cutover command and does not dispatch plugin
//! operations. It provides the durable, private selector which a later
//! semantic plugin bridge must consult. The selector is valid only alongside
//! the exact ownership transition which wrote it, so a stale file can never
//! silently route the frontend to the wrong lifecycle owner.

use crate::cutover::{CutoverPaths, MigrationLock, OwnershipMarker, OwnershipPhase, read_marker};
use crate::cutover_transaction::{BridgeTarget, CutoverHostError};
use crate::production_cutover::ProductionPluginBridge;
use nix::unistd::Uid;
use omavless_store::{atomic_replace_private, read_private_utf8};
use std::env;
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const FRONTEND_BRIDGE_TARGET_NAME: &str = "frontend-bridge.target";
pub const MAX_FRONTEND_BRIDGE_TARGET_BYTES: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendBridgeError {
    UnsafeState,
    InvalidState,
    StaleState,
    Unauthorized,
    Io,
}

impl fmt::Display for FrontendBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeState => "OmaVLESS frontend bridge state is unsafe",
            Self::InvalidState => "OmaVLESS frontend bridge state is invalid",
            Self::StaleState => "OmaVLESS frontend bridge state is stale",
            Self::Unauthorized => "OmaVLESS frontend bridge update is not authorized",
            Self::Io => "OmaVLESS frontend bridge state I/O failed",
        })
    }
}

impl std::error::Error for FrontendBridgeError {}

/// Fixed private path for one user's compatibility-bridge selector.
#[derive(Clone)]
pub struct FrontendBridgePaths {
    target: PathBuf,
}

impl FrontendBridgePaths {
    #[must_use]
    fn below(state_base: &Path) -> Self {
        Self {
            target: state_base
                .join("omavless")
                .join(FRONTEND_BRIDGE_TARGET_NAME),
        }
    }

    pub fn current() -> Result<Self, FrontendBridgeError> {
        let state_base = match env::var_os("XDG_STATE_HOME") {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(env::var_os("HOME").ok_or(FrontendBridgeError::UnsafeState)?)
                .join(".local/state"),
        };
        if !state_base.is_absolute() {
            return Err(FrontendBridgeError::UnsafeState);
        }
        Ok(Self::below(&state_base))
    }
}

#[derive(Clone, Copy)]
struct Selector {
    target: BridgeTarget,
    preparing_generation: u64,
}

fn parse_selector(raw: &str) -> Result<Selector, FrontendBridgeError> {
    let line = raw
        .strip_suffix('\n')
        .ok_or(FrontendBridgeError::InvalidState)?;
    if line.contains(['\n', '\r']) {
        return Err(FrontendBridgeError::InvalidState);
    }
    let (target, generation) = line
        .split_once(':')
        .ok_or(FrontendBridgeError::InvalidState)?;
    let target = match target {
        "legacy" => BridgeTarget::Legacy,
        "rust" => BridgeTarget::Rust,
        _ => return Err(FrontendBridgeError::InvalidState),
    };
    let preparing_generation = generation
        .parse::<u64>()
        .map_err(|_| FrontendBridgeError::InvalidState)?;
    if generation != preparing_generation.to_string() {
        return Err(FrontendBridgeError::InvalidState);
    }
    Ok(Selector {
        target,
        preparing_generation,
    })
}

fn selector_bytes(target: BridgeTarget, preparing_generation: u64) -> Vec<u8> {
    format!(
        "{}:{preparing_generation}\n",
        match target {
            BridgeTarget::Legacy => "legacy",
            BridgeTarget::Rust => "rust",
        }
    )
    .into_bytes()
}

fn read_selector(
    paths: &FrontendBridgePaths,
    uid: u32,
) -> Result<Option<Selector>, FrontendBridgeError> {
    let metadata = match fs::symlink_metadata(&paths.target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Ok(metadata) => metadata,
        Err(_) => return Err(FrontendBridgeError::Io),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(FrontendBridgeError::UnsafeState);
    }
    if metadata.len() > MAX_FRONTEND_BRIDGE_TARGET_BYTES {
        return Err(FrontendBridgeError::InvalidState);
    }
    let raw = read_private_utf8(&paths.target, uid).map_err(|error| match error {
        omavless_store::StoreIoError::UnsafePath | omavless_store::StoreIoError::WrongOwner => {
            FrontendBridgeError::UnsafeState
        }
        omavless_store::StoreIoError::TooLarge | omavless_store::StoreIoError::InvalidUtf8 => {
            FrontendBridgeError::InvalidState
        }
        omavless_store::StoreIoError::Io => FrontendBridgeError::Io,
    })?;
    if raw.len() as u64 > MAX_FRONTEND_BRIDGE_TARGET_BYTES {
        return Err(FrontendBridgeError::InvalidState);
    }
    parse_selector(&raw).map(Some)
}

fn selector_matches(
    selector: Selector,
    marker: &OwnershipMarker,
) -> Result<BridgeTarget, FrontendBridgeError> {
    let preparing = selector.preparing_generation;
    let committed = preparing
        .checked_add(1)
        .ok_or(FrontendBridgeError::StaleState)?;
    let matches = match (selector.target, marker.phase()) {
        (BridgeTarget::Rust, OwnershipPhase::CutoverPreparing) => marker.generation() == preparing,
        (BridgeTarget::Rust, OwnershipPhase::Rust) => marker.generation() == committed,
        (BridgeTarget::Legacy, OwnershipPhase::CutoverPreparing) => {
            marker.generation() == preparing
        }
        (BridgeTarget::Legacy, OwnershipPhase::Legacy) => marker.generation() == committed,
        _ => false,
    };
    matches
        .then_some(selector.target)
        .ok_or(FrontendBridgeError::StaleState)
}

/// Read the effective frontend target without guessing across transition
/// races. An absent selector is compatible only with an exact legacy marker;
/// every preparing, Rust, rollback, malformed, unsafe or stale combination
/// fails closed.
pub fn read_frontend_bridge_target(
    paths: &FrontendBridgePaths,
    cutover: &CutoverPaths,
    uid: u32,
) -> Result<BridgeTarget, FrontendBridgeError> {
    let marker = read_marker(cutover, uid).map_err(|_| FrontendBridgeError::InvalidState)?;
    match read_selector(paths, uid)? {
        None if marker.phase() == OwnershipPhase::Legacy => Ok(BridgeTarget::Legacy),
        None => Err(FrontendBridgeError::StaleState),
        Some(selector) => selector_matches(selector, &marker),
    }
}

/// Fixed-purpose bridge implementation for the production cutover host.
/// Construction and reads are inert; a switch requires the exact migration
/// lock and durable preparing marker supplied by that host.
pub struct FixedFrontendBridge {
    paths: FrontendBridgePaths,
    cutover: CutoverPaths,
    uid: u32,
}

impl FixedFrontendBridge {
    #[must_use]
    fn below(paths: FrontendBridgePaths, cutover: CutoverPaths, uid: u32) -> Self {
        Self {
            paths,
            cutover,
            uid,
        }
    }

    pub fn current() -> Result<Self, FrontendBridgeError> {
        let uid = Uid::current().as_raw();
        Ok(Self::below(
            FrontendBridgePaths::current()?,
            CutoverPaths::current(uid).map_err(|_| FrontendBridgeError::UnsafeState)?,
            uid,
        ))
    }

    fn switch_locked(
        &mut self,
        target: BridgeTarget,
        marker: &OwnershipMarker,
        lock: &MigrationLock,
    ) -> Result<(), FrontendBridgeError> {
        if !lock.authorizes(&self.cutover, self.uid) {
            return Err(FrontendBridgeError::Unauthorized);
        }
        let current =
            read_marker(&self.cutover, self.uid).map_err(|_| FrontendBridgeError::InvalidState)?;
        if &current != marker || marker.phase() != OwnershipPhase::CutoverPreparing {
            return Err(FrontendBridgeError::StaleState);
        }
        let payload = selector_bytes(target, marker.generation());
        if payload.len() as u64 > MAX_FRONTEND_BRIDGE_TARGET_BYTES {
            return Err(FrontendBridgeError::InvalidState);
        }
        atomic_replace_private(&self.paths.target, &payload, self.uid).map_err(
            |error| match error {
                omavless_store::StoreIoError::UnsafePath
                | omavless_store::StoreIoError::WrongOwner => FrontendBridgeError::UnsafeState,
                omavless_store::StoreIoError::TooLarge
                | omavless_store::StoreIoError::InvalidUtf8 => FrontendBridgeError::InvalidState,
                omavless_store::StoreIoError::Io => FrontendBridgeError::Io,
            },
        )?;
        let effective = read_frontend_bridge_target(&self.paths, &self.cutover, self.uid)?;
        (effective == target)
            .then_some(())
            .ok_or(FrontendBridgeError::StaleState)
    }
}

impl ProductionPluginBridge for FixedFrontendBridge {
    fn switch(
        &mut self,
        target: BridgeTarget,
        marker: &OwnershipMarker,
        lock: &MigrationLock,
    ) -> Result<(), CutoverHostError> {
        self.switch_locked(target, marker, lock)
            .map_err(|_| CutoverHostError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::{
        LegacyCommitEvidence, OwnershipObservation, RustCommitEvidence, abort_cutover,
        begin_cutover, commit_cutover, write_marker_locked,
    };
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture {
        root: PathBuf,
        uid: u32,
        paths: FrontendBridgePaths,
        cutover: CutoverPaths,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = env::temp_dir().join(format!(
                "omavless-frontend-bridge-{label}-{}-{nonce}",
                std::process::id()
            ));
            let runtime = root.join("runtime");
            let state = root.join("state");
            for path in [&root, &runtime, &state] {
                fs::create_dir(path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            Self {
                paths: FrontendBridgePaths::below(&state),
                cutover: CutoverPaths::below(&runtime, &state, uid),
                root,
                uid,
            }
        }

        fn begin(&self) -> (MigrationLock, OwnershipMarker) {
            let lock = MigrationLock::acquire(&self.cutover, self.uid).unwrap();
            let legacy = read_marker(&self.cutover, self.uid).unwrap();
            let preparing =
                begin_cutover(&legacy, crate::cutover::CutoverReadiness::ReadyDisconnected)
                    .unwrap();
            write_marker_locked(&self.cutover, self.uid, &lock, &legacy, &preparing).unwrap();
            (lock, preparing)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn disconnected() -> OwnershipObservation {
        OwnershipObservation::disconnected()
    }

    #[test]
    fn absent_state_is_legacy_only_and_preparing_fails_closed() {
        let fixture = Fixture::new("absent");
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid).unwrap(),
            BridgeTarget::Legacy
        );
        let (_lock, _preparing) = fixture.begin();
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid),
            Err(FrontendBridgeError::StaleState)
        );
    }

    #[test]
    fn rust_target_is_private_and_survives_only_its_exact_commit() {
        let fixture = Fixture::new("rust");
        let (lock, preparing) = fixture.begin();
        let mut bridge =
            FixedFrontendBridge::below(fixture.paths.clone(), fixture.cutover.clone(), fixture.uid);
        bridge
            .switch_locked(BridgeTarget::Rust, &preparing, &lock)
            .unwrap();
        assert_eq!(
            fs::metadata(&fixture.paths.target)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let rust = commit_cutover(
            &preparing,
            RustCommitEvidence {
                hello_verified: true,
                status_verified: true,
                plugin_bridge_switched: true,
                observation: OwnershipObservation {
                    rust_owner_active: true,
                    rust_controller_ready: true,
                    core_count: 1,
                    tun_count: 1,
                    active_profile_matches: true,
                    ..disconnected()
                },
            },
        )
        .unwrap();
        write_marker_locked(&fixture.cutover, fixture.uid, &lock, &preparing, &rust).unwrap();
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid).unwrap(),
            BridgeTarget::Rust
        );
    }

    #[test]
    fn legacy_target_covers_compensation_window_and_exact_abort() {
        let fixture = Fixture::new("legacy");
        let (lock, preparing) = fixture.begin();
        let mut bridge =
            FixedFrontendBridge::below(fixture.paths.clone(), fixture.cutover.clone(), fixture.uid);
        bridge
            .switch_locked(BridgeTarget::Legacy, &preparing, &lock)
            .unwrap();
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid).unwrap(),
            BridgeTarget::Legacy
        );
        let legacy = abort_cutover(
            &preparing,
            LegacyCommitEvidence {
                plugin_bridge_legacy: true,
                observation: disconnected(),
            },
        )
        .unwrap();
        write_marker_locked(&fixture.cutover, fixture.uid, &lock, &preparing, &legacy).unwrap();
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid).unwrap(),
            BridgeTarget::Legacy
        );
    }

    #[test]
    fn stale_malformed_oversized_and_symlinked_state_fail_closed() {
        let fixture = Fixture::new("unsafe");
        let (_lock, _preparing) = fixture.begin();
        for payload in ["rust:00\n", "rust:1\nextra\n", "unknown:1\n"] {
            fs::write(&fixture.paths.target, payload).unwrap();
            fs::set_permissions(&fixture.paths.target, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(
                read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid).is_err()
            );
        }
        fs::write(
            &fixture.paths.target,
            vec![b'x'; MAX_FRONTEND_BRIDGE_TARGET_BYTES as usize + 1],
        )
        .unwrap();
        fs::set_permissions(&fixture.paths.target, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid),
            Err(FrontendBridgeError::InvalidState)
        );
        fs::write(&fixture.paths.target, "rust:1\n").unwrap();
        fs::set_permissions(&fixture.paths.target, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid),
            Err(FrontendBridgeError::UnsafeState)
        );
        fs::remove_file(&fixture.paths.target).unwrap();
        let destination = fixture.root.join("private.example-password");
        fs::write(&destination, "unchanged").unwrap();
        symlink(&destination, &fixture.paths.target).unwrap();
        let error =
            read_frontend_bridge_target(&fixture.paths, &fixture.cutover, fixture.uid).unwrap_err();
        assert_eq!(error, FrontendBridgeError::UnsafeState);
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("private.example"));
        assert!(!rendered.contains("password"));
    }

    #[test]
    fn wrong_lock_or_changed_marker_cannot_switch() {
        let fixture = Fixture::new("authorization");
        let (lock, preparing) = fixture.begin();
        let other_runtime = fixture.root.join("other-runtime");
        fs::create_dir(&other_runtime).unwrap();
        fs::set_permissions(&other_runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let other_cutover = CutoverPaths::below(
            &other_runtime,
            fixture.cutover.state_directory.parent().unwrap(),
            fixture.uid,
        );
        let other_lock = MigrationLock::acquire(&other_cutover, fixture.uid).unwrap();
        let mut bridge =
            FixedFrontendBridge::below(fixture.paths.clone(), fixture.cutover.clone(), fixture.uid);
        assert_eq!(
            bridge.switch_locked(BridgeTarget::Rust, &preparing, &other_lock),
            Err(FrontendBridgeError::Unauthorized)
        );
        let legacy = abort_cutover(
            &preparing,
            LegacyCommitEvidence {
                plugin_bridge_legacy: true,
                observation: disconnected(),
            },
        )
        .unwrap();
        write_marker_locked(&fixture.cutover, fixture.uid, &lock, &preparing, &legacy).unwrap();
        assert_eq!(
            bridge.switch_locked(BridgeTarget::Rust, &preparing, &lock),
            Err(FrontendBridgeError::StaleState)
        );
        assert!(!fixture.paths.target.exists());
    }
}
