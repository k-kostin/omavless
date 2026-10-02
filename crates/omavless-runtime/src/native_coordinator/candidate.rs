// SPDX-License-Identifier: MIT

//! Inactive typed v4 operations on the existing native owner. No new registry,
//! production constructor, protocol method or capability is introduced.

use super::*;
use crate::candidate_store_transaction::{
    CandidateStoreWriteError, PreparedCandidateStoreWrite, prepare_candidate_store_write,
    read_candidate_edit_input_locked, read_candidate_native_export_locked,
};
use crate::mutation::MutationDigest;
use crate::private_store_transaction::{PreparedWrite, PrivateStoreWriteError};
use omavless_domain::private_store::{
    CandidateProfileEditInput, CandidateProfileExport, CandidateProfileInput,
};

/// Private typed intent. Credentials are never formatted, serialized by a
/// client protocol, retained in the replay cache or converted to a fake URI.
pub enum CandidateProfileMutation {
    Import {
        profile_id: String,
        name: String,
        input: CandidateProfileInput,
    },
    Replace {
        profile_id: String,
        name: String,
        input: CandidateProfileInput,
    },
    Delete {
        profile_id: String,
    },
}

impl CandidateProfileMutation {
    fn target(&self) -> Option<&str> {
        match self {
            Self::Import { .. } => None,
            Self::Replace { profile_id, .. } | Self::Delete { profile_id } => Some(profile_id),
        }
    }

    fn digest(&self) -> Result<MutationDigest, NativeOwnerError> {
        let (method, profile_id, name, input) = match self {
            Self::Import {
                profile_id,
                name,
                input,
            } => (
                "p4-candidate-import-v1",
                profile_id,
                name.as_str(),
                Some(input),
            ),
            Self::Replace {
                profile_id,
                name,
                input,
            } => (
                "p4-candidate-replace-v1",
                profile_id,
                name.as_str(),
                Some(input),
            ),
            Self::Delete { profile_id } => ("p4-candidate-delete-v1", profile_id, "", None),
        };
        if profile_id.is_empty()
            || profile_id.len() > crate::desired::MAX_PROFILE_ID_BYTES
            || name.len() > crate::profile_mutation_protocol::MAX_PROFILE_NAME_INPUT_BYTES
            || input.is_some_and(|input| {
                matches!(input,
                CandidateProfileInput::Uri(uri)
                    if uri.len() > omavless_profile::MAX_CLASSIFICATION_INPUT_BYTES)
            })
        {
            return Err(NativeOwnerError::Protocol(
                MutationProtocolError::InvalidArgument,
            ));
        }
        let credential = match input {
            Some(CandidateProfileInput::Uri(uri)) => serde_json::json!(["uri", uri.trim()]),
            Some(CandidateProfileInput::WireGuard(profile)) => {
                let record = profile.private_record().map_err(|_| {
                    NativeOwnerError::Protocol(MutationProtocolError::InvalidArgument)
                })?;
                serde_json::json!(["wireguard", record.expose_private_bytes()])
            }
            None => Value::Null,
        };
        // Structured length-delimited fields avoid concatenation ambiguity.
        // Only the fixed SHA-256 digest enters the existing coordinator.
        Ok(MutationDigest::from_semantic_bytes(
            &serde_json::to_vec(&serde_json::json!([method, profile_id, name, credential]))
                .map_err(|_| NativeOwnerError::Invariant)?,
        ))
    }
}

