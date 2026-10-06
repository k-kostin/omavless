// SPDX-License-Identifier: MIT

//! Inactive automatic-refresh owner composition. Durable admission precedes
//! release of runnable work; the existing batch registry, permits, member
//! validation and atomic store transaction remain authoritative.

use super::*;
use crate::subscription_batch_work::{
    BatchWorkError, BatchWorkStep, BudgetedSubscriptionTransport,
};
use crate::subscription_mutation::commit_subscription_refresh_batch_with_policy;
use crate::subscription_schedule_attempt::{
    AttemptError, AttemptSnapshot, AttemptState, AttemptTicket, begin_attempt_for_batch,
    finish_attempt_with_receipt, read_attempt,
};
use crate::subscription_schedule_plan::{
    AttemptHistory, OwnerFence, RefreshSchedule, ScheduleDecision, plan_refresh,
};
use crate::subscription_schedule_preference::{
    PreferenceError, PreferenceSnapshot, read_locked, read_preference, set_preference,
};
use serde_json::json;

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticRefreshError {
    Owner(NativeOwnerError),
    Preference(PreferenceError),
    Attempt(AttemptError),
    WorkerLost,
    StaleWorker,
    ClockInvalid,
}

impl From<NativeOwnerError> for AutomaticRefreshError {
    fn from(value: NativeOwnerError) -> Self {
        Self::Owner(value)
    }
}
impl From<PreferenceError> for AutomaticRefreshError {
    fn from(value: PreferenceError) -> Self {
        Self::Preference(value)
    }
}
impl From<AttemptError> for AutomaticRefreshError {
    fn from(value: AttemptError) -> Self {
        Self::Attempt(value)
    }
}

#[derive(Default)]
pub(super) struct AutomaticRefreshState {
    active: Option<AutomaticAttempt>,
    blocked: Option<AutomaticRefreshError>,
    next_operation: u64,
}

struct AutomaticAttempt {
    journal: AttemptTicket,
    batch: NativeBatchTicket,
    operation_id: String,
    preference_revision: u64,
    started_at_secs: u64,
}

pub enum AutomaticRefreshStart {
    Idle(ScheduleDecision),
    Started(AutomaticSubscriptionBatch),
}

/// Returned only AFTER a confirmed durable journal start. Neither the batch
/// nor its terminal authority can be extracted to bypass the schedule owner.
pub struct AutomaticSubscriptionBatch {
    job: NativeSubscriptionBatch,
}

