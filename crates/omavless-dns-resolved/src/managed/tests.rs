use super::*;
use crate::tests::{Fixture, State};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn link_path_matches_systemd_decimal_label_encoding() {
    // Golden results independently checked with libsystemd sd_bus_path_encode;
    // installed resolved GetLink(1) also returns the first value below.
    for (index, expected) in [
        (1, "/org/freedesktop/resolve1/link/_31"),
        (9, "/org/freedesktop/resolve1/link/_39"),
        (10, "/org/freedesktop/resolve1/link/_310"),
        (42, "/org/freedesktop/resolve1/link/_342"),
        (123, "/org/freedesktop/resolve1/link/_3123"),
        (i32::MAX, "/org/freedesktop/resolve1/link/_32147483647"),
    ] {
        assert_eq!(link_path(index).unwrap(), expected);
        assert_ne!(
            link_path(index).unwrap(),
            format!("/org/freedesktop/resolve1/link/_{index}")
        );
    }
    for index in [0, -1, i32::MIN] {
        assert_eq!(link_path(index), Err(Error::LeaseLost));
    }
}

struct MockLease {
    checks: Arc<AtomicUsize>,
    fail_at: usize,
    index: u32,
}

impl Lease for MockLease {
    fn index(&self) -> u32 {
        self.index
    }
    fn recheck(&self) -> Result<(), Error> {
        if self.checks.fetch_add(1, Ordering::SeqCst) >= self.fail_at {
            Err(Error::LeaseLost)
        } else {
            Ok(())
        }
    }
}

fn lease(fail_at: usize) -> MockLease {
    MockLease {
        checks: Arc::new(AtomicUsize::new(0)),
        fail_at,
        index: 42,
    }
}

#[test]
fn ownership_drift_permanently_blocks_later_compensation() {
    let fixture = Fixture::new(State::default());
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        lease(usize::MAX),
    )
    .unwrap();
    assert_eq!(
        managed.checked::<()>(|_| Err(Error::OwnershipChanged)),
        Err(Error::OwnershipChanged)
    );
    assert_eq!(
        managed.checked(Resolved::revert_pristine_link),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn constructor_uses_real_getlink_wire_and_pre_post_checks() {
    let fixture = Fixture::new(State::default());
    let held = lease(usize::MAX);
    let checks = held.checks.clone();
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        held,
    )
    .unwrap();
    assert_eq!(checks.load(Ordering::SeqCst), 2);
    managed.checked(Resolved::set_fixed_servers).unwrap();
    assert_eq!(checks.load(Ordering::SeqCst), 4);
    assert_eq!(fixture.state.lock().unwrap().calls, ["SetLinkDNS"]);
}

#[test]
fn failed_precheck_never_writes_and_remains_poisoned() {
    let fixture = Fixture::new(State::default());
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        lease(2),
    )
    .unwrap();
    assert_eq!(
        managed.checked(Resolved::set_fixed_servers),
        Err(Error::LeaseLost)
    );
    assert_eq!(
        managed.checked(Resolved::revert_pristine_link),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn failed_postcheck_retains_effect_and_blocks_compensation() {
    let fixture = Fixture::new(State::default());
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        lease(3),
    )
    .unwrap();
    assert_eq!(
        managed.checked(Resolved::set_fixed_servers),
        Err(Error::LeaseLost)
    );
    assert_eq!(
        managed.checked(Resolved::revert_pristine_link),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(fixture.state.lock().unwrap().calls, ["SetLinkDNS"]);
}

#[test]
fn owner_replacement_or_invalid_owner_cannot_enable_writes() {
    let fixture = Fixture::new(State::default());
    for owner in [
        "org.freedesktop.resolve1",
        "private.invalid",
        ":999999.999999",
    ] {
        assert!(
            Managed::from_admitted_parts(
                fixture.resolved.connection.clone(),
                owner.into(),
                lease(usize::MAX)
            )
            .is_err()
        );
    }
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        lease(usize::MAX),
    )
    .unwrap();
    fixture.server.release_name(SERVICE).unwrap();
    assert!(managed.checked(Resolved::set_fixed_servers).is_err());
    assert_eq!(
        managed.checked(Resolved::set_fixed_servers),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn invalid_or_lost_lease_cannot_construct_transport() {
    let fixture = Fixture::new(State::default());
    for index in [0, i32::MAX as u32 + 1] {
        let mut held = lease(usize::MAX);
        held.index = index;
        assert!(
            Managed::from_admitted_parts(
                fixture.resolved.connection.clone(),
                fixture.resolved.owner.clone(),
                held
            )
            .is_err()
        );
    }
    assert!(
        Managed::from_admitted_parts(
            fixture.resolved.connection.clone(),
            fixture.resolved.owner.clone(),
            lease(0)
        )
        .is_err()
    );
    assert!(
        Managed::from_admitted_parts(
            fixture.resolved.connection.clone(),
            fixture.resolved.owner.clone(),
            lease(1)
        )
        .is_err()
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn redirected_link_path_and_changed_index_are_refused() {
    let fixture = Fixture::new(State::default());
    fixture.state.lock().unwrap().wrong_link_path = true;
    assert!(
        Managed::from_admitted_parts(
            fixture.resolved.connection.clone(),
            fixture.resolved.owner.clone(),
            lease(usize::MAX)
        )
        .is_err()
    );
    fixture.state.lock().unwrap().wrong_link_path = false;
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        lease(usize::MAX),
    )
    .unwrap();
    managed.held.index += 1;
    assert_eq!(
        managed.checked(Resolved::set_fixed_servers),
        Err(Error::LeaseLost)
    );
    assert_eq!(
        managed.checked(Resolved::set_fixed_servers),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().calls.is_empty());
}

#[test]
fn guarded_baseline_apply_and_reset_use_the_same_pinned_link() {
    let fixture = Fixture::new(State::default());
    let mut managed = Managed::from_admitted_parts(
        fixture.resolved.connection.clone(),
        fixture.resolved.owner.clone(),
        lease(usize::MAX),
    )
    .unwrap();
    let baseline = managed
        .checked(|r| r.capture_baseline(&ExclusiveOwnership { _sealed: () }))
        .unwrap();
    managed.checked(Resolved::set_fixed_servers).unwrap();
    managed.checked(Resolved::set_root_routing_domain).unwrap();
    managed.checked(Resolved::set_default_route).unwrap();
    managed
        .checked(|r| r.verify_policy_preserves_baseline(&baseline))
        .unwrap();
    managed.checked(Resolved::revert_pristine_link).unwrap();
    managed.checked(|r| r.verify_reset(&baseline)).unwrap();
    assert_eq!(
        fixture.state.lock().unwrap().calls,
        [
            "SetLinkDNS",
            "SetLinkDomains",
            "SetLinkDefaultRoute",
            "RevertLink"
        ]
    );
}
