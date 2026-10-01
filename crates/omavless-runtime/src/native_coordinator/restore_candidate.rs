// SPDX-License-Identifier: MIT

//! Inactive, read-only restore preflight. This is not a reservation or restore
//! capability: a future commit must repeat every check while holding the owner
//! and migration leases, then separately prove durable multi-file recovery.

use super::*;
use crate::desired::read_desired_snapshot;

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

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// No IPC, UI, CLI, file mutation or VPN effect. A connected or uncertain
    /// owner cannot preview itself as restore-ready. Foreign cores/TUNs are
    /// never stopped or adopted; only our managed device must be absent.
    #[allow(dead_code)]
    pub(crate) fn restore_readiness_candidate(
        &mut self,
    ) -> Result<RestoreReadiness, RestoreAdmissionError> {
        let fence = self
            .required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust)
            .ok_or(RestoreAdmissionError::OwnershipUnavailable)?;
        let _lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => RestoreAdmissionError::Busy,
                _ => RestoreAdmissionError::OwnershipUnavailable,
            })?;
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
        if crate::routing_preset::pending(self.transaction.desired_paths()) {
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
