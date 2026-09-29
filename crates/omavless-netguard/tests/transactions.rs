use omavless_netguard::{policy::*, protocol::*, transaction::*};

fn apply(effect: Effect, host: &mut Observation) {
    match effect {
        Effect::InstallAtomic(policy) => host.table = Table::Verified(policy),
        Effect::VerifyTable(policy) => assert_eq!(host.table, Table::Verified(policy)),
        Effect::PersistArmedDurably(generation) => host.marker = Marker::Armed(generation),
        Effect::RemoveMarkerDurably => host.marker = Marker::Missing,
        Effect::DeleteOwnedTableAtomic => host.table = Table::Absent,
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
        table: Table::Verified(Policy::FullVpn),
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
    assert_eq!(host, empty());
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
                // Simulate process death: new plan uses only durable marker.
                let marker_at_crash = host.marker;
                let response = finish(reconcile(marker_at_crash), &mut host);
                assert_eq!(response, observe(host));
                match marker_at_crash {
                    Marker::Armed(_) => assert_eq!(host, armed()),
                    Marker::Missing => assert_eq!(host, empty()),
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
        Table::Verified(Policy::FullVpn),
        Table::Unknown,
        Table::Absent,
    ] {
        let mut host = Observation {
            marker: Marker::Armed(7),
            table,
        };
        finish(reconcile(host.marker), &mut host);
        assert_eq!(host, armed());
    }
}

#[test]
fn invalid_persistence_never_becomes_disarmed_or_accepts_mutations() {
    for table in [
        Table::Absent,
        Table::Unknown,
        Table::Verified(Policy::FullVpn),
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
        let response = finish(reconcile(host.marker), &mut host);
        assert_eq!(host.marker, Marker::Invalid);
        assert_eq!(host.table, Table::Verified(Policy::Emergency));
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
    let transaction = plan(Request::Disarm { generation: 7 }, empty()).unwrap();
    assert_eq!(transaction.response().unwrap(), observe(empty()));
    assert!(
        plan(
            Request::Disarm { generation: 7 },
            Observation {
                marker: Marker::Missing,
                table: Table::Verified(Policy::FullVpn)
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
