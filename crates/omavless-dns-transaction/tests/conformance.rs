// SPDX-License-Identifier: MIT

use omavless_dns_transaction::*;

fn prerequisites() -> Prerequisites {
    Prerequisites {
        enrolled_peer: true,
        exclusive_dns_owner: true,
        kernel_bound_lease: true,
        compatible_policy: true,
    }
}

fn transaction() -> Transaction {
    Transaction::new(1, 1, prerequisites()).unwrap()
}

fn matched() -> Completion {
    Completion::PolicyReadback(PolicyReadback {
        servers_match: true,
        domains_match: true,
        default_route_matches: true,
    })
}

fn success(action: Action) -> Completion {
    match action {
        Action::CaptureSnapshot => Completion::SnapshotCaptured,
        Action::VerifyFixedPolicy => matched(),
        Action::VerifyCapturedSnapshot => Completion::SnapshotReadback { matches: true },
        _ => Completion::WriteSettled,
    }
}

fn step(tx: &mut Transaction) -> Ticket {
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    tx.complete(ticket, success(ticket.action()), LeaseCheck::Same)
        .unwrap();
    ticket
}

fn reach(tx: &mut Transaction, phase: Phase) {
    for _ in 0..8 {
        if tx.phase() == phase {
            return;
        }
        step(tx);
    }
    panic!("phase not reached");
}

#[test]
fn all_missing_prerequisite_combinations_refuse() {
    for bits in 0..15 {
        let facts = Prerequisites {
            enrolled_peer: bits & 1 != 0,
            exclusive_dns_owner: bits & 2 != 0,
            kernel_bound_lease: bits & 4 != 0,
            compatible_policy: bits & 8 != 0,
        };
        assert!(matches!(
            Transaction::new(1, 1, facts),
            Err(ModelError::PrerequisitesMissing)
        ));
    }
}

#[test]
fn zero_epoch_or_transaction_refuses() {
    for (epoch, id) in [(0, 0), (0, 1), (1, 0)] {
        assert!(matches!(
            Transaction::new(epoch, id, prerequisites()),
            Err(ModelError::InvalidIdentity)
        ));
    }
}

#[test]
fn max_identity_is_legal_without_increment_wrap() {
    let mut tx = Transaction::new(u64::MAX, u64::MAX, prerequisites()).unwrap();
    reach(&mut tx, Phase::AppliedVerified);
    tx.release();
    reach(&mut tx, Phase::Released);
}

#[test]
fn apply_has_exact_order_and_requires_readback() {
    let mut tx = transaction();
    for action in [
        Action::CaptureSnapshot,
        Action::SetFixedServers,
        Action::SetRootRoutingDomain,
        Action::SetDefaultRoute,
        Action::VerifyFixedPolicy,
    ] {
        assert_ne!(tx.phase(), Phase::AppliedVerified);
        assert_eq!(step(&mut tx).action(), action);
    }
    assert_eq!(tx.phase(), Phase::AppliedVerified);
    assert!(tx.next_action(LeaseCheck::Same).is_none());
}

#[test]
fn partial_or_absent_readback_never_claims_applied() {
    for bits in 0..7 {
        let mut tx = transaction();
        reach(&mut tx, Phase::VerifyApplied);
        let ticket = tx.next_action(LeaseCheck::Same).unwrap();
        tx.complete(
            ticket,
            Completion::PolicyReadback(PolicyReadback {
                servers_match: bits & 1 != 0,
                domains_match: bits & 2 != 0,
                default_route_matches: bits & 4 != 0,
            }),
            LeaseCheck::Same,
        )
        .unwrap();
        assert_eq!(tx.phase(), Phase::Restore);
        reach(&mut tx, Phase::FailedRestored);
    }
}

#[test]
fn capture_failure_has_no_compensation() {
    let mut tx = transaction();
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    tx.complete(ticket, Completion::SettledFailure, LeaseCheck::Same)
        .unwrap();
    assert_eq!(tx.phase(), Phase::Refused);
    assert!(tx.next_action(LeaseCheck::Same).is_none());
}

#[test]
fn every_settled_apply_failure_requires_verified_restoration() {
    for phase in [
        Phase::SetServers,
        Phase::SetDomains,
        Phase::SetDefaultRoute,
        Phase::VerifyApplied,
    ] {
        let mut tx = transaction();
        reach(&mut tx, phase);
        let ticket = tx.next_action(LeaseCheck::Same).unwrap();
        tx.complete(ticket, Completion::SettledFailure, LeaseCheck::Same)
            .unwrap();
        assert_eq!(tx.phase(), Phase::Restore);
        assert_eq!(step(&mut tx).action(), Action::RestoreCapturedSnapshot);
        assert_eq!(tx.phase(), Phase::VerifyRestored);
        step(&mut tx);
        assert_eq!(tx.phase(), Phase::FailedRestored);
    }
}

