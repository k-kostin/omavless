// SPDX-License-Identifier: MIT

//! Inactive fixed-artifact retirement. A finalization receipt always remains
//! as a startup/mutation fence; there is no installed product caller.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::restore_decision_candidate::{DecisionRecord, RECORD_BYTES};
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_retirement_candidate::{RetirementReceipt, durable_retirement_receipt};
use crate::restore_staging_candidate::{
    MEMBERS, PENDING_DIRECTORY, READY_BYTES, READY_MAGIC, READY_MEMBER, same_directory, same_member,
};
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use nix::unistd::{UnlinkatFlags, unlinkat};
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use zeroize::Zeroizing;

const TERMINAL: &str = "restore-decision.terminal";
const INTENT: &str = "restore-decision.intent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupError {
    Admission,
    ManualRecovery,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupResult {
    RetiredStillFenced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    StageMember(usize),
    Ready,
    StageDirectory,
    Terminal,
    Intent,
    Done,
}

impl Step {
    fn index(self) -> usize {
        match self {
            Self::StageMember(index) => index,
            Self::Ready => 4,
            Self::StageDirectory => 5,
            Self::Terminal => 6,
            Self::Intent => 7,
            Self::Done => 8,
        }
    }

    fn name(self) -> Option<&'static str> {
        match self {
            Self::StageMember(index) => Some(MEMBERS[index]),
            Self::Ready => Some(READY_MEMBER),
            Self::StageDirectory => Some(PENDING_DIRECTORY),
            Self::Terminal => Some(TERMINAL),
            Self::Intent => Some(INTENT),
            Self::Done => None,
        }
    }

    fn in_stage(self) -> bool {
        matches!(self, Self::StageMember(_) | Self::Ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookPoint {
    Unlinked(Step),
    Synced(Step),
}

struct Observed {
    step: Step,
    target: Option<Metadata>,
    stage_parent: Option<Metadata>,
}

pub(crate) type PrivateMember = (Zeroizing<Vec<u8>>, Metadata);

fn exact_directory(metadata: &Metadata, uid: u32) -> bool {
    metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o700
}

fn replacement_slot_pending(config: &Path) -> bool {
    NEW_SLOT.into_iter().chain(OLD_SLOT).any(|name| {
        !matches!(
            std::fs::symlink_metadata(config.join(name)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        )
    })
}

pub(crate) fn read_optional(
    directory: &File,
    name: &str,
    uid: u32,
    limit: usize,
) -> Result<Option<PrivateMember>, CleanupError> {
    let mut file = match openat(
        directory,
        Path::new(name),
        OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::ENOENT) => return Ok(None),
        Err(_) => return Err(CleanupError::ManualRecovery),
    };
    let before = file.metadata().map_err(|_| CleanupError::ManualRecovery)?;
    if !before.is_file()
        || before.uid() != uid
        || before.mode() & 0o7777 != 0o600
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > limit as u64
    {
        return Err(CleanupError::ManualRecovery);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CleanupError::ManualRecovery)?;
    let reopened = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| CleanupError::ManualRecovery)?,
    );
    if bytes.len() as u64 != before.len()
        || !same_member(
            &before,
            &file.metadata().map_err(|_| CleanupError::ManualRecovery)?,
        )
        || !same_member(
            &before,
            &reopened
                .metadata()
                .map_err(|_| CleanupError::ManualRecovery)?,
        )
    {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(Some((bytes, before)))
}

fn names(directory: &File) -> Result<Vec<String>, CleanupError> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| CleanupError::ManualRecovery)?
    {
        let entry = item.map_err(|_| CleanupError::ManualRecovery)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| CleanupError::ManualRecovery)?;
        if !MEMBERS.contains(&name.as_str()) && name != READY_MEMBER {
            return Err(CleanupError::ManualRecovery);
        }
        entries.push(name);
        if entries.len() > 5 {
            return Err(CleanupError::ManualRecovery);
        }
    }
    entries.sort();
    Ok(entries)
}

