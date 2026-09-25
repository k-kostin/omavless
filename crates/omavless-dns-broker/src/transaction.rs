//! Serialized composition of the existing transaction model with actual leaves.
//! No model input or journal assertion is accepted from a channel client.
use crate::{admission::RootContext, journal::Journal};
use omavless_dns_resolved::{Baseline, Error as DnsError, ManagedResolved};
use omavless_dns_retention::Retention;
use omavless_dns_transaction::{
    Action, Completion, LeaseCheck, Phase, PolicyReadback, Prerequisites, Transaction,
};
use std::os::fd::OwnedFd;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Ready,
    Clean,
    Refused,
    RecoveryRequired,
}

/// Private seam; only the host implementation below can dispatch real effects.
trait Effects {
    fn recheck(&mut self) -> bool;
    fn applied_policy_matches(&mut self) -> bool;
    fn perform(&mut self, action: Action) -> Completion;
    fn mark_applied(&mut self) -> bool;
    fn finish(&mut self) -> bool;
    fn quarantine(&mut self);
}

struct Driver<E> {
    model: Transaction,
    effects: E,
}
impl<E: Effects> Driver<E> {
    fn new(effects: E) -> Self {
        // These facts are established by private host admission, never packets.
        // One model per serialized session; tickets never leave this instance.
        let model = Transaction::new(
            1,
            1,
            Prerequisites {
                enrolled_peer: true,
                exclusive_dns_owner: true,
                kernel_bound_lease: true,
                compatible_policy: true,
            },
        )
        .expect("fixed internal transaction identity");
        Self { model, effects }
    }

    fn drive(&mut self) -> Outcome {
        // Fixed finite model: <=8 actions including compensation, no retries.
        for _ in 0..=8 {
            match self.model.phase() {
                Phase::AppliedVerified => {
                    if self.effects.mark_applied() {
                        return Outcome::Ready;
                    }
                    return self.quarantine();
                }
                Phase::Released | Phase::FailedRestored => {
                    return if self.effects.finish() {
                        Outcome::Clean
                    } else {
                        self.quarantine()
                    };
                }
                Phase::Refused => return Outcome::Refused,
                Phase::ManualRecoveryRequired => return self.quarantine(),
                _ => (),
            }
            if !self.effects.recheck() {
                return self.quarantine();
            }
            let Some(ticket) = self.model.next_action(LeaseCheck::Same) else {
                return self.quarantine();
            };
            let completion = self.effects.perform(ticket.action());
            // Actual adapter does pre/post identity checks too. On any manager,
            // lease or retention loss, never convert a settled reply to ready.
            if !self.effects.recheck() {
                return self.quarantine();
            }
            if self
                .model
                .complete(ticket, completion, LeaseCheck::Same)
                .is_err()
            {
                return self.quarantine();
            }
        }
        self.quarantine()
    }

    fn release(&mut self) -> Outcome {
        // Normal release must not erase changes made after Ready. Partial-apply
        // compensation remains inside drive(), without requiring a full policy
        // that was never successfully applied in the first place.
        if self.model.phase() == Phase::AppliedVerified && !self.check_active() {
            return Outcome::RecoveryRequired;
        }
        self.model.release();
        self.drive()
    }

    fn check_active(&mut self) -> bool {
        if self.model.phase() == Phase::AppliedVerified
            && self.effects.recheck()
            && self.effects.applied_policy_matches()
            && self.effects.recheck()
        {
            true
        } else {
            self.quarantine();
            false
        }
    }

    fn quarantine(&mut self) -> Outcome {
        self.model.invalidate();
        self.effects.quarantine();
        Outcome::RecoveryRequired
    }
}

