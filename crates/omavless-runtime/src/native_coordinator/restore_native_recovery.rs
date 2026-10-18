// SPDX-License-Identifier: MIT
//! New authenticated recovery origin, never ordinary startup or former authority.
use super::*;
use crate::cutover::{CutoverPaths, OwnershipMarker, OwnershipPhase, read_marker_existing};
use crate::desired::{DesiredPaths, DesiredState, read_desired_snapshot};
use crate::native_host::{NativeHostPaths, ObservationOnlyNativeHost};
use crate::restore_old_stopped::NativeOldRecoveryQuiescence;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use zeroize::Zeroizing;
static RECOVERY_RESERVED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
#[test]
fn final_old_fd_capacity_refuses_before_original_acquisition() {
    const PROBE: &str = "OMAVLESS_SOURCE_FINAL_FD_PROBE";
    if std::env::var_os(PROBE).is_some() {
        let limit = (crate::manager_actor_service::NATIVE_RETAINED_ROLE_CEILING + 3) as u64;
        nix::sys::resource::setrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE, limit, limit)
            .unwrap();
        assert!(matches!(
            FreshRecovery::reserve(),
            Err(FirstError::Admission)
        ));
        return;
    }
    // Only this closed Source test child changes its own FD limit. It opens no
    // fixture, service, manager or production namespace and grants no recovery.
    let test = format!(
        "{}::final_old_fd_capacity_refuses_before_original_acquisition",
        module_path!().split_once("::").unwrap().1
    );
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test, "--test-threads=1"])
        .env(PROBE, "1")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(result.stdout.len() < 8192 && result.stderr.len() < 8192);
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("test result: ok. 1 passed; 0 failed")
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FreshRecoveryMode {
    MixedIntent,
    OldIntent,
    CompleteAborted,
    RecoverOldToCompletion,
    MixedIntentToOldCompletion,
    CommittedNewToCompletion,
    FinalOldOutput,
    FinalNewOutput,
    NewTicketOutput,
    NewCompleteOutput,
}

#[derive(Clone, Copy)]
enum RecoveredPairChoice {
    Old,
    CommittedNew,
    FinalOld,
    FinalNew,
    NewTicket,
    NewComplete,
}

fn recovery_progress(previewed: bool) -> crate::restore_abort_cli::stopped_owner::RecoveryProgress {
    use crate::restore_abort_cli::stopped_owner::RecoveryProgress;
    if previewed {
        RecoveryProgress::ConsumerSilent
    } else {
        RecoveryProgress::DiagnosticCli
    }
}

fn release_phase(
    progress: crate::restore_abort_cli::stopped_owner::RecoveryProgress,
    label: &'static [u8],
) {
    // Fixed public categories, one best-effort write. Diagnostic failure cannot
    // unwind the retained recovery graph or authorize another attempt.
    release_phase_with(progress, label, |bytes| {
        nix::unistd::write(std::io::stderr(), bytes).map_or(-1, |written| written as isize)
    });
}

fn release_phase_with(
    progress: crate::restore_abort_cli::stopped_owner::RecoveryProgress,
    label: &'static [u8],
    write: impl FnOnce(&[u8]) -> isize,
) {
    if progress.enabled() {
        let _ = write(label);
    }
}

#[cfg(test)]
#[test]
fn consumer_recovery_progress_suppresses_release_writes() {
    release_phase_with(
        recovery_progress(true),
        b"RESTORE_RELEASE_ALIASES\n",
        |_| panic!("consumer terminal release writer must not run"),
    );
}

#[cfg(test)]
#[test]
fn recovery_progress_preserves_diagnostic_release_labels_after_write_failure() {
    let labels: [&[u8]; 7] = [
        b"RESTORE_RELEASE_ALIASES\n",
        b"RESTORE_RELEASE_BOUND_ORIGIN\n",
        b"RESTORE_RELEASE_COMPLETION_VERIFIED\n",
        b"RESTORE_RELEASE_POINTER_PLAN_REFUSED\n",
        b"RESTORE_RELEASE_POINTER_CHANGE_REFUSED\n",
        b"RESTORE_RELEASE_POINTER_PRESERVED\n",
        b"RESTORE_RELEASE_DISPOSITION_VERIFIED\n",
    ];
    let mut seen = Vec::new();
    for label in labels {
        release_phase_with(recovery_progress(false), label, |bytes| {
            seen.push(bytes.to_vec());
            -1
        });
    }
    assert_eq!(seen, labels);
}

#[cfg(test)]
#[test]
fn native_fresh_recovery_reserves_one_original_and_retains_after_handle_loss() {
    let recovery = FreshRecovery::reserve().unwrap();
    assert!(!recovery.attempted);
    let original = Arc::downgrade(&recovery.original);
    assert!(FreshRecovery::reserve().is_err());
    drop(recovery);
    let retained = original.upgrade().unwrap();
    let slot = retained.lock().unwrap();
    let held = slot.as_ref().unwrap();
    assert!(held.lock.get().is_none() && held.authenticated.is_none() && held.host.is_none());
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
    lock: Arc<OnceLock<MigrationLock>>,
    failed_lock: Option<MigrationLock>,
    scope: Arc<AtomicBool>,
    boundary: Option<Boundary>,
    authenticated: Option<omavless_domain::private_backup::OpenedBackup>,
    host: Option<ObservationOnlyNativeHost>,
    engine: crate::manager_actor_service::NativeEngine,
    facts: Option<OriginFacts>,
}
fn move_prechecked_original(
    source: &mut Option<RecoveryHeld>,
    destination: &mut Option<RecoveryHeld>,
) {
    // The caller checked both under their original mutexes and installed the
    // destination. Only a move: no user callback, allocation or Flock Drop.
    *destination = source.take();
}

