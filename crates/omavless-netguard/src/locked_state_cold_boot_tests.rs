use super::*;
use crate::locked_state::ColdBootPort;

const OLD: HostEpoch = HostEpoch { boot: [8; 16], ..EPOCH };
struct EarlyKernel {
    inner: Kernel,
    begins: usize,
    fences: usize,
    refuse_begin: bool,
    refuse_fence: Option<usize>,
}
impl EarlyKernel {
    fn new(f: &Fixture) -> Self {
        Self { inner: Kernel::new(f), begins: 0, fences: 0,
            refuse_begin: false, refuse_fence: None }
    }
}
impl crate::effect_port::sealed::Sealed for EarlyKernel {}
impl EffectPort for EarlyKernel {
    fn observe(&mut self) -> Result<EffectSnapshot, EffectError> { self.inner.observe() }
    fn create_if_absent(&mut self, policy: Policy) -> Result<EffectIdentity, EffectError> {
        assert_eq!(self.begins, 1);
        assert_eq!(policy, Policy::FullVpn);
        self.inner.create_if_absent(policy)
    }
    fn replace_owned(&mut self, _: EffectIdentity, _: Policy) -> Result<EffectIdentity, EffectError> {
        panic!("cold startup must never replace or adopt an old handle")
    }
    fn delete_owned(&mut self, _: EffectIdentity) -> Result<(), EffectError> {
        panic!("cold startup must never delete or manufacture Closed")
    }
}
impl ColdBootPort for EarlyKernel {
    fn begin_cold_create(&mut self) -> Result<(), EffectError> {
        self.begins += 1;
        assert_eq!(self.begins, 1);
        if self.refuse_begin { Err(EffectError::UnavailableOrUncertain) } else { Ok(()) }
    }
    fn cold_fence(&mut self) -> Result<(), EffectError> {
        self.inner.lock_held();
        self.fences += 1;
        if self.refuse_fence == Some(self.fences) {
            Err(EffectError::UnavailableOrUncertain)
        } else { Ok(()) }
    }
}
fn seeded(f: &Fixture, marker: Marker, epoch: HostEpoch, operation: u64, phase: ReceiptState) -> LockedState {
    let mut s = f.state();
    match marker {
        Marker::Armed(_) => s.receipts.root().unwrap().persist(Marker::Missing, marker).unwrap(),
        Marker::Closed(n) => {
            s.receipts.root().unwrap().persist(Marker::Missing, Marker::Armed(n)).unwrap();
            s.receipts.root().unwrap().persist(Marker::Armed(n), marker).unwrap();
        }
        Marker::Missing => (),
        Marker::Invalid => unreachable!(),
    }
    let r = Receipt::transaction_record(1001, epoch, operation, phase).unwrap();
    s.receipts.publish(ReceiptRead::Missing, r).unwrap();
    s
}
fn marker_stamp(f: &Fixture) -> (Vec<u8>, [u64; 11]) {
    let path = f.0.join("omavless-netguard/armed-v1.json");
    let m = fs::metadata(&path).unwrap();
    (fs::read(path).unwrap(), [m.dev(), m.ino(), m.mode() as u64, m.uid() as u64,
        m.gid() as u64, m.nlink(), m.len(), m.mtime() as u64, m.mtime_nsec() as u64,
        m.ctime() as u64, m.ctime_nsec() as u64])
}
fn assert_no_retry(s: &mut LockedState, k: &mut EarlyKernel) {
    assert!(s.poisoned);
    let before = (k.begins, k.fences, k.inner.observes, k.inner.effects);
    assert_eq!(s.prepare_cold_boot(NS, k), Err(REFUSED));
    assert_eq!(s.request(ARM, NS, k), Err(REFUSED));
    assert_eq!(before, (k.begins, k.fences, k.inner.observes, k.inner.effects));
}

