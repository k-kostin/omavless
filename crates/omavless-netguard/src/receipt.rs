//! Inactive receipt consistency model, not an ownership authority or executor.
//!
//! Decoding bytes proves neither root storage trust nor exclusive kernel creation.
//! No result converts to `TrustedTableIdentity`, acknowledges an effect, changes
//! a generation marker, or authorizes mutation. The future adapter must establish
//! durable provenance, canonical host-netns lifetime and conditional execution.
use serde::{Deserialize, Serialize};

pub const MAX_RECEIPT_BYTES: usize = 2048;

/// Modeled canonical host namespace lifetime, independently established by a
/// future adapter. The epoch is NOT derived from an inode, generated here, or
/// trusted merely because it occurs in a receipt. Its establishment is open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostEpoch {
    pub boot: [u8; 16],
    pub namespace_epoch: [u8; 16],
    pub namespace_device: u64,
    pub namespace_inode: u64,
}

impl HostEpoch {
    fn valid(self) -> bool {
        self.boot != [0; 16] && self.namespace_epoch != [0; 16] && self.namespace_inode != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptState {
    PendingCreate,
    Live { handle: u64 },
    PendingReplace { old_handle: u64 },
    PendingDelete { old_handle: u64 },
    Retired,
}

/// A syntactically valid record only. Private fields prevent callers from
/// bypassing decoding invariants; they do not authenticate this record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Receipt {
    enrolled_uid: u32,
    epoch: HostEpoch,
    operation: u64,
    state: ReceiptState,
}

impl Receipt {
    pub fn operation(self) -> u64 {
        self.operation
    }

    pub(crate) fn transaction_record(
        enrolled_uid: u32,
        epoch: HostEpoch,
        operation: u64,
        state: ReceiptState,
    ) -> Result<Self, DecodeError> {
        let record = Self {
            enrolled_uid,
            epoch,
            operation,
            state,
        };
        // Reuse all wire invariants, including nonzero epochs/operation/handles.
        decode(&record.encode()?)
    }

    pub fn state(self) -> ReceiptState {
        self.state
    }

    pub(crate) fn enrolled_uid(self) -> u32 {
        self.enrolled_uid
    }

    #[cfg(feature = "netguard-cold-bootstrap")]
    pub(crate) fn epoch(self) -> HostEpoch {
        self.epoch
    }

