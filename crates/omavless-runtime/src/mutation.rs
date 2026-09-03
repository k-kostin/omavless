// SPDX-License-Identifier: MIT

//! Bounded mutation serialization primitives for the future R5 owner.
//!
//! This module stores only scheduling metadata, a fixed digest, and stable
//! public outcomes. It never stores request bodies, credentials, profile IDs,
//! or method-specific payloads and performs no VPN lifecycle operation.

use omavless_control_protocol::{MAX_ID_LENGTH, MAX_REVISION, StableErrorCode};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fmt;

pub const DEFAULT_QUEUE_LIMIT: usize = 64;
pub const MAX_QUEUE_LIMIT: usize = 256;
pub const DEFAULT_RESULT_CACHE_LIMIT: usize = 128;
pub const MAX_RESULT_CACHE_LIMIT: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationKind {
    Disconnect,
    Other,
}

struct OperationId(String);

impl OperationId {
    fn parse(value: &str) -> Result<Self, CoordinatorError> {
        if value.is_empty()
            || value.len() > MAX_ID_LENGTH
            || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        {
            return Err(CoordinatorError::InvalidOperationId);
        }
        Ok(Self(value.to_owned()))
    }
}

/// Caller-computed digest of canonical method + params. It is fixed-size and
/// deliberately has no formatting implementation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MutationDigest([u8; 32]);

impl MutationDigest {
    #[must_use]
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn from_semantic_bytes(value: &[u8]) -> Self {
        Self(Sha256::digest(value).into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorError {
    InvalidBounds,
    InvalidOperationId,
    RevisionConflict,
    OperationConflict,
    Busy,
    InvalidToken,
    RevisionExhausted,
}

impl CoordinatorError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::RevisionConflict | Self::OperationConflict => StableErrorCode::Conflict,
            Self::Busy => StableErrorCode::Busy,
            Self::InvalidBounds | Self::InvalidOperationId | Self::InvalidToken => {
                StableErrorCode::InvalidArgument
            }
            Self::RevisionExhausted => StableErrorCode::InternalError,
        }
    }
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBounds => "Mutation coordinator bounds are invalid",
            Self::InvalidOperationId => "Mutation operation ID is invalid",
            Self::RevisionConflict => "Mutation revision conflicts with current state",
            Self::OperationConflict => "Mutation operation ID was reused for different input",
            Self::Busy => "Mutation coordinator is busy",
            Self::InvalidToken => "Mutation token is invalid",
            Self::RevisionExhausted => "Mutation revision is exhausted",
        })
    }
}

impl std::error::Error for CoordinatorError {}

pub struct MutationRequest {
    kind: MutationKind,
    operation_id: Option<OperationId>,
    expected_revision: Option<u64>,
    digest: MutationDigest,
}

