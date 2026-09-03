// SPDX-License-Identifier: MIT

//! Offline owner-bound connection transactions with compatibility-pointer sync.
//!
//! This layer composes the accepted mutation coordinator, shared migration
//! lock, lifecycle executor and exact private-store transaction. It never
//! registers itself; only the committed production owner may expose it.

use crate::cutover::{CutoverError, CutoverPaths, MigrationLock, OwnershipPhase, read_marker};
use crate::desired::{DesiredPaths, DesiredState, read_desired};
use crate::lifecycle::{ActualState, LifecycleError, LifecycleExecutor, LifecycleHost};
use crate::mutation::{
    BeginOutcome, CachedOutcome, CoordinatorError, MutationCoordinator, MutationRequest,
    MutationResult, SubmitOutcome,
};
use crate::owner::{OwnerAction, OwnerRequest};
use crate::private_store_transaction::{
    PreparedWrite, PrivateStoreWriteError, prepare_pointer_mutation,
};
use omavless_control_protocol::StableErrorCode;
use omavless_domain::private_store::CompatibilityPointerTarget;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionTransactionError {
    Busy,
    NotFound,
    InvalidArgument,
    Conflict,
    Store,
    TransitionFailedRestored,
    RecoveryFailed,
    ManualRecoveryRequired,
}

impl ConnectionTransactionError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Busy => StableErrorCode::Busy,
            Self::NotFound => StableErrorCode::NotFound,
            Self::InvalidArgument => StableErrorCode::InvalidArgument,
            Self::Conflict => StableErrorCode::Conflict,
            Self::Store => StableErrorCode::InternalError,
            Self::TransitionFailedRestored => StableErrorCode::TransitionFailedRestored,
            Self::RecoveryFailed => StableErrorCode::CoreRejected,
            Self::ManualRecoveryRequired => StableErrorCode::ManualRecoveryRequired,
        }
    }
}

impl fmt::Display for ConnectionTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "Another OmaVLESS operation is active",
            Self::NotFound => "Requested profile was not found",
            Self::InvalidArgument => "Connection request is invalid",
            Self::Conflict => "Connection state changed concurrently",
            Self::Store => "Connection metadata update failed",
            Self::TransitionFailedRestored => "Connection transition failed and was restored",
            Self::RecoveryFailed => "Connection recovery failed",
            Self::ManualRecoveryRequired => "Manual recovery is required",
        })
    }
}

impl std::error::Error for ConnectionTransactionError {}

fn lock_error(error: CutoverError) -> ConnectionTransactionError {
    match error {
        CutoverError::Busy => ConnectionTransactionError::Busy,
        _ => ConnectionTransactionError::Store,
    }
}

fn store_error(error: PrivateStoreWriteError) -> ConnectionTransactionError {
    match error {
        PrivateStoreWriteError::StoreChanged => ConnectionTransactionError::Conflict,
        PrivateStoreWriteError::Mutation(
            omavless_domain::private_store::PrivateStoreError::ProfileNotFound,
        ) => ConnectionTransactionError::NotFound,
        PrivateStoreWriteError::UnsafeStore
        | PrivateStoreWriteError::StoreIo
        | PrivateStoreWriteError::Mutation(_)
        | PrivateStoreWriteError::LockMismatch => ConnectionTransactionError::Store,
    }
}

