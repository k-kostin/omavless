//! Inactive K1 transaction composition. No production caller, service, socket,
//! nft invocation or privileged adapter exists. The kernel port is a contract
//! for a future independently reviewed root implementation, not ownership proof.

use crate::nft::TrustedTableIdentity;
use crate::policy::Policy;
use crate::protocol::{ErrorCode, Request, Response};
use crate::root_state::RootStateStore;
use crate::transaction::{self, Effect, Marker, Observation, Table, Transaction};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelSnapshot {
    pub table: Table,
    /// Must come from an independently durable root receipt bound to the
    /// current boot, netns and table handle. The table name is not a receipt.
    pub identity: Option<TrustedTableIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelError {
    UnavailableOrUncertain,
}

impl KernelSnapshot {
    fn coherent(self) -> bool {
        match self.table {
            Table::Absent | Table::Foreign | Table::Unreadable => self.identity.is_none(),
            Table::OwnedVerified(_) | Table::OwnedUnrecognized => self.identity.is_some(),
        }
    }
}

/// The future root-only implementation must make each mutation conditional on
/// the supplied exact identity (or verified absence), atomically publish its
/// independent ownership receipt, and report uncertainty as an error. A bare
/// successful command exit is never sufficient; the coordinator re-observes.
pub trait KernelPort {
    fn observe(&mut self) -> Result<KernelSnapshot, KernelError>;
    fn create_if_absent(&mut self, policy: Policy) -> Result<TrustedTableIdentity, KernelError>;
    fn replace_owned(
        &mut self,
        identity: TrustedTableIdentity,
        policy: Policy,
    ) -> Result<TrustedTableIdentity, KernelError>;
    fn delete_owned(&mut self, identity: TrustedTableIdentity) -> Result<(), KernelError>;
}

/// Retains the exclusive durable-store lock from first observation through
/// every effect and final readback. An uncertain effect poisons this instance;
/// restart must re-open and reconcile from independently proven facts.
pub struct Coordinator<'a, K: KernelPort> {
    store: &'a mut RootStateStore,
    kernel: &'a mut K,
    poisoned: bool,
}

impl<'a, K: KernelPort> Coordinator<'a, K> {
    pub fn new(store: &'a mut RootStateStore, kernel: &'a mut K) -> Self {
        Self {
            store,
            kernel,
            poisoned: false,
        }
    }

    pub fn request(&mut self, request: Request) -> Result<Response, ErrorCode> {
        if self.poisoned {
            return Err(ErrorCode::ManualRecoveryRequired);
        }
        let initial = self.snapshot()?;
        let tx = transaction::plan(request, initial.0)?;
        self.run(tx, initial)
    }

    /// Root/internal only; never a peer-selectable wire operation.
    pub fn reconcile(&mut self) -> Result<Response, ErrorCode> {
        if self.poisoned {
            return Err(ErrorCode::ManualRecoveryRequired);
        }
        let initial = self.snapshot()?;
        let tx = transaction::reconcile(initial.0)?;
        self.run(tx, initial)
    }

    fn snapshot(&mut self) -> Result<(Observation, KernelSnapshot), ErrorCode> {
        let marker = self
            .store
            .checked_marker()
            .map_err(|_| ErrorCode::ManualRecoveryRequired)?;
        let table = self
            .kernel
            .observe()
            .map_err(|_| ErrorCode::ManualRecoveryRequired)?;
        // Observation may block. Reprove that the held store is still bound
        // and unchanged before any effect or final success response.
        if self.store.checked_marker() != Ok(marker) {
            return Err(ErrorCode::ManualRecoveryRequired);
        }
        if !table.coherent() {
            return Err(ErrorCode::ManualRecoveryRequired);
        }
        Ok((
            Observation {
                marker,
                table: table.table,
            },
            table,
        ))
    }

