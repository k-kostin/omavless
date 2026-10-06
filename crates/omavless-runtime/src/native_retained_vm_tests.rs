// SPDX-License-Identifier: MIT
//! Explicit isolated real-host owner composition. No installed-store mutation,
//! current() login/package claim, dispatcher registration or orphan cleanup.
use super::*;
use crate::desired::read_desired_snapshot;
use crate::production_observation::{ProductionObservationPaths, ProductionOwnershipObserver};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::time::{Duration, Instant};

const UID: u32 = 1000;
const HOME: &str = "/home/kdk_vm";
const ROOT: &str = "/home/kdk_vm/.cache/t4-native-retained-owner-review21";
const RUNTIME: &str = "/run/user/1000/t4n21";
const CORE: &str = "/usr/lib/omavless-dns/mihomo";
const OPT_IN: &str = "OMAVLESS_TEST_T4_NATIVE_RETAINED_VM";
const PASSWORD: &[u8] = b"public isolated native-owner fixture passphrase";
const PUBLIC_STORE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;

#[path = "native_retained_disposition_vm_tests.rs"]
mod disposition;
#[path = "native_retained_fault_vm_tests.rs"]
mod fault_matrix;
#[path = "native_retained_recovery_vm_tests.rs"]
mod fresh_recovery;
#[path = "native_retained_ordinary_vm_tests.rs"]
mod ordinary;
#[path = "native_registered_fixture_rpc_vm_tests.rs"]
mod registered_rpc;

#[test]
fn fixed_native_vm_public_archive_fixture_roundtrip_and_optional_export() {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let template = include_bytes!("../../../templates/default.yaml");
    let bytes = omavless_domain::private_backup::seal(PUBLIC_STORE, template, PASSWORD).unwrap();
    let opened = omavless_domain::private_backup::open(&bytes, PASSWORD).unwrap();
    assert!(opened.store() == PUBLIC_STORE && opened.template() == template);
    // Only public data. Explicit developer artifact output is separate from
    // the ignored VM's effect-bearing owner construction and cannot select it.
    if let Some(path) = std::env::var_os("OMAVLESS_TEST_T4_PUBLIC_ARCHIVE") {
        let path = std::path::PathBuf::from(path);
        assert!(path.is_absolute() && path.file_name() == Some("input.ovb".as_ref()));
        let parent = path.parent().unwrap();
        let metadata = fs::symlink_metadata(parent).unwrap();
        assert!(
            metadata.is_dir()
                && metadata.uid() == Uid::current().as_raw()
                && metadata.mode() & 0o7777 == 0o700
        );
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o400))
            .unwrap();
        assert!(fs::read(&path).unwrap() == bytes);
    }
}

#[test]
fn fixed_native_vm_real_reader_refuses_sealed_artifact_mode_but_accepts_private_input_mode() {
    use std::os::unix::fs::PermissionsExt;
    let root = crate::test_temp::directory_under(
        Path::new(&std::env::var_os("HOME").unwrap()),
        "t4-vm-input-mode",
    )
    .unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let file = root.join("input.ovb");
    let template = include_bytes!("../../../templates/default.yaml");
    let raw = omavless_domain::private_backup::seal(PUBLIC_STORE, template, PASSWORD).unwrap();
    fs::write(&file, raw).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o400)).unwrap();
    let uid = Uid::current().as_raw();
    assert!(crate::backup_destination_candidate::open_existing(&file, uid, PASSWORD).is_err());
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let opened = crate::backup_destination_candidate::open_existing(&file, uid, PASSWORD).unwrap();
    assert!(opened.store() == PUBLIC_STORE && opened.template() == template);
    fs::remove_file(file).unwrap();
    fs::remove_dir(root).unwrap();
}

