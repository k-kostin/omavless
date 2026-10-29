// SPDX-License-Identifier: MIT
//! Machine-local recorded TUN deltas, separate from provider assertions.
//!
//! This algebra performs no I/O and grants no observation or lifecycle authority.
//! Its caller supplies only an already-attributed, incarnation-fenced sample.
use crate::traffic::TrafficCounters;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(crate) mod collection;
mod dispatch;
pub mod file;
pub(crate) mod protocol;
#[cfg(test)]
mod read_tests;
pub(crate) mod worker;

pub const MAX_BYTES: usize = 4096;
const MAX_EXACT: u64 = 9_007_199_254_740_991;
const MAX_UTC: u64 = 253_402_300_799;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidState,
    UnsupportedSchema,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Unchanged,
    Changed,
    StaleSample,
}

// Intentionally no Debug: even aggregate dates and byte counts are private.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Baseline {
    identity: String,
    rx: u64,
    tx: u64,
    sampled_at_ms: u64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct State {
    schema_version: u64,
    enabled: bool,
    period_started_at_utc: u64,
    rx_recorded: u64,
    tx_recorded: u64,
    partial: bool,
    overflowed: bool,
    control_revision: u64,
    checkpoint_sequence: u64,
    baseline: Option<Baseline>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: 1,
            enabled: false,
            period_started_at_utc: 0,
            rx_recorded: 0,
            tx_recorded: 0,
            partial: false,
            overflowed: false,
            control_revision: 0,
            checkpoint_sequence: 0,
            baseline: None,
        }
    }
}

