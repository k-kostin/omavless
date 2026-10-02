// SPDX-License-Identifier: MIT

//! Inactive fixed-field executor. A host implementation is a trusted effect
//! boundary, not something a client/observation may supply as write authority.
//! No production constructor, IPC registration or installed host exists.

use super::{
    Phase,
    fields::{Effect, State},
    journal::{Binding, Error as JournalError, fields::FieldJournal},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostError {
    Unavailable,
    Changed,
    OutcomeUnknown,
}

/// All calls use the same retained owner/target/session. `observe` must be a
/// complete independent persisted read, never an optimistic setter cache.
/// `drain` must establish that *all* this port's admitted requests have settled
/// on that same owner, including timed-out/lost replies. A joined caller alone,
/// Settings.sync returning, a new connection/owner, or equality is insufficient.
/// External desktop edits remain possible and must be preserved.
pub trait FixedHost {
    fn observe(&mut self, binding: Binding) -> Result<State, HostError>;
    fn write(&mut self, binding: Binding, effect: &Effect) -> Result<(), HostError>;
    fn drain(&mut self, binding: Binding) -> Result<(), HostError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Journal(JournalError),
    Host(HostError),
    DrainRequired,
}

pub struct Transaction<H> {
    journal: FieldJournal,
    host: H,
    binding: Binding,
    fenced: bool,
}

impl<H: FixedHost> Transaction<H> {
    /// Fresh journal only. A new port cannot drain a predecessor's operations,
    /// even when its owner name and observed values match. Process-crash and
    /// successor recovery require a separate reviewed takeover protocol.
    pub fn new(journal: FieldJournal, host: H, binding: Binding) -> Result<Self, Error> {
        if journal.recovered() {
            return Err(Error::Journal(JournalError::RecoveryRequired));
        }
        Ok(Self {
            journal,
            host,
            binding,
            fenced: false,
        })
    }

    /// Same-owner journal reentry retains the original port and exact record.
    /// It closes effect admission before releasing/reacquiring storage. No
    /// fresh port or caller-supplied drain Boolean may replace the retained one.
    pub fn reopen_for_compensation(self, private_root: &std::path::Path) -> Result<Self, Error> {
        let Self {
            journal,
            host,
            binding,
            ..
        } = self;
        let journal = journal.reopen_exact(private_root).map_err(Error::Journal)?;
        Ok(Self {
            journal,
            host,
            binding,
            fenced: true,
        })
    }

    pub fn phase(&self) -> Phase {
        self.journal.phase()
    }

    /// Advance at most one fixed field. Durable intent precedes the final
    /// pre-effect comparison, the operation, the drain and full confirmation.
    /// A write's return value is never itself a confirmation.
    pub fn step(&mut self) -> Result<Phase, Error> {
        if self.fenced {
            return Err(Error::DrainRequired);
        }
        let before = self.host.observe(self.binding).map_err(Error::Host)?;
        let Some(effect) = self
            .journal
            .begin_next(self.binding, &before)
            .map_err(Error::Journal)?
        else {
            return Ok(self.phase());
        };
        let immediate = self.host.observe(self.binding).map_err(Error::Host)?;
        if immediate != before {
            return Err(Error::Host(HostError::Changed));
        }
        // Set before entering untrusted host code: unwind/panic cannot make an
        // outstanding operation eligible for another step or compensation.
        self.fenced = true;
        let _write_outcome = self.host.write(self.binding, &effect);
        self.host.drain(self.binding).map_err(Error::Host)?;
        self.fenced = false;
        let after = self.host.observe(self.binding).map_err(Error::Host)?;
        self.journal
            .confirm(self.binding, &after)
            .map_err(Error::Journal)?;
        Ok(self.phase())
    }

    /// Explicit same-owner compensation only. Missing drain keeps both the
    /// in-memory admission fence and original durable journal. A recovered
    /// journal never substitutes current host state as its original baseline.
    pub fn begin_restore(&mut self) -> Result<(), Error> {
        self.fenced = true;
        self.host.drain(self.binding).map_err(Error::Host)?;
        let observed = self.host.observe(self.binding).map_err(Error::Host)?;
        self.journal
            .begin_restore(self.binding, &observed)
            .map_err(Error::Journal)?;
        self.fenced = false;
        Ok(())
    }
}

impl<H> std::fmt::Debug for Transaction<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("S1Transaction([private])")
    }
}

#[cfg(test)]
mod tests;
