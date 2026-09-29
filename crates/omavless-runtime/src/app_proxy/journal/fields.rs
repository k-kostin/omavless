// SPDX-License-Identifier: MIT

//! Durable per-field S1 intent, still unregistered and without a host adapter.
//! The trusted caller MUST provide a dedicated private directory for this v2
//! journal, separate from the earlier two-surface v1 journal. Neither record
//! may be inferred from the other or used as a fresh baseline on restart.

use super::{Binding, Error, MAX_JOURNAL_BYTES, Pair, Side, Storage, StoredPhase};
use crate::app_proxy::{Phase, fields};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};

use fields::{Effect, FIELD_COUNT, Field, Planner, State};

pub struct FieldJournal {
    binding: Binding,
    planner: Planner,
    storage: Storage,
    persisted: Vec<u8>,
    recovered: bool,
    poisoned: bool,
}

impl FieldJournal {
    /// `directory` is a trusted, already-private dedicated field-journal root.
    /// The fixed storage basenames must never collide with a v1 journal.
    pub fn create(
        directory: &Path,
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
        let storage = Storage::acquire(directory)?;
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

    /// Reopening never resumes applying or confirms an unknown write. Fresh
    /// complete observations may only enter conservative compensation.
    pub fn open(directory: &Path, binding: Binding) -> Result<Self, Error> {
        if !binding.valid() {
            return Err(Error::BindingMismatch);
        }
        let storage = Storage::acquire(directory)?;
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

    pub fn phase(&self) -> Phase {
        self.planner.phase()
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

    /// Returns a fixed-key effect only after its intent is durably published.
    pub fn begin_next(
        &mut self,
        binding: Binding,
        observed: &State,
    ) -> Result<Option<Effect>, Error> {
        self.check(binding)?;
        if self.recovered {
            return Err(Error::RecoveryRequired);
        }
        let mut candidate = self.planner.clone();
        let effect = candidate
            .begin_next(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(candidate)?;
        Ok(effect)
    }

    pub fn confirm(&mut self, binding: Binding, observed: &State) -> Result<(), Error> {
        self.check(binding)?;
        if self.recovered {
            return Err(Error::RecoveryRequired);
        }
        let mut candidate = self.planner.clone();
        candidate
            .confirm(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(candidate)
    }

    pub fn begin_restore(&mut self, binding: Binding, observed: &State) -> Result<(), Error> {
        self.check(binding)?;
        let mut candidate = self.planner.clone();
        candidate
            .begin_restore(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(candidate)?;
        self.recovered = false;
        Ok(())
    }
}

impl fmt::Debug for FieldJournal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyFieldJournal([private])")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    binding: Binding,
    original: Pair,
    intended: Pair,
    expected: [Side; FIELD_COUNT],
    attempted: [bool; FIELD_COUNT],
    pending: Option<u8>,
    phase: StoredPhase,
}

fn validate(planner: &Planner) -> Result<(), Error> {
    let mut gap = false;
    let mut last_attempted = None;
    for field in Field::all() {
        let index = field.index();
        let original = planner.original.value(field);
        let intended = planner.intended.value(field);
        let expected = planner.expected.value(field);
        if original == intended && planner.attempted[index] {
            return Err(Error::Invalid);
        }
        if !planner.attempted[index] && expected != original {
            return Err(Error::Invalid);
        }
        if expected != original && expected != intended {
            return Err(Error::Invalid);
        }
        if original != intended {
            if !planner.attempted[index] {
                gap = true;
            } else if gap {
                return Err(Error::Invalid);
            }
        }
        if planner.attempted[index] {
            last_attempted = Some(field);
        }
    }
    match planner.phase {
        Phase::Applying => {
            if planner.pending.is_some() && planner.pending != last_attempted {
                return Err(Error::Invalid);
            }
            for field in Field::all() {
                let expected = planner.expected.value(field);
                let required = if planner.attempted[field.index()] && planner.pending != Some(field)
                {
                    planner.intended.value(field)
                } else {
                    planner.original.value(field)
                };
                if expected != required {
                    return Err(Error::Invalid);
                }
            }
        }
        Phase::Active => {
            if planner.pending.is_some() || planner.expected != planner.intended {
                return Err(Error::Invalid);
            }
            for field in Field::all() {
                if planner.original.value(field) != planner.intended.value(field)
                    && !planner.attempted[field.index()]
                {
                    return Err(Error::Invalid);
                }
            }
        }
        Phase::Restoring => {
            if let Some(pending) = planner.pending {
                if !planner.attempted[pending.index()]
                    || planner.expected.value(pending) == planner.original.value(pending)
                {
                    return Err(Error::Invalid);
                }
                for field in Field::all().skip(pending.index() + 1) {
                    if planner.expected.value(field) != planner.original.value(field) {
                        return Err(Error::Invalid);
                    }
                }
            }
        }
        Phase::Released => {
            if planner.pending.is_some() || planner.expected != planner.original {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(())
}

fn encode(binding: Binding, planner: &Planner) -> Result<Vec<u8>, Error> {
    validate(planner)?;
    let original = Pair::encode(&planner.original.encode().map_err(Error::Planner)?)?;
    let intended = Pair::encode(&planner.intended.encode().map_err(Error::Planner)?)?;
    let expected = std::array::from_fn(|index| {
        let field = Field::all().nth(index).expect("fixed field");
        if planner.expected.value(field) == planner.original.value(field) {
            Side::Original
        } else {
            Side::Intended
        }
    });
    let record = Record {
        version: 2,
        binding,
        original,
        intended,
        expected,
        attempted: planner.attempted,
        pending: planner.pending.map(|field| field.index() as u8),
        phase: match planner.phase {
            Phase::Applying => StoredPhase::Applying,
            Phase::Active => StoredPhase::Active,
            Phase::Restoring => StoredPhase::Restoring,
            Phase::Released => StoredPhase::Released,
        },
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
    if record.version != 2 || !record.binding.valid() {
        return Err(Error::Invalid);
    }
    let original = State::decode(&record.original.decode()?).map_err(Error::Planner)?;
    let intended = State::decode(&record.intended.decode()?).map_err(Error::Planner)?;
    let mut planner =
        Planner::prepare(record.binding.owner(), original, intended).map_err(Error::Planner)?;
    for (index, side) in record.expected.iter().enumerate() {
        let field = Field::all().nth(index).expect("fixed field");
        let value = match side {
            Side::Original => planner.original.value(field),
            Side::Intended => planner.intended.value(field),
        };
        planner.expected = planner
            .expected
            .with(field, value)
            .map_err(Error::Planner)?;
    }
    planner.attempted = record.attempted;
    planner.pending = record
        .pending
        .map(|index| Field::all().nth(usize::from(index)).ok_or(Error::Invalid))
        .transpose()?;
    planner.phase = match record.phase {
        StoredPhase::Applying => Phase::Applying,
        StoredPhase::Active => Phase::Active,
        StoredPhase::Restoring => Phase::Restoring,
        StoredPhase::Released => Phase::Released,
    };
    validate(&planner)?;
    Ok((record.binding, planner))
}

#[cfg(test)]
mod tests;
