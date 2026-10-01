// SPDX-License-Identifier: MIT

//! Inactive, read-only restore preflight. This is not a reservation or restore
//! capability: a future commit must repeat every check while holding the owner
//! and migration leases, then separately prove durable multi-file recovery.

use super::*;
use crate::backup_destination_candidate::{
    BackupPreview, ReadError, open_existing, preview_existing,
};
use crate::backup_source_candidate::{PrivateSourcePair, capture_current_pair};
use crate::desired::read_desired_snapshot;
use crate::restore_staging_candidate::{StageError, stage_private_pair};
use omavless_domain::private_backup::OpenedBackup;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RestoreAdmissionError {
    OwnershipUnavailable,
    Busy,
    RecoveryRequired,
    NotDisconnected,
    ObservationUnavailable,
}

/// Informational snapshot only. It cannot authorize a later mutation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RestoreReadiness {
    pub(crate) revision: u64,
    pub(crate) desired_generation: u64,
    pub(crate) owner_generation: u64,
}

/// The authenticated file facts and current owner facts are deliberately
/// separate. A readable backup can coexist with a connected or recovering
/// owner. Neither part reserves the file, revision, or future host state.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RestorePreview {
    pub(crate) backup: BackupPreview,
    pub(crate) readiness: Result<RestoreReadiness, RestoreAdmissionError>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RestorePrepareError {
    Backup(ReadError),
    Owner(RestoreAdmissionError),
    CurrentPairUnavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RestoreStageError {
    Prepare(RestorePrepareError),
    Staging(StageError),
}

/// Sensitive, in-memory preparation only. The old bytes and authenticated
/// replacement are never logged, persisted, formatted or copied by this type.
/// A future commit may not use this snapshot as a reservation: it must repeat
/// authentication/admission and implement durable whole-pair recovery.
#[allow(dead_code)]
pub(crate) struct PreparedRestorePair {
    original: PrivateSourcePair,
    incoming: OpenedBackup,
    readiness: RestoreReadiness,
}

#[allow(dead_code)]
impl PreparedRestorePair {
    pub(crate) fn original_store(&self) -> &[u8] {
        self.original.store()
    }

    pub(crate) fn original_template(&self) -> &[u8] {
        self.original.template()
    }

    pub(crate) fn incoming_store(&self) -> &[u8] {
        self.incoming.store()
    }

    pub(crate) fn incoming_template(&self) -> &[u8] {
        self.incoming.template()
    }

    pub(crate) fn readiness(&self) -> &RestoreReadiness {
        &self.readiness
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Inactive, read-only composition for an eventual replacement preview.
    /// A future restore must reopen and authenticate the file, then repeat
    /// disconnected-owner admission under its exclusive commit lease.
    #[allow(dead_code)]
    pub(crate) fn restore_preview_candidate(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<RestorePreview, ReadError> {
        let backup = preview_existing(source, self.transaction.uid(), passphrase)?;
        let readiness = self.restore_readiness_candidate();
        Ok(RestorePreview { backup, readiness })
    }

    /// Authentication is deliberately outside the owner lock. Afterwards one
    /// lease covers both old-file reads and two disconnected observations.
    /// This is a preparation experiment only, with no restore side effect.
    #[allow(dead_code)]
    pub(crate) fn prepare_restore_candidate(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<PreparedRestorePair, RestorePrepareError> {
        let incoming = open_existing(source, self.transaction.uid(), passphrase)
            .map_err(RestorePrepareError::Backup)?;
        let lock = self.transaction.acquire_lock().map_err(|error| {
            RestorePrepareError::Owner(match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            })
        })?;
        self.prepare_restore_locked(incoming, &lock)
    }

    /// Stage only the four fixed private members under the same lease that
    /// captured the old pair. The durable pending directory blocks another
    /// staging attempt; no live file is replaced. No product caller exists.
    #[allow(dead_code)]
    pub(crate) fn stage_restore_candidate(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<(), RestoreStageError> {
        let incoming = open_existing(source, self.transaction.uid(), passphrase)
            .map_err(|error| RestoreStageError::Prepare(RestorePrepareError::Backup(error)))?;
        let lock = self.transaction.acquire_lock().map_err(|error| {
            RestoreStageError::Prepare(RestorePrepareError::Owner(match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            }))
        })?;
        let prepared = self
            .prepare_restore_locked(incoming, &lock)
            .map_err(RestoreStageError::Prepare)?;
        stage_private_pair(
            &self.transaction.desired_paths().directory,
            self.transaction.uid(),
            prepared.original_store(),
            prepared.original_template(),
            prepared.incoming_store(),
            prepared.incoming_template(),
        )
        .map_err(RestoreStageError::Staging)
    }

    fn prepare_restore_locked(
        &mut self,
        incoming: OpenedBackup,
        lock: &MigrationLock,
    ) -> Result<PreparedRestorePair, RestorePrepareError> {
        let readiness = self
            .restore_readiness_locked(lock)
            .map_err(RestorePrepareError::Owner)?;
        let config = self
            .transaction
            .store_path()
            .parent()
            .filter(|_| {
                self.transaction
                    .store_path()
                    .file_name()
                    .is_some_and(|name| name == "profiles.json")
            })
            .ok_or(RestorePrepareError::CurrentPairUnavailable)?;
        let original = capture_current_pair(
            config,
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            readiness.owner_generation,
            lock,
        )
        .map_err(|_| RestorePrepareError::CurrentPairUnavailable)?;
        let after = self
            .restore_readiness_locked(lock)
            .map_err(RestorePrepareError::Owner)?;
        if after != readiness {
            return Err(RestorePrepareError::Owner(
                RestoreAdmissionError::ObservationUnavailable,
            ));
        }
        Ok(PreparedRestorePair {
            original,
            incoming,
            readiness,
        })
    }

    /// No IPC, UI, CLI, file mutation or VPN effect. A connected or uncertain
    /// owner cannot preview itself as restore-ready. Foreign cores/TUNs are
    /// never stopped or adopted; only our managed device must be absent.
    #[allow(dead_code)]
    pub(crate) fn restore_readiness_candidate(
        &mut self,
    ) -> Result<RestoreReadiness, RestoreAdmissionError> {
        let lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            })?;
        self.restore_readiness_locked(&lock)
    }

    fn restore_readiness_locked(
        &mut self,
        lock: &MigrationLock,
    ) -> Result<RestoreReadiness, RestoreAdmissionError> {
        let fence = self
            .required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust)
            .ok_or(RestoreAdmissionError::OwnershipUnavailable)?;
        if !lock.authorizes(self.transaction.cutover_paths(), self.transaction.uid()) {
            return Err(RestoreAdmissionError::OwnershipUnavailable);
        }
        if !self
            .transaction
            .ownership_matches(fence.phase, fence.generation)
        {
            return Err(RestoreAdmissionError::OwnershipUnavailable);
        }
        if self.transaction.blocked()
            || self.auxiliary_recovery_required
            || self.batch.as_ref().is_some_and(|batch| batch.stopped)
        {
            return Err(RestoreAdmissionError::RecoveryRequired);
        }
        if self.coordinator.active()
            || self.coordinator.queued() != 0
            || self
                .batch
                .as_ref()
                .is_some_and(|batch| batch.active.is_some())
            || self
                .transaction
                .host()
                .auxiliary_slot()
                .is_none_or(|slot| !slot.mutation_safe())
        {
            return Err(RestoreAdmissionError::Busy);
        }
        if crate::pending_private_transaction::pending(self.transaction.desired_paths()) {
            return Err(RestoreAdmissionError::RecoveryRequired);
        }
        let desired =
            read_desired_snapshot(self.transaction.desired_paths(), self.transaction.uid())
                .map_err(|_| RestoreAdmissionError::ObservationUnavailable)?;
        if desired.connected || self.actual() != ActualState::Disconnected {
            return Err(RestoreAdmissionError::NotDisconnected);
        }
        let observed = self
            .transaction
            .host_mut()
            .fresh_observation(&desired)
            .map_err(|_| RestoreAdmissionError::ObservationUnavailable)?;
        if observed.owned_core_running
            || observed.owned_auxiliary_mihomo_count != 0
            || observed.managed_tun_count != 0
            || observed.owned_controller_config_verified
            || observed.desired_profile_matches_owned
        {
            return Err(RestoreAdmissionError::NotDisconnected);
        }
        let after = read_desired_snapshot(self.transaction.desired_paths(), self.transaction.uid())
            .map_err(|_| RestoreAdmissionError::ObservationUnavailable)?;
        if after != desired
            || !self
                .transaction
                .ownership_matches(fence.phase, fence.generation)
        {
            return Err(RestoreAdmissionError::OwnershipUnavailable);
        }
        if crate::pending_private_transaction::pending(self.transaction.desired_paths()) {
            return Err(RestoreAdmissionError::RecoveryRequired);
        }
        Ok(RestoreReadiness {
            revision: self.coordinator.revision(),
            desired_generation: desired.generation,
            owner_generation: fence.generation,
        })
    }
}
