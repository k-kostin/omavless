use omavless_netguard::{policy::*, protocol::*, transaction::*};

fn apply(effect: Effect, host: &mut Observation) {
    match effect {
        Effect::CreateTableAtomic(policy) => {
            assert_eq!(host.table, Table::Absent);
            host.table = Table::OwnedVerified(policy);
        }
        Effect::ReplaceOwnedTableAtomic(policy) => {
            assert!(matches!(
                host.table,
                Table::OwnedVerified(_) | Table::OwnedUnrecognized
            ));
            host.table = Table::OwnedVerified(policy);
        }
        Effect::VerifyTable(policy) => assert_eq!(host.table, Table::OwnedVerified(policy)),
        Effect::PersistArmedDurably(generation) => host.marker = Marker::Armed(generation),
        Effect::PersistClosedDurably(generation) => host.marker = Marker::Closed(generation),
        Effect::DeleteOwnedTableAtomic => {
            assert!(matches!(
                host.table,
                Table::OwnedVerified(_) | Table::OwnedUnrecognized
            ));
            host.table = Table::Absent;
        }
        Effect::VerifyTableAbsent => assert_eq!(host.table, Table::Absent),
    }
}

fn finish(mut transaction: Transaction, host: &mut Observation) -> Response {
    while let Some(effect) = transaction.next_effect() {
        assert!(transaction.response().is_err());
        apply(effect, host);
        transaction.acknowledge(effect, true).unwrap();
    }
    transaction.response().unwrap()
}

fn empty() -> Observation {
    Observation {
        marker: Marker::Missing,
        table: Table::Absent,
    }
}

fn armed() -> Observation {
    Observation {
        marker: Marker::Armed(7),
        table: Table::OwnedVerified(Policy::FullVpn),
    }
}

#[test]
fn arm_acknowledgement_follows_verified_kernel_and_durable_marker() {
    let mut host = empty();
    let transaction = plan(
        Request::Arm {
            generation: 7,
            mode: Mode::Full,
        },
        host,
    )
    .unwrap();
    let response = finish(transaction, &mut host);
    assert_eq!(host, armed());
    assert_eq!(response, observe(host));
    let transaction = plan(Request::Disarm { generation: 7 }, host).unwrap();
    assert_eq!(finish(transaction, &mut host), observe(empty()));
    assert_eq!(host, closed(7));
}

fn closed(generation: u64) -> Observation {
    Observation {
        marker: Marker::Closed(generation),
        table: Table::Absent,
    }
}

#[test]
fn crashes_before_and_after_every_effect_reconcile_from_durable_facts() {
    for is_arm in [true, false] {
        for crash_step in 0..3 {
            for effect_applied in [false, true] {
                let mut host = if is_arm { empty() } else { armed() };
                let request = if is_arm {
                    Request::Arm {
                        generation: 7,
                        mode: Mode::Full,
                    }
                } else {
                    Request::Disarm { generation: 7 }
                };
                let mut transaction = plan(request, host).unwrap();
                for _ in 0..crash_step {
                    let effect = transaction.next_effect().unwrap();
                    apply(effect, &mut host);
                    transaction.acknowledge(effect, true).unwrap();
                }
                let effect = transaction.next_effect().unwrap();
                if effect_applied {
                    apply(effect, &mut host);
                }
                // No success response can escape an uncertain effect.
                assert!(transaction.acknowledge(effect, false).is_err());
                assert!(transaction.response().is_err());
                assert_eq!(transaction.next_effect(), None);
                // Simulate process death: re-read durable marker AND table.
                let marker_at_crash = host.marker;
                let response = finish(reconcile(host).unwrap(), &mut host);
                assert_eq!(response, observe(host));
                match marker_at_crash {
                    Marker::Armed(_) => assert_eq!(host, armed()),
                    Marker::Missing => assert_eq!(host, empty()),
                    Marker::Closed(generation) => assert_eq!(host, closed(generation)),
                    Marker::Invalid => unreachable!(),
                }
                // This proves only helper recovery. The future runtime must
                // persist connected AFTER arm acknowledgement and disconnected
                // plus verified core cleanup BEFORE requesting disarm.
            }
        }
    }
}

#[test]
fn live_connected_crashes_retain_protection_and_restart_reinstalls_it() {
    for table in [
        Table::OwnedVerified(Policy::FullVpn),
        Table::OwnedUnrecognized,
        Table::Absent,
    ] {
        let mut host = Observation {
            marker: Marker::Armed(7),
            table,
        };
        finish(reconcile(host).unwrap(), &mut host);
        assert_eq!(host, armed());
    }
}

