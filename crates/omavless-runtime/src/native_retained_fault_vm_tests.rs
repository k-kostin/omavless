// SPDX-License-Identifier: MIT
//! Three fixed disjoint actual-owner cases, one bounded retained aggregate.
use super::*;
use crate::restore_decision_candidate::{DecisionChain, DecisionPhase, RecoveryReview};
use crate::restore_staging_candidate::{
    LivePairClass, MEMBERS, PENDING_DIRECTORY, READY_MEMBER, class_from_matches,
    planned_stage_identity, ready_bytes,
};

const MATRIX_ROOT: &str = "/home/kdk_vm/.cache/t4-native-retained-owner-review22";
const MATRIX_RUNTIME: &str = "/run/user/1000/t4n22";
const CASES: [&str; 3] = ["before-pair", "after-intent", "after-first-rename"];
// Actual lower role array + Boundary directories/members/live, retained
// acquisition prefix/probe, and the original MigrationLock pair. Authenticated
// backup/prepared bytes add no persistent descriptor to the holder.
const NATIVE_RESERVED: usize =
    crate::manager_actor_service::NATIVE_VM_LOWER_SLOTS + 3 + 3 + 2 + 1 + 1 + 2;
const AGGREGATE_RESERVED: usize = 3 * (NATIVE_RESERVED + 2); // singleton originals too

struct OriginalCase {
    owner: Option<ProductionNativeOwner>,
    singleton: Option<crate::RuntimeServer>,
    reservation: Option<crate::native_coordinator::NativeVmReservation>,
}

fn reserve_aggregate(
    limit: u64,
    mut reserve: impl FnMut() -> Result<crate::native_coordinator::NativeVmReservation, ()>,
) -> Result<[OriginalCase; 3], ()> {
    // Necessary ceiling only: this does not attest all ambient descriptors or
    // promise kernel allocation. Every later refusal retains acquired prefixes.
    if limit < (AGGREGATE_RESERVED + 4) as u64 {
        return Err(());
    }
    let reservations = [reserve()?, reserve()?, reserve()?];
    if !reservations.iter().all(|slot| slot.vm_reserved()) {
        return Err(());
    }
    Ok(reservations.map(|reservation| OriginalCase {
        owner: None,
        singleton: None,
        reservation: Some(reservation),
    }))
}
impl Drop for OriginalCase {
    fn drop(&mut self) {
        // Every unwind/early return retains the actual graph. No case eviction
        // enables another context. Fatal test-process loss means unavailable.
        std::mem::forget(self.owner.take());
        std::mem::forget(self.singleton.take());
    }
}

