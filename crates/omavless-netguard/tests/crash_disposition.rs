//! Test-only counterexample model, not a Linux emulator or recovery executor.
//! Hidden creator/lifetime facts deliberately do not occur in receipt evidence.
use omavless_netguard::receipt::{
    Assessment, HostEpoch, NamespaceObservation, ReceiptRead, Refusal, TableObservation, assess,
    decode,
};
use omavless_netguard::transaction::Marker;
use serde_json::json;

const EPOCH: HostEpoch = HostEpoch {
    boot: [1; 16],
    namespace_epoch: [2; 16],
    namespace_device: 3,
    namespace_inode: 4,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Creator {
    Ours,
    Foreign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Evidence {
    namespace: NamespaceObservation,
    receipt: ReceiptRead,
    table: TableObservation,
    marker: Marker,
}

#[derive(Clone, Copy, Debug)]
struct History {
    evidence: Evidence,
    creator: Creator,
    /// An abstract nft subsystem incarnation, NOT a real exposed Linux ID.
    subsystem_lifetime: u8,
}

fn record(phase: &str, handle: u64) -> ReceiptRead {
    ReceiptRead::Durable(
        decode(
            &serde_json::to_vec(&json!({
                "version":1,"enrolled_uid":1000,"boot":EPOCH.boot,
                "host_netns_epoch":EPOCH.namespace_epoch,
                "netns_device":EPOCH.namespace_device,"netns_inode":EPOCH.namespace_inode,
                "operation":1,"phase":phase,"table_handle":handle
            }))
            .unwrap(),
        )
        .unwrap(),
    )
}

fn observe(evidence: Evidence) -> Assessment {
    assess(1000, evidence.namespace, evidence.receipt, evidence.table)
}

#[test]
fn commit_before_live_publication_and_raced_foreign_create_are_indistinguishable() {
    // A: durable PendingCreate -> our exclusive kernel commit -> process dies.
    // B: durable PendingCreate -> foreign create -> ours fails -> process dies.
    // The observed table handle, exact policy and any public comment can agree.
    let evidence = Evidence {
        namespace: NamespaceObservation::Canonical(EPOCH),
        receipt: record("pending_create", 0),
        table: TableObservation::Present { handle: 11 },
        marker: Marker::Missing,
    };
    let ours = History {
        evidence,
        creator: Creator::Ours,
        subsystem_lifetime: 1,
    };
    let foreign = History {
        evidence,
        creator: Creator::Foreign,
        subsystem_lifetime: 1,
    };
    assert_ne!(ours.creator, foreign.creator);
    assert_eq!(ours.evidence, foreign.evidence);
    for history in [ours, foreign] {
        assert_eq!(
            observe(history.evidence),
            Assessment::RecoveryRequired(Refusal::PendingOperation)
        );
    }
}

#[test]
fn live_identity_consistency_cannot_detect_subsystem_reinitialization() {
    // A: durable Live record and surviving table. B: after helper interruption,
    // the old table/subsystem was removed; a new incarnation assigned the same
    // handle to a foreign table. Boot and network namespace can remain equal.
    // This models possible identifier reuse; it does not unload kernel modules.
    let evidence = Evidence {
        namespace: NamespaceObservation::Canonical(EPOCH),
        receipt: record("live", 1),
        table: TableObservation::Present { handle: 1 },
        marker: Marker::Armed(9),
    };
    let ours = History {
        evidence,
        creator: Creator::Ours,
        subsystem_lifetime: 1,
    };
    let foreign = History {
        evidence,
        creator: Creator::Foreign,
        subsystem_lifetime: 2,
    };
    assert_ne!(ours.subsystem_lifetime, foreign.subsystem_lifetime);
    assert_ne!(ours.creator, foreign.creator);
    assert_eq!(ours.evidence, foreign.evidence);
    // Deliberately ONLY consistency, not ownership/protection or permission to
    // delete. A future caller must not upgrade this result into such authority.
    assert_eq!(observe(ours.evidence), Assessment::LiveIdentityConsistent);
    assert_eq!(
        observe(foreign.evidence),
        Assessment::LiveIdentityConsistent
    );
}

#[test]
fn pending_replace_or_delete_never_infers_completion_from_presence_or_absence() {
    for phase in ["pending_replace", "pending_delete"] {
        for table in [
            TableObservation::Absent,
            TableObservation::Present { handle: 11 },
            TableObservation::Present { handle: 12 },
        ] {
            for marker in [Marker::Missing, Marker::Armed(9), Marker::Closed(9)] {
                let evidence = Evidence {
                    namespace: NamespaceObservation::Canonical(EPOCH),
                    receipt: record(phase, 11),
                    table,
                    marker,
                };
                assert_eq!(
                    observe(evidence),
                    Assessment::RecoveryRequired(Refusal::PendingOperation)
                );
                assert_eq!(evidence.marker, marker);
            }
        }
    }
}

#[test]
fn reboot_and_namespace_replacement_never_retire_receipt_or_generation_fence() {
    for namespace in [
        NamespaceObservation::Canonical(HostEpoch {
            boot: [9; 16],
            ..EPOCH
        }),
        NamespaceObservation::Canonical(HostEpoch {
            namespace_epoch: [9; 16],
            ..EPOCH
        }),
        NamespaceObservation::Unproven,
    ] {
        for table in [
            TableObservation::Absent,
            TableObservation::Present { handle: 11 },
        ] {
            for marker in [Marker::Armed(9), Marker::Closed(9)] {
                let evidence = Evidence {
                    namespace,
                    receipt: record("live", 11),
                    table,
                    marker,
                };
                assert!(matches!(observe(evidence), Assessment::RecoveryRequired(_)));
                assert_eq!(evidence.marker, marker);
            }
        }
    }
}

#[test]
fn absence_is_not_disarmed_intent_or_protection_and_intent_never_proves_ownership() {
    for marker in [
        Marker::Missing,
        Marker::Armed(9),
        Marker::Closed(9),
        Marker::Invalid,
    ] {
        let absent = Evidence {
            namespace: NamespaceObservation::Canonical(EPOCH),
            receipt: ReceiptRead::Missing,
            table: TableObservation::Absent,
            marker,
        };
        // The armed case still requires restoration before any protected claim.
        // The invalid marker still requires independent generation recovery.
        // AbsenceConsistent cannot be interpreted as a disarmed/healthy result.
        assert_eq!(observe(absent), Assessment::AbsenceConsistent);
        let present = Evidence {
            table: TableObservation::Present { handle: 11 },
            ..absent
        };
        assert_eq!(
            observe(present),
            Assessment::RecoveryRequired(Refusal::MissingProvenance)
        );
        assert_eq!(present.marker, marker);
    }
}
