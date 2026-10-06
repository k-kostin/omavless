// SPDX-License-Identifier: MIT

//! Clock-driven, inactive automatic-refresh executor. A tick performs at most
//! one bounded provider step outside the serialized owner. Production timer,
//! IPC and UI registration remain absent pending interrupted-attempt recovery.

use crate::lifecycle::LifecycleHost;
use crate::native_coordinator::{
    AutomaticRefreshError, AutomaticRefreshStart, AutomaticSubscriptionBatch, NativeOwnerError,
    OfflineNativeCoordinator,
};
use crate::remote_fetch::RemoteFetchPool;
use crate::subscription_batch_work::{BatchWorkStep, BudgetedSubscriptionTransport};
use crate::subscription_schedule_attempt::AttemptSnapshot;
use crate::subscription_schedule_plan::ScheduleDecision;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticRefreshTick {
    Idle(ScheduleDecision),
    Running,
    Finished(AttemptSnapshot),
    Stopped,
}

/// Holds one exact admitted worker, never a second runtime/lifecycle owner.
/// Pass the existing runtime fetch pool: its clones share the unary limit.
pub struct AutomaticSubscriptionDriver<H: LifecycleHost> {
    owner: Arc<Mutex<OfflineNativeCoordinator<H>>>,
    instance: String,
    pool: RemoteFetchPool,
    work: Option<AutomaticSubscriptionBatch>,
    stopped: bool,
}

impl<H: LifecycleHost> AutomaticSubscriptionDriver<H> {
    pub fn new(
        owner: Arc<Mutex<OfflineNativeCoordinator<H>>>,
        instance: &str,
        pool: &RemoteFetchPool,
    ) -> Self {
        Self {
            owner,
            instance: instance.to_owned(),
            pool: pool.clone(),
            work: None,
            stopped: false,
        }
    }

    /// The clock and record-ID source are trusted runtime dependencies, not
    /// request fields. The clock is sampled again after I/O for terminal time.
    pub fn tick<T, G, C>(
        &mut self,
        transport: &T,
        ids: &mut G,
        clock: C,
    ) -> Result<AutomaticRefreshTick, AutomaticRefreshError>
    where
        T: BudgetedSubscriptionTransport,
        G: FnMut() -> String,
        C: Fn() -> u64,
    {
        if self.stopped {
            return Ok(AutomaticRefreshTick::Stopped);
        }
        if self.work.is_none() {
            let start = self
                .owner
                .lock()
                .map_err(|_| AutomaticRefreshError::WorkerLost)?
                .start_automatic_subscription_refresh(&self.instance, clock())?;
            match start {
                AutomaticRefreshStart::Idle(decision) => {
                    return Ok(AutomaticRefreshTick::Idle(decision));
                }
                AutomaticRefreshStart::Started(work) => self.work = Some(work),
            }
        }
        let check = self
            .owner
            .lock()
            .map_err(|_| AutomaticRefreshError::WorkerLost)?
            .check_automatic_subscription_work(self.work.as_ref().expect("admitted worker"));
        if check.is_err() {
            // A missing/changed ownership proof cannot authorize another GET.
            // Completion still uses the original receipt/journal fences.
            return self.finish(clock());
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.work
                .as_mut()
                .expect("admitted worker")
                .step(transport, &self.pool, ids)
        }));
        match result {
            Ok(Ok(BatchWorkStep::Busy | BatchWorkStep::Advanced)) => {
                Ok(AutomaticRefreshTick::Running)
            }
            Ok(Ok(BatchWorkStep::Ready) | Err(_)) => self.finish(clock()),
            Err(_) => {
                if let Some(work) = self.work.take()
                    && let Ok(mut owner) = self.owner.lock()
                {
                    owner.lose_automatic_subscription_worker(work);
                }
                Err(AutomaticRefreshError::WorkerLost)
            }
        }
    }

    fn finish(&mut self, now_secs: u64) -> Result<AutomaticRefreshTick, AutomaticRefreshError> {
        let work = self.work.take().ok_or(AutomaticRefreshError::StaleWorker)?;
        self.owner
            .lock()
            .map_err(|_| AutomaticRefreshError::Owner(NativeOwnerError::Invariant))?
            .finish_automatic_subscription_refresh(work, now_secs)
            .map(AutomaticRefreshTick::Finished)
    }

    /// Cancels before consuming unfinished work. No provider I/O or new
    /// admission occurs while stopping; shutdown cannot silently reschedule.
    pub fn stop(&mut self, now_secs: u64) -> Result<AutomaticRefreshTick, AutomaticRefreshError> {
        self.stopped = true;
        if self.work.is_none() {
            return Ok(AutomaticRefreshTick::Stopped);
        }
        self.owner
            .lock()
            .map_err(|_| AutomaticRefreshError::WorkerLost)?
            .cancel_automatic_subscription_refresh()?;
        self.finish(now_secs)
    }
}

impl<H: LifecycleHost> Drop for AutomaticSubscriptionDriver<H> {
    fn drop(&mut self) {
        if let Some(work) = self.work.take()
            && let Ok(mut owner) = self.owner.lock()
        {
            owner.lose_automatic_subscription_worker(work);
        }
    }
}
