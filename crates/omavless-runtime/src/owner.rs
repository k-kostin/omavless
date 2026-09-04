// SPDX-License-Identifier: MIT

//! Internal composition of serialized mutations and lifecycle transactions.
//!
//! This checkpoint has no control-socket dispatch path. It proves that one
//! future owner can execute method-specific actions with revision checks and
//! operation-ID replay without caching private action payloads.

use crate::desired::{DesiredPaths, RoutingMode};
use crate::lifecycle::{
    ActualState, LifecycleError, LifecycleExecutor, LifecycleHost, LifecycleOutcome,
};
use crate::mutation::{
    BeginOutcome, CachedOutcome, CoordinatorError, MutationCoordinator, MutationDigest,
    MutationKind, MutationRequest, MutationResult, SubmitOutcome,
};
use omavless_control_protocol::StableErrorCode;
use std::fmt;

/// Private action payload. Deliberately not formattable, cloneable, or
/// serializable; the mutation coordinator receives only scheduling metadata
/// and the caller's fixed digest.
pub enum OwnerAction {
    Connect {
        profile_id: String,
        mode: Option<RoutingMode>,
    },
    Disconnect,
    SetMode {
        mode: RoutingMode,
    },
}

impl OwnerAction {
    pub(crate) const fn kind(&self) -> MutationKind {
        match self {
            Self::Disconnect => MutationKind::Disconnect,
            Self::Connect { .. } | Self::SetMode { .. } => MutationKind::Other,
        }
    }
}

/// One already-validated semantic mutation request. Deliberately has no
/// formatting implementation because it contains the private action payload.
pub struct OwnerRequest {
    action: OwnerAction,
    operation_id: Option<String>,
    expected_revision: Option<u64>,
    digest: MutationDigest,
}

impl OwnerRequest {
    #[must_use]
    pub fn new(
        action: OwnerAction,
        operation_id: Option<&str>,
        expected_revision: Option<u64>,
        digest: MutationDigest,
    ) -> Self {
        Self {
            action,
            operation_id: operation_id.map(str::to_owned),
            expected_revision,
            digest,
        }
    }

    pub(crate) fn into_parts(self) -> (OwnerAction, Option<String>, Option<u64>, MutationDigest) {
        (
            self.action,
            self.operation_id,
            self.expected_revision,
            self.digest,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerError {
    Coordinator(CoordinatorError),
    Invariant,
}

impl OwnerError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Coordinator(error) => error.stable_code(),
            Self::Invariant => StableErrorCode::InternalError,
        }
    }
}

impl fmt::Display for OwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordinator(error) => error.fmt(formatter),
            Self::Invariant => formatter.write_str("Runtime mutation invariant failed"),
        }
    }
}

impl std::error::Error for OwnerError {}

