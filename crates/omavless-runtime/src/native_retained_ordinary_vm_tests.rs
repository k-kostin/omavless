// SPDX-License-Identifier: MIT
//! Four fixed ORIGINAL0 phases on a fresh fixture. History is never an issuer.
use super::*;
const ORDINARY_ROOT: &str = "/home/kdk_vm/.cache/t4-native-ordinary-review26/operation";
const ORDINARY_RUNTIME: &str = "/run/user/1000/t4n26/operation";

#[test]
#[ignore = "ROOT-only NEW26 producer; original0 prerequisite for rollback"]
fn isolated_native_ordinary_mixed_producer() {
    disposition::mixed_producer(Path::new(ORDINARY_ROOT), Path::new(ORDINARY_RUNTIME));
}
#[test]
#[ignore = "ROOT-only AFTER NEW26 producer original0; new authenticated rollback"]
fn isolated_native_ordinary_authenticated_rollback() {
    disposition::authenticated_rollback(Path::new(ORDINARY_ROOT), Path::new(ORDINARY_RUNTIME));
}

fn request(method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"api":"omavless.control","version":1,"id":"ordinary-26","method":method,"params":params})
}
fn applied(
    execution: crate::native_coordinator::NativeOwnerExecution,
    revision: u64,
    changed: bool,
) -> bool {
    matches!(execution, crate::native_coordinator::NativeOwnerExecution::Applied {
        cached, outcome: Ok(crate::native_coordinator::NativeMutationOutcome::Profile(outcome))
    } if cached.revision == revision && outcome.changed == changed)
}
fn ordinary_reads(owner: &mut ProductionNativeOwner) -> bool {
    owner.actual() == ActualState::Disconnected
        && owner.rust_ownership_available()
        && owner
            .desired()
            .is_ok_and(|desired| !desired.connected && desired.profile_id.is_empty())
        && owner
            .list_projection()
            .is_ok_and(|store| store.profiles().is_empty() && store.subscriptions().is_empty())
        && !owner.login_ready()
}

#[test]
fn ordinary_vm_requests_use_existing_closed_parsers_without_live_startup() {
    for (id, revision, mode) in [
        ("ordinary-startup", 1, "global"),
        ("ordinary-startup-two", 2, "rule"),
        ("restart-startup", 0, "global"),
    ] {
        let value = request(
            "startup.configure",
            serde_json::json!({"enabled":false,"target":"last","profileId":"","mode":mode,"operationId":id,"expectedRevision":revision}),
        );
        let parsed = crate::startup_protocol::parse_startup_request(&value).unwrap();
        assert!(!parsed.preferences.enabled);
        assert_eq!(parsed.expected_revision, Some(revision));
    }
    for (id, revision) in [("ordinary-onboarding", 0), ("ordinary-no-change", 3)] {
        assert!(
            crate::onboarding_protocol::parse(&request(
                "onboarding.complete",
                serde_json::json!({"operationId":id,"expectedRevision":revision})
            ))
            .is_ok()
        );
    }
    assert!(
        ORDINARY_ROOT.ends_with("/review26/operation")
            || ORDINARY_ROOT.ends_with("-review26/operation")
    );
    assert_eq!(ORDINARY_RUNTIME, "/run/user/1000/t4n26/operation");
}

