// SPDX-License-Identifier: MIT
// Product-only inert preview and fresh final-confirm discovery. No default path.
use crate::conditional_close_candidate::{PreviewOrigin, PreviewRow};

struct InertPreview {
    context: Context,
    origin: PreviewOrigin,
    rows: Vec<(OpaqueToken, PreviewRow)>,
}
pub(crate) struct ClosePreviewReady {
    identity: Arc<()>,
    context: Context,
    origin: PreviewOrigin,
    rows: Vec<PreviewRow>,
    retired: crate::conditional_close_candidate::CloseEpochRetirement,
}
impl CloseDiscovery {
    pub(crate) fn product_preview(&self) -> bool {
        self.product_preview
    }
    pub(crate) fn observe_preview(self) -> Result<ClosePreviewReady, NativeOwnerError> {
        let discovered = self.observe()?;
        let origin = discovered
            .observation
            .session()
            .preview_origin()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        let rows = discovered
            .rows
            .into_iter()
            .map(ObservedRow::into_preview)
            .collect();
        let retired = discovered
            .observation
            .into_session()
            .retire_before_effect()
            .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
        Ok(ClosePreviewReady {
            identity: discovered.identity,
            context: discovered.context,
            origin,
            rows,
            retired,
        })
    }
}

pub(crate) enum ProductConfirmAdmission {
    Replay(ExternalCloseReceipt),
    Discover(Box<CloseConfirmDiscovery>),
}
pub(crate) struct CloseConfirmDiscovery {
    discovery: CloseDiscovery,
    context: Context,
    origin: PreviewOrigin,
    selected: PreviewRow,
    token: ExternalCloseToken,
    expiry: Instant,
}
enum FreshSelection {
    Ready(Box<CloseDiscovered>, ObservedRow),
    Changed(crate::conditional_close_candidate::CloseEpochRetirement),
}
pub(crate) struct CloseConfirmedDiscovery {
    identity: Arc<()>,
    context: Context,
    token: ExternalCloseToken,
    expiry: Instant,
    result: Result<FreshSelection, NativeOwnerError>,
}
impl CloseConfirmDiscovery {
    pub(crate) fn observe(self) -> CloseConfirmedDiscovery {
        let identity = Arc::clone(&self.discovery.identity);
        let result = (|| {
            if Instant::now() >= self.expiry {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let mut fresh = self.discovery.observe()?;
            if Instant::now() >= self.expiry {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let position = fresh.rows.iter().position(|row| {
                fresh
                    .observation
                    .session()
                    .matches_preview(&self.origin, row, &self.selected)
            });
            match position {
                Some(index) => {
                    let row = fresh.rows.remove(index);
                    fresh.rows.clear();
                    Ok(FreshSelection::Ready(Box::new(fresh), row))
                }
                None => {
                    let retired = fresh
                        .observation
                        .into_session()
                        .retire_before_effect()
                        .map_err(|_| NativeOwnerError::OwnershipUnavailable)?;
                    Ok(FreshSelection::Changed(retired))
                }
            }
        })();
        CloseConfirmedDiscovery {
            identity,
            context: self.context,
            token: self.token,
            expiry: self.expiry,
            result,
        }
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(crate) fn retain_product_preview(
        &mut self,
        result: Result<ClosePreviewReady, NativeOwnerError>,
    ) -> Result<Vec<CloseDisplayRow>, NativeOwnerError> {
        let result = (|| {
            let ready = result?;
            if !self
                .connection_close
                .discovery
                .as_ref()
                .is_some_and(|id| Arc::ptr_eq(id, &ready.identity))
                || !self
                    .connection_close
                    .retiring
                    .as_ref()
                    .is_some_and(|id| Arc::ptr_eq(id, &ready.identity))
                || Instant::now() >= ready.retired.retirement_deadline()
            {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let _lease = self.batch_lock()?;
            self.close_context_matches(&ready.context)?;
            self.host_mut().complete_close_retirement(&ready.retired);
            if self.host().close_epoch_admission() != crate::lifecycle::CloseEpochAdmission::Ready
                || Instant::now() >= ready.retired.retirement_deadline()
            {
                return Err(NativeOwnerError::OwnershipUnavailable);
            }
            let mut rows = Vec::with_capacity(ready.rows.len());
            let mut display = Vec::with_capacity(ready.rows.len());
            for row in ready.rows {
                let handle = self.connection_close.entropy()?;
                display.push(CloseDisplayRow {
                    handle,
                    display: row.display.clone(),
                });
                rows.push((handle, row));
            }
            self.connection_close.preview = Some(InertPreview {
                context: ready.context,
                origin: ready.origin,
                rows,
            });
            self.connection_close.discovery = None;
            self.connection_close.retiring = None;
            self.connection_close.cancellation = None;
            Ok(display)
        })();
        if result.is_err() {
            self.host_mut().refuse_close_epoch();
        }
        result
    }

    fn prepare_product_preview(
        &mut self,
        handle: OpaqueToken,
    ) -> Result<CloseConfirmation, NativeOwnerError> {
        let _lease = self.batch_lock()?;
        let preview = self
            .connection_close
            .preview
            .as_ref()
            .ok_or(NativeOwnerError::OwnershipUnavailable)?;
        self.close_context_matches(&preview.context)?;
        let display = preview
            .rows
            .iter()
            .find(|(h, _)| *h == handle)
            .ok_or(NativeOwnerError::RecordNotFound)?
            .1
            .display
            .clone();
        let ticket = self.connection_close.entropy()?;
        self.connection_close.pending = Some(Pending {
            handle,
            ticket,
            expiry: Instant::now() + CONFIRMATION_LIFETIME,
        });
        Ok(CloseConfirmation { ticket, display })
    }

    pub(crate) fn admit_product_confirm(
        &mut self,
        operation: &str,
        revision: u64,
        handle: OpaqueToken,
        ticket: OpaqueToken,
    ) -> Result<ProductConfirmAdmission, NativeOwnerError> {
        let token = match self.reserve_connection_close(operation, revision, handle, ticket)? {
            ExternalCloseAdmission::Replay(receipt) => {
                return Ok(ProductConfirmAdmission::Replay(receipt));
            }
            ExternalCloseAdmission::Reserved(token) => token,
        };
        let result = (|| {
            let _lease = self.batch_lock()?;
            let pending = self
                .connection_close
                .pending
                .take()
                .ok_or(NativeOwnerError::OwnershipUnavailable)?;
            if pending.handle != handle
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
            let index = preview
                .rows
                .iter()
                .position(|(h, _)| *h == handle)
                .ok_or(NativeOwnerError::RecordNotFound)?;
            let selected = preview.rows.remove(index).1;
            drop(_lease);
            let discovery = self.capture_connection_close_inner(Some(&token))?;
            Ok(ProductConfirmAdmission::Discover(Box::new(
                CloseConfirmDiscovery {
                    discovery,
                    context: preview.context,
                    origin: preview.origin,
                    selected,
                    token: token.clone(),
                    expiry: pending.expiry,
                },
            )))
        })();
        if result.is_err() {
            self.coordinator
                .finish_external_close(&token, ExternalCloseOutcome::RefusedBeforeWrite)?;
        }
        result
    }

    pub(crate) fn complete_product_confirm(
        &mut self,
        result: CloseConfirmedDiscovery,
    ) -> Result<Option<ExternalCloseReceipt>, NativeOwnerError> {
        let token = result.token;
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
                FreshSelection::Changed(retired) => {
                    self.host_mut().complete_close_retirement(&retired);
                    if self.host().close_epoch_admission()
                        != crate::lifecycle::CloseEpochAdmission::Ready
                        || Instant::now() >= retired.retirement_deadline()
                    {
                        return Err(NativeOwnerError::OwnershipUnavailable);
                    }
                    self.connection_close.discovery = None;
                    self.connection_close.cancellation = None;
                    Ok(Some(ExternalCloseOutcome::RefusedBeforeWrite))
                }
                FreshSelection::Ready(mut fresh, selected) => {
                    if !fresh.observation.session_mut().proves_live_for_scheduling() {
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
                    let snapshot = Snapshot {
                        context: fresh.context,
                        expiry: result.expiry,
                        observation: fresh.observation,
                        rows: Vec::new(),
                    };
                    self.connection_close.discovery = None;
                    self.schedule_permitted_connection_close(
                        snapshot, selected, &token, permit, &lease,
                    )
                }
            }
        })();
        match completed {
            Ok(None) => Ok(None),
            Ok(Some(outcome)) => Ok(Some(
                self.coordinator.finish_external_close(&token, outcome)?,
            )),
            Err(error) => {
                self.host_mut().refuse_close_epoch();
                self.coordinator
                    .finish_external_close(&token, ExternalCloseOutcome::RefusedBeforeWrite)?;
                Err(error)
            }
        }
    }
}
