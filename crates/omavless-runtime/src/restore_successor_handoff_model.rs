// SPDX-License-Identifier: MIT

//! Inactive, pure predecessor-to-successor restore handoff model. No disk
//! writer, recovery owner or product caller exists. A prior completion fence
//! must never be removed simply to make a second restore pass admission.

use crate::restore_closure_model::{ClosureRecord, RECORD_BYTES as CLOSURE_BYTES};
use crate::restore_decision_candidate::{
    DecisionPhase, DecisionRecord, RECORD_BYTES as DECISION_BYTES,
};
use crate::restore_staging_candidate::planned_stage_identity;
use sha2::{Digest, Sha256};

pub(crate) const SUCCESSOR_MEMBER: &str = "restore-successor.pending";
const MAGIC: &[u8; 8] = b"OVRSUC01";
const CHECKSUM_DOMAIN: &[u8] = b"omavless-restore-successor-v1\0";
const BODY_BYTES: usize = 8 + CLOSURE_BYTES + DECISION_BYTES;
pub(crate) const RECORD_BYTES: usize = BODY_BYTES + 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HandoffError {
    Invalid,
}

/// Fixed-size, credential-free bridge to a distinct planned transaction.
/// The predecessor record remains intact until a future writer has durably
/// published this bridge, staged the exact pair, and published its matching
/// intent. A checksum detects tears, not hostile same-user replacement.
pub(crate) struct SuccessorHandoff {
    predecessor: ClosureRecord,
    successor_intent: DecisionRecord,
}

impl SuccessorHandoff {
    /// Pure constructor. The caller must independently prove that the
    /// predecessor is completion-only, live files still match it, the host is
    /// idle and the old/new bytes came from authenticated sources. The staged
    /// identity is a *plan* until actual staging is inspected under a lease.
    /// All four members are supplied together so the predecessor comparison
    /// cannot be detached from the stage identity in the successor intent.
    pub(crate) fn from_verified_plan(
        predecessor: &ClosureRecord,
        successor_intent: &DecisionRecord,
        generation: u64,
        desired_raw: Option<&[u8]>,
        planned_members: [&[u8]; 4],
    ) -> Result<Self, HandoffError> {
        if !Self::bindings_match(
            predecessor,
            successor_intent,
            generation,
            desired_raw,
            planned_members,
        ) {
            return Err(HandoffError::Invalid);
        }
        Ok(Self {
            predecessor: ClosureRecord::decode(&predecessor.encode())
                .map_err(|_| HandoffError::Invalid)?,
            successor_intent: DecisionRecord::decode(&successor_intent.encode())
                .map_err(|_| HandoffError::Invalid)?,
        })
    }

    fn bindings_match(
        predecessor: &ClosureRecord,
        successor_intent: &DecisionRecord,
        generation: u64,
        desired_raw: Option<&[u8]>,
        planned_members: [&[u8]; 4],
    ) -> bool {
        let prior = predecessor.receipt();
        let Ok(stage) = planned_stage_identity(planned_members) else {
            return false;
        };
        successor_intent.phase() == DecisionPhase::Intent
            && prior.terminal().transaction_id() != successor_intent.transaction_id()
            && prior
                .terminal()
                .matches_owner_desired(generation, desired_raw)
            && successor_intent.matches_owner_desired(generation, desired_raw)
            && successor_intent.matches_stage_identity(&stage)
            && prior.matches_pair(planned_members[0], planned_members[1])
    }

    pub(crate) fn matches_verified_plan(
        &self,
        predecessor: &ClosureRecord,
        successor_intent: &DecisionRecord,
        generation: u64,
        desired_raw: Option<&[u8]>,
        planned_members: [&[u8]; 4],
    ) -> bool {
        self.predecessor.encode() == predecessor.encode()
            && self.successor_intent.encode() == successor_intent.encode()
            && Self::bindings_match(
                predecessor,
                successor_intent,
                generation,
                desired_raw,
                planned_members,
            )
    }

    pub(crate) fn predecessor(&self) -> &ClosureRecord {
        &self.predecessor
    }

    pub(crate) fn successor_intent(&self) -> &DecisionRecord {
        &self.successor_intent
    }

