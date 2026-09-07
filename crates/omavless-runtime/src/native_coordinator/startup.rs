// SPDX-License-Identifier: MIT
//! Serialized future-login preferences, independent from live desired state.
use super::*;
use crate::desired::{DesiredState, RoutingMode};
use crate::profile_mutation::{PreparedWrite, prepare_startup_preferences};
use omavless_domain::private_store::parse_private_store;
use omavless_store::read_private_utf8;

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Foundation only: registration waits for the once-per-login host contract.
    /// Never calls connect, stop, prepare, or rewrites current desired state.
    pub fn execute_startup(
        &mut self,
        request: &Value,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::startup_protocol::parse_startup_request(request)?;
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
        let outcome = (|| {
            let store_path = self.transaction.store_path();
            let uid = self.transaction.uid();
            let plan = prepare_startup_preferences(store_path, uid, &parsed.preferences)
                .map_err(store_error)?;
            if parsed.preferences.enabled {
                let input = read_private_utf8(store_path, uid)
                    .map_err(|_| ProfileTransactionError::Store)?;
                let store =
                    parse_private_store(&input).map_err(|_| ProfileTransactionError::Store)?;
                let profile_id = store
                    .resolve_startup_selection(&parsed.preferences)
                    .map_err(|_| ProfileTransactionError::NotFound)?;
                let candidate = DesiredState {
                    connected: true,
                    profile_id,
                    mode: if parsed.preferences.mode == "global" {
                        RoutingMode::Global
                    } else {
                        RoutingMode::Rule
                    },
                    ..DesiredState::default()
                };
                self.transaction
                    .host_mut()
                    .validate_startup(&candidate)
                    .map_err(|error| {
                        if error == crate::lifecycle::HostStepError::Cleanup {
                            ProfileTransactionError::ManualRecoveryRequired
                        } else {
                            ProfileTransactionError::InvalidArgument
                        }
                    })?;
            }
            if !plan.changed() {
                return match plan
                    .commit_locked(&lock, self.transaction.cutover_paths())
                    .map_err(store_error)?
                {
                    PreparedWrite::NoChange => Ok(ProfileMutationOutcome { changed: false }),
                    PreparedWrite::Changed => Err(ProfileTransactionError::Store),
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
            outcome
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            false,
        )
    }
}
