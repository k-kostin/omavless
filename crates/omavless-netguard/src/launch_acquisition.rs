//! Default-build inactive acquisition; historical obligations below describe
//! that default. The opt-in service_origin private factory is the successor.
//! Exact configuration bytes remain evidence, never trusted launch provenance.

use crate::authority_composition::{Boundary, CanonicalCreator};
use crate::effect_port::{EffectError, EffectIdentity, EffectSnapshot};
use crate::policy::Policy;
use crate::receipt::HostEpoch;
use std::fs::File;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::rc::Rc;
use std::thread::{self, ThreadId};

// A proposed fixed contract, NOT an installed unit or an accepted package.
pub(crate) const UNIT: &[u8] = include_bytes!("launch_acquisition.unit-contract");
const EXECUTABLE: &str = "/usr/lib/omavless/omavless-netguard";

#[cfg(feature = "netguard-service-core")]
#[path = "launch_service_origin.rs"]
mod service_origin;
#[cfg(feature = "netguard-service-core")]
pub(crate) use service_origin::acquire_fixed_service;

/// A caller can validate a supplied configuration, but cannot turn it into
/// canonical authority. No PID, UID, INVOCATION_ID or namespace ID is accepted.
pub(crate) struct ConfigurationEvidence {
    _private: (),
}

impl ConfigurationEvidence {
    pub(crate) fn validate(
        package: &str,
        executable: &str,
        unit: &[u8],
        argv: &[&str],
        drop_ins: &[&str],
    ) -> Result<Self, EffectError> {
        if package != "omavless-netguard"
            || executable != EXECUTABLE
            || unit != UNIT
            || argv != [EXECUTABLE, "serve"]
            || !drop_ins.is_empty()
        {
            return Err(EffectError::UnavailableOrUncertain);
        }
        Ok(Self { _private: () })
    }
}

/// Original owners supplied BEFORE sandboxing by a future trusted launch.
/// Private fields: not reconstructible from a path, integer or user descriptor.
/// The socket must be the exclusive creator's actual socket, not a second
/// matching socket. The absent production acquisition implementation must
/// establish that relationship as well as package/manager provenance.
struct Originals {
    anchor: File,
    thread_namespace: File,
    creator_socket: OwnedFd,
    owner_thread: ThreadId,
    verifier: Box<dyn OriginalVerifier>,
}

/// A lexical borrow of original resources, not a serialized authority token.
/// No Clone/Copy/Send/Sync and no owning-descriptor extraction or replacement.
struct LaunchBorrow<'a> {
    anchor: BorrowedFd<'a>,
    thread_namespace: BorrowedFd<'a>,
    creator_socket: BorrowedFd<'a>,
    _same_thread: PhantomData<Rc<()>>,
}

/// Only implementations inside this module can exist. There is deliberately
/// none outside cfg(test). Future implementation must use adopted safe APIs
/// against THESE borrowed owners, validate network type/nonzero ID/cookie,
/// retained trusted launch and structural transition prohibition. An equality
/// check, root UID, PID 1, fixture or cached manager reply cannot implement it.
trait OriginalVerifier {
    fn recheck(&mut self, originals: LaunchBorrow<'_>) -> Result<(), EffectError>;
}

struct Retained<C> {
    originals: Originals,
    creator: C,
}

/// Required input of AuthoritySession. Not constructible in normal builds.
/// Holding this opaque owner is mandatory; a bare CanonicalCreator cannot enter
/// the session anymore. It retains resources even on constructor failure/drop.
/// The type is an implementation prerequisite, NOT proof that a provider exists.
pub(crate) struct AcquiredCreator<C: CanonicalCreator> {
    retained: ManuallyDrop<Retained<C>>,
    sealed: bool,
    _same_thread: PhantomData<Rc<()>>,
}

impl<C: CanonicalCreator> AcquiredCreator<C> {
    /// The original owners remain held throughout the callback, including any
    /// exchange/effect it performs. A panic leaves the latch set. The callback
    /// cannot extract a borrow or replace the held descriptors through this API.
    /// Structural no-setns remains a trusted-launch obligation: !Send alone is
    /// not proof against same-thread switch-and-return or a hostile kernel/root.
    // Module-private: callers must never receive &mut C, because even a
    // non-escaping borrow would permit mem::replace of the paired creator.
    fn with_lease<T>(
        &mut self,
        callback: impl FnOnce(&mut C) -> Result<T, EffectError>,
    ) -> Result<T, EffectError> {
        if self.sealed {
            return Err(EffectError::UnavailableOrUncertain);
        }
        self.sealed = true;
        let Retained { originals, creator } = &mut *self.retained;
        if thread::current().id() != originals.owner_thread {
            return Err(EffectError::UnavailableOrUncertain);
        }
        let borrow = || LaunchBorrow {
            anchor: originals.anchor.as_fd(),
            thread_namespace: originals.thread_namespace.as_fd(),
            creator_socket: originals.creator_socket.as_fd(),
            _same_thread: PhantomData,
        };
        service_cut!(VerifierBefore);
        originals.verifier.recheck(borrow())?;
        service_cut!(Creator);
        let value = callback(creator)?;
        service_cut!(VerifierAfter);
        originals.verifier.recheck(borrow())?;
        self.sealed = false;
        Ok(value)
    }

