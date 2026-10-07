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
    StartAdmission,
    StartTunAbsent,
    CoreSpawn,
    ConfiguredWait,
    ConfiguredPid,
    SecureOwned,
    CoreRunning,
    TunCheck,
    StartComplete,
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
            Self::StartAdmission => "start_admission",
            Self::StartTunAbsent => "start_tun_absent",
            Self::CoreSpawn => "core_spawn",
            Self::ConfiguredWait => "configured_wait",
            Self::ConfiguredPid => "configured_pid",
            Self::SecureOwned => "secure_owned",
            Self::CoreRunning => "core_running",
            Self::TunCheck => "tun_check",
            Self::StartComplete => "start_complete",
        }
    }
}

thread_local! {
    static LAST: Cell<Cut> = const { Cell::new(Cut::NotEntered) };
    static ORIGIN: Cell<(Site, u8)> = const { Cell::new((Site::Initial, 0)) };
    static READINESS: Cell<(Endpoint, ReadinessPhase, ControllerPhase)> = const { Cell::new((Endpoint::NotEntered, ReadinessPhase::NotEntered, ControllerPhase::NotEntered)) };
    static READ_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static POST_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static POST: Cell<(PostGuard, PostRefusal)> = const { Cell::new((PostGuard::NotEntered, PostRefusal::NotRecorded)) };
}

/// Source-closed post-interval checks; no payload, path, PID or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PostGuard {
    NotEntered,
    DesiredRead,
    DesiredEquality,
    IntervalState,
    IntervalPair,
    IntervalPackage,
    IntervalPolicy,
    IntervalStoreRead,
    IntervalStoreDigest,
    IntervalCoreFile,
    IntervalConfigFile,
    IntervalDataMetadata,
    IntervalDataIdentity,
    ConnectedObservation,
    CoreRunning,
    CoreIntent,
    CoreReadiness,
    TunVerify,
    VisibleCoreCount,
    ManagedTunCount,
    ConnectedReconcile,
    Completed,
}
impl PostGuard {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::DesiredRead => "desired_read",
            Self::DesiredEquality => "desired_equality",
            Self::IntervalState => "interval_state",
            Self::IntervalPair => "interval_pair",
            Self::IntervalPackage => "interval_package",
            Self::IntervalPolicy => "interval_policy",
            Self::IntervalStoreRead => "interval_store_read",
            Self::IntervalStoreDigest => "interval_store_digest",
            Self::IntervalCoreFile => "interval_core_file",
            Self::IntervalConfigFile => "interval_config_file",
            Self::IntervalDataMetadata => "interval_data_metadata",
            Self::IntervalDataIdentity => "interval_data_identity",
            Self::ConnectedObservation => "connected_observation",
            Self::CoreRunning => "core_running",
            Self::CoreIntent => "core_intent",
            Self::CoreReadiness => "core_readiness",
            Self::TunVerify => "tun_verify",
            Self::VisibleCoreCount => "visible_core_count",
            Self::ManagedTunCount => "managed_tun_count",
            Self::ConnectedReconcile => "connected_reconcile",
            Self::Completed => "completed",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PostRefusal {
    NotRecorded,
    DesiredChanged,
    ObservationError,
    CoreNotRunning,
    ReadinessFalse,
    TunUnverified,
    CoreCountMismatch,
    TunCountMismatch,
    ProfileMismatch,
    OtherReconcile,
}
impl PostRefusal {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotRecorded => "not_recorded",
            Self::DesiredChanged => "desired_changed",
            Self::ObservationError => "observation_error",
            Self::CoreNotRunning => "core_not_running",
            Self::ReadinessFalse => "readiness_false",
            Self::TunUnverified => "tun_unverified",
            Self::CoreCountMismatch => "core_count_mismatch",
            Self::TunCountMismatch => "tun_count_mismatch",
            Self::ProfileMismatch => "profile_mismatch",
            Self::OtherReconcile => "other_reconcile",
        }
    }
}
pub(crate) struct PostScope(bool);
impl Drop for PostScope {
    fn drop(&mut self) {
        POST_ACTIVE.set(self.0);
    }
}
pub(crate) fn post_scope() -> PostScope {
    let old = POST_ACTIVE.replace(true);
    POST.set((PostGuard::NotEntered, PostRefusal::NotRecorded));
    PostScope(old)
}
pub(crate) fn post_mark(guard: PostGuard) {
    if POST_ACTIVE.get() {
        POST.set((guard, POST.get().1));
    }
}
pub(crate) fn post_refusal(reason: PostRefusal) {
    // Existing observation Err is terminal and supersedes an earlier observed
    // false flag. Otherwise retain the first cached unsatisfied category.
    if POST_ACTIVE.get()
        && (POST.get().1 == PostRefusal::NotRecorded || reason == PostRefusal::ObservationError)
    {
        POST.set((POST.get().0, reason));
    }
}
pub(crate) fn post() -> (PostGuard, PostRefusal) {
    POST.get()
}

