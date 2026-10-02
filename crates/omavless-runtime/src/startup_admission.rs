// SPDX-License-Identifier: MIT
//! Actual startup seam with a test-only retained-evidence research alternative.
//! No product constructor, mutation permission or serialized bypass exists.
use crate::cutover::{CutoverPaths, MigrationLock};
use std::marker::PhantomData;
pub(crate) enum StartupAdmission<'a, 'b> {
    Ordinary(PhantomData<&'a &'b ()>),
    #[cfg(test)]
    HistoricalOff(&'a mut crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::RetainedEpochOff<'b>),
}
impl StartupAdmission<'_, '_> {
    pub(crate) fn desired(
        &mut self,
        paths: &crate::desired::DesiredPaths,
        uid: u32,
    ) -> Result<crate::desired::DesiredState, crate::desired::DesiredError> {
        match self {
            Self::Ordinary(_) => crate::desired::read_desired(paths, uid),
            #[cfg(test)]
            Self::HistoricalOff(_) => {
                self.recheck()
                    .map_err(|_| crate::desired::DesiredError::UnsafeStateDirectory)?;
                crate::desired::read_desired_snapshot(paths, uid)
            }
        }
    }
    pub(crate) fn marker(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
    ) -> Result<crate::cutover::OwnershipMarker, ()> {
        match self {
            Self::Ordinary(_) => crate::cutover::read_marker(paths, uid).map_err(|_| ()),
            #[cfg(test)]
            Self::HistoricalOff(_) => {
                self.recheck()?;
                crate::cutover::read_marker_existing(paths, uid).map_err(|_| ())
            }
        }
    }
    pub(crate) fn ordinary() -> Self {
        Self::Ordinary(PhantomData)
    }
    pub(crate) fn recheck(&mut self) -> Result<(), ()> {
        match self {
            Self::Ordinary(_) => Ok(()),
            #[cfg(test)]
            Self::HistoricalOff(e) => e.recheck().map_err(|_| ()),
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
            #[cfg(test)]
            Self::HistoricalOff(e) => e.bind(paths, uid, lock).map_err(|_| ()),
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
            #[cfg(test)]
            Self::HistoricalOff(_) => self.recheck(),
        }
    }
    pub(crate) fn transaction(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
        lock: &MigrationLock,
        independently_blocked: bool,
    ) -> Result<(), ()> {
        self.bind(paths, uid, lock)?;
        if independently_blocked {
            return Err(());
        }
        match self {
            Self::Ordinary(_)
                if crate::pending_private_transaction::pending_at(&paths.state_directory) =>
            {
                Err(())
            }
            _ => self.recheck(),
        }
    }
    pub(crate) fn action(&mut self, action: crate::desired::ReconcileAction) -> Result<(), ()> {
        self.recheck()?;
        match self {
            #[cfg(test)]
            Self::HistoricalOff(_)
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
            #[cfg(test)]
            Self::HistoricalOff(_) if plan.changed() => Err(()),
            _ => {
                let _ = plan;
                Ok(())
            }
        }
    }
}
