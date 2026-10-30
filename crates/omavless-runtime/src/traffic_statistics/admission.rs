// SPDX-License-Identifier: MIT
//! Slow proof runs only on the existing detached statistics worker. The short
//! seal and owner fence covers candidate selection, never filesystem I/O.
use super::collection::Capture;
use super::worker::Error;
use crate::RuntimeDispatcher;
use std::sync::{Arc, Mutex, RwLock, Weak};

pub(crate) type Preflight = Box<dyn FnOnce() -> Result<Admission, Error> + Send>;
type Selection<'a> = dyn FnMut() -> Result<(), Error> + 'a;
type OwnerFence = Box<dyn FnOnce(&mut Selection<'_>) -> Result<(), Error> + Send>;
pub(crate) struct Admission {
    pub(crate) gate: Arc<RwLock<bool>>,
    pub(crate) verify: OwnerFence,
}
impl Admission {
    pub(crate) fn run<T>(self, select: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        let gate = self.gate.try_read().map_err(|_| Error::Busy)?;
        if *gate {
            return Err(Error::Restarting);
        }
        let mut select = Some(select);
        let mut selected = None;
        (self.verify)(&mut || {
            let select = select.take().ok_or(Error::Conflict)?;
            selected = Some(select()?);
            Ok(())
        })?;
        selected.ok_or(Error::Unavailable)
    }
    pub(crate) fn current_owner(
        gate: Arc<RwLock<bool>>,
        dispatcher: Weak<Mutex<RuntimeDispatcher>>,
        capture: Capture,
        read: bool,
    ) -> Self {
        Self {
            gate,
            verify: Box::new(move |select| {
                let owner = dispatcher.upgrade().ok_or(Error::Restarting)?;
                let dispatcher = owner.try_lock().map_err(|_| Error::Busy)?;
                let RuntimeDispatcher::Native(owner) = &*dispatcher else {
                    return Err(Error::Unavailable);
                };
                let current = if read {
                    owner.stats_capture().is_some_and(|c| {
                        c.revision == capture.revision
                            && c.owner_generation == capture.owner_generation
                    })
                } else {
                    owner.stats_off_current(&capture)
                };
                if !current {
                    return Err(Error::Conflict);
                }
                // The same memory-only owner guard remains held through selection.
                // Ordinary Connect cannot interleave between Off proof and admission.
                select()
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_preflight() -> Preflight {
        Box::new(|| {
            Ok(Self {
                gate: Arc::new(RwLock::new(false)),
                verify: Box::new(|select| select()),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics_seal_covers_candidate_selection_and_refuses_late_admission() {
        let gate = Arc::new(RwLock::new(false));
        let check = Arc::clone(&gate);
        let mut selected = false;
        Admission {
            gate: Arc::clone(&gate),
            verify: Box::new(|select| select()),
        }
        .run(|| {
            assert!(
                check.try_write().is_err(),
                "seal cannot interleave candidate admission"
            );
            selected = true;
            Ok(())
        })
        .unwrap();
        assert!(selected);
        *gate.write().unwrap() = true;
        let late = Admission {
            gate,
            verify: Box::new(|_| panic!("sealed request cannot reach owner")),
        };
        assert_eq!(
            late.run(|| panic!("sealed request cannot select candidate")),
            Err::<(), _>(Error::Restarting)
        );
    }
}
