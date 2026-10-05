// SPDX-License-Identifier: MIT
//! Typed entry into the existing profile caller. Only Ordinary exists in
//! production. Historical research cannot dispatch or reach lifecycle changes.
use super::*;
use crate::profile_mutation::PreparedProfileMutation;
use crate::profile_transaction::ActionKind;
use std::marker::PhantomData;

#[cfg(test)]
use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::HistoricalProfile;
#[cfg(test)]
use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::ProfileWriteCheckpoint;

pub(super) enum ProfileAdmission<'a, 'b> {
    Ordinary(PhantomData<&'a &'b ()>),
    #[cfg(test)]
    Historical(&'a mut HistoricalProfile<'b>),
}

pub(super) struct ProfileLease<'a> {
    owned: Option<MigrationLock>,
    retained: Option<&'a MigrationLock>,
}
impl std::ops::Deref for ProfileLease<'_> {
    type Target = MigrationLock;
    fn deref(&self) -> &Self::Target {
        self.owned
            .as_ref()
            .or(self.retained)
            .expect("profile preflight always retains one lease")
    }
}

impl<'a, 'b> ProfileAdmission<'a, 'b> {
    pub(super) fn ordinary() -> Self {
        Self::Ordinary(PhantomData)
    }

    pub(super) fn kind(&self, _kind: ActionKind) -> Result<(), NativeOwnerError> {
        match self {
            Self::Ordinary(_) => Ok(()),
            #[cfg(test)]
            Self::Historical(_) if _kind == ActionKind::Favorite => Ok(()),
            #[cfg(test)]
            Self::Historical(_) => Err(NativeOwnerError::ManualRecoveryRequired),
        }
    }

    #[cfg(test)]
    fn check(
        context: &mut HistoricalProfile<'_>,
        transaction: &ConnectionTransactionState<impl LifecycleHost>,
    ) -> Result<(), NativeOwnerError> {
        context
            .check(
                transaction.cutover_paths(),
                transaction.desired_paths(),
                transaction.store_path(),
                transaction.uid(),
            )
            .map_err(|_| NativeOwnerError::ManualRecoveryRequired)
    }

    pub(super) fn admit<H: LifecycleHost>(
        &mut self,
        owner: &mut OfflineNativeCoordinator<H>,
        operation_id: Option<&str>,
        expected_revision: Option<u64>,
        digest: crate::mutation::MutationDigest,
    ) -> Result<Admission, NativeOwnerError> {
        match self {
            Self::Ordinary(_) => {
                owner.admit(MutationKind::Other, operation_id, expected_revision, digest)
            }
            #[cfg(test)]
            Self::Historical(context) => {
                context
                    .bind_owner(&owner.research_identity)
                    .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
                Self::check(context, &owner.transaction)?;
                if owner.transaction.independently_blocked() {
                    context.poison();
                    return Err(NativeOwnerError::ManualRecoveryRequired);
                }
                if owner.required_ownership.is_none_or(|f| {
                    f.phase != OwnershipPhase::Rust
                        || !owner.transaction.ownership_matches(f.phase, f.generation)
                }) {
                    context.poison();
                    return Err(NativeOwnerError::OwnershipUnavailable);
                }
                // Check again after marker observation, before Replay is possible.
                Self::check(context, &owner.transaction)?;
                owner.schedule(MutationKind::Other, operation_id, expected_revision, digest)
            }
        }
    }

    pub(super) fn blocked<H: LifecycleHost>(
        &self,
        owner: &mut OfflineNativeCoordinator<H>,
        token: MutationToken,
    ) -> Result<Option<NativeOwnerExecution>, NativeOwnerError> {
        match self {
            Self::Ordinary(_) => owner.blocked(
                token,
                NativeTransactionError::Profile(ProfileTransactionError::ManualRecoveryRequired),
            ),
            #[cfg(test)]
            Self::Historical(_) => Ok(None), // independently_blocked was checked under retained lease
        }
    }

    pub(super) fn preflight<H: LifecycleHost>(
        &mut self,
        owner: &mut OfflineNativeCoordinator<H>,
        token: MutationToken,
    ) -> Result<Result<ProfileLease<'b>, NativeOwnerExecution>, NativeOwnerError> {
        match self {
            Self::Ordinary(_) => Ok(
                match owner.preflight_lock(token, |error| {
                    NativeTransactionError::Profile(match error {
                        ConnectionTransactionError::Busy => ProfileTransactionError::Busy,
                        _ => ProfileTransactionError::Store,
                    })
                })? {
                    LockAdmission::Locked(lock) => Ok(ProfileLease {
                        owned: Some(lock),
                        retained: None,
                    }),
                    LockAdmission::Uncached(outcome) => Err(outcome),
                },
            ),
            #[cfg(test)]
            Self::Historical(context) => {
                if let Err(error) = Self::check(context, &owner.transaction) {
                    owner.coordinator.abort_active_uncached(token)?;
                    return Err(error);
                }
                Ok(Ok(ProfileLease {
                    owned: None,
                    retained: Some(context.lock()),
                }))
            }
        }
    }

    pub(super) fn apply<H: LifecycleHost>(
        &mut self,
        transaction: &mut ConnectionTransactionState<H>,
        plan: &PreparedProfileMutation,
        kind: ActionKind,
        profile_id: &str,
        lock: &MigrationLock,
    ) -> Result<ProfileMutationOutcome, ProfileTransactionError> {
        #[cfg(test)]
        if let Self::Historical(context) = self
            && (!std::ptr::eq(lock, context.lock())
                || Self::check(context, transaction).is_err()
                || context
                    .write_checkpoint(ProfileWriteCheckpoint::Before)
                    .is_err()
                || Self::check(context, transaction).is_err())
        {
            context.poison();
            return Err(ProfileTransactionError::ManualRecoveryRequired);
        }
        let paths = transaction.cutover_paths().clone();
        let result = apply_transaction(
            transaction.lifecycle_mut(),
            plan,
            kind,
            profile_id,
            lock,
            &paths,
        );
        #[cfg(test)]
        if let Self::Historical(context) = self
            && (result.is_err()
                || context.committed(plan).is_err()
                || context
                    .write_checkpoint(ProfileWriteCheckpoint::After)
                    .is_err()
                || Self::check(context, transaction).is_err())
        {
            context.poison();
            // The ordinary existence fence currently masks blocked() already;
            // retain an independent owner latch for any uncertain commit so a
            // freshly captured context cannot erase the failed transaction.
            transaction.block();
            return Err(ProfileTransactionError::ManualRecoveryRequired);
        }
        result
    }
}