impl State {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::InvalidState);
        }
        // Deserialize the typed shape FIRST: a Value-only parse loses duplicate
        // members. Also require the nullable baseline member to be explicit.
        let state: Self = serde_json::from_slice(bytes).map_err(|_| Error::InvalidState)?;
        let shape: Value = serde_json::from_slice(bytes).map_err(|_| Error::InvalidState)?;
        if !shape
            .as_object()
            .is_some_and(|fields| fields.len() == 10 && fields.contains_key("baseline"))
        {
            return Err(Error::InvalidState);
        }
        state.validate()?;
        Ok(state)
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::InvalidState)?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::InvalidState);
        }
        Ok(bytes)
    }

    fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 {
            return Err(Error::UnsupportedSchema);
        }
        if self.control_revision > MAX_EXACT
            || self.period_started_at_utc > MAX_UTC
            || (self.enabled && self.period_started_at_utc == 0)
            || (!self.enabled && self.baseline.is_some())
            || (self.overflowed && (!self.partial || self.baseline.is_some()))
            || (self.period_started_at_utc == 0
                && (self.rx_recorded != 0 || self.tx_recorded != 0 || self.partial))
            || self.baseline.as_ref().is_some_and(|b| !valid_baseline(b))
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    fn commit(&mut self, mut candidate: Self, control: bool) -> Result<Change, Error> {
        if candidate == *self {
            return Ok(Change::Unchanged);
        }
        candidate.checkpoint_sequence = self
            .checkpoint_sequence
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        if control {
            candidate.control_revision = self
                .control_revision
                .checked_add(1)
                .filter(|revision| *revision <= MAX_EXACT)
                .ok_or(Error::Exhausted)?;
        }
        candidate.validate()?;
        *self = candidate;
        Ok(Change::Changed)
    }

    /// Caller independently admits an explicit Off-only control operation.
    /// Re-enabling resumes the retained period; only reset erases its totals.
    pub fn configure(&mut self, enabled: bool, utc: u64) -> Result<Change, Error> {
        if self.enabled == enabled {
            return Ok(Change::Unchanged);
        }
        let mut next = self.clone();
        if enabled && next.period_started_at_utc == 0 {
            next.period_started_at_utc = valid_time(utc)?;
        } else if enabled || next.baseline.is_some() {
            next.partial = true;
        }
        next.enabled = enabled;
        next.baseline = None;
        self.commit(next, true)
    }

    /// Caller admits an explicit reset while settled Off; enabled is retained.
    pub fn reset(&mut self, utc: u64) -> Result<Change, Error> {
        let mut next = Self {
            enabled: self.enabled,
            period_started_at_utc: valid_time(utc)?,
            control_revision: self.control_revision,
            checkpoint_sequence: self.checkpoint_sequence,
            ..Self::default()
        };
        // Reset is a new control epoch even if the clock has not advanced.
        next.control_revision = self
            .control_revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_EXACT)
            .ok_or(Error::Exhausted)?;
        next.checkpoint_sequence = self
            .checkpoint_sequence
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        next.validate()?;
        *self = next;
        Ok(Change::Changed)
    }

    /// Never continue a serialized raw baseline into another runtime lifetime.
    pub fn new_runtime(&mut self) -> Result<Change, Error> {
        if !self.enabled {
            return Ok(Change::Unchanged);
        }
        self.unavailable()
    }

    /// Missing attribution, shutdown tails and missed epochs are not zero bytes.
    pub fn unavailable(&mut self) -> Result<Change, Error> {
        if !self.enabled {
            return Ok(Change::Unchanged);
        }
        let mut next = self.clone();
        next.partial = true;
        next.baseline = None;
        self.commit(next, false)
    }

    pub fn observe(&mut self, sample: &TrafficCounters) -> Result<Change, Error> {
        if !self.enabled || self.overflowed {
            return Ok(Change::Unchanged);
        }
        let observed = Baseline {
            identity: sample.identity.clone(),
            rx: sample.rx_bytes,
            tx: sample.tx_bytes,
            sampled_at_ms: sample.sampled_at_ms,
        };
        if !valid_baseline(&observed) {
            return self.unavailable();
        }
        let mut next = self.clone();
        if let Some(old) = &self.baseline {
            if observed.sampled_at_ms < old.sampled_at_ms
                || (observed.sampled_at_ms == old.sampled_at_ms && observed != *old)
            {
                // A late observation is not a counter reset. Moving backward
                // here would count the following interval twice.
                return Ok(Change::StaleSample);
            }
            if observed == *old {
                return Ok(Change::Unchanged);
            }
            if observed.identity == old.identity && observed.rx >= old.rx && observed.tx >= old.tx {
                match (
                    self.rx_recorded.checked_add(observed.rx - old.rx),
                    self.tx_recorded.checked_add(observed.tx - old.tx),
                ) {
                    (Some(rx), Some(tx)) => {
                        next.rx_recorded = rx;
                        next.tx_recorded = tx;
                    }
                    _ => {
                        next.partial = true;
                        next.overflowed = true;
                        next.baseline = None;
                        return self.commit(next, false);
                    }
                }
            } else {
                next.partial = true;
            }
        } else {
            // No proof covers the prefix before the first attributed sample.
            // Even a normal connection start is not a zero-origin receipt.
            next.partial = true;
        }
        next.baseline = Some(observed);
        self.commit(next, false)
    }

    /// Private client projection; raw identity/baseline and host paths stay out.
    pub fn projection(&self) -> Value {
        json!({
            "schemaVersion": 1,
            "scope": "private_recorded_tun_bytes",
            "enabled": self.enabled,
            "periodStartedAtUtc": self.period_started_at_utc,
            "receivedBytes": self.rx_recorded.to_string(),
            "sentBytes": self.tx_recorded.to_string(),
            "partial": self.partial,
            "availability": if self.overflowed { "overflow" } else { "recorded" },
            "controlRevision": self.control_revision,
        })
    }
}

fn valid_time(utc: u64) -> Result<u64, Error> {
    (utc != 0 && utc <= MAX_UTC)
        .then_some(utc)
        .ok_or(Error::InvalidState)
}

