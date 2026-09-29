// SPDX-License-Identifier: MIT

//! Durable, opt-in T4 refresh preference. This is an inactive owner-locked
//! boundary: it neither registers a timer nor starts provider work.

use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker};
use crate::subscription_schedule_plan::{RefreshSchedule, ScheduleError};
use omavless_store::{atomic_replace_private, read_private_utf8};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

const FILE_NAME: &str = "subscription-refresh-preference.json";
const SCHEMA_VERSION: u8 = 1;
const MAX_BYTES: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceError {
    Busy,
    OwnershipUnavailable,
    UnsafeState,
    InvalidState,
    RevisionConflict,
    RevisionExhausted,
    IntervalOutOfRange,
    WriteUncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferenceSnapshot {
    pub schedule: RefreshSchedule,
    pub revision: u64,
    pub owner_generation: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WirePreference {
    schema_version: u8,
    owner_generation: u64,
    revision: u64,
    interval_secs: u64,
}

impl WirePreference {
    fn snapshot(&self) -> Result<PreferenceSnapshot, PreferenceError> {
        if self.schema_version != SCHEMA_VERSION || self.owner_generation == 0 || self.revision == 0
        {
            return Err(PreferenceError::InvalidState);
        }
        let schedule = if self.interval_secs == 0 {
            RefreshSchedule::Off
        } else {
            RefreshSchedule::Every {
                interval_secs: self.interval_secs,
            }
            .validate()
            .map_err(|_| PreferenceError::InvalidState)?
        };
        Ok(PreferenceSnapshot {
            schedule,
            revision: self.revision,
            owner_generation: self.owner_generation,
        })
    }
}

pub(crate) fn locked_owner(
    paths: &CutoverPaths,
    uid: u32,
    expected_generation: u64,
) -> Result<MigrationLock, PreferenceError> {
    let lock = MigrationLock::acquire(paths, uid).map_err(|error| match error {
        crate::cutover::CutoverError::Busy => PreferenceError::Busy,
        _ => PreferenceError::UnsafeState,
    })?;
    let marker = read_marker(paths, uid).map_err(|_| PreferenceError::UnsafeState)?;
    if expected_generation == 0
        || marker.phase() != OwnershipPhase::Rust
        || marker.generation() != expected_generation
    {
        return Err(PreferenceError::OwnershipUnavailable);
    }
    Ok(lock)
}

/// Caller must hold the migration lock returned by `locked_owner`.
pub(crate) fn read_locked(
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
) -> Result<PreferenceSnapshot, PreferenceError> {
    let path = paths.state_directory.join(FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(PreferenceSnapshot {
                schedule: RefreshSchedule::Off,
                revision: 0,
                owner_generation: generation,
            });
        }
        Err(_) => return Err(PreferenceError::UnsafeState),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o7777 != 0o600
    {
        return Err(PreferenceError::UnsafeState);
    }
    if metadata.len() > MAX_BYTES {
        return Err(PreferenceError::InvalidState);
    }
    let raw = read_private_utf8(&path, uid).map_err(|_| PreferenceError::UnsafeState)?;
    if raw.len() as u64 > MAX_BYTES {
        return Err(PreferenceError::InvalidState);
    }
    let wire: WirePreference =
        serde_json::from_str(&raw).map_err(|_| PreferenceError::InvalidState)?;
    wire.snapshot()
}

/// Read only after proving the exact committed Rust generation under the
/// shared migration lock. A missing file is Off; malformed state is an error.
pub fn read_preference(
    paths: &CutoverPaths,
    uid: u32,
    expected_generation: u64,
) -> Result<PreferenceSnapshot, PreferenceError> {
    let _lock = locked_owner(paths, uid, expected_generation)?;
    let snapshot = read_locked(paths, uid, expected_generation)?;
    if snapshot.owner_generation != expected_generation {
        return Err(PreferenceError::OwnershipUnavailable);
    }
    Ok(snapshot)
}

/// Compare and atomically replace one private preference. No CLI/IPC caller is
/// registered. The expected revision rejects concurrent or stale UI choices.
pub fn set_preference(
    paths: &CutoverPaths,
    uid: u32,
    expected_generation: u64,
    expected_revision: u64,
    schedule: RefreshSchedule,
) -> Result<PreferenceSnapshot, PreferenceError> {
    schedule.validate().map_err(|error| match error {
        ScheduleError::IntervalOutOfRange => PreferenceError::IntervalOutOfRange,
        ScheduleError::InvalidAttemptHistory => PreferenceError::InvalidState,
    })?;
    let _lock = locked_owner(paths, uid, expected_generation)?;
    let current = read_locked(paths, uid, expected_generation)?;
    let stale_generation = current.owner_generation != expected_generation;
    // A previous native generation can only be rebound by an explicit Off
    // choice. A later explicit enablement must use the new revision returned
    // by that reset; stale enabled schedules never resume automatically.
    if stale_generation && (schedule != RefreshSchedule::Off || expected_revision != 0) {
        return Err(PreferenceError::OwnershipUnavailable);
    }
    if !stale_generation && current.revision != expected_revision {
        return Err(PreferenceError::RevisionConflict);
    }
    if !stale_generation && current.schedule == schedule {
        return Ok(current);
    }
    let revision = current
        .revision
        .checked_add(1)
        .ok_or(PreferenceError::RevisionExhausted)?;
    let interval_secs = match schedule {
        RefreshSchedule::Off => 0,
        RefreshSchedule::Every { interval_secs } => interval_secs,
    };
    let wire = WirePreference {
        schema_version: SCHEMA_VERSION,
        owner_generation: expected_generation,
        revision,
        interval_secs,
    };
    let mut payload = serde_json::to_vec(&wire).map_err(|_| PreferenceError::InvalidState)?;
    payload.push(b'\n');
    if payload.len() as u64 > MAX_BYTES {
        return Err(PreferenceError::InvalidState);
    }
    let path = paths.state_directory.join(FILE_NAME);
    // A post-rename sync failure may have published the new bytes without
    // proving durability. Do not claim a confirmed preference or invite a
    // blind retry with the old revision after any write failure.
    if atomic_replace_private(&path, &payload, uid).is_err() {
        return Err(PreferenceError::WriteUncertain);
    }
    let actual = read_locked(paths, uid, expected_generation)?;
    if actual.owner_generation != expected_generation
        || actual.revision != revision
        || actual.schedule != schedule
    {
        return Err(PreferenceError::WriteUncertain);
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::Uid;
    use std::os::unix::fs::DirBuilderExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    const GENERATION: u64 = 2;
    const INTERVAL: RefreshSchedule = RefreshSchedule::Every {
        interval_secs: 6 * 60 * 60,
    };

    struct Fixture {
        base: PathBuf,
        paths: CutoverPaths,
        uid: u32,
    }

    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "omavless-t4-preference-{}-{}",
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
            omavless_store::atomic_replace_private(
                &paths.ownership_marker,
                b"{\"schemaVersion\":1,\"generation\":2,\"phase\":\"rust\"}\n",
                uid,
            )
            .unwrap();
            Self { base, paths, uid }
        }

        fn preference_path(&self) -> PathBuf {
            self.paths.state_directory.join(FILE_NAME)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn missing_is_off_then_explicit_enable_persists_and_revision_fences_updates() {
        let fixture = Fixture::new();
        let before = read_preference(&fixture.paths, fixture.uid, GENERATION).unwrap();
        assert_eq!(before.schedule, RefreshSchedule::Off);
        assert_eq!(before.revision, 0);
        assert!(!fixture.preference_path().exists());

        let enabled = set_preference(&fixture.paths, fixture.uid, GENERATION, 0, INTERVAL).unwrap();
        assert_eq!(enabled.revision, 1);
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION).unwrap(),
            enabled
        );
        assert_eq!(
            fs::symlink_metadata(fixture.preference_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        assert_eq!(
            set_preference(
                &fixture.paths,
                fixture.uid,
                GENERATION,
                0,
                RefreshSchedule::Off
            ),
            Err(PreferenceError::RevisionConflict)
        );
        assert_eq!(
            set_preference(&fixture.paths, fixture.uid, GENERATION, 1, INTERVAL).unwrap(),
            enabled
        );
        let disabled = set_preference(
            &fixture.paths,
            fixture.uid,
            GENERATION,
            1,
            RefreshSchedule::Off,
        )
        .unwrap();
        assert_eq!(disabled.revision, 2);
        assert_eq!(disabled.schedule, RefreshSchedule::Off);
    }

    #[test]
    fn owner_generation_or_phase_change_refuses_stored_preference() {
        let fixture = Fixture::new();
        let enabled = set_preference(&fixture.paths, fixture.uid, GENERATION, 0, INTERVAL).unwrap();
        let bytes = fs::read(fixture.preference_path()).unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION + 1),
            Err(PreferenceError::OwnershipUnavailable)
        );
        omavless_store::atomic_replace_private(
            &fixture.paths.ownership_marker,
            b"{\"schemaVersion\":1,\"generation\":3,\"phase\":\"rollbackPreparing\"}\n",
            fixture.uid,
        )
        .unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION),
            Err(PreferenceError::OwnershipUnavailable)
        );
        assert_eq!(fs::read(fixture.preference_path()).unwrap(), bytes);
        assert_eq!(enabled.revision, 1);
    }

    #[test]
    fn old_generation_needs_explicit_off_rebind_before_enablement() {
        let fixture = Fixture::new();
        omavless_store::atomic_replace_private(
            &fixture.preference_path(),
            b"{\"schemaVersion\":1,\"ownerGeneration\":1,\"revision\":7,\"intervalSecs\":21600}",
            fixture.uid,
        )
        .unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION),
            Err(PreferenceError::OwnershipUnavailable)
        );
        assert_eq!(
            set_preference(&fixture.paths, fixture.uid, GENERATION, 0, INTERVAL),
            Err(PreferenceError::OwnershipUnavailable)
        );
        let rebound = set_preference(
            &fixture.paths,
            fixture.uid,
            GENERATION,
            0,
            RefreshSchedule::Off,
        )
        .unwrap();
        assert_eq!(rebound.revision, 8);
        assert_eq!(rebound.schedule, RefreshSchedule::Off);
        assert_eq!(rebound.owner_generation, GENERATION);
        assert_eq!(
            set_preference(&fixture.paths, fixture.uid, GENERATION, 8, INTERVAL)
                .unwrap()
                .revision,
            9
        );
    }

    #[test]
    fn malformed_duplicate_oversized_and_unsafe_files_fail_closed() {
        let fixture = Fixture::new();
        let path = fixture.preference_path();
        let invalid = [
            b"{\"schemaVersion\":1,\"ownerGeneration\":2,\"revision\":1,\"intervalSecs\":1}".as_slice(),
            b"{\"schemaVersion\":1,\"ownerGeneration\":2,\"revision\":1,\"intervalSecs\":0,\"intervalSecs\":21600}".as_slice(),
            b"{\"schemaVersion\":1,\"ownerGeneration\":2,\"revision\":1,\"intervalSecs\":0,\"url\":\"synthetic\"}".as_slice(),
        ];
        for payload in invalid {
            omavless_store::atomic_replace_private(&path, payload, fixture.uid).unwrap();
            assert_eq!(
                read_preference(&fixture.paths, fixture.uid, GENERATION),
                Err(PreferenceError::InvalidState)
            );
        }
        omavless_store::atomic_replace_private(
            &path,
            b"{\"schemaVersion\":1,\"ownerGeneration\":1,\"revision\":1,\"intervalSecs\":21600}",
            fixture.uid,
        )
        .unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION),
            Err(PreferenceError::OwnershipUnavailable)
        );
        omavless_store::atomic_replace_private(
            &path,
            &vec![b' '; MAX_BYTES as usize + 1],
            fixture.uid,
        )
        .unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION),
            Err(PreferenceError::InvalidState)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION),
            Err(PreferenceError::UnsafeState)
        );
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&fixture.paths.ownership_marker, &path).unwrap();
        assert_eq!(
            read_preference(&fixture.paths, fixture.uid, GENERATION),
            Err(PreferenceError::UnsafeState)
        );
    }

    #[test]
    fn invalid_interval_and_held_owner_lock_never_publish() {
        let fixture = Fixture::new();
        assert_eq!(
            set_preference(
                &fixture.paths,
                fixture.uid,
                GENERATION,
                0,
                RefreshSchedule::Every { interval_secs: 1 },
            ),
            Err(PreferenceError::IntervalOutOfRange)
        );
        let lock = MigrationLock::acquire(&fixture.paths, fixture.uid).unwrap();
        assert_eq!(
            set_preference(&fixture.paths, fixture.uid, GENERATION, 0, INTERVAL),
            Err(PreferenceError::Busy)
        );
        drop(lock);
        assert!(!fixture.preference_path().exists());
    }

    #[test]
    fn revision_exhaustion_refuses_replacement() {
        let fixture = Fixture::new();
        let path = fixture.preference_path();
        omavless_store::atomic_replace_private(
            &path,
            b"{\"schemaVersion\":1,\"ownerGeneration\":2,\"revision\":18446744073709551615,\"intervalSecs\":0}",
            fixture.uid,
        )
        .unwrap();
        let before = fs::read(&path).unwrap();
        assert_eq!(
            set_preference(&fixture.paths, fixture.uid, GENERATION, u64::MAX, INTERVAL),
            Err(PreferenceError::RevisionExhausted)
        );
        assert_eq!(fs::read(path).unwrap(), before);
    }
}
