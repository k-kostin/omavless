// SPDX-License-Identifier: MIT
//! Store-only DNS intent behind the ordinary owner/revision/recovery fences.
use super::*;
use omavless_store::read_private_utf8;

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(crate) fn execute_dns(
        &mut self,
        request: &Value,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::dns_protocol::parse(request)?;
        let token = match self.admit(
            MutationKind::Other,
            Some(&parsed.operation_id),
            Some(parsed.expected_revision),
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
        let result = (|| {
            // Cached UI Off is never authority. Re-read desired and observe the
            // actual owned host under the same owner/migration serialization.
            let desired = self
                .transaction
                .desired()
                .map_err(|_| ProfileTransactionError::Store)?;
            if desired.connected || self.actual() != ActualState::Disconnected {
                return Err(ProfileTransactionError::Busy);
            }
            if self
                .transaction
                .lifecycle_mut()
                .observe_active_service()
                .map_err(|_| ProfileTransactionError::ManualRecoveryRequired)?
            {
                return Err(ProfileTransactionError::Busy);
            }
            let template_path = self
                .transaction
                .store_path()
                .parent()
                .ok_or(ProfileTransactionError::Store)?
                .join("route-template.yaml");
            let template = read_private_utf8(&template_path, self.uid())
                .map_err(|_| ProfileTransactionError::Store)?;
            if !crate::managed_template::dns_editable(&template) {
                return Err(ProfileTransactionError::InvalidArgument);
            }
            let plan = crate::profile_mutation::prepare_dns_preset(
                self.transaction.store_path(),
                self.uid(),
                parsed.preset,
            )
            .map_err(store_error)?;
            if read_private_utf8(&template_path, self.uid())
                .map_err(|_| ProfileTransactionError::Store)?
                != template
                || self
                    .transaction
                    .desired()
                    .map_err(|_| ProfileTransactionError::Store)?
                    != desired
            {
                return Err(ProfileTransactionError::Store);
            }
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
        })();
        self.finish(
            token,
            result
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            false,
        )
    }
}
