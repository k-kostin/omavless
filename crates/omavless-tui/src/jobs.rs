// SPDX-License-Identifier: MIT
//! Fixed, instance-fenced long-operation client protocol. No host effects,
//! private-store access, automatic start retry, or client-side probe executor.
use crate::model::ReadError;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub const POLL_INTERVAL: Duration = Duration::from_secs(1);
pub const WATCH_BUDGET: Duration = Duration::from_secs(30 * 60 + 10);
pub const MAX_POLLS: usize = 1810;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    RefreshAll,
    SubscriptionProbe,
    ProfileProbe,
}
impl Kind {
    pub fn method(self) -> &'static str {
        match self {
            Self::RefreshAll => "subscriptions.refresh_all",
            Self::SubscriptionProbe => "subscriptions.probe",
            Self::ProfileProbe => "profiles.probe",
        }
    }
    pub fn maximum(self) -> usize {
        match self {
            Self::RefreshAll => 64,
            Self::SubscriptionProbe | Self::ProfileProbe => 256,
        }
    }
    /// An older runtime must not receive an unsupported start operation.
    pub fn supported(self, methods: &[Value]) -> bool {
        methods.len() <= 128
            && [self.method(), "operations.get", "operations.cancel"]
                .iter()
                .all(|name| methods.iter().any(|v| v == name))
            && match self {
                Self::RefreshAll => true,
                Self::SubscriptionProbe => {
                    methods.iter().any(|v| v == "subscriptions.probe_results")
                }
                Self::ProfileProbe => methods.iter().any(|v| v == "profiles.probe_results"),
            }
    }
}

// Deliberately no Debug/Serialize: these carry private record/correlation IDs.
#[derive(Clone)]
pub struct Request {
    pub kind: Kind,
    instance: String,
    revision: u64,
    operation: String,
    target: Option<String>,
}

#[derive(Clone)]
pub struct Call {
    method: &'static str,
    params: Value,
}
impl Call {
    pub fn method(&self) -> &'static str {
        self.method
    }
    pub fn params(&self) -> Value {
        self.params.clone()
    }
}

fn opaque(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (33..=126).contains(&b))
}
fn record(value: &str) -> bool {
    // Stored legacy IDs may use UUID hex/brace/URN spellings. The runtime is
    // canonical; preserve its spelling without normalizing the selected key.
    if value.len() > 64 {
        return false;
    }
    let value = value.strip_prefix("urn:uuid:").unwrap_or(value);
    let value = if value.starts_with('{') {
        let Some(inner) = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) else {
            return false;
        };
        inner
    } else {
        value
    };
    value.bytes().all(|b| b == b'-' || b.is_ascii_hexdigit())
        && value.bytes().filter(|b| b.is_ascii_hexdigit()).count() == 32
}
fn revision(value: &Value) -> Option<u64> {
    value.as_u64().filter(|r| *r <= i64::MAX as u64)
}

