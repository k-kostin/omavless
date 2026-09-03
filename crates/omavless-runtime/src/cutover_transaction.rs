// SPDX-License-Identifier: MIT

//! Deterministic orchestration for the future explicit R5 ownership cutover.
//!
//! This module has no production host adapter or CLI entry point. It sequences
//! only fixed-purpose host operations behind [`CutoverTransactionHost`], and
//! compensates back to a verified legacy state on any failure. A later host
//! adapter must make the explicit candidate lock handoff below atomic with
//! respect to the durable preparing marker.

use crate::cutover::{
    CutoverReadiness, LegacyCommitEvidence, OwnershipMarker, OwnershipObservation, OwnershipPhase,
    RustCommitEvidence, TransitionBootstrap, abort_cutover, begin_cutover, commit_cutover,
    evaluate_cutover,
};
use sha2::{Digest, Sha256};
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

/// Bounded, credential-free identity for one exact transition candidate.
/// The original runtime instance identifier is hashed and never retained in
/// transaction state, errors, or debug output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CandidateIdentity {
    digest: [u8; 32],
    bootstrap: TransitionBootstrap,
}

impl CandidateIdentity {
    pub fn from_instance_id(
        instance_id: &str,
        bootstrap: TransitionBootstrap,
    ) -> Result<Self, CutoverHostError> {
        if instance_id.is_empty() || instance_id.len() > 128 {
            return Err(CutoverHostError);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"omavless/cutover-candidate/v1\0");
        hasher.update(instance_id.as_bytes());
        hasher.update(bootstrap.preparing_generation().to_be_bytes());
        hasher.update(bootstrap.rust_generation().to_be_bytes());
        Ok(Self {
            digest: hasher.finalize().into(),
            bootstrap,
        })
    }

    #[must_use]
    pub const fn bootstrap(self) -> TransitionBootstrap {
        self.bootstrap
    }
}

/// Fixed-purpose operations required by one cutover transaction.
///
/// The production implementation starts with the shared migration lock held.
/// `release_for_candidate` and `reacquire_after_candidate` are the only legal
/// handoff: the durable exact preparing marker blocks legacy mutations while
/// the candidate acquires the same lock for reconciliation. Every
/// compensation method must be idempotent. `stop_rust` must positively prove
/// the runtime, owner lock, socket, controller, core, and owned TUN absent
/// before `restore_legacy` may be called. A successful bridge operation means
/// the target was positively observed, not merely that a command was sent.
/// A failed `release_for_candidate` must leave the caller's lock held.
pub trait CutoverTransactionHost {
    type DesiredSnapshot;

