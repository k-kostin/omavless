// SPDX-License-Identifier: MIT
//! Original validation child and inputs survive every uncertain cut together.
use std::process::Child;
use std::time::Instant;

pub(super) trait ChildState {
    /// Some is the original child's observed AND reaped exit, not a PID query.
    fn reaped(&mut self) -> Result<Option<bool>, ()>;
}
impl ChildState for Child {
    fn reaped(&mut self) -> Result<Option<bool>, ()> {
        self.try_wait()
            .map(|status| status.map(|s| s.success()))
            .map_err(|_| ())
    }
}

pub(super) struct Validation<C, B> {
    original: Option<(C, B)>,
}
impl<C, B> Drop for Validation<C, B> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            std::mem::forget(original);
        }
    }
}
impl<C: ChildState, B> Validation<C, B> {
    pub(super) fn new(child: C, bound: B) -> Self {
        Self {
            original: Some((child, bound)),
        }
    }
    pub(super) fn complete(
        mut self,
        deadline: Instant,
        mut now: impl FnMut() -> Instant,
        mut pause: impl FnMut(),
        mut recheck: impl FnMut(&B) -> Result<(), ()>,
    ) -> Result<B, ()> {
        loop {
            if now() >= deadline {
                return Err(());
            }
            let (child, bound) = self.original.as_mut().ok_or(())?;
            recheck(bound)?;
            let exit = child.reaped()?;
            if now() >= deadline {
                return Err(());
            }
            match exit {
                Some(true) => {
                    recheck(bound)?;
                    if now() >= deadline {
                        return Err(());
                    }
                    let (_, bound) = self.original.take().ok_or(())?;
                    return Ok(bound);
                }
                Some(false) => {
                    // A known reaped rejection can release inputs, but supplies
                    // no capability and does not authorize another attempt.
                    drop(self.original.take());
                    return Err(());
                }
                None => pause(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc, time::Duration};
    struct Owned(Rc<Cell<u8>>);
    impl Drop for Owned {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    struct Mock {
        held: Owned,
        outcome: Result<Option<bool>, ()>,
    }
    impl ChildState for Mock {
        fn reaped(&mut self) -> Result<Option<bool>, ()> {
            let _ = &self.held;
            self.outcome
        }
    }
    fn fixture(outcome: Result<Option<bool>, ()>) -> (Validation<Mock, Owned>, Rc<Cell<u8>>) {
        let drops = Rc::new(Cell::new(0));
        (
            Validation::new(
                Mock {
                    held: Owned(drops.clone()),
                    outcome,
                },
                Owned(drops.clone()),
            ),
            drops,
        )
    }
    #[test]
    fn only_original_success_releases_bound() {
        let (owner, drops) = fixture(Ok(Some(true)));
        let now = Instant::now();
        let bound = owner
            .complete(
                now + Duration::from_secs(1),
                || now,
                || panic!(),
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(drops.get(), 1);
        drop(bound);
        assert_eq!(drops.get(), 2);
    }
    #[test]
    fn rejected_reaped_child_is_not_admission() {
        let (owner, drops) = fixture(Ok(Some(false)));
        let now = Instant::now();
        assert!(
            owner
                .complete(
                    now + Duration::from_secs(1),
                    || now,
                    || panic!(),
                    |_| Ok(())
                )
                .is_err()
        );
        assert_eq!(drops.get(), 2);
    }
    #[test]
    fn poll_timeout_recheck_and_unwind_retain_whole_original() {
        for cut in 0..5 {
            let (owner, drops) = fixture(if cut == 0 { Err(()) } else { Ok(None) });
            let now = Instant::now();
            let calls = Cell::new(0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.complete(
                    now + Duration::from_secs(1),
                    || {
                        calls.set(calls.get() + 1);
                        if cut == 1 || (cut == 2 && calls.get() > 1) {
                            now + Duration::from_secs(1)
                        } else {
                            now
                        }
                    },
                    || panic!("synthetic pause cut"),
                    |_| if cut == 3 { Err(()) } else { Ok(()) },
                )
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert_eq!(drops.get(), 0);
        }
    }
    #[test]
    fn positive_late_reap_and_postcheck_are_not_admission() {
        for late in [false, true] {
            let (owner, drops) = fixture(Ok(Some(true)));
            let now = Instant::now();
            let clocks = Cell::new(0);
            let checks = Cell::new(0);
            assert!(
                owner
                    .complete(
                        now + Duration::from_secs(1),
                        || {
                            clocks.set(clocks.get() + 1);
                            if late && clocks.get() > 1 {
                                now + Duration::from_secs(1)
                            } else {
                                now
                            }
                        },
                        || panic!(),
                        |_| {
                            checks.set(checks.get() + 1);
                            if checks.get() > 1 { Err(()) } else { Ok(()) }
                        }
                    )
                    .is_err()
            );
            assert_eq!(drops.get(), 0);
        }
    }
}
