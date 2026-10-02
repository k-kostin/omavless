// SPDX-License-Identifier: MIT
//! Inactive terminal disposition record. Canonical bytes are not authority:
//! only a reviewed, ordered publisher and fresh startup admission can use it.

use crate::restore_disposition_ticket_model::{TICKET_BYTES, Ticket};
use sha2::{Digest, Sha256};

pub(crate) const COMPLETE_MEMBER: &str = "restore-disposition.complete";
const MAGIC: &[u8; 8] = b"OVRDON01";
const DOMAIN: &[u8] = b"omavless-restore-disposition-complete-v1\0";
const BODY: usize = 8 + TICKET_BYTES;
pub(crate) const COMPLETE_BYTES: usize = BODY + 32;

/// Embeds the exact canonical ticket, including its UID, owner generation,
/// terminal outcome and complete C1. It contains no credentials or archive
/// path. Its checksum detects corruption, not same-UID tampering.
pub(crate) struct CompleteRecord {
    ticket: Ticket,
}

impl CompleteRecord {
    pub(crate) fn from_ticket(ticket: &Ticket) -> Option<Self> {
        Some(Self {
            ticket: Ticket::decode(&ticket.encode())?,
        })
    }

    pub(crate) fn encode(&self) -> [u8; COMPLETE_BYTES] {
        let mut raw = [0_u8; COMPLETE_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8..BODY].copy_from_slice(&self.ticket.encode());
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update(&raw[..BODY]);
        raw[BODY..].copy_from_slice(&hash.finalize());
        raw
    }

    pub(crate) fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() != COMPLETE_BYTES || &raw[..8] != MAGIC {
            return None;
        }
        let record = Self {
            ticket: Ticket::decode(&raw[8..BODY])?,
        };
        (record.encode() == raw).then_some(record)
    }

    pub(crate) fn matches_ticket(&self, ticket: &Ticket) -> bool {
        self.ticket.encode() == ticket.encode()
    }
}

pub(crate) fn pending_at(directory: &std::path::Path) -> bool {
    !matches!(
        std::fs::symlink_metadata(directory.join(COMPLETE_MEMBER)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_decision_candidate::{DecisionRecord, TerminalChoice};
    use crate::restore_retirement_candidate::RetirementReceipt;
    use crate::restore_staging_candidate::planned_stage_identity;

    fn ticket(choice: TerminalChoice) -> Ticket {
        let stage = planned_stage_identity([b"old", b"template", b"new", b"template"]).unwrap();
        let terminal = DecisionRecord::intent(7, None, &stage, [3; 16])
            .unwrap()
            .terminal(choice)
            .unwrap();
        let live = if choice == TerminalChoice::Commit {
            b"new".as_slice()
        } else {
            b"old".as_slice()
        };
        let receipt = RetirementReceipt::synthetic(&terminal, live, b"template");
        let closure =
            crate::restore_closure_model::ClosureRecord::from_verified_receipt(&receipt).unwrap();
        Ticket::from_bound_closure(&closure, 1000, 7, None).unwrap()
    }

    #[test]
    fn complete_record_binds_exact_canonical_ticket_for_both_outcomes() {
        for choice in [TerminalChoice::Commit, TerminalChoice::Abort] {
            let expected_ticket = ticket(choice);
            let raw = CompleteRecord::from_ticket(&expected_ticket)
                .unwrap()
                .encode();
            let decoded = CompleteRecord::decode(&raw).unwrap();
            assert!(decoded.matches_ticket(&expected_ticket));
            assert!(
                !decoded.matches_ticket(&ticket(if choice == TerminalChoice::Commit {
                    TerminalChoice::Abort
                } else {
                    TerminalChoice::Commit
                }))
            );
            for index in [0, 7, 8, BODY - 1, BODY, COMPLETE_BYTES - 1] {
                let mut changed = raw;
                changed[index] ^= 1;
                assert!(CompleteRecord::decode(&changed).is_none());
            }
            assert!(CompleteRecord::decode(&raw[..raw.len() - 1]).is_none());
            assert!(CompleteRecord::decode(&[raw.as_slice(), b"x"].concat()).is_none());
        }
    }
}