    fn observe(&mut self) -> Result<OwnershipObservation, CutoverHostError>;
    fn read_marker(&mut self) -> Result<OwnershipMarker, CutoverHostError>;
    fn persist_marker(
        &mut self,
        expected: &OwnershipMarker,
        next: &OwnershipMarker,
    ) -> Result<(), CutoverHostError>;
    fn capture_desired(&mut self) -> Result<Self::DesiredSnapshot, CutoverHostError>;
    fn stage_desired(&mut self, readiness: CutoverReadiness) -> Result<(), CutoverHostError>;
    fn stop_legacy(&mut self) -> Result<(), CutoverHostError>;
    fn release_for_candidate(
        &mut self,
        preparing: &OwnershipMarker,
    ) -> Result<(), CutoverHostError>;
    fn start_rust_candidate(
        &mut self,
        bootstrap: TransitionBootstrap,
    ) -> Result<CandidateIdentity, CutoverHostError>;
    fn hello_compatible(&mut self, candidate: CandidateIdentity) -> Result<bool, CutoverHostError>;
    fn status_consistent(&mut self, candidate: CandidateIdentity)
    -> Result<bool, CutoverHostError>;
    fn reacquire_after_candidate(
        &mut self,
        preparing: &OwnershipMarker,
    ) -> Result<(), CutoverHostError>;
    fn observe_candidate(
        &mut self,
        candidate: CandidateIdentity,
    ) -> Result<OwnershipObservation, CutoverHostError>;
    fn switch_bridge(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError>;
    fn stop_rust(&mut self) -> Result<(), CutoverHostError>;
    fn restore_legacy(
        &mut self,
        readiness: CutoverReadiness,
        desired: &Self::DesiredSnapshot,
    ) -> Result<(), CutoverHostError>;
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
    desired: &H::DesiredSnapshot,
) -> Result<(), CutoverTransactionError> {
    // Always try to return the compatibility bridge first. A failure here is
    // a hard blocker, but the native owner must still be stopped if possible.
    let bridge_legacy = host.switch_bridge(BridgeTarget::Legacy).is_ok();
    if host.stop_rust().is_err() {
        // Never start/restore legacy while native ownership may still exist.
        return Err(CutoverTransactionError::ManualRecoveryRequired);
    }
    if host.restore_legacy(readiness, desired).is_err() {
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
    match persist_resolved(host, preparing, &legacy) {
        PersistResolution::Committed => Ok(()),
        PersistResolution::SourceUnchanged | PersistResolution::Unknown => {
            Err(CutoverTransactionError::ManualRecoveryRequired)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistResolution {
    Committed,
    SourceUnchanged,
    Unknown,
}

fn persist_resolved<H: CutoverTransactionHost>(
    host: &mut H,
    expected: &OwnershipMarker,
    next: &OwnershipMarker,
) -> PersistResolution {
    if host.persist_marker(expected, next).is_ok() {
        return PersistResolution::Committed;
    }
    match host.read_marker() {
        Ok(marker) if marker == *next => PersistResolution::Committed,
        Ok(marker) if marker == *expected => PersistResolution::SourceUnchanged,
        Ok(_) | Err(_) => PersistResolution::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransitionFailure {
    Compensate,
    ManualRecovery,
}

/// Execute one fail-closed legacy-to-Rust ownership transaction.
///
/// The caller supplies the marker read under the same lock initially held by
/// `host`. The lock is released only after the preparing marker is durable and
/// only while the exact candidate starts and verifies. It is reacquired and
/// the marker rechecked before bridge or marker commit. Marker-write errors are
/// resolved by rereading durable state before any compensation decision.
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
    let bootstrap = TransitionBootstrap::from_preparing(&preparing)
        .map_err(|_| CutoverTransactionError::PreconditionsFailed)?;
    let desired = host
        .capture_desired()
        .map_err(|_| CutoverTransactionError::PreconditionsFailed)?;
    match persist_resolved(host, marker, &preparing) {
        PersistResolution::Committed => {}
        PersistResolution::SourceUnchanged => {
            return Err(CutoverTransactionError::PreconditionsFailed);
        }
        PersistResolution::Unknown => {
            return Err(CutoverTransactionError::ManualRecoveryRequired);
        }
    }

    let mut released = false;
    let transition = (|| -> Result<OwnershipMarker, TransitionFailure> {
        host.stage_desired(readiness)
            .map_err(|_| TransitionFailure::Compensate)?;
        host.stop_legacy()
            .map_err(|_| TransitionFailure::Compensate)?;
        host.release_for_candidate(&preparing)
            .map_err(|_| TransitionFailure::Compensate)?;
        released = true;
        let candidate = host
            .start_rust_candidate(bootstrap)
            .map_err(|_| TransitionFailure::Compensate)?;
        if candidate.bootstrap() != bootstrap {
            return Err(TransitionFailure::Compensate);
        }
        let hello_verified = host
            .hello_compatible(candidate)
            .map_err(|_| TransitionFailure::Compensate)?;
        let status_verified = host
            .status_consistent(candidate)
            .map_err(|_| TransitionFailure::Compensate)?;
        if !hello_verified || !status_verified {
            return Err(TransitionFailure::Compensate);
        }
        host.reacquire_after_candidate(&preparing)
            .map_err(|_| TransitionFailure::ManualRecovery)?;
        released = false;
        if host.read_marker().ok().as_ref() != Some(&preparing) {
            return Err(TransitionFailure::ManualRecovery);
        }
        host.switch_bridge(BridgeTarget::Rust)
            .map_err(|_| TransitionFailure::Compensate)?;
        let observation = host
            .observe_candidate(candidate)
            .map_err(|_| TransitionFailure::Compensate)?;
        let rust = commit_cutover(
            &preparing,
            RustCommitEvidence {
                hello_verified,
                status_verified,
                plugin_bridge_switched: true,
                observation,
            },
        )
        .map_err(|_| TransitionFailure::Compensate)?;
        match persist_resolved(host, &preparing, &rust) {
            PersistResolution::Committed => Ok(rust),
            PersistResolution::SourceUnchanged => Err(TransitionFailure::Compensate),
            PersistResolution::Unknown => Err(TransitionFailure::ManualRecovery),
        }
    })();

    match transition {
        Ok(rust) => Ok(CutoverTransactionOutcome {
            marker: rust,
            initial_readiness: readiness,
        }),
        Err(failure) => {
            if released {
                host.reacquire_after_candidate(&preparing)
                    .map_err(|_| CutoverTransactionError::ManualRecoveryRequired)?;
            }
            if failure == TransitionFailure::ManualRecovery
                || host.read_marker().ok().as_ref() != Some(&preparing)
            {
                return Err(CutoverTransactionError::ManualRecoveryRequired);
            }
            restore_legacy(host, &preparing, readiness, &desired)?;
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
        ObserveCandidate,
        ReadMarker,
        Persist,
        CaptureDesired,
        StageDesired,
        StopLegacy,
        StartRust,
        Hello,
        Status,
        Release,
        Reacquire,
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
        fail_after_persist_at: Option<usize>,
        hello: bool,
        status: bool,
        lock_held: bool,
        candidate: Option<CandidateIdentity>,
        restart_before_status: bool,
        wrong_candidate_bootstrap: bool,
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
                fail_after_persist_at: None,
                hello: true,
                status: true,
                lock_held: true,
                candidate: None,
                restart_before_status: false,
                wrong_candidate_bootstrap: false,
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
        type DesiredSnapshot = u8;

        fn observe(&mut self) -> Result<OwnershipObservation, CutoverHostError> {
            self.fail(Step::Observe)?;
            Ok(self.current)
        }

        fn read_marker(&mut self) -> Result<OwnershipMarker, CutoverHostError> {
            self.fail(Step::ReadMarker)?;
            Ok(self.marker.clone())
        }

        fn persist_marker(
            &mut self,
            expected: &OwnershipMarker,
            next: &OwnershipMarker,
        ) -> Result<(), CutoverHostError> {
            self.fail(Step::Persist)?;
            if !self.lock_held {
                return Err(CutoverHostError);
            }
            if &self.marker != expected {
                return Err(CutoverHostError);
            }
            self.marker = next.clone();
            let occurrence = self
                .calls
                .iter()
                .filter(|step| **step == Step::Persist)
                .count();
            if self.fail_after_persist_at == Some(occurrence) {
                self.fail_after_persist_at = None;
                return Err(CutoverHostError);
            }
            Ok(())
        }

        fn capture_desired(&mut self) -> Result<Self::DesiredSnapshot, CutoverHostError> {
            self.fail(Step::CaptureDesired)?;
            Ok(7)
        }

        fn stage_desired(&mut self, _readiness: CutoverReadiness) -> Result<(), CutoverHostError> {
            self.fail(Step::StageDesired)
        }

        fn stop_legacy(&mut self) -> Result<(), CutoverHostError> {
            self.current = disconnected();
            self.fail(Step::StopLegacy)?;
            Ok(())
        }

        fn release_for_candidate(
            &mut self,
            preparing: &OwnershipMarker,
        ) -> Result<(), CutoverHostError> {
            self.fail(Step::Release)?;
            if !self.lock_held || &self.marker != preparing {
                return Err(CutoverHostError);
            }
            self.lock_held = false;
            Ok(())
        }

        fn start_rust_candidate(
            &mut self,
            bootstrap: TransitionBootstrap,
        ) -> Result<CandidateIdentity, CutoverHostError> {
            if self.lock_held {
                return Err(CutoverHostError);
            }
            self.current = if self.initial.legacy_owner_active {
                rust_connected()
            } else {
                let mut value = disconnected();
                value.rust_owner_active = true;
                value
            };
            self.fail(Step::StartRust)?;
            let candidate_bootstrap = if self.wrong_candidate_bootstrap {
                let first_preparing = begin_cutover(
                    &OwnershipMarker::default(),
                    CutoverReadiness::ReadyDisconnected,
                )
                .map_err(|_| CutoverHostError)?;
                let next_legacy = abort_cutover(
                    &first_preparing,
                    LegacyCommitEvidence {
                        plugin_bridge_legacy: true,
                        observation: disconnected(),
                    },
                )
                .map_err(|_| CutoverHostError)?;
                let later_preparing =
                    begin_cutover(&next_legacy, CutoverReadiness::ReadyDisconnected)
                        .map_err(|_| CutoverHostError)?;
                TransitionBootstrap::from_preparing(&later_preparing)
                    .map_err(|_| CutoverHostError)?
            } else {
                bootstrap
            };
            let candidate =
                CandidateIdentity::from_instance_id("candidate-a", candidate_bootstrap)?;
            self.candidate = Some(candidate);
            Ok(candidate)
        }

        fn hello_compatible(
            &mut self,
            candidate: CandidateIdentity,
        ) -> Result<bool, CutoverHostError> {
            self.fail(Step::Hello)?;
            Ok(self.hello && self.candidate == Some(candidate))
        }

        fn status_consistent(
            &mut self,
            candidate: CandidateIdentity,
        ) -> Result<bool, CutoverHostError> {
            self.fail(Step::Status)?;
            if self.restart_before_status {
                self.candidate = Some(CandidateIdentity::from_instance_id(
                    "candidate-b",
                    candidate.bootstrap(),
                )?);
            }
            Ok(self.status && self.candidate == Some(candidate))
        }

        fn reacquire_after_candidate(
            &mut self,
            preparing: &OwnershipMarker,
        ) -> Result<(), CutoverHostError> {
            self.fail(Step::Reacquire)?;
            if self.lock_held || &self.marker != preparing {
                return Err(CutoverHostError);
            }
            self.lock_held = true;
            Ok(())
        }

        fn observe_candidate(
            &mut self,
            candidate: CandidateIdentity,
        ) -> Result<OwnershipObservation, CutoverHostError> {
            self.fail(Step::ObserveCandidate)?;
            if self.candidate != Some(candidate) {
                return Err(CutoverHostError);
            }
            Ok(self.current)
        }

        fn switch_bridge(&mut self, target: BridgeTarget) -> Result<(), CutoverHostError> {
            self.fail(match target {
                BridgeTarget::Legacy => Step::BridgeLegacy,
                BridgeTarget::Rust => Step::BridgeRust,
            })
        }

        fn stop_rust(&mut self) -> Result<(), CutoverHostError> {
            if !self.lock_held {
                return Err(CutoverHostError);
            }
            self.fail(Step::StopRust)?;
            self.current = disconnected();
            self.candidate = None;
            Ok(())
        }

        fn restore_legacy(
            &mut self,
            _readiness: CutoverReadiness,
            desired: &Self::DesiredSnapshot,
        ) -> Result<(), CutoverHostError> {
            if !self.lock_held || *desired != 7 {
                return Err(CutoverHostError);
            }
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
                    Step::CaptureDesired,
                    Step::Persist,
                    Step::StageDesired,
                    Step::StopLegacy,
                    Step::Release,
                    Step::StartRust,
                    Step::Hello,
                    Step::Status,
                    Step::Reacquire,
                    Step::ReadMarker,
                    Step::BridgeRust,
                    Step::ObserveCandidate,
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
            Step::Release,
            Step::StartRust,
            Step::Hello,
            Step::Status,
            Step::BridgeRust,
            Step::ObserveCandidate,
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
    fn acknowledged_lost_marker_writes_are_resolved_from_durable_state() {
        let marker = OwnershipMarker::default();
        let mut initial_ack_lost = FakeHost::new(disconnected());
        initial_ack_lost.fail_after_persist_at = Some(1);
        let outcome = execute_cutover(&mut initial_ack_lost, &marker).unwrap();
        assert_eq!(outcome.marker.phase(), OwnershipPhase::Rust);
        assert_eq!(initial_ack_lost.marker, outcome.marker);
        assert!(initial_ack_lost.calls.contains(&Step::ReadMarker));
        assert!(!initial_ack_lost.calls.contains(&Step::BridgeLegacy));

        let mut final_ack_lost = FakeHost::new(disconnected());
        final_ack_lost.fail_after_persist_at = Some(2);
        let outcome = execute_cutover(&mut final_ack_lost, &marker).unwrap();
        assert_eq!(outcome.marker.phase(), OwnershipPhase::Rust);
        assert_eq!(final_ack_lost.marker, outcome.marker);
        assert!(!final_ack_lost.calls.contains(&Step::BridgeLegacy));
    }

    #[test]
    fn acknowledged_lost_abort_write_is_resolved_as_restored() {
        let marker = OwnershipMarker::default();
        let mut host = FakeHost::new(legacy_connected());
        host.hello = false;
        host.fail_after_persist_at = Some(2);
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::TransitionFailedRestored)
        );
        assert_eq!(host.marker.phase(), OwnershipPhase::Legacy);
    }

    #[test]
    fn failed_reacquire_never_restores_legacy_without_the_lock() {
        let marker = OwnershipMarker::default();
        let mut host = FakeHost::new(legacy_connected());
        host.fail_at = Some((Step::Reacquire, 1));
        assert_eq!(
            execute_cutover(&mut host, &marker),
            Err(CutoverTransactionError::ManualRecoveryRequired)
        );
        assert_eq!(host.marker.phase(), OwnershipPhase::CutoverPreparing);
        assert!(!host.calls.contains(&Step::RestoreLegacy));
    }

    #[test]
    fn candidate_restart_or_generation_mismatch_never_commits() {
        let marker = OwnershipMarker::default();
        let mut restarted = FakeHost::new(legacy_connected());
        restarted.restart_before_status = true;
        assert_eq!(
            execute_cutover(&mut restarted, &marker),
            Err(CutoverTransactionError::TransitionFailedRestored)
        );
        assert_eq!(restarted.marker.phase(), OwnershipPhase::Legacy);

        let mut wrong_generation = FakeHost::new(legacy_connected());
        wrong_generation.wrong_candidate_bootstrap = true;
        assert_eq!(
            execute_cutover(&mut wrong_generation, &marker),
            Err(CutoverTransactionError::TransitionFailedRestored)
        );
        assert_eq!(wrong_generation.marker.phase(), OwnershipPhase::Legacy);
        assert!(!wrong_generation.calls.contains(&Step::Hello));
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
        assert_eq!(
            host.calls,
            [
                Step::Observe,
                Step::CaptureDesired,
                Step::Persist,
                Step::ReadMarker
            ]
        );
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
