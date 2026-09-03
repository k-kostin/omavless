// SPDX-License-Identifier: MIT

//! Deterministic orchestration for the future explicit R5 ownership cutover.
//!
//! This module has no production host adapter or CLI entry point. It sequences
//! only fixed-purpose host operations behind [`CutoverTransactionHost`], and
//! compensates back to a verified legacy state on any failure. A later host
//! adapter must hold the shared migration lock for the complete call.

use crate::cutover::{
    CutoverReadiness, LegacyCommitEvidence, OwnershipMarker, OwnershipObservation, OwnershipPhase,
    RustCommitEvidence, abort_cutover, begin_cutover, commit_cutover, evaluate_cutover,
};
use std::fmt;

/// Fixed bridge targets. This is deliberately not a path, command, or argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeTarget {
    Legacy,
    Rust,
}

/// Opaque host-step failure. Host output and private data never cross into the
/// transaction result or public logs through this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutoverHostError;

/// Fixed-purpose operations required by one cutover transaction.
///
/// The production implementation must be constructed while holding the shared
/// migration lock. Every compensation method must be idempotent. In
/// particular, `stop_rust` must prove the native owner/core absent before
/// `restore_legacy` may be called, so rollback cannot create two owners.
pub trait CutoverTransactionHost {
    fn observe(&mut self) -> Result<OwnershipObservation, CutoverHostError>;
    fn persist_marker(
        &mut self,
        expected: &OwnershipMarker,
        next: &OwnershipMarker,
    ) -> Result<(), CutoverHostError>;
    fn stage_desired(&mut self, readiness: CutoverReadiness) -> Result<(), CutoverHostError>;
    fn stop_legacy(&mut self) -> Result<(), CutoverHostError>;
    fn start_rust(&mut self) -> Result<(), CutoverHostError>;
    fn hello_compatible(&mut self) -> Result<bool, CutoverHostError>;
    fn status_consistent(&mut self) -> Result<bool, CutoverHostError>;
    fn switch_bridge(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError>;
    fn stop_rust(&mut self) -> Result<(), CutoverHostError>;
    fn restore_legacy(&mut self, readiness: CutoverReadiness) -> Result<(), CutoverHostError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverTransactionError {
    PreconditionsFailed,
    TransitionFailedRestored,
    ManualRecoveryRequired,
}

impl fmt::Display for CutoverTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PreconditionsFailed => "Ownership cutover preconditions are not satisfied",
            Self::TransitionFailedRestored => "Ownership cutover failed and was restored",
            Self::ManualRecoveryRequired => "Ownership cutover requires manual recovery",
        })
    }
}

impl std::error::Error for CutoverTransactionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoverTransactionOutcome {
    pub marker: OwnershipMarker,
    pub initial_readiness: CutoverReadiness,
}

fn restore_legacy<H: CutoverTransactionHost>(
    host: &mut H,
    preparing: &OwnershipMarker,
    readiness: CutoverReadiness,
) -> Result<(), CutoverTransactionError> {
    // Always try to return the compatibility bridge first. A failure here is
    // a hard blocker, but the native owner must still be stopped if possible.
    let bridge_legacy = host.switch_bridge(BridgeTarget::Legacy).is_ok();
    if host.stop_rust().is_err() {
        // Never start/restore legacy while native ownership may still exist.
        return Err(CutoverTransactionError::ManualRecoveryRequired);
    }
    if host.restore_legacy(readiness).is_err() {
        return Err(CutoverTransactionError::ManualRecoveryRequired);
    }
    let observation = host
        .observe()
        .map_err(|_| CutoverTransactionError::ManualRecoveryRequired)?;
    let legacy = abort_cutover(
        preparing,
        LegacyCommitEvidence {
            plugin_bridge_legacy: bridge_legacy,
            observation,
        },
    )
    .map_err(|_| CutoverTransactionError::ManualRecoveryRequired)?;
    host.persist_marker(preparing, &legacy)
        .map_err(|_| CutoverTransactionError::ManualRecoveryRequired)
}

