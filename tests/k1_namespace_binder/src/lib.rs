//! External, review-only real descriptor binding. Never canonical authority.
#![forbid(unsafe_code)]

use nix::sys::nsfs::{namespace_id, namespace_type, NamespaceType};
use nix::sys::socket::{
    bind, getsockopt, socket, sockopt::NetnsCookie, AddressFamily, NetlinkAddr, SockFlag,
    SockProtocol, SockType,
};
use std::fs::File;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::rc::Rc;
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq)]
pub enum Refused {
    Unavailable,
    Mismatch,
    Expired,
    Sealed,
}

// Private and never serialized. Equality is only local kernel consistency.
#[derive(Clone, Copy)]
struct Sample {
    kind: NamespaceType,
    id: u64,
}

fn agree(
    anchor: Sample,
    retained: Sample,
    current: Sample,
    cookie: u64,
    initial: Option<u64>,
) -> Result<u64, Refused> {
    let id = same_namespace(anchor, retained, current, initial)?;
    if cookie == 0 || cookie != id {
        return Err(Refused::Mismatch);
    }
    Ok(id)
}

fn same_namespace(
    anchor: Sample,
    retained: Sample,
    current: Sample,
    initial: Option<u64>,
) -> Result<u64, Refused> {
    if anchor.kind != NamespaceType::Network
        || retained.kind != NamespaceType::Network
        || current.kind != NamespaceType::Network
        || anchor.id == 0
        || anchor.id != retained.id
        || anchor.id != current.id
        || initial.map_or(false, |id| id != anchor.id)
    {
        return Err(Refused::Mismatch);
    }
    Ok(anchor.id)
}

struct Originals {
    anchor: File,
    thread_namespace: Option<File>,
    creator: Option<OwnedFd>,
    owner_thread: ThreadId,
    initial: Option<u64>,
}

// Fixed leaf calls only. The normal API cannot accept or replace this backend.
trait Queries {
    fn now(&mut self) -> Instant;
    fn open_current(&mut self) -> Result<File, Refused>;
    fn kind(&mut self, file: &File) -> Result<NamespaceType, Refused>;
    fn id(&mut self, file: &File) -> Result<u64, Refused>;
    fn create(&mut self) -> Result<OwnedFd, Refused>;
    fn bind(&mut self, creator: &OwnedFd) -> Result<(), Refused>;
    fn cookie(&mut self, creator: &OwnedFd) -> Result<u64, Refused>;
}

struct Real;
impl Queries for Real {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn open_current(&mut self) -> Result<File, Refused> {
        File::open("/proc/thread-self/ns/net").map_err(|_| Refused::Unavailable)
    }
    fn kind(&mut self, file: &File) -> Result<NamespaceType, Refused> {
        namespace_type(file.as_fd()).map_err(|_| Refused::Unavailable)
    }
    fn id(&mut self, file: &File) -> Result<u64, Refused> {
        namespace_id(file.as_fd()).map_err(|_| Refused::Unavailable)
    }
    fn create(&mut self) -> Result<OwnedFd, Refused> {
        socket(
            AddressFamily::Netlink,
            SockType::Raw,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            SockProtocol::NetlinkNetFilter,
        )
        .map_err(|_| Refused::Unavailable)
    }
    fn bind(&mut self, creator: &OwnedFd) -> Result<(), Refused> {
        bind(creator.as_raw_fd(), &NetlinkAddr::new(0, 0)).map_err(|_| Refused::Unavailable)
    }
    fn cookie(&mut self, creator: &OwnedFd) -> Result<u64, Refused> {
        getsockopt(creator, NetnsCookie).map_err(|_| Refused::Unavailable)
    }
}

/// Retains the exact supplied namespace object and the socket this binder creates.
/// No descriptor, mutable creator, cookie, epoch or authority token can be extracted.
/// The supplied anchor is UNTRUSTED: successful binding is not trusted launch.
/// ```compile_fail,E0277
/// use k1_real_namespace_binder_review::LocalBinding;
/// fn send<T: Send>() {}
/// send::<LocalBinding>();
/// ```
/// ```compile_fail,E0277
/// use k1_real_namespace_binder_review::LocalBinding;
/// fn sync<T: Sync>() {}
/// sync::<LocalBinding>();
/// ```
/// ```compile_fail,E0616
/// use k1_real_namespace_binder_review::LocalBinding;
/// fn replace(a: &mut LocalBinding, b: &mut LocalBinding) {
///     std::mem::swap(&mut a.originals.creator, &mut b.originals.creator);
/// }
/// ```
pub struct LocalBinding {
    originals: ManuallyDrop<Originals>,
    deadline: Instant,
    sealed: bool,
    _same_thread: PhantomData<Rc<()>>,
}