#[test]
fn postcheck_categories_are_scoped_resettable_and_thread_local() {
    mark(Cut::NotEntered);
    post_mark(PostGuard::DesiredRead);
    assert_eq!(post(), (PostGuard::NotEntered, PostRefusal::NotRecorded));
    {
        let _scope = post_scope();
        post_mark(PostGuard::CoreReadiness);
        post_refusal(PostRefusal::ReadinessFalse);
        post_mark(PostGuard::VisibleCoreCount);
        post_refusal(PostRefusal::CoreCountMismatch);
        assert_eq!(
            post(),
            (PostGuard::VisibleCoreCount, PostRefusal::ReadinessFalse)
        );
        post_refusal(PostRefusal::ObservationError);
        assert_eq!(post().1, PostRefusal::ObservationError);
        std::thread::spawn(|| {
            assert_eq!(post(), (PostGuard::NotEntered, PostRefusal::NotRecorded));
            let _scope = post_scope();
            post_mark(PostGuard::DesiredEquality);
        })
        .join()
        .unwrap();
        assert_eq!(post().0, PostGuard::VisibleCoreCount);
    }
    let retained = post();
    post_mark(PostGuard::Completed);
    assert_eq!(post(), retained);
    mark(Cut::NotEntered);
    assert_eq!(post(), (PostGuard::NotEntered, PostRefusal::NotRecorded));
}

