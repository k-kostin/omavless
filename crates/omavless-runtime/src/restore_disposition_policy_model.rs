// SPDX-License-Identifier: MIT
//! Counterfactual two-phase policy for owner review, NOT approved admission.
//! No persistence format, filesystem observation, writer or startup exception.
//! Every result remains fenced; facts supplied here prove nothing by themselves.
use super::{ArchiveRecovery, ClosureBinding, Evidence, Transients};

// Simulation labels only, not an on-disk version/domain decision.
const MODEL_SCHEMA: u16 = 1;
const MODEL_DOMAIN: &str = "terminal-output-history-policy";

#[derive(Clone, Copy)]
pub(crate) struct HistoricalBinding {
    closure: ClosureBinding,
    uid: u32,
    schema: u16,
    domain: &'static str,
}
impl HistoricalBinding {
    pub(crate) fn for_model(closure: ClosureBinding, uid: u32) -> Self {
        Self {
            closure,
            uid,
            schema: MODEL_SCHEMA,
            domain: MODEL_DOMAIN,
        }
    }
    fn matches(&self, closure: ClosureBinding, uid: u32) -> bool {
        self.closure == closure
            && self.uid == uid
            && self.schema == MODEL_SCHEMA
            && self.domain == MODEL_DOMAIN
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Disposition {
    Absent,
    /// Complete visible bytes do NOT assert durability after process death.
    Visible(HistoricalBinding),
    /// Hypothetical input; no code in this module establishes this fact.
    DurableRechecked(HistoricalBinding),
    Invalid,
}

#[derive(Clone, Copy)]
pub(crate) struct BeforeDisposition {
    /// Fresh authentication of payload matching staged NEW, not identity of
    /// one ciphertext file. OLD==NEW permits the same authenticated payload.
    pub new_payload: ArchiveRecovery,
    pub exact_terminal_output: bool,
    pub exact_owner_desired_sources: bool,
    pub off: bool,
    pub empty_owned_host: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Facts {
    pub closure: Evidence<ClosureBinding>,
    pub disposition: Disposition,
    pub transients: Transients,
    pub expected_uid: u32,
    pub observed_uid: Option<u32>,
    pub current_generation: Option<u64>,
    pub uninterrupted_lease_and_pins: bool,
    pub before: BeforeDisposition,
    /// Later live schema/revision/ownership is independently validated by a
    /// future normal owner, NOT compared forever against archived output.
    pub ordinary_live_validation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Review {
    TransientFence,
    AwaitFreshAuthenticationStillFenced,
    BeforeDispositionCandidateStillFenced,
    VisibleDispositionNeedsResyncStillFenced,
    /// Proposed policy only; owner approval and typed call-site integration
    /// are absent. Never use this as a portable or mutation capability.
    CompletedDispositionHistoricalCandidateStillFenced,
    ManualRecovery,
}

pub(crate) fn review_proposed_policy(f: Facts) -> Review {
    if matches!(f.closure, Evidence::Invalid)
        || matches!(f.disposition, Disposition::Invalid)
        || f.transients == Transients::UnknownOrUnsafe
    {
        return Review::ManualRecovery;
    }
    if f.transients == Transients::Present {
        return Review::TransientFence;
    }
    let Evidence::Observed(closure) = f.closure else {
        return Review::ManualRecovery;
    };
    if f.observed_uid != Some(f.expected_uid)
        || f.current_generation != Some(closure.owner_generation)
        || !f.uninterrupted_lease_and_pins
    {
        return Review::ManualRecovery;
    }
    if let Disposition::Visible(binding) | Disposition::DurableRechecked(binding) = f.disposition
        && !binding.matches(closure, f.expected_uid)
    {
        return Review::ManualRecovery;
    }
    if matches!(f.disposition, Disposition::DurableRechecked(_)) {
        // Completed history has no archive operation. Refuse contradictory
        // supplied authentication facts rather than silently ignoring them.
        if f.before.new_payload != ArchiveRecovery::NotSupplied {
            return Review::ManualRecovery;
        }
        return if f.ordinary_live_validation {
            Review::CompletedDispositionHistoricalCandidateStillFenced
        } else {
            Review::ManualRecovery
        };
    }
    // An absent ticket after later edits cannot authorize archive replay. Even
    // a valid archive does not repair a divergent pair through this model.
    if !f.before.exact_terminal_output
        || !f.before.exact_owner_desired_sources
        || !f.before.off
        || !f.before.empty_owned_host
    {
        return Review::ManualRecovery;
    }
    match f.before.new_payload {
        ArchiveRecovery::MismatchOrInvalid => Review::ManualRecovery,
        ArchiveRecovery::NotSupplied => Review::AwaitFreshAuthenticationStillFenced,
        ArchiveRecovery::AuthenticatedOutputMatch => {
            if matches!(f.disposition, Disposition::Visible(_)) {
                Review::VisibleDispositionNeedsResyncStillFenced
            } else {
                Review::BeforeDispositionCandidateStillFenced
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::binding;
    use super::*;

    fn facts(commit: bool) -> Facts {
        Facts {
            closure: Evidence::Observed(binding(7, 1, commit, b"same-output")),
            disposition: Disposition::Absent,
            transients: Transients::Absent,
            expected_uid: 1000,
            observed_uid: Some(1000),
            current_generation: Some(7),
            uninterrupted_lease_and_pins: true,
            before: BeforeDisposition {
                new_payload: ArchiveRecovery::AuthenticatedOutputMatch,
                exact_terminal_output: true,
                exact_owner_desired_sources: true,
                off: true,
                empty_owned_host: true,
            },
            ordinary_live_validation: true,
        }
    }
    fn ticket(f: &Facts) -> HistoricalBinding {
        let Evidence::Observed(c) = f.closure else {
            panic!("synthetic closure required")
        };
        HistoricalBinding::for_model(c, f.expected_uid)
    }
    #[test]
    fn disposition_policy_before_requires_each_authenticated_off_and_lease_gate() {
        for commit in [false, true] {
            assert_eq!(
                review_proposed_policy(facts(commit)),
                Review::BeforeDispositionCandidateStillFenced
            );
            for gate in 0..6 {
                let mut f = facts(commit);
                match gate {
                    0 => f.uninterrupted_lease_and_pins = false,
                    1 => f.before.exact_terminal_output = false,
                    2 => f.before.exact_owner_desired_sources = false,
                    3 => f.before.off = false,
                    4 => f.before.empty_owned_host = false,
                    _ => f.before.new_payload = ArchiveRecovery::MismatchOrInvalid,
                }
                assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
            }
            let mut missing = facts(commit);
            missing.before.new_payload = ArchiveRecovery::NotSupplied;
            assert_eq!(
                review_proposed_policy(missing),
                Review::AwaitFreshAuthenticationStillFenced
            );
        }
    }
    #[test]
    fn disposition_policy_visible_after_crash_never_means_durable_history() {
        for commit in [false, true] {
            let mut f = facts(commit);
            f.disposition = Disposition::Visible(ticket(&f));
            assert_eq!(
                review_proposed_policy(f),
                Review::VisibleDispositionNeedsResyncStillFenced
            );
            f.before.new_payload = ArchiveRecovery::NotSupplied;
            assert_eq!(
                review_proposed_policy(f),
                Review::AwaitFreshAuthenticationStillFenced
            );
            f.before.exact_terminal_output = false;
            assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        }
    }
    #[test]
    fn disposition_policy_historical_output_does_not_replace_ordinary_live_validation() {
        for commit in [false, true] {
            let mut f = facts(commit);
            f.disposition = Disposition::DurableRechecked(ticket(&f));
            // Model legitimate later edits/Connect: no perpetual archived-byte
            // or Off constraint, but existing native validation remains required.
            f.before = BeforeDisposition {
                new_payload: ArchiveRecovery::NotSupplied,
                exact_terminal_output: false,
                exact_owner_desired_sources: false,
                off: false,
                empty_owned_host: false,
            };
            assert_eq!(
                review_proposed_policy(f),
                Review::CompletedDispositionHistoricalCandidateStillFenced
            );
            for supplied in [
                ArchiveRecovery::AuthenticatedOutputMatch,
                ArchiveRecovery::MismatchOrInvalid,
            ] {
                let mut crossed = f;
                crossed.before.new_payload = supplied;
                assert_eq!(review_proposed_policy(crossed), Review::ManualRecovery);
            }
            let mut stale_lease = f;
            stale_lease.uninterrupted_lease_and_pins = false;
            assert_eq!(review_proposed_policy(stale_lease), Review::ManualRecovery);
            f.ordinary_live_validation = false;
            assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
            f.ordinary_live_validation = true;
            f.disposition = Disposition::Absent;
            assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        }
    }
    #[test]
    fn disposition_policy_exact_uid_generation_schema_domain_and_c1_are_mandatory() {
        let f = facts(true);
        for kind in 0..7 {
            let mut t = ticket(&f);
            match kind {
                0 => t.uid += 1,
                1 => t.schema += 1,
                2 => t.domain = "other-policy",
                3 => t.closure = binding(8, 1, true, b"same-output"),
                4 => t.closure = binding(7, 2, true, b"same-output"),
                5 => t.closure = binding(7, 1, false, b"same-output"),
                _ => t.closure = binding(7, 1, true, b"different-output"),
            }
            for durable in [false, true] {
                let mut crossed = f;
                crossed.disposition = if durable {
                    Disposition::DurableRechecked(t)
                } else {
                    Disposition::Visible(t)
                };
                assert_eq!(review_proposed_policy(crossed), Review::ManualRecovery);
            }
        }
        for current in [None, Some(6), Some(8)] {
            let mut changed = f;
            changed.disposition = Disposition::DurableRechecked(ticket(&f));
            changed.before.new_payload = ArchiveRecovery::NotSupplied;
            changed.current_generation = current;
            assert_eq!(review_proposed_policy(changed), Review::ManualRecovery);
        }
        for uid in [None, Some(999), Some(1001)] {
            let mut changed = f;
            changed.disposition = Disposition::DurableRechecked(ticket(&f));
            changed.before.new_payload = ArchiveRecovery::NotSupplied;
            changed.observed_uid = uid;
            assert_eq!(review_proposed_policy(changed), Review::ManualRecovery);
        }
    }
    #[test]
    fn disposition_policy_transients_override_and_old_ticket_never_authorizes_successor() {
        for disposition in [
            Disposition::Absent,
            Disposition::Visible(ticket(&facts(true))),
            Disposition::DurableRechecked(ticket(&facts(true))),
        ] {
            let mut f = facts(true);
            f.disposition = disposition;
            f.transients = Transients::Present;
            assert_eq!(review_proposed_policy(f), Review::TransientFence);
            f.transients = Transients::UnknownOrUnsafe;
            assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        }
        let mut f = facts(true);
        f.disposition = Disposition::DurableRechecked(ticket(&f));
        f.closure = Evidence::Observed(binding(7, 2, true, b"successor-output"));
        assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        f.closure = Evidence::Absent;
        assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        f.closure = Evidence::Invalid;
        assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        f = facts(true);
        f.disposition = Disposition::Invalid;
        assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
    }
    #[test]
    fn disposition_policy_static_caller_matrix_keeps_existing_presence_fences() {
        // Static retention guard ONLY, not behavioral coverage of future typed
        // integration. Every caller still uses the conservative old predicate.
        for (name, source) in [
            ("login", include_str!("login_transaction.rs")),
            ("connection", include_str!("connection_transaction.rs")),
            ("coordinator", include_str!("native_coordinator.rs")),
            ("batch", include_str!("native_coordinator/batch.rs")),
            ("backup", include_str!("backup_source_candidate.rs")),
            ("cutover", include_str!("production_cutover.rs")),
            ("owner-review", include_str!("production_owner.rs")),
        ] {
            assert!(
                source.contains("pending_private_transaction::pending"),
                "{name}"
            );
            assert!(!source.contains("restore_disposition_model"), "{name}");
        }
        // Ordinary restore admission delegates through the retained-stage helper.
        // Protect the complete None -> conservative predicate chain, not merely
        // the presence of that predicate somewhere in the helper's source.
        let restore = include_str!("native_coordinator/restore_candidate.rs");
        let execution = include_str!("native_coordinator/restore_first_execution.rs");
        for source in [restore, execution] {
            assert!(!source.contains("restore_disposition_model"));
        }
        assert!(static_ordinary_presence_chain(restore, execution));
    }

    // Source retention only, not a parser or behavioral admission proof. The
    // common readiness helper now receives the typed writer's predicate; its
    // ordinary None path must still reach the conservative existence fence.
    fn static_ordinary_presence_chain(restore: &str, execution: &str) -> bool {
        let compact = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let restore = compact(restore);
        let execution = compact(execution);
        let Some(ordinary) = restore.split("fnrestore_readiness_locked(").nth(1) else {
            return false;
        };
        let ordinary = ordinary
            .split("fnrestore_readiness_with_created_stage(")
            .next()
            .unwrap();
        if !ordinary.contains("self.restore_readiness_with_created_stage(lock,None)") {
            return false;
        }
        let Some(created) = restore
            .split("fnrestore_readiness_with_created_stage(")
            .nth(1)
        else {
            return false;
        };
        let created = created
            .split("fnrestore_readiness_with_native_stage(")
            .next()
            .unwrap();
        if !created.contains("self.restore_readiness_with_pending_check(lock,created.is_some(),|paths,uid|{first_execution::pending_allowed(paths,uid,created)})") { return false; }
        let Some(readiness) = restore
            .split("fnrestore_readiness_with_pending_check(")
            .nth(1)
        else {
            return false;
        };
        let Some(observation) = readiness.find(".fresh_observation(&desired)") else {
            return false;
        };
        let guard = "!pending_allowed(self.transaction.desired_paths(),self.uid())";
        if !readiness[..observation].contains(guard) || !readiness[observation..].contains(guard) {
            return false;
        }
        let Some(pending) = execution.split("fnpending_allowed(").nth(1) else {
            return false;
        };
        let pending = pending.split("structPinnedMember").next().unwrap();
        pending.contains("letSome(created)=createdelse{return!crate::pending_private_transaction::pending(paths);};")
    }

    #[test]
    fn disposition_policy_static_detector_rejects_each_missing_link_or_guard() {
        let restore: String = include_str!("native_coordinator/restore_candidate.rs")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let execution: String = include_str!("native_coordinator/restore_first_execution.rs")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(static_ordinary_presence_chain(&restore, &execution));
        for link in [
            "self.restore_readiness_with_created_stage(lock,None)",
            "first_execution::pending_allowed(paths,uid,created)",
        ] {
            assert!(restore.contains(link));
            assert!(!static_ordinary_presence_chain(
                &restore.replacen(link, "omitted", 1),
                &execution
            ));
        }
        let guard = "!pending_allowed(self.transaction.desired_paths(),self.uid())";
        assert_eq!(restore.matches(guard).count(), 2);
        assert!(!static_ordinary_presence_chain(
            &restore.replacen(guard, "omitted", 1),
            &execution
        ));
        let last = restore.rfind(guard).unwrap();
        let mut after = restore.clone();
        after.replace_range(last..last + guard.len(), "omitted");
        assert!(!static_ordinary_presence_chain(&after, &execution));
        let fallback = "letSome(created)=createdelse{return!crate::pending_private_transaction::pending(paths);};";
        assert!(execution.contains(fallback));
        assert!(!static_ordinary_presence_chain(
            &restore,
            &execution.replacen(fallback, "omitted", 1)
        ));
    }

    #[test]
    fn disposition_policy_identical_old_new_payload_still_binds_terminal_outcome() {
        use crate::restore_closure_model::ClosureRecord;
        use crate::restore_decision_candidate::{DecisionRecord, TerminalChoice};
        use crate::restore_retirement_candidate::RetirementReceipt;
        use crate::restore_staging_candidate::planned_stage_identity;
        let stage = planned_stage_identity([b"same", b"template", b"same", b"template"]).unwrap();
        let intent = DecisionRecord::intent(7, None, &stage, [1; 16]).unwrap();
        let bindings = [TerminalChoice::Commit, TerminalChoice::Abort].map(|outcome| {
            let terminal = intent.terminal(outcome).unwrap();
            let receipt = RetirementReceipt::synthetic(&terminal, b"same", b"template");
            let closure = ClosureRecord::from_verified_receipt(&receipt).unwrap();
            ClosureBinding::from_record(&closure, 7, None).unwrap()
        });
        for index in 0..2 {
            let mut f = facts(index == 0);
            f.closure = Evidence::Observed(bindings[index]);
            // Authentication remains an external fact: this tests identical
            // payload identity, not envelope/ciphertext provenance.
            assert_eq!(
                review_proposed_policy(f),
                Review::BeforeDispositionCandidateStillFenced
            );
            f.disposition =
                Disposition::Visible(HistoricalBinding::for_model(bindings[1 - index], 1000));
            assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
            f.disposition = Disposition::DurableRechecked(HistoricalBinding::for_model(
                bindings[1 - index],
                1000,
            ));
            f.before.new_payload = ArchiveRecovery::NotSupplied;
            assert_eq!(review_proposed_policy(f), Review::ManualRecovery);
        }
    }
}
