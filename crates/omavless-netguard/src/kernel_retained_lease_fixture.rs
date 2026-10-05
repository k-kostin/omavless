// SPDX-License-Identifier: MIT
//! Fresh cfg(test)-only retained fixture; no canonical or product authority.
use super::response_diagnostic::{Isolation, root_directory};
use super::*;
use crate::locked_state::LockedState;
use crate::protocol::{ErrorCode, Health, Mode, Protection, Request, Response};
use crate::receipt::NamespaceObservation;
use crate::root_state::RootStateStore;
use std::fs::{DirBuilder, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

const STAGE: &str = "/run/omavless-k1-retained-lease-regression";
const TEST: &str = "kernel_observer::creator_lifecycle::retained_lease::manager_retained_lease";
const ARM: Request = Request::Arm {
    generation: 7,
    mode: Mode::Full,
};

// Every effect-bearing owner is installed here BEFORE use and leaked until
// process exit. No error/panic Drop can close a causal socket or state lock.
struct Held {
    isolation: Isolation,
    stage: Option<File>,
    creator: Option<FixtureCreator>,
    state: Option<LockedState>,
    refused: Option<FixtureCreator>,
    refused_state: Option<LockedState>,
    observer: Option<LocalReadSession>,
    pending: Option<File>,
    output: Option<File>,
}

fn state(parent: &Path) -> Result<LockedState> {
    DirBuilder::new()
        .mode(0o700)
        .create(parent)
        .map_err(|_| REFUSE)?;
    DirBuilder::new()
        .mode(0o700)
        .create(parent.join("omavless-netguard"))
        .map_err(|_| REFUSE)?;
    Ok(LockedState::from_root(
        RootStateStore::open_test_parent(root_directory(parent)?, (0, 0), 1001)
            .map_err(|_| REFUSE)?,
    ))
}

fn status(response: Response, armed: bool) -> Result<()> {
    require(
        matches!(response, Response::Status { protection, health: Health::Verified, .. }
        if protection == if armed { Protection::Armed { generation: 7 } } else { Protection::Disarmed {} }),
    )
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    version: u32,
    enrolled_uid: u32,
    boot: [u8; 16],
    host_netns_epoch: [u8; 16],
    netns_device: u64,
    netns_inode: u64,
    operation: u64,
    phase: String,
    table_handle: u64,
}
fn pending_matches(raw: &[u8], epoch: HostEpoch) -> Result<()> {
    require(!raw.is_empty() && raw.len() <= 4096)?;
    let value: Pending = serde_json::from_slice(raw).map_err(|_| REFUSE)?;
    require(
        value.version == 1
            && value.enrolled_uid == 1001
            && value.boot == epoch.boot
            && value.host_netns_epoch == epoch.namespace_epoch
            && value.netns_device == epoch.namespace_device
            && value.netns_inode == epoch.namespace_inode
            && value.operation == 1
            && value.phase == "pending_create"
            && value.table_handle == 0,
    )
}

fn run(held: &mut Held) -> Result<()> {
    held.isolation.recheck()?;
    let stage = Path::new(STAGE);
    held.stage = Some(root_directory(stage)?);
    let primary = stage.join("state");
    held.state = Some(state(&primary)?);
    held.isolation.recheck()?;
    held.creator = Some(FixtureCreator::open(primary)?);
    let creator = held.creator.as_mut().ok_or(REFUSE)?;
    require(creator.session.identity == held.isolation.own_id)?;
    let namespace = NamespaceObservation::Canonical(creator.epoch); // explicitly synthetic
    let state = held.state.as_mut().ok_or(REFUSE)?;
    require(creator.session.inspect_policy_inventory()? == LocalPolicyInventory::TableAbsent)?;
    status(
        state.request(ARM, namespace, creator).map_err(|_| REFUSE)?,
        true,
    )?;
    let first = creator.created.ok_or(REFUSE)?;
    require(creator.effects == 1)?;
    held.isolation.recheck()?;
    // A real matching-generation arm performs one atomic delete+exclusive-create.
    status(
        state.request(ARM, namespace, creator).map_err(|_| REFUSE)?,
        true,
    )?;
    require(creator.effects == 2 && creator.created.is_some_and(|id| id != first))?;
    require(
        creator.session.inspect_policy_inventory()?
            == LocalPolicyInventory::ExactUntrusted(Policy::FullVpn),
    )?;
    held.isolation.recheck()?;
    status(
        state
            .request(Request::Disarm { generation: 7 }, namespace, creator)
            .map_err(|_| REFUSE)?,
        false,
    )?;
    require(creator.effects == 3 && creator.created.is_none())?;
    require(creator.session.inspect_policy_inventory()? == LocalPolicyInventory::TableAbsent)?;
    status(
        state
            .request(Request::Status {}, namespace, creator)
            .map_err(|_| REFUSE)?,
        false,
    )?;

    // Separate retained state: the successful Closed/Retired record above is
    // never reused, repaired or replaced by this deliberate refusal case.
    held.isolation.recheck()?;
    let negative = stage.join("generation-state");
    held.refused_state = Some(self::state(&negative)?);
    held.refused = Some(FixtureCreator::open(negative.clone())?);
    held.observer = Some(LocalReadSession::open()?);
    let observer = held.observer.as_mut().ok_or(REFUSE)?;
    require(
        observer.identity == held.isolation.own_id
            && observer.inspect_policy_inventory()? == LocalPolicyInventory::TableAbsent,
    )?;
    let refused = held.refused.as_mut().ok_or(REFUSE)?;
    require(refused.session.identity == held.isolation.own_id)?;
    refused.retained_generation_cut = Some(Box::new(FixtureCreator::open(negative.clone())?));
    require(
        refused
            .retained_generation_cut
            .as_ref()
            .ok_or(REFUSE)?
            .session
            .identity
            == held.isolation.own_id,
    )?;
    held.isolation.recheck()?;
    require(
        held.refused_state.as_mut().ok_or(REFUSE)?.request(
            ARM,
            NamespaceObservation::Canonical(refused.epoch),
            refused,
        ) == Err(ErrorCode::ManualRecoveryRequired),
    )?;
    require(
        refused.generation_refused
            && refused.session.poisoned
            && refused.created.is_none()
            && refused.effects == 1
            && refused
                .retained_generation_cut
                .as_ref()
                .ok_or(REFUSE)?
                .effects
                == 1,
    )?;
    // No follow-up on the poisoned creator/LockedState, even to "confirm".
    // This already-held independent reader can report only untrusted absence.
    require(observer.inspect_policy_inventory()? == LocalPolicyInventory::TableAbsent)?;
    let pending_path = negative.join("omavless-netguard/table-receipt-v1.json");
    held.pending = Some(
        OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC | nix::libc::O_NONBLOCK)
            .open(&pending_path)
            .map_err(|_| REFUSE)?,
    );
    let file = held.pending.as_mut().ok_or(REFUSE)?;
    let before = file.metadata().map_err(|_| REFUSE)?;
    require(
        before.is_file()
            && before.uid() == 0
            && before.gid() == 0
            && before.mode() & 0o7777 == 0o600
            && before.nlink() == 1
            && before.len() <= 4096,
    )?;
    let mut raw = Vec::new();
    file.take(4097).read_to_end(&mut raw).map_err(|_| REFUSE)?;
    pending_matches(&raw, refused.epoch)?;
    let key = |m: std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.uid(),
            m.gid(),
            m.nlink(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let expected = key(before);
    require(
        key(held
            .pending
            .as_ref()
            .ok_or(REFUSE)?
            .metadata()
            .map_err(|_| REFUSE)?)
            == expected
            && key(pending_path.symlink_metadata().map_err(|_| REFUSE)?) == expected,
    )?;
    held.isolation.recheck()?;
    held.output = Some(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(stage.join("native-result.json"))
            .map_err(|_| REFUSE)?,
    );
    let output = held.output.as_mut().ok_or(REFUSE)?;
    output.write_all(b"{\"schema\":1,\"synthetic_epoch\":true,\"effects\":3,\"replacement\":true,\"full_inventory\":true,\"absent\":true,\"generation_refused\":true,\"refused_send_attempts\":1,\"generation_cut_effects\":1,\"pending_retained\":true}\n").map_err(|_| REFUSE)?;
    output.sync_all().map_err(|_| REFUSE)?;
    held.stage
        .as_ref()
        .ok_or(REFUSE)?
        .sync_all()
        .map_err(|_| REFUSE)
}

#[test]
#[ignore = "fresh fixed manager-owned PrivateNetwork fixture; reviewed outer guard and dedicated VM lease required"]
fn manager_retained_lease() {
    assert_eq!(
        std::env::var("OMAVLESS_K1_RETAINED_LEASE_WRITER").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        [
            "--exact",
            TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1"
        ]
    );
    let isolation =
        Isolation::capture_lease_regression().expect("K1_RETAINED_LEASE_ADMISSION_REFUSED");
    let held = Box::leak(Box::new(Held {
        isolation,
        stage: None,
        creator: None,
        state: None,
        refused: None,
        refused_state: None,
        observer: None,
        pending: None,
        output: None,
    }));
    if !matches!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(held))),
        Ok(Ok(()))
    ) {
        eprintln!("K1_RETAINED_LEASE_UNCERTAIN_RETAINED");
        loop {
            std::thread::park();
        }
    }
    println!("K1_RETAINED_LEASE_NATIVE_PASS_NOT_CANONICAL");
}