fn metadata_bound_read(path: &Path, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let before = fs::symlink_metadata(path).ok()?;
    if !before.is_file()
        || before.uid() != UID
        || before.gid() != UID
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
        || before.len() > limit as u64
    {
        return None;
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let opened = file.metadata().ok()?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let after = file.metadata().ok()?;
    let named = fs::symlink_metadata(path).ok()?;
    if bytes.len() > limit
        || !crate::restore_staging_candidate::same_member(&before, &opened)
        || !crate::restore_staging_candidate::same_member(&before, &after)
        || !crate::restore_staging_candidate::same_member(&before, &named)
        || [opened.gid(), after.gid(), named.gid()]
            .into_iter()
            .any(|gid| gid != before.gid())
        || bytes.len() as u64 != before.len()
    {
        return None;
    }
    Some(bytes)
}

fn pending_readback(
    root: &Path,
    case: usize,
    old: [&[u8]; 2],
    new: [&[u8]; 2],
    desired: &[u8],
) -> bool {
    let config = root.join("home/.config/omavless");
    let state = root.join("state/omavless");
    let current = [
        metadata_bound_read(
            &config.join("profiles.json"),
            old[0].len().max(new[0].len()),
        ),
        metadata_bound_read(
            &config.join("route-template.yaml"),
            old[1].len().max(new[1].len()),
        ),
    ];
    let [Some(store), Some(template)] = current else {
        return false;
    };
    if case == 0 {
        return store == old[0]
            && template == old[1]
            && absent(&state.join(PENDING_DIRECTORY))
            && absent(&state.join("restore-decision.intent"))
            && absent(&state.join("restore-decision.terminal"));
    }
    let members = [old[0], old[1], new[0], new[1]];
    let stage = state.join(PENDING_DIRECTORY);
    for (name, bytes) in MEMBERS.into_iter().zip(members) {
        if metadata_bound_read(&stage.join(name), bytes.len()).as_deref() != Some(bytes) {
            return false;
        }
    }
    if metadata_bound_read(&stage.join(READY_MEMBER), ready_bytes(members).len()).as_deref()
        != Some(ready_bytes(members).as_slice())
    {
        return false;
    }
    let Some(intent) = metadata_bound_read(
        &state.join("restore-decision.intent"),
        crate::restore_decision_candidate::RECORD_BYTES,
    ) else {
        return false;
    };
    let Ok(chain) = DecisionChain::decode(&intent, None) else {
        return false;
    };
    if !absent(&state.join("restore-decision.terminal"))
        || chain.active().phase() != DecisionPhase::Intent
    {
        return false;
    }
    let class = class_from_matches(
        store == old[0],
        template == old[1],
        store == new[0],
        template == new[1],
    );
    let expected = if case == 1 {
        LivePairClass::Old
    } else {
        LivePairClass::Mixed
    };
    class == expected
        && chain.active().review_inspection(
            2,
            Some(desired),
            &planned_stage_identity(members).unwrap(),
            class,
        ) == RecoveryReview::OldRollbackCandidate
}

fn construct(
    slot: &mut OriginalCase,
    root: &Path,
    runtime_base: &Path,
    until: Instant,
) -> Option<(CutoverPaths, Vec<u8>)> {
    let fixture_home = root.join("home");
    let config = fixture_home.join(".config/omavless");
    let state_base = root.join("state");
    let runtime = RuntimePaths::below(runtime_base);
    let desired = DesiredPaths::below(&state_base);
    let cutover = CutoverPaths::below(runtime_base, &state_base, UID);
    if ![
        root,
        fixture_home.as_path(),
        fixture_home.join(".config").as_path(),
        config.as_path(),
        state_base.as_path(),
        desired.directory.as_path(),
        runtime_base,
        runtime.directory.as_path(),
    ]
    .into_iter()
    .all(private_directory)
        || !absent(&runtime.socket)
        || !absent(&runtime.owner_lock)
        || !no_orphans(runtime_base)
        || !no_orphans(&runtime.directory)
    {
        return None;
    }
    let saved = read_desired_snapshot(&desired, UID).ok()?;
    if saved.connected || !saved.profile_id.is_empty() {
        return None;
    }
    let raw = metadata_bound_read(
        &desired.file,
        crate::desired::MAX_DESIRED_STATE_BYTES as usize,
    )?;
    let paths = ProductionObservationPaths::below(
        Path::new("/usr/bin/systemctl").into(),
        &fixture_home,
        runtime_base,
        Path::new("/proc").into(),
        Path::new("/sys/class/net").into(),
        UID,
    );
    ProductionOwnershipObserver::new(paths, UID)
        .ok()?
        .verify_native_empty()
        .ok()?;
    let mut host = NativeLifecycleHost::new(
        NativeHostPaths::new(
            Path::new(CORE).into(),
            config.clone(),
            config.clone(),
            runtime.directory.clone(),
            Path::new("/proc").into(),
            Path::new("/sys/class/net").into(),
        ),
        UID,
    )
    .ok()?;
    if !host.fresh_observation(&saved).is_ok_and(empty) || Instant::now() >= until {
        return None;
    }
    slot.singleton = Some(crate::RuntimeServer::bind(runtime).ok()?);
    slot.owner = Some(
        ProductionNativeOwner::initialize(
            host,
            desired,
            &config.join("profiles.json"),
            cutover.clone(),
            UID,
        )
        .ok()?,
    );
    slot.owner
        .as_mut()?
        .coordinator
        .retained_vm_install_reservation(slot.reservation.take()?)
        .ok()?;
    if slot.owner.as_ref()?.actual() != ActualState::Disconnected
        || slot.owner.as_ref()?.startup_outcome().changed
        || Instant::now() >= until
    {
        return None;
    }
    Some((cutover, raw))
}

#[test]
fn native_fault_matrix_has_one_fixed_bounded_non_evictable_aggregate() {
    assert_eq!(CASES.len(), 3);
    assert_eq!(AGGREGATE_RESERVED, 150);
    assert!(
        CASES
            .into_iter()
            .all(|name| !name.contains('/') && !name.contains(".."))
    );
}

#[test]
fn native_fault_matrix_reserves_all_original_storage_before_acquisition() {
    use crate::native_coordinator::NativeVmReservation;
    let limit = (AGGREGATE_RESERVED + 4) as u64;
    for refused in 0..3 {
        let mut calls = 0;
        let aggregate = reserve_aggregate(limit, || {
            let index = calls;
            calls += 1;
            if index == refused {
                Err(())
            } else {
                NativeVmReservation::reserve_vm().map_err(|_| ())
            }
        });
        assert!(aggregate.is_err());
        assert_eq!(calls, refused + 1);
    }
    assert!(
        reserve_aggregate(limit - 1, || panic!(
            "reservation before capacity admission"
        ))
        .is_err()
    );
    let aggregate =
        reserve_aggregate(limit, || NativeVmReservation::reserve_vm().map_err(|_| ())).unwrap();
    assert!(aggregate.iter().all(|case| {
        case.owner.is_none()
            && case.singleton.is_none()
            && case
                .reservation
                .as_ref()
                .is_some_and(NativeVmReservation::vm_reserved)
    }));
}

#[test]
#[ignore = "ROOT-only three fresh real-host owner prefixes; no foreign or installed-store effect"]
fn isolated_installed_host_native_fault_matrix() {
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
    // Reserve every case owner/prefix slot BEFORE the first acquisition. These
    // are disjoint fixtures; no uncertain original is reused or released.
    let (limit, _) =
        nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE).unwrap();
    let mut aggregate = reserve_aggregate(limit, || {
        crate::native_coordinator::NativeVmReservation::reserve_vm().map_err(|_| ())
    })
    .expect("fixed_native_vm_matrix_reservation_refused");
    let until = Instant::now() + Duration::from_secs(240);
    for (index, name) in CASES.into_iter().enumerate() {
        let root = Path::new(MATRIX_ROOT).join(name);
        let runtime = Path::new(MATRIX_RUNTIME).join(name);
        let Some((cutover, desired)) = construct(&mut aggregate[index], &root, &runtime, until)
        else {
            panic!("fixed_native_vm_matrix_construct_refused");
        };
        let config = root.join("home/.config/omavless");
        let old_store = metadata_bound_read(
            &config.join("profiles.json"),
            omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
        )
        .unwrap();
        let old_template = metadata_bound_read(
            &config.join("route-template.yaml"),
            omavless_domain::config::MAX_TEMPLATE_BYTES,
        )
        .unwrap();
        let archive = crate::backup_destination_candidate::open_existing(
            &root.join("input.ovb"),
            UID,
            PASSWORD,
        )
        .unwrap();
        let new_store = archive.restore_store_off().unwrap();
        let new = [new_store.as_slice(), archive.template()];
        let owner = aggregate[index].owner.as_mut().unwrap();
        let hit =
            owner
                .coordinator
                .retained_vm_fault(&root.join("input.ovb"), PASSWORD, index as u8);
        let held = owner.coordinator.retained_vm_custody();
        let denied = owner.coordinator.retained_vm_ordinary_and_recovery_denied();
        let busy = MigrationLock::acquire(&cutover, UID).is_err();
        let factual = pending_readback(&root, index, [&old_store, &old_template], new, &desired);
        assert!(
            hit && held && denied && busy && factual && Instant::now() < until,
            "fixed_native_vm_matrix_cut_refused"
        );
    }
    std::mem::forget(aggregate);
    println!("T4_NATIVE_REAL_OWNER_THREE_FAULT_PREFIXES_STILL_FENCED");
    assert!(Instant::now() < until, "fixed_native_vm_deadline_refused");
}
