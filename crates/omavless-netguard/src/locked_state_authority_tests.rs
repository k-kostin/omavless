mod authority_composition_controls {
    use super::*;
    use crate::authority_composition::{self, AuthoritySession, Boundary, CanonicalCreator};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Control {
        fail: Option<Boundary>,
        panic: Option<Boundary>,
        changed: bool,
        checks: Vec<Boundary>,
        drops: usize,
        effects: usize,
        effect_error: bool,
        effect_panic: bool,
        post_effect_observe: bool,
        launch_lost: Rc<std::cell::Cell<bool>>,
        lose_launch_after_effect: bool,
    }
    struct Provider { kernel: Kernel, control: Rc<RefCell<Control>> }
    impl Drop for Provider {
        fn drop(&mut self) { self.control.borrow_mut().drops += 1; }
    }
    impl crate::effect_port::sealed::Sealed for Provider {}
    impl authority_composition::sealed::Sealed for Provider {}
    impl CanonicalCreator for Provider {
        fn retained_epoch(&mut self, point: Boundary) -> Result<HostEpoch, EffectError> {
            let mut c = self.control.borrow_mut();
            c.checks.push(point);
            assert_ne!(c.panic, Some(point), "synthetic authority panic");
            if c.post_effect_observe && c.effects > 0 && point == Boundary::AfterObserve {
                return Err(EffectError::UnavailableOrUncertain);
            }
            if c.fail == Some(point) { return Err(EffectError::UnavailableOrUncertain); }
            Ok(if c.changed { HostEpoch { namespace_epoch: [9; 16], ..EPOCH } } else { EPOCH })
        }
    }
    impl Provider {
        fn returned<T>(&self, value: Result<T, EffectError>) -> Result<T, EffectError> {
            let control = self.control.borrow();
            if control.lose_launch_after_effect { control.launch_lost.set(true); }
            assert!(!control.effect_panic, "synthetic effect panic after execution");
            if control.effect_error { Err(EffectError::UnavailableOrUncertain) } else { value }
        }
    }
    impl EffectPort for Provider {
        fn observe(&mut self) -> Result<EffectSnapshot, EffectError> { self.kernel.observe() }
        fn create_if_absent(&mut self, policy: Policy) -> Result<EffectIdentity, EffectError> {
            self.control.borrow_mut().effects += 1;
            let result = self.kernel.create_if_absent(policy);
            self.returned(result)
        }
        fn replace_owned(&mut self, id: EffectIdentity, policy: Policy) -> Result<EffectIdentity, EffectError> {
            self.control.borrow_mut().effects += 1;
            let result = self.kernel.replace_owned(id, policy);
            self.returned(result)
        }
        fn delete_owned(&mut self, id: EffectIdentity) -> Result<(), EffectError> {
            self.control.borrow_mut().effects += 1;
            let result = self.kernel.delete_owned(id);
            self.returned(result)
        }
    }
    fn owner(f: &Fixture, control: Rc<RefCell<Control>>) -> (AuthoritySession<Provider>, PathBuf) {
        let (parent, path, permissions) = publication_site(f);
        // Actual private bind-before-access publisher and retained listener;
        // only canonical/kernel observations are synthetic, not socket/state IO.
        let listener = publish_test_parent(File::open(parent).unwrap(), &path,
            permissions, permissions.1, || true).unwrap();
        let lost = control.borrow().launch_lost.clone();
        let provider = Provider { kernel: Kernel::new(f), control };
        (AuthoritySession::from_admitted(listener, bound_state(f, peer_uid()),
            crate::launch_acquisition::AcquiredCreator::synthetic_controlled(provider, lost)).unwrap(), path)
    }
    fn assert_sealed(owner: &mut AuthoritySession<Provider>, control: &Rc<RefCell<Control>>) {
        control.borrow_mut().fail = None;
        control.borrow_mut().panic = None;
        let checks = control.borrow().checks.len();
        let effects = control.borrow().effects;
        for _ in 0..2 { assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost); }
        assert_eq!(control.borrow().checks.len(), checks);
        assert_eq!(control.borrow().effects, effects);
    }

    #[test]
    fn actual_publisher_session_and_single_writer_share_one_provider() {
        let f = Fixture::new();
        let control = Rc::new(RefCell::new(Control::default()));
        let (mut owner, path) = owner(&f, control.clone());
        let mut arm = client(&path, ARM);
        assert_eq!(owner.poll_one(), SessionProgress::Served);
        assert!(matches!(receive(&mut arm), Response::Status { protection: Protection::Armed { .. }, .. }));
        assert_eq!(record(owner.test_state()).state(), ReceiptState::Live { handle: 5 });
        let mut stop = client(&path, DISARM);
        assert_eq!(owner.poll_one(), SessionProgress::Served);
        assert!(matches!(receive(&mut stop), Response::Status { protection: Protection::Disarmed { closed_generation: Some(7) }, .. }));
        assert_eq!(record(owner.test_state()).state(), ReceiptState::Retired);
        assert_eq!(control.borrow().effects, 2);
        assert_eq!(control.borrow().checks.iter().filter(|&&b| b == Boundary::Admission).count(), 1);
        for boundary in [Boundary::BeforeCreate, Boundary::AfterCreate, Boundary::BeforeDelete, Boundary::AfterDelete,
                         Boundary::Exchange(ExchangeBoundary::BeforeReply), Boundary::Exchange(ExchangeBoundary::AfterReply)] {
            assert!(control.borrow().checks.contains(&boundary));
        }
        owner.release_synthetic();
        assert_eq!(control.borrow().drops, 1);
    }

    #[test]
    fn every_authority_cut_refuses_without_retry_or_false_terminal() {
        use ExchangeBoundary::*;
        for cut in [Boundary::Admission, Boundary::Exchange(BeforeAccept), Boundary::Exchange(AfterAccept),
                    Boundary::Exchange(BeforeReceive), Boundary::Exchange(AfterReceive),
                    Boundary::BeforeObserve, Boundary::AfterObserve, Boundary::BeforeCreate,
                    Boundary::AfterCreate, Boundary::Exchange(BeforeReply), Boundary::Exchange(AfterReply)] {
            let f = Fixture::new();
            let control = Rc::new(RefCell::new(Control { fail: Some(cut), ..Control::default() }));
            let (mut owner, path) = owner(&f, control.clone());
            let mut stream = client(&path, ARM);
            assert_ne!(owner.poll_one(), SessionProgress::Served, "{cut:?}");
            let expected_effects = usize::from(matches!(cut, Boundary::AfterCreate | Boundary::Exchange(BeforeReply | AfterReply)));
            assert_eq!(control.borrow().effects, expected_effects, "{cut:?}");
            if matches!(cut, Boundary::BeforeCreate | Boundary::AfterCreate) {
                assert_eq!(record(owner.test_state()).state(), ReceiptState::PendingCreate);
            } else if matches!(cut, Boundary::Exchange(BeforeReply | AfterReply)) {
                // The durable commit already happened: never rewrite it as
                // Pending or compensate just because delivery authority failed.
                assert_eq!(record(owner.test_state()).state(), ReceiptState::Live { handle: 5 });
            } else { assert_eq!(f.bytes(), (None, None)); }
            if cut == Boundary::Exchange(AfterReply) {
                // A final fence cannot retract an already delivered response.
                assert!(matches!(receive(&mut stream), Response::Status { .. }));
            }
            assert_sealed(&mut owner, &control);
            owner.release_synthetic();
        }
    }

    #[test]
    fn epoch_change_with_identical_inode_is_not_readmitted() {
        let f = Fixture::new();
        let control = Rc::new(RefCell::new(Control::default()));
        let (mut owner, _path) = owner(&f, control.clone());
        control.borrow_mut().changed = true;
        assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
        control.borrow_mut().changed = false;
        assert_sealed(&mut owner, &control);
        assert_eq!(f.bytes(), (None, None));
        owner.release_synthetic();
    }

    #[test]
    fn panic_during_provider_check_permanently_seals_without_unwind_cleanup() {
        for cut in [Boundary::Exchange(ExchangeBoundary::BeforeAccept), Boundary::AfterCreate] {
            let f = Fixture::new();
            let control = Rc::new(RefCell::new(Control::default()));
            let (mut owner, path) = owner(&f, control.clone());
            let _stream = client(&path, ARM);
            control.borrow_mut().panic = Some(cut);
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.poll_one())).is_err());
            assert_sealed(&mut owner, &control);
            assert_eq!(control.borrow().drops, 0);
            if cut == Boundary::AfterCreate { assert_eq!(record(owner.test_state()).state(), ReceiptState::PendingCreate); }
            owner.release_synthetic();
        }
    }

    #[test]
    fn normal_drop_retains_listener_provider_and_store_without_cleanup() {
        let f = Fixture::new();
        let control = Rc::new(RefCell::new(Control::default()));
        let path = {
            let (mut owner, path) = owner(&f, control.clone());
            control.borrow_mut().fail = Some(Boundary::Exchange(ExchangeBoundary::BeforeAccept));
            assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
            path
        };
        assert_eq!(control.borrow().drops, 0);
        assert!(path.exists());
        assert!(matches!(open_fixture(&f.0), Err(StateError::Busy)));
        assert_eq!(f.bytes(), (None, None));
    }

    #[test]
    fn replacement_and_delete_cuts_preserve_actual_pending_and_close_order() {
        for (request, before, after, pending) in [
            (ARM, Boundary::BeforeReplace,
             Boundary::AfterReplace, ReceiptState::PendingReplace { old_handle: 5 }),
            (DISARM, Boundary::BeforeDelete, Boundary::AfterDelete,
             ReceiptState::PendingDelete { old_handle: 5 }),
        ] {
            for cut in [before, after] {
                let f = Fixture::new();
                let control = Rc::new(RefCell::new(Control::default()));
                let (mut owner, path) = owner(&f, control.clone());
                let mut stream = client(&path, ARM);
                assert_eq!(owner.poll_one(), SessionProgress::Served);
                receive(&mut stream);
                control.borrow_mut().fail = Some(cut);
                let _stream = client(&path, request);
                assert_ne!(owner.poll_one(), SessionProgress::Served);
                assert_eq!(record(owner.test_state()).state(), pending);
                assert_eq!(control.borrow().effects, 1 + usize::from(cut == after));
                if request == DISARM {
                    let marker: serde_json::Value = serde_json::from_slice(&f.bytes().0.unwrap()).unwrap();
                    assert_eq!(marker["armed"], false);
                    assert_eq!(marker["generation"], 7);
                }
                assert_sealed(&mut owner, &control);
                owner.release_synthetic();
            }
        }
    }

    #[test]
    fn effect_error_or_panic_after_actual_effect_never_commits_or_compensates() {
        for kind in 0..3 {
            let request = if kind == 2 { DISARM } else { ARM };
            for panic in [false, true] {
                let f = Fixture::new();
                let control = Rc::new(RefCell::new(Control::default()));
                let (mut owner, path) = owner(&f, control.clone());
                if kind != 0 {
                    let mut first = client(&path, ARM);
                    assert_eq!(owner.poll_one(), SessionProgress::Served);
                    receive(&mut first);
                }
                control.borrow_mut().effect_error = !panic;
                control.borrow_mut().effect_panic = panic;
                let _stream = client(&path, request);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.poll_one()));
                if panic { assert!(result.is_err()); }
                else { assert_ne!(result.unwrap(), SessionProgress::Served); }
                let pending = if kind == 0 { ReceiptState::PendingCreate }
                    else if kind == 2 { ReceiptState::PendingDelete { old_handle: 5 } }
                    else { ReceiptState::PendingReplace { old_handle: 5 } };
                assert_eq!(record(owner.test_state()).state(), pending);
                assert_eq!(control.borrow().effects, if kind == 0 { 1 } else { 2 });
                control.borrow_mut().effect_error = false;
                control.borrow_mut().effect_panic = false;
                assert_sealed(&mut owner, &control);
                owner.release_synthetic();
            }
        }
    }

    #[test]
    fn post_effect_observation_refusal_retains_pending_before_terminal_write() {
        let f = Fixture::new();
        let control = Rc::new(RefCell::new(Control { post_effect_observe: true, ..Control::default() }));
        let (mut owner, path) = owner(&f, control.clone());
        let _stream = client(&path, ARM);
        assert_ne!(owner.poll_one(), SessionProgress::Served);
        assert_eq!(record(owner.test_state()).state(), ReceiptState::PendingCreate);
        assert_eq!(control.borrow().effects, 1);
        assert_sealed(&mut owner, &control);
        owner.release_synthetic();
    }

    #[test]
    fn constructor_panic_retains_original_listener_provider_and_lock() {
        let f = Fixture::new();
        let control = Rc::new(RefCell::new(Control { panic: Some(Boundary::Admission), ..Control::default() }));
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner(&f, control.clone()))).is_err());
        assert_eq!(control.borrow().drops, 0);
        assert_eq!(control.borrow().effects, 0);
        assert!(matches!(open_fixture(&f.0), Err(StateError::Busy)));
        assert!(f.0.join("run/omavless-netguard/control.sock").exists());
        assert_eq!(f.bytes(), (None, None));
    }

    #[test]
    fn listener_replacement_seals_stronger_owner_before_any_followup_provider_call() {
        let f = Fixture::new();
        let control = Rc::new(RefCell::new(Control::default()));
        let (mut owner, path) = owner(&f, control.clone());
        fs::rename(&path, path.with_extension("original")).unwrap();
        let _replacement = UnixListener::bind(&path).unwrap();
        assert_eq!(owner.poll_one(), SessionProgress::ListenerLost);
        assert_sealed(&mut owner, &control);
        assert_eq!(control.borrow().effects, 0);
        owner.release_synthetic();
    }

    #[test]
    fn acquisition_loss_after_effect_preserves_actual_pending_and_never_retries() {
        for kind in 0..3 {
            let f = Fixture::new();
            let control = Rc::new(RefCell::new(Control::default()));
            let (mut owner, path) = owner(&f, control.clone());
            if kind != 0 {
                let mut stream = client(&path, ARM);
                assert_eq!(owner.poll_one(), SessionProgress::Served);
                receive(&mut stream);
            }
            control.borrow_mut().lose_launch_after_effect = true;
            let _stream = client(&path, if kind == 2 { DISARM } else { ARM });
            assert_ne!(owner.poll_one(), SessionProgress::Served);
            let pending = match kind {
                0 => ReceiptState::PendingCreate,
                1 => ReceiptState::PendingReplace { old_handle: 5 },
                _ => ReceiptState::PendingDelete { old_handle: 5 },
            };
            assert_eq!(record(owner.test_state()).state(), pending);
            assert_eq!(control.borrow().effects, if kind == 0 { 1 } else { 2 });
            control.borrow().launch_lost.set(false);
            assert_sealed(&mut owner, &control);
            owner.release_synthetic();
        }
    }
}