fn identity(uid: u32, effective: u32, home: Option<&std::ffi::OsStr>, opt: Option<&str>) -> bool {
    uid == UID && effective == UID && home == Some(std::ffi::OsStr::new(HOME)) && opt == Some("1")
}
fn private_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|m| m.is_dir() && m.uid() == UID && m.mode() & 0o7777 == 0o700)
        && fs::canonicalize(path).ok().as_deref() == Some(path)
}
fn absent(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}
fn no_orphans(parent: &Path) -> bool {
    let Ok(entries) = fs::read_dir(parent) else {
        return false;
    };
    let mut count = 0;
    for entry in entries {
        count += 1;
        if count > 128 {
            return false;
        }
        let Ok(entry) = entry else {
            return false;
        };
        if entry.file_name().as_encoded_bytes().starts_with(b"probe-") {
            return false;
        }
    }
    true
}
fn empty(facts: crate::lifecycle::NativeLocalObservation) -> bool {
    !facts.owned_core_running
        && facts.visible_mihomo_count == 0
        && facts.owned_auxiliary_mihomo_count == 0
        && facts.visible_tun_count == 0
        && facts.managed_tun_count == 0
        && !facts.owned_controller_config_verified
        && !facts.desired_profile_matches_owned
}

fn observer_refusal(
    construction: bool,
    error: crate::production_observation::ProductionObservationError,
) -> &'static str {
    use crate::production_observation::ProductionObservationError as E;
    macro_rules! label {
        ($category:literal) => {
            if construction {
                concat!(
                    "fixed_native_vm_inventory_constructor_",
                    $category,
                    "_refused"
                )
            } else {
                concat!("fixed_native_vm_inventory_verify_", $category, "_refused")
            }
        };
    }
    match error {
        E::UnsafePath => label!("unsafe_path"),
        E::ServiceQuery => label!("service_query"),
        E::ServiceResponse => label!("service_response"),
        E::PrivateState => label!("private_state"),
        E::HostNotEmpty => label!("host_not_empty"),
        E::IncompleteInventory => label!("incomplete_inventory"),
        E::Cutover(_) => label!("cutover"),
    }
}

#[test]
fn fixed_native_vm_original_observer_error_projection_is_closed_and_stage_bound() {
    use crate::production_observation::ProductionObservationError as E;
    for (error, category) in [
        (E::UnsafePath, "unsafe_path"),
        (E::ServiceQuery, "service_query"),
        (E::ServiceResponse, "service_response"),
        (E::PrivateState, "private_state"),
        (E::HostNotEmpty, "host_not_empty"),
        (E::IncompleteInventory, "incomplete_inventory"),
        (E::Cutover(CutoverError::Io), "cutover"),
    ] {
        assert_eq!(
            observer_refusal(true, error),
            format!("fixed_native_vm_inventory_constructor_{category}_refused")
        );
        assert_eq!(
            observer_refusal(false, error),
            format!("fixed_native_vm_inventory_verify_{category}_refused")
        );
    }
}

#[test]
fn fixed_native_vm_identity_is_not_an_ambient_opt_in() {
    let h = Some(std::ffi::OsStr::new(HOME));
    assert!(identity(UID, UID, h, Some("1")));
    for (u, e, home, flag) in [
        (0, UID, h, Some("1")),
        (UID, 0, h, Some("1")),
        (UID, UID, None, Some("1")),
        (UID, UID, h, None),
        (UID, UID, h, Some("01")),
    ] {
        assert!(!identity(u, e, home, flag));
    }
}

