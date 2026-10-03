// SPDX-License-Identifier: MIT
//! Explicit ignored disposable-account driver. No root actions, manager
//! simulation or receipt publication. The reviewed external root harness alone
//! provisions the account/image and invokes the genuine packaged login unit.

use super::*;

const UID: u32 = 61080;
const HOME: &str = "/home/ov-t4-system";
const OPT_IN: &str = "OMAVLESS_TEST_SYSTEM_OFF_VM";
const RECEIPT: &str = "omavless-login.receipt";

fn observation_category(
    result: Result<(), crate::production_observation::ProductionObservationError>,
) -> &'static str {
    use crate::production_observation::ProductionObservationError as E;
    match result {
        Ok(()) => "Empty",
        Err(E::UnsafePath) => "UnsafePath",
        Err(E::ServiceQuery) => "ServiceQuery",
        Err(E::ServiceResponse) => "ServiceResponse",
        Err(E::PrivateState) => "PrivateState",
        Err(E::HostNotEmpty) => "HostNotEmpty",
        Err(E::IncompleteInventory) => "IncompleteInventory",
        Err(E::Cutover(_)) => "CutoverRefused",
    }
}

#[test]
#[ignore = "fixed disposable account read-only post-login-failure diagnostic; no receipt retry"]
fn system_provider_vm_diagnose_login_prerequisites() {
    let (_, config) = paths();
    use crate::production_observation::{ProductionOwnershipObserver, service_state_with_timeout};
    println!(
        "Inputs:{}",
        crate::login_transaction::diagnose_fixed_login_inputs()
    );
    for (label, service) in [
        ("LegacyService", "omavless.service"),
        ("NativeService", "omavless-runtime.service"),
    ] {
        let result = service_state_with_timeout(
            Path::new("/usr/bin/systemctl"),
            service,
            std::time::Duration::from_secs(2),
        )
        .and_then(|state| {
            if state.active || state.main_pid != 0 {
                Err(crate::production_observation::ProductionObservationError::HostNotEmpty)
            } else {
                Ok(())
            }
        });
        println!("{label}:{}", observation_category(result));
    }
    let processes =
        omavless_mihomo::observation::processes_named_strict(Path::new("/proc"), "mihomo");
    println!(
        "ProcessInventory:{}",
        match processes {
            Ok(p) if p.is_empty() => "Empty",
            Ok(_) => "Present",
            Err(_) => "Refused",
        }
    );
    let devices = crate::tun_scope::configured_devices(&config, UID);
    println!(
        "TunConfig:{}",
        match devices {
            Ok(Some(_)) => "Recognized",
            Ok(None) => "WholeHostFallback",
            Err(_) => "Refused",
        }
    );
    println!(
        "TunInventory:{}",
        match crate::tun_scope::inventory(Path::new("/sys/class/net")) {
            Ok(p) if p.is_empty() => "Empty",
            Ok(_) => "Present",
            Err(_) => "Refused",
        }
    );
    let result =
        ProductionOwnershipObserver::current().and_then(|observer| observer.verify_native_empty());
    println!("NativeEmpty:{}", observation_category(result));
    // A passing diagnostic means only that fixed read-only stages ran. Its
    // printed refusals remain refusals; this is never System admission evidence.
}

fn identity_allowed(uid: u32, home: Option<&std::ffi::OsStr>, opt_in: Option<&str>) -> bool {
    uid == UID && home == Some(std::ffi::OsStr::new(HOME)) && opt_in == Some("fixture-v1")
}

fn paths() -> (CutoverPaths, std::path::PathBuf) {
    let uid = nix::unistd::Uid::current();
    assert!(
        identity_allowed(
            uid.as_raw(),
            std::env::var_os("HOME").as_deref(),
            std::env::var(OPT_IN).ok().as_deref()
        ),
        "fixed disposable-account opt-in required"
    );
    assert_eq!(nix::unistd::Gid::current().as_raw(), UID);
    assert!(std::env::var_os("OMAVLESS_HOME").is_none());
    for key in ["XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"] {
        assert!(
            std::env::var_os(key).is_none(),
            "default disposable account roots required"
        );
    }
    assert!(
        std::env::var_os("XDG_RUNTIME_DIR")
            .is_some_and(|p| p == std::ffi::OsStr::new("/run/user/61080"))
    );
    let account = nix::unistd::User::from_uid(uid).unwrap().unwrap();
    assert!(
        account.name == "ov-t4-system"
            && account.dir == Path::new(HOME)
            && account.gid.as_raw() == UID
    );
    let paths = CutoverPaths::current(UID).unwrap();
    let config = Path::new(HOME).join(".config/omavless");
    assert_eq!(paths.runtime_base, Path::new("/run/user/61080"));
    assert_eq!(
        paths.state_directory,
        Path::new(HOME).join(".local/state/omavless")
    );
    for directory in [&config, &paths.state_directory, &paths.runtime_base] {
        open_private_directory(directory, UID).unwrap();
    }
    (paths, config)
}