#[test]
#[ignore = "ROOT-only AFTER NEW26 rollback original0; consuming same lease and ordinary repeated mutations"]
fn isolated_native_ordinary_repeated_mutations() {
    disposition::selected();
    let until = Instant::now() + Duration::from_secs(90);
    let root = Path::new(ORDINARY_ROOT);
    let (paths, desired, host) = disposition::recovery_paths_at(root, Path::new(ORDINARY_RUNTIME));
    let mut recovery = crate::native_coordinator::FreshRecovery::reserve()
        .expect("fixed_native_vm_recovery_reservation_refused");
    let complete = recovery
        .complete_aborted(
            &root.join("input.ovb"),
            PASSWORD,
            paths.clone(),
            desired,
            host,
            UID,
        )
        .is_ok();
    let transfer = complete && recovery.dispose_and_transfer_completed().is_ok();
    let activated = transfer && recovery.activate_ordinary_owner().is_ok();
    let original_desired = if activated {
        fault_matrix::metadata_bound_read(
            &paths.state_directory.join("desired.json"),
            crate::desired::MAX_DESIRED_STATE_BYTES as usize,
        )
    } else {
        None
    };
    let success = if activated && original_desired.is_some() {
        let owner = recovery.ordinary_owner().unwrap();
        let onboarding = request(
            "onboarding.complete",
            serde_json::json!({"operationId":"ordinary-onboarding","expectedRevision":0}),
        );
        let first = owner
            .coordinator
            .execute_onboarding(&onboarding)
            .is_ok_and(|v| applied(v, 1, true));
        let startup = request(
            "startup.configure",
            serde_json::json!({"enabled":false,"target":"last","profileId":"","mode":"global","operationId":"ordinary-startup","expectedRevision":1}),
        );
        let second = first
            && owner
                .coordinator
                .execute_startup(&startup)
                .is_ok_and(|v| applied(v, 2, true));
        let replay = second
            && matches!(owner.coordinator.execute_startup(&startup), Ok(crate::native_coordinator::NativeOwnerExecution::Replay(ref cached)) if cached.revision == 2);
        let third = request(
            "startup.configure",
            serde_json::json!({"enabled":false,"target":"last","profileId":"","mode":"rule","operationId":"ordinary-startup-two","expectedRevision":2}),
        );
        let changed = replay
            && owner
                .coordinator
                .execute_startup(&third)
                .is_ok_and(|v| applied(v, 3, true));
        let no_change = request(
            "onboarding.complete",
            serde_json::json!({"operationId":"ordinary-no-change","expectedRevision":3}),
        );
        changed
            && owner
                .coordinator
                .execute_onboarding(&no_change)
                .is_ok_and(|v| applied(v, 3, false))
            && ordinary_reads(owner)
    } else {
        false
    };
    let unchanged = success
        && fault_matrix::metadata_bound_read(
            &paths.state_directory.join("desired.json"),
            crate::desired::MAX_DESIRED_STATE_BYTES as usize,
        ) == original_desired;
    let store = unchanged
        && fault_matrix::metadata_bound_read(
            &root.join("home/.config/omavless/profiles.json"),
            omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
        )
        .is_some_and(|raw| {
            serde_json::from_slice::<serde_json::Value>(&raw).is_ok_and(|v| {
                v["onboardingComplete"] == true
                    && v["startup"]["enabled"] == false
                    && v["startup"]["mode"] == "rule"
            })
        });
    let busy = store && MigrationLock::acquire_existing(&paths, UID).is_err();
    std::mem::forget(recovery);
    assert!(
        complete
            && transfer
            && activated
            && success
            && unchanged
            && store
            && busy
            && Instant::now() < until,
        "fixed_native_vm_ordinary_mutations_refused"
    );
    println!("T4_NATIVE_ORIGINAL_LEASE_REPEATED_ORDINARY_MUTATIONS_COMPLETED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}

// Retains every reported original on error/unwind; no guessed cleanup. This
// process uses only ordinary bind/initialize, never recovery or history decode.
struct RestartOriginals {
    server: Option<crate::RuntimeServer>,
    owner: Option<ProductionNativeOwner>,
}
impl Drop for RestartOriginals {
    fn drop(&mut self) {
        std::mem::forget(self.owner.take());
        std::mem::forget(self.server.take());
    }
}
#[test]
#[ignore = "ROOT-only AFTER NEW26 repeated mutation original0; independently admitted ordinary restart"]
fn isolated_native_ordinary_independent_restart() {
    disposition::selected();
    let until = Instant::now() + Duration::from_secs(90);
    let root = Path::new(ORDINARY_ROOT);
    let runtime = Path::new(ORDINARY_RUNTIME);
    let config = root.join("home/.config/omavless");
    let (paths, desired, host_paths) = disposition::recovery_paths_at(root, runtime);
    let mut originals = RestartOriginals {
        server: None,
        owner: None,
    };
    assert!(
        !crate::pending_private_transaction::pending_at(&paths.state_directory),
        "fixed_native_vm_ordinary_restart_refused"
    );
    let saved = read_desired_snapshot(&desired, UID).unwrap();
    assert!(
        !saved.connected && saved.profile_id.is_empty(),
        "fixed_native_vm_ordinary_restart_refused"
    );
    // The transition observer requires an absent control.sock and therefore
    // cannot classify the preceding ordinary process's stale endpoint. Normal
    // bind owns singleton exclusion/removal; do not waive that observer or
    // borrow the former endpoint. The real units and native host are still
    // checked before the unchanged normal bind/initialize entry.
    for service in [
        crate::production_observation::LEGACY_SERVICE,
        crate::production_observation::RUST_SERVICE,
    ] {
        let state = crate::production_observation::service_state_with_timeout(
            Path::new("/usr/bin/systemctl"),
            service,
            Duration::from_millis(250),
        )
        .expect("fixed_native_vm_ordinary_restart_refused");
        assert!(
            !state.active
                && state.main_pid == 0
                && state.exit_status == 0
                && state.result == "success",
            "fixed_native_vm_ordinary_restart_refused"
        );
    }
    let mut host = NativeLifecycleHost::new(host_paths, UID)
        .expect("fixed_native_vm_ordinary_restart_refused");
    assert!(
        host.fresh_observation(&saved).is_ok_and(empty),
        "fixed_native_vm_ordinary_restart_refused"
    );
    // This is the unchanged normal singleton policy, after the preceding
    // original process exited0; no borrowed former FD/lock/socket authority.
    originals.server = Some(
        crate::RuntimeServer::bind(RuntimePaths::below(runtime))
            .expect("fixed_native_vm_ordinary_restart_refused"),
    );
    originals.owner = Some(
        ProductionNativeOwner::initialize(host, desired, &config.join("profiles.json"), paths, UID)
            .expect("fixed_native_vm_ordinary_restart_refused"),
    );
    let owner = originals.owner.as_mut().unwrap();
    let startup = ordinary_reads(owner) && !owner.startup_outcome().changed;
    let request = request(
        "startup.configure",
        serde_json::json!({"enabled":false,"target":"last","profileId":"","mode":"global","operationId":"restart-startup","expectedRevision":0}),
    );
    let mutation = startup
        && owner
            .coordinator
            .execute_startup(&request)
            .is_ok_and(|v| applied(v, 1, true))
        && ordinary_reads(owner);
    std::mem::forget(originals);
    assert!(
        startup && mutation && Instant::now() < until,
        "fixed_native_vm_ordinary_restart_refused"
    );
    println!("T4_NATIVE_INDEPENDENT_ORDINARY_RESTART_AND_MUTATION_COMPLETED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}