fn lifecycle_error(error: LifecycleError) -> ConnectionTransactionError {
    match error {
        LifecycleError::InvalidRequest => ConnectionTransactionError::InvalidArgument,
        LifecycleError::State => ConnectionTransactionError::Store,
        LifecycleError::TransitionFailedRestored => {
            ConnectionTransactionError::TransitionFailedRestored
        }
        LifecycleError::RecoveryFailed => ConnectionTransactionError::RecoveryFailed,
        LifecycleError::ManualRecoveryRequired => {
            ConnectionTransactionError::ManualRecoveryRequired
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionTransactionOutcome {
    pub changed: bool,
    pub pruned: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionOwnerExecution {
    Applied {
        cached: CachedOutcome,
        outcome: Result<ConnectionTransactionOutcome, ConnectionTransactionError>,
    },
    UncachedPreflightFailure {
        revision: u64,
        error: ConnectionTransactionError,
    },
    Replay(CachedOutcome),
    Rejected(CachedOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionOwnerError {
    Coordinator(CoordinatorError),
    Invariant,
}

impl fmt::Display for ConnectionOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Coordinator(_) => "Connection mutation scheduling failed",
            Self::Invariant => "Connection owner invariant failed",
        })
    }
}

impl std::error::Error for ConnectionOwnerError {}

impl From<CoordinatorError> for ConnectionOwnerError {
    fn from(value: CoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}

pub(crate) enum Completion {
    Ordinary(Result<ConnectionTransactionOutcome, ConnectionTransactionError>),
    CommittedFailure(ConnectionTransactionError),
}

pub(crate) struct ConnectionTransactionState<H> {
    lifecycle: LifecycleExecutor<H>,
    desired_paths: DesiredPaths,
    store_path: PathBuf,
    cutover_paths: CutoverPaths,
    uid: u32,
    blocked: bool,
}

impl<H: LifecycleHost> ConnectionTransactionState<H> {
    pub(crate) fn new(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
        uid: u32,
    ) -> Self {
        Self {
            lifecycle: LifecycleExecutor::new(host, desired_paths.clone(), uid),
            desired_paths,
            store_path: store_path.to_path_buf(),
            cutover_paths,
            uid,
            blocked: false,
        }
    }

    pub(crate) const fn actual(&self) -> ActualState {
        self.lifecycle.actual()
    }

    pub(crate) const fn host(&self) -> &H {
        self.lifecycle.host()
    }

    pub(crate) fn host_mut(&mut self) -> &mut H {
        self.lifecycle.host_mut()
    }

    pub(crate) fn lifecycle_mut(&mut self) -> &mut LifecycleExecutor<H> {
        &mut self.lifecycle
    }

    pub(crate) fn store_path(&self) -> &Path {
        &self.store_path
    }

    pub(crate) const fn cutover_paths(&self) -> &CutoverPaths {
        &self.cutover_paths
    }

    pub(crate) const fn uid(&self) -> u32 {
        self.uid
    }

    pub(crate) fn desired(&self) -> Result<DesiredState, ConnectionTransactionError> {
        read_desired(&self.desired_paths, self.uid).map_err(|_| ConnectionTransactionError::Store)
    }

    /// Recheck durable native ownership while holding the same migration lock
    /// used by mutation admission. Capability/status replies must never claim
    /// a native owner during a concurrent cutover or rollback phase.
    pub(crate) fn rust_ownership_available(&self) -> bool {
        let Ok(lock) = self.acquire_lock() else {
            return false;
        };
        let available = read_marker(&self.cutover_paths, self.uid)
            .is_ok_and(|marker| marker.phase() == OwnershipPhase::Rust);
        drop(lock);
        available
    }

    pub(crate) fn acquire_lock(&self) -> Result<MigrationLock, ConnectionTransactionError> {
        MigrationLock::acquire(&self.cutover_paths, self.uid).map_err(lock_error)
    }

    pub(crate) const fn blocked(&self) -> bool {
        self.blocked
    }

    pub(crate) fn block(&mut self) {
        self.blocked = true;
    }

    /// Execute one bounded restart reconciliation and then repair legacy
    /// pointers from desired state plus verified owned-host truth.
    pub(crate) fn reconcile_startup(
        &mut self,
    ) -> Result<ConnectionTransactionOutcome, ConnectionTransactionError> {
        if self.blocked {
            return Err(ConnectionTransactionError::ManualRecoveryRequired);
        }
        let lock = MigrationLock::acquire(&self.cutover_paths, self.uid).map_err(lock_error)?;
        self.reconcile_startup_locked(&lock)
    }

    /// Reconcile while a trusted production constructor already owns the
    /// shared migration lease. This prevents a check/drop/reacquire window
    /// between validating the `rust` marker and touching lifecycle state.
    pub(crate) fn reconcile_startup_locked(
        &mut self,
        lock: &MigrationLock,
    ) -> Result<ConnectionTransactionOutcome, ConnectionTransactionError> {
        if self.blocked {
            return Err(ConnectionTransactionError::ManualRecoveryRequired);
        }
        let lifecycle = self.lifecycle.reconcile_startup();
        let desired = match read_desired(&self.desired_paths, self.uid) {
            Ok(desired) => desired,
            Err(_) => {
                // Reconciliation may already have changed owned host state.
                // Losing the durable authority at this boundary is never a
                // retryable metadata-only failure.
                self.blocked = true;
                return Err(ConnectionTransactionError::ManualRecoveryRequired);
            }
        };
        let (target, lifecycle_changed, deferred_error) = match lifecycle {
            Ok(outcome) if outcome.actual == ActualState::Connected && desired.connected => (
                CompatibilityPointerTarget::Connected {
                    profile_id: desired.profile_id,
                },
                outcome.changed,
                None,
            ),
            Ok(outcome) if outcome.actual == ActualState::Disconnected && !desired.connected => (
                CompatibilityPointerTarget::Disconnected {
                    prune_missing: true,
                },
                outcome.changed,
                None,
            ),
            Err(LifecycleError::RecoveryFailed) if desired.connected => (
                // Owned runtime is proven empty, but intent remains connected.
                // Clear only the compatibility active pointer so the retained
                // profile remains available for the next bounded recovery.
                CompatibilityPointerTarget::Disconnected {
                    prune_missing: false,
                },
                false,
                Some(ConnectionTransactionError::RecoveryFailed),
            ),
            Err(error) => {
                let error = lifecycle_error(error);
                if error == ConnectionTransactionError::ManualRecoveryRequired {
                    self.blocked = true;
                }
                return Err(error);
            }
            _ => {
                self.blocked = true;
                return Err(ConnectionTransactionError::ManualRecoveryRequired);
            }
        };
        let plan = match prepare_pointer_mutation(&self.store_path, self.uid, target) {
            Ok(plan) => plan,
            Err(_) if self.lifecycle.actual() == ActualState::Connected => {
                // A healthy native tunnel without a valid compatibility-store
                // target must remain owned but cannot accept more mutations.
                self.blocked = true;
                return Err(ConnectionTransactionError::ManualRecoveryRequired);
            }
            Err(error) => return Err(store_error(error)),
        };
        let write = match plan.commit_locked(lock, &self.cutover_paths) {
            Ok(write) => write,
            Err(_) if self.lifecycle.actual() == ActualState::Connected => {
                // Adoption/recovery already proved a healthy requested tunnel.
                // Pointer metadata failure must not kill it.
                self.blocked = true;
                return Err(ConnectionTransactionError::ManualRecoveryRequired);
            }
            Err(error) => return Err(store_error(error)),
        };
        if let Some(error) = deferred_error {
            return Err(error);
        }
        Ok(ConnectionTransactionOutcome {
            changed: lifecycle_changed || write == PreparedWrite::Changed,
            pruned: plan.pruned,
        })
    }

    pub(crate) fn connect(
        &mut self,
        lock: &MigrationLock,
        profile_id: String,
        mode: Option<crate::desired::RoutingMode>,
    ) -> Completion {
        let plan = match prepare_pointer_mutation(
            &self.store_path,
            self.uid,
            CompatibilityPointerTarget::Connected {
                profile_id: profile_id.clone(),
            },
        ) {
            Ok(plan) => plan,
            Err(error) => return Completion::Ordinary(Err(store_error(error))),
        };
        let lifecycle = match self.lifecycle.connect_requested(&profile_id, mode) {
            Ok(outcome) => outcome,
            Err(error) => return Completion::Ordinary(Err(lifecycle_error(error))),
        };
        match plan.commit_locked(lock, &self.cutover_paths) {
            Ok(write) => Completion::Ordinary(Ok(ConnectionTransactionOutcome {
                changed: lifecycle.changed || write == PreparedWrite::Changed,
                pruned: 0,
            })),
            Err(_) if !lifecycle.changed => {
                // Never kill a healthy connection merely because its legacy
                // compatibility pointer could not be repaired.
                Completion::Ordinary(Err(ConnectionTransactionError::ManualRecoveryRequired))
            }
            Err(_) => {
                let disconnected = self.lifecycle.disconnect().is_ok();
                let restored = plan.restore_locked(lock, &self.cutover_paths).is_ok();
                if disconnected && restored {
                    Completion::Ordinary(Err(ConnectionTransactionError::TransitionFailedRestored))
                } else {
                    Completion::Ordinary(Err(ConnectionTransactionError::ManualRecoveryRequired))
                }
            }
        }
    }

    pub(crate) fn disconnect(&mut self, lock: &MigrationLock) -> Completion {
        let plan = match prepare_pointer_mutation(
            &self.store_path,
            self.uid,
            CompatibilityPointerTarget::Disconnected {
                prune_missing: true,
            },
        ) {
            Ok(plan) => plan,
            Err(error) => return Completion::Ordinary(Err(store_error(error))),
        };
        let lifecycle = match self.lifecycle.disconnect() {
            Ok(outcome) => outcome,
            Err(error) => return Completion::Ordinary(Err(lifecycle_error(error))),
        };
        match plan.commit_locked(lock, &self.cutover_paths) {
            Ok(write) => Completion::Ordinary(Ok(ConnectionTransactionOutcome {
                changed: lifecycle.changed || write == PreparedWrite::Changed,
                pruned: plan.pruned,
            })),
            Err(_) if lifecycle.changed => {
                // Desired state and host are safely disconnected. Do not
                // reconnect merely to restore legacy presentation metadata.
                Completion::CommittedFailure(ConnectionTransactionError::Store)
            }
            Err(error) => Completion::Ordinary(Err(store_error(error))),
        }
    }
}

pub struct OfflineConnectionOwner<H> {
    coordinator: MutationCoordinator,
    transaction: ConnectionTransactionState<H>,
}

impl<H: LifecycleHost> OfflineConnectionOwner<H> {
    #[must_use]
    pub fn new(
        host: H,
        desired_paths: DesiredPaths,
        store_path: &Path,
        cutover_paths: CutoverPaths,
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
        }
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.coordinator.revision()
    }

    #[must_use]
    pub const fn actual(&self) -> ActualState {
        self.transaction.actual()
    }

    #[must_use]
    pub const fn host(&self) -> &H {
        self.transaction.host()
    }

    pub fn host_mut(&mut self) -> &mut H {
        self.transaction.host_mut()
    }

    pub fn reconcile_startup(
        &mut self,
    ) -> Result<ConnectionTransactionOutcome, ConnectionTransactionError> {
        self.transaction.reconcile_startup()
    }

    pub fn execute(
        &mut self,
        request: OwnerRequest,
    ) -> Result<ConnectionOwnerExecution, ConnectionOwnerError> {
        let (action, operation_id, expected_revision, digest) = request.into_parts();
        let scheduling = MutationRequest::new(
            action.kind(),
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match self.coordinator.submit(scheduling)? {
            SubmitOutcome::Queued { token } => token,
            SubmitOutcome::Replay(outcome) => return Ok(ConnectionOwnerExecution::Replay(outcome)),
        };
        match self.coordinator.begin_next()? {
            BeginOutcome::Started(active) if active.token == token => {}
            BeginOutcome::Rejected {
                token: rejected,
                outcome,
            } if rejected == token => return Ok(ConnectionOwnerExecution::Rejected(outcome)),
            _ => return Err(ConnectionOwnerError::Invariant),
        }

        if self.transaction.blocked() {
            let error = ConnectionTransactionError::ManualRecoveryRequired;
            let cached = self
                .coordinator
                .finish(token, MutationResult::Failure(error.stable_code()))?;
            return Ok(ConnectionOwnerExecution::Applied {
                cached,
                outcome: Err(error),
            });
        }

        let lock =
            match MigrationLock::acquire(&self.transaction.cutover_paths, self.transaction.uid) {
                Ok(lock) => lock,
                Err(error) => {
                    self.coordinator.abort_active_uncached(token)?;
                    return Ok(ConnectionOwnerExecution::UncachedPreflightFailure {
                        revision: self.coordinator.revision(),
                        error: lock_error(error),
                    });
                }
            };
        let completion = match action {
            OwnerAction::Connect { profile_id, mode } => {
                self.transaction.connect(&lock, profile_id, mode)
            }
            OwnerAction::Disconnect => self.transaction.disconnect(&lock),
        };
        let (result, outcome) = match completion {
            Completion::Ordinary(outcome) => {
                let result = match outcome {
                    Ok(value) if value.changed => MutationResult::Success,
                    Ok(_) => MutationResult::NoChange,
                    Err(error) => MutationResult::Failure(error.stable_code()),
                };
                (result, outcome)
            }
            Completion::CommittedFailure(error) => (
                MutationResult::CommittedFailure(error.stable_code()),
                Err(error),
            ),
        };
        if outcome
            .as_ref()
            .is_err_and(|error| *error == ConnectionTransactionError::ManualRecoveryRequired)
        {
            self.transaction.block();
        }
        let cached = self.coordinator.finish(token, result)?;
        if cached.error != outcome.as_ref().err().map(|error| error.stable_code()) {
            return Err(ConnectionOwnerError::Invariant);
        }
        Ok(ConnectionOwnerExecution::Applied { cached, outcome })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::{
        DesiredState, OwnedObservation, RoutingMode, read_desired, write_desired,
    };
    use crate::lifecycle::HostStepError;
    use crate::mutation::MutationDigest;
    use serde_json::{Value, json};
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    const MISSING: &str = "00000000-0000-4000-8000-000000000002";

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
        store_path: PathBuf,
        sabotage_observe: bool,
        sabotage_commit: bool,
        sabotage_stop: bool,
        fail_start: bool,
    }

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            if self.sabotage_observe {
                fs::set_permissions(&self.store_path, fs::Permissions::from_mode(0o644)).unwrap();
                self.sabotage_observe = false;
            }
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            if self.fail_start {
                return Err(HostStepError::Start);
            }
            self.observation = healthy();
            Ok(())
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            if self.sabotage_commit {
                fs::set_permissions(&self.store_path, fs::Permissions::from_mode(0o644)).unwrap();
            }
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.observation = empty();
            if self.sabotage_commit {
                fs::set_permissions(&self.store_path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            if self.sabotage_stop {
                fs::set_permissions(&self.store_path, fs::Permissions::from_mode(0o644)).unwrap();
            }
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            Ok(())
        }
    }

    fn store(active: &str, last: &str) -> Value {
        json!({
            "version": 3,
            "activeId": active,
            "lastId": last,
            "profiles": [{
                "id": PROFILE,
                "name": "Local",
                "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Local",
                "protocol": "vless",
                "favorite": false
            }, {
                "id": MISSING,
                "name": "Retained",
                "uri": "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Retained",
                "protocol": "vless",
                "subscriptionId": "10000000-0000-4000-8000-000000000001",
                "subscriptionKey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "missing": true,
                "favorite": false
            }],
            "subscriptions": [{
                "id": "10000000-0000-4000-8000-000000000001",
                "name": "Source",
                "url": "https://example.invalid/sub",
                "updatedAt": 1
            }],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": true, "target": "profile", "profileId": MISSING, "mode": "rule"},
            "onboardingComplete": true
        })
    }

    fn fixture(
        label: &str,
        connected: bool,
        active: &str,
        last: &str,
    ) -> (PathBuf, PathBuf, DesiredPaths, CutoverPaths, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-connection-transaction-{label}-{}-{nonce}",
            std::process::id()
        ));
        let config = root.join("config");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for path in [&config, &runtime, &state] {
            fs::create_dir_all(path).unwrap();
        }
        for path in [&root, &config, &runtime, &state] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let store_path = config.join("profiles.json");
        fs::write(
            &store_path,
            serde_json::to_vec(&store(active, last)).unwrap(),
        )
        .unwrap();
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let desired_paths = DesiredPaths::below(&state);
        write_desired(
            &desired_paths,
            uid,
            &DesiredState {
                schema_version: 1,
                generation: 0,
                connected,
                profile_id: if connected {
                    PROFILE.to_owned()
                } else {
                    String::new()
                },
                mode: RoutingMode::Rule,
            },
        )
        .unwrap();
        let cutover_paths = CutoverPaths::below(&runtime, &state, uid);
        (root, store_path, desired_paths, cutover_paths, uid)
    }

    fn request(action: OwnerAction, operation: &str, revision: u64) -> OwnerRequest {
        OwnerRequest::new(
            action,
            Some(operation),
            Some(revision),
            MutationDigest::new([revision as u8 + 1; 32]),
        )
    }

    fn applied(
        execution: ConnectionOwnerExecution,
    ) -> (
        CachedOutcome,
        Result<ConnectionTransactionOutcome, ConnectionTransactionError>,
    ) {
        match execution {
            ConnectionOwnerExecution::Applied { cached, outcome } => (cached, outcome),
            _ => panic!("connection mutation was not applied"),
        }
    }

    #[test]
    fn connect_commits_pointers_only_after_verified_lifecycle_success() {
        let (root, store_path, desired, cutover, uid) = fixture("connect", false, "", "");
        let host = FakeHost {
            observation: empty(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner = OfflineConnectionOwner::new(host, desired, &store_path, cutover, uid);
        let (cached, outcome) = applied(
            owner
                .execute(request(
                    OwnerAction::Connect {
                        profile_id: PROFILE.to_owned(),
                        mode: Some(RoutingMode::Global),
                    },
                    "connect-1",
                    0,
                ))
                .unwrap(),
        );
        assert_eq!(cached.revision, 1);
        assert!(outcome.unwrap().changed);
        let written: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(written["activeId"], PROFILE);
        assert_eq!(written["lastId"], PROFILE);
        assert_eq!(owner.actual(), ActualState::Connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disconnect_prunes_only_missing_rows_and_repairs_dependent_pointers() {
        let (root, store_path, desired, cutover, uid) =
            fixture("disconnect", true, PROFILE, MISSING);
        let host = FakeHost {
            observation: healthy(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner =
            OfflineConnectionOwner::new(host, desired.clone(), &store_path, cutover, uid);
        let (cached, outcome) = applied(
            owner
                .execute(request(OwnerAction::Disconnect, "disconnect-1", 0))
                .unwrap(),
        );
        let outcome = outcome.unwrap();
        assert_eq!(cached.revision, 1);
        assert_eq!(outcome.pruned, 1);
        let written: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(written["activeId"], "");
        assert_eq!(written["lastId"], PROFILE);
        assert_eq!(written["profiles"].as_array().unwrap().len(), 1);
        assert_eq!(written["startup"]["enabled"], false);
        assert!(!read_desired(&desired, uid).unwrap().connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn connect_pointer_failure_compensates_to_original_disconnected_state() {
        let (root, store_path, desired, cutover, uid) = fixture("connect-fail", false, "", "");
        let before = fs::read(&store_path).unwrap();
        let host = FakeHost {
            observation: empty(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: true,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner =
            OfflineConnectionOwner::new(host, desired.clone(), &store_path, cutover, uid);
        let (cached, outcome) = applied(
            owner
                .execute(request(
                    OwnerAction::Connect {
                        profile_id: PROFILE.to_owned(),
                        mode: None,
                    },
                    "connect-fail",
                    0,
                ))
                .unwrap(),
        );
        assert_eq!(cached.revision, 0);
        assert_eq!(
            outcome.unwrap_err(),
            ConnectionTransactionError::TransitionFailedRestored
        );
        assert_eq!(fs::read(&store_path).unwrap(), before);
        assert!(!read_desired(&desired, uid).unwrap().connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disconnect_pointer_failure_is_committed_without_reconnecting() {
        let (root, store_path, desired, cutover, uid) =
            fixture("disconnect-fail", true, PROFILE, PROFILE);
        let host = FakeHost {
            observation: healthy(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: true,
            fail_start: false,
        };
        let mut owner =
            OfflineConnectionOwner::new(host, desired.clone(), &store_path, cutover, uid);
        let (cached, outcome) = applied(
            owner
                .execute(request(OwnerAction::Disconnect, "disconnect-fail", 0))
                .unwrap(),
        );
        assert_eq!(cached.revision, 1);
        assert_eq!(cached.error, Some(StableErrorCode::InternalError));
        assert_eq!(outcome.unwrap_err(), ConnectionTransactionError::Store);
        assert!(!read_desired(&desired, uid).unwrap().connected);
        assert_eq!(owner.actual(), ActualState::Disconnected);
        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn adopted_healthy_connection_is_not_killed_when_pointer_repair_fails() {
        let (root, store_path, desired, cutover, uid) = fixture("adopt-pointer-fail", true, "", "");
        let host = FakeHost {
            observation: healthy(),
            store_path: store_path.clone(),
            sabotage_observe: true,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner =
            OfflineConnectionOwner::new(host, desired.clone(), &store_path, cutover, uid);
        let (cached, outcome) = applied(
            owner
                .execute(request(
                    OwnerAction::Connect {
                        profile_id: PROFILE.to_owned(),
                        mode: Some(RoutingMode::Rule),
                    },
                    "adopt-fail",
                    0,
                ))
                .unwrap(),
        );
        assert_eq!(cached.revision, 0);
        assert_eq!(
            outcome.unwrap_err(),
            ConnectionTransactionError::ManualRecoveryRequired
        );
        assert!(read_desired(&desired, uid).unwrap().connected);
        assert_eq!(owner.actual(), ActualState::Connected);

        fs::set_permissions(&store_path, fs::Permissions::from_mode(0o600)).unwrap();
        let (_, blocked) = applied(
            owner
                .execute(request(OwnerAction::Disconnect, "blocked", 0))
                .unwrap(),
        );
        assert_eq!(
            blocked.unwrap_err(),
            ConnectionTransactionError::ManualRecoveryRequired
        );
        assert!(read_desired(&desired, uid).unwrap().connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shared_lock_busy_is_uncached_and_exact_operation_can_retry() {
        let (root, store_path, desired, cutover, uid) = fixture("busy", false, "", "");
        let host = FakeHost {
            observation: empty(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let lock = MigrationLock::acquire(&cutover, uid).unwrap();
        let mut owner = OfflineConnectionOwner::new(host, desired, &store_path, cutover, uid);
        let first = owner
            .execute(request(
                OwnerAction::Connect {
                    profile_id: PROFILE.to_owned(),
                    mode: None,
                },
                "retry",
                0,
            ))
            .unwrap();
        assert_eq!(
            first,
            ConnectionOwnerExecution::UncachedPreflightFailure {
                revision: 0,
                error: ConnectionTransactionError::Busy,
            }
        );
        drop(lock);
        let (_, outcome) = applied(
            owner
                .execute(request(
                    OwnerAction::Connect {
                        profile_id: PROFILE.to_owned(),
                        mode: None,
                    },
                    "retry",
                    0,
                ))
                .unwrap(),
        );
        assert!(outcome.is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_adoption_repairs_connected_compatibility_pointers() {
        let (root, store_path, desired, cutover, uid) = fixture("startup-adopt", true, "", "");
        let host = FakeHost {
            observation: healthy(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner = OfflineConnectionOwner::new(host, desired, &store_path, cutover, uid);
        let outcome = owner.reconcile_startup().unwrap();
        assert!(outcome.changed);
        let written: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(written["activeId"], PROFILE);
        assert_eq!(written["lastId"], PROFILE);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_never_kills_healthy_owned_runtime_when_store_target_is_missing() {
        let (root, store_path, desired, cutover, uid) =
            fixture("startup-missing-target", true, "", "");
        write_desired(
            &desired,
            uid,
            &DesiredState {
                schema_version: 1,
                generation: 1,
                connected: true,
                profile_id: "00000000-0000-4000-8000-000000000099".to_owned(),
                mode: RoutingMode::Rule,
            },
        )
        .unwrap();
        let host = FakeHost {
            observation: healthy(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner =
            OfflineConnectionOwner::new(host, desired.clone(), &store_path, cutover, uid);
        assert_eq!(
            owner.reconcile_startup(),
            Err(ConnectionTransactionError::ManualRecoveryRequired)
        );
        assert!(read_desired(&desired, uid).unwrap().connected);
        assert_eq!(owner.actual(), ActualState::Connected);
        let (_, blocked) = applied(
            owner
                .execute(request(OwnerAction::Disconnect, "blocked-startup", 0))
                .unwrap(),
        );
        assert_eq!(
            blocked.unwrap_err(),
            ConnectionTransactionError::ManualRecoveryRequired
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_disconnected_cleanup_prunes_missing_profiles() {
        let (root, store_path, desired, cutover, uid) =
            fixture("startup-disconnected", false, PROFILE, MISSING);
        let host = FakeHost {
            observation: empty(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: false,
        };
        let mut owner = OfflineConnectionOwner::new(host, desired, &store_path, cutover, uid);
        let outcome = owner.reconcile_startup().unwrap();
        assert_eq!(outcome.pruned, 1);
        let written: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(written["activeId"], "");
        assert_eq!(written["lastId"], PROFILE);
        assert_eq!(written["profiles"].as_array().unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_startup_recovery_clears_only_active_pointer_and_preserves_intent() {
        let (root, store_path, desired, cutover, uid) =
            fixture("startup-recovery-fail", true, PROFILE, MISSING);
        let host = FakeHost {
            observation: empty(),
            store_path: store_path.clone(),
            sabotage_observe: false,
            sabotage_commit: false,
            sabotage_stop: false,
            fail_start: true,
        };
        let mut owner =
            OfflineConnectionOwner::new(host, desired.clone(), &store_path, cutover, uid);
        assert_eq!(
            owner.reconcile_startup(),
            Err(ConnectionTransactionError::RecoveryFailed)
        );
        let written: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(written["activeId"], "");
        assert_eq!(written["lastId"], MISSING);
        assert_eq!(written["profiles"].as_array().unwrap().len(), 2);
        assert!(read_desired(&desired, uid).unwrap().connected);
        assert_eq!(owner.actual(), ActualState::Failed);
        fs::remove_dir_all(root).unwrap();
    }
}
