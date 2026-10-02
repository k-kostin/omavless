// SPDX-License-Identifier: MIT

//! Private durable S1 intent journal. No production caller or host effects.
//! A returned Effect is permitted only after its intent reaches disk. Storage
//! errors poison the handle; reopening never resumes applying automatically.

use super::codec::{DesktopSnapshot, EnvironmentSnapshot};
use super::{Effect, Owner, Phase, ProxyLease, Snapshot, Surface};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};

pub mod fields;
pub mod staged;
mod storage;
use storage::Storage;

const MAX_JOURNAL_BYTES: usize = 192 * 1024;

/// Trusted caller observations, not client-selected identity or host discovery.
/// Boot and graphical/user-manager session identity must be revalidated by the
/// future adapter; this journal does not establish their provenance.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub owner_instance: [u8; 16],
    pub owner_generation: u64,
    pub boot: [u8; 16],
    pub session: [u8; 16],
    pub uid: u32,
}

impl Binding {
    fn owner(self) -> Owner {
        Owner {
            instance: self.owner_instance,
            generation: self.owner_generation,
        }
    }

    fn valid(self) -> bool {
        self.owner_instance != [0; 16]
            && self.owner_generation > 0
            && self.boot != [0; 16]
            && self.session != [0; 16]
            && self.uid == nix::unistd::geteuid().as_raw()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    UnsafePath,
    Busy,
    Missing,
    AlreadyExists,
    BindingMismatch,
    Interrupted,
    ForeignChange,
    RecoveryRequired,
    OutcomeUnknown,
    Planner(super::Error),
}

pub struct Journal {
    binding: Binding,
    lease: ProxyLease,
    storage: Storage,
    persisted: Vec<u8>,
    recovered: bool,
    poisoned: bool,
}

impl Journal {
    /// The directory must already be private and selected by trusted runtime
    /// code. All journal/lock/staging basenames are fixed by this module.
    pub fn create(
        directory: &Path,
        binding: Binding,
        original: [Snapshot; 2],
        intended: [Snapshot; 2],
    ) -> Result<Self, Error> {
        if !binding.valid() {
            return Err(Error::BindingMismatch);
        }
        let lease = ProxyLease::prepare(binding.owner(), original, intended);
        let persisted = encode(binding, &lease)?;
        let storage = Storage::acquire(directory)?;
        if storage.read()?.is_some() {
            return Err(Error::AlreadyExists);
        }
        storage.replace(None, &persisted)?;
        Ok(Self {
            binding,
            lease,
            storage,
            persisted,
            recovered: false,
            poisoned: false,
        })
    }

    /// A restart can inspect a valid matching journal, but can only compensate
    /// after fresh observations. It cannot confirm/reissue an unknown effect.
    pub fn open(directory: &Path, binding: Binding) -> Result<Self, Error> {
        if !binding.valid() {
            return Err(Error::BindingMismatch);
        }
        let storage = Storage::acquire(directory)?;
        let persisted = storage.read()?.ok_or(Error::Missing)?;
        let (recorded_binding, lease) = decode(&persisted)?;
        if binding != recorded_binding {
            return Err(Error::BindingMismatch);
        }
        Ok(Self {
            binding,
            lease,
            storage,
            persisted,
            recovered: true,
            poisoned: false,
        })
    }

