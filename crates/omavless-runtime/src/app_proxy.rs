// SPDX-License-Identifier: MIT

//! Effect-free S1 proxy-state transaction foundation. Not registered in IPC.
//!
//! The future fixed host adapters supply canonical snapshots of only their
//! allowlisted proxy fields. Snapshots may contain credentials and must remain
//! private. This module never reads or writes host settings, starts a core, or
//! establishes that a listener is owned/ready. Callers must serialize effects,
//! persist each prepared step before executing it, and revalidate native owner
//! identity and complete observations around each effect. A state comparison
//! is not an OS compare-and-swap and cannot prevent an external writer racing it.

use std::fmt;

const MAX_SNAPSHOT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Desktop,
    UserManager,
}

impl Surface {
    fn index(self) -> usize {
        match self {
            Self::Desktop => 0,
            Self::UserManager => 1,
        }
    }
}

/// Canonical adapter bytes, including field presence; never a shell command.
/// `None` and an empty present value remain distinct.
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot(Option<Vec<u8>>);

impl Snapshot {
    pub fn new(bytes: Option<Vec<u8>>) -> Result<Self, Error> {
        if bytes
            .as_ref()
            .is_some_and(|bytes| bytes.len() > MAX_SNAPSHOT_BYTES)
        {
            return Err(Error::InvalidSnapshot);
        }
        Ok(Self(bytes))
    }

    /// Private adapter data. Do not put this into argv, logs or diagnostics.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.0.as_deref()
    }
}

impl fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxySnapshot([private])")
    }
}

/// Identity supplied and revalidated by the canonical runtime, not by a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner {
    pub instance: [u8; 16],
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Applying,
    Active,
    Restoring,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidSnapshot,
    StaleOwner,
    ForeignChange,
    WrongPhase,
    PendingEffect,
    UnconfirmedEffect,
}

/// A single fixed-surface write with exact expected and replacement snapshots.
/// No arbitrary key, path, executable or command can be selected here.
#[derive(Debug, PartialEq, Eq)]
pub struct Effect {
    pub surface: Surface,
    pub expected: Snapshot,
    pub replacement: Snapshot,
}

/// Pure in-memory transaction; not a durable journal or a production lease.
/// Dropping it must never be interpreted as permission to reset host settings.
#[derive(Debug)]
pub struct ProxyLease {
    owner: Owner,
    original: [Snapshot; 2],
    target: [Snapshot; 2],
    expected: [Snapshot; 2],
    attempted: [bool; 2],
    pending: Option<Surface>,
    phase: Phase,
}

