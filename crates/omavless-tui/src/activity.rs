// SPDX-License-Identifier: MIT
//! Bounded client-session history, never raw daemon logs or private targets.
use crate::{
    actions::Kind,
    model::{Mode, ReadError, Status},
};
use std::{collections::VecDeque, time::Instant};

pub const CAPACITY: usize = 32;

/// Closed local categories only: never retain an arbitrary notice or remote text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    Changed,
    Busy,
    Recovery,
    Restored,
    Denied,
    DnsSetup,
}
impl RefusalReason {
    fn key(self) -> &'static str {
        match self {
            Self::Changed => "tui.action_changed",
            Self::Busy => "tui.action_busy",
            Self::Recovery => "tui.action_recovery",
            Self::Restored => "tui.action_restored",
            Self::Denied => "tui.action_denied",
            Self::DnsSetup => "error.dns_pair_required",
        }
    }
}

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
    Refused(RefusalReason),
    Unknown,
    Acknowledged,
}
impl Event {
    pub(crate) fn rejected(key: &str) -> Self {
        let reason = match key {
            "tui.action_changed" => RefusalReason::Changed,
            "tui.action_busy" => RefusalReason::Busy,
            "tui.action_recovery" => RefusalReason::Recovery,
            "tui.action_restored" => RefusalReason::Restored,
            "tui.action_denied" => RefusalReason::Denied,
            "error.dns_pair_required" => RefusalReason::DnsSetup,
            _ => return Self::Rejected,
        };
        Self::Refused(reason)
    }

    pub(crate) fn detail_key(self) -> Option<&'static str> {
        match self {
            Self::Refused(reason) => Some(reason.key()),
            _ => None,
        }
    }

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
            Self::Rejected | Self::Refused(_) => "tui.event_rejected",
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
    older_events_discarded: bool,
}
impl Default for Activity {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            entries: VecDeque::with_capacity(CAPACITY),
            older_events_discarded: false,
        }
    }
}
impl Activity {
    pub fn record(&mut self, event: Event, at: Instant) {
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
            self.older_events_discarded = true;
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
    /// Sticky for this window, including across read failures and owner changes.
    pub fn older_events_discarded(&self) -> bool {
        self.older_events_discarded
    }
}
