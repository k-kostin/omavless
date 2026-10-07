use super::*;
use crate::authority_composition::StartupAuthority;
use crate::launch_acquisition::{AcquiredCreator,SyntheticStartup};

type ControlPair=(Rc<RefCell<Control>>,Rc<RefCell<SyntheticStartup>>);
fn startup(f:&Fixture,armed:bool) -> (StartupAuthority<Provider>,ControlPair) {
    let mut state=bound_state(f,peer_uid());
    if armed {
        state.receipts.root().unwrap().persist(Marker::Missing,Marker::Armed(192)).unwrap();
        state.receipts.publish(ReceiptRead::Missing,Receipt::transaction_record(peer_uid(),
            HostEpoch {boot:[8;16],..EPOCH},1,ReceiptState::Live{handle:5}).unwrap()).unwrap();
    }
    let provider=Rc::new(RefCell::new(Control::default()));
    let origin=Rc::new(RefCell::new(SyntheticStartup::default()));
    let creator=AcquiredCreator::synthetic_startup(Provider {kernel:Kernel::new(f),control:provider.clone()},origin.clone());
    (StartupAuthority::new(state,creator),(provider,origin))
}
fn listener(f:&Fixture) -> (crate::listener_admission::AdmittedListener,PathBuf) {
    let (parent,path,permissions)=publication_site(f);
    (publish_test_parent(File::open(parent).unwrap(),&path,permissions,permissions.1,||true).unwrap(),path)
}

#[test]
fn exact_terminal_creator_and_lock_transfer_precedes_one_ready_and_first_accept() {
    for armed in [false,true] {
        let f=Fixture::new();let (mut held,(provider,origin))=startup(&f,armed);
        let before=f.bytes().0;held.prepare().unwrap();
        assert_eq!(f.bytes().0,before);assert_eq!(origin.borrow().begins,usize::from(armed));
        assert_eq!(origin.borrow().consumes,1);assert_eq!(origin.borrow().notifications,0);
        assert!(matches!(open_fixture(&f.0),Err(StateError::Busy)));
        let (listener,path)=listener(&f);
        let mut session=held.into_session(listener).unwrap();
        let mut stream=client(&path,Request::Status{});
        // The live named listener is retained, but NO client is accepted yet.
        assert_eq!(session.poll_one(),SessionProgress::AuthorityLost);
        assert_eq!(origin.borrow().notifications,0);
        session.publish_ready().unwrap();
        origin.borrow_mut().early_lost=true; // NM may start AFTER READY.
        assert_eq!(session.poll_one(),SessionProgress::Served);
        let response=receive(&mut stream);
        if armed {
            assert!(matches!(response,Response::Status{protection:Protection::Armed{generation:192},..}));
            let r=record(session.test_state());assert_eq!(r.operation(),2);assert_eq!(r.epoch(),EPOCH);
            assert_eq!(provider.borrow().effects,1);
        } else { assert!(matches!(response,Response::Status{protection:Protection::Disarmed{..},..})); }
        assert_eq!(origin.borrow().notifications,1);
        assert!(session.publish_ready().is_err());
        session.release_synthetic();assert_eq!(provider.borrow().drops,1);
    }
}

#[test]
fn notification_failure_retains_named_prefix_and_original_lock_without_accept_or_retry() {
    let f=Fixture::new();let (mut held,(provider,origin))=startup(&f,true);
    held.prepare().unwrap();let complete=f.bytes();let (listener,path)=listener(&f);
    let mut session=held.into_session(listener).unwrap();origin.borrow_mut().fail_notify=true;
    let _queued=client(&path,Request::Status{});
    assert!(session.publish_ready().is_err());assert!(!origin.borrow().ready);
    origin.borrow_mut().fail_notify=false;
    assert!(session.publish_ready().is_err());assert_eq!(origin.borrow().notifications,1);
    assert_eq!(session.poll_one(),SessionProgress::AuthorityLost);
    assert_eq!(f.bytes(),complete);assert_eq!(provider.borrow().effects,1);
    { let _retained = session; }assert_eq!(provider.borrow().drops,0);
    assert!(path.exists());assert!(UnixStream::connect(&path).is_ok());
    assert!(matches!(open_fixture(&f.0),Err(StateError::Busy)));
}

#[test]
fn final_startup_guard_or_unwind_retains_terminal_and_graph_without_publication_or_ready() {
    for panic in [false,true] {
        let f=Fixture::new();let (mut held,(provider,origin))=startup(&f,true);
        origin.borrow_mut().panic_consume=panic;
        if !panic { provider.borrow_mut().post_effect_observe=true; }
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||held.prepare()));
        if panic { assert!(result.is_err()); }
        else { assert!(result.unwrap().is_err()); }
        if panic {
            let record=receipt::decode(&f.bytes().1.unwrap()).unwrap();
            assert_eq!(record.state(),ReceiptState::Live{handle:5});
            assert_eq!(record.operation(),2);assert_eq!(record.epoch(),EPOCH);
        }
        provider.borrow_mut().post_effect_observe=false;origin.borrow_mut().panic_consume=false;
        assert!(held.prepare().is_err());assert_eq!(origin.borrow().notifications,0);
        let marker:serde_json::Value=serde_json::from_slice(&f.bytes().0.unwrap()).unwrap();
        assert_eq!(marker["armed"],true);assert_eq!(marker["generation"],192);
        { let _retained = held; }assert_eq!(provider.borrow().drops,0);
        assert!(matches!(open_fixture(&f.0),Err(StateError::Busy)));
        assert!(!f.0.join("run/omavless-netguard/control.sock").exists());
    }
}

#[test]
fn no_listener_first_unproven_epoch_or_failed_early_gate_cannot_transfer_readiness() {
    for kind in 0..2 {
        let f=Fixture::new();let (mut held,(provider,origin))=startup(&f,true);
        if kind==0 {provider.borrow_mut().fail=Some(Boundary::Admission);}
        else {origin.borrow_mut().early_lost=true;}
        let before=f.bytes();assert!(held.prepare().is_err());
        assert_eq!(f.bytes(),before);assert_eq!(provider.borrow().effects,0);
        let (listener,path)=listener(&f);
        assert!(held.into_session(listener).is_err());assert!(path.exists());
        assert_eq!(origin.borrow().notifications,0);assert_eq!(provider.borrow().drops,0);
        assert!(matches!(open_fixture(&f.0),Err(StateError::Busy)));
    }
}

#[test]
fn late_private_bind_refusal_keeps_actual_listener_prefix_but_never_notifies_or_accepts() {
    let f=Fixture::new();let (mut held,(provider,origin))=startup(&f,true);held.prepare().unwrap();
    let (parent,path,permissions)=publication_site(&f);
    assert!(publish_test_parent(File::open(parent).unwrap(),&path,permissions,permissions.1,||false).is_err());
    assert!(path.exists());assert!(UnixStream::connect(&path).is_ok());
    assert_eq!(origin.borrow().notifications,0);assert_eq!(provider.borrow().effects,1);
    { let _retained = held; }assert_eq!(provider.borrow().drops,0);
    assert!(matches!(open_fixture(&f.0),Err(StateError::Busy)));
}
