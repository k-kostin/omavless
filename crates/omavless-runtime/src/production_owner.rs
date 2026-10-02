// SPDX-License-Identifier: MIT

//! Production construction boundary for the native runtime owner.
//!
//! Construction is allowed only while the shared migration lock proves a
//! committed `rust` ownership marker. Startup reconciliation completes under
//! that same lease before the owner can be returned. Socket registration is a
//! separate boundary: constructing this value alone still exposes no IPC.

use crate::RuntimePaths;
use crate::connection_transaction::{ConnectionTransactionError, ConnectionTransactionOutcome};
use crate::cutover::{
    CutoverError, CutoverPaths, MigrationLock, OwnershipPhase, TransitionBootstrap, read_marker,
    read_marker_existing,
};
use crate::desired::DesiredPaths;
use crate::lifecycle::{ActualState, LifecycleHost};
use crate::login_transaction::{check_login_receipt_without_private_fence, check_startup_receipt};
use crate::native_coordinator::{
    CandidatePromotion, NativeOwnerError, OfflineNativeCoordinator, PreparedSubscriptionRefresh,
};
use crate::native_dispatch::{
    RemoteSubscriptionPreflight, RemoteSubscriptionRefreshPreflight, preflight_native_subscription,
    preflight_native_subscription_refresh, respond_to_fetched_subscription,
    respond_to_fetched_subscription_refresh, respond_to_native_mutation,
    respond_to_subscription_edit_input,
};
use crate::native_host::{NativeHostPaths, NativeLifecycleHost};
use crate::subscription_transport::{SubscriptionTransport, SubscriptionTransportError};
use nix::unistd::Uid;
use omavless_control_protocol::ProtocolError;
use omavless_domain::private_store::{StoreListProjection, parse_private_store};
use omavless_domain::subscription_feed::PrivateSubscriptionBody;
use omavless_store::read_private_utf8;
use serde_json::Value;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionOwnerError {
    Busy,
    OwnershipUnavailable,
    HostUnavailable,
    RecoveryFailed,
    ManualRecoveryRequired,
}

impl fmt::Display for ProductionOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "Another OmaVLESS operation owns the migration lock",
            Self::OwnershipUnavailable => "Native runtime ownership is unavailable",
            Self::HostUnavailable => "Native runtime host is unavailable",
            Self::RecoveryFailed => "Native runtime startup recovery failed",
            Self::ManualRecoveryRequired => "Manual recovery is required",
        })
    }
}

impl std::error::Error for ProductionOwnerError {}

/// Inactive, read-only restart classification. It is deliberately not a
/// `ProductionNativeOwner`: no normal mutation, login or IPC capability can be
/// obtained from it. Every later recovery effect needs fresh admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestoreStartupReview {
    Undecided,
    VerifyCommitted,
    VerifyAborted,
    FinalReceipt,
    VerifyCompleted,
    ManualRecovery,
}

fn lock_error(error: CutoverError) -> ProductionOwnerError {
    match error {
        CutoverError::Busy => ProductionOwnerError::Busy,
        _ => ProductionOwnerError::OwnershipUnavailable,
    }
}

fn recovery_error(error: ConnectionTransactionError) -> ProductionOwnerError {
    match error {
        ConnectionTransactionError::Busy => ProductionOwnerError::Busy,
        ConnectionTransactionError::ManualRecoveryRequired => {
            ProductionOwnerError::ManualRecoveryRequired
        }
        ConnectionTransactionError::RecoveryFailed => ProductionOwnerError::RecoveryFailed,
        ConnectionTransactionError::NotFound
        | ConnectionTransactionError::InvalidArgument
        | ConnectionTransactionError::Conflict
        | ConnectionTransactionError::Store
        | ConnectionTransactionError::TransitionFailedRestored => {
            ProductionOwnerError::RecoveryFailed
        }
    }
}

