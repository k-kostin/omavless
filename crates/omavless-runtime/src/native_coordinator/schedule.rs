// SPDX-License-Identifier: MIT
//! Schedule admission uses the ordinary owner and its existing migration lease.
use super::*;
use crate::private_store_transaction::{
    PreparedPrivateStoreWrite, PreparedWrite, PrivateStoreWriteError, prepare_private_store_write,
};
use omavless_domain::private_store::{
    ScheduledAttempt, admit_subscription_schedule, parse_private_store,
};

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(crate) fn execute_subscription_schedule<N: FnOnce() -> u64>(
        &mut self,
        request: &Value,
        now: N,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::subscription_schedule::parse(request)?;
        if self.batch.as_ref().is_some_and(|s| s.active.is_some())
            && !self.mutation_operation_known(request)
        {
            return Err(NativeOwnerError::LongOperation(
                crate::long_operation::LongOperationError::Busy,
            ));
        }
        let token = match self.admit(
            MutationKind::Other,
            Some(&parsed.operation),
            Some(parsed.revision),
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
        let lock = match self.preflight_lock(token, NativeTransactionError::Connection)? {
            LockAdmission::Locked(lock) => lock,
            LockAdmission::Uncached(outcome) => return Ok(outcome),
        };
        let result = (|| {
            let plan = crate::profile_mutation::prepare_subscription_schedule(
                self.transaction.store_path(),
                self.transaction.uid(),
                &parsed.id,
                parsed.enabled,
                now(),
            )
            .map_err(store_error)?;
            if !plan.changed() {
                plan.commit_locked(&lock, self.transaction.cutover_paths())
                    .map_err(store_error)?;
                return Ok(ProfileMutationOutcome { changed: false });
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

    pub(crate) fn subscription_schedule(
        &mut self,
        request: &Value,
        instance: &str,
    ) -> Result<Value, NativeOwnerError> {
        let id = crate::subscription_schedule::parse_get(request)?;
        let _lock = self.batch_lock()?;
        crate::private_store_transaction::validate_store_path(
            self.transaction.store_path(),
            self.transaction.uid(),
        )
        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let input = omavless_store::read_private_utf8(
            self.transaction.store_path(),
            self.transaction.uid(),
        )
        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let store =
            parse_private_store(&input).map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let mut result = store
            .subscription_schedule(&id, instance)
            .map_err(|error| {
                NativeOwnerError::Subscription(subscription_store_error(
                    crate::subscription_mutation::SubscriptionMutationCommitError::Mutation(error),
                ))
            })?;
        // A retained terminal operation is stronger evidence than admission.
        // Foreign-instance or evicted admission is Unknown, never a fresh job.
        if result["lastAttempt"]["disposition"] == "admitted" {
            let operation = result["lastAttempt"]["operationId"].clone();
            let lookup = omavless_control_protocol::make_request(
                "schedule-read",
                "operations.get",
                serde_json::json!({"instanceId":instance,"operationId":operation}),
            )
            .map_err(|_| NativeOwnerError::Invariant)?;
            match self.subscription_batch_status(&lookup) {
                Ok(projection) => {
                    result["operation"] = projection["operation"].clone();
                }
                Err(_) => result["lastAttempt"]["disposition"] = serde_json::json!("unknown"),
            }
        }
        Ok(result)
    }

    pub(crate) fn start_due_subscription_batch(
        &mut self,
        instance: &str,
        operation: &str,
        now: u64,
    ) -> Result<Option<NativeSubscriptionBatch>, NativeOwnerError> {
        let lock = self.batch_lock()?;
        let state = self
            .batch
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.stopped || state.instance != instance {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if state.active.is_some() {
            return Ok(None);
        }
        let mut selected = None;
        let mut attempt = None;
        let plan = prepare_private_store_write(
            self.transaction.store_path(),
            self.transaction.uid(),
            |input| {
                let store = parse_private_store(input)?;
                let Some(due) = store.due_subscription(now) else {
                    return Ok((input.as_bytes().to_vec(), false));
                };
                let binding = ScheduledAttempt {
                    subscription_id: due.subscription_id.clone(),
                    instance_id: instance.to_owned(),
                    operation_id: operation.to_owned(),
                };
                let result = admit_subscription_schedule(input, &due, &binding, now)?;
                selected = Some(due.subscription_id);
                attempt = Some(binding);
                Ok(result)
            },
        )
        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let Some(selected) = selected else {
            return Ok(None);
        };
        self.commit_schedule_admission(&plan, &lock)?;
        let request = omavless_control_protocol::make_request("scheduled-refresh", "subscriptions.refresh_selected", serde_json::json!({"instanceId":instance,"operationId":operation,"subscriptionId":selected,"expectedRevision":self.revision()})).map_err(|_| NativeOwnerError::Invariant)?;
        let parsed = crate::long_operation_protocol::parse_selected_refresh_start(&request)?;
        let mut job = self.start_subscription_batch_locked(
            parsed.metadata,
            crate::long_operation_protocol::LongOperationMethod::SubscriptionRefreshSelected,
            Some(parsed.subscription_id),
            &mut super::batch_admission::BatchAdmission::ordinary(),
            &lock,
        )?;
        if let Some(job) = &mut job {
            job.scheduled = attempt;
        }
        Ok(job)
    }

    pub(super) fn record_schedule_terminal(&mut self, attempt: &ScheduledAttempt, now: u64) {
        use omavless_domain::private_store::{ScheduleDisposition, finish_subscription_schedule};
        // Unknown publication fences all further writes; never overwrite it
        // with a convenient failure label or re-admit its slot.
        if self.transaction.blocked() {
            return;
        }
        let Ok(lookup) = omavless_control_protocol::make_request(
            "schedule-terminal",
            "operations.get",
            serde_json::json!({"instanceId":attempt.instance_id,"operationId":attempt.operation_id}),
        ) else {
            return;
        };
        let Ok(status) = self.subscription_batch_status(&lookup) else {
            return;
        };
        let disposition = match status["operation"]["state"].as_str() {
            Some("cancelled") => ScheduleDisposition::CancelledBeforeCommit,
            Some("failed") => ScheduleDisposition::FailedBeforeCommit,
            // Success already persisted both feed and receipt in one rename.
            _ => return,
        };
        let Ok(lock) = self.batch_lock() else {
            return;
        };
        let Ok(plan) = prepare_private_store_write(
            self.transaction.store_path(),
            self.transaction.uid(),
            |input| finish_subscription_schedule(input, attempt, disposition, now),
        ) else {
            return;
        };
        let _ = self.commit_schedule_admission(&plan, &lock);
    }

    fn commit_schedule_admission(
        &mut self,
        plan: &PreparedPrivateStoreWrite,
        lock: &crate::connection_transaction::MigrationLease,
    ) -> Result<(), NativeOwnerError> {
        let token = match self.schedule(
            MutationKind::Other,
            None,
            Some(self.revision()),
            crate::mutation::MutationDigest::from_semantic_bytes(b"scheduled-admission-v1"),
        )? {
            Admission::Execute(token) => token,
            _ => return Err(NativeOwnerError::Invariant),
        };
        let outcome = match plan.commit_locked(lock, self.transaction.cutover_paths()) {
            Ok(PreparedWrite::Changed) => Ok(ProfileMutationOutcome { changed: true }),
            Err(PrivateStoreWriteError::StoreIo) => {
                Err(ProfileTransactionError::ManualRecoveryRequired)
            }
            _ => Err(ProfileTransactionError::Store),
        };
        let failed = outcome.is_err();
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            false,
        )?;
        if failed {
            Err(if self.transaction.blocked() {
                NativeOwnerError::ManualRecoveryRequired
            } else {
                NativeOwnerError::OwnershipUnavailable
            })
        } else {
            Ok(())
        }
    }
}