#[test]
fn restore_error_is_manual_recovery_not_retry_loop() {
    let mut tx = transaction();
    reach(&mut tx, Phase::AppliedVerified);
    tx.release();
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    tx.complete(ticket, Completion::SettledFailure, LeaseCheck::Same)
        .unwrap();
    assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
    for _ in 0..100 {
        assert!(tx.next_action(LeaseCheck::Same).is_none());
    }
}

#[test]
fn restore_readback_error_or_mismatch_is_manual_recovery() {
    for result in [
        Completion::SettledFailure,
        Completion::SnapshotReadback { matches: false },
        Completion::OutcomeUnknown,
    ] {
        let mut tx = transaction();
        reach(&mut tx, Phase::AppliedVerified);
        tx.release();
        step(&mut tx);
        let ticket = tx.next_action(LeaseCheck::Same).unwrap();
        tx.complete(ticket, result, LeaseCheck::Same).unwrap();
        assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
    }
}

#[test]
fn explicit_release_restores_before_reporting_released() {
    let mut tx = transaction();
    reach(&mut tx, Phase::AppliedVerified);
    tx.release();
    assert_eq!(step(&mut tx).action(), Action::RestoreCapturedSnapshot);
    assert_ne!(tx.phase(), Phase::Released);
    assert_eq!(step(&mut tx).action(), Action::VerifyCapturedSnapshot);
    assert_eq!(tx.phase(), Phase::Released);
}

#[test]
fn cancel_before_dispatch_has_no_effects() {
    let mut tx = transaction();
    tx.cancel();
    assert_eq!(tx.phase(), Phase::Refused);
    assert!(tx.next_action(LeaseCheck::Same).is_none());
}

#[test]
fn cancel_during_capture_waits_then_refuses_without_writes() {
    let mut tx = transaction();
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    tx.cancel();
    assert!(tx.next_action(LeaseCheck::Same).is_none());
    tx.complete(ticket, Completion::SnapshotCaptured, LeaseCheck::Same)
        .unwrap();
    assert_eq!(tx.phase(), Phase::Refused);
}

#[test]
fn cancel_after_capture_does_not_write_or_restore() {
    let mut tx = transaction();
    step(&mut tx);
    tx.cancel();
    assert_eq!(tx.phase(), Phase::Refused);
}

#[test]
fn cancel_at_every_inflight_apply_stage_joins_late_success() {
    for phase in [
        Phase::SetServers,
        Phase::SetDomains,
        Phase::SetDefaultRoute,
        Phase::VerifyApplied,
    ] {
        let mut tx = transaction();
        reach(&mut tx, phase);
        let ticket = tx.next_action(LeaseCheck::Same).unwrap();
        tx.cancel();
        // Human authorization may take arbitrarily many polling intervals.
        for _ in 0..1000 {
            assert!(tx.next_action(LeaseCheck::Same).is_none());
        }
        assert_ne!(tx.phase(), Phase::AppliedVerified);
        tx.complete(ticket, success(ticket.action()), LeaseCheck::Same)
            .unwrap();
        assert_eq!(tx.phase(), Phase::Restore);
        reach(&mut tx, Phase::FailedRestored);
    }
}

#[test]
fn delayed_authorization_without_cancellation_can_finish() {
    let mut tx = transaction();
    step(&mut tx);
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    for _ in 0..1000 {
        assert!(tx.next_action(LeaseCheck::Same).is_none());
    }
    tx.complete(ticket, Completion::WriteSettled, LeaseCheck::Same)
        .unwrap();
    reach(&mut tx, Phase::AppliedVerified);
}

#[test]
fn cancel_between_writes_restores_without_next_apply() {
    for phase in [
        Phase::SetDomains,
        Phase::SetDefaultRoute,
        Phase::VerifyApplied,
    ] {
        let mut tx = transaction();
        reach(&mut tx, phase);
        tx.cancel();
        assert_eq!(
            tx.next_action(LeaseCheck::Same).unwrap().action(),
            Action::RestoreCapturedSnapshot
        );
    }
}

