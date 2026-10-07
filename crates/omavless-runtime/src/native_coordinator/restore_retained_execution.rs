// SPDX-License-Identifier: MIT
//! One installed, nonescaping native restore custody slot. No dispatcher.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub(super) fn completion_owns_lease(lease: &crate::connection_transaction::MigrationLease) -> bool {
    matches!(
        lease,
        crate::connection_transaction::MigrationLease::Owned(_)
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeMode {
    FenceCommitted,
    CompleteCommitted,
    PauseIntent,
}

pub(crate) struct HeldExecution {
    // Each reported object is inserted before the next fallible operation.
    lock: Option<crate::connection_transaction::MigrationLease>,
    boundary: Option<Boundary>,
    authenticated: Option<omavless_domain::private_backup::OpenedBackup>,
    prepared: Option<PreparedRestorePair>,
    engine: crate::manager_actor_service::NativeEngine,
    fenced: bool,
    // Reserved BEFORE any lease/source acquisition. Owns the ONE moved lock,
    // never another Flock wrapper; the source holder/engine remains installed.
    ordinary_lock: Arc<OnceLock<MigrationLock>>,
    failed_lock: Option<MigrationLock>,
    available: Arc<AtomicBool>,
    scope: Arc<AtomicBool>,
    activated: bool,
    intent_pause: Option<crate::manager_actor_service::NativeIntentPaused>,
    pause_revision: Option<crate::mutation::RetainedRestoreRevision>,
    pause_instance: Option<Option<String>>,
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
            ordinary_lock: Arc::new(OnceLock::new()),
            failed_lock: None,
            available: Arc::new(AtomicBool::new(true)),
            scope: Arc::new(AtomicBool::new(false)),
            activated: false,
            intent_pause: None,
            pause_revision: None,
            pause_instance: None,
        }
    }
    pub(super) fn refuse_ordinary(&mut self) {
        self.available.store(false, Ordering::Release);
        self.engine.revoke_native();
    }
    pub(super) fn check_ordinary(&mut self, lock: &Arc<OnceLock<MigrationLock>>) -> Result<(), ()> {
        if !Arc::ptr_eq(lock, &self.ordinary_lock)
            || self.failed_lock.is_some()
            || self.lock.is_some()
            || !self.engine.ordinary_lease_held()
        {
            return Err(());
        }
        self.engine.check_ordinary_lease_prefix()
    }
}

