// SPDX-License-Identifier: MIT
//! Owner-side admission and volatile results. This module never launches a core,
//! performs DNS/controller I/O, writes the store, or registers a live method.
use super::batch::ActiveCancellation;
use super::*;
use crate::long_operation::{CommitFence, LongOperationError, LongOperationToken};
use crate::long_operation_protocol::{
    LongOperationMethod, parse_profile_probe_results, parse_profile_probe_start,
    parse_subscription_probe_results, parse_subscription_probe_start,
};
use omavless_mihomo::probe_plan::ProbeResult;
use omavless_profile::canonical::CanonicalProfile;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Notification only; the operation registry remains the publication authority.
#[derive(Clone, Default)]
pub struct ProbeCancellation(Arc<AtomicBool>);
impl ProbeCancellation {
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }
    #[must_use]
    pub fn requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct Snapshot {
    revision: u64,
    desired: crate::desired::DesiredState,
    store: [u8; 32],
    template: [u8; 32],
    active_config: Option<[u8; 32]>,
}

enum ProbeTarget {
    Subscription(String),
    Profiles(Option<String>),
}
impl ProbeTarget {
    fn method(&self) -> LongOperationMethod {
        match self {
            Self::Subscription(_) => LongOperationMethod::SubscriptionProbe,
            Self::Profiles(_) => LongOperationMethod::ProfileProbe,
        }
    }
}

/// Non-cloneable private worker capability. Keep its ticket before spawning;
/// dropping a job must be followed by ticket abort by the scheduler supervisor.
pub struct NativeSubscriptionProbe {
    instance: String,
    operation: String,
    token: LongOperationToken,
    target: ProbeTarget,
    snapshot: Snapshot,
    profiles: Vec<(String, CanonicalProfile)>,
    template: String,
    active_config: Option<String>,
    cancellation: ProbeCancellation,
}
impl NativeSubscriptionProbe {
    #[must_use]
    pub fn supervisor_ticket(&self) -> NativeBatchTicket {
        NativeBatchTicket {
            instance: self.instance.clone(),
            token: self.token,
        }
    }
    #[must_use]
    pub fn profiles(&self) -> &[(String, CanonicalProfile)] {
        &self.profiles
    }
    #[must_use]
    pub fn private_template(&self) -> &str {
        &self.template
    }
    #[must_use]
    pub fn private_active_config(&self) -> Option<&str> {
        self.active_config.as_deref()
    }
    #[must_use]
    pub fn cancellation(&self) -> ProbeCancellation {
        self.cancellation.clone()
    }
}

pub(super) struct RetainedProbeResults {
    instance: String,
    operation: String,
    target: ProbeTarget,
    snapshot: Snapshot,
    rows: Vec<(String, ProbeResult)>,
}