impl From<CoordinatorError> for OwnerError {
    fn from(value: CoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerExecution {
    Applied {
        cached: CachedOutcome,
        lifecycle: Result<LifecycleOutcome, LifecycleError>,
    },
    Replay(CachedOutcome),
    Rejected(CachedOutcome),
}

pub struct OwnerEngine<H> {
    coordinator: MutationCoordinator,
    lifecycle: LifecycleExecutor<H>,
}

impl<H: LifecycleHost> OwnerEngine<H> {
    #[must_use]
    pub fn new(host: H, desired_paths: DesiredPaths, uid: u32) -> Self {
        Self {
            coordinator: MutationCoordinator::default(),
            lifecycle: LifecycleExecutor::new(host, desired_paths, uid),
        }
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.coordinator.revision()
    }

    #[must_use]
    pub const fn actual(&self) -> ActualState {
        self.lifecycle.actual()
    }

    #[must_use]
    pub const fn host(&self) -> &H {
        self.lifecycle.host()
    }

    pub fn host_mut(&mut self) -> &mut H {
        self.lifecycle.host_mut()
    }

    pub fn reconcile_startup(&mut self) -> Result<LifecycleOutcome, LifecycleError> {
        self.lifecycle.reconcile_startup()
    }

    pub fn execute(&mut self, request: OwnerRequest) -> Result<OwnerExecution, OwnerError> {
        let OwnerRequest {
            action,
            operation_id,
            expected_revision,
            digest,
        } = request;
        let mutation = MutationRequest::new(
            action.kind(),
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match self.coordinator.submit(mutation)? {
            SubmitOutcome::Queued { token } => token,
            SubmitOutcome::Replay(outcome) => return Ok(OwnerExecution::Replay(outcome)),
        };
        match self.coordinator.begin_next()? {
            BeginOutcome::Started(active) if active.token == token => {}
            BeginOutcome::Rejected {
                token: rejected,
                outcome,
            } if rejected == token => return Ok(OwnerExecution::Rejected(outcome)),
            BeginOutcome::Started(_) | BeginOutcome::Rejected { .. } | BeginOutcome::Empty => {
                return Err(OwnerError::Invariant);
            }
        }

        let lifecycle = match action {
            OwnerAction::Connect { profile_id, mode } => {
                self.lifecycle.connect_requested(&profile_id, mode)
            }
            OwnerAction::Disconnect => self.lifecycle.disconnect(),
            OwnerAction::SetMode { mode } => self.lifecycle.set_mode(mode),
        };
        let mutation_result = match lifecycle {
            Ok(outcome) if outcome.changed => MutationResult::Success,
            Ok(_) => MutationResult::NoChange,
            Err(error) => MutationResult::Failure(error.stable_code()),
        };
        let cached = self.coordinator.finish(token, mutation_result)?;
        let lifecycle_error = match &lifecycle {
            Ok(_) => None,
            Err(error) => Some(error.stable_code()),
        };
        if cached.error != lifecycle_error {
            return Err(OwnerError::Invariant);
        }
        Ok(OwnerExecution::Applied { cached, lifecycle })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::{DesiredState, OwnedObservation, read_desired};
    use crate::lifecycle::HostStepError;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeHost {
        observation: OwnedObservation,
        calls: usize,
        fail_start: bool,
    }

    impl Default for FakeHost {
        fn default() -> Self {
            Self {
                observation: empty(),
                calls: 0,
                fail_start: false,
            }
        }
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
            if self.fail_start {
                Err(HostStepError::Start)
            } else {
                Ok(())
            }
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            self.observation = empty();
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls += 1;
            Ok(())
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

    fn root(label: &str) -> (PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-owner-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn engine(label: &str, host: FakeHost) -> (PathBuf, OwnerEngine<FakeHost>) {
        let (root, uid) = root(label);
        let paths = DesiredPaths::below(&root);
        (root, OwnerEngine::new(host, paths, uid))
    }

    fn digest(value: u8) -> MutationDigest {
        MutationDigest::new([value; 32])
    }

    #[test]
    fn connect_disconnect_revisions_and_replay_do_not_repeat_side_effects() {
        let (root, mut owner) = engine("success", FakeHost::default());
        let connect = OwnerRequest::new(
            OwnerAction::Connect {
                profile_id: "opaque-id".to_owned(),
                mode: Some(RoutingMode::Global),
            },
            Some("connect-1"),
            Some(0),
            digest(1),
        );
        let OwnerExecution::Applied { cached, lifecycle } = owner.execute(connect).unwrap() else {
            panic!("connect was not applied");
        };
        assert_eq!(cached.revision, 1);
        assert!(cached.succeeded());
        assert_eq!(lifecycle.unwrap().actual, ActualState::Connected);
        let calls = owner.host().calls;

        let replay = OwnerRequest::new(
            OwnerAction::Connect {
                profile_id: "opaque-id".to_owned(),
                mode: Some(RoutingMode::Global),
            },
            Some("connect-1"),
            None,
            digest(1),
        );
        assert_eq!(
            owner.execute(replay).unwrap(),
            OwnerExecution::Replay(cached)
        );
        assert_eq!(owner.host().calls, calls);

        let disconnect = OwnerRequest::new(
            OwnerAction::Disconnect,
            Some("disconnect-1"),
            Some(1),
            digest(2),
        );
        let OwnerExecution::Applied { cached, lifecycle } = owner.execute(disconnect).unwrap()
        else {
            panic!("disconnect was not applied");
        };
        assert_eq!(cached.revision, 2);
        assert_eq!(lifecycle.unwrap().actual, ActualState::Disconnected);
        assert_eq!(owner.revision(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn revision_and_operation_conflicts_happen_before_lifecycle_side_effects() {
        let (root, mut owner) = engine("conflict", FakeHost::default());
        let stale = OwnerRequest::new(OwnerAction::Disconnect, Some("stale"), Some(1), digest(1));
        assert_eq!(
            owner.execute(stale),
            Err(OwnerError::Coordinator(CoordinatorError::RevisionConflict))
        );
        assert_eq!(owner.host().calls, 0);

        let first = OwnerRequest::new(OwnerAction::Disconnect, Some("same-id"), Some(0), digest(2));
        assert!(matches!(
            owner.execute(first).unwrap(),
            OwnerExecution::Applied { .. }
        ));
        let calls = owner.host().calls;
        let conflict = OwnerRequest::new(OwnerAction::Disconnect, Some("same-id"), None, digest(3));
        assert_eq!(
            owner.execute(conflict),
            Err(OwnerError::Coordinator(CoordinatorError::OperationConflict))
        );
        assert_eq!(owner.host().calls, calls);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restored_failure_is_cached_without_revision_increment_or_private_error() {
        let (root, mut owner) = engine(
            "failure",
            FakeHost {
                fail_start: true,
                ..FakeHost::default()
            },
        );
        let private = "private.example/password";
        let request = OwnerRequest::new(
            OwnerAction::Connect {
                profile_id: private.to_owned(),
                mode: Some(RoutingMode::Rule),
            },
            Some("failed"),
            Some(0),
            digest(4),
        );
        let OwnerExecution::Applied { cached, lifecycle } = owner.execute(request).unwrap() else {
            panic!("failure was not applied");
        };
        assert_eq!(
            cached.error,
            Some(StableErrorCode::TransitionFailedRestored)
        );
        assert_eq!(cached.revision, 0);
        assert_eq!(lifecycle, Err(LifecycleError::TransitionFailedRestored));
        let public = format!("{:?}", lifecycle.unwrap_err());
        assert!(!public.contains(private));
        assert!(!public.contains("password"));
        let calls = owner.host().calls;

        let replay = OwnerRequest::new(
            OwnerAction::Connect {
                profile_id: private.to_owned(),
                mode: Some(RoutingMode::Rule),
            },
            Some("failed"),
            None,
            digest(4),
        );
        assert_eq!(
            owner.execute(replay).unwrap(),
            OwnerExecution::Replay(cached)
        );
        assert_eq!(owner.host().calls, calls);
        assert_eq!(owner.revision(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_reconciliation_is_delegated_before_mutations() {
        let (root, mut owner) = engine("startup", FakeHost::default());
        let outcome = owner.reconcile_startup().unwrap();
        assert_eq!(outcome.actual, ActualState::Disconnected);
        assert_eq!(owner.actual(), ActualState::Disconnected);
        assert_eq!(owner.revision(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desired_state_stays_private_on_disk_and_out_of_owner_results() {
        let (root, uid) = root("privacy");
        let paths = DesiredPaths::below(&root);
        let mut owner = OwnerEngine::new(FakeHost::default(), paths.clone(), uid);
        owner
            .execute(OwnerRequest::new(
                OwnerAction::Connect {
                    profile_id: "opaque-private-id".to_owned(),
                    mode: Some(RoutingMode::Direct),
                },
                None,
                None,
                digest(8),
            ))
            .unwrap();
        assert_eq!(
            read_desired(&paths, uid).unwrap().profile_id,
            "opaque-private-id"
        );
        let public = format!("{:?}", owner.actual());
        assert!(!public.contains("opaque-private-id"));
        fs::remove_dir_all(root).unwrap();
    }
}
