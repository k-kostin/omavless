// SPDX-License-Identifier: MIT
//! Nonescaping genuine-native-session pair engine. No actor or public issuer.
// Recovery uses a distinct issuer below; normal Session admission is unchanged.
use super::*;
use crate::lifecycle::LifecycleHost;
use crate::native_coordinator::{NativeFirstError as FirstError, PreparedRestorePair};
use crate::native_coordinator::{NativeRecoveryOrigin, NativeSessionOrigin};
use std::fs::File;
// Terminal audit bytes are not an admission token. A collision always refuses;
// repeated disposition needs a separately reviewed history policy.
const NATIVE_HISTORY: &str = "restore-disposition.history";
// This role is unused in the native issuer, which never captures actor-manager
// libraries. Reserve it for one reported current-store replacement, not a
// recycled original or imported manager/image proof.
const NATIVE_CURRENT_STORE: Slot = Slot::ManagerLib;
const NATIVE_REPLACEMENTS: [(Slot, &str); 2] = [
    (
        Slot::ReplacementStore,
        crate::restore_executor_candidate::NEW_SLOT[0],
    ),
    (
        Slot::ReplacementTemplate,
        crate::restore_executor_candidate::NEW_SLOT[1],
    ),
];
const NATIVE_ROLLBACKS: [(Slot, &str); 2] = [
    (
        Slot::RollbackStore,
        crate::restore_executor_candidate::OLD_SLOT[0],
    ),
    (
        Slot::RollbackTemplate,
        crate::restore_executor_candidate::OLD_SLOT[1],
    ),
];
const RECOVERED_NEW: [(Slot, &str); 2] = [
    (
        Slot::Scratch4,
        crate::restore_executor_candidate::NEW_SLOT[0],
    ),
    (
        Slot::Scratch5,
        crate::restore_executor_candidate::NEW_SLOT[1],
    ),
];
// Private dispatch only: two concrete issuer types, never caller-provided grant
// closures, public trait objects or reconstructed Session snapshots.
trait NativeGate {
    fn check(&mut self, view: NativeStageView<'_>) -> Result<(), FirstError>;
}

#[cfg(test)]
#[test]
fn native_recovery_mixed_intent_admission_is_exact_and_non_authoritative() {
    let members: [&[u8]; 4] = [b"old-store", b"old-template", b"new-store", b"new-template"];
    let desired =
        br#"{"schemaVersion":1,"generation":0,"connected":false,"profileId":"","mode":"rule"}"#;
    let plan = planned_stage_identity(members).unwrap();
    let intent = DecisionRecord::intent(2, Some(desired), &plan, [7; 16]).unwrap();
    assert!(
        mixed_intent_review(
            &intent.encode(),
            members,
            [members[2], members[1]],
            2,
            desired
        )
        .is_ok()
    );
    for current in [
        [members[0], members[1]],
        [members[2], members[3]],
        [members[0], members[3]],
        [b"corrupt".as_slice(), members[1]],
    ] {
        assert!(mixed_intent_review(&intent.encode(), members, current, 2, desired).is_err());
    }
    for (generation, bytes) in [
        (1, desired.as_slice()),
        (3, desired.as_slice()),
        (2, b"stale".as_slice()),
    ] {
        assert!(
            mixed_intent_review(
                &intent.encode(),
                members,
                [members[2], members[1]],
                generation,
                bytes
            )
            .is_err()
        );
    }
    for terminal in [TerminalChoice::Commit, TerminalChoice::Abort] {
        assert!(
            mixed_intent_review(
                &intent.terminal(terminal).unwrap().encode(),
                members,
                [members[2], members[1]],
                2,
                desired
            )
            .is_err()
        );
    }
    let wrong_members: [&[u8]; 4] = [
        members[0],
        members[1],
        b"different authenticated NEW",
        members[3],
    ];
    assert!(
        mixed_intent_review(
            &intent.encode(),
            wrong_members,
            [members[2], members[1]],
            2,
            desired
        )
        .is_err()
    );
    let encoded = intent.encode();
    for raw in [&encoded[..RECORD_BYTES - 1], &[0_u8; RECORD_BYTES][..]] {
        assert!(mixed_intent_review(raw, members, [members[2], members[1]], 2, desired).is_err());
    }
}

#[cfg(test)]
#[test]
fn native_recovery_existing_singleton_busy_and_named_drift_are_real_files() {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    for (index, case) in [
        "positive",
        "busy",
        "directory-drift",
        "lock-drift",
        "socket-drift",
    ]
    .into_iter()
    .enumerate()
    {
        let root = std::env::temp_dir().join(format!("rs-{:x}-{index}", std::process::id()));
        assert!(
            root.join("omavless")
                .join(crate::SOCKET_NAME)
                .as_os_str()
                .as_encoded_bytes()
                .len()
                < 108
        );
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let directory = root.join("omavless");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join(crate::OWNER_LOCK_NAME))
            .unwrap();
        let socket = UnixListener::bind(directory.join(crate::SOCKET_NAME)).unwrap();
        fs::set_permissions(
            directory.join(crate::SOCKET_NAME),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        if case == "busy" {
            rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
        }
        let mut engine = NativeEngine::reserve();
        engine.uid = Some(nix::unistd::getuid().as_raw());
        engine.gid = Some(nix::unistd::getgid().as_raw());
        engine.recovery = true;
        engine.lower.io.native_admit().unwrap();
        let until = Instant::now() + std::time::Duration::from_secs(10);
        engine
            .clone_file(Slot::Run, &File::open(&root).unwrap(), true, until)
            .unwrap();
        let result = engine.capture_recovery_singleton(until);
        if case == "busy" {
            assert!(result.is_err());
            assert!(!engine.singleton_locked);
            assert!(engine.lower.io.test_retains_original(Slot::Lock));
        } else {
            result.unwrap();
            assert!(engine.singleton_locked);
            assert!(
                rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
                    .is_err()
            );
            engine.check_recovery_singleton(until).unwrap(); // SAME call baseline PASS
            engine.check_recovery_singleton(until).unwrap(); // complete rescan, no cursor alias
            match case {
                "directory-drift" => {
                    fs::rename(&directory, root.join("original-directory")).unwrap();
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&directory)
                        .unwrap();
                }
                "lock-drift" => {
                    fs::rename(
                        directory.join(crate::OWNER_LOCK_NAME),
                        directory.join("original-lock"),
                    )
                    .unwrap();
                    fs::write(directory.join(crate::OWNER_LOCK_NAME), b"").unwrap();
                    fs::set_permissions(
                        directory.join(crate::OWNER_LOCK_NAME),
                        fs::Permissions::from_mode(0o600),
                    )
                    .unwrap();
                }
                "socket-drift" => {
                    fs::rename(
                        directory.join(crate::SOCKET_NAME),
                        directory.join("original-socket"),
                    )
                    .unwrap();
                }
                _ => {}
            }
            if case != "positive" {
                assert!(engine.check_recovery_singleton(until).is_err());
            }
        }
        // Only these known local synthetic objects; never an actor/VM/foreign
        // original or recovery effect. Production uses the retained Arc holder.
        drop((engine, socket, lock));
        fs::remove_dir_all(root).unwrap();
    }
}

fn mixed_intent_review(
    intent: &[u8],
    members: [&[u8]; 4],
    current: [&[u8]; 2],
    generation: u64,
    desired: &[u8],
) -> Result<DecisionChain, FirstError> {
    let plan = planned_stage_identity(members).map_err(|_| FirstError::Admission)?;
    let chain = DecisionChain::decode(intent, None).map_err(|_| FirstError::Admission)?;
    let class = class_from_matches(
        current[0] == members[0],
        current[1] == members[1],
        current[0] == members[2],
        current[1] == members[3],
    );
    if class != LivePairClass::Mixed
        || current[0] != members[2]
        || current[1] != members[1]
        || chain.active().phase() != DecisionPhase::Intent
        || chain
            .active()
            .review_inspection(generation, Some(desired), &plan, class)
            != RecoveryReview::OldRollbackCandidate
    {
        return Err(FirstError::Admission);
    }
    Ok(chain)
}
impl<H: LifecycleHost> NativeGate for NativeSessionOrigin<'_, H> {
    fn check(&mut self, view: NativeStageView<'_>) -> Result<(), FirstError> {
        NativeSessionOrigin::check(self, view)
    }
}
impl NativeGate for NativeRecoveryOrigin<'_> {
    fn check(&mut self, view: NativeStageView<'_>) -> Result<(), FirstError> {
        NativeRecoveryOrigin::check(self, view)
    }
}

