// SPDX-License-Identifier: MIT
//! Effect-free desktop-only three-state model. No host adapter, drain proof,
//! installed readiness, executable/session authority or activation constructor.
use super::{
    Error, Owner,
    codec::*,
    fields::{self, Field, State, Value},
};
use serde::{Deserialize, Serialize};

pub const CONTROL_COUNT: usize = DesktopKey::ALL.len() - 1;
const MODE: Field = Field::Desktop(DesktopKey::Mode);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    QuiescingApply,
    ApplyingControls,
    Enabling,
    Active,
    QuiescingRestore,
    RestoringControls,
    RestoringMode,
    Released,
}

/// Repeated writes to the mode key are distinct operations, even when saved
/// and intended mode are equal. These fixed identities contain no values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "key",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Step {
    QuiesceApply,
    ApplyControl(DesktopKey),
    Enable,
    QuiesceRestore,
    RestoreControl(DesktopKey),
    RestoreMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ModeSide {
    Original,
    Quiescent,
    Intended,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Effect {
    pub step: Step,
    pub change: fields::Effect,
}

fn controls() -> impl DoubleEndedIterator<Item = DesktopKey> {
    DesktopKey::ALL
        .into_iter()
        .filter(|key| *key != DesktopKey::Mode)
}
fn index(key: DesktopKey) -> usize {
    controls()
        .position(|item| item == key)
        .expect("fixed non-mode control")
}

#[derive(Clone)]
pub struct Planner {
    pub(super) owner: Owner,
    pub(super) original: State,
    pub(super) intended: State,
    pub(super) controls: [bool; CONTROL_COUNT],
    pub(super) mode: ModeSide,
    pub(super) stage: Stage,
    pub(super) pending: Option<Step>,
}

impl Planner {
    pub fn prepare(owner: Owner, original: State, intended: State) -> Result<Self, Error> {
        // Reuse existing complete layered/lock/default checks, not its order.
        fields::Planner::prepare(owner, original.clone(), intended.clone())?;
        for key in EnvironmentKey::ALL {
            let field = Field::UserManager(key);
            if original.value(field) != intended.value(field) {
                return Err(Error::UnsupportedOrder);
            }
        }
        let Value::Desktop(mode) = intended.value(MODE) else {
            unreachable!()
        };
        if mode.effective != DesktopValue::String("manual".into())
            || mode.user != Override::Present(DesktopValue::String("manual".into()))
        {
            return Err(Error::UnsupportedOrder);
        }
        Ok(Self {
            owner,
            original,
            intended,
            controls: [false; CONTROL_COUNT],
            mode: ModeSide::Original,
            stage: Stage::QuiescingApply,
            pending: None,
        })
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn pending(&self) -> Option<Step> {
        self.pending
    }
    pub fn intended_controls(&self) -> u8 {
        self.controls.iter().filter(|side| **side).count() as u8
    }

    fn mode_value(&self, side: ModeSide) -> Value {
        match side {
            ModeSide::Original => self.original.value(MODE),
            ModeSide::Intended => self.intended.value(MODE),
            ModeSide::Quiescent => {
                let Value::Desktop(mut entry) = self.original.value(MODE) else {
                    unreachable!()
                };
                entry.effective = DesktopValue::String("none".into());
                entry.user = Override::Present(entry.effective.clone());
                Value::Desktop(entry)
            }
        }
    }

    pub(super) fn expected(&self) -> Result<State, Error> {
        let mut state = self.original.with(MODE, self.mode_value(self.mode))?;
        for key in controls() {
            if self.controls[index(key)] {
                let field = Field::Desktop(key);
                state = state.with(field, self.intended.value(field))?;
            }
        }
        Ok(state)
    }

    fn required_control(&self, apply: bool) -> Option<DesktopKey> {
        let mut keys: Vec<_> = controls().collect();
        if !apply {
            keys.reverse();
        }
        keys.into_iter().find(|key| {
            let field = Field::Desktop(*key);
            if apply {
                !self.controls[index(*key)]
                    && self.original.value(field) != self.intended.value(field)
            } else {
                self.controls[index(*key)]
            }
        })
    }

    fn effect(&self, step: Step) -> Result<Effect, Error> {
        let (field, replacement) = match step {
            Step::QuiesceApply | Step::QuiesceRestore => {
                (MODE, self.mode_value(ModeSide::Quiescent))
            }
            Step::Enable => (MODE, self.mode_value(ModeSide::Intended)),
            Step::RestoreMode => (MODE, self.mode_value(ModeSide::Original)),
            Step::ApplyControl(key) if key != DesktopKey::Mode => (
                Field::Desktop(key),
                self.intended.value(Field::Desktop(key)),
            ),
            Step::RestoreControl(key) if key != DesktopKey::Mode => (
                Field::Desktop(key),
                self.original.value(Field::Desktop(key)),
            ),
            _ => return Err(Error::InvalidSnapshot),
        };
        Ok(Effect {
            step,
            change: fields::Effect {
                field,
                expected: self.expected()?.value(field),
                replacement,
            },
        })
    }

    pub(super) fn validate(&self) -> Result<(), Error> {
        let mut gap = false;
        for key in controls() {
            let field = Field::Desktop(key);
            if self.original.value(field) == self.intended.value(field) {
                if self.controls[index(key)] {
                    return Err(Error::InvalidSnapshot);
                }
            } else if self.controls[index(key)] {
                if gap {
                    return Err(Error::InvalidSnapshot);
                }
            } else {
                gap = true;
            }
        }
        let original = self.required_control(false).is_none();
        let intended = self.required_control(true).is_none();
        let valid = match self.stage {
            Stage::QuiescingApply => self.mode == ModeSide::Original && original,
            Stage::ApplyingControls => self.mode == ModeSide::Quiescent,
            Stage::Enabling => self.mode == ModeSide::Quiescent && intended,
            Stage::Active => self.mode == ModeSide::Intended && intended,
            Stage::QuiescingRestore => match self.mode {
                ModeSide::Original => original,
                ModeSide::Intended => intended,
                ModeSide::Quiescent => true,
            },
            Stage::RestoringControls => self.mode == ModeSide::Quiescent,
            Stage::RestoringMode => self.mode == ModeSide::Quiescent && original,
            Stage::Released => self.mode == ModeSide::Original && original,
        };
        if !valid {
            return Err(Error::InvalidSnapshot);
        }
        if let Some(step) = self.pending {
            let valid = match (self.stage, step) {
                (Stage::QuiescingApply, Step::QuiesceApply)
                | (Stage::Enabling, Step::Enable)
                | (Stage::QuiescingRestore, Step::QuiesceRestore)
                | (Stage::RestoringMode, Step::RestoreMode) => true,
                (Stage::ApplyingControls, Step::ApplyControl(key)) => {
                    self.required_control(true) == Some(key)
                }
                (Stage::RestoringControls, Step::RestoreControl(key)) => {
                    self.required_control(false) == Some(key)
                }
                _ => false,
            };
            if !valid {
                return Err(Error::InvalidSnapshot);
            }
            let effect = self.effect(step)?;
            if effect.change.expected == effect.change.replacement {
                return Err(Error::InvalidSnapshot);
            }
        }
        Ok(())
    }

    fn check_owner(&self, owner: Owner) -> Result<(), Error> {
        if owner == self.owner {
            Ok(())
        } else {
            Err(Error::StaleOwner)
        }
    }

    pub fn begin_next(&mut self, owner: Owner, observed: &State) -> Result<Option<Effect>, Error> {
        self.check_owner(owner)?;
        if self.pending.is_some() {
            return Err(Error::PendingEffect);
        }
        if observed != &self.expected()? {
            return Err(Error::ForeignChange);
        }
        loop {
            let step = match self.stage {
                Stage::QuiescingApply => Step::QuiesceApply,
                Stage::ApplyingControls => {
                    if let Some(key) = self.required_control(true) {
                        Step::ApplyControl(key)
                    } else {
                        self.stage = Stage::Enabling;
                        continue;
                    }
                }
                Stage::Enabling => Step::Enable,
                Stage::Active | Stage::Released => return Ok(None),
                Stage::QuiescingRestore => Step::QuiesceRestore,
                Stage::RestoringControls => {
                    if let Some(key) = self.required_control(false) {
                        Step::RestoreControl(key)
                    } else {
                        self.stage = Stage::RestoringMode;
                        continue;
                    }
                }
                Stage::RestoringMode => Step::RestoreMode,
            };
            let effect = self.effect(step)?;
            if effect.change.expected == effect.change.replacement {
                *self = self.after(step)?;
                continue;
            }
            self.pending = Some(step);
            self.validate()?;
            return Ok(Some(effect));
        }
    }

    fn after(&self, step: Step) -> Result<Self, Error> {
        let mut next = self.clone();
        match step {
            Step::QuiesceApply => {
                next.mode = ModeSide::Quiescent;
                next.stage = Stage::ApplyingControls;
            }
            Step::ApplyControl(key) if key != DesktopKey::Mode => next.controls[index(key)] = true,
            Step::Enable => {
                next.mode = ModeSide::Intended;
                next.stage = Stage::Active;
            }
            Step::QuiesceRestore => {
                next.mode = ModeSide::Quiescent;
                next.stage = Stage::RestoringControls;
            }
            Step::RestoreControl(key) if key != DesktopKey::Mode => {
                next.controls[index(key)] = false
            }
            Step::RestoreMode => {
                next.mode = ModeSide::Original;
                next.stage = Stage::Released;
            }
            _ => return Err(Error::InvalidSnapshot),
        }
        next.pending = None;
        next.validate()?;
        Ok(next)
    }

    pub(super) fn pending_after(&self) -> Result<Option<State>, Error> {
        self.pending
            .map(|step| self.after(step)?.expected())
            .transpose()
    }

    pub fn confirm(&mut self, owner: Owner, observed: &State) -> Result<(), Error> {
        self.check_owner(owner)?;
        let next = self.after(self.pending.ok_or(Error::WrongPhase)?)?;
        if observed != &next.expected()? {
            return Err(Error::UnconfirmedEffect);
        }
        *self = next;
        Ok(())
    }

    /// Pure compensation decision only. The trusted private test driver MUST
    /// drain its retained originating port before this observation. Matching
    /// before/after values never constitute that proof, especially on restart.
    pub fn begin_restore(&mut self, owner: Owner, observed: &State) -> Result<(), Error> {
        self.check_owner(owner)?;
        if self.stage == Stage::Released {
            return Err(Error::WrongPhase);
        }
        let mut next = if observed == &self.expected()? {
            self.clone()
        } else if let Some(step) = self.pending {
            let after = self.after(step)?;
            if observed != &after.expected()? {
                return Err(Error::ForeignChange);
            }
            after
        } else {
            return Err(Error::ForeignChange);
        };
        next.pending = None;
        next.stage = Stage::QuiescingRestore;
        next.validate()?;
        *self = next;
        Ok(())
    }
}

impl std::fmt::Debug for Planner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StagedProxyPlanner([private])")
    }
}

#[cfg(test)]
pub(crate) mod tests;