impl Request {
    pub fn new(
        kind: Kind,
        instance: String,
        expected_revision: u64,
        operation: String,
        target: Option<String>,
    ) -> Option<Self> {
        if !opaque(&instance, 128)
            || !opaque(&operation, 64)
            || expected_revision > i64::MAX as u64
            || match kind {
                Kind::RefreshAll => target.is_some(),
                Kind::SubscriptionProbe => !target.as_deref().is_some_and(record),
                Kind::ProfileProbe => target.as_deref().is_some_and(|id| !record(id)),
            }
        {
            return None;
        }
        Some(Self {
            kind,
            instance,
            revision: expected_revision,
            operation,
            target,
        })
    }
    fn lookup(&self, method: &'static str) -> Call {
        Call {
            method,
            params: json!({"instanceId":self.instance,"operationId":self.operation}),
        }
    }
    pub fn start(&self) -> Call {
        let mut call = self.lookup(self.kind.method());
        call.params["expectedRevision"] = json!(self.revision);
        if let Some(id) = &self.target {
            call.params[if self.kind == Kind::ProfileProbe {
                "profileId"
            } else {
                "subscriptionId"
            }] = json!(id);
        }
        call
    }
    pub fn poll(&self) -> Call {
        self.lookup("operations.get")
    }
    pub fn cancel(&self) -> Call {
        self.lookup("operations.cancel")
    }
    pub fn results_call(&self) -> Option<Call> {
        match self.kind {
            Kind::RefreshAll => None,
            Kind::SubscriptionProbe => Some(self.lookup("subscriptions.probe_results")),
            Kind::ProfileProbe => Some(self.lookup("profiles.probe_results")),
        }
    }
    /// Errors return only local catalog keys, never daemon messages/details.
    /// Transport failure remains unknown; callers must not submit a fresh ID.
    pub fn progress(&self, response: Result<Value, ReadError>) -> Update {
        let Ok(value) = response else {
            return Update::Unknown;
        };
        let Some(envelope_revision) = revision(&value["revision"]) else {
            return Update::Unknown;
        };
        if value["ok"] == false {
            return safe_error(&value["error"])
                .map(Update::Rejected)
                .unwrap_or(Update::Unknown);
        }
        if value["ok"] != true {
            return Update::Unknown;
        }
        let op = &value["result"]["operation"];
        let parsed = (|| {
            let fields = op.as_object()?;
            if fields.len() != 10
                || ![
                    "instanceId",
                    "operationId",
                    "method",
                    "state",
                    "baseRevision",
                    "outcomeRevision",
                    "progress",
                    "cancelRequested",
                    "cancellable",
                    "error",
                ]
                .iter()
                .all(|key| fields.contains_key(*key))
                || op["instanceId"] != self.instance
                || op["operationId"] != self.operation
                || op["method"] != self.kind.method()
                || revision(&op["baseRevision"]) != Some(self.revision)
                || envelope_revision < self.revision
            {
                return None;
            }
            let state = match op["state"].as_str()? {
                "queued" => State::Queued,
                "running" => State::Running,
                "succeeded" => State::Succeeded,
                "failed" => State::Failed,
                "cancelled" => State::Cancelled,
                _ => return None,
            };
            let completed = usize::try_from(op["progress"]["completed"].as_u64()?).ok()?;
            let total = usize::try_from(op["progress"]["total"].as_u64()?).ok()?;
            let cancel_requested = op["cancelRequested"].as_bool()?;
            let cancellable = op["cancellable"].as_bool()?;
            let outcome_revision = if op["outcomeRevision"].is_null() {
                None
            } else {
                Some(revision(&op["outcomeRevision"])?)
            };
            let error = if op["error"].is_null() {
                None
            } else {
                let error = op["error"].as_object()?;
                if error.len() != 3
                    || !opaque(error.get("code")?.as_str()?, 64)
                    || error.get("message")?.as_str()?.len() > 1024
                    || !error.get("retryable")?.is_boolean()
                {
                    return None;
                }
                Some(safe_error(&op["error"]).unwrap_or("tui.action_rejected"))
            };
            if total > self.kind.maximum()
                || completed > total
                || op["progress"].as_object()?.len() != 2
                || state.terminal() != outcome_revision.is_some()
                || state.terminal() && cancellable
                || (state == State::Failed) != error.is_some()
                || state == State::Cancelled && !cancel_requested
                || state == State::Queued && completed != 0
                || state == State::Succeeded && completed != total
                || outcome_revision.is_some_and(|r| r < self.revision || r > envelope_revision)
                || state == State::Succeeded
                    && outcome_revision
                        != self
                            .revision
                            .checked_add(u64::from(total != 0 && self.kind == Kind::RefreshAll))
            {
                return None;
            }
            Some(Progress {
                state,
                completed,
                total,
                cancel_requested,
                cancellable,
                error,
            })
        })();
        parsed.map(Update::Progress).unwrap_or(Update::Unknown)
    }

