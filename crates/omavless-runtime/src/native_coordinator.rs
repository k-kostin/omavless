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

mod batch;
mod onboarding;
mod provider;
mod startup;
pub use batch::{NativeBatchTicket, NativeSubscriptionBatch};
pub use provider::{NativeProviderRefresh, ProviderRefreshAdmission, ProviderRefreshSnapshot};

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
    Provider(crate::provider_refresh::ProviderRefreshError),
    Protocol(MutationProtocolError),
    LongOperation(crate::long_operation::LongOperationError),
    Coordinator(CoordinatorError),
    Subscription(SubscriptionTransactionError),
    OwnershipBusy,
    RecordNotFound,
    OwnershipUnavailable,
    ManualRecoveryRequired,
    Invariant,
}

impl NativeOwnerError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Provider(error) => match error {
                crate::provider_refresh::ProviderRefreshError::Unavailable
                | crate::provider_refresh::ProviderRefreshError::NoRemoteProviders => {
                    StableErrorCode::CapabilityUnavailable
                }
                crate::provider_refresh::ProviderRefreshError::Cancelled => {
                    StableErrorCode::Conflict
                }
                _ => StableErrorCode::CoreRejected,
            },
            Self::Protocol(error) => error.stable_code(),
            Self::LongOperation(error) => error.stable_code(),
            Self::Coordinator(error) => error.stable_code(),
            Self::Subscription(error) => error.stable_code(),
            Self::OwnershipBusy => StableErrorCode::Busy,
            Self::RecordNotFound => StableErrorCode::NotFound,
            Self::OwnershipUnavailable => StableErrorCode::CapabilityUnavailable,
            Self::ManualRecoveryRequired => StableErrorCode::ManualRecoveryRequired,
            Self::Invariant => StableErrorCode::InternalError,
        }
    }
}

