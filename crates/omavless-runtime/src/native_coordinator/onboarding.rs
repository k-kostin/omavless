// SPDX-License-Identifier: MIT
//! Completion is a compensated store-only mutation, never a tunnel transition.
use super::*;
use crate::profile_mutation::prepare_onboarding_completion;

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
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
                    if !plan.changed() {
                        return match plan
                            .commit_locked(&lock, self.transaction.cutover_paths())
                            .map_err(store_error)?
                        {
                            crate::profile_mutation::PreparedWrite::NoChange => {
                                Ok(ProfileMutationOutcome { changed: false })
                            }
                            crate::profile_mutation::PreparedWrite::Changed => {
                                Err(ProfileTransactionError::Store)
                            }
                        };
                    }
                    crate::profile_transaction::commit_store_only_profile(
                        &plan,
                        &lock,
                        self.transaction.cutover_paths(),
                    )
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
