use super::*;
use crate::authority_composition::{self, Boundary};
use crate::effect_port::{EffectIdentity, EffectPort, EffectSnapshot};
use crate::policy::Policy;
use crate::receipt::HostEpoch;
use std::cell::{Cell, RefCell};
use std::os::fd::AsRawFd;

struct Creator(Rc<Cell<usize>>);
impl Drop for Creator {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
impl crate::effect_port::sealed::Sealed for Creator {}
impl authority_composition::sealed::Sealed for Creator {}
impl CanonicalCreator for Creator {
    fn retained_epoch(&mut self, _: Boundary) -> Result<HostEpoch, EffectError> {
        unreachable!("no epoch needed by inert acquisition tests")
    }
}
impl EffectPort for Creator {
    fn observe(&mut self) -> Result<EffectSnapshot, EffectError> {
        unreachable!()
    }
    fn create_if_absent(&mut self, _: Policy) -> Result<EffectIdentity, EffectError> {
        unreachable!()
    }
    fn replace_owned(
        &mut self,
        _: EffectIdentity,
        _: Policy,
    ) -> Result<EffectIdentity, EffectError> {
        unreachable!()
    }
    fn delete_owned(&mut self, _: EffectIdentity) -> Result<(), EffectError> {
        unreachable!()
    }
}

#[derive(Default)]
struct Control {
    calls: usize,
    fail: Option<usize>,
    panic: Option<usize>,
    descriptors: Vec<[i32; 3]>,
}
struct Verifier(Rc<RefCell<Control>>);
impl OriginalVerifier for Verifier {
    fn recheck(&mut self, b: LaunchBorrow<'_>) -> Result<(), EffectError> {
        let mut c = self.0.borrow_mut();
        c.calls += 1;
        c.descriptors.push([
            b.anchor.as_raw_fd(),
            b.thread_namespace.as_raw_fd(),
            b.creator_socket.as_raw_fd(),
        ]);
        assert_ne!(c.panic, Some(c.calls), "synthetic verifier panic");
        if c.fail == Some(c.calls) {
            Err(EffectError::UnavailableOrUncertain)
        } else {
            Ok(())
        }
    }
}
fn fixture() -> (
    AcquiredCreator<Creator>,
    Rc<RefCell<Control>>,
    Rc<Cell<usize>>,
) {
    let drops = Rc::new(Cell::new(0));
    let mut owner = AcquiredCreator::synthetic(Creator(drops.clone()));
    let control = Rc::new(RefCell::new(Control::default()));
    owner.retained.originals.verifier = Box::new(Verifier(control.clone()));
    (owner, control, drops)
}

#[test]
fn exact_configuration_is_only_evidence_and_all_mutations_refuse() {
    let valid = |unit: &[u8], argv: &[&str], drops: &[&str]| {
        ConfigurationEvidence::validate("omavless-netguard", EXECUTABLE, unit, argv, drops)
    };
    assert!(valid(UNIT, &[EXECUTABLE, "serve"], &[]).is_ok());
    for (from, to) in [
        ("RestrictNamespaces=yes", "RestrictNamespaces=no"),
        ("PrivateNetwork=no", "PrivateNetwork=yes"),
        ("PrivateUsers=no", "PrivateUsers=yes"),
        ("PrivateMounts=no", "PrivateMounts=yes"),
        ("NoNewPrivileges=yes", "NoNewPrivileges=no"),
        ("User=root", "User=1000"),
        (
            "CapabilityBoundingSet=CAP_NET_ADMIN",
            "CapabilityBoundingSet=CAP_SYS_PTRACE",
        ),
    ] {
        let unit = std::str::from_utf8(UNIT).unwrap().replace(from, to);
        assert_ne!(unit.as_bytes(), UNIT);
        assert!(valid(unit.as_bytes(), &[EXECUTABLE, "serve"], &[]).is_err());
    }
    for suffix in [
        "Environment=X=y\n",
        "NetworkNamespacePath=/proc/1/ns/net\n",
        "JoinsNamespaceOf=other.service\n",
        "ExecStartPost=/bin/true\n",
        "RootDirectory=/other\n",
        "User=root\n",
    ] {
        let unit = [UNIT, suffix.as_bytes()].concat();
        assert!(valid(&unit, &[EXECUTABLE, "serve"], &[]).is_err());
    }
    for argv in [
        vec![],
        vec![EXECUTABLE],
        vec![EXECUTABLE, "serve", "extra"],
        vec!["/bin/sh", "serve"],
    ] {
        assert!(valid(UNIT, &argv, &[]).is_err());
    }
    assert!(valid(UNIT, &[EXECUTABLE, "serve"], &["anything.conf"]).is_err());
    assert!(
        ConfigurationEvidence::validate("other", EXECUTABLE, UNIT, &[EXECUTABLE, "serve"], &[])
            .is_err()
    );
    assert!(
        ConfigurationEvidence::validate(
            "omavless-netguard",
            "/other",
            UNIT,
            &[EXECUTABLE, "serve"],
            &[]
        )
        .is_err()
    );
}

#[test]
fn same_original_owners_survive_callback_and_both_rechecks() {
    let (mut owner, c, drops) = fixture();
    let expected = {
        let o = &owner.retained.originals;
        [
            o.anchor.as_raw_fd(),
            o.thread_namespace.as_raw_fd(),
            o.creator_socket.as_raw_fd(),
        ]
    };
    assert_eq!(
        owner.with_lease(|_| {
            assert_eq!(drops.get(), 0);
            Ok(17)
        }),
        Ok(17)
    );
    assert_eq!(c.borrow().descriptors, vec![expected, expected]);
    assert_eq!(drops.get(), 0);
    owner.release_synthetic();
    assert_eq!(drops.get(), 1);
}

#[test]
fn before_after_error_and_panic_poison_without_second_call_or_cleanup() {
    for at in [1, 2] {
        for panic in [false, true] {
            let drops = {
                let (mut owner, c, drops) = fixture();
                if panic {
                    c.borrow_mut().panic = Some(at);
                } else {
                    c.borrow_mut().fail = Some(at);
                }
                let effects = Cell::new(0);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    owner.with_lease(|_| {
                        effects.set(effects.get() + 1);
                        Ok(())
                    })
                }));
                if panic {
                    assert!(outcome.is_err());
                } else {
                    assert!(outcome.unwrap().is_err());
                }
                assert_eq!(effects.get(), usize::from(at == 2));
                c.borrow_mut().fail = None;
                c.borrow_mut().panic = None;
                assert!(
                    owner
                        .with_lease(|_| {
                            effects.set(99);
                            Ok(())
                        })
                        .is_err()
                );
                assert_eq!(c.borrow().calls, at);
                assert_eq!(effects.get(), usize::from(at == 2));
                drops
            };
            assert_eq!(drops.get(), 0);
        }
    }
}

#[test]
fn callback_error_or_unwind_poison_before_postcheck_and_retain_owner() {
    for panic in [false, true] {
        let drops = {
            let (mut owner, c, drops) = fixture();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.with_lease::<()>(|_| {
                    assert!(!panic, "synthetic callback panic");
                    Err(EffectError::UnavailableOrUncertain)
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(c.borrow().calls, 1);
            assert!(owner.with_lease(|_| Ok(())).is_err());
            assert_eq!(c.borrow().calls, 1);
            drops
        };
        assert_eq!(drops.get(), 0);
    }
}

#[test]
fn different_actual_thread_identity_refuses_before_verifier_or_callback() {
    let drops = {
        let (mut owner, c, drops) = fixture();
        owner.retained.originals.owner_thread = std::thread::spawn(|| std::thread::current().id())
            .join()
            .unwrap();
        assert!(
            owner
                .with_lease::<()>(|_| panic!("wrong thread callback"))
                .is_err()
        );
        assert_eq!(c.borrow().calls, 0);
        owner.retained.originals.owner_thread = std::thread::current().id();
        assert!(owner.with_lease(|_| Ok(())).is_err());
        drops
    };
    assert_eq!(drops.get(), 0);
}
