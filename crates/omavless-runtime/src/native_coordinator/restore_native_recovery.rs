// SPDX-License-Identifier: MIT
//! New authenticated recovery origin, never ordinary startup or former authority.
use super::*;
use crate::cutover::{CutoverPaths, OwnershipMarker, OwnershipPhase, read_marker_existing};
use crate::desired::{DesiredPaths, DesiredState, read_desired_snapshot};
use crate::native_host::{NativeHostPaths, ObservationOnlyNativeHost};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;
static RECOVERY_RESERVED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
#[test]
fn native_fresh_recovery_reserves_one_original_and_retains_after_handle_loss() {
    let recovery = FreshRecovery::reserve().unwrap();
    assert!(!recovery.attempted);
    let original = Arc::downgrade(&recovery.original);
    assert!(FreshRecovery::reserve().is_err());
    drop(recovery);
    let retained = original.upgrade().unwrap();
    let held = retained.lock().unwrap();
    assert!(held.lock.is_none() && held.authenticated.is_none() && held.host.is_none());
    assert!(
        held.boundary
            .as_ref()
            .is_some_and(|boundary| boundary.directories.capacity() >= 3
                && boundary.members.capacity() >= 3
                && boundary.live.capacity() >= 2
                && boundary.capture_prefix.capacity() >= 1)
    );
    assert!(FreshRecovery::reserve().is_err());
}

struct RecoveryHeld {
    lock: Option<MigrationLock>,
    boundary: Option<Boundary>,
    authenticated: Option<omavless_domain::private_backup::OpenedBackup>,
    host: Option<ObservationOnlyNativeHost>,
    engine: crate::manager_actor_service::NativeEngine,
}

/// One preinstalled, nonescaping prefix. Losing this handle while the process
/// lives cannot release an uncertain recovery lease/ledger to another caller.
/// Fatal process loss only means unavailable. No reset/retry or ordinary owner.
#[allow(dead_code)]
pub(crate) struct FreshRecovery {
    original: Arc<Mutex<RecoveryHeld>>,
    attempted: bool,
}
impl Drop for FreshRecovery {
    fn drop(&mut self) {
        std::mem::forget(Arc::clone(&self.original));
    }
}

pub(crate) struct NativeRecoveryOrigin<'a> {
    lock: &'a MigrationLock,
    boundary: &'a Boundary,
    host: &'a mut ObservationOnlyNativeHost,
    paths: &'a CutoverPaths,
    desired_paths: &'a DesiredPaths,
    marker: OwnershipMarker,
    desired: DesiredState,
    marker_bytes: Zeroizing<Vec<u8>>,
    desired_bytes: Zeroizing<Vec<u8>>,
    login_bytes: Option<Zeroizing<Vec<u8>>>,
    uid: u32,
}

fn held_bytes(file: &File, maximum: usize) -> Result<Zeroizing<Vec<u8>>, FirstError> {
    use std::os::unix::fs::FileExt;
    let before = file.metadata().map_err(|_| FirstError::Admission)?;
    let length = usize::try_from(before.len()).map_err(|_| FirstError::Admission)?;
    if length > maximum {
        return Err(FirstError::Admission);
    }
    let mut bytes = Zeroizing::new(vec![0; length + 1]);
    let mut done = 0;
    while done < bytes.len() {
        let n = file
            .read_at(&mut bytes[done..], done as u64)
            .map_err(|_| FirstError::Admission)?;
        if n == 0 {
            break;
        }
        done += n;
    }
    let after = file.metadata().map_err(|_| FirstError::Admission)?;
    if done != length || !same_member(&before, &after) || before.gid() != after.gid() {
        return Err(FirstError::Admission);
    }
    bytes.truncate(length);
    Ok(bytes)
}

