// SPDX-License-Identifier: MIT

//! Transaction ordering for the future native runtime owner.
//!
//! The executor deliberately has no IPC entry point and no production host
//! adapter yet. It makes desired-state persistence, core preparation, startup,
//! verification, commit, cleanup, and rollback ordering executable without
//! allowing this R5 checkpoint to compete with the Python production owner.

use crate::desired::{
    DesiredError, DesiredPaths, DesiredState, MAX_GENERATION, OwnedObservation, ReconcileAction,
    RoutingMode, read_desired, reconcile, write_desired,
};
use omavless_control_protocol::StableErrorCode;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActualState {
    Disconnected,
    Starting,
    Connected,
    Reconnecting,
    Stopping,
    Failed,
    ManualRecoveryRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostStepError {
    Observation,
    Prepare,
    Start,
    Commit,
    Stop,
    Cleanup,
}

impl fmt::Display for HostStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Observation => "Runtime ownership observation failed",
            Self::Prepare => "Connection preparation failed",
            Self::Start => "Mihomo startup failed",
            Self::Commit => "Prepared connection commit failed",
            Self::Stop => "Mihomo cleanup failed",
            Self::Cleanup => "Prepared connection cleanup failed",
        })
    }
}

impl std::error::Error for HostStepError {}

/// Fixed-purpose boundary implemented later by the package-specific host
/// adapter. Inputs are semantic desired state; there is no arbitrary argv,
/// shell, service, or privileged-command surface.
pub trait LifecycleHost {
    fn observe(&mut self, desired: &DesiredState) -> Result<OwnedObservation, HostStepError>;
    fn prepare(&mut self, desired: &DesiredState) -> Result<(), HostStepError>;
    fn start_prepared(&mut self) -> Result<(), HostStepError>;
    fn commit_prepared(&mut self) -> Result<(), HostStepError>;
    fn stop_owned(&mut self) -> Result<(), HostStepError>;
    fn discard_prepared(&mut self) -> Result<(), HostStepError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidRequest,
    State,
    TransitionFailedRestored,
    RecoveryFailed,
    ManualRecoveryRequired,
}

impl LifecycleError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::InvalidRequest => StableErrorCode::InvalidArgument,
            Self::State => StableErrorCode::InternalError,
            Self::TransitionFailedRestored => StableErrorCode::TransitionFailedRestored,
            Self::RecoveryFailed => StableErrorCode::CoreRejected,
            Self::ManualRecoveryRequired => StableErrorCode::ManualRecoveryRequired,
        }
    }
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "Connection request is invalid",
            Self::State => "Connection state could not be updated",
            Self::TransitionFailedRestored => "Connection transition failed and was restored",
            Self::RecoveryFailed => "Connection recovery failed",
            Self::ManualRecoveryRequired => "Manual recovery is required",
        })
    }
}

impl std::error::Error for LifecycleError {}

