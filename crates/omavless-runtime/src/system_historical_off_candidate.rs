// SPDX-License-Identifier: MIT
//! Inactive System-path bridge, deliberately absent from all normal dispatch.
//! One shared startup review, with the original proof and lease retained until
//! the actual owner is destroyed. No reusable admission authority is returned.

use super::*;
use crate::restore_executor_candidate::successor::rotation::final_review::disposition::recovery::completion::historical::{RetainedCurrentOff, RetainedEpochOff};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Review {
    ReviewedOffStillFenced,
}

/// No path, source, host, proof or caller Boolean input. Installed package and
/// same-manager consumed-receipt admission cannot be replaced by a test ELF.
#[allow(dead_code)]
fn current() -> Result<Review, ProductionOwnerError> {
    let uid = Uid::current().as_raw();
    let runtime = RuntimePaths::current().map_err(|_| ProductionOwnerError::HostUnavailable)?;
    let desired = DesiredPaths::current().map_err(|_| ProductionOwnerError::HostUnavailable)?;
    let paths = CutoverPaths::current(uid).map_err(|_| ProductionOwnerError::HostUnavailable)?;
    let lock = MigrationLock::acquire_existing(&paths, uid).map_err(lock_error)?;
    let marker = read_marker_existing(&paths, uid)
        .map_err(|_| ProductionOwnerError::OwnershipUnavailable)?;
    if marker.phase() != OwnershipPhase::Rust {
        return Err(ProductionOwnerError::OwnershipUnavailable);
    }
    let host_paths = NativeHostPaths::current(&runtime.directory)
        .map_err(|_| ProductionOwnerError::HostUnavailable)?;
    let store = host_paths.store.clone();
    let config = host_paths.config_directory.clone();
    // Capture must precede every external package/manager query.
    let original = RetainedCurrentOff::capture(&config, &paths, uid, marker.generation(), &lock)
        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
    let mut host = crate::native_host::ObservationOnlyNativeHost::new(host_paths, uid)
        .map_err(|_| ProductionOwnerError::HostUnavailable)?;
    // No probe orphan cleanup or normal login consume occurs here.
    let evidence = original
        .system_off(&mut host)
        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
    run_under_lease(host, desired, &store, paths.clone(), uid, (&lock, evidence))
}

/// Internal shared body, not a general normal-admission API. Tests can supply
/// a deterministic host, but the only non-test caller above supplies the fixed
/// observation-only native host and genuine System witness. The owner never escapes.
fn run_under_lease<H: LifecycleHost>(
    mut host: H,
    desired: DesiredPaths,
    store: &Path,
    paths: CutoverPaths,
    uid: u32,
    retained: (&MigrationLock, RetainedEpochOff<'_>),
) -> Result<Review, ProductionOwnerError> {
    let (lock, mut evidence) = retained;
    if !evidence.paths_match(&desired, store) {
        return Err(ProductionOwnerError::ManualRecoveryRequired);
    }
    evidence
        .bind(&paths, uid, lock)
        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
    observe_empty(&mut host, &desired, uid, &mut evidence)?;
    let mut owner = ProductionNativeOwner::initialize_under_lease(
        host,
        desired.clone(),
        store,
        paths,
        uid,
        (
            lock,
            &mut crate::startup_admission::StartupAdmission::HistoricalOff(&mut evidence),
        ),
    )?;
    owner.ownership = ProductionOwnership::Stale;
    let result = (|| {
        if owner.actual() != ActualState::Disconnected
            || owner.startup_outcome().changed
            || owner.login_ready()
            || owner.coordinator.revision() != 0
        {
            return Err(ProductionOwnerError::ManualRecoveryRequired);
        }
        observe_empty(owner.coordinator.host_mut(), &desired, uid, &mut evidence)
    })();
    // Drop the real owner even on final refusal, before the witness or lease.
    drop(owner);
    result?;
    evidence
        .recheck()
        .map_err(|_| ProductionOwnerError::ManualRecoveryRequired)?;
    Ok(Review::ReviewedOffStillFenced)
}

#[cfg(test)]
pub(crate) fn review_for_test<H: LifecycleHost>(
    host: H,
    desired: DesiredPaths,
    store: &Path,
    paths: CutoverPaths,
    uid: u32,
    retained: (&MigrationLock, RetainedEpochOff<'_>),
) -> Result<Review, ProductionOwnerError> {
    run_under_lease(host, desired, store, paths, uid, retained)
}

fn observe_empty<H: LifecycleHost>(
    host: &mut H,
    paths: &DesiredPaths,
    uid: u32,
    evidence: &mut RetainedEpochOff<'_>,
) -> Result<(), ProductionOwnerError> {
    let refuse = ProductionOwnerError::ManualRecoveryRequired;
    evidence.recheck().map_err(|_| refuse)?;
    let desired = crate::desired::read_desired_snapshot(paths, uid).map_err(|_| refuse)?;
    if desired.connected
        || !host.fresh_observation(&desired).is_ok_and(|o| {
            !o.owned_core_running
                && o.visible_mihomo_count == 0
                && o.owned_auxiliary_mihomo_count == 0
                && o.visible_tun_count == 0
                && o.managed_tun_count == 0
                && !o.owned_controller_config_verified
                && !o.desired_profile_matches_owned
        })
    {
        return Err(refuse);
    }
    evidence.recheck().map_err(|_| refuse)
}
