// SPDX-License-Identifier: MIT

//! Inactive, pure model for a future durable two-file restore decision.
//! Encoding is not persistence, and a decoded record is not an authority to
//! mutate live files. There is no product caller or recovery executor.

use crate::desired::{DesiredState, MAX_DESIRED_STATE_BYTES, MAX_GENERATION};
use crate::restore_staging_candidate::{LivePairClass, StageIdentity, VerifiedLivePair};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"OVRDEC01";
const CHECKSUM_DOMAIN: &[u8] = b"omavless-restore-decision-v1\0";
const BODY_BYTES: usize = 8 + 1 + 8 + 8 + 1 + 32 + 32 + 16;
const RECORD_BYTES: usize = BODY_BYTES + 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecisionError {
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecisionPhase {
    Intent,
    Committed,
    Aborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalChoice {
    Commit,
    Abort,
}

/// Each result is only a candidate for a later, fully revalidated procedure.
/// In particular, New without a durable Committed decision never means success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryReview {
    OldRollbackCandidate,
    VerifyCommittedCandidate,
    VerifyAbortedCandidate,
    ManualRecovery,
}

/// Credential-free, fixed-size identity. No Debug/Display/serialization of a
/// record or its hashes; this checksum detects tears, not hostile same-user
/// edits. A future writer must durably bind it to the exact owner/lease.
#[derive(PartialEq, Eq)]
pub(crate) struct DecisionRecord {
    phase: DecisionPhase,
    owner_generation: u64,
    desired_generation: u64,
    desired_present: bool,
    desired_digest: [u8; 32],
    stage_digest: [u8; 32],
    transaction_id: [u8; 16],
}

fn checksum(body: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CHECKSUM_DOMAIN);
    hash.update(body);
    hash.finalize().into()
}

fn desired_binding(raw: Option<&[u8]>) -> Result<(u64, bool, [u8; 32]), DecisionError> {
    let desired = match raw {
        Some(bytes) if !bytes.is_empty() && bytes.len() as u64 <= MAX_DESIRED_STATE_BYTES => {
            serde_json::from_slice::<DesiredState>(bytes).map_err(|_| DecisionError::Invalid)?
        }
        None => DesiredState::default(),
        Some(_) => return Err(DecisionError::Invalid),
    };
    desired.validate().map_err(|_| DecisionError::Invalid)?;
    if desired.connected {
        return Err(DecisionError::Invalid);
    }
    Ok((
        desired.generation,
        raw.is_some(),
        raw.map(|bytes| Sha256::digest(bytes).into())
            .unwrap_or([0; 32]),
    ))
}

impl DecisionRecord {
    /// This constructor checks an Off desired snapshot, not its filesystem
    /// origin or the current host. The caller supplies a fresh random ID.
    pub(crate) fn intent(
        owner_generation: u64,
        desired_raw: Option<&[u8]>,
        stage: &StageIdentity,
        transaction_id: [u8; 16],
    ) -> Result<Self, DecisionError> {
        if owner_generation == 0
            || owner_generation > MAX_GENERATION
            || transaction_id.iter().all(|byte| *byte == 0)
        {
            return Err(DecisionError::Invalid);
        }
        let (desired_generation, desired_present, desired_digest) = desired_binding(desired_raw)?;
        Ok(Self {
            phase: DecisionPhase::Intent,
            owner_generation,
            desired_generation,
            desired_present,
            desired_digest,
            stage_digest: stage.digest(),
            transaction_id,
        })
    }

    pub(crate) fn phase(&self) -> DecisionPhase {
        self.phase
    }

    /// A terminal record keeps every bound byte; it cannot change an earlier
    /// terminal choice or substitute another transaction identity.
    pub(crate) fn terminal(&self, choice: TerminalChoice) -> Result<Self, DecisionError> {
        if self.phase != DecisionPhase::Intent {
            return Err(DecisionError::Invalid);
        }
        Ok(Self {
            phase: match choice {
                TerminalChoice::Commit => DecisionPhase::Committed,
                TerminalChoice::Abort => DecisionPhase::Aborted,
            },
            owner_generation: self.owner_generation,
            desired_generation: self.desired_generation,
            desired_present: self.desired_present,
            desired_digest: self.desired_digest,
            stage_digest: self.stage_digest,
            transaction_id: self.transaction_id,
        })
    }

