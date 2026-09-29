use super::*;

struct Case {
    state: RecordState,
    record: RecordIdentity,
    old: Binding,
    new: Binding,
    old_scope: HostScope,
    new_scope: HostScope,
    exclusive: bool,
    predecessor: Predecessor,
}

impl Case {
    fn valid() -> Self {
        let old = Binding {
            owner_instance: [1; 16],
            owner_generation: 1,
            boot: [3; 16],
            session: [4; 16],
            uid: nix::unistd::geteuid().as_raw(),
        };
        let new = Binding {
            owner_instance: [2; 16],
            owner_generation: 2,
            ..old
        };
        let record = RecordIdentity {
            digest: [7; 32],
            sequence: 4,
        };
        let scope = HostScope {
            manager_incarnation: [5; 16],
            bus_incarnation: [6; 16],
            settings_profile: [7; 16],
            shared_activation_environment: true,
        };
        Self {
            state: RecordState::Restorable,
            record,
            old,
            new,
            old_scope: scope,
            new_scope: scope,
            exclusive: true,
            predecessor: Predecessor::Graceful(GracefulReceipt {
                record,
                predecessor_instance: old.owner_instance,
                successor_instance: new.owner_instance,
                workers_joined: true,
                effects_settled: true,
            }),
        }
    }

    fn decide(&self) -> Result<CompensationTransfer, Refusal> {
        evaluate(Admission {
            record_state: self.state,
            record: self.record,
            predecessor: self.old,
            successor: self.new,
            old_scope: self.old_scope,
            new_scope: self.new_scope,
            native_owner_exclusive: self.exclusive,
            quiescence: self.predecessor,
        })
    }
}

#[test]
fn only_exact_graceful_transfer_is_eligible_for_later_durable_commit() {
    let case = Case::valid();
    let plan = case.decide().unwrap();
    assert_eq!(plan.record, case.record);
    assert_eq!(plan.predecessor, case.old);
    assert_eq!(plan.successor, case.new);
    let printed = format!("{plan:?} {:?} {:?}", case.record, case.new_scope);
    assert!(!printed.contains("070707"));
    assert!(!printed.contains("050505"));
}

#[test]
fn missing_tombstone_or_changed_record_never_becomes_fresh_baseline() {
    let mut case = Case::valid();
    case.state = RecordState::Missing;
    assert!(matches!(case.decide(), Err(Refusal::MissingOrReleased)));
    case.state = RecordState::Released;
    assert!(matches!(case.decide(), Err(Refusal::MissingOrReleased)));
    case.state = RecordState::Restorable;
    case.record.sequence += 1;
    assert!(matches!(case.decide(), Err(Refusal::RecordChanged)));
    case.record = Case::valid().record;
    case.record.digest[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::RecordChanged)));
}

#[test]
fn native_owner_and_successor_must_be_distinct_and_monotonic() {
    let mut case = Case::valid();
    case.exclusive = false;
    assert!(matches!(case.decide(), Err(Refusal::UnverifiedAuthority)));
    case = Case::valid();
    case.new.owner_instance = case.old.owner_instance;
    assert!(matches!(case.decide(), Err(Refusal::UnverifiedAuthority)));
    case = Case::valid();
    case.new.owner_generation = case.old.owner_generation;
    assert!(matches!(case.decide(), Err(Refusal::UnverifiedAuthority)));
    case = Case::valid();
    case.record.digest = [0; 32];
    assert!(matches!(case.decide(), Err(Refusal::UnverifiedAuthority)));
    case = Case::valid();
    case.record.sequence = 0;
    assert!(matches!(case.decide(), Err(Refusal::UnverifiedAuthority)));
}

#[test]
fn changed_or_unverified_host_scope_refuses_even_with_matching_values() {
    let mut case = Case::valid();
    case.new.boot[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.new.session[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.new.uid = case.new.uid.saturating_add(1);
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.new_scope.manager_incarnation[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.new_scope.bus_incarnation[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.new_scope.settings_profile[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.new_scope.shared_activation_environment = false;
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
    case = Case::valid();
    case.old_scope.bus_incarnation = [0; 16];
    assert!(matches!(case.decide(), Err(Refusal::ScopeChanged)));
}

#[test]
fn crash_or_unfinished_async_effect_cannot_transfer() {
    let mut case = Case::valid();
    case.predecessor = Predecessor::Unknown;
    assert!(matches!(case.decide(), Err(Refusal::PredecessorUnsettled)));
    case = Case::valid();
    let Predecessor::Graceful(ref mut receipt) = case.predecessor else {
        unreachable!()
    };
    receipt.workers_joined = false;
    assert!(matches!(case.decide(), Err(Refusal::PredecessorUnsettled)));
    case = Case::valid();
    let Predecessor::Graceful(ref mut receipt) = case.predecessor else {
        unreachable!()
    };
    receipt.effects_settled = false;
    assert!(matches!(case.decide(), Err(Refusal::PredecessorUnsettled)));
}

#[test]
fn receipt_is_exactly_bound_to_both_owner_instances() {
    let mut case = Case::valid();
    let Predecessor::Graceful(ref mut receipt) = case.predecessor else {
        unreachable!()
    };
    receipt.predecessor_instance[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::RecordChanged)));
    case = Case::valid();
    let Predecessor::Graceful(ref mut receipt) = case.predecessor else {
        unreachable!()
    };
    receipt.successor_instance[0] ^= 1;
    assert!(matches!(case.decide(), Err(Refusal::RecordChanged)));
}
