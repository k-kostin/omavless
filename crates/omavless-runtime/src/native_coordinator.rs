// SPDX-License-Identifier: MIT

//! One offline coordinator for native connection, profile and subscription
//! transactions.
//!
//! This is the first composition boundary that gives all mutation families
//! one revision, replay cache, lifecycle executor, migration lock namespace and
//! manual-recovery barrier. It never registers itself with `RuntimeServer`;
//! the production owner is the only ownership-gated registration boundary.
//! Subscription network work uses a fixed, bounded transport and never runs
//! while the Python/Rust migration lock is held.

use crate::connection_transaction::{
    Completion, ConnectionTransactionError, ConnectionTransactionOutcome,
    ConnectionTransactionState,
};
use crate::cutover::{MigrationLock, OwnershipPhase, read_marker};
use crate::desired::DesiredPaths;
use crate::lifecycle::{ActualState, LifecycleError, LifecycleHost};
use crate::mutation::{
    BeginOutcome, CachedOutcome, CoordinatorError, ExternalWorkPreflight, MutationCoordinator,
    MutationKind, MutationRequest, MutationResult, MutationToken, SubmitOutcome,
};
use crate::mutation_protocol::MutationProtocolError;
use crate::owner::{OwnerAction, OwnerRequest};
use crate::profile_mutation::prepare_profile_mutation;
use crate::profile_mutation_protocol::parse_profile_mutation_request;
use crate::profile_transaction::{
    ProfileMutationOutcome, ProfileTransactionError, apply_transaction, mutation_identity,
    store_error,
};
use crate::subscription_mutation::{
    SubscriptionMutationCommit, SubscriptionMutationCommitError, SubscriptionRefreshCommit,
    commit_subscription_mutation, commit_subscription_refresh, read_subscription_edit_input,
    snapshot_subscription_refresh,
};
use crate::subscription_mutation_protocol::{
    SubscriptionMutationIntent, parse_subscription_mutation_request,
};
use crate::subscription_read_protocol::parse_subscription_edit_input_request;
use crate::subscription_refresh_protocol::parse_subscription_refresh_request;
use crate::subscription_transport::{SubscriptionTransport, SubscriptionTransportError};
use omavless_control_protocol::StableErrorCode;
use omavless_domain::private_store::{
    PrivateStoreError, SubscriptionEditInput, SubscriptionMutation, SubscriptionMutationContext,
    SubscriptionRefreshSnapshot,
};
use omavless_domain::subscription_feed::{PrivateSubscriptionBody, decode_subscription_feed};
use serde_json::Value;
use std::cell::RefCell;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeMutationOutcome {
    Connection(ConnectionTransactionOutcome),
    Profile(ProfileMutationOutcome),
    Subscription(SubscriptionMutationCommit),
    SubscriptionRefresh(SubscriptionRefreshCommit),
}

impl NativeMutationOutcome {
    const fn changed(self) -> bool {
        match self {
            Self::Connection(outcome) => outcome.changed,
            Self::Profile(outcome) => outcome.changed,
            Self::Subscription(outcome) => outcome.changed,
            Self::SubscriptionRefresh(_) => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionTransactionError {
    Busy,
    NotFound,
    InvalidArgument,
    Conflict,
    Transport,
    Store,
    ManualRecoveryRequired,
}

impl SubscriptionTransactionError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Busy => StableErrorCode::Busy,
            Self::NotFound => StableErrorCode::NotFound,
            Self::InvalidArgument => StableErrorCode::InvalidArgument,
            Self::Conflict => StableErrorCode::Conflict,
            Self::Transport => StableErrorCode::CoreRejected,
            Self::Store => StableErrorCode::InternalError,
            Self::ManualRecoveryRequired => StableErrorCode::ManualRecoveryRequired,
        }
    }
}

impl fmt::Display for SubscriptionTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "Another OmaVLESS operation is active",
            Self::NotFound => "Requested subscription was not found",
            Self::InvalidArgument => "Subscription mutation is not permitted",
            Self::Conflict => "Subscription mutation conflicts with current state",
            Self::Transport => "Subscription provider response could not be accepted",
            Self::Store => "Subscription store transaction failed",
            Self::ManualRecoveryRequired => "Manual recovery is required",
        })
    }
}

impl std::error::Error for SubscriptionTransactionError {}

fn subscription_lock_error(error: ConnectionTransactionError) -> SubscriptionTransactionError {
    match error {
        ConnectionTransactionError::Busy => SubscriptionTransactionError::Busy,
        _ => SubscriptionTransactionError::Store,
    }
}

fn subscription_store_error(
    error: SubscriptionMutationCommitError,
) -> SubscriptionTransactionError {
    match error {
        SubscriptionMutationCommitError::Busy => SubscriptionTransactionError::Busy,
        SubscriptionMutationCommitError::Mutation(PrivateStoreError::SubscriptionNotFound) => {
            SubscriptionTransactionError::NotFound
        }
        SubscriptionMutationCommitError::Mutation(
            PrivateStoreError::DuplicateSubscriptionUrl
            | PrivateStoreError::SubscriptionChanged
            | PrivateStoreError::ActiveSubscription,
        ) => SubscriptionTransactionError::Conflict,
        SubscriptionMutationCommitError::Mutation(
            PrivateStoreError::InvalidName | PrivateStoreError::InvalidSubscriptionUrl,
        ) => SubscriptionTransactionError::InvalidArgument,
        SubscriptionMutationCommitError::UnsafeStore
        | SubscriptionMutationCommitError::StoreIo
        | SubscriptionMutationCommitError::UnsafeLock
        | SubscriptionMutationCommitError::Mutation(_) => SubscriptionTransactionError::Store,
    }
}

fn subscription_lifecycle_error(error: LifecycleError) -> SubscriptionTransactionError {
    match error {
        LifecycleError::ManualRecoveryRequired | LifecycleError::RecoveryFailed => {
            SubscriptionTransactionError::ManualRecoveryRequired
        }
        LifecycleError::InvalidRequest
        | LifecycleError::State
        | LifecycleError::TransitionFailedRestored => SubscriptionTransactionError::Store,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTransactionError {
    Connection(ConnectionTransactionError),
    Profile(ProfileTransactionError),
    Subscription(SubscriptionTransactionError),
}

impl NativeTransactionError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Connection(error) => error.stable_code(),
            Self::Profile(error) => error.stable_code(),
            Self::Subscription(error) => error.stable_code(),
        }
    }

    const fn requires_manual_recovery(self) -> bool {
        matches!(
            self,
            Self::Connection(ConnectionTransactionError::ManualRecoveryRequired)
                | Self::Profile(ProfileTransactionError::ManualRecoveryRequired)
                | Self::Subscription(SubscriptionTransactionError::ManualRecoveryRequired)
        )
    }
}