#[test]
fn invalid_persistence_never_becomes_disarmed_or_accepts_mutations() {
    for table in [
        Table::Absent,
        Table::OwnedUnrecognized,
        Table::OwnedVerified(Policy::FullVpn),
    ] {
        let mut host = Observation {
            marker: Marker::Invalid,
            table,
        };
        assert!(
            plan(
                Request::Arm {
                    generation: 7,
                    mode: Mode::Full
                },
                host
            )
            .is_err()
        );
        assert!(plan(Request::Disarm { generation: 7 }, host).is_err());
        let response = finish(reconcile(host).unwrap(), &mut host);
        assert_eq!(host.marker, Marker::Invalid);
        assert_eq!(host.table, Table::OwnedVerified(Policy::Emergency));
        assert_eq!(response, observe(host));
        assert_eq!(
            response,
            Response::Status {
                policy_version: 1,
                protection: Protection::Emergency {},
                health: Health::ManualRecoveryRequired
            }
        );
    }
}

#[test]
fn stale_generation_and_out_of_order_acknowledgements_cannot_release_policy() {
    for request in [
        Request::Arm {
            generation: 6,
            mode: Mode::Full,
        },
        Request::Arm {
            generation: 8,
            mode: Mode::Full,
        },
        Request::Disarm { generation: 6 },
    ] {
        assert_eq!(
            plan(request, armed()).unwrap_err(),
            ErrorCode::GenerationConflict
        );
    }
    let mut transaction = plan(Request::Disarm { generation: 7 }, armed()).unwrap();
    assert!(
        transaction
            .acknowledge(Effect::VerifyTableAbsent, true)
            .is_err()
    );
    assert!(transaction.response().is_err());
    assert_eq!(transaction.next_effect(), None);
}

#[test]
fn retries_reverify_arm_and_only_confirm_fully_disarmed_state() {
    let mut host = armed();
    host.table = Table::Absent;
    assert!(matches!(observe(host), Response::Error { .. }));
    let transaction = plan(
        Request::Arm {
            generation: 7,
            mode: Mode::Full,
        },
        host,
    )
    .unwrap();
    finish(transaction, &mut host);
    assert_eq!(host, armed());
    let transaction = plan(Request::Disarm { generation: 7 }, closed(7)).unwrap();
    assert_eq!(transaction.response().unwrap(), observe(empty()));
    assert!(
        plan(
            Request::Disarm { generation: 7 },
            Observation {
                marker: Marker::Missing,
                table: Table::OwnedVerified(Policy::FullVpn)
            }
        )
        .is_err()
    );
}

#[test]
fn fixed_policy_has_no_general_physical_established_dns_or_lan_bypass() {
    assert_eq!(TABLE_FAMILY, "inet");
    assert_eq!(TABLE_NAME, "omavless_netguard");
    assert_eq!(
        Policy::FullVpn.rules().last(),
        Some(&Rule::DropAllOtherOutput)
    );
    assert_eq!(
        Policy::Emergency.rules(),
        &[Rule::AllowLoopback, Rule::DropAllOtherOutput]
    );
    assert_eq!(Policy::FullVpn.rules().len(), 7);
}

fn arm(generation: u64) -> Request {
    Request::Arm {
        generation,
        mode: Mode::Full,
    }
}

#[test]
fn completed_disarm_retires_generation_across_restart_and_delayed_messages() {
    let mut host = empty();
    finish(plan(arm(7), host).unwrap(), &mut host);
    finish(
        plan(Request::Disarm { generation: 7 }, host).unwrap(),
        &mut host,
    );
    // New transactions/processes receive only the persisted snapshot: no
    // in-memory replay cache participates in this fence.
    finish(reconcile(host).unwrap(), &mut host);
    assert_eq!(host, closed(7));
    for generation in [0, 6, 7] {
        assert_eq!(
            plan(arm(generation), host).unwrap_err(),
            ErrorCode::GenerationConflict
        );
    }
    assert!(plan(Request::Disarm { generation: 6 }, host).is_err());
    assert!(plan(Request::Disarm { generation: 8 }, host).is_err());
    assert!(
        plan(Request::Disarm { generation: 7 }, host)
            .unwrap()
            .response()
            .is_ok()
    );
    finish(plan(arm(8), host).unwrap(), &mut host);
    assert_eq!(host.marker, Marker::Armed(8));
    assert_eq!(
        plan(arm(7), host).unwrap_err(),
        ErrorCode::GenerationConflict
    );
}