fn open_stage(state: &File, uid: u32) -> Result<Option<(File, Metadata)>, CleanupError> {
    let stage = match openat(
        state,
        Path::new(PENDING_DIRECTORY),
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::ENOENT) => return Ok(None),
        Err(_) => return Err(CleanupError::ManualRecovery),
    };
    let metadata = stage.metadata().map_err(|_| CleanupError::ManualRecovery)?;
    if !exact_directory(&metadata, uid) {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(Some((stage, metadata)))
}

fn stage_step(
    state: &File,
    uid: u32,
    receipt: &RetirementReceipt,
) -> Result<Option<Observed>, CleanupError> {
    let Some((stage, identity)) = open_stage(state, uid)? else {
        return Ok(None);
    };
    stage.sync_all().map_err(|_| CleanupError::Ambiguous)?;
    let before = names(&stage)?;
    let ready = read_optional(&stage, READY_MEMBER, uid, READY_BYTES)?;
    let observed = if let Some((ready, ready_identity)) = ready {
        if ready.len() != READY_BYTES
            || &ready[..8] != READY_MAGIC
            || !receipt.terminal().matches_stage_ready(&ready)
        {
            return Err(CleanupError::ManualRecovery);
        }
        let mut first = None;
        let mut target = None;
        for (index, member) in MEMBERS.iter().enumerate() {
            let limit = if index % 2 == 0 {
                MAX_PRIVATE_STORE_BYTES
            } else {
                MAX_TEMPLATE_BYTES
            };
            match read_optional(&stage, member, uid, limit)? {
                Some((bytes, metadata)) => {
                    let length = u32::from_be_bytes(
                        ready[8 + index * 4..12 + index * 4]
                            .try_into()
                            .map_err(|_| CleanupError::ManualRecovery)?,
                    );
                    let digest = &ready[24 + index * 32..56 + index * 32];
                    if length as usize != bytes.len() || *digest != Sha256::digest(&bytes)[..] {
                        return Err(CleanupError::ManualRecovery);
                    }
                    if first.is_none() {
                        first = Some(index);
                        target = Some(metadata);
                    }
                }
                None if first.is_some() => return Err(CleanupError::ManualRecovery),
                None => {}
            }
        }
        match first {
            Some(index) => Observed {
                step: Step::StageMember(index),
                target,
                stage_parent: Some(identity.clone()),
            },
            None => Observed {
                step: Step::Ready,
                target: Some(ready_identity),
                stage_parent: Some(identity.clone()),
            },
        }
    } else if before.is_empty() {
        Observed {
            step: Step::StageDirectory,
            target: Some(identity.clone()),
            stage_parent: Some(identity.clone()),
        }
    } else {
        return Err(CleanupError::ManualRecovery);
    };
    if names(&stage)? != before
        || !same_directory(
            &identity,
            &stage.metadata().map_err(|_| CleanupError::ManualRecovery)?,
        )
        || !same_directory(
            &identity,
            &open_stage(state, uid)?
                .ok_or(CleanupError::ManualRecovery)?
                .1,
        )
    {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(Some(observed))
}

fn inspect_inventory(
    paths: &CutoverPaths,
    state: &File,
    uid: u32,
    receipt: &RetirementReceipt,
) -> Result<Observed, CleanupError> {
    state.sync_all().map_err(|_| CleanupError::Ambiguous)?;
    let parent = state.metadata().map_err(|_| CleanupError::ManualRecovery)?;
    let staged = stage_step(state, uid, receipt)?;
    let terminal = read_optional(state, TERMINAL, uid, RECORD_BYTES)?;
    let intent = read_optional(state, INTENT, uid, RECORD_BYTES)?;
    if let Some((bytes, _)) = &terminal
        && bytes.as_slice() != receipt.terminal().encode()
    {
        return Err(CleanupError::ManualRecovery);
    }
    if let Some((bytes, _)) = &intent {
        let record = DecisionRecord::decode(bytes).map_err(|_| CleanupError::ManualRecovery)?;
        if !record.is_intent_of(receipt.terminal()) {
            return Err(CleanupError::ManualRecovery);
        }
    }
    let observed = match (staged, terminal, intent) {
        (Some(stage), Some(_), Some(_)) => stage,
        (None, Some((_, metadata)), Some(_)) => Observed {
            step: Step::Terminal,
            target: Some(metadata),
            stage_parent: None,
        },
        (None, None, Some((_, metadata))) => Observed {
            step: Step::Intent,
            target: Some(metadata),
            stage_parent: None,
        },
        (None, None, None) => Observed {
            step: Step::Done,
            target: None,
            stage_parent: None,
        },
        _ => return Err(CleanupError::ManualRecovery),
    };
    if !same_directory(
        &parent,
        &open_private_directory(&paths.state_directory, uid)
            .map_err(|_| CleanupError::ManualRecovery)?
            .metadata()
            .map_err(|_| CleanupError::ManualRecovery)?,
    ) {
        return Err(CleanupError::ManualRecovery);
    }
    Ok(observed)
}

struct Context<'a> {
    config: &'a Path,
    paths: &'a CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &'a MigrationLock,
    state: File,
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
    ) -> Result<Self, CleanupError> {
        let state = open_private_directory(&paths.state_directory, uid)
            .map_err(|_| CleanupError::ManualRecovery)?;
        let state_identity = state.metadata().map_err(|_| CleanupError::ManualRecovery)?;
        Ok(Self {
            config,
            paths,
            uid,
            generation,
            lock,
            state,
            state_identity,
            receipt_identity: None,
        })
    }

    fn check_bindings(&mut self) -> Result<Observed, CleanupError> {
        if !self.lock.authorizes(self.paths, self.uid) {
            return Err(CleanupError::Admission);
        }
        let (receipt, identity) = durable_retirement_receipt(
            self.config,
            self.paths,
            self.uid,
            self.generation,
            self.lock,
        )
        .map_err(|_| CleanupError::ManualRecovery)?;
        if replacement_slot_pending(self.config) {
            return Err(CleanupError::ManualRecovery);
        }
        if self
            .receipt_identity
            .as_ref()
            .is_some_and(|before| !same_member(before, &identity))
            || !same_directory(
                &self.state_identity,
                &self
                    .state
                    .metadata()
                    .map_err(|_| CleanupError::ManualRecovery)?,
            )
        {
            return Err(CleanupError::ManualRecovery);
        }
        let observed = inspect_inventory(self.paths, &self.state, self.uid, &receipt)?;
        self.receipt_identity = Some(identity);
        Ok(observed)
    }

    fn check(&mut self, gate: &mut impl FnMut() -> bool) -> Result<Observed, CleanupError> {
        if !gate() {
            return Err(CleanupError::Admission);
        }
        let observed = self.check_bindings()?;
        if !gate() {
            return Err(CleanupError::Admission);
        }
        let after_gate = self.check_bindings()?;
        let same_target = match (&observed.target, &after_gate.target) {
            (Some(before), Some(after)) => same_member(before, after),
            (None, None) => true,
            _ => false,
        };
        let same_stage = match (&observed.stage_parent, &after_gate.stage_parent) {
            (Some(before), Some(after)) => same_directory(before, after),
            (None, None) => true,
            _ => false,
        };
        if after_gate.step != observed.step || !same_target || !same_stage {
            return Err(CleanupError::ManualRecovery);
        }
        Ok(after_gate)
    }
}

