// SPDX-License-Identifier: MIT
//! Inactive desktop-only staged intent. Distinct fixed directory/strict format;
//! neither v2/v3 records nor a reopened journal grant installed effect authority.
use super::{Binding, Error, MAX_JOURNAL_BYTES, Pair, Storage};
use crate::app_proxy::{
    fields::State,
    staged::{CONTROL_COUNT, ModeSide, Planner, Stage, Step},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

const DIRECTORY: &str = "app-proxy-staged";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    RecordedBefore,
    PendingAfter,
    Foreign,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    RetainUnsettledEvidence,
    PreserveForeignEdits,
    RetainReleasedTombstone,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Review {
    pub stage: Stage,
    pub pending: Option<Step>,
    pub intended_controls: u8,
    pub relationship: Relationship,
    pub decision: Decision,
}

pub struct StagedJournal {
    binding: Binding,
    planner: Planner,
    storage: Storage,
    persisted: Vec<u8>,
    recovered: bool,
    poisoned: bool,
}
impl StagedJournal {
    /// Pure library initializer only. Caller-selected observations are not
    /// readiness/session/drain capabilities. Fixed child must already be 0700.
    pub fn create(
        root: &Path,
        binding: Binding,
        original: State,
        intended: State,
    ) -> Result<Self, Error> {
        if !binding.valid() {
            return Err(Error::BindingMismatch);
        }
        let planner =
            Planner::prepare(binding.owner(), original, intended).map_err(Error::Planner)?;
        let persisted = encode(binding, &planner)?;
        let storage = Storage::acquire(&root.join(DIRECTORY))?;
        if storage.read()?.is_some() {
            return Err(Error::AlreadyExists);
        }
        storage.replace(None, &persisted)?;
        Ok(Self {
            binding,
            planner,
            storage,
            persisted,
            recovered: false,
            poisoned: false,
        })
    }
    pub fn open(root: &Path, binding: Binding) -> Result<Self, Error> {
        if !binding.valid() {
            return Err(Error::BindingMismatch);
        }
        let storage = Storage::acquire(&root.join(DIRECTORY))?;
        let persisted = storage.read()?.ok_or(Error::Missing)?;
        let (recorded, planner) = decode(&persisted)?;
        if binding != recorded {
            return Err(Error::BindingMismatch);
        }
        Ok(Self {
            binding,
            planner,
            storage,
            persisted,
            recovered: true,
            poisoned: false,
        })
    }
    pub fn stage(&self) -> Stage {
        self.planner.stage()
    }
    fn check(&self, binding: Binding) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::RecoveryRequired);
        }
        if binding != self.binding || !binding.valid() {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
    fn commit(&mut self, candidate: Planner) -> Result<(), Error> {
        let bytes = encode(self.binding, &candidate)?;
        if let Err(error) = self.storage.replace(Some(&self.persisted), &bytes) {
            self.poisoned = true;
            return Err(error);
        }
        self.planner = candidate;
        self.persisted = bytes;
        Ok(())
    }
    pub fn begin_next(
        &mut self,
        binding: Binding,
        observed: &State,
    ) -> Result<Option<crate::app_proxy::staged::Effect>, Error> {
        self.check(binding)?;
        if self.recovered {
            return Err(Error::RecoveryRequired);
        }
        let mut next = self.planner.clone();
        let effect = next
            .begin_next(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(next)?;
        Ok(effect)
    }
    pub fn confirm(&mut self, binding: Binding, observed: &State) -> Result<(), Error> {
        self.check(binding)?;
        if self.recovered {
            return Err(Error::RecoveryRequired);
        }
        let mut next = self.planner.clone();
        next.confirm(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(next)
    }
    /// Pure model only: caller must first establish retained same-port drain.
    /// No new production executor/constructor accepts this journal.
    pub fn begin_restore(&mut self, binding: Binding, observed: &State) -> Result<(), Error> {
        self.check(binding)?;
        let mut next = self.planner.clone();
        next.begin_restore(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(next)?;
        self.recovered = false;
        Ok(())
    }
    pub fn recovery_review(&self, binding: Binding, observed: &State) -> Result<Review, Error> {
        self.check(binding)?;
        self.storage.review_exact(&self.persisted)?;
        let relationship = if observed == &self.planner.expected().map_err(Error::Planner)? {
            Relationship::RecordedBefore
        } else if self
            .planner
            .pending_after()
            .map_err(Error::Planner)?
            .as_ref()
            == Some(observed)
        {
            Relationship::PendingAfter
        } else {
            Relationship::Foreign
        };
        let decision = if relationship == Relationship::Foreign {
            Decision::PreserveForeignEdits
        } else if self.stage() == Stage::Released {
            Decision::RetainReleasedTombstone
        } else {
            Decision::RetainUnsettledEvidence
        };
        Ok(Review {
            stage: self.stage(),
            pending: self.planner.pending(),
            intended_controls: self.planner.intended_controls(),
            relationship,
            decision,
        })
    }
}
impl std::fmt::Debug for StagedJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StagedProxyJournal([private])")
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    binding: Binding,
    original: Pair,
    intended: Pair,
    controls: [bool; CONTROL_COUNT],
    mode: ModeSide,
    stage: Stage,
    pending: Option<Step>,
}
fn encode(binding: Binding, planner: &Planner) -> Result<Vec<u8>, Error> {
    planner.validate().map_err(|_| Error::Invalid)?;
    let record = Record {
        version: 1,
        binding,
        original: Pair::encode(&planner.original.encode().map_err(Error::Planner)?)?,
        intended: Pair::encode(&planner.intended.encode().map_err(Error::Planner)?)?,
        controls: planner.controls,
        mode: planner.mode,
        stage: planner.stage,
        pending: planner.pending,
    };
    let bytes = serde_json::to_vec(&record).map_err(|_| Error::Invalid)?;
    if bytes.len() > MAX_JOURNAL_BYTES {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}
fn decode(bytes: &[u8]) -> Result<(Binding, Planner), Error> {
    if bytes.len() > MAX_JOURNAL_BYTES {
        return Err(Error::Invalid);
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    if record.version != 1 || !record.binding.valid() {
        return Err(Error::Invalid);
    }
    let original = State::decode(&record.original.decode()?).map_err(Error::Planner)?;
    let intended = State::decode(&record.intended.decode()?).map_err(Error::Planner)?;
    let mut planner =
        Planner::prepare(record.binding.owner(), original, intended).map_err(Error::Planner)?;
    planner.controls = record.controls;
    planner.mode = record.mode;
    planner.stage = record.stage;
    planner.pending = record.pending;
    planner.validate().map_err(|_| Error::Invalid)?;
    Ok((record.binding, planner))
}
#[cfg(test)]
mod tests;
