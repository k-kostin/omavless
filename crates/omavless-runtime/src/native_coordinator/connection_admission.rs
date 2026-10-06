// SPDX-License-Identifier: MIT
//! The ordinary connection caller with a private test-only historical variant.
use super::*;
use crate::private_store_transaction::{PreparedPointerMutation, PreparedWrite};
use std::marker::PhantomData;
#[cfg(test)]
use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::{HistoricalConnection, ConnectionCheckpoint};

pub(crate) enum ConnectionAdmission<'a, 'b> {
    Ordinary(PhantomData<&'a &'b ()>),
    #[cfg(test)]
    Historical(&'a mut HistoricalConnection<'b>),
}
pub(super) struct ConnectionLease<'a> {
    owned: Option<crate::connection_transaction::MigrationLease>,
    retained: Option<&'a MigrationLock>,
}
impl std::ops::Deref for ConnectionLease<'_> {
    type Target = MigrationLock;
    fn deref(&self) -> &Self::Target {
        self.owned
            .as_deref()
            .or(self.retained)
            .expect("connection preflight retains a lease")
    }
}
impl<'a, 'b> ConnectionAdmission<'a, 'b> {
    pub(crate) fn ordinary() -> Self {
        Self::Ordinary(PhantomData)
    }
    pub(super) fn kind(&self, _action: &OwnerAction) -> Result<(), NativeOwnerError> {
        match self {
            #[cfg(test)]
            Self::Historical(_) if matches!(_action, OwnerAction::SetMode { .. }) => {
                Err(NativeOwnerError::ManualRecoveryRequired)
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn recheck(&mut self) -> Result<(), LifecycleError> {
        match self {
            Self::Ordinary(_) => Ok(()),
            #[cfg(test)]
            Self::Historical(context) => context
                .recheck()
                .map_err(|_| LifecycleError::ManualRecoveryRequired),
        }
    }
    pub(super) fn latch<H: LifecycleHost>(&self, owner: &mut OfflineNativeCoordinator<H>) {
        #[cfg(test)]
        if matches!(self,Self::Historical(context) if context.poisoned()) {
            owner.transaction.block();
        }
        #[cfg(not(test))]
        let _ = owner;
    }

    pub(super) fn completion(&mut self, completion: &Completion) {
        #[cfg(test)]
        if matches!(
            completion,
            Completion::Ordinary(Err(ConnectionTransactionError::ManualRecoveryRequired))
                | Completion::CommittedFailure(ConnectionTransactionError::ManualRecoveryRequired)
        ) && let Self::Historical(context) = self
        {
            context.poison();
        }
        #[cfg(not(test))]
        let _ = completion;
    }
    pub(super) fn admit<H: LifecycleHost>(
        &mut self,
        owner: &mut OfflineNativeCoordinator<H>,
        kind: MutationKind,
        operation_id: Option<&str>,
        revision: Option<u64>,
        digest: crate::mutation::MutationDigest,
    ) -> Result<Admission, NativeOwnerError> {
        match self {
            Self::Ordinary(_) => owner.admit(kind, operation_id, revision, digest),
            #[cfg(test)]
            Self::Historical(context) => {
                // Refuse foreign-bound evidence without poisoning its owner
                // or latching this receiver. New-intent close revocation ran
                // before typed admission and is deliberately preserved.
                if context.owner_identity().is_some_and(|original| {
                    !std::sync::Arc::ptr_eq(original, &owner.research_identity)
                }) {
                    return Err(NativeOwnerError::OwnershipUnavailable);
                }
                let checked = context.bind_owner(&owner.research_identity).and_then(|()| {
                    context.check(
                        owner.transaction.cutover_paths(),
                        owner.transaction.desired_paths(),
                        owner.transaction.store_path(),
                        owner.transaction.uid(),
                    )
                });
                if checked.is_err() || owner.transaction.independently_blocked() {
                    context.poison();
                    owner.transaction.block();
                    return Err(NativeOwnerError::ManualRecoveryRequired);
                }
                if owner.required_ownership.is_none_or(|f| {
                    f.phase != OwnershipPhase::Rust
                        || !owner.transaction.ownership_matches(f.phase, f.generation)
                }) {
                    context.poison();
                    owner.transaction.block();
                    return Err(NativeOwnerError::OwnershipUnavailable);
                }
                if context.recheck().is_err() {
                    owner.transaction.block();
                    return Err(NativeOwnerError::ManualRecoveryRequired);
                }
                owner.schedule(kind, operation_id, revision, digest)
            }
        }
    }
    pub(super) fn blocked<H: LifecycleHost>(
        &self,
        owner: &OfflineNativeCoordinator<H>,
        disconnect: bool,
    ) -> bool {
        match self {
            Self::Ordinary(_) => {
                if disconnect {
                    owner.transaction.stop_blocked()
                } else {
                    owner.transaction.blocked()
                }
            }
            #[cfg(test)]
            Self::Historical(_) => owner.transaction.independently_blocked(),
        }
    }
    pub(super) fn preflight<H: LifecycleHost>(
        &mut self,
        owner: &mut OfflineNativeCoordinator<H>,
        token: MutationToken,
    ) -> Result<Result<ConnectionLease<'b>, NativeOwnerExecution>, NativeOwnerError> {
        match self {
            Self::Ordinary(_) => Ok(
                match owner.preflight_lock(token, NativeTransactionError::Connection)? {
                    LockAdmission::Locked(lock) => Ok(ConnectionLease {
                        owned: Some(lock),
                        retained: None,
                    }),
                    LockAdmission::Uncached(outcome) => Err(outcome),
                },
            ),
            #[cfg(test)]
            Self::Historical(context) => {
                if context.recheck().is_err() {
                    owner.transaction.block();
                    owner.coordinator.abort_active_uncached(token)?;
                    return Err(NativeOwnerError::ManualRecoveryRequired);
                }
                Ok(Ok(ConnectionLease {
                    owned: None,
                    retained: Some(context.lock()),
                }))
            }
        }
    }
    pub(crate) fn write_desired(
        &mut self,
        paths: &crate::desired::DesiredPaths,
        uid: u32,
        state: &crate::desired::DesiredState,
    ) -> Result<(), LifecycleError> {
        self.recheck()?;
        #[cfg(test)]
        if let Self::Historical(context) = self {
            context
                .checkpoint(ConnectionCheckpoint::BeforeDesired)
                .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
        }
        let result = crate::desired::write_desired(paths, uid, state).map_err(LifecycleError::from);
        #[cfg(test)]
        if let Self::Historical(context) = self {
            if result.is_ok() {
                context
                    .desired_written(state)
                    .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
            } else {
                context
                    .recheck()
                    .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
            }
            context
                .checkpoint(ConnectionCheckpoint::AfterDesired)
                .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
        }
        result
    }
    pub(crate) fn pointer(
        &mut self,
        plan: &PreparedPointerMutation,
        lock: &MigrationLock,
        paths: &crate::cutover::CutoverPaths,
        restored: bool,
    ) -> Result<
        Result<PreparedWrite, crate::private_store_transaction::PrivateStoreWriteError>,
        LifecycleError,
    > {
        self.recheck()?;
        #[cfg(test)]
        if let Self::Historical(context) = self {
            if !std::ptr::eq(lock, context.lock()) {
                context.poison();
                return Err(LifecycleError::ManualRecoveryRequired);
            }
            context
                .checkpoint(if restored {
                    ConnectionCheckpoint::BeforeRestore
                } else {
                    ConnectionCheckpoint::BeforePointer
                })
                .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
        }
        let result = if restored {
            plan.restore_locked(lock, paths)
        } else {
            plan.commit_locked(lock, paths)
        };
        #[cfg(test)]
        if let Self::Historical(context) = self {
            match result {
                Ok(write) => context.pointer_written(plan, write, restored),
                Err(_) => context.recheck(),
            }
            .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
            context
                .checkpoint(if restored {
                    ConnectionCheckpoint::AfterRestore
                } else {
                    ConnectionCheckpoint::AfterPointer
                })
                .map_err(|_| LifecycleError::ManualRecoveryRequired)?;
        }
        Ok(result)
    }
}