#[test]
fn pending_receipt_is_strict_and_cannot_promote_authority() {
    let epoch = HostEpoch {
        boot: [0x31; 16],
        namespace_epoch: [0x32; 16],
        namespace_device: 5,
        namespace_inode: 9,
    };
    let value = serde_json::json!({"version":1,"enrolled_uid":1001,"boot":epoch.boot,
        "host_netns_epoch":epoch.namespace_epoch,"netns_device":5,"netns_inode":9,
        "operation":1,"phase":"pending_create","table_handle":0});
    let raw = serde_json::to_vec(&value).unwrap();
    assert!(pending_matches(&raw, epoch).is_ok());
    for end in 0..raw.len() {
        assert!(pending_matches(&raw[..end], epoch).is_err());
    }
    for (field, bad) in [
        ("phase", serde_json::json!("live")),
        ("operation", serde_json::json!(2)),
        ("table_handle", serde_json::json!(7)),
        ("version", serde_json::json!(true)),
        ("canonical_authority", serde_json::json!(true)),
    ] {
        let mut changed = value.clone();
        changed[field] = bad;
        assert!(pending_matches(&serde_json::to_vec(&changed).unwrap(), epoch).is_err());
    }
    let duplicate = String::from_utf8(raw)
        .unwrap()
        .replace("\"version\":1", "\"version\":1,\"version\":1");
    assert!(pending_matches(duplicate.as_bytes(), epoch).is_err());
}

#[test]
fn generation_cut_has_only_fixed_empty_table_and_conditioned_atomic_boundaries() {
    for (generation, first) in [(0, 1), (1, 0), (1, u32::MAX)] {
        assert!(atomic_batch::lease_generation_cut_batch(generation, first).is_err());
    }
    let batch = atomic_batch::lease_generation_cut_batch(11, 101).unwrap();
    assert_eq!(batch.len(), 3);
    assert_eq!(
        batch[0],
        message(
            16,
            5,
            101,
            0,
            &[vec![0, 0, 0, 10], attribute(1, &11u32.to_be_bytes())].concat()
        )
    );
    assert_eq!(
        batch[1],
        message(
            NFT,
            0x605,
            102,
            0,
            &[vec![1, 0, 0, 0], attribute(1, b"k1_retained_lease_cut\0")].concat()
        )
    );
    assert_eq!(batch[2], message(17, 5, 103, 0, &[0, 0, 0, 10]));
    assert!(
        !batch
            .concat()
            .windows(TABLE.len())
            .any(|part| part == TABLE)
    );
}
