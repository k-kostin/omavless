//! Inactive lifetime composition, not a canonical namespace authenticator.
//! No non-test provider exists. Receipts, matching integers, fixture witnesses
//! and the legacy EffectPort cannot be converted into a provider here.

use crate::effect_port::{
    EffectError, EffectIdentity, EffectPort, EffectSnapshot, ExchangeBoundary,
};
use crate::launch_acquisition::AcquiredCreator;
use crate::listener_admission::AdmittedListener;
use crate::locked_state::LockedState;
use crate::policy::Policy;
use crate::receipt::{HostEpoch, NamespaceObservation};
use crate::session_owner_candidate::{SessionOwner, SessionProgress};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Boundary {
    Admission,
    Exchange(ExchangeBoundary),
    BeforeObserve,
    AfterObserve,
    BeforeCreate,
    AfterCreate,
    BeforeReplace,
    AfterReplace,
    BeforeDelete,
    AfterDelete,
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// Future implementation must OWN the authenticated launch anchor, original
/// namespace descriptor, creator socket and complete live-create provenance.
/// It must prohibit namespace switching structurally, verify namespace type/ID
/// and socket cookie using reviewed safe APIs, and retain subsystem continuity.
/// `HostEpoch` is only the consistency projection of those held resources.
/// No serialized epoch/receipt constructor or blanket EffectPort impl exists.
/// The only implementation today is synthetic and compiled under cfg(test).
pub(crate) trait CanonicalCreator: EffectPort + sealed::Sealed {
    fn retained_epoch(&mut self, boundary: Boundary) -> Result<HostEpoch, EffectError>;
}

pub(crate) struct BoundEffects<C: CanonicalCreator> {
    creator: AcquiredCreator<C>,
    epoch: Option<HostEpoch>,
    sealed: bool,
    // Moving a creator to a different thread could change the namespace
    // relation. This fence does not itself prevent same-thread setns/ABA.
    _same_thread: PhantomData<Rc<()>>,
}

impl<C: CanonicalCreator> BoundEffects<C> {
    fn new(creator: AcquiredCreator<C>) -> Self {
        Self {
            creator,
            epoch: None,
            sealed: true,
            _same_thread: PhantomData,
        }
    }

    fn admit(&mut self) -> NamespaceObservation {
        if let Ok(epoch) = self.creator.retained_epoch(Boundary::Admission)
            && epoch.boot != [0; 16]
            && epoch.namespace_epoch != [0; 16]
            && epoch.namespace_inode != 0
        {
            self.epoch = Some(epoch);
            self.sealed = false;
        }
        self.namespace()
    }

    fn namespace(&self) -> NamespaceObservation {
        self.epoch.map_or(
            NamespaceObservation::Unproven,
            NamespaceObservation::Canonical,
        )
    }

    fn check(&mut self, boundary: Boundary) -> Result<(), EffectError> {
        if self.sealed {
            return Err(EffectError::UnavailableOrUncertain);
        }
        // Set before entering provider code: unwind cannot revive this owner.
        self.sealed = true;
        let epoch = self.creator.retained_epoch(boundary)?;
        if Some(epoch) != self.epoch {
            return Err(EffectError::UnavailableOrUncertain);
        }
        self.sealed = false;
        Ok(())
    }

    fn operation<T>(
        &mut self,
        before: Boundary,
        effect: impl FnOnce(&mut AcquiredCreator<C>) -> Result<(T, HostEpoch), EffectError>,
    ) -> Result<T, EffectError> {
        self.check(before)?;
        self.sealed = true;
        let (value, epoch) = effect(&mut self.creator)?;
        if Some(epoch) != self.epoch {
            return Err(EffectError::UnavailableOrUncertain);
        }
        self.sealed = false;
        Ok(value)
    }
}

impl<C: CanonicalCreator> crate::effect_port::sealed::Sealed for BoundEffects<C> {}
impl<C: CanonicalCreator> EffectPort for BoundEffects<C> {
    fn exchange_boundary(&mut self, boundary: ExchangeBoundary) -> Result<(), EffectError> {
        self.check(Boundary::Exchange(boundary))
    }
    fn observe(&mut self) -> Result<EffectSnapshot, EffectError> {
        self.operation(Boundary::BeforeObserve, AcquiredCreator::observe)
    }
    fn create_if_absent(&mut self, policy: Policy) -> Result<EffectIdentity, EffectError> {
        self.operation(Boundary::BeforeCreate, |c| c.create_if_absent(policy))
    }
    fn replace_owned(
        &mut self,
        identity: EffectIdentity,
        policy: Policy,
    ) -> Result<EffectIdentity, EffectError> {
        self.operation(Boundary::BeforeReplace, |c| {
            c.replace_owned(identity, policy)
        })
    }
    fn delete_owned(&mut self, identity: EffectIdentity) -> Result<(), EffectError> {
        self.operation(Boundary::BeforeDelete, |c| c.delete_owned(identity))
    }
}

/// Holds one listener, one enrollment/store lock and the inseparable provider.
/// There is deliberately no clean-shutdown/recovery API yet. Drop retains the
/// whole owner for process lifetime, including after an unwind: it cannot
/// silently close an ownership socket, release the lock, disarm or unlink.
/// Process death is not prevented and persistent recovery remains unresolved.
pub(crate) struct AuthoritySession<C: CanonicalCreator> {
    owner: ManuallyDrop<SessionOwner<BoundEffects<C>>>,
    sealed: bool,
}

impl<C: CanonicalCreator> AuthoritySession<C> {
    pub(crate) fn from_admitted(
        listener: AdmittedListener,
        state: LockedState,
        creator: AcquiredCreator<C>,
    ) -> std::io::Result<Self> {
        let bound = BoundEffects::new(creator);
        SessionOwner::from_admitted_retaining(listener, state, bound, BoundEffects::admit).map(
            |owner| Self {
                owner,
                sealed: false,
            },
        )
    }

    pub(crate) fn poll_one(&mut self) -> SessionProgress {
        if self.sealed {
            return SessionProgress::AuthorityLost;
        }
        self.sealed = true;
        let progress = self.owner.poll_one();
        // This stronger composition has no automatic retry of any uncertain
        // accept, malformed client, failed reply or lost provenance. Legacy
        // SessionOwner policy stays unchanged for its separate model callers.
        if matches!(progress, SessionProgress::Idle | SessionProgress::Served) {
            self.sealed = false;
        }
        progress
    }

    #[cfg(test)]
    pub(crate) fn test_state(&self) -> &LockedState {
        self.owner.test_state()
    }

    /// Synthetic-only teardown. Normal compilation deliberately has no release.
    #[cfg(test)]
    pub(crate) fn release_synthetic(self) {
        ManuallyDrop::into_inner(self.owner)
            .into_test_kernel()
            .creator
            .release_synthetic();
    }
}
