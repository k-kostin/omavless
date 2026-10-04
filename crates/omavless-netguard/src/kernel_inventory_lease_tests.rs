//! Inert lease controls: real original local descriptors, no nft datagrams,
//! namespace transition, policy effects or canonical authority.
use super::*;

fn synthetic_absence(session: &mut LocalReadSession) -> LocalInventoryLease<'_> {
    // Private test fabrication only; the public constructor always performs the
    // complete actual inventory. This never fabricates creator authority.
    session.last_generation = Some(41);
    LocalInventoryLease {
        session,
        inventory: LocalPolicyInventory::TableAbsent,
        generation: 41,
        table: None,
        deadline: Instant::now() + Duration::from_secs(1),
    }
}

#[test]
fn cancel_retains_same_original_descriptors_and_sequence_without_exchange() {
    let mut session = LocalReadSession::open().unwrap();
    let original = (
        session.namespace.as_raw_fd(),
        session.socket.as_raw_fd(),
        session.next_sequence,
    );
    {
        let mut lease = synthetic_absence(&mut session);
        assert_eq!(lease.observed(), LocalPolicyInventory::TableAbsent);
        lease.recheck().unwrap();
    }
    assert_eq!(
        original,
        (
            session.namespace.as_raw_fd(),
            session.socket.as_raw_fd(),
            session.next_sequence,
        )
    );
    assert!(!session.poisoned);
    session
        .check(Instant::now() + Duration::from_secs(1))
        .unwrap();
}

#[test]
fn expired_original_budget_seals_session_without_refresh_or_exchange() {
    let mut session = LocalReadSession::open().unwrap();
    let sequence = session.next_sequence;
    {
        let mut lease = synthetic_absence(&mut session);
        lease.deadline = Instant::now();
        assert_eq!(lease.recheck(), Err(REFUSE));
        lease.deadline = Instant::now() + Duration::from_secs(1);
        assert_eq!(lease.recheck(), Err(REFUSE));
    }
    assert!(session.poisoned);
    assert_eq!(session.next_sequence, sequence);
    assert!(session.borrow_policy_inventory().is_err());
    assert_eq!(session.next_sequence, sequence);
}

#[test]
fn remembered_generation_or_shape_mismatch_permanently_refuses() {
    for changed_generation in [true, false] {
        let mut session = LocalReadSession::open().unwrap();
        let mut lease = synthetic_absence(&mut session);
        if changed_generation {
            lease.session.last_generation = Some(42);
        } else {
            lease.inventory = LocalPolicyInventory::OtherUntrusted;
        }
        assert_eq!(lease.recheck(), Err(REFUSE));
        assert!(lease.session.poisoned);
    }
}
