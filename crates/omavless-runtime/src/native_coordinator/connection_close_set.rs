// SPDX-License-Identifier: MIT
// Product-only bounded group; never a loop over Single confirmations.
use crate::mutation::{ExternalSetAdmission, ExternalSetMember, ExternalSetReceipt};

struct PendingSet {
    handles: Vec<OpaqueToken>,
    ticket: OpaqueToken,
    expiry: Instant,
}
struct ActiveSet {
    token: ExternalCloseToken,
    worker: crate::conditional_close_candidate::GroupWorker,
}
pub(crate) struct SetConfirmation {
    pub(crate) ticket: OpaqueToken,
    pub(crate) rows: Vec<CloseDisplayRow>,
}
pub(crate) enum SetAdmission {
    Replay(ExternalSetReceipt),
    Discover(Box<SetDiscovery>),
}
pub(crate) struct SetDiscovery {
    discovery: CloseDiscovery,
    context: Context,
    origin: PreviewOrigin,
    selected: Vec<PreviewRow>,
    token: ExternalCloseToken,
    expiry: Instant,
}
enum FreshSet {
    Ready(Box<CloseDiscovered>, Vec<ObservedRow>),
    Changed(crate::conditional_close_candidate::CloseEpochRetirement),
}
pub(crate) struct SetDiscovered {
    identity: Arc<()>,
    context: Context,
    token: ExternalCloseToken,
    expiry: Instant,
    count: usize,
    result: Result<FreshSet, NativeOwnerError>,
}
impl SetDiscovery {
    pub(crate) fn observe(self) -> SetDiscovered {
        let identity = Arc::clone(&self.discovery.identity);
        let count = self.selected.len();
        let result = (|| {
            if Instant::now() >= self.expiry {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let mut fresh = self.discovery.observe()?;
            if Instant::now() >= self.expiry {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let mut selected = Vec::with_capacity(count);
            for expected in &self.selected {
                let index = fresh.rows.iter().position(|row| {
                    fresh
                        .observation
                        .session()
                        .matches_preview(&self.origin, row, expected)
                });
                let Some(index) = index else {
                    let retired = fresh
                        .observation
                        .into_session()
                        .retire_before_effect()
                        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
                    return Ok(FreshSet::Changed(retired));
                };
                selected.push(fresh.rows.remove(index));
            }
            fresh.rows.clear();
            Ok(FreshSet::Ready(Box::new(fresh), selected))
        })();
        SetDiscovered {
            identity,
            context: self.context,
            token: self.token,
            expiry: self.expiry,
            count,
            result,
        }
    }
}
impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(crate) fn prepare_connection_set(
        &mut self,
        handles: Vec<OpaqueToken>,
    ) -> Result<SetConfirmation, NativeOwnerError> {
        if handles.is_empty()
            || handles.len() > 128
            || handles.iter().collect::<BTreeSet<_>>().len() != handles.len()
        {
            return Err(NativeOwnerError::Invariant);
        }
        let _lease = self.batch_lock()?;
        let preview = self
            .connection_close
            .preview
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        self.close_context_matches(&preview.context)?;
        let mut rows = Vec::with_capacity(handles.len());
        for h in &handles {
            let row = preview
                .rows
                .iter()
                .find(|(handle, _)| handle == h)
                .ok_or(NativeOwnerError::RecordNotFound)?;
            rows.push(CloseDisplayRow {
                handle: *h,
                display: row.1.display.clone(),
            });
        }
        let ticket = self.connection_close.entropy()?;
        self.connection_close.pending = None;
        self.connection_close.pending_set = Some(PendingSet {
            handles,
            ticket,
            expiry: Instant::now() + CONFIRMATION_LIFETIME,
        });
        Ok(SetConfirmation { ticket, rows })
    }
    pub(crate) fn admit_connection_set(
        &mut self,
        operation: &str,
        revision: u64,
        handles: Vec<OpaqueToken>,
        ticket: OpaqueToken,
    ) -> Result<SetAdmission, NativeOwnerError> {
        if handles.is_empty()
            || handles.len() > 128
            || handles.iter().collect::<BTreeSet<_>>().len() != handles.len()
        {
            return Err(NativeOwnerError::Invariant);
        }
        let mut semantic = b"omavless-owner-set-conditional-close-v1\0".to_vec();
        let state = self
            .batch
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        semantic.extend_from_slice(&(state.instance.len() as u64).to_be_bytes());
        semantic.extend_from_slice(state.instance.as_bytes());
        semantic.extend_from_slice(&(handles.len() as u64).to_be_bytes());
        for handle in &handles {
            semantic.extend_from_slice(&handle.0);
        }
        semantic.extend_from_slice(&ticket.0);
        semantic.extend_from_slice(&revision.to_be_bytes());
        let long_id = state.registry.has_operation_id(operation);
        let token = match self.coordinator.reserve_external_set(
            operation,
            revision,
            MutationDigest::from_semantic_bytes(&semantic),
            long_id,
            handles.len(),
        )? {
            ExternalSetAdmission::Replay(r) => return Ok(SetAdmission::Replay(r)),
            ExternalSetAdmission::Reserved(t) => t,
        };
        let count = handles.len();
        let result = (|| {
            let _lease = self.batch_lock()?;
            let pending = self
                .connection_close
                .pending_set
                .take()
                .ok_or(NativeOwnerError::OwnershipUnavailable)?;
            if pending.handles != handles
                || pending.ticket != ticket
                || Instant::now() >= pending.expiry
            {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let mut preview = self
                .connection_close
                .preview
                .take()
                .ok_or(NativeOwnerError::OwnershipUnavailable)?;
            self.close_context_matches(&preview.context)?;
            let mut selected = Vec::with_capacity(count);
            for handle in handles {
                let index = preview
                    .rows
                    .iter()
                    .position(|(h, _)| *h == handle)
                    .ok_or(NativeOwnerError::RecordNotFound)?;
                selected.push(preview.rows.remove(index).1);
            }
            drop(_lease);
            let discovery = self.capture_connection_close_inner(Some(&token))?;
            Ok(SetAdmission::Discover(Box::new(SetDiscovery {
                discovery,
                context: preview.context,
                origin: preview.origin,
                selected,
                token: token.clone(),
                expiry: pending.expiry,
            })))
        })();
        if result.is_err() {
            self.coordinator
                .finish_external_set(&token, vec![ExternalSetMember::NotAttempted; count])?;
        }
        result
    }
    pub(crate) fn complete_connection_set(
        &mut self,
        result: SetDiscovered,
    ) -> Result<Option<ExternalSetReceipt>, NativeOwnerError> {
        let token = result.token;
        let count = result.count;
        let completed = (|| {
            if !self.coordinator.external_close_reservation_current(&token)
                || !self
                    .connection_close
                    .discovery
                    .as_ref()
                    .is_some_and(|id| Arc::ptr_eq(id, &result.identity))
                || Instant::now() >= result.expiry
            {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let lease = self.batch_lock()?;
            self.close_context_matches(&result.context)?;
            match result.result? {
                FreshSet::Changed(retired) => {
                    self.host_mut().complete_close_retirement(&retired);
                    if self.host().close_epoch_admission()
                        != crate::lifecycle::CloseEpochAdmission::Ready
                        || Instant::now() >= retired.retirement_deadline()
                    {
                        return Err(NativeOwnerError::OwnershipUnavailable);
                    }
                    self.connection_close.discovery = None;
                    self.connection_close.cancellation = None;
                    Ok(Some(self.coordinator.finish_external_set(
                        &token,
                        vec![ExternalSetMember::NotAttempted; count],
                    )?))
                }
                FreshSet::Ready(mut fresh, selected) => {
                    if !lease.authorizes(&fresh.context.cutover_paths, fresh.context.uid)
                        || !fresh.observation.session_mut().proves_live_for_scheduling()
                    {
                        return Err(NativeOwnerError::OwnershipUnavailable);
                    }
                    let permit = fresh
                        .observation
                        .session_mut()
                        .qualified_pair_permit()
                        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
                    #[cfg(test)]
                    let permit = permit.or_else(|| fresh.observation.fixture_permit());
                    let permit = permit.ok_or(NativeOwnerError::OwnershipUnavailable)?;
                    let session = fresh
                        .observation
                        .into_session()
                        .authorize_set(EffectProof(fresh.context), result.expiry, selected, permit)
                        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
                    let worker = self
                        .connection_close
                        .scheduler
                        .start_set(session)
                        .map_err(|_| NativeOwnerError::Invariant)?;
                    self.connection_close.discovery = None;
                    self.connection_close.active_set = Some(ActiveSet {
                        token: token.clone(),
                        worker,
                    });
                    Ok(None)
                }
            }
        })();
        if completed.is_err() {
            self.host_mut().refuse_close_epoch();
            self.coordinator
                .finish_external_set(&token, vec![ExternalSetMember::NotAttempted; count])?;
        }
        completed
    }
    fn poll_connection_set(&mut self) -> Result<(), NativeOwnerError> {
        let Some(active) = self.connection_close.active_set.as_mut() else {
            return Ok(());
        };
        let Some(completed) = active.worker.poll_epoch() else {
            return Ok(());
        };
        let active = self
            .connection_close
            .active_set
            .take()
            .ok_or(NativeOwnerError::Invariant)?;
        use crate::conditional_close_candidate::GroupMemberOutcome as M;
        let members = completed
            .outcome()
            .members()
            .iter()
            .map(|m| match m {
                M::Closed => ExternalSetMember::Closed,
                M::Missing => ExternalSetMember::Missing,
                M::Changed => ExternalSetMember::Changed,
                M::Unsupported => ExternalSetMember::Unsupported,
                M::Unknown => ExternalSetMember::Unknown,
                M::NotAttempted => ExternalSetMember::NotAttempted,
            })
            .collect();
        self.coordinator
            .finish_external_set(&active.token, members)?;
        self.connection_close.cancellation = None;
        self.host_mut().complete_group_close_epoch(&completed);
        Ok(())
    }
    pub(crate) fn connection_set_receipt(
        &mut self,
        operation: &str,
    ) -> Result<Option<Option<ExternalSetReceipt>>, NativeOwnerError> {
        self.poll_connection_close()?;
        self.coordinator
            .external_set_receipt(operation)
            .map_err(Into::into)
    }
}