/// A reconciled, ownership-gated native owner. It intentionally has no
/// `Debug`, serialization, or public access to the credential-bearing host.
pub struct ProductionNativeOwner<H = NativeLifecycleHost> {
    coordinator: OfflineNativeCoordinator<H>,
    startup: ConnectionTransactionOutcome,
    ownership: ProductionOwnership,
    login_ready: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProductionOwnership {
    Candidate(TransitionBootstrap),
    Committed {
        rust_generation: u64,
        origin_preparing_generation: Option<u64>,
    },
    Stale,
}

impl<H: LifecycleHost> ProductionNativeOwner<H> {
    /// Inspect a pending restore at the same lock/owner boundary as production
    /// startup, but never construct or reconcile the ordinary owner. Only
    /// synthetic tests call this candidate; it is not a product recovery path.
    #[allow(dead_code)]
    pub(crate) fn review_restore_startup(
        mut host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
        uid: u32,
    ) -> Result<RestoreStartupReview, ProductionOwnerError> {
        use crate::desired::read_desired_snapshot;
        use crate::restore_cleanup_candidate::inspect_completion_record;
        use crate::restore_closure_model::CLOSURE_MEMBER;
        use crate::restore_decision_candidate::RecoveryReview;
        use crate::restore_journal_candidate::{
            inspect_decision_journal, read_desired_for_decision,
        };
        use crate::restore_retirement_candidate::inspect_retirement_receipt;
        use crate::restore_staging_candidate::classify_live_pair_bound;

        // The publication-only handoff has no restart continuation yet. Even
        // a valid predecessor must not mask this separate existence fence.
        let successor_pending = || {
            !matches!(
                std::fs::symlink_metadata(cutover_paths.state_directory.join(
                    crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            )
        };

        let lock = MigrationLock::acquire_existing(&cutover_paths, uid).map_err(lock_error)?;
        let marker = read_marker_existing(&cutover_paths, uid)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        if marker.phase() != OwnershipPhase::Rust {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        let config = store_path
            .parent()
            .filter(|_| {
                store_path
                    .file_name()
                    .is_some_and(|name| name == "profiles.json")
            })
            .ok_or(ProductionOwnerError::ManualRecoveryRequired)?;
        if !crate::pending_private_transaction::pending_at(&cutover_paths.state_directory)
            || crate::routing_preset::pending_at(&cutover_paths.state_directory)
            || successor_pending()
        {
            return Err(ProductionOwnerError::ManualRecoveryRequired);
        }
        check_login_receipt_without_private_fence(
            &cutover_paths,
            uid,
            &lock,
            Some(marker.generation()),
        )
        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        let desired = read_desired_snapshot(&desired_paths, uid)
            .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        if desired.connected {
            return Err(ProductionOwnerError::ManualRecoveryRequired);
        }
        let empty = |observation: crate::lifecycle::NativeLocalObservation| {
            !observation.owned_core_running
                && observation.owned_auxiliary_mihomo_count == 0
                && observation.managed_tun_count == 0
                && !observation.owned_controller_config_verified
                && !observation.desired_profile_matches_owned
        };
        if !host.fresh_observation(&desired).is_ok_and(empty) {
            return Err(ProductionOwnerError::ManualRecoveryRequired);
        }
        let receipt_path = cutover_paths
            .state_directory
            .join("restore-finalization.pending");
        let completion_path = cutover_paths.state_directory.join(CLOSURE_MEMBER);
        let completion_present = !matches!(
            std::fs::symlink_metadata(&completion_path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        );
        let review = if !matches!(
            std::fs::symlink_metadata(&receipt_path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ) {
            let pending =
                inspect_retirement_receipt(config, &cutover_paths, uid, marker.generation(), &lock)
                    .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
            if completion_present
                && !inspect_completion_record(
                    config,
                    &cutover_paths,
                    uid,
                    marker.generation(),
                    &lock,
                )
                .is_ok_and(|completed| completed.matches_pending(&pending))
            {
                return Err(ProductionOwnerError::ManualRecoveryRequired);
            }
            RestoreStartupReview::FinalReceipt
        } else if completion_present {
            inspect_completion_record(config, &cutover_paths, uid, marker.generation(), &lock)
                .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
            RestoreStartupReview::VerifyCompleted
        } else {
            match inspect_decision_journal(&cutover_paths, uid, &lock) {
                Ok(chain) => {
                    let raw_desired = read_desired_for_decision(&cutover_paths, uid, &lock)
                        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
                    let pair = classify_live_pair_bound(
                        config,
                        &cutover_paths,
                        uid,
                        marker.generation(),
                        &lock,
                    )
                    .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
                    match chain.review(
                        marker.generation(),
                        raw_desired.as_ref().map(|bytes| bytes.as_slice()),
                        &pair,
                    ) {
                        RecoveryReview::OldRollbackCandidate => RestoreStartupReview::Undecided,
                        RecoveryReview::VerifyCommittedCandidate => {
                            RestoreStartupReview::VerifyCommitted
                        }
                        RecoveryReview::VerifyAbortedCandidate => {
                            RestoreStartupReview::VerifyAborted
                        }
                        RecoveryReview::ManualRecovery => RestoreStartupReview::ManualRecovery,
                    }
                }
                Err(_) => RestoreStartupReview::ManualRecovery,
            }
        };
        if read_marker_existing(&cutover_paths, uid).ok() != Some(marker)
            || read_desired_snapshot(&desired_paths, uid).ok().as_ref() != Some(&desired)
            || !host.fresh_observation(&desired).is_ok_and(empty)
            || !crate::pending_private_transaction::pending_at(&cutover_paths.state_directory)
            || successor_pending()
        {
            return Err(ProductionOwnerError::ManualRecoveryRequired);
        }
        Ok(review)
    }

    /// Build an owner from trusted paths and an already constructed host.
    /// Tests use this boundary with a deterministic host; production uses
    /// [`ProductionNativeOwner::current`].
    pub fn initialize(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
        uid: u32,
    ) -> Result<Self, ProductionOwnerError> {
        let lock = MigrationLock::acquire(&cutover_paths, uid).map_err(lock_error)?;
        Self::initialize_locked(host, desired_paths, store_path, cutover_paths, uid, lock)
    }

    fn initialize_locked(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
        uid: u32,
        lock: MigrationLock,
    ) -> Result<Self, ProductionOwnerError> {
        let marker = read_marker(&cutover_paths, uid)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        if marker.phase() != OwnershipPhase::Rust {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        check_startup_receipt(&cutover_paths, uid, &lock, Some(marker.generation()))
            .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        let mut coordinator = OfflineNativeCoordinator::new_ownership_gated(
            host,
            desired_paths,
            store_path,
            cutover_paths,
            uid,
            marker.generation(),
        );
        let startup = coordinator
            .reconcile_startup_locked(&lock)
            .map_err(recovery_error)?;
        drop(lock);
        Ok(Self {
            coordinator,
            startup,
            login_ready: false,
            ownership: ProductionOwnership::Committed {
                rust_generation: marker.generation(),
                origin_preparing_generation: None,
            },
        })
    }

    /// Build a reconciled read-only candidate for one exact preparing marker.
    /// The candidate never writes the marker and remains mutation-gated until
    /// the immediate committed Rust successor is observed.
    #[cfg(test)]
    pub(crate) fn initialize_candidate(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
        uid: u32,
        preparing_generation: u64,
    ) -> Result<Self, ProductionOwnerError> {
        let lock = MigrationLock::acquire(&cutover_paths, uid).map_err(lock_error)?;
        let marker = read_marker(&cutover_paths, uid)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        let bootstrap = TransitionBootstrap::from_preparing(&marker)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        if bootstrap.preparing_generation() != preparing_generation {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        Self::initialize_candidate_locked(
            host,
            desired_paths,
            store_path,
            cutover_paths,
            uid,
            bootstrap,
            lock,
        )
    }

    fn initialize_candidate_locked(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
        uid: u32,
        bootstrap: TransitionBootstrap,
        lock: MigrationLock,
    ) -> Result<Self, ProductionOwnerError> {
        check_startup_receipt(&cutover_paths, uid, &lock, None)
            .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        let mut coordinator = OfflineNativeCoordinator::new_transition_candidate(
            host,
            desired_paths,
            store_path,
            cutover_paths,
            uid,
            bootstrap.preparing_generation(),
        );
        let startup = coordinator
            .reconcile_startup_locked(&lock)
            .map_err(recovery_error)?;
        drop(lock);
        Ok(Self {
            coordinator,
            startup,
            login_ready: false,
            ownership: ProductionOwnership::Candidate(bootstrap),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.coordinator.revision()
    }

    #[must_use]
    pub const fn actual(&self) -> ActualState {
        self.coordinator.actual()
    }

    #[must_use]
    pub const fn startup_outcome(&self) -> ConnectionTransactionOutcome {
        self.startup
    }

    pub(crate) const fn login_ready(&self) -> bool {
        self.login_ready
    }

    #[cfg(test)]
    pub(crate) fn set_test_login_ready(&mut self, ready: bool) {
        self.login_ready = ready;
    }

    pub(crate) fn desired(&self) -> Result<crate::desired::DesiredState, ProductionOwnerError> {
        self.coordinator
            .desired()
            .map_err(|_| ProductionOwnerError::RecoveryFailed)
    }

    /// Read the current private store and release only the bounded metadata
    /// projection accepted for the same-user frontend control socket.
    pub(crate) fn list_projection(&self) -> Result<StoreListProjection, ProductionOwnerError> {
        let input = read_private_utf8(self.coordinator.store_path(), self.coordinator.uid())
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let store =
            parse_private_store(&input).map_err(|_| ProductionOwnerError::HostUnavailable)?;
        Ok(store.list_projection())
    }

    pub(crate) fn rust_ownership_available(&mut self) -> bool {
        match self.ownership {
            ProductionOwnership::Committed {
                rust_generation,
                origin_preparing_generation: _,
            } => {
                let _ = rust_generation;
                self.coordinator.rust_ownership_available()
            }
            ProductionOwnership::Candidate(bootstrap) => {
                match self.coordinator.try_promote_candidate() {
                    Ok(CandidatePromotion::Pending) | Err(NativeOwnerError::OwnershipBusy) => false,
                    Ok(CandidatePromotion::Promoted { rust_generation })
                        if rust_generation == bootstrap.rust_generation() =>
                    {
                        self.ownership = ProductionOwnership::Committed {
                            rust_generation,
                            origin_preparing_generation: Some(bootstrap.preparing_generation()),
                        };
                        true
                    }
                    Ok(CandidatePromotion::Promoted { .. })
                    | Ok(CandidatePromotion::Stale)
                    | Err(_) => {
                        self.ownership = ProductionOwnership::Stale;
                        false
                    }
                }
            }
            ProductionOwnership::Stale => false,
        }
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) const fn preparing_generation(&self) -> Option<u64> {
        match self.ownership {
            ProductionOwnership::Candidate(bootstrap) => Some(bootstrap.preparing_generation()),
            ProductionOwnership::Committed { .. } | ProductionOwnership::Stale => None,
        }
    }

    #[must_use]
    pub(crate) const fn bootstrap_generations(&self) -> Option<(u64, u64)> {
        match self.ownership {
            ProductionOwnership::Candidate(bootstrap) => Some((
                bootstrap.preparing_generation(),
                bootstrap.rust_generation(),
            )),
            ProductionOwnership::Committed {
                rust_generation,
                origin_preparing_generation: Some(preparing_generation),
            } => Some((preparing_generation, rust_generation)),
            ProductionOwnership::Committed {
                origin_preparing_generation: None,
                ..
            }
            | ProductionOwnership::Stale => None,
        }
    }

    #[must_use]
    pub(crate) const fn transition(&self) -> Option<&'static str> {
        match self.ownership {
            ProductionOwnership::Candidate(_) => Some("cutoverPreparing"),
            ProductionOwnership::Stale => Some("staleCandidate"),
            ProductionOwnership::Committed { .. } => None,
        }
    }

    // Only the runtime registration/scheduler may access the serialized batch
    // owner. This is not an alternative production constructor or writer.
    pub(crate) fn batch_coordinator(&mut self) -> &mut OfflineNativeCoordinator<H> {
        &mut self.coordinator
    }

    pub(crate) fn respond<T, G, N>(
        &mut self,
        request: &Value,
        transport: &T,
        next_record_id: G,
        now_millis: N,
    ) -> Result<Value, ProtocolError>
    where
        T: SubscriptionTransport,
        G: FnMut() -> String,
        N: FnOnce() -> u64,
    {
        respond_to_native_mutation(
            &mut self.coordinator,
            request,
            transport,
            next_record_id,
            now_millis,
        )
    }

    pub(crate) fn preflight_remote_subscription(
        &mut self,
        request: &Value,
    ) -> Result<RemoteSubscriptionPreflight, ProtocolError> {
        preflight_native_subscription(&mut self.coordinator, request)
    }

    pub(crate) fn preflight_remote_subscription_refresh(
        &mut self,
        request: &Value,
    ) -> Result<RemoteSubscriptionRefreshPreflight, ProtocolError> {
        preflight_native_subscription_refresh(&mut self.coordinator, request)
    }

    pub(crate) fn subscription_edit_input(
        &mut self,
        request: &Value,
    ) -> Result<Value, ProtocolError> {
        respond_to_subscription_edit_input(&mut self.coordinator, request)
    }

    pub(crate) fn import_preview(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_import_preview(&mut self.coordinator, request)
    }

    pub(crate) fn profile_export(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_profile_export(&mut self.coordinator, request)
    }
    pub(crate) fn profile_details(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_profile_details(&mut self.coordinator, request)
    }

    pub(crate) fn custom_rules(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_custom_rules(&mut self.coordinator, request)
    }

    pub(crate) fn support_report(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_support_report(&mut self.coordinator, request)
    }

    pub(crate) fn ui_snapshot(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        let mut response =
            crate::native_dispatch::respond_to_ui_snapshot(&mut self.coordinator, request)?;
        if response["ok"] == true {
            response["result"]["transition"] = serde_json::json!(self.transition());
        }
        Ok(response)
    }

    pub(crate) fn runtime_observation(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        let mut response =
            crate::native_dispatch::respond_to_runtime_observation(&mut self.coordinator, request)?;
        if response["ok"] == true && request["method"] == "runtime.observation" {
            response["result"]["transition"] = serde_json::json!(self.transition());
        }
        Ok(response)
    }

    pub(crate) fn traffic(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_traffic(&mut self.coordinator, request)
    }
    pub(crate) fn connections(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_connections(&mut self.coordinator, request)
    }

    pub(crate) fn diagnostic_snapshot(&mut self) -> Result<Vec<String>, NativeOwnerError> {
        self.coordinator.diagnostic_snapshot()
    }

    pub(crate) fn route_plan(
        &mut self,
        request: &Value,
    ) -> Result<crate::route_probe::Plan, NativeOwnerError> {
        self.coordinator.route_plan(request)
    }

    pub(crate) fn ping_plan(
        &mut self,
        request: &Value,
        deadline: std::time::Instant,
    ) -> Result<crate::tun_ping::Context, NativeOwnerError> {
        self.coordinator.ping_plan(request, deadline)
    }

    pub(crate) fn check_route(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_route_check(&mut self.coordinator, request)
    }

    pub(crate) fn profile_edit_input(&mut self, request: &Value) -> Result<Value, ProtocolError> {
        crate::native_dispatch::respond_to_profile_edit_input(&mut self.coordinator, request)
    }

    pub(crate) fn respond_to_fetched_subscription<G, N>(
        &mut self,
        request: &Value,
        preflight_revision: u64,
        fetched: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
        next_record_id: G,
        now_millis: N,
    ) -> Result<Value, ProtocolError>
    where
        G: FnMut() -> String,
        N: FnOnce() -> u64,
    {
        respond_to_fetched_subscription(
            &mut self.coordinator,
            request,
            preflight_revision,
            fetched,
            next_record_id,
            now_millis,
        )
    }

    pub(crate) fn respond_to_fetched_subscription_refresh<G, N>(
        &mut self,
        request: &Value,
        preflight_revision: u64,
        prepared: PreparedSubscriptionRefresh,
        fetched: Result<PrivateSubscriptionBody, SubscriptionTransportError>,
        next_record_id: G,
        now_millis: N,
    ) -> Result<Value, ProtocolError>
    where
        G: FnMut() -> String,
        N: FnOnce() -> u64,
    {
        respond_to_fetched_subscription_refresh(
            &mut self.coordinator,
            request,
            preflight_revision,
            prepared,
            fetched,
            next_record_id,
            now_millis,
        )
    }
}

impl ProductionNativeOwner<NativeLifecycleHost> {
    /// Resolve only package-fixed/current-user paths and construct the native
    /// owner. A legacy, preparing, rollback, missing, malformed, or unsafe
    /// marker fails closed before reconciliation can touch lifecycle state.
    pub fn current(runtime_paths: &RuntimePaths) -> Result<Self, ProductionOwnerError> {
        let uid = Uid::current().as_raw();
        let desired_paths =
            DesiredPaths::current().map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let cutover_paths =
            CutoverPaths::current(uid).map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let lock = MigrationLock::acquire(&cutover_paths, uid).map_err(lock_error)?;
        let marker = read_marker(&cutover_paths, uid)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        if marker.phase() != OwnershipPhase::Rust {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        crate::login_activation::require_current_receipt(
            &cutover_paths,
            uid,
            &lock,
            marker.generation(),
        )
        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        check_startup_receipt(&cutover_paths, uid, &lock, Some(marker.generation()))
            .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        let host_paths = NativeHostPaths::current(&runtime_paths.directory)
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let store_path = host_paths.store.clone();
        let host = NativeLifecycleHost::new(host_paths, uid)
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        host.cleanup_probe_orphans()
            .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        let mut owner =
            Self::initialize_locked(host, desired_paths, &store_path, cutover_paths, uid, lock)?;
        owner.login_ready = crate::login_activation::startup_configuration_available();
        Ok(owner)
    }

    /// Construct a transition candidate only through the package-fixed current
    /// paths. The caller supplies no path, command, service, or phase value.
    pub(crate) fn candidate_current(
        runtime_paths: &RuntimePaths,
        preparing_generation: u64,
    ) -> Result<Self, ProductionOwnerError> {
        let uid = Uid::current().as_raw();
        let desired_paths =
            DesiredPaths::current().map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let cutover_paths =
            CutoverPaths::current(uid).map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let lock = MigrationLock::acquire(&cutover_paths, uid).map_err(lock_error)?;
        let marker = read_marker(&cutover_paths, uid)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        let bootstrap = TransitionBootstrap::from_preparing(&marker)
            .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
        if bootstrap.preparing_generation() != preparing_generation {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        check_startup_receipt(&cutover_paths, uid, &lock, None)
            .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
        let host_paths = NativeHostPaths::current(&runtime_paths.directory)
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let store_path = host_paths.store.clone();
        let host = NativeLifecycleHost::new(host_paths, uid)
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        Self::initialize_candidate_locked(
            host,
            desired_paths,
            &store_path,
            cutover_paths,
            uid,
            bootstrap,
            lock,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::{DesiredState, OwnedObservation, write_desired};
    use crate::lifecycle::HostStepError;
    use serde_json::json;
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::rc::Rc;

    struct FakeHost {
        observation: OwnedObservation,
        local_observation: Option<crate::lifecycle::NativeLocalObservation>,
        calls: usize,
        lock_check: Option<(CutoverPaths, u32)>,
        lock_was_held: bool,
        observed_calls: Rc<Cell<usize>>,
        successor_after_observe: Option<(PathBuf, usize)>,
    }

    impl FakeHost {
        fn called(&mut self) {
            self.calls += 1;
            self.observed_calls.set(self.observed_calls.get() + 1);
        }
    }

    impl LifecycleHost for FakeHost {
        fn fresh_observation(
            &mut self,
            _desired: &DesiredState,
        ) -> Result<crate::lifecycle::NativeLocalObservation, HostStepError> {
            self.called();
            if let Some((path, count)) = &self.successor_after_observe
                && self.calls == *count
            {
                fs::write(path, b"synthetic late successor").unwrap();
            }
            Ok(self
                .local_observation
                .unwrap_or(crate::lifecycle::NativeLocalObservation {
                    owned_core_running: self.observation.core_count != 0,
                    visible_mihomo_count: self.observation.core_count,
                    owned_auxiliary_mihomo_count: 0,
                    visible_tun_count: self.observation.tun_count,
                    managed_tun_count: self.observation.tun_count,
                    owned_controller_config_verified: self.observation.controller_ready,
                    desired_profile_matches_owned: self.observation.active_profile_matches,
                }))
        }

        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.called();
            if let Some((paths, uid)) = self.lock_check.as_ref() {
                self.lock_was_held =
                    matches!(MigrationLock::acquire(paths, *uid), Err(CutoverError::Busy));
            }
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.called();
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.called();
            Ok(())
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.called();
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.called();
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.called();
            Ok(())
        }
    }

    struct Fixture {
        root: PathBuf,
        uid: u32,
        desired: DesiredPaths,
        cutover: CutoverPaths,
        store: PathBuf,
    }

    impl Fixture {
        fn new(phase: OwnershipPhase) -> Self {
            let root = crate::test_temp::directory("production-owner").unwrap();
            Self::at(root, phase)
        }

        fn new_private(phase: OwnershipPhase) -> Self {
            let home = std::env::var_os("HOME").expect("restore review needs private home");
            let root =
                crate::test_temp::directory_under(Path::new(&home), "production-owner-private")
                    .unwrap();
            Self::at(root, phase)
        }

        fn at(root: PathBuf, phase: OwnershipPhase) -> Self {
            let runtime = root.join("runtime");
            let state = root.join("state");
            let config = root.join("config");
            for path in [&root, &runtime, &state, &config] {
                fs::create_dir_all(path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            let desired = DesiredPaths::below(&state);
            write_desired(&desired, uid, &DesiredState::default()).unwrap();
            let cutover = CutoverPaths::below(&runtime, &state, uid);
            let marker = json!({
                "schemaVersion": 1,
                "generation": 1,
                "phase": phase.as_str(),
            });
            fs::write(
                &cutover.ownership_marker,
                serde_json::to_vec(&marker).unwrap(),
            )
            .unwrap();
            fs::set_permissions(&cutover.ownership_marker, fs::Permissions::from_mode(0o600))
                .unwrap();
            let store = config.join("profiles.json");
            fs::write(
                &store,
                br#"{"version":3,"activeId":"","lastId":"","profiles":[],"subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"onboardingComplete":false}"#,
            )
            .unwrap();
            fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
            Self {
                root,
                uid,
                desired,
                cutover,
                store,
            }
        }

        fn host(&self) -> FakeHost {
            FakeHost {
                observation: OwnedObservation {
                    service_active: false,
                    controller_ready: false,
                    core_count: 0,
                    tun_count: 0,
                    active_profile_matches: false,
                },
                local_observation: None,
                calls: 0,
                lock_check: Some((self.cutover.clone(), self.uid)),
                lock_was_held: false,
                observed_calls: Rc::new(Cell::new(0)),
                successor_after_observe: None,
            }
        }

        fn write_marker(&self, phase: OwnershipPhase, generation: u64) {
            let marker = json!({
                "schemaVersion": 1,
                "generation": generation,
                "phase": phase.as_str(),
            });
            fs::write(
                &self.cutover.ownership_marker,
                serde_json::to_vec(&marker).unwrap(),
            )
            .unwrap();
            fs::set_permissions(
                &self.cutover.ownership_marker,
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn committed_rust_owner_reconciles_under_the_shared_lock() {
        let fixture = Fixture::new(OwnershipPhase::Rust);
        let owner = ProductionNativeOwner::initialize(
            fixture.host(),
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
        )
        .unwrap();
        assert_eq!(owner.actual(), ActualState::Disconnected);
        assert_eq!(owner.revision(), 0);
        assert!(!owner.startup_outcome().changed);
        assert_eq!(owner.coordinator.host().calls, 1);
        assert!(owner.coordinator.host().lock_was_held);
    }

    #[test]
    fn restore_restart_review_is_read_only_and_never_constructs_a_normal_owner() {
        use crate::restore_decision_candidate::{DecisionRecord, TerminalChoice};
        use crate::restore_staging_candidate::{inspect_stage_identity, stage_private_pair};

        for phase in [
            "stage",
            "intent",
            "commit",
            "abort",
            "receipt",
            "receipt-completion",
            "receipt-completion-corrupt",
            "completion",
            "completion-corrupt",
        ] {
            let fixture = Fixture::new_private(OwnershipPhase::Rust);
            let config = fixture.store.parent().unwrap();
            let template = config.join("route-template.yaml");
            fs::write(&template, b"old synthetic template").unwrap();
            fs::set_permissions(&template, fs::Permissions::from_mode(0o600)).unwrap();
            let old_store = fs::read(&fixture.store).unwrap();
            let new_store = b"new synthetic store";
            stage_private_pair(
                &fixture.cutover.state_directory,
                fixture.uid,
                &old_store,
                b"old synthetic template",
                new_store,
                b"new synthetic template",
            )
            .unwrap();
            if phase != "stage" {
                let identity =
                    inspect_stage_identity(&fixture.cutover.state_directory, fixture.uid).unwrap();
                let desired = fs::read(&fixture.desired.file).unwrap();
                let intent = DecisionRecord::intent(1, Some(&desired), &identity, [7; 16]).unwrap();
                let intent_path = fixture
                    .cutover
                    .state_directory
                    .join("restore-decision.intent");
                fs::write(&intent_path, intent.encode()).unwrap();
                fs::set_permissions(&intent_path, fs::Permissions::from_mode(0o600)).unwrap();
                if matches!(
                    phase,
                    "commit"
                        | "abort"
                        | "receipt"
                        | "receipt-completion"
                        | "receipt-completion-corrupt"
                        | "completion"
                        | "completion-corrupt"
                ) {
                    let choice = if phase != "abort" {
                        TerminalChoice::Commit
                    } else {
                        TerminalChoice::Abort
                    };
                    let terminal = intent.terminal(choice).unwrap();
                    let path = fixture
                        .cutover
                        .state_directory
                        .join("restore-decision.terminal");
                    fs::write(&path, terminal.encode()).unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                    if phase != "abort" {
                        fs::write(&fixture.store, new_store).unwrap();
                        fs::write(&template, b"new synthetic template").unwrap();
                    }
                    if matches!(
                        phase,
                        "receipt"
                            | "receipt-completion"
                            | "receipt-completion-corrupt"
                            | "completion"
                            | "completion-corrupt"
                    ) {
                        let lock = MigrationLock::acquire(&fixture.cutover, fixture.uid).unwrap();
                        assert_eq!(
                            crate::restore_retirement_candidate::publish_retirement_receipt(
                                config,
                                &fixture.cutover,
                                fixture.uid,
                                1,
                                &lock,
                                || true,
                            ),
                            Ok(crate::restore_executor_candidate::PendingOutcome::Committed)
                        );
                        if matches!(
                            phase,
                            "receipt-completion"
                                | "receipt-completion-corrupt"
                                | "completion"
                                | "completion-corrupt"
                        ) {
                            crate::restore_cleanup_candidate::retire_fixed_restore_artifacts(
                                config,
                                &fixture.cutover,
                                fixture.uid,
                                1,
                                &lock,
                                || true,
                            )
                            .unwrap();
                            crate::restore_cleanup_candidate::publish_completion_record(
                                config,
                                &fixture.cutover,
                                fixture.uid,
                                1,
                                &lock,
                                || true,
                            )
                            .unwrap();
                            if matches!(phase, "completion" | "completion-corrupt") {
                                crate::restore_cleanup_candidate::finalize_fenced_restore(
                                    config,
                                    &fixture.cutover,
                                    fixture.uid,
                                    1,
                                    &lock,
                                    || true,
                                )
                                .unwrap();
                            }
                            if phase.ends_with("corrupt") {
                                let completion = fixture
                                    .cutover
                                    .state_directory
                                    .join(crate::restore_closure_model::CLOSURE_MEMBER);
                                fs::write(&completion, b"torn synthetic completion").unwrap();
                            }
                        }
                    }
                }
            }
            let before = fs::read(&fixture.store).unwrap();
            let host = fixture.host();
            let calls = host.observed_calls.clone();
            assert!(matches!(
                ProductionNativeOwner::initialize(
                    fixture.host(),
                    fixture.desired.clone(),
                    &fixture.store,
                    fixture.cutover.clone(),
                    fixture.uid,
                ),
                Err(ProductionOwnerError::ManualRecoveryRequired)
            ));
            if phase.ends_with("corrupt") {
                assert_eq!(
                    ProductionNativeOwner::review_restore_startup(
                        host,
                        fixture.desired.clone(),
                        &fixture.store,
                        fixture.cutover.clone(),
                        fixture.uid,
                    ),
                    Err(ProductionOwnerError::ManualRecoveryRequired),
                );
                assert_eq!(
                    calls.get(),
                    1,
                    "corrupt evidence never gets final admission"
                );
                assert_eq!(fs::read(&fixture.store).unwrap(), before);
                continue;
            }
            let expected = match phase {
                "stage" => RestoreStartupReview::ManualRecovery,
                "intent" => RestoreStartupReview::Undecided,
                "commit" => RestoreStartupReview::VerifyCommitted,
                "abort" => RestoreStartupReview::VerifyAborted,
                "receipt" => RestoreStartupReview::FinalReceipt,
                "receipt-completion" => RestoreStartupReview::FinalReceipt,
                "completion" => RestoreStartupReview::VerifyCompleted,
                _ => unreachable!(),
            };
            assert_eq!(
                ProductionNativeOwner::review_restore_startup(
                    host,
                    fixture.desired.clone(),
                    &fixture.store,
                    fixture.cutover.clone(),
                    fixture.uid,
                ),
                Ok(expected),
            );
            assert_eq!(calls.get(), 2, "read-only host observations only");
            assert_eq!(fs::read(&fixture.store).unwrap(), before);
            if phase == "completion" {
                let successor = fixture
                    .cutover
                    .state_directory
                    .join(crate::restore_successor_handoff_model::SUCCESSOR_MEMBER);
                for late in [false, true] {
                    let mut host = fixture.host();
                    let calls = host.observed_calls.clone();
                    if late {
                        host.successor_after_observe = Some((successor.clone(), 2));
                    } else {
                        // Unsafe type still fences before any host observation.
                        symlink("missing-synthetic", &successor).unwrap();
                    }
                    assert_eq!(
                        ProductionNativeOwner::review_restore_startup(
                            host,
                            fixture.desired.clone(),
                            &fixture.store,
                            fixture.cutover.clone(),
                            fixture.uid,
                        ),
                        Err(ProductionOwnerError::ManualRecoveryRequired),
                    );
                    assert_eq!(calls.get(), if late { 2 } else { 0 });
                    assert_eq!(fs::read(&fixture.store).unwrap(), before);
                    fs::remove_file(&successor).unwrap();
                }
            }
        }
    }

    #[test]
    fn restore_restart_review_refuses_connected_or_owned_host_and_ignores_foreign_visibility() {
        let fixture = Fixture::new_private(OwnershipPhase::Rust);
        drop(MigrationLock::acquire(&fixture.cutover, fixture.uid).unwrap());
        fs::write(
            fixture
                .cutover
                .state_directory
                .join("restore-decision.intent"),
            b"incomplete synthetic journal",
        )
        .unwrap();
        let host = fixture.host();
        let calls = host.observed_calls.clone();
        let connected = DesiredState {
            connected: true,
            profile_id: "synthetic".into(),
            ..DesiredState::default()
        };
        write_desired(&fixture.desired, fixture.uid, &connected).unwrap();
        assert_eq!(
            ProductionNativeOwner::review_restore_startup(
                host,
                fixture.desired.clone(),
                &fixture.store,
                fixture.cutover.clone(),
                fixture.uid,
            ),
            Err(ProductionOwnerError::ManualRecoveryRequired)
        );
        assert_eq!(calls.get(), 0);
        write_desired(&fixture.desired, fixture.uid, &DesiredState::default()).unwrap();
        let mut foreign = fixture.host();
        foreign.local_observation = Some(crate::lifecycle::NativeLocalObservation {
            owned_core_running: false,
            visible_mihomo_count: 1,
            owned_auxiliary_mihomo_count: 0,
            visible_tun_count: 1,
            managed_tun_count: 0,
            owned_controller_config_verified: false,
            desired_profile_matches_owned: false,
        });
        assert_eq!(
            ProductionNativeOwner::review_restore_startup(
                foreign,
                fixture.desired.clone(),
                &fixture.store,
                fixture.cutover.clone(),
                fixture.uid,
            ),
            Ok(RestoreStartupReview::ManualRecovery)
        );
        let mut owned = fixture.host();
        owned.local_observation = Some(crate::lifecycle::NativeLocalObservation {
            owned_core_running: true,
            visible_mihomo_count: 1,
            owned_auxiliary_mihomo_count: 0,
            visible_tun_count: 1,
            managed_tun_count: 1,
            owned_controller_config_verified: false,
            desired_profile_matches_owned: false,
        });
        assert_eq!(
            ProductionNativeOwner::review_restore_startup(
                owned,
                fixture.desired.clone(),
                &fixture.store,
                fixture.cutover.clone(),
                fixture.uid,
            ),
            Err(ProductionOwnerError::ManualRecoveryRequired)
        );
    }

    #[test]
    fn restore_restart_review_never_creates_a_missing_operation_lock() {
        let fixture = Fixture::new_private(OwnershipPhase::Rust);
        let path = &fixture.cutover.operation_lock;
        assert!(!path.exists());
        fs::write(
            fixture
                .cutover
                .state_directory
                .join("restore-decision.intent"),
            b"incomplete synthetic journal",
        )
        .unwrap();
        let host = fixture.host();
        let calls = host.observed_calls.clone();
        assert_eq!(
            ProductionNativeOwner::review_restore_startup(
                host,
                fixture.desired.clone(),
                &fixture.store,
                fixture.cutover.clone(),
                fixture.uid,
            ),
            Err(ProductionOwnerError::OwnershipUnavailable)
        );
        assert!(!path.exists());
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn consumed_matching_receipt_allows_recovery_without_login_replay() {
        let fixture = Fixture::new(OwnershipPhase::Rust);
        let receipt = fixture.cutover.runtime_base.join("omavless-login.receipt");
        let payload = json!({"schemaVersion":1,"epochHash":"a".repeat(64),
            "ownershipGeneration":1,"phase":"consumed"})
        .to_string();
        omavless_store::atomic_replace_private(&receipt, payload.as_bytes(), fixture.uid).unwrap();
        let before = fs::read(&fixture.desired.file).unwrap();
        let owner = ProductionNativeOwner::initialize(
            fixture.host(),
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
        )
        .unwrap();
        assert_eq!(owner.actual(), ActualState::Disconnected);
        assert_eq!(owner.coordinator.host().calls, 1);
        assert_eq!(fs::read(&fixture.desired.file).unwrap(), before);
        assert_eq!(fs::read_to_string(receipt).unwrap(), payload);
    }

    #[test]
    fn unsafe_receipts_block_committed_and_candidate_before_any_host_call() {
        for candidate in [false, true] {
            // A consumed generation matching a committed owner is still invalid
            // for a preparing candidate; test it separately below.
            for kind in [
                "pending",
                "stale",
                "malformed",
                "duplicate",
                "oversize",
                "mode",
                "symlink",
            ] {
                let fixture = Fixture::new(if candidate {
                    OwnershipPhase::CutoverPreparing
                } else {
                    OwnershipPhase::Rust
                });
                write_desired(
                    &fixture.desired,
                    fixture.uid,
                    &DesiredState {
                        connected: true,
                        profile_id: "synthetic".to_owned(),
                        ..DesiredState::default()
                    },
                )
                .unwrap();
                let path = fixture.cutover.runtime_base.join("omavless-login.receipt");
                let raw = match kind {
                    "malformed" => "{private-fragment".to_owned(),
                    "duplicate" => format!(
                        r#"{{"schemaVersion":1,"schemaVersion":1,"epochHash":"{}","ownershipGeneration":1,"phase":"consumed"}}"#,
                        "a".repeat(64)
                    ),
                    "oversize" => "x".repeat(1025),
                    _ => json!({"schemaVersion":1,"epochHash":"a".repeat(64),
                        "ownershipGeneration":if kind == "stale" {2} else {1},
                        "phase":if kind == "pending" {"pending"} else {"consumed"}})
                    .to_string(),
                };
                if kind == "symlink" {
                    symlink(&fixture.store, &path).unwrap();
                } else {
                    omavless_store::atomic_replace_private(&path, raw.as_bytes(), fixture.uid)
                        .unwrap();
                    if kind == "mode" {
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                    }
                }
                let before = fs::read(&fixture.desired.file).unwrap();
                let store_before = fs::read(&fixture.store).unwrap();
                let receipt_before = fs::read(&path).unwrap();
                let host = fixture.host();
                let calls = host.observed_calls.clone();
                let result = if candidate {
                    ProductionNativeOwner::initialize_candidate(
                        host,
                        fixture.desired.clone(),
                        &fixture.store,
                        fixture.cutover.clone(),
                        fixture.uid,
                        1,
                    )
                } else {
                    ProductionNativeOwner::initialize(
                        host,
                        fixture.desired.clone(),
                        &fixture.store,
                        fixture.cutover.clone(),
                        fixture.uid,
                    )
                };
                assert!(
                    matches!(result, Err(ProductionOwnerError::ManualRecoveryRequired)),
                    "{candidate}/{kind}"
                );
                assert_eq!(calls.get(), 0);
                assert_eq!(fs::read(&fixture.desired.file).unwrap(), before);
                assert_eq!(fs::read(&fixture.store).unwrap(), store_before);
                assert_eq!(fs::read(&path).unwrap(), receipt_before);
            }
        }
    }

    #[test]
    fn candidate_refuses_even_valid_consumed_receipt() {
        let fixture = Fixture::new(OwnershipPhase::CutoverPreparing);
        let path = fixture.cutover.runtime_base.join("omavless-login.receipt");
        let raw = json!({"schemaVersion":1,"epochHash":"a".repeat(64),"ownershipGeneration":1,"phase":"consumed"}).to_string();
        omavless_store::atomic_replace_private(&path, raw.as_bytes(), fixture.uid).unwrap();
        let host = fixture.host();
        let calls = host.observed_calls.clone();
        let before = fs::read(&fixture.desired.file).unwrap();
        let result = ProductionNativeOwner::initialize_candidate(
            host,
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
            1,
        );
        assert!(matches!(
            result,
            Err(ProductionOwnerError::ManualRecoveryRequired)
        ));
        assert_eq!(calls.get(), 0);
        assert_eq!(fs::read(&fixture.desired.file).unwrap(), before);
    }

    #[test]
    fn every_non_rust_phase_fails_before_host_observation() {
        for phase in [
            OwnershipPhase::Legacy,
            OwnershipPhase::CutoverPreparing,
            OwnershipPhase::RollbackPreparing,
        ] {
            let fixture = Fixture::new(phase);
            let result = ProductionNativeOwner::initialize(
                fixture.host(),
                fixture.desired.clone(),
                &fixture.store,
                fixture.cutover.clone(),
                fixture.uid,
            );
            assert!(matches!(
                result,
                Err(ProductionOwnerError::OwnershipUnavailable)
            ));
        }
    }

    #[test]
    fn malformed_or_unsafe_marker_fails_closed_without_observation() {
        let fixture = Fixture::new(OwnershipPhase::Rust);
        fs::write(&fixture.cutover.ownership_marker, b"{private-broken}").unwrap();
        let result = ProductionNativeOwner::initialize(
            fixture.host(),
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("malformed marker constructed a native owner"),
        };
        assert_eq!(error, ProductionOwnerError::OwnershipUnavailable);
        assert!(!format!("{error}").contains("private-broken"));
    }

    #[test]
    fn candidate_reconciles_read_only_then_promotes_only_to_exact_successor() {
        let fixture = Fixture::new(OwnershipPhase::CutoverPreparing);
        let mut owner = ProductionNativeOwner::initialize_candidate(
            fixture.host(),
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
            1,
        )
        .unwrap();
        assert_eq!(owner.actual(), ActualState::Disconnected);
        assert_eq!(owner.preparing_generation(), Some(1));
        assert_eq!(owner.transition(), Some("cutoverPreparing"));
        assert!(!owner.rust_ownership_available());
        assert_eq!(owner.coordinator.host().calls, 1);

        fixture.write_marker(OwnershipPhase::Rust, 2);
        assert!(owner.rust_ownership_available());
        assert_eq!(owner.preparing_generation(), None);
        assert_eq!(owner.transition(), None);

        fixture.write_marker(OwnershipPhase::RollbackPreparing, 3);
        assert!(!owner.rust_ownership_available());
    }

    #[test]
    fn wrong_marker_permanently_stales_candidate() {
        let fixture = Fixture::new(OwnershipPhase::CutoverPreparing);
        let mut owner = ProductionNativeOwner::initialize_candidate(
            fixture.host(),
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
            1,
        )
        .unwrap();

        fixture.write_marker(OwnershipPhase::Legacy, 2);
        assert!(!owner.rust_ownership_available());
        assert_eq!(owner.transition(), Some("staleCandidate"));
        fixture.write_marker(OwnershipPhase::Rust, 2);
        assert!(!owner.rust_ownership_available());
    }

    #[test]
    fn busy_promotion_is_retryable_but_wrong_generation_is_not() {
        let fixture = Fixture::new(OwnershipPhase::CutoverPreparing);
        let mut owner = ProductionNativeOwner::initialize_candidate(
            fixture.host(),
            fixture.desired.clone(),
            &fixture.store,
            fixture.cutover.clone(),
            fixture.uid,
            1,
        )
        .unwrap();
        fixture.write_marker(OwnershipPhase::Rust, 2);
        let lock = MigrationLock::acquire(&fixture.cutover, fixture.uid).unwrap();
        assert!(!owner.rust_ownership_available());
        assert_eq!(owner.transition(), Some("cutoverPreparing"));
        drop(lock);
        assert!(owner.rust_ownership_available());

        let stale_fixture = Fixture::new(OwnershipPhase::CutoverPreparing);
        let mut stale = ProductionNativeOwner::initialize_candidate(
            stale_fixture.host(),
            stale_fixture.desired.clone(),
            &stale_fixture.store,
            stale_fixture.cutover.clone(),
            stale_fixture.uid,
            1,
        )
        .unwrap();
        stale_fixture.write_marker(OwnershipPhase::Rust, 3);
        assert!(!stale.rust_ownership_available());
        stale_fixture.write_marker(OwnershipPhase::Rust, 2);
        assert!(!stale.rust_ownership_available());
    }
}