#[derive(Default)]
pub(crate) struct HeldExecutionSlot {
    original: Option<Arc<Mutex<HeldExecution>>>,
    #[cfg(test)]
    reservation: Option<Arc<Mutex<HeldExecution>>>,
}
impl HeldExecutionSlot {
    #[cfg(test)]
    pub(crate) fn occupied(&self) -> bool {
        self.original.is_some()
    }
    pub(crate) fn unavailable(&self) -> bool {
        self.original.as_ref().is_some_and(|original| {
            original.lock().map_or(true, |held| {
                !held.activated || !held.available.load(Ordering::Acquire)
            })
        })
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
    pub(crate) fn pause_current_restore_intent(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<(), FirstError> {
        self.execute_first_restore_mode(
            source,
            passphrase,
            || Ok(()),
            |_| Ok(()),
            NativeMode::PauseIntent,
        )
        .map(|_| ())
    }
    #[cfg(test)]
    pub(super) fn pause_current_restore_intent_cut(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        self.execute_first_restore_mode(source, passphrase, || Ok(()), cut, NativeMode::PauseIntent)
            .map(|_| ())
    }
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
    /// Private, one-shot SAME-owner NEW/Committed continuation. No dispatcher
    /// registration or default availability. Err retains/fences the graph.
    #[allow(dead_code)]
    pub(crate) fn execute_first_restore_completed(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<(), FirstError> {
        self.execute_first_restore_mode(
            source,
            passphrase,
            || Ok(()),
            |_| Ok(()),
            NativeMode::CompleteCommitted,
        )
        .map(|_| ())
    }

    pub(crate) fn execute_previewed_restore_completed(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        expected: &[u8; 32],
    ) -> Result<(), FirstError> {
        self.execute_first_restore_mode_bound(
            source,
            passphrase,
            || Ok(()),
            |_| Ok(()),
            NativeMode::CompleteCommitted,
            Some(expected),
        )
        .map(|_| ())
    }
    #[cfg(test)]
    pub(super) fn execute_first_restore_completed_cut(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        self.execute_first_restore_mode(
            source,
            passphrase,
            || Ok(()),
            cut,
            NativeMode::CompleteCommitted,
        )
        .map(|_| ())
    }
    fn execute_first_restore_retained_gated(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        acquired: impl FnMut() -> Result<(), FirstError>,
        cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
    ) -> Result<FirstOutcome, FirstError> {
        self.execute_first_restore_mode(
            source,
            passphrase,
            acquired,
            cut,
            NativeMode::FenceCommitted,
        )
    }
    fn execute_first_restore_mode(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        acquired: impl FnMut() -> Result<(), FirstError>,
        cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
        mode: NativeMode,
    ) -> Result<FirstOutcome, FirstError> {
        self.execute_first_restore_mode_bound(source, passphrase, acquired, cut, mode, None)
    }
    fn execute_first_restore_mode_bound(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        mut acquired: impl FnMut() -> Result<(), FirstError>,
        mut cut: impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
        mode: NativeMode,
        expected: Option<&[u8; 32]>,
    ) -> Result<FirstOutcome, FirstError> {
        let complete = mode == NativeMode::CompleteCommitted;
        let pause = mode == NativeMode::PauseIntent;
        if self.retained_restore_busy() {
            return Err(FirstError::StillFenced);
        }
        let mut revision = if complete || pause {
            Some(
                self.coordinator
                    .prepare_retained_restore()
                    .map_err(|_| FirstError::Admission)?,
            )
        } else {
            None
        };
        let mut disposition = None;
        let incoming = if let Some(expected) = expected {
            let (incoming, digest) =
                crate::backup_destination_candidate::open_existing_with_digest(
                    source,
                    self.uid(),
                    passphrase,
                )
                .map_err(|_| FirstError::Prepare)?;
            // This SAME bounded read is the engine's original authentication.
            // Mismatch precedes slot installation, lease and every effect.
            if &digest != expected {
                return Err(FirstError::CiphertextMismatch);
            }
            incoming
        } else {
            open_existing(source, self.uid(), passphrase).map_err(|_| FirstError::Prepare)?
        };
        // Pre-effect plan data only. No marker/history decoder can grant entry.
        let ordinary_paths = self.transaction.cutover_paths().clone();
        let ordinary_uid = self.uid();
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
            if (complete || pause)
                && (!held.lock.as_ref().is_some_and(completion_owns_lease)
                    || !self.transaction.original_lease_vacant()
                    || held.ordinary_lock.get().is_some())
            {
                return Err(FirstError::Admission);
            }
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
            let pause_instance = pause.then(|| instance.clone());
            let session = Session {
                owner: self,
                lock: lock.as_ref().ok_or(FirstError::Admission)?,
                boundary: boundary.as_ref().ok_or(FirstError::Admission)?,
                readiness: *prepared.readiness(),
                instance,
            };
            let mut origin = NativeSessionOrigin { session };
            if pause {
                let paused = engine.pause_native_intent(&mut origin, prepared, &mut cut)?;
                held.intent_pause = Some(paused);
                held.pause_revision = revision.take();
                held.pause_instance = pause_instance;
                return Ok(FirstOutcome::IntentPaused);
            }
            engine
                .execute_native(&mut origin, prepared, &mut cut)
                .map_err(|_| FirstError::StillFenced)?;
            if complete {
                disposition =
                    Some(engine.complete_native_committed(&mut origin, prepared, &mut cut)?);
                engine
                    .consume_into_ordinary_lease()
                    .map_err(|_| FirstError::StillFenced)?;
            }
            held.fenced = true;
            Ok(FirstOutcome::CommittedStillFenced)
        })();
        let result = if complete && result.is_ok() {
            (|| {
                let mut held = original.lock().map_err(|_| FirstError::StillFenced)?;
                let generation = held
                    .prepared
                    .as_ref()
                    .ok_or(FirstError::StillFenced)?
                    .readiness()
                    .owner_generation;
                let lock = match held.lock.take() {
                    Some(crate::connection_transaction::MigrationLease::Owned(lock)) => lock,
                    other => {
                        held.lock = other;
                        held.refuse_ordinary();
                        return Err(FirstError::StillFenced);
                    }
                };
                // Prechecked empty OnceLock. On even an impossible collision,
                // retain the returned actual lock in its reserved failure slot.
                if let Err(lock) = held.ordinary_lock.set(lock) {
                    held.failed_lock = Some(lock);
                    held.refuse_ordinary();
                    return Err(FirstError::StillFenced);
                }
                let keeper = native_recovery::NativeOrdinaryLease::committed(
                    Arc::clone(&original),
                    Arc::clone(&held.ordinary_lock),
                    Arc::clone(&held.available),
                    Arc::clone(&held.scope),
                    ordinary_paths,
                    ordinary_uid,
                    generation,
                );
                drop(held); // same graph remains installed; borrower locks it
                self.transaction
                    .install_original_lease(keeper)
                    .map_err(|_| FirstError::StillFenced)?;
                self.coordinator
                    .finish_retained_restore(
                        revision.ok_or(FirstError::StillFenced)?,
                        disposition.ok_or(FirstError::StillFenced)?,
                    )
                    .map_err(|_| FirstError::StillFenced)?;
                original
                    .lock()
                    .map_err(|_| FirstError::StillFenced)?
                    .activated = true;
                Ok(FirstOutcome::CompletedOrdinary)
            })()
        } else {
            result
        };
        if (mode == NativeMode::FenceCommitted) || result.is_err() {
            self.transaction.block();
        }
        self.invalidate_connection_close();
        // Never take/reinsert or clear the installed holder, even on success.
        result
    }

    pub(crate) fn current_intent_paused(&self) -> bool {
        self.held_restore_execution
            .original
            .as_ref()
            .is_some_and(|original| {
                original.lock().is_ok_and(|held| {
                    held.intent_pause.is_some()
                        && held.pause_revision.is_some()
                        && held.pause_instance.is_some()
                        && !held.activated
                        && held.available.load(Ordering::Acquire)
                })
            })
    }
    pub(crate) fn abort_current_restore_intent(&mut self) -> Result<(), FirstError> {
        self.abort_current_restore_intent_cut(&mut |_| Ok(()))
    }
    /// Factual diagnostic only: true means this SAME pause was consumed,
    /// original engine revoked/unavailable and transaction independently blocked.
    /// It cannot enable a continuation or mint any caller permission.
    pub(crate) fn refuse_unpublished_intent_pause(&mut self, revision: u64) -> bool {
        let mut sealed = false;
        if self.revision() == revision && self.current_intent_paused() {
            if let Some(original) = &self.held_restore_execution.original
                && let Ok(mut held) = original.lock()
            {
                held.intent_pause = None;
                held.pause_revision = None;
                held.pause_instance = None;
                held.refuse_ordinary();
                sealed = true; // all nonfallible SAME-held seal actions completed
            }
            self.transaction.block();
        }
        sealed && self.transaction.independently_blocked()
    }
    pub(super) fn abort_current_restore_intent_cut(
        &mut self,
        cut: &mut impl FnMut(crate::manager_actor_service::NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        if !self.current_intent_paused() {
            return Err(FirstError::Admission);
        }
        let original = self
            .held_restore_execution
            .original
            .as_ref()
            .cloned()
            .ok_or(FirstError::Admission)?;
        let result = (|| {
            let mut held = original.lock().map_err(|_| FirstError::StillFenced)?;
            if held.activated || !held.available.load(Ordering::Acquire) {
                return Err(FirstError::Admission);
            }
            // Exact one attempt: consume the original positive pause BEFORE
            // any current-origin check, read, synchronization or publication.
            let paused = held.intent_pause.take().ok_or(FirstError::Admission)?;
            let revision = held.pause_revision.take().ok_or(FirstError::StillFenced)?;
            let instance = held.pause_instance.take().ok_or(FirstError::StillFenced)?;
            if self.batch.as_ref().map(|batch| batch.instance.as_str()) != instance.as_deref() {
                return Err(FirstError::StillFenced);
            }
            let HeldExecution {
                lock,
                boundary,
                prepared,
                engine,
                ..
            } = &mut *held;
            let prepared = prepared.as_ref().ok_or(FirstError::StillFenced)?;
            let session = Session {
                owner: self,
                lock: lock.as_ref().ok_or(FirstError::StillFenced)?,
                boundary: boundary.as_ref().ok_or(FirstError::StillFenced)?,
                readiness: *prepared.readiness(),
                instance,
            };
            let mut origin = NativeSessionOrigin { session };
            let proof = engine.abort_paused_native_intent(&mut origin, prepared, paused, cut)?;
            engine
                .consume_into_ordinary_lease()
                .map_err(|_| FirstError::StillFenced)?;
            let generation = prepared.readiness().owner_generation;
            let paths = origin.session.owner.transaction.cutover_paths().clone();
            let uid = origin.uid();
            let lock = match held.lock.take() {
                Some(crate::connection_transaction::MigrationLease::Owned(lock)) => lock,
                other => {
                    held.lock = other;
                    return Err(FirstError::StillFenced);
                }
            };
            if let Err(lock) = held.ordinary_lock.set(lock) {
                held.failed_lock = Some(lock);
                return Err(FirstError::StillFenced);
            }
            let keeper = native_recovery::NativeOrdinaryLease::committed(
                Arc::clone(&original),
                Arc::clone(&held.ordinary_lock),
                Arc::clone(&held.available),
                Arc::clone(&held.scope),
                paths,
                uid,
                generation,
            );
            drop(held); // SAME installed graph; no fallible vacant ownership gap.
            self.transaction
                .install_original_lease(keeper)
                .map_err(|_| FirstError::StillFenced)?;
            self.coordinator
                .finish_retained_abort(revision, proof)
                .map_err(|_| FirstError::StillFenced)?;
            original
                .lock()
                .map_err(|_| FirstError::StillFenced)?
                .activated = true;
            Ok(())
        })();
        if result.is_err() {
            self.transaction.block();
            if let Ok(mut held) = original.lock() {
                held.refuse_ordinary();
            }
        }
        self.invalidate_connection_close();
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
