// SPDX-License-Identifier: MIT

//! Inactive native-owner composition of terminal restore retirement. This
//! has no socket, command, startup or UI caller and never clears the receipt.

use super::*;
use crate::desired::read_desired_snapshot;
use crate::restore_cleanup_candidate::{
    CleanupError, CleanupResult, ClosurePublicationResult, FinalizeError, FinalizeResult,
    finalize_fenced_restore, publish_completion_record, retire_fixed_restore_artifacts,
};
use crate::restore_retirement_candidate::{RetirementError, durable_retirement_receipt};
use crate::restore_slot_retirement_candidate::{SlotError, retire_replacement_slots};
use crate::restore_staging_candidate::{read_staged_pair, same_member};
use restore_candidate::{RestoreAdmissionError, RestoreReadiness};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RestoreRetireError {
    Owner(RestoreAdmissionError),
    Receipt(RetirementError),
    Slots(SlotError),
    Artifacts(CleanupError),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RestoreFinalizeError {
    Owner(RestoreAdmissionError),
    Receipt(RetirementError),
    Closure(FinalizeError),
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Publish the completion proof while the terminal receipt still fences
    /// startup. No product caller can reach this inactive candidate.
    #[allow(dead_code)]
    pub(crate) fn publish_restore_completion_candidate(
        &mut self,
    ) -> Result<ClosurePublicationResult, RestoreFinalizeError> {
        if self.retained_restore_busy() {
            return Err(RestoreFinalizeError::Owner(
                RestoreAdmissionError::RecoveryRequired,
            ));
        }
        let lock = self.transaction.acquire_lock().map_err(|error| {
            RestoreFinalizeError::Owner(match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            })
        })?;
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
            .ok_or(RestoreFinalizeError::Owner(
                RestoreAdmissionError::OwnershipUnavailable,
            ))?
            .to_path_buf();
        let paths = self.transaction.cutover_paths().clone();
        let uid = self.transaction.uid();
        let readiness = self
            .retirement_readiness_locked(&lock)
            .map_err(RestoreFinalizeError::Owner)?;
        let generation = readiness.owner_generation;
        let (receipt, receipt_identity) =
            durable_retirement_receipt(&config, &paths, uid, generation, &lock)
                .map_err(RestoreFinalizeError::Receipt)?;
        let terminal = receipt.terminal().encode();
        let gate = || {
            self.retirement_readiness_locked(&lock)
                .is_ok_and(|current| current == readiness)
                && durable_retirement_receipt(&config, &paths, uid, generation, &lock).is_ok_and(
                    |(current, identity)| {
                        current.terminal().encode() == terminal
                            && same_member(&receipt_identity, &identity)
                    },
                )
        };
        publish_completion_record(&config, &paths, uid, generation, &lock, gate)
            .map_err(RestoreFinalizeError::Closure)
    }

    /// Retire only the older pending receipt after the separate retirement
    /// and completion-publication steps. The completion fence remains. This
    /// inactive path has no product startup, IPC, UI or CLI caller.
    #[allow(dead_code)]
    pub(crate) fn finalize_terminal_restore_candidate(
        &mut self,
    ) -> Result<FinalizeResult, RestoreFinalizeError> {
        if self.retained_restore_busy() {
            return Err(RestoreFinalizeError::Owner(
                RestoreAdmissionError::RecoveryRequired,
            ));
        }
        let lock = self.transaction.acquire_lock().map_err(|error| {
            RestoreFinalizeError::Owner(match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            })
        })?;
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
            .ok_or(RestoreFinalizeError::Owner(
                RestoreAdmissionError::OwnershipUnavailable,
            ))?
            .to_path_buf();
        let paths = self.transaction.cutover_paths().clone();
        let uid = self.transaction.uid();
        let readiness = self
            .retirement_readiness_locked(&lock)
            .map_err(RestoreFinalizeError::Owner)?;
        let generation = readiness.owner_generation;
        let (receipt, receipt_identity) =
            durable_retirement_receipt(&config, &paths, uid, generation, &lock)
                .map_err(RestoreFinalizeError::Receipt)?;
        let terminal = receipt.terminal().encode();
        let gate = || {
            self.retirement_readiness_locked(&lock)
                .is_ok_and(|current| current == readiness)
                && durable_retirement_receipt(&config, &paths, uid, generation, &lock).is_ok_and(
                    |(current, identity)| {
                        current.terminal().encode() == terminal
                            && same_member(&receipt_identity, &identity)
                    },
                )
        };
        let result = finalize_fenced_restore(&config, &paths, uid, generation, &lock, gate)
            .map_err(RestoreFinalizeError::Closure);
        // The pending unlink can succeed before a later fsync/readback fails.
        // The completion fence remains, and this owner also latches manual
        // recovery rather than reporting ordinary mutation availability.
        if matches!(
            result,
            Err(RestoreFinalizeError::Closure(FinalizeError::Ambiguous))
        ) {
            self.transaction.block();
        }
        result
    }

    /// Only a terminal transaction with a durable receipt can enter this
    /// internal path. Complete staging permits slot retirement first; an
    /// already-started cleanup prefix continues directly. Both children
    /// independently reprove their exact filesystem bindings at each effect.
    #[allow(dead_code)]
    pub(crate) fn retire_terminal_restore_candidate(
        &mut self,
    ) -> Result<CleanupResult, RestoreRetireError> {
        if self.retained_restore_busy() {
            return Err(RestoreRetireError::Owner(
                RestoreAdmissionError::RecoveryRequired,
            ));
        }
        let lock = self.transaction.acquire_lock().map_err(|error| {
            RestoreRetireError::Owner(match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            })
        })?;
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
            .ok_or(RestoreRetireError::Owner(
                RestoreAdmissionError::OwnershipUnavailable,
            ))?
            .to_path_buf();
        let paths = self.transaction.cutover_paths().clone();
        let uid = self.transaction.uid();
        let readiness = self
            .retirement_readiness_locked(&lock)
            .map_err(RestoreRetireError::Owner)?;
        let generation = readiness.owner_generation;
        let (receipt, receipt_identity) =
            durable_retirement_receipt(&config, &paths, uid, generation, &lock)
                .map_err(RestoreRetireError::Receipt)?;
        let terminal = receipt.terminal().encode();
        let mut gate = || {
            self.retirement_readiness_locked(&lock)
                .is_ok_and(|current| current == readiness)
                && durable_retirement_receipt(&config, &paths, uid, generation, &lock).is_ok_and(
                    |(current, identity)| {
                        current.terminal().encode() == terminal
                            && same_member(&receipt_identity, &identity)
                    },
                )
        };
        if !gate() {
            return Err(RestoreRetireError::Owner(
                RestoreAdmissionError::ObservationUnavailable,
            ));
        }
        if read_staged_pair(&paths.state_directory, uid).is_ok() {
            retire_replacement_slots(&config, &paths, uid, generation, &lock, &mut gate)
                .map_err(RestoreRetireError::Slots)?;
        }
        if !gate() {
            return Err(RestoreRetireError::Owner(
                RestoreAdmissionError::ObservationUnavailable,
            ));
        }
        retire_fixed_restore_artifacts(&config, &paths, uid, generation, &lock, gate)
            .map_err(RestoreRetireError::Artifacts)
    }

    /// Unlike initial restore admission, this exact terminal-receipt path
    /// expects the restore existence fence. It never bypasses an independent
    /// owner failure, pending routing preset, active work or owned host state.
    fn retirement_readiness_locked(
        &mut self,
        lock: &MigrationLock,
    ) -> Result<RestoreReadiness, RestoreAdmissionError> {
        let fence = self
            .required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust)
            .ok_or(RestoreAdmissionError::OwnershipUnavailable)?;
        if !lock.authorizes(self.transaction.cutover_paths(), self.transaction.uid())
            || !self
                .transaction
                .ownership_matches(fence.phase, fence.generation)
        {
            return Err(RestoreAdmissionError::OwnershipUnavailable);
        }
        if self.transaction.independently_blocked()
            || self.auxiliary_recovery_required
            || self.batch.as_ref().is_some_and(|batch| batch.stopped)
            || crate::routing_preset::pending(self.transaction.desired_paths())
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
        if crate::routing_preset::pending(self.transaction.desired_paths()) {
            return Err(RestoreAdmissionError::RecoveryRequired);
        }
        Ok(RestoreReadiness {
            revision: self.coordinator.revision(),
            desired_generation: desired.generation,
            owner_generation: fence.generation,
        })
    }
}