#[test]
fn new_boot_new_causal_full_preserves_exact_armed_high_water_and_checked_operation() {
    for generation in [0, 192, u64::MAX] {
        let f = Fixture::new();
        let mut s = seeded(&f, Marker::Armed(generation), OLD, 19, ReceiptState::Live { handle: 5 });
        let marker = marker_stamp(&f);
        let mut k = EarlyKernel::new(&f);
        let status = s.prepare_cold_boot(NS, &mut k).unwrap();
        assert!(matches!(status, Response::Status { protection: Protection::Armed { generation: n }, health: Health::Verified, .. } if n == generation));
        assert_eq!(marker_stamp(&f), marker);
        let ReceiptRead::Durable(live) = s.receipts.read() else { panic!() };
        assert_eq!(live.operation(), 20);assert_eq!(live.epoch(), EPOCH);
        // Handle 5 may be reused numerically across boots; this is not adoption:
        // the complete initial inventory was absent and exactly ONE create ran.
        assert_eq!(live.state(), ReceiptState::Live { handle: 5 });
        assert_eq!(k.inner.effects, 1);assert_eq!(k.begins, 1);
        assert!(!s.poisoned);
        assert_eq!(s.request(Request::Status {}, NS, &mut k).unwrap(), status);
    }
}

#[test]
fn stable_current_absence_is_read_only_and_stale_retired_is_not_rebased() {
    let f = Fixture::new();let mut s = f.state();let mut k = EarlyKernel::new(&f);
    assert!(s.prepare_cold_boot(NS, &mut k).is_ok());
    assert_eq!(f.bytes(), (None, None));assert_eq!(k.begins, 0);assert_eq!(k.inner.effects, 0);
    for epoch in [EPOCH, OLD] {
        let f = Fixture::new();let mut s = seeded(&f, Marker::Closed(192), epoch, 5, ReceiptState::Retired);
        let before = f.bytes();let mut k = EarlyKernel::new(&f);
        assert_eq!(s.prepare_cold_boot(NS, &mut k).is_ok(), epoch == EPOCH);
        assert_eq!(before, f.bytes());assert_eq!(k.begins, 0);assert_eq!(k.inner.effects, 0);
        if epoch == OLD { assert_no_retry(&mut s, &mut k); }
    }
}

#[test]
fn current_epoch_or_same_boot_namespace_drift_never_adopts_an_orphan() {
    for epoch in [EPOCH, HostEpoch { namespace_epoch: [9;16], ..EPOCH },
        HostEpoch { namespace_inode: 88, ..EPOCH }] {
        let f = Fixture::new();let mut s = seeded(&f, Marker::Armed(192), epoch, 1, ReceiptState::Live { handle: 5 });
        let before = f.bytes();let mut k = EarlyKernel::new(&f);
        assert_eq!(s.prepare_cold_boot(NS, &mut k), Err(REFUSED));
        assert_eq!(before, f.bytes());assert_eq!(k.begins, 0);assert_eq!(k.inner.effects, 0);
        assert_no_retry(&mut s, &mut k);
    }
}

#[test]
fn every_incomplete_or_inconsistent_marker_receipt_combination_refuses_without_writes() {
    for phase in [ReceiptState::PendingCreate, ReceiptState::PendingReplace { old_handle: 5 },
        ReceiptState::PendingDelete { old_handle: 5 }, ReceiptState::Retired, ReceiptState::Live { handle: 5 }] {
        for marker in [Marker::Missing, Marker::Closed(192), Marker::Armed(192)] {
            if matches!((marker, phase), (Marker::Armed(_), ReceiptState::Live { .. })) { continue; }
            let f = Fixture::new();let mut s = seeded(&f, marker, OLD, 1, phase);
            let before = f.bytes();let mut k = EarlyKernel::new(&f);
            assert_eq!(s.prepare_cold_boot(NS, &mut k), Err(REFUSED));
            assert_eq!(before, f.bytes());assert_eq!(k.begins, 0);assert_eq!(k.inner.effects, 0);
            assert_no_retry(&mut s, &mut k);
        }
    }
}

