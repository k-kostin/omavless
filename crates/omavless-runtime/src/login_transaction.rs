// SPDX-License-Identifier: MIT
//! OFFLINE login receipt transaction. No production caller or activation exists.
//!
//! A trusted future host supplies an epoch; hashing it does not prove a new login.
//! That host must order this transaction before daemon startup and make startup
//! honor pending receipts. Currently startup does neither: this module alone is
//! NOT a production crash-safety or once-per-login guarantee. Receipts live below
//! the user runtime base, outside the removable daemon runtime directory. They
//! do not provide recovery across user-manager teardown or reboot.
//!
//! Order: owner lock, migration lock, exact ownership/snapshots, read-only host
//! validation, pending receipt, desired publication/readback, consumed receipt.
//! There is no multi-file atomicity or automatic rollback. An uncertain write
//! retains its receipt and returns manual recovery. A consumed publication may
//! report failure after rename; a retry can recognize its exact epoch/owner and
//! preserve current desired state, but never reapply login preferences. Pending
//! or corrupt receipts always block. Directory-fsync uncertainty is not hidden.

use crate::cutover::{CutoverError, CutoverPaths, MigrationLock, OwnershipPhase, read_marker};
use crate::desired::{DesiredPaths, DesiredState, MAX_DESIRED_STATE_BYTES, MAX_GENERATION};
use crate::login_intent::{LoginIntentError, LoginTrigger, plan_login_intent};
use crate::{OwnerLock, RuntimeError, RuntimePaths};
use nix::unistd::Uid;
use omavless_domain::config::MAX_TEMPLATE_BYTES;
use omavless_domain::private_store::{PrivateStore, parse_private_store};
use omavless_store::{atomic_replace_private, read_private_utf8};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const RECEIPT_NAME: &str = "omavless-login.receipt";
const MAX_RECEIPT_BYTES: u64 = 1024;
const MAX_EPOCH_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginTransactionError {
    InvalidInput,
    Busy,
    OwnershipUnavailable,
    InvalidState,
    EpochMismatch,
    ValidationRejected,
    SnapshotChanged,
    Plan(LoginIntentError),
    ManualRecoveryRequired,
}

impl fmt::Display for LoginTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "OmaVLESS login transaction input is invalid",
            Self::Busy => "OmaVLESS login transaction is busy",
            Self::OwnershipUnavailable => "OmaVLESS login ownership is unavailable",
            Self::InvalidState => "OmaVLESS login state is invalid",
            Self::EpochMismatch => "OmaVLESS login receipt does not match this session",
            Self::ValidationRejected => "OmaVLESS login host validation failed",
            Self::SnapshotChanged => "OmaVLESS login state changed during validation",
            Self::Plan(_) => "OmaVLESS login intent could not be planned",
            Self::ManualRecoveryRequired => "OmaVLESS login requires manual recovery",
        })
    }
}
impl std::error::Error for LoginTransactionError {}
type Result<T> = std::result::Result<T, LoginTransactionError>;

/// Trusted construction only, not client-supplied paths. Fields remain private.
pub struct LoginPaths {
    runtime: RuntimePaths,
    cutover: CutoverPaths,
    desired: DesiredPaths,
    store: PathBuf,
    template: PathBuf,
    receipt: PathBuf,
    uid: u32,
}

impl LoginPaths {
    pub fn below(home: &Path, runtime_base: &Path, state_base: &Path, uid: u32) -> Result<Self> {
        if uid != Uid::current().as_raw()
            || [home, runtime_base, state_base].iter().any(|path| {
                !path.is_absolute()
                    || path.as_os_str().len() > 4096
                    || path
                        .components()
                        .any(|part| matches!(part, std::path::Component::ParentDir))
            })
        {
            return Err(LoginTransactionError::InvalidInput);
        }
        let config = home.join(".config/omavless");
        Ok(Self {
            runtime: RuntimePaths::below(runtime_base),
            cutover: CutoverPaths::below(runtime_base, state_base, uid),
            desired: DesiredPaths::below(state_base),
            store: config.join("profiles.json"),
            template: config.join("route-template.yaml"),
            receipt: runtime_base.join(RECEIPT_NAME),
            uid,
        })
    }