    pub(crate) fn encode(&self) -> [u8; RECORD_BYTES] {
        let mut raw = [0_u8; RECORD_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8..8 + CLOSURE_BYTES].copy_from_slice(&self.predecessor.encode());
        raw[8 + CLOSURE_BYTES..BODY_BYTES].copy_from_slice(&self.successor_intent.encode());
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&raw[..BODY_BYTES]);
        raw[BODY_BYTES..].copy_from_slice(&checksum.finalize());
        raw
    }

    pub(crate) fn decode(raw: &[u8]) -> Result<Self, HandoffError> {
        if raw.len() != RECORD_BYTES || &raw[..8] != MAGIC {
            return Err(HandoffError::Invalid);
        }
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&raw[..BODY_BYTES]);
        if raw[BODY_BYTES..] != checksum.finalize()[..] {
            return Err(HandoffError::Invalid);
        }
        let predecessor =
            ClosureRecord::decode(&raw[8..8 + CLOSURE_BYTES]).map_err(|_| HandoffError::Invalid)?;
        let successor_intent = DecisionRecord::decode(&raw[8 + CLOSURE_BYTES..BODY_BYTES])
            .map_err(|_| HandoffError::Invalid)?;
        if successor_intent.phase() != DecisionPhase::Intent
            || predecessor.receipt().terminal().transaction_id()
                == successor_intent.transaction_id()
        {
            return Err(HandoffError::Invalid);
        }
        Ok(Self {
            predecessor,
            successor_intent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::DesiredState;
    use crate::restore_decision_candidate::TerminalChoice;
    use crate::restore_retirement_candidate::RetirementReceipt;
    use crate::restore_staging_candidate::{StageIdentity, planned_stage_identity};

    const OLD_STORE: &[u8] = b"synthetic old store";
    const OLD_TEMPLATE: &[u8] = b"synthetic old template";
    const NEW_STORE: &[u8] = b"synthetic new store";
    const NEW_TEMPLATE: &[u8] = b"synthetic new template";
    const PLAN: [&[u8]; 4] = [OLD_STORE, OLD_TEMPLATE, NEW_STORE, NEW_TEMPLATE];

    fn example() -> (ClosureRecord, DecisionRecord, StageIdentity) {
        let previous_stage =
            planned_stage_identity([b"prior store", b"prior template", OLD_STORE, OLD_TEMPLATE])
                .unwrap();
        let terminal = DecisionRecord::intent(2, None, &previous_stage, [1; 16])
            .unwrap()
            .terminal(TerminalChoice::Commit)
            .unwrap();
        let receipt = RetirementReceipt::synthetic(&terminal, OLD_STORE, OLD_TEMPLATE);
        let predecessor = ClosureRecord::from_verified_receipt(&receipt).unwrap();
        let successor_stage = planned_stage_identity(PLAN).unwrap();
        let intent = DecisionRecord::intent(2, None, &successor_stage, [2; 16]).unwrap();
        (predecessor, intent, successor_stage)
    }

    #[test]
    fn second_restore_plan_round_trips_without_raw_pair_bytes() {
        let (predecessor, intent, _stage) = example();
        let handoff =
            SuccessorHandoff::from_verified_plan(&predecessor, &intent, 2, None, PLAN).unwrap();
        let raw = handoff.encode();
        assert!(
            !raw.windows(OLD_STORE.len())
                .any(|window| window == OLD_STORE)
        );
        assert!(
            !raw.windows(NEW_STORE.len())
                .any(|window| window == NEW_STORE)
        );
        let decoded = SuccessorHandoff::decode(&raw).unwrap();
        assert_eq!(decoded.predecessor().encode(), predecessor.encode());
        assert_eq!(decoded.successor_intent().encode(), intent.encode());
        assert!(decoded.matches_verified_plan(&predecessor, &intent, 2, None, PLAN,));
    }

    #[test]
    fn second_restore_refuses_owner_transaction_stage_and_old_pair_drift() {
        let (predecessor, intent, stage) = example();
        let handoff =
            SuccessorHandoff::from_verified_plan(&predecessor, &intent, 2, None, PLAN).unwrap();
        let same_id = DecisionRecord::intent(2, None, &stage, [1; 16]).unwrap();
        let other_id = DecisionRecord::intent(2, None, &stage, [3; 16]).unwrap();
        for (candidate, generation, planned) in [
            (&intent, 3, PLAN),
            (
                &intent,
                2,
                [OLD_STORE, OLD_TEMPLATE, b"different", NEW_TEMPLATE],
            ),
            (
                &intent,
                2,
                [b"changed old store", OLD_TEMPLATE, NEW_STORE, NEW_TEMPLATE],
            ),
            (&same_id, 2, PLAN),
        ] {
            assert!(
                SuccessorHandoff::from_verified_plan(
                    &predecessor,
                    candidate,
                    generation,
                    None,
                    planned,
                )
                .is_err()
            );
        }
        let foreign_plan = [
            b"foreign old store".as_slice(),
            OLD_TEMPLATE,
            NEW_STORE,
            NEW_TEMPLATE,
        ];
        let foreign_stage = planned_stage_identity(foreign_plan).unwrap();
        let foreign_intent = DecisionRecord::intent(2, None, &foreign_stage, [4; 16]).unwrap();
        assert!(
            SuccessorHandoff::from_verified_plan(
                &predecessor,
                &foreign_intent,
                2,
                None,
                foreign_plan,
            )
            .is_err()
        );
        assert!(!handoff.matches_verified_plan(&predecessor, &other_id, 2, None, PLAN,));
        let different_off_bytes = serde_json::to_vec(&DesiredState::default()).unwrap();
        assert!(!handoff.matches_verified_plan(
            &predecessor,
            &intent,
            2,
            Some(&different_off_bytes),
            PLAN,
        ));
        assert!(!handoff.matches_verified_plan(
            &predecessor,
            &intent,
            2,
            None,
            [OLD_STORE, b"changed old template", NEW_STORE, NEW_TEMPLATE],
        ));
    }

    #[test]
    fn successor_record_rejects_torn_wrong_length_and_wrong_phase() {
        let (predecessor, intent, _stage) = example();
        let handoff =
            SuccessorHandoff::from_verified_plan(&predecessor, &intent, 2, None, PLAN).unwrap();
        let raw = handoff.encode();
        for candidate in [&raw[..RECORD_BYTES - 1], &raw[1..]] {
            assert!(SuccessorHandoff::decode(candidate).is_err());
        }
        let mut torn = raw;
        torn[12] ^= 1;
        assert!(SuccessorHandoff::decode(&torn).is_err());
        let committed = intent.terminal(TerminalChoice::Commit).unwrap();
        let mut wrong_phase = raw;
        wrong_phase[8 + CLOSURE_BYTES..BODY_BYTES].copy_from_slice(&committed.encode());
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&wrong_phase[..BODY_BYTES]);
        wrong_phase[BODY_BYTES..].copy_from_slice(&checksum.finalize());
        assert!(SuccessorHandoff::decode(&wrong_phase).is_err());
    }
}