    pub(crate) fn encode(self) -> Result<Vec<u8>, DecodeError> {
        let (phase, table_handle) = match self.state {
            ReceiptState::PendingCreate => ("pending_create", 0),
            ReceiptState::Live { handle } => ("live", handle),
            ReceiptState::PendingReplace { old_handle } => ("pending_replace", old_handle),
            ReceiptState::PendingDelete { old_handle } => ("pending_delete", old_handle),
            ReceiptState::Retired => ("retired", 0),
        };
        serde_json::to_vec(&Record {
            version: 1,
            enrolled_uid: self.enrolled_uid,
            boot: self.epoch.boot,
            host_netns_epoch: self.epoch.namespace_epoch,
            netns_device: self.epoch.namespace_device,
            netns_inode: self.epoch.namespace_inode,
            operation: self.operation,
            phase: phase.into(),
            table_handle,
        })
        .map_err(|_| DecodeError::Invalid)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Invalid,
}

// Deliberately flat: derived struct decoding rejects duplicate fields. The
// explicit leading-object check also refuses Serde's positional-array form.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    enrolled_uid: u32,
    boot: [u8; 16],
    host_netns_epoch: [u8; 16],
    netns_device: u64,
    netns_inode: u64,
    operation: u64,
    phase: String,
    table_handle: u64,
}

/// Pure bounded decoding; missing/unsafe storage must never be passed as an
/// empty document or interpreted as a missing receipt by a future adapter.
pub fn decode(bytes: &[u8]) -> Result<Receipt, DecodeError> {
    let invalid = DecodeError::Invalid;
    if bytes.len() > MAX_RECEIPT_BYTES
        || bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{')
    {
        return Err(invalid);
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| invalid)?;
    let epoch = HostEpoch {
        boot: record.boot,
        namespace_epoch: record.host_netns_epoch,
        namespace_device: record.netns_device,
        namespace_inode: record.netns_inode,
    };
    if record.version != 1 || record.enrolled_uid == 0 || record.operation == 0 || !epoch.valid() {
        return Err(invalid);
    }
    let handle = record.table_handle;
    let state = match (record.phase.as_str(), handle) {
        ("pending_create", 0) => ReceiptState::PendingCreate,
        ("retired", 0) => ReceiptState::Retired,
        (_, 0) => return Err(invalid),
        ("live", handle) => ReceiptState::Live { handle },
        ("pending_replace", old_handle) => ReceiptState::PendingReplace { old_handle },
        ("pending_delete", old_handle) => ReceiptState::PendingDelete { old_handle },
        _ => return Err(invalid),
    };
    Ok(Receipt {
        enrolled_uid: record.enrolled_uid,
        epoch,
        operation: record.operation,
        state,
    })
}

/// Evidence vocabulary, not itself a filesystem reader. The separate inactive
/// store verifies storage, publication and directory lock binding; synthetic
/// callers may model those facts. Simply calling `decode` cannot establish them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRead {
    Missing,
    Durable(Receipt),
    UnsafeOrUncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespaceObservation {
    Canonical(HostEpoch),
    Unproven,
}

/// Only complete observations in the independently bound canonical namespace
/// are eligible. Policy verification is deliberately outside this model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableObservation {
    Absent,
    Present { handle: u64 },
    Unreadable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    UnsafeOrUncertainReceipt,
    UnprovenNamespace,
    InvalidEnrollment,
    EnrollmentMismatch,
    EpochMismatch,
    UnreadableTable,
    PendingOperation,
    MissingProvenance,
    IdentityMismatch,
}

/// Consistency is a necessary condition only, never permission to mutate or
/// proof of protection. Pending operations always require a separate recovery
/// decision, even when the old handle or complete absence is observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Assessment {
    AbsenceConsistent,
    LiveIdentityConsistent,
    RecoveryRequired(Refusal),
}

