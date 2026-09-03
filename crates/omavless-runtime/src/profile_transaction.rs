// SPDX-License-Identifier: MIT

//! Offline owner-bound profile mutation transactions.
//!
//! The engine composes the accepted v1 parser, shared mutation coordinator,
//! prepared exact-byte store replacement and lifecycle compensation. It is not
//! registered with `RuntimeServer`, advertised as a capability or constructed
//! by the production daemon.

use crate::cutover::{CutoverError, CutoverPaths, MigrationLock};
use crate::desired::{DesiredPaths, RoutingMode};
use crate::lifecycle::{LifecycleError, LifecycleExecutor, LifecycleHost};
use crate::mutation::{
    BeginOutcome, CachedOutcome, CoordinatorError, MutationCoordinator, MutationKind,
    MutationRequest, MutationResult, SubmitOutcome,
};
use crate::mutation_protocol::MutationProtocolError;
use crate::profile_mutation::{
    PreparedProfileMutation, PreparedWrite, ProfileMutationCommitError, prepare_profile_mutation,
};
use crate::profile_mutation_protocol::{ProfileMutationKind, parse_profile_mutation_request};
use omavless_control_protocol::StableErrorCode;
use omavless_domain::private_store::{PrivateStoreError, ProfileMutation};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

pub(crate) trait StorePlan {
    fn changed(&self) -> bool;
    fn commit(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, ProfileMutationCommitError>;
    fn restore(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, ProfileMutationCommitError>;
}

impl StorePlan for PreparedProfileMutation {
    fn changed(&self) -> bool {
        self.changed()
    }

    fn commit(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, ProfileMutationCommitError> {
        self.commit_locked(lock, paths)
    }

    fn restore(
        &self,
        lock: &MigrationLock,
        paths: &CutoverPaths,
    ) -> Result<PreparedWrite, ProfileMutationCommitError> {
        self.restore_locked(lock, paths)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileTransactionError {
    Busy,
    NotFound,
    InvalidArgument,
    Conflict,
    Store,
    TransitionFailedRestored,
    ManualRecoveryRequired,
}

impl ProfileTransactionError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Busy => StableErrorCode::Busy,
            Self::NotFound => StableErrorCode::NotFound,
            Self::InvalidArgument => StableErrorCode::InvalidArgument,
            Self::Conflict => StableErrorCode::Conflict,
            Self::Store => StableErrorCode::InternalError,
            Self::TransitionFailedRestored => StableErrorCode::TransitionFailedRestored,
            Self::ManualRecoveryRequired => StableErrorCode::ManualRecoveryRequired,
        }
    }
}

impl fmt::Display for ProfileTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "Another OmaVLESS operation is active",
            Self::NotFound => "Requested profile was not found",
            Self::InvalidArgument => "Profile mutation is not permitted",
            Self::Conflict => "Profile store changed concurrently",
            Self::Store => "Profile store transaction failed",
            Self::TransitionFailedRestored => "Profile transition failed and was restored",
            Self::ManualRecoveryRequired => "Manual recovery is required",
        })
    }
}

fn lock_error(error: CutoverError) -> ProfileTransactionError {
    match error {
        CutoverError::Busy => ProfileTransactionError::Busy,
        CutoverError::UnsafeRuntimeDirectory
        | CutoverError::UnsafeStateDirectory
        | CutoverError::InvalidMarker
        | CutoverError::MarkerTooLarge
        | CutoverError::InvalidTransition
        | CutoverError::PreconditionsFailed
        | CutoverError::Io => ProfileTransactionError::Store,
    }
}

impl std::error::Error for ProfileTransactionError {}

pub(crate) fn store_error(error: ProfileMutationCommitError) -> ProfileTransactionError {
    match error {
        ProfileMutationCommitError::StoreChanged => ProfileTransactionError::Conflict,
        ProfileMutationCommitError::Mutation(PrivateStoreError::ProfileNotFound) => {
            ProfileTransactionError::NotFound
        }
        ProfileMutationCommitError::Mutation(PrivateStoreError::DuplicateProfileName) => {
            ProfileTransactionError::Conflict
        }
        ProfileMutationCommitError::Mutation(PrivateStoreError::InvalidName)
        | ProfileMutationCommitError::Mutation(PrivateStoreError::SubscribedProfile) => {
            ProfileTransactionError::InvalidArgument
        }
        ProfileMutationCommitError::UnsafeStore
        | ProfileMutationCommitError::StoreIo
        | ProfileMutationCommitError::LockMismatch
        | ProfileMutationCommitError::Mutation(_) => ProfileTransactionError::Store,
    }
}