fn absent_path(path: &Path) {
    assert!(
        matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
        "fixture entry must be absent"
    );
}

fn create(path: &Path, bytes: &[u8]) {
    assert_eq!(
        omavless_store::atomic_create_private(path, bytes, UID).unwrap(),
        omavless_store::PrivateCreateOutcome::Created
    );
}

struct Pinned {
    parent: File,
    parent_path: std::path::PathBuf,
    parent_metadata: Metadata,
    name: String,
    file: File,
    bytes: Zeroizing<Vec<u8>>,
    metadata: Metadata,
    limit: usize,
}
impl Pinned {
    fn read(path: &Path, limit: usize) -> Self {
        let parent_path = path.parent().unwrap().to_owned();
        let parent = open_private_directory(&parent_path, UID).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let (bytes, metadata) = read_optional(&parent, &name, UID, limit).unwrap().unwrap();
        let file = pin_member(&parent, &name, &metadata).unwrap();
        let parent_metadata = parent.metadata().unwrap();
        Self {
            parent,
            parent_path,
            parent_metadata,
            name,
            file,
            bytes,
            metadata,
            limit,
        }
    }
    fn unchanged(&self) {
        let current_parent = open_private_directory(&self.parent_path, UID).unwrap();
        assert!(same_directory(
            &self.parent_metadata,
            &current_parent.metadata().unwrap()
        ));
        assert!(same_directory(
            &self.parent_metadata,
            &self.parent.metadata().unwrap()
        ));
        assert!(same_member(&self.metadata, &self.file.metadata().unwrap()));
        let (bytes, metadata) = read_optional(&current_parent, &self.name, UID, self.limit)
            .unwrap()
            .unwrap();
        assert!(
            bytes == self.bytes && same_member(&self.metadata, &metadata),
            "retained fixture member changed"
        );
    }
}

#[test]
fn system_vm_identity_guard_never_admits_primary_root_or_unselected_account() {
    let home = Some(std::ffi::OsStr::new(HOME));
    assert!(identity_allowed(UID, home, Some("fixture-v1")));
    for uid in [0, 1000, UID - 1, UID + 1] {
        assert!(!identity_allowed(uid, home, Some("fixture-v1")));
    }
    assert!(!identity_allowed(UID, None, Some("fixture-v1")));
    assert!(!identity_allowed(
        UID,
        Some(std::ffi::OsStr::new("/home/other")),
        Some("fixture-v1")
    ));
    assert!(!identity_allowed(UID, home, None));
    assert!(!identity_allowed(UID, home, Some("true")));
}

#[test]
#[ignore = "exclusive reviewed Dev-VM disposable account; seed before genuine packaged login"]
fn system_provider_vm_seed_off_inputs() {
    let (paths, config) = paths();
    for path in [
        paths.ownership_marker.clone(),
        paths.state_directory.join("desired.json"),
        config.join(LIVE[0]),
        config.join(LIVE[1]),
        paths.runtime_base.join(RECEIPT),
    ] {
        absent_path(&path);
    }
    assert!(!crate::pending_private_transaction::pending_at(
        &paths.state_directory
    ));
    let lock = MigrationLock::acquire(&paths, UID).unwrap();
    let mut store: serde_json::Value =
        serde_json::from_slice(crate::store_bootstrap::EMPTY_STORE_PAYLOAD).unwrap();
    store["routingPreset"] = "default".into();
    create(&config.join(LIVE[0]), &serde_json::to_vec(&store).unwrap());
    create(
        &config.join(LIVE[1]),
        include_bytes!("../../../templates/default.yaml"),
    );
    create(
        &paths.state_directory.join("desired.json"),
        &serde_json::to_vec(&crate::desired::DesiredState::default()).unwrap(),
    );
    create(
        &paths.ownership_marker,
        br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
    );
    assert!(lock.authorizes(&paths, UID));
    absent_path(&paths.runtime_base.join(RECEIPT));
}

