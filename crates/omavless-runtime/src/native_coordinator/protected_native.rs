//! One private consuming owner experiment; not a normal mutation registration.
use super::*;
use crate::lifecycle::{LifecycleError, LifecycleOutcome};

impl OfflineNativeCoordinator<crate::native_host::NativeLifecycleHost> {
    pub(crate) fn protected_native_lock(&self) -> Result<MigrationLock, LifecycleError> {
        self.transaction
            .acquire_lock()
            .map_err(|_| LifecycleError::ManualRecoveryRequired)
    }

    pub(crate) fn protected_native_paths(&self) -> &crate::cutover::CutoverPaths {
        self.transaction.cutover_paths()
    }

    pub(crate) fn protected_native_roundtrip(
        &mut self,
        lock: &MigrationLock,
        profile: &str,
        singleton: &mut dyn FnMut() -> Result<(), LifecycleError>,
    ) -> Result<LifecycleOutcome, LifecycleError> {
        let refuse = LifecycleError::ManualRecoveryRequired;
        let fence = self
            .required_ownership
            .filter(|f| f.phase == OwnershipPhase::Rust)
            .ok_or(refuse)?;
        if self.transaction.blocked()
            || self.auxiliary_recovery_required
            || self.coordinator.active()
            || self.coordinator.queued() != 0
            || self.batch.is_some()
            || self.actual() != ActualState::Disconnected
            || self
                .transaction
                .host()
                .auxiliary_slot()
                .is_none_or(|slot| !slot.mutation_safe())
        {
            return Err(refuse);
        }
        let paths = self.transaction.cutover_paths().clone();
        let desired = self.transaction.desired_paths().clone();
        let uid = self.uid();
        let mut origin = || {
            singleton()?;
            if !lock.authorizes(&paths, uid)
                || !read_marker(&paths, uid)
                    .is_ok_and(|m| m.phase() == fence.phase && m.generation() == fence.generation)
                || crate::pending_private_transaction::pending(&desired)
                || crate::login_transaction::check_startup_receipt(
                    &paths,
                    uid,
                    lock,
                    Some(fence.generation),
                )
                .is_err()
            {
                return Err(LifecycleError::ManualRecoveryRequired);
            }
            Ok(())
        };
        origin()?;
        // Consumed coordinator cannot admit a parallel ordinary mutation. The
        // existing executor stays in its original transaction, borrowed only.
        self.transaction.block();
        crate::lifecycle::protected_candidate::borrowed_native_roundtrip(
            self.transaction.lifecycle_mut(),
            profile,
            &mut origin,
        )
    }
}