#[test]
#[ignore = "ROOT-only fresh isolated fixture; real NativeLifecycleHost/private pair effects"]
fn isolated_installed_host_native_retained_pair_commit() {
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
        std::env::var_os("OMAVLESS_HOME").is_none(),
        "fixed_native_vm_environment_refused"
    );
    let until = Instant::now() + Duration::from_secs(90);
    let root = Path::new(ROOT);
    let fixture_home = root.join("home");
    let state_base = root.join("state");
    let config = fixture_home.join(".config/omavless");
    // Real systemctl uses the ambient user's systemd/private transport. Keep it
    // real while isolating only the explicitly constructed owner filesystem.
    assert!(
        std::env::var_os("XDG_RUNTIME_DIR").as_deref()
            == Some(std::ffi::OsStr::new("/run/user/1000")),
        "fixed_native_vm_runtime_binding_refused"
    );
    let runtime = RuntimePaths::below(Path::new(RUNTIME));
    let desired = DesiredPaths::below(&state_base);
    let cutover = CutoverPaths::below(Path::new(RUNTIME), &state_base, UID);
    assert!(
        [
            root,
            fixture_home.as_path(),
            fixture_home.join(".config").as_path(),
            config.as_path(),
            state_base.as_path(),
            desired.directory.as_path(),
            Path::new(RUNTIME),
            runtime.directory.as_path()
        ]
        .into_iter()
        .all(private_directory),
        "fixed_native_vm_private_paths_refused"
    );
    assert!(
        absent(&runtime.socket)
            && absent(&runtime.owner_lock)
            && no_orphans(Path::new(RUNTIME))
            && no_orphans(&runtime.directory),
        "fixed_native_vm_existing_runtime_refused"
    );
    let saved_desired = read_desired_snapshot(&desired, UID).expect("fixed_native_vm_off_refused");
    assert!(
        !saved_desired.connected && saved_desired.profile_id.is_empty(),
        "fixed_native_vm_off_refused"
    );
    let observed = ProductionObservationPaths::below(
        Path::new("/usr/bin/systemctl").to_owned(),
        &fixture_home,
        Path::new(RUNTIME),
        Path::new("/proc").to_owned(),
        Path::new("/sys/class/net").to_owned(),
        UID,
    );
    let observer = ProductionOwnershipObserver::new(observed, UID)
        .unwrap_or_else(|error| panic!("{}", observer_refusal(true, error)));
    observer
        .verify_native_empty()
        .unwrap_or_else(|error| panic!("{}", observer_refusal(false, error)));
    let core = fs::symlink_metadata(CORE).expect("fixed_native_vm_installed_core_refused");
    assert!(
        core.is_file()
            && core.uid() == 0
            && core.mode() & 0o022 == 0
            && core.mode() & 0o111 != 0
            && core.nlink() == 1,
        "fixed_native_vm_installed_core_refused"
    );
    let paths = NativeHostPaths::new(
        Path::new(CORE).to_owned(),
        config.clone(),
        config.clone(),
        runtime.directory.clone(),
        Path::new("/proc").to_owned(),
        Path::new("/sys/class/net").to_owned(),
    );
    let mut host = NativeLifecycleHost::new(paths, UID).expect("fixed_native_vm_real_host_refused");
    assert!(
        host.fresh_observation(&saved_desired).is_ok_and(empty),
        "fixed_native_vm_real_host_not_empty"
    );
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
    // Only this fresh fixture's actual singleton. No registration/serve loop.
    let server = crate::RuntimeServer::bind(runtime).expect("fixed_native_vm_singleton_refused");
    let owner = ProductionNativeOwner::initialize(
        host,
        desired,
        &config.join("profiles.json"),
        cutover.clone(),
        UID,
    );
    let mut owner = match owner {
        Ok(owner) => owner,
        Err(_) => {
            std::mem::forget(server);
            panic!("fixed_native_vm_owner_refused");
        }
    };
    let admitted = owner.actual() == ActualState::Disconnected
        && !owner.startup_outcome().changed
        && Instant::now() < until;
    let result = admitted
        && owner
            .coordinator
            .retained_vm_execute(&root.join("input.ovb"), PASSWORD);
    let held = owner.coordinator.retained_vm_custody();
    let denied = owner
        .coordinator
        .initialize_batch_operations("blocked-native-vm-entry")
        .is_err();
    let original_lease_busy = MigrationLock::acquire(&cutover, UID).is_err();
    let completed = result && held && denied && original_lease_busy && Instant::now() < until;
    // Keep the actual server singleton, owner and held execution through the
    // harness's original terminal return; never cleanup a refused prefix.
    std::mem::forget((owner, server));
    // Existing original results only. Retain first: stdout failure cannot Drop
    // the original owner/singleton graph before the final assertion.
    println!(
        "T4_NATIVE_CUTS admitted={} result={} held={} denied={} original_lease_busy={}",
        u8::from(admitted),
        u8::from(result),
        u8::from(held),
        u8::from(denied),
        u8::from(original_lease_busy)
    );
    assert!(completed, "fixed_native_vm_continuation_refused");
    println!("T4_NATIVE_ISOLATED_OWNER_COMMITTED_STILL_FENCED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}