impl fmt::Display for NativeOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Provider(_) => "Native rule provider refresh failed",
            Self::Protocol(_) => "Native mutation request is invalid",
            Self::LongOperation(_) => "Native batch operation failed",
            Self::Coordinator(_) => "Native mutation scheduling failed",
            Self::Subscription(_) => "Native subscription refresh failed",
            Self::OwnershipBusy => "Native mutation ownership is being changed",
            Self::RecordNotFound => "Requested record was not found",
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
    batch: Option<batch::BatchOwnerState>,
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
            batch: None,
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
        self.check_batch_operation_id(request["params"]["operationId"].as_str())?;
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
        self.check_batch_operation_id(request["params"]["operationId"].as_str())?;
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

    /// Classify private import input against the current store while exact native
    /// ownership and the shared private-store lock are continuously held.
    pub(crate) fn import_preview(
        &mut self,
        request: &Value,
    ) -> Result<omavless_domain::import::ImportPreview, NativeOwnerError> {
        let parsed = crate::import_read_protocol::parse_import_preview_request(request)?;
        self.with_owned_private_store(|store| {
            store
                .preview_import(parsed.private_input())
                .map_err(|_| MutationProtocolError::InvalidArgument.into())
        })
    }

    /// Hold exact ownership and the store lease through projection creation.
    fn with_owned_private_store<T>(
        &mut self,
        project: impl FnOnce(
            &omavless_domain::private_store::PrivateStore,
        ) -> Result<T, NativeOwnerError>,
    ) -> Result<T, NativeOwnerError> {
        self.with_owned_read(|owner| {
            crate::private_store_transaction::validate_store_path(
                owner.transaction.store_path(),
                owner.transaction.uid(),
            )
            .map_err(|_| NativeOwnerError::Invariant)?;
            let input = omavless_store::read_private_utf8(
                owner.transaction.store_path(),
                owner.transaction.uid(),
            )
            .map_err(|_| NativeOwnerError::Invariant)?;
            let store = omavless_domain::private_store::parse_private_store(&input)
                .map_err(|_| NativeOwnerError::Invariant)?;
            project(&store)
        })
    }

    /// Hold the migration lease and exact ownership through a bounded read.
    /// Runtime observation deliberately does not depend on parsing the store.
    fn with_owned_read<T>(
        &mut self,
        project: impl FnOnce(&mut Self) -> Result<T, NativeOwnerError>,
    ) -> Result<T, NativeOwnerError> {
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
        let result = project(self)?;
        if self.required_ownership.is_none_or(|fence| {
            !self
                .transaction
                .ownership_matches(fence.phase, fence.generation)
        }) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        Ok(result)
    }

    pub(crate) fn runtime_observation(
        &mut self,
        request: &Value,
    ) -> Result<Value, NativeOwnerError> {
        crate::runtime_observation::validate(request)?;
        self.with_owned_read(|owner| {
            let desired = crate::desired::read_desired_snapshot(
                owner.transaction.desired_paths(),
                owner.transaction.uid(),
            )
            .map_err(|_| NativeOwnerError::Invariant)?;
            let actual = owner.actual();
            let observation = owner.host_mut().fresh_observation(&desired).ok();
            // Do not hide an independently changed desired file behind a valid
            // ownership marker, even though cooperating writers share this lock.
            let after = crate::desired::read_desired_snapshot(
                owner.transaction.desired_paths(),
                owner.transaction.uid(),
            )
            .map_err(|_| NativeOwnerError::Invariant)?;
            if after != desired {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            Ok(crate::runtime_observation::project(
                &desired,
                actual,
                observation,
            ))
        })
    }

    pub(crate) fn custom_rules(
        &mut self,
        request: &Value,
    ) -> Result<omavless_domain::private_store::PrivateCustomRules, NativeOwnerError> {
        crate::routing_read_protocol::validate_custom_rules_request(request)?;
        self.with_owned_private_store(|store| Ok(store.custom_rules_for_editor()))
    }

    pub(crate) fn support_report(&mut self, request: &Value) -> Result<Value, NativeOwnerError> {
        crate::support_diagnostics::validate(request)?;
        let actual = self.actual();
        let desired_paths = self.transaction.desired_paths().clone();
        self.with_owned_private_store(|store| {
            Ok(crate::support_diagnostics::report(
                store.support_projection(),
                actual,
                crate::routing_preset::pending(&desired_paths),
            ))
        })
    }

    pub(crate) fn ui_snapshot(&mut self, request: &Value) -> Result<Value, NativeOwnerError> {
        crate::ui_snapshot::validate(request)?;
        let actual = self.actual();
        let desired_paths = self.transaction.desired_paths().clone();
        let uid = self.transaction.uid();
        self.with_owned_private_store(|store| {
            let desired = crate::desired::read_desired_snapshot(&desired_paths, uid)
                .map_err(|_| NativeOwnerError::Invariant)?;
            Ok(crate::ui_snapshot::project(store, &desired, actual))
        })
    }

    pub(crate) fn diagnostic_snapshot(&mut self) -> Result<Vec<String>, NativeOwnerError> {
        if self.actual() == ActualState::ManualRecoveryRequired {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        if self.actual() != ActualState::Connected {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        self.with_owned_private_store(|store| {
            let fragments = store.diagnostic_private_fragments();
            // Reject the whole diagnostic, never drop private redaction inputs.
            if !omavless_mihomo::diagnostics::private_fragment_budget(&fragments) {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            Ok(fragments)
        })
    }

    pub(crate) fn check_route(
        &mut self,
        request: &Value,
    ) -> Result<omavless_domain::route_check::PrivateRouteCheck, NativeOwnerError> {
        let query = crate::route_check_protocol::query(request)?;
        if self.actual() == ActualState::ManualRecoveryRequired {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        let desired = self.desired().map_err(|_| NativeOwnerError::Invariant)?;
        let connected = self.actual() == ActualState::Connected;
        self.with_owned_private_store(|store| {
            store
                .check_route_fast_paths(desired.mode.as_str(), connected, query)
                .map_err(|_| NativeOwnerError::Protocol(MutationProtocolError::InvalidArgument))?
                .ok_or(NativeOwnerError::OwnershipUnavailable)
        })
    }

    pub(crate) fn route_plan(
        &mut self,
        request: &Value,
    ) -> Result<crate::route_probe::Plan, NativeOwnerError> {
        match self.check_route(request) {
            Ok(result) => return Ok(crate::route_probe::Plan::Fast(result.private_ui_value())),
            Err(NativeOwnerError::OwnershipUnavailable) => (),
            Err(error) => return Err(error),
        }
        // Recheck all owner/store fences: unavailable is also used by the
        // existing fast-path API for revoked ownership, not just unmatched Rule.
        use sha2::{Digest, Sha256};
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
        if self.transaction.blocked() || self.actual() == ActualState::ManualRecoveryRequired {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        let desired = self.desired().map_err(|_| NativeOwnerError::Invariant)?;
        if !desired.connected
            || desired.mode.as_str() != "rule"
            || self.actual() != ActualState::Connected
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        crate::private_store_transaction::validate_store_path(
            self.transaction.store_path(),
            self.transaction.uid(),
        )
        .map_err(|_| NativeOwnerError::Invariant)?;
        let input = omavless_store::read_private_utf8(
            self.transaction.store_path(),
            self.transaction.uid(),
        )
        .map_err(|_| NativeOwnerError::Invariant)?;
        let store = omavless_domain::private_store::parse_private_store(&input)
            .map_err(|_| NativeOwnerError::Invariant)?;
        let query = crate::route_check_protocol::query(request)?;
        // A writer may have added a custom rule between the initial pure read
        // and this snapshot lease. Preserve the no-probe custom fast path.
        if let Some(result) = store
            .check_route_fast_paths("rule", true, query)
            .map_err(|_| NativeOwnerError::Protocol(MutationProtocolError::InvalidArgument))?
        {
            return Ok(crate::route_probe::Plan::Fast(result.private_ui_value()));
        }
        let private = store.diagnostic_private_fragments();
        if !omavless_mihomo::diagnostics::private_fragment_budget(&private) {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        let store_digest = Sha256::digest(input.as_bytes()).into();
        let (pid, config_digest) = self
            .host_mut()
            .route_core_identity()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let query = omavless_domain::route_check::canonical_query(query)
            .map_err(|_| NativeOwnerError::Protocol(MutationProtocolError::InvalidArgument))?;
        Ok(crate::route_probe::Plan::Live(
            crate::route_probe::Context {
                query,
                pid,
                private,
                desired,
                store_digest,
                config_digest,
            },
        ))
    }

    pub(crate) fn profile_export(
        &mut self,
        request: &Value,
    ) -> Result<omavless_domain::private_store::PrivateProfileExport, NativeOwnerError> {
        let parsed = crate::profile_export_protocol::parse_profile_export_request(request)?;
        self.with_owned_private_store(|store| {
            store
                .profile_export(parsed.private_profile_id())
                .map_err(|_| NativeOwnerError::RecordNotFound)
        })
    }

    pub(crate) fn profile_edit_input(
        &mut self,
        request: &Value,
    ) -> Result<omavless_domain::private_store::ProfileEditInput, NativeOwnerError> {
        let parsed = crate::profile_read_protocol::parse_profile_edit_input_request(request)?;
        self.with_owned_private_store(|store| {
            store
                .profile_edit_input(parsed.private_profile_id())
                .map_err(|error| match error {
                    PrivateStoreError::ProfileNotFound => NativeOwnerError::RecordNotFound,
                    PrivateStoreError::SubscribedProfile => {
                        MutationProtocolError::InvalidArgument.into()
                    }
                    _ => NativeOwnerError::Invariant,
                })
        })
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
        if crate::routing_preset::pending(self.transaction.desired_paths()) {
            return Err(NativeOwnerError::ManualRecoveryRequired);
        }
        self.check_batch_operation_id(operation_id)?;
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
                // A durable interrupted preset must also fence the effect
                // boundary, not only the earlier queue/replay admission.
                if crate::routing_preset::pending(self.transaction.desired_paths()) {
                    self.coordinator.abort_active_uncached(token)?;
                    return Err(NativeOwnerError::ManualRecoveryRequired);
                }
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

    pub fn execute_routing_preset(
        &mut self,
        request: &Value,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::routing_preset::parse(request)?;
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
        let outcome = crate::routing_preset::PresetPlan::prepare(
            self.transaction.store_path(),
            self.transaction.desired_paths(),
            self.transaction.uid(),
            &parsed,
        )
        .map_err(store_error)
        .and_then(|plan| {
            let paths = self.transaction.cutover_paths().clone();
            if !plan.restart_required() && crate::profile_transaction::StorePlan::changed(&plan) {
                let outcome =
                    crate::profile_transaction::commit_store_only_profile(&plan, &lock, &paths);
                return plan.finish_outcome(outcome, &lock, &paths);
            }
            let desired = self
                .transaction
                .desired()
                .map_err(|_| ProfileTransactionError::Store)?;
            let outcome = apply_transaction(
                self.transaction.lifecycle_mut(),
                &plan,
                crate::profile_transaction::ActionKind::Replace,
                &desired.profile_id,
                &lock,
                &paths,
            );
            plan.finish_outcome(outcome, &lock, &paths)
        });
        self.finish(
            token,
            outcome
                .map(NativeMutationOutcome::Profile)
                .map_err(NativeTransactionError::Profile),
            false,
        )
    }

    /// Custom rules affect the current config regardless of profile identity.
    /// Reuse active replacement compensation, with the trusted desired target.
    pub fn execute_custom_rule<G: FnMut() -> String>(
        &mut self,
        request: &Value,
        mut next_record_id: G,
    ) -> Result<NativeOwnerExecution, NativeOwnerError> {
        let parsed = crate::custom_rule_protocol::parse(request)?;
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
        let generated_id = if matches!(
            &parsed.mutation,
            omavless_domain::private_store::CustomRuleMutation::Add { .. }
        ) {
            next_record_id()
        } else {
            String::new()
        };
        let outcome = crate::profile_mutation::prepare_custom_rule_mutation(
            self.transaction.store_path(),
            self.transaction.uid(),
            parsed.mutation,
            &generated_id,
        )
        .map_err(store_error)
        .and_then(|plan| {
            let desired = self
                .transaction
                .desired()
                .map_err(|_| ProfileTransactionError::Store)?;
            let paths = self.transaction.cutover_paths().clone();
            apply_transaction(
                self.transaction.lifecycle_mut(),
                &plan,
                crate::profile_transaction::ActionKind::Replace,
                &desired.profile_id,
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

    /// Add a new profile only; never replace or reconnect an existing one.
    pub fn execute_profile_import<G>(
        &mut self,
        request: &Value,
        mut next_record_id: G,
    ) -> Result<NativeOwnerExecution, NativeOwnerError>
    where
        G: FnMut() -> String,
    {
        let parsed = crate::profile_import_protocol::parse_profile_import_request(request)?;
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
        let profile_id = next_record_id();
        let outcome = crate::profile_mutation::prepare_profile_import(
            self.transaction.store_path(),
            self.transaction.uid(),
            &profile_id,
            &parsed.name,
            &parsed.input,
        )
        .map_err(store_error)
        .and_then(|plan| {
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
        self.check_batch_operation_id(request["params"]["operationId"].as_str())?;
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
        self.check_batch_operation_id(request["params"]["operationId"].as_str())?;
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
        fail_starts: usize,
        calls: usize,
    }

    impl LifecycleHost for FakeHost {
        fn validate_startup(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            if self.fail_stop {
                Err(HostStepError::Prepare)
            } else {
                Ok(())
            }
        }
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
            if self.fail_starts > 0 {
                self.fail_starts -= 1;
                return Err(HostStepError::Start);
            }
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
                fail_starts: 0,
                calls: 0,
            },
            desired_paths,
            &store_path,
            cutover,
            uid,
        );
        (root, store_path, owner)
    }

    #[test]
    fn custom_rule_mutations_share_revision_replay_and_active_restart() {
        let (root, store, mut owner) = fixture("custom-rule");
        let id = "00000000-0000-4000-8000-000000000099";
        let add = profile_request(
            "routing.custom_rules.add",
            json!({"kind":"suffix","action":"direct","value":"*.example.invalid","operationId":"rule-1","expectedRevision":0}),
        );
        let (cached, _) = applied(owner.execute_custom_rule(&add, || id.into()).unwrap());
        assert_eq!(cached.revision, 1);
        let original = fs::read(&store).unwrap();
        let calls = owner.host().calls;
        assert!(matches!(
            owner
                .execute_custom_rule(&add, || panic!("replay generated ID"))
                .unwrap(),
            NativeOwnerExecution::Replay(_)
        ));
        assert_eq!(owner.host().calls, calls);
        assert!(fs::read(&store).unwrap() == original);
        let mut duplicate = add.clone();
        duplicate["params"]
            .as_object_mut()
            .unwrap()
            .remove("operationId");
        duplicate["params"]
            .as_object_mut()
            .unwrap()
            .remove("expectedRevision");
        let NativeOwnerExecution::Applied { cached, .. } =
            owner.execute_custom_rule(&duplicate, || id.into()).unwrap()
        else {
            panic!("duplicate not classified")
        };
        assert_eq!(cached.error, Some(StableErrorCode::Conflict));
        applied(
            owner
                .execute_connection(connect("connect-before-delete", 1))
                .unwrap(),
        );
        let calls = owner.host().calls;
        let delete = profile_request(
            "routing.custom_rules.delete",
            json!({"ruleId":id,"operationId":"rule-2","expectedRevision":2}),
        );
        let (cached, _) = applied(
            owner
                .execute_custom_rule(&delete, || panic!("delete generated ID"))
                .unwrap(),
        );
        assert_eq!(cached.revision, 3);
        assert!(owner.host().calls > calls + 1);
        let payload: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        assert!(payload["customRules"].as_array().unwrap().is_empty());
        assert!(owner.transaction.desired().unwrap().connected);
        let NativeOwnerExecution::Applied { cached, .. } = owner
            .execute_custom_rule(
                &profile_request("routing.custom_rules.delete", json!({"ruleId":id})),
                String::new,
            )
            .unwrap()
        else {
            panic!("missing not classified")
        };
        assert_eq!(cached.error, Some(StableErrorCode::NotFound));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn custom_rule_candidate_failure_restores_exact_store_or_blocks() {
        for (fail_starts, fail_stop, expected) in [
            (1, false, StableErrorCode::TransitionFailedRestored),
            (2, false, StableErrorCode::ManualRecoveryRequired),
            (0, true, StableErrorCode::ManualRecoveryRequired),
        ] {
            let (root, store, mut owner) = fixture("custom-rule-fault");
            applied(owner.execute_connection(connect("connect", 0)).unwrap());
            let original = fs::read(&store).unwrap();
            owner.host_mut().fail_starts = fail_starts;
            owner.host_mut().fail_stop = fail_stop;
            let request = profile_request(
                "routing.custom_rules.add",
                json!({"kind":"domain","action":"reject","value":"example.invalid","operationId":"fault","expectedRevision":1}),
            );
            let NativeOwnerExecution::Applied { cached, .. } = owner
                .execute_custom_rule(&request, || "00000000-0000-4000-8000-000000000099".into())
                .unwrap()
            else {
                panic!("fault not classified")
            };
            assert_eq!(cached.error, Some(expected));
            assert!(
                fs::read(&store).unwrap() == original,
                "original store was not restored"
            );
            let calls = owner.host().calls;
            assert!(matches!(
                owner
                    .execute_custom_rule(&request, || panic!("fault replay generated ID"))
                    .unwrap(),
                NativeOwnerExecution::Replay(_)
            ));
            assert_eq!(owner.host().calls, calls);
            if expected == StableErrorCode::ManualRecoveryRequired {
                assert!(owner.transaction.blocked());
            } else {
                assert!(owner.transaction.desired().unwrap().connected);
                assert_eq!(owner.actual(), crate::lifecycle::ActualState::Connected);
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn custom_rule_preflight_lock_retry_and_unsafe_store_have_no_effects() {
        let (root, store, mut owner) = fixture("custom-rule-preflight");
        let request = profile_request(
            "routing.custom_rules.add",
            json!({"kind":"domain","action":"direct","value":"example.invalid","operationId":"retry","expectedRevision":0}),
        );
        let before = fs::read(&store).unwrap();
        let lock =
            MigrationLock::acquire(owner.transaction.cutover_paths(), owner.transaction.uid())
                .unwrap();
        assert!(matches!(
            owner
                .execute_custom_rule(&request, || panic!("busy generated ID"))
                .unwrap(),
            NativeOwnerExecution::UncachedPreflightFailure { .. }
        ));
        assert_eq!(owner.revision(), 0);
        assert_eq!(owner.host().calls, 0);
        assert!(fs::read(&store).unwrap() == before);
        drop(lock);
        fs::set_permissions(&store, fs::Permissions::from_mode(0o644)).unwrap();
        let mut uncached = request.clone();
        uncached["params"]
            .as_object_mut()
            .unwrap()
            .remove("operationId");
        let NativeOwnerExecution::Applied { cached, .. } = owner
            .execute_custom_rule(&uncached, || "00000000-0000-4000-8000-000000000099".into())
            .unwrap()
        else {
            panic!("unsafe store not rejected")
        };
        assert_eq!(cached.error, Some(StableErrorCode::InternalError));
        assert_eq!(owner.host().calls, 0);
        assert!(fs::read(&store).unwrap() == before);
        fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
        applied(
            owner
                .execute_custom_rule(&request, || "00000000-0000-4000-8000-000000000099".into())
                .unwrap(),
        );
        assert_eq!(owner.revision(), 1);
        fs::remove_dir_all(root).unwrap();
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
    fn onboarding_completion_never_touches_connected_intent_or_host() {
        let (root, store_path, mut owner) = fixture("onboarding-connected");
        applied(
            owner
                .execute_connection(connect("initial-connect", 0))
                .unwrap(),
        );
        let mut store: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        store["onboardingComplete"] = json!(false);
        fs::write(&store_path, serde_json::to_vec(&store).unwrap()).unwrap();
        let desired = fs::read(root.join("state/omavless/desired.json")).unwrap();
        let calls = owner.host().calls;
        let request = profile_request(
            "onboarding.complete",
            json!({"operationId":"completion","expectedRevision":1}),
        );
        let (cached, _) = applied(owner.execute_onboarding(&request).unwrap());
        assert_eq!(cached.revision, 2);
        assert_eq!(owner.host().calls, calls);
        assert_eq!(
            fs::read(root.join("state/omavless/desired.json")).unwrap(),
            desired
        );
        assert_eq!(owner.actual(), ActualState::Connected);
        assert_eq!(
            owner.execute_onboarding(&request).unwrap(),
            NativeOwnerExecution::Replay(cached)
        );
        let (no_change, _) = applied(
            owner
                .execute_onboarding(&profile_request("onboarding.complete", json!({})))
                .unwrap(),
        );
        assert_eq!(no_change.revision, 2);
        let written: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        store["onboardingComplete"] = json!(true);
        assert!(written == store);
        assert_eq!(owner.host().calls, calls);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_policy_preserves_active_intent_and_shares_replay_revision() {
        let (root, store_path, mut owner) = fixture("startup-policy");
        applied(
            owner
                .execute_connection(connect("startup-connect", 0))
                .unwrap(),
        );
        let before_desired = fs::read(root.join("state/omavless/desired.json")).unwrap();
        let calls = owner.host().calls;
        let request = profile_request(
            "startup.configure",
            json!({"enabled":true,"target":"last","profileId":"","mode":"rule","operationId":"startup-policy","expectedRevision":1}),
        );
        let (cached, _) = applied(owner.execute_startup(&request).unwrap());
        assert_eq!(cached.revision, 2);
        assert_eq!(owner.host().calls, calls);
        assert_eq!(
            fs::read(root.join("state/omavless/desired.json")).unwrap(),
            before_desired
        );
        let store: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(store["startup"]["enabled"], true);
        assert_eq!(
            owner.execute_startup(&request).unwrap(),
            NativeOwnerExecution::Replay(cached)
        );
        let mut same = request.clone();
        same["params"]
            .as_object_mut()
            .unwrap()
            .remove("operationId");
        same["params"]
            .as_object_mut()
            .unwrap()
            .remove("expectedRevision");
        let (unchanged, outcome) = applied(owner.execute_startup(&same).unwrap());
        assert_eq!(unchanged.revision, 2);
        assert_eq!(
            outcome,
            NativeMutationOutcome::Profile(ProfileMutationOutcome { changed: false })
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_invalid_preflight_preserves_store_and_desired() {
        let (root, store_path, mut owner) = fixture("startup-rejected");
        let before = fs::read(&store_path).unwrap();
        owner.host_mut().fail_stop = true;
        let request = profile_request(
            "startup.configure",
            json!({"enabled":true,"target":"last","profileId":"","mode":"rule"}),
        );
        assert!(matches!(
            owner.execute_startup(&request).unwrap(),
            NativeOwnerExecution::Applied {
                outcome: Err(_),
                ..
            }
        ));
        assert_eq!(fs::read(&store_path).unwrap(), before);
        assert_eq!(owner.revision(), 0);
        assert_eq!(owner.host().calls, 0);
        fs::remove_dir_all(root).unwrap();
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
    impl crate::subscription_batch_work::BudgetedSubscriptionTransport for FakeTransport {
        fn fetch_with_budget(
            &self,
            url: &str,
            _budget: std::time::Duration,
        ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
            SubscriptionTransport::fetch(self, url)
        }
    }

    fn batch_request(method: &str, operation: &str) -> Value {
        json!({"api": "omavless.control", "version": 1, "id": "batch-test", "method": method,
            "params": {"instanceId": "owner-instance", "operationId": operation}})
    }

    fn batch_fixture(label: &str) -> (PathBuf, PathBuf, OfflineNativeCoordinator<FakeHost>) {
        let (root, path, mut owner) = fixture(label);
        let transport = FakeTransport::success(&owner);
        let mut ids = [SUBSCRIPTION.to_owned(), SUBSCRIPTION_PROFILE.to_owned()].into_iter();
        applied(
            owner
                .execute_subscription(
                    &subscription_add("ordinary-add", 0),
                    &transport,
                    || ids.next().unwrap(),
                    || 10,
                )
                .unwrap(),
        );
        owner.initialize_batch_operations("owner-instance").unwrap();
        (root, path, owner)
    }

    fn run_batch(owner: &OfflineNativeCoordinator<FakeHost>, job: &mut NativeSubscriptionBatch) {
        let transport = FakeTransport::success(owner);
        assert_eq!(
            job.step(
                &transport,
                &crate::remote_fetch::RemoteFetchPool::default(),
                &mut || SUBSCRIPTION_PROFILE.to_owned()
            )
            .unwrap(),
            crate::subscription_batch_work::BatchWorkStep::Ready
        );
        assert!(transport.lock_was_free.get());
    }

    fn batch_status(owner: &OfflineNativeCoordinator<FakeHost>, id: &str) -> Value {
        owner
            .subscription_batch_status(&batch_request("operations.get", id))
            .unwrap()["operation"]
            .clone()
    }

    struct ProviderTransport {
        paths: CutoverPaths,
        uid: u32,
        fail: bool,
    }
    impl crate::provider_refresh::RuleProviderTransport for ProviderTransport {
        fn discover(
            &self,
            _: std::time::Duration,
        ) -> Result<
            Vec<omavless_mihomo::rule_provider::RuleProviderTarget>,
            crate::provider_refresh::ProviderRefreshError,
        > {
            assert!(MigrationLock::acquire(&self.paths, self.uid).is_ok());
            Ok(omavless_mihomo::rule_provider::refresh_targets(
                &json!({"providers":{"synthetic":{"vehicleType":"http"}}}),
            )
            .unwrap())
        }
        fn update(
            &self,
            _: &omavless_mihomo::rule_provider::RuleProviderTarget,
            _: std::time::Duration,
        ) -> Result<(), crate::provider_refresh::ProviderRefreshError> {
            assert!(MigrationLock::acquire(&self.paths, self.uid).is_ok());
            if self.fail {
                Err(crate::provider_refresh::ProviderRefreshError::Rejected)
            } else {
                Ok(())
            }
        }
    }
    fn provider_fixture() -> (
        PathBuf,
        PathBuf,
        OfflineNativeCoordinator<FakeHost>,
        ProviderTransport,
    ) {
        let (root, path, mut owner) = fixture("provider");
        let request = OwnerRequest::new(
            OwnerAction::Connect {
                profile_id: PROFILE.to_owned(),
                mode: Some(RoutingMode::Rule),
            },
            Some("connect"),
            Some(0),
            MutationDigest::from_semantic_bytes(b"provider-fixture-connect"),
        );
        applied(owner.execute_connection(request).unwrap());
        let config = path.parent().unwrap().join("config.yaml");
        fs::write(&config, b"mode: rule\n").unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        owner.initialize_batch_operations("owner-instance").unwrap();
        let transport = ProviderTransport {
            paths: owner.transaction.cutover_paths().clone(),
            uid: owner.uid(),
            fail: false,
        };
        (root, path, owner, transport)
    }
    fn provider_start(
        owner: &mut OfflineNativeCoordinator<FakeHost>,
        transport: &ProviderTransport,
        operation: &str,
    ) -> NativeProviderRefresh {
        use crate::provider_refresh::RuleProviderTransport;
        let request = batch_request("routing.refresh_providers", operation);
        let ProviderRefreshAdmission::Discover(snapshot) =
            owner.preflight_provider_refresh(&request).unwrap()
        else {
            panic!("unexpected replay");
        };
        let targets = transport
            .discover(std::time::Duration::from_secs(1))
            .unwrap();
        owner
            .start_provider_refresh(&request, snapshot, targets)
            .unwrap()
            .unwrap()
    }
    #[test]
    fn provider_owner_commits_once_and_shares_registry_replay_and_collision_namespace() {
        let (root, path, mut owner, transport) = provider_fixture();
        let mut job = provider_start(&mut owner, &transport, "refresh");
        owner.publish_provider_refresh_progress(&job).unwrap();
        assert_eq!(
            job.step(&transport, &crate::remote_fetch::RemoteFetchPool::default()),
            Ok(crate::provider_refresh::ProviderRefreshStep::Ready)
        );
        owner
            .complete_provider_refresh(job, || 9, || Ok(()))
            .unwrap();
        assert_eq!(owner.revision(), 2);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["rulesUpdatedAt"],
            9
        );
        assert_eq!(batch_status(&owner, "refresh")["state"], "succeeded");
        assert_eq!(
            batch_status(&owner, "refresh")["method"],
            "routing.refresh_providers"
        );
        assert!(matches!(
            owner
                .preflight_provider_refresh(&batch_request("routing.refresh_providers", "refresh"))
                .unwrap(),
            ProviderRefreshAdmission::Replay(_)
        ));
        assert!(
            owner
                .start_subscription_batch(&batch_request("subscriptions.refresh_all", "refresh"))
                .is_err()
        );
        assert!(owner.execute_connection(connect("refresh", 2)).is_err());
        let mut next = provider_start(&mut owner, &transport, "refresh-two");
        next.step(&transport, &crate::remote_fetch::RemoteFetchPool::default())
            .unwrap();
        owner
            .complete_provider_refresh(next, || 1, || Ok(()))
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap()["rulesUpdatedAt"],
            10
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn provider_owner_failure_cancel_and_stale_snapshot_never_stamp() {
        for scenario in [
            "failure",
            "cancel",
            "cancel_identity",
            "cancel_failure",
            "store",
            "config",
            "disconnect",
            "identity",
        ] {
            let (root, path, mut owner, mut transport) = provider_fixture();
            let mut job = provider_start(&mut owner, &transport, "refresh");
            let before = fs::read(&path).unwrap();
            transport.fail = matches!(scenario, "failure" | "cancel_failure");
            let _ = job.step(&transport, &crate::remote_fetch::RemoteFetchPool::default());
            match scenario {
                "cancel" | "cancel_identity" | "cancel_failure" => {
                    owner
                        .cancel_subscription_batch(&batch_request("operations.cancel", "refresh"))
                        .unwrap();
                }
                "store" => {
                    let mut document: Value = serde_json::from_slice(&before).unwrap();
                    document["onboardingComplete"] = json!(false);
                    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
                }
                "config" => {
                    fs::write(
                        path.parent().unwrap().join("config.yaml"),
                        b"mode: direct\n",
                    )
                    .unwrap();
                }
                "disconnect" => {
                    applied(
                        owner
                            .execute_connection(OwnerRequest::new(
                                OwnerAction::Disconnect,
                                Some("disconnect"),
                                Some(1),
                                MutationDigest::from_semantic_bytes(b"provider-disconnect"),
                            ))
                            .unwrap(),
                    );
                }
                _ => {}
            }
            let expected = fs::read(&path).unwrap();
            let _ = owner.complete_provider_refresh(
                job,
                || panic!("failed provider job read timestamp"),
                || {
                    assert!(
                        scenario != "cancel_identity",
                        "cancelled job verified controller identity"
                    );
                    if scenario == "identity" {
                        Err(crate::provider_refresh::ProviderRefreshError::Unavailable)
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(fs::read(&path).unwrap() == expected);
            assert_eq!(
                batch_status(&owner, "refresh")["state"],
                if matches!(scenario, "cancel" | "cancel_identity" | "cancel_failure") {
                    "cancelled"
                } else {
                    "failed"
                }
            );
            if scenario.starts_with("cancel") {
                assert_eq!(owner.revision(), 1);
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn batch_owner_commits_once_and_replays_without_fetch_or_store_read() {
        let (root, path, mut owner) = batch_fixture("batch-success");
        let mut request = batch_request("subscriptions.refresh_all", "batch-1");
        request["params"]["expectedRevision"] = json!(1);
        let mut job = owner.start_subscription_batch(&request).unwrap().unwrap();
        run_batch(&owner, &mut job);
        owner.publish_subscription_batch_progress(&job).unwrap();
        assert_eq!(batch_status(&owner, "batch-1")["progress"]["completed"], 1);
        owner.complete_subscription_batch(job, || 20).unwrap();
        assert_eq!(owner.revision(), 2);
        assert_eq!(batch_status(&owner, "batch-1")["state"], "succeeded");
        assert_eq!(batch_status(&owner, "batch-1")["outcomeRevision"], 2);
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["subscriptions"][0]["updatedAt"],
            20
        );
        fs::remove_file(&path).unwrap();
        assert!(owner.start_subscription_batch(&request).unwrap().is_none());
        assert!(
            !owner
                .cancel_subscription_batch(&batch_request("operations.cancel", "batch-1"))
                .unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_owner_cancellation_before_fetch_and_after_preparation_never_writes() {
        for prepared in [false, true] {
            let (root, path, mut owner) = batch_fixture("batch-cancel");
            let before = fs::read(&path).unwrap();
            let mut job = owner
                .start_subscription_batch(&batch_request("subscriptions.refresh_all", "cancel"))
                .unwrap()
                .unwrap();
            if prepared {
                run_batch(&owner, &mut job);
            }
            assert!(
                owner
                    .cancel_subscription_batch(&batch_request("operations.cancel", "cancel"))
                    .unwrap()
            );
            if !prepared {
                let transport = FakeTransport::failure();
                assert_eq!(
                    job.step(
                        &transport,
                        &crate::remote_fetch::RemoteFetchPool::default(),
                        &mut || panic!("cancelled work generated an ID")
                    ),
                    Err(crate::subscription_batch_work::BatchWorkError::Cancelled)
                );
                assert_eq!(transport.calls.get(), 0);
            }
            assert!(
                owner
                    .complete_subscription_batch(job, || panic!("cancelled batch read clock"))
                    .is_err()
            );
            assert_eq!(batch_status(&owner, "cancel")["state"], "cancelled");
            assert_eq!(owner.revision(), 1);
            assert_eq!(fs::read(&path).unwrap(), before);
            assert!(
                owner
                    .start_subscription_batch(&batch_request("subscriptions.refresh_all", "next"))
                    .unwrap()
                    .is_some()
            );
            owner.stop_batch_operations().unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn batch_owner_disconnect_revision_wins_over_ready_batch() {
        let (root, path, mut owner) = batch_fixture("batch-disconnect");
        applied(
            owner
                .execute_connection(connect("connect-before", 1))
                .unwrap(),
        );
        let mut job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "stale"))
            .unwrap()
            .unwrap();
        run_batch(&owner, &mut job);
        let request = OwnerRequest::new(
            OwnerAction::Disconnect,
            Some("disconnect"),
            Some(2),
            MutationDigest::new([42; 32]),
        );
        applied(owner.execute_connection(request).unwrap());
        let before = fs::read(&path).unwrap();
        assert_eq!(
            owner.complete_subscription_batch(job, || panic!("stale batch read clock")),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict
            ))
        );
        assert_eq!(owner.revision(), 3);
        assert_eq!(batch_status(&owner, "stale")["state"], "failed");
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_owner_namespace_blocks_both_directions_and_external_preflight() {
        let (root, _path, mut owner) = batch_fixture("batch-ids");
        assert!(matches!(
            owner.start_subscription_batch(&batch_request(
                "subscriptions.refresh_all",
                "ordinary-add"
            )),
            Err(NativeOwnerError::LongOperation(
                crate::long_operation::LongOperationError::OperationConflict
            ))
        ));
        let _job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "long-id"))
            .unwrap()
            .unwrap();
        assert!(matches!(
            owner.preflight_subscription_fetch(&subscription_add("long-id", 1)),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        ));
        assert!(matches!(
            owner.preflight_subscription_refresh(&subscription_refresh("long-id", 1)),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        ));
        assert!(matches!(
            owner.execute_connection(connect("long-id", 1)),
            Err(NativeOwnerError::Coordinator(
                CoordinatorError::OperationConflict
            ))
        ));
        assert!(matches!(
            owner.start_subscription_batch(&batch_request("subscriptions.refresh_all", "another")),
            Err(NativeOwnerError::LongOperation(
                crate::long_operation::LongOperationError::Busy
            ))
        ));
        owner.stop_batch_operations().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_owner_empty_success_does_not_write_or_read_clock_or_advance_revision() {
        let (root, path, mut owner) = fixture("batch-empty");
        owner.initialize_batch_operations("owner-instance").unwrap();
        let before = fs::read(&path).unwrap();
        let inode = fs::metadata(&path).unwrap().ino();
        let job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "empty"))
            .unwrap()
            .unwrap();
        owner
            .complete_subscription_batch(job, || panic!("empty batch read clock"))
            .unwrap();
        assert_eq!(owner.revision(), 0);
        assert_eq!(batch_status(&owner, "empty")["state"], "succeeded");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_owner_rejects_changed_store_without_overwriting_external_change() {
        let (root, path, mut owner) = batch_fixture("batch-store-change");
        let mut job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "store-change"))
            .unwrap()
            .unwrap();
        run_batch(&owner, &mut job);
        let mut store: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        store["subscriptions"][0]["url"] = json!("https://changed.invalid/feed");
        let before = serde_json::to_vec(&store).unwrap();
        fs::write(&path, &before).unwrap();
        assert!(owner.complete_subscription_batch(job, || 20).is_err());
        assert_eq!(owner.revision(), 1);
        assert_eq!(batch_status(&owner, "store-change")["state"], "failed");
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_owner_shutdown_revokes_ready_work_and_stops_admission() {
        let (root, path, mut owner) = batch_fixture("batch-shutdown");
        let before = fs::read(&path).unwrap();
        let mut job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "shutdown"))
            .unwrap()
            .unwrap();
        run_batch(&owner, &mut job);
        owner.stop_batch_operations().unwrap();
        owner.stop_batch_operations().unwrap();
        assert!(
            owner
                .complete_subscription_batch(job, || panic!("revoked job read clock"))
                .is_err()
        );
        assert!(
            owner
                .start_subscription_batch(&batch_request("subscriptions.refresh_all", "new"))
                .is_err()
        );
        assert_eq!(batch_status(&owner, "shutdown")["state"], "failed");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(owner.revision(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_pending_preset_blocks_start_replay_and_prepared_commit() {
        for phase in ["start", "replay", "commit"] {
            let (root, path, mut owner) = batch_fixture("batch-preset-barrier");
            let request = batch_request("subscriptions.refresh_all", "guarded");
            let mut job = if phase == "start" {
                None
            } else {
                let mut job = owner.start_subscription_batch(&request).unwrap().unwrap();
                run_batch(&owner, &mut job);
                Some(job)
            };
            if phase == "replay" {
                owner
                    .complete_subscription_batch(job.take().unwrap(), || 20)
                    .unwrap();
            }
            let before = fs::read(&path).unwrap();
            let revision = owner.revision();
            let marker = owner
                .transaction
                .desired_paths()
                .directory
                .join("routing-preset.pending.json");
            fs::write(&marker, b"interrupted").unwrap();
            if phase == "commit" {
                assert!(matches!(
                    owner.complete_subscription_batch(job.take().unwrap(), || panic!(
                        "no clock read"
                    )),
                    Err(NativeOwnerError::ManualRecoveryRequired)
                ));
                assert_eq!(
                    batch_status(&owner, "guarded")["error"]["code"],
                    "manual_recovery_required"
                );
            } else {
                assert!(matches!(
                    owner.start_subscription_batch(&request),
                    Err(NativeOwnerError::ManualRecoveryRequired)
                ));
            }
            assert_eq!(fs::read(&path).unwrap(), before);
            assert_eq!(owner.revision(), revision);
            assert!(marker.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn batch_uncertain_store_write_blocks_owner_without_rewriting_or_replay() {
        for after_replace in [false, true] {
            let (root, path, mut owner) = batch_fixture("batch-uncertain-write");
            let request = batch_request("subscriptions.refresh_all", "uncertain");
            let mut job = owner.start_subscription_batch(&request).unwrap().unwrap();
            run_batch(&owner, &mut job);
            let before = fs::read(&path).unwrap();
            let revision = owner.revision();
            let result = owner.complete_subscription_batch_with_store(
                job,
                || 20,
                |path, uid, snapshot, entries, timestamp| {
                    if after_replace {
                        crate::subscription_mutation::commit_subscription_refresh_batch(
                            path, uid, snapshot, entries, timestamp,
                        )
                        .unwrap();
                    }
                    Err(SubscriptionMutationCommitError::StoreIo)
                },
            );
            assert!(matches!(
                result,
                Err(NativeOwnerError::ManualRecoveryRequired)
            ));
            let after = fs::read(&path).unwrap();
            assert_eq!(after != before, after_replace);
            assert_eq!(owner.revision(), revision);
            assert!(owner.transaction.blocked());
            assert_eq!(
                batch_status(&owner, "uncertain")["error"]["code"],
                "manual_recovery_required"
            );
            let favorite = profile_request(
                "profiles.favorite",
                json!({"profileId":PROFILE,"enabled":true}),
            );
            let NativeOwnerExecution::Applied {
                outcome: Err(error),
                ..
            } = owner.execute_profile(&favorite).unwrap()
            else {
                panic!("ordinary mutation bypassed uncertain batch write");
            };
            assert_eq!(error.stable_code(), StableErrorCode::ManualRecoveryRequired);
            assert!(matches!(
                owner.start_subscription_batch(&request),
                Err(NativeOwnerError::ManualRecoveryRequired)
            ));
            assert!(matches!(
                owner.start_subscription_batch(&batch_request("subscriptions.refresh_all", "next")),
                Err(NativeOwnerError::ManualRecoveryRequired)
            ));
            assert_eq!(fs::read(&path).unwrap(), after);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn batch_owner_ownership_withdrawal_prevents_commit() {
        let (root, path, mut owner) = batch_fixture("batch-ownership");
        let before = fs::read(&path).unwrap();
        let mut job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "ownership"))
            .unwrap()
            .unwrap();
        run_batch(&owner, &mut job);
        owner.required_ownership = Some(OwnershipFence {
            phase: OwnershipPhase::Rust,
            generation: 99,
        });
        assert_eq!(
            owner.complete_subscription_batch(job, || panic!("unowned job read clock")),
            Err(NativeOwnerError::OwnershipUnavailable)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(owner.revision(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn batch_owner_remains_available_during_fetch_and_cancel_beats_fetch_failure() {
        use std::sync::mpsc;
        use std::time::Duration;
        struct PausedTransport {
            started: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
        }
        impl crate::subscription_batch_work::BudgetedSubscriptionTransport for PausedTransport {
            fn fetch_with_budget(
                &self,
                _url: &str,
                _budget: Duration,
            ) -> Result<PrivateSubscriptionBody, SubscriptionTransportError> {
                self.started.send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(5)).unwrap();
                Err(SubscriptionTransportError::Unavailable)
            }
        }
        let (root, path, mut owner) = batch_fixture("batch-concurrent");
        let before = fs::read(&path).unwrap();
        let mut job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "concurrent"))
            .unwrap()
            .unwrap();
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = job.step(
                &PausedTransport {
                    started,
                    release: wait,
                },
                &crate::remote_fetch::RemoteFetchPool::default(),
                &mut || panic!("failed fetch generated IDs"),
            );
            (job, result)
        });
        let began = ready.recv_timeout(Duration::from_secs(5));
        let status =
            owner.subscription_batch_status(&batch_request("operations.get", "concurrent"));
        let cancelled =
            owner.cancel_subscription_batch(&batch_request("operations.cancel", "concurrent"));
        // Release/join before assertions so failures cannot leave a blocked worker.
        let _ = release.send(());
        let (job, result) = worker.join().unwrap();
        assert!(began.is_ok());
        assert!(status.is_ok());
        assert_eq!(cancelled, Ok(true));
        assert_eq!(
            result,
            Err(crate::subscription_batch_work::BatchWorkError::Cancelled)
        );
        assert!(
            owner
                .complete_subscription_batch(job, || panic!("cancel read clock"))
                .is_err()
        );
        assert_eq!(batch_status(&owner, "concurrent")["state"], "cancelled");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(owner.revision(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_owner_supervisor_reclaims_lost_worker_without_revoking_successor() {
        let (root, path, mut owner) = batch_fixture("batch-lost-worker");
        let before = fs::read(&path).unwrap();
        let job = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "lost"))
            .unwrap()
            .unwrap();
        let ticket = job.supervisor_ticket();
        let stale = ticket.clone();
        drop(job); // models failed spawn/panicked worker before returning its payload
        owner.abort_subscription_batch(ticket).unwrap();
        assert_eq!(batch_status(&owner, "lost")["state"], "failed");
        let mut next = owner
            .start_subscription_batch(&batch_request("subscriptions.refresh_all", "successor"))
            .unwrap()
            .unwrap();
        assert!(owner.abort_subscription_batch(stale).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        run_batch(&owner, &mut next);
        owner.complete_subscription_batch(next, || 20).unwrap();
        assert_eq!(batch_status(&owner, "successor")["state"], "succeeded");
        assert_eq!(owner.revision(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
