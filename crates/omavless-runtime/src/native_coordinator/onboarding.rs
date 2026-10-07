// SPDX-License-Identifier: MIT
//! Completion is a compensated store-only mutation, never a tunnel transition.
use super::*;
use crate::profile_mutation::prepare_onboarding_completion;

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    #[cfg(feature = "t4-manager-actor-service")]
    pub(crate) fn execute_onboarding_native_completed(
        &mut self,
        request: &Value,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let original = self
            .native_completed_origin
            .as_ref()
            .ok_or(NativeOwnerError::ManualRecoveryRequired)?
            .clone();
        original
            .with_mutation(|loan| {
                self.execute_onboarding_original(request, loan)
                    .map_err(|_| ())
            })
            .map_err(|_| NativeOwnerError::ManualRecoveryRequired)
    }
    #[cfg(feature = "t4-manager-actor-service")]
    fn execute_onboarding_original(
        &mut self,
        request: &Value,
        loan: &mut NativeMutationLease<'_, '_>,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::onboarding_protocol::parse(request)?;
        loan.bind(self.transaction.cutover_paths(), self.transaction.uid())
            .map_err(|_| NativeOwnerError::ManualRecoveryRequired)?;
        if self.retained_restore_busy()
            || self.transaction.blocked()
            || !loan
                .lock()
                .authorizes(self.transaction.cutover_paths(), self.transaction.uid())
            || self.required_ownership.is_none_or(|fence| {
                fence.phase != OwnershipPhase::Rust
                    || !self
                        .transaction
                        .ownership_matches(fence.phase, fence.generation)
            })
            || self.actual() != ActualState::Disconnected
            || !self
                .transaction
                .desired()
                .is_ok_and(|desired| !desired.connected && desired.profile_id.is_empty())
        {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        let token = match self.schedule(
            MutationKind::Other,
            parsed.operation_id.as_deref(),
            parsed.expected_revision,
            parsed.digest,
        )? {
            Admission::Execute(token) => token,
            Admission::Replay(outcome) => return Ok(NativeOwnerExecution::Replay(outcome)),
            Admission::Rejected(outcome) => return Ok(NativeOwnerExecution::Rejected(outcome)),
        };
        let outcome = (|| {
            let plan = prepare_onboarding_completion(
                self.transaction.store_path(),
                self.transaction.uid(),
            )
            .map_err(store_error)?;
            loan.begin_store()
                .map_err(|_| ProfileTransactionError::ManualRecoveryRequired)?;
            let result =
                commit_onboarding_plan(&plan, loan.lock(), self.transaction.cutover_paths());
            let changed = match result {
                Ok(outcome) => outcome.changed,
                Err(error) => {
                    loan.refuse();
                    return Err(error);
                }
            };
            loan.reported_store(plan.planned_output(), changed)
                .map_err(|_| ProfileTransactionError::ManualRecoveryRequired)?;
            Ok(ProfileMutationOutcome { changed })
        })();
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            false,
        )
    }
    pub(crate) fn execute_onboarding(
        &mut self,
        request: &Value,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::onboarding_protocol::parse(request)?;
        let token = match self.admit(
            MutationKind::Other,
            parsed.operation_id.as_deref(),
            parsed.expected_revision,
            parsed.digest,
        )? {
            Admission::Execute(token) => token,
            Admission::Replay(outcome) => return Ok(NativeOwnerExecution::Replay(outcome)),
            Admission::Rejected(outcome) => return Ok(NativeOwnerExecution::Rejected(outcome)),
        };
        if let Some(outcome) = self.blocked(
            token,
            NativeTransactionError::Profile(ProfileTransactionError::ManualRecoveryRequired),
        )? {
            return Ok(outcome);
        }
        let lock = match self.preflight_lock(token, |error| {
            NativeTransactionError::Profile(if error == ConnectionTransactionError::Busy {
                ProfileTransactionError::Busy
            } else {
                ProfileTransactionError::Store
            })
        })? {
            LockAdmission::Locked(lock) => lock,
            LockAdmission::Uncached(outcome) => return Ok(outcome),
        };
        let outcome =
            prepare_onboarding_completion(self.transaction.store_path(), self.transaction.uid())
                .map_err(store_error)
                .and_then(|plan| {
                    commit_onboarding_plan(&plan, &lock, self.transaction.cutover_paths())
                });
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            false,
        )
    }
}

fn commit_onboarding_plan(
    plan: &crate::profile_mutation::PreparedProfileMutation,
    lock: &MigrationLock,
    paths: &crate::cutover::CutoverPaths,
) -> Result<ProfileMutationOutcome, ProfileTransactionError> {
    if !plan.changed() {
        return match plan.commit_locked(lock, paths).map_err(store_error)? {
            crate::profile_mutation::PreparedWrite::NoChange => {
                Ok(ProfileMutationOutcome { changed: false })
            }
            crate::profile_mutation::PreparedWrite::Changed => Err(ProfileTransactionError::Store),
        };
    }
    crate::profile_transaction::commit_store_only_profile(plan, lock, paths)
}
