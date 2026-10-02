// SPDX-License-Identifier: MIT
//! Pure, inactive discussion model. No disk format, I/O, persisted admission
//! ticket, source observer or normal-startup authority is defined here.

use crate::restore_closure_model::{ClosureRecord, ReceiptIdentity};
use crate::restore_decision_candidate::DecisionPhase;

#[path = "restore_disposition_policy_model.rs"]
pub(crate) mod proposed_policy;

/// Exact terminal output identity, not full predecessor lineage. No raw
/// profile/template bytes, public field access or printable identity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClosureBinding {
    identity: ReceiptIdentity,
    transaction: [u8; 16],
    outcome: DecisionPhase,
    owner_generation: u64,
}

impl ClosureBinding {
    /// Pure consistency check only: neither decoding nor matching caller-
    /// supplied desired bytes proves filesystem origin, authentication or sync.
    pub(crate) fn from_record(
        record: &ClosureRecord,
        owner_generation: u64,
        desired: Option<&[u8]>,
    ) -> Option<Self> {
        let canonical = ClosureRecord::decode(&record.encode()).ok()?;
        let terminal = canonical.receipt().terminal();
        if !terminal.matches_owner_desired(owner_generation, desired)
            || terminal.phase() == DecisionPhase::Intent
        {
            return None;
        }
        Some(Self {
            identity: canonical.identity(),
            transaction: terminal.transaction_id(),
            outcome: terminal.phase(),
            owner_generation,
        })
    }
}

/// A hypothetical matching disposition assertion, NOT an admission receipt or
/// proof that recovery happened. There is deliberately no encoder or writer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DispositionCandidate(ClosureBinding);