#[test]
fn interrupted_disarm_cannot_forget_closed_generation() {
    for table in [
        Table::Absent,
        Table::OwnedVerified(Policy::FullVpn),
        Table::OwnedUnrecognized,
    ] {
        let initial = Observation {
            marker: Marker::Armed(7),
            table,
        };
        let mut step_count = 0;
        let mut transaction = plan(Request::Disarm { generation: 7 }, initial).unwrap();
        while let Some(effect) = transaction.next_effect() {
            step_count += 1;
            transaction.acknowledge(effect, true).unwrap();
        }
        for crash_after in 1..=step_count {
            let mut host = initial;
            let mut transaction = plan(Request::Disarm { generation: 7 }, host).unwrap();
            for _ in 0..crash_after {
                let effect = transaction.next_effect().unwrap();
                apply(effect, &mut host);
                transaction.acknowledge(effect, true).unwrap();
            }
            assert_eq!(host.marker, Marker::Closed(7));
            assert_eq!(
                plan(arm(7), host).unwrap_err(),
                ErrorCode::GenerationConflict
            );
            finish(reconcile(host).unwrap(), &mut host);
            assert_eq!(host, closed(7));
            assert!(plan(arm(7), host).is_err());
        }
    }
}

#[test]
fn successor_arm_crashes_preserve_the_previous_retirement_floor() {
    for crash_after in 0..=3 {
        let mut host = closed(7);
        let mut transaction = plan(arm(8), host).unwrap();
        for _ in 0..crash_after {
            let effect = transaction.next_effect().unwrap();
            apply(effect, &mut host);
            transaction.acknowledge(effect, true).unwrap();
        }
        finish(reconcile(host).unwrap(), &mut host);
        assert!(plan(arm(7), host).is_err());
        if crash_after < 3 {
            assert_eq!(host, closed(7));
        } else {
            assert_eq!(host.marker, Marker::Armed(8));
        }
    }
}

#[test]
fn zero_is_valid_initially_but_exhausted_generation_never_wraps_or_resets() {
    let mut host = empty();
    finish(plan(arm(0), host).unwrap(), &mut host);
    finish(
        plan(Request::Disarm { generation: 0 }, host).unwrap(),
        &mut host,
    );
    assert!(plan(arm(0), host).is_err());
    finish(plan(arm(u64::MAX), host).unwrap(), &mut host);
    // Same-generation retry is valid only while still armed.
    finish(plan(arm(u64::MAX), host).unwrap(), &mut host);
    finish(
        plan(
            Request::Disarm {
                generation: u64::MAX,
            },
            host,
        )
        .unwrap(),
        &mut host,
    );
    finish(reconcile(host).unwrap(), &mut host);
    assert_eq!(host, closed(u64::MAX));
    for generation in [0, 1, u64::MAX - 1, u64::MAX] {
        assert_eq!(
            plan(arm(generation), host).unwrap_err(),
            ErrorCode::GenerationConflict
        );
    }
}

#[test]
fn absent_marker_is_not_a_disarm_receipt_and_corruption_is_not_a_reset() {
    assert!(plan(Request::Disarm { generation: 7 }, empty()).is_err());
    let mut host = closed(7);
    host.marker = Marker::Invalid;
    finish(reconcile(host).unwrap(), &mut host);
    for generation in [0, 7, 8, u64::MAX] {
        assert_eq!(
            plan(arm(generation), host).unwrap_err(),
            ErrorCode::ManualRecoveryRequired
        );
        assert_eq!(
            plan(Request::Disarm { generation }, host).unwrap_err(),
            ErrorCode::ManualRecoveryRequired
        );
    }
    assert_eq!(host.marker, Marker::Invalid);
}

#[test]
fn foreign_and_unreadable_tables_refuse_all_mutations_even_during_emergency_reconcile() {
    for marker in [
        Marker::Missing,
        Marker::Armed(7),
        Marker::Closed(7),
        Marker::Invalid,
    ] {
        for table in [Table::Foreign, Table::Unreadable] {
            let host = Observation { marker, table };
            for request in [arm(7), arm(8), Request::Disarm { generation: 7 }] {
                assert_eq!(
                    plan(request, host).unwrap_err(),
                    ErrorCode::ManualRecoveryRequired
                );
            }
            assert_eq!(
                reconcile(host).unwrap_err(),
                ErrorCode::ManualRecoveryRequired
            );
            assert_eq!(
                observe(host),
                Response::Error {
                    code: ErrorCode::ManualRecoveryRequired
                }
            );
        }
    }
}

#[test]
fn create_replace_and_delete_plans_follow_observed_ownership() {
    for table in [
        Table::Absent,
        Table::OwnedVerified(Policy::FullVpn),
        Table::OwnedUnrecognized,
    ] {
        let host = Observation {
            marker: Marker::Closed(7),
            table,
        };
        let arm_plan = plan(arm(8), host).unwrap();
        let expected = if table == Table::Absent {
            Effect::CreateTableAtomic(Policy::FullVpn)
        } else {
            Effect::ReplaceOwnedTableAtomic(Policy::FullVpn)
        };
        assert_eq!(arm_plan.next_effect(), Some(expected));
        let reconcile_plan = reconcile(host).unwrap();
        assert_eq!(
            reconcile_plan.next_effect(),
            Some(if table == Table::Absent {
                Effect::VerifyTableAbsent
            } else {
                Effect::DeleteOwnedTableAtomic
            })
        );
    }
}