/// Execute one fail-closed legacy-to-Rust ownership transaction.
///
/// The caller supplies the marker read under the same lock held by `host`.
/// The function never retries service, bridge, controller, or marker actions.
/// On any post-marker failure it runs one bounded compensation pass. A failed
/// compensation deliberately leaves `cutoverPreparing` as a hard blocker.
pub fn execute_cutover<H: CutoverTransactionHost>(
    host: &mut H,
    marker: &OwnershipMarker,
) -> Result<CutoverTransactionOutcome, CutoverTransactionError> {
    if marker.phase() != OwnershipPhase::Legacy {
        return Err(CutoverTransactionError::PreconditionsFailed);
    }
    let initial_observation = host
        .observe()
        .map_err(|_| CutoverTransactionError::PreconditionsFailed)?;
    let readiness = evaluate_cutover(marker, initial_observation);
    let preparing = begin_cutover(marker, readiness)
        .map_err(|_| CutoverTransactionError::PreconditionsFailed)?;
    host.persist_marker(marker, &preparing)
        .map_err(|_| CutoverTransactionError::PreconditionsFailed)?;

    let transition = (|| {
        host.stage_desired(readiness)?;
        host.stop_legacy()?;
        host.start_rust()?;
        let hello_verified = host.hello_compatible()?;
        let status_verified = host.status_consistent()?;
        if !hello_verified || !status_verified {
            return Err(CutoverHostError);
        }
        host.switch_bridge(BridgeTarget::Rust)?;
        let observation = host.observe()?;
        let rust = commit_cutover(
            &preparing,
            RustCommitEvidence {
                hello_verified,
                status_verified,
                plugin_bridge_switched: true,
                observation,
            },
        )
        .map_err(|_| CutoverHostError)?;
        host.persist_marker(&preparing, &rust)?;
        Ok::<OwnershipMarker, CutoverHostError>(rust)
    })();

    match transition {
        Ok(rust) => Ok(CutoverTransactionOutcome {
            marker: rust,
            initial_readiness: readiness,
        }),
        Err(_) => {
            restore_legacy(host, &preparing, readiness)?;
            Err(CutoverTransactionError::TransitionFailedRestored)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::CutoverBlocker;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Step {
        Observe,
        Persist,
        StageDesired,
        StopLegacy,
        StartRust,
        Hello,
        Status,
        BridgeLegacy,
        BridgeRust,
        StopRust,
        RestoreLegacy,
    }

    struct FakeHost {
        calls: Vec<Step>,
        marker: OwnershipMarker,
        initial: OwnershipObservation,
        current: OwnershipObservation,
        fail_at: Option<(Step, usize)>,
        hello: bool,
        status: bool,
    }

    fn disconnected() -> OwnershipObservation {
        OwnershipObservation::disconnected()
    }

    fn legacy_connected() -> OwnershipObservation {
        OwnershipObservation {
            legacy_owner_active: true,
            rust_owner_active: false,
            legacy_controller_ready: true,
            rust_controller_ready: false,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        }
    }

    fn rust_connected() -> OwnershipObservation {
        OwnershipObservation {
            legacy_owner_active: false,
            rust_owner_active: true,
            legacy_controller_ready: false,
            rust_controller_ready: true,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        }
    }

    impl FakeHost {
        fn new(initial: OwnershipObservation) -> Self {
            Self {
                calls: Vec::new(),
                marker: OwnershipMarker::default(),
                initial,
                current: initial,
                fail_at: None,
                hello: true,
                status: true,
            }
        }

        fn fail(&mut self, step: Step) -> Result<(), CutoverHostError> {
            let occurrence = self.calls.iter().filter(|item| **item == step).count() + 1;
            self.calls.push(step);
            if self.fail_at == Some((step, occurrence)) {
                self.fail_at = None;
                Err(CutoverHostError)
            } else {
                Ok(())
            }
        }
    }

    impl CutoverTransactionHost for FakeHost {
        fn observe(&mut self) -> Result<OwnershipObservation, CutoverHostError> {
            self.fail(Step::Observe)?;
            Ok(self.current)
        }

        fn persist_marker(
            &mut self,
            expected: &OwnershipMarker,
            next: &OwnershipMarker,
        ) -> Result<(), CutoverHostError> {
            self.fail(Step::Persist)?;
            if &self.marker != expected {
                return Err(CutoverHostError);
            }
            self.marker = next.clone();
            Ok(())
        }

        fn stage_desired(&mut self, _readiness: CutoverReadiness) -> Result<(), CutoverHostError> {
            self.fail(Step::StageDesired)
        }

        fn stop_legacy(&mut self) -> Result<(), CutoverHostError> {
            self.current = disconnected();
            self.fail(Step::StopLegacy)?;
            Ok(())
        }

        fn start_rust(&mut self) -> Result<(), CutoverHostError> {
            self.current = if self.initial.legacy_owner_active {
                rust_connected()
            } else {
                let mut value = disconnected();
                value.rust_owner_active = true;
                value
            };
            self.fail(Step::StartRust)?;
            Ok(())
        }

        fn hello_compatible(&mut self) -> Result<bool, CutoverHostError> {
            self.fail(Step::Hello)?;
            Ok(self.hello)
        }

        fn status_consistent(&mut self) -> Result<bool, CutoverHostError> {
            self.fail(Step::Status)?;
            Ok(self.status)
        }

        fn switch_bridge(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError> {
            self.fail(match target {
                BridgeTarget::Legacy => Step::BridgeLegacy,
                BridgeTarget::Rust => Step::BridgeRust,
            })
        }

        fn stop_rust(&mut self) -> Result<(), CutoverHostError> {
            self.fail(Step::StopRust)?;
            self.current = disconnected();
            Ok(())
        }

        fn restore_legacy(&mut self, _readiness: CutoverReadiness) -> Result<(), CutoverHostError> {
            self.fail(Step::RestoreLegacy)?;
            self.current = self.initial;
            Ok(())
        }
    }

    #[test]
    fn disconnected_and_connected_cutovers_follow_one_exact_order() {
        for (initial, expected_readiness) in [
            (disconnected(), CutoverReadiness::ReadyDisconnected),
            (legacy_connected(), CutoverReadiness::ReadyToAdopt),
        ] {
            let marker = OwnershipMarker::default();
            let mut host = FakeHost::new(initial);
            let outcome = execute_cutover(&mut host, &marker).unwrap();
            assert_eq!(outcome.initial_readiness, expected_readiness);
            assert_eq!(outcome.marker.phase(), OwnershipPhase::Rust);
            assert_eq!(host.marker, outcome.marker);
            assert_eq!(
                host.calls,
                [
                    Step::Observe,
                    Step::Persist,
                    Step::StageDesired,
                    Step::StopLegacy,
                    Step::StartRust,
                    Step::Hello,
                    Step::Status,
                    Step::BridgeRust,
                    Step::Observe,
                    Step::Persist,
                ]
            );
        }
    }

    #[test]
    fn every_transition_step_failure_restores_verified_legacy_ownership() {
        for failed in [
            Step::StageDesired,
            Step::StopLegacy,
            Step::StartRust,
            Step::Hello,
            Step::Status,
            Step::BridgeRust,
        ] {
            let marker = OwnershipMarker::default();
            let mut host = FakeHost::new(legacy_connected());
            host.fail_at = Some((failed, 1));
            assert_eq!(
                execute_cutover(&mut host, &marker),
                Err(CutoverTransactionError::TransitionFailedRestored),
                "failed step: {failed:?}"
            );
            assert_eq!(host.marker.phase(), OwnershipPhase::Legacy);
            assert_eq!(host.current, legacy_connected());
            let tail = &host.calls[host.calls.len() - 5..];
            assert_eq!(
                tail,
                [
                    Step::BridgeLegacy,
                    Step::StopRust,
                    Step::RestoreLegacy,
                    Step::Observe,
                    Step::Persist,
                ]
            );
        }
    }

    #[test]
    fn failed_verification_or_final_marker_commit_is_compensated() {
        for false_hello in [true, false] {
            let marker = OwnershipMarker::default();
            let mut host = FakeHost::new(disconnected());
            if false_hello {
                host.hello = false;
            } else {
                host.status = false;
            }
            assert_eq!(
                execute_cutover(&mut host, &marker),
                Err(CutoverTransactionError::TransitionFailedRestored)
            );
            assert_eq!(host.marker.phase(), OwnershipPhase::Legacy);
            assert_eq!(host.current, disconnected());
        }

        let marker = OwnershipMarker::default();
        let mut host = FakeHost::new(disconnected());
        host.fail_at = Some((Step::Persist, 2));
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::TransitionFailedRestored)
        );
        assert_eq!(host.marker.phase(), OwnershipPhase::Legacy);
        assert_eq!(
            host.calls
                .iter()
                .filter(|step| **step == Step::Persist)
                .count(),
            3
        );
    }

    #[test]
    fn uncertain_native_stop_never_restores_legacy_or_clears_preparing_marker() {
        let marker = OwnershipMarker::default();
        let mut host = FakeHost::new(legacy_connected());
        host.fail_at = Some((Step::StopRust, 1));
        host.hello = false;
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::ManualRecoveryRequired)
        );
        assert_eq!(host.marker.phase(), OwnershipPhase::CutoverPreparing);
        assert!(!host.calls.contains(&Step::RestoreLegacy));
    }

    #[test]
    fn failed_legacy_bridge_restore_still_cleans_owners_but_stays_blocked() {
        let marker = OwnershipMarker::default();
        let mut host = FakeHost::new(legacy_connected());
        host.fail_at = Some((Step::BridgeLegacy, 1));
        host.hello = false;
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::ManualRecoveryRequired)
        );
        assert_eq!(host.current, legacy_connected());
        assert_eq!(host.marker.phase(), OwnershipPhase::CutoverPreparing);
    }

    #[test]
    fn any_late_compensation_failure_preserves_the_preparing_blocker() {
        for (step, occurrence) in [
            (Step::RestoreLegacy, 1),
            (Step::Observe, 2),
            (Step::Persist, 2),
        ] {
            let marker = OwnershipMarker::default();
            let mut host = FakeHost::new(legacy_connected());
            host.hello = false;
            host.fail_at = Some((step, occurrence));
            assert_eq!(
                execute_cutover(&mut host, &marker),
                Err(CutoverTransactionError::ManualRecoveryRequired),
                "failed compensation: {step:?}"
            );
            assert_eq!(host.marker.phase(), OwnershipPhase::CutoverPreparing);
        }
    }

    #[test]
    fn initial_marker_compare_and_swap_failure_changes_nothing() {
        let marker = OwnershipMarker::default();
        let mut host = FakeHost::new(disconnected());
        host.fail_at = Some((Step::Persist, 1));
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::PreconditionsFailed)
        );
        assert_eq!(host.marker.phase(), OwnershipPhase::Legacy);
        assert_eq!(host.calls, [Step::Observe, Step::Persist]);
    }

    #[test]
    fn blocked_or_nonlegacy_preflight_has_no_mutating_side_effect() {
        let marker = OwnershipMarker::default();
        let mut inconsistent = legacy_connected();
        inconsistent.core_count = 2;
        let mut host = FakeHost::new(inconsistent);
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::PreconditionsFailed)
        );
        assert_eq!(host.calls, [Step::Observe]);
        assert_eq!(
            evaluate_cutover(&marker, inconsistent),
            CutoverReadiness::Blocked(CutoverBlocker::InconsistentHostState)
        );

        let preparing = begin_cutover(&marker, CutoverReadiness::ReadyDisconnected).unwrap();
        let mut host = FakeHost::new(disconnected());
        assert_eq!(
            execute_cutover(&mut host, &preparing),
            Err(CutoverTransactionError::PreconditionsFailed)
        );
        assert!(host.calls.is_empty());
    }

    #[test]
    fn errors_are_fixed_and_cannot_echo_host_private_data() {
        let rendered = format!(
            "{:?} {}",
            CutoverHostError,
            CutoverTransactionError::ManualRecoveryRequired
        );
        for private in ["private.example", "password", "vless://", "uuid"] {
            assert!(!rendered.contains(private));
        }
    }
}
