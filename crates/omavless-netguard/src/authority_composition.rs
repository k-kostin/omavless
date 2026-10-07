//! Default-build inactive lifetime composition. The opt-in service provider
//! is private; historical missing-provider obligations below describe default
//! builds. Receipts/integers/fixtures/legacy ports cannot acquire that provider.

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

#[cfg(feature = "netguard-cold-bootstrap")]
impl<C: CanonicalCreator> crate::locked_state::ColdBootPort for BoundEffects<C> {
    fn begin_cold_create(&mut self) -> Result<(), EffectError> {
        self.check(Boundary::Admission)?;
        self.sealed = true;
        self.creator.begin_cold_create()?;
        self.sealed = false;
        Ok(())
    }
    fn cold_fence(&mut self) -> Result<(), EffectError> {
        self.check(Boundary::Admission)
    }
}

#[cfg(feature = "netguard-cold-bootstrap")]
pub(crate) trait StartupReadyPort: EffectPort {
    fn notify_ready(&mut self) -> Result<(), EffectError>;
}
#[cfg(feature = "netguard-cold-bootstrap")]
impl<C: CanonicalCreator> StartupReadyPort for BoundEffects<C> {
    fn notify_ready(&mut self) -> Result<(), EffectError> {
        if self.sealed {
            return Err(EffectError::UnavailableOrUncertain);
        }
        self.sealed = true;
        self.creator.notify_ready()?;
        self.sealed = false;
        Ok(())
    }
}

#[cfg(feature = "netguard-cold-bootstrap")]
struct StartupParts<C: CanonicalCreator> {
    state: LockedState,
    bound: BoundEffects<C>,
}

/// Installed BEFORE any cold mutation. This exact lock/creator graph survives
/// refusal, unwind or ordinary Drop. It is moved once, never reconstructed from
/// a returned status, decoded receipt, namespace projection or creator handle.
#[cfg(feature = "netguard-cold-bootstrap")]
pub(crate) struct StartupAuthority<C: CanonicalCreator> {
    held: ManuallyDrop<StartupParts<C>>,
    prepared: bool,
    attempted: bool,
}
#[cfg(feature = "netguard-cold-bootstrap")]
impl<C: CanonicalCreator> StartupAuthority<C> {
    pub(crate) fn new(state: LockedState, creator: AcquiredCreator<C>) -> Self {
        Self {
            held: ManuallyDrop::new(StartupParts {
                state,
                bound: BoundEffects::new(creator),
            }),
            prepared: false,
            attempted: false,
        }
    }
    pub(crate) fn prepare(&mut self) -> Result<(), EffectError> {
        if self.attempted {
            return Err(EffectError::UnavailableOrUncertain);
        }
        self.attempted = true;
        let StartupParts { state, bound } = &mut *self.held;
        let namespace = bound.admit();
        if bound.sealed || !matches!(namespace, NamespaceObservation::Canonical(_)) {
            return Err(EffectError::UnavailableOrUncertain);
        }
        let status = state
            .prepare_cold_boot(namespace, bound)
            .map_err(|_| EffectError::UnavailableOrUncertain)?;
        if state
            .request(crate::protocol::Request::Status {}, namespace, bound)
            .map_err(|_| EffectError::UnavailableOrUncertain)?
            != status
        {
            return Err(EffectError::UnavailableOrUncertain);
        }
        // All final cold state/current-manager guards precede publication.
        // This irreversible phase transfer does NOT send READY or allow accept.
        bound.sealed = true;
        bound.creator.consume_startup()?;
        bound.sealed = false;
        self.prepared = true;
        Ok(())
    }
    pub(crate) fn into_session(
        self,
        listener: AdmittedListener,
    ) -> std::io::Result<AuthoritySession<C>> {
        // Retain both arguments BEFORE even a preparation refusal. Extracting
        // the SAME graph into from_admitted_retaining has no fallible step; its
        // first action installs the whole SessionOwner before listener setup.
        let transfer = ManuallyDrop::new((self, listener));
        if !transfer.0.prepared {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let (startup, listener) = ManuallyDrop::into_inner(transfer);
        let StartupParts { state, bound } = ManuallyDrop::into_inner(startup.held);
        let owner = SessionOwner::from_admitted_retaining(listener, state, bound, |bound| {
            bound.namespace()
        })?;
        Ok(AuthoritySession {
            owner,
            sealed: true,
            boot_pending: true,
        })
    }
    #[cfg(test)]
    pub(crate) fn release_synthetic(self) {
        let parts = ManuallyDrop::into_inner(self.held);
        parts.bound.creator.release_synthetic();
        drop(parts.state);
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
    #[cfg(feature = "netguard-cold-bootstrap")]
    boot_pending: bool,
}

impl<C: CanonicalCreator> AuthoritySession<C> {
    #[cfg(feature = "netguard-service-core")]
    pub(crate) fn recover_one(
        &mut self,
        stream: std::os::unix::net::UnixStream,
    ) -> SessionProgress {
        if self.sealed {
            return SessionProgress::AuthorityLost;
        }
        self.sealed = true;
        let progress = self.owner.recover_one(stream);
        if progress == SessionProgress::Served {
            self.sealed = false;
        }
        progress
    }
    #[cfg(any(not(feature = "netguard-cold-bootstrap"), test))]
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
                #[cfg(feature = "netguard-cold-bootstrap")]
                boot_pending: false,
            },
        )
    }

    #[cfg(feature = "netguard-cold-bootstrap")]
    pub(crate) fn publish_ready(&mut self) -> Result<(), EffectError> {
        if !self.boot_pending || !self.sealed {
            return Err(EffectError::UnavailableOrUncertain);
        }
        self.boot_pending = false; // Any attempted delivery permanently consumes it.
        self.owner.publish_startup_ready()?;
        self.sealed = false;
        Ok(())
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