pub fn assess(
    enrolled_uid: u32,
    namespace: NamespaceObservation,
    receipt: ReceiptRead,
    table: TableObservation,
) -> Assessment {
    use Assessment::{AbsenceConsistent, LiveIdentityConsistent, RecoveryRequired};
    if enrolled_uid == 0 {
        return RecoveryRequired(Refusal::InvalidEnrollment);
    }
    let NamespaceObservation::Canonical(epoch) = namespace else {
        return RecoveryRequired(Refusal::UnprovenNamespace);
    };
    if !epoch.valid() {
        return RecoveryRequired(Refusal::UnprovenNamespace);
    }
    let record = match receipt {
        ReceiptRead::UnsafeOrUncertain => {
            return RecoveryRequired(Refusal::UnsafeOrUncertainReceipt);
        }
        ReceiptRead::Missing => None,
        ReceiptRead::Durable(record) => {
            if record.enrolled_uid != enrolled_uid {
                return RecoveryRequired(Refusal::EnrollmentMismatch);
            }
            // Even an absent table cannot silently retire an old-boot receipt.
            // Reboot disposition and generation fencing belong to another plan.
            if record.epoch != epoch {
                return RecoveryRequired(Refusal::EpochMismatch);
            }
            Some(record)
        }
    };
    if matches!(
        table,
        TableObservation::Unreadable | TableObservation::Present { handle: 0 }
    ) {
        return RecoveryRequired(Refusal::UnreadableTable);
    }
    match (record.map(|r| r.state), table) {
        (None | Some(ReceiptState::Retired), TableObservation::Absent) => AbsenceConsistent,
        (Some(ReceiptState::Live { handle }), TableObservation::Present { handle: observed })
            if handle == observed =>
        {
            LiveIdentityConsistent
        }
        (Some(ReceiptState::Live { .. }), _) => RecoveryRequired(Refusal::IdentityMismatch),
        (
            Some(
                ReceiptState::PendingCreate
                | ReceiptState::PendingReplace { .. }
                | ReceiptState::PendingDelete { .. },
            ),
            _,
        ) => RecoveryRequired(Refusal::PendingOperation),
        _ => RecoveryRequired(Refusal::MissingProvenance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const EPOCH: HostEpoch = HostEpoch {
        boot: [1; 16],
        namespace_epoch: [2; 16],
        namespace_device: 3,
        namespace_inode: 4,
    };

    fn fixture(phase: &str, handle: u64) -> Value {
        json!({
            "version": 1, "enrolled_uid": 1001, "boot": EPOCH.boot,
            "host_netns_epoch": EPOCH.namespace_epoch,
            "netns_device": EPOCH.namespace_device, "netns_inode": EPOCH.namespace_inode,
            "operation": 1, "phase": phase, "table_handle": handle
        })
    }

    fn record(phase: &str, handle: u64) -> Receipt {
        decode(&serde_json::to_vec(&fixture(phase, handle)).unwrap()).unwrap()
    }

    fn assessment(receipt: ReceiptRead, table: TableObservation) -> Assessment {
        assess(1001, NamespaceObservation::Canonical(EPOCH), receipt, table)
    }

    #[test]
    fn states_retain_only_the_intended_old_identity() {
        for (phase, handle, expected) in [
            ("pending_create", 0, ReceiptState::PendingCreate),
            ("live", 7, ReceiptState::Live { handle: 7 }),
            (
                "pending_replace",
                7,
                ReceiptState::PendingReplace { old_handle: 7 },
            ),
            (
                "pending_delete",
                7,
                ReceiptState::PendingDelete { old_handle: 7 },
            ),
            ("retired", 0, ReceiptState::Retired),
        ] {
            assert_eq!(record(phase, handle).state(), expected);
            let wrong = fixture(phase, if handle == 0 { 7 } else { 0 });
            assert_eq!(
                decode(&serde_json::to_vec(&wrong).unwrap()),
                Err(DecodeError::Invalid)
            );
        }
    }

    #[test]
    fn strict_document_bounds_duplicates_and_types() {
        let valid = serde_json::to_string(&fixture("live", 7)).unwrap();
        let mut padded = valid.as_bytes().to_vec();
        padded.resize(MAX_RECEIPT_BYTES, b' ');
        assert!(decode(&padded).is_ok());
        padded.push(b' ');
        assert_eq!(decode(&padded), Err(DecodeError::Invalid));
        for bytes in [
            b"".as_slice(),
            b"{}",
            b"[]",
            b"null",
            b"\xff",
            &valid.as_bytes()[..valid.len() - 1],
        ] {
            assert_eq!(decode(bytes), Err(DecodeError::Invalid));
        }
        for changed in [
            format!("{valid}{{}}"),
            valid.replacen('{', "{\"version\":1,", 1),
            valid.replacen('{', "{\"unexpected\":true,", 1),
            valid.replace("\"table_handle\":7", "\"table_handle\":7.0"),
            valid.replace("\"table_handle\":7", "\"table_handle\":-1"),
            valid.replace(
                "\"table_handle\":7",
                "\"table_handle\":18446744073709551616",
            ),
        ] {
            assert_eq!(decode(changed.as_bytes()), Err(DecodeError::Invalid));
        }
        // Duplicate every actual field, including equal values and phase.
        for (key, value) in fixture("live", 7).as_object().unwrap() {
            let duplicate = valid.replacen('{', &format!("{{{}:{value},", json!(key)), 1);
            assert_eq!(decode(duplicate.as_bytes()), Err(DecodeError::Invalid));
        }
        for (field, value) in [
            ("version", json!(2)),
            ("enrolled_uid", json!(0)),
            ("operation", json!(0)),
            ("netns_inode", json!(0)),
            ("boot", json!(vec![0; 16])),
            ("host_netns_epoch", json!(vec![0; 16])),
            ("boot", json!(vec![1; 15])),
            ("boot", json!(vec![256; 16])),
            ("phase", json!("unknown")),
            ("phase", json!({"live": null})),
        ] {
            let mut changed = fixture("live", 7);
            changed[field] = value;
            assert_eq!(
                decode(&serde_json::to_vec(&changed).unwrap()),
                Err(DecodeError::Invalid)
            );
        }
        for field in fixture("live", 7).as_object().unwrap().keys() {
            let mut changed = fixture("live", 7);
            changed.as_object_mut().unwrap().remove(field);
            assert_eq!(
                decode(&serde_json::to_vec(&changed).unwrap()),
                Err(DecodeError::Invalid)
            );
        }
    }

    #[test]
    fn live_matching_identity_is_consistency_only() {
        let live = ReceiptRead::Durable(record("live", 7));
        assert_eq!(
            assessment(live, TableObservation::Present { handle: 7 }),
            Assessment::LiveIdentityConsistent
        );
        for table in [
            TableObservation::Absent,
            TableObservation::Present { handle: 8 },
        ] {
            assert_eq!(
                assessment(live, table),
                Assessment::RecoveryRequired(Refusal::IdentityMismatch)
            );
        }
        for table in [
            TableObservation::Unreadable,
            TableObservation::Present { handle: 0 },
        ] {
            assert_eq!(
                assessment(live, table),
                Assessment::RecoveryRequired(Refusal::UnreadableTable)
            );
        }
    }

    #[test]
    fn boot_namespace_lifetime_and_enrollment_changes_refuse() {
        let live = ReceiptRead::Durable(record("live", 7));
        for epoch in [
            HostEpoch {
                boot: [3; 16],
                ..EPOCH
            },
            // Same device/inode after modeled namespace recreation is not the same lifetime.
            HostEpoch {
                namespace_epoch: [3; 16],
                ..EPOCH
            },
            HostEpoch {
                namespace_device: 8,
                ..EPOCH
            },
            HostEpoch {
                namespace_inode: 8,
                ..EPOCH
            },
        ] {
            for table in [
                TableObservation::Absent,
                TableObservation::Present { handle: 7 },
            ] {
                assert_eq!(
                    assess(1001, NamespaceObservation::Canonical(epoch), live, table),
                    Assessment::RecoveryRequired(Refusal::EpochMismatch)
                );
            }
        }
        for namespace in [
            NamespaceObservation::Unproven,
            NamespaceObservation::Canonical(HostEpoch {
                boot: [0; 16],
                ..EPOCH
            }),
        ] {
            assert_eq!(
                assess(1001, namespace, live, TableObservation::Absent),
                Assessment::RecoveryRequired(Refusal::UnprovenNamespace)
            );
        }
        for (uid, reason) in [
            (0, Refusal::InvalidEnrollment),
            (1002, Refusal::EnrollmentMismatch),
        ] {
            assert_eq!(
                assess(
                    uid,
                    NamespaceObservation::Canonical(EPOCH),
                    live,
                    TableObservation::Absent
                ),
                Assessment::RecoveryRequired(reason)
            );
        }
    }

    #[test]
    fn all_pending_crash_outcomes_require_recovery() {
        for (phase, handle) in [
            ("pending_create", 0),
            ("pending_replace", 7),
            ("pending_delete", 7),
        ] {
            let pending = ReceiptRead::Durable(record(phase, handle));
            // Before execution, lost acknowledgement, after replacement, or
            // after deletion: none can silently promote/retire a pending receipt.
            for table in [
                TableObservation::Absent,
                TableObservation::Present { handle: 7 },
                TableObservation::Present { handle: 8 },
            ] {
                assert_eq!(
                    assessment(pending, table),
                    Assessment::RecoveryRequired(Refusal::PendingOperation)
                );
            }
        }
    }

    #[test]
    fn crash_before_receipt_publication_cannot_adopt_kernel_table() {
        for receipt in [
            ReceiptRead::Missing,
            ReceiptRead::Durable(record("retired", 0)),
        ] {
            assert_eq!(
                assessment(receipt, TableObservation::Absent),
                Assessment::AbsenceConsistent
            );
            assert_eq!(
                assessment(receipt, TableObservation::Present { handle: 7 }),
                Assessment::RecoveryRequired(Refusal::MissingProvenance)
            );
        }
        // The receipt write may have reached any staging/fsync/rename boundary.
        // Visible bytes alone are not a durable-publication acknowledgement.
        for table in [
            TableObservation::Absent,
            TableObservation::Present { handle: 7 },
            TableObservation::Unreadable,
        ] {
            assert_eq!(
                assessment(ReceiptRead::UnsafeOrUncertain, table),
                Assessment::RecoveryRequired(Refusal::UnsafeOrUncertainReceipt)
            );
        }
    }
}