impl fmt::Display for NativeTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(error) => error.fmt(formatter),
            Self::Profile(error) => error.fmt(formatter),
            Self::Subscription(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeTransactionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeOwnerExecution {
    Applied {
        cached: CachedOutcome,
        outcome: Result<NativeMutationOutcome, NativeTransactionError>,
    },
    UncachedPreflightFailure {
        revision: u64,
        error: NativeTransactionError,
    },
    Replay(CachedOutcome),
    Rejected(CachedOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeOwnerError {
    Protocol(MutationProtocolError),
    Coordinator(CoordinatorError),
    Subscription(SubscriptionTransactionError),
    OwnershipBusy,
    OwnershipUnavailable,
    ManualRecoveryRequired,
    Invariant,
}

impl NativeOwnerError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Protocol(error) => error.stable_code(),
            Self::Coordinator(error) => error.stable_code(),
            Self::Subscription(error) => error.stable_code(),
            Self::OwnershipBusy => StableErrorCode::Busy,
            Self::OwnershipUnavailable => StableErrorCode::CapabilityUnavailable,
            Self::ManualRecoveryRequired => StableErrorCode::ManualRecoveryRequired,
            Self::Invariant => StableErrorCode::InternalError,
        }
    }
}

impl fmt::Display for NativeOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Protocol(_) => "Native mutation request is invalid",
            Self::Coordinator(_) => "Native mutation scheduling failed",
            Self::Subscription(_) => "Native subscription refresh failed",
            Self::OwnershipBusy => "Native mutation ownership is being changed",
            Self::OwnershipUnavailable => "Native mutation ownership is unavailable",
            Self::ManualRecoveryRequired => "Native mutation requires manual recovery",
            Self::Invariant => "Native mutation coordinator invariant failed",
        })
    }
}

impl std::error::Error for NativeOwnerError {}

