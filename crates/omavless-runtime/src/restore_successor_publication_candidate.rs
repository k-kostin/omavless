// SPDX-License-Identifier: MIT

//! Inactive publication only. Never stages, changes live files, retires either
//! fence, or admits a normal owner. A supplied gate must freshly establish an
//! idle mutation owner and absence of owned core/TUN under the same lease.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::restore_cleanup_candidate::{inspect_completion_record, read_optional};
use crate::restore_closure_model::{CLOSURE_MEMBER, ClosureRecord, RECORD_BYTES as CLOSURE_BYTES};
use crate::restore_decision_candidate::DecisionRecord;
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_journal_candidate::read_desired_for_decision;
use crate::restore_retirement_candidate::RECEIPT_MEMBER;
use crate::restore_staging_candidate::{
    PENDING_DIRECTORY, planned_stage_identity, same_directory, same_member,
};
use crate::restore_successor_handoff_model::{RECORD_BYTES, SUCCESSOR_MEMBER, SuccessorHandoff};
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use omavless_domain::{
    config::MAX_TEMPLATE_BYTES, private_backup::OpenedBackup,
    private_store::MAX_PRIVATE_STORE_BYTES,
};
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationError {
    Admission,
    Ambiguous,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationResult {
    PublishedStillFenced,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    Created,
    Written,
    FileSynced,
    DirectorySynced,
    Reopened,
}
type Result<T> = std::result::Result<T, PublicationError>;
const REFUSE: PublicationError = PublicationError::Admission;

fn absent(directory: &File, name: &str) -> Result<()> {
    match openat(
        directory,
        Path::new(name),
        OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Err(Errno::ENOENT) => Ok(()),
        _ => Err(REFUSE),
    }
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    state: File,
    state_identity: Metadata,
    config_dir: File,
    config_identity: Metadata,
    predecessor: ClosureRecord,
    predecessor_identity: Metadata,
    desired: Option<Zeroizing<Vec<u8>>>,
    old: [(Zeroizing<Vec<u8>>, Metadata); 2],
}

impl Context<'_> {
    fn check(&self, successor_absent: bool) -> Result<()> {
        let current = inspect_completion_record(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
        )
        .map_err(|_| REFUSE)?;
        let (raw, identity) = read_optional(&self.state, CLOSURE_MEMBER, self.uid, CLOSURE_BYTES)
            .map_err(|_| REFUSE)?
            .ok_or(REFUSE)?;
        if current.encode() != self.predecessor.encode()
            || raw.as_slice() != self.predecessor.encode()
            || !same_member(&identity, &self.predecessor_identity)
            || read_desired_for_decision(self.paths, self.uid, self.lock).map_err(|_| REFUSE)?
                != self.desired
        {
            return Err(REFUSE);
        }
        // The older inspector intentionally allows a finalization receipt.
        // This publisher requires completion-only, not that dual-fence state.
        for name in [
            RECEIPT_MEMBER,
            PENDING_DIRECTORY,
            "restore-decision.intent",
            "restore-decision.terminal",
            "routing-preset.pending.json",
        ] {
            absent(&self.state, name)?;
        }
        if successor_absent {
            absent(&self.state, SUCCESSOR_MEMBER)?;
        }
        for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
            absent(&self.config_dir, name)?;
        }
        for (index, (name, limit)) in [
            ("profiles.json", MAX_PRIVATE_STORE_BYTES),
            ("route-template.yaml", MAX_TEMPLATE_BYTES),
        ]
        .into_iter()
        .enumerate()
        {
            let (raw, identity) = read_optional(&self.config_dir, name, self.uid, limit)
                .map_err(|_| REFUSE)?
                .ok_or(REFUSE)?;
            if raw != self.old[index].0 || !same_member(&identity, &self.old[index].1) {
                return Err(REFUSE);
            }
        }
        for (path, identity) in [
            (&self.paths.state_directory as &Path, &self.state_identity),
            (self.config, &self.config_identity),
        ] {
            if !same_directory(
                identity,
                &open_private_directory(path, self.uid)
                    .map_err(|_| REFUSE)?
                    .metadata()
                    .map_err(|_| REFUSE)?,
            ) {
                return Err(REFUSE);
            }
        }
        Ok(())
    }
}

