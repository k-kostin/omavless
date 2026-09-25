use super::*;

#[derive(Default)]
struct Fake {
    actions: Vec<Action>,
    fail: Option<(Action, Completion)>,
    checks: usize,
    lose_at: Option<usize>,
    marks: usize,
    finishes: usize,
    quarantines: usize,
    fail_mark: bool,
    fail_finish: bool,
    policy_drift: bool,
    policy_checks: usize,
}
impl Effects for Fake {
    fn applied_policy_matches(&mut self) -> bool {
        self.policy_checks += 1;
        !self.policy_drift
    }
    fn recheck(&mut self) -> bool {
        self.checks += 1;
        self.lose_at != Some(self.checks)
    }
    fn perform(&mut self, action: Action) -> Completion {
        self.actions.push(action);
        if let Some((at, result)) = self.fail
            && at == action
        {
            return result;
        }
        match action {
            Action::CaptureSnapshot => Completion::SnapshotCaptured,
            Action::SetFixedServers
            | Action::SetRootRoutingDomain
            | Action::SetDefaultRoute
            | Action::RestoreCapturedSnapshot => Completion::WriteSettled,
            Action::VerifyFixedPolicy => Completion::PolicyReadback(PolicyReadback {
                servers_match: true,
                domains_match: true,
                default_route_matches: true,
            }),
            Action::VerifyCapturedSnapshot => Completion::SnapshotReadback { matches: true },
        }
    }
    fn mark_applied(&mut self) -> bool {
        self.marks += 1;
        !self.fail_mark
    }
    fn finish(&mut self) -> bool {
        self.finishes += 1;
        !self.fail_finish
    }
    fn quarantine(&mut self) {
        self.quarantines += 1;
    }
}

#[test]
fn post_ready_policy_drift_blocks_health_and_normal_release_without_reset() {
    for health_first in [false, true] {
        let mut driver = Driver::new(Fake::default());
        assert_eq!(driver.drive(), Outcome::Ready);
        assert!(driver.check_active());
        driver.effects.policy_drift = true;
        if health_first {
            assert!(!driver.check_active());
        }
        assert_eq!(driver.release(), Outcome::RecoveryRequired);
        assert!(
            !driver
                .effects
                .actions
                .contains(&Action::RestoreCapturedSnapshot)
        );
        assert_eq!(driver.effects.finishes, 0);
        assert!(driver.effects.quarantines >= 1);
    }
}

#[test]
fn partial_apply_compensation_does_not_require_never_completed_fixed_policy() {
    let mut driver = Driver::new(Fake {
        fail: Some((Action::SetRootRoutingDomain, Completion::SettledFailure)),
        policy_drift: true,
        ..Fake::default()
    });
    assert_eq!(driver.drive(), Outcome::Clean);
    assert!(
        driver
            .effects
            .actions
            .contains(&Action::RestoreCapturedSnapshot)
    );
    assert_eq!(driver.effects.policy_checks, 0);
    assert_eq!(driver.effects.finishes, 1);
}

#[test]
fn ready_is_after_all_joined_writes_readback_and_durable_active() {
    let mut driver = Driver::new(Fake::default());
    assert_eq!(driver.drive(), Outcome::Ready);
    assert_eq!(
        driver.effects.actions,
        [
            Action::CaptureSnapshot,
            Action::SetFixedServers,
            Action::SetRootRoutingDomain,
            Action::SetDefaultRoute,
            Action::VerifyFixedPolicy
        ]
    );
    assert_eq!(driver.effects.marks, 1);
    assert_eq!(driver.effects.finishes, 0);
    assert_eq!(driver.release(), Outcome::Clean);
    assert_eq!(
        &driver.effects.actions[5..],
        &[
            Action::RestoreCapturedSnapshot,
            Action::VerifyCapturedSnapshot
        ]
    );
    assert_eq!(driver.effects.finishes, 1);
    assert_eq!(driver.effects.quarantines, 0);
}

#[test]
fn incompatible_baseline_never_dispatches_write_or_removes_retention() {
    let mut driver = Driver::new(Fake {
        fail: Some((Action::CaptureSnapshot, Completion::SettledFailure)),
        ..Fake::default()
    });
    assert_eq!(driver.drive(), Outcome::Refused);
    assert_eq!(driver.effects.actions, [Action::CaptureSnapshot]);
    assert_eq!(driver.effects.finishes, 0);
    assert_eq!(driver.effects.marks, 0);
}

