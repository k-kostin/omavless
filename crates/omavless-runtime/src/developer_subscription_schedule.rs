// SPDX-License-Identifier: MIT

//! Explicit dormant registration only. Trusted wakeup/time are injected by a
//! developer constructor, never read from IPC. Normal startup registers none.

use crate::native_coordinator::{AutomaticRefreshError, NativeOwnerError};
use crate::subscription_schedule_attempt::{AttemptError, AttemptState};
use crate::subscription_schedule_plan::RefreshSchedule;
use crate::subscription_schedule_preference::PreferenceError;
use omavless_control_protocol::StableErrorCode;
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) const METHODS: &[&str] = &[
    "developer.subscription_schedule.get",
    "developer.subscription_schedule.set",
    "developer.subscription_schedule.acknowledge",
];
pub(crate) type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

pub struct DeveloperSubscriptionSchedule {
    pub(crate) clock: Clock,
    wakeup: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl DeveloperSubscriptionSchedule {
    pub fn new<C, W>(clock: C, wakeup: W) -> Self
    where
        C: Fn() -> u64 + Send + Sync + 'static,
        W: Fn() -> bool + Send + Sync + 'static,
    {
        Self {
            clock: Arc::new(clock),
            wakeup: Arc::new(wakeup),
        }
    }
    pub(crate) fn take_wakeup(&self) -> bool {
        (self.wakeup)()
    }
}

pub(crate) fn parse(
    request: &Value,
    instance: &str,
) -> Result<Option<(u64, RefreshSchedule)>, StableErrorCode> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| StableErrorCode::InvalidArgument)?;
    let params = request["params"]
        .as_object()
        .ok_or(StableErrorCode::InvalidArgument)?;
    if params.get("instanceId").and_then(Value::as_str) != Some(instance) {
        return Err(StableErrorCode::Conflict);
    }
    match request["method"].as_str() {
        Some("developer.subscription_schedule.get") if params.len() == 1 => Ok(None),
        Some("developer.subscription_schedule.set") if params.len() == 3 => {
            let revision = params
                .get("expectedPreferenceRevision")
                .and_then(Value::as_u64)
                .ok_or(StableErrorCode::InvalidArgument)?;
            let interval = params
                .get("intervalSecs")
                .and_then(Value::as_u64)
                .ok_or(StableErrorCode::InvalidArgument)?;
            let schedule = if interval == 0 {
                RefreshSchedule::Off
            } else {
                RefreshSchedule::Every {
                    interval_secs: interval,
                }
            };
            schedule
                .validate()
                .map_err(|_| StableErrorCode::InvalidArgument)?;
            Ok(Some((revision, schedule)))
        }
        _ => Err(StableErrorCode::InvalidArgument),
    }
}

pub(crate) fn code(error: AutomaticRefreshError) -> StableErrorCode {
    match error {
        AutomaticRefreshError::Owner(error) => error.stable_code(),
        AutomaticRefreshError::Preference(PreferenceError::Busy)
        | AutomaticRefreshError::Attempt(AttemptError::Busy) => StableErrorCode::Busy,
        AutomaticRefreshError::Preference(PreferenceError::RevisionConflict) => {
            StableErrorCode::Conflict
        }
        AutomaticRefreshError::Preference(PreferenceError::OwnershipUnavailable)
        | AutomaticRefreshError::Attempt(AttemptError::OwnershipUnavailable) => {
            StableErrorCode::CapabilityUnavailable
        }
        AutomaticRefreshError::Preference(PreferenceError::IntervalOutOfRange) => {
            StableErrorCode::InvalidArgument
        }
        _ => StableErrorCode::ManualRecoveryRequired,
    }
}

pub(crate) fn parse_acknowledgement(
    request: &Value,
    instance: &str,
) -> Result<(u64, u64, u64), StableErrorCode> {
    omavless_control_protocol::validate_request(request)
        .map_err(|_| StableErrorCode::InvalidArgument)?;
    let params = request["params"]
        .as_object()
        .ok_or(StableErrorCode::InvalidArgument)?;
    if request["method"] != "developer.subscription_schedule.acknowledge" || params.len() != 4 {
        return Err(StableErrorCode::InvalidArgument);
    }
    if params.get("instanceId").and_then(Value::as_str) != Some(instance) {
        return Err(StableErrorCode::Conflict);
    }
    let sequence = params
        .get("attemptSequence")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or(StableErrorCode::InvalidArgument)?;
    let preference = params
        .get("expectedPreferenceRevision")
        .and_then(Value::as_u64)
        .ok_or(StableErrorCode::InvalidArgument)?;
    let revision = params
        .get("expectedRevision")
        .and_then(Value::as_u64)
        .ok_or(StableErrorCode::InvalidArgument)?;
    Ok((sequence, preference, revision))
}

pub(crate) fn projection(
    preference: crate::subscription_schedule_preference::PreferenceSnapshot,
    attempt: Option<crate::subscription_schedule_attempt::AttemptSnapshot>,
    registered: bool,
) -> Value {
    let interval = match preference.schedule {
        RefreshSchedule::Off => 0,
        RefreshSchedule::Every { interval_secs } => interval_secs,
    };
    let attempt=attempt.map(|attempt|json!({
        "sequence":attempt.sequence, "startedAtSecs":attempt.started_at_secs,
        "finishedAtSecs":attempt.finished_at_secs,"consecutiveFailures":attempt.consecutive_failures,
        "state":match attempt.state {
            AttemptState::StartedInCurrentInstance=>"running", AttemptState::UncertainFromPreviousInstance=>"uncertain",
            AttemptState::Succeeded=>"succeeded",AttemptState::Empty=>"empty",AttemptState::Failed=>"failed",
            AttemptState::Cancelled=>"cancelled",AttemptState::Superseded=>"superseded",
            AttemptState::AcknowledgedUncertain=>"acknowledgedUncertain",
        }
    }));
    json!({"schemaVersion":1,"intervalSecs":interval,"preferenceRevision":preference.revision,"workerRegistered":registered,"attempt":attempt})
}

pub(crate) fn unavailable() -> AutomaticRefreshError {
    AutomaticRefreshError::Owner(NativeOwnerError::OwnershipUnavailable)
}