impl AutomaticSubscriptionBatch {
    pub(crate) fn step<T, G>(
        &mut self,
        transport: &T,
        pool: &crate::remote_fetch::RemoteFetchPool,
        ids: &mut G,
    ) -> Result<BatchWorkStep, BatchWorkError>
    where
        T: BudgetedSubscriptionTransport,
        G: FnMut() -> String,
    {
        self.job.step(transport, pool, ids)
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    fn automatic_generation(&self) -> Result<u64, AutomaticRefreshError> {
        self.required_ownership
            .filter(|fence| fence.phase == OwnershipPhase::Rust && fence.generation != 0)
            .map(|fence| fence.generation)
            .ok_or(AutomaticRefreshError::Owner(
                NativeOwnerError::OwnershipUnavailable,
            ))
    }

    fn automatic_fence(&self) -> Result<OwnerFence, AutomaticRefreshError> {
        let generation = self.automatic_generation()?;
        Ok(OwnerFence {
            expected_generation: generation,
            current_generation: generation,
            expected_revision: self.revision(),
            current_revision: self.revision(),
        })
    }

    /// Explicit owner-serialized choice. Any changed preference cancels the
    /// exact admitted automatic batch. A failed publication latches uncertainty
    /// even if a later read appears to contain the desired preference.
    pub fn set_automatic_subscription_preference(
        &mut self,
        expected_revision: u64,
        schedule: RefreshSchedule,
    ) -> Result<PreferenceSnapshot, AutomaticRefreshError> {
        let generation = self.automatic_generation()?;
        let result = set_preference(
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            generation,
            expected_revision,
            schedule,
        );
        self.accept_automatic_preference_result(schedule, result)
    }

    fn accept_automatic_preference_result(
        &mut self,
        schedule: RefreshSchedule,
        result: Result<PreferenceSnapshot, PreferenceError>,
    ) -> Result<PreferenceSnapshot, AutomaticRefreshError> {
        match result {
            Ok(snapshot) => {
                if self
                    .automatic_refresh
                    .active
                    .as_ref()
                    .is_some_and(|active| {
                        active.preference_revision != snapshot.revision
                            || schedule == RefreshSchedule::Off
                    })
                {
                    self.cancel_automatic_subscription_refresh()?;
                }
                Ok(snapshot)
            }
            Err(error) => {
                if error == PreferenceError::WriteUncertain {
                    self.automatic_refresh.blocked = Some(error.into());
                    let _ = self.cancel_automatic_subscription_refresh();
                }
                Err(error.into())
            }
        }
    }

    /// Uses trusted instance/time inputs, never client scheduling or URLs. The
    /// journal and the live owner registry must BOTH agree before admission.
    pub fn start_automatic_subscription_refresh(
        &mut self,
        instance: &str,
        now_secs: u64,
    ) -> Result<AutomaticRefreshStart, AutomaticRefreshError> {
        if let Some(error) = self.automatic_refresh.blocked {
            return Err(error);
        }
        if self.automatic_refresh.active.is_some() {
            return Err(NativeOwnerError::LongOperation(
                crate::long_operation::LongOperationError::Busy,
            )
            .into());
        }
        let fence = self.automatic_fence()?;
        let preference = read_preference(
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            fence.expected_generation,
        )?;
        if preference.schedule == RefreshSchedule::Off {
            return Ok(AutomaticRefreshStart::Idle(ScheduleDecision::Off));
        }
        let previous = read_attempt(
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            fence.expected_generation,
            instance,
        )?;
        if previous.is_some_and(|attempt| {
            matches!(
                attempt.state,
                AttemptState::StartedInCurrentInstance
                    | AttemptState::UncertainFromPreviousInstance
            )
        }) {
            // A matching PID/time instance string is not evidence of a worker.
            return Err(AttemptError::AttemptUncertain.into());
        }
        let history = previous.map(|attempt| AttemptHistory {
            last_attempt_at_secs: attempt
                .finished_at_secs
                .expect("validated terminal attempt"),
            consecutive_failures: attempt.consecutive_failures,
        });
        let decision = plan_refresh(preference.schedule, history, now_secs, fence)
            .map_err(|_| AutomaticRefreshError::ClockInvalid)?;
        if decision != ScheduleDecision::Due {
            return Ok(AutomaticRefreshStart::Idle(decision));
        }
        now_secs
            .checked_mul(1000)
            .ok_or(AutomaticRefreshError::ClockInvalid)?;
        if let Some(state) = &self.batch {
            if state.instance != instance || state.stopped {
                return Err(NativeOwnerError::OwnershipUnavailable.into());
            }
        } else {
            self.initialize_batch_operations(instance)?;
        }
        let sequence = self
            .automatic_refresh
            .next_operation
            .checked_add(1)
            .ok_or(AttemptError::CounterExhausted)?;
        self.automatic_refresh.next_operation = sequence;
        let operation_id = format!("scheduled-subscription-{sequence}");
        let request = json!({"api":"omavless.control","version":1,"id":"automatic-start", "method":"subscriptions.refresh_all", "params":{
            "instanceId":instance, "operationId":operation_id, "expectedRevision":fence.current_revision
        }});
        let job = self
            .start_subscription_batch(&request)?
            .ok_or(AutomaticRefreshError::StaleWorker)?;
        let batch = job.supervisor_ticket();
        // The admitted job is local and cannot run before this publication.
        let journal = match begin_attempt_for_batch(
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            &batch,
            preference.revision,
            now_secs,
            fence,
        ) {
            Ok(ticket) => ticket,
            Err(error) => {
                let _ = self.abort_subscription_batch(batch);
                if error == AttemptError::WriteUncertain {
                    self.automatic_refresh.blocked = Some(error.into());
                }
                return Err(error.into());
            }
        };
        self.automatic_refresh.active = Some(AutomaticAttempt {
            journal,
            batch,
            operation_id,
            preference_revision: preference.revision,
            started_at_secs: now_secs,
        });
        Ok(AutomaticRefreshStart::Started(AutomaticSubscriptionBatch {
            job,
        }))
    }

    pub fn cancel_automatic_subscription_refresh(&mut self) -> Result<bool, AutomaticRefreshError> {
        let Some(active) = &self.automatic_refresh.active else {
            return Ok(false);
        };
        let request = json!({"api":"omavless.control","version":1,"id":"automatic-cancel", "method":"operations.cancel", "params":{
            "instanceId":active.batch.instance(), "operationId":active.operation_id
        }});
        self.cancel_subscription_batch(&request).map_err(Into::into)
    }

    /// Before each detached worker step, prove the exact registry token, owner,
    /// revision and preference. Cancellation remains cooperative during I/O;
    /// final publication repeats preference validation under the commit lease.
    pub(crate) fn check_automatic_subscription_work(
        &mut self,
        work: &AutomaticSubscriptionBatch,
    ) -> Result<(), AutomaticRefreshError> {
        let active = self
            .automatic_refresh
            .active
            .as_ref()
            .ok_or(AutomaticRefreshError::StaleWorker)?;
        let ticket = work.job.supervisor_ticket();
        if active.batch.instance() != ticket.instance()
            || active.batch.sequence() != ticket.sequence()
        {
            return Err(AutomaticRefreshError::StaleWorker);
        }
        let _lock = self.batch_lock()?;
        let generation = self.automatic_generation()?;
        let preference = read_locked(
            self.transaction.cutover_paths(),
            self.transaction.uid(),
            generation,
        )?;
        if self.automatic_refresh.blocked.is_some()
            || preference.owner_generation != generation
            || preference.revision != active.preference_revision
            || preference.schedule == RefreshSchedule::Off
            || ticket.base_revision() != Some(self.revision())
        {
            self.cancel_automatic_subscription_refresh()?;
        }
        self.publish_subscription_batch_progress(&work.job)?;
        Ok(())
    }

    pub(crate) fn finish_automatic_subscription_refresh(
        &mut self,
        work: AutomaticSubscriptionBatch,
        now_secs: u64,
    ) -> Result<AttemptSnapshot, AutomaticRefreshError> {
        let desired_paths = self.transaction.desired_paths().clone();
        self.finish_automatic_with_store(
            work,
            now_secs,
            move |path, uid, snapshot, updates, stamp| {
                // Called inside the batch commit lease after ownership/revision and
                // preference fencing. Protect the selected profile's exact config
                // contribution without observing, stopping or restarting the core.
                let desired = crate::desired::read_desired_snapshot(&desired_paths, uid)
                    .map_err(|_| SubscriptionMutationCommitError::UnsafeStore)?;
                commit_subscription_refresh_batch_with_policy(
                    path,
                    uid,
                    snapshot,
                    updates,
                    stamp,
                    |before, candidate| {
                        if desired.connected
                            && !unchanged_selected_profile(before, candidate, &desired.profile_id)
                        {
                            return Err(SubscriptionMutationCommitError::Mutation(
                                PrivateStoreError::ActiveSubscription,
                            ));
                        }
                        Ok(())
                    },
                )
            },
        )
    }

    pub(super) fn finish_automatic_with_store<F>(
        &mut self,
        work: AutomaticSubscriptionBatch,
        now_secs: u64,
        commit: F,
    ) -> Result<AttemptSnapshot, AutomaticRefreshError>
    where
        F: FnOnce(
            &Path,
            u32,
            omavless_domain::private_store::SubscriptionRefreshBatchSnapshot,
            Vec<omavless_domain::private_store::SubscriptionRefreshBatchEntries>,
            u64,
        ) -> Result<SubscriptionRefreshCommit, SubscriptionMutationCommitError>,
    {
        let ticket = work.job.supervisor_ticket();
        let active = self
            .automatic_refresh
            .active
            .as_ref()
            .ok_or(AutomaticRefreshError::StaleWorker)?;
        if active.batch.instance() != ticket.instance()
            || active.batch.sequence() != ticket.sequence()
        {
            return Err(AutomaticRefreshError::StaleWorker);
        }
        let active = self
            .automatic_refresh
            .active
            .take()
            .expect("checked active attempt");
        let generation = self.automatic_generation()?;
        let clock_valid =
            now_secs >= active.started_at_secs && now_secs.checked_mul(1000).is_some();
        if !clock_valid || self.automatic_refresh.blocked.is_some() {
            let _ = self.abort_subscription_batch(ticket);
            let error = self
                .automatic_refresh
                .blocked
                .unwrap_or(AutomaticRefreshError::ClockInvalid);
            self.automatic_refresh.blocked = Some(error);
            return Err(error);
        }
        let preference_revision = active.preference_revision;
        let _completion = self.complete_subscription_batch_with_guard(
            work.job,
            || now_secs * 1000,
            commit,
            |owner| {
                let preference = read_locked(
                    owner.transaction.cutover_paths(),
                    owner.transaction.uid(),
                    generation,
                )
                .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
                if preference.owner_generation != generation
                    || preference.revision != preference_revision
                    || preference.schedule == RefreshSchedule::Off
                {
                    return Err(NativeOwnerError::Coordinator(
                        CoordinatorError::RevisionConflict,
                    ));
                }
                Ok(())
            },
        );
        let result = (|| {
            let receipt = self.subscription_batch_receipt(&active.batch)?;
            finish_attempt_with_receipt(
                self.transaction.cutover_paths(),
                self.transaction.uid(),
                active.journal,
                receipt,
                now_secs,
                self.automatic_fence()?,
            )
            .map_err(Into::into)
        })();
        if let Err(error) = result {
            self.automatic_refresh.blocked = Some(error);
        }
        result
    }

    /// Lost runnable work is never guessed to have failed. The shared batch
    /// slot may be reclaimed, but the durable Started journal remains blocked.
    pub(crate) fn lose_automatic_subscription_worker(&mut self, work: AutomaticSubscriptionBatch) {
        let ticket = work.job.supervisor_ticket();
        if self
            .automatic_refresh
            .active
            .as_ref()
            .is_some_and(|active| {
                active.batch.instance() == ticket.instance()
                    && active.batch.sequence() == ticket.sequence()
            })
        {
            let _ = self.abort_subscription_batch(ticket);
            self.automatic_refresh.active = None;
            self.automatic_refresh.blocked = Some(AutomaticRefreshError::WorkerLost);
        }
    }
}

/// Both documents have already passed the canonical full-store validator.
/// Metadata/preferences unrelated to the selected proxy may change; private
/// URI or display name changes conservatively defer the entire automatic batch.
fn unchanged_selected_profile(before: &str, candidate: &[u8], selected: &str) -> bool {
    let Ok(before) = serde_json::from_str::<Value>(before) else {
        return false;
    };
    let Ok(candidate) = serde_json::from_slice::<Value>(candidate) else {
        return false;
    };
    let row = |document: Value| {
        document["profiles"]
            .as_array()
            .and_then(|profiles| profiles.iter().find(|profile| profile["id"] == selected))
            .cloned()
    };
    let (Some(before), Some(candidate)) = (row(before), row(candidate)) else {
        return false;
    };
    before["missing"] != true
        && candidate["missing"] != true
        && before["uri"] == candidate["uri"]
        && before["name"] == candidate["name"]
}