fn candidate_error(error: CandidateStoreWriteError) -> ProfileTransactionError {
    match error {
        CandidateStoreWriteError::OwnershipUnavailable => ProfileTransactionError::Store,
        CandidateStoreWriteError::Write(error) => match error {
            PrivateStoreWriteError::StoreChanged => ProfileTransactionError::Conflict,
            PrivateStoreWriteError::Mutation(PrivateStoreError::ProfileNotFound) => {
                ProfileTransactionError::NotFound
            }
            PrivateStoreWriteError::Mutation(PrivateStoreError::DuplicateProfileName) => {
                ProfileTransactionError::Conflict
            }
            PrivateStoreWriteError::Mutation(
                PrivateStoreError::SubscribedProfile
                | PrivateStoreError::InvalidName
                | PrivateStoreError::InvalidShape
                | PrivateStoreError::Profile(_),
            ) => ProfileTransactionError::InvalidArgument,
            _ => ProfileTransactionError::Store,
        },
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    fn candidate_generation(&self) -> Result<u64, NativeOwnerError> {
        self.required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust)
            .map(|fence| fence.generation)
            .ok_or(NativeOwnerError::OwnershipUnavailable)
    }

    /// Deliberate revision-fenced private read. It is never replay-cached:
    /// export/editor credentials are not mutation outcomes. This method is
    /// unregistered and eventual authentication/framing remain mandatory.
    pub fn candidate_native_export(
        &mut self,
        expected_revision: u64,
        profile_id: &str,
    ) -> Result<CandidateProfileExport, NativeOwnerError> {
        self.candidate_private_read(expected_revision, |owner, lock, generation| {
            read_candidate_native_export_locked(
                owner.transaction.store_path(),
                owner.transaction.uid(),
                lock,
                owner.transaction.cutover_paths(),
                generation,
                profile_id,
            )
        })
    }

    pub fn candidate_edit_input(
        &mut self,
        expected_revision: u64,
        profile_id: &str,
    ) -> Result<CandidateProfileEditInput, NativeOwnerError> {
        self.candidate_private_read(expected_revision, |owner, lock, generation| {
            read_candidate_edit_input_locked(
                owner.transaction.store_path(),
                owner.transaction.uid(),
                lock,
                owner.transaction.cutover_paths(),
                generation,
                profile_id,
            )
        })
    }

    fn candidate_private_read<T>(
        &mut self,
        expected_revision: u64,
        read: impl FnOnce(&Self, &MigrationLock, u64) -> Result<T, CandidateStoreWriteError>,
    ) -> Result<T, NativeOwnerError> {
        let generation = self.candidate_generation()?;
        if expected_revision != self.revision() {
            return Err(CoordinatorError::RevisionConflict.into());
        }
        if self.actual() == ActualState::ManualRecoveryRequired || self.transaction.blocked() {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        let lock = self.transaction.acquire_lock().map_err(|error| {
            if error == ConnectionTransactionError::Busy {
                NativeOwnerError::OwnershipBusy
            } else {
                NativeOwnerError::OwnershipUnavailable
            }
        })?;
        read(self, &lock, generation).map_err(|error| match error {
            CandidateStoreWriteError::OwnershipUnavailable => {
                NativeOwnerError::OwnershipUnavailable
            }
            _ => NativeOwnerError::Candidate(candidate_error(error)),
        })
    }

    /// Shared revision/replay/ownership/recovery admission and real private
    /// publication. Active replace/delete refuse before effects: the installed
    /// lifecycle host still cannot restore/recover typed v4 records.
    pub fn execute_candidate_profile(
        &mut self,
        mutation: CandidateProfileMutation,
        operation_id: &str,
        expected_revision: u64,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        self.execute_candidate_with_commit(
            mutation,
            operation_id,
            expected_revision,
            |plan, lock| plan.commit_locked(lock),
        )
    }

    fn execute_candidate_with_commit(
        &mut self,
        mutation: CandidateProfileMutation,
        operation_id: &str,
        expected_revision: u64,
        commit: impl FnOnce(
            &PreparedCandidateStoreWrite,
            &MigrationLock,
        ) -> Result<PreparedWrite, CandidateStoreWriteError>,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let generation = self.candidate_generation()?;
        let digest = mutation.digest()?;
        let token = match self.admit(
            MutationKind::Other,
            Some(operation_id),
            Some(expected_revision),
            digest,
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
        let mut unproved_publication = false;
        let outcome = (|| {
            let target = mutation.target().map(str::to_owned);
            let desired = self
                .transaction
                .desired()
                .map_err(|_| ProfileTransactionError::Store)?;
            if target
                .as_deref()
                .is_some_and(|id| desired.connected && desired.profile_id == id)
            {
                return Err(ProfileTransactionError::Conflict);
            }
            let mut active_target = false;
            let plan = prepare_candidate_store_write(
                self.transaction.store_path(),
                self.transaction.uid(),
                &lock,
                self.transaction.cutover_paths(),
                generation,
                |candidate| {
                    if target
                        .as_deref()
                        .is_some_and(|id| candidate.references_active_profile(id))
                    {
                        active_target = true;
                        return Err(PrivateStoreError::InvalidShape);
                    }
                    match mutation {
                        CandidateProfileMutation::Import {
                            profile_id,
                            name,
                            input,
                        } => candidate
                            .with_profile(&profile_id, &name, input)
                            .map(|store| (store, true)),
                        CandidateProfileMutation::Replace {
                            profile_id,
                            name,
                            input,
                        } => candidate.replace_standalone(&profile_id, &name, input),
                        CandidateProfileMutation::Delete { profile_id } => candidate
                            .delete_standalone(&profile_id)
                            .map(|store| (store, true)),
                    }
                },
            )
            .map_err(|error| {
                if active_target {
                    ProfileTransactionError::Conflict
                } else {
                    candidate_error(error)
                }
            })?;
            // Observation only, never prepare/start/stop. Ambiguous owned
            // state is not treated as disconnected merely because core is absent.
            self.transaction
                .lifecycle_mut()
                .observe_active_service()
                .map_err(|_| ProfileTransactionError::ManualRecoveryRequired)?;
            if self
                .transaction
                .desired()
                .map_err(|_| ProfileTransactionError::Store)?
                != desired
            {
                return Err(ProfileTransactionError::Conflict);
            }
            match commit(&plan, &lock) {
                Ok(PreparedWrite::Changed) if plan.changed() => {
                    Ok(ProfileMutationOutcome { changed: true })
                }
                Ok(PreparedWrite::NoChange) if !plan.changed() => {
                    Ok(ProfileMutationOutcome { changed: false })
                }
                result => {
                    if plan.restore_locked(&lock).is_err() {
                        unproved_publication = true;
                        return Err(ProfileTransactionError::ManualRecoveryRequired);
                    }
                    Err(result
                        .err()
                        .map_or(ProfileTransactionError::Store, candidate_error))
                }
            }
        })();
        if outcome == Err(ProfileTransactionError::ManualRecoveryRequired) {
            // Store compensation uncertainty is an auxiliary owner barrier,
            // not a disconnected lifecycle observation. Expose the same hard
            // recovery status used by other non-connection native families.
            self.auxiliary_recovery_required = true;
        }
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            unproved_publication,
        )
    }
}

#[cfg(test)]
mod tests;