    pub(crate) fn same_transaction(&self, other: &Self) -> bool {
        self.owner_generation == other.owner_generation
            && self.desired_generation == other.desired_generation
            && self.desired_present == other.desired_present
            && self.desired_digest == other.desired_digest
            && self.stage_digest == other.stage_digest
            && self.transaction_id == other.transaction_id
    }

    /// Every review must bind a fresh stage inspection, exact owner generation
    /// and exact Off desired-state bytes. This is still a point-in-time check.
    pub(crate) fn matches_current_bindings(
        &self,
        owner_generation: u64,
        desired_raw: Option<&[u8]>,
        stage: &StageIdentity,
    ) -> bool {
        if owner_generation != self.owner_generation || stage.digest() != self.stage_digest {
            return false;
        }
        desired_binding(desired_raw).is_ok_and(|(generation, present, digest)| {
            generation == self.desired_generation
                && present == self.desired_present
                && digest == self.desired_digest
        })
    }

    pub(crate) fn encode(&self) -> [u8; RECORD_BYTES] {
        let mut raw = [0_u8; RECORD_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8] = match self.phase {
            DecisionPhase::Intent => 1,
            DecisionPhase::Committed => 2,
            DecisionPhase::Aborted => 3,
        };
        raw[9..17].copy_from_slice(&self.owner_generation.to_be_bytes());
        raw[17..25].copy_from_slice(&self.desired_generation.to_be_bytes());
        raw[25] = u8::from(self.desired_present);
        raw[26..58].copy_from_slice(&self.desired_digest);
        raw[58..90].copy_from_slice(&self.stage_digest);
        raw[90..106].copy_from_slice(&self.transaction_id);
        let check = checksum(&raw[..BODY_BYTES]);
        raw[BODY_BYTES..].copy_from_slice(&check);
        raw
    }

    pub(crate) fn decode(raw: &[u8]) -> Result<Self, DecisionError> {
        if raw.len() != RECORD_BYTES
            || &raw[..8] != MAGIC
            || raw[BODY_BYTES..] != checksum(&raw[..BODY_BYTES])
        {
            return Err(DecisionError::Invalid);
        }
        let phase = match raw[8] {
            1 => DecisionPhase::Intent,
            2 => DecisionPhase::Committed,
            3 => DecisionPhase::Aborted,
            _ => return Err(DecisionError::Invalid),
        };
        let owner_generation = u64::from_be_bytes(raw[9..17].try_into().unwrap());
        let desired_generation = u64::from_be_bytes(raw[17..25].try_into().unwrap());
        let desired_present = match raw[25] {
            0 => false,
            1 => true,
            _ => return Err(DecisionError::Invalid),
        };
        let desired_digest: [u8; 32] = raw[26..58].try_into().unwrap();
        let stage_digest: [u8; 32] = raw[58..90].try_into().unwrap();
        let transaction_id: [u8; 16] = raw[90..106].try_into().unwrap();
        if owner_generation == 0
            || owner_generation > MAX_GENERATION
            || desired_generation > MAX_GENERATION
            || (!desired_present && (desired_generation != 0 || desired_digest != [0; 32]))
            || stage_digest == [0; 32]
            || transaction_id == [0; 16]
        {
            return Err(DecisionError::Invalid);
        }
        Ok(Self {
            phase,
            owner_generation,
            desired_generation,
            desired_present,
            desired_digest,
            stage_digest,
            transaction_id,
        })
    }