/// Remove only the eight fixed artifacts in order, never the receipt. Any
/// hole, extra entry, drift or failed synchronization preserves its fence.
#[allow(dead_code)]
pub(crate) fn retire_fixed_restore_artifacts(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    gate: impl FnMut() -> bool,
) -> Result<CleanupResult, CleanupError> {
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
) -> Result<CleanupResult, CleanupError> {
    let mut context = Context::new(config, paths, uid, generation, lock)?;
    for _ in 0..=8 {
        let observed = context.check(&mut gate)?;
        let step = observed.step;
        if step == Step::Done {
            return Ok(CleanupResult::RetiredStillFenced);
        }
        let name = step.name().ok_or(CleanupError::ManualRecovery)?;
        // check() performs its final filesystem pass after the last gate
        // callback, so that callback cannot make a stale unlink decision.
        let stage = if step.in_stage() {
            Some(
                open_stage(&context.state, uid)?
                    .ok_or(CleanupError::ManualRecovery)?
                    .0,
            )
        } else {
            None
        };
        if let Some(stage) = &stage
            && !same_directory(
                observed
                    .stage_parent
                    .as_ref()
                    .ok_or(CleanupError::ManualRecovery)?,
                &stage.metadata().map_err(|_| CleanupError::ManualRecovery)?,
            )
        {
            return Err(CleanupError::ManualRecovery);
        }
        let parent = stage.as_ref().unwrap_or(&context.state);
        let flags = OFlag::O_RDONLY
            | OFlag::O_NONBLOCK
            | OFlag::O_NOFOLLOW
            | OFlag::O_CLOEXEC
            | if step == Step::StageDirectory {
                OFlag::O_DIRECTORY
            } else {
                OFlag::empty()
            };
        let target = File::from(
            openat(parent, Path::new(name), flags, Mode::empty())
                .map_err(|_| CleanupError::ManualRecovery)?,
        );
        if !same_member(
            observed
                .target
                .as_ref()
                .ok_or(CleanupError::ManualRecovery)?,
            &target
                .metadata()
                .map_err(|_| CleanupError::ManualRecovery)?,
        ) {
            return Err(CleanupError::ManualRecovery);
        }
        let current = File::from(
            openat(parent, Path::new(name), flags, Mode::empty())
                .map_err(|_| CleanupError::ManualRecovery)?,
        );
        if !same_member(
            observed
                .target
                .as_ref()
                .ok_or(CleanupError::ManualRecovery)?,
            &current
                .metadata()
                .map_err(|_| CleanupError::ManualRecovery)?,
        ) {
            return Err(CleanupError::ManualRecovery);
        }
        unlinkat(
            parent,
            Path::new(name),
            if step == Step::StageDirectory {
                UnlinkatFlags::RemoveDir
            } else {
                UnlinkatFlags::NoRemoveDir
            },
        )
        .map_err(|_| CleanupError::Ambiguous)?;
        if !hook(HookPoint::Unlinked(step)) {
            return Err(CleanupError::Ambiguous);
        }
        parent.sync_all().map_err(|_| CleanupError::Ambiguous)?;
        if !hook(HookPoint::Synced(step)) {
            return Err(CleanupError::Ambiguous);
        }
        let after = context
            .check(&mut gate)
            .map_err(|_| CleanupError::Ambiguous)?;
        if after.step.index() != step.index() + 1 {
            return Err(CleanupError::Ambiguous);
        }
    }
    Err(CleanupError::Ambiguous)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired::DesiredState;
    use crate::restore_decision_candidate::TerminalChoice;
    use crate::restore_executor_candidate::{PendingOutcome, execute_staged_pair};
    use crate::restore_retirement_candidate::{
        RECEIPT_MEMBER, inspect_retirement_receipt, publish_retirement_receipt,
    };
    use crate::restore_staging_candidate::{read_staged_pair, stage_private_pair};
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
            let root =
                crate::test_temp::directory_under(Path::new(&home), "restore-cleanup").unwrap();
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
                    [45; 16],
                    || true,
                ),
                Ok(PendingOutcome::Committed)
            );
            assert_eq!(
                publish_retirement_receipt(&self.config, &self.paths, self.uid, 2, lock, || true,),
                Ok(PendingOutcome::Committed)
            );
        }

        fn assert_fenced_live(&self, lock: &MigrationLock) {
            assert_eq!(
                fs::read(self.config.join("profiles.json")).unwrap(),
                NEW_STORE
            );
            assert_eq!(
                fs::read(self.config.join("route-template.yaml")).unwrap(),
                NEW_TEMPLATE
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
    fn exact_cleanup_leaves_only_independent_receipt_and_is_idempotent() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        for _ in 0..2 {
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                ),
                Ok(CleanupResult::RetiredStillFenced)
            );
            fixture.assert_fenced_live(&lock);
        }
        assert!(
            !fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .exists()
        );
        assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        assert!(!fixture.paths.state_directory.join(INTENT).exists());
    }

    #[test]
    fn aborted_terminal_and_identical_pair_can_retire_without_touching_live() {
        let aborted = Fixture::new();
        let lock = aborted.lock();
        let stage = read_staged_pair(&aborted.paths.state_directory, aborted.uid).unwrap();
        let desired = fs::read(aborted.paths.state_directory.join("desired.json")).unwrap();
        let intent = DecisionRecord::intent(2, Some(&desired), stage.identity(), [46; 16]).unwrap();
        Fixture::member(
            &aborted.paths.state_directory.join(INTENT),
            &intent.encode(),
        );
        Fixture::member(
            &aborted.paths.state_directory.join(TERMINAL),
            &intent.terminal(TerminalChoice::Abort).unwrap().encode(),
        );
        assert_eq!(
            publish_retirement_receipt(
                &aborted.config,
                &aborted.paths,
                aborted.uid,
                2,
                &lock,
                || true,
            ),
            Ok(PendingOutcome::Aborted)
        );
        assert_eq!(
            retire_fixed_restore_artifacts(
                &aborted.config,
                &aborted.paths,
                aborted.uid,
                2,
                &lock,
                || true,
            ),
            Ok(CleanupResult::RetiredStillFenced)
        );
        assert_eq!(
            fs::read(aborted.config.join("profiles.json")).unwrap(),
            OLD_STORE
        );
        assert_eq!(
            fs::read(aborted.config.join("route-template.yaml")).unwrap(),
            OLD_TEMPLATE
        );
        assert!(
            inspect_retirement_receipt(&aborted.config, &aborted.paths, aborted.uid, 2, &lock)
                .is_ok()
        );

        let identical = Fixture::new_pair(OLD_STORE, OLD_TEMPLATE);
        let lock = identical.lock();
        identical.committed(&lock);
        assert_eq!(
            retire_fixed_restore_artifacts(
                &identical.config,
                &identical.paths,
                identical.uid,
                2,
                &lock,
                || true,
            ),
            Ok(CleanupResult::RetiredStillFenced)
        );
        assert_eq!(
            fs::read(identical.config.join("profiles.json")).unwrap(),
            OLD_STORE
        );
        assert!(
            inspect_retirement_receipt(
                &identical.config,
                &identical.paths,
                identical.uid,
                2,
                &lock,
            )
            .is_ok()
        );
    }

    #[test]
    fn every_post_unlink_and_post_sync_interruption_resumes_from_one_prefix() {
        for index in 0..8 {
            for after_sync in [false, true] {
                let fixture = Fixture::new();
                let lock = fixture.lock();
                fixture.committed(&lock);
                let stopped = retire_with_hook(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                    |point| {
                        !matches!(point, HookPoint::Unlinked(step) if !after_sync && step.index() == index)
                            && !matches!(point, HookPoint::Synced(step) if after_sync && step.index() == index)
                    },
                );
                assert_eq!(
                    stopped,
                    Err(CleanupError::Ambiguous),
                    "{index} {after_sync}"
                );
                fixture.assert_fenced_live(&lock);
                drop(lock);
                let lock = fixture.lock();
                assert_eq!(
                    retire_fixed_restore_artifacts(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(CleanupResult::RetiredStillFenced),
                    "{index} {after_sync}"
                );
                fixture.assert_fenced_live(&lock);
            }
        }
    }

    #[test]
    fn abrupt_process_loss_after_every_unlink_and_sync_reopens_to_fenced_retirement() {
        for index in 0..8 {
            for phase in ["unlinked", "synced"] {
                let fixture = Fixture::new();
                let lock = fixture.lock();
                fixture.committed(&lock);
                drop(lock);
                let child = Command::new(std::env::current_exe().unwrap())
                    .arg("--exact")
                    .arg("restore_cleanup_candidate::tests::crash_worker")
                    .env("OMAVLESS_CLEANUP_TEST_ROOT", &fixture.root)
                    .env("OMAVLESS_CLEANUP_TEST_INDEX", index.to_string())
                    .env("OMAVLESS_CLEANUP_TEST_PHASE", phase)
                    .status()
                    .unwrap();
                assert_eq!(child.signal(), Some(9), "{index} {phase}");
                let lock = fixture.lock();
                fixture.assert_fenced_live(&lock);
                assert_eq!(
                    retire_fixed_restore_artifacts(
                        &fixture.config,
                        &fixture.paths,
                        fixture.uid,
                        2,
                        &lock,
                        || true,
                    ),
                    Ok(CleanupResult::RetiredStillFenced),
                    "{index} {phase}"
                );
                fixture.assert_fenced_live(&lock);
            }
        }
    }

    #[test]
    fn crash_worker() {
        let Some(root) = std::env::var_os("OMAVLESS_CLEANUP_TEST_ROOT") else {
            return;
        };
        let fixture = Fixture::reopen(PathBuf::from(root));
        let lock = fixture.lock();
        let index: usize = std::env::var("OMAVLESS_CLEANUP_TEST_INDEX")
            .unwrap()
            .parse()
            .unwrap();
        let phase = std::env::var("OMAVLESS_CLEANUP_TEST_PHASE").unwrap();
        let _ = retire_with_hook(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &lock,
            || true,
            |point| {
                let selected = match point {
                    HookPoint::Unlinked(step) => phase == "unlinked" && step.index() == index,
                    HookPoint::Synced(step) => phase == "synced" && step.index() == index,
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
        panic!("expected abrupt cleanup worker termination");
    }

    #[test]
    fn holes_unexpected_entries_and_bad_journal_refuse_without_further_deletion() {
        for change in 0..4 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let stage = fixture.paths.state_directory.join(PENDING_DIRECTORY);
            match change {
                0 => fs::remove_file(stage.join(MEMBERS[2])).unwrap(),
                1 => fs::remove_file(stage.join(READY_MEMBER)).unwrap(),
                2 => Fixture::member(&stage.join("unexpected"), b"other"),
                _ => Fixture::member(
                    &fixture.paths.state_directory.join(TERMINAL),
                    b"bad journal",
                ),
            }
            let old_first = fs::metadata(stage.join(MEMBERS[0])).unwrap().ino();
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert_eq!(
                fs::metadata(stage.join(MEMBERS[0])).unwrap().ino(),
                old_first
            );
            assert!(crate::pending_private_transaction::pending_at(
                &fixture.paths.state_directory
            ));
        }
    }

    #[test]
    fn receipt_owner_host_and_unsafe_target_refuse_before_unlink() {
        for change in 0..7 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            let inode = fs::metadata(&first).unwrap().ino();
            match change {
                0 => Fixture::member(&fixture.paths.state_directory.join(RECEIPT_MEMBER), b"torn"),
                1 => Fixture::member(
                    &fixture.paths.ownership_marker,
                    br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                ),
                2 => {
                    assert_eq!(
                        retire_fixed_restore_artifacts(
                            &fixture.config,
                            &fixture.paths,
                            fixture.uid,
                            2,
                            &lock,
                            || false,
                        ),
                        Err(CleanupError::Admission)
                    );
                    assert_eq!(fs::metadata(&first).unwrap().ino(), inode);
                    continue;
                }
                _ => match change {
                    3 => {
                        fs::remove_file(&first).unwrap();
                        symlink("new-profiles.json", &first).unwrap();
                    }
                    4 => fs::hard_link(&first, fixture.root.join("linked-copy")).unwrap(),
                    5 => fs::set_permissions(&first, fs::Permissions::from_mode(0o644)).unwrap(),
                    _ => Fixture::member(&first, b"changed staged bytes\n"),
                },
            }
            assert!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true,
                )
                .is_err()
            );
            if change != 3 {
                assert_eq!(fs::metadata(&first).unwrap().ino(), inode);
            }
        }
    }

    #[test]
    fn final_gate_target_replacement_cannot_delete_foreign_or_replaced_member() {
        for same_bytes in [false, true] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            // Keep the unlinked inode alive. Otherwise the filesystem may
            // legitimately reuse its number for the replacement immediately.
            let held_original = File::open(&first).unwrap();
            let old_inode = held_original.metadata().unwrap().ino();
            let mut calls = 0;
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if calls == 2 {
                            fs::remove_file(&first).unwrap();
                            Fixture::member(
                                &first,
                                if same_bytes {
                                    OLD_STORE
                                } else {
                                    b"foreign synthetic bytes\n"
                                },
                            );
                        }
                        true
                    },
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert!(first.exists());
            assert_eq!(held_original.metadata().unwrap().ino(), old_inode);
            assert_ne!(fs::metadata(&first).unwrap().ino(), old_inode);
            assert_eq!(
                fs::read(&first).unwrap(),
                if same_bytes {
                    OLD_STORE
                } else {
                    b"foreign synthetic bytes\n"
                }
            );
            assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        }
    }

    #[test]
    fn final_gate_live_or_desired_drift_refuses_before_first_unlink() {
        for desired_drift in [false, true] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            let before = fs::metadata(&first).unwrap().ino();
            let mut calls = 0;
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if calls == 2 {
                            if desired_drift {
                                Fixture::member(
                                    &fixture.paths.state_directory.join("desired.json"),
                                    &serde_json::to_vec(&DesiredState {
                                        generation: 1,
                                        ..DesiredState::default()
                                    })
                                    .unwrap(),
                                );
                            } else {
                                Fixture::member(
                                    &fixture.config.join("profiles.json"),
                                    b"foreign live bytes\n",
                                );
                            }
                        }
                        true
                    },
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert_eq!(fs::metadata(&first).unwrap().ino(), before);
            assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        }
    }

    #[test]
    fn surviving_or_new_replacement_slot_preserves_complete_stage() {
        for introduced_at_gate in [false, true] {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.committed(&lock);
            let first = fixture
                .paths
                .state_directory
                .join(PENDING_DIRECTORY)
                .join(MEMBERS[0]);
            let before = fs::metadata(&first).unwrap().ino();
            let slot = fixture.config.join(NEW_SLOT[0]);
            if !introduced_at_gate {
                Fixture::member(&slot, NEW_STORE);
            }
            let mut calls = 0;
            assert_eq!(
                retire_fixed_restore_artifacts(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || {
                        calls += 1;
                        if introduced_at_gate && calls == 2 {
                            Fixture::member(&slot, NEW_STORE);
                        }
                        true
                    },
                ),
                Err(CleanupError::ManualRecovery)
            );
            assert_eq!(fs::metadata(&first).unwrap().ino(), before);
            assert!(slot.exists());
            assert!(fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        }
    }

    #[test]
    fn final_gate_receipt_loss_cannot_remove_last_journal_fence() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.committed(&lock);
        assert_eq!(
            retire_with_hook(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true,
                |point| point != HookPoint::Synced(Step::Terminal),
            ),
            Err(CleanupError::Ambiguous)
        );
        assert!(fixture.paths.state_directory.join(INTENT).exists());
        assert!(!fixture.paths.state_directory.join(TERMINAL).exists());
        let mut calls = 0;
        assert_eq!(
            retire_fixed_restore_artifacts(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || {
                    calls += 1;
                    if calls == 2 {
                        fs::remove_file(fixture.paths.state_directory.join(RECEIPT_MEMBER))
                            .unwrap();
                    }
                    true
                },
            ),
            Err(CleanupError::ManualRecovery)
        );
        assert!(fixture.paths.state_directory.join(INTENT).exists());
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
    }
}