    fn validate(&self) -> Result<()> {
        for path in [
            &self.cutover.runtime_base,
            &self.runtime.directory,
            &self.desired.directory,
            self.store
                .parent()
                .ok_or(LoginTransactionError::InvalidInput)?,
        ] {
            let metadata =
                fs::symlink_metadata(path).map_err(|_| LoginTransactionError::InvalidState)?;
            if fs::canonicalize(path).ok().as_deref() != Some(path)
                || !metadata.is_dir()
                || metadata.uid() != self.uid
                || metadata.mode() & 0o7777 != 0o700
            {
                return Err(LoginTransactionError::InvalidState);
            }
        }
        Ok(())
    }
}

/// Fixed opaque host rejection; no command output or private input is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginHostError;

/// Fixed-purpose read-only validation. Implementations must not start/stop a
/// service/core or mutate private state. The isolated tests deliberately violate
/// that promise to verify post-validation fencing. No generic command API exists.
pub trait LoginReadiness {
    fn verify_empty(&mut self) -> std::result::Result<(), LoginHostError>;
    fn validate_candidate(
        &mut self,
        desired: &DesiredState,
        store: &PrivateStore,
        template: &str,
    ) -> std::result::Result<(), LoginHostError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginTransactionOutcome {
    Consumed { desired_changed: bool },
    AlreadyConsumed,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: u8,
    epoch_hash: String,
    ownership_generation: u64,
    phase: Phase,
}
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Pending,
    Consumed,
}

fn private_optional(path: &Path, uid: u32, maximum: u64) -> Result<Option<String>> {
    let before = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(LoginTransactionError::InvalidState),
    };
    let safe = |metadata: &fs::Metadata| {
        metadata.is_file()
            && metadata.uid() == uid
            && metadata.mode() & 0o7777 == 0o600
            && metadata.len() <= maximum
    };
    if !safe(&before) {
        return Err(LoginTransactionError::InvalidState);
    }
    let raw = read_private_utf8(path, uid).map_err(|_| LoginTransactionError::InvalidState)?;
    let after = fs::symlink_metadata(path).map_err(|_| LoginTransactionError::InvalidState)?;
    if !safe(&after)
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || raw.len() as u64 != after.len()
        || raw.len() as u64 > maximum
    {
        return Err(LoginTransactionError::InvalidState);
    }
    Ok(Some(raw))
}

fn required(path: &Path, uid: u32, limit: u64) -> Result<String> {
    private_optional(path, uid, limit)?.ok_or(LoginTransactionError::InvalidState)
}

#[derive(PartialEq, Eq)]
struct Snapshot {
    marker: String,
    store: String,
    template: Option<String>,
    desired: Option<String>,
}
impl Snapshot {
    fn read(paths: &LoginPaths) -> Result<Self> {
        Ok(Self {
            marker: required(&paths.cutover.ownership_marker, paths.uid, 1024)?,
            store: required(
                &paths.store,
                paths.uid,
                omavless_store::MAX_STORE_BYTES as u64,
            )?,
            template: private_optional(&paths.template, paths.uid, MAX_TEMPLATE_BYTES as u64)?,
            desired: private_optional(&paths.desired.file, paths.uid, MAX_DESIRED_STATE_BYTES)?,
        })
    }
}

fn receipt(paths: &LoginPaths) -> Result<Option<Receipt>> {
    let raw = private_optional(&paths.receipt, paths.uid, MAX_RECEIPT_BYTES)
        .map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
    raw.map(|raw| {
        let parsed: Receipt = serde_json::from_str(&raw)
            .map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
        if parsed.schema_version != 1
            || parsed.epoch_hash.len() != 64
            || !parsed
                .epoch_hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || parsed.ownership_generation == 0
            || parsed.ownership_generation > MAX_GENERATION
        {
            return Err(LoginTransactionError::ManualRecoveryRequired);
        }
        Ok(parsed)
    })
    .transpose()
}

