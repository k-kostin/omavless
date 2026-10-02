// SPDX-License-Identifier: MIT

//! Inactive retirement of fixed credential-bearing replacement slots. The
//! complete staged pair, decision journal and receipt remain untouched.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::restore_cleanup_candidate::read_optional;
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_journal_candidate::inspect_decision_journal;
use crate::restore_retirement_candidate::durable_retirement_receipt;
use crate::restore_staging_candidate::{
    PENDING_DIRECTORY, VerifiedStage, read_staged_pair, same_directory, same_member,
};
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use nix::unistd::{UnlinkatFlags, unlinkat};
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use std::array;
use std::fs::{File, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

const SLOTS: [&str; 4] = [NEW_SLOT[0], NEW_SLOT[1], OLD_SLOT[0], OLD_SLOT[1]];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotError {
    Admission,
    ManualRecovery,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotOutcome {
    RetiredStillFenced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookPoint {
    Unlinked(usize),
    Synced(usize),
}

struct Observation {
    slots: [Option<Metadata>; 4],
    stage_directory: Metadata,
}

impl Observation {
    fn first_present(&self) -> Option<usize> {
        self.slots.iter().position(Option::is_some)
    }

    fn same_as(&self, other: &Self) -> bool {
        same_directory(&self.stage_directory, &other.stage_directory)
            && (0..4).all(|index| match (&self.slots[index], &other.slots[index]) {
                (Some(before), Some(after)) => same_member(before, after),
                (None, None) => true,
                _ => false,
            })
    }

    fn exactly_one_removed(&self, after: &Self, removed: usize) -> bool {
        same_directory(&self.stage_directory, &after.stage_directory)
            && (0..4).all(|index| {
                if index == removed {
                    self.slots[index].is_some() && after.slots[index].is_none()
                } else {
                    match (&self.slots[index], &after.slots[index]) {
                        (Some(before), Some(now)) => same_member(before, now),
                        (None, None) => true,
                        _ => false,
                    }
                }
            })
    }
}

fn expected(stage: &VerifiedStage, index: usize) -> &[u8] {
    match index {
        0 => stage.new_store(),
        1 => stage.new_template(),
        2 => stage.old_store(),
        _ => stage.old_template(),
    }
}

fn inspect_slots(
    config: &File,
    uid: u32,
    stage: &VerifiedStage,
    stage_directory: Metadata,
) -> Result<Observation, SlotError> {
    let mut slots: [Option<Metadata>; 4] = array::from_fn(|_| None);
    for (index, name) in SLOTS.iter().enumerate() {
        let limit = if index % 2 == 0 {
            MAX_PRIVATE_STORE_BYTES
        } else {
            MAX_TEMPLATE_BYTES
        };
        if let Some((bytes, metadata)) =
            read_optional(config, name, uid, limit).map_err(|_| SlotError::ManualRecovery)?
        {
            if bytes.as_slice() != expected(stage, index) {
                return Err(SlotError::ManualRecovery);
            }
            slots[index] = Some(metadata);
        }
    }
    Ok(Observation {
        slots,
        stage_directory,
    })
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    config_directory: File,
    config_identity: Metadata,
    state_directory: File,
    state_identity: Metadata,
    receipt_identity: Option<Metadata>,
}

impl<'a> Context<'a> {
    fn new(
        config: &'a Path,
        paths: &'a CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &'a MigrationLock,
    ) -> Result<Self, SlotError> {
        let config_directory =
            open_private_directory(config, uid).map_err(|_| SlotError::ManualRecovery)?;
        let config_identity = config_directory
            .metadata()
            .map_err(|_| SlotError::ManualRecovery)?;
        let state_directory = open_private_directory(&paths.state_directory, uid)
            .map_err(|_| SlotError::ManualRecovery)?;
        let state_identity = state_directory
            .metadata()
            .map_err(|_| SlotError::ManualRecovery)?;
        Ok(Self {
            config,
            paths,
            uid,
            generation,
            lock,
            config_directory,
            config_identity,
            state_directory,
            state_identity,
            receipt_identity: None,
        })
    }

    fn check_bindings(&mut self) -> Result<Observation, SlotError> {
        if !self.lock.authorizes(self.paths, self.uid)
            || crate::restore_disposition_ticket_model::pending_at(&self.paths.state_directory)
            || crate::restore_disposition_complete_model::pending_at(&self.paths.state_directory)
        {
            return Err(SlotError::Admission);
        }
        let (receipt, identity) = durable_retirement_receipt(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
        )
        .map_err(|_| SlotError::ManualRecovery)?;
        if self
            .receipt_identity
            .as_ref()
            .is_some_and(|before| !same_member(before, &identity))
        {
            return Err(SlotError::ManualRecovery);
        }
        self.config_directory
            .sync_all()
            .map_err(|_| SlotError::Ambiguous)?;
        let stage = read_staged_pair(&self.paths.state_directory, self.uid)
            .map_err(|_| SlotError::ManualRecovery)?;
        let journal = inspect_decision_journal(self.paths, self.uid, self.lock)
            .map_err(|_| SlotError::ManualRecovery)?;
        if journal.active().encode() != receipt.terminal().encode()
            || !receipt.terminal().matches_stage_identity(stage.identity())
        {
            return Err(SlotError::ManualRecovery);
        }
        let stage_directory = File::from(
            openat(
                &self.state_directory,
                Path::new(PENDING_DIRECTORY),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| SlotError::ManualRecovery)?,
        );
        let stage_identity = stage_directory
            .metadata()
            .map_err(|_| SlotError::ManualRecovery)?;
        if !stage_identity.is_dir()
            || stage_identity.uid() != self.uid
            || stage_identity.mode() & 0o7777 != 0o700
        {
            return Err(SlotError::ManualRecovery);
        }
        let observed = inspect_slots(
            &self.config_directory,
            self.uid,
            &stage,
            stage_identity.clone(),
        )?;
        if !same_directory(
            &stage_identity,
            &File::from(
                openat(
                    &self.state_directory,
                    Path::new(PENDING_DIRECTORY),
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| SlotError::ManualRecovery)?,
            )
            .metadata()
            .map_err(|_| SlotError::ManualRecovery)?,
        ) || !same_directory(
            &self.state_identity,
            &open_private_directory(&self.paths.state_directory, self.uid)
                .map_err(|_| SlotError::ManualRecovery)?
                .metadata()
                .map_err(|_| SlotError::ManualRecovery)?,
        ) || !same_directory(
            &self.config_identity,
            &open_private_directory(self.config, self.uid)
                .map_err(|_| SlotError::ManualRecovery)?
                .metadata()
                .map_err(|_| SlotError::ManualRecovery)?,
        ) {
            return Err(SlotError::ManualRecovery);
        }
        self.receipt_identity = Some(identity);
        Ok(observed)
    }

    fn check(&mut self, gate: &mut impl FnMut() -> bool) -> Result<Observation, SlotError> {
        if !gate() {
            return Err(SlotError::Admission);
        }
        let before = self.check_bindings()?;
        if !gate() {
            return Err(SlotError::Admission);
        }
        let after = self.check_bindings()?;
        if !before.same_as(&after) {
            return Err(SlotError::ManualRecovery);
        }
        Ok(after)
    }
}

/// Retire only provenance-matched fixed slots. All other staged and decision
/// evidence, including the terminal receipt, remains present and fenced.
#[allow(dead_code)]
pub(crate) fn retire_replacement_slots(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<SlotOutcome, SlotError> {
    retire_with_hook(config, paths, uid, generation, lock, gate, |_| true)
}

fn retire_with_hook<G: FnMut() -> bool, H: FnMut(HookPoint) -> bool>(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: G,
    mut hook: H,
) -> Result<SlotOutcome, SlotError> {
    let mut context = Context::new(config, paths, uid, generation, lock)?;
    for _ in 0..=4 {
        let before = context.check(&mut gate)?;
        let Some(index) = before.first_present() else {
            return Ok(SlotOutcome::RetiredStillFenced);
        };
        let name = SLOTS[index];
        let target = File::from(
            openat(
                &context.config_directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| SlotError::ManualRecovery)?,
        );
        let expected = before.slots[index]
            .as_ref()
            .ok_or(SlotError::ManualRecovery)?;
        if !same_member(
            expected,
            &target.metadata().map_err(|_| SlotError::ManualRecovery)?,
        ) {
            return Err(SlotError::ManualRecovery);
        }
        let reopened = File::from(
            openat(
                &context.config_directory,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| SlotError::ManualRecovery)?,
        );
        if !same_member(
            expected,
            &reopened.metadata().map_err(|_| SlotError::ManualRecovery)?,
        ) {
            return Err(SlotError::ManualRecovery);
        }
        unlinkat(
            &context.config_directory,
            Path::new(name),
            UnlinkatFlags::NoRemoveDir,
        )
        .map_err(|_| SlotError::Ambiguous)?;
        if !hook(HookPoint::Unlinked(index)) {
            return Err(SlotError::Ambiguous);
        }
        context
            .config_directory
            .sync_all()
            .map_err(|_| SlotError::Ambiguous)?;
        if !hook(HookPoint::Synced(index)) {
            return Err(SlotError::Ambiguous);
        }
        let after = context.check(&mut gate).map_err(|_| SlotError::Ambiguous)?;
        if !before.exactly_one_removed(&after, index) {
            return Err(SlotError::Ambiguous);
        }
    }
    Err(SlotError::Ambiguous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::DesiredState;
    use crate::restore_executor_candidate::{
        EffectStep, ExecutionError, PendingOutcome, execute_staged_pair, execute_with_hook,
        recover_staged_pair,
    };
    use crate::restore_retirement_candidate::{
        RECEIPT_MEMBER, inspect_retirement_receipt, publish_retirement_receipt,
    };
    use crate::restore_staging_candidate::stage_private_pair;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Command;

    const OLD_STORE: &[u8] = b"synthetic old store\n";
    const OLD_TEMPLATE: &[u8] = b"synthetic old template\n";
    const NEW_STORE: &[u8] = b"synthetic new store\n";
    const NEW_TEMPLATE: &[u8] = b"synthetic new template\n";

    struct Fixture {
        root: PathBuf,
        config: PathBuf,
        paths: CutoverPaths,
        uid: u32,
    }

    impl Fixture {
        fn new() -> Self {
            Self::new_pair(NEW_STORE, NEW_TEMPLATE)
        }

        fn new_pair(new_store: &[u8], new_template: &[u8]) -> Self {
            let home = std::env::var_os("HOME").expect("test needs home");
            let root = crate::test_temp::directory_under(Path::new(&home), "restore-slot").unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let config = root.join("config");
            let runtime = root.join("runtime");
            let state = root.join("state");
            for directory in [&config, &runtime, &state] {
                fs::create_dir(directory).unwrap();
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            let paths = CutoverPaths::below(&runtime, &state, uid);
            fs::create_dir(&paths.state_directory).unwrap();
            fs::set_permissions(&paths.state_directory, fs::Permissions::from_mode(0o700)).unwrap();
            Self::member(
                &paths.ownership_marker,
                br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
            );
            Self::member(
                &paths.state_directory.join("desired.json"),
                &serde_json::to_vec(&DesiredState::default()).unwrap(),
            );
            Self::member(&config.join("profiles.json"), OLD_STORE);
            Self::member(&config.join("route-template.yaml"), OLD_TEMPLATE);
            stage_private_pair(
                &paths.state_directory,
                uid,
                OLD_STORE,
                OLD_TEMPLATE,
                new_store,
                new_template,
            )
            .unwrap();
            Self {
                root,
                config,
                paths,
                uid,
            }
        }

        fn member(path: &Path, bytes: &[u8]) {
            fs::write(path, bytes).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        fn reopen(root: PathBuf) -> Self {
            let uid = fs::metadata(&root).unwrap().uid();
            let paths = CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid);
            Self {
                config: root.join("config"),
                root,
                paths,
                uid,
            }
        }

        fn lock(&self) -> MigrationLock {
            MigrationLock::acquire(&self.paths, self.uid).unwrap()
        }

        fn committed(&self, lock: &MigrationLock) {
            assert_eq!(
                execute_staged_pair(
                    &self.config,
                    &self.paths,
                    self.uid,
                    2,
                    lock,
                    [67; 16],
                    || true,
                ),
                Ok(PendingOutcome::Committed)
            );
            assert_eq!(
                publish_retirement_receipt(&self.config, &self.paths, self.uid, 2, lock, || true,),
                Ok(PendingOutcome::Committed)
            );
        }

        fn aborted_after_linked_new_slot(&self, lock: &MigrationLock) {
            assert_eq!(
                execute_with_hook(
                    &self.config,
                    &self.paths,
                    self.uid,
                    2,
                    lock,
                    [68; 16],
                    || true,
                    |step| step != EffectStep::Linked(0),
                ),
                Err(ExecutionError::Ambiguous)
            );
            assert!(self.config.join(NEW_SLOT[0]).exists());
            assert_eq!(
                recover_staged_pair(&self.config, &self.paths, self.uid, 2, lock, || true),
                Ok(PendingOutcome::Aborted)
            );
            assert_eq!(
                publish_retirement_receipt(&self.config, &self.paths, self.uid, 2, lock, || true,),
                Ok(PendingOutcome::Aborted)
            );
        }

        fn all_slots(&self, new_store: &[u8], new_template: &[u8]) {
            for (name, bytes) in [
                (SLOTS[0], new_store),
                (SLOTS[1], new_template),
                (SLOTS[2], OLD_STORE),
                (SLOTS[3], OLD_TEMPLATE),
            ] {
                Self::member(&self.config.join(name), bytes);
            }
        }

        fn assert_evidence(&self, lock: &MigrationLock) {
            assert!(self.paths.state_directory.join(PENDING_DIRECTORY).exists());
            assert!(
                self.paths
                    .state_directory
                    .join("restore-decision.intent")
                    .exists()
            );
            assert!(
                self.paths
                    .state_directory
                    .join("restore-decision.terminal")
                    .exists()
            );
            assert!(self.paths.state_directory.join(RECEIPT_MEMBER).exists());
            assert!(crate::pending_private_transaction::pending_at(
                &self.paths.state_directory
            ));
            assert!(
                inspect_retirement_receipt(&self.config, &self.paths, self.uid, 2, lock).is_ok()
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn no_slots_matching_subsets_and_identical_pairs_keep_evidence() {
        for subset in [0_u8, 1, 2, 4, 8, 15] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            for (index, bytes) in [NEW_STORE, NEW_TEMPLATE, OLD_STORE, OLD_TEMPLATE]
                .into_iter()
                .enumerate()
            {
                if subset & (1 << index) != 0 {
                    Fixture::member(&fixture.config.join(SLOTS[index]), bytes);
                }
            }
            for _ in 0..2 {
                assert_eq!(
                    retire_replacement_slots(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(SlotOutcome::RetiredStillFenced)
                );
                fixture.assert_evidence(&lock);
            }
            assert!(SLOTS.iter().all(|name| !fixture.config.join(name).exists()));
            assert_eq!(
                fs::read(fixture.config.join("profiles.json")).unwrap(),
                NEW_STORE
            );
        }

        let identical = Fixture::new_pair(OLD_STORE, OLD_TEMPLATE);
        let lock = identical.lock();
        identical.committed(&lock);
        identical.all_slots(OLD_STORE, OLD_TEMPLATE);
        assert_eq!(
            retire_replacement_slots(
                &identical.config,
                &identical.paths,
                identical.uid,
                2,
                &lock,
                || true,
            ),
            Ok(SlotOutcome::RetiredStillFenced)
        );
        identical.assert_evidence(&lock);
        assert_eq!(
            fs::read(identical.config.join("profiles.json")).unwrap(),
            OLD_STORE
        );
    }

    #[test]
    fn actual_forward_interruption_leaves_new_slot_that_aborted_retirement_can_verify() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.aborted_after_linked_new_slot(&lock);
        fixture.assert_evidence(&lock);
        assert_eq!(
            retire_replacement_slots(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            ),
            Ok(SlotOutcome::RetiredStillFenced)
        );
        assert!(!fixture.config.join(NEW_SLOT[0]).exists());
        assert_eq!(
            fs::read(fixture.config.join("profiles.json")).unwrap(),
            OLD_STORE
        );
        fixture.assert_evidence(&lock);
    }

    #[test]
    fn preflight_refuses_foreign_and_unsafe_slots_without_deleting_any() {
        for change in 0..6 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            fixture.all_slots(NEW_STORE, NEW_TEMPLATE);
            let path = fixture.config.join(SLOTS[1]);
            match change {
                0 => Fixture::member(&path, b"foreign bytes\n"),
                1 => Fixture::member(&path, OLD_TEMPLATE),
                2 => {
                    fs::remove_file(&path).unwrap();
                    symlink("route-template.yaml", &path).unwrap();
                }
                3 => fs::hard_link(&path, fixture.root.join("linked-slot")).unwrap(),
                4 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
                _ => {
                    fs::remove_file(&path).unwrap();
                    fs::create_dir(&path).unwrap();
                }
            }
            assert_eq!(
                retire_replacement_slots(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                ),
                Err(SlotError::ManualRecovery)
            );
            assert!(fixture.config.join(SLOTS[0]).exists());
            fixture.assert_evidence(&lock);
        }
    }

    #[test]
    fn changed_complete_stage_cannot_authorize_slots_from_other_receipt() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        fixture.all_slots(NEW_STORE, NEW_TEMPLATE);
        let stage = fixture.paths.state_directory.join(PENDING_DIRECTORY);
        fs::remove_dir_all(&stage).unwrap();
        stage_private_pair(
            &fixture.paths.state_directory,
            fixture.uid,
            OLD_STORE,
            OLD_TEMPLATE,
            b"different new store\n",
            NEW_TEMPLATE,
        )
        .unwrap();
        assert_eq!(
            retire_replacement_slots(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
            ),
            Err(SlotError::ManualRecovery)
        );
        assert!(SLOTS.iter().all(|name| fixture.config.join(name).exists()));
        assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
    }

    #[test]
    fn gate_driven_target_and_receipt_replacement_refuse_before_unlink() {
        for change in 0..3 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            fixture.all_slots(NEW_STORE, NEW_TEMPLATE);
            let first = fixture.config.join(SLOTS[0]);
            // Keep the original inode alive so a replacement cannot reuse its
            // number immediately and make this regression assertion flaky.
            let held_original = File::open(&first).unwrap();
            let old_inode = held_original.metadata().unwrap().ino();
            let mut calls = 0;
            assert_eq!(
                retire_replacement_slots(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if calls == 2 {
                            match change {
                                0 | 1 => {
                                    fs::remove_file(&first).unwrap();
                                    Fixture::member(
                                        &first,
                                        if change == 0 {
                                            NEW_STORE
                                        } else {
                                            b"foreign slot\n"
                                        },
                                    );
                                }
                                _ => {
                                    fs::remove_file(
                                        fixture.paths.state_directory.join(RECEIPT_MEMBER),
                                    )
                                    .unwrap();
                                }
                            }
                        }
                        true
                    },
                ),
                Err(SlotError::ManualRecovery)
            );
            assert!(first.exists());
            if change != 2 {
                assert_eq!(held_original.metadata().unwrap().ino(), old_inode);
                assert_ne!(fs::metadata(&first).unwrap().ino(), old_inode);
            }
            assert!(fixture.config.join(SLOTS[1]).exists());
        }
    }

    #[test]
    fn every_post_effect_interruption_resumes_without_touching_stage() {
        for index in 0..4 {
            for after_sync in [false, true] {
                let fixture = Fixture::new();
                let lock = fixture.lock();
                fixture.committed(&lock);
                fixture.all_slots(NEW_STORE, NEW_TEMPLATE);
                assert_eq!(
                    retire_with_hook(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                        |point| {
                            !matches!(point, HookPoint::Unlinked(i) if !after_sync && i == index)
                                && !matches!(point, HookPoint::Synced(i) if after_sync && i == index)
                        },
                    ),
                    Err(SlotError::Ambiguous)
                );
                fixture.assert_evidence(&lock);
                drop(lock);
                let lock = fixture.lock();
                assert_eq!(
                    retire_replacement_slots(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(SlotOutcome::RetiredStillFenced)
                );
                fixture.assert_evidence(&lock);
            }
        }
    }

    #[test]
    fn subprocess_crash_after_every_unlink_and_sync_resumes() {
        for index in 0..4 {
            for phase in ["unlinked", "synced"] {
                let fixture = Fixture::new();
                let lock = fixture.lock();
                fixture.committed(&lock);
                fixture.all_slots(NEW_STORE, NEW_TEMPLATE);
                drop(lock);
                let child = Command::new(std::env::current_exe().unwrap())
                    .arg("--exact")
                    .arg("restore_slot_retirement_candidate::tests::crash_worker")
                    .env("OMAVLESS_SLOT_TEST_ROOT", &fixture.root)
                    .env("OMAVLESS_SLOT_TEST_INDEX", index.to_string())
                    .env("OMAVLESS_SLOT_TEST_PHASE", phase)
                    .status()
                    .unwrap();
                assert_eq!(child.signal(), Some(9), "{index} {phase}");
                let lock = fixture.lock();
                fixture.assert_evidence(&lock);
                assert_eq!(
                    retire_replacement_slots(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(SlotOutcome::RetiredStillFenced)
                );
                fixture.assert_evidence(&lock);
            }
        }
    }

    #[test]
    fn crash_worker() {
        let Some(root) = std::env::var_os("OMAVLESS_SLOT_TEST_ROOT") else {
            return;
        };
        let fixture = Fixture::reopen(PathBuf::from(root));
        let lock = fixture.lock();
        let index: usize = std::env::var("OMAVLESS_SLOT_TEST_INDEX")
            .unwrap()
            .parse()
            .unwrap();
        let phase = std::env::var("OMAVLESS_SLOT_TEST_PHASE").unwrap();
        let _ = retire_with_hook(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
            |point| {
                let selected = match point {
                    HookPoint::Unlinked(i) => phase == "unlinked" && i == index,
                    HookPoint::Synced(i) => phase == "synced" && i == index,
                };
                if selected {
                    nix::sys::signal::kill(
                        nix::unistd::Pid::this(),
                        nix::sys::signal::Signal::SIGKILL,
                    )
                    .unwrap();
                }
                true
            },
        );
        panic!("expected abrupt slot worker termination");
    }
}