    pub(crate) fn retained_epoch(&mut self, boundary: Boundary) -> Result<HostEpoch, EffectError> {
        self.with_lease(|creator| creator.retained_epoch(boundary))
    }

    pub(crate) fn observe(&mut self) -> Result<(EffectSnapshot, HostEpoch), EffectError> {
        self.with_lease(|creator| {
            let value = creator.observe()?;
            Ok((value, creator.retained_epoch(Boundary::AfterObserve)?))
        })
    }

    pub(crate) fn create_if_absent(
        &mut self,
        policy: Policy,
    ) -> Result<(EffectIdentity, HostEpoch), EffectError> {
        self.with_lease(|creator| {
            let value = creator.create_if_absent(policy)?;
            Ok((value, creator.retained_epoch(Boundary::AfterCreate)?))
        })
    }

    pub(crate) fn replace_owned(
        &mut self,
        identity: EffectIdentity,
        policy: Policy,
    ) -> Result<(EffectIdentity, HostEpoch), EffectError> {
        self.with_lease(|creator| {
            let value = creator.replace_owned(identity, policy)?;
            Ok((value, creator.retained_epoch(Boundary::AfterReplace)?))
        })
    }

    pub(crate) fn delete_owned(
        &mut self,
        identity: EffectIdentity,
    ) -> Result<((), HostEpoch), EffectError> {
        self.with_lease(|creator| {
            creator.delete_owned(identity)?;
            Ok(((), creator.retained_epoch(Boundary::AfterDelete)?))
        })
    }

    // The ONLY constructor is synthetic. It cannot be reached by a normal
    // product, arbitrary descriptor supplier or the configuration validator.
    #[cfg(test)]
    pub(crate) fn synthetic(creator: C) -> Self {
        Self::synthetic_controlled(creator, Rc::new(std::cell::Cell::new(false)))
    }

    #[cfg(test)]
    pub(crate) fn synthetic_controlled(creator: C, lost: Rc<std::cell::Cell<bool>>) -> Self {
        let anchor = File::open("/dev/null").unwrap();
        let thread_namespace = File::open("/dev/null").unwrap();
        let (socket, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
        Self {
            retained: ManuallyDrop::new(Retained {
                originals: Originals {
                    anchor,
                    thread_namespace,
                    creator_socket: socket.into(),
                    owner_thread: thread::current().id(),
                    verifier: Box::new(SyntheticVerifier(lost)),
                },
                creator,
            }),
            sealed: false,
            _same_thread: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn release_synthetic(self) {
        drop(ManuallyDrop::into_inner(self.retained));
    }
}

#[cfg(test)]
struct SyntheticVerifier(Rc<std::cell::Cell<bool>>);
#[cfg(test)]
impl OriginalVerifier for SyntheticVerifier {
    fn recheck(&mut self, originals: LaunchBorrow<'_>) -> Result<(), EffectError> {
        // Descriptor borrows are real; namespace/launch facts are NOT.
        let _ = (
            originals.anchor,
            originals.thread_namespace,
            originals.creator_socket,
        );
        if self.0.get() {
            Err(EffectError::UnavailableOrUncertain)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "launch_acquisition_tests.rs"]
mod tests;