impl From<DesiredError> for LifecycleError {
    fn from(_value: DesiredError) -> Self {
        Self::State
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleOutcome {
    pub actual: ActualState,
    pub generation: u64,
    pub changed: bool,
}

pub struct LifecycleExecutor<H> {
    host: H,
    paths: DesiredPaths,
    uid: u32,
    actual: ActualState,
}

impl<H: LifecycleHost> LifecycleExecutor<H> {
    #[must_use]
    pub const fn new(host: H, paths: DesiredPaths, uid: u32) -> Self {
        Self {
            host,
            paths,
            uid,
            actual: ActualState::Disconnected,
        }
    }

    #[must_use]
    pub const fn actual(&self) -> ActualState {
        self.actual
    }

    #[must_use]
    pub const fn host(&self) -> &H {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    pub fn into_host(self) -> H {
        self.host
    }

    fn read(&self) -> Result<DesiredState, LifecycleError> {
        read_desired(&self.paths, self.uid).map_err(Into::into)
    }

    fn write(&self, state: &DesiredState) -> Result<(), LifecycleError> {
        write_desired(&self.paths, self.uid, state).map_err(Into::into)
    }

    fn next_generation(current: u64, steps: u64) -> Result<u64, LifecycleError> {
        current
            .checked_add(steps)
            .filter(|generation| *generation <= MAX_GENERATION)
            .ok_or(LifecycleError::State)
    }

    fn outcome(&self, state: &DesiredState, changed: bool) -> LifecycleOutcome {
        LifecycleOutcome {
            actual: self.actual,
            generation: state.generation,
            changed,
        }
    }

    fn observe_or_manual(
        &mut self,
        desired: &DesiredState,
    ) -> Result<OwnedObservation, LifecycleError> {
        self.host.observe(desired).map_err(|_| {
            self.actual = ActualState::ManualRecoveryRequired;
            LifecycleError::ManualRecoveryRequired
        })
    }

    fn discard_or_manual(&mut self) -> Result<(), LifecycleError> {
        self.host.discard_prepared().map_err(|_| {
            self.actual = ActualState::ManualRecoveryRequired;
            LifecycleError::ManualRecoveryRequired
        })
    }

    fn verify_empty(&mut self, desired: &DesiredState) -> Result<(), LifecycleError> {
        let observed = self.observe_or_manual(desired)?;
        if reconcile(desired, observed) != ReconcileAction::SettledDisconnected {
            self.actual = ActualState::ManualRecoveryRequired;
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        Ok(())
    }

    fn verify_connected(&mut self, desired: &DesiredState) -> Result<(), LifecycleError> {
        let observed = self.observe_or_manual(desired)?;
        if reconcile(desired, observed) != ReconcileAction::AdoptConnected {
            return Err(LifecycleError::RecoveryFailed);
        }
        Ok(())
    }

    fn cleanup_failed_connect(
        &mut self,
        armed: &DesiredState,
        rollback: &DesiredState,
    ) -> Result<(), LifecycleError> {
        self.actual = ActualState::Stopping;
        if self.host.stop_owned().is_err() {
            self.actual = ActualState::ManualRecoveryRequired;
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        self.discard_or_manual()?;
        // Keep desired connected until owned runtime cleanup is proven. This
        // prevents an ambiguous live core from being reported as disconnected.
        let empty_observation = self.observe_or_manual(armed)?;
        let empty = !empty_observation.service_active
            && !empty_observation.controller_ready
            && empty_observation.core_count == 0
            && empty_observation.tun_count == 0;
        if !empty {
            self.actual = ActualState::ManualRecoveryRequired;
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        if self.write(rollback).is_err() {
            self.actual = ActualState::ManualRecoveryRequired;
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        self.actual = ActualState::Disconnected;
        Ok(())
    }

    pub fn connect(
        &mut self,
        profile_id: &str,
        mode: RoutingMode,
    ) -> Result<LifecycleOutcome, LifecycleError> {
        let current = self.read()?;
        let target = DesiredState {
            schema_version: current.schema_version,
            generation: current.generation,
            connected: true,
            profile_id: profile_id.to_owned(),
            mode,
        };
        target
            .validate()
            .map_err(|_| LifecycleError::InvalidRequest)?;
        let observed = self.observe_or_manual(&current)?;
        match reconcile(&current, observed) {
            ReconcileAction::AdoptConnected
                if current.profile_id == profile_id && current.mode == mode =>
            {
                self.actual = ActualState::Connected;
                return Ok(self.outcome(&current, false));
            }
            ReconcileAction::SettledDisconnected => {}
            _ => {
                self.actual = ActualState::ManualRecoveryRequired;
                return Err(LifecycleError::ManualRecoveryRequired);
            }
        }

        // Reserve both generations before side effects: one for arming the
        // connect and one for a credential-free disconnected rollback.
        let armed_generation = Self::next_generation(current.generation, 1)?;
        let rollback_generation = Self::next_generation(current.generation, 2)?;
        let armed = DesiredState {
            generation: armed_generation,
            ..target
        };
        let rollback = DesiredState {
            generation: rollback_generation,
            ..current
        };

        self.actual = ActualState::Starting;
        if self.host.prepare(&armed).is_err() {
            self.discard_or_manual()?;
            self.actual = ActualState::Disconnected;
            return Err(LifecycleError::TransitionFailedRestored);
        }
        if let Err(error) = self.write(&armed) {
            self.discard_or_manual()?;
            self.actual = ActualState::Disconnected;
            return Err(error);
        }
        let started = self.host.start_prepared().is_ok();
        let verified = started && self.verify_connected(&armed).is_ok();
        let committed = verified && self.host.commit_prepared().is_ok();
        if committed {
            self.actual = ActualState::Connected;
            return Ok(self.outcome(&armed, true));
        }
        self.cleanup_failed_connect(&armed, &rollback)?;
        Err(LifecycleError::TransitionFailedRestored)
    }

    pub fn disconnect(&mut self) -> Result<LifecycleOutcome, LifecycleError> {
        let current = self.read()?;
        let observed = self.observe_or_manual(&current)?;
        let action = reconcile(&current, observed);
        if action == ReconcileAction::SettledDisconnected {
            self.actual = ActualState::Disconnected;
            return Ok(self.outcome(&current, false));
        }
        if action == ReconcileAction::ManualRecoveryRequired {
            self.actual = ActualState::ManualRecoveryRequired;
            return Err(LifecycleError::ManualRecoveryRequired);
        }

        let disconnected = DesiredState {
            generation: Self::next_generation(current.generation, 1)?,
            connected: false,
            profile_id: String::new(),
            ..current
        };
        // Explicit disconnect changes durable intent before stopping. A stop
        // failure must not silently restore desired connected state.
        self.write(&disconnected)?;
        self.actual = ActualState::Stopping;
        if action != ReconcileAction::RecoverConnected && self.host.stop_owned().is_err() {
            self.actual = ActualState::ManualRecoveryRequired;
            return Err(LifecycleError::ManualRecoveryRequired);
        }
        self.discard_or_manual()?;
        self.verify_empty(&disconnected)?;
        self.actual = ActualState::Disconnected;
        Ok(self.outcome(&disconnected, true))
    }

    /// Execute exactly one accepted restart-reconciliation decision. Callers
    /// must not loop this method after `RecoveryFailed`; a fresh owner process
    /// may attempt one new bounded recovery after re-observation.
    pub fn reconcile_startup(&mut self) -> Result<LifecycleOutcome, LifecycleError> {
        let desired = self.read()?;
        let observed = self.observe_or_manual(&desired)?;
        match reconcile(&desired, observed) {
            ReconcileAction::SettledDisconnected => {
                self.actual = ActualState::Disconnected;
                Ok(self.outcome(&desired, false))
            }
            ReconcileAction::AdoptConnected => {
                self.actual = ActualState::Connected;
                Ok(self.outcome(&desired, false))
            }
            ReconcileAction::ManualRecoveryRequired => {
                self.actual = ActualState::ManualRecoveryRequired;
                Err(LifecycleError::ManualRecoveryRequired)
            }
            ReconcileAction::StopOwned => {
                self.actual = ActualState::Stopping;
                if self.host.stop_owned().is_err() {
                    self.actual = ActualState::ManualRecoveryRequired;
                    return Err(LifecycleError::ManualRecoveryRequired);
                }
                self.discard_or_manual()?;
                self.verify_empty(&desired)?;
                self.actual = ActualState::Disconnected;
                Ok(self.outcome(&desired, true))
            }
            ReconcileAction::RecoverConnected => {
                self.actual = ActualState::Reconnecting;
                let recovered = self.host.prepare(&desired).is_ok()
                    && self.host.start_prepared().is_ok()
                    && self.verify_connected(&desired).is_ok()
                    && self.host.commit_prepared().is_ok();
                if recovered {
                    self.actual = ActualState::Connected;
                    return Ok(self.outcome(&desired, true));
                }
                // Recovery preserves connected intent. Clean partial owned
                // state, but never rewrite desired disconnected on its own.
                if self.host.stop_owned().is_err() || self.host.discard_prepared().is_err() {
                    self.actual = ActualState::ManualRecoveryRequired;
                    return Err(LifecycleError::ManualRecoveryRequired);
                }
                let empty = self.host.observe(&desired).is_ok_and(|value| {
                    !value.service_active
                        && !value.controller_ready
                        && value.core_count == 0
                        && value.tun_count == 0
                });
                if !empty {
                    self.actual = ActualState::ManualRecoveryRequired;
                    return Err(LifecycleError::ManualRecoveryRequired);
                }
                self.actual = ActualState::Failed;
                Err(LifecycleError::RecoveryFailed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeHost {
        calls: Vec<&'static str>,
        observation: OwnedObservation,
        fail_prepare: bool,
        fail_start: bool,
        fail_commit: bool,
        fail_stop: bool,
        fail_discard: bool,
        leave_running_after_stop: bool,
        desired_probe: Option<(DesiredPaths, u32)>,
        connected_intent_seen_at_start: bool,
        disconnected_intent_seen_at_stop: bool,
    }

    impl Default for FakeHost {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                observation: empty(),
                fail_prepare: false,
                fail_start: false,
                fail_commit: false,
                fail_stop: false,
                fail_discard: false,
                leave_running_after_stop: false,
                desired_probe: None,
                connected_intent_seen_at_start: false,
                disconnected_intent_seen_at_stop: false,
            }
        }
    }

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.calls.push("observe");
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.calls.push("prepare");
            (!self.fail_prepare)
                .then_some(())
                .ok_or(HostStepError::Prepare)
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls.push("start");
            if let Some((paths, uid)) = &self.desired_probe {
                self.connected_intent_seen_at_start =
                    read_desired(paths, *uid).is_ok_and(|state| state.connected);
            }
            if self.fail_start {
                // Readiness can fail after a child was spawned.
                self.observation = healthy();
                return Err(HostStepError::Start);
            }
            self.observation = healthy();
            Ok(())
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls.push("commit");
            (!self.fail_commit)
                .then_some(())
                .ok_or(HostStepError::Commit)
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.calls.push("stop");
            if let Some((paths, uid)) = &self.desired_probe {
                self.disconnected_intent_seen_at_stop =
                    read_desired(paths, *uid).is_ok_and(|state| !state.connected);
            }
            if self.fail_stop {
                return Err(HostStepError::Stop);
            }
            if !self.leave_running_after_stop {
                self.observation = empty();
            }
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.calls.push("discard");
            (!self.fail_discard)
                .then_some(())
                .ok_or(HostStepError::Cleanup)
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
            "omavless-lifecycle-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn executor(label: &str, host: FakeHost) -> (PathBuf, LifecycleExecutor<FakeHost>) {
        let (root, uid) = root(label);
        let paths = DesiredPaths::below(&root);
        (root, LifecycleExecutor::new(host, paths, uid))
    }

    fn desired(executor: &LifecycleExecutor<FakeHost>) -> DesiredState {
        read_desired(&executor.paths, executor.uid).unwrap()
    }

    #[test]
    fn connect_orders_prepare_start_verify_commit_and_persists_intent() {
        let (root, mut executor) = executor("connect", FakeHost::default());
        executor.host_mut().desired_probe = Some((executor.paths.clone(), executor.uid));
        let outcome = executor.connect("opaque-id", RoutingMode::Global).unwrap();
        assert_eq!(outcome.actual, ActualState::Connected);
        assert_eq!(outcome.generation, 1);
        assert!(outcome.changed);
        let state = desired(&executor);
        assert!(state.connected);
        assert_eq!(state.profile_id, "opaque-id");
        assert_eq!(state.mode, RoutingMode::Global);
        assert_eq!(
            executor.host().calls,
            ["observe", "prepare", "start", "observe", "commit"]
        );
        assert!(executor.host().connected_intent_seen_at_start);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_connect_cleans_owned_state_then_writes_disconnected_rollback() {
        let host = FakeHost {
            fail_start: true,
            ..FakeHost::default()
        };
        let (root, mut executor) = executor("rollback", host);
        assert_eq!(
            executor.connect("opaque-id", RoutingMode::Rule),
            Err(LifecycleError::TransitionFailedRestored)
        );
        assert_eq!(executor.actual(), ActualState::Disconnected);
        let state = desired(&executor);
        assert!(!state.connected);
        assert!(state.profile_id.is_empty());
        assert_eq!(state.generation, 2);
        assert_eq!(
            executor.host().calls,
            ["observe", "prepare", "start", "stop", "discard", "observe"]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incomplete_connect_cleanup_is_a_hard_manual_recovery_blocker() {
        let host = FakeHost {
            fail_commit: true,
            leave_running_after_stop: true,
            ..FakeHost::default()
        };
        let (root, mut executor) = executor("manual", host);
        assert_eq!(
            executor.connect("opaque-id", RoutingMode::Rule),
            Err(LifecycleError::ManualRecoveryRequired)
        );
        assert_eq!(executor.actual(), ActualState::ManualRecoveryRequired);
        // Connected intent is retained because empty cleanup was not proven.
        assert!(desired(&executor).connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_disconnect_persists_intent_and_verifies_empty() {
        let (root, mut executor) = executor(
            "disconnect",
            FakeHost {
                observation: healthy(),
                ..FakeHost::default()
            },
        );
        let connected = DesiredState {
            generation: 7,
            connected: true,
            profile_id: "opaque-id".to_owned(),
            mode: RoutingMode::Direct,
            ..DesiredState::default()
        };
        executor.write(&connected).unwrap();
        executor.host_mut().desired_probe = Some((executor.paths.clone(), executor.uid));
        let outcome = executor.disconnect().unwrap();
        assert_eq!(outcome.actual, ActualState::Disconnected);
        assert_eq!(outcome.generation, 8);
        let state = desired(&executor);
        assert!(!state.connected);
        assert!(state.profile_id.is_empty());
        assert_eq!(state.mode, RoutingMode::Direct);
        assert_eq!(
            executor.host().calls,
            ["observe", "stop", "discard", "observe"]
        );
        assert!(executor.host().disconnected_intent_seen_at_stop);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disconnect_stop_failure_keeps_disconnected_intent_and_blocks() {
        let (root, mut executor) = executor(
            "disconnect-fail",
            FakeHost {
                observation: healthy(),
                fail_stop: true,
                ..FakeHost::default()
            },
        );
        executor
            .write(&DesiredState {
                generation: 3,
                connected: true,
                profile_id: "opaque-id".to_owned(),
                ..DesiredState::default()
            })
            .unwrap();
        assert_eq!(
            executor.disconnect(),
            Err(LifecycleError::ManualRecoveryRequired)
        );
        assert!(!desired(&executor).connected);
        assert_eq!(desired(&executor).generation, 4);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_adopts_healthy_or_recovers_absent_owned_core_once() {
        let (adopt_root, mut adopt) = executor(
            "adopt",
            FakeHost {
                observation: healthy(),
                ..FakeHost::default()
            },
        );
        adopt
            .write(&DesiredState {
                connected: true,
                profile_id: "opaque-id".to_owned(),
                ..DesiredState::default()
            })
            .unwrap();
        let adopted = adopt.reconcile_startup().unwrap();
        assert_eq!(adopted.actual, ActualState::Connected);
        assert!(!adopted.changed);
        assert_eq!(adopt.host().calls, ["observe"]);
        fs::remove_dir_all(adopt_root).unwrap();

        let (recover_root, mut recover) = executor("recover", FakeHost::default());
        recover
            .write(&DesiredState {
                generation: 9,
                connected: true,
                profile_id: "opaque-id".to_owned(),
                ..DesiredState::default()
            })
            .unwrap();
        let recovered = recover.reconcile_startup().unwrap();
        assert_eq!(recovered.actual, ActualState::Connected);
        assert_eq!(recovered.generation, 9);
        assert!(recovered.changed);
        assert_eq!(
            recover.host().calls,
            ["observe", "prepare", "start", "observe", "commit"]
        );
        fs::remove_dir_all(recover_root).unwrap();
    }

    #[test]
    fn failed_startup_recovery_preserves_connected_intent_without_retry_loop() {
        let (root, mut executor) = executor(
            "recover-fail",
            FakeHost {
                fail_start: true,
                ..FakeHost::default()
            },
        );
        executor
            .write(&DesiredState {
                generation: 5,
                connected: true,
                profile_id: "opaque-id".to_owned(),
                ..DesiredState::default()
            })
            .unwrap();
        assert_eq!(
            executor.reconcile_startup(),
            Err(LifecycleError::RecoveryFailed)
        );
        assert_eq!(executor.actual(), ActualState::Failed);
        assert!(desired(&executor).connected);
        assert_eq!(desired(&executor).generation, 5);
        assert_eq!(
            executor.host().calls,
            ["observe", "prepare", "start", "stop", "discard", "observe"]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inconsistent_observation_and_generation_exhaustion_have_no_side_effects() {
        let (manual_root, mut manual) = executor(
            "inconsistent",
            FakeHost {
                observation: OwnedObservation {
                    core_count: 2,
                    ..empty()
                },
                ..FakeHost::default()
            },
        );
        assert_eq!(
            manual.connect("opaque-id", RoutingMode::Rule),
            Err(LifecycleError::ManualRecoveryRequired)
        );
        assert_eq!(manual.host().calls, ["observe"]);
        fs::remove_dir_all(manual_root).unwrap();

        let (generation_root, mut generation) = executor("generation", FakeHost::default());
        generation
            .write(&DesiredState {
                generation: MAX_GENERATION - 1,
                ..DesiredState::default()
            })
            .unwrap();
        assert_eq!(
            generation.connect("opaque-id", RoutingMode::Rule),
            Err(LifecycleError::State)
        );
        assert_eq!(generation.host().calls, ["observe"]);
        assert_eq!(desired(&generation).generation, MAX_GENERATION - 1);
        fs::remove_dir_all(generation_root).unwrap();
    }

    #[test]
    fn public_errors_never_echo_private_inputs() {
        let private = "vless://user:password@private.example";
        let (root, mut executor) = executor(
            "privacy",
            FakeHost {
                fail_prepare: true,
                ..FakeHost::default()
            },
        );
        let error = executor.connect(private, RoutingMode::Rule).unwrap_err();
        let output = format!("{error:?} {error}");
        assert!(!output.contains(private));
        assert!(!output.contains("password"));
        assert!(!output.contains("private.example"));
        fs::remove_dir_all(root).unwrap();
    }
}
