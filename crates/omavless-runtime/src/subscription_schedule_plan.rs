// SPDX-License-Identifier: MIT

//! Pure T4 subscription refresh timing and admission. No runtime caller is
//! registered: deciding `Due` never starts a provider request by itself.

/// Bounds for an explicitly enabled schedule. Values are whole seconds.
pub const MIN_INTERVAL_SECS: u64 = 6 * 60 * 60;
pub const MAX_INTERVAL_SECS: u64 = 7 * 24 * 60 * 60;
pub const INITIAL_RETRY_SECS: u64 = 5 * 60;
pub const MAX_RETRY_SECS: u64 = 24 * 60 * 60;
const MAX_BACKOFF_SHIFTS: u32 = 9;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RefreshSchedule {
    #[default]
    Off,
    Every {
        interval_secs: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    IntervalOutOfRange,
    InvalidAttemptHistory,
}

impl RefreshSchedule {
    /// Validate before a future private-state writer persists this preference.
    pub fn validate(self) -> Result<Self, ScheduleError> {
        if let Self::Every { interval_secs } = self
            && !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&interval_secs)
        {
            return Err(ScheduleError::IntervalOutOfRange);
        }
        Ok(self)
    }
}

/// Values must be read from the same committed owner snapshot. The scheduler
/// must recheck them under the owner lock immediately before actual admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerFence {
    pub expected_generation: u64,
    pub current_generation: u64,
    pub expected_revision: u64,
    pub current_revision: u64,
}

impl OwnerFence {
    #[must_use]
    pub fn matches(self) -> bool {
        self.expected_generation != 0
            && self.expected_generation == self.current_generation
            && self.expected_revision == self.current_revision
    }
}

/// One persisted attempt summary; an absent summary means no automatic fetch
/// has yet been attempted. A failed attempt increases the capped retry delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptHistory {
    pub last_attempt_at_secs: u64,
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleDecision {
    Off,
    StaleOwner,
    WaitUntil(u64),
    Due,
}

/// Plan with caller-supplied time. Clock rollback waits until the recorded due
/// time; saturating arithmetic cannot wrap into an immediate retry.
pub fn plan_refresh(
    schedule: RefreshSchedule,
    history: Option<AttemptHistory>,
    now_secs: u64,
    owner: OwnerFence,
) -> Result<ScheduleDecision, ScheduleError> {
    let schedule = schedule.validate()?;
    if schedule == RefreshSchedule::Off {
        return Ok(ScheduleDecision::Off);
    }
    if !owner.matches() {
        return Ok(ScheduleDecision::StaleOwner);
    }
    let RefreshSchedule::Every { interval_secs } = schedule else {
        unreachable!("Off returned above")
    };
    let Some(history) = history else {
        return Ok(ScheduleDecision::Due);
    };
    // A zero failure count records success; a nonzero count records failure.
    // The first retry is five minutes and doubles up to the 24-hour cap.
    let delay = if history.consecutive_failures == 0 {
        interval_secs
    } else {
        INITIAL_RETRY_SECS
            .saturating_mul(1u64 << (history.consecutive_failures - 1).min(MAX_BACKOFF_SHIFTS))
            .min(MAX_RETRY_SECS)
    };
    let due = history
        .last_attempt_at_secs
        .checked_add(delay)
        .ok_or(ScheduleError::InvalidAttemptHistory)?;
    if now_secs >= due {
        Ok(ScheduleDecision::Due)
    } else {
        Ok(ScheduleDecision::WaitUntil(due))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: OwnerFence = OwnerFence {
        expected_generation: 2,
        current_generation: 2,
        expected_revision: 17,
        current_revision: 17,
    };
    const SCHEDULE: RefreshSchedule = RefreshSchedule::Every {
        interval_secs: MIN_INTERVAL_SECS,
    };

    #[test]
    fn default_is_off_and_invalid_intervals_are_rejected() {
        assert_eq!(RefreshSchedule::default(), RefreshSchedule::Off);
        assert_eq!(
            plan_refresh(RefreshSchedule::default(), None, 0, OWNER),
            Ok(ScheduleDecision::Off)
        );
        for invalid in [0, MIN_INTERVAL_SECS - 1, MAX_INTERVAL_SECS + 1, u64::MAX] {
            assert_eq!(
                RefreshSchedule::Every {
                    interval_secs: invalid
                }
                .validate(),
                Err(ScheduleError::IntervalOutOfRange)
            );
        }
        for valid in [MIN_INTERVAL_SECS, MAX_INTERVAL_SECS] {
            assert!(
                RefreshSchedule::Every {
                    interval_secs: valid
                }
                .validate()
                .is_ok()
            );
        }
    }

    #[test]
    fn ownership_and_revision_must_match_even_on_first_attempt() {
        assert_eq!(
            plan_refresh(SCHEDULE, None, 0, OWNER),
            Ok(ScheduleDecision::Due)
        );
        for stale in [
            OwnerFence {
                current_generation: 3,
                ..OWNER
            },
            OwnerFence {
                current_revision: 18,
                ..OWNER
            },
            OwnerFence {
                expected_generation: 0,
                current_generation: 0,
                ..OWNER
            },
        ] {
            assert_eq!(
                plan_refresh(SCHEDULE, None, 0, stale),
                Ok(ScheduleDecision::StaleOwner)
            );
        }
    }

    #[test]
    fn success_waits_full_interval_and_clock_rollback_does_not_fetch() {
        let history = Some(AttemptHistory {
            last_attempt_at_secs: 100_000,
            consecutive_failures: 0,
        });
        let due = 100_000 + MIN_INTERVAL_SECS;
        for now in [0, 99_999, due - 1] {
            assert_eq!(
                plan_refresh(SCHEDULE, history, now, OWNER),
                Ok(ScheduleDecision::WaitUntil(due))
            );
        }
        assert_eq!(
            plan_refresh(SCHEDULE, history, due, OWNER),
            Ok(ScheduleDecision::Due)
        );
    }

    #[test]
    fn failures_back_off_with_a_fixed_cap() {
        let expected = [
            (1, 300),
            (2, 600),
            (3, 1200),
            (8, 38_400),
            (9, 76_800),
            (10, 86_400),
            (u32::MAX, 86_400),
        ];
        for (failures, delay) in expected {
            let history = Some(AttemptHistory {
                last_attempt_at_secs: 100,
                consecutive_failures: failures,
            });
            assert_eq!(
                plan_refresh(SCHEDULE, history, 100, OWNER),
                Ok(ScheduleDecision::WaitUntil(100 + delay))
            );
            assert_eq!(
                plan_refresh(SCHEDULE, history, 100 + delay, OWNER),
                Ok(ScheduleDecision::Due)
            );
        }
    }

    #[test]
    fn timestamp_overflow_fails_closed() {
        let history = Some(AttemptHistory {
            last_attempt_at_secs: u64::MAX,
            consecutive_failures: 1,
        });
        assert_eq!(
            plan_refresh(SCHEDULE, history, u64::MAX, OWNER),
            Err(ScheduleError::InvalidAttemptHistory)
        );
    }
}
