// SPDX-License-Identifier: MIT

//! Inactive, pure decision model for retaining proof across the last restore
//! receipt unlink. No filesystem operation, product startup or IPC calls this.
//! In particular, an absent receipt is never by itself proof of completion.

use crate::restore_retirement_candidate::{RECEIPT_BYTES, RetirementReceipt};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"OVRCLS01";
const CHECKSUM_DOMAIN: &[u8] = b"omavless-restore-closure-v1\0";
const ID_DOMAIN: &[u8] = b"omavless-restore-receipt-id-v1\0";
const BODY_BYTES: usize = 8 + RECEIPT_BYTES;
pub(crate) const RECORD_BYTES: usize = BODY_BYTES + 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClosureRecordError {
    Invalid,
}

/// Inactive fixed-size candidate carrying the entire terminal receipt, not
/// just its hash. This allows an eventual independent restart reader to
/// recheck owner/desired/live bindings after the pending receipt is gone.
/// Its checksum detects tears, not a hostile same-user rewrite. Encoding
/// alone has no durability or startup authority.
pub(crate) struct ClosureRecord {
    receipt: RetirementReceipt,
}

impl ClosureRecord {
    pub(crate) fn from_verified_receipt(
        receipt: &RetirementReceipt,
    ) -> Result<Self, ClosureRecordError> {
        Ok(Self {
            receipt: RetirementReceipt::decode(&receipt.encode())
                .map_err(|_| ClosureRecordError::Invalid)?,
        })
    }

    pub(crate) fn encode(&self) -> [u8; RECORD_BYTES] {
        let mut raw = [0_u8; RECORD_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8..BODY_BYTES].copy_from_slice(&self.receipt.encode());
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&raw[..BODY_BYTES]);
        raw[BODY_BYTES..].copy_from_slice(&checksum.finalize());
        raw
    }

    pub(crate) fn decode(raw: &[u8]) -> Result<Self, ClosureRecordError> {
        if raw.len() != RECORD_BYTES || &raw[..8] != MAGIC {
            return Err(ClosureRecordError::Invalid);
        }
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&raw[..BODY_BYTES]);
        if raw[BODY_BYTES..] != checksum.finalize()[..] {
            return Err(ClosureRecordError::Invalid);
        }
        let receipt = RetirementReceipt::decode(&raw[8..BODY_BYTES])
            .map_err(|_| ClosureRecordError::Invalid)?;
        if receipt.encode() != raw[8..BODY_BYTES] {
            return Err(ClosureRecordError::Invalid);
        }
        Ok(Self { receipt })
    }

    pub(crate) fn receipt(&self) -> &RetirementReceipt {
        &self.receipt
    }

    pub(crate) fn matches_pending(&self, pending: &RetirementReceipt) -> bool {
        self.receipt.encode() == pending.encode()
    }

    pub(crate) fn identity(&self) -> ReceiptIdentity {
        ReceiptIdentity::from_receipt(&self.receipt)
    }
}

/// Opaque identity of a separately validated terminal receipt. A future
/// durable completion record must retain enough information to independently
/// revalidate owner/desired/live bindings after the pending receipt is gone;
/// this model intentionally does not choose its on-disk format.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReceiptIdentity([u8; 32]);

impl ReceiptIdentity {
    pub(crate) fn from_receipt(receipt: &RetirementReceipt) -> Self {
        let mut hash = Sha256::new();
        hash.update(ID_DOMAIN);
        hash.update(receipt.encode());
        Self(hash.finalize().into())
    }

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
