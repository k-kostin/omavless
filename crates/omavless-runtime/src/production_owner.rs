// SPDX-License-Identifier: MIT

//! Production construction boundary for the native runtime owner.
//!
//! Construction is allowed only while the shared migration lock proves a
//! committed `rust` ownership marker. Startup reconciliation completes under
//! that same lease before the owner can be returned. This module deliberately
//! does not register mutation methods with [`RuntimeServer`](crate::RuntimeServer);
//! socket dispatch is the next bounded cutover checkpoint.

use crate::RuntimePaths;
use crate::connection_transaction::{ConnectionTransactionError, ConnectionTransactionOutcome};
use crate::cutover::{CutoverError, CutoverPaths, MigrationLock, OwnershipPhase, read_marker};
use crate::desired::DesiredPaths;
use crate::lifecycle::{ActualState, LifecycleHost};
use crate::native_coordinator::OfflineNativeCoordinator;
use crate::native_host::{NativeHostPaths, NativeLifecycleHost};
use nix::unistd::Uid;
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
        let owned = read_marker(&cutover_paths, uid)
            .is_ok_and(|marker| marker.phase() == OwnershipPhase::Rust);
        if !owned {
            return Err(ProductionOwnerError::OwnershipUnavailable);
        }
        let mut coordinator = OfflineNativeCoordinator::new_ownership_gated(
            host,
            desired_paths,
            store_path,
            cutover_paths,
            uid,
        );
        let startup = coordinator
            .reconcile_startup_locked(&lock)
            .map_err(recovery_error)?;
        drop(lock);
        Ok(Self {
            coordinator,
            startup,
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
}
