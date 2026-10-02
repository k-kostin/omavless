// SPDX-License-Identifier: MIT

//! Inactive, pure decision model for retaining proof across the last restore
//! receipt unlink. No filesystem operation, product startup or IPC calls this.
//! In particular, an absent receipt is never by itself proof of completion.

/// Opaque identity of a separately validated terminal receipt. A future
/// durable completion record must retain enough information to independently
/// revalidate owner/desired/live bindings after the pending receipt is gone;
/// this model intentionally does not choose its on-disk format.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReceiptIdentity([u8; 32]);

impl ReceiptIdentity {
    #[cfg(test)]
    fn synthetic(value: u8) -> Self {
        Self([value; 32])
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Evidence {
    Absent,
    Valid(ReceiptIdentity),
    Invalid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosureReview {
    /// Could be a fresh installation or lost evidence; not success proof.
    NoEvidence,
    /// The final receipt still fences normal startup, with or without a
    /// matching durably published completion record.
    PendingFence,
    /// The completion record survived without the pending receipt. This is a
    /// candidate for a later independent read-only startup review, not an
    /// instruction to start the ordinary owner.
    VerifyCompleted,
    ManualRecovery,
}

#[derive(Clone, Copy)]
pub(crate) struct ClosureFacts {
    pub pending: Evidence,
    pub completed: Evidence,
    pub fixed_artifacts_absent: bool,
    pub owner_off_and_live_pair_verified: bool,
}

/// Model only. Both valid evidence variants must have been authenticated and
/// bound by independent I/O before reaching this function. A future writer
/// must make completion durable before unlinking the last pending receipt.
pub(crate) fn review(facts: ClosureFacts) -> ClosureReview {
    use ClosureReview as R;
    use Evidence as E;
    if matches!(facts.pending, E::Invalid) || matches!(facts.completed, E::Invalid) {
        return R::ManualRecovery;
    }
    match (facts.pending, facts.completed) {
        (E::Absent, E::Absent) => R::NoEvidence,
        (E::Valid(_), E::Absent) if facts.owner_off_and_live_pair_verified => R::PendingFence,
        (E::Valid(pending), E::Valid(completed))
            if pending == completed
                && facts.fixed_artifacts_absent
                && facts.owner_off_and_live_pair_verified =>
        {
            R::PendingFence
        }
        (E::Absent, E::Valid(_))
            if facts.fixed_artifacts_absent && facts.owner_off_and_live_pair_verified =>
        {
            R::VerifyCompleted
        }
        _ => R::ManualRecovery,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(pending: Evidence, completed: Evidence) -> ClosureFacts {
        ClosureFacts {
            pending,
            completed,
            fixed_artifacts_absent: true,
            owner_off_and_live_pair_verified: true,
        }
    }

    #[test]
    fn absent_evidence_is_never_a_completed_restore() {
        assert!(review(facts(Evidence::Absent, Evidence::Absent)) == ClosureReview::NoEvidence);
        let mut lost_live = facts(
            Evidence::Absent,
            Evidence::Valid(ReceiptIdentity::synthetic(1)),
        );
        lost_live.owner_off_and_live_pair_verified = false;
        assert!(review(lost_live) == ClosureReview::ManualRecovery);
    }

    #[test]
    fn completion_requires_matching_durable_evidence_and_retired_artifacts() {
        let identity = ReceiptIdentity::synthetic(1);
        assert!(
            review(facts(Evidence::Valid(identity), Evidence::Absent))
                == ClosureReview::PendingFence
        );
        assert!(
            review(facts(Evidence::Valid(identity), Evidence::Valid(identity)))
                == ClosureReview::PendingFence
        );
        assert!(
            review(facts(Evidence::Absent, Evidence::Valid(identity)))
                == ClosureReview::VerifyCompleted
        );
        assert!(
            review(facts(
                Evidence::Valid(identity),
                Evidence::Valid(ReceiptIdentity::synthetic(2)),
            )) == ClosureReview::ManualRecovery
        );
        for pending in [Evidence::Absent, Evidence::Valid(identity)] {
            let mut unfinished = facts(pending, Evidence::Valid(identity));
            unfinished.fixed_artifacts_absent = false;
            assert!(review(unfinished) == ClosureReview::ManualRecovery);
        }
        for pending in [Evidence::Invalid, Evidence::Valid(identity)] {
            assert!(review(facts(pending, Evidence::Invalid)) == ClosureReview::ManualRecovery);
        }
    }

    #[test]
    fn every_crash_point_preserves_pending_or_completion_evidence() {
        let identity = ReceiptIdentity::synthetic(7);
        // Before publishing the completion record, a crash leaves the pending
        // receipt. During publication it may leave either old-only or both.
        for (pending, completed, expected) in [
            (
                Evidence::Valid(identity),
                Evidence::Absent,
                ClosureReview::PendingFence,
            ),
            (
                Evidence::Valid(identity),
                Evidence::Valid(identity),
                ClosureReview::PendingFence,
            ),
            // After durable publication, unlink + sync may leave both or only
            // the completion record. Neither case is ordinary startup success.
            (
                Evidence::Absent,
                Evidence::Valid(identity),
                ClosureReview::VerifyCompleted,
            ),
        ] {
            assert!(review(facts(pending, completed)) == expected);
        }
        // A hypothetical state with neither item violates the required
        // publication order. It is never interpreted as completion.
        assert!(review(facts(Evidence::Absent, Evidence::Absent)) == ClosureReview::NoEvidence);
    }
}