#[cfg(test)]
#[test]
fn old_release_without_positive_completion_keeps_original_migration_custody() {
    let root = tempfile::Builder::new().prefix("or-").tempdir().unwrap();
    std::fs::set_permissions(
        root.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let uid = nix::unistd::getuid().as_raw();
    let paths = CutoverPaths::below(root.path(), root.path(), uid);
    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    let original = Arc::new(Mutex::new(Some(RecoveryHeld {
        lock: Arc::new(OnceLock::from(lock)),
        failed_lock: None,
        scope: Arc::new(AtomicBool::new(false)),
        boundary: None,
        authenticated: None,
        host: None,
        engine: crate::manager_actor_service::NativeEngine::reserve(),
        facts: None,
    })));
    let weak = Arc::downgrade(&original);
    let recovery = FreshRecovery {
        preview_binding: None,
        original,
        destination: Arc::new(Mutex::new(None)),
        available: Arc::new(AtomicBool::new(true)),
        disposition_attempted: false,
        attempted: true,
        released: false,
        old_quiescence: None,
        normal_owner: None,
    };
    assert!(recovery.dispose_and_release_old().is_err());
    let retained = weak.upgrade().unwrap();
    assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    let mut slot = retained.lock().unwrap();
    assert!(
        slot.as_ref()
            .unwrap()
            .lock
            .get()
            .unwrap()
            .authorizes(&paths, uid)
    );
    // This local pre-admission fixture never acquired native/manager resources
    // or issued an effect. Clean only that known original synthetic flock.
    drop(slot.take());
    drop(slot);
    drop(MigrationLock::acquire_existing(&paths, uid).unwrap());
    std::fs::remove_file(&paths.operation_lock).unwrap();
    drop(retained);
}

#[cfg(test)]
#[test]
fn native_completion_move_and_borrow_drop_keep_the_one_original_flock() {
    use std::fs;
    use std::os::unix::fs::DirBuilderExt;
    let root = std::env::temp_dir().join(format!("nl-{:x}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let paths = CutoverPaths::below(&root, &root, nix::unistd::getuid().as_raw());
    let uid = nix::unistd::getuid().as_raw();
    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    let mut source = Some(RecoveryHeld {
        lock: Arc::new(OnceLock::from(lock)),
        failed_lock: None,
        scope: Arc::new(AtomicBool::new(false)),
        boundary: None,
        authenticated: None,
        host: None,
        engine: crate::manager_actor_service::NativeEngine::reserve(),
        facts: None,
    });
    let mut destination = None;
    assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    move_prechecked_original(&mut source, &mut destination);
    assert!(source.is_none());
    {
        // The actual production borrower uses a nonescaping reference, not an
        // owned Flock wrapper. Letting that reference end cannot unlock it.
        let original = destination.as_ref().unwrap().lock.get().unwrap();
        assert!(original.authorizes(&paths, uid));
        assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    }
    assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    drop(source);
    assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    for _ in 0..3 {
        let once = Arc::clone(&destination.as_ref().unwrap().lock);
        let borrower = NativeMigrationBorrow {
            original: once,
            _scope: NativeScopeBorrow {
                scope: Arc::new(AtomicBool::new(true)),
                available: Arc::new(AtomicBool::new(true)),
            },
        };
        assert!(std::ptr::eq(
            &*borrower,
            destination.as_ref().unwrap().lock.get().unwrap()
        ));
        assert!(borrower.authorizes(&paths, uid));
        assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
        drop(borrower); // Arc borrow ends, but the ONE original Flock stays held
        assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    }
    let available = Arc::new(AtomicBool::new(true));
    let scope = Arc::new(AtomicBool::new(false));
    let original = Arc::clone(&destination.as_ref().unwrap().lock);
    let cut = std::panic::catch_unwind(|| {
        let _borrower = NativeMigrationBorrow {
            original: Arc::clone(&original),
            _scope: NativeScopeBorrow::reserve(&scope, &available).unwrap(),
        };
        panic!("public original borrower unwind");
    });
    assert!(cut.is_err() && !available.load(Ordering::Acquire));
    assert!(!scope.load(Ordering::Acquire));
    assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
    drop(original);
    drop(destination); // only this fully positive local fixture, no uncertain owner
    drop(MigrationLock::acquire_existing(&paths, uid).unwrap());
    fs::remove_file(paths.operation_lock).unwrap();
    fs::remove_dir(root).unwrap();
}
struct OriginFacts {
    paths: CutoverPaths,
    desired_paths: DesiredPaths,
    marker: OwnershipMarker,
    desired: DesiredState,
    marker_bytes: Zeroizing<Vec<u8>>,
    desired_bytes: Zeroizing<Vec<u8>>,
    login_bytes: Option<Zeroizing<Vec<u8>>>,
    uid: u32,
}

/// One preinstalled, nonescaping prefix. Losing this handle while the process
/// lives cannot release an uncertain recovery lease/ledger to another caller.
/// Fatal process loss only means unavailable. No reset/retry or implicit
/// ordinary admission; its explicit completion consumer stays separately gated.
#[allow(dead_code)]
pub(crate) struct FreshRecovery {
    // Consumer observation DATA only; compared again under this new lease
    // before capture/Stop. Legacy closed developer paths leave this absent.
    preview_binding: Option<crate::restore_recovery_preview::Preview>,
    original: Arc<Mutex<Option<RecoveryHeld>>>,
    // Empty destination reserved before any original acquisition. Moving the
    // payload here never duplicates a MigrationLock/Flock or its authority.
    destination: Arc<Mutex<Option<RecoveryHeld>>>,
    available: Arc<AtomicBool>,
    disposition_attempted: bool,
    attempted: bool,
    // Only the new OLD consumer can positively release its completed graph.
    // All errors/unwinds retain the pre-existing fail-closed Drop behavior.
    released: bool,
    // The synchronous new consumer's guard is not installed in the ordinary
    // Send owner graph. Its unknown original child is retained separately.
    old_quiescence: Option<Box<NativeOldRecoveryQuiescence>>,
    normal_owner: Option<
        crate::production_owner::ProductionNativeOwner<crate::native_host::NativeLifecycleHost>,
    >,
}

/// Private readback DATA retained after positive release, never a startup or
/// mutation grant. Normal Start must earn ordinary admission independently.
pub(crate) struct ReleasedOldConfiguration(ReleasedConfiguration);
#[allow(dead_code)] // Source-only recovery slice; public product entry remains denied.
pub(crate) struct ReleasedNewConfiguration(ReleasedConfiguration);
struct ReleasedConfiguration {
    paths: CutoverPaths,
    desired_paths: DesiredPaths,
    config: std::path::PathBuf,
    uid: u32,
    marker: OwnershipMarker,
    desired: DesiredState,
    pair: crate::backup_source_candidate::PrivateSourcePair,
    history_name: &'static str,
    history_bytes: Zeroizing<Vec<u8>>,
}

impl ReleasedOldConfiguration {
    pub(crate) fn verify_current(&self) -> Result<(), FirstError> {
        self.0.verify_current()
    }
}
#[allow(dead_code)] // Installed recovery driver integration is a separate gate.
impl ReleasedNewConfiguration {
    pub(crate) fn verify_current(&self) -> Result<(), FirstError> {
        self.0.verify_current()
    }
}
impl ReleasedConfiguration {
    fn verify_current(&self) -> Result<(), FirstError> {
        // New read-only lease after the normal service has independently
        // started. Never reacquire its singleton or replay recovery here.
        let lock = MigrationLock::acquire_existing(&self.paths, self.uid)
            .map_err(|_| FirstError::StillFenced)?;
        let mut admission = crate::startup_admission::StartupAdmission::ordinary();
        verify_ordinary_off(
            &mut admission,
            &self.paths,
            &self.desired_paths,
            self.uid,
            &lock,
            &self.marker,
            &self.desired,
        )?;
        let current = crate::backup_source_candidate::capture_current_pair(
            &self.config,
            &self.paths,
            self.uid,
            self.marker.generation(),
            &lock,
        )
        .map_err(|_| FirstError::StillFenced)?;
        if current.store() != self.pair.store() || current.template() != self.pair.template() {
            return Err(FirstError::StillFenced);
        }
        let state = open_private_directory(&self.paths.state_directory, self.uid)
            .map_err(|_| FirstError::StillFenced)?;
        let history = File::from(
            openat(
                &state,
                self.history_name,
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| FirstError::StillFenced)?,
        );
        let meta = history.metadata().map_err(|_| FirstError::StillFenced)?;
        if !meta.is_file()
            || meta.uid() != self.uid
            || meta.mode() & 0o7777 != 0o600
            || meta.nlink() != 1
            || held_bytes(&history, self.history_bytes.len())?.as_slice()
                != self.history_bytes.as_slice()
        {
            return Err(FirstError::StillFenced);
        }
        verify_ordinary_off(
            &mut admission,
            &self.paths,
            &self.desired_paths,
            self.uid,
            &lock,
            &self.marker,
            &self.desired,
        )
    }
}

fn verify_ordinary_off(
    admission: &mut crate::startup_admission::StartupAdmission<'_, '_>,
    paths: &CutoverPaths,
    desired_paths: &DesiredPaths,
    uid: u32,
    lock: &MigrationLock,
    marker: &OwnershipMarker,
    desired: &DesiredState,
) -> Result<(), FirstError> {
    admission
        .bind(paths, uid, lock)
        .map_err(|_| FirstError::StillFenced)?;
    if admission.marker(paths, uid).ok().as_ref() != Some(marker)
        || marker.phase() != OwnershipPhase::Rust
        || admission.desired(desired_paths, uid).ok().as_ref() != Some(desired)
        || desired.connected
        || !desired.profile_id.is_empty()
    {
        return Err(FirstError::StillFenced);
    }
    crate::login_activation::require_current_receipt(paths, uid, lock, marker.generation())
        .map_err(|_| FirstError::StillFenced)?;
    admission
        .receipt(paths, uid, lock, marker.generation())
        .map_err(|_| FirstError::StillFenced)?;
    admission
        .transaction(paths, desired_paths, uid, lock, false)
        .map_err(|_| FirstError::StillFenced)?;
    admission
        .action(crate::desired::ReconcileAction::SettledDisconnected)
        .map_err(|_| FirstError::StillFenced)?;
    admission.recheck().map_err(|_| FirstError::StillFenced)
}
#[derive(Clone)]
pub(crate) struct NativeSteadyCompletion {
    original: Arc<Mutex<Option<RecoveryHeld>>>,
    available: Arc<AtomicBool>,
}

/// Current ordinary lease origin, never a decoded history or frozen-Off grant.
/// The ONE initialized OnceLock owns the ONE actual MigrationLock object.
pub(crate) struct NativeOrdinaryLease {
    original: NativeOrdinaryCustody,
    lock: Arc<OnceLock<MigrationLock>>,
    available: Arc<AtomicBool>,
    paths: CutoverPaths,
    uid: u32,
    generation: u64,
    scope: Arc<AtomicBool>,
}
enum NativeOrdinaryCustody {
    Recovery(Arc<Mutex<Option<RecoveryHeld>>>),
    Committed(Arc<Mutex<super::retained::HeldExecution>>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeLeaseError {
    Busy,
    Unavailable,
}
struct NativeScopeBorrow {
    scope: Arc<AtomicBool>,
    available: Arc<AtomicBool>,
}
impl NativeScopeBorrow {
    fn reserve(
        scope: &Arc<AtomicBool>,
        available: &Arc<AtomicBool>,
    ) -> Result<Self, NativeLeaseError> {
        scope
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| NativeLeaseError::Busy)?;
        Ok(Self {
            scope: Arc::clone(scope),
            available: Arc::clone(available),
        })
    }
}
impl Drop for NativeScopeBorrow {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // No mutex recovery, callback or Flock Drop during unwind. The
            // original graph remains in its installed keeper; later admission
            // is permanently unavailable even when the scope bit is released.
            self.available.store(false, Ordering::Release);
        }
        self.scope.store(false, Ordering::Release);
    }
}
pub(crate) struct NativeMigrationBorrow {
    original: Arc<OnceLock<MigrationLock>>,
    _scope: NativeScopeBorrow,
}
impl std::ops::Deref for NativeMigrationBorrow {
    type Target = MigrationLock;
    fn deref(&self) -> &MigrationLock {
        // Minted only after get() succeeded; OnceLock cannot be unset. Keeping
        // this same Arc alive cannot create a second Flock or unlock the first.
        self.original
            .get()
            .expect("original migration lease initialized")
    }
}
impl NativeOrdinaryLease {
    pub(crate) fn borrow(
        &self,
        paths: &CutoverPaths,
        uid: u32,
    ) -> Result<NativeMigrationBorrow, NativeLeaseError> {
        if !self.available.load(Ordering::Acquire) {
            return Err(NativeLeaseError::Unavailable);
        }
        if paths != &self.paths || uid != self.uid {
            self.refuse();
            return Err(NativeLeaseError::Unavailable);
        }
        // A flock excludes another open description, not simultaneous leases
        // from this SAME owner. This guard spans the ordinary effect scope.
        let reservation = NativeScopeBorrow::reserve(&self.scope, &self.available)?;
        let result = self.borrow_checked(paths, uid);
        if result.is_err() {
            self.refuse();
            return Err(NativeLeaseError::Unavailable); // reservation release cannot restore availability
        }
        Ok(NativeMigrationBorrow {
            original: Arc::clone(&self.lock),
            _scope: reservation,
        })
    }
    fn refuse(&self) {
        self.available.store(false, Ordering::Release);
        // Poison is itself permanent unavailability; never recover the guard
        // or reconstruct an authority snapshot from its possibly partial data.
        match &self.original {
            NativeOrdinaryCustody::Recovery(original) => {
                if let Ok(mut slot) = original.lock()
                    && let Some(held) = slot.as_mut()
                {
                    held.engine.revoke_native();
                }
            }
            NativeOrdinaryCustody::Committed(original) => {
                if let Ok(mut held) = original.lock() {
                    held.refuse_ordinary();
                }
            }
        }
    }
    fn borrow_checked(&self, paths: &CutoverPaths, uid: u32) -> Result<(), ()> {
        match &self.original {
            NativeOrdinaryCustody::Recovery(original) => {
                let mut held = original.lock().map_err(|_| ())?;
                let held = held.as_mut().ok_or(())?;
                if !Arc::ptr_eq(&held.lock, &self.lock)
                    || !held.engine.ordinary_lease_held()
                    || held.failed_lock.is_some()
                {
                    return Err(());
                }
                held.engine.check_ordinary_lease_prefix()?;
            }
            NativeOrdinaryCustody::Committed(original) => {
                original
                    .lock()
                    .map_err(|_| ())?
                    .check_ordinary(&self.lock)?;
            }
        }
        let actual = self.lock.get().ok_or(())?;
        current_ordinary_lease(actual, paths, uid, self.generation)
    }
    pub(super) fn committed(
        original: Arc<Mutex<super::retained::HeldExecution>>,
        lock: Arc<OnceLock<MigrationLock>>,
        available: Arc<AtomicBool>,
        scope: Arc<AtomicBool>,
        paths: CutoverPaths,
        uid: u32,
        generation: u64,
    ) -> Self {
        Self {
            original: NativeOrdinaryCustody::Committed(original),
            lock,
            available,
            scope,
            paths,
            uid,
            generation,
        }
    }
}
fn current_ordinary_lease(
    actual: &MigrationLock,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
) -> Result<(), ()> {
    if !actual.authorizes(paths, uid)
        || !read_marker_existing(paths, uid).is_ok_and(|marker| {
            marker.phase() == OwnershipPhase::Rust && marker.generation() == generation
        })
        || crate::pending_private_transaction::pending_at(&paths.state_directory)
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn native_ordinary_scope_busy_never_releases_the_other_borrow() {
    let scope = Arc::new(AtomicBool::new(false));
    let available = Arc::new(AtomicBool::new(true));
    let first = NativeScopeBorrow::reserve(&scope, &available).unwrap();
    for _ in 0..3 {
        assert!(matches!(
            NativeScopeBorrow::reserve(&scope, &available),
            Err(NativeLeaseError::Busy)
        ));
        assert!(scope.load(Ordering::Acquire));
    }
    drop(first);
    assert!(available.load(Ordering::Acquire));
    let second = NativeScopeBorrow::reserve(&scope, &available).unwrap();
    assert!(scope.load(Ordering::Acquire));
    drop(second);
    assert!(!scope.load(Ordering::Acquire));
}

#[cfg(test)]
#[test]
fn native_ordinary_scope_unwind_cannot_restore_available_authority() {
    let scope = Arc::new(AtomicBool::new(false));
    let available = Arc::new(AtomicBool::new(true));
    let result = std::panic::catch_unwind(|| {
        let _original_scope = NativeScopeBorrow::reserve(&scope, &available).unwrap();
        panic!("public inert scope cut");
    });
    assert!(result.is_err());
    assert!(!scope.load(Ordering::Acquire));
    assert!(!available.load(Ordering::Acquire));
}

#[cfg(test)]
#[test]
fn native_ordinary_current_marker_pending_and_lease_drift_are_permanent_refusal() {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let root = crate::test_temp::directory_under(
        Path::new(&std::env::var_os("HOME").unwrap()),
        "t4-ordinary-drift",
    )
    .unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    for cut in ["marker", "pending", "lease"] {
        let base = root.join(cut);
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        let paths = CutoverPaths::below(&base, &base, nix::unistd::getuid().as_raw());
        let uid = nix::unistd::getuid().as_raw();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&paths.state_directory)
            .unwrap();
        let marker = br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#;
        fs::write(&paths.ownership_marker, marker).unwrap();
        fs::set_permissions(&paths.ownership_marker, fs::Permissions::from_mode(0o600)).unwrap();
        let actual = Arc::new(OnceLock::from(MigrationLock::acquire(&paths, uid).unwrap()));
        // Real retained/named lease and marker checks, not a native completion
        // issuer. The reserved empty engine below cannot grant native effects.
        assert!(current_ordinary_lease(actual.get().unwrap(), &paths, uid, 2).is_ok());
        let original = Arc::new(Mutex::new(Some(RecoveryHeld {
            lock: Arc::clone(&actual),
            failed_lock: None,
            scope: Arc::new(AtomicBool::new(false)),
            boundary: None,
            authenticated: None,
            host: None,
            engine: crate::manager_actor_service::NativeEngine::reserve(),
            facts: None,
        })));
        let keeper = NativeOrdinaryLease {
            original: NativeOrdinaryCustody::Recovery(Arc::clone(&original)),
            lock: Arc::clone(&actual),
            available: Arc::new(AtomicBool::new(true)),
            paths: paths.clone(),
            uid,
            generation: 2,
            scope: Arc::new(AtomicBool::new(false)),
        };
        match cut {
            "marker" => fs::write(&paths.ownership_marker, b"{}").unwrap(),
            "pending" => fs::write(
                paths
                    .state_directory
                    .join(crate::restore_closure_model::CLOSURE_MEMBER),
                b"foreign",
            )
            .unwrap(),
            "lease" => fs::rename(&paths.operation_lock, base.join("original-lock")).unwrap(),
            _ => unreachable!(),
        }
        assert!(current_ordinary_lease(actual.get().unwrap(), &paths, uid, 2).is_err());
        keeper.refuse(); // exact production irreversible refusal operation
        match cut {
            "marker" => fs::write(&paths.ownership_marker, marker).unwrap(),
            "pending" => fs::remove_file(
                paths
                    .state_directory
                    .join(crate::restore_closure_model::CLOSURE_MEMBER),
            )
            .unwrap(),
            "lease" => fs::rename(base.join("original-lock"), &paths.operation_lock).unwrap(),
            _ => unreachable!(),
        }
        assert!(current_ordinary_lease(actual.get().unwrap(), &paths, uid, 2).is_ok());
        assert!(matches!(
            keeper.borrow(&paths, uid),
            Err(NativeLeaseError::Unavailable)
        ));
        assert!(!keeper.available.load(Ordering::Acquire));
        assert!(MigrationLock::acquire_existing(&paths, uid).is_err());
        drop((keeper, original, actual)); // known wholly local fixture, no uncertain actor
        drop(MigrationLock::acquire_existing(&paths, uid).unwrap());
        fs::remove_dir_all(base).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

/// Private loan minted only from the moved original holder under its mutex.
pub(crate) struct NativeMutationLease<'a, 'b> {
    origin: &'a mut NativeRecoveryOrigin<'b>,
    engine: &'a mut crate::manager_actor_service::NativeEngine,
    until: std::time::Instant,
}
impl NativeMutationLease<'_, '_> {
    pub(crate) fn lock(&self) -> &MigrationLock {
        self.origin.lock
    }
    pub(crate) fn bind(&mut self, paths: &CutoverPaths, uid: u32) -> Result<(), ()> {
        if paths != self.origin.paths || uid != self.origin.uid || !self.engine.disposition_ready()
        {
            return Err(());
        }
        self.check()
    }
    pub(crate) fn check(&mut self) -> Result<(), ()> {
        self.engine
            .check_native_completed(self.origin, self.until)
            .map_err(|_| ())
    }
    pub(crate) fn begin_store(&mut self) -> Result<(), ()> {
        self.engine
            .begin_native_store_mutation(self.origin, self.until)
            .map_err(|_| ())
    }
    pub(crate) fn reported_store(&mut self, expected: &[u8], changed: bool) -> Result<(), ()> {
        self.engine
            .finish_native_store_mutation(self.origin, expected, changed, self.until)
            .map_err(|_| ())
    }
    pub(crate) fn refuse(&mut self) {
        self.engine.revoke_native();
    }
}
impl NativeSteadyCompletion {
    pub(crate) fn ordinary_origin(&self) -> Result<NativeOrdinaryLease, ()> {
        let result = self.ordinary_origin_inner();
        if result.is_err() {
            self.available.store(false, Ordering::Release);
            if let Ok(mut slot) = self.original.lock()
                && let Some(held) = slot.as_mut()
            {
                held.engine.revoke_native();
            }
        }
        result
    }
    fn ordinary_origin_inner(&self) -> Result<NativeOrdinaryLease, ()> {
        self.recheck()?;
        let mut slot = self.original.lock().map_err(|_| ())?;
        let held = slot.as_mut().ok_or(())?;
        let facts = held.facts.as_ref().ok_or(())?;
        if !held.engine.disposition_ready()
            || held.failed_lock.is_some()
            || held.lock.get().is_none()
        {
            return Err(());
        }
        let result = NativeOrdinaryLease {
            original: NativeOrdinaryCustody::Recovery(Arc::clone(&self.original)),
            lock: Arc::clone(&held.lock),
            available: Arc::clone(&self.available),
            paths: facts.paths.clone(),
            uid: facts.uid,
            generation: facts.marker.generation(),
            scope: Arc::clone(&held.scope),
        };
        // Original table stays retained; older completion proof is consumed.
        // Later ordinary operations use fresh ordinary state, not OriginFacts.
        held.engine.consume_into_ordinary_lease()?;
        Ok(result)
    }
    pub(crate) fn with_mutation<T>(
        &self,
        operation: impl FnOnce(&mut NativeMutationLease<'_, '_>) -> Result<T, ()>,
    ) -> Result<T, ()> {
        if !self.available.load(Ordering::Acquire) {
            return Err(());
        }
        let result = self.with_mutation_inner(operation);
        if result.is_err() {
            self.available.store(false, Ordering::Release);
        }
        result
    }
    fn with_mutation_inner<T>(
        &self,
        operation: impl FnOnce(&mut NativeMutationLease<'_, '_>) -> Result<T, ()>,
    ) -> Result<T, ()> {
        let mut slot = self.original.lock().map_err(|_| ())?;
        let RecoveryHeld {
            lock,
            boundary,
            host,
            engine,
            facts,
            ..
        } = slot.as_mut().ok_or(())?;
        let facts = facts.as_ref().ok_or(())?;
        if !engine.disposition_ready() {
            return Err(());
        }
        let mut origin = NativeRecoveryOrigin {
            lock: lock.get().ok_or(())?,
            boundary: boundary.as_ref().ok_or(())?,
            host: host.as_mut().ok_or(())?,
            paths: &facts.paths,
            desired_paths: &facts.desired_paths,
            marker: facts.marker.clone(),
            desired: facts.desired.clone(),
            marker_bytes: facts.marker_bytes.clone(),
            desired_bytes: facts.desired_bytes.clone(),
            login_bytes: facts.login_bytes.clone(),
            uid: facts.uid,
        };
        let mut loan = NativeMutationLease {
            origin: &mut origin,
            engine,
            until: std::time::Instant::now() + std::time::Duration::from_secs(15),
        };
        loan.check()?;
        let result = operation(&mut loan);
        if result.is_err() {
            loan.refuse();
            return result;
        }
        loan.check()?;
        result
    }
    pub(crate) fn same_original(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.original, &other.original)
            && Arc::ptr_eq(&self.available, &other.available)
    }
    fn checked<T>(&self, read: impl FnOnce(&OriginFacts) -> T) -> Result<T, ()> {
        if !self.available.load(Ordering::Acquire) {
            return Err(());
        }
        let result = self.checked_inner(read);
        if result.is_err() {
            self.available.store(false, Ordering::Release);
        }
        result
    }
    fn checked_inner<T>(&self, read: impl FnOnce(&OriginFacts) -> T) -> Result<T, ()> {
        let mut slot = self.original.lock().map_err(|_| ())?;
        let held = slot.as_mut().ok_or(())?;
        let RecoveryHeld {
            lock,
            boundary,
            host,
            engine,
            facts,
            ..
        } = held;
        let facts = facts.as_ref().ok_or(())?;
        let mut origin = NativeRecoveryOrigin {
            lock: lock.get().ok_or(())?,
            boundary: boundary.as_ref().ok_or(())?,
            host: host.as_mut().ok_or(())?,
            paths: &facts.paths,
            desired_paths: &facts.desired_paths,
            marker: facts.marker.clone(),
            desired: facts.desired.clone(),
            marker_bytes: facts.marker_bytes.clone(),
            desired_bytes: facts.desired_bytes.clone(),
            login_bytes: facts.login_bytes.clone(),
            uid: facts.uid,
        };
        engine
            .check_native_completed(
                &mut origin,
                std::time::Instant::now() + std::time::Duration::from_secs(2),
            )
            .map_err(|_| ())?;
        Ok(read(facts))
    }
    pub(crate) fn desired(&self, paths: &DesiredPaths, uid: u32) -> Result<DesiredState, ()> {
        self.checked(|facts| {
            if uid == facts.uid && paths == &facts.desired_paths {
                Some(facts.desired.clone())
            } else {
                None
            }
        })?
        .ok_or(())
    }
    pub(crate) fn ownership(&self, paths: &CutoverPaths, uid: u32, generation: u64) -> bool {
        self.checked(|facts| {
            paths == &facts.paths && uid == facts.uid && generation == facts.marker.generation()
        })
        .unwrap_or(false)
    }
    pub(crate) fn recheck(&self) -> Result<(), ()> {
        self.checked(|_| ())
    }
}
impl Drop for FreshRecovery {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        std::mem::forget(self.normal_owner.take());
        if let Some(original) = self.old_quiescence.take() {
            // Preserve the actual typed child/observer in stable storage, not
            // only leaked raw descriptor numbers from a forgotten stack value.
            Box::leak(original);
        }
        std::mem::forget(Arc::clone(&self.original));
        std::mem::forget(Arc::clone(&self.destination));
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

/// Nonescaping SAME-holder completion loan. Private fields; no snapshot,
/// HistoricalOff fabrication, caller boolean or missing-fence inference.
pub(crate) struct NativeCompletedOff<'a, 'b> {
    origin: &'a mut NativeRecoveryOrigin<'b>,
    engine: &'a mut crate::manager_actor_service::NativeEngine,
    until: std::time::Instant,
}
impl NativeCompletedOff<'_, '_> {
    pub(crate) fn recheck(&mut self) -> Result<(), ()> {
        self.engine
            .check_native_completed(self.origin, self.until)
            .map_err(|_| ())
    }
    pub(crate) fn bind(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
        lock: &MigrationLock,
    ) -> Result<(), ()> {
        if uid != self.origin.uid
            || paths != self.origin.paths
            || !std::ptr::eq(lock, self.origin.lock)
        {
            return Err(());
        }
        self.recheck()
    }
    pub(crate) fn marker(&mut self, paths: &CutoverPaths, uid: u32) -> Result<OwnershipMarker, ()> {
        if uid != self.origin.uid || paths != self.origin.paths {
            return Err(());
        }
        self.recheck()?;
        Ok(self.origin.marker.clone())
    }
    pub(crate) fn desired(&mut self, paths: &DesiredPaths, uid: u32) -> Result<DesiredState, ()> {
        if uid != self.origin.uid || paths != self.origin.desired_paths {
            return Err(());
        }
        self.recheck()?;
        Ok(self.origin.desired.clone())
    }
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
    pub(crate) fn check_old_recovery_lease(&mut self) -> Result<(), FirstError> {
        // Used only before/after the engine's explicit fixed quiescence phase.
        // The original lease and Off members are still borrowed, never minted
        // from a manager snapshot or an earlier constructor-time observation.
        self.bindings(false)?;
        self.check_observed_off()?;
        self.bindings(false)
    }
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
        self.check_inner(view, None)
    }
    pub(crate) fn check_old_recovery(
        &mut self,
        view: crate::manager_actor_service::NativeStageView<'_>,
        quiescence: &NativeOldRecoveryQuiescence,
    ) -> Result<(), FirstError> {
        // The concrete original guard, not inactive-unit bytes or a caller
        // flag, qualifies the failed/stopped runtime in this new consumer.
        quiescence.bind(self.lock, self.paths, self.uid)?;
        self.check_inner(view, Some(quiescence))
    }
    fn check_inner(
        &mut self,
        view: crate::manager_actor_service::NativeStageView<'_>,
        quiescence: Option<&NativeOldRecoveryQuiescence>,
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
            if !view.own_completion_member(name) && !absent(&self.paths.state_directory.join(name))
            {
                return Err(FirstError::Admission);
            }
        }
        self.check_manager(quiescence)?;
        self.check_observed_off()?;
        if view.stage_present() && !view.pending_allowed(self.desired_paths, self.uid) {
            return Err(FirstError::Admission);
        }
        self.check_manager(quiescence)?;
        self.bindings(view.live_changed())
    }
    fn check_observed_off(&mut self) -> Result<(), FirstError> {
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
        {
            return Err(FirstError::Admission);
        }
        Ok(())
    }
    fn check_manager(
        &self,
        quiescence: Option<&NativeOldRecoveryQuiescence>,
    ) -> Result<(), FirstError> {
        match quiescence {
            Some(original) => original.recheck(),
            None => self.manager_empty(),
        }
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
    /// Distinct consuming release, not the historical in-process owner transfer.
    /// Every fallible check runs while the original keeper remains installed.
    pub(crate) fn dispose_and_release_old(self) -> Result<ReleasedOldConfiguration, FirstError> {
        self.dispose_and_release(RecoveredPairChoice::Old)
            .map(ReleasedOldConfiguration)
    }

    pub(crate) fn dispose_and_release_new(self) -> Result<ReleasedNewConfiguration, FirstError> {
        self.dispose_and_release(RecoveredPairChoice::CommittedNew)
            .map(ReleasedNewConfiguration)
    }
    pub(crate) fn dispose_and_release_final_old(
        self,
    ) -> Result<ReleasedOldConfiguration, FirstError> {
        self.dispose_and_release(RecoveredPairChoice::FinalOld)
            .map(ReleasedOldConfiguration)
    }
    pub(crate) fn dispose_and_release_final_new(
        self,
    ) -> Result<ReleasedNewConfiguration, FirstError> {
        self.dispose_and_release(RecoveredPairChoice::FinalNew)
            .map(ReleasedNewConfiguration)
    }

    pub(crate) fn dispose_and_release_new_ticket(
        self,
    ) -> Result<ReleasedNewConfiguration, FirstError> {
        self.dispose_and_release(RecoveredPairChoice::NewTicket)
            .map(ReleasedNewConfiguration)
    }
    pub(crate) fn dispose_and_release_new_complete(
        self,
    ) -> Result<ReleasedNewConfiguration, FirstError> {
        self.dispose_and_release(RecoveredPairChoice::NewComplete)
            .map(ReleasedNewConfiguration)
    }
    fn dispose_and_release(
        mut self,
        choice: RecoveredPairChoice,
    ) -> Result<ReleasedConfiguration, FirstError> {
        if self.disposition_attempted
            || !self.available.load(Ordering::Acquire)
            || self.normal_owner.is_some()
            || self.released
        {
            return Err(FirstError::StillFenced);
        }
        self.disposition_attempted = true;
        let result = self.dispose_and_release_inner(choice);
        if result.is_err() {
            self.available.store(false, Ordering::Release);
            if let Ok(mut slot) = self.original.lock()
                && let Some(held) = slot.as_mut()
            {
                held.engine.revoke_native();
            }
        }
        result
    }

    fn dispose_and_release_inner(
        &mut self,
        choice: RecoveredPairChoice,
    ) -> Result<ReleasedConfiguration, FirstError> {
        let progress = recovery_progress(self.preview_binding.is_some());
        release_phase(progress, b"RESTORE_RELEASE_ALIASES\n");
        // No installed normal owner, completion borrower or external keeper may
        // survive this release. All aliases are classified before taking data.
        if Arc::strong_count(&self.original) != 1
            || Arc::strong_count(&self.destination) != 1
            || Arc::strong_count(&self.available) != 1
        {
            return Err(FirstError::StillFenced);
        }
        let mut slot = self.original.lock().map_err(|_| FirstError::StillFenced)?;
        let destination = self
            .destination
            .lock()
            .map_err(|_| FirstError::StillFenced)?;
        if destination.is_some() {
            return Err(FirstError::StillFenced);
        }
        let held = slot.as_mut().ok_or(FirstError::StillFenced)?;
        if held.failed_lock.is_some()
            || held.scope.load(Ordering::Acquire)
            || Arc::strong_count(&held.scope) != 1
            || Arc::strong_count(&held.lock) != 2
        {
            return Err(FirstError::StillFenced);
        }
        release_phase(progress, b"RESTORE_RELEASE_BOUND_ORIGIN\n");
        let RecoveryHeld {
            lock,
            boundary,
            host,
            engine,
            facts,
            ..
        } = held;
        let facts = facts.as_ref().ok_or(FirstError::StillFenced)?;
        let boundary = boundary.as_ref().ok_or(FirstError::StillFenced)?;
        let config = boundary
            .directories
            .first()
            .ok_or(FirstError::StillFenced)?
            .0
            .clone();
        let quiescence = self
            .old_quiescence
            .as_mut()
            .ok_or(FirstError::StillFenced)?;
        let lock = lock.get().ok_or(FirstError::StillFenced)?;
        let mut origin = NativeRecoveryOrigin {
            lock,
            boundary,
            host: host.as_mut().ok_or(FirstError::StillFenced)?,
            paths: &facts.paths,
            desired_paths: &facts.desired_paths,
            marker: facts.marker.clone(),
            desired: facts.desired.clone(),
            marker_bytes: facts.marker_bytes.clone(),
            desired_bytes: facts.desired_bytes.clone(),
            login_bytes: facts.login_bytes.clone(),
            uid: facts.uid,
        };
        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
        match choice {
            RecoveredPairChoice::NewComplete => {
                engine.check_native_new_complete_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::NewTicket => {
                engine.check_native_new_ticket_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::FinalNew => {
                engine.check_native_final_new_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::FinalOld => {
                engine.check_native_final_old_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::Old => {
                engine.check_native_old_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::CommittedNew => {
                engine.check_native_new_completed(&mut origin, quiescence, until)?
            }
        }
        release_phase(progress, b"RESTORE_RELEASE_COMPLETION_VERIFIED\n");
        // Ordinary reconciliation must not rewrite/prune the recovered pair. Check
        // the existing pure pointer planner before publishing disposition.
        let pointers = crate::private_store_transaction::prepare_pointer_mutation(
            &config.join("profiles.json"),
            facts.uid,
            omavless_domain::private_store::CompatibilityPointerTarget::Disconnected {
                prune_missing: true,
            },
        )
        .map_err(|_| {
            release_phase(progress, b"RESTORE_RELEASE_POINTER_PLAN_REFUSED\n");
            FirstError::StillFenced
        })?;
        if pointers.changed() {
            release_phase(progress, b"RESTORE_RELEASE_POINTER_CHANGE_REFUSED\n");
            return Err(FirstError::StillFenced);
        }
        release_phase(progress, b"RESTORE_RELEASE_POINTER_PRESERVED\n");
        match choice {
            RecoveredPairChoice::NewComplete => {
                engine.dispose_native_new_complete_completed(&mut origin, quiescence)?
            }
            RecoveredPairChoice::NewTicket => {
                engine.dispose_native_new_ticket_completed(&mut origin, quiescence)?
            }
            RecoveredPairChoice::FinalNew => {
                engine.dispose_native_final_new_completed(&mut origin, quiescence)?
            }
            RecoveredPairChoice::FinalOld => {
                engine.dispose_native_final_old_completed(&mut origin, quiescence)?
            }
            RecoveredPairChoice::Old => {
                engine.dispose_native_old_completed(&mut origin, quiescence)?
            }
            RecoveredPairChoice::CommittedNew => {
                engine.dispose_native_new_completed(&mut origin, quiescence)?
            }
        }
        release_phase(progress, b"RESTORE_RELEASE_DISPOSITION_VERIFIED\n");
        let mut ordinary = crate::startup_admission::StartupAdmission::ordinary();
        verify_ordinary_off(
            &mut ordinary,
            &facts.paths,
            &facts.desired_paths,
            facts.uid,
            lock,
            &facts.marker,
            &facts.desired,
        )?;
        let pair = crate::backup_source_candidate::capture_current_pair(
            &config,
            &facts.paths,
            facts.uid,
            facts.marker.generation(),
            lock,
        )
        .map_err(|_| FirstError::StillFenced)?;
        let (history_name, history) = engine.completed_history_data(&mut origin, quiescence)?;
        let released = ReleasedConfiguration {
            paths: facts.paths.clone(),
            desired_paths: facts.desired_paths.clone(),
            config,
            uid: facts.uid,
            marker: facts.marker.clone(),
            desired: facts.desired.clone(),
            pair,
            history_name,
            history_bytes: Zeroizing::new(history.to_vec()),
        };
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        match choice {
            RecoveredPairChoice::NewComplete => {
                engine.check_native_new_complete_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::NewTicket => {
                engine.check_native_new_ticket_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::FinalNew => {
                engine.check_native_final_new_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::FinalOld => {
                engine.check_native_final_old_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::Old => {
                engine.check_native_old_completed(&mut origin, quiescence, until)?
            }
            RecoveredPairChoice::CommittedNew => {
                engine.check_native_new_completed(&mut origin, quiescence, until)?
            }
        }
        if !engine.disposition_ready() {
            return Err(FirstError::StillFenced);
        }
        // Final positive classification above. No fallible work, callback,
        // service command or lifecycle-host destructor before lease closure.
        self.available.store(false, Ordering::Release);
        self.released = true;
        drop(self.old_quiescence.take());
        drop(slot.take());
        drop(destination);
        drop(slot);
        Ok(released)
    }
    #[cfg(test)]
    pub(crate) fn ordinary_owner(
        &mut self,
    ) -> Option<
        &mut crate::production_owner::ProductionNativeOwner<
            crate::native_host::NativeLifecycleHost,
        >,
    > {
        // Test selector uses the real existing coordinator methods, never a
        // second per-operation writer API or copied completion grant.
        self.normal_owner.as_mut()
    }
    pub(crate) fn activate_ordinary_owner(&mut self) -> Result<(), FirstError> {
        self.normal_owner
            .as_mut()
            .ok_or(FirstError::StillFenced)?
            .activate_native_ordinary_lease()
            .map_err(|_| FirstError::StillFenced)
    }
    pub(crate) fn completed_owner_onboarding(
        &mut self,
        request: &serde_json::Value,
    ) -> Result<crate::native_coordinator::NativeOwnerExecution, FirstError> {
        self.normal_owner
            .as_mut()
            .ok_or(FirstError::StillFenced)?
            .dispatch_native_completed_onboarding(request)
            .map_err(|_| FirstError::StillFenced)
    }
    /// Developer-only consuming disposition. The destination is already
    /// reserved and the actual normal owner already installed. No mutable
    /// caller receives an authority snapshot or a newly acquired lease.
    pub(crate) fn dispose_and_transfer_completed(&mut self) -> Result<(), FirstError> {
        if self.disposition_attempted {
            return Err(FirstError::StillFenced);
        }
        self.disposition_attempted = true;
        let result = self.dispose_and_transfer_inner();
        if result.is_err() {
            self.available.store(false, Ordering::Release);
        }
        result
    }
    fn dispose_and_transfer_inner(&mut self) -> Result<(), FirstError> {
        let source = NativeSteadyCompletion {
            original: Arc::clone(&self.original),
            available: Arc::clone(&self.available),
        };
        source.recheck().map_err(|_| FirstError::StillFenced)?;
        {
            let mut slot = self.original.lock().map_err(|_| FirstError::StillFenced)?;
            let RecoveryHeld {
                lock,
                boundary,
                host,
                engine,
                facts,
                ..
            } = slot.as_mut().ok_or(FirstError::StillFenced)?;
            let facts = facts.as_ref().ok_or(FirstError::StillFenced)?;
            let mut origin = NativeRecoveryOrigin {
                lock: lock.get().ok_or(FirstError::StillFenced)?,
                boundary: boundary.as_ref().ok_or(FirstError::StillFenced)?,
                host: host.as_mut().ok_or(FirstError::StillFenced)?,
                paths: &facts.paths,
                desired_paths: &facts.desired_paths,
                marker: facts.marker.clone(),
                desired: facts.desired.clone(),
                marker_bytes: facts.marker_bytes.clone(),
                desired_bytes: facts.desired_bytes.clone(),
                login_bytes: facts.login_bytes.clone(),
                uid: facts.uid,
            };
            engine.dispose_native_completed(&mut origin)?;
        }
        source.recheck().map_err(|_| FirstError::StillFenced)?;
        let mut original = self.original.lock().map_err(|_| FirstError::StillFenced)?;
        let mut destination = self
            .destination
            .lock()
            .map_err(|_| FirstError::StillFenced)?;
        if destination.is_some()
            || !original
                .as_ref()
                .is_some_and(|held| held.engine.disposition_ready())
        {
            return Err(FirstError::StillFenced);
        }
        let target = NativeSteadyCompletion {
            original: Arc::clone(&self.destination),
            available: Arc::clone(&self.available),
        };
        self.normal_owner
            .as_mut()
            .ok_or(FirstError::StillFenced)?
            .transfer_native_completion(&source, target)
            .map_err(|_| FirstError::StillFenced)?;
        // Destination is installed BEFORE this single nonfallible move. No
        // callbacks, assertions, allocation, unlock, Drop or later fallible
        // operation interposes between source consumption and destination.
        move_prechecked_original(&mut original, &mut destination);
        Ok(())
    }
    pub(crate) fn completed_status(&mut self) -> Result<serde_json::Value, FirstError> {
        self.dispatch_completed_read(crate::production_owner::NativeCompletedRead::Status)
    }
    pub(crate) fn completed_store(&mut self) -> Result<serde_json::Value, FirstError> {
        self.dispatch_completed_read(crate::production_owner::NativeCompletedRead::Store)
    }
    pub(crate) fn dispatch_completed_read(
        &mut self,
        request: crate::production_owner::NativeCompletedRead,
    ) -> Result<serde_json::Value, FirstError> {
        self.normal_owner
            .as_mut()
            .ok_or(FirstError::StillFenced)?
            .dispatch_native_completed_read(request)
            .map_err(|_| FirstError::StillFenced)
    }
    #[cfg(test)]
    pub(crate) fn original_leases_held(&self, paths: &CutoverPaths, uid: u32) -> bool {
        self.attempted
            && self.original.lock().is_ok_and(|slot| {
                let Some(held) = slot.as_ref() else {
                    return false;
                };
                held.lock
                    .get()
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
            preview_binding: None,
            original: Arc::new(Mutex::new(Some(RecoveryHeld {
                lock: Arc::new(OnceLock::new()),
                failed_lock: None,
                scope: Arc::new(AtomicBool::new(false)),
                boundary: Some(Boundary::reserve_installed()?),
                authenticated: None,
                host: None,
                engine: crate::manager_actor_service::NativeEngine::reserve(),
                facts: None,
            }))),
            destination: Arc::new(Mutex::new(None)),
            available: Arc::new(AtomicBool::new(true)),
            disposition_attempted: false,
            attempted: false,
            released: false,
            old_quiescence: None,
            normal_owner: None,
        })
    }
    /// Private developer issuer. The supplied paths are positively bound anew;
    pub(crate) fn reserve_previewed(
        preview: crate::restore_recovery_preview::Preview,
    ) -> Result<Self, FirstError> {
        if !crate::product_scope::restore_enabled() {
            return Err(FirstError::Admission);
        }
        let mut recovery = Self::reserve()?;
        recovery.preview_binding = Some(preview);
        Ok(recovery)
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
        self.reconcile(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::MixedIntent,
        )
    }
    /// New authentication plus the existing released original operation and
    /// singleton locks. OLD/Intent ends AbortedStillFenced, never ordinary owner.
    pub(crate) fn reconcile_old_intent(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        self.reconcile(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::OldIntent,
        )
    }
    pub(crate) fn complete_aborted(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        self.reconcile(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::CompleteAborted,
        )
    }
    /// One consumer progression: qualify the stopped fixed service under the
    /// original leases, authenticate OLD/Intent-or-Aborted, and complete it.
    /// No normal owner is installed and no service is started by this method.
    pub(crate) fn recover_old_to_completion(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::RecoverOldToCompletion,
        )
    }
    /// Exact mixed Intent is rolled back under retained locks, then completed OLD.
    pub(crate) fn recover_mixed_to_completion(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::MixedIntentToOldCompletion,
        )
    }
    /// Strict complete Intent + Committed + authenticated staged/live NEW only.
    /// This does not infer or repair partial retirement/disposition evidence.
    pub(crate) fn recover_committed_to_completion(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::CommittedNewToCompletion,
        )
    }
    /// New closure-only OLD context. No previous engine, receipt FD, terminal
    /// origin or completed permission is imported from the lost invocation.
    pub(crate) fn recover_final_old_output(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        if crate::product_scope::backup_only() {
            return Err(FirstError::Admission);
        }
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::FinalOldOutput,
        )
    }
    pub(crate) fn recover_final_new_output(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        if crate::product_scope::backup_only() {
            return Err(FirstError::Admission);
        }
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::FinalNewOutput,
        )
    }
    pub(crate) fn recover_new_ticket_output(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        if !crate::product_scope::restore_enabled() {
            return Err(FirstError::Admission);
        }
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::NewTicketOutput,
        )
    }
    pub(crate) fn recover_new_complete_output(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
    ) -> Result<(), FirstError> {
        if !crate::product_scope::restore_enabled() {
            return Err(FirstError::Admission);
        }
        self.recover_to_completion(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            FreshRecoveryMode::NewCompleteOutput,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn recover_to_completion(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
        mode: FreshRecoveryMode,
    ) -> Result<(), FirstError> {
        let result = self.reconcile(
            source,
            passphrase,
            paths,
            desired_paths,
            host_paths,
            uid,
            mode,
        );
        if result.is_err() {
            self.available.store(false, Ordering::Release);
            if let Ok(mut slot) = self.original.lock()
                && let Some(held) = slot.as_mut()
            {
                held.engine.revoke_native();
            }
        }
        result
    }
    #[cfg(test)]
    pub(crate) fn completed_owner_readonly(&mut self) -> bool {
        let Some(owner) = self.normal_owner.as_mut() else {
            return false;
        };
        owner.actual() == crate::lifecycle::ActualState::Disconnected
            && owner.rust_ownership_available()
            && owner
                .desired()
                .is_ok_and(|desired| !desired.connected && desired.profile_id.is_empty())
            && owner
                .list_projection()
                .is_ok_and(|store| store.profiles().is_empty())
            && !owner.login_ready()
            && owner
                .dispatch_native_completed_read(
                    crate::production_owner::NativeCompletedRead::Status,
                )
                .is_ok_and(|v| {
                    v["disconnected"] == true
                        && v["desiredOff"] == true
                        && v["ownership"] == true
                        && v["loginReady"] == false
                })
            && owner
                .dispatch_native_completed_read(crate::production_owner::NativeCompletedRead::Store)
                .is_ok_and(|v| v["profiles"] == 0 && v["subscriptions"] == 0)
    }
    #[allow(clippy::too_many_arguments)] // private closed entry modes, no public selector
    fn reconcile(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        paths: CutoverPaths,
        desired_paths: DesiredPaths,
        host_paths: NativeHostPaths,
        uid: u32,
        mode: FreshRecoveryMode,
    ) -> Result<(), FirstError> {
        if self.attempted {
            return Err(FirstError::StillFenced);
        }
        self.attempted = true; // before authentication/lease/capture; no reuse
        let completion = matches!(
            mode,
            FreshRecoveryMode::RecoverOldToCompletion
                | FreshRecoveryMode::CommittedNewToCompletion
                | FreshRecoveryMode::MixedIntentToOldCompletion
                | FreshRecoveryMode::FinalOldOutput
                | FreshRecoveryMode::FinalNewOutput
                | FreshRecoveryMode::NewTicketOutput
                | FreshRecoveryMode::NewCompleteOutput
        );
        // Fixed development-driver phases only: no paths, archive data, errors
        // or credentials. Diagnostics must never alter recovery success.
        let phase = |label: &'static [u8]| {
            if completion && self.preview_binding.is_none() {
                use std::io::Write;
                let _ = std::io::stderr().write_all(label);
            }
        };
        phase(b"Restore recovery: authenticate\n");
        let mut slot = self.original.lock().map_err(|_| FirstError::StillFenced)?;
        let held = slot.as_mut().ok_or(FirstError::StillFenced)?;
        let (authenticated, ciphertext) =
            crate::backup_destination_candidate::open_existing_with_digest(source, uid, passphrase)
                .map_err(|_| FirstError::Prepare)?;
        held.authenticated = Some(authenticated);
        if let Some(expected) = &self.preview_binding {
            use crate::restore_recovery_preview::Choice;
            let wanted = match expected.choice {
                Choice::Old => FreshRecoveryMode::RecoverOldToCompletion,
                Choice::Mixed => FreshRecoveryMode::MixedIntentToOldCompletion,
                Choice::Committed => FreshRecoveryMode::CommittedNewToCompletion,
                Choice::FinalOld => FreshRecoveryMode::FinalOldOutput,
                Choice::FinalNew => FreshRecoveryMode::FinalNewOutput,
                Choice::NewTicket => FreshRecoveryMode::NewTicketOutput,
                Choice::NewComplete => FreshRecoveryMode::NewCompleteOutput,
            };
            if ciphertext != expected.ciphertext || mode != wanted {
                return Err(FirstError::CiphertextMismatch);
            }
        }
        phase(b"Restore recovery: fixed paths\n");
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
        phase(b"Restore recovery: lock\n");
        let reported =
            MigrationLock::acquire_existing(&paths, uid).map_err(|_| FirstError::Admission)?;
        if let Err(reported) = held.lock.set(reported) {
            // This private single-writer slot was empty before acquisition;
            // retain any positively reported value if that invariant fails.
            held.failed_lock = Some(reported);
            return Err(FirstError::StillFenced);
        }
        phase(b"Restore recovery: boundary\n");
        if let Some(expected) = &self.preview_binding {
            let observed = crate::restore_recovery_preview::inspect_authenticated(
                &host_paths.config_directory,
                &paths,
                uid,
                held.lock.get().ok_or(FirstError::Admission)?,
                held.authenticated.as_ref().ok_or(FirstError::Prepare)?,
                ciphertext,
            )
            .map_err(|_| FirstError::Admission)?;
            if !expected.same(&observed) {
                return Err(FirstError::Admission);
            }
        }
        Boundary::capture_native_paths(&paths, &host_paths.store, uid, &mut held.boundary)?;
        let normal_paths = NativeHostPaths::new(
            host_paths.core.clone(),
            host_paths.data_directory.clone(),
            host_paths.config_directory.clone(),
            host_paths.runtime_directory.clone(),
            host_paths.proc_root.clone(),
            host_paths.sys_class_net.clone(),
        );
        let normal_store = normal_paths.store.clone();
        phase(b"Restore recovery: host\n");
        held.host = Some(
            ObservationOnlyNativeHost::new(host_paths, uid).map_err(|_| FirstError::Admission)?,
        );
        phase(b"Restore recovery: marker and desired\n");
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
            facts,
            ..
        } = held;
        let boundary = boundary.as_ref().ok_or(FirstError::Admission)?;
        let original_member = |index: usize, maximum| {
            boundary
                .members
                .get(index)
                .and_then(|pin| pin.held.as_ref())
                .map(|(file, _)| held_bytes(file, maximum))
                .transpose()
        };
        phase(b"Restore recovery: retained origin\n");
        let mut origin = NativeRecoveryOrigin {
            lock: lock.get().ok_or(FirstError::Admission)?,
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
        let backup = authenticated.as_ref().ok_or(FirstError::Admission)?;
        if matches!(
            mode,
            FreshRecoveryMode::RecoverOldToCompletion
                | FreshRecoveryMode::CommittedNewToCompletion
                | FreshRecoveryMode::MixedIntentToOldCompletion
                | FreshRecoveryMode::FinalOldOutput
                | FreshRecoveryMode::FinalNewOutput
                | FreshRecoveryMode::NewTicketOutput
                | FreshRecoveryMode::NewCompleteOutput
        ) {
            phase(b"Restore recovery: stopped service\n");
            // Only the authenticated, same-graph consumer preview above selects
            // the ordinary TUI invocation. Legacy CLI and Abort guards retain
            // their exact argument grammar and all stopped-owner observations.
            let prepare = if self.preview_binding.is_some() {
                NativeOldRecoveryQuiescence::prepare_consumer
            } else {
                NativeOldRecoveryQuiescence::prepare
            };
            self.old_quiescence = Some(Box::new(prepare(&paths, uid, Arc::clone(lock))?));
            let quiescence = self.old_quiescence.as_mut().ok_or(FirstError::Admission)?;
            phase(b"Restore recovery: engine\n");
            match mode {
                FreshRecoveryMode::NewCompleteOutput => {
                    engine.qualify_native_new_complete(&mut origin, backup, quiescence)?
                }
                FreshRecoveryMode::NewTicketOutput => {
                    engine.qualify_native_new_ticket(&mut origin, backup, quiescence)?
                }
                FreshRecoveryMode::FinalNewOutput => {
                    engine.qualify_native_final_new(&mut origin, backup, quiescence)?
                }
                FreshRecoveryMode::FinalOldOutput => {
                    engine.qualify_native_final_old(&mut origin, backup, quiescence)?
                }
                FreshRecoveryMode::MixedIntentToOldCompletion => {
                    engine.recover_native_mixed_to_completion(&mut origin, backup, quiescence)?
                }
                FreshRecoveryMode::RecoverOldToCompletion => {
                    engine.recover_native_old_to_completion(&mut origin, backup, quiescence)?
                }
                FreshRecoveryMode::CommittedNewToCompletion => engine
                    .recover_native_committed_to_completion(&mut origin, backup, quiescence)?,
                _ => return Err(FirstError::Admission),
            }
            *facts = Some(OriginFacts {
                paths: paths.clone(),
                desired_paths: desired_paths.clone(),
                marker: origin.marker.clone(),
                desired: origin.desired.clone(),
                marker_bytes: origin.marker_bytes.clone(),
                desired_bytes: origin.desired_bytes.clone(),
                login_bytes: origin.login_bytes.clone(),
                uid,
            });
            Ok(())
        } else if mode == FreshRecoveryMode::CompleteAborted {
            engine.complete_native_aborted(&mut origin, backup)?;
            let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let proof = NativeCompletedOff {
                origin: &mut origin,
                engine,
                until,
            };
            let mut admission = crate::startup_admission::StartupAdmission::NativeCompleted(proof);
            crate::production_owner::ProductionNativeOwner::initialize_native_completed_installed(
                &mut self.normal_owner,
                || {
                    crate::native_host::NativeLifecycleHost::new_retained_completion(
                        normal_paths,
                        uid,
                    )
                    .map_err(|_| crate::production_owner::ProductionOwnerError::HostUnavailable)
                },
                desired_paths.clone(),
                &normal_store,
                paths.clone(),
                uid,
                lock.get().ok_or(FirstError::Admission)?,
                &mut admission,
            )
            .map_err(|_| FirstError::StillFenced)?;
            *facts = Some(OriginFacts {
                paths: paths.clone(),
                desired_paths: desired_paths.clone(),
                marker: origin.marker.clone(),
                desired: origin.desired.clone(),
                marker_bytes: origin.marker_bytes.clone(),
                desired_bytes: origin.desired_bytes.clone(),
                login_bytes: origin.login_bytes.clone(),
                uid,
            });
            self.normal_owner
                .as_mut()
                .ok_or(FirstError::StillFenced)?
                .install_native_completion(NativeSteadyCompletion {
                    original: Arc::clone(&self.original),
                    available: Arc::clone(&self.available),
                })
                .map_err(|_| FirstError::StillFenced)?;
            Ok(())
        } else if mode == FreshRecoveryMode::OldIntent {
            engine.reconcile_native_old_intent(&mut origin, backup)
        } else {
            engine.reconcile_native_mixed(&mut origin, backup)
        }
    }
}