    fn run(
        &mut self,
        mut tx: Transaction,
        mut state: (Observation, KernelSnapshot),
    ) -> Result<Response, ErrorCode> {
        let result = (|| {
            while let Some(effect) = tx.next_effect() {
                // No effect may rely on a table that changed since the last
                // independently proven snapshot, even while our store is locked.
                if self.snapshot()? != state {
                    return Err(ErrorCode::ManualRecoveryRequired);
                }
                self.apply(effect, &mut state)?;
                tx.acknowledge(effect, true)?;
            }
            let result = tx.response()?;
            if self.snapshot()? != state || result != transaction::observe(state.0) {
                return Err(ErrorCode::ManualRecoveryRequired);
            }
            Ok(result)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn apply(
        &mut self,
        effect: Effect,
        state: &mut (Observation, KernelSnapshot),
    ) -> Result<(), ErrorCode> {
        use ErrorCode::ManualRecoveryRequired as Refused;
        match effect {
            Effect::CreateTableAtomic(policy) => {
                if state.1.table != Table::Absent {
                    return Err(Refused);
                }
                let identity = self.kernel.create_if_absent(policy).map_err(|_| Refused)?;
                state.1 = self.owned_readback(policy, identity)?;
                state.0.table = state.1.table;
            }
            Effect::ReplaceOwnedTableAtomic(policy) => {
                let identity = state.1.identity.ok_or(Refused)?;
                let replacement = self
                    .kernel
                    .replace_owned(identity, policy)
                    .map_err(|_| Refused)?;
                state.1 = self.owned_readback(policy, replacement)?;
                state.0.table = state.1.table;
            }
            Effect::VerifyTable(policy) => {
                if state.1.table != Table::OwnedVerified(policy) {
                    return Err(Refused);
                }
            }
            Effect::PersistArmedDurably(generation) => {
                let next = Marker::Armed(generation);
                self.store
                    .persist(state.0.marker, next)
                    .map_err(|_| Refused)?;
                if self.store.marker() != next {
                    return Err(Refused);
                }
                state.0.marker = next;
            }
            Effect::PersistClosedDurably(generation) => {
                let next = Marker::Closed(generation);
                self.store
                    .persist(state.0.marker, next)
                    .map_err(|_| Refused)?;
                if self.store.marker() != next {
                    return Err(Refused);
                }
                state.0.marker = next;
            }
            Effect::DeleteOwnedTableAtomic => {
                let identity = state.1.identity.ok_or(Refused)?;
                self.kernel.delete_owned(identity).map_err(|_| Refused)?;
                let absent = self.kernel.observe().map_err(|_| Refused)?;
                if absent.table != Table::Absent || !absent.coherent() {
                    return Err(Refused);
                }
                state.1 = absent;
                state.0.table = Table::Absent;
            }
            Effect::VerifyTableAbsent => {
                if state.1.table != Table::Absent {
                    return Err(Refused);
                }
            }
        }
        Ok(())
    }

    fn owned_readback(
        &mut self,
        policy: Policy,
        expected_identity: TrustedTableIdentity,
    ) -> Result<KernelSnapshot, ErrorCode> {
        let observed = self
            .kernel
            .observe()
            .map_err(|_| ErrorCode::ManualRecoveryRequired)?;
        if !observed.coherent()
            || observed.table != Table::OwnedVerified(policy)
            || observed.identity != Some(expected_identity)
        {
            return Err(ErrorCode::ManualRecoveryRequired);
        }
        Ok(observed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Health, Mode, POLICY_VERSION, Protection};
    use crate::root_state::StateError;
    use std::fs::{self, File};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ID: TrustedTableIdentity = TrustedTableIdentity {
        boot: [1; 16],
        netns_inode: 2,
        table_handle: 3,
    };
    const OTHER_ID: TrustedTableIdentity = TrustedTableIdentity {
        boot: [1; 16],
        netns_inode: 2,
        table_handle: 4,
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let parent = std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let path = parent.join(format!(
                "omavless-netguard-coordinator-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path.join("omavless-netguard"))
                .unwrap();
            Self(path)
        }

        fn open(&self) -> Result<RootStateStore, StateError> {
            let parent = File::open(&self.0).unwrap();
            let m = parent.metadata().unwrap();
            RootStateStore::open_test_parent(parent, (m.uid(), m.gid()), 1001)
        }

        fn stage(&self) -> PathBuf {
            self.0.join("omavless-netguard/.armed-v1.json.next")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    enum Injection {
        Stage(PathBuf),
        Rebind(PathBuf),
        Drift(KernelSnapshot),
    }

    struct FakeKernel {
        current: KernelSnapshot,
        observes: usize,
        injected_at: Option<(usize, Injection)>,
        creates: usize,
        replaces: usize,
        deletes: usize,
        refuse_create: bool,
    }

    impl FakeKernel {
        fn absent() -> Self {
            Self {
                current: KernelSnapshot {
                    table: Table::Absent,
                    identity: None,
                },
                observes: 0,
                injected_at: None,
                creates: 0,
                replaces: 0,
                deletes: 0,
                refuse_create: false,
            }
        }
        fn owned(policy: Policy) -> Self {
            let mut out = Self::absent();
            out.current = KernelSnapshot {
                table: Table::OwnedVerified(policy),
                identity: Some(ID),
            };
            out
        }
    }

    impl KernelPort for FakeKernel {
        fn observe(&mut self) -> Result<KernelSnapshot, KernelError> {
            self.observes += 1;
            if self.injected_at.as_ref().map(|v| v.0) == Some(self.observes) {
                match self.injected_at.take().unwrap().1 {
                    Injection::Stage(path) => fs::write(path, b"").unwrap(),
                    Injection::Rebind(parent) => {
                        fs::rename(
                            parent.join("omavless-netguard"),
                            parent.join("old-directory"),
                        )
                        .unwrap();
                        fs::DirBuilder::new()
                            .mode(0o700)
                            .create(parent.join("omavless-netguard"))
                            .unwrap();
                    }
                    Injection::Drift(snapshot) => self.current = snapshot,
                }
            }
            Ok(self.current)
        }

        fn create_if_absent(
            &mut self,
            policy: Policy,
        ) -> Result<TrustedTableIdentity, KernelError> {
            self.creates += 1;
            if self.refuse_create || self.current.table != Table::Absent {
                return Err(KernelError::UnavailableOrUncertain);
            }
            self.current = KernelSnapshot {
                table: Table::OwnedVerified(policy),
                identity: Some(ID),
            };
            Ok(ID)
        }

        fn replace_owned(
            &mut self,
            identity: TrustedTableIdentity,
            policy: Policy,
        ) -> Result<TrustedTableIdentity, KernelError> {
            self.replaces += 1;
            if self.current.identity != Some(identity) {
                return Err(KernelError::UnavailableOrUncertain);
            }
            self.current = KernelSnapshot {
                table: Table::OwnedVerified(policy),
                identity: Some(OTHER_ID),
            };
            Ok(OTHER_ID)
        }

        fn delete_owned(&mut self, identity: TrustedTableIdentity) -> Result<(), KernelError> {
            self.deletes += 1;
            if self.current.identity != Some(identity) {
                return Err(KernelError::UnavailableOrUncertain);
            }
            self.current = KernelSnapshot {
                table: Table::Absent,
                identity: None,
            };
            Ok(())
        }
    }

    fn arm(generation: u64) -> Request {
        Request::Arm {
            generation,
            mode: Mode::Full,
        }
    }

    #[test]
    fn durable_arm_disarm_and_closed_generation_fence() {
        let fixture = Fixture::new();
        let mut store = fixture.open().unwrap();
        let mut kernel = FakeKernel::absent();
        {
            let mut coordinator = Coordinator::new(&mut store, &mut kernel);
            assert_eq!(
                coordinator.request(arm(7)),
                Ok(Response::Status {
                    policy_version: POLICY_VERSION,
                    protection: Protection::Armed { generation: 7 },
                    health: Health::Verified,
                })
            );
            assert_eq!(
                coordinator.request(Request::Disarm { generation: 7 }),
                Ok(Response::Status {
                    policy_version: POLICY_VERSION,
                    protection: Protection::Disarmed {},
                    health: Health::Verified,
                })
            );
        }
        assert_eq!(kernel.creates, 1);
        assert_eq!(kernel.deletes, 1);
        assert_eq!(store.marker(), Marker::Closed(7));
        assert!(matches!(fixture.open(), Err(StateError::Busy)));
        drop(store);
        let mut store = fixture.open().unwrap();
        let mut coordinator = Coordinator::new(&mut store, &mut kernel);
        assert_eq!(
            coordinator.request(arm(7)),
            Err(ErrorCode::GenerationConflict)
        );
        assert!(coordinator.request(arm(8)).is_ok());
        assert_eq!(store.marker(), Marker::Armed(8));
    }

    #[test]
    fn changed_storage_or_ownership_before_closed_never_delete() {
        for inject_stage in [true, false] {
            let fixture = Fixture::new();
            let mut store = fixture.open().unwrap();
            store.persist(Marker::Missing, Marker::Armed(9)).unwrap();
            let mut kernel = FakeKernel::owned(Policy::FullVpn);
            kernel.injected_at = Some((
                2,
                if inject_stage {
                    Injection::Stage(fixture.stage())
                } else {
                    Injection::Drift(KernelSnapshot {
                        table: Table::Foreign,
                        identity: None,
                    })
                },
            ));
            let mut coordinator = Coordinator::new(&mut store, &mut kernel);
            assert_eq!(
                coordinator.request(Request::Disarm { generation: 9 }),
                Err(ErrorCode::ManualRecoveryRequired)
            );
            assert_eq!(
                coordinator.request(Request::Status {}),
                Err(ErrorCode::ManualRecoveryRequired)
            );
            assert_eq!(kernel.deletes, 0);
        }
    }

    #[test]
    fn identity_races_refuse_replace_and_delete_at_the_effect_boundary() {
        for delete in [false, true] {
            let fixture = Fixture::new();
            let mut store = fixture.open().unwrap();
            store.persist(Marker::Missing, Marker::Armed(9)).unwrap();
            let mut kernel = FakeKernel::owned(Policy::FullVpn);
            if !delete {
                kernel.current.table = Table::OwnedUnrecognized;
            }
            kernel.injected_at = Some((
                if delete { 3 } else { 2 },
                Injection::Drift(KernelSnapshot {
                    table: kernel.current.table,
                    identity: Some(OTHER_ID),
                }),
            ));
            let result = if delete {
                Coordinator::new(&mut store, &mut kernel).request(Request::Disarm { generation: 9 })
            } else {
                Coordinator::new(&mut store, &mut kernel).reconcile()
            };
            assert_eq!(result, Err(ErrorCode::ManualRecoveryRequired));
            assert_eq!(kernel.replaces, 0);
            assert_eq!(kernel.deletes, 0);
            assert_eq!(
                store.marker(),
                if delete {
                    Marker::Closed(9)
                } else {
                    Marker::Armed(9)
                }
            );
        }
    }

    #[test]
    fn create_failure_or_missing_receipt_never_acknowledges_armed() {
        for no_receipt in [true, false] {
            let fixture = Fixture::new();
            let mut store = fixture.open().unwrap();
            let mut kernel = FakeKernel::absent();
            kernel.refuse_create = !no_receipt;
            if no_receipt {
                kernel.injected_at = Some((
                    3,
                    Injection::Drift(KernelSnapshot {
                        table: Table::Foreign,
                        identity: None,
                    }),
                ));
            }
            let mut coordinator = Coordinator::new(&mut store, &mut kernel);
            assert_eq!(
                coordinator.request(arm(2)),
                Err(ErrorCode::ManualRecoveryRequired)
            );
            assert_eq!(store.marker(), Marker::Missing);
        }
    }

    #[test]
    fn final_readback_drift_refuses_success_and_poisons_session() {
        let fixture = Fixture::new();
        let mut store = fixture.open().unwrap();
        store.persist(Marker::Missing, Marker::Armed(3)).unwrap();
        let mut kernel = FakeKernel::owned(Policy::FullVpn);
        // A status has no effect; its second observation is still mandatory.
        kernel.injected_at = Some((
            2,
            Injection::Drift(KernelSnapshot {
                table: Table::Foreign,
                identity: None,
            }),
        ));
        let mut coordinator = Coordinator::new(&mut store, &mut kernel);
        assert_eq!(
            coordinator.request(Request::Status {}),
            Err(ErrorCode::ManualRecoveryRequired)
        );
        assert_eq!(
            coordinator.reconcile(),
            Err(ErrorCode::ManualRecoveryRequired)
        );
        assert_eq!(kernel.deletes, 0);
    }

    #[test]
    fn invalid_marker_emergency_requires_independent_table_ownership() {
        let fixture = Fixture::new();
        let path = fixture.0.join("omavless-netguard/armed-v1.json");
        fs::write(&path, b"{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut store = fixture.open().unwrap();
        assert_eq!(store.marker(), Marker::Invalid);
        let mut foreign = FakeKernel::absent();
        foreign.current.table = Table::Foreign;
        assert_eq!(
            Coordinator::new(&mut store, &mut foreign).reconcile(),
            Err(ErrorCode::ManualRecoveryRequired)
        );
        let mut absent = FakeKernel::absent();
        assert_eq!(
            Coordinator::new(&mut store, &mut absent).reconcile(),
            Ok(Response::Status {
                policy_version: POLICY_VERSION,
                protection: Protection::Emergency {},
                health: Health::ManualRecoveryRequired,
            })
        );
        assert_eq!(store.marker(), Marker::Invalid);
        assert_eq!(
            absent.current.table,
            Table::OwnedVerified(Policy::Emergency)
        );
    }

    #[test]
    fn unsafe_storage_or_lost_lock_never_authorizes_emergency_mutation() {
        for rebind in [false, true] {
            let fixture = Fixture::new();
            let mut store = fixture.open().unwrap();
            if rebind {
                fs::rename(
                    fixture.0.join("omavless-netguard"),
                    fixture.0.join("old-directory"),
                )
                .unwrap();
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(fixture.0.join("omavless-netguard"))
                    .unwrap();
            } else {
                fs::write(fixture.stage(), b"").unwrap();
            }
            let mut kernel = FakeKernel::absent();
            assert_eq!(
                Coordinator::new(&mut store, &mut kernel).reconcile(),
                Err(ErrorCode::ManualRecoveryRequired)
            );
            assert_eq!(kernel.creates, 0);
        }
    }

    #[test]
    fn store_change_during_kernel_observation_refuses_effect_or_final_success() {
        for (status, rebind, final_arm) in [
            (false, false, false),
            (false, true, false),
            (true, false, false),
            (false, false, true),
        ] {
            let fixture = Fixture::new();
            let mut store = fixture.open().unwrap();
            let mut kernel = FakeKernel::absent();
            kernel.injected_at = Some((
                if final_arm { 6 } else { 2 },
                if rebind {
                    Injection::Rebind(fixture.0.clone())
                } else {
                    Injection::Stage(fixture.stage())
                },
            ));
            let result = if status {
                Coordinator::new(&mut store, &mut kernel).request(Request::Status {})
            } else {
                Coordinator::new(&mut store, &mut kernel).request(arm(6))
            };
            assert_eq!(result, Err(ErrorCode::ManualRecoveryRequired));
            assert_eq!(kernel.creates, usize::from(final_arm));
        }
    }
}