fn exact_owner(paths: &LoginPaths, generation: u64) -> Result<()> {
    let marker = read_marker(&paths.cutover, paths.uid)
        .map_err(|_| LoginTransactionError::OwnershipUnavailable)?;
    if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
        return Err(LoginTransactionError::OwnershipUnavailable);
    }
    Ok(())
}

fn desired_from_snapshot(raw: Option<&str>) -> Result<DesiredState> {
    let value: DesiredState = match raw {
        Some(raw) => serde_json::from_str(raw).map_err(|_| LoginTransactionError::InvalidState)?,
        None => DesiredState::default(),
    };
    value
        .validate()
        .map_err(|_| LoginTransactionError::InvalidState)?;
    Ok(value)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Publication {
    Pending,
    Desired,
    Consumed,
}
#[derive(Default)]
struct Publisher {
    #[cfg(test)]
    fault: Option<(Publication, bool)>,
    #[cfg(test)]
    tamper: Option<(Publication, PathBuf, Vec<u8>)>,
}
impl Publisher {
    fn write(&self, paths: &LoginPaths, path: &Path, raw: &[u8], stage: Publication) -> Result<()> {
        #[cfg(not(test))]
        let _ = stage;
        #[cfg(test)]
        if self.fault == Some((stage, false)) {
            return Err(LoginTransactionError::ManualRecoveryRequired);
        }
        atomic_replace_private(path, raw, paths.uid)
            .map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
        #[cfg(test)]
        if let Some((target_stage, target, bytes)) = &self.tamper
            && *target_stage == stage
        {
            atomic_replace_private(target, bytes, paths.uid)
                .map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
        }
        #[cfg(test)]
        if self.fault == Some((stage, true)) {
            return Err(LoginTransactionError::ManualRecoveryRequired);
        }
        Ok(())
    }
}

/// Offline transaction only. A daemon restart must bypass this API completely.
pub fn consume_login(
    paths: &LoginPaths,
    expected_generation: u64,
    epoch: &str,
    host: &mut impl LoginReadiness,
) -> Result<LoginTransactionOutcome> {
    consume(
        paths,
        expected_generation,
        epoch,
        host,
        &Publisher::default(),
    )
}

fn consume(
    paths: &LoginPaths,
    generation: u64,
    epoch: &str,
    host: &mut impl LoginReadiness,
    publisher: &Publisher,
) -> Result<LoginTransactionOutcome> {
    if epoch.is_empty()
        || epoch.len() > MAX_EPOCH_BYTES
        || !epoch.bytes().all(|b| (33..=126).contains(&b))
        || generation == 0
        || generation > MAX_GENERATION
    {
        return Err(LoginTransactionError::InvalidInput);
    }
    paths.validate()?;
    let _owner = OwnerLock::acquire(&paths.runtime.owner_lock, paths.uid).map_err(|error| {
        if error == RuntimeError::AlreadyRunning {
            LoginTransactionError::Busy
        } else {
            LoginTransactionError::InvalidState
        }
    })?;
    let _migration = MigrationLock::acquire(&paths.cutover, paths.uid).map_err(|error| {
        if error == CutoverError::Busy {
            LoginTransactionError::Busy
        } else {
            LoginTransactionError::InvalidState
        }
    })?;
    let existing = receipt(paths)?;
    if existing
        .as_ref()
        .is_some_and(|value| value.phase == Phase::Pending)
    {
        return Err(LoginTransactionError::ManualRecoveryRequired);
    }
    exact_owner(paths, generation)?;
    let epoch_hash = format!(
        "{:x}",
        Sha256::digest([b"omavless-login-epoch-v1\0".as_slice(), epoch.as_bytes()].concat())
    );
    if let Some(existing) = existing {
        if existing.epoch_hash != epoch_hash || existing.ownership_generation != generation {
            return Err(LoginTransactionError::EpochMismatch);
        }
        desired_from_snapshot(
            private_optional(&paths.desired.file, paths.uid, MAX_DESIRED_STATE_BYTES)?.as_deref(),
        )?;
        return Ok(LoginTransactionOutcome::AlreadyConsumed);
    }
    let snapshot = Snapshot::read(paths)?;
    let current = desired_from_snapshot(snapshot.desired.as_deref())?;
    let store =
        parse_private_store(&snapshot.store).map_err(|_| LoginTransactionError::InvalidState)?;
    let candidate = plan_login_intent(LoginTrigger::FirstLogin, &current, &store)
        .map_err(LoginTransactionError::Plan)?;
    host.verify_empty()
        .map_err(|_| LoginTransactionError::ValidationRejected)?;
    if candidate.as_ref().unwrap_or(&current).connected {
        host.validate_candidate(
            candidate.as_ref().unwrap_or(&current),
            &store,
            snapshot
                .template
                .as_deref()
                .ok_or(LoginTransactionError::ValidationRejected)?,
        )
        .map_err(|_| LoginTransactionError::ValidationRejected)?;
    }
    // Cooperating actors are locked; an unrelated core may still invalidate an
    // earlier observation. This second observation is not an atomic host proof.
    host.verify_empty()
        .map_err(|_| LoginTransactionError::ValidationRejected)?;
    exact_owner(paths, generation)?;
    if Snapshot::read(paths)? != snapshot || receipt(paths)?.is_some() {
        return Err(LoginTransactionError::SnapshotChanged);
    }
    let mut journal = Receipt {
        schema_version: 1,
        epoch_hash,
        ownership_generation: generation,
        phase: Phase::Pending,
    };
    let encode = |value: &Receipt| {
        serde_json::to_vec(value).map_err(|_| LoginTransactionError::ManualRecoveryRequired)
    };
    publisher.write(
        paths,
        &paths.receipt,
        &encode(&journal)?,
        Publication::Pending,
    )?;
    if receipt(paths)? != Some(journal.clone()) {
        return Err(LoginTransactionError::ManualRecoveryRequired);
    }
    exact_owner(paths, generation).map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
    if Snapshot::read(paths).ok().as_ref() != Some(&snapshot) {
        return Err(LoginTransactionError::ManualRecoveryRequired);
    }
    let mut expected_desired = snapshot.desired.clone();
    if let Some(candidate) = &candidate {
        let payload = serde_json::to_vec(candidate)
            .map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
        publisher.write(paths, &paths.desired.file, &payload, Publication::Desired)?;
        expected_desired = Some(
            String::from_utf8(payload)
                .map_err(|_| LoginTransactionError::ManualRecoveryRequired)?,
        );
    }
    exact_owner(paths, generation).map_err(|_| LoginTransactionError::ManualRecoveryRequired)?;
    if required(
        &paths.store,
        paths.uid,
        omavless_store::MAX_STORE_BYTES as u64,
    )
    .ok()
    .as_ref()
        != Some(&snapshot.store)
        || private_optional(&paths.template, paths.uid, MAX_TEMPLATE_BYTES as u64).ok()
            != Some(snapshot.template.clone())
        || private_optional(&paths.desired.file, paths.uid, MAX_DESIRED_STATE_BYTES).ok()
            != Some(expected_desired)
    {
        return Err(LoginTransactionError::ManualRecoveryRequired);
    }
    if receipt(paths)? != Some(journal.clone()) {
        return Err(LoginTransactionError::ManualRecoveryRequired);
    }
    journal.phase = Phase::Consumed;
    publisher.write(
        paths,
        &paths.receipt,
        &encode(&journal)?,
        Publication::Consumed,
    )?;
    if receipt(paths)? != Some(journal) {
        return Err(LoginTransactionError::ManualRecoveryRequired);
    }
    Ok(LoginTransactionOutcome::Consumed {
        desired_changed: candidate.is_some(),
    })
}

#[cfg(test)]
mod tests;