fn lifecycle_error(error: LifecycleError) -> ProfileTransactionError {
    match error {
        LifecycleError::InvalidRequest => ProfileTransactionError::InvalidArgument,
        LifecycleError::State => ProfileTransactionError::Store,
        LifecycleError::ManualRecoveryRequired => ProfileTransactionError::ManualRecoveryRequired,
        LifecycleError::TransitionFailedRestored => {
            ProfileTransactionError::TransitionFailedRestored
        }
        LifecycleError::RecoveryFailed => ProfileTransactionError::ManualRecoveryRequired,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileMutationOutcome {
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileOwnerExecution {
    Applied {
        cached: CachedOutcome,
        outcome: Result<ProfileMutationOutcome, ProfileTransactionError>,
    },
    /// A failure before store preparation, lifecycle observation, or any
    /// mutation. It is deliberately not retained in the operation replay
    /// cache, so callers may retry the same operation ID.
    UncachedPreflightFailure {
        revision: u64,
        error: ProfileTransactionError,
    },
    Replay(CachedOutcome),
    Rejected(CachedOutcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileOwnerError {
    Protocol(MutationProtocolError),
    Coordinator(CoordinatorError),
    Invariant,
}

impl ProfileOwnerError {
    #[must_use]
    pub const fn stable_code(self) -> StableErrorCode {
        match self {
            Self::Protocol(error) => error.stable_code(),
            Self::Coordinator(error) => error.stable_code(),
            Self::Invariant => StableErrorCode::InternalError,
        }
    }
}

impl fmt::Display for ProfileOwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => error.fmt(formatter),
            Self::Coordinator(error) => error.fmt(formatter),
            Self::Invariant => formatter.write_str("Profile owner invariant failed"),
        }
    }
}

impl std::error::Error for ProfileOwnerError {}

impl From<MutationProtocolError> for ProfileOwnerError {
    fn from(value: MutationProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl From<CoordinatorError> for ProfileOwnerError {
    fn from(value: CoordinatorError) -> Self {
        Self::Coordinator(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionKind {
    Rename,
    Favorite,
    Delete,
}

pub(crate) fn mutation_identity(mutation: &ProfileMutation) -> (ActionKind, &str) {
    match mutation {
        ProfileMutation::Rename { profile_id, .. } => (ActionKind::Rename, profile_id),
        ProfileMutation::Favorite { profile_id, .. } => (ActionKind::Favorite, profile_id),
        ProfileMutation::Delete { profile_id } => (ActionKind::Delete, profile_id),
    }
}

fn rollback_rename<H: LifecycleHost, P: StorePlan>(
    lifecycle: &mut LifecycleExecutor<H>,
    plan: &P,
    profile_id: &str,
    lock: &MigrationLock,
    paths: &CutoverPaths,
) -> ProfileTransactionError {
    if plan.restore(lock, paths).is_err() {
        return ProfileTransactionError::ManualRecoveryRequired;
    }
    match lifecycle.recover_preserved_profile(profile_id) {
        Ok(_) => ProfileTransactionError::TransitionFailedRestored,
        Err(_) => ProfileTransactionError::ManualRecoveryRequired,
    }
}

fn rollback_delete<H: LifecycleHost, P: StorePlan>(
    lifecycle: &mut LifecycleExecutor<H>,
    plan: &P,
    profile_id: &str,
    mode: RoutingMode,
    lock: &MigrationLock,
    paths: &CutoverPaths,
) -> ProfileTransactionError {
    if plan.restore(lock, paths).is_err() {
        return ProfileTransactionError::ManualRecoveryRequired;
    }
    match lifecycle.connect(profile_id, mode) {
        Ok(_) => ProfileTransactionError::TransitionFailedRestored,
        Err(_) => ProfileTransactionError::ManualRecoveryRequired,
    }
}

fn commit_changed<P: StorePlan>(
    plan: &P,
    lock: &MigrationLock,
    paths: &CutoverPaths,
) -> Result<(), ProfileTransactionError> {
    match plan.commit(lock, paths).map_err(store_error)? {
        PreparedWrite::Changed => Ok(()),
        PreparedWrite::NoChange => Err(ProfileTransactionError::Store),
    }
}

pub(crate) fn apply_transaction<H: LifecycleHost, P: StorePlan>(
    lifecycle: &mut LifecycleExecutor<H>,
    plan: &P,
    kind: ActionKind,
    profile_id: &str,
    lock: &MigrationLock,
    paths: &CutoverPaths,
) -> Result<ProfileMutationOutcome, ProfileTransactionError> {
    if !plan.changed() {
        return match plan.commit(lock, paths).map_err(store_error)? {
            PreparedWrite::NoChange => Ok(ProfileMutationOutcome { changed: false }),
            PreparedWrite::Changed => Err(ProfileTransactionError::Store),
        };
    }
    if kind == ActionKind::Favorite {
        commit_changed(plan, lock, paths)?;
        return Ok(ProfileMutationOutcome { changed: true });
    }

    let target = lifecycle
        .observe_profile_target(profile_id)
        .map_err(lifecycle_error)?;
    if !target.active {
        commit_changed(plan, lock, paths)?;
        return Ok(ProfileMutationOutcome { changed: true });
    }

    match kind {
        ActionKind::Rename => {
            lifecycle
                .quiesce_profile_preserving_desired(profile_id)
                .map_err(lifecycle_error)?;
            if commit_changed(plan, lock, paths).is_err() {
                return Err(rollback_rename(lifecycle, plan, profile_id, lock, paths));
            }
            match lifecycle.recover_preserved_profile(profile_id) {
                Ok(_) => Ok(ProfileMutationOutcome { changed: true }),
                Err(LifecycleError::ManualRecoveryRequired) => {
                    Err(ProfileTransactionError::ManualRecoveryRequired)
                }
                Err(_) => Err(rollback_rename(lifecycle, plan, profile_id, lock, paths)),
            }
        }
        ActionKind::Delete => {
            lifecycle.disconnect().map_err(lifecycle_error)?;
            match plan.commit(lock, paths) {
                Ok(PreparedWrite::Changed) => Ok(ProfileMutationOutcome { changed: true }),
                Ok(PreparedWrite::NoChange) => Err(rollback_delete(
                    lifecycle,
                    plan,
                    profile_id,
                    target.mode,
                    lock,
                    paths,
                )),
                Err(_) => Err(rollback_delete(
                    lifecycle,
                    plan,
                    profile_id,
                    target.mode,
                    lock,
                    paths,
                )),
            }
        }
        ActionKind::Favorite => unreachable!("favorite returned before lifecycle inspection"),
    }
}

/// Offline-only profile mutation owner. It has no socket registration and is
/// not constructed by the production runtime.
pub struct OfflineProfileOwner<H> {
    coordinator: MutationCoordinator,
    lifecycle: LifecycleExecutor<H>,
    store_path: PathBuf,
    cutover_paths: CutoverPaths,
    uid: u32,
}

impl<H: LifecycleHost> OfflineProfileOwner<H> {
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
            lifecycle: LifecycleExecutor::new(host, desired_paths, uid),
            store_path: store_path.to_path_buf(),
            cutover_paths,
            uid,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.coordinator.revision()
    }

    #[must_use]
    pub const fn host(&self) -> &H {
        self.lifecycle.host()
    }

    pub fn host_mut(&mut self) -> &mut H {
        self.lifecycle.host_mut()
    }

    pub fn execute(&mut self, request: &Value) -> Result<ProfileOwnerExecution, ProfileOwnerError> {
        let parsed = parse_profile_mutation_request(request)?;
        let parsed_kind = parsed.kind();
        let (mutation, operation_id, expected_revision, digest) = parsed.into_parts();
        let (kind, profile_id) = mutation_identity(&mutation);
        let profile_id = profile_id.to_owned();
        debug_assert_eq!(
            parsed_kind,
            match kind {
                ActionKind::Rename => ProfileMutationKind::Rename,
                ActionKind::Favorite => ProfileMutationKind::Favorite,
                ActionKind::Delete => ProfileMutationKind::Delete,
            }
        );
        let scheduling = MutationRequest::new(
            MutationKind::Other,
            operation_id.as_deref(),
            expected_revision,
            digest,
        )?;
        let token = match self.coordinator.submit(scheduling)? {
            SubmitOutcome::Queued { token } => token,
            SubmitOutcome::Replay(outcome) => return Ok(ProfileOwnerExecution::Replay(outcome)),
        };
        match self.coordinator.begin_next()? {
            BeginOutcome::Started(active) if active.token == token => {}
            BeginOutcome::Rejected {
                token: rejected,
                outcome,
            } if rejected == token => {
                return Ok(ProfileOwnerExecution::Rejected(outcome));
            }
            BeginOutcome::Started(_) | BeginOutcome::Rejected { .. } | BeginOutcome::Empty => {
                return Err(ProfileOwnerError::Invariant);
            }
        }

        // This established lock is shared with the legacy Python owner. Keep
        // it alive across store preparation, desired/actual observation, every
        // lifecycle side effect, compare-before-replace and compensation.
        // Exact-byte comparison is a second fail-closed guard, not a standalone
        // lock-free filesystem CAS.
        let _owner_lock = match MigrationLock::acquire(&self.cutover_paths, self.uid) {
            Ok(owner_lock) => owner_lock,
            Err(error) => {
                // Lock acquisition precedes store reads and lifecycle
                // observation, so it is the only safe point where an active
                // slot can be retired without an idempotency record. Caching
                // Busy here would make an exact operation-ID retry replay the
                // transient failure forever after contention clears.
                self.coordinator.abort_active_uncached(token)?;
                return Ok(ProfileOwnerExecution::UncachedPreflightFailure {
                    revision: self.coordinator.revision(),
                    error: lock_error(error),
                });
            }
        };
        let outcome = prepare_profile_mutation(&self.store_path, self.uid, mutation)
            .map_err(store_error)
            .and_then(|plan| {
                apply_transaction(
                    &mut self.lifecycle,
                    &plan,
                    kind,
                    &profile_id,
                    &_owner_lock,
                    &self.cutover_paths,
                )
            });
        let result = match outcome {
            Ok(value) if value.changed => MutationResult::Success,
            Ok(_) => MutationResult::NoChange,
            Err(error) => MutationResult::Failure(error.stable_code()),
        };
        let cached = self.coordinator.finish(token, result)?;
        if cached.error != outcome.as_ref().err().map(|error| error.stable_code()) {
            return Err(ProfileOwnerError::Invariant);
        }
        Ok(ProfileOwnerExecution::Applied { cached, outcome })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::{DesiredState, OwnedObservation, read_desired, write_desired};
    use crate::lifecycle::HostStepError;
    use omavless_control_protocol::make_request;
    use serde_json::{Value, json};
    use std::collections::VecDeque;
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE: &str = "00000000-0000-4000-8000-000000000001";
    const OTHER: &str = "00000000-0000-4000-8000-000000000002";

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
        calls: Vec<&'static str>,
        observation: OwnedObservation,
        start_results: VecDeque<bool>,
        fail_stop: bool,
        leave_running: bool,
        python_lock_probe: Option<PathBuf>,
        python_lock_blocked: Vec<bool>,
    }

    impl FakeHost {
        fn disconnected() -> Self {
            Self {
                calls: Vec::new(),
                observation: empty(),
                start_results: VecDeque::new(),
                fail_stop: false,
                leave_running: false,
                python_lock_probe: None,
                python_lock_blocked: Vec::new(),
            }
        }

        fn connected() -> Self {
            Self {
                observation: healthy(),
                ..Self::disconnected()
            }
        }
    }

    impl FakeHost {
        fn probe_python_lock(&mut self) {
            let Some(path) = &self.python_lock_probe else {
                return;
            };
            let blocked = Command::new("python3")
                .arg("-c")
                .arg(
                    "import fcntl,sys\nwith open(sys.argv[1], 'a') as f:\n try: fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)\n except BlockingIOError: raise SystemExit(0)\nraise SystemExit(1)",
                )
                .arg(path)
                .status()
                .is_ok_and(|status| status.success());
            self.python_lock_blocked.push(blocked);
        }
    }

    impl LifecycleHost for FakeHost {
        fn observe(&mut self, _desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
            self.probe_python_lock();
            self.calls.push("observe");
            Ok(self.observation)
        }

        fn prepare(&mut self, _desired: &DesiredState) -> Result<(), HostStepError> {
            self.probe_python_lock();
            self.calls.push("prepare");
            Ok(())
        }

        fn start_prepared(&mut self) -> Result<(), HostStepError> {
            self.probe_python_lock();
            self.calls.push("start");
            let succeeds = self.start_results.pop_front().unwrap_or(true);
            if succeeds {
                self.observation = healthy();
                Ok(())
            } else {
                Err(HostStepError::Start)
            }
        }

        fn commit_prepared(&mut self) -> Result<(), HostStepError> {
            self.probe_python_lock();
            self.calls.push("commit");
            Ok(())
        }

        fn stop_owned(&mut self) -> Result<(), HostStepError> {
            self.probe_python_lock();
            self.calls.push("stop");
            if self.fail_stop {
                return Err(HostStepError::Stop);
            }
            if !self.leave_running {
                self.observation = empty();
            }
            Ok(())
        }

        fn discard_prepared(&mut self) -> Result<(), HostStepError> {
            self.probe_python_lock();
            self.calls.push("discard");
            Ok(())
        }
    }

    fn temp(label: &str) -> (PathBuf, PathBuf, DesiredPaths, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-profile-owner-{label}-{}-{nonce}",
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
        let store = config.join("profiles.json");
        fs::write(&store, serde_json::to_vec(&store_value()).unwrap()).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
        let desired = DesiredPaths::below(&root.join("state"));
        (root, store, desired, uid)
    }

    fn cutover(root: &Path, uid: u32) -> CutoverPaths {
        CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid)
    }

    fn store_value() -> Value {
        json!({
            "version": 3,
            "activeId": OTHER,
            "lastId": PROFILE,
            "profiles": [
                {"id": PROFILE, "name": "Private", "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Private", "protocol": "vless", "favorite": false},
                {"id": OTHER, "name": "Other", "uri": "vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Other", "protocol": "vless", "favorite": false}
            ],
            "subscriptions": [],
            "routingPreset": "custom",
            "customRules": [],
            "rulesUpdatedAt": 0,
            "startupConfigured": true,
            "startup": {"enabled": true, "target": "profile", "profileId": PROFILE, "mode": "global"},
            "onboardingComplete": true
        })
    }

    fn request(method: &str, params: Value) -> Value {
        make_request("request-1", method, params).unwrap()
    }

    fn set_desired(
        paths: &DesiredPaths,
        uid: u32,
        connected: bool,
        profile: &str,
        generation: u64,
    ) {
        write_desired(
            paths,
            uid,
            &DesiredState {
                schema_version: 1,
                generation,
                connected,
                profile_id: profile.to_owned(),
                mode: RoutingMode::Global,
            },
        )
        .unwrap();
    }

    fn applied(
        execution: ProfileOwnerExecution,
    ) -> (
        CachedOutcome,
        Result<ProfileMutationOutcome, ProfileTransactionError>,
    ) {
        match execution {
            ProfileOwnerExecution::Applied { cached, outcome } => (cached, outcome),
            _ => panic!("expected applied outcome"),
        }
    }

    #[test]
    fn favorite_and_inactive_mutations_are_store_only_and_preserve_pointers() {
        let (root, store, desired, uid) = temp("inactive");
        set_desired(&desired, uid, true, OTHER, 7);
        let mut owner = OfflineProfileOwner::new(
            FakeHost::connected(),
            desired.clone(),
            &store,
            cutover(&root, uid),
            uid,
        );
        let (_, favorite) = applied(
            owner
                .execute(&request(
                    "profiles.favorite",
                    json!({"profileId": PROFILE, "enabled": true}),
                ))
                .unwrap(),
        );
        assert!(favorite.unwrap().changed);
        assert!(owner.host().calls.is_empty());

        let (_, rename) = applied(
            owner
                .execute(&request(
                    "profiles.rename",
                    json!({"profileId": PROFILE, "name": "Renamed"}),
                ))
                .unwrap(),
        );
        assert!(rename.unwrap().changed);
        assert_eq!(owner.host().calls, ["observe"]);
        let document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        assert_eq!(document["lastId"], PROFILE);
        assert_eq!(document["startup"]["profileId"], PROFILE);
        assert_eq!(document["startup"]["enabled"], true);
        assert_eq!(read_desired(&desired, uid).unwrap().profile_id, OTHER);

        let (_, delete) = applied(
            owner
                .execute(&request("profiles.delete", json!({"profileId": PROFILE})))
                .unwrap(),
        );
        assert!(delete.unwrap().changed);
        assert_eq!(owner.host().calls, ["observe", "observe"]);
        let document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        assert_eq!(document["profiles"].as_array().unwrap().len(), 1);
        assert_eq!(document["lastId"], OTHER);
        assert_eq!(document["startup"]["enabled"], false);
        assert_eq!(document["startup"]["profileId"], "");
        assert_eq!(read_desired(&desired, uid).unwrap().profile_id, OTHER);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_rename_quiesces_with_connected_intent_then_recovers_candidate() {
        let (root, store, desired, uid) = temp("active-rename");
        set_desired(&desired, uid, true, PROFILE, 3);
        let cutover_paths = cutover(&root, uid);
        let mut host = FakeHost::connected();
        host.python_lock_probe = Some(cutover_paths.operation_lock.clone());
        let mut owner = OfflineProfileOwner::new(host, desired.clone(), &store, cutover_paths, uid);
        let (cached, result) = applied(owner.execute(&request("profiles.rename", json!({"profileId": PROFILE, "name": "Renamed", "operationId": "rename-1", "expectedRevision": 0}))).unwrap());
        assert!(result.unwrap().changed);
        assert_eq!(cached.revision, 1);
        assert_eq!(
            owner.host().calls,
            [
                "observe", "observe", "stop", "discard", "observe", "observe", "prepare", "start",
                "observe", "commit"
            ]
        );
        let state = read_desired(&desired, uid).unwrap();
        assert!(state.connected);
        assert_eq!(state.profile_id, PROFILE);
        assert_eq!(state.generation, 3);
        let document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        assert_eq!(document["profiles"][0]["name"], "Renamed");
        assert!(!owner.host().python_lock_blocked.is_empty());
        assert!(
            owner
                .host()
                .python_lock_blocked
                .iter()
                .all(|blocked| *blocked)
        );

        let replay = owner.execute(&request("profiles.rename", json!({"profileId": PROFILE, "name": "Renamed", "operationId": "rename-1", "expectedRevision": 0}))).unwrap();
        assert_eq!(replay, ProfileOwnerExecution::Replay(cached));
        assert_eq!(owner.host().calls.len(), 10);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_change_and_revision_conflict_have_zero_lifecycle_or_write_effects() {
        let (root, store, desired, uid) = temp("revision");
        set_desired(&desired, uid, true, PROFILE, 1);
        let before = fs::read(&store).unwrap();
        let mut owner = OfflineProfileOwner::new(
            FakeHost::connected(),
            desired,
            &store,
            cutover(&root, uid),
            uid,
        );
        let (cached, outcome) = applied(
            owner
                .execute(&request(
                    "profiles.rename",
                    json!({"profileId": PROFILE, "name": "Private", "operationId": "same"}),
                ))
                .unwrap(),
        );
        assert!(!outcome.unwrap().changed);
        assert_eq!(cached.revision, 0);
        assert!(owner.host().calls.is_empty());
        assert_eq!(fs::read(&store).unwrap(), before);
        assert_eq!(
            owner
                .execute(&request(
                    "profiles.delete",
                    json!({"profileId": PROFILE, "expectedRevision": 9})
                ))
                .unwrap_err(),
            ProfileOwnerError::Coordinator(CoordinatorError::RevisionConflict)
        );
        assert!(owner.host().calls.is_empty());
        assert_eq!(fs::read(&store).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shared_python_lock_busy_is_uncached_and_same_operation_id_retries() {
        let (root, store, desired, uid) = temp("lock-exclusion");
        set_desired(&desired, uid, false, "", 0);
        let before = fs::read(&store).unwrap();
        let paths = cutover(&root, uid);
        let mut competing = Command::new("python3")
            .arg("-c")
            .arg(
                "import fcntl,sys\nwith open(sys.argv[1], 'a') as f:\n fcntl.flock(f.fileno(), fcntl.LOCK_EX)\n print('ready', flush=True)\n sys.stdin.read()",
            )
            .arg(&paths.operation_lock)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut ready = String::new();
        BufReader::new(competing.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        assert_eq!(ready.trim(), "ready");
        let mut owner =
            OfflineProfileOwner::new(FakeHost::disconnected(), desired, &store, paths, uid);
        let operation = request(
            "profiles.favorite",
            json!({
                "profileId": PROFILE,
                "enabled": true,
                "operationId": "retry-after-lock",
                "expectedRevision": 0
            }),
        );
        assert_eq!(
            owner.execute(&operation).unwrap(),
            ProfileOwnerExecution::UncachedPreflightFailure {
                revision: 0,
                error: ProfileTransactionError::Busy,
            }
        );
        assert_eq!(owner.revision(), 0);
        assert!(owner.host().calls.is_empty());
        assert_eq!(fs::read(&store).unwrap(), before);

        drop(competing.stdin.take());
        assert!(competing.wait().unwrap().success());
        let (cached, applied_result) = applied(owner.execute(&operation).unwrap());
        assert!(applied_result.unwrap().changed);
        assert_eq!(owner.revision(), 1);
        assert_eq!(cached.revision, 1);
        assert_eq!(
            owner.execute(&operation).unwrap(),
            ProfileOwnerExecution::Replay(cached)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_lock_boundary_maps_to_fixed_error_before_store_or_lifecycle() {
        let (root, store, desired, uid) = temp("unsafe-lock");
        let before = fs::read(&store).unwrap();
        fs::set_permissions(root.join("runtime"), fs::Permissions::from_mode(0o755)).unwrap();
        let mut owner = OfflineProfileOwner::new(
            FakeHost::disconnected(),
            desired,
            &store,
            cutover(&root, uid),
            uid,
        );
        let result = owner
            .execute(&request(
                "profiles.favorite",
                json!({"profileId": PROFILE, "enabled": true}),
            ))
            .unwrap();
        assert_eq!(
            result,
            ProfileOwnerExecution::UncachedPreflightFailure {
                revision: 0,
                error: ProfileTransactionError::Store,
            }
        );
        assert_eq!(owner.revision(), 0);
        assert!(owner.host().calls.is_empty());
        assert_eq!(fs::read(&store).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_delete_disconnects_durably_before_store_commit_and_repairs_pointers() {
        let (root, store, desired, uid) = temp("active-delete");
        set_desired(&desired, uid, true, PROFILE, 8);
        let mut owner = OfflineProfileOwner::new(
            FakeHost::connected(),
            desired.clone(),
            &store,
            cutover(&root, uid),
            uid,
        );
        let (_, result) = applied(
            owner
                .execute(&request("profiles.delete", json!({"profileId": PROFILE})))
                .unwrap(),
        );
        assert!(result.unwrap().changed);
        assert_eq!(
            owner.host().calls,
            ["observe", "observe", "stop", "discard", "observe"]
        );
        let state = read_desired(&desired, uid).unwrap();
        assert!(!state.connected);
        assert_eq!(state.generation, 9);
        let document: Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
        assert_eq!(document["profiles"].as_array().unwrap().len(), 1);
        assert_eq!(document["lastId"], OTHER);
        assert_eq!(document["startup"]["enabled"], false);
        assert_eq!(document["startup"]["profileId"], "");
        fs::remove_dir_all(root).unwrap();
    }

    struct FakePlan {
        changed: bool,
        commit_results:
            std::cell::RefCell<VecDeque<Result<PreparedWrite, ProfileMutationCommitError>>>,
        restore_result: Result<PreparedWrite, ProfileMutationCommitError>,
    }

    impl StorePlan for FakePlan {
        fn changed(&self) -> bool {
            self.changed
        }
        fn commit(
            &self,
            _lock: &MigrationLock,
            _paths: &CutoverPaths,
        ) -> Result<PreparedWrite, ProfileMutationCommitError> {
            self.commit_results
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(PreparedWrite::Changed))
        }
        fn restore(
            &self,
            _lock: &MigrationLock,
            _paths: &CutoverPaths,
        ) -> Result<PreparedWrite, ProfileMutationCommitError> {
            self.restore_result
        }
    }

    fn plan(
        commit: Result<PreparedWrite, ProfileMutationCommitError>,
        restore: Result<PreparedWrite, ProfileMutationCommitError>,
    ) -> FakePlan {
        FakePlan {
            changed: true,
            commit_results: std::cell::RefCell::new(VecDeque::from([commit])),
            restore_result: restore,
        }
    }

    fn lifecycle_fixture(
        label: &str,
        host: FakeHost,
        profile: &str,
    ) -> (PathBuf, LifecycleExecutor<FakeHost>) {
        let (root, _store, desired, uid) = temp(label);
        set_desired(&desired, uid, true, profile, 0);
        (root, LifecycleExecutor::new(host, desired, uid))
    }

    fn apply_fixture<P: StorePlan>(
        root: &Path,
        lifecycle: &mut LifecycleExecutor<FakeHost>,
        plan: &P,
        kind: ActionKind,
        profile_id: &str,
    ) -> Result<ProfileMutationOutcome, ProfileTransactionError> {
        let uid = fs::metadata(root).unwrap().uid();
        let paths = cutover(root, uid);
        let lock = MigrationLock::acquire(&paths, uid).unwrap();
        apply_transaction(lifecycle, plan, kind, profile_id, &lock, &paths)
    }

    #[test]
    fn active_rename_commit_failure_restores_and_recovers_old() {
        let (root, mut lifecycle) =
            lifecycle_fixture("rename-commit-fail", FakeHost::connected(), PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(
                Err(ProfileMutationCommitError::StoreIo),
                Ok(PreparedWrite::NoChange),
            ),
            ActionKind::Rename,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::TransitionFailedRestored);
        assert_eq!(lifecycle.actual(), crate::lifecycle::ActualState::Connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_rename_candidate_failure_restores_then_recovers_old() {
        let mut host = FakeHost::connected();
        host.start_results = VecDeque::from([false, true]);
        let (root, mut lifecycle) = lifecycle_fixture("rename-recover-fail", host, PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(Ok(PreparedWrite::Changed), Ok(PreparedWrite::Changed)),
            ActionKind::Rename,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::TransitionFailedRestored);
        assert_eq!(lifecycle.actual(), crate::lifecycle::ActualState::Connected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_restore_or_uncertain_quiesce_requires_manual_recovery() {
        let (root, mut lifecycle) =
            lifecycle_fixture("restore-fail", FakeHost::connected(), PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(
                Err(ProfileMutationCommitError::StoreIo),
                Err(ProfileMutationCommitError::StoreChanged),
            ),
            ActionKind::Rename,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::ManualRecoveryRequired);
        fs::remove_dir_all(root).unwrap();

        let mut host = FakeHost::connected();
        host.leave_running = true;
        let (root, mut lifecycle) = lifecycle_fixture("quiesce-uncertain", host, PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(Ok(PreparedWrite::Changed), Ok(PreparedWrite::Changed)),
            ActionKind::Rename,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::ManualRecoveryRequired);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_delete_commit_failure_reconnects_old_or_blocks() {
        let (root, mut lifecycle) =
            lifecycle_fixture("delete-rollback", FakeHost::connected(), PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(
                Err(ProfileMutationCommitError::StoreIo),
                Ok(PreparedWrite::NoChange),
            ),
            ActionKind::Delete,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::TransitionFailedRestored);
        assert_eq!(lifecycle.actual(), crate::lifecycle::ActualState::Connected);
        fs::remove_dir_all(root).unwrap();

        let mut host = FakeHost::connected();
        host.start_results = VecDeque::from([false]);
        let (root, mut lifecycle) = lifecycle_fixture("delete-reconnect-fail", host, PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(
                Err(ProfileMutationCommitError::StoreIo),
                Ok(PreparedWrite::NoChange),
            ),
            ActionKind::Delete,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::ManualRecoveryRequired);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_delete_unexpected_no_change_reconnects_old_or_blocks() {
        let (root, mut lifecycle) =
            lifecycle_fixture("delete-no-change", FakeHost::connected(), PROFILE);
        let error = apply_fixture(
            &root,
            &mut lifecycle,
            &plan(Ok(PreparedWrite::NoChange), Ok(PreparedWrite::NoChange)),
            ActionKind::Delete,
            PROFILE,
        )
        .unwrap_err();
        assert_eq!(error, ProfileTransactionError::TransitionFailedRestored);
        assert_eq!(lifecycle.actual(), crate::lifecycle::ActualState::Connected);
        let desired = DesiredPaths::below(&root.join("state"));
        let uid = fs::metadata(&root).unwrap().uid();
        let state = read_desired(&desired, uid).unwrap();
        assert!(state.connected);
        assert_eq!(state.profile_id, PROFILE);
        assert_eq!(state.mode, RoutingMode::Global);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_active_id_cannot_make_an_inactive_target_live_or_hide_active_target() {
        let (root, store, desired, uid) = temp("active-id-untrusted");
        set_desired(&desired, uid, true, PROFILE, 0);
        let mut document = store_value();
        document["activeId"] = json!(OTHER);
        fs::write(&store, serde_json::to_vec(&document).unwrap()).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
        let mut owner = OfflineProfileOwner::new(
            FakeHost::connected(),
            desired,
            &store,
            cutover(&root, uid),
            uid,
        );
        let (_, outcome) = applied(
            owner
                .execute(&request(
                    "profiles.rename",
                    json!({"profileId": PROFILE, "name": "Renamed"}),
                ))
                .unwrap(),
        );
        assert!(outcome.is_ok());
        assert!(owner.host().calls.contains(&"stop"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uncertain_lifecycle_is_cached_and_replayed_without_store_or_host_side_effects() {
        let (root, store, desired, uid) = temp("manual-replay");
        set_desired(&desired, uid, true, PROFILE, 0);
        let before = fs::read(&store).unwrap();
        let mut host = FakeHost::connected();
        host.observation.controller_ready = false;
        let mut owner = OfflineProfileOwner::new(host, desired, &store, cutover(&root, uid), uid);
        let initial_request = request(
            "profiles.rename",
            json!({
                "profileId": PROFILE,
                "name": "Renamed",
                "operationId": "manual-1",
                "expectedRevision": 0
            }),
        );
        let (cached, result) = applied(owner.execute(&initial_request).unwrap());
        assert_eq!(result, Err(ProfileTransactionError::ManualRecoveryRequired));
        assert_eq!(cached.revision, 0);
        assert_eq!(cached.error, Some(StableErrorCode::ManualRecoveryRequired));
        assert_eq!(owner.host().calls, ["observe"]);
        assert_eq!(fs::read(&store).unwrap(), before);
        assert_eq!(
            owner.execute(&initial_request).unwrap(),
            ProfileOwnerExecution::Replay(cached)
        );
        assert_eq!(owner.host().calls, ["observe"]);
        assert_eq!(
            owner
                .execute(&request(
                    "profiles.rename",
                    json!({
                        "profileId": PROFILE,
                        "name": "Different",
                        "operationId": "manual-1",
                        "expectedRevision": 0
                    })
                ))
                .unwrap_err(),
            ProfileOwnerError::Coordinator(CoordinatorError::OperationConflict)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_active_stop_never_commits_rename_or_delete() {
        for method in ["profiles.rename", "profiles.delete"] {
            let (root, store, desired, uid) = temp(method);
            set_desired(&desired, uid, true, PROFILE, 4);
            let before = fs::read(&store).unwrap();
            let mut host = FakeHost::connected();
            host.fail_stop = true;
            let mut owner =
                OfflineProfileOwner::new(host, desired.clone(), &store, cutover(&root, uid), uid);
            let params = if method == "profiles.rename" {
                json!({"profileId": PROFILE, "name": "Renamed"})
            } else {
                json!({"profileId": PROFILE})
            };
            let (_, outcome) = applied(owner.execute(&request(method, params)).unwrap());
            assert_eq!(
                outcome,
                Err(ProfileTransactionError::ManualRecoveryRequired)
            );
            assert_eq!(fs::read(&store).unwrap(), before);
            let state = read_desired(&desired, uid).unwrap();
            if method == "profiles.rename" {
                assert!(state.connected);
                assert_eq!(state.generation, 4);
            } else {
                assert!(!state.connected);
                assert_eq!(state.generation, 5);
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn public_errors_and_debug_output_never_echo_private_request_data() {
        let private = "private.example password=secret";
        for error in [
            ProfileTransactionError::Store,
            ProfileTransactionError::Conflict,
            ProfileTransactionError::ManualRecoveryRequired,
        ] {
            let public = format!("{error:?} {error}");
            assert!(!public.contains("private.example"));
            assert!(!public.contains("password"));
        }
        let protocol = parse_profile_mutation_request(&request(
            "profiles.rename",
            json!({"profileId": PROFILE, "name": private, "unexpected": private}),
        ))
        .err()
        .unwrap();
        assert!(!format!("{protocol:?} {protocol}").contains(private));
    }
}
