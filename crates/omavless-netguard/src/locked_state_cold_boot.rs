//! A new causal owner in a DIFFERENT boot, never stale-owner adoption.
use super::*;

/// Module-private production implementation is the already retained bound
/// creator. Test implementations model the independent early-manager gate;
/// receipt data and EffectPort alone cannot grant this extra startup authority.
pub(crate) trait ColdBootPort: EffectPort {
    fn begin_cold_create(&mut self) -> Result<(), crate::effect_port::EffectError>;
    fn cold_fence(&mut self) -> Result<(), crate::effect_port::EffectError>;
}

impl LockedState {
    pub(crate) fn prepare_cold_boot<K: ColdBootPort>(
        &mut self,
        namespace: NamespaceObservation,
        kernel: &mut K,
    ) -> Result<Response, ErrorCode> {
        self.cold_boot_with(namespace, kernel, |_| Ok(()))
    }

    pub(super) fn cold_boot_with<K: ColdBootPort>(
        &mut self,
        namespace: NamespaceObservation,
        kernel: &mut K,
        mut checkpoint: impl FnMut(Point) -> Result<(), ErrorCode>,
    ) -> Result<Response, ErrorCode> {
        if self.poisoned {
            return Err(REFUSED);
        }
        // Install the uncertainty latch BEFORE reads/provider code. A panic or
        // any refused startup never permits another cold attempt on this owner.
        self.poisoned = true;
        let NamespaceObservation::Canonical(epoch) = namespace else {
            return Err(REFUSED);
        };
        if epoch.boot == [0; 16] || epoch.namespace_epoch == [0; 16] || epoch.namespace_inode == 0 {
            return Err(REFUSED);
        }
        let mut state = self.snapshot(kernel)?;
        let uid = self
            .receipts
            .root()
            .map_err(|_| REFUSED)?
            .receipt_directory()
            .map_err(|_| REFUSED)?
            .2;
        if state.kernel
            != (EffectSnapshot {
                table: Table::Absent,
                identity: None,
            })
        {
            return Err(REFUSED);
        }
        // Existing stable no-effect cases keep their CURRENT-epoch admission
        // rules. In particular stale Closed/Retired is not silently rebased.
        if matches!(
            (state.marker, state.receipt),
            (Marker::Missing, ReceiptRead::Missing)
        ) || matches!((state.marker, state.receipt),
                (Marker::Closed(_), ReceiptRead::Durable(r)) if r.state() == ReceiptState::Retired)
        {
            self.admit(epoch, state)?;
            kernel.cold_fence().map_err(|_| REFUSED)?;
            if self.snapshot(kernel)? != state {
                return Err(REFUSED);
            }
            self.poisoned = false;
            return Ok(transaction::observe(Observation {
                marker: state.marker,
                table: state.kernel.table,
            }));
        }
        let (Marker::Armed(_), ReceiptRead::Durable(old)) = (state.marker, state.receipt) else {
            return Err(REFUSED);
        };
        if old.enrolled_uid() != uid
            || old.epoch().boot == epoch.boot
            || !matches!(old.state(), ReceiptState::Live { .. })
        {
            return Err(REFUSED);
        }
        let operation = old.operation().checked_add(1).ok_or(REFUSED)?;
        let pending =
            Receipt::transaction_record(uid, epoch, operation, ReceiptState::PendingCreate)
                .map_err(|_| REFUSED)?;
        // This one-use gate changes the original verifier's phase. ALL later
        // provider/file-publication fences require the same early-manager
        // condition until the startup holder explicitly consumes it.
        kernel.begin_cold_create().map_err(|_| REFUSED)?;
        checkpoint(Point::BeforePending)?;
        kernel.cold_fence().map_err(|_| REFUSED)?;
        if self.snapshot(kernel)? != state {
            return Err(REFUSED);
        }
        self.receipts
            .publish_with(state.receipt, pending, |boundary| {
                checkpoint(Point::ReceiptWrite {
                    terminal: false,
                    boundary,
                })
                .map_err(|_| StateError::UncertainWrite)?;
                kernel.cold_fence().map_err(|_| StateError::UncertainWrite)
            })
            .map_err(|_| REFUSED)?;
        state.receipt = ReceiptRead::Durable(pending);
        checkpoint(Point::Pending)?;
        if self.snapshot(kernel)? != state {
            return Err(REFUSED);
        }
        checkpoint(Point::BeforeKernel)?;
        let identity = kernel
            .create_if_absent(Policy::FullVpn)
            .map_err(|_| REFUSED)?;
        checkpoint(Point::KernelReturned)?;
        let observed = kernel.observe().map_err(|_| REFUSED)?;
        if !identity_in_epoch(identity, epoch)
            || observed
                != (EffectSnapshot {
                    table: Table::OwnedVerified(Policy::FullVpn),
                    identity: Some(identity),
                })
        {
            return Err(REFUSED);
        }
        state.kernel = observed;
        checkpoint(Point::KernelVerified)?;
        checkpoint(Point::BeforeTerminal)?;
        if self.snapshot(kernel)? != state {
            return Err(REFUSED);
        }
        let live = Receipt::transaction_record(
            uid,
            epoch,
            operation,
            ReceiptState::Live {
                handle: identity.table_handle,
            },
        )
        .map_err(|_| REFUSED)?;
        self.receipts
            .publish_with(state.receipt, live, |boundary| {
                checkpoint(Point::ReceiptWrite {
                    terminal: true,
                    boundary,
                })
                .map_err(|_| StateError::UncertainWrite)?;
                kernel.cold_fence().map_err(|_| StateError::UncertainWrite)
            })
            .map_err(|_| REFUSED)?;
        state.receipt = ReceiptRead::Durable(live);
        checkpoint(Point::Terminal)?;
        checkpoint(Point::BeforeReply)?;
        kernel.cold_fence().map_err(|_| REFUSED)?;
        if self.snapshot(kernel)? != state {
            return Err(REFUSED);
        }
        self.admit(epoch, state)?;
        // There is deliberately NO marker publication anywhere in this path:
        // Armed/high-water remains the original exact intent, not manufactured
        // Closed or a reset generation. The new handle is causal creator output.
        self.poisoned = false;
        Ok(transaction::observe(Observation {
            marker: state.marker,
            table: state.kernel.table,
        }))
    }
}
