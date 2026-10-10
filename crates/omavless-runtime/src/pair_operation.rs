// SPDX-License-Identifier: MIT
//! Fixed-capacity, non-evicting result metadata, never execution authority.
use crate::mutation::{CachedOutcome, CoordinatorError, MutationDigest};
use omavless_control_protocol::StableErrorCode;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const LIMIT: usize = 8;
struct Entry {
    id: String,
    digest: MutationDigest,
    identity: Arc<AtomicBool>,
    result: Option<CachedOutcome>,
    uncertain: bool,
}
pub(crate) struct PairOperations(Vec<Entry>);
impl Default for PairOperations {
    fn default() -> Self {
        Self(Vec::with_capacity(LIMIT))
    }
}
pub(crate) struct Reservation(Arc<AtomicBool>);
impl Drop for Reservation {
    fn drop(&mut self) {
        // If the caller unwinds, this SAME reservation permanently denies new
        // work. It never fabricates a result or releases an execution graph.
        self.0.store(true, Ordering::Release);
    }
}
pub(crate) enum Admission {
    Reserved(Reservation),
    Replay(CachedOutcome),
}
/// Only the reached backend error taxonomy may select a settled denial. This
/// metadata cannot authorize execution or turn a post-effect error into refusal.
pub(crate) enum Outcome {
    Completed,
    InputDenied,
    DestinationExists,
    Unknown,
}
impl PairOperations {
    pub(crate) fn known(&self, id: &str) -> bool {
        self.0.iter().any(|e| e.id == id)
    }
    pub(crate) fn unresolved(&self) -> bool {
        self.0.iter().any(|e| e.result.is_none() || e.uncertain)
    }
    pub(crate) fn abandoned(&self) -> bool {
        self.0
            .iter()
            .any(|e| e.identity.load(Ordering::Acquire) && (e.result.is_none() || e.uncertain))
    }
    pub(crate) fn replay(
        &self,
        id: &str,
        digest: MutationDigest,
    ) -> Result<Option<CachedOutcome>, CoordinatorError> {
        let Some(e) = self.0.iter().find(|e| e.id == id) else {
            return Ok(None);
        };
        if e.digest != digest {
            return Err(CoordinatorError::OperationConflict);
        }
        e.result.map(Some).ok_or(CoordinatorError::Busy)
    }
    pub(crate) fn reserve(
        &mut self,
        id: &str,
        digest: MutationDigest,
    ) -> Result<Reservation, CoordinatorError> {
        if self.known(id) {
            return Err(CoordinatorError::OperationConflict);
        }
        if self.unresolved() || self.0.len() == LIMIT {
            return Err(CoordinatorError::Busy);
        }
        let identity = Arc::new(AtomicBool::new(false));
        self.0.push(Entry {
            id: id.to_owned(),
            digest,
            identity: identity.clone(),
            result: None,
            uncertain: false,
        });
        Ok(Reservation(identity))
    }
    pub(crate) fn finish(
        &mut self,
        reservation: Reservation,
        revision: u64,
        outcome: Outcome,
    ) -> Result<CachedOutcome, CoordinatorError> {
        let e = self
            .0
            .iter_mut()
            .find(|e| Arc::ptr_eq(&e.identity, &reservation.0))
            .ok_or(CoordinatorError::InvalidToken)?;
        if e.result.is_some() || e.identity.load(Ordering::Acquire) {
            return Err(CoordinatorError::InvalidToken);
        }
        let result = CachedOutcome {
            revision,
            error: match outcome {
                Outcome::Completed => None,
                Outcome::InputDenied => Some(StableErrorCode::InvalidArgument),
                Outcome::DestinationExists => Some(StableErrorCode::Conflict),
                Outcome::Unknown => Some(StableErrorCode::ManualRecoveryRequired),
            },
        };
        e.uncertain = matches!(outcome, Outcome::Unknown);
        e.result = Some(result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn digest(n: u8) -> MutationDigest {
        MutationDigest::new([n; 32])
    }
    #[test]
    fn fixed_pair_results_replay_without_eviction_or_authority() {
        let mut r = PairOperations::default();
        for n in 0..LIMIT {
            let id = format!("pair-{n}");
            let token = r.reserve(&id, digest(n as u8)).unwrap();
            let result = r.finish(token, n as u64, Outcome::Completed).unwrap();
            assert_eq!(r.replay(&id, digest(n as u8)).unwrap(), Some(result));
        }
        assert!(matches!(
            r.reserve("ninth", digest(9)),
            Err(CoordinatorError::Busy)
        ));
        assert!(r.replay("pair-0", digest(99)).is_err());
        assert!(!r.unresolved());
    }
    #[test]
    fn unknown_and_unwind_never_restore_resend_eligibility() {
        let mut r = PairOperations::default();
        let token = r.reserve("unknown", digest(1)).unwrap();
        let result = r.finish(token, 7, Outcome::Unknown).unwrap();
        assert_eq!(result.error, Some(StableErrorCode::ManualRecoveryRequired));
        assert!(r.abandoned());
        assert_eq!(r.replay("unknown", digest(1)).unwrap(), Some(result));
        assert!(r.reserve("new", digest(2)).is_err());
        let mut r = PairOperations::default();
        let token = r.reserve("unwind", digest(3)).unwrap();
        assert!(
            std::panic::catch_unwind(|| {
                let _held = token;
                panic!("synthetic");
            })
            .is_err()
        );
        assert!(r.abandoned());
        assert!(r.replay("unwind", digest(3)).is_err());
        assert!(r.reserve("new", digest(4)).is_err());
    }
    #[test]
    fn settled_denial_replays_without_blocking_a_distinct_operation() {
        let mut r = PairOperations::default();
        for (id, outcome) in [
            ("input", Outcome::InputDenied),
            ("exists", Outcome::DestinationExists),
        ] {
            let token = r.reserve(id, digest(1)).unwrap();
            let result = r.finish(token, 0, outcome).unwrap();
            assert!(result.error.is_some());
            assert_eq!(r.replay(id, digest(1)).unwrap(), Some(result));
            assert!(!r.unresolved());
            assert!(!r.abandoned());
        }
        let token = r.reserve("valid", digest(2)).unwrap();
        assert!(
            r.finish(token, 0, Outcome::Completed)
                .unwrap()
                .error
                .is_none()
        );
    }
}