#[test]
fn absent_with_identity_foreign_owned_or_unreadable_is_not_complete_absence() {
    for observed in [EffectSnapshot { table: Table::Absent, identity: Some(ID) },
        EffectSnapshot { table: Table::Foreign, identity: None },
        EffectSnapshot { table: Table::Unreadable, identity: None }, LIVE] {
        let f = Fixture::new();let mut s = seeded(&f, Marker::Armed(192), OLD, 1, ReceiptState::Live { handle: 5 });
        let before = f.bytes();let mut k = EarlyKernel::new(&f);k.inner.current = observed;
        assert_eq!(s.prepare_cold_boot(NS, &mut k), Err(REFUSED));
        assert_eq!(before, f.bytes());assert_eq!(k.inner.effects, 0);assert_eq!(k.begins, 0);
        assert_no_retry(&mut s, &mut k);
    }
}

#[test]
fn exhausted_operation_and_unproven_namespace_fail_before_pending_or_early_permission() {
    for namespace in [NamespaceObservation::Unproven,
        NamespaceObservation::Canonical(HostEpoch { boot:[0;16], ..EPOCH }),
        NamespaceObservation::Canonical(HostEpoch { namespace_epoch:[0;16], ..EPOCH }),
        NamespaceObservation::Canonical(HostEpoch { namespace_inode:0, ..EPOCH }), NS] {
        let f = Fixture::new();let mut s = seeded(&f, Marker::Armed(192), OLD, u64::MAX, ReceiptState::Live { handle:5 });
        let before = f.bytes();let mut k = EarlyKernel::new(&f);
        assert_eq!(s.prepare_cold_boot(namespace, &mut k), Err(REFUSED));
        assert_eq!(before,f.bytes());assert_eq!(k.begins,0);assert_eq!(k.inner.effects,0);
        assert_no_retry(&mut s,&mut k);
    }
}

#[test]
fn early_manager_refusal_is_pre_effect_and_cannot_be_retried() {
    let f=Fixture::new();let mut s=seeded(&f,Marker::Armed(192),OLD,1,ReceiptState::Live{handle:5});
    let before=f.bytes();let mut k=EarlyKernel::new(&f);k.refuse_begin=true;
    assert_eq!(s.prepare_cold_boot(NS,&mut k),Err(REFUSED));
    assert_eq!(f.bytes(),before);assert_eq!(k.inner.effects,0);
    k.refuse_begin=false;assert_no_retry(&mut s,&mut k);
}

#[test]
fn every_pending_terminal_and_final_cut_preserves_marker_and_retains_uncertainty() {
    let mut cuts = vec![Point::BeforePending, Point::Pending, Point::BeforeKernel,
        Point::KernelReturned, Point::KernelVerified, Point::BeforeTerminal,
        Point::Terminal, Point::BeforeReply];
    for terminal in [false,true] { for boundary in 0..=10 {
        cuts.push(Point::ReceiptWrite { terminal, boundary });
    }}
    for cut in cuts {
        let f=Fixture::new();let mut s=seeded(&f,Marker::Armed(192),OLD,1,ReceiptState::Live{handle:5});
        let marker=marker_stamp(&f);let mut k=EarlyKernel::new(&f);
        assert_eq!(s.cold_boot_with(NS,&mut k,|point| if point==cut {Err(REFUSED)} else {Ok(())}),Err(REFUSED),"{cut:?}");
        assert_eq!(marker_stamp(&f),marker,"{cut:?}");
        assert_no_retry(&mut s,&mut k);
    }
}