impl MutationRequest {
    pub fn new(
        kind: MutationKind,
        operation_id: Option<&str>,
        expected_revision: Option<u64>,
        digest: MutationDigest,
    ) -> Result<Self, CoordinatorError> {
        if expected_revision.is_some_and(|revision| revision > MAX_REVISION) {
            return Err(CoordinatorError::RevisionConflict);
        }
        Ok(Self {
            kind,
            operation_id: operation_id.map(OperationId::parse).transpose()?,
            expected_revision,
            digest,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutationToken(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachedOutcome {
    pub revision: u64,
    pub error: Option<StableErrorCode>,
}

impl CachedOutcome {
    #[must_use]
    pub const fn succeeded(self) -> bool {
        self.error.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitOutcome {
    Queued { token: MutationToken },
    Replay(CachedOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveMutation {
    pub token: MutationToken,
    pub kind: MutationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginOutcome {
    Started(ActiveMutation),
    Rejected {
        token: MutationToken,
        outcome: CachedOutcome,
    },
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationResult {
    Success,
    /// Durable semantic state changed, but a bounded follow-up failed. Cache
    /// the stable error and advance revision so clients cannot act on the old
    /// precondition.
    CommittedFailure(StableErrorCode),
    NoChange,
    Failure(StableErrorCode),
}

struct QueuedMutation {
    token: MutationToken,
    request: MutationRequest,
}

struct CachedOperation {
    operation_id: OperationId,
    digest: MutationDigest,
    outcome: CachedOutcome,
}

pub struct MutationCoordinator {
    revision: u64,
    next_token: u64,
    queue_limit: usize,
    result_cache_limit: usize,
    queue: VecDeque<QueuedMutation>,
    active: Option<QueuedMutation>,
    result_cache: VecDeque<CachedOperation>,
}

impl Default for MutationCoordinator {
    fn default() -> Self {
        Self::with_limits(DEFAULT_QUEUE_LIMIT, DEFAULT_RESULT_CACHE_LIMIT)
            .expect("default mutation bounds are valid")
    }
}

impl MutationCoordinator {
    pub fn with_limits(
        queue_limit: usize,
        result_cache_limit: usize,
    ) -> Result<Self, CoordinatorError> {
        if queue_limit == 0
            || queue_limit > MAX_QUEUE_LIMIT
            || result_cache_limit == 0
            || result_cache_limit > MAX_RESULT_CACHE_LIMIT
        {
            return Err(CoordinatorError::InvalidBounds);
        }
        Ok(Self {
            revision: 0,
            next_token: 1,
            queue_limit,
            result_cache_limit,
            queue: VecDeque::new(),
            active: None,
            result_cache: VecDeque::new(),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub const fn active(&self) -> bool {
        self.active.is_some()
    }

    fn matching_operation(
        &self,
        operation_id: &OperationId,
    ) -> Option<(&MutationDigest, Option<CachedOutcome>)> {
        if let Some(cached) = self
            .result_cache
            .iter()
            .rev()
            .find(|cached| cached.operation_id.0 == operation_id.0)
        {
            return Some((&cached.digest, Some(cached.outcome)));
        }
        if let Some(active) = self.active.as_ref().filter(|active| {
            active
                .request
                .operation_id
                .as_ref()
                .is_some_and(|active_id| active_id.0 == operation_id.0)
        }) {
            return Some((&active.request.digest, None));
        }
        self.queue
            .iter()
            .find(|queued| {
                queued
                    .request
                    .operation_id
                    .as_ref()
                    .is_some_and(|queued_id| queued_id.0 == operation_id.0)
            })
            .map(|queued| (&queued.request.digest, None))
    }

    pub fn submit(&mut self, request: MutationRequest) -> Result<SubmitOutcome, CoordinatorError> {
        // Operation replay is checked before current-revision preconditions.
        // An exact retry necessarily carries the revision from the original
        // request and must recover its cached result after that mutation has
        // advanced the daemon revision. Different input under the same ID is
        // still a conflict, regardless of revision.
        if let Some(operation_id) = &request.operation_id
            && let Some((digest, cached)) = self.matching_operation(operation_id)
        {
            if digest != &request.digest {
                return Err(CoordinatorError::OperationConflict);
            }
            return cached.map_or(Err(CoordinatorError::Busy), |outcome| {
                Ok(SubmitOutcome::Replay(outcome))
            });
        }
        if request
            .expected_revision
            .is_some_and(|expected| expected != self.revision)
        {
            return Err(CoordinatorError::RevisionConflict);
        }
        if self.queue.len() >= self.queue_limit {
            return Err(CoordinatorError::Busy);
        }
        let token = MutationToken(self.next_token);
        self.next_token = self.next_token.checked_add(1).unwrap_or(1);
        let queued = QueuedMutation { token, request };
        if queued.request.kind == MutationKind::Disconnect {
            let position = self
                .queue
                .iter()
                .position(|pending| pending.request.kind != MutationKind::Disconnect)
                .unwrap_or(self.queue.len());
            self.queue.insert(position, queued);
        } else {
            self.queue.push_back(queued);
        }
        Ok(SubmitOutcome::Queued { token })
    }

    fn cache(
        &mut self,
        operation_id: Option<OperationId>,
        digest: MutationDigest,
        outcome: CachedOutcome,
    ) {
        let Some(operation_id) = operation_id else {
            return;
        };
        self.result_cache.push_back(CachedOperation {
            operation_id,
            digest,
            outcome,
        });
        while self.result_cache.len() > self.result_cache_limit {
            self.result_cache.pop_front();
        }
    }

    pub fn begin_next(&mut self) -> Result<BeginOutcome, CoordinatorError> {
        if self.active.is_some() {
            return Err(CoordinatorError::Busy);
        }
        let Some(queued) = self.queue.pop_front() else {
            return Ok(BeginOutcome::Empty);
        };
        if self.revision == MAX_REVISION {
            let outcome = CachedOutcome {
                revision: self.revision,
                error: Some(StableErrorCode::InternalError),
            };
            let token = queued.token;
            self.cache(queued.request.operation_id, queued.request.digest, outcome);
            return Ok(BeginOutcome::Rejected { token, outcome });
        }
        if queued
            .request
            .expected_revision
            .is_some_and(|expected| expected != self.revision)
        {
            let outcome = CachedOutcome {
                revision: self.revision,
                error: Some(StableErrorCode::Conflict),
            };
            let token = queued.token;
            self.cache(queued.request.operation_id, queued.request.digest, outcome);
            return Ok(BeginOutcome::Rejected { token, outcome });
        }
        let active = ActiveMutation {
            token: queued.token,
            kind: queued.request.kind,
        };
        self.active = Some(queued);
        Ok(BeginOutcome::Started(active))
    }

    pub fn finish(
        &mut self,
        token: MutationToken,
        result: MutationResult,
    ) -> Result<CachedOutcome, CoordinatorError> {
        if self.active.as_ref().map(|active| active.token) != Some(token) {
            return Err(CoordinatorError::InvalidToken);
        }
        if matches!(
            result,
            MutationResult::Success | MutationResult::CommittedFailure(_)
        ) && self.revision == MAX_REVISION
        {
            return Err(CoordinatorError::RevisionExhausted);
        }
        let active = self.active.take().ok_or(CoordinatorError::InvalidToken)?;
        let error = match result {
            MutationResult::Success => {
                self.revision += 1;
                None
            }
            MutationResult::CommittedFailure(error) => {
                self.revision += 1;
                Some(error)
            }
            MutationResult::NoChange => None,
            MutationResult::Failure(error) => Some(error),
        };
        let outcome = CachedOutcome {
            revision: self.revision,
            error,
        };
        self.cache(active.request.operation_id, active.request.digest, outcome);
        Ok(outcome)
    }

    /// Retires the active slot without changing revision or recording a replay
    /// result. This is only for failures that happen before the caller can
    /// perform any externally observable mutation. In particular, a busy
    /// owner-exclusion lock must remain retryable under the same operation ID.
    pub(crate) fn abort_active_uncached(
        &mut self,
        token: MutationToken,
    ) -> Result<(), CoordinatorError> {
        if self.active.as_ref().map(|active| active.token) != Some(token) {
            return Err(CoordinatorError::InvalidToken);
        }
        self.active.take().ok_or(CoordinatorError::InvalidToken)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(value: u8) -> MutationDigest {
        MutationDigest::new([value; 32])
    }

    #[test]
    fn semantic_digest_is_deterministic_and_not_formattable() {
        assert!(
            MutationDigest::from_semantic_bytes(b"connection.disconnect")
                == MutationDigest::from_semantic_bytes(b"connection.disconnect")
        );
        assert!(
            MutationDigest::from_semantic_bytes(b"connection.disconnect")
                != MutationDigest::from_semantic_bytes(b"connection.connect")
        );
    }

    fn request(
        kind: MutationKind,
        operation_id: Option<&str>,
        expected_revision: Option<u64>,
        digest_value: u8,
    ) -> MutationRequest {
        MutationRequest::new(kind, operation_id, expected_revision, digest(digest_value)).unwrap()
    }

    fn token(outcome: SubmitOutcome) -> MutationToken {
        match outcome {
            SubmitOutcome::Queued { token } => token,
            SubmitOutcome::Replay(_) => panic!("unexpected replay"),
        }
    }

    #[test]
    fn one_active_mutation_and_urgent_disconnect_order_are_enforced() {
        let mut coordinator = MutationCoordinator::default();
        let first = token(
            coordinator
                .submit(request(MutationKind::Other, Some("one"), Some(0), 1))
                .unwrap(),
        );
        assert_eq!(
            coordinator.begin_next().unwrap(),
            BeginOutcome::Started(ActiveMutation {
                token: first,
                kind: MutationKind::Other
            })
        );
        let second = token(
            coordinator
                .submit(request(MutationKind::Other, Some("two"), None, 2))
                .unwrap(),
        );
        let disconnect = token(
            coordinator
                .submit(request(
                    MutationKind::Disconnect,
                    Some("disconnect"),
                    None,
                    3,
                ))
                .unwrap(),
        );
        assert_eq!(coordinator.begin_next(), Err(CoordinatorError::Busy));
        assert!(
            coordinator
                .finish(first, MutationResult::Success)
                .unwrap()
                .succeeded()
        );
        assert_eq!(coordinator.revision(), 1);
        assert_eq!(
            coordinator.begin_next().unwrap(),
            BeginOutcome::Started(ActiveMutation {
                token: disconnect,
                kind: MutationKind::Disconnect
            })
        );
        coordinator
            .finish(disconnect, MutationResult::Success)
            .unwrap();
        assert_eq!(
            coordinator.begin_next().unwrap(),
            BeginOutcome::Started(ActiveMutation {
                token: second,
                kind: MutationKind::Other
            })
        );
    }

    #[test]
    fn queued_expected_revision_is_rechecked_immediately_before_side_effects() {
        let mut coordinator = MutationCoordinator::default();
        let first = token(
            coordinator
                .submit(request(MutationKind::Other, Some("one"), Some(0), 1))
                .unwrap(),
        );
        let stale = token(
            coordinator
                .submit(request(MutationKind::Other, Some("stale"), Some(0), 2))
                .unwrap(),
        );
        let BeginOutcome::Started(active) = coordinator.begin_next().unwrap() else {
            panic!("first mutation did not start");
        };
        assert_eq!(active.token, first);
        coordinator.finish(first, MutationResult::Success).unwrap();
        assert_eq!(
            coordinator.begin_next().unwrap(),
            BeginOutcome::Rejected {
                token: stale,
                outcome: CachedOutcome {
                    revision: 1,
                    error: Some(StableErrorCode::Conflict)
                }
            }
        );
    }

    #[test]
    fn operation_id_replays_same_digest_and_conflicts_on_different_input() {
        let mut coordinator = MutationCoordinator::default();
        let queued = coordinator
            .submit(request(MutationKind::Other, Some("retry-safe"), None, 7))
            .unwrap();
        assert_eq!(
            coordinator.submit(request(MutationKind::Other, Some("retry-safe"), None, 7)),
            Err(CoordinatorError::Busy)
        );
        let active = coordinator.begin_next().unwrap();
        let BeginOutcome::Started(active) = active else {
            panic!("mutation did not start");
        };
        let completed = coordinator
            .finish(active.token, MutationResult::Success)
            .unwrap();
        assert_eq!(
            queued,
            SubmitOutcome::Queued {
                token: active.token
            }
        );
        assert_eq!(
            coordinator
                .submit(request(MutationKind::Other, Some("retry-safe"), None, 7))
                .unwrap(),
            SubmitOutcome::Replay(completed)
        );
        assert_eq!(
            coordinator.submit(request(MutationKind::Other, Some("retry-safe"), None, 8)),
            Err(CoordinatorError::OperationConflict)
        );
    }

    #[test]
    fn failed_mutations_do_not_increment_revision_and_are_replayed() {
        let mut coordinator = MutationCoordinator::default();
        let token = token(
            coordinator
                .submit(request(MutationKind::Other, Some("failed"), None, 9))
                .unwrap(),
        );
        let BeginOutcome::Started(active) = coordinator.begin_next().unwrap() else {
            panic!("mutation did not start");
        };
        assert_eq!(active.token, token);
        let outcome = coordinator
            .finish(
                token,
                MutationResult::Failure(StableErrorCode::TransitionFailedRestored),
            )
            .unwrap();
        assert_eq!(outcome.revision, 0);
        assert_eq!(
            outcome.error,
            Some(StableErrorCode::TransitionFailedRestored)
        );
        assert_eq!(
            coordinator
                .submit(request(MutationKind::Other, Some("failed"), None, 9))
                .unwrap(),
            SubmitOutcome::Replay(outcome)
        );
    }

    #[test]
    fn committed_failure_advances_revision_and_replays_the_stable_error() {
        let mut coordinator = MutationCoordinator::default();
        let token = token(
            coordinator
                .submit(request(
                    MutationKind::Disconnect,
                    Some("committed-error"),
                    Some(0),
                    10,
                ))
                .unwrap(),
        );
        let BeginOutcome::Started(active) = coordinator.begin_next().unwrap() else {
            panic!("mutation did not start");
        };
        assert_eq!(active.token, token);
        let outcome = coordinator
            .finish(
                token,
                MutationResult::CommittedFailure(StableErrorCode::InternalError),
            )
            .unwrap();
        assert_eq!(outcome.revision, 1);
        assert_eq!(outcome.error, Some(StableErrorCode::InternalError));
        assert_eq!(coordinator.revision(), 1);
        assert_eq!(
            coordinator
                .submit(request(
                    MutationKind::Disconnect,
                    Some("committed-error"),
                    Some(0),
                    10,
                ))
                .unwrap(),
            SubmitOutcome::Replay(outcome)
        );
    }

    #[test]
    fn successful_no_op_is_cached_without_incrementing_revision() {
        let mut coordinator = MutationCoordinator::default();
        let token = token(
            coordinator
                .submit(request(MutationKind::Other, Some("no-change"), None, 6))
                .unwrap(),
        );
        let BeginOutcome::Started(active) = coordinator.begin_next().unwrap() else {
            panic!("mutation did not start");
        };
        assert_eq!(active.token, token);
        let outcome = coordinator.finish(token, MutationResult::NoChange).unwrap();
        assert!(outcome.succeeded());
        assert_eq!(outcome.revision, 0);
        assert_eq!(coordinator.revision(), 0);
        assert_eq!(
            coordinator
                .submit(request(MutationKind::Other, Some("no-change"), None, 6))
                .unwrap(),
            SubmitOutcome::Replay(outcome)
        );
    }

    #[test]
    fn pre_effect_abort_is_uncached_and_preserves_revision_and_queue() {
        let mut coordinator = MutationCoordinator::default();
        let first_request = || request(MutationKind::Other, Some("retryable"), Some(0), 6);
        let first = token(coordinator.submit(first_request()).unwrap());
        let second = token(
            coordinator
                .submit(request(MutationKind::Other, Some("next"), Some(0), 7))
                .unwrap(),
        );
        let BeginOutcome::Started(active) = coordinator.begin_next().unwrap() else {
            panic!("first mutation did not start");
        };
        assert_eq!(active.token, first);

        coordinator.abort_active_uncached(first).unwrap();

        assert!(!coordinator.active());
        assert_eq!(coordinator.revision(), 0);
        assert_eq!(coordinator.queued(), 1);
        assert_eq!(
            coordinator.begin_next().unwrap(),
            BeginOutcome::Started(ActiveMutation {
                token: second,
                kind: MutationKind::Other,
            })
        );
        coordinator
            .finish(second, MutationResult::NoChange)
            .unwrap();
        assert!(matches!(
            coordinator.submit(first_request()).unwrap(),
            SubmitOutcome::Queued { .. }
        ));
        assert_eq!(coordinator.revision(), 0);
    }

    #[test]
    fn uncached_abort_rejects_wrong_token_without_retiring_active_slot() {
        let mut coordinator = MutationCoordinator::default();
        let token = token(
            coordinator
                .submit(request(MutationKind::Other, Some("active"), None, 1))
                .unwrap(),
        );
        let BeginOutcome::Started(active) = coordinator.begin_next().unwrap() else {
            panic!("mutation did not start");
        };
        assert_eq!(active.token, token);
        assert_eq!(
            coordinator.abort_active_uncached(MutationToken(999)),
            Err(CoordinatorError::InvalidToken)
        );
        assert!(coordinator.active());
        coordinator.finish(token, MutationResult::NoChange).unwrap();
    }

    #[test]
    fn queue_and_result_cache_are_bounded() {
        let mut coordinator = MutationCoordinator::with_limits(1, 1).unwrap();
        coordinator
            .submit(request(MutationKind::Other, Some("one"), None, 1))
            .unwrap();
        assert_eq!(
            coordinator.submit(request(MutationKind::Other, Some("two"), None, 2)),
            Err(CoordinatorError::Busy)
        );
        let BeginOutcome::Started(one) = coordinator.begin_next().unwrap() else {
            panic!("first mutation did not start");
        };
        coordinator
            .finish(one.token, MutationResult::Success)
            .unwrap();
        coordinator
            .submit(request(MutationKind::Other, Some("two"), None, 2))
            .unwrap();
        let BeginOutcome::Started(two) = coordinator.begin_next().unwrap() else {
            panic!("second mutation did not start");
        };
        coordinator
            .finish(two.token, MutationResult::Success)
            .unwrap();
        assert!(matches!(
            coordinator
                .submit(request(MutationKind::Other, Some("one"), None, 1))
                .unwrap(),
            SubmitOutcome::Queued { .. }
        ));
    }

    #[test]
    fn ids_tokens_revisions_and_public_errors_fail_closed() {
        for invalid in ["", "contains space", &"x".repeat(MAX_ID_LENGTH + 1)] {
            assert!(matches!(
                MutationRequest::new(MutationKind::Other, Some(invalid), None, digest(1)),
                Err(CoordinatorError::InvalidOperationId)
            ));
        }
        let private = "private.example password=secret";
        let error = MutationRequest::new(MutationKind::Other, Some(private), None, digest(1))
            .err()
            .unwrap();
        assert!(!format!("{error:?} {error}").contains("private"));
        let mut coordinator = MutationCoordinator::default();
        assert_eq!(
            coordinator.submit(request(MutationKind::Other, None, Some(1), 1)),
            Err(CoordinatorError::RevisionConflict)
        );
        assert_eq!(
            coordinator.finish(MutationToken(999), MutationResult::Success),
            Err(CoordinatorError::InvalidToken)
        );
    }

    #[test]
    fn exhausted_revision_is_rejected_before_any_side_effect_can_begin() {
        let mut coordinator = MutationCoordinator {
            revision: MAX_REVISION,
            ..MutationCoordinator::default()
        };
        let token = token(
            coordinator
                .submit(request(MutationKind::Other, Some("last"), None, 9))
                .unwrap(),
        );
        assert_eq!(
            coordinator.begin_next().unwrap(),
            BeginOutcome::Rejected {
                token,
                outcome: CachedOutcome {
                    revision: MAX_REVISION,
                    error: Some(StableErrorCode::InternalError),
                },
            }
        );
        assert!(!coordinator.active());
    }
}
