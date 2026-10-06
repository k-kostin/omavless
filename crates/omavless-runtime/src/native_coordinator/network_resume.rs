// SPDX-License-Identifier: MIT
//! Original coordinator port for the test-only event continuation. No host
//! implements binding evidence in production and no IPC method exposes this.

use super::*;
use crate::desired::{DesiredState, ReconcileAction, reconcile};
use crate::network_recovery_receipt::{Fence, Observation, Refused};
use crate::network_resume::Context;
use crate::network_transition_plan::{Attempt, Current, OwnedState};
use sha2::{Digest, Sha256};

/// Test host evidence for an exact target, separate from local core liveness.
/// A real implementation needs reviewed DNS/route/protection/foreign-VPN
/// attribution. There is deliberately no blanket LifecycleHost implementation.
pub(crate) trait ResumeBinding: LifecycleHost {
    fn binding_safe_for(&mut self, desired: &DesiredState) -> bool;
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(crate) fn resume_lease(&self) -> Result<MigrationLock, Refused> {
        let lease = self.transaction.acquire_lock().map_err(|_| Refused)?;
        self.resume_owned(&lease)?;
        Ok(lease)
    }

    fn resume_owned(&self, lease: &MigrationLock) -> Result<(), Refused> {
        if !lease.authorizes(self.transaction.cutover_paths(), self.uid())
            || self.required_ownership.is_none_or(|fence| {
                fence.phase != OwnershipPhase::Rust
                    || !self
                        .transaction
                        .ownership_matches(fence.phase, fence.generation)
            })
            || self.transaction.blocked()
            || self.actual() == ActualState::ManualRecoveryRequired
            || crate::routing_preset::pending(self.transaction.desired_paths())
        {
            return Err(Refused);
        }
        Ok(())
    }

    pub(crate) fn resume_context(
        &self,
        boot: [u8; 16],
        instance: [u8; 16],
        epoch: u64,
    ) -> Result<Context, Refused> {
        let _lease = self.resume_lease()?;
        let desired = self.desired().map_err(|_| Refused)?;
        let store_digest = self.resume_store_digest(&desired)?;
        Ok(Context {
            fence: Fence {
                boot,
                owner_instance: instance,
                owner_generation: self.required_ownership.ok_or(Refused)?.generation,
                desired_revision: self.revision(),
                network_epoch: epoch,
            },
            desired,
            store_digest,
        })
    }

    fn resume_store_digest(&self, desired: &DesiredState) -> Result<[u8; 32], Refused> {
        crate::private_store_transaction::validate_store_path(self.store_path(), self.uid())
            .map_err(|_| Refused)?;
        let raw = omavless_store::read_private_utf8(self.store_path(), self.uid())
            .map_err(|_| Refused)?;
        let store =
            omavless_domain::private_store::parse_private_store(&raw).map_err(|_| Refused)?;
        if desired.connected
            && !store
                .list_projection()
                .profiles()
                .iter()
                .any(|profile| profile.id() == desired.profile_id && !profile.missing())
        {
            return Err(Refused);
        }
        Ok(Sha256::digest(raw.as_bytes()).into())
    }
}

pub(crate) struct Port<'a, H> {
    owner: &'a mut OfflineNativeCoordinator<H>,
    lease: &'a MigrationLock,
    context: &'a Context,
    now: u64,
    pub(crate) effect_calls: usize,
}

impl<'a, H: ResumeBinding> Port<'a, H> {
    pub(crate) fn new(
        owner: &'a mut OfflineNativeCoordinator<H>,
        lease: &'a MigrationLock,
        context: &'a Context,
        now: u64,
    ) -> Self {
        Self {
            owner,
            lease,
            context,
            now,
            effect_calls: 0,
        }
    }

    fn desired(&self) -> Result<DesiredState, Refused> {
        self.owner.resume_owned(self.lease)?;
        let desired = self.owner.desired().map_err(|_| Refused)?;
        if desired != self.context.desired
            || self.owner.resume_store_digest(&desired)? != self.context.store_digest
            || self.owner.revision() != self.context.fence.desired_revision
            || self.owner.required_ownership.ok_or(Refused)?.generation
                != self.context.fence.owner_generation
        {
            return Err(Refused);
        }
        Ok(desired)
    }
}