    pub fn phase(&self) -> Phase {
        self.lease.phase()
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

    fn candidate(&self) -> ProxyLease {
        ProxyLease {
            owner: self.lease.owner,
            original: self.lease.original.clone(),
            target: self.lease.target.clone(),
            expected: self.lease.expected.clone(),
            attempted: self.lease.attempted,
            pending: self.lease.pending,
            phase: self.lease.phase,
        }
    }

    fn commit(&mut self, candidate: ProxyLease) -> Result<(), Error> {
        let bytes = encode(self.binding, &candidate)?;
        // Any failure after preparation forbids this handle from emitting more
        // effects, even if the old or new record appears readable afterward.
        if let Err(error) = self.storage.replace(Some(&self.persisted), &bytes) {
            self.poisoned = true;
            return Err(error);
        }
        self.lease = candidate;
        self.persisted = bytes;
        Ok(())
    }

    pub fn begin_next(
        &mut self,
        binding: Binding,
        observed: &[Snapshot; 2],
    ) -> Result<Option<Effect>, Error> {
        self.check(binding)?;
        if self.recovered {
            return Err(Error::RecoveryRequired);
        }
        let mut candidate = self.candidate();
        let effect = candidate
            .begin_next(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(candidate)?;
        Ok(effect)
    }

    pub fn confirm(&mut self, binding: Binding, observed: &[Snapshot; 2]) -> Result<(), Error> {
        self.check(binding)?;
        if self.recovered {
            return Err(Error::RecoveryRequired);
        }
        let mut candidate = self.candidate();
        candidate
            .confirm(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(candidate)
    }

    pub fn begin_restore(
        &mut self,
        binding: Binding,
        observed: &[Snapshot; 2],
    ) -> Result<(), Error> {
        self.check(binding)?;
        let mut candidate = self.candidate();
        candidate
            .begin_restore(binding.owner(), observed)
            .map_err(Error::Planner)?;
        self.commit(candidate)?;
        self.recovered = false;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    desktop: String,
    manager: String,
}

impl Pair {
    fn encode(value: &[Snapshot; 2]) -> Result<Self, Error> {
        let desktop = DesktopSnapshot::decode(&value[0]).map_err(|_| Error::Invalid)?;
        let manager = EnvironmentSnapshot::decode(&value[1]).map_err(|_| Error::Invalid)?;
        if desktop.encode().map_err(|_| Error::Invalid)? != value[0]
            || manager.encode().map_err(|_| Error::Invalid)? != value[1]
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            desktop: String::from_utf8(value[0].bytes().ok_or(Error::Invalid)?.to_vec())
                .map_err(|_| Error::Invalid)?,
            manager: String::from_utf8(value[1].bytes().ok_or(Error::Invalid)?.to_vec())
                .map_err(|_| Error::Invalid)?,
        })
    }

    fn decode(self) -> Result<[Snapshot; 2], Error> {
        let pair = [
            Snapshot::new(Some(self.desktop.into_bytes())).map_err(|_| Error::Invalid)?,
            Snapshot::new(Some(self.manager.into_bytes())).map_err(|_| Error::Invalid)?,
        ];
        Self::encode(&pair)?;
        Ok(pair)
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Side {
    Original,
    Intended,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Pending {
    None,
    Desktop,
    Manager,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredPhase {
    Applying,
    Active,
    Restoring,
    Released,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    binding: Binding,
    original: Pair,
    intended: Pair,
    expected: [Side; 2],
    attempted: [bool; 2],
    pending: Pending,
    phase: StoredPhase,
}

fn validate(lease: &ProxyLease) -> Result<(), Error> {
    for i in 0..2 {
        if lease.expected[i] != lease.original[i]
            && (!lease.attempted[i] || lease.expected[i] != lease.target[i])
        {
            return Err(Error::Invalid);
        }
        if lease.original[i] == lease.target[i] && lease.attempted[i] {
            return Err(Error::Invalid);
        }
    }
    if lease.attempted[1] && !lease.attempted[0] && lease.original[0] != lease.target[0] {
        return Err(Error::Invalid);
    }
    match lease.phase {
        Phase::Applying => {
            if lease.attempted[1] && lease.expected[0] != lease.target[0] {
                return Err(Error::Invalid);
            }
            for i in 0..2 {
                let pending = lease.pending.is_some_and(|s| s.index() == i);
                if pending {
                    if !lease.attempted[i] || lease.expected[i] != lease.original[i] {
                        return Err(Error::Invalid);
                    }
                } else if lease.attempted[i] && lease.expected[i] != lease.target[i] {
                    return Err(Error::Invalid);
                }
            }
        }
        Phase::Active => {
            if lease.pending.is_some() || lease.expected != lease.target {
                return Err(Error::Invalid);
            }
        }
        Phase::Restoring => {
            if let Some(surface) = lease.pending {
                let i = surface.index();
                if lease.expected[i] == lease.original[i] || !lease.attempted[i] {
                    return Err(Error::Invalid);
                }
                if i == 0 && lease.expected[1] != lease.original[1] {
                    return Err(Error::Invalid);
                }
            }
        }
        Phase::Released => {
            if lease.pending.is_some() || lease.expected != lease.original {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(())
}

fn encode(binding: Binding, lease: &ProxyLease) -> Result<Vec<u8>, Error> {
    validate(lease)?;
    let record = Record {
        version: 1,
        binding,
        original: Pair::encode(&lease.original)?,
        intended: Pair::encode(&lease.target)?,
        expected: std::array::from_fn(|i| {
            if lease.expected[i] == lease.original[i] {
                Side::Original
            } else {
                Side::Intended
            }
        }),
        attempted: lease.attempted,
        pending: match lease.pending {
            None => Pending::None,
            Some(Surface::Desktop) => Pending::Desktop,
            Some(Surface::UserManager) => Pending::Manager,
        },
        phase: match lease.phase {
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

fn decode(bytes: &[u8]) -> Result<(Binding, ProxyLease), Error> {
    if bytes.len() > MAX_JOURNAL_BYTES {
        return Err(Error::Invalid);
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    if record.version != 1 || !record.binding.valid() {
        return Err(Error::Invalid);
    }
    let original = record.original.decode()?;
    let target = record.intended.decode()?;
    let lease = ProxyLease {
        owner: record.binding.owner(),
        expected: std::array::from_fn(|i| match record.expected[i] {
            Side::Original => original[i].clone(),
            Side::Intended => target[i].clone(),
        }),
        original,
        target,
        attempted: record.attempted,
        pending: match record.pending {
            Pending::None => None,
            Pending::Desktop => Some(Surface::Desktop),
            Pending::Manager => Some(Surface::UserManager),
        },
        phase: match record.phase {
            StoredPhase::Applying => Phase::Applying,
            StoredPhase::Active => Phase::Active,
            StoredPhase::Restoring => Phase::Restoring,
            StoredPhase::Released => Phase::Released,
        },
    };
    validate(&lease)?;
    Ok((record.binding, lease))
}

impl fmt::Debug for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyJournalBinding([private])")
    }
}

impl fmt::Debug for Journal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyJournal([private])")
    }
}

#[cfg(test)]
mod tests;