/// Requires authenticated new bytes by construction. No raw-new-pair overload.
/// Existing, partial or ambiguous successor records are never retried/repaired.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn publish_successor_handoff(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    transaction_id: [u8; 16],
    gate: impl FnMut() -> bool,
) -> Result<PublicationResult> {
    publish_with_hook(
        config,
        paths,
        uid,
        generation,
        lock,
        backup,
        transaction_id,
        gate,
        |_| true,
    )
}

#[allow(clippy::too_many_arguments)]
fn publish_with_hook(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    transaction_id: [u8; 16],
    mut gate: impl FnMut() -> bool,
    mut hook: impl FnMut(Checkpoint) -> bool,
) -> Result<PublicationResult> {
    if !lock.authorizes(paths, uid) || !gate() {
        return Err(REFUSE);
    }
    let new_store = backup.restore_store_off().map_err(|_| REFUSE)?;
    let predecessor =
        inspect_completion_record(config, paths, uid, generation, lock).map_err(|_| REFUSE)?;
    let state = open_private_directory(&paths.state_directory, uid).map_err(|_| REFUSE)?;
    let config_dir = open_private_directory(config, uid).map_err(|_| REFUSE)?;
    let (_, predecessor_identity) = read_optional(&state, CLOSURE_MEMBER, uid, CLOSURE_BYTES)
        .map_err(|_| REFUSE)?
        .ok_or(REFUSE)?;
    let context = Context {
        config,
        paths,
        uid,
        generation,
        lock,
        state_identity: state.metadata().map_err(|_| REFUSE)?,
        config_identity: config_dir.metadata().map_err(|_| REFUSE)?,
        predecessor,
        predecessor_identity,
        desired: read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?,
        old: [
            read_optional(&config_dir, "profiles.json", uid, MAX_PRIVATE_STORE_BYTES)
                .map_err(|_| REFUSE)?
                .ok_or(REFUSE)?,
            read_optional(&config_dir, "route-template.yaml", uid, MAX_TEMPLATE_BYTES)
                .map_err(|_| REFUSE)?
                .ok_or(REFUSE)?,
        ],
        state,
        config_dir,
    };
    context.check(true)?;
    let members = [
        context.old[0].0.as_slice(),
        context.old[1].0.as_slice(),
        new_store.as_slice(),
        backup.template(),
    ];
    let desired = context.desired.as_ref().map(|value| value.as_slice());
    let stage = planned_stage_identity(members).map_err(|_| REFUSE)?;
    let intent =
        DecisionRecord::intent(generation, desired, &stage, transaction_id).map_err(|_| REFUSE)?;
    let handoff = SuccessorHandoff::from_verified_plan(
        &context.predecessor,
        &intent,
        generation,
        desired,
        members,
    )
    .map_err(|_| REFUSE)?;
    // Ensure the retained predecessor name is durable before adding another
    // fence. No old fence is ever unlinked, including on any failure below.
    let prior = File::from(
        openat(
            &context.state,
            Path::new(CLOSURE_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| REFUSE)?,
    );
    if !same_member(
        &context.predecessor_identity,
        &prior.metadata().map_err(|_| REFUSE)?,
    ) {
        return Err(REFUSE);
    }
    prior.sync_all().map_err(|_| REFUSE)?;
    context.state.sync_all().map_err(|_| REFUSE)?;
    if !gate() {
        return Err(REFUSE);
    }
    context.check(true)?;
    // From creation onward every failure is ambiguous, with names retained.
    let mut publish = || -> Result<PublicationResult> {
        let mut file = File::from(
            openat(
                &context.state,
                Path::new(SUCCESSOR_MEMBER),
                OFlag::O_WRONLY
                    | OFlag::O_CREAT
                    | OFlag::O_EXCL
                    | OFlag::O_NOFOLLOW
                    | OFlag::O_CLOEXEC,
                Mode::S_IRUSR | Mode::S_IWUSR,
            )
            .map_err(|_| REFUSE)?,
        );
        let created = file.metadata().map_err(|_| REFUSE)?;
        if !created.is_file()
            || created.uid() != uid
            || created.mode() & 0o7777 != 0o600
            || created.nlink() != 1
            || !hook(Checkpoint::Created)
        {
            return Err(REFUSE);
        }
        file.write_all(&handoff.encode()).map_err(|_| REFUSE)?;
        if !hook(Checkpoint::Written) {
            return Err(REFUSE);
        }
        file.sync_all().map_err(|_| REFUSE)?;
        if !hook(Checkpoint::FileSynced) {
            return Err(REFUSE);
        }
        context.state.sync_all().map_err(|_| REFUSE)?;
        if !hook(Checkpoint::DirectorySynced) {
            return Err(REFUSE);
        }
        let durable = file.metadata().map_err(|_| REFUSE)?;
        let verify = || -> Result<()> {
            let (raw, identity) =
                read_optional(&context.state, SUCCESSOR_MEMBER, uid, RECORD_BYTES)
                    .map_err(|_| REFUSE)?
                    .ok_or(REFUSE)?;
            let reopened = SuccessorHandoff::decode(&raw).map_err(|_| REFUSE)?;
            if created.dev() != durable.dev()
                || created.ino() != durable.ino()
                || !same_member(&durable, &identity)
                || !same_member(&durable, &file.metadata().map_err(|_| REFUSE)?)
                || !reopened.matches_verified_plan(
                    &context.predecessor,
                    &intent,
                    generation,
                    desired,
                    members,
                )
            {
                return Err(REFUSE);
            }
            Ok(())
        };
        verify()?;
        if !hook(Checkpoint::Reopened) || !gate() {
            return Err(REFUSE);
        }
        context.check(false)?;
        verify()?;
        Ok(PublicationResult::PublishedStillFenced)
    };
    publish().map_err(|_| PublicationError::Ambiguous)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::restore_decision_candidate::TerminalChoice;
    use crate::restore_retirement_candidate::RetirementReceipt;
    use omavless_domain::private_backup::{open, seal};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::OnceLock;

    const STORE: &[u8] = br#"{"version":3,"profiles":[{"id":"10000000-0000-4000-8000-000000000001","name":"Synthetic","uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic","protocol":"vless","favorite":false}],"subscriptions":[],"activeId":"10000000-0000-4000-8000-000000000001","lastId":"10000000-0000-4000-8000-000000000001","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":true,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
    const TEMPLATE: &[u8] = include_bytes!("../../../templates/default.yaml");
    pub(crate) const OLD: [&[u8]; 2] = [b"synthetic old store", b"synthetic old template"];
    const PASS: &[u8] = b"synthetic successor passphrase";

    pub(crate) fn backup() -> &'static OpenedBackup {
        static BACKUP: OnceLock<OpenedBackup> = OnceLock::new();
        BACKUP.get_or_init(|| open(&seal(STORE, TEMPLATE, PASS).unwrap(), PASS).unwrap())
    }
    fn member(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    pub(crate) struct Fixture {
        pub(crate) root: PathBuf,
        pub(crate) config: PathBuf,
        pub(crate) paths: CutoverPaths,
        pub(crate) uid: u32,
    }
    impl Fixture {
        pub(crate) fn new() -> Self {
            let home = std::env::var_os("HOME").unwrap();
            let root =
                crate::test_temp::directory_under(Path::new(&home), "restore-successor").unwrap();
            let fixture = Self::reopen(root);
            for path in [
                &fixture.root,
                &fixture.config,
                &fixture.root.join("runtime"),
                &fixture.root.join("state"),
                &fixture.paths.state_directory,
            ] {
                fs::create_dir_all(path).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            member(
                &fixture.paths.ownership_marker,
                br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
            );
            member(&fixture.config.join("profiles.json"), OLD[0]);
            member(&fixture.config.join("route-template.yaml"), OLD[1]);
            let prior_stage =
                planned_stage_identity([b"before store", b"before template", OLD[0], OLD[1]])
                    .unwrap();
            let terminal = DecisionRecord::intent(2, None, &prior_stage, [1; 16])
                .unwrap()
                .terminal(TerminalChoice::Commit)
                .unwrap();
            let prior = ClosureRecord::from_verified_receipt(&RetirementReceipt::synthetic(
                &terminal, OLD[0], OLD[1],
            ))
            .unwrap();
            member(
                &fixture.paths.state_directory.join(CLOSURE_MEMBER),
                &prior.encode(),
            );
            fixture
        }
        pub(crate) fn reopen(root: PathBuf) -> Self {
            let uid = fs::metadata(&root).unwrap().uid();
            Self {
                config: root.join("config"),
                paths: CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid),
                root,
                uid,
            }
        }
        pub(crate) fn lock(&self) -> MigrationLock {
            MigrationLock::acquire(&self.paths, self.uid).unwrap()
        }
        fn publish(
            &self,
            lock: &MigrationLock,
            hook: impl FnMut(Checkpoint) -> bool,
        ) -> Result<PublicationResult> {
            publish_with_hook(
                &self.config,
                &self.paths,
                self.uid,
                2,
                lock,
                backup(),
                [2; 16],
                || true,
                hook,
            )
        }
        fn assert_preserved(&self, original: &[u8]) {
            assert_eq!(
                fs::read(self.paths.state_directory.join(CLOSURE_MEMBER)).unwrap(),
                original
            );
            assert_eq!(fs::read(self.config.join("profiles.json")).unwrap(), OLD[0]);
            assert_eq!(
                fs::read(self.config.join("route-template.yaml")).unwrap(),
                OLD[1]
            );
            assert!(crate::pending_private_transaction::pending_at(
                &self.paths.state_directory
            ));
            assert!(!self.paths.state_directory.join(PENDING_DIRECTORY).exists());
            assert!(
                !self
                    .paths
                    .state_directory
                    .join("restore-decision.intent")
                    .exists()
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn authenticated_successor_publication_keeps_predecessor_and_binds_off_copy() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        let prior = fs::read(fixture.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
        assert_eq!(
            fixture.publish(&lock, |_| true),
            Ok(PublicationResult::PublishedStillFenced)
        );
        fixture.assert_preserved(&prior);
        let raw = fs::read(fixture.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap();
        let record = SuccessorHandoff::decode(&raw).unwrap();
        let off = backup().restore_store_off().unwrap();
        let stage = planned_stage_identity([OLD[0], OLD[1], &off, TEMPLATE]).unwrap();
        assert!(record.successor_intent().matches_stage_identity(&stage));
        assert!(
            serde_json::from_slice::<serde_json::Value>(&off).unwrap()["startup"]["enabled"]
                == false
        );
        assert_eq!(backup().store(), STORE);
        assert_eq!(record.predecessor().encode().as_slice(), prior);
        assert_eq!(
            fixture.publish(&lock, |_| true),
            Err(PublicationError::Admission)
        );
        assert_eq!(
            fs::read(fixture.paths.state_directory.join(SUCCESSOR_MEMBER)).unwrap(),
            raw
        );
    }

    #[test]
    fn completion_only_admission_refuses_all_surviving_fixed_artifacts() {
        for name in [
            RECEIPT_MEMBER,
            PENDING_DIRECTORY,
            "restore-decision.intent",
            "restore-decision.terminal",
            "routing-preset.pending.json",
            SUCCESSOR_MEMBER,
        ] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            member(
                &fixture.paths.state_directory.join(name),
                b"surviving synthetic fence",
            );
            assert_eq!(
                fixture.publish(&lock, |_| true),
                Err(PublicationError::Admission)
            );
            if name != SUCCESSOR_MEMBER {
                assert!(
                    !fixture
                        .paths
                        .state_directory
                        .join(SUCCESSOR_MEMBER)
                        .exists()
                );
            }
        }
        for name in NEW_SLOT.into_iter().chain(OLD_SLOT) {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            member(&fixture.config.join(name), b"synthetic slot");
            assert_eq!(
                fixture.publish(&lock, |_| true),
                Err(PublicationError::Admission)
            );
        }
    }

    #[test]
    fn unsafe_predecessor_and_live_members_refuse_without_publication() {
        for kind in ["symlink", "hardlink", "mode", "torn", "fifo", "live"] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            let path = fixture.paths.state_directory.join(CLOSURE_MEMBER);
            match kind {
                "symlink" => {
                    fs::remove_file(&path).unwrap();
                    symlink("missing", &path).unwrap();
                }
                "hardlink" => {
                    fs::hard_link(&path, fixture.root.join("alias")).unwrap();
                }
                "mode" => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
                "torn" => member(&path, b"torn"),
                "fifo" => {
                    fs::remove_file(&path).unwrap();
                    nix::unistd::mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
                }
                "live" => member(
                    &fixture.config.join("profiles.json"),
                    b"different synthetic live",
                ),
                _ => unreachable!(),
            }
            assert_eq!(
                fixture.publish(&lock, |_| true),
                Err(PublicationError::Admission)
            );
            assert!(
                !fixture
                    .paths
                    .state_directory
                    .join(SUCCESSOR_MEMBER)
                    .exists()
            );
        }
    }

    #[test]
    fn late_gate_and_record_drift_never_report_success() {
        for drift in [
            "owner",
            "desired",
            "live",
            "predecessor",
            "successor",
            "state",
            "config",
            "gate",
        ] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            let mut gates = 0;
            let result = publish_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                backup(),
                [2; 16],
                || {
                    gates += 1;
                    if gates == 3 {
                        match drift {
                            "owner" => member(
                                &fixture.paths.ownership_marker,
                                br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                            ),
                            "desired" => member(
                                &fixture.paths.state_directory.join("desired.json"),
                                &serde_json::to_vec(&crate::desired::DesiredState::default())
                                    .unwrap(),
                            ),
                            "live" => member(
                                &fixture.config.join("profiles.json"),
                                b"changed synthetic live",
                            ),
                            "predecessor" => {
                                member(&fixture.paths.state_directory.join(CLOSURE_MEMBER), b"torn")
                            }
                            "successor" => member(
                                &fixture.paths.state_directory.join(SUCCESSOR_MEMBER),
                                b"torn",
                            ),
                            "state" | "config" => {
                                let path = if drift == "state" {
                                    &fixture.paths.state_directory
                                } else {
                                    &fixture.config
                                };
                                fs::rename(path, fixture.root.join("moved")).unwrap();
                                fs::create_dir(path).unwrap();
                                fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                                    .unwrap();
                            }
                            "gate" => return false,
                            _ => unreachable!(),
                        }
                    }
                    true
                },
                |_| true,
            );
            assert_eq!(result, Err(PublicationError::Ambiguous));
        }
    }

    #[test]
    fn invalid_transaction_owner_and_gate_refuse_before_create() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        for id in [[0; 16], [1; 16]] {
            assert_eq!(
                publish_successor_handoff(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    backup(),
                    id,
                    || true
                ),
                Err(PublicationError::Admission)
            );
        }
        assert_eq!(
            publish_successor_handoff(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                3,
                &lock,
                backup(),
                [2; 16],
                || true
            ),
            Err(PublicationError::Admission)
        );
        assert_eq!(
            publish_successor_handoff(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                backup(),
                [2; 16],
                || false
            ),
            Err(PublicationError::Admission)
        );
        assert!(
            !fixture
                .paths
                .state_directory
                .join(SUCCESSOR_MEMBER)
                .exists()
        );
    }

    #[test]
    fn unsafe_existing_successor_is_never_repaired_or_overwritten() {
        for kind in ["symlink", "hardlink", "fifo", "directory", "mode"] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            let prior = fs::read(fixture.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let path = fixture.paths.state_directory.join(SUCCESSOR_MEMBER);
            match kind {
                "symlink" => symlink("missing-synthetic", &path).unwrap(),
                "hardlink" => {
                    member(&fixture.root.join("synthetic-alias"), b"keep");
                    fs::hard_link(fixture.root.join("synthetic-alias"), &path).unwrap();
                }
                "fifo" => nix::unistd::mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR).unwrap(),
                "directory" => fs::create_dir(&path).unwrap(),
                "mode" => {
                    member(&path, b"keep");
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                }
                _ => unreachable!(),
            }
            let before = fs::symlink_metadata(&path).unwrap();
            assert_eq!(
                fixture.publish(&lock, |_| true),
                Err(PublicationError::Admission)
            );
            assert!(same_member(&before, &fs::symlink_metadata(&path).unwrap()));
            fixture.assert_preserved(&prior);
        }
    }

    #[test]
    fn partial_write_and_same_byte_replacement_remain_ambiguous() {
        for partial in [true, false] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            let prior = fs::read(fixture.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let path = fixture.paths.state_directory.join(SUCCESSOR_MEMBER);
            let result = fixture.publish(&lock, |checkpoint| {
                if partial && checkpoint == Checkpoint::Created {
                    member(&path, &[0x41; RECORD_BYTES / 2]);
                    return false;
                }
                if !partial && checkpoint == Checkpoint::Reopened {
                    let raw = fs::read(&path).unwrap();
                    fs::rename(&path, fixture.root.join("replaced-synthetic")).unwrap();
                    member(&path, &raw);
                }
                true
            });
            assert_eq!(result, Err(PublicationError::Ambiguous));
            fixture.assert_preserved(&prior);
            assert_eq!(
                fixture.publish(&lock, |_| true),
                Err(PublicationError::Admission)
            );
        }
    }

    #[test]
    fn precreation_gate_drift_and_wrong_uid_cannot_publish() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        assert_eq!(
            publish_successor_handoff(
                &fixture.config,
                &fixture.paths,
                fixture.uid + 1,
                2,
                &lock,
                backup(),
                [2; 16],
                || true
            ),
            Err(PublicationError::Admission)
        );
        let mut calls = 0;
        assert_eq!(
            publish_successor_handoff(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                backup(),
                [2; 16],
                || {
                    calls += 1;
                    if calls == 2 {
                        member(
                            &fixture.paths.state_directory.join(RECEIPT_MEMBER),
                            b"late synthetic receipt",
                        );
                    }
                    true
                }
            ),
            Err(PublicationError::Admission)
        );
        assert!(
            !fixture
                .paths
                .state_directory
                .join(SUCCESSOR_MEMBER)
                .exists()
        );
    }

    #[test]
    fn successor_publication_crashes_keep_the_predecessor_fence() {
        for checkpoint in 0..5 {
            let fixture = Fixture::new();
            // Create the operation lock before spawning; child takes its own lease.
            drop(fixture.lock());
            let original = fs::read(fixture.paths.state_directory.join(CLOSURE_MEMBER)).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "restore_successor_publication_candidate::tests::successor_crash_worker",
                ])
                .env("OMAVLESS_SYNTHETIC_SUCCESSOR_ROOT", &fixture.root)
                .env(
                    "OMAVLESS_SYNTHETIC_SUCCESSOR_CHECKPOINT",
                    checkpoint.to_string(),
                )
                .output()
                .unwrap();
            assert_eq!(output.status.signal(), Some(9));
            fixture.assert_preserved(&original);
            assert!(
                fixture
                    .paths
                    .state_directory
                    .join(SUCCESSOR_MEMBER)
                    .exists()
            );
            let lock = fixture.lock();
            assert!(
                inspect_completion_record(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                    .is_ok()
            );
            assert_eq!(
                fixture.publish(&lock, |_| true),
                Err(PublicationError::Admission)
            );
        }
    }

    #[test]
    #[ignore = "internal synthetic crash worker"]
    fn successor_crash_worker() {
        let root = PathBuf::from(std::env::var_os("OMAVLESS_SYNTHETIC_SUCCESSOR_ROOT").unwrap());
        let selected: usize = std::env::var("OMAVLESS_SYNTHETIC_SUCCESSOR_CHECKPOINT")
            .unwrap()
            .parse()
            .unwrap();
        let fixture = std::mem::ManuallyDrop::new(Fixture::reopen(root));
        let lock = fixture.lock();
        let mut index = 0;
        let _ = fixture.publish(&lock, |_| {
            if index == selected {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
            }
            index += 1;
            true
        });
        panic!("expected synthetic worker termination");
    }
}
