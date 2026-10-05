// SPDX-License-Identifier: MIT
//! Inert charged original-row owner. No proc/query/backend or wire caller.
//! Production integration must supply genuine strict capture/observation bodies.

use super::Unavailable;

const MAX_ROWS: usize = 4096;
const SCRATCH: usize = 8;
// Candidate ONLY: the fixed origin/query/lower total still needs full counting.
const CANDIDATE_FIXED: usize = 120;
const CANDIDATE_NOFILE: usize = 8320;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    OtherUid,
    SameUid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Rows,
    Sweep,
    Complete,
    Revoked,
    Finished,
}

struct Row<T> {
    pid: u32,
    directory: T,
    executable: Option<T>,
    class: Option<Class>,
    complete: bool,
}

struct Scratch<T> {
    original: T,
    completed: bool,
}

struct Owner<T> {
    expected: Vec<u32>,
    rows: Vec<Row<T>>,
    scratch: [Option<Scratch<T>>; SCRATCH],
    phase: Phase,
    charged: usize,
    bound: usize,
    swept: usize,
}

impl<T> Owner<T> {
    fn reserve(expected: &[u32], fixed: usize, bound: usize) -> Result<Self, Unavailable> {
        if expected.is_empty()
            || expected.len() > MAX_ROWS
            || expected[0] == 0
            || expected.windows(2).any(|pair| pair[0] >= pair[1])
            || fixed.checked_add(SCRATCH).is_none_or(|need| need > bound)
        {
            return Err(Unavailable);
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(expected.len())
            .map_err(|_| Unavailable)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(expected.len())
            .map_err(|_| Unavailable)?;
        names.extend_from_slice(expected);
        Ok(Self {
            expected: names,
            rows,
            scratch: std::array::from_fn(|_| None),
            phase: Phase::Rows,
            charged: fixed,
            bound,
            swept: 0,
        })
    }

    fn refuse(&mut self) -> Result<(), Unavailable> {
        self.phase = Phase::Revoked;
        Err(Unavailable)
    }

    fn guarded<R>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<R, Unavailable>,
    ) -> Result<R, Unavailable> {
        if matches!(self.phase, Phase::Revoked | Phase::Finished) {
            return Err(Unavailable);
        }
        let result = body(self);
        if result.is_err() {
            self.phase = Phase::Revoked;
        }
        result
    }

    fn charge(&mut self) -> Result<(), Unavailable> {
        let next = self.charged.checked_add(1).ok_or(Unavailable)?;
        if next > self.bound {
            return Err(Unavailable);
        }
        self.charged = next; // BEFORE backend/open, not after EMFILE
        Ok(())
    }

    fn current_row(&self) -> Result<usize, Unavailable> {
        match self.phase {
            Phase::Rows if self.rows.last().is_some_and(|row| !row.complete) => {
                Ok(self.rows.len() - 1)
            }
            Phase::Sweep if self.swept < self.rows.len() => Ok(self.swept),
            _ => Err(Unavailable),
        }
    }