#[test]
fn repeated_cancel_during_compensation_cannot_interrupt_cleanup() {
    let mut tx = transaction();
    reach(&mut tx, Phase::SetDomains);
    tx.cancel();
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    for _ in 0..10 {
        tx.cancel();
        tx.release();
    }
    assert!(tx.next_action(LeaseCheck::Same).is_none());
    tx.complete(ticket, Completion::WriteSettled, LeaseCheck::Same)
        .unwrap();
    tx.cancel();
    step(&mut tx);
    assert_eq!(tx.phase(), Phase::FailedRestored);
}

#[test]
fn cancelling_completed_apply_does_not_implicitly_disconnect() {
    let mut tx = transaction();
    reach(&mut tx, Phase::AppliedVerified);
    tx.cancel();
    assert_eq!(tx.phase(), Phase::AppliedVerified);
    // An explicit Release, not client/TUI exit, is needed.
    tx.release();
    assert_eq!(tx.phase(), Phase::Restore);
}

#[test]
fn unknown_write_outcome_never_dispatches_competing_restore() {
    for phase in [
        Phase::SetServers,
        Phase::SetDomains,
        Phase::SetDefaultRoute,
        Phase::VerifyApplied,
    ] {
        let mut tx = transaction();
        reach(&mut tx, phase);
        let ticket = tx.next_action(LeaseCheck::Same).unwrap();
        tx.complete(ticket, Completion::OutcomeUnknown, LeaseCheck::Same)
            .unwrap();
        assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
        assert!(tx.next_action(LeaseCheck::Same).is_none());
        assert_eq!(
            tx.complete(ticket, success(ticket.action()), LeaseCheck::Same),
            Err(ModelError::StaleCompletion)
        );
    }
}

#[test]
fn unknown_capture_outcome_refuses_without_writes() {
    let mut tx = transaction();
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    tx.complete(ticket, Completion::OutcomeUnknown, LeaseCheck::Same)
        .unwrap();
    assert_eq!(tx.phase(), Phase::Refused);
}

#[test]
fn lease_loss_or_unavailability_before_effect_refuses() {
    for lease in [LeaseCheck::Lost, LeaseCheck::Unavailable] {
        let mut tx = transaction();
        assert!(tx.next_action(lease).is_none());
        assert_eq!(tx.phase(), Phase::Refused);
    }
}

#[test]
fn lease_loss_after_each_write_blocks_cleanup_on_reused_interface() {
    for phase in [
        Phase::SetServers,
        Phase::SetDomains,
        Phase::SetDefaultRoute,
        Phase::VerifyApplied,
    ] {
        for lease in [LeaseCheck::Lost, LeaseCheck::Unavailable] {
            let mut tx = transaction();
            reach(&mut tx, phase);
            let ticket = tx.next_action(LeaseCheck::Same).unwrap();
            tx.complete(ticket, success(ticket.action()), lease)
                .unwrap();
            assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
            assert!(tx.next_action(LeaseCheck::Same).is_none());
        }
    }
}

#[test]
fn lease_is_rechecked_before_restore_and_after_restore() {
    for after in [false, true] {
        let mut tx = transaction();
        reach(&mut tx, Phase::AppliedVerified);
        tx.release();
        if after {
            let ticket = tx.next_action(LeaseCheck::Same).unwrap();
            tx.complete(ticket, Completion::WriteSettled, LeaseCheck::Lost)
                .unwrap();
        } else {
            assert!(tx.next_action(LeaseCheck::Lost).is_none());
        }
        assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
    }
}

#[test]
fn restart_invalidates_ready_and_inflight_transaction() {
    for phase in [Phase::SetDomains, Phase::AppliedVerified] {
        let mut tx = transaction();
        reach(&mut tx, phase);
        tx.invalidate();
        assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
        assert!(tx.next_action(LeaseCheck::Same).is_none());
    }
}

#[test]
fn repeated_dispatch_never_issues_same_effect_twice() {
    let mut tx = transaction();
    for _ in 0..5 {
        let ticket = tx.next_action(LeaseCheck::Same).unwrap();
        for _ in 0..100 {
            assert!(tx.next_action(LeaseCheck::Same).is_none());
        }
        tx.complete(ticket, success(ticket.action()), LeaseCheck::Same)
            .unwrap();
    }
    assert_eq!(tx.phase(), Phase::AppliedVerified);
}

#[test]
fn duplicate_completion_is_rejected() {
    let mut tx = transaction();
    let ticket = step(&mut tx);
    assert_eq!(
        tx.complete(ticket, Completion::SnapshotCaptured, LeaseCheck::Same),
        Err(ModelError::StaleCompletion)
    );
    assert_eq!(tx.phase(), Phase::SetServers);
}

