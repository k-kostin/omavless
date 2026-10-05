// SPDX-License-Identifier: MIT

//! Effect-free DNS transaction model for issue #270.
//!
//! No runtime depends on this crate. It opens no socket, runs no commands, and
//! neither grants authority nor verifies kernel identity. Inputs describe facts
//! a future trusted host adapter MUST establish. They must never come from an
//! untrusted IPC client. See docs/development/DNS_TRANSACTION_FOUNDATION.md.
//!
//! In particular, a ticket fences model completions, NOT already-dispatched
//! D-Bus effects. An unknown write outcome blocks all automatic compensation:
//! restoring while that write may still arrive would create a second race.

use std::fmt;

pub mod wire;

/// Required host facts, not a capability or substitute for their verification.
#[derive(Clone, Copy, Debug, Default)]
pub struct Prerequisites {
    pub enrolled_peer: bool,
    pub exclusive_dns_owner: bool,
    pub kernel_bound_lease: bool,
    pub compatible_policy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseCheck {
    Same,
    Lost,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Capture,
    SetServers,
    SetDomains,
    SetDefaultRoute,
    VerifyApplied,
    /// Transaction-time proof only, not ongoing internet/DNS health.
    AppliedVerified,
    Restore,
    VerifyRestored,
    Released,
    FailedRestored,
    Refused,
    ManualRecoveryRequired,
}

/// All parameters and original settings remain inside the future host adapter.
/// There is deliberately no caller-selected DNS/address/interface/command here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    CaptureSnapshot,
    SetFixedServers,
    SetRootRoutingDomain,
    SetDefaultRoute,
    VerifyFixedPolicy,
    RestoreCapturedSnapshot,
    VerifyCapturedSnapshot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolicyReadback {
    pub servers_match: bool,
    pub domains_match: bool,
    pub default_route_matches: bool,
}

impl PolicyReadback {
    fn matches(self) -> bool {
        self.servers_match && self.domains_match && self.default_route_matches
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    SnapshotCaptured,
    WriteSettled,
    PolicyReadback(PolicyReadback),
    SnapshotReadback {
        matches: bool,
    },
    /// No further effect from this operation can arrive. This is stronger than
    /// a timeout or a closed UI dialog; writes may already have partially applied.
    SettledFailure,
    /// Cannot establish whether/when the effect will finish. Never auto-restore.
    OutcomeUnknown,
}

/// Internal correlation, not IPC authentication or kernel ownership evidence.
/// A driver supplies a fresh, non-reused transaction number per broker epoch.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    epoch: u64,
    transaction: u64,
    sequence: u8,
    action: Action,
}

impl Ticket {
    #[must_use]
    pub fn action(self) -> Action {
        self.action
    }
}

impl fmt::Debug for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ticket")
            .field("action", &self.action)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    PrerequisitesMissing,
    InvalidIdentity,
    StaleCompletion,
    WrongCompletion,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PrerequisitesMissing => "DNS host prerequisites are unavailable",
            Self::InvalidIdentity => "DNS transaction identity is invalid",
            Self::StaleCompletion => "DNS completion is stale",
            Self::WrongCompletion => "DNS completion does not match the pending action",
        })
    }
}

impl std::error::Error for ModelError {}

/// Bounded one-shot state machine. Snapshot bytes, leases and actual D-Bus work
/// are deliberately absent; a fake host exercises the contract in tests.
pub struct Transaction {
    epoch: u64,
    transaction: u64,
    sequence: u8,
    phase: Phase,
    pending: Option<Ticket>,
    dirty: bool,
    abort_requested: bool,
    explicit_release: bool,
}

impl Transaction {
    pub fn new(epoch: u64, transaction: u64, facts: Prerequisites) -> Result<Self, ModelError> {
        if epoch == 0 || transaction == 0 {
            return Err(ModelError::InvalidIdentity);
        }
        if !(facts.enrolled_peer
            && facts.exclusive_dns_owner
            && facts.kernel_bound_lease
            && facts.compatible_policy)
        {
            return Err(ModelError::PrerequisitesMissing);
        }
        Ok(Self {
            epoch,
            transaction,
            sequence: 0,
            phase: Phase::Capture,
            pending: None,
            dirty: false,
            abort_requested: false,
            explicit_release: false,
        })
    }

    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Recheck the held lease before EACH effect, including compensation.
    /// The real adapter must additionally prevent check/use races; a boolean
    /// from two independent name/index lookups cannot satisfy this contract.
    pub fn next_action(&mut self, lease: LeaseCheck) -> Option<Ticket> {
        if lease != LeaseCheck::Same {
            self.invalidate();
            return None;
        }
        if self.pending.is_some() {
            return None;
        }
        let action = match self.phase {
            Phase::Capture => Action::CaptureSnapshot,
            Phase::SetServers => Action::SetFixedServers,
            Phase::SetDomains => Action::SetRootRoutingDomain,
            Phase::SetDefaultRoute => Action::SetDefaultRoute,
            Phase::VerifyApplied => Action::VerifyFixedPolicy,
            Phase::Restore => Action::RestoreCapturedSnapshot,
            Phase::VerifyRestored => Action::VerifyCapturedSnapshot,
            _ => return None,
        };
        // A transaction has at most eight actions, without retries or loops.
        self.sequence += 1;
        let ticket = Ticket {
            epoch: self.epoch,
            transaction: self.transaction,
            sequence: self.sequence,
            action,
        };
        if matches!(
            action,
            Action::SetFixedServers | Action::SetRootRoutingDomain | Action::SetDefaultRoute
        ) {
            // A dispatched write can have effects even when it reports failure.
            self.dirty = true;
        }
        self.pending = Some(ticket);
        Some(ticket)
    }