impl LocalBinding {
    /// Create one fixed nonblocking NETLINK_NETFILTER socket on this thread.
    /// Calls no setns, sends no datagram and changes no firewall state.
    /// All acquired originals survive refusal/unwind/drop until process exit.
    pub fn bind_untrusted_anchor(anchor: File) -> Result<Self, Refused> {
        Self::bind_with(anchor, &mut Real)
    }

    fn bind_with(anchor: File, queries: &mut impl Queries) -> Result<Self, Refused> {
        let originals = ManuallyDrop::new(Originals {
            anchor,
            thread_namespace: None,
            creator: None,
            owner_thread: thread::current().id(),
            initial: None,
        });
        let deadline = queries
            .now()
            .checked_add(Duration::from_secs(2))
            .ok_or(Refused::Expired)?;
        let mut owner = Self {
            originals,
            deadline,
            sealed: true,
            _same_thread: PhantomData,
        };
        owner.budget(queries)?;
        let opened = queries.open_current();
        // Retain a successful opener BEFORE any late/error classification.
        if let Ok(file) = opened {
            owner.originals.thread_namespace = Some(file);
        }
        owner.budget(queries)?;
        // An unavailable opener is terminal before any subsequent query.
        let retained_file = owner
            .originals
            .thread_namespace
            .as_ref()
            .ok_or(Refused::Unavailable)?;
        let anchor = owner.sample(&owner.originals.anchor, queries)?;
        let retained = owner.sample(retained_file, queries)?;
        same_namespace(anchor, retained, retained, None)?;
        owner.budget(queries)?;
        let created = queries.create();
        if let Ok(fd) = created {
            owner.originals.creator = Some(fd);
        }
        owner.budget(queries)?;
        let creator = owner
            .originals
            .creator
            .as_ref()
            .ok_or(Refused::Unavailable)?;
        let result = queries.bind(creator);
        owner.budget(queries)?;
        result?;
        owner.check(queries)?;
        owner.sealed = false;
        Ok(owner)
    }

    fn budget(&self, queries: &mut impl Queries) -> Result<(), Refused> {
        if thread::current().id() != self.originals.owner_thread {
            return Err(Refused::Mismatch);
        }
        if queries.now() >= self.deadline {
            return Err(Refused::Expired);
        }
        Ok(())
    }

    fn sample(&self, file: &File, queries: &mut impl Queries) -> Result<Sample, Refused> {
        self.budget(queries)?;
        let kind = queries.kind(file);
        self.budget(queries)?;
        let kind = kind?;
        if kind != NamespaceType::Network {
            return Err(Refused::Mismatch);
        }
        let id = queries.id(file);
        self.budget(queries)?;
        Ok(Sample { kind, id: id? })
    }

    fn check(&mut self, queries: &mut impl Queries) -> Result<(), Refused> {
        self.budget(queries)?;
        let anchor = self.sample(&self.originals.anchor, queries)?;
        let retained = self.sample(
            self.originals
                .thread_namespace
                .as_ref()
                .ok_or(Refused::Unavailable)?,
            queries,
        )?;
        self.budget(queries)?;
        // A retained old thread namespace alone cannot detect a later transition.
        let fresh = queries.open_current();
        let fresh = fresh.map(ManuallyDrop::new);
        self.budget(queries)?;
        let fresh = fresh?;
        let current = self.sample(&fresh, queries)?;
        self.budget(queries)?;
        let cookie = queries.cookie(
            self.originals
                .creator
                .as_ref()
                .ok_or(Refused::Unavailable)?,
        );
        self.budget(queries)?;
        let cookie = cookie?;
        let id = agree(anchor, retained, current, cookie, self.originals.initial)?;
        self.budget(queries)?;
        self.originals.initial = Some(id);
        // Only this temporary non-creator read handle is released on proven success.
        drop(ManuallyDrop::into_inner(fresh));
        self.budget(queries)?;
        Ok(())
    }