fn conflict() -> NativeOwnerError {
    NativeOwnerError::LongOperation(LongOperationError::RevisionConflict)
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Read only under the serialized owner. A known ID can only replay or
    /// conflict in ordinary admission, so it must not cancel unrelated work.
    /// This is not authorization: dispatch must still validate full intent and
    /// re-admit under the owner after any released lock or deferred operation.
    #[must_use]
    pub fn mutation_operation_known(&self, request: &Value) -> bool {
        request["params"]["operationId"].as_str().is_some_and(|id| {
            self.coordinator.operation_id_in_use(id).unwrap_or(false)
                || self
                    .batch
                    .as_ref()
                    .is_some_and(|state| state.registry.has_operation_id(id))
        })
    }
    /// Unknown auxiliary cleanup is a global lifecycle barrier, never merely a
    /// failed latency measurement. The caller must not proceed to host effects.
    pub fn mark_auxiliary_recovery_required(&mut self) {
        self.auxiliary_recovery_required = true;
        self.transaction.block();
    }
    fn probe_inputs_locked(
        &self,
    ) -> Result<(Snapshot, String, String, Option<String>), NativeOwnerError> {
        let path = self.transaction.store_path();
        crate::private_store_transaction::validate_store_path(path, self.transaction.uid())
            .map_err(|_| NativeOwnerError::Invariant)?;
        let store = omavless_store::read_private_utf8(path, self.transaction.uid())
            .map_err(|_| NativeOwnerError::Invariant)?;
        let template_path = path
            .parent()
            .ok_or(NativeOwnerError::Invariant)?
            .join("route-template.yaml");
        let template = omavless_store::read_private_utf8(&template_path, self.transaction.uid())
            .map_err(|_| NativeOwnerError::Invariant)?;
        if template.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
            return Err(NativeOwnerError::Invariant);
        }
        let active_path = path
            .parent()
            .ok_or(NativeOwnerError::Invariant)?
            .join("config.yaml");
        let active_config = match std::fs::symlink_metadata(&active_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(NativeOwnerError::Invariant),
            Ok(_) => {
                let text = omavless_store::read_private_utf8(&active_path, self.transaction.uid())
                    .map_err(|_| NativeOwnerError::Invariant)?;
                if text.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
                    return Err(NativeOwnerError::Invariant);
                }
                Some(text)
            }
        };
        Ok((
            Snapshot {
                revision: self.revision(),
                desired: self
                    .transaction
                    .desired()
                    .map_err(|_| NativeOwnerError::Invariant)?,
                store: Sha256::digest(store.as_bytes()).into(),
                template: Sha256::digest(template.as_bytes()).into(),
                active_config: active_config
                    .as_ref()
                    .map(|text| Sha256::digest(text.as_bytes()).into()),
            },
            store,
            template,
            active_config,
        ))
    }
    fn probe_snapshot_matches(&self, expected: &Snapshot) -> Result<(), NativeOwnerError> {
        if self.revision() != expected.revision {
            return Err(conflict());
        }
        let (actual, _, _, _) = self.probe_inputs_locked()?;
        if actual.store != expected.store
            || actual.template != expected.template
            || actual.desired != expected.desired
            || actual.active_config != expected.active_config
        {
            return Err(conflict());
        }
        Ok(())
    }
    /// Exact replay precedes all private snapshot access. Only one shared batch
    /// may be active, including provider and subscription-refresh work.
    pub fn start_subscription_probe(
        &mut self,
        request: &Value,
    ) -> Result<Option<NativeSubscriptionProbe>, NativeOwnerError> {
        let (meta, target) = if request["method"] == "profiles.probe" {
            let parsed = parse_profile_probe_start(request)?;
            (parsed.metadata, ProbeTarget::Profiles(parsed.profile_id))
        } else {
            let parsed = parse_subscription_probe_start(request)?;
            (
                parsed.metadata,
                ProbeTarget::Subscription(parsed.subscription_id),
            )
        };
        let _lock = self.batch_lock()?;
        let revision = self.revision();
        let ordinary = self.coordinator.operation_id_in_use(meta.operation_id())?;
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.stopped {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if state.registry.has_operation_id(meta.operation_id()) {
            state
                .registry
                .start_method(
                    target.method(),
                    meta.instance_id(),
                    meta.operation_id(),
                    meta.digest(),
                    meta.expected_revision(),
                    revision,
                    0,
                    ordinary,
                )
                .map_err(NativeOwnerError::LongOperation)?;
            return Ok(None);
        }
        if meta.instance_id() != state.instance {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::InstanceMismatch,
            ));
        }
        if ordinary {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::OperationConflict,
            ));
        }
        if meta
            .expected_revision()
            .is_some_and(|value| value != revision)
        {
            return Err(conflict());
        }
        if state.active.is_some() {
            return Err(NativeOwnerError::LongOperation(LongOperationError::Busy));
        }
        let (snapshot, store, template, active_config) = self.probe_inputs_locked()?;
        let store = omavless_domain::private_store::parse_private_store(&store)
            .map_err(|_| NativeOwnerError::Invariant)?;
        let profiles = match &target {
            ProbeTarget::Subscription(id) => store.into_subscription_probe_profiles(id),
            ProbeTarget::Profiles(id) => store.into_profile_probe_profiles(id.as_deref()),
        }
        .map_err(|error| match error {
            PrivateStoreError::SubscriptionNotFound | PrivateStoreError::ProfileNotFound => {
                NativeOwnerError::RecordNotFound
            }
            _ => NativeOwnerError::Invariant,
        })?;
        if profiles.len() > 256 {
            return Err(NativeOwnerError::Invariant);
        }
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let token = state
            .registry
            .start_method(
                target.method(),
                meta.instance_id(),
                meta.operation_id(),
                meta.digest(),
                meta.expected_revision(),
                revision,
                profiles.len(),
                false,
            )
            .map_err(NativeOwnerError::LongOperation)?
            .token();
        state
            .registry
            .begin(token, revision)
            .map_err(NativeOwnerError::LongOperation)?;
        let cancellation = ProbeCancellation::default();
        state.active = Some((token, ActiveCancellation::Probe(cancellation.clone())));
        Ok(Some(NativeSubscriptionProbe {
            instance: state.instance.clone(),
            operation: meta.operation_id().to_owned(),
            token,
            target,
            snapshot,
            profiles,
            template,
            active_config,
            cancellation,
        }))
    }
    pub fn publish_subscription_probe_progress(
        &mut self,
        job: &NativeSubscriptionProbe,
        completed: usize,
    ) -> Result<(), NativeOwnerError> {
        let _lock = self.batch_lock()?;
        self.probe_snapshot_matches(&job.snapshot)?;
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.stopped
            || state.instance != job.instance
            || state.active.as_ref().map(|v| v.0) != Some(job.token)
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        state
            .registry
            .advance(job.token, completed)
            .map_err(NativeOwnerError::LongOperation)
    }
    /// Caller must reap its auxiliary child before entering this owner boundary.
    /// Cancellation wins even over a late executor failure. No persistent write
    /// or canonical revision increment is permitted for volatile measurements.
    pub fn complete_subscription_probe(
        &mut self,
        job: NativeSubscriptionProbe,
        result: Result<Vec<ProbeResult>, StableErrorCode>,
    ) -> Result<(), NativeOwnerError> {
        if matches!(result, Err(StableErrorCode::ManualRecoveryRequired)) {
            self.mark_auxiliary_recovery_required();
        }
        let mut state = self
            .batch
            .take()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let outcome = (|| {
            if state.stopped
                || state.instance != job.instance
                || state.active.as_ref().map(|v| v.0) != Some(job.token)
            {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let checked = (|| {
                let _lock = self.batch_lock()?;
                self.probe_snapshot_matches(&job.snapshot)?;
                let rows = result.map_err(NativeOwnerError::Probe)?;
                if rows.len() != job.profiles.len()
                    || rows.iter().any(|row| {
                        (!row.resolved && row.reachable)
                            || (row.reachable && !(0..=60000).contains(&row.latency_ms))
                            || (!row.reachable && row.latency_ms != -1)
                    })
                {
                    return Err(NativeOwnerError::Invariant);
                }
                state
                    .registry
                    .advance(job.token, rows.len())
                    .map_err(NativeOwnerError::LongOperation)?;
                if state
                    .registry
                    .fence_commit(job.token, self.revision())
                    .map_err(NativeOwnerError::LongOperation)?
                    == CommitFence::Cancelled
                {
                    return Ok(None);
                }
                Ok(Some(rows))
            })();
            state.active = None;
            match checked {
                Ok(Some(rows)) => {
                    state
                        .registry
                        .finish_success(job.token, self.revision())
                        .map_err(NativeOwnerError::LongOperation)?;
                    self.probe_results.push_back(RetainedProbeResults {
                        instance: job.instance,
                        operation: job.operation,
                        target: job.target,
                        snapshot: job.snapshot,
                        rows: job.profiles.into_iter().map(|v| v.0).zip(rows).collect(),
                    });
                    while self.probe_results.len() > 16 {
                        self.probe_results.pop_front();
                    }
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(error) => {
                    state
                        .registry
                        .finish_failure(job.token, self.revision(), error.stable_code())
                        .map_err(NativeOwnerError::LongOperation)?;
                    Err(error)
                }
            }
        })();
        self.batch = Some(state);
        outcome
    }
    /// Explicit private UI result read. Opaque IDs are never ordinary status or
    /// support output. Results survive panel close, not daemon restart; only the
    /// last 16 successful batches are retained and stale snapshots are refused.
    pub fn subscription_probe_results(
        &mut self,
        request: &Value,
    ) -> Result<Value, NativeOwnerError> {
        let profile_result = request["method"] == "profiles.probe_results";
        let parsed = if profile_result {
            parse_profile_probe_results(request)?
        } else {
            parse_subscription_probe_results(request)?
        };
        let _lock = self.batch_lock()?;
        let state = self
            .batch
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.stopped {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        state
            .registry
            .projection(parsed.instance_id(), parsed.operation_id())
            .map_err(NativeOwnerError::LongOperation)?;
        let result = self
            .probe_results
            .iter()
            .find(|item| {
                item.instance == parsed.instance_id() && item.operation == parsed.operation_id()
            })
            .ok_or(NativeOwnerError::RecordNotFound)?;
        self.probe_snapshot_matches(&result.snapshot)?;
        let mut projection = serde_json::json!({"version":1,"results":result.rows.iter().map(|(id,row)|serde_json::json!({"id":id,"resolved":row.resolved,"reachable":row.reachable,"latencyMs":row.latency_ms})).collect::<Vec<_>>()});
        match &result.target {
            ProbeTarget::Subscription(id) if !profile_result => {
                projection["subscriptionId"] = serde_json::json!(id)
            }
            ProbeTarget::Profiles(id) if profile_result => {
                projection["profileId"] = serde_json::json!(id)
            }
            _ => return Err(NativeOwnerError::RecordNotFound),
        }
        Ok(projection)
    }
}
