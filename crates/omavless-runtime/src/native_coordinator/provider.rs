// SPDX-License-Identifier: MIT
//! Owner-fenced live rule updates sharing the existing long-operation registry.
use super::batch::{ActiveCancellation, BatchOwnerState};
use super::*;
use crate::desired::{DesiredState, RoutingMode};
use crate::long_operation::{CommitFence, LongOperationError, LongOperationToken};
use crate::long_operation_protocol::{LongOperationMethod, parse_provider_refresh_start};
use crate::private_store_transaction::prepare_private_store_write;
use crate::provider_refresh::{
    ProviderRefreshCancellation, ProviderRefreshError, ProviderRefreshStep, ProviderRefreshWork,
    RuleProviderTransport,
};
use omavless_domain::private_store::{apply_rule_provider_refresh_timestamp, parse_private_store};
use omavless_mihomo::rule_provider::RuleProviderTarget;
use omavless_store::read_private_utf8;
use sha2::{Digest, Sha256};

pub struct ProviderRefreshSnapshot {
    revision: u64,
    desired: DesiredState,
    store: [u8; 32],
    config: [u8; 32],
}
pub enum ProviderRefreshAdmission {
    Replay(Value),
    Discover(ProviderRefreshSnapshot),
}
pub struct NativeProviderRefresh {
    instance: String,
    token: LongOperationToken,
    snapshot: ProviderRefreshSnapshot,
    work: ProviderRefreshWork,
    failure: Option<ProviderRefreshError>,
}
impl NativeProviderRefresh {
    #[must_use]
    pub fn supervisor_ticket(&self) -> NativeBatchTicket {
        NativeBatchTicket {
            instance: self.instance.clone(),
            token: self.token,
        }
    }
    pub fn step<T: RuleProviderTransport>(
        &mut self,
        transport: &T,
        pool: &crate::remote_fetch::RemoteFetchPool,
    ) -> Result<ProviderRefreshStep, ProviderRefreshError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.work.step(transport, pool);
        if let Err(error) = result {
            self.failure = Some(error);
        }
        result
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    fn provider_snapshot_locked(&mut self) -> Result<ProviderRefreshSnapshot, NativeOwnerError> {
        let desired = self
            .transaction
            .desired()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        if !desired.connected
            || desired.mode != RoutingMode::Rule
            || self.actual() != ActualState::Connected
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        // The committed owner's actual state is the lifecycle authority.
        // Controller liveness/identity is proved by detached discovery/PUT,
        // never by host.observe while this short owner lease is held.
        let path = self.transaction.store_path();
        crate::private_store_transaction::validate_store_path(path, self.transaction.uid())
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let store = read_private_utf8(path, self.transaction.uid())
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        parse_private_store(&store).map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let config_path = path
            .parent()
            .ok_or(NativeOwnerError::Invariant)?
            .join("config.yaml");
        let config = read_private_utf8(&config_path, self.transaction.uid())
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        if config.len() > omavless_domain::config::MAX_TEMPLATE_BYTES {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        Ok(ProviderRefreshSnapshot {
            revision: self.revision(),
            desired,
            store: Sha256::digest(store.as_bytes()).into(),
            config: Sha256::digest(config.as_bytes()).into(),
        })
    }
    fn provider_snapshot_matches(
        &mut self,
        snapshot: &ProviderRefreshSnapshot,
    ) -> Result<(), NativeOwnerError> {
        if self.revision() != snapshot.revision {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict,
            ));
        }
        let current = self.provider_snapshot_locked()?;
        if current.revision != snapshot.revision
            || current.desired != snapshot.desired
            || current.store != snapshot.store
            || current.config != snapshot.config
        {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionConflict,
            ));
        }
        Ok(())
    }
    pub fn preflight_provider_refresh(
        &mut self,
        request: &Value,
    ) -> Result<ProviderRefreshAdmission, NativeOwnerError> {
        let parsed = parse_provider_refresh_start(request)?;
        let _lock = self.batch_lock()?;
        let ordinary = self
            .coordinator
            .operation_id_in_use(parsed.operation_id())?;
        let revision = self.revision();
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        if state.stopped {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        if state.registry.has_operation_id(parsed.operation_id()) {
            state
                .registry
                .start_method(
                    LongOperationMethod::RuleProviderRefresh,
                    parsed.instance_id(),
                    parsed.operation_id(),
                    parsed.digest(),
                    parsed.expected_revision(),
                    revision,
                    0,
                    ordinary,
                )
                .map_err(NativeOwnerError::LongOperation)?;
            return Ok(ProviderRefreshAdmission::Replay(
                state
                    .registry
                    .projection(parsed.instance_id(), parsed.operation_id())
                    .map_err(NativeOwnerError::LongOperation)?
                    .result_value()?,
            ));
        }
        if parsed.instance_id() != state.instance {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::InstanceMismatch,
            ));
        }
        if ordinary {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::OperationConflict,
            ));
        }
        if parsed
            .expected_revision()
            .is_some_and(|expected| expected != revision)
        {
            return Err(NativeOwnerError::LongOperation(
                LongOperationError::RevisionConflict,
            ));
        }
        if state.active.is_some() {
            return Err(NativeOwnerError::LongOperation(LongOperationError::Busy));
        }
        if revision == omavless_control_protocol::MAX_REVISION {
            return Err(NativeOwnerError::Coordinator(
                CoordinatorError::RevisionExhausted,
            ));
        }
        self.provider_snapshot_locked()
            .map(ProviderRefreshAdmission::Discover)
    }
    pub fn start_provider_refresh(
        &mut self,
        request: &Value,
        snapshot: ProviderRefreshSnapshot,
        targets: Vec<RuleProviderTarget>,
    ) -> Result<Option<NativeProviderRefresh>, NativeOwnerError> {
        let parsed = parse_provider_refresh_start(request)?;
        match self.preflight_provider_refresh(request)? {
            ProviderRefreshAdmission::Replay(_) => return Ok(None),
            ProviderRefreshAdmission::Discover(_) => {}
        }
        let _lock = self.batch_lock()?;
        self.provider_snapshot_matches(&snapshot)?;
        if targets.is_empty() {
            return Err(NativeOwnerError::Provider(
                ProviderRefreshError::NoRemoteProviders,
            ));
        }
        let revision = self.revision();
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let started = state
            .registry
            .start_method(
                LongOperationMethod::RuleProviderRefresh,
                parsed.instance_id(),
                parsed.operation_id(),
                parsed.digest(),
                parsed.expected_revision(),
                revision,
                targets.len(),
                false,
            )
            .map_err(NativeOwnerError::LongOperation)?;
        let token = started.token();
        state
            .registry
            .begin(token, revision)
            .map_err(NativeOwnerError::LongOperation)?;
        let cancellation = ProviderRefreshCancellation::default();
        state.active = Some((token, ActiveCancellation::Provider(cancellation.clone())));
        Ok(Some(NativeProviderRefresh {
            instance: state.instance.clone(),
            token,
            snapshot,
            work: ProviderRefreshWork::discovered(targets, cancellation),
            failure: None,
        }))
    }
    fn provider_handle(
        state: &BatchOwnerState,
        job: &NativeProviderRefresh,
    ) -> Result<(), NativeOwnerError> {
        if state.stopped
            || state.instance != job.instance
            || state.active.as_ref().map(|entry| entry.0) != Some(job.token)
        {
            return Err(NativeOwnerError::OwnershipUnavailable);
        }
        Ok(())
    }
    pub fn publish_provider_refresh_progress(
        &mut self,
        job: &NativeProviderRefresh,
    ) -> Result<(), NativeOwnerError> {
        let _lock = self.batch_lock()?;
        self.provider_snapshot_matches(&job.snapshot)?;
        let state = self
            .batch
            .as_mut()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        Self::provider_handle(state, job)?;
        state
            .registry
            .advance(job.token, job.work.progress().0)
            .map_err(NativeOwnerError::LongOperation)
    }
    pub fn complete_provider_refresh<
        N: FnOnce() -> u64,
        V: FnOnce() -> Result<(), ProviderRefreshError>,
    >(
        &mut self,
        job: NativeProviderRefresh,
        now: N,
        verify_identity: V,
    ) -> Result<(), NativeOwnerError> {
        let mut state = self
            .batch
            .take()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        let outcome = (|| {
            Self::provider_handle(&state, &job)?;
            let token = job.token;
            state
                .registry
                .advance(token, job.work.progress().0)
                .map_err(NativeOwnerError::LongOperation)?;
            let result = self.commit_provider_refresh(&mut state, job, now, verify_identity);
            state.active = None;
            match result {
                Ok(true) => state
                    .registry
                    .finish_success(token, self.revision())
                    .map_err(NativeOwnerError::LongOperation),
                Ok(false) => Ok(()),
                Err(error) => {
                    state
                        .registry
                        .finish_failure(token, self.revision(), error.stable_code())
                        .map_err(NativeOwnerError::LongOperation)?;
                    Err(error)
                }
            }
        })();
        self.batch = Some(state);
        outcome
    }
    fn commit_provider_refresh<
        N: FnOnce() -> u64,
        V: FnOnce() -> Result<(), ProviderRefreshError>,
    >(
        &mut self,
        state: &mut BatchOwnerState,
        job: NativeProviderRefresh,
        now: N,
        verify_identity: V,
    ) -> Result<bool, NativeOwnerError> {
        let _lock = self.batch_lock()?;
        self.provider_snapshot_matches(&job.snapshot)?;
        // The registry cancellation fence precedes interpretation of any late
        // transport error: accepted cancellation wins, but cannot undo PUTs.
        if state
            .registry
            .fence_commit(job.token, self.revision())
            .map_err(NativeOwnerError::LongOperation)?
            == CommitFence::Cancelled
        {
            return Ok(false);
        }
        verify_identity().map_err(NativeOwnerError::Provider)?;
        if let Some(error) = job.failure {
            return Err(NativeOwnerError::Provider(error));
        }
        job.work.finish().map_err(NativeOwnerError::Provider)?;
        let prepared = prepare_private_store_write(
            self.transaction.store_path(),
            self.transaction.uid(),
            |input| {
                let changed = apply_rule_provider_refresh_timestamp(input, now())?;
                Ok((changed.payload().to_vec(), true))
            },
        )
        .map_err(|_| NativeOwnerError::Provider(ProviderRefreshError::Rejected))?;
        let request = MutationRequest::new(
            MutationKind::Other,
            None,
            Some(job.snapshot.revision),
            crate::mutation::MutationDigest::from_semantic_bytes(b"native-rule-provider-stamp-v1"),
        )?;
        let SubmitOutcome::Queued { token } = self.coordinator.submit(request)? else {
            return Err(NativeOwnerError::Invariant);
        };
        match self.coordinator.begin_next()? {
            BeginOutcome::Started(active) if active.token == token => {}
            _ => return Err(NativeOwnerError::Invariant),
        }
        let result = match prepared.commit_locked(&_lock, self.transaction.cutover_paths()) {
            Ok(_) => Ok(()),
            Err(_)
                if prepared
                    .restore_locked(&_lock, self.transaction.cutover_paths())
                    .is_ok() =>
            {
                Err(NativeOwnerError::Provider(ProviderRefreshError::Rejected))
            }
            Err(_) => {
                self.transaction.block();
                Err(NativeOwnerError::ManualRecoveryRequired)
            }
        };
        self.coordinator.finish(
            token,
            match result {
                Ok(_) => MutationResult::Success,
                Err(error) => MutationResult::Failure(error.stable_code()),
            },
        )?;
        result.map(|_| true)
    }
}
