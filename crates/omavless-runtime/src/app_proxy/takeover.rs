// SPDX-License-Identifier: MIT

//! Inactive S1 compensation-transfer admission model. This does not discover
//! host identity, prove a receipt, rewrite a journal, or authorize a host
//! effect. In particular, an ordinary daemon crash has no graceful receipt.

use super::journal::Binding;
use std::fmt;

/// Exact private journal identity, supplied by a future storage transaction.
/// A digest alone is not a lock or evidence that a predecessor has stopped.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RecordIdentity {
    pub digest: [u8; 32],
    pub sequence: u64,
}

impl fmt::Debug for RecordIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1RecordIdentity([private])")
    }
}

/// Opaque identities of verified host objects, not hashes of their values.
/// The future adapter must prove these refer to the same installed session,
/// bus, manager, settings backend/profile, and shared activation environment.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HostScope {
    pub manager_incarnation: [u8; 16],
    pub bus_incarnation: [u8; 16],
    pub settings_profile: [u8; 16],
    pub shared_activation_environment: bool,
}

impl HostScope {
    fn valid(self) -> bool {
        self.manager_incarnation != [0; 16]
            && self.bus_incarnation != [0; 16]
            && self.settings_profile != [0; 16]
            && self.shared_activation_environment
    }
}

impl fmt::Debug for HostScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1HostScope([private])")
    }
}

/// An accepted, completed handoff is narrower than recovery after a crash.
/// The future native owner must prove old workers are joined, no async effect
/// can arrive later, and the exact record was not changed after this receipt.
/// Constructing this Rust value by itself proves none of those host facts.
#[derive(Clone, Copy)]
pub struct GracefulReceipt {
    pub record: RecordIdentity,
    pub predecessor_instance: [u8; 16],
    pub successor_instance: [u8; 16],
    pub workers_joined: bool,
    pub effects_settled: bool,
}

impl fmt::Debug for GracefulReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1GracefulReceipt([private])")
    }
}

#[derive(Clone, Copy)]
pub enum Predecessor {
    Unknown,
    Graceful(GracefulReceipt),
}

impl fmt::Debug for Predecessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1Predecessor([private])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordState {
    Missing,
    Released,
    Restorable,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    MissingOrReleased,
    UnverifiedAuthority,
    ScopeChanged,
    PredecessorUnsettled,
    RecordChanged,
}

/// No effect or new binding is returned. A future durable CAS-like record
/// transfer must verify this identity again under its exclusive storage lock.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CompensationTransfer {
    pub record: RecordIdentity,
    pub predecessor: Binding,
    pub successor: Binding,
}

impl fmt::Debug for CompensationTransfer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1CompensationTransfer([private])")
    }
}

#[derive(Clone, Copy)]
pub struct Admission {
    pub record_state: RecordState,
    pub record: RecordIdentity,
    pub predecessor: Binding,
    pub successor: Binding,
    pub old_scope: HostScope,
    pub new_scope: HostScope,
    pub native_owner_exclusive: bool,
    pub quiescence: Predecessor,
}

impl fmt::Debug for Admission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("S1TakeoverAdmission([private])")
    }
}

/// Pure fail-closed decision. `native_owner_exclusive` and both scopes are
/// external evidence, not discovered or made trustworthy by this function.
pub fn evaluate(admission: Admission) -> Result<CompensationTransfer, Refusal> {
    let Admission {
        record_state,
        record,
        predecessor,
        successor,
        old_scope,
        new_scope,
        native_owner_exclusive,
        quiescence,
    } = admission;
    if record_state != RecordState::Restorable {
        return Err(Refusal::MissingOrReleased);
    }
    if !native_owner_exclusive
        || record.digest == [0; 32]
        || record.sequence == 0
        || predecessor.owner_instance == [0; 16]
        || successor.owner_instance == [0; 16]
        || predecessor.owner_instance == successor.owner_instance
        || predecessor.owner_generation == 0
        || successor.owner_generation <= predecessor.owner_generation
    {
        return Err(Refusal::UnverifiedAuthority);
    }
    if predecessor.uid != successor.uid
        || predecessor.boot != successor.boot
        || predecessor.session != successor.session
        || predecessor.uid != nix::unistd::geteuid().as_raw()
        || predecessor.boot == [0; 16]
        || predecessor.session == [0; 16]
        || !old_scope.valid()
        || old_scope != new_scope
    {
        return Err(Refusal::ScopeChanged);
    }
    let Predecessor::Graceful(receipt) = quiescence else {
        return Err(Refusal::PredecessorUnsettled);
    };
    if !receipt.workers_joined || !receipt.effects_settled {
        return Err(Refusal::PredecessorUnsettled);
    }
    if receipt.record != record
        || receipt.predecessor_instance != predecessor.owner_instance
        || receipt.successor_instance != successor.owner_instance
    {
        return Err(Refusal::RecordChanged);
    }
    Ok(CompensationTransfer {
        record,
        predecessor,
        successor,
    })
}

#[cfg(test)]
mod tests;
