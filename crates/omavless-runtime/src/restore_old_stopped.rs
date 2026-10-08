// SPDX-License-Identifier: MIT
//! Fixed stopped-service qualification for one newly admitted OLD recovery.
//!
//! Cooperative same-UID writers must obey the continuously held migration and
//! singleton locks. This guard establishes no dead-process FD provenance and
//! performs no endpoint retirement, journal operation, Start or admission bypass.

use crate::cutover::{CutoverPaths, MigrationLock};
use crate::manager_actor_service::NativeStageView;
use crate::native_coordinator::NativeFirstError;
use crate::restore_abort_cli::stopped_owner::OldRecoveryOwner;
use std::cell::Cell;
use std::path::Path;
use std::sync::{Arc, OnceLock};

type Result<T> = std::result::Result<T, NativeFirstError>;

#[derive(Default)]
struct Qualification {
    attempted: Cell<bool>,
    qualified: Cell<bool>,
    refused: Cell<bool>,
}

impl Qualification {
    fn checked(&self, observe: impl FnOnce() -> Result<()>) -> Result<()> {
        if self.refused.get() {
            return Err(NativeFirstError::Admission);
        }
        let result = observe();
        if result.is_err() {
            self.refused.set(true);
        }
        result
    }

    fn qualify(&self, exclusive: bool, operation: impl FnOnce() -> Result<()>) -> Result<()> {
        self.checked(|| {
            if self.attempted.replace(true) || !exclusive {
                return Err(NativeFirstError::Admission);
            }
            // Consume once before the first potentially effect-bearing call.
            operation()?;
            self.qualified.set(true);
            Ok(())
        })
    }

    fn recheck(&self, observe: impl FnOnce() -> Result<()>) -> Result<()> {
        self.checked(|| {
            if !self.qualified.get() {
                return Err(NativeFirstError::Admission);
            }
            observe()
        })
    }
}

/// Neither Clone nor an exported proof. Original custody retains the exact Arc
/// containing the one actual lock and the original process/manager observations.
pub(crate) struct NativeOldRecoveryQuiescence {
    paths: CutoverPaths,
    uid: u32,
    lock: Arc<OnceLock<MigrationLock>>,
    owner: OldRecoveryOwner,
    qualification: Qualification,
}

impl NativeOldRecoveryQuiescence {
    /// Called after the parent's existing Migration acquisition, before engine
    /// staging. Reserve fixed tool/path custody only; no subprocess or process
    /// capture occurs until this guard is installed and qualification begins.
    pub(crate) fn prepare(
        paths: &CutoverPaths,
        uid: u32,
        lock: Arc<OnceLock<MigrationLock>>,
    ) -> Result<Self> {
        if paths.runtime_base != Path::new(&format!("/run/user/{uid}"))
            || !lock.get().is_some_and(|held| held.authorizes(paths, uid))
        {
            return Err(NativeFirstError::Admission);
        }
        let socket = paths.runtime_base.join("omavless/control.sock");
        let owner =
            OldRecoveryOwner::prepare(uid, &socket).map_err(|()| NativeFirstError::Admission)?;
        if !lock.get().is_some_and(|held| held.authorizes(paths, uid)) {
            return Err(NativeFirstError::Admission);
        }
        Ok(Self {
            paths: paths.clone(),
            uid,
            lock,
            owner,
            qualification: Qualification::default(),
        })
    }

    fn migration(&self) -> Result<()> {
        if self
            .lock
            .get()
            .is_some_and(|held| held.authorizes(&self.paths, self.uid))
        {
            Ok(())
        } else {
            Err(NativeFirstError::Admission)
        }
    }

    pub(crate) fn qualify_after_exclusive(&mut self, view: NativeStageView<'_>) -> Result<()> {
        let Self {
            paths,
            uid,
            lock,
            owner,
            qualification,
        } = self;
        qualification.qualify(view.recovery_exclusive(), || {
            let migration = || {
                if lock.get().is_some_and(|held| held.authorizes(paths, *uid)) {
                    Ok(())
                } else {
                    Err(NativeFirstError::Admission)
                }
            };
            migration()?;
            owner
                .stop_and_qualify()
                .map_err(|()| NativeFirstError::Admission)?;
            migration()
        })
    }

    /// Subsequent proof checks never Stop or Start and cannot repair a failed
    /// observation or replace this invocation's retained originals.
    pub(crate) fn recheck(&self) -> Result<()> {
        self.qualification.recheck(|| {
            self.migration()?;
            self.owner
                .recheck()
                .map_err(|()| NativeFirstError::Admission)?;
            self.migration()
        })
    }

    /// Identity-only binding to the SAME Migration in NativeRecoveryOrigin,
    /// with qualified/unpoisoned state and current lock authorization. The
    /// caller must bracket effects with full recheck; this is no effect grant.
    pub(crate) fn bind(&self, lock: &MigrationLock, paths: &CutoverPaths, uid: u32) -> Result<()> {
        self.qualification.checked(|| {
            if uid != self.uid
                || paths != &self.paths
                || !self
                    .lock
                    .get()
                    .is_some_and(|original| std::ptr::eq(original, lock))
            {
                return Err(NativeFirstError::Admission);
            }
            self.qualification.recheck(|| self.migration())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_quiescence_stop_requires_exclusive_and_is_once_only() {
        let refused = Qualification::default();
        let effects = Cell::new(0);
        assert!(
            refused
                .qualify(false, || {
                    effects.set(1);
                    Ok(())
                })
                .is_err()
        );
        assert!(
            refused
                .qualify(true, || {
                    effects.set(2);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(effects.get(), 0);

        let qualified = Qualification::default();
        assert!(
            qualified
                .qualify(true, || {
                    effects.set(effects.get() + 1);
                    Ok(())
                })
                .is_ok()
        );
        assert!(qualified.recheck(|| Ok(())).is_ok());
        assert!(
            qualified
                .qualify(true, || {
                    effects.set(99);
                    Ok(())
                })
                .is_err()
        );
        assert!(qualified.recheck(|| Ok(())).is_err());
        assert_eq!(effects.get(), 1);
    }

    #[test]
    fn old_quiescence_uncertainty_and_premature_recheck_latch_forever() {
        let effects = Cell::new(0);
        let uncertain = Qualification::default();
        assert!(
            uncertain
                .qualify(true, || {
                    effects.set(effects.get() + 1);
                    Err(NativeFirstError::Admission)
                })
                .is_err()
        );
        assert!(
            uncertain
                .qualify(true, || {
                    effects.set(99);
                    Ok(())
                })
                .is_err()
        );
        assert!(
            uncertain
                .recheck(|| {
                    effects.set(99);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(effects.get(), 1);

        let early = Qualification::default();
        assert!(
            early
                .recheck(|| {
                    effects.set(99);
                    Ok(())
                })
                .is_err()
        );
        assert!(
            early
                .qualify(true, || {
                    effects.set(99);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(effects.get(), 1);

        let drift = Qualification::default();
        assert!(drift.qualify(true, || Ok(())).is_ok());
        assert!(drift.recheck(|| Err(NativeFirstError::Admission)).is_err());
        assert!(
            drift
                .recheck(|| {
                    effects.set(99);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(effects.get(), 1);
    }
}