#[test]
fn out_of_order_completion_does_not_retire_current_action() {
    let mut tx = transaction();
    let old = step(&mut tx);
    let current = tx.next_action(LeaseCheck::Same).unwrap();
    assert_eq!(
        tx.complete(old, Completion::SnapshotCaptured, LeaseCheck::Same),
        Err(ModelError::StaleCompletion)
    );
    tx.complete(current, Completion::WriteSettled, LeaseCheck::Same)
        .unwrap();
    assert_eq!(tx.phase(), Phase::SetDomains);
}

#[test]
fn stale_epoch_and_other_transaction_cannot_finish_current_action() {
    for (epoch, id) in [(2, 1), (1, 2)] {
        let mut tx = transaction();
        let current = tx.next_action(LeaseCheck::Same).unwrap();
        let mut other = Transaction::new(epoch, id, prerequisites()).unwrap();
        let foreign = other.next_action(LeaseCheck::Same).unwrap();
        assert_eq!(
            tx.complete(foreign, Completion::SnapshotCaptured, LeaseCheck::Same),
            Err(ModelError::StaleCompletion)
        );
        tx.complete(current, Completion::SnapshotCaptured, LeaseCheck::Same)
            .unwrap();
    }
}

#[test]
fn wrong_completion_shape_cannot_skip_stages() {
    let mut tx = transaction();
    let ticket = tx.next_action(LeaseCheck::Same).unwrap();
    for result in [
        Completion::WriteSettled,
        matched(),
        Completion::SnapshotReadback { matches: true },
    ] {
        assert_eq!(
            tx.complete(ticket, result, LeaseCheck::Same),
            Err(ModelError::WrongCompletion)
        );
        assert_eq!(tx.phase(), Phase::Capture);
    }
    tx.complete(ticket, Completion::SnapshotCaptured, LeaseCheck::Same)
        .unwrap();
}

#[test]
fn terminal_states_cannot_be_revived_by_cancel_release_or_lease_changes() {
    for terminal in [
        Phase::Released,
        Phase::FailedRestored,
        Phase::Refused,
        Phase::ManualRecoveryRequired,
    ] {
        let mut tx = transaction();
        match terminal {
            Phase::Released => {
                reach(&mut tx, Phase::AppliedVerified);
                tx.release();
                reach(&mut tx, terminal);
            }
            Phase::FailedRestored => {
                reach(&mut tx, Phase::SetDomains);
                tx.cancel();
                reach(&mut tx, terminal);
            }
            Phase::Refused => tx.cancel(),
            Phase::ManualRecoveryRequired => {
                reach(&mut tx, Phase::SetDomains);
                tx.invalidate();
            }
            _ => unreachable!(),
        }
        tx.cancel();
        tx.release();
        tx.invalidate();
        assert!(tx.next_action(LeaseCheck::Lost).is_none());
        assert_eq!(tx.phase(), terminal);
    }
}

