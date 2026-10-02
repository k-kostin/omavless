// SPDX-License-Identifier: MIT

//! Fixed-key S1 compensation planner. This is effect-free and NOT a durable
//! journal or permission to write host settings. The existing two-surface
//! journal cannot recover a partially written GSettings batch; this planner
//! supplies the per-key state semantics that a future durable journal needs.

use super::{Error, Owner, Phase, Snapshot};
use crate::app_proxy::codec::{
    DesktopEntry, DesktopKey, DesktopSnapshot, DesktopValue, EnvironmentEntry, EnvironmentKey,
    EnvironmentSnapshot, Override,
};
use std::fmt;

pub const FIELD_COUNT: usize = DesktopKey::ALL.len() + EnvironmentKey::ALL.len();

/// Ordering is a journal/model property, never listener readiness or authority.
/// Legacy records retain their historical order; they must not be reinterpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    LegacyModeFirst,
    ModeLastOriginalNone,
}

impl Order {
    pub fn fields(self) -> impl DoubleEndedIterator<Item = Field> {
        let mut fields: Vec<_> = Field::all().collect();
        if self == Self::ModeLastOriginalNone {
            let mode = Field::Desktop(DesktopKey::Mode);
            fields.retain(|field| *field != mode);
            fields.push(mode);
        }
        fields.into_iter()
    }

