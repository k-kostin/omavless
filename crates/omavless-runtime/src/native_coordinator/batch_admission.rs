// SPDX-License-Identifier: MIT
//! Only Ordinary exists in production. Historical evidence holds no lease
//! during worker steps and cannot rebase onto a fresh source snapshot.
use super::*;
use super::batch::{BatchOwnerState, NativeSubscriptionBatch};
#[cfg(test)]
use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::{DetachedHistoricalBatch, BatchWriteCheckpoint};

pub(super) enum BatchAdmission<'a> {
    Ordinary(std::marker::PhantomData<&'a ()>),
    #[cfg(test)]
    Historical(&'a mut DetachedHistoricalBatch),
}
impl BatchAdmission<'_> {
    #[cfg(test)]
    pub(super) fn research_identity(&self) -> Option<std::sync::Arc<()>> {
        match self {
            Self::Ordinary(_) => None,
            Self::Historical(context) => Some(context.owner_identity()),
        }
    }
    pub(super) fn ordinary() -> Self {
        Self::Ordinary(std::marker::PhantomData)
    }
    #[cfg(test)]
    fn check<H: LifecycleHost>(
        context: &mut DetachedHistoricalBatch,
        owner: &mut OfflineNativeCoordinator<H>,
        lock: &MigrationLock,
    ) -> Result<(), NativeOwnerError> {
        if owner.transaction.independently_blocked()
            || owner.required_ownership.is_none_or(|f| {
                f.phase != OwnershipPhase::Rust
                    || !owner.transaction.ownership_matches(f.phase, f.generation)
            })
            || owner.actual() != ActualState::Disconnected
        {
            context.poison();
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        context
            .check(
                &owner.research_identity,
                owner.transaction.cutover_paths(),
                owner.transaction.desired_paths(),
                owner.transaction.store_path(),
                owner.transaction.uid(),
                lock,
            )
            .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
        let desired = owner.transaction.desired().map_err(|_| {
            context.poison();
            NativeOwnerError::ManualRecoveryRequired
        })?;
        if desired.connected {
            context.poison();
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        let observation = owner.host_mut().observe(&desired).map_err(|_| {
            context.poison();
            NativeOwnerError::ManualRecoveryRequired
        })?;
        if observation.service_active
            || observation.controller_ready
            || observation.core_count != 0
            || observation.tun_count != 0
        {
            context.poison();
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        // Bracket external host observation with the same original evidence.
        context
            .check(
                &owner.research_identity,
                owner.transaction.cutover_paths(),
                owner.transaction.desired_paths(),
                owner.transaction.store_path(),
                owner.transaction.uid(),
                lock,
            )
            .map_err(|_| NativeOwnerError::ManualRecoveryRequired)
    }
    pub(super) fn lock<H: LifecycleHost>(
        &mut self,
        owner: &mut OfflineNativeCoordinator<H>,
    ) -> Result<MigrationLock, NativeOwnerError> {
        match self {
            Self::Ordinary(_) => owner.batch_lock(),
            #[cfg(test)]
            Self::Historical(context) => {
                let lock = owner
                    .transaction
                    .acquire_lock()
                    .map_err(|error| match error {
                        ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                        _ => NativeOwnerError::OwnershipUnavailable,
                    })?;
                Self::check(context, owner, &lock)?;
                Ok(lock)
            }
        }
    }
    pub(super) fn started(
        &mut self,
        _instance: &str,
        _token: crate::long_operation::LongOperationToken,
        _revision: u64,
    ) -> Result<(), NativeOwnerError> {
        match self {
            Self::Ordinary(_) => Ok(()),
            #[cfg(test)]
            Self::Historical(context) => context
                .bind_job(_instance, _token, _revision)
                .map_err(|_| NativeOwnerError::ManualRecoveryRequired),
        }
    }
    pub(super) fn matches(
        &mut self,
        _state: &BatchOwnerState,
        _job: &NativeSubscriptionBatch,
    ) -> Result<(), NativeOwnerError> {
        match self {
            Self::Ordinary(_) => Ok(()),
            #[cfg(test)]
            Self::Historical(context)
                if !std::sync::Arc::ptr_eq(
                    &context.owner_identity(),
                    &_state.research_identity,
                ) =>
            {
                // Refuse an unrelated genuine capability without poisoning it
                // or terminalizing this owner's still-authentic operation.
                Err(NativeOwnerError::OwnershipUnavailable)
            }
            #[cfg(test)]
            Self::Historical(context)
                if context.matches_job(&_state.instance, _job.token, _job.base_revision) =>
            {
                Ok(())
            }
            #[cfg(test)]
            Self::Historical(context) => {
                context.poison();
                Err(NativeOwnerError::ManualRecoveryRequired)
            }
        }
    }
    pub(super) fn finished(
        &mut self,
        _instance: &str,
        _token: crate::long_operation::LongOperationToken,
        _revision: u64,
    ) {
        #[cfg(test)]
        if let Self::Historical(context) = self {
            context.finish_job(_instance, _token, _revision);
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn commit<H, N, F>(
        &mut self,
        owner: &mut OfflineNativeCoordinator<H>,
        _lock: &MigrationLock,
        snapshot: omavless_domain::private_store::SubscriptionRefreshBatchSnapshot,
        updates: Vec<omavless_domain::private_store::SubscriptionRefreshBatchEntries>,
        now: N,
        commit: F,
    ) -> Result<SubscriptionRefreshCommit, NativeOwnerError>
    where
        H: LifecycleHost,
        N: FnOnce() -> u64,
        F: FnOnce(
            &Path,
            u32,
            omavless_domain::private_store::SubscriptionRefreshBatchSnapshot,
            Vec<omavless_domain::private_store::SubscriptionRefreshBatchEntries>,
            u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>,
    {
        match self {
            Self::Ordinary(_) => commit(
                owner.transaction.store_path(),
                owner.transaction.uid(),
                snapshot,
                updates,
                now(),
            )
            .map_err(|error| {
                if error == SubscriptionMutationCommitError::StoreIo {
                    owner.transaction.block();
                    NativeOwnerError::ManualRecoveryRequired
                } else {
                    NativeOwnerError::Subscription(subscription_store_error(error))
                }
            }),
            #[cfg(test)]
            Self::Historical(context) => {
                let result = (|| {
                    Self::check(context, owner, _lock)?;
                    let plan = crate::subscription_mutation::prepare_subscription_batch_commit(
                        owner.transaction.store_path(),
                        owner.transaction.uid(),
                        snapshot,
                        updates,
                        now(),
                    )
                    .map_err(|error| {
                        NativeOwnerError::Subscription(subscription_store_error(error))
                    })?;
                    context
                        .checkpoint(BatchWriteCheckpoint::Before)
                        .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
                    Self::check(context, owner, _lock)?;
                    let committed = plan
                        .commit_retained()
                        .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
                    context
                        .checkpoint(BatchWriteCheckpoint::AfterWriterBeforePin)
                        .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
                    // Pin the actual canonical output BEFORE any after-write hook.
                    context
                        .committed(&committed, _lock)
                        .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
                    context
                        .checkpoint(BatchWriteCheckpoint::After)
                        .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
                    Self::check(context, owner, _lock)?;
                    Ok(committed.outcome())
                })();
                if result.is_err() {
                    context.poison();
                    owner.transaction.block();
                }
                result
            }
        }
    }
}