fn valid_baseline(value: &Baseline) -> bool {
    value.identity.len() == 64
        && value
            .identity
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        && value.rx <= MAX_EXACT
        && value.tx <= MAX_EXACT
        && value.sampled_at_ms <= MAX_EXACT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(epoch: char, rx: u64, tx: u64, at: u64) -> TrafficCounters {
        TrafficCounters {
            identity: epoch.to_string().repeat(64),
            rx_bytes: rx,
            tx_bytes: tx,
            sampled_at_ms: at,
        }
    }

    fn enabled() -> State {
        let mut state = State::default();
        assert_eq!(state.configure(true, 100), Ok(Change::Changed));
        state
    }

    #[test]
    fn first_sample_is_only_a_partial_baseline_and_ordered_deltas_are_recorded() {
        let mut state = enabled();
        state.observe(&sample('a', 5000, 7000, 1)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (0, 0));
        assert!(state.partial);
        state.observe(&sample('a', 5500, 7200, 2)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (500, 200));
        assert_eq!(state.control_revision, 1);
    }

    #[test]
    fn identical_replay_and_late_sample_cannot_double_count_or_move_baseline_backward() {
        let mut state = enabled();
        state.observe(&sample('a', 5000, 100, 1)).unwrap();
        state.observe(&sample('a', 5500, 200, 2)).unwrap();
        let committed = state.encode().unwrap();
        assert_eq!(
            state.observe(&sample('a', 5500, 200, 2)),
            Ok(Change::Unchanged)
        );
        assert_eq!(state.encode().unwrap(), committed);
        assert_eq!(
            state.observe(&sample('a', 5000, 100, 1)),
            Ok(Change::StaleSample)
        );
        assert_eq!(state.encode().unwrap(), committed);
        state.observe(&sample('a', 5600, 220, 3)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (600, 120));
    }

    #[test]
    fn one_counter_reset_or_epoch_change_establishes_baseline_without_backfill() {
        let mut state = enabled();
        state.observe(&sample('a', 5000, 100, 1)).unwrap();
        state.observe(&sample('a', 5500, 200, 2)).unwrap();
        state.observe(&sample('a', 100, 300, 3)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (500, 100));
        state.observe(&sample('b', 200, 400, 4)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (500, 100));
        state.observe(&sample('b', 220, 450, 5)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (520, 150));
    }

    #[test]
    fn unavailable_and_new_runtime_break_continuity_even_with_matching_identity() {
        let mut state = enabled();
        state.observe(&sample('a', 100, 200, 10)).unwrap();
        state.observe(&sample('a', 200, 300, 20)).unwrap();
        state.unavailable().unwrap();
        state.observe(&sample('a', 900, 900, 30)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (100, 100));
        let mut restarted = State::decode(&state.encode().unwrap()).unwrap();
        restarted.new_runtime().unwrap();
        restarted.observe(&sample('a', 1500, 1500, 0)).unwrap();
        assert_eq!((restarted.rx_recorded, restarted.tx_recorded), (100, 100));
        assert!(restarted.partial);
    }

    #[test]
    fn disabled_observations_are_noops_and_disable_retains_period_until_explicit_reset() {
        let mut state = State::default();
        let absent = state.encode().unwrap();
        state.observe(&sample('a', 100, 200, 1)).unwrap();
        state.new_runtime().unwrap();
        state.unavailable().unwrap();
        assert_eq!(state.encode().unwrap(), absent);
        state.configure(true, 100).unwrap();
        state.observe(&sample('a', 100, 100, 1)).unwrap();
        state.observe(&sample('a', 200, 300, 2)).unwrap();
        state.configure(false, 200).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (100, 200));
        let disabled = state.encode().unwrap();
        state.observe(&sample('a', 999, 999, 3)).unwrap();
        assert_eq!(state.encode().unwrap(), disabled);
        state.configure(true, 300).unwrap();
        assert_eq!(state.period_started_at_utc, 100);
        assert_eq!(state.rx_recorded, 100);
        state.reset(400).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (0, 0));
        assert_eq!(state.period_started_at_utc, 400);
        assert!(!state.partial);
        assert!(state.enabled);
    }

    #[test]
    fn both_atomic_disk_outcomes_resume_without_replaying_unknown_delta() {
        let mut old = enabled();
        old.observe(&sample('a', 5000, 0, 1)).unwrap();
        old.rx_recorded = 1000;
        let mut new = old.clone();
        new.observe(&sample('a', 5500, 0, 2)).unwrap();
        for (disk, expected) in [(old.encode().unwrap(), 1000), (new.encode().unwrap(), 1500)] {
            let mut restarted = State::decode(&disk).unwrap();
            restarted.new_runtime().unwrap();
            restarted.observe(&sample('a', 5500, 0, 0)).unwrap();
            assert_eq!(restarted.rx_recorded, expected);
            restarted.observe(&sample('a', 5600, 0, 1)).unwrap();
            assert_eq!(restarted.rx_recorded, expected + 100);
        }
        // Deliberately broken reference, not an existing production algorithm:
        // persisting total without baseline would count the same interval twice.
        assert_eq!(1500 + (5500 - 5000), 2000);
        assert_ne!(new.rx_recorded, 2000);
    }

    #[test]
    fn overflow_preserves_known_totals_and_never_wraps_or_partially_adds_other_direction() {
        let mut state = enabled();
        state.rx_recorded = u64::MAX - 1;
        state.observe(&sample('a', 10, 10, 1)).unwrap();
        state.observe(&sample('a', 20, 20, 2)).unwrap();
        assert_eq!((state.rx_recorded, state.tx_recorded), (u64::MAX - 1, 0));
        assert!(state.overflowed && state.partial && state.baseline.is_none());
        let bytes = state.encode().unwrap();
        state.observe(&sample('a', 30, 30, 3)).unwrap();
        assert_eq!(state.encode().unwrap(), bytes);
        assert_eq!(
            state.projection()["receivedBytes"],
            (u64::MAX - 1).to_string()
        );
    }

    #[test]
    fn projection_never_exposes_raw_epoch_baseline_device_path_or_loses_u64_precision() {
        let mut state = enabled();
        state.rx_recorded = u64::MAX;
        state.observe(&sample('a', 123, 456, 1)).unwrap();
        let projection = state.projection().to_string();
        assert!(projection.contains(&u64::MAX.to_string()));
        for secret in [
            "aaaaaaaa",
            "baseline",
            "sampledAt",
            "Meta",
            "/sys/",
            "123",
            "456",
        ] {
            assert!(!projection.contains(secret));
        }
    }

    #[test]
    fn strict_shape_rejects_unknown_duplicate_missing_wrong_types_and_bad_raw_values() {
        let original = enabled().encode().unwrap();
        let mut shape: Value = serde_json::from_slice(&original).unwrap();
        shape["schemaVersion"] = json!(2);
        assert_eq!(
            State::decode(&serde_json::to_vec(&shape).unwrap()).err(),
            Some(Error::UnsupportedSchema)
        );
        shape["schemaVersion"] = json!(1);
        for (key, invalid) in [
            ("enabled", json!(1)),
            ("rxRecorded", json!(-1)),
            ("controlRevision", json!(MAX_EXACT + 1)),
            ("periodStartedAtUtc", json!(0)),
            ("unknown", json!(true)),
        ] {
            let mut bad = shape.clone();
            bad[key] = invalid;
            assert!(State::decode(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let duplicate =
            String::from_utf8(original.clone())
                .unwrap()
                .replacen("{", "{\"enabled\":false,", 1);
        assert!(State::decode(duplicate.as_bytes()).is_err());
        shape.as_object_mut().unwrap().remove("baseline");
        assert!(State::decode(&serde_json::to_vec(&shape).unwrap()).is_err());
        assert!(State::decode(&vec![b' '; MAX_BYTES + 1]).is_err());
        let mut state = enabled();
        state.observe(&sample('x', 1, 1, 1)).unwrap();
        assert!(state.partial && state.baseline.is_none());
        state.observe(&sample('a', MAX_EXACT + 1, 1, 1)).unwrap();
        assert!(state.baseline.is_none());
    }

    #[test]
    fn control_revision_is_separate_from_checkpoints_and_exhaustion_keeps_exact_state() {
        let mut state = enabled();
        for i in 1..4 {
            state.observe(&sample('a', i * 100, i * 200, i)).unwrap();
        }
        assert_eq!(state.control_revision, 1);
        assert!(state.checkpoint_sequence > state.control_revision);
        state.checkpoint_sequence = u64::MAX;
        let before = state.encode().unwrap();
        assert_eq!(state.reset(200), Err(Error::Exhausted));
        assert_eq!(state.encode().unwrap(), before);
        state.checkpoint_sequence = 1;
        state.control_revision = MAX_EXACT;
        let before = state.encode().unwrap();
        assert_eq!(state.configure(false, 200), Err(Error::Exhausted));
        assert_eq!(state.encode().unwrap(), before);
    }
}