impl NativeRecoveryOrigin<'_> {
    pub(crate) fn uid(&self) -> u32 {
        self.uid
    }
    pub(crate) fn generation(&self) -> u64 {
        self.marker.generation()
    }
    pub(crate) fn desired_bytes(&self) -> &[u8] {
        &self.desired_bytes
    }
    pub(crate) fn directory(&self, index: usize) -> Result<&File, FirstError> {
        self.boundary
            .directories
            .get(index)
            .map(|(_, file, _)| file)
            .ok_or(FirstError::Admission)
    }
    pub(crate) fn member(&self, index: usize) -> Result<Option<&File>, FirstError> {
        self.boundary
            .members
            .get(index)
            .map(|pin| pin.held.as_ref().map(|(file, _)| file))
            .ok_or(FirstError::Admission)
    }
    pub(crate) fn live(&self, index: usize) -> Result<&File, FirstError> {
        self.boundary
            .live
            .get(index)
            .and_then(|pin| pin.held.as_ref())
            .map(|(file, _)| file)
            .ok_or(FirstError::Admission)
    }
    fn bindings(&self, changed: bool) -> Result<(), FirstError> {
        self.boundary.native_recheck(self.uid, !changed)?;
        if !self.lock.authorizes(self.paths, self.uid)
            || read_marker_existing(self.paths, self.uid).ok().as_ref() != Some(&self.marker)
            || read_desired_snapshot(self.desired_paths, self.uid)
                .ok()
                .as_ref()
                != Some(&self.desired)
            || crate::login_transaction::check_login_receipt_without_private_fence(
                self.paths,
                self.uid,
                self.lock,
                Some(self.marker.generation()),
            )
            .is_err()
        {
            return Err(FirstError::Admission);
        }
        for (index, original, limit) in [
            (0, Some(&self.marker_bytes), 1024),
            (1, Some(&self.desired_bytes), 65536),
            (2, self.login_bytes.as_ref(), 1024),
        ] {
            match (self.member(index)?, original) {
                (None, None) => {}
                (Some(file), Some(bytes))
                    if held_bytes(file, limit)?.as_slice() == bytes.as_slice() => {}
                _ => return Err(FirstError::Admission),
            }
        }
        self.boundary.native_recheck(self.uid, !changed)
    }
    pub(crate) fn check(
        &mut self,
        view: crate::manager_actor_service::NativeStageView<'_>,
    ) -> Result<(), FirstError> {
        if !view.recovery_exclusive() {
            return Err(FirstError::Admission);
        }
        self.bindings(view.live_changed())?;
        for name in [
            "routing-preset.pending.json",
            "restore-finalization.pending",
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        ] {
            if !absent(&self.paths.state_directory.join(name)) {
                return Err(FirstError::Admission);
            }
        }
        self.manager_empty()?;
        if !self
            .host
            .fresh_observation(&self.desired)
            .is_ok_and(|observation| {
                !observation.owned_core_running
                    && observation.owned_auxiliary_mihomo_count == 0
                    && observation.visible_mihomo_count == 0
                    && observation.visible_tun_count == 0
                    && observation.managed_tun_count == 0
                    && !observation.owned_controller_config_verified
                    && !observation.desired_profile_matches_owned
            })
            || (view.stage_present() && !view.pending_allowed(self.desired_paths, self.uid))
        {
            return Err(FirstError::Admission);
        }
        self.manager_empty()?;
        self.bindings(view.live_changed())
    }
    fn manager_empty(&self) -> Result<(), FirstError> {
        for service in [
            crate::production_observation::LEGACY_SERVICE,
            crate::production_observation::RUST_SERVICE,
        ] {
            let state = crate::production_observation::service_state_with_timeout(
                Path::new("/usr/bin/systemctl"),
                service,
                std::time::Duration::from_millis(250),
            )
            .map_err(|_| FirstError::Admission)?;
            if state.active
                || state.main_pid != 0
                || state.exit_status != 0
                || state.result != "success"
            {
                return Err(FirstError::Admission);
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
impl FreshRecovery {
    #[cfg(test)]
    pub(crate) fn original_leases_held(&self, paths: &CutoverPaths, uid: u32) -> bool {
        self.attempted
            && self.original.lock().is_ok_and(|held| {
                held.lock
                    .as_ref()
                    .is_some_and(|lock| lock.authorizes(paths, uid))
                    && held.engine.recovery_held()
            })
    }
    pub(crate) fn reserve() -> Result<Self, FirstError> {
        // One fixed unresolved recovery slot per process. No second handle or
        // authority can be created by dropping a failed/nonfatal first handle.
        RECOVERY_RESERVED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| FirstError::StillFenced)?;
        let (limit, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| FirstError::Admission)?;
        if limit < (crate::manager_actor_service::NATIVE_RETAINED_ROLE_CEILING + 4) as u64 {
            return Err(FirstError::Admission);
        }
        Ok(Self {
            original: Arc::new(Mutex::new(RecoveryHeld {
                lock: None,
                boundary: Some(Boundary::reserve_installed()?),
                authenticated: None,
                host: None,
                engine: crate::manager_actor_service::NativeEngine::reserve(),
            })),
            attempted: false,
        })
    }
    /// Private developer issuer. The supplied paths are positively bound anew;
    /// no old observer result, transaction ID, Session or descriptor is imported.
    pub(crate) fn reconcile_mixed(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        if self.attempted {
            return Err(FirstError::StillFenced);
        }
        self.attempted = true; // before authentication/lease/capture; no reuse
        let mut held = self.original.lock().map_err(|_| FirstError::StillFenced)?;
        held.authenticated =
            Some(open_existing(source, uid, passphrase).map_err(|_| FirstError::Prepare)?);
        if nix::unistd::getuid().as_raw() != uid
            || nix::unistd::geteuid().as_raw() != uid
            || nix::unistd::getgid() != nix::unistd::getegid()
            || desired_paths.directory != paths.state_directory
            || desired_paths.file != paths.state_directory.join("desired.json")
            || paths.ownership_marker != paths.state_directory.join("ownership.json")
            || paths.operation_lock != paths.runtime_base.join(format!("omavless.{uid}.lock"))
            || host_paths.store.file_name() != Some("profiles.json".as_ref())
            || host_paths.core != Path::new("/usr/lib/omavless-dns/mihomo")
            || host_paths.proc_root != Path::new("/proc")
            || host_paths.sys_class_net != Path::new("/sys/class/net")
            || host_paths.runtime_directory != paths.runtime_base.join("omavless")
            || host_paths.config_directory
                != host_paths.store.parent().ok_or(FirstError::Admission)?
            || host_paths.template != host_paths.config_directory.join("route-template.yaml")
        {
            return Err(FirstError::Admission);
        }
        held.lock =
            Some(MigrationLock::acquire_existing(&paths, uid).map_err(|_| FirstError::Admission)?);
        Boundary::capture_native_paths(&paths, &host_paths.store, uid, &mut held.boundary)?;
        held.host = Some(
            ObservationOnlyNativeHost::new(host_paths, uid).map_err(|_| FirstError::Admission)?,
        );
        let marker = read_marker_existing(&paths, uid).map_err(|_| FirstError::Admission)?;
        let desired =
            read_desired_snapshot(&desired_paths, uid).map_err(|_| FirstError::Admission)?;
        if marker.phase() != OwnershipPhase::Rust
            || desired.connected
            || !desired.profile_id.is_empty()
        {
            return Err(FirstError::Admission);
        }
        let RecoveryHeld {
            lock,
            boundary,
            authenticated,
            host,
            engine,
        } = &mut *held;
        let boundary = boundary.as_ref().ok_or(FirstError::Admission)?;
        let original_member = |index: usize, maximum| {
            boundary
                .members
                .get(index)
                .and_then(|pin| pin.held.as_ref())
                .map(|(file, _)| held_bytes(file, maximum))
                .transpose()
        };
        let mut origin = NativeRecoveryOrigin {
            lock: lock.as_ref().ok_or(FirstError::Admission)?,
            boundary,
            host: host.as_mut().ok_or(FirstError::Admission)?,
            paths: &paths,
            desired_paths: &desired_paths,
            marker,
            desired,
            marker_bytes: original_member(0, 1024)?.ok_or(FirstError::Admission)?,
            desired_bytes: original_member(1, 65536)?.ok_or(FirstError::Admission)?,
            login_bytes: original_member(2, 1024)?,
            uid,
        };
        engine.reconcile_native_mixed(
            &mut origin,
            authenticated.as_ref().ok_or(FirstError::Admission)?,
        )
    }
}
