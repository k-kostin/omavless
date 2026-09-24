// SPDX-License-Identifier: MIT
//! Client presentation state for runtime-owned jobs, never a job executor.
use crate::{
    jobs::{self, Kind, Progress, Request, Tracker, Update},
    model::{ReadError, Snapshot},
};
use serde_json::Value;
use std::time::Instant;

#[derive(Clone)]
pub struct Intent {
    pub kind: Kind,
    pub target: Option<String>,
    pub label: String,
    pub count: usize,
    instance: String,
    revision: u64,
}
impl Intent {
    pub fn new(s: &Snapshot, kind: Kind, target: Option<String>) -> Option<Self> {
        let (label, count) = match kind {
            Kind::RefreshAll
                if s.capabilities.refresh_all && !s.metadata.subscriptions.is_empty() =>
            {
                (String::new(), s.metadata.subscriptions.len())
            }
            Kind::ProfileProbe if s.capabilities.profile_probe => {
                if let Some(id) = &target {
                    let p = s
                        .metadata
                        .profiles
                        .iter()
                        .find(|p| p.id == *id && !p.missing)?;
                    (p.name.clone(), 1)
                } else {
                    let count = s.metadata.profiles.iter().filter(|p| !p.missing).count();
                    if count == 0 {
                        return None;
                    }
                    (String::new(), count)
                }
            }
            _ => return None,
        };
        Some(Self {
            kind,
            target,
            label,
            count,
            instance: s.metadata.instance_id.clone(),
            revision: s.revision,
        })
    }
    pub fn matches(&self, s: &Snapshot) -> bool {
        self.instance == s.metadata.instance_id
            && self.revision == s.revision
            && Self::new(s, self.kind, self.target.clone())
                .is_some_and(|new| new.label == self.label && new.count == self.count)
    }
    pub fn request(&self, operation: String) -> Option<Request> {
        Request::new(
            self.kind,
            self.instance.clone(),
            self.revision,
            operation,
            self.target.clone(),
        )
    }
    pub fn key(&self) -> &'static str {
        match self.kind {
            Kind::RefreshAll => "tui.refresh_all",
            _ if self.target.is_some() => "tui.probe_selected",
            _ => "tui.probe_all",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Start,
    Poll,
    Cancel,
    Results,
}
pub struct Session {
    pub intent: Intent,
    pub request: Request,
    pub tracker: Tracker,
    pub in_flight: bool,
    pub finished: bool,
    pub notice: &'static str,
    pub rows: Option<Vec<jobs::ProbeRow>>,
    results_requested: bool,
}
impl Session {
    pub fn new(intent: Intent, request: Request, now: Instant) -> Self {
        Self {
            intent,
            request,
            tracker: Tracker::new(now),
            in_flight: false,
            finished: false,
            notice: "tui.job_waiting",
            rows: None,
            results_requested: false,
        }
    }
    pub fn blocks_actions(&self) -> bool {
        !self.finished || self.in_flight
    }
    pub fn call(&mut self, phase: Phase) -> Option<jobs::Call> {
        if self.in_flight {
            return None;
        }
        let call = match phase {
            Phase::Start => self.request.start(),
            Phase::Poll => self.request.poll(),
            Phase::Cancel if self.tracker.request_cancel() => self.request.cancel(),
            Phase::Results => self.request.results_call()?,
            _ => return None,
        };
        self.in_flight = true;
        Some(call)
    }
    pub fn next(&mut self, now: Instant) -> Option<Phase> {
        if self.in_flight {
            return None;
        }
        if self.finished {
            if !self.results_requested
                && self
                    .tracker
                    .progress
                    .is_some_and(|p| p.state == jobs::State::Succeeded)
                && self.request.results_call().is_some()
            {
                self.results_requested = true;
                return Some(Phase::Results);
            }
            return None;
        }
        let due = self.tracker.poll_due(now);
        if self.tracker.unknown {
            self.notice = "tui.job_unknown";
        }
        due.then_some(Phase::Poll)
    }
    pub fn accept(&mut self, phase: Phase, value: Result<Value, ReadError>) {
        self.in_flight = false;
        if phase == Phase::Results {
            self.rows = self.request.results(value);
            if self
                .rows
                .as_ref()
                .is_some_and(|rows| self.tracker.progress.is_none_or(|p| p.total != rows.len()))
            {
                self.rows = None;
            }
            if self.rows.is_none() {
                self.notice = "tui.job_results_unavailable";
            }
            return;
        }
        let update = self.request.progress(value);
        // Only a rejection of the initial start is known not to have run.
        if let Update::Rejected(key) = update
            && phase == Phase::Start
        {
            self.finished = true;
            self.notice = key;
            return;
        }
        if !self.tracker.accept(update) {
            self.notice = "tui.job_unknown";
            return;
        }
        let Some(p) = self.tracker.progress else {
            return;
        };
        self.finished = p.state.terminal();
        self.notice = progress_key(p);
    }
}
pub fn progress_key(p: Progress) -> &'static str {
    match p.state {
        jobs::State::Queued => "tui.job_waiting",
        jobs::State::Running if p.cancel_requested => "tui.job_cancelling",
        jobs::State::Running => "tui.job_running",
        jobs::State::Succeeded => "tui.job_succeeded",
        jobs::State::Failed => p.error.unwrap_or("tui.action_rejected"),
        jobs::State::Cancelled => "tui.job_cancelled",
    }
}