#[test]
fn postcheck_hooks_preserve_reached_check_order_and_are_test_feature_only() {
    let candidate = include_str!("lifecycle/protected_candidate.rs");
    let body = candidate
        .split("let _post_scope =")
        .nth(1)
        .unwrap()
        .split("Site::IntervalAfter")
        .next()
        .unwrap();
    for token in [
        "e.read()?",
        "if current != desired",
        ".recheck_interval(&desired)",
        "e.verify_connected(&desired)?",
    ] {
        assert_eq!(body.matches(token).count(), 1);
    }
    let positions = [
        "e.read()?",
        "if current != desired",
        ".recheck_interval(&desired)",
        "e.verify_connected(&desired)?",
    ]
    .map(|s| body.find(s).unwrap());
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    let source = include_str!("native_host/protected_preparation.rs");
    let body = source
        .split("fn recheck_interval(")
        .nth(1)
        .unwrap()
        .split("fn prepare_admitted(")
        .next()
        .unwrap();
    let checks = [
        ".verify_protected()?",
        ".recheck(\n                self.paths",
        "if bound.policy.version",
        "read_private_utf8(&self.paths.store",
        "Sha256::digest(store.as_bytes())",
        ".recheck(&self.paths.core)",
        ".recheck(&self.paths.config_directory.join(STAGING))",
        "bound.data.file.metadata()",
        "fs::symlink_metadata(&self.paths.data_directory)",
    ];
    let positions = checks.map(|s| {
        assert_eq!(body.matches(s).count(), 1);
        body.find(s).unwrap()
    });
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
    for source in [
        include_str!("lifecycle.rs"),
        candidate,
        include_str!("native_host.rs"),
        source,
        include_str!("production_owner/protected_native_vm_tests.rs"),
    ] {
        for (at, _) in source.match_indices("crate::protected_native_diagnostic::post_") {
            let prefix = &source[..at];
            let guard = prefix.rfind("#[cfg(all(test, feature = \"netguard-native-scenario\"))]");
            assert!(guard.is_some_and(|p| at - p < 1700));
        }
    }
    let output = include_str!("production_owner/protected_native_vm_tests.rs");
    assert_eq!(output.matches("K1_NATIVE_POSTCHECK_DIAGNOSTIC").count(), 1);
    assert!(
        output.find("K1_NATIVE_READINESS_DIAGNOSTIC").unwrap()
            < output.find("K1_NATIVE_POSTCHECK_DIAGNOSTIC").unwrap()
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Endpoint {
    NotEntered,
    Configs,
    Rules,
    Providers,
    Proxies,
}
impl Endpoint {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::Configs => "configs",
            Self::Rules => "rules",
            Self::Providers => "providers",
            Self::Proxies => "proxies",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadinessPhase {
    NotEntered,
    Deadline,
    Read,
    ReadUnavailable,
    Policy,
    PolicyRejected,
    FinalDeadline,
}
impl ReadinessPhase {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::Deadline => "deadline",
            Self::Read => "read",
            Self::ReadUnavailable => "read_unavailable",
            Self::Policy => "policy",
            Self::PolicyRejected => "policy_rejected",
            Self::FinalDeadline => "final_deadline",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControllerPhase {
    NotEntered,
    ParentMetadata,
    SocketMetadata,
    MetadataPolicy,
    Exchange,
    Timeout,
}
impl ControllerPhase {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::NotEntered => "not_entered",
            Self::ParentMetadata => "parent_metadata",
            Self::SocketMetadata => "socket_metadata",
            Self::MetadataPolicy => "metadata_policy",
            Self::Exchange => "exchange",
            Self::Timeout => "timeout",
        }
    }
}
pub(crate) struct ReadinessScope(bool);
impl Drop for ReadinessScope {
    fn drop(&mut self) {
        READ_ACTIVE.set(self.0);
    }
}
pub(crate) fn readiness_scope() -> ReadinessScope {
    let old = READ_ACTIVE.replace(true);
    ReadinessScope(old)
}
pub(crate) fn readiness_mark(endpoint: Endpoint, phase: ReadinessPhase) {
    let controller = if phase == ReadinessPhase::Deadline {
        ControllerPhase::NotEntered
    } else {
        READINESS.get().2
    };
    READINESS.set((endpoint, phase, controller));
}
pub(crate) fn controller_mark(phase: ControllerPhase) {
    if READ_ACTIVE.get() {
        let (e, p, _) = READINESS.get();
        READINESS.set((e, p, phase));
    }
}
pub(crate) fn readiness() -> (Endpoint, ReadinessPhase, ControllerPhase) {
    READINESS.get()
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
        crate::login_transaction::diagnostic::reset();
        POST_ACTIVE.set(false);
        POST.set((PostGuard::NotEntered, PostRefusal::NotRecorded));
        READINESS.set((
            Endpoint::NotEntered,
            ReadinessPhase::NotEntered,
            ControllerPhase::NotEntered,
        ));
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

#[test]
fn readiness_catalogue_is_closed_scoped_resettable_and_thread_local() {
    mark(Cut::NotEntered);
    controller_mark(ControllerPhase::ParentMetadata);
    assert_eq!(
        readiness(),
        (
            Endpoint::NotEntered,
            ReadinessPhase::NotEntered,
            ControllerPhase::NotEntered
        )
    );
    {
        let _scope = readiness_scope();
        readiness_mark(Endpoint::Configs, ReadinessPhase::Deadline);
        readiness_mark(Endpoint::Configs, ReadinessPhase::Read);
        controller_mark(ControllerPhase::SocketMetadata);
        readiness_mark(Endpoint::Configs, ReadinessPhase::ReadUnavailable);
        assert_eq!(
            readiness(),
            (
                Endpoint::Configs,
                ReadinessPhase::ReadUnavailable,
                ControllerPhase::SocketMetadata
            )
        );
    }
    controller_mark(ControllerPhase::Timeout);
    assert_eq!(readiness().2, ControllerPhase::SocketMetadata);
    std::thread::spawn(|| {
        assert_eq!(
            readiness(),
            (
                Endpoint::NotEntered,
                ReadinessPhase::NotEntered,
                ControllerPhase::NotEntered
            )
        );
        let _scope = readiness_scope();
        controller_mark(ControllerPhase::Timeout);
    })
    .join()
    .unwrap();
    assert_eq!(readiness().2, ControllerPhase::SocketMetadata);
    mark(Cut::NotEntered);
    assert_eq!(readiness().0, Endpoint::NotEntered);
    assert_eq!(
        [
            Cut::ConfiguredWait,
            Cut::SecureOwned,
            Cut::CoreRunning,
            Cut::TunCheck
        ]
        .map(Cut::token),
        [
            "configured_wait",
            "secure_owned",
            "core_running",
            "tun_check"
        ]
    );
    assert_eq!(
        [
            Endpoint::Configs,
            Endpoint::Rules,
            Endpoint::Providers,
            Endpoint::Proxies
        ]
        .map(Endpoint::token),
        ["configs", "rules", "providers", "proxies"]
    );
    assert_eq!(
        [
            ControllerPhase::ParentMetadata,
            ControllerPhase::SocketMetadata,
            ControllerPhase::MetadataPolicy,
            ControllerPhase::Exchange,
            ControllerPhase::Timeout
        ]
        .map(ControllerPhase::token),
        [
            "parent_metadata",
            "socket_metadata",
            "metadata_policy",
            "exchange",
            "timeout"
        ]
    );
}

#[test]
fn source_cuts_preserve_original_call_order_short_circuit_and_default_absence() {
    let source = include_str!("native_host/protected_preparation.rs");
    let body = source
        .split("fn start_admitted(")
        .nth(1)
        .unwrap()
        .split("fn commit_protected(")
        .next()
        .unwrap();
    let calls = [
        "self.recheck_admission(&admission)?",
        "self.ping_slot.revoke()",
        "self.managed_tuns()?",
        "self.remove_controller()?",
        "OwnedCore::spawn_protected(",
        "self.core = Some(core)",
        "core.wait_configured(",
        "let pid = core.pid()",
        "controller_permissions::secure_owned(",
        "core.running()",
        "self.verify_tun(pid)?",
    ];
    let mut position = 0;
    for call in calls {
        assert_eq!(body.matches(call).count(), 1);
        let next = body.find(call).unwrap();
        assert!(next >= position);
        position = next;
    }
    let secure = body.find("controller_permissions::secure_owned(").unwrap();
    let running = body.find("Cut::CoreRunning").unwrap();
    assert!(body[secure..running].contains(") || !{"));
    assert!(running < body.find("core.running()").unwrap());
    let selectors = include_str!("core_selector.rs");
    let exchange = selectors
        .split("fn exchange(")
        .nth(1)
        .unwrap()
        .split("fn selector")
        .next()
        .unwrap();
    assert_eq!(
        exchange.matches("symlink_metadata(path.parent()?)").count(),
        1
    );
    assert_eq!(exchange.matches("symlink_metadata(path)").count(), 2);
    assert_eq!(
        exchange
            .matches("checked_duration_since(Instant::now())")
            .count(),
        1
    );
    assert!(
        exchange.find("ControllerPhase::ParentMetadata").unwrap()
            < exchange.find("symlink_metadata(path.parent()?)").unwrap()
    );
    assert!(
        exchange.find("ControllerPhase::MetadataPolicy").unwrap()
            < exchange.find("if !directory.is_dir()").unwrap()
    );
    let library = include_str!("lib.rs");
    assert!(library.contains("#[cfg(all(test, feature = \"netguard-native-scenario\"))]\nmod protected_native_diagnostic;"));
    for text in [
        include_str!("core_selector.rs"),
        include_str!("core_readiness.rs"),
        source,
    ] {
        let text = text.split("#[cfg(test)]\nmod tests").next().unwrap();
        for prefix in text
            .split("crate::protected_native_diagnostic::")
            .take(text.matches("crate::protected_native_diagnostic::").count())
        {
            assert!(
                prefix
                    .rsplit("#[cfg(all(test, feature = \"netguard-native-scenario\"))]")
                    .next()
                    .unwrap()
                    .len()
                    < 2500
            );
        }
    }
    let output = include_str!("production_owner/protected_native_vm_tests.rs");
    assert!(
        output.find("std::panic::catch_unwind(||").unwrap()
            < output.find("K1_NATIVE_READINESS_DIAGNOSTIC").unwrap()
    );
}