#[cfg(test)]
#[test]
fn native_retirement_real_files_keep_originals_and_closure_fence() {
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    struct ReachedPendingView {
        paths: crate::desired::DesiredPaths,
        uid: u32,
        saw_owned_receipt_with_stage: bool,
    }
    impl NativeGate for ReachedPendingView {
        fn check(&mut self, view: NativeStageView<'_>) -> Result<(), FirstError> {
            // Exercise the exact pending predicate called by the real recovery
            // origin, without claiming its real manager/host authority.
            if view.stage_present() {
                if !view.pending_allowed(&self.paths, self.uid) {
                    return Err(FirstError::Admission);
                }
                self.saw_owned_receipt_with_stage |=
                    view.own_completion_member(crate::restore_retirement_candidate::RECEIPT_MEMBER);
            }
            Ok(())
        }
    }
    let root = std::env::temp_dir().join(format!("nr-{:x}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let config = root.join("config");
    let state = root.join("state");
    let run = root.join("run");
    for path in [&config, &state, &run] {
        fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    }
    let singleton = run.join("omavless");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&singleton)
        .unwrap();
    let singleton_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(singleton.join(crate::OWNER_LOCK_NAME))
        .unwrap();
    let run_original = File::open(&run).unwrap();
    // Address only this owned directory through its original descriptor to
    // avoid a long TMPDIR exceeding the kernel Unix-address length bound.
    let socket = UnixListener::bind(format!(
        "/proc/self/fd/{}/omavless/{}",
        run_original.as_raw_fd(),
        crate::SOCKET_NAME
    ))
    .unwrap();
    fs::set_permissions(
        singleton.join(crate::SOCKET_NAME),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let stage = state.join(PENDING_DIRECTORY);
    fs::DirBuilder::new().mode(0o700).create(&stage).unwrap();
    let members: [&[u8]; 4] = [b"OLD store", b"OLD template", b"NEW store", b"NEW template"];
    let desired =
        br#"{"schemaVersion":1,"generation":0,"connected":false,"profileId":"","mode":"rule"}"#;
    let plan = planned_stage_identity(members).unwrap();
    let intent = DecisionRecord::intent(2, Some(desired), &plan, [9; 16]).unwrap();
    let terminal = intent.terminal(TerminalChoice::Abort).unwrap();
    let write = |path: &std::path::Path, bytes: &[u8]| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
    };
    for (name, bytes) in MEMBERS.into_iter().zip(members) {
        write(&stage.join(name), bytes);
    }
    write(&stage.join(READY_MEMBER), &ready_bytes(members));
    write(&config.join(LIVE[0].1), members[0]);
    write(&config.join(LIVE[1].1), members[1]);
    write(&state.join(INTENT), &intent.encode());
    write(&state.join(TERMINAL), &terminal.encode());
    // Only actual owned synthetic files, never a NativeSession/recovery issuer.
    // This checks the SAME retained lower effect/checking code, not host authority.
    let mut engine = NativeEngine::reserve();
    engine.uid = Some(nix::unistd::getuid().as_raw());
    engine.gid = Some(nix::unistd::getgid().as_raw());
    engine.lower.io.native_admit().unwrap();
    let until = Instant::now() + std::time::Duration::from_secs(10);
    for (slot, path) in [
        (Slot::Config, &config),
        (Slot::State, &state),
        (Slot::Run, &run),
        (Slot::StageDirectory, &stage),
    ] {
        engine
            .clone_file(slot, &File::open(path).unwrap(), true, until)
            .unwrap();
    }
    engine.stage_name = engine.lower.original[Slot::StageDirectory as usize].clone();
    for (index, (slot, name)) in LIVE.into_iter().enumerate() {
        engine
            .clone_file(slot, &File::open(config.join(name)).unwrap(), false, until)
            .unwrap();
        engine.read_original_live(index, until).unwrap();
    }
    for (index, name) in MEMBERS.into_iter().enumerate() {
        engine
            .recover_member(
                Slot::StageDirectory,
                STAGED[index],
                name,
                members[index].len(),
                until,
            )
            .unwrap();
    }
    engine
        .recover_member(
            Slot::StageDirectory,
            Slot::StageReady,
            READY_MEMBER,
            crate::restore_staging_candidate::READY_BYTES,
            until,
        )
        .unwrap();
    engine
        .recover_member(Slot::State, Slot::Intent, INTENT, RECORD_BYTES, until)
        .unwrap();
    engine
        .recover_member(Slot::State, Slot::Terminal, TERMINAL, RECORD_BYTES, until)
        .unwrap();
    engine
        .scan_catalogue(Slot::Config, Some(0), &[], until)
        .unwrap();
    engine
        .scan_catalogue(Slot::State, Some(1), &[], until)
        .unwrap();
    engine.catalogues_captured = true;
    let paths = crate::desired::DesiredPaths {
        directory: state.clone(),
        file: state.join("desired.json"),
    };
    let uid = nix::unistd::getuid().as_raw();
    assert!(
        !engine
            .view()
            .own_completion_member(crate::restore_retirement_candidate::RECEIPT_MEMBER)
    );
    assert!(engine.view().pending_allowed(&paths, uid));
    // A mere named receipt is never admitted: it must be an actual retained
    // member of this engine. This remains true for every unrelated fence.
    for name in [
        crate::restore_retirement_candidate::RECEIPT_MEMBER,
        crate::restore_closure_model::CLOSURE_MEMBER,
        crate::restore_disposition_complete_model::COMPLETE_MEMBER,
    ] {
        let path = state.join(name);
        write(&path, b"unowned");
        assert!(!engine.view().pending_allowed(&paths, uid));
        fs::remove_file(path).unwrap();
    }
    // Test-only local origin; enable retained receipt digest roles, but no
    // singleton/manager assertion is inferred from these synthetic controls.
    engine.recovery = true;
    engine.capture_recovery_singleton(until).unwrap();
    let mut local_gate = ReachedPendingView {
        paths,
        uid,
        saw_owned_receipt_with_stage: false,
    };
    engine
        .retire_native_aborted(
            &mut local_gate,
            members,
            &intent.encode(),
            &terminal.encode(),
            until,
        )
        .unwrap();
    assert!(local_gate.saw_owned_receipt_with_stage);
    assert!(engine.completed);
    assert!(
        engine.unlinked[Slot::StageDirectory as usize]
            && engine.unlinked[Slot::Intent as usize]
            && engine.unlinked[Slot::Terminal as usize]
    );
    for slot in STAGED
        .into_iter()
        .chain([Slot::Intent, Slot::Terminal, Slot::Scratch2])
    {
        assert_eq!(
            engine
                .lower
                .io
                .original(slot)
                .unwrap()
                .metadata()
                .unwrap()
                .nlink(),
            0
        );
    }
    assert_eq!(fs::read(config.join(LIVE[0].1)).unwrap(), members[0]);
    assert_eq!(fs::read(config.join(LIVE[1].1)).unwrap(), members[1]);
    assert!(crate::pending_private_transaction::pending_at(&state)); // closure is NOT ordinary permission
    assert!(fs::symlink_metadata(state.join(PENDING_DIRECTORY)).is_err());
    // SAME retained lower graph, not a decoded closure authority grant.
    engine
        .dispose_completed_inner(&mut local_gate, uid, 2, desired, until)
        .unwrap();
    assert!(engine.disposition_ready());
    assert!(!crate::pending_private_transaction::pending_at(&state));
    assert_eq!(fs::read(config.join(LIVE[0].1)).unwrap(), members[0]);
    assert_eq!(fs::read(config.join(LIVE[1].1)).unwrap(), members[1]);
    assert_eq!(
        engine
            .lower
            .io
            .original(Slot::Scratch3)
            .unwrap()
            .metadata()
            .unwrap()
            .nlink(),
        0
    );
    assert_eq!(
        engine
            .lower
            .io
            .original(Slot::Scratch0)
            .unwrap()
            .metadata()
            .unwrap()
            .nlink(),
        0
    );
    assert_eq!(
        engine
            .lower
            .io
            .original(Slot::Scratch1)
            .unwrap()
            .metadata()
            .unwrap()
            .nlink(),
        1
    );
    let ticket = crate::restore_disposition_ticket_model::Ticket::decode(&{
        let mut bytes = [0; crate::restore_disposition_ticket_model::TICKET_BYTES];
        engine
            .lower
            .io
            .original(Slot::Scratch0)
            .unwrap()
            .read_exact_at(&mut bytes, 0)
            .unwrap();
        bytes
    })
    .unwrap();
    assert!(
        crate::restore_disposition_complete_model::CompleteRecord::decode(
            &fs::read(state.join(NATIVE_HISTORY)).unwrap()
        )
        .unwrap()
        .matches_ticket(&ticket)
    );
    assert!(
        engine
            .dispose_completed_inner(&mut local_gate, uid, 2, desired, until)
            .is_err()
    );
    engine.begin_store_inner(&mut local_gate, until).unwrap();
    let next = b"public onboarded OLD store";
    let replacement = config.join("local-native-store-replacement");
    write(&replacement, next);
    fs::rename(replacement, config.join(LIVE[0].1)).unwrap(); // known local positive writer
    engine
        .finish_store_inner(&mut local_gate, next, true, until)
        .unwrap();
    assert!(engine.current_store && engine.disposition_ready());
    assert_eq!(
        engine
            .lower
            .io
            .original(Slot::OldStore)
            .unwrap()
            .metadata()
            .unwrap()
            .nlink(),
        0
    );
    assert_eq!(
        engine
            .lower
            .io
            .original(NATIVE_CURRENT_STORE)
            .unwrap()
            .metadata()
            .unwrap()
            .nlink(),
        1
    );
    assert_eq!(fs::read(config.join(LIVE[0].1)).unwrap(), next);
    assert!(engine.begin_store_inner(&mut local_gate, until).is_err());
    drop(engine);
    drop((socket, singleton_lock, run_original));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(test)]
#[test]
fn native_catalogue_known_unlink_is_one_way_no_unknown_or_reintroduced_name() {
    let mut catalogue = NativeCatalogue::empty();
    catalogue.insert(b"original").unwrap();
    catalogue.insert(b"unrelated").unwrap();
    assert!(catalogue.own_unlinked(b"unknown").is_err());
    catalogue.own_unlinked(b"original").unwrap();
    assert!(catalogue.own_unlinked(b"original").is_err());
    assert!(catalogue.insert(b"original").is_err());
    assert_eq!(catalogue.count, 2);
    assert!(catalogue.removed[catalogue.index(b"original").unwrap()]);
    assert!(!catalogue.removed[catalogue.index(b"unrelated").unwrap()]);
}

// Original directory membership is a bounded fact of the same held objects,
// not authority for any additional file. No name allocation after effects.
const CATALOGUE_LIMIT: usize = 128;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeStep {
    StageReady,
    Intent,
    Replacement(usize),
    Renamed(usize),
    Terminal,
}
struct NativeCatalogue {
    names: [[u8; 255]; CATALOGUE_LIMIT],
    lengths: [usize; CATALOGUE_LIMIT],
    count: usize,
    removed: [bool; CATALOGUE_LIMIT],
}
impl NativeCatalogue {
    fn empty() -> Self {
        Self {
            names: [[0; 255]; CATALOGUE_LIMIT],
            lengths: [0; CATALOGUE_LIMIT],
            count: 0,
            removed: [false; CATALOGUE_LIMIT],
        }
    }
    fn index(&self, name: &[u8]) -> Option<usize> {
        (0..self.count).find(|&i| &self.names[i][..self.lengths[i]] == name)
    }
    fn insert(&mut self, name: &[u8]) -> Result<(), Unavailable> {
        if name.is_empty()
            || name.len() > 255
            || self.count == CATALOGUE_LIMIT
            || self.index(name).is_some()
        {
            return Err(Unavailable);
        }
        self.names[self.count][..name.len()].copy_from_slice(name);
        self.lengths[self.count] = name.len();
        self.count += 1;
        Ok(())
    }
    fn own_unlinked(&mut self, name: &[u8]) -> Result<(), Unavailable> {
        let index = self.index(name).ok_or(Unavailable)?;
        if self.removed[index] {
            return Err(Unavailable);
        }
        self.removed[index] = true;
        Ok(())
    }
}

pub(crate) struct NativeEngine {
    lower: Stage,
    stage_name: Option<Metadata>,
    uid: Option<u32>,
    gid: Option<u32>,
    sealed: bool,
    expected: [Option<(usize, [u8; 32])>; IO_SLOTS],
    catalogues: [NativeCatalogue; 2],
    catalogues_captured: bool,
    recovery: bool,
    singleton_locked: bool,
    unlinked: [bool; IO_SLOTS],
    completed: bool,
    retirement: bool,
    disposition: bool,
    history: bool,
    disposition_done: bool,
    store_attempted: bool,
    current_store: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct NativeStageView<'a> {
    engine: &'a NativeEngine,
}
impl NativeStageView<'_> {
    pub(crate) fn stage_present(self) -> bool {
        self.engine.stage_name.is_some() && !self.engine.unlinked[Slot::StageDirectory as usize]
    }
    pub(crate) fn live_changed(self) -> bool {
        self.engine.lower.live.contains(&LiveRole::Renamed) || self.engine.current_store
    }
    pub(crate) fn recovery_exclusive(self) -> bool {
        self.engine.recovery
            && self.engine.singleton_locked
            && !self.engine.sealed
            && self.engine.lower.io.original(Slot::Root).is_ok()
            && self.engine.lower.io.original(Slot::Lock).is_ok()
    }
    pub(crate) fn own_completion_member(self, name: &str) -> bool {
        let slot = if name == crate::restore_retirement_candidate::RECEIPT_MEMBER {
            Slot::Scratch2
        } else if name == crate::restore_closure_model::CLOSURE_MEMBER {
            Slot::Scratch3
        } else if self.engine.disposition
            && name == crate::restore_disposition_ticket_model::TICKET_MEMBER
        {
            Slot::Scratch0
        } else if self.engine.disposition
            && name == crate::restore_disposition_complete_model::COMPLETE_MEMBER
            && !self.engine.history
        {
            Slot::Scratch1
        } else {
            return false;
        };
        self.engine.recovery
            && !self.engine.sealed
            && self.engine.lower.original[slot as usize].is_some()
            && !self.engine.unlinked[slot as usize]
    }
    pub(crate) fn pending_allowed(self, paths: &crate::desired::DesiredPaths, uid: u32) -> bool {
        if self.engine.uid.is_some_and(|original| original != uid) || self.engine.sealed {
            return false;
        }
        if !self.stage_present() {
            return !crate::pending_private_transaction::pending(paths);
        }
        let Ok(state) = self.engine.lower.io.original(Slot::State) else {
            return false;
        };
        let Some(expected) = self.engine.stage_name.as_ref() else {
            return false;
        };
        let Ok(named) = fstatat(state, PENDING_DIRECTORY, AtFlags::AT_SYMLINK_NOFOLLOW) else {
            return false;
        };
        if (
            expected.dev(),
            expected.ino(),
            expected.mode(),
            expected.uid(),
            expected.gid(),
        ) != (
            named.st_dev,
            named.st_ino,
            named.st_mode,
            named.st_uid,
            named.st_gid,
        ) {
            return false;
        }
        for (slot, name) in STAGED
            .into_iter()
            .zip(MEMBERS.into_iter().chain(std::iter::once(READY_MEMBER)))
        {
            if self.engine.unlinked[slot as usize] {
                let Ok(directory) = self.engine.lower.io.original(Slot::StageDirectory) else {
                    return false;
                };
                if !matches!(
                    fstatat(directory, name, AtFlags::AT_SYMLINK_NOFOLLOW),
                    Err(nix::errno::Errno::ENOENT)
                ) {
                    return false;
                }
                continue;
            }
            if let Some(original) = self.engine.lower.original[slot as usize].as_ref() {
                let Ok(file) = self.engine.lower.io.original(slot) else {
                    return false;
                };
                let Ok(current) = file.metadata() else {
                    return false;
                };
                let Ok(directory) = self.engine.lower.io.original(Slot::StageDirectory) else {
                    return false;
                };
                let Ok(named) = fstatat(directory, name, AtFlags::AT_SYMLINK_NOFOLLOW) else {
                    return false;
                };
                if !same_member(original, &current)
                    || original.gid() != current.gid()
                    || (
                        original.dev(),
                        original.ino(),
                        original.mode(),
                        original.uid(),
                        original.gid(),
                        original.nlink(),
                        original.len(),
                        original.mtime(),
                        original.mtime_nsec(),
                        original.ctime(),
                        original.ctime_nsec(),
                    ) != (
                        named.st_dev,
                        named.st_ino,
                        named.st_mode,
                        named.st_uid,
                        named.st_gid,
                        named.st_nlink,
                        named.st_size as u64,
                        named.st_mtime,
                        named.st_mtime_nsec,
                        named.st_ctime,
                        named.st_ctime_nsec,
                    )
                {
                    return false;
                }
            }
        }
        !crate::routing_preset::pending(paths) && [
            "restore-finalization.pending", crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        ].iter().all(|name| self.own_completion_member(name) || matches!(std::fs::symlink_metadata(paths.directory.join(name)), Err(error) if error.kind() == std::io::ErrorKind::NotFound))
    }
}
impl NativeEngine {
    pub(crate) fn reserve() -> Self {
        Self {
            lower: Stage::reserve_canonical(),
            stage_name: None,
            uid: None,
            gid: None,
            sealed: false,
            expected: [None; IO_SLOTS],
            catalogues: [NativeCatalogue::empty(), NativeCatalogue::empty()],
            catalogues_captured: false,
            recovery: false,
            singleton_locked: false,
            unlinked: [false; IO_SLOTS],
            completed: false,
            retirement: false,
            disposition: false,
            history: false,
            disposition_done: false,
            store_attempted: false,
            current_store: false,
        }
    }
    pub(crate) fn disposition_ready(&self) -> bool {
        self.completed
            && self.disposition_done
            && self.history
            && !self.sealed
            && self.recovery
            && self.singleton_locked
    }
    pub(crate) fn revoke_native(&mut self) {
        self.sealed = true;
        self.lower.revoke();
    }
    pub(crate) fn begin_native_store_mutation(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        until: Instant,
    ) -> Result<(), FirstError> {
        let result = self.begin_store_inner(origin, until);
        if result.is_err() {
            self.revoke_native();
        }
        result
    }
    fn begin_store_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        until: Instant,
    ) -> Result<(), FirstError> {
        self.gate(origin, until)?;
        if !self.disposition_ready()
            || self.store_attempted
            || self.lower.original[NATIVE_CURRENT_STORE as usize].is_some()
        {
            return Err(FirstError::StillFenced);
        }
        self.store_attempted = true; // consumes before the ordinary writer effect
        Ok(())
    }
    pub(crate) fn finish_native_store_mutation(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        expected: &[u8],
        changed: bool,
        until: Instant,
    ) -> Result<(), FirstError> {
        self.finish_store_inner(origin, expected, changed, until)
    }
    fn finish_store_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        expected: &[u8],
        changed: bool,
        until: Instant,
    ) -> Result<(), FirstError> {
        let result = (|| {
            if !self.store_attempted || !self.disposition_ready() {
                return Err(FirstError::StillFenced);
            }
            if changed {
                // Only a positive ordinary commit can advance this exact OLD
                // original's known link/ctime transition. On Err do not guess.
                let previous = if self.lower.live[0] == LiveRole::Renamed {
                    NATIVE_ROLLBACKS[0].0
                } else {
                    Slot::OldStore
                };
                self.lower
                    .io
                    .perform(
                        previous,
                        || tick(until),
                        |file| {
                            let before = self.lower.original[previous as usize]
                                .as_ref()
                                .ok_or(Unavailable)?;
                            let after = file.metadata().map_err(|_| Unavailable)?;
                            if !same_after_own_rename(before, &after, 0) {
                                return Err(Unavailable);
                            }
                            self.lower.original[previous as usize] = Some(after);
                            Ok(())
                        },
                    )
                    .map_err(|_| FirstError::StillFenced)?;
                self.lower
                    .io
                    .child(
                        ChildPlan {
                            parent: Slot::Config,
                            slot: NATIVE_CURRENT_STORE,
                            name: LIVE[0].1,
                            flags: OFlag::O_RDONLY | OFlag::O_NONBLOCK,
                            mode: Mode::empty(),
                        },
                        || tick(until),
                        |_| Ok(()),
                    )
                    .map_err(|_| FirstError::StillFenced)?;
                self.shape(NATIVE_CURRENT_STORE, false, until)
                    .map_err(|_| FirstError::StillFenced)?;
                self.lower
                    .verify_member(
                        Slot::Config,
                        NATIVE_CURRENT_STORE,
                        LIVE[0].1,
                        expected,
                        until,
                    )
                    .map_err(|_| FirstError::StillFenced)?;
                self.expected[NATIVE_CURRENT_STORE as usize] =
                    Some((expected.len(), Sha256::digest(expected).into()));
                self.current_store = true; // only full retained current-byte proof
            } else {
                self.lower
                    .verify_member(Slot::Config, Slot::OldStore, LIVE[0].1, expected, until)
                    .map_err(|_| FirstError::StillFenced)?;
            }
            self.gate(origin, until)
        })();
        if result.is_err() {
            self.revoke_native();
        }
        result
    }
    #[cfg(test)]
    pub(crate) fn recovery_held(&self) -> bool {
        self.view().recovery_exclusive()
    }
    fn view(&self) -> NativeStageView<'_> {
        NativeStageView { engine: self }
    }
    fn gate<O: NativeGate>(&mut self, origin: &mut O, until: Instant) -> Result<(), FirstError> {
        tick(until).map_err(|_| FirstError::StillFenced)?;
        self.check_original_bytes(until)?;
        self.check_bindings(until)
            .map_err(|_| FirstError::StillFenced)?;
        origin.check(self.view())?;
        self.check_bindings(until)
            .map_err(|_| FirstError::StillFenced)?;
        self.check_original_bytes(until)?;
        tick(until).map_err(|_| FirstError::StillFenced)
    }

    fn scan_catalogue(
        &mut self,
        slot: Slot,
        capture: Option<usize>,
        additions: &[&str],
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.lower.io.perform(
            slot,
            || tick(until),
            |file| {
                if seek(file, SeekFrom::Start(0)).map_err(|_| Unavailable)? != 0 {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )?;
        let catalogue_index = usize::from(matches!(slot, Slot::State));
        let original = &mut self.catalogues[catalogue_index];
        self.lower.io.perform(
            slot,
            || tick(until),
            |file| {
                let mut iterator = RawDir::new(file, &mut self.lower.directory_buffer);
                let mut seen = [false; CATALOGUE_LIMIT];
                let mut extra = [false; 3];
                let mut dots = [false; 2];
                for _ in 0..CATALOGUE_LIMIT + 6 {
                    tick(until)?;
                    let row = iterator.next();
                    tick(until)?;
                    let Some(row) = row else {
                        return if dots == [true; 2]
                            && (capture.is_some()
                                || ((0..original.count)
                                    .all(|index| seen[index] != original.removed[index])
                                    && extra[..additions.len()].iter().all(|b| *b)))
                        {
                            Ok(())
                        } else {
                            Err(Unavailable)
                        };
                    };
                    let row = row.map_err(|_| Unavailable)?;
                    let name = row.file_name().to_bytes();
                    if let Some(i) = [b".".as_slice(), b"..".as_slice()]
                        .iter()
                        .position(|dot| *dot == name)
                    {
                        if dots[i] {
                            return Err(Unavailable);
                        }
                        dots[i] = true;
                    } else if capture.is_some() {
                        original.insert(name)?;
                    } else if let Some(i) = original.index(name) {
                        if seen[i] || original.removed[i] {
                            return Err(Unavailable);
                        }
                        seen[i] = true;
                    } else if let Some(i) =
                        additions.iter().position(|extra| extra.as_bytes() == name)
                    {
                        if extra[i] {
                            return Err(Unavailable);
                        }
                        extra[i] = true;
                    } else {
                        return Err(Unavailable);
                    }
                }
                Err(Unavailable)
            },
        )
    }

    fn check_bindings(&mut self, until: Instant) -> Result<(), Unavailable> {
        if !self.catalogues_captured {
            return Ok(());
        }
        for slot in [Slot::Config, Slot::State, Slot::Run] {
            let before = self.lower.original[slot as usize]
                .as_ref()
                .ok_or(Unavailable)?;
            self.lower.io.perform(
                slot,
                || tick(until),
                |file| {
                    let after = file.metadata().map_err(|_| Unavailable)?;
                    let mut attributes = [0; 1];
                    if !same_directory(before, &after)
                        || before.gid() != after.gid()
                        || flistxattr(file, &mut attributes).map_err(|_| Unavailable)? != 0
                    {
                        return Err(Unavailable);
                    }
                    Ok(())
                },
            )?;
        }
        let replacements = if self.recovery {
            NATIVE_ROLLBACKS
        } else {
            NATIVE_REPLACEMENTS
        };
        if self.recovery {
            self.check_recovery_singleton(until)?;
            if self.lower.original[Slot::Terminal as usize].is_none()
                && !matches!(
                    fstatat(
                        self.lower.io.original(Slot::State)?,
                        TERMINAL,
                        AtFlags::AT_SYMLINK_NOFOLLOW
                    ),
                    Err(nix::errno::Errno::ENOENT)
                )
            {
                return Err(Unavailable);
            }
            for (slot, name) in NATIVE_ROLLBACKS {
                if self.lower.original[slot as usize].is_none()
                    && !matches!(
                        fstatat(
                            self.lower.io.original(Slot::Config)?,
                            name,
                            AtFlags::AT_SYMLINK_NOFOLLOW
                        ),
                        Err(nix::errno::Errno::ENOENT)
                    )
                {
                    return Err(Unavailable);
                }
            }
        }
        for index in 0..2 {
            let (old, name) = LIVE[index];
            if index == 0 && self.current_store {
                let before = self.lower.original[old as usize]
                    .as_ref()
                    .ok_or(Unavailable)?;
                let after = self
                    .lower
                    .io
                    .original(old)?
                    .metadata()
                    .map_err(|_| Unavailable)?;
                if before.nlink() != 0
                    || !same_member(before, &after)
                    || before.gid() != after.gid()
                {
                    return Err(Unavailable);
                }
                self.lower
                    .binding(Slot::Config, NATIVE_CURRENT_STORE, name, false, until)?;
            } else if self.lower.live[index] == LiveRole::Renamed {
                let original = self.lower.original[old as usize]
                    .as_ref()
                    .ok_or(Unavailable)?;
                let current = self
                    .lower
                    .io
                    .original(old)?
                    .metadata()
                    .map_err(|_| Unavailable)?;
                if original.nlink() != 0
                    || !same_member(original, &current)
                    || original.gid() != current.gid()
                {
                    return Err(Unavailable);
                }
                self.lower
                    .binding(Slot::Config, replacements[index].0, name, false, until)?;
            } else {
                self.lower.binding(Slot::Config, old, name, false, until)?;
            }
        }
        let mut extra_config = [""; 2];
        let mut count = 0;
        for (index, (slot, name)) in replacements.into_iter().enumerate() {
            if self.lower.original[slot as usize].is_some()
                && self.lower.live[index] != LiveRole::Renamed
            {
                self.lower.binding(Slot::Config, slot, name, false, until)?;
                if self.catalogues[0].index(name.as_bytes()).is_none() {
                    extra_config[count] = name;
                    count += 1;
                }
            }
        }
        if self.recovery {
            for (slot, name) in RECOVERED_NEW {
                if self.lower.original[slot as usize].is_some() && !self.unlinked[slot as usize] {
                    self.lower.binding(Slot::Config, slot, name, false, until)?;
                } else if !matches!(
                    fstatat(
                        self.lower.io.original(Slot::Config)?,
                        name,
                        AtFlags::AT_SYMLINK_NOFOLLOW
                    ),
                    Err(nix::errno::Errno::ENOENT)
                ) {
                    return Err(Unavailable);
                }
            }
        }
        self.scan_catalogue(Slot::Config, None, &extra_config[..count], until)?;
        let mut extra_state = [""; 3];
        let mut count = 0;
        if self.stage_name.is_some() && !self.unlinked[Slot::StageDirectory as usize] {
            self.lower.binding(
                Slot::State,
                Slot::StageDirectory,
                PENDING_DIRECTORY,
                true,
                until,
            )?;
            if self.catalogues[1]
                .index(PENDING_DIRECTORY.as_bytes())
                .is_none()
            {
                extra_state[count] = PENDING_DIRECTORY;
                count += 1;
            }
            let mut expected = [""; 5];
            let mut staged_count = 0;
            for (slot, name) in STAGED
                .into_iter()
                .zip(MEMBERS.into_iter().chain(std::iter::once(READY_MEMBER)))
            {
                if self.lower.original[slot as usize].is_some() && !self.unlinked[slot as usize] {
                    self.lower
                        .binding(Slot::StageDirectory, slot, name, false, until)?;
                    expected[staged_count] = name;
                    staged_count += 1;
                }
            }
            self.lower
                .catalogue_inner(Slot::StageDirectory, &expected[..staged_count], until)?;
        } else if self.unlinked[Slot::StageDirectory as usize] {
            if !matches!(
                fstatat(
                    self.lower.io.original(Slot::State)?,
                    PENDING_DIRECTORY,
                    AtFlags::AT_SYMLINK_NOFOLLOW
                ),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(Unavailable);
            }
            let before = self.lower.original[Slot::StageDirectory as usize]
                .as_ref()
                .ok_or(Unavailable)?;
            let after = self
                .lower
                .io
                .original(Slot::StageDirectory)?
                .metadata()
                .map_err(|_| Unavailable)?;
            if !same_member(before, &after) || before.gid() != after.gid() || after.nlink() != 0 {
                return Err(Unavailable);
            }
        }
        for (slot, name) in [
            (Slot::Intent, INTENT),
            (Slot::Terminal, TERMINAL),
            (
                Slot::Scratch2,
                crate::restore_retirement_candidate::RECEIPT_MEMBER,
            ),
            (Slot::Scratch3, crate::restore_closure_model::CLOSURE_MEMBER),
            (
                Slot::Scratch0,
                crate::restore_disposition_ticket_model::TICKET_MEMBER,
            ),
            (
                Slot::Scratch1,
                if self.history {
                    NATIVE_HISTORY
                } else {
                    crate::restore_disposition_complete_model::COMPLETE_MEMBER
                },
            ),
        ] {
            if self.lower.original[slot as usize].is_some() && !self.unlinked[slot as usize] {
                self.lower.binding(Slot::State, slot, name, false, until)?;
                if self.catalogues[1].index(name.as_bytes()).is_none() {
                    extra_state[count] = name;
                    count += 1;
                }
            } else if self.unlinked[slot as usize]
                && !matches!(
                    fstatat(
                        self.lower.io.original(Slot::State)?,
                        name,
                        AtFlags::AT_SYMLINK_NOFOLLOW
                    ),
                    Err(nix::errno::Errno::ENOENT)
                )
            {
                return Err(Unavailable);
            }
        }
        self.scan_catalogue(Slot::State, None, &extra_state[..count], until)
    }
    fn check_original_bytes(&self, until: Instant) -> Result<(), FirstError> {
        for (index, expected) in self.expected.iter().enumerate() {
            let Some((size, wanted)) = expected else {
                continue;
            };
            let slot = match index {
                n if n == Slot::OldStore as usize => Slot::OldStore,
                n if n == Slot::OldTemplate as usize => Slot::OldTemplate,
                n if n == Slot::StageOldStore as usize => Slot::StageOldStore,
                n if n == Slot::StageOldTemplate as usize => Slot::StageOldTemplate,
                n if n == Slot::StageNewStore as usize => Slot::StageNewStore,
                n if n == Slot::StageNewTemplate as usize => Slot::StageNewTemplate,
                n if n == Slot::StageReady as usize => Slot::StageReady,
                n if n == Slot::Intent as usize => Slot::Intent,
                n if n == Slot::Terminal as usize => Slot::Terminal,
                n if n == Slot::ReplacementStore as usize => Slot::ReplacementStore,
                n if n == Slot::ReplacementTemplate as usize => Slot::ReplacementTemplate,
                n if n == Slot::RollbackStore as usize => Slot::RollbackStore,
                n if n == Slot::RollbackTemplate as usize => Slot::RollbackTemplate,
                n if n == Slot::Scratch4 as usize && self.recovery => Slot::Scratch4,
                n if n == Slot::Scratch5 as usize && self.recovery => Slot::Scratch5,
                n if n == Slot::Scratch2 as usize && self.retirement => Slot::Scratch2,
                n if n == Slot::Scratch3 as usize && self.retirement => Slot::Scratch3,
                n if n == Slot::Scratch0 as usize && self.disposition => Slot::Scratch0,
                n if n == Slot::Scratch1 as usize && self.disposition => Slot::Scratch1,
                n if n == NATIVE_CURRENT_STORE as usize && self.current_store => {
                    NATIVE_CURRENT_STORE
                }
                _ => return Err(FirstError::StillFenced),
            };
            let file = self
                .lower
                .io
                .original(slot)
                .map_err(|_| FirstError::StillFenced)?;
            let original = self.lower.original[index]
                .as_ref()
                .ok_or(FirstError::StillFenced)?;
            let current = file.metadata().map_err(|_| FirstError::StillFenced)?;
            if !same_member(original, &current) || current.gid() != original.gid() {
                return Err(FirstError::StillFenced);
            }
            let mut digest = Sha256::new();
            let mut offset = 0usize;
            loop {
                tick(until).map_err(|_| FirstError::StillFenced)?;
                let mut chunk = [0; 4096];
                let remaining = size
                    .checked_add(1)
                    .and_then(|bound| bound.checked_sub(offset))
                    .ok_or(FirstError::StillFenced)?;
                let take = remaining.min(chunk.len());
                let n = file
                    .read_at(&mut chunk[..take], offset as u64)
                    .map_err(|_| FirstError::StillFenced)?;
                tick(until).map_err(|_| FirstError::StillFenced)?;
                if n == 0 {
                    break;
                }
                offset = offset.checked_add(n).ok_or(FirstError::StillFenced)?;
                if offset > *size {
                    return Err(FirstError::StillFenced);
                }
                digest.update(&chunk[..n]);
            }
            if offset != *size || digest.finalize().as_slice() != wanted {
                return Err(FirstError::StillFenced);
            }
            let after = file.metadata().map_err(|_| FirstError::StillFenced)?;
            if !same_member(original, &after) || after.gid() != original.gid() {
                return Err(FirstError::StillFenced);
            }
        }
        Ok(())
    }
    fn shape(&mut self, slot: Slot, directory: bool, until: Instant) -> Result<(), Unavailable> {
        self.lower.io.perform(
            slot,
            || tick(until),
            |file| {
                self.lower.original[slot as usize] =
                    Some(file.metadata().map_err(|_| Unavailable)?);
                Ok(())
            },
        )?;
        let m = self.lower.original[slot as usize]
            .as_ref()
            .ok_or(Unavailable)?;
        if Some(m.uid()) != self.uid
            || Some(m.gid()) != self.gid
            || m.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
            || if directory {
                !m.is_dir()
            } else {
                !m.is_file() || m.nlink() != 1
            }
        {
            return Err(Unavailable);
        }
        self.lower.io.perform(
            slot,
            || tick(until),
            |file| {
                let mut data = [0; 1];
                if flistxattr(file, &mut data).map_err(|_| Unavailable)? != 0 {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )
    }
    fn clone_file(
        &mut self,
        slot: Slot,
        source: &File,
        directory: bool,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.lower.io.clone_original(slot, source, || tick(until))?;
        self.shape(slot, directory, until)
    }
    fn write<O: NativeGate>(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        bytes: &[u8],
        origin: &mut O,
        until: Instant,
    ) -> Result<(), FirstError> {
        self.gate(origin, until)?;
        self.lower
            .io
            .child(
                ChildPlan {
                    parent,
                    slot,
                    name,
                    flags: OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NONBLOCK,
                    mode: Mode::S_IRUSR | Mode::S_IWUSR,
                },
                || tick(until),
                |_| Ok(()),
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.shape(slot, false, until)
            .map_err(|_| FirstError::StillFenced)?;
        let mut done = 0;
        while done < bytes.len() {
            self.lower
                .io
                .perform(
                    slot,
                    || tick(until),
                    |mut file| {
                        let n = file.write(&bytes[done..]).map_err(|_| Unavailable)?;
                        if n == 0 || n > bytes.len() - done {
                            return Err(Unavailable);
                        }
                        done += n;
                        Ok(())
                    },
                )
                .map_err(|_| FirstError::StillFenced)?;
        }
        self.lower
            .io
            .perform(
                slot,
                || tick(until),
                |file| file.sync_all().map_err(|_| Unavailable),
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.shape(slot, false, until)
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .verify_member(parent, slot, name, bytes, until)
            .map_err(|_| FirstError::StillFenced)?;
        self.expected[slot as usize] = Some((bytes.len(), Sha256::digest(bytes).into()));
        self.lower
            .io
            .perform(
                parent,
                || tick(until),
                |file| file.sync_all().map_err(|_| Unavailable),
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.gate(origin, until)
    }

    fn check_recovery_singleton(&mut self, until: Instant) -> Result<(), Unavailable> {
        if !self.singleton_locked {
            return Err(Unavailable);
        }
        self.lower
            .binding(Slot::Run, Slot::Root, "omavless", true, until)?;
        self.lower
            .binding(Slot::Root, Slot::Lock, crate::OWNER_LOCK_NAME, false, until)?;
        self.lower.catalogue_inner(
            Slot::Root,
            &[crate::OWNER_LOCK_NAME, crate::SOCKET_NAME],
            until,
        )?;
        let socket = self.lower.original[Slot::Scratch6 as usize]
            .as_ref()
            .ok_or(Unavailable)?;
        let held = self
            .lower
            .io
            .original(Slot::Scratch6)?
            .metadata()
            .map_err(|_| Unavailable)?;
        let named = fstatat(
            self.lower.io.original(Slot::Root)?,
            crate::SOCKET_NAME,
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )
        .map_err(|_| Unavailable)?;
        if !same_member(socket, &held)
            || socket.gid() != held.gid()
            || (
                socket.dev(),
                socket.ino(),
                socket.mode(),
                socket.uid(),
                socket.gid(),
                socket.nlink(),
                socket.len(),
                socket.mtime(),
                socket.mtime_nsec(),
                socket.ctime(),
                socket.ctime_nsec(),
            ) != (
                named.st_dev,
                named.st_ino,
                named.st_mode,
                named.st_uid,
                named.st_gid,
                named.st_nlink,
                named.st_size as u64,
                named.st_mtime,
                named.st_mtime_nsec,
                named.st_ctime,
                named.st_ctime_nsec,
            )
        {
            return Err(Unavailable);
        }
        Ok(())
    }

    fn capture_recovery_singleton(&mut self, until: Instant) -> Result<(), FirstError> {
        // Existing original singleton directory and owner.lock are charged
        // in Root/Lock of the same36-role ledger BEFORE any postchecks.
        self.lower
            .io
            .child(
                ChildPlan {
                    parent: Slot::Run,
                    slot: Slot::Root,
                    name: "omavless",
                    flags: OFlag::O_RDONLY | OFlag::O_DIRECTORY,
                    mode: Mode::empty(),
                },
                || tick(until),
                |_| Ok(()),
            )
            .map_err(|_| FirstError::Admission)?;
        self.shape(Slot::Root, true, until)
            .map_err(|_| FirstError::Admission)?;
        self.lower
            .binding(Slot::Run, Slot::Root, "omavless", true, until)
            .map_err(|_| FirstError::Admission)?;
        self.lower
            .io
            .child(
                ChildPlan {
                    parent: Slot::Root,
                    slot: Slot::Lock,
                    name: crate::OWNER_LOCK_NAME,
                    flags: OFlag::O_RDONLY | OFlag::O_NONBLOCK,
                    mode: Mode::empty(),
                },
                || tick(until),
                |_| Ok(()),
            )
            .map_err(|_| FirstError::Admission)?;
        self.shape(Slot::Lock, false, until)
            .map_err(|_| FirstError::Admission)?;
        self.lower
            .io
            .perform(
                Slot::Lock,
                || tick(until),
                |file| {
                    rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
                        .map_err(|_| Unavailable)?;
                    self.singleton_locked = true; // positive original syscall only
                    Ok(())
                },
            )
            .map_err(|_| FirstError::Admission)?;
        self.lower
            .binding(Slot::Root, Slot::Lock, crate::OWNER_LOCK_NAME, false, until)
            .map_err(|_| FirstError::Admission)?;
        // The old endpoint is retained ONLY as an inert named object. The
        // NEW existing-owner.lock flock excludes RuntimeServer bind; no
        // connection, rebound socket, old PID or creation proof is adopted.
        self.lower
            .io
            .child(
                ChildPlan {
                    parent: Slot::Root,
                    slot: Slot::Scratch6,
                    name: crate::SOCKET_NAME,
                    flags: OFlag::O_PATH,
                    mode: Mode::empty(),
                },
                || tick(until),
                |_| Ok(()),
            )
            .map_err(|_| FirstError::Admission)?;
        let socket = self
            .lower
            .io
            .original(Slot::Scratch6)
            .map_err(|_| FirstError::Admission)?
            .metadata()
            .map_err(|_| FirstError::Admission)?;
        use std::os::unix::fs::FileTypeExt;
        if !socket.file_type().is_socket()
            || socket.uid() != self.uid.ok_or(FirstError::Admission)?
            || socket.gid() != nix::unistd::getgid().as_raw()
            || socket.mode() & 0o7777 != 0o600
            || socket.nlink() != 1
        {
            return Err(FirstError::Admission);
        }
        self.lower.original[Slot::Scratch6 as usize] = Some(socket);
        Ok(())
    }

    fn recover_member(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        maximum: usize,
        until: Instant,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, FirstError> {
        use std::os::unix::fs::FileExt;
        self.lower
            .io
            .child(
                ChildPlan {
                    parent,
                    slot,
                    name,
                    flags: OFlag::O_RDONLY | OFlag::O_NONBLOCK,
                    mode: Mode::empty(),
                },
                || tick(until),
                |_| Ok(()),
            )
            .map_err(|_| FirstError::Admission)?;
        self.shape(slot, false, until)
            .map_err(|_| FirstError::Admission)?;
        let file = self
            .lower
            .io
            .original(slot)
            .map_err(|_| FirstError::Admission)?;
        let length = usize::try_from(file.metadata().map_err(|_| FirstError::Admission)?.len())
            .map_err(|_| FirstError::Admission)?;
        if length > maximum {
            return Err(FirstError::Admission);
        }
        let mut bytes = zeroize::Zeroizing::new(vec![0; length + 1]);
        let mut done = 0;
        while done < bytes.len() {
            tick(until).map_err(|_| FirstError::Admission)?;
            let n = file
                .read_at(&mut bytes[done..], done as u64)
                .map_err(|_| FirstError::Admission)?;
            tick(until).map_err(|_| FirstError::Admission)?;
            if n == 0 {
                break;
            }
            done += n;
        }
        if done != length {
            return Err(FirstError::Admission);
        }
        bytes.truncate(length);
        self.lower
            .verify_member(parent, slot, name, &bytes, until)
            .map_err(|_| FirstError::Admission)?;
        self.expected[slot as usize] = Some((bytes.len(), Sha256::digest(&bytes).into()));
        Ok(bytes)
    }

    fn own_unlink<O: NativeGate>(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        directory: bool,
        origin: &mut O,
        until: Instant,
    ) -> Result<(), FirstError> {
        if self.unlinked[slot as usize] {
            return Err(FirstError::StillFenced);
        }
        self.gate(origin, until)?;
        self.lower
            .binding(parent, slot, name, directory, until)
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .perform(
                parent,
                || tick(until),
                |file| {
                    nix::unistd::unlinkat(
                        file,
                        name,
                        if directory {
                            nix::unistd::UnlinkatFlags::RemoveDir
                        } else {
                            nix::unistd::UnlinkatFlags::NoRemoveDir
                        },
                    )
                    .map_err(|_| Unavailable)?;
                    self.unlinked[slot as usize] = true; // reported success BEFORE postcheck
                    if matches!(parent, Slot::Config | Slot::State) {
                        let index = usize::from(matches!(parent, Slot::State));
                        if self.catalogues[index].index(name.as_bytes()).is_some() {
                            self.catalogues[index].own_unlinked(name.as_bytes())?;
                        }
                    }
                    Ok(())
                },
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .perform(
                slot,
                || tick(until),
                |file| {
                    let before = self.lower.original[slot as usize]
                        .as_ref()
                        .ok_or(Unavailable)?;
                    let after = file.metadata().map_err(|_| Unavailable)?;
                    let same = if directory {
                        same_directory(before, &after)
                            && before.gid() == after.gid()
                            && after.nlink() == 0
                    } else {
                        same_after_own_rename(before, &after, 0)
                    };
                    if !same {
                        return Err(Unavailable);
                    }
                    self.lower.original[slot as usize] = Some(after);
                    Ok(())
                },
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .perform(
                parent,
                || tick(until),
                |file| file.sync_all().map_err(|_| Unavailable),
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.gate(origin, until)
    }

    fn retire_native_aborted<O: NativeGate>(
        &mut self,
        origin: &mut O,
        members: [&[u8]; 4],
        intent: &[u8],
        terminal: &[u8],
        until: Instant,
    ) -> Result<(), FirstError> {
        self.retirement = true;
        let terminal_record =
            DecisionRecord::decode(terminal).map_err(|_| FirstError::Admission)?;
        let receipt =
            crate::restore_retirement_candidate::RetirementReceipt::from_retained_terminal(
                &terminal_record,
                members,
            )
            .map_err(|_| FirstError::Admission)?;
        let pending = crate::restore_retirement_candidate::RECEIPT_MEMBER;
        let closure_name = crate::restore_closure_model::CLOSURE_MEMBER;
        for name in [pending, closure_name] {
            if !matches!(
                fstatat(
                    self.lower
                        .io
                        .original(Slot::State)
                        .map_err(|_| FirstError::Admission)?,
                    name,
                    AtFlags::AT_SYMLINK_NOFOLLOW
                ),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(FirstError::Admission);
            }
        }
        self.gate(origin, until)?;
        self.write(
            Slot::State,
            Slot::Scratch2,
            pending,
            &receipt.encode(),
            origin,
            until,
        )?;
        for (slot, name) in RECOVERED_NEW {
            if self.lower.original[slot as usize].is_some() {
                self.own_unlink(Slot::Config, slot, name, false, origin, until)?;
            }
        }
        for (slot, name) in STAGED
            .into_iter()
            .zip(MEMBERS.into_iter().chain(std::iter::once(READY_MEMBER)))
        {
            self.own_unlink(Slot::StageDirectory, slot, name, false, origin, until)?;
        }
        self.own_unlink(
            Slot::State,
            Slot::StageDirectory,
            PENDING_DIRECTORY,
            true,
            origin,
            until,
        )?;
        self.own_unlink(Slot::State, Slot::Terminal, TERMINAL, false, origin, until)?;
        self.own_unlink(Slot::State, Slot::Intent, INTENT, false, origin, until)?;
        // Receipt remains the durable existence fence through every destructive
        // operation. Closure is made durable/read back BEFORE its removal.
        let closure = crate::restore_closure_model::ClosureRecord::from_verified_receipt(&receipt)
            .map_err(|_| FirstError::StillFenced)?;
        self.write(
            Slot::State,
            Slot::Scratch3,
            closure_name,
            &closure.encode(),
            origin,
            until,
        )?;
        self.lower
            .verify_member(
                Slot::State,
                Slot::Scratch2,
                pending,
                &receipt.encode(),
                until,
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .verify_member(
                Slot::State,
                Slot::Scratch3,
                closure_name,
                &closure.encode(),
                until,
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.own_unlink(Slot::State, Slot::Scratch2, pending, false, origin, until)?;
        for index in 0..2 {
            self.lower
                .verify_member(
                    Slot::Config,
                    LIVE[index].0,
                    LIVE[index].1,
                    members[index],
                    until,
                )
                .map_err(|_| FirstError::StillFenced)?;
        }
        self.check_original_bytes(until)?; // includes held unlinked Intent/Aborted/data
        if !DecisionRecord::decode(intent)
            .map_err(|_| FirstError::StillFenced)?
            .is_intent_of(&terminal_record)
            || !closure.matches_pending(&receipt)
            || !receipt.matches_pair(members[0], members[1])
        {
            return Err(FirstError::StillFenced);
        }
        self.gate(origin, until)?;
        self.completed = true; // only after last ORIGINAL origin + all durable reads
        Ok(())
    }

    /// Only fresh Intent/MIXED, authenticated NEW, and the newly acquired
    /// existing singleton/operation leases. No former actor/session is imported.
    pub(crate) fn reconcile_native_mixed(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        backup: &omavless_domain::private_backup::OpenedBackup,
    ) -> Result<(), FirstError> {
        self.recover_native(origin, backup, false)
    }
    pub(crate) fn complete_native_aborted(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        backup: &omavless_domain::private_backup::OpenedBackup,
    ) -> Result<(), FirstError> {
        self.recover_native(origin, backup, true)
    }
    pub(crate) fn check_native_completed(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        until: Instant,
    ) -> Result<(), FirstError> {
        let result = self.check_native_completed_inner(origin, until);
        if result.is_err() {
            self.sealed = true;
            self.lower.revoke();
        }
        result
    }
    fn check_native_completed_inner(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.completed || self.sealed || !self.recovery || !self.singleton_locked {
            return Err(FirstError::StillFenced);
        }
        let receipt = crate::restore_retirement_candidate::RECEIPT_MEMBER;
        for name in [PENDING_DIRECTORY, INTENT, TERMINAL, receipt] {
            if !matches!(
                fstatat(
                    self.lower
                        .io
                        .original(Slot::State)
                        .map_err(|_| FirstError::StillFenced)?,
                    name,
                    AtFlags::AT_SYMLINK_NOFOLLOW
                ),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(FirstError::StillFenced);
            }
        }
        if !self.unlinked[Slot::Scratch2 as usize]
            || self.unlinked[Slot::Scratch3 as usize] != self.disposition_done
            || self.expected[Slot::Scratch3 as usize].is_none()
        {
            return Err(FirstError::StillFenced);
        }
        self.gate(origin, until)
    }

    /// Consumes only this engine's positive completion, never decoded history.
    /// The exact existing original lease and current origin stay borrowed.
    pub(crate) fn dispose_native_completed(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
    ) -> Result<(), FirstError> {
        let until = Instant::now() + std::time::Duration::from_secs(15);
        let uid = origin.uid();
        let generation = origin.generation();
        let mut desired = zeroize::Zeroizing::new(Vec::new());
        desired
            .try_reserve_exact(origin.desired_bytes().len())
            .map_err(|_| FirstError::StillFenced)?;
        desired.extend_from_slice(origin.desired_bytes());
        let result = self.dispose_completed_inner(origin, uid, generation, &desired, until);
        if result.is_err() {
            self.sealed = true;
            self.lower.revoke();
        }
        result
    }

    fn dispose_completed_inner<O: NativeGate>(
        &mut self,
        origin: &mut O,
        uid: u32,
        generation: u64,
        desired: &[u8],
        until: Instant,
    ) -> Result<(), FirstError> {
        use crate::restore_disposition_complete_model::{COMPLETE_MEMBER, CompleteRecord};
        use crate::restore_disposition_ticket_model::{TICKET_MEMBER, Ticket};
        if !self.completed
            || self.sealed
            || !self.recovery
            || !self.singleton_locked
            || self.disposition
            || self.unlinked[Slot::Scratch3 as usize]
            || self.uid != Some(uid)
        {
            return Err(FirstError::StillFenced);
        }
        self.disposition = true; // one attempt consumed before first publisher effect
        self.gate(origin, until)?;
        let state = self
            .lower
            .io
            .original(Slot::State)
            .map_err(|_| FirstError::StillFenced)?;
        for name in [TICKET_MEMBER, COMPLETE_MEMBER, NATIVE_HISTORY] {
            if !matches!(
                fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(FirstError::StillFenced);
            }
        }
        let size = crate::restore_closure_model::RECORD_BYTES;
        let mut raw = [0; crate::restore_closure_model::RECORD_BYTES];
        let closure_file = self
            .lower
            .io
            .original(Slot::Scratch3)
            .map_err(|_| FirstError::StillFenced)?;
        tick(until).map_err(|_| FirstError::StillFenced)?;
        closure_file
            .read_exact_at(&mut raw, 0)
            .map_err(|_| FirstError::StillFenced)?;
        tick(until).map_err(|_| FirstError::StillFenced)?;
        let closure = crate::restore_closure_model::ClosureRecord::decode(&raw)
            .map_err(|_| FirstError::StillFenced)?;
        if self.expected[Slot::Scratch3 as usize] != Some((size, Sha256::digest(raw).into())) {
            return Err(FirstError::StillFenced);
        }
        // These pure records are consistency data, not producers of authority.
        let ticket = Ticket::from_bound_closure(&closure, uid, generation, Some(desired))
            .ok_or(FirstError::StillFenced)?;
        let complete = CompleteRecord::from_ticket(&ticket).ok_or(FirstError::StillFenced)?;
        self.write(
            Slot::State,
            Slot::Scratch0,
            TICKET_MEMBER,
            &ticket.encode(),
            origin,
            until,
        )?;
        self.write(
            Slot::State,
            Slot::Scratch1,
            COMPLETE_MEMBER,
            &complete.encode(),
            origin,
            until,
        )?;
        self.own_unlink(
            Slot::State,
            Slot::Scratch3,
            crate::restore_closure_model::CLOSURE_MEMBER,
            false,
            origin,
            until,
        )?;
        self.own_unlink(
            Slot::State,
            Slot::Scratch0,
            TICKET_MEMBER,
            false,
            origin,
            until,
        )?;
        self.gate(origin, until)?;
        self.lower
            .binding(Slot::State, Slot::Scratch1, COMPLETE_MEMBER, false, until)
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .perform(
                Slot::State,
                || tick(until),
                |state| {
                    #[cfg(all(target_os = "linux", target_env = "gnu"))]
                    {
                        nix::fcntl::renameat2(
                            state,
                            COMPLETE_MEMBER,
                            state,
                            NATIVE_HISTORY,
                            nix::fcntl::RenameFlags::RENAME_NOREPLACE,
                        )
                        .map_err(|_| Unavailable)?;
                        self.history = true; // positive original rename BEFORE sampled postcheck
                        Ok(())
                    }
                    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
                    {
                        let _ = state;
                        Err(Unavailable)
                    }
                },
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .perform(
                Slot::Scratch1,
                || tick(until),
                |file| {
                    let before = self.lower.original[Slot::Scratch1 as usize]
                        .as_ref()
                        .ok_or(Unavailable)?;
                    let after = file.metadata().map_err(|_| Unavailable)?;
                    if !same_after_own_rename(before, &after, 1) {
                        return Err(Unavailable);
                    }
                    self.lower.original[Slot::Scratch1 as usize] = Some(after);
                    Ok(())
                },
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .io
            .perform(
                Slot::State,
                || tick(until),
                |file| file.sync_all().map_err(|_| Unavailable),
            )
            .map_err(|_| FirstError::StillFenced)?;
        self.lower
            .verify_member(
                Slot::State,
                Slot::Scratch1,
                NATIVE_HISTORY,
                &complete.encode(),
                until,
            )
            .map_err(|_| FirstError::StillFenced)?;
        let state = self
            .lower
            .io
            .original(Slot::State)
            .map_err(|_| FirstError::StillFenced)?;
        for name in [
            "routing-preset.pending.json",
            PENDING_DIRECTORY,
            INTENT,
            TERMINAL,
            crate::restore_retirement_candidate::RECEIPT_MEMBER,
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            TICKET_MEMBER,
            COMPLETE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        ] {
            tick(until).map_err(|_| FirstError::StillFenced)?;
            if !matches!(
                fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW),
                Err(nix::errno::Errno::ENOENT)
            ) {
                return Err(FirstError::StillFenced);
            }
            tick(until).map_err(|_| FirstError::StillFenced)?;
        }
        self.gate(origin, until)?;
        self.disposition_done = true;
        Ok(())
    }
    fn recover_native(
        &mut self,
        origin: &mut NativeRecoveryOrigin<'_>,
        backup: &omavless_domain::private_backup::OpenedBackup,
        complete: bool,
    ) -> Result<(), FirstError> {
        if self.sealed || self.uid.is_some() {
            return Err(FirstError::StillFenced);
        }
        let until = Instant::now() + std::time::Duration::from_secs(45);
        self.recovery = true; // consumed before every original acquisition
        let result = (|| {
            self.uid = Some(origin.uid());
            self.gid = Some(nix::unistd::getgid().as_raw());
            self.lower
                .io
                .native_admit()
                .map_err(|_| FirstError::Admission)?;
            for (index, slot) in [Slot::Config, Slot::State, Slot::Run]
                .into_iter()
                .enumerate()
            {
                self.clone_file(slot, origin.directory(index)?, true, until)
                    .map_err(|_| FirstError::Admission)?;
            }
            self.capture_recovery_singleton(until)?;
            for (index, slot) in [Slot::Owner, Slot::Desired, Slot::Login]
                .into_iter()
                .enumerate()
            {
                if let Some(file) = origin.member(index)? {
                    self.clone_file(slot, file, false, until)
                        .map_err(|_| FirstError::Admission)?;
                }
            }
            self.gate(origin, until)?;
            for (index, (slot, _)) in LIVE.into_iter().enumerate() {
                self.clone_file(slot, origin.live(index)?, false, until)
                    .map_err(|_| FirstError::Admission)?;
            }
            for name in [
                TERMINAL,
                crate::restore_executor_candidate::OLD_SLOT[0],
                crate::restore_executor_candidate::OLD_SLOT[1],
            ] {
                if name == TERMINAL && complete {
                    continue;
                }
                let parent = if name == TERMINAL {
                    Slot::State
                } else {
                    Slot::Config
                };
                if !matches!(
                    fstatat(
                        self.lower
                            .io
                            .original(parent)
                            .map_err(|_| FirstError::Admission)?,
                        name,
                        AtFlags::AT_SYMLINK_NOFOLLOW
                    ),
                    Err(nix::errno::Errno::ENOENT)
                ) {
                    return Err(FirstError::Admission);
                }
            }
            self.lower
                .io
                .child(
                    ChildPlan {
                        parent: Slot::State,
                        slot: Slot::StageDirectory,
                        name: PENDING_DIRECTORY,
                        flags: OFlag::O_RDONLY | OFlag::O_DIRECTORY,
                        mode: Mode::empty(),
                    },
                    || tick(until),
                    |_| Ok(()),
                )
                .map_err(|_| FirstError::Admission)?;
            self.shape(Slot::StageDirectory, true, until)
                .map_err(|_| FirstError::Admission)?;
            self.stage_name = self.lower.original[Slot::StageDirectory as usize].clone();
            let old_store = self.recover_member(
                Slot::StageDirectory,
                Slot::StageOldStore,
                MEMBERS[0],
                omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
                until,
            )?;
            let old_template = self.recover_member(
                Slot::StageDirectory,
                Slot::StageOldTemplate,
                MEMBERS[1],
                omavless_domain::config::MAX_TEMPLATE_BYTES,
                until,
            )?;
            let new_store = self.recover_member(
                Slot::StageDirectory,
                Slot::StageNewStore,
                MEMBERS[2],
                omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
                until,
            )?;
            let new_template = self.recover_member(
                Slot::StageDirectory,
                Slot::StageNewTemplate,
                MEMBERS[3],
                omavless_domain::config::MAX_TEMPLATE_BYTES,
                until,
            )?;
            let members = [
                old_store.as_slice(),
                old_template.as_slice(),
                new_store.as_slice(),
                new_template.as_slice(),
            ];
            let ready = self.recover_member(
                Slot::StageDirectory,
                Slot::StageReady,
                READY_MEMBER,
                crate::restore_staging_candidate::READY_BYTES,
                until,
            )?;
            let intent =
                self.recover_member(Slot::State, Slot::Intent, INTENT, RECORD_BYTES, until)?;
            if ready.as_slice() != ready_bytes(members)
                || backup
                    .restore_store_off()
                    .map_err(|_| FirstError::Prepare)?
                    .as_slice()
                    != members[2]
                || backup.template() != members[3]
            {
                return Err(FirstError::Admission);
            }
            let plan = planned_stage_identity(members).map_err(|_| FirstError::Admission)?;
            let current_store = self.read_original_live(0, until)?;
            let current_template = self.read_original_live(1, until)?;
            let current = [current_store.as_slice(), current_template.as_slice()];
            let existing_terminal = if complete {
                Some(self.recover_member(
                    Slot::State,
                    Slot::Terminal,
                    TERMINAL,
                    RECORD_BYTES,
                    until,
                )?)
            } else {
                None
            };
            let chain = if let Some(terminal) = &existing_terminal {
                let chain = DecisionChain::decode(&intent, Some(terminal))
                    .map_err(|_| FirstError::Admission)?;
                if current != [members[0], members[1]]
                    || chain.active().phase() != DecisionPhase::Aborted
                    || chain.active().review_inspection(
                        origin.generation(),
                        Some(origin.desired_bytes()),
                        &plan,
                        LivePairClass::Old,
                    ) != RecoveryReview::VerifyAbortedCandidate
                {
                    return Err(FirstError::Admission);
                }
                chain
            } else {
                mixed_intent_review(
                    &intent,
                    members,
                    current,
                    origin.generation(),
                    origin.desired_bytes(),
                )?
            };
            for (index, (slot, name)) in RECOVERED_NEW.into_iter().enumerate() {
                match fstatat(
                    self.lower
                        .io
                        .original(Slot::Config)
                        .map_err(|_| FirstError::Admission)?,
                    name,
                    AtFlags::AT_SYMLINK_NOFOLLOW,
                ) {
                    Err(nix::errno::Errno::ENOENT) => {}
                    Ok(_) => {
                        if self
                            .recover_member(
                                Slot::Config,
                                slot,
                                name,
                                members[index + 2].len(),
                                until,
                            )?
                            .as_slice()
                            != members[index + 2]
                        {
                            return Err(FirstError::Admission);
                        }
                    }
                    Err(_) => return Err(FirstError::Admission),
                }
            }
            self.scan_catalogue(Slot::Config, Some(0), &[], until)
                .map_err(|_| FirstError::Admission)?;
            self.scan_catalogue(Slot::State, Some(1), &[], until)
                .map_err(|_| FirstError::Admission)?;
            self.catalogues_captured = true;
            self.gate(origin, until)?;
            if let Some(terminal) = existing_terminal.as_ref() {
                return self.retire_native_aborted(origin, members, &intent, terminal, until);
            }
            // All admission/classification is complete before the first OLD copy.
            for index in 0..2 {
                self.write(
                    Slot::Config,
                    NATIVE_ROLLBACKS[index].0,
                    NATIVE_ROLLBACKS[index].1,
                    members[index],
                    origin,
                    until,
                )?;
            }
            for index in 0..2 {
                self.gate(origin, until)?;
                self.lower
                    .io
                    .perform(
                        Slot::Config,
                        || tick(until),
                        |config| {
                            renameat(config, NATIVE_ROLLBACKS[index].1, config, LIVE[index].1)
                                .map_err(|_| Unavailable)?;
                            self.lower.live[index] = LiveRole::Renamed;
                            Ok(())
                        },
                    )
                    .map_err(|_| FirstError::StillFenced)?;
                for (slot, links) in [(LIVE[index].0, 0), (NATIVE_ROLLBACKS[index].0, 1)] {
                    self.lower
                        .io
                        .perform(
                            slot,
                            || tick(until),
                            |file| {
                                let after = file.metadata().map_err(|_| Unavailable)?;
                                if !same_after_own_rename(
                                    self.lower.original[slot as usize]
                                        .as_ref()
                                        .ok_or(Unavailable)?,
                                    &after,
                                    links,
                                ) {
                                    return Err(Unavailable);
                                }
                                self.lower.original[slot as usize] = Some(after);
                                Ok(())
                            },
                        )
                        .map_err(|_| FirstError::StillFenced)?;
                }
                self.lower
                    .verify_unlinked_old(LIVE[index].0, current[index], until)
                    .map_err(|_| FirstError::StillFenced)?;
                self.lower
                    .verify_member(
                        Slot::Config,
                        NATIVE_ROLLBACKS[index].0,
                        LIVE[index].1,
                        members[index],
                        until,
                    )
                    .map_err(|_| FirstError::StillFenced)?;
                for slot in [NATIVE_ROLLBACKS[index].0, Slot::Config] {
                    self.lower
                        .io
                        .perform(
                            slot,
                            || tick(until),
                            |file| file.sync_all().map_err(|_| Unavailable),
                        )
                        .map_err(|_| FirstError::StillFenced)?;
                }
                self.gate(origin, until)?;
            }
            let terminal = chain
                .active()
                .terminal(TerminalChoice::Abort)
                .map_err(|_| FirstError::StillFenced)?
                .encode();
            self.write(
                Slot::State,
                Slot::Terminal,
                TERMINAL,
                &terminal,
                origin,
                until,
            )?;
            let final_chain = DecisionChain::decode(&intent, Some(&terminal))
                .map_err(|_| FirstError::StillFenced)?;
            if final_chain.active().phase() != DecisionPhase::Aborted
                || final_chain.active().review_inspection(
                    origin.generation(),
                    Some(origin.desired_bytes()),
                    &plan,
                    LivePairClass::Old,
                ) != RecoveryReview::VerifyAbortedCandidate
            {
                return Err(FirstError::StillFenced);
            }
            for index in 0..2 {
                self.lower
                    .verify_member(
                        Slot::Config,
                        NATIVE_ROLLBACKS[index].0,
                        LIVE[index].1,
                        members[index],
                        until,
                    )
                    .map_err(|_| FirstError::StillFenced)?;
            }
            self.lower
                .verify_member(Slot::State, Slot::Intent, INTENT, &intent, until)
                .map_err(|_| FirstError::StillFenced)?;
            self.lower
                .verify_member(Slot::State, Slot::Terminal, TERMINAL, &terminal, until)
                .map_err(|_| FirstError::StillFenced)?;
            self.gate(origin, until) // final SAME fresh origin after all readbacks
        })();
        if result.is_err() {
            self.sealed = true;
            self.lower.revoke();
        }
        result
    }

    fn read_original_live(
        &mut self,
        index: usize,
        until: Instant,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, FirstError> {
        use std::os::unix::fs::FileExt;
        let (slot, name) = LIVE[index];
        let file = self
            .lower
            .io
            .original(slot)
            .map_err(|_| FirstError::Admission)?;
        let length = usize::try_from(file.metadata().map_err(|_| FirstError::Admission)?.len())
            .map_err(|_| FirstError::Admission)?;
        let maximum = if index == 0 {
            omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES
        } else {
            omavless_domain::config::MAX_TEMPLATE_BYTES
        };
        if length > maximum {
            return Err(FirstError::Admission);
        }
        let mut bytes = zeroize::Zeroizing::new(vec![0; length + 1]);
        let mut done = 0;
        while done < bytes.len() {
            tick(until).map_err(|_| FirstError::Admission)?;
            let n = file
                .read_at(&mut bytes[done..], done as u64)
                .map_err(|_| FirstError::Admission)?;
            tick(until).map_err(|_| FirstError::Admission)?;
            if n == 0 {
                break;
            }
            done += n;
        }
        if done != length {
            return Err(FirstError::Admission);
        }
        bytes.truncate(length);
        self.lower
            .verify_member(Slot::Config, slot, name, &bytes, until)
            .map_err(|_| FirstError::Admission)?;
        self.expected[slot as usize] = Some((bytes.len(), Sha256::digest(&bytes).into()));
        Ok(bytes)
    }
    pub(crate) fn execute_native<H: LifecycleHost>(
        &mut self,
        origin: &mut NativeSessionOrigin<'_, H>,
        prepared: &PreparedRestorePair,
        mut cut: impl FnMut(NativeStep) -> Result<(), FirstError>,
    ) -> Result<(), FirstError> {
        if self.sealed || self.uid.is_some() {
            return Err(FirstError::StillFenced);
        }
        let until = Instant::now() + std::time::Duration::from_secs(45);
        let result = (|| {
            self.uid = Some(origin.uid());
            self.gid = Some(nix::unistd::getgid().as_raw());
            if nix::unistd::getuid().as_raw() != origin.uid()
                || nix::unistd::geteuid().as_raw() != origin.uid()
                || nix::unistd::getegid() != nix::unistd::getgid()
            {
                return Err(FirstError::Admission);
            }
            self.gate(origin, until)?;
            self.lower
                .io
                .native_admit()
                .map_err(|_| FirstError::Admission)?;
            for (index, slot) in [Slot::Config, Slot::State, Slot::Run]
                .into_iter()
                .enumerate()
            {
                self.clone_file(slot, origin.directory(index)?, true, until)
                    .map_err(|_| FirstError::Admission)?;
            }
            for (index, slot) in [Slot::Owner, Slot::Desired, Slot::Login]
                .into_iter()
                .enumerate()
            {
                if let Some(file) = origin.member(index)? {
                    self.clone_file(slot, file, false, until)
                        .map_err(|_| FirstError::Admission)?;
                }
            }
            for (index, (slot, name)) in LIVE.into_iter().enumerate() {
                self.clone_file(slot, origin.live(index)?, false, until)
                    .map_err(|_| FirstError::Admission)?;
                self.lower
                    .verify_member(
                        Slot::Config,
                        slot,
                        name,
                        [prepared.original_store(), prepared.original_template()][index],
                        until,
                    )
                    .map_err(|_| FirstError::Admission)?;
                let bytes = [prepared.original_store(), prepared.original_template()][index];
                self.expected[slot as usize] = Some((bytes.len(), Sha256::digest(bytes).into()));
            }
            for name in crate::restore_executor_candidate::NEW_SLOT
                .iter()
                .chain(crate::restore_executor_candidate::OLD_SLOT.iter())
            {
                if !matches!(
                    fstatat(
                        self.lower
                            .io
                            .original(Slot::Config)
                            .map_err(|_| FirstError::Admission)?,
                        *name,
                        AtFlags::AT_SYMLINK_NOFOLLOW
                    ),
                    Err(nix::errno::Errno::ENOENT)
                ) {
                    return Err(FirstError::Admission);
                }
            }
            self.scan_catalogue(Slot::Config, Some(0), &[], until)
                .map_err(|_| FirstError::Admission)?;
            self.scan_catalogue(Slot::State, Some(1), &[], until)
                .map_err(|_| FirstError::Admission)?;
            self.catalogues_captured = true;
            self.gate(origin, until)?;
            self.lower
                .io
                .perform(
                    Slot::State,
                    || tick(until),
                    |state| {
                        mkdirat(state, PENDING_DIRECTORY, Mode::S_IRWXU).map_err(|_| Unavailable)
                    },
                )
                .map_err(|_| FirstError::StillFenced)?;
            self.lower
                .io
                .child(
                    ChildPlan {
                        parent: Slot::State,
                        slot: Slot::StageDirectory,
                        name: PENDING_DIRECTORY,
                        flags: OFlag::O_RDONLY | OFlag::O_DIRECTORY,
                        mode: Mode::empty(),
                    },
                    || tick(until),
                    |_| Ok(()),
                )
                .map_err(|_| FirstError::StillFenced)?;
            self.shape(Slot::StageDirectory, true, until)
                .map_err(|_| FirstError::StillFenced)?;
            self.stage_name = self.lower.original[Slot::StageDirectory as usize].clone();
            self.lower
                .binding(
                    Slot::State,
                    Slot::StageDirectory,
                    PENDING_DIRECTORY,
                    true,
                    until,
                )
                .map_err(|_| FirstError::StillFenced)?;
            self.lower
                .io
                .perform(
                    Slot::State,
                    || tick(until),
                    |state| state.sync_all().map_err(|_| Unavailable),
                )
                .map_err(|_| FirstError::StillFenced)?;
            self.gate(origin, until)?;
            let members = [
                prepared.original_store(),
                prepared.original_template(),
                prepared.incoming_store(),
                prepared.incoming_template(),
            ];
            for index in 0..4 {
                self.write(
                    Slot::StageDirectory,
                    STAGED[index],
                    MEMBERS[index],
                    members[index],
                    origin,
                    until,
                )?;
            }
            self.write(
                Slot::StageDirectory,
                Slot::StageReady,
                READY_MEMBER,
                &ready_bytes(members),
                origin,
                until,
            )?;
            cut(NativeStep::StageReady)?;
            self.gate(origin, until)?;
            let plan = planned_stage_identity(members).map_err(|_| FirstError::StillFenced)?;
            let desired = origin.desired_bytes()?;
            let mut transaction = [0; 16];
            self.lower
                .io
                .native_entropy(|| tick(until))
                .map_err(|_| FirstError::StillFenced)?;
            let mut done = 0;
            while done < transaction.len() {
                self.lower
                    .io
                    .perform(
                        Slot::Scratch7,
                        || tick(until),
                        |mut file| {
                            let n = std::io::Read::read(&mut file, &mut transaction[done..])
                                .map_err(|_| Unavailable)?;
                            if n == 0 || n > transaction.len() - done {
                                return Err(Unavailable);
                            }
                            done += n;
                            Ok(())
                        },
                    )
                    .map_err(|_| FirstError::StillFenced)?;
            }
            let intent =
                DecisionRecord::intent(origin.generation(), Some(&desired), &plan, transaction)
                    .map_err(|_| FirstError::StillFenced)?
                    .encode();
            self.write(Slot::State, Slot::Intent, INTENT, &intent, origin, until)?;
            cut(NativeStep::Intent)?;
            self.gate(origin, until)?;
            pair_steps(|step| {
                let index = match step {
                    CommitStep::Write(i) | CommitStep::Rename(i) => i,
                };
                self.gate(origin, until).map_err(|_| Unavailable)?;
                if matches!(step, CommitStep::Write(_)) {
                    self.write(
                        Slot::Config,
                        NATIVE_REPLACEMENTS[index].0,
                        NATIVE_REPLACEMENTS[index].1,
                        members[index + 2],
                        origin,
                        until,
                    )
                    .map_err(|_| Unavailable)?;
                    self.lower.live[index] = LiveRole::ReplacementReady;
                    cut(NativeStep::Replacement(index)).map_err(|_| Unavailable)?;
                } else {
                    self.lower.io.perform(
                        Slot::Config,
                        || tick(until),
                        |config| {
                            renameat(config, NATIVE_REPLACEMENTS[index].1, config, LIVE[index].1)
                                .map_err(|_| Unavailable)?;
                            self.lower.live[index] = LiveRole::Renamed;
                            Ok(())
                        },
                    )?;
                    self.lower.advance_own_rename(index, until)?;
                    self.lower
                        .verify_unlinked_old(LIVE[index].0, members[index], until)?;
                    self.lower.verify_member(
                        Slot::Config,
                        NATIVE_REPLACEMENTS[index].0,
                        LIVE[index].1,
                        members[index + 2],
                        until,
                    )?;
                    for slot in [NATIVE_REPLACEMENTS[index].0, Slot::Config] {
                        self.lower.io.perform(
                            slot,
                            || tick(until),
                            |file| file.sync_all().map_err(|_| Unavailable),
                        )?;
                    }
                    cut(NativeStep::Renamed(index)).map_err(|_| Unavailable)?;
                }
                self.gate(origin, until).map_err(|_| Unavailable)
            })
            .map_err(|_| FirstError::StillFenced)?;
            let terminal = DecisionRecord::decode(&intent)
                .map_err(|_| FirstError::StillFenced)?
                .terminal(TerminalChoice::Commit)
                .map_err(|_| FirstError::StillFenced)?
                .encode();
            self.write(
                Slot::State,
                Slot::Terminal,
                TERMINAL,
                &terminal,
                origin,
                until,
            )?;
            cut(NativeStep::Terminal)?;
            for index in 0..2 {
                self.lower
                    .verify_member(
                        Slot::Config,
                        NATIVE_REPLACEMENTS[index].0,
                        LIVE[index].1,
                        members[index + 2],
                        until,
                    )
                    .map_err(|_| FirstError::StillFenced)?;
            }
            self.lower
                .verify_member(Slot::State, Slot::Intent, INTENT, &intent, until)
                .map_err(|_| FirstError::StillFenced)?;
            self.lower
                .verify_member(Slot::State, Slot::Terminal, TERMINAL, &terminal, until)
                .map_err(|_| FirstError::StillFenced)?;
            let chain = DecisionChain::decode(&intent, Some(&terminal))
                .map_err(|_| FirstError::StillFenced)?;
            let class = class_from_matches(
                members[2] == members[0],
                members[3] == members[1],
                true,
                true,
            );
            if chain.active().phase() != DecisionPhase::Committed
                || chain.active().review_inspection(
                    origin.generation(),
                    Some(&desired),
                    &plan,
                    class,
                ) != RecoveryReview::VerifyCommittedCandidate
            {
                return Err(FirstError::StillFenced);
            }
            self.gate(origin, until)
        })();
        if result.is_err() {
            self.sealed = true;
            self.lower.revoke();
        }
        result
    }
}