    /// IDs stay in private UI memory for matching to current snapshot rows.
    /// Unknown fields, duplicate IDs and impossible timing facts fail closed.
    pub fn results(&self, response: Result<Value, ReadError>) -> Option<Vec<ProbeRow>> {
        if self.kind == Kind::RefreshAll {
            return None;
        }
        let value = response.ok()?;
        let result = &value["result"];
        if value["ok"] != true
            || revision(&value["revision"]) != Some(self.revision)
            || result.as_object()?.len() != 3
            || !result
                .as_object()?
                .contains_key(if self.kind == Kind::ProfileProbe {
                    "profileId"
                } else {
                    "subscriptionId"
                })
            || result["version"] != 1
            || match self.kind {
                Kind::ProfileProbe => result["profileId"] != json!(self.target),
                _ => result["subscriptionId"].as_str() != self.target.as_deref(),
            }
        {
            return None;
        }
        let rows = result["results"].as_array()?;
        if rows.len() > 256 {
            return None;
        }
        if self.kind == Kind::ProfileProbe
            && self.target.is_some()
            && (rows.len() != 1 || rows[0]["id"].as_str() != self.target.as_deref())
        {
            return None;
        }
        let mut seen = HashSet::new();
        rows.iter()
            .map(|row| {
                let id = row["id"].as_str()?;
                let resolved = row["resolved"].as_bool()?;
                let reachable = row["reachable"].as_bool()?;
                let millis = row["latencyMs"].as_i64()?;
                if row.as_object()?.len() != 4
                    || !record(id)
                    || !seen.insert(id)
                    || reachable && (!resolved || !(0..=60000).contains(&millis))
                    || !reachable && millis != -1
                {
                    return None;
                }
                Some(ProbeRow {
                    id: id.to_owned(),
                    resolved,
                    latency_ms: reachable.then_some(millis as u32),
                })
            })
            .collect()
    }
}

fn safe_error(value: &Value) -> Option<&'static str> {
    match value["code"].as_str()? {
        "conflict" | "daemon_restarting" => Some("tui.action_changed"),
        "busy" => Some("tui.action_busy"),
        "manual_recovery_required" => Some("tui.action_recovery"),
        "permission_denied" => Some("tui.action_denied"),
        "invalid_argument"
        | "not_found"
        | "capability_unavailable"
        | "core_rejected"
        | "deadline_exceeded"
        | "internal_error" => Some("tui.action_rejected"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
impl State {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub state: State,
    pub completed: usize,
    pub total: usize,
    pub cancel_requested: bool,
    pub cancellable: bool,
    pub error: Option<&'static str>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    Progress(Progress),
    Rejected(&'static str),
    Unknown,
}
#[derive(Clone)]
pub struct ProbeRow {
    pub id: String,
    pub resolved: bool,
    pub latency_ms: Option<u32>,
}

/// One watch per explicit user start. Exhaustion/lost replies never imply a
/// cancelled job and never create a new operation; an explicit poll can resume.
pub struct Tracker {
    started: Instant,
    last_poll: Option<Instant>,
    polls: usize,
    pub progress: Option<Progress>,
    pub unknown: bool,
    cancel_sent: bool,
}
impl Tracker {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            last_poll: None,
            polls: 0,
            progress: None,
            unknown: false,
            cancel_sent: false,
        }
    }
    pub fn poll_due(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.started) >= WATCH_BUDGET || self.polls >= MAX_POLLS {
            if !self.progress.is_some_and(|p| p.state.terminal()) {
                self.unknown = true;
            }
            return false;
        }
        if self.progress.is_some_and(|p| p.state.terminal())
            || self
                .last_poll
                .is_some_and(|last| now.saturating_duration_since(last) < POLL_INTERVAL)
        {
            return false;
        }
        self.polls += 1;
        self.last_poll = Some(now);
        true
    }
    /// A deliberate receipt read can outlive the automatic watch budget, but
    /// never creates a fresh operation or bypasses the one-second rate limit.
    pub fn manual_poll_due(&mut self, now: Instant) -> bool {
        if self.progress.is_some_and(|p| p.state.terminal())
            || self
                .last_poll
                .is_some_and(|last| now.saturating_duration_since(last) < POLL_INTERVAL)
        {
            return false;
        }
        self.last_poll = Some(now);
        true
    }
    /// Explicit cancellation is sent at most once. A cancelled receipt, not a
    /// sent request or accepted flag, is the proof that the operation stopped.
    pub fn request_cancel(&mut self) -> bool {
        if self.cancel_sent
            || !self
                .progress
                .is_some_and(|p| p.cancellable && !p.state.terminal())
        {
            return false;
        }
        self.cancel_sent = true;
        true
    }
    pub fn accept(&mut self, update: Update) -> bool {
        let Update::Progress(next) = update else {
            self.unknown = true;
            return false;
        };
        if self.progress.is_some_and(|old| {
            next.total != old.total
                || next.completed < old.completed
                || old.cancel_requested && !next.cancel_requested
                || old.state.terminal() && next != old
                || old.state == State::Running && next.state == State::Queued
        }) {
            self.unknown = true;
            return false;
        }
        self.progress = Some(next);
        self.unknown = false;
        true
    }
}
