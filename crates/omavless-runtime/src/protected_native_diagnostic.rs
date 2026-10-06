// SPDX-License-Identifier: MIT
//! Test-only last-entered source point; never an admission or effect capability.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Cut {
    NotEntered,
    OwnerEligibility,
    SingletonCapture,
    MigrationAcquire,
    RequiredFence,
    CoordinatorEligibility,
    SingletonCheck,
    OriginEnvelope,
    DesiredRead,
    EmptyObservation,
    ProtectedEligibility,
    StatusExchange,
    StatusInterpretation,
    GenerationReservation,
    HostPreparation,
    PackageVerify,
    CoreCapture,
    BoundEligibility,
    ProfileParse,
    PolicyRender,
}

impl Cut {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::OwnerEligibility => "owner_eligibility",
            Self::SingletonCapture => "singleton_capture",
            Self::MigrationAcquire => "migration_acquire",
            Self::RequiredFence => "required_fence",
            Self::CoordinatorEligibility => "coordinator_eligibility",
            Self::SingletonCheck => "singleton_check",
            Self::OriginEnvelope => "origin_envelope",
            Self::DesiredRead => "desired_read",
            Self::EmptyObservation => "empty_observation",
            Self::ProtectedEligibility => "protected_eligibility",
            Self::StatusExchange => "status_exchange",
            Self::StatusInterpretation => "status_interpretation",
            Self::GenerationReservation => "generation_reservation",
            Self::HostPreparation => "host_preparation",
            Self::PackageVerify => "package_verify",
            Self::CoreCapture => "core_capture",
            Self::BoundEligibility => "bound_eligibility",
            Self::ProfileParse => "profile_parse",
            Self::PolicyRender => "policy_render",
        }
    }
}

thread_local! {
    static LAST: Cell<Cut> = const { Cell::new(Cut::NotEntered) };
}

pub(crate) fn mark(cut: Cut) {
    LAST.set(cut);
}

pub(crate) fn last() -> Cut {
    LAST.get()
}

#[test]
fn diagnostic_is_closed_resettable_and_thread_local_not_authority() {
    mark(Cut::NotEntered);
    assert_eq!(last(), Cut::NotEntered);
    mark(Cut::StatusExchange);
    let child = std::thread::spawn(|| {
        assert_eq!(last(), Cut::NotEntered);
        mark(Cut::PolicyRender);
        assert_eq!(last().token(), "policy_render");
    });
    child.join().unwrap();
    assert_eq!(last().token(), "status_exchange");
    mark(Cut::NotEntered);
    assert_eq!(last().token(), "not_entered");
}
