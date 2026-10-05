// SPDX-License-Identifier: MIT
//! Lower I/O custody for the opt-in actor transaction successor.
//!
//! The complete ledger is reserved before acquisition. A reported return is
//! inserted before post-call checks; any Result failure latches this original
//! ledger and retains its prefix. No retry, FD export, eviction or error cleanup.
//! Fatal loss and unreported constructor/close internals remain unavailable.

use super::Unavailable;
use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use std::fs::File;
use std::path::Path;

const PERSISTENT: usize = 28;
const SCRATCH: usize = 8;
const IO_SLOTS: usize = PERSISTENT + SCRATCH;
const MANAGER_ORIGINALS: usize = 17;
const BASE_FDS: usize = 4; // actor stdio and its original channel, not supervisor
const NOFILE: usize = 64;

// Names are internal operation roles, never an input-selected descriptor index.
// This is a conservative prospective whole-path envelope; unused roles do not
// prove acquisition or a canonical manager origin. Scratch is not recycled on
// failure and remains charged even if a later check has no need for the handle.
#[derive(Clone, Copy)]
#[repr(usize)]
pub(super) enum Slot {
    Root,
    Run,
    Epoch,
    Transaction,
    Config,
    State,
    Lock,
    Owner,
    Desired,
    Login,
    OldStore,
    OldTemplate,
    StageDirectory,
    StageOldStore,
    StageOldTemplate,
    StageNewStore,
    StageNewTemplate,
    StageReady,
    Intent,
    Terminal,
    ReplacementStore,
    ReplacementTemplate,
    ManagerRoot,
    ManagerUsr,
    ManagerLib,
    ManagerImage,
    ManagerChannel,
    ManagerPidfd,
    Scratch0,
    Scratch1,
    Scratch2,
    Scratch3,
    Scratch4,
    Scratch5,
    Scratch6,
    Scratch7,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Live,
    Revoked,
    Finished,
}

struct Ledger<T> {
    // Array, not a growable vector: no continuation allocation on positive open.
    slots: [Option<T>; IO_SLOTS],
    state: State,
}

impl<T> Ledger<T> {
    fn new(manager_originals: usize) -> Result<Self, Unavailable> {
        // Three-observation capacity (51 manager originals) cannot be silently
        // combined with this transaction plan. Admission requires ONE actual
        // original capture, with no reacquisition at each transaction gate.
        if manager_originals != MANAGER_ORIGINALS
            || manager_originals
                .checked_add(BASE_FDS + IO_SLOTS)
                .is_none_or(|n| n > NOFILE)
        {
            return Err(Unavailable);
        }
        Ok(Self {
            slots: std::array::from_fn(|_| None),
            state: State::Live,
        })
    }

    fn attempted<R>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<R, Unavailable>,
    ) -> Result<R, Unavailable> {
        if self.state != State::Live {
            return Err(Unavailable);
        }
        let result = operation(self);
        if result.is_err() {
            self.state = State::Revoked;
        }
        result
    }

    fn acquire(
        &mut self,
        slot: Slot,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        acquire: impl FnOnce() -> Result<T, Unavailable>,
        check: impl FnOnce(&T) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.attempted(|owner| {
            let index = slot as usize;
            if owner.slots[index].is_some() {
                return Err(Unavailable);
            }
            gate()?;
            let reported = acquire()?;
            owner.slots[index] = Some(reported); // BEFORE post-tick/shape/fs checks
            gate()?;
            check(owner.slots[index].as_ref().ok_or(Unavailable)?)?;
            gate()
        })
    }

    fn acquire_from(
        &mut self,
        parent: Slot,
        slot: Slot,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        acquire: impl FnOnce(&T) -> Result<T, Unavailable>,
        check: impl FnOnce(&T) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.attempted(|owner| {
            if owner.slots[slot as usize].is_some() {
                return Err(Unavailable);
            }
            gate()?;
            let reported = acquire(owner.slots[parent as usize].as_ref().ok_or(Unavailable)?)?;
            owner.slots[slot as usize] = Some(reported);
            gate()?;
            check(owner.slots[slot as usize].as_ref().ok_or(Unavailable)?)?;
            gate()
        })
    }

    fn perform(
        &mut self,
        slot: Slot,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        operation: impl FnOnce(&mut T) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.attempted(|owner| {
            gate()?;
            operation(owner.slots[slot as usize].as_mut().ok_or(Unavailable)?)?;
            gate()
        })
    }

    fn finish(&mut self) -> Result<(), Unavailable> {
        if self.state != State::Live {
            return Err(Unavailable);
        }
        self.state = State::Finished;
        // Valid normal Halt only. Ordinary File Drop close semantics, not a
        // per-FD absence proof; a revoked owner cannot enter this branch.
        for slot in &mut self.slots {
            *slot = None;
        }
        Ok(())
    }
}