    pub(crate) fn review(
        &self,
        owner_generation: u64,
        desired_raw: Option<&[u8]>,
        observed: &VerifiedLivePair,
    ) -> RecoveryReview {
        if !self.matches_current_bindings(owner_generation, desired_raw, observed.stage()) {
            return RecoveryReview::ManualRecovery;
        }
        match (self.phase, observed.class()) {
            (
                DecisionPhase::Intent,
                LivePairClass::Old
                | LivePairClass::New
                | LivePairClass::Identical
                | LivePairClass::Mixed,
            ) => RecoveryReview::OldRollbackCandidate,
            (DecisionPhase::Committed, LivePairClass::New | LivePairClass::Identical) => {
                RecoveryReview::VerifyCommittedCandidate
            }
            (DecisionPhase::Aborted, LivePairClass::Old | LivePairClass::Identical) => {
                RecoveryReview::VerifyAbortedCandidate
            }
            _ => RecoveryReview::ManualRecovery,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_staging_candidate::{inspect_stage_identity, stage_private_pair};
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    fn stage_with(new_store: &[u8]) -> (std::path::PathBuf, u32, StageIdentity) {
        let home = std::env::var_os("HOME").expect("test needs home");
        let root = crate::test_temp::directory_under(Path::new(&home), "restore-decision").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        stage_private_pair(
            &root,
            uid,
            b"old store",
            b"old template",
            new_store,
            b"new template",
        )
        .unwrap();
        let identity = inspect_stage_identity(&root, uid).unwrap();
        (root, uid, identity)
    }

    fn stage() -> (std::path::PathBuf, u32, StageIdentity) {
        stage_with(b"new store")
    }

    fn valid_desired() -> Vec<u8> {
        serde_json::to_vec(&DesiredState {
            generation: 9,
            ..DesiredState::default()
        })
        .unwrap()
    }

    fn with_valid_checksum(mut wire: [u8; RECORD_BYTES]) -> [u8; RECORD_BYTES] {
        let check = checksum(&wire[..BODY_BYTES]);
        wire[BODY_BYTES..].copy_from_slice(&check);
        wire
    }

    #[test]
    fn fixed_record_roundtrip_and_terminal_identity_are_exact() {
        let (root, _, stage) = stage();
        let desired = valid_desired();
        let intent = DecisionRecord::intent(3, Some(&desired), &stage, [7; 16]).unwrap();
        let wire = intent.encode();
        assert_eq!(wire.len(), RECORD_BYTES);
        assert!(DecisionRecord::decode(&wire).unwrap() == intent);
        assert!(!wire.windows(desired.len()).any(|part| part == desired));
        let committed = intent.terminal(TerminalChoice::Commit).unwrap();
        let aborted = intent.terminal(TerminalChoice::Abort).unwrap();
        assert!(committed.same_transaction(&intent));
        assert!(aborted.same_transaction(&intent));
        assert_ne!(committed.phase(), aborted.phase());
        assert!(committed.terminal(TerminalChoice::Abort).is_err());
        assert!(DecisionRecord::decode(&committed.encode()).unwrap() == committed);
        assert!(DecisionRecord::decode(&aborted.encode()).unwrap() == aborted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desired_must_be_off_and_record_refuses_torn_or_incompatible_bytes() {
        let (root, _, stage) = stage();
        let connected = DesiredState {
            connected: true,
            profile_id: "synthetic-id".into(),
            ..DesiredState::default()
        };
        let connected = serde_json::to_vec(&connected).unwrap();
        assert!(DecisionRecord::intent(3, Some(&connected), &stage, [7; 16]).is_err());
        assert!(DecisionRecord::intent(0, None, &stage, [7; 16]).is_err());
        assert!(DecisionRecord::intent(3, None, &stage, [0; 16]).is_err());
        assert!(DecisionRecord::intent(3, Some(b"invalid"), &stage, [7; 16]).is_err());
        let absent = DecisionRecord::intent(3, None, &stage, [7; 16]).unwrap();
        assert!(DecisionRecord::decode(&absent.encode()).unwrap() == absent);
        let valid = DecisionRecord::intent(3, Some(&valid_desired()), &stage, [7; 16])
            .unwrap()
            .encode();
        for cut in 0..valid.len() {
            assert!(DecisionRecord::decode(&valid[..cut]).is_err());
        }
        assert!(DecisionRecord::decode(&[valid.as_slice(), &[0]].concat()).is_err());
        for index in [0, 8, 9, 17, 25, 58, 90, 105, BODY_BYTES] {
            let mut damaged = valid;
            damaged[index] ^= 1;
            assert!(DecisionRecord::decode(&damaged).is_err());
        }
        for invalid in [
            {
                let mut wire = valid;
                wire[8] = 4;
                wire
            },
            {
                let mut wire = valid;
                wire[9..17].fill(0);
                wire
            },
            {
                let mut wire = valid;
                wire[17..25].fill(0xff);
                wire
            },
            {
                let mut wire = valid;
                wire[25] = 2;
                wire
            },
            {
                let mut wire = valid;
                wire[58..90].fill(0);
                wire
            },
            {
                let mut wire = valid;
                wire[90..106].fill(0);
                wire
            },
        ] {
            assert!(DecisionRecord::decode(&with_valid_checksum(invalid)).is_err());
        }
        let mut absent_with_digest = absent.encode();
        absent_with_digest[26] = 1;
        assert!(DecisionRecord::decode(&with_valid_checksum(absent_with_digest)).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_review_never_treats_new_without_commit_as_success() {
        let (root, _, stage) = stage();
        let intent = DecisionRecord::intent(3, None, &stage, [7; 16]).unwrap();
        for live in [
            LivePairClass::Old,
            LivePairClass::New,
            LivePairClass::Identical,
            LivePairClass::Mixed,
        ] {
            assert_eq!(
                intent.review(3, None, &VerifiedLivePair::synthetic(stage, live)),
                RecoveryReview::OldRollbackCandidate
            );
        }
        assert_eq!(
            intent.review(
                3,
                None,
                &VerifiedLivePair::synthetic(stage, LivePairClass::Diverged)
            ),
            RecoveryReview::ManualRecovery
        );
        let committed = intent.terminal(TerminalChoice::Commit).unwrap();
        let aborted = intent.terminal(TerminalChoice::Abort).unwrap();
        for live in [
            LivePairClass::Old,
            LivePairClass::New,
            LivePairClass::Identical,
            LivePairClass::Mixed,
            LivePairClass::Diverged,
        ] {
            assert_eq!(
                committed.review(3, None, &VerifiedLivePair::synthetic(stage, live))
                    == RecoveryReview::VerifyCommittedCandidate,
                matches!(live, LivePairClass::New | LivePairClass::Identical)
            );
            assert_eq!(
                aborted.review(3, None, &VerifiedLivePair::synthetic(stage, live))
                    == RecoveryReview::VerifyAbortedCandidate,
                matches!(live, LivePairClass::Old | LivePairClass::Identical)
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn review_refuses_different_owner_desired_or_verified_stage() {
        let (root, _, stage) = stage();
        let (other_root, _, other_stage) = stage_with(b"different new store");
        let desired = valid_desired();
        let record = DecisionRecord::intent(3, Some(&desired), &stage, [7; 16])
            .unwrap()
            .terminal(TerminalChoice::Commit)
            .unwrap();
        let valid_review = RecoveryReview::VerifyCommittedCandidate;
        let live = LivePairClass::New;
        let observed = VerifiedLivePair::synthetic(stage, live);
        assert_eq!(record.review(3, Some(&desired), &observed), valid_review);
        assert_eq!(
            record.review(4, Some(&desired), &observed),
            RecoveryReview::ManualRecovery
        );
        assert_eq!(
            record.review(3, None, &observed),
            RecoveryReview::ManualRecovery
        );
        assert_eq!(
            record.review(
                3,
                Some(&desired),
                &VerifiedLivePair::synthetic(other_stage, live)
            ),
            RecoveryReview::ManualRecovery
        );
        let other_off = serde_json::to_vec(&DesiredState {
            mode: crate::desired::RoutingMode::Global,
            generation: 9,
            ..DesiredState::default()
        })
        .unwrap();
        assert_eq!(
            record.review(3, Some(&other_off), &observed),
            RecoveryReview::ManualRecovery
        );
        let connected = serde_json::to_vec(&DesiredState {
            connected: true,
            profile_id: "synthetic-id".into(),
            generation: 9,
            ..DesiredState::default()
        })
        .unwrap();
        assert_eq!(
            record.review(3, Some(&connected), &observed),
            RecoveryReview::ManualRecovery
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other_root).unwrap();
    }
}