#[test]
fn errors_and_tickets_never_format_host_or_correlation_material() {
    for error in [
        ModelError::InvalidIdentity,
        ModelError::PrerequisitesMissing,
        ModelError::StaleCompletion,
        ModelError::WrongCompletion,
    ] {
        let text = error.to_string();
        assert!(text.is_ascii() && text.len() < 100);
        assert!(!text.contains('/') && !text.contains('@'));
    }
    let mut tx = Transaction::new(912345678, 876543219, prerequisites()).unwrap();
    let text = format!("{:?}", tx.next_action(LeaseCheck::Same).unwrap());
    assert!(!text.contains("912345678") && !text.contains("876543219"));
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Settings {
    server: [u8; 4],
    domain: &'static str,
    default_route: bool,
}

/// In-memory resolved model; independent mutable fields and captured originals.
/// No bus connection, no system files, no real network or credentials.
struct FakeResolved {
    original: Settings,
    current: Settings,
    snapshot: Option<Settings>,
    unrelated: Settings,
}

impl FakeResolved {
    fn new() -> Self {
        let original = Settings {
            server: [192, 0, 2, 53],
            domain: "example.invalid",
            default_route: false,
        };
        Self {
            original: original.clone(),
            current: original.clone(),
            snapshot: None,
            unrelated: original,
        }
    }

    fn execute(&mut self, action: Action) -> Completion {
        match action {
            Action::CaptureSnapshot => {
                self.snapshot = Some(self.current.clone());
                Completion::SnapshotCaptured
            }
            Action::SetFixedServers => {
                self.current.server = [198, 18, 0, 2];
                Completion::WriteSettled
            }
            Action::SetRootRoutingDomain => {
                self.current.domain = "~.";
                Completion::WriteSettled
            }
            Action::SetDefaultRoute => {
                self.current.default_route = true;
                Completion::WriteSettled
            }
            Action::VerifyFixedPolicy => Completion::PolicyReadback(PolicyReadback {
                servers_match: self.current.server == [198, 18, 0, 2],
                domains_match: self.current.domain == "~.",
                default_route_matches: self.current.default_route,
            }),
            Action::RestoreCapturedSnapshot => {
                self.current = self.snapshot.clone().unwrap();
                Completion::WriteSettled
            }
            Action::VerifyCapturedSnapshot => Completion::SnapshotReadback {
                matches: Some(&self.current) == self.snapshot.as_ref(),
            },
        }
    }
}

#[test]
fn fake_host_apply_release_preserves_exact_original_and_unrelated_link() {
    let mut host = FakeResolved::new();
    let mut tx = transaction();
    while let Some(ticket) = tx.next_action(LeaseCheck::Same) {
        let result = host.execute(ticket.action());
        tx.complete(ticket, result, LeaseCheck::Same).unwrap();
    }
    assert_eq!(tx.phase(), Phase::AppliedVerified);
    assert_ne!(host.current, host.original);
    tx.release();
    while let Some(ticket) = tx.next_action(LeaseCheck::Same) {
        let result = host.execute(ticket.action());
        tx.complete(ticket, result, LeaseCheck::Same).unwrap();
    }
    assert_eq!(tx.phase(), Phase::Released);
    assert_eq!(host.current, host.original);
    assert_eq!(host.unrelated, host.original);
}

#[test]
fn fake_host_partial_failure_matrix_restores_exact_snapshot() {
    for failed_action in [
        Action::SetFixedServers,
        Action::SetRootRoutingDomain,
        Action::SetDefaultRoute,
    ] {
        for applied_before_failure in [false, true] {
            let mut host = FakeResolved::new();
            let mut tx = transaction();
            let mut failure_injected = false;
            while let Some(ticket) = tx.next_action(LeaseCheck::Same) {
                let result = if ticket.action() == failed_action && !failure_injected {
                    failure_injected = true;
                    if applied_before_failure {
                        host.execute(ticket.action());
                    }
                    Completion::SettledFailure
                } else {
                    host.execute(ticket.action())
                };
                tx.complete(ticket, result, LeaseCheck::Same).unwrap();
            }
            assert_eq!(tx.phase(), Phase::FailedRestored);
            assert_eq!(host.current, host.original);
            assert_eq!(host.unrelated, host.original);
        }
    }
}

#[test]
fn fake_host_late_cancelled_write_finishes_before_compensation() {
    let mut host = FakeResolved::new();
    let mut tx = transaction();
    let capture = tx.next_action(LeaseCheck::Same).unwrap();
    tx.complete(capture, host.execute(capture.action()), LeaseCheck::Same)
        .unwrap();
    let pending = tx.next_action(LeaseCheck::Same).unwrap();
    tx.cancel();
    assert!(tx.next_action(LeaseCheck::Same).is_none());
    // Successful OS reply arrives only now, after cancellation was requested.
    tx.complete(pending, host.execute(pending.action()), LeaseCheck::Same)
        .unwrap();
    while let Some(ticket) = tx.next_action(LeaseCheck::Same) {
        tx.complete(ticket, host.execute(ticket.action()), LeaseCheck::Same)
            .unwrap();
    }
    assert_eq!(tx.phase(), Phase::FailedRestored);
    assert_eq!(host.current, host.original);
}

#[test]
fn fake_host_corrupt_restore_is_not_success() {
    let mut host = FakeResolved::new();
    let mut tx = transaction();
    while let Some(ticket) = tx.next_action(LeaseCheck::Same) {
        tx.complete(ticket, host.execute(ticket.action()), LeaseCheck::Same)
            .unwrap();
    }
    tx.release();
    let restore = tx.next_action(LeaseCheck::Same).unwrap();
    // Lying transport acknowledgement without actual restoration.
    tx.complete(restore, Completion::WriteSettled, LeaseCheck::Same)
        .unwrap();
    let verify = tx.next_action(LeaseCheck::Same).unwrap();
    tx.complete(verify, host.execute(verify.action()), LeaseCheck::Same)
        .unwrap();
    assert_eq!(tx.phase(), Phase::ManualRecoveryRequired);
}
