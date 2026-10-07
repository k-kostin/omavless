// SPDX-License-Identifier: MIT
//! Actual startup seam with an inactive retained-evidence alternative.
//! No normal dispatch, mutation permission or serialized bypass exists.
use crate::cutover::{CutoverPaths, MigrationLock};
use std::marker::PhantomData;
pub(crate) enum StartupAdmission<'a, 'b> {
    Ordinary(PhantomData<&'a &'b ()>),
    HistoricalOff(&'a mut crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::RetainedEpochOff<'b>),
    #[cfg(feature = "t4-manager-actor-service")]
    NativeCompleted(crate::native_coordinator::NativeCompletedOff<'a, 'b>),
}
impl StartupAdmission<'_, '_> {
    pub(crate) fn desired(
        &mut self,
        paths: &crate::desired::DesiredPaths,
        uid: u32,
    ) -> Result<crate::desired::DesiredState, crate::desired::DesiredError> {
        match self {
            Self::Ordinary(_) => crate::desired::read_desired(paths, uid),
            Self::HistoricalOff(_) => {
                self.recheck()
                    .map_err(|_| crate::desired::DesiredError::UnsafeStateDirectory)?;
                crate::desired::read_desired_snapshot(paths, uid)
            }
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(e) => e
                .desired(paths, uid)
                .map_err(|_| crate::desired::DesiredError::UnsafeStateDirectory),
        }
    }
    pub(crate) fn marker(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
    ) -> Result<crate::cutover::OwnershipMarker, ()> {
        match self {
            Self::Ordinary(_) => crate::cutover::read_marker(paths, uid).map_err(|_| ()),
            Self::HistoricalOff(_) => {
                self.recheck()?;
                crate::cutover::read_marker_existing(paths, uid).map_err(|_| ())
            }
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(e) => e.marker(paths, uid),
        }
    }
    pub(crate) fn ordinary() -> Self {
        Self::Ordinary(PhantomData)
    }
    pub(crate) fn recheck(&mut self) -> Result<(), ()> {
        match self {
            Self::Ordinary(_) => Ok(()),
            Self::HistoricalOff(e) => e.recheck().map_err(|_| ()),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(e) => e.recheck(),
        }
    }
    pub(crate) fn bind(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
        lock: &MigrationLock,
    ) -> Result<(), ()> {
        match self {
            Self::Ordinary(_) => {
                if lock.authorizes(paths, uid) {
                    Ok(())
                } else {
                    Err(())
                }
            }
            Self::HistoricalOff(e) => e.bind(paths, uid, lock).map_err(|_| ()),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(e) => e.bind(paths, uid, lock),
        }
    }
    pub(crate) fn receipt(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
        lock: &MigrationLock,
        generation: u64,
    ) -> Result<(), ()> {
        self.bind(paths, uid, lock)?;
        match self {
            Self::Ordinary(_) => {
                crate::login_transaction::check_startup_receipt(paths, uid, lock, Some(generation))
                    .map_err(|_| ())
            }
            Self::HistoricalOff(_) => self.recheck(),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(_) => self.recheck(),
        }
    }
    pub(crate) fn transaction(
        &mut self,
        paths: &CutoverPaths,
        desired: &crate::desired::DesiredPaths,
        uid: u32,
        lock: &MigrationLock,
        independently_blocked: bool,
    ) -> Result<(), ()> {
        self.bind(paths, uid, lock)?;
        if independently_blocked {
            return Err(());
        }
        match self {
            Self::Ordinary(_) if crate::pending_private_transaction::pending(desired) => Err(()),
            _ => self.recheck(),
        }
    }
    pub(crate) fn action(&mut self, action: crate::desired::ReconcileAction) -> Result<(), ()> {
        self.recheck()?;
        match self {
            Self::HistoricalOff(_)
                if action != crate::desired::ReconcileAction::SettledDisconnected =>
            {
                Err(())
            }
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(_)
                if action != crate::desired::ReconcileAction::SettledDisconnected =>
            {
                Err(())
            }
            _ => {
                let _ = action;
                Ok(())
            }
        }
    }
    pub(crate) fn pointer(
        &mut self,
        plan: &crate::private_store_transaction::PreparedPointerMutation,
    ) -> Result<(), ()> {
        self.recheck()?;
        match self {
            Self::HistoricalOff(_) if plan.changed() => Err(()),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::NativeCompleted(_) if plan.changed() => Err(()),
            _ => {
                let _ = plan;
                Ok(())
            }
        }
    }
}
