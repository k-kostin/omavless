// SPDX-License-Identifier: MIT
//! Inactive fixed ticket. Syntax is not authentication, durability or admission.
use crate::restore_closure_model::{ClosureRecord, RECORD_BYTES as CLOSURE_BYTES};
use sha2::{Digest, Sha256};

pub(crate) const TICKET_MEMBER: &str = "restore-disposition.pending";
/// Existence-only fence, including unsafe/inaccessible entries. Never decode
/// a ticket here or infer permission from its contents.
pub(crate) fn pending_at(directory: &std::path::Path) -> bool {
    !matches!(std::fs::symlink_metadata(directory.join(TICKET_MEMBER)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}
const MAGIC: &[u8; 8] = b"OVRDSP01";
const DOMAIN: &[u8] = b"omavless-restore-disposition-candidate-v1\0";
const BODY: usize = 24 + CLOSURE_BYTES;
pub(crate) const TICKET_BYTES: usize = BODY + 32;

/// No profile/template/passphrase bytes; the nested closure has only bounded
/// terminal metadata, lengths and digests. Not a historical admission token.
pub(crate) struct Ticket {
    uid: u32,
    generation: u64,
    closure: ClosureRecord,
}

impl Ticket {
    pub(crate) fn from_bound_closure(
        closure: &ClosureRecord,
        uid: u32,
        generation: u64,
        desired: Option<&[u8]>,
    ) -> Option<Self> {
        if !closure
            .receipt()
            .terminal()
            .matches_owner_desired(generation, desired)
        {
            return None;
        }
        Some(Self {
            uid,
            generation,
            closure: ClosureRecord::decode(&closure.encode()).ok()?,
        })
    }
    pub(crate) fn encode(&self) -> [u8; TICKET_BYTES] {
        let mut raw = [0; TICKET_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8..12].copy_from_slice(&self.uid.to_le_bytes());
        raw[16..24].copy_from_slice(&self.generation.to_le_bytes());
        raw[24..BODY].copy_from_slice(&self.closure.encode());
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update(&raw[..BODY]);
        raw[BODY..].copy_from_slice(&hash.finalize());
        raw
    }
    pub(crate) fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() != TICKET_BYTES || &raw[..8] != MAGIC || raw[12..16] != [0; 4] {
            return None;
        }
        let value = Self {
            uid: u32::from_le_bytes(raw[8..12].try_into().ok()?),
            generation: u64::from_le_bytes(raw[16..24].try_into().ok()?),
            closure: ClosureRecord::decode(&raw[24..BODY]).ok()?,
        };
        (value.encode() == raw
            && value
                .closure
                .receipt()
                .terminal()
                .matches_owner_generation(value.generation))
        .then_some(value)
    }
    pub(crate) fn matches(
        &self,
        closure: &ClosureRecord,
        uid: u32,
        generation: u64,
        desired: Option<&[u8]>,
    ) -> bool {
        self.uid == uid
            && self.generation == generation
            && self.closure.encode() == closure.encode()
            && closure
                .receipt()
                .terminal()
                .matches_owner_desired(generation, desired)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_decision_candidate::{DecisionRecord, TerminalChoice};
    use crate::restore_retirement_candidate::RetirementReceipt;
    use crate::restore_staging_candidate::planned_stage_identity;
    #[test]
    fn disposition_ticket_rechecksummed_invalid_metadata_is_not_canonical() {
        let stage = planned_stage_identity([b"same", b"template", b"same", b"template"]).unwrap();
        for outcome in [TerminalChoice::Commit, TerminalChoice::Abort] {
            let terminal = DecisionRecord::intent(7, None, &stage, [1; 16])
                .unwrap()
                .terminal(outcome)
                .unwrap();
            let receipt = RetirementReceipt::synthetic(&terminal, b"same", b"template");
            let closure = ClosureRecord::from_verified_receipt(&receipt).unwrap();
            let ticket = Ticket::from_bound_closure(&closure, 1000, 7, None).unwrap();
            assert!(Ticket::from_bound_closure(&closure, 1000, 8, None).is_none());
            for index in [0, 7, 12, 15, 16] {
                let mut raw = ticket.encode();
                raw[index] ^= 1;
                let mut hash = Sha256::new();
                hash.update(DOMAIN);
                hash.update(&raw[..BODY]);
                raw[BODY..].copy_from_slice(&hash.finalize());
                assert!(Ticket::decode(&raw).is_none());
            }
            let mut other_uid = ticket.encode();
            other_uid[8..12].copy_from_slice(&1001_u32.to_le_bytes());
            let mut hash = Sha256::new();
            hash.update(DOMAIN);
            hash.update(&other_uid[..BODY]);
            other_uid[BODY..].copy_from_slice(&hash.finalize());
            assert!(
                !Ticket::decode(&other_uid)
                    .unwrap()
                    .matches(&closure, 1000, 7, None)
            );
        }
    }
}
