//! Inactive shared-lock transaction composition, exercised with synthetic ports.
//! No production caller or provenance provider exists. Receipts cannot turn an
//! untrusted orphan into an owned table. There is deliberately no recovery API.
use crate::effect_port::{EffectIdentity, EffectPort, EffectSnapshot, ExchangeBoundary};
use crate::enrollment::EnrollmentBinding;

use crate::policy::Policy;
use crate::protocol::{ErrorCode, Request, Response};
use crate::receipt::{
    self, Assessment, HostEpoch, NamespaceObservation, Receipt, ReceiptRead, ReceiptState,
    TableObservation,
};
use crate::receipt_store::ReceiptStore;
use crate::root_state::{RootStateStore, StateError};
use crate::transaction::{self, Effect, Marker, Observation, Table};
use crate::transport_candidate::{self, TransportError};
use std::os::unix::net::UnixStream;

const REFUSED: ErrorCode = ErrorCode::ManualRecoveryRequired;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Point {
    BeforePending,
    Pending,
    BeforeKernel,
    KernelReturned,
    KernelVerified,
    BeforeMarker,
    Marker,
    BeforeTerminal,
    Terminal,
    BeforeReply,
    ReceiptWrite { terminal: bool, boundary: u8 },
    MarkerWrite(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    marker: Marker,
    receipt: ReceiptRead,
    kernel: EffectSnapshot,
}

/// The request was never admitted, or the outcome of delivering an already
/// processed response is unknown. Neither case permits an automatic retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExchangeError {
    NoEnrollment,
    AuthorityUnavailable,
    Receive(TransportError),
    ReplyDeliveryUnknown(TransportError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExchangePoint {
    Received,
    BeforeResponse,
}

/// Owns both records through exactly one pinned-directory lock. Construction
/// transfers ownership, so a caller cannot retain a separately usable marker
/// writer. Poisoning and pending receipts are barriers, never replay tickets.
pub struct LockedState {
    receipts: ReceiptStore,
    enrollment: Option<EnrollmentBinding>,
    poisoned: bool,
}

impl LockedState {
    /// Fixed administrator enrollment is read from a root-owned file; no UID
    /// from an IPC request or caller is accepted by this entry point.
    pub fn open_fixed() -> Result<Self, StateError> {
        let enrollment =
            EnrollmentBinding::open_fixed().map_err(|_| StateError::UnsafeOrUnreadable)?;
        let mut state = Self::from_root(RootStateStore::open_fixed(enrollment.uid())?);
        enrollment
            .validate()
            .map_err(|_| StateError::UnsafeOrUnreadable)?;
        state.enrollment = Some(enrollment);
        Ok(state)
    }

    pub(crate) fn from_root(root: RootStateStore) -> Self {
        Self {
            receipts: ReceiptStore::from_root(root),
            enrollment: None,
            poisoned: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn bind_fixture_enrollment(&mut self, binding: EnrollmentBinding) {
        assert!(self.enrollment.is_none());
        self.enrollment = Some(binding);
    }

    #[allow(dead_code)] // The inactive session owner has no product caller yet.
    pub(crate) fn enrollment_current(&self) -> bool {
        self.enrollment
            .as_ref()
            .is_some_and(|binding| binding.validate().is_ok())
    }

    /// Inactive one-request composition. The transport and transaction use
    /// this state's single pinned enrollment; a caller cannot substitute a
    /// different binding. The owned stream is closed after this exchange.
    /// Namespace and kernel provenance remain separate unimplemented gates.
    #[allow(dead_code)]
    pub(crate) fn exchange_once<K: EffectPort>(
        &mut self,
        stream: UnixStream,
        namespace: NamespaceObservation,
        kernel: &mut K,
    ) -> Result<(), ExchangeError> {
        self.exchange_with(stream, namespace, kernel, |_| {})
    }

    fn exchange_with<K: EffectPort>(
        &mut self,
        stream: UnixStream,
        namespace: NamespaceObservation,
        kernel: &mut K,
        mut checkpoint: impl FnMut(ExchangePoint),
    ) -> Result<(), ExchangeError> {
        kernel
            .exchange_boundary(ExchangeBoundary::BeforeReceive)
            .map_err(|_| ExchangeError::AuthorityUnavailable)?;
        let binding = self
            .enrollment
            .as_ref()
            .ok_or(ExchangeError::NoEnrollment)?;
        let request = transport_candidate::receive_request(&stream, binding)
            .map_err(ExchangeError::Receive)?;
        checkpoint(ExchangePoint::Received);
        kernel
            .exchange_boundary(ExchangeBoundary::AfterReceive)
            .map_err(|_| ExchangeError::AuthorityUnavailable)?;
        let response = match self.request(request, namespace, kernel) {
            Ok(response) => response,
            Err(code) => Response::Error { code },
        };
        checkpoint(ExchangePoint::BeforeResponse);
        kernel
            .exchange_boundary(ExchangeBoundary::BeforeReply)
            .map_err(|_| ExchangeError::AuthorityUnavailable)?;
        // Reborrow the same binding after request's mutable borrow. A failed
        // socket write never undoes a durable transaction or replays effects.
        let binding = self
            .enrollment
            .as_ref()
            .ok_or(ExchangeError::NoEnrollment)?;
        transport_candidate::send_response(&stream, binding, response)
            .map_err(ExchangeError::ReplyDeliveryUnknown)?;
        // Failure here cannot retract bytes already delivered or roll back a
        // committed record. It only seals the bound provider's future work.
        kernel
            .exchange_boundary(ExchangeBoundary::AfterReply)
            .map_err(|_| ExchangeError::AuthorityUnavailable)
    }

    /// Namespace and port facts remain independently supplied proof obligations,
    /// not evidence this adapter can authenticate. Keep the effect entry inside
    /// this crate until a reviewed production adapter can establish them.
    pub fn request<K: EffectPort>(
        &mut self,
        request: Request,
        namespace: NamespaceObservation,
        kernel: &mut K,
    ) -> Result<Response, ErrorCode> {
        self.request_with(request, namespace, kernel, |_| Ok(()))
    }

    fn snapshot<K: EffectPort>(&mut self, kernel: &mut K) -> Result<Snapshot, ErrorCode> {
        if let Some(binding) = &self.enrollment {
            binding.validate().map_err(|_| REFUSED)?;
        }
        let marker = self
            .receipts
            .root()
            .map_err(|_| REFUSED)?
            .checked_marker()
            .map_err(|_| REFUSED)?;
        let receipt = self.receipts.read();
        if receipt == ReceiptRead::UnsafeOrUncertain {
            return Err(REFUSED);
        }
        let observed = kernel.observe().map_err(|_| REFUSED)?;
        if self
            .receipts
            .root()
            .map_err(|_| REFUSED)?
            .checked_marker()
            .map_err(|_| REFUSED)?
            != marker
            || self.receipts.read() != receipt
        {
            return Err(REFUSED);
        }
        if let Some(binding) = &self.enrollment {
            binding.validate().map_err(|_| REFUSED)?;
        }
        Ok(Snapshot {
            marker,
            receipt,
            kernel: observed,
        })
    }

    fn admit(&mut self, epoch: HostEpoch, state: Snapshot) -> Result<u32, ErrorCode> {
        let uid = self
            .receipts
            .root()
            .map_err(|_| REFUSED)?
            .receipt_directory()
            .map_err(|_| REFUSED)?
            .2;
        let table = match (state.kernel.table, state.kernel.identity) {
            (Table::Absent, None) => TableObservation::Absent,
            (Table::OwnedVerified(Policy::FullVpn), Some(id)) if identity_in_epoch(id, epoch) => {
                TableObservation::Present {
                    handle: id.table_handle,
                }
            }
            _ => return Err(REFUSED),
        };
        let assessed = receipt::assess(
            uid,
            NamespaceObservation::Canonical(epoch),
            state.receipt,
            table,
        );
        let stable = match (state.marker, state.receipt, assessed) {
            (Marker::Missing, ReceiptRead::Missing, Assessment::AbsenceConsistent) => true,
            (Marker::Closed(_), ReceiptRead::Durable(r), Assessment::AbsenceConsistent) => {
                r.state() == ReceiptState::Retired
            }
            (Marker::Armed(_), ReceiptRead::Durable(r), Assessment::LiveIdentityConsistent) => {
                matches!(r.state(), ReceiptState::Live { .. })
            }
            _ => false,
        };
        if stable { Ok(uid) } else { Err(REFUSED) }
    }

    fn request_with<K: EffectPort>(
        &mut self,
        request: Request,
        namespace: NamespaceObservation,
        kernel: &mut K,
        mut checkpoint: impl FnMut(Point) -> Result<(), ErrorCode>,
    ) -> Result<Response, ErrorCode> {
        if self.poisoned {
            return Err(REFUSED);
        }
        let NamespaceObservation::Canonical(epoch) = namespace else {
            return Err(REFUSED);
        };
        let mut state = self.snapshot(kernel)?;
        let uid = self.admit(epoch, state)?;
        let mut tx = transaction::plan(
            request,
            Observation {
                marker: state.marker,
                table: state.kernel.table,
            },
        )?;
        // Status and an exact completed disarm retry do not allocate or write.
        if tx.next_effect().is_none() {
            if self.snapshot(kernel)? != state {
                self.poisoned = true;
                return Err(REFUSED);
            }
            return tx.response();
        }
        let phase = match (request, state.kernel.table, state.kernel.identity) {
            (Request::Arm { .. }, Table::Absent, None) => ReceiptState::PendingCreate,
            (Request::Arm { .. }, Table::OwnedVerified(_), Some(id)) => {
                ReceiptState::PendingReplace {
                    old_handle: id.table_handle,
                }
            }
            (Request::Disarm { .. }, Table::OwnedVerified(_), Some(id)) => {
                ReceiptState::PendingDelete {
                    old_handle: id.table_handle,
                }
            }
            _ => return Err(REFUSED),
        };
        let operation = match state.receipt {
            ReceiptRead::Missing => 1,
            ReceiptRead::Durable(r) => r.operation().checked_add(1).ok_or(REFUSED)?,
            ReceiptRead::UnsafeOrUncertain => return Err(REFUSED),
        };
        let pending =
            Receipt::transaction_record(uid, epoch, operation, phase).map_err(|_| REFUSED)?;
        let result = (|| {
            checkpoint(Point::BeforePending)?;
            if self.snapshot(kernel)? != state {
                return Err(REFUSED);
            }
            self.receipts
                .publish_with(state.receipt, pending, |boundary| {
                    checkpoint(Point::ReceiptWrite {
                        terminal: false,
                        boundary,
                    })
                    .map_err(|_| StateError::UncertainWrite)
                })
                .map_err(|_| REFUSED)?;
            state.receipt = ReceiptRead::Durable(pending);
            checkpoint(Point::Pending)?;
            while let Some(effect) = tx.next_effect() {
                if self.snapshot(kernel)? != state {
                    return Err(REFUSED);
                }
                match effect {
                    Effect::CreateTableAtomic(policy) | Effect::ReplaceOwnedTableAtomic(policy) => {
                        checkpoint(Point::BeforeKernel)?;
                        let identity = if matches!(effect, Effect::CreateTableAtomic(_)) {
                            kernel.create_if_absent(policy)
                        } else {
                            kernel.replace_owned(state.kernel.identity.ok_or(REFUSED)?, policy)
                        }
                        .map_err(|_| REFUSED)?;
                        checkpoint(Point::KernelReturned)?;
                        let observed = kernel.observe().map_err(|_| REFUSED)?;
                        if !identity_in_epoch(identity, epoch)
                            || observed
                                != (EffectSnapshot {
                                    table: Table::OwnedVerified(policy),
                                    identity: Some(identity),
                                })
                        {
                            return Err(REFUSED);
                        }
                        state.kernel = observed;
                        checkpoint(Point::KernelVerified)?;
                    }
                    Effect::DeleteOwnedTableAtomic => {
                        checkpoint(Point::BeforeKernel)?;
                        kernel
                            .delete_owned(state.kernel.identity.ok_or(REFUSED)?)
                            .map_err(|_| REFUSED)?;
                        checkpoint(Point::KernelReturned)?;
                        let absent = EffectSnapshot {
                            table: Table::Absent,
                            identity: None,
                        };
                        if kernel.observe().map_err(|_| REFUSED)? != absent {
                            return Err(REFUSED);
                        }
                        state.kernel = absent;
                        checkpoint(Point::KernelVerified)?;
                    }
                    Effect::PersistArmedDurably(n) | Effect::PersistClosedDurably(n) => {
                        checkpoint(Point::BeforeMarker)?;
                        let next = if matches!(effect, Effect::PersistArmedDurably(_)) {
                            Marker::Armed(n)
                        } else {
                            Marker::Closed(n)
                        };
                        self.receipts
                            .root()
                            .map_err(|_| REFUSED)?
                            .persist_inner(state.marker, next, |boundary| {
                                checkpoint(Point::MarkerWrite(boundary))
                                    .map_err(|_| StateError::UncertainWrite)
                            })
                            .map_err(|_| REFUSED)?;
                        state.marker = next;
                        checkpoint(Point::Marker)?;
                    }
                    Effect::VerifyTable(policy) => {
                        if state.kernel.table != Table::OwnedVerified(policy) {
                            return Err(REFUSED);
                        }
                    }
                    Effect::VerifyTableAbsent => {
                        if state.kernel.table != Table::Absent || state.kernel.identity.is_some() {
                            return Err(REFUSED);
                        }
                    }
                }
                tx.acknowledge(effect, true)?;
            }
            checkpoint(Point::BeforeTerminal)?;
            if self.snapshot(kernel)? != state {
                return Err(REFUSED);
            }
            // Only this invocation's pending phase may finish. Reopening never
            // enters here with a pending receipt, even after a committed effect.
            let terminal = match (phase, state.kernel.table, state.kernel.identity) {
                (
                    ReceiptState::PendingCreate | ReceiptState::PendingReplace { .. },
                    Table::OwnedVerified(Policy::FullVpn),
                    Some(id),
                ) => ReceiptState::Live {
                    handle: id.table_handle,
                },
                (ReceiptState::PendingDelete { .. }, Table::Absent, None) => ReceiptState::Retired,
                _ => return Err(REFUSED),
            };
            let complete = Receipt::transaction_record(uid, epoch, operation, terminal)
                .map_err(|_| REFUSED)?;
            self.receipts
                .publish_with(state.receipt, complete, |boundary| {
                    checkpoint(Point::ReceiptWrite {
                        terminal: true,
                        boundary,
                    })
                    .map_err(|_| StateError::UncertainWrite)
                })
                .map_err(|_| REFUSED)?;
            state.receipt = ReceiptRead::Durable(complete);
            checkpoint(Point::Terminal)?;
            let response = tx.response()?;
            checkpoint(Point::BeforeReply)?;
            if self.snapshot(kernel)? != state
                || transaction::observe(Observation {
                    marker: state.marker,
                    table: state.kernel.table,
                }) != response
            {
                return Err(REFUSED);
            }
            self.admit(epoch, state)?;
            Ok(response)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}

fn identity_in_epoch(identity: EffectIdentity, epoch: HostEpoch) -> bool {
    identity.boot == epoch.boot
        && identity.netns_inode == epoch.namespace_inode
        && identity.table_handle != 0
}

#[cfg(test)]
mod tests {
    mod kernel_crash {
        include!("locked_state_kernel_crash.rs");
    }

    mod exchange {
        include!("locked_state_exchange_tests.rs");
        include!("locked_state_session_tests.rs");
        include!("locked_state_authority_tests.rs");
    }

    use super::*;
    use crate::effect_port::EffectError;
    use crate::protocol::{Health, Mode, Protection};
    use std::fs::{self, File};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    use std::path::PathBuf;

    const EPOCH: HostEpoch = HostEpoch {
        boot: [1; 16],
        namespace_epoch: [2; 16],
        namespace_device: 3,
        namespace_inode: 4,
    };
    const NS: NamespaceObservation = NamespaceObservation::Canonical(EPOCH);
    const ID: EffectIdentity = EffectIdentity {
        boot: [1; 16],
        netns_inode: 4,
        table_handle: 5,
    };
    const ARM: Request = Request::Arm {
        generation: 7,
        mode: Mode::Full,
    };
    const DISARM: Request = Request::Disarm { generation: 7 };
    const ABSENT: EffectSnapshot = EffectSnapshot {
        table: Table::Absent,
        identity: None,
    };
    const LIVE: EffectSnapshot = EffectSnapshot {
        table: Table::OwnedVerified(Policy::FullVpn),
        identity: Some(ID),
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            // Leave room for run/omavless-netguard/control.sock under a
            // HOME-based TMPDIR; the old descriptive root exceeded SUN_LEN.
            // Exclusive allocation also skips stale files/dirs/symlinks.
            let path = crate::test_temp::directory("k1l").unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path.join("omavless-netguard"))
                .unwrap();
            Self(path)
        }
        fn root(&self) -> Result<RootStateStore, StateError> {
            open_fixture(&self.0)
        }
        fn state(&self) -> LockedState {
            LockedState::from_root(self.root().unwrap())
        }
        fn bytes(&self) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
            let dir = self.0.join("omavless-netguard");
            (
                fs::read(dir.join("armed-v1.json")).ok(),
                fs::read(dir.join("table-receipt-v1.json")).ok(),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn open_fixture(path: &PathBuf) -> Result<RootStateStore, StateError> {
        let metadata = fs::metadata(path).unwrap();
        RootStateStore::open_test_parent(
            File::open(path).unwrap(),
            (metadata.uid(), metadata.gid()),
            1001,
        )
    }

    struct Kernel {
        current: EffectSnapshot,
        parent: PathBuf,
        effects: usize,
        fail_after_effect: bool,
        bad_readback: bool,
        observes: usize,
        drift_at: Option<usize>,
        rebind_enrollment_on_observe: bool,
        rebind_enrollment_on_effect: bool,
    }
    impl Kernel {
        fn new(f: &Fixture) -> Self {
            Self {
                current: ABSENT,
                parent: f.0.clone(),
                effects: 0,
                fail_after_effect: false,
                bad_readback: false,
                observes: 0,
                drift_at: None,
                rebind_enrollment_on_observe: false,
                rebind_enrollment_on_effect: false,
            }
        }
        fn lock_held(&self) {
            assert!(matches!(open_fixture(&self.parent), Err(StateError::Busy)));
        }
        fn finish_effect(&mut self, next: EffectSnapshot) -> Result<(), EffectError> {
            self.lock_held();
            // The pending record is already published while this effect runs.
            let record = receipt::decode(
                &fs::read(self.parent.join("omavless-netguard/table-receipt-v1.json")).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                record.state(),
                ReceiptState::PendingCreate
                    | ReceiptState::PendingReplace { .. }
                    | ReceiptState::PendingDelete { .. }
            ));
            self.effects += 1;
            self.current = next;
            if self.rebind_enrollment_on_effect {
                self.rebind_enrollment_on_effect = false;
                self.rebind_enrollment();
            }
            if self.fail_after_effect {
                Err(EffectError::UnavailableOrUncertain)
            } else {
                Ok(())
            }
        }
        fn rebind_enrollment(&self) {
            let config = self.parent.join("omavless-netguard/enrollment-v1.json");
            fs::rename(
                &config,
                self.parent.join("omavless-netguard/old-enrollment"),
            )
            .unwrap();
            fs::copy(
                self.parent.join("omavless-netguard/old-enrollment"),
                &config,
            )
            .unwrap();
        }
    }
    impl crate::effect_port::sealed::Sealed for Kernel {}
    impl EffectPort for Kernel {
        fn observe(&mut self) -> Result<EffectSnapshot, EffectError> {
            self.lock_held();
            self.observes += 1;
            if self.rebind_enrollment_on_observe {
                self.rebind_enrollment_on_observe = false;
                self.rebind_enrollment();
            }
            if self.drift_at == Some(self.observes) {
                self.current = EffectSnapshot {
                    table: Table::Foreign,
                    identity: None,
                };
            }
            if self.bad_readback && self.effects > 0 {
                return Ok(ABSENT);
            }
            Ok(self.current)
        }
        fn create_if_absent(&mut self, _: Policy) -> Result<EffectIdentity, EffectError> {
            assert_eq!(self.current, ABSENT);
            self.finish_effect(LIVE)?;
            Ok(ID)
        }
        fn replace_owned(
            &mut self,
            identity: EffectIdentity,
            _: Policy,
        ) -> Result<EffectIdentity, EffectError> {
            assert_eq!(self.current.identity, Some(identity));
            let replacement = EffectIdentity {
                table_handle: identity.table_handle + 1,
                ..identity
            };
            self.finish_effect(EffectSnapshot {
                table: Table::OwnedVerified(Policy::FullVpn),
                identity: Some(replacement),
            })?;
            Ok(replacement)
        }
        fn delete_owned(&mut self, identity: EffectIdentity) -> Result<(), EffectError> {
            assert_eq!(self.current.identity, Some(identity));
            // Closed intent is durable before deletion starts.
            let bytes = fs::read(self.parent.join("omavless-netguard/armed-v1.json")).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["armed"], false);
            assert_eq!(value["generation"], 7);
            self.finish_effect(ABSENT)
        }
    }

    #[test]
    fn enrollment_rebinding_during_kernel_observation_refuses_before_effect() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new();
        let config = f.0.join("omavless-netguard/enrollment-v1.json");
        fs::write(&config, b"{\"version\":1,\"enrolled_uid\":1001}").unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        let parent = File::open(&f.0).unwrap();
        let m = parent.metadata().unwrap();
        let binding = EnrollmentBinding::open_test_parent(parent, (m.uid(), m.gid())).unwrap();
        let mut state = f.state();
        state.enrollment = Some(binding);
        let mut kernel = Kernel::new(&f);
        kernel.rebind_enrollment_on_observe = true;
        let before = f.bytes();
        assert_eq!(state.request(ARM, NS, &mut kernel), Err(REFUSED));
        assert_eq!(kernel.effects, 0);
        assert_eq!(f.bytes(), before);
    }

    fn record(state: &LockedState) -> Receipt {
        let ReceiptRead::Durable(record) = state.receipts.read() else {
            panic!("missing receipt")
        };
        record
    }

    #[test]
    fn kernel_success_does_not_publish_a_terminal_receipt() {
        for kind in 0..3 {
            let f = Fixture::new();
            let mut s = f.state();
            let mut k = Kernel::new(&f);
            if kind != 0 {
                s.request(ARM, NS, &mut k).unwrap();
            }
            let request = if kind == 2 { DISARM } else { ARM };
            let expected = match kind {
                0 => ReceiptState::PendingCreate,
                1 => ReceiptState::PendingReplace { old_handle: 5 },
                _ => ReceiptState::PendingDelete { old_handle: 5 },
            };
            let mut returned = false;
            s.request_with(request, NS, &mut k, |point| {
                if point == Point::KernelReturned {
                    returned = true;
                    let bytes = f.bytes().1.unwrap();
                    assert_eq!(receipt::decode(&bytes).unwrap().state(), expected);
                    assert!(matches!(f.root(), Err(StateError::Busy)));
                    // A successful kernel return cannot finish the operation:
                    // independent readback and marker persistence still follow.
                    return Err(REFUSED);
                }
                Ok(())
            })
            .unwrap_err();
            assert!(returned);
            assert_eq!(record(&s).state(), expected);
            drop(s);
            let bytes = f.bytes();
            let effects = k.effects;
            assert_eq!(f.state().request(request, NS, &mut k), Err(REFUSED));
            assert_eq!(f.bytes(), bytes);
            assert_eq!(k.effects, effects);
        }
    }

    #[test]
    fn single_lock_complete_sequence_reopen_and_independent_fences() {
        let f = Fixture::new();
        let mut s = f.state();
        let mut k = Kernel::new(&f);
        assert!(matches!(
            s.request(ARM, NS, &mut k),
            Ok(Response::Status {
                protection: Protection::Armed { generation: 7 },
                health: Health::Verified,
                ..
            })
        ));
        assert_eq!(
            (record(&s).operation(), record(&s).state()),
            (1, ReceiptState::Live { handle: 5 })
        );
        s.request(ARM, NS, &mut k).unwrap();
        assert_eq!(
            (record(&s).operation(), record(&s).state()),
            (2, ReceiptState::Live { handle: 6 })
        );
        s.request(DISARM, NS, &mut k).unwrap();
        assert_eq!(
            (record(&s).operation(), record(&s).state()),
            (3, ReceiptState::Retired)
        );
        let bytes = f.bytes();
        s.request(DISARM, NS, &mut k).unwrap();
        s.request(Request::Status {}, NS, &mut k).unwrap();
        assert_eq!(f.bytes(), bytes);
        assert_eq!(
            s.request(ARM, NS, &mut k),
            Err(ErrorCode::GenerationConflict)
        );
        drop(s);
        let mut reopened = f.state();
        assert_eq!(
            reopened.request(ARM, NS, &mut k),
            Err(ErrorCode::GenerationConflict)
        );
        reopened
            .request(
                Request::Arm {
                    generation: 8,
                    mode: Mode::Full,
                },
                NS,
                &mut k,
            )
            .unwrap();
        assert_eq!(record(&reopened).operation(), 4);
        assert_eq!(reopened.receipts.root().unwrap().marker(), Marker::Armed(8));
    }

    #[test]
    fn every_transaction_boundary_refuses_uncertain_success_and_pending_restart() {
        let points = [
            Point::BeforePending,
            Point::Pending,
            Point::BeforeKernel,
            Point::KernelReturned,
            Point::KernelVerified,
            Point::BeforeMarker,
            Point::Marker,
            Point::BeforeTerminal,
            Point::Terminal,
            Point::BeforeReply,
        ];
        for request in [ARM, DISARM] {
            for point in points {
                let f = Fixture::new();
                let mut s = f.state();
                let mut k = Kernel::new(&f);
                if request == DISARM {
                    s.request(ARM, NS, &mut k).unwrap();
                }
                let baseline = f.bytes();
                assert_eq!(
                    s.request_with(request, NS, &mut k, |seen| {
                        assert!(matches!(f.root(), Err(StateError::Busy)));
                        if seen == point { Err(REFUSED) } else { Ok(()) }
                    }),
                    Err(REFUSED),
                    "{request:?} {point:?}"
                );
                assert_eq!(s.request(Request::Status {}, NS, &mut k), Err(REFUSED));
                let failed_bytes = f.bytes();
                drop(s);
                let mut reopened = f.state();
                let effects = k.effects;
                if point == Point::BeforePending {
                    assert_eq!(failed_bytes, baseline);
                    reopened.request(Request::Status {}, NS, &mut k).unwrap();
                } else if matches!(point, Point::Terminal | Point::BeforeReply) {
                    // A fully durable terminal state is observable, but cannot
                    // prove the interrupted invocation received its reply.
                    reopened.request(Request::Status {}, NS, &mut k).unwrap();
                } else {
                    assert_eq!(
                        reopened.request(Request::Status {}, NS, &mut k),
                        Err(REFUSED)
                    );
                    assert_eq!(reopened.request(request, NS, &mut k), Err(REFUSED));
                }
                assert_eq!(k.effects, effects);
                assert_eq!(f.bytes(), failed_bytes);
            }
        }
    }

    #[test]
    fn lost_kernel_reply_bad_readback_and_changed_observation_keep_pending() {
        for kind in 0..3 {
            let f = Fixture::new();
            let mut s = f.state();
            let mut k = Kernel::new(&f);
            k.fail_after_effect = kind == 0;
            k.bad_readback = kind == 1;
            // Drift before the first actual kernel mutation, after Pending.
            k.drift_at = (kind == 2).then_some(3);
            assert_eq!(s.request(ARM, NS, &mut k), Err(REFUSED));
            assert_eq!(record(&s).state(), ReceiptState::PendingCreate);
            assert_eq!(s.receipts.root().unwrap().marker(), Marker::Missing);
            assert_eq!(k.effects, usize::from(kind != 2));
        }
    }

    #[test]
    fn no_receipt_or_namespace_or_orphan_claim_is_ownership() {
        for kind in 0..7 {
            let f = Fixture::new();
            let mut s = f.state();
            let mut k = Kernel::new(&f);
            if kind != 0 {
                s.request(ARM, NS, &mut k).unwrap();
            }
            let namespace = match kind {
                1 => NamespaceObservation::Unproven,
                2 => NamespaceObservation::Canonical(HostEpoch {
                    namespace_epoch: [3; 16],
                    ..EPOCH
                }),
                3 => NamespaceObservation::Canonical(HostEpoch {
                    boot: [4; 16],
                    ..EPOCH
                }),
                _ => NS,
            };
            match kind {
                0 | 4 => {
                    k.current = EffectSnapshot {
                        table: Table::Foreign,
                        identity: None,
                    }
                }
                5 => k.current.identity = None,
                6 => k.current = ABSENT,
                _ => {}
            }
            let bytes = f.bytes();
            let effects = k.effects;
            assert_eq!(s.request(ARM, namespace, &mut k), Err(REFUSED));
            assert_eq!(f.bytes(), bytes);
            assert_eq!(k.effects, effects);
        }
    }

    #[test]
    fn operation_exhaustion_and_cross_record_mismatch_do_not_write() {
        for kind in 0..4 {
            let f = Fixture::new();
            let mut s = f.state();
            let mut k = Kernel::new(&f);
            if kind < 3 {
                s.request(ARM, NS, &mut k).unwrap();
            }
            let next = match kind {
                0 => Receipt::transaction_record(
                    1001,
                    EPOCH,
                    u64::MAX,
                    ReceiptState::Live { handle: 5 },
                )
                .unwrap(),
                1 => Receipt::transaction_record(
                    1001,
                    EPOCH,
                    2,
                    ReceiptState::PendingReplace { old_handle: 5 },
                )
                .unwrap(),
                2 => Receipt::transaction_record(1001, EPOCH, 2, ReceiptState::Retired).unwrap(),
                _ => Receipt::transaction_record(1001, EPOCH, 1, ReceiptState::Live { handle: 5 })
                    .unwrap(),
            };
            s.receipts.publish(s.receipts.read(), next).unwrap();
            if kind == 3 {
                k.current = LIVE;
            }
            let bytes = f.bytes();
            let effects = k.effects;
            assert_eq!(s.request(ARM, NS, &mut k), Err(REFUSED));
            assert_eq!(f.bytes(), bytes);
            assert_eq!(k.effects, effects);
        }
    }

    #[test]
    fn marker_write_failure_never_removes_protection() {
        let f = Fixture::new();
        let mut s = f.state();
        let mut k = Kernel::new(&f);
        s.request(ARM, NS, &mut k).unwrap();
        assert_eq!(
            s.request_with(DISARM, NS, &mut k, |point| {
                if point == Point::BeforeMarker {
                    fs::create_dir(f.0.join("omavless-netguard/.armed-v1.json.next")).unwrap();
                }
                Ok(())
            }),
            Err(REFUSED)
        );
        assert_eq!(k.current, LIVE);
        assert_eq!(k.effects, 1);
        assert_eq!(
            record(&s).state(),
            ReceiptState::PendingDelete { old_handle: 5 }
        );
    }

    #[test]
    fn every_composed_storage_fault_preserves_restart_barriers() {
        let points: Vec<_> = [false, true]
            .into_iter()
            .flat_map(|terminal| {
                (0..=10).map(move |boundary| Point::ReceiptWrite { terminal, boundary })
            })
            .chain((0..=5).map(Point::MarkerWrite))
            .collect();
        for kind in 0..3 {
            for point in &points {
                let f = Fixture::new();
                let mut s = f.state();
                let mut k = Kernel::new(&f);
                if kind != 0 {
                    s.request(ARM, NS, &mut k).unwrap();
                }
                let request = if kind == 2 { DISARM } else { ARM };
                let effects = k.effects;
                assert_eq!(
                    s.request_with(request, NS, &mut k, |seen| {
                        assert!(matches!(f.root(), Err(StateError::Busy)));
                        if seen == *point { Err(REFUSED) } else { Ok(()) }
                    }),
                    Err(REFUSED),
                    "{kind} {point:?}"
                );
                assert_eq!(s.request(Request::Status {}, NS, &mut k), Err(REFUSED));
                if matches!(
                    point,
                    Point::ReceiptWrite {
                        terminal: false,
                        ..
                    }
                ) || (kind == 2 && matches!(point, Point::MarkerWrite(_)))
                {
                    assert_eq!(k.effects, effects);
                }
                let bytes = f.bytes();
                let effects_after = k.effects;
                drop(s);
                let mut reopened = f.state();
                let status = reopened.request(Request::Status {}, NS, &mut k);
                // Receipt boundary zero precedes even the guard; terminal 9/10
                // follows durable terminal publication and guard removal.
                let stable = matches!(
                    point,
                    Point::ReceiptWrite {
                        terminal: false,
                        boundary: 0
                    } | Point::ReceiptWrite {
                        terminal: true,
                        boundary: 9 | 10
                    }
                );
                assert_eq!(status.is_ok(), stable, "{kind} {point:?}");
                if !stable {
                    assert_eq!(reopened.request(request, NS, &mut k), Err(REFUSED));
                }
                assert_eq!(k.effects, effects_after);
                assert_eq!(f.bytes(), bytes);
            }
        }
    }

    // Real process death complements returned-error injection: no destructor,
    // poisoning assignment or cleanup runs in the interrupted writer. The
    // kernel below is still a model, never Linux ownership evidence.
    fn crash_points() -> Vec<Point> {
        vec![
            Point::BeforePending,
            Point::Pending,
            Point::BeforeKernel,
            Point::KernelReturned,
            Point::KernelVerified,
            Point::BeforeMarker,
            Point::Marker,
            Point::BeforeTerminal,
            Point::Terminal,
            Point::BeforeReply,
        ]
        .into_iter()
        .chain([false, true].into_iter().flat_map(|terminal| {
            (0..=10).map(move |boundary| Point::ReceiptWrite { terminal, boundary })
        }))
        .chain((0..=5).map(Point::MarkerWrite))
        .collect()
    }

    #[test]
    fn process_crash_child() {
        let Some(parent) = std::env::var_os("OMAVLESS_K1_CRASH_FIXTURE") else {
            return;
        };
        // Only the parent owns fixture cleanup, including on child failure.
        let f = std::mem::ManuallyDrop::new(Fixture(PathBuf::from(parent)));
        let kind: u8 = std::env::var("OMAVLESS_K1_CRASH_KIND")
            .unwrap()
            .parse()
            .unwrap();
        assert!(kind < 3);
        let index: usize = std::env::var("OMAVLESS_K1_CRASH_POINT")
            .unwrap()
            .parse()
            .unwrap();
        let target = crash_points()[index];
        let mut s = f.state();
        let mut k = Kernel::new(&f);
        if kind != 0 {
            k.current = LIVE;
        }
        let request = if kind == 2 { DISARM } else { ARM };
        let _ = s.request_with(request, NS, &mut k, |point| {
            if point == target {
                fs::write(f.0.join("ready"), b"checkpoint").unwrap();
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        });
        panic!("crash checkpoint not reached");
    }

    #[test]
    fn sigkill_releases_lock_but_never_completes_pending_transactions() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::{Child, Command, Stdio};
        use std::time::{Duration, Instant};

        struct ChildGuard(Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let publication_files = |f: &Fixture| {
            fs::read_dir(f.0.join("omavless-netguard"))
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), fs::read(entry.path()).unwrap())
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        };

        for kind in 0..3 {
            for (index, point) in crash_points().into_iter().enumerate() {
                let f = Fixture::new();
                let mut k = Kernel::new(&f);
                if kind != 0 {
                    f.state().request(ARM, NS, &mut k).unwrap();
                }
                let mut child = ChildGuard(
                    Command::new(std::env::current_exe().unwrap())
                        .args(["--exact", "locked_state::tests::process_crash_child"])
                        .env("OMAVLESS_K1_CRASH_FIXTURE", &f.0)
                        .env("OMAVLESS_K1_CRASH_KIND", kind.to_string())
                        .env("OMAVLESS_K1_CRASH_POINT", index.to_string())
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .unwrap(),
                );
                let deadline = Instant::now() + Duration::from_secs(10);
                while !f.0.join("ready").exists() {
                    assert!(child.0.try_wait().unwrap().is_none(), "{kind} {point:?}");
                    assert!(Instant::now() < deadline, "checkpoint timeout");
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(matches!(f.root(), Err(StateError::Busy)));
                child.0.kill().unwrap();
                assert_eq!(child.0.wait().unwrap().signal(), Some(9));

                // Reconstruct only the synthetic effect state from the selected
                // boundary. This is not a persistent kernel identity provider.
                let after_kernel = matches!(
                    point,
                    Point::KernelReturned
                        | Point::KernelVerified
                        | Point::BeforeTerminal
                        | Point::Terminal
                        | Point::BeforeReply
                        | Point::ReceiptWrite { terminal: true, .. }
                ) || (kind != 2
                    && matches!(
                        point,
                        Point::BeforeMarker | Point::Marker | Point::MarkerWrite(_)
                    ));
                if after_kernel {
                    k.current = match kind {
                        0 => LIVE,
                        1 => EffectSnapshot {
                            table: Table::OwnedVerified(Policy::FullVpn),
                            identity: Some(EffectIdentity {
                                table_handle: 6,
                                ..ID
                            }),
                        },
                        _ => ABSENT,
                    };
                }
                let bytes = publication_files(&f);
                let effects = k.effects;
                let mut reopened = f.state();
                let stable = matches!(
                    point,
                    Point::BeforePending
                        | Point::Terminal
                        | Point::BeforeReply
                        | Point::ReceiptWrite {
                            terminal: false,
                            boundary: 0
                        }
                        | Point::ReceiptWrite {
                            terminal: true,
                            boundary: 9 | 10
                        }
                );
                assert_eq!(
                    reopened.request(Request::Status {}, NS, &mut k).is_ok(),
                    stable,
                    "{kind} {point:?}"
                );
                if !stable {
                    for request in [ARM, DISARM] {
                        assert_eq!(reopened.request(request, NS, &mut k), Err(REFUSED));
                    }
                }
                assert_eq!(k.effects, effects);
                assert_eq!(publication_files(&f), bytes);
                // A completed delete retains its generation fence even when
                // the writer dies before delivering its success response.
                if kind == 2 && after_kernel && stable {
                    assert_eq!(
                        reopened.request(ARM, NS, &mut k),
                        Err(ErrorCode::GenerationConflict)
                    );
                }
            }
        }
    }
}
