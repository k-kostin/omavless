// SPDX-License-Identifier: MIT

//! Test-only composition with the actual private atomic writer. All paths and
//! inputs below are synthetic. This does not authorize runtime persistence.

use super::*;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::private_store_transaction::{
    PreparedPrivateStoreWrite, PreparedWrite, PrivateStoreWriteError, prepare_private_store_write,
};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

struct Refresh {
    snapshot: Snapshot,
    fetched: FetchedSubscription,
    retention: Retention,
}

#[derive(Debug, PartialEq, Eq)]
enum PrepareError {
    Write(PrivateStoreWriteError),
    Composition(Error),
}

/// Keeps the ORIGINAL latest bytes, including old usage and whitespace, as the
/// compare/rollback baseline. Clearing usage is confined to the candidate.
fn prepare_locked(
    path: &Path,
    uid: u32,
    paths: &CutoverPaths,
    lock: &MigrationLock,
    refresh: Refresh,
) -> Result<PreparedPrivateStoreWrite, PrepareError> {
    if !lock.authorizes(paths, uid) {
        return Err(PrepareError::Write(PrivateStoreWriteError::LockMismatch));
    }
    let mut composition_error = None;
    let result = prepare_private_store_write(path, uid, |latest| {
        match compose(
            latest,
            refresh.snapshot,
            refresh.fetched,
            30,
            200,
            refresh.retention,
        ) {
            Ok(candidate) => Ok((candidate.payload.payload().to_vec(), true)),
            Err(error) => {
                composition_error = Some(error);
                Err(PrivateStoreError::InvalidShape)
            }
        }
    });
    if let Some(error) = composition_error {
        Err(PrepareError::Composition(error))
    } else {
        result.map_err(PrepareError::Write)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Disposition {
    Committed,
    Restored,
    RecoveryRequired,
}

/// Models the store half of compensation only. The test callback supplies an
/// outcome after publication; it is NOT an actual lifecycle verification.
/// Consuming the prepared write prevents an accidental retry of this attempt.
fn finish(
    prepared: PreparedPrivateStoreWrite,
    paths: &CutoverPaths,
    lock: &MigrationLock,
    accept: impl FnOnce() -> bool,
) -> Disposition {
    if prepared.commit_locked(lock, paths).is_ok()
        && accept()
        && prepared.verify_outcome_locked(lock, paths, false).is_ok()
    {
        return Disposition::Committed;
    }
    if prepared.restore_locked(lock, paths).is_ok()
        && prepared.verify_outcome_locked(lock, paths, true).is_ok()
    {
        Disposition::Restored
    } else {
        // Never overwrite unrecognized bytes or call an uncertain state clean.
        // Production would have to retain its existing owner recovery barrier.
        Disposition::RecoveryRequired
    }
}

struct Fixture {
    root: PathBuf,
    path: PathBuf,
    paths: CutoverPaths,
    uid: u32,
    original: String,
}

impl Fixture {
    fn new() -> Self {
        let root = crate::test_temp::directory("quota-transaction").unwrap();
        let config = root.join("config");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for path in [&root, &config, &runtime, &state] {
            fs::create_dir_all(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let uid = fs::metadata(&root).unwrap().uid();
        let path = config.join("profiles.json");
        let original = store();
        let prior = compose(
            &original,
            Snapshot::capture(&original, ID).unwrap(),
            fetched(true),
            20,
            100,
            Retention::ModelPrivatePersistence,
        )
        .unwrap();
        // Noncanonical formatting catches restoration from normalized/cleaned
        // bytes instead of the exact original read by the established writer.
        let original = format!("\n  {}\n", text(&prior));
        fs::write(&path, &original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            root,
            path,
            paths: CutoverPaths::below(&runtime, &state, uid),
            uid,
            original,
        }
    }

    fn refresh(&self, with_usage: bool) -> Refresh {
        let mut fetched = fetched(with_usage);
        fetched.body = omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
            URI.replace("#Synthetic", "#Updated").into_bytes(),
        )
        .unwrap();
        Refresh {
            snapshot: Snapshot::capture(&self.original, ID).unwrap(),
            fetched,
            retention: Retention::ModelPrivatePersistence,
        }
    }

    fn prepare(
        &self,
        lock: &MigrationLock,
        refresh: Refresh,
    ) -> Result<PreparedPrivateStoreWrite, PrepareError> {
        prepare_locked(&self.path, self.uid, &self.paths, lock, refresh)
    }

    fn read(&self) -> String {
        fs::read_to_string(&self.path).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn real_writer_publishes_one_feed_and_usage_then_restores_exact_original() {
    let f = Fixture::new();
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    let prepared = f.prepare(&lock, f.refresh(true)).unwrap();
    assert!(f.read() == f.original);
    assert_eq!(
        prepared.commit_locked(&lock, &f.paths),
        Ok(PreparedWrite::Changed)
    );
    let published = f.read();
    let claim = read_provider_usage(&published, ID).unwrap().unwrap();
    assert_eq!(claim.observed_at_unix_seconds(), 200);
    let value: serde_json::Value = serde_json::from_str(&published).unwrap();
    assert_eq!(value["subscriptions"][0]["updatedAt"], 30);
    assert_eq!(value["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(value["profiles"][0]["name"], "Updated");
    assert_eq!(
        fs::metadata(&f.path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        prepared.commit_locked(&lock, &f.paths),
        Err(PrivateStoreWriteError::StoreChanged)
    );
    assert_eq!(
        prepared.restore_locked(&lock, &f.paths),
        Ok(PreparedWrite::Changed)
    );
    assert!(f.read() == f.original);
    assert_eq!(
        prepared.restore_locked(&lock, &f.paths),
        Ok(PreparedWrite::NoChange)
    );
}

#[test]
fn missing_usage_and_discard_commit_clear_claim_but_failure_restores_it() {
    for discard in [false, true] {
        for accept in [false, true] {
            let f = Fixture::new();
            let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
            let mut refresh = f.refresh(discard);
            if discard {
                refresh.retention = Retention::Discard;
            }
            let prepared = f.prepare(&lock, refresh).unwrap();
            let result = finish(prepared, &f.paths, &lock, || {
                assert!(!f.read().contains("providerUsageV1"));
                accept
            });
            if accept {
                assert_eq!(result, Disposition::Committed);
                assert!(!f.read().contains("providerUsageV1"));
            } else {
                assert_eq!(result, Disposition::Restored);
                assert!(f.read() == f.original);
                assert!(read_provider_usage(&f.read(), ID).unwrap().is_some());
            }
        }
    }
}

#[test]
fn intervening_bytes_are_never_overwritten_by_commit_or_compensation() {
    for after_publication in [false, true] {
        let f = Fixture::new();
        let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
        let prepared = f.prepare(&lock, f.refresh(true)).unwrap();
        let foreign = format!("{}\n", f.original);
        if !after_publication {
            fs::write(&f.path, &foreign).unwrap();
        }
        let mut callback_called = false;
        let result = finish(prepared, &f.paths, &lock, || {
            callback_called = true;
            fs::write(&f.path, &foreign).unwrap();
            false
        });
        assert_eq!(callback_called, after_publication);
        assert_eq!(result, Disposition::RecoveryRequired);
        assert!(f.read() == foreign);
    }
}

#[test]
fn final_readback_catches_changed_bytes_even_when_callback_accepts() {
    let f = Fixture::new();
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    let prepared = f.prepare(&lock, f.refresh(true)).unwrap();
    let result = finish(prepared, &f.paths, &lock, || {
        fs::write(&f.path, "{}").unwrap();
        true
    });
    assert_eq!(result, Disposition::RecoveryRequired);
    assert_eq!(f.read(), "{}");
}

#[test]
fn unsafe_publication_or_restore_never_claims_success() {
    for after_publication in [false, true] {
        let f = Fixture::new();
        let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
        let prepared = f.prepare(&lock, f.refresh(true)).unwrap();
        if !after_publication {
            fs::set_permissions(&f.path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let result = finish(prepared, &f.paths, &lock, || {
            fs::set_permissions(&f.path, fs::Permissions::from_mode(0o644)).unwrap();
            false
        });
        assert_eq!(result, Disposition::RecoveryRequired);
        assert_eq!(f.read() == f.original, !after_publication);
    }
}

#[test]
fn wrong_lock_feed_failure_and_stale_snapshot_never_prepare_a_write() {
    let f = Fixture::new();
    let other = Fixture::new();
    let wrong = MigrationLock::acquire(&other.paths, other.uid).unwrap();
    assert!(matches!(
        f.prepare(&wrong, f.refresh(true)),
        Err(PrepareError::Write(PrivateStoreWriteError::LockMismatch))
    ));
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    let mut invalid = f.refresh(true);
    invalid.fetched.body = omavless_domain::subscription_feed::PrivateSubscriptionBody::from_bytes(
        b"invalid-feed".to_vec(),
    )
    .unwrap();
    assert!(matches!(
        f.prepare(&lock, invalid),
        Err(PrepareError::Composition(Error::Feed(_)))
    ));
    assert!(f.read() == f.original);
    let mut changed: serde_json::Value = serde_json::from_str(&f.original).unwrap();
    changed["subscriptions"][0]["updatedAt"] = 21.into();
    let latest = changed.to_string();
    fs::write(&f.path, &latest).unwrap();
    assert!(matches!(
        f.prepare(&lock, f.refresh(true)),
        Err(PrepareError::Composition(Error::Store(
            PrivateStoreError::SubscriptionChanged
        )))
    ));
    assert!(f.read() == latest);
}

#[test]
fn latest_unrelated_changes_survive_publication_and_exact_compensation() {
    let f = Fixture::new();
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    let mut latest: serde_json::Value = serde_json::from_str(&f.original).unwrap();
    latest["subscriptions"][0]["name"] = "Concurrent rename".into();
    latest["extension"]["other"] = true.into();
    let latest = format!("\n{}\n", latest);
    fs::write(&f.path, &latest).unwrap();
    let prepared = f.prepare(&lock, f.refresh(true)).unwrap();
    let result = finish(prepared, &f.paths, &lock, || {
        let value: serde_json::Value = serde_json::from_str(&f.read()).unwrap();
        assert_eq!(value["subscriptions"][0]["name"], "Concurrent rename");
        assert_eq!(value["extension"]["other"], true);
        false
    });
    assert_eq!(result, Disposition::Restored);
    assert!(f.read() == latest);
}
