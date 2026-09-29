// SPDX-License-Identifier: MIT

//! Private, inactive journal for one automatic subscription-refresh attempt.
//! A durable start must precede any future provider request. A start left by
//! another daemon instance is uncertain and never becomes an automatic retry.

use crate::cutover::CutoverPaths;
use crate::subscription_schedule_plan::{
    AttemptHistory, OwnerFence, RefreshSchedule, ScheduleDecision, plan_refresh,
};
use crate::subscription_schedule_preference::{
    PreferenceError, locked_owner, read_locked as read_preference_locked,
};
use omavless_store::{atomic_replace_private, read_private_utf8};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

const FILE_NAME: &str = "subscription-refresh-attempt.json";
const SCHEMA_VERSION: u8 = 1;
const MAX_BYTES: u64 = 768;
const MAX_INSTANCE_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptError {
    Busy,
    OwnershipUnavailable,
    UnsafeState,
    InvalidState,
    InvalidInstance,
    ScheduleOff,
    PreferenceChanged,
    StaleOwner,
    WaitUntil(u64),
    AttemptInProgress,
    AttemptUncertain,
    StaleTicket,
    ClockRegressed,
    CounterExhausted,
    WriteUncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptState {
    StartedInCurrentInstance,
    UncertainFromPreviousInstance,
    Succeeded,
    Failed,
}

/// The private instance string is deliberately absent from this read projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptSnapshot {
    pub sequence: u64,
    pub owner_generation: u64,
    pub preference_revision: u64,
    pub started_at_secs: u64,
    pub finished_at_secs: Option<u64>,
    pub consecutive_failures: u32,
    pub state: AttemptState,
}

