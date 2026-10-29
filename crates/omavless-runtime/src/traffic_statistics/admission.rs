// SPDX-License-Identifier: MIT
//! Slow proof runs only on the existing detached statistics worker. The short
//! seal and owner fence covers candidate selection, never filesystem I/O.
use super::worker::Error;
use std::sync::{Arc, RwLock};

pub(crate) type Preflight = Box<dyn FnOnce() -> Result<Admission, Error> + Send>;
pub(crate) struct Admission {
    pub(crate) gate: Arc<RwLock<bool>>,
    pub(crate) verify: Box<dyn FnOnce() -> Result<(), Error> + Send>,
}
impl Admission {
    pub(crate) fn run<T>(self, select: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        let gate = self.gate.try_read().map_err(|_| Error::Busy)?;
        if *gate {
            return Err(Error::Restarting);
        }
        (self.verify)()?;
        // Admission is the candidate selection itself; a queued receipt is not
        // permission to write after an intervening whole-runtime seal.
        select()
    }
    #[cfg(test)]
    pub(crate) fn test_preflight() -> Preflight {
        Box::new(|| {
            Ok(Self {
                gate: Arc::new(RwLock::new(false)),
                verify: Box::new(|| Ok(())),
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
            verify: Box::new(|| Ok(())),
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
            verify: Box::new(|| panic!("sealed request cannot reach owner")),
        };
        assert_eq!(
            late.run(|| panic!("sealed request cannot select candidate")),
            Err::<(), _>(Error::Restarting)
        );
    }
}