impl<H: ResumeBinding> Observation for Port<'_, H> {
    fn current(&mut self) -> Result<(Fence, Current), Refused> {
        let desired = self.desired()?;
        let idle = !self.owner.coordinator.active() && self.owner.coordinator.queued() == 0;
        // Busy and Off never even enter the lifecycle observer.
        let (owned, safe) = if !desired.connected || !idle {
            (OwnedState::Unknown, false)
        } else {
            let observation = self
                .owner
                .host_mut()
                .observe(&desired)
                .map_err(|_| Refused)?;
            let owned = match reconcile(&desired, observation) {
                ReconcileAction::AdoptConnected => OwnedState::VerifiedLocal,
                ReconcileAction::RecoverConnected => OwnedState::ProvenEmpty,
                _ => OwnedState::Unknown,
            };
            let safe = owned == OwnedState::ProvenEmpty
                && self.owner.host_mut().binding_safe_for(&desired);
            (owned, safe)
        };
        Ok((
            self.context.fence,
            Current {
                owner_generation: self.context.fence.owner_generation,
                desired_revision: self.owner.revision(),
                network_epoch: self.context.fence.network_epoch,
                now_tick: self.now,
                desired_connected: desired.connected,
                mutation_idle: idle,
                owned,
                attempt: Attempt::OutcomeUnknown,
                recovery_safety_proven: safe,
            },
        ))
    }

    fn synthetic_effect(&mut self) -> Result<(), Refused> {
        let desired = self.desired()?;
        // Repeat actual binding proof at the effect boundary. Its implementation
        // must be observation only; no arbitrary host callback is registered.
        if !self.owner.host_mut().binding_safe_for(&desired) {
            return Err(Refused);
        }
        let request = MutationRequest::new(
            MutationKind::Other,
            None,
            Some(self.context.fence.desired_revision),
            crate::mutation::MutationDigest::from_semantic_bytes(b"t4-network-resume-fixture-v1"),
        )
        .map_err(|_| Refused)?;
        let SubmitOutcome::Queued { token } = self
            .owner
            .coordinator
            .submit(request)
            .map_err(|_| Refused)?
        else {
            return Err(Refused);
        };
        let BeginOutcome::Started(active) =
            self.owner.coordinator.begin_next().map_err(|_| Refused)?
        else {
            return Err(Refused);
        };
        if active.token != token {
            return Err(Refused);
        }
        self.effect_calls += 1;
        let outcome = self
            .owner
            .transaction
            .lifecycle_mut()
            .recover_network_empty(&desired);
        let result = match outcome {
            Ok(_) => {
                let pointers = crate::private_store_transaction::prepare_pointer_mutation(
                    self.owner.store_path(),
                    self.owner.uid(),
                    omavless_domain::private_store::CompatibilityPointerTarget::Connected {
                        profile_id: desired.profile_id,
                    },
                );
                match pointers.and_then(|plan| {
                    plan.commit_locked(self.lease, self.owner.transaction.cutover_paths())
                }) {
                    Ok(_) => Ok(NativeMutationOutcome::Connection(
                        ConnectionTransactionOutcome {
                            changed: true,
                            pruned: 0,
                        },
                    )),
                    Err(_) => Err(NativeTransactionError::Connection(
                        ConnectionTransactionError::ManualRecoveryRequired,
                    )),
                }
            }
            Err(_) => Err(NativeTransactionError::Connection(
                ConnectionTransactionError::ManualRecoveryRequired,
            )),
        };
        // An event failure never opens a second implicit recovery. Preserve the
        // coordinator barrier even for an ordinary cleaned-up start failure.
        let succeeded = result.is_ok();
        self.owner
            .finish(token, result, false)
            .map_err(|_| Refused)?;
        if succeeded { Ok(()) } else { Err(Refused) }
    }
}