#[test]
fn each_definite_denial_restores_only_after_the_reply_and_never_announces_ready() {
    for action in [
        Action::SetFixedServers,
        Action::SetRootRoutingDomain,
        Action::SetDefaultRoute,
        Action::VerifyFixedPolicy,
    ] {
        let mut driver = Driver::new(Fake {
            fail: Some((action, Completion::SettledFailure)),
            ..Fake::default()
        });
        assert_eq!(driver.drive(), Outcome::Clean);
        let n = driver.effects.actions.len();
        assert_eq!(
            &driver.effects.actions[n - 3..],
            &[
                action,
                Action::RestoreCapturedSnapshot,
                Action::VerifyCapturedSnapshot
            ]
        );
        assert_eq!(driver.effects.marks, 0);
        assert_eq!(driver.effects.finishes, 1);
    }
}

#[test]
fn every_unknown_write_or_readback_quarantines_without_compensation_or_ready() {
    for action in [
        Action::SetFixedServers,
        Action::SetRootRoutingDomain,
        Action::SetDefaultRoute,
        Action::VerifyFixedPolicy,
    ] {
        let mut driver = Driver::new(Fake {
            fail: Some((action, Completion::OutcomeUnknown)),
            ..Fake::default()
        });
        assert_eq!(driver.drive(), Outcome::RecoveryRequired);
        assert_eq!(driver.effects.actions.last(), Some(&action));
        assert!(
            !driver
                .effects
                .actions
                .contains(&Action::RestoreCapturedSnapshot)
        );
        assert_eq!(driver.effects.marks, 0);
        assert_eq!(driver.effects.finishes, 0);
        assert_eq!(driver.effects.quarantines, 1);
    }
}

#[test]
fn lease_manager_or_retention_loss_at_any_check_never_promotes_success() {
    for lose_at in 1..=10 {
        let mut driver = Driver::new(Fake {
            lose_at: Some(lose_at),
            ..Fake::default()
        });
        assert_eq!(driver.drive(), Outcome::RecoveryRequired);
        assert_eq!(driver.effects.marks, 0);
        assert_eq!(driver.effects.finishes, 0);
        assert!(
            !driver
                .effects
                .actions
                .contains(&Action::RestoreCapturedSnapshot)
        );
    }
}

#[test]
fn failed_active_journal_never_announces_ready() {
    let mut driver = Driver::new(Fake {
        fail_mark: true,
        ..Fake::default()
    });
    assert_eq!(driver.drive(), Outcome::RecoveryRequired);
    assert_eq!(driver.effects.marks, 1);
    assert_eq!(driver.effects.finishes, 0);
    assert_eq!(driver.effects.quarantines, 1);
}

#[test]
fn failed_cleanup_or_readback_never_releases_retention() {
    for action in [
        Action::RestoreCapturedSnapshot,
        Action::VerifyCapturedSnapshot,
    ] {
        for result in [Completion::SettledFailure, Completion::OutcomeUnknown] {
            let mut driver = Driver::new(Fake {
                fail: Some((action, result)),
                ..Fake::default()
            });
            assert_eq!(driver.drive(), Outcome::Ready);
            assert_eq!(driver.release(), Outcome::RecoveryRequired);
            assert_eq!(driver.effects.finishes, 0);
            assert_eq!(driver.effects.actions.last(), Some(&action));
        }
    }
}

#[test]
fn cleanup_journal_store_or_final_delete_failure_never_announces_released() {
    let mut driver = Driver::new(Fake {
        fail_finish: true,
        ..Fake::default()
    });
    assert_eq!(driver.drive(), Outcome::Ready);
    assert_eq!(driver.release(), Outcome::RecoveryRequired);
    assert_eq!(driver.effects.finishes, 1);
    assert_eq!(driver.effects.quarantines, 1);
}

#[test]
fn false_applied_readback_does_not_promote_ready() {
    let mut driver = Driver::new(Fake {
        fail: Some((
            Action::VerifyFixedPolicy,
            Completion::PolicyReadback(PolicyReadback::default()),
        )),
        ..Fake::default()
    });
    assert_eq!(driver.drive(), Outcome::Clean);
    assert_eq!(driver.effects.marks, 0);
    assert_eq!(driver.effects.finishes, 1);
}

#[test]
fn false_reset_readback_blocks_finish() {
    let mut driver = Driver::new(Fake {
        fail: Some((
            Action::VerifyCapturedSnapshot,
            Completion::SnapshotReadback { matches: false },
        )),
        ..Fake::default()
    });
    assert_eq!(driver.drive(), Outcome::Ready);
    assert_eq!(driver.release(), Outcome::RecoveryRequired);
    assert_eq!(driver.effects.finishes, 0);
}
