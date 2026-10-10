//! Descriptor-free SOURCE research; not a process, receiver, or production API.
//!
//! The original owner below is a synthetic Rust value. Borrowing it cannot
//! establish real process acquisition, an unshared FD table, authenticated
//! completion, non-exiting quarantine, or survival through unwinding/abort.
//! Completion inputs are fabricated observations for state-machine controls,
//! not a parser or serialized authority. No runtime interface is changed.

#![forbid(unsafe_code)]

/// Public state labels contain no process or descriptor identity.
#[derive(Debug, PartialEq, Eq)]
pub enum Status { Fresh, Prepared, Live, Quarantined }

/// A single synthetic refusal; no recovery or cleanup permission is implied.
#[derive(Debug, PartialEq, Eq)]
pub struct Refused;

/// Fixed synthetic operations; no arbitrary command or descriptor forwarding.
#[derive(Clone, Copy)]
pub enum Observation { ExecutableMatched, PidNamespaceMatched, UserNamespaceMatched }

/// Fabricated completion facts, used only by descriptor-free transition tests.
/// All four facts must be positive. Actual validation/authentication is absent.
pub struct Completion {
    pub receive_returned_success: bool,
    pub all_known_slots_retained_and_validated: bool,
    pub positive_reply_complete: bool,
    pub original_channel_authenticated: bool,
}

/// Synthetic original owner. Its immutable observations stand in for results,
/// not FDs, a descriptor inventory, a process handle, or acquisition evidence.
/// It has no Clone, serialization, numeric identity, reset, or cleanup method.
pub struct SyntheticActor {
    status: Status,
    observations: [bool; 3],
}

impl SyntheticActor {
    /// Creates only an in-memory model, never an actual actor or capability.
    pub fn new(observations: [bool; 3]) -> Self {
        Self { status: Status::Fresh, observations }
    }

    /// Reports only this model's label; not a live process observation.
    pub fn status(&self) -> &Status { &self.status }

    /// Permanently consumes the sole preparation eligibility before returning
    /// the non-Clone original mutable borrow. A refusal never resets it.
    pub fn prepare(&mut self) -> Result<Prepared<'_>, Refused> {
        if self.status != Status::Fresh { return Err(Refused); }
        self.status = Status::Prepared;
        Ok(Prepared { original: Some(self) })
    }
}

/// Incomplete preparation keeps the original owner borrowed. No operation API.
pub struct Prepared<'a> { original: Option<&'a mut SyntheticActor> }

impl<'a> Prepared<'a> {
    /// Promotes the SAME original borrow on one complete synthetic positive
    /// event. Any uncertainty seals its owner; bytes/tokens cannot reconstruct
    /// an owner. This models a transition, not the correctness of these facts.
    pub fn complete(mut self, event: Completion) -> Result<Live<'a>, Refused> {
        let original = self.original.take().ok_or(Refused)?;
        if original.status != Status::Prepared
            || !event.receive_returned_success
            || !event.all_known_slots_retained_and_validated
            || !event.positive_reply_complete
            || !event.original_channel_authenticated
        {
            original.status = Status::Quarantined;
            return Err(Refused);
        }
        original.status = Status::Live;
        Ok(Live { original: Some(original) })
    }
}

impl Drop for Prepared<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.as_mut() {
            original.status = Status::Quarantined;
        }
    }
}

/// Live operations remain on the same non-Clone original mutable borrow.
/// Dropping this model handle quarantines the owner; it never releases FDs.
pub struct Live<'a> { original: Option<&'a mut SyntheticActor> }

impl Live<'_> {
    /// Returns a fixed synthetic result, not a descriptor or new capability.
    pub fn observe(&mut self, request: Observation) -> Result<bool, Refused> {
        let original = self.original.as_mut().ok_or(Refused)?;
        if original.status != Status::Live { return Err(Refused); }
        let index = match request {
            Observation::ExecutableMatched => 0,
            Observation::PidNamespaceMatched => 1,
            Observation::UserNamespaceMatched => 2,
        };
        Ok(original.observations[index])
    }

    /// Synthetic uncertain operation has no query/retry/reap/cleanup action.
    /// Consumes this handle and permanently quarantines the same original.
    pub fn uncertain(mut self) {
        if let Some(original) = self.original.take() {
            original.status = Status::Quarantined;
        }
    }
}

impl Drop for Live<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.as_mut() {
            original.status = Status::Quarantined;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn positive() -> Completion {
        Completion { receive_returned_success: true,
            all_known_slots_retained_and_validated: true,
            positive_reply_complete: true, original_channel_authenticated: true }
    }

    #[test]
    fn exact_positive_uses_original_results_only() {
        let mut owner = SyntheticActor::new([true, false, true]);
        let mut live = owner.prepare().unwrap().complete(positive()).unwrap();
        assert_eq!(live.observe(Observation::ExecutableMatched), Ok(true));
        assert_eq!(live.observe(Observation::PidNamespaceMatched), Ok(false));
        assert_eq!(live.observe(Observation::UserNamespaceMatched), Ok(true));
        drop(live);
        assert_eq!(owner.status(), &Status::Quarantined);
        assert!(owner.prepare().is_err());
    }

    #[test]
    fn every_nonpositive_fact_seals_without_promotion() {
        for cut in 0..4 {
            let mut owner = SyntheticActor::new([false; 3]);
            let mut event = positive();
            match cut {
                0 => event.receive_returned_success = false,
                1 => event.all_known_slots_retained_and_validated = false,
                2 => event.positive_reply_complete = false,
                _ => event.original_channel_authenticated = false,
            }
            assert!(owner.prepare().unwrap().complete(event).is_err());
            assert_eq!(owner.status(), &Status::Quarantined);
            assert!(owner.prepare().is_err());
        }
    }

    #[test]
    fn abandoned_preparation_cannot_reset_owner() {
        let mut owner = SyntheticActor::new([false; 3]);
        drop(owner.prepare().unwrap());
        assert_eq!(owner.status(), &Status::Quarantined);
        assert!(owner.prepare().is_err());
    }

    #[test]
    fn uncertain_live_consumes_same_owner_without_recovery() {
        let mut owner = SyntheticActor::new([true; 3]);
        owner.prepare().unwrap().complete(positive()).unwrap().uncertain();
        assert_eq!(owner.status(), &Status::Quarantined);
        assert_eq!(owner.observations, [true; 3]);
        assert!(owner.prepare().is_err());
    }

    #[test]
    fn fresh_label_is_not_live_authority() {
        let owner = SyntheticActor::new([true; 3]);
        assert_eq!(owner.status(), &Status::Fresh);
    }

    #[test]
    fn incomplete_drop_during_caught_unwind_only_models_label_poison() {
        let mut owner = SyntheticActor::new([false; 3]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _prepared = owner.prepare().unwrap();
            panic!("synthetic control only");
        }));
        assert!(result.is_err());
        assert_eq!(owner.status(), &Status::Quarantined);
        assert!(owner.prepare().is_err());
    }
}