#[test]
fn every_manager_fence_failure_including_post_terminal_preserves_fence_and_no_retry() {
    let f=Fixture::new();let mut s=seeded(&f,Marker::Armed(192),OLD,1,ReceiptState::Live{handle:5});
    let mut k=EarlyKernel::new(&f);s.prepare_cold_boot(NS,&mut k).unwrap();let count=k.fences;
    assert!(count>20);
    drop(s);
    for cut in 1..=count {
        let f=Fixture::new();let mut s=seeded(&f,Marker::Armed(192),OLD,1,ReceiptState::Live{handle:5});
        let marker=marker_stamp(&f);let mut k=EarlyKernel::new(&f);k.refuse_fence=Some(cut);
        assert_eq!(s.prepare_cold_boot(NS,&mut k),Err(REFUSED),"fence {cut}");
        assert_eq!(marker_stamp(&f),marker);k.refuse_fence=None;assert_no_retry(&mut s,&mut k);
    }
}

#[test]
fn kernel_uncertainty_or_bad_new_readback_never_publishes_live_or_compensates() {
    for kind in 0..2 {
        let f=Fixture::new();let mut s=seeded(&f,Marker::Armed(192),OLD,1,ReceiptState::Live{handle:5});
        let marker=marker_stamp(&f);let mut k=EarlyKernel::new(&f);
        k.inner.fail_after_effect=kind==0;k.inner.bad_readback=kind==1;
        assert_eq!(s.prepare_cold_boot(NS,&mut k),Err(REFUSED));
        assert_eq!(marker_stamp(&f),marker);assert_eq!(k.inner.effects,1);
        assert!(matches!(s.receipts.read(),ReceiptRead::Durable(r) if r.state()==ReceiptState::PendingCreate && r.operation()==2 && r.epoch()==EPOCH));
        assert_no_retry(&mut s,&mut k);
    }
}

#[test]
fn old_default_classifier_remains_a_refusal_not_a_new_boot_creation_entry() {
    let f = Fixture::new();
    let mut s = seeded(&f, Marker::Armed(192), OLD, 1, ReceiptState::Live { handle: 5 });
    let before = f.bytes();
    s.seal_cold_state();
    let mut k = EarlyKernel::new(&f);
    assert_no_retry(&mut s, &mut k);
    assert_eq!(before, f.bytes());
}

#[test]
fn missing_receipt_invalid_marker_wrong_uid_and_publication_remnants_are_not_absence() {
    use std::os::unix::fs::{PermissionsExt,symlink};
    for kind in 0..6 {
        let f=Fixture::new();
        let mut s=if kind>=3 {
            seeded(&f,Marker::Armed(192),OLD,1,ReceiptState::Live{handle:5})
        } else {
            let mut s=f.state();
            s.receipts.root().unwrap().persist(Marker::Missing,Marker::Armed(192)).unwrap();
            s
        };
        let dir=f.0.join("omavless-netguard");
        match kind {
            0 => (), // genuine Armed but no receipt
            1 => fs::write(dir.join("armed-v1.json"),b"{}").unwrap(),
            2 => {
                let path=dir.join("table-receipt-v1.json");
                fs::write(&path,Receipt::transaction_record(1002,OLD,1,ReceiptState::Live{handle:5}).unwrap().encode().unwrap()).unwrap();
                fs::set_permissions(path,fs::Permissions::from_mode(0o600)).unwrap();
            }
            3 => {
                fs::remove_file(dir.join("table-receipt-v1.json")).unwrap();
                symlink("missing",dir.join("table-receipt-v1.json")).unwrap();
            }
            4 => fs::write(dir.join(".table-receipt-v1.pending"),b"foreign sentinel").unwrap(),
            _ => fs::write(dir.join(".table-receipt-v1.json.next"),b"foreign sentinel").unwrap(),
        }
        let before=f.bytes();let mut k=EarlyKernel::new(&f);
        assert_eq!(s.prepare_cold_boot(NS,&mut k),Err(REFUSED));
        assert_eq!(f.bytes(),before);assert_eq!(k.begins,0);assert_eq!(k.inner.effects,0);
        assert_no_retry(&mut s,&mut k);
    }
}