// Private File adapter. Callers must use these retained originals rather than
// invoke old helpers whose local Files are dropped on Err. This adapter alone
// is NOT a path-wide Restore/journal/canonical-manager integration claim.
pub(super) struct FileIo {
    ledger: Ledger<File>,
}

pub(super) struct ChildPlan {
    pub parent: Slot,
    pub slot: Slot,
    pub name: &'static str,
    pub flags: OFlag,
    pub mode: Mode,
}

impl FileIo {
    pub fn new(manager_originals: usize) -> Result<Self, Unavailable> {
        Ok(Self {
            ledger: Ledger::new(manager_originals)?,
        })
    }

    pub fn root(
        &mut self,
        gate: impl FnMut() -> Result<(), Unavailable>,
        check: impl FnOnce(&File) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.ledger.acquire(
            Slot::Root,
            gate,
            || {
                open(
                    "/",
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map(File::from)
                .map_err(|_| Unavailable)
            },
            check,
        )
    }

    pub fn child(
        &mut self,
        plan: ChildPlan,
        gate: impl FnMut() -> Result<(), Unavailable>,
        check: impl FnOnce(&File) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        // Only internal fixed single components. There is no caller path or
        // generic privileged IPC surface and no FD returned from this adapter.
        if plan.name.is_empty()
            || plan.name == "."
            || plan.name == ".."
            || plan.name.contains('/')
            || plan.name.as_bytes().contains(&0)
        {
            self.ledger.state = State::Revoked;
            return Err(Unavailable);
        }
        self.ledger.acquire_from(
            plan.parent,
            plan.slot,
            gate,
            |directory| {
                openat(
                    directory,
                    Path::new(plan.name),
                    plan.flags | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    plan.mode,
                )
                .map(File::from)
                .map_err(|_| Unavailable)
            },
            check,
        )
    }

    pub fn perform(
        &mut self,
        slot: Slot,
        gate: impl FnMut() -> Result<(), Unavailable>,
        operation: impl FnOnce(&mut File) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.ledger.perform(slot, gate, operation)
    }

    pub fn finish(&mut self) -> Result<(), Unavailable> {
        self.ledger.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Handle(Rc<Cell<usize>>);
    impl Drop for Handle {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn complete_slot_envelope_does_not_borrow_three_capture_capacity() {
        assert_eq!(Slot::Scratch0 as usize, PERSISTENT);
        assert_eq!(Slot::Scratch7 as usize + 1, IO_SLOTS);
        assert_eq!(BASE_FDS + MANAGER_ORIGINALS + IO_SLOTS, 57);
        for originals in [0, 16, 18, 34, 51, usize::MAX] {
            assert!(Ledger::<Handle>::new(originals).is_err());
        }
    }

    #[test]
    fn reported_original_is_retained_before_each_fallible_post_check() {
        for cut in 0..4 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Ledger::new(17).unwrap();
            let step = Cell::new(0);
            let gate = || {
                let now = step.get();
                step.set(now + 1);
                if now == cut { Err(Unavailable) } else { Ok(()) }
            };
            let result = owner.acquire(
                Slot::Root,
                gate,
                || Ok(Handle(drops.clone())),
                |_| {
                    if cut == 3 { Err(Unavailable) } else { Ok(()) }
                },
            );
            assert!(result.is_err());
            assert_eq!(drops.get(), 0);
            assert_eq!(owner.slots[0].is_some(), cut != 0);
            assert!(
                owner
                    .acquire(
                        Slot::Run,
                        || panic!("reentry tick"),
                        || panic!("reentry acquisition"),
                        |_| panic!("reentry validation")
                    )
                    .is_err()
            );
            assert!(owner.finish().is_err());
            assert_eq!(drops.get(), 0); // owner is alive; no fatal/Drop survival claim
        }
    }

    #[test]
    fn acquisition_error_duplicate_role_and_missing_parent_latch_no_later_effect() {
        for cut in 0..3 {
            let mut owner = Ledger::<Handle>::new(17).unwrap();
            let calls = Cell::new(0);
            if cut == 1 {
                owner
                    .acquire(
                        Slot::Root,
                        || Ok(()),
                        || Ok(Handle(Rc::new(Cell::new(0)))),
                        |_| Ok(()),
                    )
                    .unwrap();
            }
            let result = if cut == 2 {
                owner.acquire_from(
                    Slot::Epoch,
                    Slot::Run,
                    || Ok(()),
                    |_| panic!("missing parent must not open"),
                    |_| Ok(()),
                )
            } else {
                owner.acquire(
                    Slot::Root,
                    || Ok(()),
                    || {
                        calls.set(calls.get() + 1);
                        Err(Unavailable)
                    },
                    |_| Ok(()),
                )
            };
            assert!(result.is_err());
            assert_eq!(calls.get(), usize::from(cut == 0));
            assert!(
                owner
                    .perform(Slot::Root, || panic!("reentry"), |_| panic!("later effect"))
                    .is_err()
            );
        }
    }

    #[test]
    fn every_operation_failure_retains_handles_and_blocks_downstream_and_halt() {
        for cut in 0..3 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Ledger::new(17).unwrap();
            owner
                .acquire(
                    Slot::Root,
                    || Ok(()),
                    || Ok(Handle(drops.clone())),
                    |_| Ok(()),
                )
                .unwrap();
            let gate_calls = Cell::new(0);
            let effects = Cell::new(0);
            assert!(
                owner
                    .perform(
                        Slot::Root,
                        || {
                            let now = gate_calls.get();
                            gate_calls.set(now + 1);
                            if (cut == 0 && now == 0) || (cut == 2 && now == 1) {
                                Err(Unavailable)
                            } else {
                                Ok(())
                            }
                        },
                        |_| {
                            effects.set(effects.get() + 1);
                            if cut == 1 { Err(Unavailable) } else { Ok(()) }
                        }
                    )
                    .is_err()
            );
            assert_eq!(effects.get(), usize::from(cut != 0));
            assert_eq!(drops.get(), 0);
            assert!(owner.finish().is_err());
            assert!(
                owner
                    .perform(
                        Slot::Root,
                        || panic!("tick after revoke"),
                        |_| panic!("effect after revoke")
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn child_report_is_retained_and_only_live_normal_finish_releases() {
        let drops = Rc::new(Cell::new(0));
        let mut owner = Ledger::new(17).unwrap();
        owner
            .acquire(
                Slot::Root,
                || Ok(()),
                || Ok(Handle(drops.clone())),
                |_| Ok(()),
            )
            .unwrap();
        owner
            .acquire_from(
                Slot::Root,
                Slot::Run,
                || Ok(()),
                |_| Ok(Handle(drops.clone())),
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(drops.get(), 0);
        owner.finish().unwrap();
        assert_eq!(drops.get(), 2);
        assert!(owner.finish().is_err());
        assert!(
            owner
                .acquire(
                    Slot::Root,
                    || panic!("finished reentry"),
                    || panic!("finished open"),
                    |_| Ok(())
                )
                .is_err()
        );
    }
}