impl ProxyLease {
    pub fn prepare(owner: Owner, original: [Snapshot; 2], target: [Snapshot; 2]) -> Self {
        Self {
            owner,
            expected: original.clone(),
            original,
            target,
            attempted: [false; 2],
            pending: None,
            phase: Phase::Applying,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    fn check_owner(&self, owner: Owner) -> Result<(), Error> {
        if self.owner != owner {
            return Err(Error::StaleOwner);
        }
        Ok(())
    }

    /// Mark intent BEFORE the future host writes. Persist this updated state
    /// first. After a write, `confirm` must receive a fresh complete readback.
    /// `None` means the complete observed pair matches the desired pair.
    pub fn begin_next(
        &mut self,
        owner: Owner,
        observed: &[Snapshot; 2],
    ) -> Result<Option<Effect>, Error> {
        self.check_owner(owner)?;
        if self.pending.is_some() {
            return Err(Error::PendingEffect);
        }
        if self.phase != Phase::Applying && self.phase != Phase::Restoring {
            return Err(Error::WrongPhase);
        }
        if observed != &self.expected {
            return Err(Error::ForeignChange);
        }
        let destination = if self.phase == Phase::Applying {
            &self.target
        } else {
            &self.original
        };
        // Restore in reverse order, including a write whose outcome was lost.
        let surfaces = if self.phase == Phase::Applying {
            [Surface::Desktop, Surface::UserManager]
        } else {
            [Surface::UserManager, Surface::Desktop]
        };
        for surface in surfaces {
            let index = surface.index();
            if observed[index] != destination[index] {
                self.pending = Some(surface);
                if self.phase == Phase::Applying {
                    self.attempted[index] = true;
                }
                return Ok(Some(Effect {
                    surface,
                    expected: observed[index].clone(),
                    replacement: destination[index].clone(),
                }));
            }
        }
        self.phase = if self.phase == Phase::Applying {
            Phase::Active
        } else {
            Phase::Released
        };
        Ok(None)
    }

    pub fn confirm(&mut self, owner: Owner, observed: &[Snapshot; 2]) -> Result<(), Error> {
        self.check_owner(owner)?;
        let surface = self.pending.ok_or(Error::WrongPhase)?;
        let mut expected = self.expected.clone();
        expected[surface.index()] = if self.phase == Phase::Applying {
            self.target[surface.index()].clone()
        } else {
            self.original[surface.index()].clone()
        };
        if observed != &expected {
            return Err(Error::UnconfirmedEffect);
        }
        self.expected = expected;
        self.pending = None;
        Ok(())
    }

    /// Explicit disable or compensation after a failed/uncertain write.
    /// Any foreign value refuses the entire plan without overwriting it.
    /// A surface never attempted by this lease must still equal its original.
    /// The caller retains the lease on refusal for explicit recovery.
    pub fn begin_restore(&mut self, owner: Owner, observed: &[Snapshot; 2]) -> Result<(), Error> {
        self.check_owner(owner)?;
        if self.phase == Phase::Released {
            return Err(Error::WrongPhase);
        }
        for (index, value) in observed.iter().enumerate() {
            if value != &self.original[index]
                && (!self.attempted[index] || value != &self.target[index])
            {
                return Err(Error::ForeignChange);
            }
        }
        self.expected = observed.clone();
        self.pending = None;
        self.phase = Phase::Restoring;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(s: &str) -> Snapshot {
        Snapshot::new(Some(s.as_bytes().to_vec())).unwrap()
    }

    fn owner() -> Owner {
        Owner {
            instance: [1; 16],
            generation: 7,
        }
    }

    fn pair() -> ([Snapshot; 2], [Snapshot; 2]) {
        (
            [Snapshot::new(None).unwrap(), value("")],
            [value("desktop-loopback"), value("manager-loopback")],
        )
    }

    fn apply(lease: &mut ProxyLease, current: &mut [Snapshot; 2]) {
        while let Some(effect) = lease.begin_next(owner(), current).unwrap() {
            current[effect.surface.index()] = effect.replacement;
            lease.confirm(owner(), current).unwrap();
        }
    }

    #[test]
    fn exact_round_trip_distinguishes_absence_and_empty() {
        let (original, target) = pair();
        let mut current = original.clone();
        let mut lease = ProxyLease::prepare(owner(), original.clone(), target.clone());
        apply(&mut lease, &mut current);
        assert_eq!(lease.phase(), Phase::Active);
        assert_eq!(current, target);
        lease.begin_restore(owner(), &current).unwrap();
        let step = lease.begin_next(owner(), &current).unwrap().unwrap();
        assert_eq!(step.surface, Surface::UserManager);
        current[1] = step.replacement;
        lease.confirm(owner(), &current).unwrap();
        apply(&mut lease, &mut current);
        assert_eq!(current, original);
        assert_eq!(lease.phase(), Phase::Released);
    }

    #[test]
    fn uncertain_write_can_restore_both_applied_and_unapplied_outcomes() {
        for did_write in [false, true] {
            let (original, target) = pair();
            let mut current = original.clone();
            let mut lease = ProxyLease::prepare(owner(), original.clone(), target);
            let step = lease.begin_next(owner(), &current).unwrap().unwrap();
            if did_write {
                current[0] = step.replacement;
            }
            assert_eq!(
                lease.begin_next(owner(), &current),
                Err(Error::PendingEffect)
            );
            lease.begin_restore(owner(), &current).unwrap();
            apply(&mut lease, &mut current);
            assert_eq!(current, original);
        }
    }

    #[test]
    fn partial_enable_failure_restores_first_surface_only() {
        let (original, target) = pair();
        let mut current = original.clone();
        let mut lease = ProxyLease::prepare(owner(), original.clone(), target);
        let step = lease.begin_next(owner(), &current).unwrap().unwrap();
        current[0] = step.replacement;
        lease.confirm(owner(), &current).unwrap();
        lease.begin_next(owner(), &current).unwrap().unwrap();
        lease.begin_restore(owner(), &current).unwrap();
        assert_eq!(
            lease
                .begin_next(owner(), &current)
                .unwrap()
                .unwrap()
                .surface,
            Surface::Desktop
        );
        current[0] = original[0].clone();
        lease.confirm(owner(), &current).unwrap();
        assert!(lease.begin_next(owner(), &current).unwrap().is_none());
        assert_eq!(current, original);
    }

    #[test]
    fn foreign_changes_and_unattempted_target_are_not_owned() {
        let (original, target) = pair();
        let mut lease = ProxyLease::prepare(owner(), original.clone(), target.clone());
        let mut current = original.clone();
        current[1] = target[1].clone();
        assert_eq!(
            lease.begin_restore(owner(), &current),
            Err(Error::ForeignChange)
        );
        apply(&mut lease, &mut original.clone());
        let mut current = target;
        current[0] = value("changed-elsewhere");
        assert_eq!(
            lease.begin_restore(owner(), &current),
            Err(Error::ForeignChange)
        );
        assert_eq!(lease.phase(), Phase::Active);
    }

    #[test]
    fn complete_readback_and_owner_identity_required() {
        let (original, target) = pair();
        let mut lease = ProxyLease::prepare(owner(), original.clone(), target.clone());
        let mut stale = owner();
        stale.generation += 1;
        assert_eq!(lease.begin_next(stale, &original), Err(Error::StaleOwner));
        lease.begin_next(owner(), &original).unwrap();
        assert_eq!(
            lease.confirm(owner(), &original),
            Err(Error::UnconfirmedEffect)
        );
        // Both target values appearing does not confirm a one-surface write.
        assert_eq!(
            lease.confirm(owner(), &target),
            Err(Error::UnconfirmedEffect)
        );
        assert_eq!(
            lease.begin_restore(stale, &original),
            Err(Error::StaleOwner)
        );
        stale = owner();
        stale.instance[0] = 2;
        assert_eq!(lease.confirm(stale, &target), Err(Error::StaleOwner));
    }

    #[test]
    fn original_already_matches_target_needs_no_write_or_reset() {
        let (_, target) = pair();
        let mut lease = ProxyLease::prepare(owner(), target.clone(), target.clone());
        assert!(lease.begin_next(owner(), &target).unwrap().is_none());
        lease.begin_restore(owner(), &target).unwrap();
        assert!(lease.begin_next(owner(), &target).unwrap().is_none());
    }

    #[test]
    fn restoration_failure_retains_original_for_retry() {
        let (original, target) = pair();
        let mut current = original.clone();
        let mut lease = ProxyLease::prepare(owner(), original.clone(), target);
        apply(&mut lease, &mut current);
        lease.begin_restore(owner(), &current).unwrap();
        lease.begin_next(owner(), &current).unwrap();
        assert_eq!(
            lease.confirm(owner(), &current),
            Err(Error::UnconfirmedEffect)
        );
        lease.begin_restore(owner(), &current).unwrap();
        apply(&mut lease, &mut current);
        assert_eq!(current, original);
    }

    #[test]
    fn private_snapshot_debug_and_bounds() {
        assert!(Snapshot::new(Some(vec![0; MAX_SNAPSHOT_BYTES])).is_ok());
        assert_eq!(
            Snapshot::new(Some(vec![0; MAX_SNAPSHOT_BYTES + 1])),
            Err(Error::InvalidSnapshot)
        );
        let secret = value("synthetic-private-value");
        assert!(!format!("{secret:?}").contains("synthetic-private-value"));
        let lease = ProxyLease::prepare(
            owner(),
            [secret.clone(), secret.clone()],
            [secret.clone(), secret],
        );
        assert!(!format!("{lease:?}").contains("synthetic-private-value"));
    }
}
