// SPDX-License-Identifier: MIT
//! Fresh process reconciliation of the positively completed matrix22 MIXED case.
use super::*;

const RECOVERY_ROOT: &str =
    "/home/kdk_vm/.cache/t4-native-retained-owner-review22/after-first-rename";
const RECOVERY_RUNTIME: &str = "/run/user/1000/t4n22/after-first-rename";

#[test]
#[ignore = "ROOT-only NEW original authenticated recovery after matrix22 original exit0"]
fn isolated_installed_host_fresh_intent_mixed_rollback() {
    assert!(
        identity(
            Uid::current().as_raw(),
            nix::unistd::geteuid().as_raw(),
            std::env::var_os("HOME").as_deref(),
            std::env::var(OPT_IN).ok().as_deref()
        ),
        "fixed_native_vm_identity_refused"
    );
    assert!(
        std::env::var_os("OMAVLESS_HOME").is_none()
            && std::env::var_os("XDG_RUNTIME_DIR").as_deref()
                == Some(std::ffi::OsStr::new("/run/user/1000")),
        "fixed_native_vm_environment_refused"
    );
    let until = Instant::now() + Duration::from_secs(90);
    let root = Path::new(RECOVERY_ROOT);
    let runtime = Path::new(RECOVERY_RUNTIME);
    let config = root.join("home/.config/omavless");
    let desired = DesiredPaths::below(&root.join("state"));
    let paths = CutoverPaths::below(runtime, &root.join("state"), UID);
    assert!(
        [root, runtime, config.as_path(), desired.directory.as_path()]
            .into_iter()
            .all(private_directory),
        "fixed_native_vm_private_paths_refused"
    );
    let mut recovery = crate::native_coordinator::FreshRecovery::reserve()
        .expect("fixed_native_vm_recovery_reservation_refused");
    let result = recovery.reconcile_mixed(
        &root.join("input.ovb"),
        PASSWORD,
        paths.clone(),
        desired,
        NativeHostPaths::new(
            Path::new(CORE).into(),
            config.clone(),
            config,
            runtime.join("omavless"),
            Path::new("/proc").into(),
            Path::new("/sys/class/net").into(),
        ),
        UID,
    );
    let original = recovery.original_leases_held(&paths, UID);
    let operation_busy = MigrationLock::acquire_existing(&paths, UID).is_err();
    let second_reservation_refused = crate::native_coordinator::FreshRecovery::reserve().is_err();
    let complete = result.is_ok()
        && original
        && operation_busy
        && second_reservation_refused
        && Instant::now() < until;
    std::mem::forget(recovery); // before output/final assertion; never Drop uncertain resources
    assert!(complete, "fixed_native_vm_recovery_continuation_refused");
    println!("T4_NATIVE_NEW_AUTHENTICATED_INTENT_MIXED_ABORTED_STILL_FENCED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}