impl DispositionCandidate {
    pub(crate) fn for_model(binding: ClosureBinding) -> Self {
        Self(binding)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Evidence<T> {
    Absent,
    Observed(T),
    Invalid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transients {
    Absent,
    Present,
    UnknownOrUnsafe,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveOutput {
    Exact,
    Changed,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveRecovery {
    AuthenticatedOutputMatch,
    NotSupplied,
    MismatchOrInvalid,
}

pub(crate) struct Facts {
    pub closure: Evidence<ClosureBinding>,
    pub disposition: Evidence<DispositionCandidate>,
    /// Includes H/R/next/stage/journal/slots/routing pending. Missing evidence
    /// must mean observed absence, never an unreadable or unchecked path.
    pub transients: Transients,
    pub current_owner_generation: Option<u64>,
    pub live: LiveOutput,
    pub archive: ArchiveRecovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Review {
    /// Not proof of fresh installation, successful recovery or safe startup.
    NoClosureEvidence,
    UnresolvedClosure,
    TransientFence,
    AwaitAuthenticatedRecovery,
    /// Rollover and legitimate later edits require a separate policy. They
    /// cannot silently turn an old closure into current live-pair authority.
    PolicyUnresolved,
    /// Output-only hypothetical history; still no mutation/startup authority.
    HistoricalCandidateStillFenced,
    ManualRecovery,
}

pub(crate) fn review(facts: Facts) -> Review {
    use Evidence::{Absent, Invalid, Observed};
    if matches!(facts.closure, Invalid)
        || matches!(facts.disposition, Invalid)
        || facts.transients == Transients::UnknownOrUnsafe
        || facts.archive == ArchiveRecovery::MismatchOrInvalid
    {
        return Review::ManualRecovery;
    }
    // A hypothetical disposition never masks any unresolved private artifact.
    if facts.transients == Transients::Present {
        return Review::TransientFence;
    }
    let binding = match (facts.closure, facts.disposition) {
        (Absent, Absent) => return Review::NoClosureEvidence,
        (Observed(_), Absent) => return Review::UnresolvedClosure,
        (Observed(closure), Observed(disposition)) if closure == disposition.0 => closure,
        _ => return Review::ManualRecovery,
    };
    if facts.current_owner_generation != Some(binding.owner_generation)
        || facts.live != LiveOutput::Exact
    {
        return Review::PolicyUnresolved;
    }
    match facts.archive {
        ArchiveRecovery::NotSupplied => Review::AwaitAuthenticatedRecovery,
        ArchiveRecovery::AuthenticatedOutputMatch => Review::HistoricalCandidateStillFenced,
        ArchiveRecovery::MismatchOrInvalid => Review::ManualRecovery,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_decision_candidate::{DecisionRecord, TerminalChoice};
    use crate::restore_retirement_candidate::RetirementReceipt;
    use crate::restore_staging_candidate::planned_stage_identity;

    pub(super) fn binding(generation: u64, id: u8, commit: bool, output: &[u8]) -> ClosureBinding {
        let stage =
            planned_stage_identity([b"old", b"old-template", b"new", b"new-template"]).unwrap();
        let terminal = DecisionRecord::intent(generation, None, &stage, [id; 16])
            .unwrap()
            .terminal(if commit {
                TerminalChoice::Commit
            } else {
                TerminalChoice::Abort
            })
            .unwrap();
        let receipt = RetirementReceipt::synthetic(&terminal, output, b"synthetic-template");
        let closure = ClosureRecord::from_verified_receipt(&receipt).unwrap();
        assert!(ClosureBinding::from_record(&closure, generation + 1, None).is_none());
        assert!(ClosureBinding::from_record(&closure, generation, Some(b"invalid")).is_none());
        ClosureBinding::from_record(&closure, generation, None).unwrap()
    }

    fn facts(binding: ClosureBinding) -> Facts {
        Facts {
            closure: Evidence::Observed(binding),
            disposition: Evidence::Observed(DispositionCandidate::for_model(binding)),
            transients: Transients::Absent,
            current_owner_generation: Some(binding.owner_generation),
            live: LiveOutput::Exact,
            archive: ArchiveRecovery::AuthenticatedOutputMatch,
        }
    }

    #[test]
    fn disposition_model_exact_commit_abort_are_only_still_fenced_candidates() {
        for commit in [false, true] {
            let binding = binding(7, 1, commit, b"same-output");
            assert_eq!(
                review(facts(binding)),
                Review::HistoricalCandidateStillFenced
            );
            let mut missing = facts(binding);
            missing.disposition = Evidence::Absent;
            assert_eq!(review(missing), Review::UnresolvedClosure);
            let mut no_evidence = facts(binding);
            no_evidence.closure = Evidence::Absent;
            no_evidence.disposition = Evidence::Absent;
            assert_eq!(review(no_evidence), Review::NoClosureEvidence);
        }
    }

    #[test]
    fn disposition_model_crossed_outcome_transaction_generation_or_output_refuses() {
        let original = binding(7, 1, true, b"same-output");
        for wrong in [
            binding(7, 1, false, b"same-output"),
            binding(7, 2, true, b"same-output"),
            binding(8, 1, true, b"same-output"),
            binding(7, 1, true, b"other-output"),
        ] {
            let mut crossed = facts(original);
            crossed.disposition = Evidence::Observed(DispositionCandidate::for_model(wrong));
            assert_eq!(review(crossed), Review::ManualRecovery);
        }
        let mut orphan = facts(original);
        orphan.closure = Evidence::Absent;
        assert_eq!(review(orphan), Review::ManualRecovery);
    }

    #[test]
    fn disposition_model_all_transient_and_invalid_evidence_overrides_history() {
        let binding = binding(7, 1, true, b"output");
        for closure in [
            Evidence::Absent,
            Evidence::Observed(binding),
            Evidence::Invalid,
        ] {
            for disposition in [
                Evidence::Absent,
                Evidence::Observed(DispositionCandidate::for_model(binding)),
                Evidence::Invalid,
            ] {
                for transients in [Transients::Present, Transients::UnknownOrUnsafe] {
                    let invalid = matches!(closure, Evidence::Invalid)
                        || matches!(disposition, Evidence::Invalid)
                        || transients == Transients::UnknownOrUnsafe;
                    let mut f = facts(binding);
                    f.closure = closure;
                    f.disposition = disposition;
                    f.transients = transients;
                    assert_eq!(
                        review(f),
                        if invalid {
                            Review::ManualRecovery
                        } else {
                            Review::TransientFence
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn disposition_model_invalid_records_refuse_even_without_transients() {
        let binding = binding(7, 1, true, b"output");
        for bad_closure in [false, true] {
            let mut f = facts(binding);
            if bad_closure {
                f.closure = Evidence::Invalid;
            } else {
                f.disposition = Evidence::Invalid;
            }
            assert_eq!(review(f), Review::ManualRecovery);
        }
    }

    #[test]
    fn disposition_model_rollover_and_later_edits_never_silently_admit() {
        let binding = binding(7, 1, true, b"output");
        for owner in [None, Some(6), Some(7), Some(8)] {
            for live in [LiveOutput::Exact, LiveOutput::Changed, LiveOutput::Unknown] {
                let mut f = facts(binding);
                f.current_owner_generation = owner;
                f.live = live;
                assert_eq!(
                    review(f),
                    if owner == Some(7) && live == LiveOutput::Exact {
                        Review::HistoricalCandidateStillFenced
                    } else {
                        Review::PolicyUnresolved
                    }
                );
            }
        }
    }

    #[test]
    fn disposition_model_archive_resupply_is_not_replaced_by_hypothetical_history() {
        for commit in [false, true] {
            let binding = binding(7, 1, commit, b"output");
            let mut missing = facts(binding);
            missing.archive = ArchiveRecovery::NotSupplied;
            assert_eq!(review(missing), Review::AwaitAuthenticatedRecovery);
            let mut wrong = facts(binding);
            wrong.archive = ArchiveRecovery::MismatchOrInvalid;
            assert_eq!(review(wrong), Review::ManualRecovery);
        }
    }
}
