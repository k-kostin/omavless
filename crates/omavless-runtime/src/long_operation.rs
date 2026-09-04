// SPDX-License-Identifier: MIT

//! Pure bounded state model for future per-instance long operations.
//!
//! This registry contains only safe progress metadata, correlation IDs and a
//! fixed semantic digest. It has no threads, timers, sockets, store access or
//! provider transport. The live executor must collision-check IDs against the
//! ordinary mutation replay ledger before calling [`LongOperationRegistry::start`].

use crate::long_operation_protocol::{
    LongOperationProgress, LongOperationProjection, LongOperationState, MAX_INSTANCE_ID_BYTES,
    MAX_REFRESH_ALL_SUBSCRIPTIONS,
};
use crate::mutation::MutationDigest;
use omavless_control_protocol::{MAX_ID_LENGTH, MAX_REVISION, StableErrorCode};
use std::collections::VecDeque;
use std::fmt;

pub const DEFAULT_COMPLETED_OPERATION_LIMIT: usize = 128;
pub const MAX_COMPLETED_OPERATION_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongOperationError {
    InvalidBounds,
    InvalidOperationId,
    InvalidInstanceId,
    InstanceMismatch,
    RevisionConflict,
    OperationConflict,
    Busy,
    NotFound,
    InvalidToken,
    InvalidState,
    TokenExhausted,
}

impl LongOperationError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::InvalidBounds
            | Self::InvalidOperationId
            | Self::InvalidInstanceId
            | Self::InvalidToken
            | Self::InvalidState => StableErrorCode::InvalidArgument,
            Self::RevisionConflict | Self::OperationConflict => StableErrorCode::Conflict,
            Self::Busy => StableErrorCode::Busy,
            Self::NotFound | Self::InstanceMismatch => StableErrorCode::NotFound,
            Self::TokenExhausted => StableErrorCode::InternalError,
        }
    }
}

impl fmt::Display for LongOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBounds => "Long-operation bounds are invalid",
            Self::InvalidOperationId => "Long-operation ID is invalid",
            Self::InvalidInstanceId => "Long-operation instance ID is invalid",
            Self::InstanceMismatch => "Long operation was not found",
            Self::RevisionConflict => "Long-operation revision conflicts with current state",
            Self::OperationConflict => "Operation ID was reused for different input",
            Self::Busy => "Another long operation is active",
            Self::NotFound => "Long operation was not found",
            Self::InvalidToken => "Long-operation token is invalid",
            Self::InvalidState => "Long-operation transition is invalid",
            Self::TokenExhausted => "Long-operation token space is exhausted",
        })
    }
}