    /// May be called while authorization is pending: no timeout is implied.
    /// Cancellation waits for the in-flight operation to settle. It never
    /// dispatches compensation concurrently with that operation.
    pub fn cancel(&mut self) {
        if self.terminal() || self.phase == Phase::AppliedVerified {
            return;
        }
        self.abort_requested = true;
        if self.pending.is_none() && !matches!(self.phase, Phase::Restore | Phase::VerifyRestored) {
            self.phase = if self.dirty {
                Phase::Restore
            } else {
                Phase::Refused
            };
        }
    }

    /// Explicit disconnect after confirmed application; not ordinary UI close.
    pub fn release(&mut self) {
        if self.phase == Phase::AppliedVerified {
            self.explicit_release = true;
            self.phase = Phase::Restore;
        } else {
            self.cancel();
        }
    }

    /// Lease loss, broker/core restart or an unjoinable in-flight operation.
    /// Refuse writes against possibly reused interfaces. Recovery is separate.
    pub fn invalidate(&mut self) {
        if self.terminal() {
            return;
        }
        self.pending = None;
        self.phase = if self.dirty {
            Phase::ManualRecoveryRequired
        } else {
            Phase::Refused
        };
    }

    fn terminal(&self) -> bool {
        matches!(
            self.phase,
            Phase::Released
                | Phase::FailedRestored
                | Phase::Refused
                | Phase::ManualRecoveryRequired
        )
    }

    pub fn complete(
        &mut self,
        ticket: Ticket,
        result: Completion,
        lease: LeaseCheck,
    ) -> Result<(), ModelError> {
        if self.pending != Some(ticket) {
            return Err(ModelError::StaleCompletion);
        }
        if lease != LeaseCheck::Same {
            self.invalidate();
            return Ok(());
        }
        let shape_valid = matches!(
            result,
            Completion::SettledFailure | Completion::OutcomeUnknown
        ) || matches!(
            (ticket.action, result),
            (Action::CaptureSnapshot, Completion::SnapshotCaptured)
                | (
                    Action::SetFixedServers
                        | Action::SetRootRoutingDomain
                        | Action::SetDefaultRoute
                        | Action::RestoreCapturedSnapshot,
                    Completion::WriteSettled
                )
                | (Action::VerifyFixedPolicy, Completion::PolicyReadback(_))
                | (
                    Action::VerifyCapturedSnapshot,
                    Completion::SnapshotReadback { .. }
                )
        );
        if !shape_valid {
            return Err(ModelError::WrongCompletion);
        }
        self.pending = None;
        if result == Completion::OutcomeUnknown {
            self.invalidate();
            return Ok(());
        }
        if result == Completion::SettledFailure {
            self.abort_requested = true;
            self.phase = match ticket.action {
                Action::CaptureSnapshot => Phase::Refused,
                Action::RestoreCapturedSnapshot | Action::VerifyCapturedSnapshot => {
                    Phase::ManualRecoveryRequired
                }
                _ => Phase::Restore,
            };
            return Ok(());
        }
        // Even a successful late result after cancellation cannot publish ready.
        if self.abort_requested
            && !matches!(
                ticket.action,
                Action::RestoreCapturedSnapshot | Action::VerifyCapturedSnapshot
            )
        {
            self.phase = if self.dirty {
                Phase::Restore
            } else {
                Phase::Refused
            };
            return Ok(());
        }
        self.phase = match (ticket.action, result) {
            (Action::CaptureSnapshot, _) => Phase::SetServers,
            (Action::SetFixedServers, _) => Phase::SetDomains,
            (Action::SetRootRoutingDomain, _) => Phase::SetDefaultRoute,
            (Action::SetDefaultRoute, _) => Phase::VerifyApplied,
            (Action::VerifyFixedPolicy, Completion::PolicyReadback(facts)) if facts.matches() => {
                Phase::AppliedVerified
            }
            (Action::VerifyFixedPolicy, _) => Phase::Restore,
            (Action::RestoreCapturedSnapshot, _) => Phase::VerifyRestored,
            (Action::VerifyCapturedSnapshot, Completion::SnapshotReadback { matches: true }) => {
                self.dirty = false;
                if self.explicit_release {
                    Phase::Released
                } else {
                    Phase::FailedRestored
                }
            }
            (Action::VerifyCapturedSnapshot, _) => Phase::ManualRecoveryRequired,
        };
        Ok(())
    }
}