/// In-memory completion authority. Never serialize this value or send it over
/// IPC; a new daemon instance cannot finish an interrupted prior attempt.
pub struct AttemptTicket {
    sequence: u64,
    owner_generation: u64,
    preference_revision: u64,
    owner_revision: u64,
    instance_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum WireState {
    Started,
    Succeeded,
    Failed,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireAttempt {
    schema_version: u8,
    owner_generation: u64,
    preference_revision: u64,
    sequence: u64,
    owner_revision: u64,
    completed_owner_revision: Option<u64>,
    instance_id: String,
    started_at_secs: u64,
    finished_at_secs: Option<u64>,
    consecutive_failures: u32,
    state: WireState,
}

impl WireAttempt {
    fn validate(&self) -> Result<(), AttemptError> {
        if self.schema_version != SCHEMA_VERSION
            || self.owner_generation == 0
            || self.preference_revision == 0
            || self.sequence == 0
            || u64::from(self.consecutive_failures) > self.sequence
            || !valid_instance(&self.instance_id)
        {
            return Err(AttemptError::InvalidState);
        }
        match self.state {
            WireState::Started
                if self.finished_at_secs.is_none() && self.completed_owner_revision.is_none() => {}
            WireState::Succeeded
                if self
                    .finished_at_secs
                    .is_some_and(|finished| finished >= self.started_at_secs)
                    && self
                        .completed_owner_revision
                        .is_some_and(|revision| revision >= self.owner_revision)
                    && self.consecutive_failures == 0 => {}
            WireState::Failed
                if self
                    .finished_at_secs
                    .is_some_and(|finished| finished >= self.started_at_secs)
                    && self
                        .completed_owner_revision
                        .is_some_and(|revision| revision == self.owner_revision)
                    && self.consecutive_failures > 0 => {}
            _ => return Err(AttemptError::InvalidState),
        }
        Ok(())
    }

    fn snapshot(&self, current_instance: &str) -> AttemptSnapshot {
        let state = match self.state {
            WireState::Started if self.instance_id == current_instance => {
                AttemptState::StartedInCurrentInstance
            }
            WireState::Started => AttemptState::UncertainFromPreviousInstance,
            WireState::Succeeded => AttemptState::Succeeded,
            WireState::Failed => AttemptState::Failed,
        };
        AttemptSnapshot {
            sequence: self.sequence,
            owner_generation: self.owner_generation,
            preference_revision: self.preference_revision,
            started_at_secs: self.started_at_secs,
            finished_at_secs: self.finished_at_secs,
            consecutive_failures: self.consecutive_failures,
            state,
        }
    }
}

fn valid_instance(instance: &str) -> bool {
    !instance.is_empty()
        && instance.len() <= MAX_INSTANCE_BYTES
        && instance
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn preference_error(error: PreferenceError) -> AttemptError {
    match error {
        PreferenceError::Busy => AttemptError::Busy,
        PreferenceError::OwnershipUnavailable => AttemptError::OwnershipUnavailable,
        PreferenceError::UnsafeState => AttemptError::UnsafeState,
        PreferenceError::InvalidState
        | PreferenceError::IntervalOutOfRange
        | PreferenceError::RevisionExhausted
        | PreferenceError::RevisionConflict => AttemptError::InvalidState,
        PreferenceError::WriteUncertain => AttemptError::WriteUncertain,
    }
}

/// Caller must hold the shared migration lock.
fn read_attempt_locked(
    paths: &CutoverPaths,
    uid: u32,
) -> Result<Option<WireAttempt>, AttemptError> {
    let path = paths.state_directory.join(FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(AttemptError::UnsafeState),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o7777 != 0o600
    {
        return Err(AttemptError::UnsafeState);
    }
    if metadata.len() > MAX_BYTES {
        return Err(AttemptError::InvalidState);
    }
    let raw = read_private_utf8(&path, uid).map_err(|_| AttemptError::UnsafeState)?;
    if raw.len() as u64 > MAX_BYTES {
        return Err(AttemptError::InvalidState);
    }
    let wire: WireAttempt = serde_json::from_str(&raw).map_err(|_| AttemptError::InvalidState)?;
    wire.validate()?;
    Ok(Some(wire))
}

fn write_attempt_locked(
    paths: &CutoverPaths,
    uid: u32,
    next: &WireAttempt,
) -> Result<(), AttemptError> {
    let mut payload = serde_json::to_vec(next).map_err(|_| AttemptError::InvalidState)?;
    payload.push(b'\n');
    if payload.len() as u64 > MAX_BYTES {
        return Err(AttemptError::InvalidState);
    }
    let path = paths.state_directory.join(FILE_NAME);
    // Any error may follow publication but precede durable sync. The caller
    // must stop; it cannot assume the previous bytes are still authoritative.
    atomic_replace_private(&path, &payload, uid).map_err(|_| AttemptError::WriteUncertain)?;
    let actual = read_attempt_locked(paths, uid)
        .map_err(|_| AttemptError::WriteUncertain)?
        .ok_or(AttemptError::WriteUncertain)?;
    if actual != *next {
        return Err(AttemptError::WriteUncertain);
    }
    Ok(())
}

/// Read a bounded same-user summary. An unfinished attempt from an older
/// daemon instance is uncertain; no automatic retry is permitted.
pub fn read_attempt(
    paths: &CutoverPaths,
    uid: u32,
    expected_generation: u64,
    current_instance: &str,
) -> Result<Option<AttemptSnapshot>, AttemptError> {
    if !valid_instance(current_instance) {
        return Err(AttemptError::InvalidInstance);
    }
    let _lock = locked_owner(paths, uid, expected_generation).map_err(preference_error)?;
    let Some(attempt) = read_attempt_locked(paths, uid)? else {
        return Ok(None);
    };
    if attempt.owner_generation != expected_generation {
        return Err(AttemptError::OwnershipUnavailable);
    }
    Ok(Some(attempt.snapshot(current_instance)))
}

/// Persist the attempt before a future worker performs network I/O. This
/// inactive API still requires its future caller to hold the serialized owner
/// while obtaining and rechecking the supplied owner revision.
pub fn begin_attempt(
    paths: &CutoverPaths,
    uid: u32,
    current_instance: &str,
    expected_preference_revision: u64,
    now_secs: u64,
    owner: OwnerFence,
) -> Result<AttemptTicket, AttemptError> {
    if !valid_instance(current_instance) {
        return Err(AttemptError::InvalidInstance);
    }
    if !owner.matches() {
        return Err(AttemptError::StaleOwner);
    }
    let _lock = locked_owner(paths, uid, owner.expected_generation).map_err(preference_error)?;
    let preference =
        read_preference_locked(paths, uid, owner.expected_generation).map_err(preference_error)?;
    if preference.owner_generation != owner.expected_generation {
        return Err(AttemptError::OwnershipUnavailable);
    }
    if preference.revision != expected_preference_revision {
        return Err(AttemptError::PreferenceChanged);
    }
    if preference.schedule == RefreshSchedule::Off {
        return Err(AttemptError::ScheduleOff);
    }
    let previous = read_attempt_locked(paths, uid)?;
    if previous
        .as_ref()
        .is_some_and(|attempt| attempt.owner_generation != owner.expected_generation)
    {
        return Err(AttemptError::OwnershipUnavailable);
    }
    let history = match previous.as_ref() {
        Some(attempt) if attempt.state == WireState::Started => {
            return Err(if attempt.instance_id == current_instance {
                AttemptError::AttemptInProgress
            } else {
                AttemptError::AttemptUncertain
            });
        }
        Some(attempt) => Some(AttemptHistory {
            last_attempt_at_secs: attempt.finished_at_secs.ok_or(AttemptError::InvalidState)?,
            consecutive_failures: attempt.consecutive_failures,
        }),
        None => None,
    };
    match plan_refresh(preference.schedule, history, now_secs, owner)
        .map_err(|_| AttemptError::InvalidState)?
    {
        ScheduleDecision::Due => {}
        ScheduleDecision::WaitUntil(due) => return Err(AttemptError::WaitUntil(due)),
        ScheduleDecision::Off => return Err(AttemptError::ScheduleOff),
        ScheduleDecision::StaleOwner => return Err(AttemptError::StaleOwner),
    }
    let sequence = previous
        .as_ref()
        .map_or(Some(1), |attempt| attempt.sequence.checked_add(1))
        .ok_or(AttemptError::CounterExhausted)?;
    let failures = previous
        .as_ref()
        .map_or(0, |attempt| attempt.consecutive_failures);
    let next = WireAttempt {
        schema_version: SCHEMA_VERSION,
        owner_generation: owner.expected_generation,
        preference_revision: preference.revision,
        sequence,
        owner_revision: owner.current_revision,
        completed_owner_revision: None,
        instance_id: current_instance.to_owned(),
        started_at_secs: now_secs,
        finished_at_secs: None,
        consecutive_failures: failures,
        state: WireState::Started,
    };
    write_attempt_locked(paths, uid, &next)?;
    Ok(AttemptTicket {
        sequence,
        owner_generation: next.owner_generation,
        preference_revision: next.preference_revision,
        owner_revision: next.owner_revision,
        instance_id: next.instance_id,
    })
}

/// Terminalize only the exact live attempt. A success means the future owner
/// already proved the corresponding refresh commit; this helper cannot prove
/// that by itself and has no production caller.
pub fn finish_attempt(
    paths: &CutoverPaths,
    uid: u32,
    ticket: AttemptTicket,
    outcome: AttemptOutcome,
    now_secs: u64,
    owner: OwnerFence,
) -> Result<AttemptSnapshot, AttemptError> {
    if !owner.matches()
        || owner.expected_generation != ticket.owner_generation
        || owner.current_revision < ticket.owner_revision
        || (outcome == AttemptOutcome::Failure && owner.current_revision != ticket.owner_revision)
    {
        return Err(AttemptError::StaleOwner);
    }
    let _lock = locked_owner(paths, uid, owner.expected_generation).map_err(preference_error)?;
    let preference =
        read_preference_locked(paths, uid, owner.expected_generation).map_err(preference_error)?;
    if preference.owner_generation != ticket.owner_generation
        || preference.revision != ticket.preference_revision
        || preference.schedule == RefreshSchedule::Off
    {
        return Err(AttemptError::PreferenceChanged);
    }
    let mut attempt = read_attempt_locked(paths, uid)?.ok_or(AttemptError::StaleTicket)?;
    if attempt.state != WireState::Started
        || attempt.sequence != ticket.sequence
        || attempt.owner_generation != ticket.owner_generation
        || attempt.preference_revision != ticket.preference_revision
        || attempt.owner_revision != ticket.owner_revision
        || attempt.instance_id != ticket.instance_id
    {
        return Err(AttemptError::StaleTicket);
    }
    if now_secs < attempt.started_at_secs {
        return Err(AttemptError::ClockRegressed);
    }
    attempt.state = match outcome {
        AttemptOutcome::Success => {
            attempt.consecutive_failures = 0;
            WireState::Succeeded
        }
        AttemptOutcome::Failure => {
            attempt.consecutive_failures = attempt
                .consecutive_failures
                .checked_add(1)
                .ok_or(AttemptError::CounterExhausted)?;
            WireState::Failed
        }
    };
    attempt.finished_at_secs = Some(now_secs);
    attempt.completed_owner_revision = Some(owner.current_revision);
    write_attempt_locked(paths, uid, &attempt)?;
    Ok(attempt.snapshot(&ticket.instance_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::{CutoverPaths, read_marker};
    use crate::subscription_schedule_preference::{read_preference, set_preference};
    use nix::unistd::Uid;
    use std::os::unix::fs::DirBuilderExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    const GENERATION: u64 = 2;
    const INTERVAL: u64 = 6 * 60 * 60;
    const OWNER: OwnerFence = OwnerFence {
        expected_generation: GENERATION,
        current_generation: GENERATION,
        expected_revision: 7,
        current_revision: 7,
    };

    struct Fixture {
        base: PathBuf,
        paths: CutoverPaths,
        uid: u32,
    }

    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "omavless-t4-attempt-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
            let runtime = base.join("run");
            let state = base.join("state");
            fs::DirBuilder::new().mode(0o700).create(&runtime).unwrap();
            fs::DirBuilder::new().mode(0o700).create(&state).unwrap();
            let uid = Uid::current().as_raw();
            let paths = CutoverPaths::below(&runtime, &state, uid);
            read_marker(&paths, uid).unwrap();
            atomic_replace_private(
                &paths.ownership_marker,
                b"{\"schemaVersion\":1,\"generation\":2,\"phase\":\"rust\"}\n",
                uid,
            )
            .unwrap();
            Self { base, paths, uid }
        }

        fn path(&self) -> PathBuf {
            self.paths.state_directory.join(FILE_NAME)
        }

        fn enable(&self) -> u64 {
            set_preference(
                &self.paths,
                self.uid,
                GENERATION,
                0,
                RefreshSchedule::Every {
                    interval_secs: INTERVAL,
                },
            )
            .unwrap()
            .revision
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn off_is_inert_and_success_waits_full_interval() {
        let fixture = Fixture::new();
        assert_eq!(
            begin_attempt(&fixture.paths, fixture.uid, "daemon-1", 0, 100, OWNER).err(),
            Some(AttemptError::ScheduleOff)
        );
        assert!(!fixture.path().exists());
        let revision = fixture.enable();
        let ticket = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-1",
            revision,
            100,
            OWNER,
        )
        .unwrap();
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-1")
                .unwrap()
                .unwrap()
                .state,
            AttemptState::StartedInCurrentInstance
        );
        assert_eq!(
            fs::symlink_metadata(fixture.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-1",
                revision,
                100,
                OWNER
            )
            .err(),
            Some(AttemptError::AttemptInProgress)
        );
        let result = finish_attempt(
            &fixture.paths,
            fixture.uid,
            ticket,
            AttemptOutcome::Success,
            110,
            OWNER,
        )
        .unwrap();
        assert_eq!(result.state, AttemptState::Succeeded);
        assert_eq!(result.sequence, 1);
        assert_eq!(result.consecutive_failures, 0);
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-1",
                revision,
                110,
                OWNER
            )
            .err(),
            Some(AttemptError::WaitUntil(110 + INTERVAL))
        );
        let next = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-1",
            revision,
            110 + INTERVAL,
            OWNER,
        )
        .unwrap();
        assert_eq!(next.sequence, 2);
    }

    #[test]
    fn failure_backoff_persists_and_restart_does_not_discard_it() {
        let fixture = Fixture::new();
        let revision = fixture.enable();
        let first = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-1",
            revision,
            100,
            OWNER,
        )
        .unwrap();
        let failed = finish_attempt(
            &fixture.paths,
            fixture.uid,
            first,
            AttemptOutcome::Failure,
            120,
            OWNER,
        )
        .unwrap();
        assert_eq!(failed.consecutive_failures, 1);
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-2",
                revision,
                419,
                OWNER
            )
            .err(),
            Some(AttemptError::WaitUntil(420))
        );
        let second = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-2",
            revision,
            420,
            OWNER,
        )
        .unwrap();
        assert_eq!(
            finish_attempt(
                &fixture.paths,
                fixture.uid,
                second,
                AttemptOutcome::Failure,
                440,
                OWNER,
            )
            .unwrap()
            .consecutive_failures,
            2
        );
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-3",
                revision,
                1039,
                OWNER
            )
            .err(),
            Some(AttemptError::WaitUntil(1040))
        );
    }

    #[test]
    fn unfinished_prior_instance_is_uncertain_and_never_retried() {
        let fixture = Fixture::new();
        let revision = fixture.enable();
        let ticket = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-1",
            revision,
            100,
            OWNER,
        )
        .unwrap();
        let interrupted = read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-2")
            .unwrap()
            .unwrap();
        assert_eq!(
            interrupted.state,
            AttemptState::UncertainFromPreviousInstance
        );
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-2",
                revision,
                u64::MAX,
                OWNER
            )
            .err(),
            Some(AttemptError::AttemptUncertain)
        );
        assert_eq!(
            finish_attempt(
                &fixture.paths,
                fixture.uid,
                ticket,
                AttemptOutcome::Success,
                99,
                OWNER,
            )
            .err(),
            Some(AttemptError::ClockRegressed)
        );
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-2")
                .unwrap()
                .unwrap()
                .state,
            AttemptState::UncertainFromPreviousInstance
        );
    }

    #[test]
    fn preference_and_owner_fences_block_completion_without_rewriting_history() {
        let fixture = Fixture::new();
        let revision = fixture.enable();
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-1",
                revision,
                100,
                OwnerFence {
                    current_revision: 8,
                    ..OWNER
                },
            )
            .err(),
            Some(AttemptError::StaleOwner)
        );
        let ticket = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-1",
            revision,
            100,
            OWNER,
        )
        .unwrap();
        let before = fs::read(fixture.path()).unwrap();
        set_preference(
            &fixture.paths,
            fixture.uid,
            GENERATION,
            revision,
            RefreshSchedule::Off,
        )
        .unwrap();
        assert_eq!(
            finish_attempt(
                &fixture.paths,
                fixture.uid,
                ticket,
                AttemptOutcome::Success,
                110,
                OWNER,
            )
            .err(),
            Some(AttemptError::PreferenceChanged)
        );
        assert_eq!(fs::read(fixture.path()).unwrap(), before);
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION)
                .unwrap()
                .schedule,
            RefreshSchedule::Off
        );
    }

    #[test]
    fn malformed_and_unsafe_journal_is_never_treated_as_absent() {
        let fixture = Fixture::new();
        fixture.enable();
        let path = fixture.path();
        for payload in [
            b"{}".as_slice(),
            b"{\"schemaVersion\":1,\"schemaVersion\":1}".as_slice(),
            b"{\"schemaVersion\":1,\"url\":\"synthetic\"}".as_slice(),
        ] {
            atomic_replace_private(&path, payload, fixture.uid).unwrap();
            assert_eq!(
                read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-1"),
                Err(AttemptError::InvalidState)
            );
        }
        atomic_replace_private(&path, &vec![b' '; MAX_BYTES as usize + 1], fixture.uid).unwrap();
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-1"),
            Err(AttemptError::InvalidState)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-1"),
            Err(AttemptError::UnsafeState)
        );
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&fixture.paths.ownership_marker, &path).unwrap();
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-1"),
            Err(AttemptError::UnsafeState)
        );
    }

    #[test]
    fn stale_generation_and_exhausted_sequence_never_reset_history() {
        let fixture = Fixture::new();
        let revision = fixture.enable();
        let terminal = WireAttempt {
            schema_version: SCHEMA_VERSION,
            owner_generation: GENERATION,
            preference_revision: revision,
            sequence: u64::MAX,
            owner_revision: OWNER.current_revision,
            completed_owner_revision: Some(OWNER.current_revision),
            instance_id: "daemon-1".into(),
            started_at_secs: 100,
            finished_at_secs: Some(110),
            consecutive_failures: 0,
            state: WireState::Succeeded,
        };
        let mut payload = serde_json::to_vec(&terminal).unwrap();
        payload.push(b'\n');
        atomic_replace_private(&fixture.path(), &payload, fixture.uid).unwrap();
        assert_eq!(
            begin_attempt(
                &fixture.paths,
                fixture.uid,
                "daemon-2",
                revision,
                110 + INTERVAL,
                OWNER,
            )
            .err(),
            Some(AttemptError::CounterExhausted)
        );
        assert_eq!(fs::read(fixture.path()).unwrap(), payload);
        atomic_replace_private(
            &fixture.paths.ownership_marker,
            b"{\"schemaVersion\":1,\"generation\":3,\"phase\":\"rust\"}\n",
            fixture.uid,
        )
        .unwrap();
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, 3, "daemon-2"),
            Err(AttemptError::OwnershipUnavailable)
        );
        assert_eq!(fs::read(fixture.path()).unwrap(), payload);
    }

    #[test]
    fn invalid_instance_and_failure_counter_exhaustion_refuse_network_admission() {
        let fixture = Fixture::new();
        let revision = fixture.enable();
        let too_long = "a".repeat(129);
        for invalid in ["", "x/y", "x\n", too_long.as_str()] {
            assert_eq!(
                begin_attempt(&fixture.paths, fixture.uid, invalid, revision, 100, OWNER).err(),
                Some(AttemptError::InvalidInstance)
            );
        }
        assert!(!fixture.path().exists());
        let terminal = WireAttempt {
            schema_version: SCHEMA_VERSION,
            owner_generation: GENERATION,
            preference_revision: revision,
            sequence: u64::from(u32::MAX),
            owner_revision: OWNER.current_revision,
            completed_owner_revision: Some(OWNER.current_revision),
            instance_id: "daemon-1".into(),
            started_at_secs: 100,
            finished_at_secs: Some(110),
            consecutive_failures: u32::MAX,
            state: WireState::Failed,
        };
        let mut payload = serde_json::to_vec(&terminal).unwrap();
        payload.push(b'\n');
        atomic_replace_private(&fixture.path(), &payload, fixture.uid).unwrap();
        let ticket = begin_attempt(
            &fixture.paths,
            fixture.uid,
            "daemon-2",
            revision,
            110 + 24 * 60 * 60,
            OWNER,
        )
        .unwrap();
        assert_eq!(
            finish_attempt(
                &fixture.paths,
                fixture.uid,
                ticket,
                AttemptOutcome::Failure,
                110 + 24 * 60 * 60,
                OWNER,
            )
            .err(),
            Some(AttemptError::CounterExhausted)
        );
        assert_eq!(
            read_attempt(&fixture.paths, fixture.uid, GENERATION, "daemon-3")
                .unwrap()
                .unwrap()
                .state,
            AttemptState::UncertainFromPreviousInstance
        );
    }
}