impl std::error::Error for LongOperationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongOperationToken(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartOutcome {
    Started(LongOperationToken),
    Replay(LongOperationToken),
}

impl StartOutcome {
    #[must_use]
    pub const fn token(self) -> LongOperationToken {
        match self {
            Self::Started(token) | Self::Replay(token) => token,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitFence {
    Ready,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginOperation {
    Running,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelOutcome {
    pub accepted: bool,
    pub token: LongOperationToken,
}

struct OperationRecord {
    token: LongOperationToken,
    operation_id: String,
    digest: MutationDigest,
    state: LongOperationState,
    base_revision: u64,
    outcome_revision: Option<u64>,
    completed: usize,
    total: usize,
    cancel_requested: bool,
    cancellable: bool,
    error: Option<StableErrorCode>,
}

impl OperationRecord {
    fn projection<'a>(&'a self, instance_id: &'a str) -> LongOperationProjection<'a> {
        LongOperationProjection {
            instance_id,
            operation_id: &self.operation_id,
            state: self.state,
            base_revision: self.base_revision,
            outcome_revision: self.outcome_revision,
            progress: LongOperationProgress {
                completed: self.completed,
                total: self.total,
            },
            cancel_requested: self.cancel_requested,
            cancellable: self.cancellable,
            error: self.error,
        }
    }
}

pub struct LongOperationRegistry {
    instance_id: String,
    next_token: u64,
    completed_limit: usize,
    active: Option<OperationRecord>,
    completed: VecDeque<OperationRecord>,
}

impl Default for LongOperationRegistry {
    fn default() -> Self {
        Self::new("test-instance", DEFAULT_COMPLETED_OPERATION_LIMIT)
            .expect("default long-operation bounds are valid")
    }
}

impl LongOperationRegistry {
    pub fn new(instance_id: &str, limit: usize) -> Result<Self, LongOperationError> {
        if limit == 0 || limit > MAX_COMPLETED_OPERATION_LIMIT {
            return Err(LongOperationError::InvalidBounds);
        }
        if !Self::valid_instance_id(instance_id) {
            return Err(LongOperationError::InvalidInstanceId);
        }
        Ok(Self {
            instance_id: instance_id.to_owned(),
            next_token: 1,
            completed_limit: limit,
            active: None,
            completed: VecDeque::new(),
        })
    }

    fn valid_operation_id(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= MAX_ID_LENGTH
            && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    }

    fn valid_instance_id(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= MAX_INSTANCE_ID_BYTES
            && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    }

    fn require_instance(&self, instance_id: &str) -> Result<(), LongOperationError> {
        if instance_id == self.instance_id {
            Ok(())
        } else {
            Err(LongOperationError::InstanceMismatch)
        }
    }

    fn record_by_id(&self, operation_id: &str) -> Option<&OperationRecord> {
        self.active
            .as_ref()
            .filter(|record| record.operation_id == operation_id)
            .or_else(|| {
                self.completed
                    .iter()
                    .rev()
                    .find(|record| record.operation_id == operation_id)
            })
    }

    fn active_mut(
        &mut self,
        token: LongOperationToken,
    ) -> Result<&mut OperationRecord, LongOperationError> {
        self.active
            .as_mut()
            .filter(|record| record.token == token)
            .ok_or(LongOperationError::InvalidToken)
    }

    fn finish_active(
        &mut self,
        token: LongOperationToken,
        state: LongOperationState,
        outcome_revision: u64,
        error: Option<StableErrorCode>,
    ) -> Result<(), LongOperationError> {
        if outcome_revision > MAX_REVISION {
            return Err(LongOperationError::RevisionConflict);
        }
        if self.active.as_ref().map(|record| record.token) != Some(token) {
            return Err(LongOperationError::InvalidToken);
        }
        let active = self
            .active
            .as_ref()
            .ok_or(LongOperationError::InvalidToken)?;
        if outcome_revision < active.base_revision {
            return Err(LongOperationError::RevisionConflict);
        }
        if state == LongOperationState::Succeeded {
            let expected = if active.total == 0 {
                active.base_revision
            } else {
                active
                    .base_revision
                    .checked_add(1)
                    .ok_or(LongOperationError::RevisionConflict)?
            };
            if outcome_revision != expected {
                return Err(LongOperationError::RevisionConflict);
            }
        }
        let mut record = self.active.take().ok_or(LongOperationError::InvalidToken)?;
        record.state = state;
        record.outcome_revision = Some(outcome_revision);
        record.cancellable = false;
        record.error = error;
        self.completed.push_back(record);
        while self.completed.len() > self.completed_limit {
            self.completed.pop_front();
        }
        Ok(())
    }

    /// Admit a refresh-all start. `ordinary_id_in_use_under_owner_lock` is the
    /// mandatory future bridge to the normal mutation replay ledger; callers
    /// must obtain it while holding the same composite owner lock across this
    /// call so an ID cannot race into two operation families.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        instance_id: &str,
        operation_id: &str,
        digest: MutationDigest,
        expected_revision: Option<u64>,
        current_revision: u64,
        total: usize,
        ordinary_id_in_use_under_owner_lock: bool,
    ) -> Result<StartOutcome, LongOperationError> {
        self.require_instance(instance_id)?;
        if !Self::valid_operation_id(operation_id) {
            return Err(LongOperationError::InvalidOperationId);
        }
        if current_revision > MAX_REVISION || total > MAX_REFRESH_ALL_SUBSCRIPTIONS {
            return Err(LongOperationError::InvalidBounds);
        }
        if ordinary_id_in_use_under_owner_lock {
            return Err(LongOperationError::OperationConflict);
        }
        if let Some(record) = self.record_by_id(operation_id) {
            if record.digest != digest {
                return Err(LongOperationError::OperationConflict);
            }
            return Ok(StartOutcome::Replay(record.token));
        }
        if expected_revision.is_some_and(|expected| expected != current_revision) {
            return Err(LongOperationError::RevisionConflict);
        }
        if self.active.is_some() {
            return Err(LongOperationError::Busy);
        }
        let token = LongOperationToken(self.next_token);
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or(LongOperationError::TokenExhausted)?;
        self.active = Some(OperationRecord {
            token,
            operation_id: operation_id.to_owned(),
            digest,
            state: LongOperationState::Queued,
            base_revision: current_revision,
            outcome_revision: None,
            completed: 0,
            total,
            cancel_requested: false,
            cancellable: true,
            error: None,
        });
        Ok(StartOutcome::Started(token))
    }

    #[must_use]
    pub fn has_operation_id(&self, operation_id: &str) -> bool {
        self.record_by_id(operation_id).is_some()
    }

    pub fn projection(
        &self,
        instance_id: &str,
        operation_id: &str,
    ) -> Result<LongOperationProjection<'_>, LongOperationError> {
        self.require_instance(instance_id)?;
        self.record_by_id(operation_id)
            .map(|record| record.projection(&self.instance_id))
            .ok_or(LongOperationError::NotFound)
    }

    /// Start provider work unless an accepted queued cancellation already won.
    /// The check and state transition must occur under the same registry lock.
    pub fn begin(
        &mut self,
        token: LongOperationToken,
        current_revision: u64,
    ) -> Result<BeginOperation, LongOperationError> {
        let cancelled = {
            let record = self.active_mut(token)?;
            if record.state != LongOperationState::Queued || !record.cancellable {
                return Err(LongOperationError::InvalidState);
            }
            if record.cancel_requested {
                true
            } else {
                record.state = LongOperationState::Running;
                false
            }
        };
        if cancelled {
            self.finish_active(token, LongOperationState::Cancelled, current_revision, None)?;
            Ok(BeginOperation::Cancelled)
        } else {
            Ok(BeginOperation::Running)
        }
    }

    pub fn advance(
        &mut self,
        token: LongOperationToken,
        completed: usize,
    ) -> Result<(), LongOperationError> {
        let record = self.active_mut(token)?;
        if record.state != LongOperationState::Running
            || !record.cancellable
            || completed < record.completed
            || completed > record.total
        {
            return Err(LongOperationError::InvalidState);
        }
        record.completed = completed;
        Ok(())
    }

    pub fn request_cancel(
        &mut self,
        instance_id: &str,
        operation_id: &str,
    ) -> Result<CancelOutcome, LongOperationError> {
        self.require_instance(instance_id)?;
        if let Some(record) = self
            .active
            .as_mut()
            .filter(|record| record.operation_id == operation_id)
        {
            let accepted = record.cancellable;
            if accepted {
                record.cancel_requested = true;
            }
            return Ok(CancelOutcome {
                accepted,
                token: record.token,
            });
        }
        self.completed
            .iter()
            .rev()
            .find(|record| record.operation_id == operation_id)
            .map(|record| CancelOutcome {
                accepted: false,
                token: record.token,
            })
            .ok_or(LongOperationError::NotFound)
    }

    /// Atomically close cancellation before final owner admission/commit. If
    /// cancellation already won, the operation becomes terminal with no store
    /// effect. Otherwise later cancellation is rejected.
    pub fn fence_commit(
        &mut self,
        token: LongOperationToken,
        current_revision: u64,
    ) -> Result<CommitFence, LongOperationError> {
        let cancelled = {
            let record = self.active_mut(token)?;
            if record.state != LongOperationState::Running || !record.cancellable {
                return Err(LongOperationError::InvalidState);
            }
            if record.completed != record.total {
                return Err(LongOperationError::InvalidState);
            }
            if record.cancel_requested {
                true
            } else {
                record.cancellable = false;
                false
            }
        };
        if cancelled {
            self.finish_active(token, LongOperationState::Cancelled, current_revision, None)?;
            Ok(CommitFence::Cancelled)
        } else {
            Ok(CommitFence::Ready)
        }
    }

    pub fn finish_success(
        &mut self,
        token: LongOperationToken,
        outcome_revision: u64,
    ) -> Result<(), LongOperationError> {
        let record = self.active_mut(token)?;
        if record.state != LongOperationState::Running
            || record.cancellable
            || record.completed != record.total
        {
            return Err(LongOperationError::InvalidState);
        }
        self.finish_active(token, LongOperationState::Succeeded, outcome_revision, None)
    }

    pub fn finish_failure(
        &mut self,
        token: LongOperationToken,
        outcome_revision: u64,
        error: StableErrorCode,
    ) -> Result<LongOperationState, LongOperationError> {
        let record = self.active_mut(token)?;
        if record.state.terminal() {
            return Err(LongOperationError::InvalidState);
        }
        if record.cancel_requested && record.cancellable {
            self.finish_active(token, LongOperationState::Cancelled, outcome_revision, None)?;
            Ok(LongOperationState::Cancelled)
        } else {
            self.finish_active(
                token,
                LongOperationState::Failed,
                outcome_revision,
                Some(error),
            )?;
            Ok(LongOperationState::Failed)
        }
    }

    pub fn finish_cancelled(
        &mut self,
        token: LongOperationToken,
        outcome_revision: u64,
    ) -> Result<(), LongOperationError> {
        let record = self.active_mut(token)?;
        if record.state.terminal() || !record.cancellable || !record.cancel_requested {
            return Err(LongOperationError::InvalidState);
        }
        self.finish_active(token, LongOperationState::Cancelled, outcome_revision, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTANCE: &str = "test-instance";

    fn digest(value: u8) -> MutationDigest {
        MutationDigest::new([value; 32])
    }

    #[test]
    fn lifecycle_progress_and_success_are_strict_and_bounded() {
        let mut registry = LongOperationRegistry::default();
        let token = registry
            .start(INSTANCE, "batch-1", digest(1), Some(7), 7, 3, false)
            .unwrap()
            .token();
        let queued = registry.projection(INSTANCE, "batch-1").unwrap();
        assert_eq!(queued.state, LongOperationState::Queued);
        assert_eq!(queued.progress.total, 3);
        assert_eq!(registry.begin(token, 7), Ok(BeginOperation::Running));
        registry.advance(token, 1).unwrap();
        assert_eq!(
            registry
                .projection(INSTANCE, "batch-1")
                .unwrap()
                .progress
                .completed,
            1
        );
        assert_eq!(
            registry.advance(token, 0),
            Err(LongOperationError::InvalidState)
        );
        registry.advance(token, 3).unwrap();
        assert_eq!(registry.fence_commit(token, 7), Ok(CommitFence::Ready));
        assert!(
            !registry
                .request_cancel(INSTANCE, "batch-1")
                .unwrap()
                .accepted
        );
        assert!(
            registry
                .projection(INSTANCE, "batch-1")
                .unwrap()
                .result_value()
                .is_ok()
        );
        registry.finish_success(token, 8).unwrap();
        let complete = registry.projection(INSTANCE, "batch-1").unwrap();
        assert_eq!(complete.state, LongOperationState::Succeeded);
        assert_eq!(complete.outcome_revision, Some(8));
    }

    #[test]
    fn exact_retries_replay_every_state_and_conflicting_ids_fail() {
        let mut registry = LongOperationRegistry::default();
        let started = registry
            .start(INSTANCE, "batch-1", digest(1), Some(0), 0, 2, false)
            .unwrap();
        assert!(matches!(started, StartOutcome::Started(_)));
        assert!(matches!(
            registry.start(INSTANCE, "batch-1", digest(1), Some(0), 99, 2, false),
            Ok(StartOutcome::Replay(_))
        ));
        assert_eq!(
            registry.start(INSTANCE, "batch-1", digest(2), Some(0), 0, 2, false),
            Err(LongOperationError::OperationConflict)
        );
        assert_eq!(
            registry.start(INSTANCE, "normal-id", digest(2), None, 0, 1, true),
            Err(LongOperationError::OperationConflict)
        );
        assert_eq!(
            registry.start(INSTANCE, "batch-2", digest(2), Some(1), 0, 1, false),
            Err(LongOperationError::RevisionConflict)
        );
        assert_eq!(
            registry.start(INSTANCE, "batch-2", digest(2), None, 0, 1, false),
            Err(LongOperationError::Busy)
        );
    }

    #[test]
    fn cancellation_wins_before_commit_and_loses_after_fence() {
        let mut registry = LongOperationRegistry::default();
        let token = registry
            .start(INSTANCE, "cancel-1", digest(1), None, 4, 1, false)
            .unwrap()
            .token();
        assert_eq!(registry.begin(token, 4), Ok(BeginOperation::Running));
        assert!(
            registry
                .request_cancel(INSTANCE, "cancel-1")
                .unwrap()
                .accepted
        );
        assert!(
            registry
                .request_cancel(INSTANCE, "cancel-1")
                .unwrap()
                .accepted
        );
        registry.advance(token, 1).unwrap();
        assert_eq!(registry.fence_commit(token, 4), Ok(CommitFence::Cancelled));
        let cancelled = registry.projection(INSTANCE, "cancel-1").unwrap();
        assert_eq!(cancelled.state, LongOperationState::Cancelled);
        assert!(cancelled.cancel_requested);

        let token = registry
            .start(INSTANCE, "cancel-2", digest(2), None, 4, 0, false)
            .unwrap()
            .token();
        assert_eq!(registry.begin(token, 4), Ok(BeginOperation::Running));
        assert_eq!(registry.fence_commit(token, 4), Ok(CommitFence::Ready));
        assert!(
            !registry
                .request_cancel(INSTANCE, "cancel-2")
                .unwrap()
                .accepted
        );
        registry.finish_success(token, 4).unwrap();
    }

    #[test]
    fn queued_cancel_prevents_first_fetch_and_accepted_cancel_beats_failure() {
        let mut registry = LongOperationRegistry::default();
        let token = registry
            .start(INSTANCE, "queued-cancel", digest(1), None, 2, 3, false)
            .unwrap()
            .token();
        assert!(
            registry
                .request_cancel(INSTANCE, "queued-cancel")
                .unwrap()
                .accepted
        );
        assert_eq!(registry.begin(token, 2), Ok(BeginOperation::Cancelled));
        assert_eq!(
            registry
                .projection(INSTANCE, "queued-cancel")
                .unwrap()
                .state,
            LongOperationState::Cancelled
        );

        let token = registry
            .start(INSTANCE, "fetch-cancel", digest(2), None, 2, 3, false)
            .unwrap()
            .token();
        assert_eq!(registry.begin(token, 2), Ok(BeginOperation::Running));
        assert!(
            registry
                .request_cancel(INSTANCE, "fetch-cancel")
                .unwrap()
                .accepted
        );
        assert_eq!(
            registry
                .finish_failure(token, 2, StableErrorCode::CoreRejected)
                .unwrap(),
            LongOperationState::Cancelled
        );
        let projection = registry.projection(INSTANCE, "fetch-cancel").unwrap();
        assert_eq!(projection.state, LongOperationState::Cancelled);
        assert_eq!(projection.error, None);
    }

    #[test]
    fn failures_eviction_and_restart_are_per_instance_and_safe() {
        let mut registry = LongOperationRegistry::new(INSTANCE, 1).unwrap();
        let token = registry
            .start(INSTANCE, "failed-1", digest(1), None, 3, 1, false)
            .unwrap()
            .token();
        assert_eq!(registry.begin(token, 3), Ok(BeginOperation::Running));
        registry
            .finish_failure(token, 3, StableErrorCode::CoreRejected)
            .unwrap();
        let failed = registry.projection(INSTANCE, "failed-1").unwrap();
        assert_eq!(failed.state, LongOperationState::Failed);
        assert_eq!(failed.error, Some(StableErrorCode::CoreRejected));

        let token = registry
            .start(INSTANCE, "done-2", digest(2), None, 3, 0, false)
            .unwrap()
            .token();
        assert_eq!(registry.begin(token, 3), Ok(BeginOperation::Running));
        registry.fence_commit(token, 3).unwrap();
        registry.finish_success(token, 3).unwrap();
        assert_eq!(
            registry.projection(INSTANCE, "failed-1").err().unwrap(),
            LongOperationError::NotFound
        );
        assert!(registry.has_operation_id("done-2"));

        let mut restarted = LongOperationRegistry::new("next-instance", 128).unwrap();
        restarted
            .start("next-instance", "done-2", digest(9), None, 3, 0, false)
            .unwrap();
        assert_eq!(
            restarted.projection(INSTANCE, "done-2").err().unwrap(),
            LongOperationError::InstanceMismatch
        );
        assert_eq!(
            restarted.request_cancel(INSTANCE, "done-2").err().unwrap(),
            LongOperationError::InstanceMismatch
        );
        assert!(restarted.projection("next-instance", "done-2").is_ok());
        let private = "private.example/password";
        let error = registry.projection(INSTANCE, private).err().unwrap();
        let public = format!("{error:?} {error}");
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
    }

    #[test]
    fn invalid_bounds_ids_progress_tokens_and_states_fail_closed() {
        assert!(LongOperationRegistry::new(INSTANCE, 0).is_err());
        assert!(LongOperationRegistry::new(INSTANCE, MAX_COMPLETED_OPERATION_LIMIT + 1).is_err());
        let mut registry = LongOperationRegistry::default();
        for id in ["", "has space", &"x".repeat(65)] {
            assert_eq!(
                registry.start(INSTANCE, id, digest(1), None, 0, 1, false),
                Err(LongOperationError::InvalidOperationId)
            );
        }
        assert_eq!(
            registry.start(
                INSTANCE,
                "too-many",
                digest(1),
                None,
                0,
                MAX_REFRESH_ALL_SUBSCRIPTIONS + 1,
                false,
            ),
            Err(LongOperationError::InvalidBounds)
        );
        let token = registry
            .start(INSTANCE, "valid", digest(1), None, 0, 1, false)
            .unwrap()
            .token();
        assert_eq!(
            registry.advance(token, 1),
            Err(LongOperationError::InvalidState)
        );
        assert_eq!(registry.begin(token, 0), Ok(BeginOperation::Running));
        assert_eq!(
            registry.advance(token, 2),
            Err(LongOperationError::InvalidState)
        );
        assert_eq!(
            registry.fence_commit(token, 0),
            Err(LongOperationError::InvalidState)
        );
        assert_eq!(
            registry.begin(LongOperationToken(u64::MAX), 0),
            Err(LongOperationError::InvalidToken)
        );
        assert_eq!(
            registry.finish_failure(
                LongOperationToken(u64::MAX),
                0,
                StableErrorCode::InternalError,
            ),
            Err(LongOperationError::InvalidToken)
        );
        assert!(registry.has_operation_id("valid"));
    }

    #[test]
    fn terminal_revision_and_token_space_never_move_backwards_or_wrap() {
        let mut registry = LongOperationRegistry::default();
        let token = registry
            .start(INSTANCE, "revision", digest(1), None, 7, 1, false)
            .unwrap()
            .token();
        registry.begin(token, 7).unwrap();
        registry.advance(token, 1).unwrap();
        registry.fence_commit(token, 7).unwrap();
        assert_eq!(
            registry.finish_success(token, 7),
            Err(LongOperationError::RevisionConflict)
        );
        assert!(registry.has_operation_id("revision"));
        registry.finish_success(token, 8).unwrap();

        let mut exhausted = LongOperationRegistry {
            next_token: u64::MAX,
            ..LongOperationRegistry::default()
        };
        assert_eq!(
            exhausted.start(INSTANCE, "exhausted", digest(2), None, 0, 1, false),
            Err(LongOperationError::TokenExhausted)
        );
        assert!(!exhausted.has_operation_id("exhausted"));
    }
}