impl From<MutationProtocolError> for NativeOwnerError {
    fn from(value: MutationProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl From<CoordinatorError> for NativeOwnerError {
    fn from(value: CoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}

enum Admission {
    Execute(MutationToken),
    Replay(CachedOutcome),
    Rejected(CachedOutcome),
}

enum LockAdmission {
    Locked(MigrationLock),
    Uncached(NativeOwnerExecution),
}

/// Private remote target accepted for one bounded subscription fetch. It is a
/// reservation-free snapshot: the original request must be admitted again
/// after fetch before any commit, so a concurrent mutation wins by revision.
pub(crate) struct PreparedSubscriptionFetch {
    url: String,
}

impl PreparedSubscriptionFetch {
    pub(crate) fn private_url(&self) -> &str {
        &self.url
    }
}

pub(crate) enum SubscriptionFetchPreflight {
    Ready(PreparedSubscriptionFetch),
    Replay(CachedOutcome),
}

/// Private optimistic snapshot carried across one bounded provider refresh.
/// It deliberately has no formatting, cloning, or serialization boundary.
pub(crate) struct PreparedSubscriptionRefresh {
    snapshot: SubscriptionRefreshSnapshot,
}

impl PreparedSubscriptionRefresh {
    pub(crate) fn private_url(&self) -> &str {
        self.snapshot.private_url()
    }
}

pub(crate) enum SubscriptionRefreshPreflight {
    Ready(PreparedSubscriptionRefresh),
    Replay(CachedOutcome),
}

struct FetchedSubscriptionTransport(
    RefCell<Option<Result<PrivateSubscriptionBody, SubscriptionTransportError>>>,
);

impl SubscriptionTransport for FetchedSubscriptionTransport {
    fn fetch(&self, _url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
        self.0
            .borrow_mut()
            .take()
            .unwrap_or(Err(SubscriptionTransportError::Unavailable))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OwnershipFence {
    phase: OwnershipPhase,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidatePromotion {
    Pending,
    Promoted { rust_generation: u64 },
    Stale,
}

/// Socket-independent composition of the accepted native mutation
/// transactions. There is deliberately no socket constructor or registration
/// side effect in this type.
pub struct OfflineNativeCoordinator<H> {
    coordinator: MutationCoordinator,
    transaction: ConnectionTransactionState<H>,
    required_ownership: Option<OwnershipFence>,
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    #[must_use]
    pub fn new(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: crate::cutover::CutoverPaths,
        uid: u32,
    ) -> Self {
        Self {
            coordinator: MutationCoordinator::default(),
            transaction: ConnectionTransactionState::new(
                host,
                desired_paths,
                store_path,
                cutover_paths,
                uid,
            ),
            required_ownership: None,
        }
    }

    /// Construct the same coordinator with a fail-closed ownership gate.
    /// Every mutation re-reads the private marker while holding the shared
    /// Python/Rust migration lock. This does not register the coordinator with
    /// the production socket by itself.
    #[must_use]
    pub fn new_ownership_gated(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: crate::cutover::CutoverPaths,
        uid: u32,
        ownership_generation: u64,
    ) -> Self {
        let mut owner = Self::new(host, desired_paths, store_path, cutover_paths, uid);
        owner.required_ownership = Some(OwnershipFence {
            phase: OwnershipPhase::Rust,
            generation: ownership_generation,
        });
        owner
    }

    /// Construct a transition candidate pinned to one exact durable
    /// `cutoverPreparing` generation. The candidate can reconcile lifecycle
    /// state under the migration lock, but every mutation remains unavailable
    /// until [`Self::try_promote_candidate`] observes the immediate committed
    /// Rust successor.
    #[must_use]
    pub(crate) fn new_transition_candidate(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: crate::cutover::CutoverPaths,
        uid: u32,
        preparing_generation: u64,
    ) -> Self {
        let mut owner = Self::new(host, desired_paths, store_path, cutover_paths, uid);
        owner.required_ownership = Some(OwnershipFence {
            phase: OwnershipPhase::CutoverPreparing,
            generation: preparing_generation,
        });
        owner
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.coordinator.revision()
    }

    #[must_use]
    pub const fn actual(&self) -> ActualState {
        self.transaction.actual()
    }

    pub(crate) fn desired(
        &self,
    ) -> Result<crate::desired::DesiredState, ConnectionTransactionError> {
        self.transaction.desired()
    }

    pub(crate) fn store_path(&self) -> &Path {
        self.transaction.store_path()
    }

    pub(crate) const fn uid(&self) -> u32 {
        self.transaction.uid()
    }

    pub(crate) fn rust_ownership_available(&self) -> bool {
        self.required_ownership.is_some_and(|fence| {
            fence.phase == OwnershipPhase::Rust
                && self
                    .transaction
                    .ownership_available(fence.phase, fence.generation)
        })
    }

    /// Promote one already reconciled candidate without reconstructing its
    /// lifecycle host, revision, or replay state. Promotion is a pure in-memory
    /// fence update after the exact durable successor is observed under the
    /// shared lock. A rollback, later attempt, malformed marker, or generation
    /// gap makes this candidate permanently stale at the caller.
    pub(crate) fn try_promote_candidate(&mut self) -> Result<CandidatePromotion, NativeOwnerError> {
        let Some(fence) = self.required_ownership else {
            return Ok(CandidatePromotion::Stale);
        };
        if fence.phase == OwnershipPhase::Rust {
            return Ok(CandidatePromotion::Promoted {
                rust_generation: fence.generation,
            });
        }
        if fence.phase != OwnershipPhase::CutoverPreparing {
            return Ok(CandidatePromotion::Stale);
        }
        let expected_rust_generation = match fence.generation.checked_add(1) {
            Some(generation) => generation,
            None => return Ok(CandidatePromotion::Stale),
        };
        let lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                _ => NativeOwnerError::OwnershipUnavailable,
            })?;
        let marker = read_marker(self.transaction.cutover_paths(), self.transaction.uid());
        let promotion = match marker {
            Ok(marker)
                if marker.phase() == OwnershipPhase::CutoverPreparing
                    && marker.generation() == fence.generation =>
            {
                CandidatePromotion::Pending
            }
            Ok(marker)
                if marker.phase() == OwnershipPhase::Rust
                    && marker.generation() == expected_rust_generation =>
            {
                self.required_ownership = Some(OwnershipFence {
                    phase: OwnershipPhase::Rust,
                    generation: expected_rust_generation,
                });
                CandidatePromotion::Promoted {
                    rust_generation: expected_rust_generation,
                }
            }
            Ok(_) | Err(_) => CandidatePromotion::Stale,
        };
        drop(lock);
        Ok(promotion)
    }

    #[must_use]
    pub const fn host(&self) -> &H {
        self.transaction.host()
    }

    pub fn host_mut(&mut self) -> &mut H {
        self.transaction.host_mut()
    }

    /// Validate a network-backed subscription request before the caller
    /// releases the serialized owner for bounded transport work. This method
    /// performs no fetch, queue reservation, store write, or lifecycle work.
    /// The final mutation path must parse and admit the same request again.
    pub(crate) fn preflight_subscription_fetch(
        &mut self,
        request: &Value,
    ) -> Result<SubscriptionFetchPreflight, NativeOwnerError> {
        let parsed = parse_subscription_mutation_request(request)?;
        let url = parsed.remote_url().ok_or(NativeOwnerError::Protocol(
            MutationProtocolError::InvalidArgument,
        ))?;
        let scheduling = parsed.external_work_request()?;

        let lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                _ => NativeOwnerError::OwnershipUnavailable,
            })?;
        if self.required_ownership.is_some_and(|fence| {
            fence.phase != OwnershipPhase::Rust
                || !self
                    .transaction
                    .ownership_matches(fence.phase, fence.generation)
        }) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if self.transaction.blocked() {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        let preflight = self.coordinator.preflight_external_work(&scheduling)?;
        drop(lock);

        match preflight {
            ExternalWorkPreflight::Ready => Ok(SubscriptionFetchPreflight::Ready(
                PreparedSubscriptionFetch {
                    url: url.to_owned(),
                },
            )),
            ExternalWorkPreflight::Replay(outcome) => {
                Ok(SubscriptionFetchPreflight::Replay(outcome))
            }
        }
    }

    /// Capture the existing subscription's private optimistic snapshot before
    /// releasing the serialized owner for provider I/O. Cached exact retries
    /// return without re-reading the store or repeating the fetch.
    pub(crate) fn preflight_subscription_refresh(
        &mut self,
        request: &Value,
    ) -> Result<SubscriptionRefreshPreflight, NativeOwnerError> {
        let parsed = parse_subscription_refresh_request(request)?;
        let scheduling = parsed.external_work_request()?;

        let _lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                _ => NativeOwnerError::OwnershipUnavailable,
            })?;
        if self.required_ownership.is_some_and(|fence| {
            fence.phase != OwnershipPhase::Rust
                || !self
                    .transaction
                    .ownership_matches(fence.phase, fence.generation)
        }) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if self.transaction.blocked() {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        match self.coordinator.preflight_external_work(&scheduling)? {
            ExternalWorkPreflight::Replay(outcome) => {
                Ok(SubscriptionRefreshPreflight::Replay(outcome))
            }
            ExternalWorkPreflight::Ready => {
                let snapshot = snapshot_subscription_refresh(
                    self.transaction.store_path(),
                    self.transaction.uid(),
                    parsed.private_subscription_id(),
                )
                .map_err(|error| NativeOwnerError::Subscription(subscription_store_error(error)))?;
                Ok(SubscriptionRefreshPreflight::Ready(
                    PreparedSubscriptionRefresh { snapshot },
                ))
            }
        }
    }

    /// Read one explicit subscription editor payload while exact native
    /// ownership and the shared private-store lock are continuously held.
    pub(crate) fn subscription_edit_input(
        &mut self,
        request: &Value,
    ) -> Result<SubscriptionEditInput, NativeOwnerError> {
        let parsed = parse_subscription_edit_input_request(request)?;
        let _lock = self
            .transaction
            .acquire_lock()
            .map_err(|error| match error {
                ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                _ => NativeOwnerError::OwnershipUnavailable,
            })?;
        if self.required_ownership.is_none_or(|fence| {
            fence.phase != OwnershipPhase::Rust
                || !self
                    .transaction
                    .ownership_matches(fence.phase, fence.generation)
        }) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        read_subscription_edit_input(
            self.transaction.store_path(),
            self.transaction.uid(),
            parsed.private_subscription_id(),
        )
        .map_err(|error| NativeOwnerError::Subscription(subscription_store_error(error)))
    }

    pub fn reconcile_startup(
        &mut self,
    ) -> Result<ConnectionTransactionOutcome, ConnectionTransactionError> {
        self.transaction.reconcile_startup()
    }

    pub(crate) fn reconcile_startup_locked(
        &mut self,
        lock: &MigrationLock,
    ) -> Result<ConnectionTransactionOutcome, ConnectionTransactionError> {
        self.transaction.reconcile_startup_locked(lock)
    }

    fn admit(
        &mut self,
        kind: MutationKind,
        operation_id: Option<&str>,
        expected_revision: Option<u64>,
        digest: crate::mutation::MutationDigest,
    ) -> Result<Admission, NativeOwnerError> {
        if let Some(fence) = self.required_ownership {
            let lock = self
                .transaction
                .acquire_lock()
                .map_err(|error| match error {
                    ConnectionTransactionError::Busy => NativeOwnerError::OwnershipBusy,
                    _ => NativeOwnerError::OwnershipUnavailable,
                })?;
            let owned = fence.phase == OwnershipPhase::Rust
                && self
                    .transaction
                    .ownership_matches(fence.phase, fence.generation);
            drop(lock);
            if !owned {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
        }
        let scheduling = MutationRequest::new(kind, operation_id, expected_revision, digest)?;
        let token = match self.coordinator.submit(scheduling)? {
            SubmitOutcome::Queued { token } => token,
            SubmitOutcome::Replay(outcome) => return Ok(Admission::Replay(outcome)),
        };
        match self.coordinator.begin_next()? {
            BeginOutcome::Started(active) if active.token == token => Ok(Admission::Execute(token)),
            BeginOutcome::Rejected {
                token: rejected,
                outcome,
            } if rejected == token => Ok(Admission::Rejected(outcome)),
            _ => Err(NativeOwnerError::Invariant),
        }
    }

    fn preflight_lock(
        &mut self,
        token: MutationToken,
        family: fn(ConnectionTransactionError) -> NativeTransactionError,
    ) -> Result<LockAdmission, NativeOwnerError> {
        match self.transaction.acquire_lock() {
            Ok(lock) => {
                if self.required_ownership.is_some_and(|fence| {
                    fence.phase != OwnershipPhase::Rust
                        || !self
                            .transaction
                            .ownership_matches(fence.phase, fence.generation)
                }) {
                    drop(lock);
                    self.coordinator.abort_active_uncached(token)?;
                    return Err(NativeOwnerError::OwnershipUnavailable);
                }
                Ok(LockAdmission::Locked(lock))
            }
            Err(error) => {
                self.coordinator.abort_active_uncached(token)?;
                Ok(LockAdmission::Uncached(
                    NativeOwnerExecution::UncachedPreflightFailure {
                        revision: self.coordinator.revision(),
                        error: family(error),
                    },
                ))
            }
        }
    }

    fn finish(
        &mut self,
        token: MutationToken,
        outcome: Result<NativeMutationOutcome, NativeTransactionError>,
        committed_failure: bool,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        if outcome
            .as_ref()
            .is_err_and(|error| error.requires_manual_recovery())
        {
            self.transaction.block();
        }
        let result = match outcome {
            Ok(value) if value.changed() => MutationResult::Success,
            Ok(_) => MutationResult::NoChange,
            Err(error) if committed_failure => {
                MutationResult::CommittedFailure(error.stable_code())
            }
            Err(error) => MutationResult::Failure(error.stable_code()),
        };
        let cached = self.coordinator.finish(token, result)?;
        if cached.error != outcome.as_ref().err().map(|error| error.stable_code()) {
            return Err(NativeOwnerError::Invariant);
        }
        Ok(NativeOwnerExecution::Applied { cached, outcome })
    }

    fn blocked(
        &mut self,
        token: MutationToken,
        error: NativeTransactionError,
    ) -> Result<Option<NativeOwnerExecution>, NativeOwnerError> {
        if !self.transaction.blocked() {
            return Ok(None);
        }
        self.finish(token, Err(error), false).map(Some)
    }

    pub fn execute_connection(
        &mut self,
        request: OwnerRequest,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let (action, operation_id, expected_revision, digest) = request.into_parts();
        let admission = self.admit(
            action.kind(),
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match admission {
            Admission::Execute(token) => token,
            Admission::Replay(outcome) => return Ok(NativeOwnerExecution::Replay(outcome)),
            Admission::Rejected(outcome) => return Ok(NativeOwnerExecution::Rejected(outcome)),
        };
        if let Some(outcome) = self.blocked(
            token,
            NativeTransactionError::Connection(ConnectionTransactionError::ManualRecoveryRequired),
        )? {
            return Ok(outcome);
        }
        let lock = match self.preflight_lock(token, NativeTransactionError::Connection)? {
            LockAdmission::Locked(lock) => lock,
            LockAdmission::Uncached(outcome) => return Ok(outcome),
        };
        let completion = match action {
            OwnerAction::Connect { profile_id, mode } => {
                self.transaction.connect(&lock, profile_id, mode)
            }
            OwnerAction::Disconnect => self.transaction.disconnect(&lock),
            OwnerAction::SetMode { mode } => self.transaction.set_mode(&lock, mode),
        };
        match completion {
            Completion::Ordinary(outcome) => self.finish(
                token,
                outcome
                    .map(NativeMutationOutcome::Connection)
                    .map_err(NativeTransactionError::Connection),
                false,
            ),
            Completion::CommittedFailure(error) => {
                self.finish(token, Err(NativeTransactionError::Connection(error)), true)
            }
        }
    }

    pub fn execute_profile(
        &mut self,
        request: &Value,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = parse_profile_mutation_request(request)?;
        let (mutation, operation_id, expected_revision, digest) = parsed.into_parts();
        let (kind, profile_id) = mutation_identity(&mutation);
        let profile_id = profile_id.to_owned();
        let admission = self.admit(
            MutationKind::Other,
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match admission {
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
            NativeTransactionError::Profile(match error {
                ConnectionTransactionError::Busy => ProfileTransactionError::Busy,
                _ => ProfileTransactionError::Store,
            })
        })? {
            LockAdmission::Locked(lock) => lock,
            LockAdmission::Uncached(outcome) => return Ok(outcome),
        };
        let plan = prepare_profile_mutation(
            self.transaction.store_path(),
            self.transaction.uid(),
            mutation,
        )
        .map_err(store_error);
        let outcome = plan.and_then(|plan| {
            let paths = self.transaction.cutover_paths().clone();
            apply_transaction(
                self.transaction.lifecycle_mut(),
                &plan,
                kind,
                &profile_id,
                &lock,
                &paths,
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

    /// Execute one subscription mutation with injected trusted ID and time
    /// sources. The concrete transport is bounded and credential-private; no
    /// network work occurs while the shared Python/Rust migration lock is held.
    pub fn execute_subscription<T, G, N>(
        &mut self,
        request: &Value,
        transport: &T,
        mut next_record_id: G,
        now_millis: N,
    ) -> Result<NativeOwnerExecution, NativeOwnerError>
    where
        T: SubscriptionTransport,
        G: FnMut() -> String,
        N: FnOnce() -> u64,
    {
        let parsed = parse_subscription_mutation_request(request)?;
        let (intent, operation_id, expected_revision, digest) = parsed.into_parts();
        let admission = self.admit(
            MutationKind::Other,
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match admission {
            Admission::Execute(token) => token,
            Admission::Replay(outcome) => return Ok(NativeOwnerExecution::Replay(outcome)),
            Admission::Rejected(outcome) => return Ok(NativeOwnerExecution::Rejected(outcome)),
        };
        if let Some(outcome) = self.blocked(
            token,
            NativeTransactionError::Subscription(
                SubscriptionTransactionError::ManualRecoveryRequired,
            ),
        )? {
            return Ok(outcome);
        }

        let mutation_and_lock = match intent {
            SubscriptionMutationIntent::Add { name, url } => {
                // Reject an already-busy store owner before issuing a remote
                // request, then release the lease for the bounded fetch.
                let preflight = match self.preflight_lock(token, |error| {
                    NativeTransactionError::Subscription(subscription_lock_error(error))
                })? {
                    LockAdmission::Locked(lock) => lock,
                    LockAdmission::Uncached(outcome) => return Ok(outcome),
                };
                drop(preflight);
                let body = match transport.fetch(&url) {
                    Ok(body) => body,
                    Err(_error) => {
                        return self.finish(
                            token,
                            Err(NativeTransactionError::Subscription(
                                SubscriptionTransactionError::Transport,
                            )),
                            false,
                        );
                    }
                };
                let feed = match decode_subscription_feed(body) {
                    Ok(feed) => feed,
                    Err(_error) => {
                        return self.finish(
                            token,
                            Err(NativeTransactionError::Subscription(
                                SubscriptionTransactionError::Transport,
                            )),
                            false,
                        );
                    }
                };
                let subscription_id = next_record_id();
                let entries = feed.into_private_entries(&mut next_record_id);
                let lock = match self.preflight_lock(token, |error| {
                    NativeTransactionError::Subscription(subscription_lock_error(error))
                })? {
                    LockAdmission::Locked(lock) => lock,
                    LockAdmission::Uncached(outcome) => return Ok(outcome),
                };
                (
                    SubscriptionMutation::Add {
                        subscription_id,
                        name,
                        url,
                        entries,
                        updated_at: now_millis(),
                    },
                    lock,
                )
            }
            SubscriptionMutationIntent::Update {
                subscription_id,
                name,
                url,
            } => {
                let preflight = match self.preflight_lock(token, |error| {
                    NativeTransactionError::Subscription(subscription_lock_error(error))
                })? {
                    LockAdmission::Locked(lock) => lock,
                    LockAdmission::Uncached(outcome) => return Ok(outcome),
                };
                drop(preflight);
                let body = match transport.fetch(&url) {
                    Ok(body) => body,
                    Err(_error) => {
                        return self.finish(
                            token,
                            Err(NativeTransactionError::Subscription(
                                SubscriptionTransactionError::Transport,
                            )),
                            false,
                        );
                    }
                };
                let feed = match decode_subscription_feed(body) {
                    Ok(feed) => feed,
                    Err(_error) => {
                        return self.finish(
                            token,
                            Err(NativeTransactionError::Subscription(
                                SubscriptionTransactionError::Transport,
                            )),
                            false,
                        );
                    }
                };
                let entries = feed.into_private_entries(&mut next_record_id);
                let lock = match self.preflight_lock(token, |error| {
                    NativeTransactionError::Subscription(subscription_lock_error(error))
                })? {
                    LockAdmission::Locked(lock) => lock,
                    LockAdmission::Uncached(outcome) => return Ok(outcome),
                };
                (
                    SubscriptionMutation::Update {
                        subscription_id,
                        name,
                        url,
                        entries,
                        updated_at: now_millis(),
                    },
                    lock,
                )
            }
            SubscriptionMutationIntent::Delete { subscription_id } => {
                let lock = match self.preflight_lock(token, |error| {
                    NativeTransactionError::Subscription(subscription_lock_error(error))
                })? {
                    LockAdmission::Locked(lock) => lock,
                    LockAdmission::Uncached(outcome) => return Ok(outcome),
                };
                (SubscriptionMutation::Delete { subscription_id }, lock)
            }
        };

        let (mutation, _lock) = mutation_and_lock;
        let active_service = match self.transaction.lifecycle_mut().observe_active_service() {
            Ok(active) => active,
            Err(error) => {
                return self.finish(
                    token,
                    Err(NativeTransactionError::Subscription(
                        subscription_lifecycle_error(error),
                    )),
                    false,
                );
            }
        };
        let outcome = commit_subscription_mutation(
            self.transaction.store_path(),
            self.transaction.uid(),
            mutation,
            SubscriptionMutationContext { active_service },
        )
        .map_err(subscription_store_error);
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::Subscription)
                .map_err(NativeTransactionError::Subscription),
            false,
        )
    }

    /// Re-admit one request after its bounded provider fetch completed outside
    /// the serialized owner. The normal subscription transaction performs all
    /// ownership, replay, revision, lock, decode and commit checks again; the
    /// in-memory transport can be consumed at most once.
    pub(crate) fn execute_fetched_subscription<G, N>(
        &mut self,
        request: &Value,
        preflight_revision: u64,
        fetched: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
        next_record_id: G,
        now_millis: N,
    ) -> Result<NativeOwnerExecution, NativeOwnerError>
    where
        G: FnMut() -> String,
        N: FnOnce() -> u64,
    {
        let parsed = parse_subscription_mutation_request(request)?;
        let scheduling = parsed.external_work_request()?;
        match self.coordinator.preflight_external_work(&scheduling)? {
            ExternalWorkPreflight::Replay(outcome) => {
                return Ok(NativeOwnerExecution::Replay(outcome));
            }
            ExternalWorkPreflight::Ready => {}
        }
        if self.coordinator.revision() != preflight_revision {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict,
            ));
        }
        let transport = FetchedSubscriptionTransport(RefCell::new(Some(fetched)));
        self.execute_subscription(request, &transport, next_record_id, now_millis)
    }

    /// Re-admit and commit a single subscription refresh after provider I/O
    /// completed outside the serialized owner and migration lock.
    pub(crate) fn execute_fetched_subscription_refresh<G, N>(
        &mut self,
        request: &Value,
        preflight_revision: u64,
        prepared: PreparedSubscriptionRefresh,
        fetched: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
        mut next_record_id: G,
        now_millis: N,
    ) -> Result<NativeOwnerExecution, NativeOwnerError>
    where
        G: FnMut() -> String,
        N: FnOnce() -> u64,
    {
        let parsed = parse_subscription_refresh_request(request)?;
        let scheduling = parsed.external_work_request()?;
        match self.coordinator.preflight_external_work(&scheduling)? {
            ExternalWorkPreflight::Replay(outcome) => {
                return Ok(NativeOwnerExecution::Replay(outcome));
            }
            ExternalWorkPreflight::Ready => {}
        }
        if self.coordinator.revision() != preflight_revision {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict,
            ));
        }
        let (_subscription_id, operation_id, expected_revision, digest) = parsed.into_parts();
        let admission = self.admit(
            MutationKind::Other,
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match admission {
            Admission::Execute(token) => token,
            Admission::Replay(outcome) => return Ok(NativeOwnerExecution::Replay(outcome)),
            Admission::Rejected(outcome) => return Ok(NativeOwnerExecution::Rejected(outcome)),
        };
        if let Some(outcome) = self.blocked(
            token,
            NativeTransactionError::Subscription(
                SubscriptionTransactionError::ManualRecoveryRequired,
            ),
        )? {
            return Ok(outcome);
        }
        let body = match fetched {
            Ok(body) => body,
            Err(_error) => {
                return self.finish(
                    token,
                    Err(NativeTransactionError::Subscription(
                        SubscriptionTransactionError::Transport,
                    )),
                    false,
                );
            }
        };
        let feed = match decode_subscription_feed(body) {
            Ok(feed) => feed,
            Err(_error) => {
                return self.finish(
                    token,
                    Err(NativeTransactionError::Subscription(
                        SubscriptionTransactionError::Transport,
                    )),
                    false,
                );
            }
        };
        let skipped = feed.counts().skipped;
        let entries = feed.into_private_entries(&mut next_record_id);
        let lock = match self.preflight_lock(token, |error| {
            NativeTransactionError::Subscription(subscription_lock_error(error))
        })? {
            LockAdmission::Locked(lock) => lock,
            LockAdmission::Uncached(outcome) => return Ok(outcome),
        };
        let outcome = commit_subscription_refresh(
            self.transaction.store_path(),
            self.transaction.uid(),
            prepared.snapshot,
            entries,
            now_millis(),
            skipped,
        )
        .map_err(subscription_store_error);
        drop(lock);
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::SubscriptionRefresh)
                .map_err(NativeTransactionError::Subscription),
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::CutoverPaths;
    use crate::desired::{DesiredState, OwnedObservation, RoutingMode, write_desired};
    use crate::lifecycle::HostStepError;
    use crate::mutation::MutationDigest;
    use crate::subscription_transport::SubscriptionTransportError;
    use omavless_domain::subscription_feed::PrivateSubscriptionBody;
    use serde_json::json;
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    const SUBSCRIPTION: &str = "10000000-0000-4000-8000-000000000001";
    const SUBSCRIPTION_PROFILE: &str = "20000000-0000-4000-8000-000000000001";
    const SUBSCRIPTION_URL: &str = "https://provider.invalid/private-token";
    const SUBSCRIPTION_BODY: &str =
        "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Managed";

    struct FakeTransport {
        body: Option<&'static [u8]>,
        calls: Cell<usize>,
        lock_probe: Option<(CutoverPaths, u32)>,
        lock_was_free: Cell<bool>,
    }

    impl FakeTransport {
        fn success(owner: &OfflineNativeCoordinator<FakeHost>) -> Self {
            Self {
                body: Some(SUBSCRIPTION_BODY.as_bytes()),
                calls: Cell::new(0),
                lock_probe: Some((
                    owner.transaction.cutover_paths().clone(),
                    owner.transaction.uid(),
                )),
                lock_was_free: Cell::new(false),
            }
        }

        fn failure() -> Self {
            Self {
                body: None,
                calls: Cell::new(0),
                lock_probe: None,
                lock_was_free: Cell::new(false),
            }
        }
    }

    impl SubscriptionTransport for FakeTransport {
        fn fetch(&self, _url: &str) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
            self.calls.set(self.calls.get() + 1);
            if let Some((paths, uid)) = &self.lock_probe {
                let probe = MigrationLock::acquire(paths, *uid)
                    .expect("subscription fetch ran while the migration lock was held");
                self.lock_was_free.set(true);
                drop(probe);
            }
            self.body
                .map(|body| PrivateSubscriptionBody::from_bytes(body.to_vec()).unwrap())
                .ok_or(SubscriptionTransportError::Unavailable)
        }
    }

    fn empty() -> OwnedObservation {
        OwnedObservation {
            service_active: false,
            controller_ready: false,
            core_count: 0,
            tun_count: 0,
            active_profile_matches: false,
        }
    }

    fn healthy() -> OwnedObservation {
        OwnedObservation {
            service_active: true,
            controller_ready: true,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        }
    }

    struct FakeHost {
        observation: OwnedObservation,
        fail_stop: bool,
        calls: usize,
    }

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.calls += 1;
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            self.observation = healthy();
            Ok(())
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            if self.fail_stop {
                return Err(HostStepError::Stop);
            }
            self.observation = empty();
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }
    }

    fn fixture(label: &str) -> (PathBuf, PathBuf, OfflineNativeCoordinator<FakeHost>) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-native-coordinator-{label}-{}-{nonce}",
            std::process::id()
        ));
        let config = root.join("config");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for path in [&root, &config, &runtime, &state] {
            fs::create_dir_all(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let store_path = config.join("profiles.json");
        let store = json!({
            "version": 3,
            "activeId": "",
            "lastId": "",
            "profiles": [{
                "id": PROFILE,
                "name": "Example",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Example",
                "protocol": "vless",
                "favorite": false
            }],
            "subscriptions": [],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": false, "target": "last", "profileId": "", "mode": "rule"},
            "onboardingComplete": true
        });
        fs::write(&store_path, serde_json::to_vec(&store).unwrap()).unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let desired_paths = DesiredPaths::below(&state);
        write_desired(
            &desired_paths,
            uid,
            &DesiredState {
                schema_version: 1,
                generation: 0,
                connected: false,
                profile_id: String::new(),
                mode: RoutingMode::Rule,
            },
        )
        .unwrap();
        let cutover = CutoverPaths::below(&runtime, &state, uid);
        let owner = OfflineNativeCoordinator::new(
            FakeHost {
                observation: empty(),
                fail_stop: false,
                calls: 0,
            },
            desired_paths,
            &store_path,
            cutover,
            uid,
        );
        (root, store_path, owner)
    }

    fn connect(operation_id: &str, revision: u64) -> OwnerRequest {
        OwnerRequest::new(
            OwnerAction::Connect {
                profile_id: PROFILE.to_owned(),
                mode: Some(RoutingMode::Global),
            },
            Some(operation_id),
            Some(revision),
            MutationDigest::from_semantic_bytes(b"connect/example/global"),
        )
    }

    fn profile_request(method: &str, params: Value) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "request-1",
            "method": method,
            "params": params,
        })
    }

    fn subscription_add(operation_id: &str, revision: u64) -> Value {
        profile_request(
            "subscriptions.add",
            json!({
                "name": "Private source",
                "url": SUBSCRIPTION_URL,
                "operationId": operation_id,
                "expectedRevision": revision
            }),
        )
    }

    fn subscription_delete(operation_id: &str, revision: u64) -> Value {
        profile_request(
            "subscriptions.delete",
            json!({
                "subscriptionId": SUBSCRIPTION,
                "operationId": operation_id,
                "expectedRevision": revision
            }),
        )
    }

    fn subscription_refresh(operation_id: &str, revision: u64) -> Value {
        json!({
            "api": "omavless.control",
            "version": 1,
            "id": "subscription-refresh-request",
            "method": "subscriptions.refresh",
            "params": {
                "subscriptionId": SUBSCRIPTION,
                "operationId": operation_id,
                "expectedRevision": revision
            }
        })
    }

    fn applied(execution: NativeOwnerExecution) -> (CachedOutcome, NativeMutationOutcome) {
        match execution {
            NativeOwnerExecution::Applied {
                cached,
                outcome: Ok(outcome),
            } => (cached, outcome),
            _ => panic!("native mutation was not successfully applied"),
        }
    }

    #[test]
    fn connection_and_profile_share_revision_lifecycle_and_replay_owner() {
        let (root, store_path, mut owner) = fixture("shared");
        let (connected, outcome) =
            applied(owner.execute_connection(connect("connect-1", 0)).unwrap());
        assert_eq!(connected.revision, 1);
        assert!(matches!(outcome, NativeMutationOutcome::Connection(_)));
        assert_eq!(owner.actual(), ActualState::Connected);

        let rename = profile_request(
            "profiles.rename",
            json!({
                "profileId": PROFILE,
                "name": "Renamed",
                "operationId": "rename-1",
                "expectedRevision": 1
            }),
        );
        let (renamed, outcome) = applied(owner.execute_profile(&rename).unwrap());
        assert_eq!(renamed.revision, 2);
        assert!(matches!(outcome, NativeMutationOutcome::Profile(_)));
        assert_eq!(owner.actual(), ActualState::Connected);
        let store: Value = serde_json::from_slice(&fs::read(store_path).unwrap()).unwrap();
        assert_eq!(store["profiles"][0]["name"], "Renamed");
        assert_eq!(store["activeId"], PROFILE);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn operation_ids_conflict_across_mutation_families() {
        let (root, store_path, mut owner) = fixture("operation-conflict");
        applied(owner.execute_connection(connect("shared-id", 0)).unwrap());
        let before = fs::read(&store_path).unwrap();
        let calls = owner.host().calls;
        let favorite = profile_request(
            "profiles.favorite",
            json!({
                "profileId": PROFILE,
                "enabled": true,
                "operationId": "shared-id",
                "expectedRevision": 1
            }),
        );
        assert_eq!(
            owner.execute_profile(&favorite),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        );
        assert_eq!(owner.revision(), 1);
        assert_eq!(owner.host().calls, calls);
        assert_eq!(fs::read(store_path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manual_recovery_blocks_every_mutation_family() {
        let (root, store_path, mut owner) = fixture("manual-block");
        applied(owner.execute_connection(connect("connect-1", 0)).unwrap());
        owner.host_mut().fail_stop = true;
        let disconnect = OwnerRequest::new(
            OwnerAction::Disconnect,
            Some("disconnect-1"),
            Some(1),
            MutationDigest::from_semantic_bytes(b"disconnect"),
        );
        let NativeOwnerExecution::Applied {
            cached,
            outcome: Err(NativeTransactionError::Connection(error)),
        } = owner.execute_connection(disconnect).unwrap()
        else {
            panic!("disconnect did not reach the expected blocker");
        };
        assert_eq!(error, ConnectionTransactionError::ManualRecoveryRequired);
        assert_eq!(cached.revision, 1);

        let before = fs::read(&store_path).unwrap();
        let favorite = profile_request(
            "profiles.favorite",
            json!({
                "profileId": PROFILE,
                "enabled": true,
                "operationId": "favorite-1",
                "expectedRevision": 1
            }),
        );
        let NativeOwnerExecution::Applied {
            cached,
            outcome: Err(error),
        } = owner.execute_profile(&favorite).unwrap()
        else {
            panic!("profile mutation bypassed the manual-recovery barrier");
        };
        assert_eq!(cached.revision, 1);
        assert_eq!(error.stable_code(), StableErrorCode::ManualRecoveryRequired);
        assert_eq!(fs::read(store_path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_revision_is_shared_across_families_before_side_effects() {
        let (root, store_path, mut owner) = fixture("revision-conflict");
        applied(owner.execute_connection(connect("connect-1", 0)).unwrap());
        let before = fs::read(&store_path).unwrap();
        let calls = owner.host().calls;
        let favorite = profile_request(
            "profiles.favorite",
            json!({
                "profileId": PROFILE,
                "enabled": true,
                "operationId": "favorite-1",
                "expectedRevision": 0
            }),
        );
        assert_eq!(
            owner.execute_profile(&favorite),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict
            ))
        );
        assert_eq!(owner.host().calls, calls);
        assert_eq!(fs::read(store_path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shared_python_lock_contention_is_uncached_and_retryable() {
        let (root, store_path, mut owner) = fixture("lock-retry");
        let external =
            MigrationLock::acquire(owner.transaction.cutover_paths(), owner.transaction.uid())
                .unwrap();
        let favorite = profile_request(
            "profiles.favorite",
            json!({
                "profileId": PROFILE,
                "enabled": true,
                "operationId": "favorite-retry",
                "expectedRevision": 0
            }),
        );
        assert_eq!(
            owner.execute_profile(&favorite).unwrap(),
            NativeOwnerExecution::UncachedPreflightFailure {
                revision: 0,
                error: NativeTransactionError::Profile(ProfileTransactionError::Busy)
            }
        );
        assert_eq!(owner.revision(), 0);
        drop(external);

        let (cached, outcome) = applied(owner.execute_profile(&favorite).unwrap());
        assert_eq!(cached.revision, 1);
        assert!(matches!(outcome, NativeMutationOutcome::Profile(_)));
        let store: Value = serde_json::from_slice(&fs::read(store_path).unwrap()).unwrap();
        assert_eq!(store["profiles"][0]["favorite"], true);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn subscription_joins_shared_revision_and_fetches_without_store_lock() {
        let (root, store_path, mut owner) = fixture("subscription-shared");
        applied(owner.execute_connection(connect("connect-1", 0)).unwrap());
        let transport = FakeTransport::success(&owner);
        let ids = [SUBSCRIPTION.to_owned(), SUBSCRIPTION_PROFILE.to_owned()];
        let mut ids = ids.into_iter();
        let request = subscription_add("subscription-add-1", 1);
        let (cached, outcome) = applied(
            owner
                .execute_subscription(
                    &request,
                    &transport,
                    || ids.next().unwrap(),
                    || 1_800_000_000_000,
                )
                .unwrap(),
        );
        assert_eq!(cached.revision, 2);
        assert!(matches!(outcome, NativeMutationOutcome::Subscription(_)));
        assert_eq!(transport.calls.get(), 1);
        assert!(transport.lock_was_free.get());
        assert_eq!(owner.actual(), ActualState::Connected);

        let store: Value = serde_json::from_slice(&fs::read(store_path).unwrap()).unwrap();
        assert_eq!(store["subscriptions"].as_array().unwrap().len(), 1);
        assert_eq!(store["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(store["profiles"][1]["subscriptionId"], SUBSCRIPTION);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn subscription_fetch_preflight_is_private_reservation_free_and_rechecked() {
        let (root, store_path, mut owner) = fixture("subscription-preflight");
        let request = subscription_add("subscription-preflight-1", 0);
        let before = fs::read(&store_path).unwrap();
        let SubscriptionFetchPreflight::Ready(prepared) =
            owner.preflight_subscription_fetch(&request).unwrap()
        else {
            panic!("fresh remote mutation unexpectedly replayed");
        };
        assert_eq!(prepared.private_url(), SUBSCRIPTION_URL);
        assert_eq!(owner.revision(), 0);
        assert_eq!(fs::read(&store_path).unwrap(), before);

        applied(
            owner
                .execute_connection(connect("connect-after-preflight", 0))
                .unwrap(),
        );
        let transport = FakeTransport::success(&owner);
        assert_eq!(
            owner.execute_subscription(&request, &transport, || unreachable!(), || 0),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict
            ))
        );
        assert_eq!(transport.calls.get(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn subscription_fetch_preflight_replays_and_blocks_unsafe_external_work() {
        let (root, _store_path, mut owner) = fixture("subscription-preflight-replay");
        let request = subscription_add("subscription-preflight-replay-1", 0);
        let transport = FakeTransport::success(&owner);
        let ids = [SUBSCRIPTION.to_owned(), SUBSCRIPTION_PROFILE.to_owned()];
        let mut ids = ids.into_iter();
        let (cached, _) = applied(
            owner
                .execute_subscription(
                    &request,
                    &transport,
                    || ids.next().unwrap(),
                    || 1_800_000_000_000,
                )
                .unwrap(),
        );
        assert_eq!(cached.revision, 1);
        assert!(matches!(
            owner.preflight_subscription_fetch(&request).unwrap(),
            SubscriptionFetchPreflight::Replay(CachedOutcome {
                revision: 1,
                error: None
            })
        ));

        assert!(matches!(
            owner.preflight_subscription_fetch(&subscription_delete("delete", 1)),
            Err(NativeOwnerError::Protocol(
                MutationProtocolError::InvalidArgument
            ))
        ));
        owner.transaction.block();
        assert!(matches!(
            owner.preflight_subscription_fetch(&subscription_add("blocked", 1)),
            Err(NativeOwnerError::ManualRecoveryRequired)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn subscription_refresh_carries_private_snapshot_and_exact_retry_skips_fetch() {
        let (root, store_path, mut owner) = fixture("subscription-refresh-dispatch");
        let transport = FakeTransport::success(&owner);
        let ids = [SUBSCRIPTION.to_owned(), SUBSCRIPTION_PROFILE.to_owned()];
        let mut ids = ids.into_iter();
        applied(
            owner
                .execute_subscription(
                    &subscription_add("add-before-refresh", 0),
                    &transport,
                    || ids.next().unwrap(),
                    || 1_800_000_000_000,
                )
                .unwrap(),
        );

        let request = subscription_refresh("refresh-once", 1);
        let SubscriptionRefreshPreflight::Ready(prepared) =
            owner.preflight_subscription_refresh(&request).unwrap()
        else {
            panic!("fresh refresh unexpectedly replayed");
        };
        assert_eq!(prepared.private_url(), SUBSCRIPTION_URL);
        let body =
            PrivateSubscriptionBody::from_bytes(SUBSCRIPTION_BODY.as_bytes().to_vec()).unwrap();
        let (cached, outcome) = applied(
            owner
                .execute_fetched_subscription_refresh(
                    &request,
                    1,
                    prepared,
                    Ok(body),
                    || "30000000-0000-4000-8000-000000000001".to_owned(),
                    || 1_800_000_000_001,
                )
                .unwrap(),
        );
        assert_eq!(cached.revision, 2);
        assert!(matches!(
            outcome,
            NativeMutationOutcome::SubscriptionRefresh(_)
        ));
        assert!(matches!(
            owner.preflight_subscription_refresh(&request).unwrap(),
            SubscriptionRefreshPreflight::Replay(CachedOutcome {
                revision: 2,
                error: None
            })
        ));
        let written = fs::read(store_path).unwrap();
        let rendered = String::from_utf8(written).unwrap();
        assert!(rendered.contains("1800000000001"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn subscription_transport_failure_is_private_cached_and_has_no_store_effect() {
        let (root, store_path, mut owner) = fixture("subscription-failure");
        let before = fs::read(&store_path).unwrap();
        let transport = FakeTransport::failure();
        let request = subscription_add("subscription-failure-1", 0);
        let first = owner
            .execute_subscription(&request, &transport, || unreachable!(), || 0)
            .unwrap();
        let NativeOwnerExecution::Applied {
            cached,
            outcome: Err(error),
        } = first
        else {
            panic!("transport failure was not returned as a bounded mutation error");
        };
        assert_eq!(cached.revision, 0);
        assert_eq!(error.stable_code(), StableErrorCode::CoreRejected);
        let public = error.to_string();
        assert!(!public.contains("provider.invalid"));
        assert!(!public.contains("private-token"));
        assert_eq!(fs::read(&store_path).unwrap(), before);

        assert!(matches!(
            owner
                .execute_subscription(&request, &transport, || unreachable!(), || 0)
                .unwrap(),
            NativeOwnerExecution::Replay(_)
        ));
        assert_eq!(transport.calls.get(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_subscription_delete_uses_trusted_runtime_observation() {
        let (root, store_path, mut owner) = fixture("subscription-active-delete");
        let transport = FakeTransport::success(&owner);
        let ids = [SUBSCRIPTION.to_owned(), SUBSCRIPTION_PROFILE.to_owned()];
        let mut ids = ids.into_iter();
        applied(
            owner
                .execute_subscription(
                    &subscription_add("subscription-add-1", 0),
                    &transport,
                    || ids.next().unwrap(),
                    || 1_800_000_000_000,
                )
                .unwrap(),
        );
        applied(
            owner
                .execute_connection(OwnerRequest::new(
                    OwnerAction::Connect {
                        profile_id: SUBSCRIPTION_PROFILE.to_owned(),
                        mode: Some(RoutingMode::Global),
                    },
                    Some("connect-managed"),
                    Some(1),
                    MutationDigest::from_semantic_bytes(b"connect/managed/global"),
                ))
                .unwrap(),
        );
        let before = fs::read(&store_path).unwrap();
        let NativeOwnerExecution::Applied {
            cached,
            outcome: Err(error),
        } = owner
            .execute_subscription(
                &subscription_delete("subscription-delete-1", 2),
                &transport,
                || unreachable!(),
                || 0,
            )
            .unwrap()
        else {
            panic!("active subscription delete was not rejected");
        };
        assert_eq!(cached.revision, 2);
        assert_eq!(error.stable_code(), StableErrorCode::Conflict);
        assert_eq!(fs::read(store_path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }
}
