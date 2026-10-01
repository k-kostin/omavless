// SPDX-License-Identifier: MIT

//! Inactive native-owner composition of terminal restore retirement. This
//! has no socket, command, startup or UI caller and never clears the receipt.

use super::*;
use crate::desired::read_desired_snapshot;
use crate::restore_cleanup_candidate::{
    CleanupError, CleanupResult, retire_fixed_restore_artifacts,
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

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Only a terminal transaction with a durable receipt can enter this
    /// internal path. Complete staging permits slot retirement first; an
    /// already-started cleanup prefix continues directly. Both children
    /// independently reprove their exact filesystem bindings at each effect.
    #[allow(dead_code)]
    pub(crate) fn retire_terminal_restore_candidate(
        &mut self,
    ) -> Result<CleanupResult, RestoreRetireError> {
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
