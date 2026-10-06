// SPDX-License-Identifier: MIT
//! Three separate ORIGINAL0 gates on ONE NEW private fixture. No auto progression.
use super::*;

const DISPOSITION_ROOT: &str = "/home/kdk_vm/.cache/t4-native-disposition-review25/operation";
const DISPOSITION_RUNTIME: &str = "/run/user/1000/t4n25/operation";

fn selected() {
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
}
fn recovery_paths() -> (CutoverPaths, DesiredPaths, NativeHostPaths) {
    let root = Path::new(DISPOSITION_ROOT);
    let runtime = Path::new(DISPOSITION_RUNTIME);
    let config = root.join("home/.config/omavless");
    (
        CutoverPaths::below(runtime, &root.join("state"), UID),
        DesiredPaths::below(&root.join("state")),
        NativeHostPaths::new(
            Path::new(CORE).into(),
            config.clone(),
            config,
            runtime.join("omavless"),
            Path::new("/proc").into(),
            Path::new("/sys/class/net").into(),
        ),
    )
}

#[test]
#[ignore = "ROOT-only NEW25 real owner fixed MIXED producer; original0 before any later phase"]
fn isolated_native_disposition_mixed_producer() {
    selected();
    let (limit, _) =
        nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE).unwrap();
    // Reuse the reviewed fixed three-slot reserve; only one case acquires any
    // originals, all others stay empty. Nothing uncertain is evicted to fit.
    let mut aggregate = fault_matrix::reserve_aggregate(limit, || {
        crate::native_coordinator::NativeVmReservation::reserve_vm().map_err(|_| ())
    })
    .expect("fixed_native_vm_matrix_reservation_refused");
    let until = Instant::now() + Duration::from_secs(90);
    let root = Path::new(DISPOSITION_ROOT);
    let Some((cutover, desired)) = fault_matrix::construct(
        &mut aggregate[0],
        root,
        Path::new(DISPOSITION_RUNTIME),
        until,
    ) else {
        panic!("fixed_native_vm_matrix_construct_refused");
    };
    let config = root.join("home/.config/omavless");
    let old_store = fault_matrix::metadata_bound_read(
        &config.join("profiles.json"),
        omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
    )
    .unwrap();
    let old_template = fault_matrix::metadata_bound_read(
        &config.join("route-template.yaml"),
        omavless_domain::config::MAX_TEMPLATE_BYTES,
    )
    .unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&old_store)
            .is_ok_and(|store| store["onboardingComplete"] == false),
        "fixed_native_vm_disposition_fixture_refused"
    );
    let archive =
        crate::backup_destination_candidate::open_existing(&root.join("input.ovb"), UID, PASSWORD)
            .unwrap();
    let new_store = archive.restore_store_off().unwrap();
    assert!(
        old_store != new_store && old_template.as_slice() != archive.template(),
        "fixed_native_vm_disposition_fixture_refused"
    );
    let owner = aggregate[0].owner.as_mut().unwrap();
    let hit = owner
        .coordinator
        .retained_vm_fault(&root.join("input.ovb"), PASSWORD, 2);
    let held = hit && owner.coordinator.retained_vm_custody();
    let denied = held && owner.coordinator.retained_vm_ordinary_and_recovery_denied();
    let busy = denied && MigrationLock::acquire_existing(&cutover, UID).is_err();
    let factual = busy
        && fault_matrix::pending_readback(
            root,
            2,
            [&old_store, &old_template],
            [&new_store, archive.template()],
            &desired,
        );
    std::mem::forget(aggregate);
    assert!(
        hit && held && denied && busy && factual && Instant::now() < until,
        "fixed_native_vm_disposition_producer_refused"
    );
    println!("T4_NATIVE_DISPOSITION_NEW_MIXED_PRODUCER_COMPLETED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}

#[test]
#[ignore = "ROOT-only AFTER exact new25 producer original0; fresh authenticated rollback"]
fn isolated_native_disposition_new_authenticated_rollback() {
    selected();
    let until = Instant::now() + Duration::from_secs(90);
    let (paths, desired, host) = recovery_paths();
    let mut recovery = crate::native_coordinator::FreshRecovery::reserve()
        .expect("fixed_native_vm_recovery_reservation_refused");
    let result = recovery.reconcile_mixed(
        &Path::new(DISPOSITION_ROOT).join("input.ovb"),
        PASSWORD,
        paths.clone(),
        desired,
        host,
        UID,
    );
    let held = result.is_ok() && recovery.original_leases_held(&paths, UID);
    let busy = held && MigrationLock::acquire_existing(&paths, UID).is_err();
    std::mem::forget(recovery);
    assert!(
        result.is_ok() && held && busy && Instant::now() < until,
        "fixed_native_vm_recovery_continuation_refused"
    );
    println!("T4_NATIVE_DISPOSITION_NEW_ABORTED_RECOVERY_COMPLETED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}

#[test]
#[ignore = "ROOT-only AFTER new25 rollback original0; consuming completion plus real ordinary store mutation"]
fn isolated_native_disposition_completion_and_ordinary_onboarding() {
    selected();
    let until = Instant::now() + Duration::from_secs(90);
    let (paths, desired, host) = recovery_paths();
    let mut recovery = crate::native_coordinator::FreshRecovery::reserve()
        .expect("fixed_native_vm_recovery_reservation_refused");
    let complete = recovery
        .complete_aborted(
            &Path::new(DISPOSITION_ROOT).join("input.ovb"),
            PASSWORD,
            paths.clone(),
            desired,
            host,
            UID,
        )
        .is_ok();
    let transfer = complete && recovery.dispose_and_transfer_completed().is_ok();
    let request = serde_json::json!({"api":"omavless.control","version":1,"id":"disposition-onboarding",
        "method":"onboarding.complete","params":{"operationId":"disposition-onboarding","expectedRevision":0}});
    let mutation = transfer
        && matches!(recovery.completed_owner_onboarding(&request),
        Ok(crate::native_coordinator::NativeOwnerExecution::Applied { outcome: Ok(
            crate::native_coordinator::NativeMutationOutcome::Profile(outcome)), .. }) if outcome.changed);
    let readonly = mutation && recovery.completed_owner_readonly();
    let raw = if readonly {
        fault_matrix::metadata_bound_read(
            &Path::new(DISPOSITION_ROOT).join("home/.config/omavless/profiles.json"),
            omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
        )
    } else {
        None
    };
    let store = raw
        .as_deref()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
        .is_some_and(|store| {
            store["onboardingComplete"] == true
                && store["profiles"].as_array().is_some_and(Vec::is_empty)
        });
    let no_pending =
        store && !crate::pending_private_transaction::pending_at(&paths.state_directory);
    let busy = no_pending && MigrationLock::acquire_existing(&paths, UID).is_err();
    std::mem::forget(recovery);
    assert!(
        complete
            && transfer
            && mutation
            && readonly
            && store
            && no_pending
            && busy
            && Instant::now() < until,
        "fixed_native_vm_disposition_mutation_refused"
    );
    println!("T4_NATIVE_DISPOSITION_CONSUMED_ORDINARY_ONBOARDING_COMPLETED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}
