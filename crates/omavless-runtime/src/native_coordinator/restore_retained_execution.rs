// SPDX-License-Identifier: MIT
//! One installed, nonescaping native restore custody slot. No dispatcher.
use super::*;
use std::sync::{Arc, Mutex};

pub(crate) struct HeldExecution {
    // Each reported object is inserted before the next fallible operation.
    lock: Option<MigrationLock>,
    boundary: Option<Boundary>,
    authenticated: Option<omavless_domain::private_backup::OpenedBackup>,
    prepared: Option<PreparedRestorePair>,
    engine: crate::manager_actor_service::NativeEngine,
    fenced: bool,
}
impl HeldExecution {
    fn reserve() -> Self {
        Self {
            lock: None,
            boundary: None,
            authenticated: None,
            prepared: None,
            engine: crate::manager_actor_service::NativeEngine::reserve(),
            fenced: false,
        }
    }
}

#[derive(Default)]
pub(crate) struct HeldExecutionSlot {
    original: Option<Arc<Mutex<HeldExecution>>>,
    #[cfg(test)]
    reservation: Option<Arc<Mutex<HeldExecution>>>,
}
impl HeldExecutionSlot {
    pub(crate) fn occupied(&self) -> bool {
        self.original.is_some()
    }
    fn install(&mut self) -> Result<Arc<Mutex<HeldExecution>>, FirstError> {
        if self.original.is_some() {
            return Err(FirstError::StillFenced);
        }
        #[cfg(test)]
        let original = self
            .reservation
            .take()
            .unwrap_or_else(|| Arc::new(Mutex::new(HeldExecution::reserve())));
        #[cfg(not(test))]
        let original = Arc::new(Mutex::new(HeldExecution::reserve()));
        self.original = Some(Arc::clone(&original)); // BEFORE acquisition/effects
        Ok(original)
    }
}
impl Drop for HeldExecutionSlot {
    fn drop(&mut self) {
        // Losing the owner cannot turn its unresolved lease/prefix into free
        // capacity while this process lives. No resurrection/lookup API.
        // Actual process death only means unavailable, not descriptor survival.
        if let Some(original) = self.original.take() {
            std::mem::forget(original);
        }
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Private developer path; authentication still precedes the lease. The
    /// installed slot remains occupied across errors, unwind and StillFenced.
    #[allow(dead_code)]
    pub(super) fn execute_first_restore_retained(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<FirstOutcome, FirstError> {
        self.execute_first_restore_retained_gated(source, passphrase, || Ok(()), |_| Ok(()))
    }
    fn execute_first_restore_retained_gated(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        mut acquired: impl FnMut() -> Result<(), FirstError>,
        mut cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
    ) -> Result<FirstOutcome, FirstError> {
        if self.retained_restore_busy() {
            return Err(FirstError::StillFenced);
        }
        let incoming =
            open_existing(source, self.uid(), passphrase).map_err(|_| FirstError::Prepare)?;
        let original = self.held_restore_execution.install()?;
        let result = (|| {
            // This is a clone of the SAME installed holder, not copied authority.
            let mut held = original.lock().map_err(|_| FirstError::StillFenced)?;
            held.authenticated = Some(incoming); // same original plaintext before the lease
            held.lock = Some(
                self.transaction
                    .acquire_lock()
                    .map_err(|_| FirstError::Admission)?,
            );
            acquired()?; // internal fault withdrawal only, not a grant
            Boundary::capture_installed(self, &mut held.boundary)?;
            let (original_pair, restore_store, readiness) = self
                .prepare_restore_parts(
                    held.authenticated.as_ref().ok_or(FirstError::Admission)?,
                    held.lock.as_ref().ok_or(FirstError::Admission)?,
                )
                .map_err(|_| FirstError::Admission)?;
            // After complete preparation, a nonallocating move inside the same
            // installed holder. Never remove the holder/lease/engine around work.
            held.prepared = Some(PreparedRestorePair {
                original: original_pair,
                incoming: held.authenticated.take().ok_or(FirstError::Admission)?,
                restore_store,
                readiness,
            });
            let HeldExecution {
                lock,
                boundary,
                prepared,
                engine,
                ..
            } = &mut *held;
            let prepared = prepared.as_ref().ok_or(FirstError::Admission)?;
            let instance = self.batch.as_ref().map(|batch| batch.instance.clone());
            let session = Session {
                owner: self,
                lock: lock.as_ref().ok_or(FirstError::Admission)?,
                boundary: boundary.as_ref().ok_or(FirstError::Admission)?,
                readiness: *prepared.readiness(),
                instance,
            };
            let mut origin = NativeSessionOrigin { session };
            engine
                .execute_native(&mut origin, prepared, &mut cut)
                .map_err(|_| FirstError::StillFenced)?;
            held.fenced = true;
            Ok(FirstOutcome::CommittedStillFenced)
        })();
        self.transaction.block();
        self.invalidate_connection_close();
        // Never take/reinsert or clear the installed holder, even on success.
        result
    }
}

#[cfg(test)]
#[test]
fn native_vm_reservation_installs_same_original_once() {
    let mut slot = HeldExecutionSlot::reserve_vm().unwrap();
    assert!(slot.vm_reserved() && !slot.occupied());
    let original = Arc::clone(slot.reservation.as_ref().unwrap());
    let installed = slot.install().unwrap();
    assert!(Arc::ptr_eq(&original, &installed));
    assert!(Arc::ptr_eq(slot.original.as_ref().unwrap(), &installed));
    assert!(slot.occupied() && !slot.vm_reserved() && slot.reservation.is_none());
    assert!(slot.install().is_err());
}

#[cfg(test)]
impl HeldExecutionSlot {
    pub(crate) fn reserve_vm() -> Result<Self, FirstError> {
        let mut held = HeldExecution::reserve();
        held.boundary = Some(Boundary::reserve_installed()?);
        Ok(Self {
            original: None,
            reservation: Some(Arc::new(Mutex::new(held))),
        })
    }
    pub(crate) fn vm_reserved(&self) -> bool {
        self.original.is_none()
            && self.reservation.as_ref().is_some_and(|original| {
                original.lock().is_ok_and(|held| {
                    held.lock.is_none()
                        && held.authenticated.is_none()
                        && held.prepared.is_none()
                        && held.boundary.as_ref().is_some_and(|boundary| {
                            boundary.directories.capacity() >= 3
                                && boundary.members.capacity() >= 3
                                && boundary.live.capacity() >= 2
                                && boundary.capture_prefix.capacity() >= 1
                        })
                })
            })
    }
    pub(super) fn original_lease_held(
        &self,
        paths: &crate::cutover::CutoverPaths,
        uid: u32,
    ) -> bool {
        let Some(original) = self.original.as_ref() else {
            return false;
        };
        let Ok(held) = original.lock() else {
            return false;
        };
        held.lock
            .as_ref()
            .is_some_and(|lock| lock.authorizes(paths, uid))
    }
}

#[cfg(test)]
impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    pub(crate) fn retained_vm_install_reservation(
        &mut self,
        reserved: HeldExecutionSlot,
    ) -> Result<(), FirstError> {
        if self.held_restore_execution.occupied()
            || self.held_restore_execution.reservation.is_some()
            || !reserved.vm_reserved()
        {
            return Err(FirstError::StillFenced);
        }
        self.held_restore_execution = reserved;
        Ok(())
    }
    pub(crate) fn retained_vm_fault(&mut self, source: &Path, passphrase: &[u8], case: u8) -> bool {
        if case > 2 {
            return false;
        }
        let reached = std::cell::Cell::new(false);
        let result = self.execute_first_restore_retained_gated(
            source,
            passphrase,
            || {
                if case == 0 {
                    reached.set(true);
                    Err(FirstError::StillFenced)
                } else {
                    Ok(())
                }
            },
            |step| {
                if matches!(
                    (case, step),
                    (1, crate::manager_actor_service::NativeStep::Intent)
                        | (2, crate::manager_actor_service::NativeStep::Renamed(0))
                ) {
                    reached.set(true);
                    return Err(FirstError::StillFenced);
                }
                Ok(())
            },
        );
        reached.get() && result == Err(FirstError::StillFenced)
    }
    pub(crate) fn retained_vm_ordinary_and_recovery_denied(&mut self) -> bool {
        self.retained_restore_busy()
            && self
                .initialize_batch_operations("fixed-held-denial")
                .is_err()
            && self.reconcile_startup().is_err()
            && self.publish_restore_completion_candidate().is_err()
            && self.finalize_terminal_restore_candidate().is_err()
            && self.retire_terminal_restore_candidate().is_err()
    }
    pub(crate) fn retained_vm_execute(&mut self, source: &Path, passphrase: &[u8]) -> bool {
        self.execute_first_restore_retained(source, passphrase)
            == Ok(FirstOutcome::CommittedStillFenced)
    }
    pub(crate) fn retained_vm_custody(&self) -> bool {
        self.held_restore_execution.occupied()
            && self
                .held_restore_execution
                .original_lease_held(self.transaction.cutover_paths(), self.uid())
    }
    pub(super) fn retained_test_after_lease(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        unwind: bool,
    ) -> Result<FirstOutcome, FirstError> {
        self.execute_first_restore_retained_gated(
            source,
            passphrase,
            || {
                if unwind {
                    panic!("fixed_native_lease_cut");
                }
                Err(FirstError::StillFenced)
            },
            |_| Ok(()),
        )
    }

    pub(super) fn retained_test_at_step(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
    ) -> Result<FirstOutcome, FirstError> {
        self.execute_first_restore_retained_gated(source, passphrase, || Ok(()), cut)
    }
}
