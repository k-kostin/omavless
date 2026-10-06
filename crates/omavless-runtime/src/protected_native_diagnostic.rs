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
    OriginLock,
    OriginMarker,
    OriginPending,
    OriginLogin,
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
            Self::OriginLock => "origin_lock",
            Self::OriginMarker => "origin_marker",
            Self::OriginPending => "origin_pending",
            Self::OriginLogin => "origin_login",
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
    static ORIGIN: Cell<(Site, u8)> = const { Cell::new((Site::Initial, 0)) };
}

/// Closed call class plus saturating source invocation ordinal, never a PID,
/// generation, caller input or permission. Saturation cannot alter execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Site {
    Initial,
    Local,
    Status,
    Arm,
    Disarm,
    Preparation,
    IntervalBefore,
    IntervalAfter,
}
impl Site {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Local => "local",
            Self::Status => "status",
            Self::Arm => "arm",
            Self::Disarm => "disarm",
            Self::Preparation => "preparation",
            Self::IntervalBefore => "interval_before",
            Self::IntervalAfter => "interval_after",
        }
    }
}
pub(crate) fn site(site: Site) {
    ORIGIN.set((site, ORIGIN.get().1));
}
pub(crate) fn enter_origin() {
    let (site, n) = ORIGIN.get();
    ORIGIN.set((site, n.saturating_add(1)));
}
pub(crate) fn origin() -> (Site, u8) {
    ORIGIN.get()
}

pub(crate) fn mark(cut: Cut) {
    if cut == Cut::NotEntered {
        ORIGIN.set((Site::Initial, 0));
    }
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

#[test]
fn origin_diagnostic_is_bounded_private_free_and_thread_local() {
    mark(Cut::NotEntered);
    assert_eq!(origin(), (Site::Initial, 0));
    site(Site::Arm);
    enter_origin();
    assert_eq!(origin(), (Site::Arm, 1));
    std::thread::spawn(|| {
        assert_eq!(origin(), (Site::Initial, 0));
        site(Site::Disarm);
        enter_origin();
    })
    .join()
    .unwrap();
    assert_eq!(origin(), (Site::Arm, 1));
    for _ in 0..300 {
        enter_origin();
    }
    assert_eq!(origin(), (Site::Arm, 255));
    let sites = [
        Site::Initial,
        Site::Local,
        Site::Status,
        Site::Arm,
        Site::Disarm,
        Site::Preparation,
        Site::IntervalBefore,
        Site::IntervalAfter,
    ];
    let labels = sites.map(Site::token);
    assert_eq!(
        labels,
        [
            "initial",
            "local",
            "status",
            "arm",
            "disarm",
            "preparation",
            "interval_before",
            "interval_after"
        ]
    );
    assert_eq!(
        [
            Cut::OriginLock,
            Cut::OriginMarker,
            Cut::OriginPending,
            Cut::OriginLogin
        ]
        .map(Cut::token),
        [
            "origin_lock",
            "origin_marker",
            "origin_pending",
            "origin_login"
        ]
    );
    mark(Cut::NotEntered);
    assert_eq!(origin(), (Site::Initial, 0));
}

#[test]
fn original_origin_predicates_remain_once_in_short_circuit_order() {
    let source = include_str!("native_coordinator/protected_native.rs");
    let body = source
        .split("let mut origin = || {")
        .nth(1)
        .unwrap()
        .split("\n        };")
        .next()
        .unwrap();
    let checks = [
        "if !lock.authorizes(&paths, uid)",
        "if !read_marker(&paths, uid)",
        "if crate::pending_private_transaction::pending(&desired)",
        "if crate::login_transaction::check_startup_receipt(",
    ];
    let mut previous = 0;
    for check in checks {
        assert_eq!(body.matches(check).count(), 1);
        let at = body.find(check).unwrap();
        assert!(at > previous);
        previous = at;
    }
    assert_eq!(
        body.matches("return Err(LifecycleError::ManualRecoveryRequired);")
            .count(),
        4
    );
    assert_eq!(body.matches("singleton()?").count(), 1);
    assert_eq!(body.matches("enter_origin()").count(), 1);
}