    /// Recheck the held originals and current thread against the SAME creator.
    /// One original two-second lifetime budget; no reset, repair or retry after error.
    pub fn verify_local(&mut self) -> Result<(), Refused> {
        self.verify_queries(&mut Real)
    }

    fn verify_queries(&mut self, queries: &mut impl Queries) -> Result<(), Refused> {
        if self.sealed {
            return Err(Refused::Sealed);
        }
        self.sealed = true;
        self.check(queries)?;
        self.sealed = false;
        Ok(())
    }

    // Private seam for error/unwind controls, never a caller-selected operation.
    #[cfg(test)]
    fn verify_with(
        &mut self,
        check: impl FnOnce(&mut Self) -> Result<(), Refused>,
    ) -> Result<(), Refused> {
        if self.sealed {
            return Err(Refused::Sealed);
        }
        self.sealed = true;
        self.budget(&mut Real)?;
        check(self)?;
        self.budget(&mut Real)?;
        self.sealed = false;
        Ok(())
    }
}

#[cfg(test)]
mod fault_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn net(id: u64) -> Sample {
        Sample {
            kind: NamespaceType::Network,
            id,
        }
    }

    #[test]
    fn local_agreement_is_not_authority_and_preserves_full_u64() {
        for id in [1, u64::MAX] {
            assert_eq!(agree(net(id), net(id), net(id), id, None), Ok(id));
            assert_eq!(agree(net(id), net(id), net(id), id, Some(id)), Ok(id));
        }
    }
    #[test]
    fn all_mismatches_zero_and_replaced_initial_refuse() {
        for (a, r, c, cookie, initial) in [
            (0, 0, 0, 0, None),
            (1, 1, 1, 0, None),
            (1, 2, 1, 1, None),
            (1, 1, 2, 1, None),
            (1, 1, 1, 2, None),
            (2, 2, 2, 2, Some(1)),
        ] {
            assert_eq!(
                agree(net(a), net(r), net(c), cookie, initial),
                Err(Refused::Mismatch)
            );
        }
    }
    #[test]
    fn every_wrong_kind_refuses_at_every_position() {
        for kind in [
            NamespaceType::Pid,
            NamespaceType::User,
            NamespaceType::Mount,
            NamespaceType::Time,
            NamespaceType::Cgroup,
            NamespaceType::Ipc,
            NamespaceType::Uts,
        ] {
            let bad = Sample { kind, id: 1 };
            assert_eq!(agree(bad, net(1), net(1), 1, None), Err(Refused::Mismatch));
            assert_eq!(agree(net(1), bad, net(1), 1, None), Err(Refused::Mismatch));
            assert_eq!(agree(net(1), net(1), bad, 1, None), Err(Refused::Mismatch));
        }
    }

    // Ordinary local file only; no namespace open/ioctl/netlink creation or send.
    fn inert_owner() -> LocalBinding {
        LocalBinding {
            originals: ManuallyDrop::new(Originals {
                anchor: File::open("/dev/null").unwrap(),
                thread_namespace: None,
                creator: None,
                owner_thread: thread::current().id(),
                initial: None,
            }),
            deadline: Instant::now().checked_add(Duration::from_secs(30)).unwrap(),
            sealed: false,
            _same_thread: PhantomData,
        }
    }
    #[test]
    fn error_and_unwind_permanently_seal_before_second_callback() {
        for panic in [false, true] {
            let mut owner = inert_owner();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.verify_with(|_| {
                    assert!(!panic, "inert refusal");
                    Err(Refused::Unavailable)
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(Refused::Unavailable));
            }
            assert_eq!(
                owner.verify_with(|_| panic!("sealed callback")),
                Err(Refused::Sealed)
            );
            // Explicit test-only release of an ordinary file, not a creator.
            drop(ManuallyDrop::into_inner(owner.originals));
        }
    }
    #[test]
    fn expired_before_or_after_callback_never_resets_original_budget() {
        for before in [false, true] {
            let mut owner = inert_owner();
            if before {
                owner.deadline = Instant::now();
            }
            let result = owner.verify_with(|owner| {
                assert!(!before, "expired callback");
                owner.deadline = Instant::now();
                Ok(())
            });
            assert_eq!(result, Err(Refused::Expired));
            assert_eq!(
                owner.verify_with(|_| panic!("expired reuse")),
                Err(Refused::Sealed)
            );
            drop(ManuallyDrop::into_inner(owner.originals));
        }
    }
}