struct Host<'a> {
    context: &'a RootContext,
    journal: &'a mut Journal,
    resolved: ManagedResolved,
    retention: Retention,
    proof: Option<OwnedFd>,
    index: u32,
    baseline: Option<Baseline>,
    intent: bool,
    retained: bool,
    cleanup_started: bool,
}
impl Host<'_> {
    fn renew_budget(&mut self, budget: std::time::Duration) -> bool {
        let until = std::time::Instant::now() + budget;
        self.context.set_deadline(until).is_ok()
            && self.resolved.set_deadline(until).is_ok()
            && self.retention.set_deadline(until).is_ok()
    }
}
impl Effects for Host<'_> {
    fn recheck(&mut self) -> bool {
        self.context.recheck().is_ok()
            && self.resolved.recheck().is_ok()
            && (!self.retained || self.retention.verify().is_ok())
    }

    fn applied_policy_matches(&mut self) -> bool {
        self.baseline.as_ref().is_some_and(|baseline| {
            self.resolved
                .verify_policy_preserves_baseline(baseline)
                .is_ok()
        })
    }

    fn perform(&mut self, action: Action) -> Completion {
        if action == Action::CaptureSnapshot {
            // Historical model name means reserved empty baseline, NOT an
            // arbitrary reversible snapshot. No DNS writes have happened yet.
            return match self.resolved.capture_reserved_baseline() {
                Ok(baseline) => {
                    self.baseline = Some(baseline);
                    Completion::SnapshotCaptured
                }
                Err(_) => Completion::SettledFailure,
            };
        }
        if action == Action::SetFixedServers {
            self.intent = true;
            if self.journal.begin(self.index).is_err() {
                return Completion::OutcomeUnknown;
            }
            let Some(proof) = self.proof.take() else {
                return Completion::OutcomeUnknown;
            };
            if self.retention.retain(proof).is_err() {
                return Completion::OutcomeUnknown;
            }
            self.retained = true;
        }
        if !self.retained || !self.recheck() {
            return Completion::OutcomeUnknown;
        }
        let result = match action {
            Action::SetFixedServers => self.resolved.set_fixed_servers(),
            Action::SetRootRoutingDomain => self.resolved.set_root_routing_domain(),
            Action::SetDefaultRoute => self.resolved.set_default_route(),
            Action::VerifyFixedPolicy => {
                let Some(baseline) = &self.baseline else {
                    return Completion::OutcomeUnknown;
                };
                return match self.resolved.verify_policy_preserves_baseline(baseline) {
                    Ok(()) => Completion::PolicyReadback(PolicyReadback {
                        servers_match: true,
                        domains_match: true,
                        default_route_matches: true,
                    }),
                    Err(DnsError::ReadbackMismatch) => Completion::SettledFailure,
                    Err(_) => Completion::OutcomeUnknown,
                };
            }
            Action::RestoreCapturedSnapshot => {
                if self.cleanup_started || self.journal.begin_release().is_err() {
                    return Completion::OutcomeUnknown;
                }
                self.cleanup_started = true;
                let Some(baseline) = &self.baseline else {
                    return Completion::OutcomeUnknown;
                };
                self.resolved.revert_owned_link(baseline)
            }
            Action::VerifyCapturedSnapshot => {
                let Some(baseline) = &self.baseline else {
                    return Completion::OutcomeUnknown;
                };
                return match self.resolved.verify_reset(baseline) {
                    Ok(()) => Completion::SnapshotReadback { matches: true },
                    Err(_) => Completion::OutcomeUnknown,
                };
            }
            Action::CaptureSnapshot => return Completion::OutcomeUnknown,
        };
        match result {
            Ok(()) => Completion::WriteSettled,
            // This typed error only covers definite AccessDenied / no-interactive
            // auth replies. A generic remote failure never means no effect.
            Err(DnsError::AuthorizationRefused) => Completion::SettledFailure,
            Err(_) => Completion::OutcomeUnknown,
        }
    }

    fn mark_applied(&mut self) -> bool {
        self.recheck() && self.journal.applied_verified().is_ok()
    }

    fn finish(&mut self) -> bool {
        if !self.recheck() || self.journal.cleanup_verified().is_err() {
            return false;
        }
        if self.retention.release_after_proven_boundary().is_err() {
            return false;
        }
        self.retained = false;
        // The local ManagedResolved + channel proof still hold the object while
        // the durable record is cleared. Manager removal alone is not cleanup.
        self.context.recheck().is_ok()
            && self.resolved.recheck().is_ok()
            && self.journal.finish_after_store_empty().is_ok()
    }

    fn quarantine(&mut self) {
        self.retention.quarantine();
        if self.intent {
            let _ = self.journal.quarantine();
        }
        // No FDSTOREREMOVE on Drop. Keep manager retention and durable intent.
    }
}

pub(crate) struct Lease<'a> {
    driver: Driver<Host<'a>>,
}
impl<'a> Lease<'a> {
    pub(crate) fn new(
        context: &'a RootContext,
        journal: &'a mut Journal,
        resolved: ManagedResolved,
        retention: Retention,
        proof: OwnedFd,
        index: u32,
    ) -> Self {
        Self {
            driver: Driver::new(Host {
                context,
                journal,
                resolved,
                retention,
                proof: Some(proof),
                index,
                baseline: None,
                intent: false,
                retained: false,
                cleanup_started: false,
            }),
        }
    }
    pub(crate) fn apply(&mut self) -> Outcome {
        self.driver.drive()
    }
    pub(crate) fn release(&mut self) -> Outcome {
        if !self
            .driver
            .effects
            .renew_budget(std::time::Duration::from_secs(30))
        {
            return self.driver.quarantine();
        }
        self.driver.release()
    }
    pub(crate) fn check_active(&mut self) -> bool {
        if self
            .driver
            .effects
            .renew_budget(std::time::Duration::from_secs(5))
        {
            self.driver.check_active()
        } else {
            self.driver.quarantine();
            false
        }
    }
}

#[cfg(test)]
mod integration;
#[cfg(test)]
mod tests;