    fn admit(self, original: &State, intended: &State) -> Result<(), Error> {
        if self == Self::ModeLastOriginalNone {
            let field = Field::Desktop(DesktopKey::Mode);
            let (Value::Desktop(before), Value::Desktop(after)) =
                (original.value(field), intended.value(field))
            else {
                unreachable!()
            };
            // Prior manual/auto requires a third-value quiesce stage, then old
            // mode LAST after restoring every consumed field. Two-side intent
            // cannot represent that stage; refuse rather than guess/reset.
            if before.effective != DesktopValue::String("none".into())
                || after.effective != DesktopValue::String("manual".into())
                || after.user != Override::Present(DesktopValue::String("manual".into()))
            {
                return Err(Error::UnsupportedOrder);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Desktop(DesktopKey),
    UserManager(EnvironmentKey),
}

impl Field {
    pub fn all() -> impl DoubleEndedIterator<Item = Self> {
        DesktopKey::ALL
            .into_iter()
            .map(Self::Desktop)
            .chain(EnvironmentKey::ALL.into_iter().map(Self::UserManager))
    }

    pub(super) fn index(self) -> usize {
        Self::all()
            .position(|field| field == self)
            .expect("fixed field")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    Desktop(DesktopEntry),
    UserManager(EnvironmentEntry),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyFieldValue([private])")
    }
}

/// A complete, typed, fixed-allowlist observation. Values may contain proxy
/// credentials; never log, serialize to activity or use in an argv string.
#[derive(Clone, PartialEq, Eq)]
pub struct State {
    desktop: DesktopSnapshot,
    manager: EnvironmentSnapshot,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyFieldState([private])")
    }
}

impl State {
    pub fn new(desktop: DesktopSnapshot, manager: EnvironmentSnapshot) -> Self {
        Self { desktop, manager }
    }

    pub fn decode(pair: &[Snapshot; 2]) -> Result<Self, Error> {
        Ok(Self {
            desktop: DesktopSnapshot::decode(&pair[0]).map_err(|_| Error::InvalidSnapshot)?,
            manager: EnvironmentSnapshot::decode(&pair[1]).map_err(|_| Error::InvalidSnapshot)?,
        })
    }

    pub fn encode(&self) -> Result<[Snapshot; 2], Error> {
        Ok([
            self.desktop.encode().map_err(|_| Error::InvalidSnapshot)?,
            self.manager.encode().map_err(|_| Error::InvalidSnapshot)?,
        ])
    }

    pub fn value(&self, field: Field) -> Value {
        match field {
            Field::Desktop(key) => Value::Desktop(
                self.desktop
                    .entries()
                    .iter()
                    .find(|entry| entry.key == key)
                    .expect("complete desktop snapshot")
                    .clone(),
            ),
            Field::UserManager(key) => Value::UserManager(
                self.manager
                    .entries()
                    .iter()
                    .find(|entry| entry.key == key)
                    .expect("complete environment snapshot")
                    .clone(),
            ),
        }
    }

    pub(super) fn with(&self, field: Field, replacement: Value) -> Result<Self, Error> {
        let mut next = self.clone();
        match (field, replacement) {
            (Field::Desktop(key), Value::Desktop(entry)) if entry.key == key => {
                let mut entries = next.desktop.entries().to_vec();
                let item = entries
                    .iter_mut()
                    .find(|entry| entry.key == key)
                    .ok_or(Error::InvalidSnapshot)?;
                *item = entry;
                next.desktop =
                    DesktopSnapshot::capture(entries).map_err(|_| Error::InvalidSnapshot)?;
            }
            (Field::UserManager(key), Value::UserManager(entry)) if entry.key == key => {
                let mut entries = next.manager.entries().to_vec();
                let item = entries
                    .iter_mut()
                    .find(|entry| entry.key == key)
                    .ok_or(Error::InvalidSnapshot)?;
                *item = entry;
                next.manager =
                    EnvironmentSnapshot::capture(entries).map_err(|_| Error::InvalidSnapshot)?;
            }
            _ => return Err(Error::InvalidSnapshot),
        }
        Ok(next)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Effect {
    pub field: Field,
    pub expected: Value,
    pub replacement: Value,
}

/// The single pending field must be durably recorded before an adapter acts.
/// A readback may be a mixture of original/intended values only for attempted
/// fields. Third values and changes to unattempted fields always refuse.
#[derive(Clone)]
pub struct Planner {
    pub(super) order: Order,
    pub(super) owner: Owner,
    pub(super) original: State,
    pub(super) intended: State,
    pub(super) expected: State,
    pub(super) attempted: [bool; FIELD_COUNT],
    pub(super) pending: Option<Field>,
    pub(super) phase: Phase,
}

impl Planner {
    pub fn prepare(owner: Owner, original: State, intended: State) -> Result<Self, Error> {
        Self::prepare_ordered(owner, original, intended, Order::LegacyModeFirst)
    }

    /// Effect-free model only. An eligible order does not establish loopback
    /// listener readiness, ownership, lifetime, session or installed admission.
    pub fn prepare_ordered(
        owner: Owner,
        original: State,
        intended: State,
        order: Order,
    ) -> Result<Self, Error> {
        original
            .desktop
            .admit_writes()
            .map_err(|_| Error::InvalidSnapshot)?;
        intended
            .desktop
            .admit_writes()
            .map_err(|_| Error::InvalidSnapshot)?;
        order.admit(&original, &intended)?;
        // Defaults and locks cannot be changed by the future user-value API.
        for key in DesktopKey::ALL {
            let Value::Desktop(before) = original.value(Field::Desktop(key)) else {
                unreachable!()
            };
            let Value::Desktop(after) = intended.value(Field::Desktop(key)) else {
                unreachable!()
            };
            if before.default != after.default || before.writable != after.writable {
                return Err(Error::InvalidSnapshot);
            }
        }
        Ok(Self {
            order,
            owner,
            expected: original.clone(),
            original,
            intended,
            attempted: [false; FIELD_COUNT],
            pending: None,
            phase: Phase::Applying,
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn order(&self) -> Order {
        self.order
    }

    fn check_owner(&self, owner: Owner) -> Result<(), Error> {
        if owner != self.owner {
            Err(Error::StaleOwner)
        } else {
            Ok(())
        }
    }

    pub fn begin_next(&mut self, owner: Owner, observed: &State) -> Result<Option<Effect>, Error> {
        self.check_owner(owner)?;
        if self.pending.is_some() {
            return Err(Error::PendingEffect);
        }
        if !matches!(self.phase, Phase::Applying | Phase::Restoring) {
            return Err(Error::WrongPhase);
        }
        if observed != &self.expected {
            return Err(Error::ForeignChange);
        }
        let destination = if self.phase == Phase::Applying {
            &self.intended
        } else {
            &self.original
        };
        let mut fields: Vec<_> = self.order.fields().collect();
        if self.phase == Phase::Restoring {
            fields.reverse();
        }
        for field in fields {
            let expected = observed.value(field);
            let replacement = destination.value(field);
            if expected != replacement {
                self.pending = Some(field);
                if self.phase == Phase::Applying {
                    self.attempted[field.index()] = true;
                }
                return Ok(Some(Effect {
                    field,
                    expected,
                    replacement,
                }));
            }
        }
        self.phase = if self.phase == Phase::Applying {
            Phase::Active
        } else {
            Phase::Released
        };
        Ok(None)
    }

    pub fn confirm(&mut self, owner: Owner, observed: &State) -> Result<(), Error> {
        self.check_owner(owner)?;
        let field = self.pending.ok_or(Error::WrongPhase)?;
        let replacement = if self.phase == Phase::Applying {
            self.intended.value(field)
        } else {
            self.original.value(field)
        };
        let expected = self.expected.with(field, replacement)?;
        if observed != &expected {
            return Err(Error::UnconfirmedEffect);
        }
        self.expected = expected;
        self.pending = None;
        Ok(())
    }

    pub fn begin_restore(&mut self, owner: Owner, observed: &State) -> Result<(), Error> {
        self.check_owner(owner)?;
        if self.phase == Phase::Released {
            return Err(Error::WrongPhase);
        }
        for field in Field::all() {
            let actual = observed.value(field);
            let original = self.original.value(field);
            let intended = self.intended.value(field);
            if actual != original && (!self.attempted[field.index()] || actual != intended) {
                return Err(Error::ForeignChange);
            }
        }
        self.expected = observed.clone();
        self.pending = None;
        self.phase = Phase::Restoring;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