#[test]
#[ignore = "exclusive reviewed Dev-VM real package/manager/receipt and private root-issued image"]
fn system_provider_vm_real_current_off_preserves_original_receipt_and_fences() {
    let (paths, config) = paths();
    let receipt = Pinned::read(&paths.runtime_base.join(RECEIPT), 1024);
    let marker = Pinned::read(&paths.ownership_marker, 1024);
    let lock = MigrationLock::acquire_existing(&paths, UID).unwrap();
    // Actual System capture BEFORE synthetic historical seeding. The receipt
    // came from the packaged login unit; no test receipt/epoch constructor runs.
    let mut epoch =
        crate::login_activation::epoch_candidate::CurrentEpochProof::capture(&paths, UID, 2, &lock)
            .unwrap();
    assert!(!crate::pending_private_transaction::pending_at(
        &paths.state_directory
    ));
    for name in [CLOSURE_MEMBER, TICKET_MEMBER, COMPLETE_MEMBER] {
        absent_path(&paths.state_directory.join(name));
    }
    {
        // This Fixture owns ONLY its freshly allocated temporary subtree. Never
        // retarget its root: Drop recursively removes that owned temp subtree.
        let (fixture, source_lock) = super::tests::prepared(true);
        super::tests::ordinary_edit(&fixture);
        crate::private_store_transaction::prepare_pointer_mutation(
            &fixture.config.join(LIVE[0]),
            UID,
            omavless_domain::private_store::CompatibilityPointerTarget::Disconnected {
                prune_missing: true,
            },
        )
        .unwrap()
        .commit_locked(&source_lock, &fixture.paths)
        .unwrap();
        let source = Snapshot::read_policy(
            &fixture.config,
            &fixture.paths,
            UID,
            2,
            &source_lock,
            LivePolicy::ValidCurrentOff,
        )
        .unwrap();
        for (index, name) in [CLOSURE_MEMBER, TICKET_MEMBER, COMPLETE_MEMBER]
            .into_iter()
            .enumerate()
        {
            create(&paths.state_directory.join(name), &source.members[index].0);
            receipt.unchanged();
            epoch.recheck(&paths, UID, 2, &lock).unwrap();
        }
        for (index, name) in LIVE.into_iter().enumerate() {
            omavless_store::atomic_replace_private(
                &config.join(name),
                &source.members[index + 3].0,
                UID,
            )
            .unwrap();
            receipt.unchanged();
            epoch.recheck(&paths, UID, 2, &lock).unwrap();
        }
        let desired = source.boundary[1].as_ref().unwrap();
        omavless_store::atomic_replace_private(
            &paths.state_directory.join("desired.json"),
            &desired.0,
            UID,
        )
        .unwrap();
        receipt.unchanged();
        marker.unchanged();
        epoch.recheck(&paths, UID, 2, &lock).unwrap();
        drop(source_lock);
    }
    drop(epoch);
    drop(lock);
    let pinned: Vec<_> = [
        paths.state_directory.join(CLOSURE_MEMBER),
        paths.state_directory.join(TICKET_MEMBER),
        paths.state_directory.join(COMPLETE_MEMBER),
        paths.state_directory.join("desired.json"),
        config.join(LIVE[0]),
        config.join(LIVE[1]),
    ]
    .iter()
    .map(|path| Pinned::read(path, MAX_PRIVATE_STORE_BYTES))
    .collect();
    assert_eq!(
        crate::production_owner::system_historical_off::current_for_vm_test().unwrap(),
        crate::production_owner::system_historical_off::Review::ReviewedOffStillFenced
    );
    receipt.unchanged();
    marker.unchanged();
    for source in &pinned {
        source.unchanged();
    }
    assert!(crate::pending_private_transaction::pending_at(
        &paths.state_directory
    ));
    let runtime = crate::RuntimePaths::current().unwrap();
    assert!(matches!(
        crate::production_owner::ProductionNativeOwner::current(&runtime),
        Err(crate::production_owner::ProductionOwnerError::ManualRecoveryRequired)
    ));
    receipt.unchanged();
    marker.unchanged();
    for source in &pinned {
        source.unchanged();
    }
}
