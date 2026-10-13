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

impl BackupCreateError {
    /// Fixed diagnostic only: never changes the operation's outcome or permits retry.
    #[cfg(feature = "t4-manager-actor-service")]
    pub(crate) fn diagnostic(&self) -> &'static [u8] {
        match self {
            Self::OwnershipUnavailable => b"T4_BACKUP_OWNERSHIP\n",
            Self::Busy => b"T4_BACKUP_BUSY\n",
            Self::RecoveryRequired => b"T4_BACKUP_RECOVERY\n",
            Self::Source(SnapshotError::Admission) => b"T4_BACKUP_SOURCE_ADMISSION\n",
            Self::Source(SnapshotError::UnsafeSource) => b"T4_BACKUP_SOURCE_UNSAFE\n",
            Self::Source(SnapshotError::SourceChanged) => b"T4_BACKUP_SOURCE_CHANGED\n",
            Self::Source(SnapshotError::InvalidBackupInput) => b"T4_BACKUP_PAYLOAD\n",
            Self::Source(SnapshotError::SealingUnavailable) => b"T4_BACKUP_SEALING\n",
            Self::Publish(PublishError::InvalidDestination) => b"T4_BACKUP_DESTINATION\n",
            Self::Publish(PublishError::Exists) => b"T4_BACKUP_EXISTS\n",
            Self::Publish(PublishError::Unavailable) => b"T4_BACKUP_PUBLICATION_UNAVAILABLE\n",
            Self::Publish(PublishError::Ambiguous) => b"T4_BACKUP_PUBLICATION_AMBIGUOUS\n",
        }
    }
}

#[cfg(all(test, feature = "t4-manager-actor-service"))]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn categories_are_fixed_bounded_ascii_and_distinguish_publication_ambiguity() {
        let errors = [
            BackupCreateError::OwnershipUnavailable,
            BackupCreateError::Busy,
            BackupCreateError::RecoveryRequired,
            BackupCreateError::Source(SnapshotError::Admission),
            BackupCreateError::Source(SnapshotError::UnsafeSource),
            BackupCreateError::Source(SnapshotError::SourceChanged),
            BackupCreateError::Source(SnapshotError::InvalidBackupInput),
            BackupCreateError::Source(SnapshotError::SealingUnavailable),
            BackupCreateError::Publish(PublishError::InvalidDestination),
            BackupCreateError::Publish(PublishError::Exists),
            BackupCreateError::Publish(PublishError::Unavailable),
            BackupCreateError::Publish(PublishError::Ambiguous),
        ];
        let mut seen = std::collections::HashSet::new();
        for error in errors {
            let code = error.diagnostic();
            assert!(code.len() <= 64 && code.starts_with(b"T4_BACKUP_") && code.ends_with(b"\n"));
            assert!(
                code[..code.len() - 1]
                    .iter()
                    .all(|b| b.is_ascii_uppercase() || *b == b'_' || *b == b'4')
            );
            assert!(seen.insert(code));
        }
    }
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
