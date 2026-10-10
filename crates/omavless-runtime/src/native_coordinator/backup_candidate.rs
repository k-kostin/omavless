// SPDX-License-Identifier: MIT

//! Inactive native-owner composition for a private encrypted backup. There is
//! no CLI, IPC, picker, button or installed caller. Destination selection and
//! passphrase UX are deliberately outside this experiment.

use super::*;
use crate::backup_destination_candidate::{PublishError, publish_new};
use crate::backup_source_candidate::{SnapshotError, seal_current_pair};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BackupCreateError {
    OwnershipUnavailable,
    Busy,
    RecoveryRequired,
    Source(SnapshotError),
    Publish(PublishError),
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Capture and seal one exact committed pair while the native owner holds
    /// its mutation lease, then exclusively publish only ciphertext. A caller
    /// must separately review destination authority and secret input before
    /// this can ever become a product method.
    #[allow(dead_code)]
    pub(crate) fn create_backup_candidate(
        &mut self,
        destination: &Path,
        passphrase: &[u8],
    ) -> Result<(), BackupCreateError> {
        if self.retained_restore_busy() {
            return Err(BackupCreateError::RecoveryRequired);
        }
        let fence = self
            .required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust)
            .ok_or(BackupCreateError::OwnershipUnavailable)?;
        let lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => BackupCreateError::Busy,
                _ => BackupCreateError::OwnershipUnavailable,
            })?;
        if !self
            .transaction
            .ownership_matches(fence.phase, fence.generation)
        {
            return Err(BackupCreateError::OwnershipUnavailable);
        }
        if self.transaction.blocked()
            || self.auxiliary_recovery_required
            || self.batch.as_ref().is_some_and(|batch| batch.stopped)
        {
            return Err(BackupCreateError::RecoveryRequired);
        }
        if self.coordinator.active()
            || self.coordinator.queued() != 0
            || self
                .batch
                .as_ref()
                .is_some_and(|batch| batch.active.is_some())
        {
            return Err(BackupCreateError::Busy);
        }
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
            .ok_or(BackupCreateError::Source(SnapshotError::UnsafeSource))?;
        let revision = self.coordinator.revision();
        let sealed = seal_current_pair(
            config,
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            fence.generation,
            &lock,
            passphrase,
        )
        .map_err(BackupCreateError::Source)?;
        if self.coordinator.revision() != revision
            || !self
                .transaction
                .ownership_matches(fence.phase, fence.generation)
            || self.transaction.blocked()
        {
            return Err(BackupCreateError::OwnershipUnavailable);
        }
        publish_new(destination, self.transaction.uid(), &sealed)
            .map_err(BackupCreateError::Publish)
    }
}
