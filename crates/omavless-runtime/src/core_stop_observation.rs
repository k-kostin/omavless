// SPDX-License-Identifier: MIT
//! Scoped test-only evidence. No host queries, logging or production interface.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    ChildObservation,
    ChildIdentity,
    ProcessGroup,
    TermSignal,
    InventoryRoot,
    InventoryEntry,
    InventoryType,
    InventoryPid,
    InventoryOpenStat,
    InventoryReadStat,
    InventoryStatSize,
    InventoryParseStat,
    InventoryIdentity,
    InventoryEntryBound,
    InventoryMemberBound,
    Reap,
    KillSignal,
    Deadline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Failure {
    pub(crate) stage: Stage,
    pub(crate) errno: Option<i32>,
    pub(crate) injected: bool,
}

#[derive(Default)]
struct State {
    first: Option<Failure>,
    reject_inventory: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

// A scope cannot migrate to a different thread. Nested scopes restore the
// previous recorder; tests without a scope collect nothing.
pub(crate) struct Scope {
    previous: Option<State>,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Scope {
    pub(crate) fn enter() -> Self {
        Self {
            previous: STATE.with(|state| state.replace(Some(State::default()))),
            _thread_bound: PhantomData,
        }
    }

    pub(crate) fn failure(&self) -> Option<Failure> {
        STATE.with(|state| state.borrow().as_ref().and_then(|s| s.first))
    }

    pub(crate) fn reject_inventory(&self, reject: bool) {
        STATE.with(|state| {
            if let Some(state) = state.borrow_mut().as_mut() {
                state.reject_inventory = reject;
            }
        });
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        STATE.with(|state| state.replace(self.previous.take()));
    }
}

pub(crate) fn record(stage: Stage, errno: Option<i32>) {
    record_inner(stage, errno, false);
}

fn record_inner(stage: Stage, errno: Option<i32>, injected: bool) {
    STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut()
            && state.first.is_none()
        {
            state.first = Some(Failure {
                stage,
                errno,
                injected,
            });
        }
    });
}

pub(crate) fn reject_inventory() -> bool {
    STATE.with(|state| state.borrow().as_ref().is_some_and(|s| s.reject_inventory))
}

pub(crate) fn inventory_refusal() -> bool {
    if !reject_inventory() {
        return false;
    }
    record_inner(
        Stage::InventoryRoot,
        Some(nix::errno::Errno::EACCES as i32),
        true,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_failure_is_scoped_and_cannot_be_overwritten_by_later_cleanup() {
        record(Stage::Deadline, None);
        let outer = Scope::enter();
        assert_eq!(outer.failure(), None);
        record(Stage::TermSignal, Some(1));
        let first = outer.failure();
        record(Stage::Deadline, None);
        assert_eq!(outer.failure(), first);
        {
            let inner = Scope::enter();
            assert_eq!(inner.failure(), None);
            record(Stage::Reap, Some(10));
            assert_eq!(inner.failure().unwrap().stage, Stage::Reap);
        }
        assert_eq!(outer.failure(), first);
    }

    #[test]
    fn scopes_do_not_cross_threads_or_leak_faults() {
        let scope = Scope::enter();
        scope.reject_inventory(true);
        assert!(reject_inventory());
        assert!(!std::thread::spawn(reject_inventory).join().unwrap());
        drop(scope);
        assert!(!reject_inventory());
    }
}
