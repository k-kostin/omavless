// SPDX-License-Identifier: MIT
//! Bounded client-session history, never raw daemon logs or private targets.
use crate::{
    actions::Kind,
    model::{Mode, ReadError, Status},
};
use std::{collections::VecDeque, time::Instant};

pub const CAPACITY: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Observed(Status),
    ReadFailed(ReadError),
    RuntimeChanged,
    ModeObserved(Mode),
    Submitted(Kind),
    Retried,
    Applied,
    Rejected,
    Unknown,
    Acknowledged,
}
impl Event {
    pub fn key(self) -> &'static str {
        match self {
            Self::Observed(Status::Connected) => "tui.event_connected",
            Self::Observed(Status::Disconnected) => "tui.event_disconnected",
            Self::Observed(Status::Recovery) => "tui.event_recovery",
            Self::Observed(Status::Unverified) => "tui.event_unverified",
            Self::ReadFailed(ReadError::Unavailable) => "tui.event_unavailable",
            Self::ReadFailed(ReadError::Incompatible) => "tui.event_incompatible",
            Self::ReadFailed(ReadError::Invalid) => "tui.event_invalid",
            Self::ReadFailed(ReadError::Changed) => "tui.event_changed",
            Self::RuntimeChanged => "tui.event_restarted",
            Self::ModeObserved(Mode::Rule) => "tui.event_mode_routing",
            Self::ModeObserved(Mode::Global) => "tui.event_mode_full",
            Self::ModeObserved(Mode::Direct) => "tui.event_mode_direct",
            Self::Submitted(Kind::Connect) => "tui.event_connect_request",
            Self::Submitted(Kind::Disconnect) => "tui.event_disconnect_request",
            Self::Submitted(Kind::Mode) => "tui.event_mode_request",
            Self::Submitted(Kind::RefreshSubscription) => "tui.event_refresh_request",
            Self::Retried => "tui.event_retry",
            Self::Applied => "tui.event_applied",
            Self::Rejected => "tui.event_rejected",
            Self::Unknown => "tui.event_unknown",
            Self::Acknowledged => "tui.event_acknowledged",
        }
    }
}

pub struct Entry {
    pub elapsed_seconds: u64,
    pub event: Event,
}
pub struct Activity {
    started: Instant,
    entries: VecDeque<Entry>,
}
impl Default for Activity {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            entries: VecDeque::with_capacity(CAPACITY),
        }
    }
}
impl Activity {
    pub fn record(&mut self, event: Event, at: Instant) {
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        // A slow read may have started before a command event; preserve
        // monotonic display times without inventing wall-clock timestamps.
        let elapsed_seconds = at
            .saturating_duration_since(self.started)
            .as_secs()
            .max(self.entries.back().map_or(0, |e| e.elapsed_seconds));
        self.entries.push_back(Entry {
            elapsed_seconds,
            event,
        });
    }
    pub fn newest_first(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().rev()
    }
}