    fn directory(
        &mut self,
        pid: u32,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        acquire: impl FnOnce() -> Result<T, Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Rows
                || owner.expected.get(owner.rows.len()) != Some(&pid)
                || owner.rows.last().is_some_and(|row| !row.complete)
                || owner.scratch.iter().any(Option::is_some)
            {
                return Err(Unavailable);
            }
            gate()?;
            owner.charge()?;
            let directory = acquire()?;
            owner.rows.push(Row {
                pid,
                directory,
                executable: None,
                class: None,
                complete: false,
            });
            gate() // reported original is already retained if late
        })
    }

    fn classify(
        &mut self,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        strict_capture: impl FnOnce(&T) -> Result<Class, Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Rows {
                return Err(Unavailable);
            }
            let row = owner.rows.last_mut().ok_or(Unavailable)?;
            if row.class.is_some() || row.complete {
                return Err(Unavailable);
            }
            gate()?;
            let class = strict_capture(&row.directory)?;
            gate()?;
            row.class = Some(class);
            Ok(())
        })
    }

    fn executable(
        &mut self,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        acquire_from_original: impl FnOnce(&T) -> Result<T, Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Rows
                || owner.rows.last().is_none_or(|row| {
                    row.class != Some(Class::SameUid) || row.executable.is_some() || row.complete
                })
            {
                return Err(Unavailable);
            }
            gate()?;
            owner.charge()?;
            let row = owner.rows.last_mut().ok_or(Unavailable)?;
            let executable = acquire_from_original(&row.directory)?;
            row.executable = Some(executable);
            gate()
        })
    }

    fn scratch_acquire(
        &mut self,
        slot: usize,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        acquire_from_original: impl FnOnce(&T) -> Result<T, Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if slot >= SCRATCH || owner.scratch[slot].is_some() {
                return Err(Unavailable);
            }
            let index = owner.current_row()?;
            gate()?;
            owner.charge()?;
            let original = acquire_from_original(&owner.rows[index].directory)?;
            owner.scratch[slot] = Some(Scratch {
                original,
                completed: false,
            });
            gate()
        })
    }

    fn scratch_perform(
        &mut self,
        slot: usize,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        operation: impl FnOnce(&T) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if !matches!(owner.phase, Phase::Rows | Phase::Sweep) || slot >= SCRATCH {
                return Err(Unavailable);
            }
            let scratch = owner.scratch[slot].as_mut().ok_or(Unavailable)?;
            scratch.completed = false;
            gate()?;
            operation(&scratch.original)?;
            gate()?;
            scratch.completed = true;
            Ok(())
        })
    }

    fn scratch_release(
        &mut self,
        slot: usize,
        gate: impl FnOnce() -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if !matches!(owner.phase, Phase::Rows | Phase::Sweep)
                || slot >= SCRATCH
                || owner.scratch[slot]
                    .as_ref()
                    .is_none_or(|scratch| !scratch.completed)
            {
                return Err(Unavailable);
            }
            gate()?; // every fallible check BEFORE positive-only Drop
            let completed = owner.scratch[slot].take().ok_or(Unavailable)?;
            owner.charged -= 1;
            drop(completed); // ordinary completed-close backend, not uncertain eviction
            Ok(())
        })
    }

    fn complete_row(
        &mut self,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        strict_recheck: impl FnOnce(&T, Option<&T>, Class) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Rows || owner.scratch.iter().any(Option::is_some) {
                return Err(Unavailable);
            }
            let row = owner.rows.last_mut().ok_or(Unavailable)?;
            let class = row.class.ok_or(Unavailable)?;
            if row.complete || (class == Class::SameUid) != row.executable.is_some() {
                return Err(Unavailable);
            }
            gate()?;
            strict_recheck(&row.directory, row.executable.as_ref(), class)?;
            gate()?;
            row.complete = true;
            Ok(())
        })
    }

    fn begin_sweep(&mut self, current: &[u32]) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Rows
                || owner.rows.len() != owner.expected.len()
                || owner.rows.iter().any(|row| !row.complete)
                || current != owner.expected
            {
                return Err(Unavailable);
            }
            owner.phase = Phase::Sweep;
            Ok(())
        })
    }

    fn recheck_row(
        &mut self,
        pid: u32,
        mut gate: impl FnMut() -> Result<(), Unavailable>,
        recheck_original: impl FnOnce(&T, Option<&T>, Class) -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Sweep
                || owner.expected.get(owner.swept) != Some(&pid)
                || owner.scratch.iter().any(Option::is_some)
            {
                return Err(Unavailable);
            }
            let row = &owner.rows[owner.swept];
            if row.pid != pid {
                return Err(Unavailable);
            }
            gate()?;
            recheck_original(
                &row.directory,
                row.executable.as_ref(),
                row.class.ok_or(Unavailable)?,
            )?;
            gate()?;
            owner.swept += 1;
            Ok(())
        })
    }

    fn complete_inventory(
        &mut self,
        current: &[u32],
        gate: impl FnOnce() -> Result<(), Unavailable>,
    ) -> Result<(), Unavailable> {
        self.guarded(|owner| {
            if owner.phase != Phase::Sweep
                || owner.swept != owner.expected.len()
                || current != owner.expected
                || owner.scratch.iter().any(Option::is_some)
            {
                return Err(Unavailable);
            }
            gate()?;
            owner.phase = Phase::Complete;
            Ok(())
        })
    }

    fn finish(&mut self) -> Result<(), Unavailable> {
        if self.phase != Phase::Complete {
            return self.refuse();
        }
        self.phase = Phase::Finished;
        self.rows.clear(); // only separately valid completed-owner finish
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Handle {
        id: usize,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    fn handle(id: usize, drops: &Rc<Cell<usize>>) -> Handle {
        Handle {
            id,
            drops: drops.clone(),
        }
    }

    fn other_row(owner: &mut Owner<Handle>, pid: u32, drops: &Rc<Cell<usize>>) {
        owner
            .directory(pid, || Ok(()), || Ok(handle(pid as usize, drops)))
            .unwrap();
        owner.classify(|| Ok(()), |_| Ok(Class::OtherUid)).unwrap();
        owner
            .complete_row(
                || Ok(()),
                |directory, executable, class| {
                    assert_eq!(directory.id, pid as usize);
                    assert!(executable.is_none());
                    assert!(class == Class::OtherUid);
                    Ok(())
                },
            )
            .unwrap();
    }

    #[test]
    fn worst_same_uid_catalogue_reserves_all_originals_plus_scratch() {
        let pids: Vec<u32> = (1..=MAX_ROWS as u32).collect();
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve(&pids, CANDIDATE_FIXED, CANDIDATE_NOFILE).unwrap();
        for pid in &pids {
            owner
                .directory(*pid, || Ok(()), || Ok(handle(*pid as usize, &drops)))
                .unwrap();
            owner.classify(|| Ok(()), |_| Ok(Class::SameUid)).unwrap();
            owner
                .executable(
                    || Ok(()),
                    |original| Ok(handle(original.id + MAX_ROWS, &drops)),
                )
                .unwrap();
            // At the last row, every persistent slot plus all eight scratch fits.
            if *pid == MAX_ROWS as u32 {
                for slot in 0..SCRATCH {
                    owner
                        .scratch_acquire(slot, || Ok(()), |_| Ok(handle(slot, &drops)))
                        .unwrap();
                }
                assert_eq!(owner.charged, CANDIDATE_NOFILE);
                for slot in 0..SCRATCH {
                    owner.scratch_perform(slot, || Ok(()), |_| Ok(())).unwrap();
                    owner.scratch_release(slot, || Ok(())).unwrap();
                }
            }
            owner.complete_row(|| Ok(()), |_, _, _| Ok(())).unwrap();
        }
        assert_eq!(owner.charged, MAX_ROWS * 2 + CANDIDATE_FIXED);
        assert_eq!(drops.get(), SCRATCH);
        owner.begin_sweep(&pids).unwrap();
        for pid in &pids {
            owner
                .scratch_acquire(
                    0,
                    || Ok(()),
                    |original| {
                        assert_eq!(original.id, *pid as usize);
                        Ok(handle(0, &drops))
                    },
                )
                .unwrap();
            owner.scratch_perform(0, || Ok(()), |_| Ok(())).unwrap();
            owner.scratch_release(0, || Ok(())).unwrap();
            owner
                .recheck_row(
                    *pid,
                    || Ok(()),
                    |original, executable, _| {
                        assert_eq!(original.id, *pid as usize);
                        assert_eq!(executable.unwrap().id, *pid as usize + MAX_ROWS);
                        Ok(())
                    },
                )
                .unwrap();
        }
        owner.complete_inventory(&pids, || Ok(())).unwrap();
        assert_eq!(drops.get(), SCRATCH + MAX_ROWS);
        owner.finish().unwrap();
        assert_eq!(drops.get(), SCRATCH + MAX_ROWS * 3);
    }

    #[test]
    fn scratch_acquire_and_last_release_gate_keep_reported_prefix() {
        for cut in 0..3 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
            owner
                .directory(1, || Ok(()), || Ok(handle(1, &drops)))
                .unwrap();
            let calls = Cell::new(0);
            assert!(
                owner
                    .scratch_acquire(
                        0,
                        || {
                            let n = calls.get();
                            calls.set(n + 1);
                            if n == cut { Err(Unavailable) } else { Ok(()) }
                        },
                        |_| if cut == 2 {
                            Err(Unavailable)
                        } else {
                            Ok(handle(2, &drops))
                        }
                    )
                    .is_err()
            );
            assert_eq!(owner.scratch[0].is_some(), cut == 1);
            assert_eq!(drops.get(), 0);
            assert!(
                owner
                    .scratch_acquire(1, || panic!("reentry gate"), |_| panic!("reentry open"))
                    .is_err()
            );
        }
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
        owner
            .directory(1, || Ok(()), || Ok(handle(1, &drops)))
            .unwrap();
        owner
            .scratch_acquire(0, || Ok(()), |_| Ok(handle(2, &drops)))
            .unwrap();
        owner.scratch_perform(0, || Ok(()), |_| Ok(())).unwrap();
        assert!(owner.scratch_release(0, || Err(Unavailable)).is_err());
        assert!(owner.scratch[0].is_some());
        assert_eq!(drops.get(), 0);
        assert!(owner.finish().is_err());
    }

    #[test]
    fn required_image_and_original_row_classification_cannot_be_omitted() {
        for cut in 0..3 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
            owner
                .directory(1, || Ok(()), || Ok(handle(1, &drops)))
                .unwrap();
            if cut != 0 {
                owner
                    .classify(
                        || Ok(()),
                        |_| {
                            Ok(if cut == 1 {
                                Class::SameUid
                            } else {
                                Class::OtherUid
                            })
                        },
                    )
                    .unwrap();
            }
            if cut == 2 {
                assert!(
                    owner
                        .executable(
                            || panic!("foreign image gate"),
                            |_| panic!("foreign image open")
                        )
                        .is_err()
                );
            } else {
                assert!(
                    owner
                        .complete_row(
                            || panic!("missing admission gate"),
                            |_, _, _| panic!("missing admission recheck")
                        )
                        .is_err()
                );
            }
            assert_eq!(drops.get(), 0);
            assert!(owner.finish().is_err());
        }
    }

    #[test]
    fn exact_4096_row_capacity_has_no_skip_or_active_limit_change() {
        assert_eq!(MAX_ROWS * 2 + CANDIDATE_FIXED + SCRATCH, CANDIDATE_NOFILE);
        assert_eq!(super::super::ACTOR_NOFILE, 64);
        let pids: Vec<u32> = (1..=MAX_ROWS as u32).collect();
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve(&pids, CANDIDATE_FIXED, CANDIDATE_NOFILE).unwrap();
        for pid in &pids {
            other_row(&mut owner, *pid, &drops);
        }
        assert_eq!(drops.get(), 0); // even negative UID rows retain original directory
        owner.begin_sweep(&pids).unwrap();
        for pid in &pids {
            owner
                .recheck_row(
                    *pid,
                    || Ok(()),
                    |directory, _, _| {
                        assert_eq!(directory.id, *pid as usize);
                        Ok(())
                    },
                )
                .unwrap();
        }
        owner.complete_inventory(&pids, || Ok(())).unwrap();
        owner.finish().unwrap();
        assert_eq!(drops.get(), MAX_ROWS);
    }

    #[test]
    fn capacity_and_every_directory_cut_refuse_before_next_open() {
        for cut in 0..3 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
            let calls = Cell::new(0);
            let opened = Cell::new(0);
            assert!(
                owner
                    .directory(
                        1,
                        || {
                            let n = calls.get();
                            calls.set(n + 1);
                            if n == cut { Err(Unavailable) } else { Ok(()) }
                        },
                        || {
                            opened.set(opened.get() + 1);
                            if cut == 2 {
                                Err(Unavailable)
                            } else {
                                Ok(handle(1, &drops))
                            }
                        }
                    )
                    .is_err()
            );
            assert_eq!(opened.get(), usize::from(cut != 0));
            assert_eq!(owner.rows.len(), usize::from(cut == 1));
            assert_eq!(drops.get(), 0);
            assert!(
                owner
                    .directory(1, || panic!("revoked gate"), || panic!("revoked open"))
                    .is_err()
            );
            assert!(owner.finish().is_err());
        }
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve(&(1..=9).collect::<Vec<_>>(), 0, 8).unwrap();
        for pid in 1..=8 {
            other_row(&mut owner, pid, &drops);
        }
        assert!(
            owner
                .directory(9, || Ok(()), || panic!("capacity open"))
                .is_err()
        );
        assert_eq!(owner.charged, 8);
        assert_eq!(drops.get(), 0);
    }

    #[test]
    fn one_original_directory_is_reused_for_same_uid_and_final_sweep() {
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
        owner
            .directory(1, || Ok(()), || Ok(handle(41, &drops)))
            .unwrap();
        owner
            .classify(
                || Ok(()),
                |original| {
                    assert_eq!(original.id, 41);
                    Ok(Class::SameUid)
                },
            )
            .unwrap();
        owner
            .executable(
                || Ok(()),
                |original| {
                    assert_eq!(original.id, 41);
                    Ok(handle(42, &drops))
                },
            )
            .unwrap();
        owner
            .complete_row(
                || Ok(()),
                |original, executable, class| {
                    assert_eq!(original.id, 41);
                    assert_eq!(executable.unwrap().id, 42);
                    assert!(class == Class::SameUid);
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(owner.charged, 2);
        owner.begin_sweep(&[1]).unwrap();
        owner
            .recheck_row(
                1,
                || Ok(()),
                |original, executable, _| {
                    assert_eq!(original.id, 41);
                    assert_eq!(executable.unwrap().id, 42);
                    Ok(())
                },
            )
            .unwrap();
        owner.complete_inventory(&[1], || Ok(())).unwrap();
        owner.finish().unwrap();
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn scratch_release_requires_completed_io_and_preserves_each_error_prefix() {
        for cut in 0..4 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
            owner
                .directory(1, || Ok(()), || Ok(handle(1, &drops)))
                .unwrap();
            owner
                .scratch_acquire(0, || Ok(()), |_| Ok(handle(2, &drops)))
                .unwrap();
            let calls = Cell::new(0);
            if cut == 3 {
                assert!(
                    owner
                        .scratch_release(0, || panic!("uncompleted release gate"))
                        .is_err()
                );
            } else {
                assert!(
                    owner
                        .scratch_perform(
                            0,
                            || {
                                let n = calls.get();
                                calls.set(n + 1);
                                if n == cut { Err(Unavailable) } else { Ok(()) }
                            },
                            |_| if cut == 2 { Err(Unavailable) } else { Ok(()) }
                        )
                        .is_err()
                );
            }
            assert_eq!(drops.get(), 0);
            assert!(owner.scratch[0].is_some());
            assert!(
                owner
                    .scratch_release(0, || panic!("revoked release"))
                    .is_err()
            );
            assert!(owner.finish().is_err());
        }
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve(&[1], 0, 8).unwrap();
        owner
            .directory(1, || Ok(()), || Ok(handle(1, &drops)))
            .unwrap();
        for id in 2..=12 {
            owner
                .scratch_acquire(
                    0,
                    || Ok(()),
                    |original| {
                        assert_eq!(original.id, 1);
                        Ok(handle(id, &drops))
                    },
                )
                .unwrap();
            owner.scratch_perform(0, || Ok(()), |_| Ok(())).unwrap();
            owner.scratch_release(0, || Ok(())).unwrap();
        }
        assert_eq!(drops.get(), 11);
        assert_eq!(owner.charged, 1); // original row stays
    }

    #[test]
    fn malformed_omitted_or_changed_catalogue_and_reentry_never_skip_rows() {
        for invalid in [&[][..], &[0][..], &[1, 1][..], &[2, 1][..]] {
            assert!(Owner::<Handle>::reserve(invalid, 0, 8).is_err());
        }
        for cut in 0..5 {
            let drops = Rc::new(Cell::new(0));
            let mut owner = Owner::reserve(&[1, 2], 0, 8).unwrap();
            other_row(&mut owner, 1, &drops);
            if cut == 0 {
                assert!(owner.begin_sweep(&[1, 2]).is_err());
            } else {
                other_row(&mut owner, 2, &drops);
                if cut == 1 {
                    assert!(owner.begin_sweep(&[1, 3]).is_err());
                } else {
                    owner.begin_sweep(&[1, 2]).unwrap();
                    if cut == 2 {
                        assert!(
                            owner
                                .recheck_row(
                                    2,
                                    || panic!("skipped row gate"),
                                    |_, _, _| panic!("skipped row read")
                                )
                                .is_err()
                        );
                    } else {
                        owner.recheck_row(1, || Ok(()), |_, _, _| Ok(())).unwrap();
                        if cut == 3 {
                            assert!(
                                owner
                                    .complete_inventory(&[1, 2], || panic!("incomplete gate"))
                                    .is_err()
                            );
                        } else {
                            owner.recheck_row(2, || Ok(()), |_, _, _| Ok(())).unwrap();
                            assert!(
                                owner
                                    .complete_inventory(&[1, 3], || panic!("changed final gate"))
                                    .is_err()
                            );
                        }
                    }
                }
            }
            assert_eq!(drops.get(), 0);
            assert!(owner.finish().is_err());
            assert!(
                owner
                    .directory(1, || panic!("revoked gate"), || panic!("revoked open"))
                    .is_err()
            );
        }
    }
}
