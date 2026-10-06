// SPDX-License-Identifier: MIT
//! Dormant fixed OS hint source. Neither provenance nor quiescence is a
//! connection/recovery authorization. No normal factory constructs this.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "private source awaits reviewed owner integration")
)]

use std::collections::VecDeque;
#[cfg(test)]
use std::path::Path;
use std::time::{Duration, Instant};

mod bounded_bus;
mod logind;
mod route;
#[cfg(test)]
mod tests;

const MAX_WIRE_BYTES: usize = 8 * 1024;
const MAX_QUEUED_FRAMES: usize = 32;
const MAX_HINTS: usize = 16;
const MAX_POLL_BYTES: usize = 64 * 1024;
const MAX_STARTUP_BYTES: usize = MAX_WIRE_BYTES * MAX_QUEUED_FRAMES;
const STARTUP_BUDGET: Duration = Duration::from_secs(2);
const POLL_BUDGET: Duration = Duration::from_millis(100);

/// Categorical only. No raw bus errors, names, addresses or datagrams retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lost {
    Unavailable,
    Deadline,
    OwnerChanged,
    Authentication,
    InitializationRace,
    InvalidFrame,
    Overflow,
    UnexpectedDescriptors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Suspend,
    Resume,
    NetworkChanged,
}

/// Receiver-assigned sequence counts emitted hints ONLY, not raw bus replies,
/// netlink header sequence fields or ignored traffic. It is not a network epoch.
pub(crate) struct Emission {
    pub(crate) sequence: u64,
    pub(crate) event: Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SleepState {
    Awake,
    Suspended,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotCurrent {
    Pending,
    Suspended,
    Lost(Lost),
}

pub(crate) struct HostEventSource {
    logind: logind::Logind,
    route: Option<route::RouteSocket>, // None is reachable only by test fixture ctor.
    pending: VecDeque<Emission>,
    sequence: u64,
    lost: Option<Lost>,
}
impl HostEventSource {
    #[expect(
        dead_code,
        reason = "fixed host constructor is dormant; no fixture invokes host transports"
    )]
    pub(crate) fn system() -> Result<Self, Lost> {
        let deadline = Instant::now() + STARTUP_BUDGET;
        let logind = async_io::block_on(within(deadline, logind::Logind::system(deadline)))?;
        let route = route::RouteSocket::system()?;
        if Instant::now() >= deadline {
            return Err(Lost::Deadline);
        }
        Self::assemble(logind, Some(route))
    }
    #[cfg(test)]
    pub(super) fn fixture(path: &Path) -> Result<Self, Lost> {
        let deadline = Instant::now() + STARTUP_BUDGET;
        let logind = async_io::block_on(within(deadline, logind::Logind::fixture(path, deadline)))?;
        Self::assemble(logind, None)
    }
    fn assemble(logind: logind::Logind, route: Option<route::RouteSocket>) -> Result<Self, Lost> {
        let mut source = Self {
            logind,
            route,
            pending: VecDeque::new(),
            sequence: 0,
            lost: None,
        };
        if source.logind.sleeping() {
            source.emit(Event::Suspend)?;
        }
        // Initial false establishes observed Awake; NEVER fabricates Resume.
        Ok(source)
    }
    fn emit(&mut self, event: Event) -> Result<(), Lost> {
        if self.pending.len() == MAX_HINTS {
            return self.fail(Lost::Overflow);
        }
        self.sequence = match self.sequence.checked_add(1) {
            Some(n) => n,
            None => return self.fail(Lost::Overflow),
        };
        self.pending.push_back(Emission {
            sequence: self.sequence,
            event,
        });
        Ok(())
    }
    fn fail<T>(&mut self, error: Lost) -> Result<T, Lost> {
        self.lost.get_or_insert(error);
        self.logind.terminal(error);
        self.pending.clear();
        Err(self.lost.unwrap())
    }
    fn check(&self) -> Result<(), Lost> {
        self.lost.map_or(Ok(()), Err)
    }
    pub(crate) fn sleep_state(&self) -> SleepState {
        if self.logind.sleeping() {
            SleepState::Suspended
        } else {
            SleepState::Awake
        }
    }
    pub(crate) fn readable(&mut self) -> Result<bool, Lost> {
        self.check()?;
        let result = (|| {
            // Pending downstream hints never short-circuit upstream loss checks.
            let bus = self.logind.readable()?;
            let route = self
                .route
                .as_ref()
                .map_or(Ok(false), route::RouteSocket::readable)?;
            Ok(!self.pending.is_empty() || bus || route)
        })();
        match result {
            Ok(ready) => Ok(ready),
            Err(error) => self.fail(error),
        }
    }
    pub(crate) fn poll(&mut self) -> Result<(), Lost> {
        self.check()?;
        let deadline = Instant::now() + POLL_BUDGET;
        let result = (|| {
            let events = async_io::block_on(within(deadline, self.logind.poll(deadline)))?;
            for event in events {
                self.emit(event)?;
            }
            if let Some(route) = &self.route {
                let events = route.poll(deadline)?;
                for event in events {
                    self.emit(event)?;
                }
            }
            if Instant::now() >= deadline {
                return Err(Lost::Deadline);
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.fail(error),
        }
    }
    pub(crate) fn next(&mut self) -> Result<Option<Emission>, Lost> {
        self.check()?;
        // Classify original upstream loss/replacement before releasing older
        // queued hints; the downstream queue is never an authenticity boundary.
        self.poll()?;
        Ok(self.pending.pop_front())
    }
    fn check_quiescent(&mut self) -> Result<(), NotCurrent> {
        self.check().map_err(NotCurrent::Lost)?;
        let deadline = Instant::now() + POLL_BUDGET;
        let result = (|| {
            let events = async_io::block_on(within(deadline, self.logind.quiescent(deadline)))?;
            for event in events {
                self.emit(event)?;
            }
            if let Some(route) = &self.route {
                let events = route.poll(deadline)?;
                for event in events {
                    self.emit(event)?;
                }
            }
            if Instant::now() >= deadline {
                return Err(Lost::Deadline);
            }
            Ok(())
        })();
        if let Err(error) = result {
            return self.fail(error).map_err(NotCurrent::Lost);
        }
        if !self.pending.is_empty() {
            return Err(NotCurrent::Pending);
        }
        if self.sleep_state() == SleepState::Suspended {
            return Err(NotCurrent::Suspended);
        }
        Ok(())
    }
    /// Momentary CURRENT original transports, not atomic event exclusion or a
    /// recovery permit. Checks retained pause and same-connection owner readback.
    pub(crate) fn quiescent(&mut self) -> Result<SourceContinuity<'_>, NotCurrent> {
        self.check_quiescent()?;
        let sequence = self.sequence;
        Ok(SourceContinuity {
            source: self,
            sequence,
        })
    }
    #[cfg(test)]
    fn synthetic_route(&mut self, bytes: &[u8], port: u32, truncated: bool) -> Result<(), Lost> {
        self.check()?;
        let count = match route::synthetic(bytes, port, truncated) {
            Ok(n) => n,
            Err(e) => return self.fail(e),
        };
        for _ in 0..count {
            self.emit(Event::NetworkChanged)?;
        }
        Ok(())
    }
}

/// Non-cloneable borrow of THAT original mutable adapter. No generic healthy
/// boolean/copy token escapes; loss and generation changes cannot be rearmed.
pub(crate) struct SourceContinuity<'a> {
    source: &'a mut HostEventSource,
    sequence: u64,
}
impl SourceContinuity<'_> {
    pub(crate) fn recheck(&mut self) -> Result<(), NotCurrent> {
        self.source.check_quiescent()?;
        if self.source.sequence != self.sequence {
            Err(NotCurrent::Pending)
        } else {
            Ok(())
        }
    }
}

async fn within<T>(
    deadline: Instant,
    future: impl std::future::Future<Output = Result<T, Lost>>,
) -> Result<T, Lost> {
    if Instant::now() >= deadline {
        return Err(Lost::Deadline);
    }
    let result = futures_lite::future::or(future, async {
        async_io::Timer::at(deadline).await;
        Err(Lost::Deadline)
    })
    .await;
    if Instant::now() >= deadline {
        Err(Lost::Deadline)
    } else {
        result
    }
}
