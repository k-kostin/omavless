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
};
use crate::desired::DesiredPaths;
use crate::lifecycle::{ActualState, LifecycleHost};
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
        if !read_marker(&cutover_paths, uid)
            .is_ok_and(|marker| marker.phase() == OwnershipPhase::Rust)
        {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        let host_paths = NativeHostPaths::current(&runtime_paths.directory)
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        let store_path = host_paths.store.clone();
        let host = NativeLifecycleHost::new(host_paths, uid)
            .map_err(|_| ProductionOwnerError::HostUnavailable)?;
        Self::initialize_locked(host, desired_paths, &store_path, cutover_paths, uid, lock)
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
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeHost {
        observation: OwnedObservation,
        calls: usize,
        lock_check: Option<(CutoverPaths, u32)>,
        lock_was_held: bool,
    }

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.calls += 1;
            if let Some((paths, uid)) = self.lock_check.as_ref() {
                self.lock_was_held =
                    matches!(MigrationLock::acquire(paths, *uid), Err(CutoverError::Busy));
            }
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
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
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "omavless-production-owner-{}-{nonce}",
                std::process::id()
            ));
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
                calls: 0,
                lock_check: Some((self.cutover.clone(), self.uid)),
                lock_was_held: false,
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
