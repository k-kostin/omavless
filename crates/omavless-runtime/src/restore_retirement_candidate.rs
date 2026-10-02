// SPDX-License-Identifier: MIT

//! Inactive standalone evidence for eventual retirement of restore copies.
//! Publishing this fixed receipt deletes nothing and never clears the pending
//! fence. Only synthetic tests call it; installed restore remains unavailable.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::restore_decision_candidate::{DecisionPhase, DecisionRecord, RECORD_BYTES};
use crate::restore_executor_candidate::{PendingOutcome, verify_terminal_staged_pair};
use crate::restore_journal_candidate::{inspect_decision_journal, read_desired_for_decision};
use crate::restore_staging_candidate::{
    VerifiedStage, read_member, read_staged_pair, same_directory, same_member,
};
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(crate) const RECEIPT_MEMBER: &str = "restore-finalization.pending";
const MAGIC: &[u8; 8] = b"OVRFIN01";
const CHECKSUM_DOMAIN: &[u8] = b"omavless-restore-finalization-v1\0";
const BODY_BYTES: usize = 8 + RECORD_BYTES + (4 + 32) * 2;
const RECEIPT_BYTES: usize = BODY_BYTES + 32;
const LIVE: [&str; 2] = ["profiles.json", "route-template.yaml"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetirementError {
    Admission,
    ManualRecovery,
    Ambiguous,
}

#[derive(Clone, Copy)]
struct MemberBinding {
    length: u32,
    digest: [u8; 32],
}

impl MemberBinding {
    fn from_bytes(bytes: &[u8]) -> Result<Self, RetirementError> {
        Ok(Self {
            length: u32::try_from(bytes.len()).map_err(|_| RetirementError::Admission)?,
            digest: Sha256::digest(bytes).into(),
        })
    }

    fn matches(&self, bytes: &[u8]) -> bool {
        usize::try_from(self.length).ok() == Some(bytes.len())
            && self.digest == Sha256::digest(bytes)[..]
    }
}

/// No raw profile/template or desired-state bytes occur in this record. The
/// checksum detects tears, not an adversarial same-user rewrite.
pub(crate) struct RetirementReceipt {
    terminal: DecisionRecord,
    expected: [MemberBinding; 2],
}

impl RetirementReceipt {
    pub(crate) fn terminal(&self) -> &DecisionRecord {
        &self.terminal
    }

    fn from_terminal(
        terminal: &DecisionRecord,
        stage: &VerifiedStage,
    ) -> Result<Self, RetirementError> {
        let (store, template) = match terminal.phase() {
            DecisionPhase::Committed => (stage.new_store(), stage.new_template()),
            DecisionPhase::Aborted => (stage.old_store(), stage.old_template()),
            DecisionPhase::Intent => return Err(RetirementError::Admission),
        };
        Ok(Self {
            terminal: DecisionRecord::decode(&terminal.encode())
                .map_err(|_| RetirementError::Admission)?,
            expected: [
                MemberBinding::from_bytes(store)?,
                MemberBinding::from_bytes(template)?,
            ],
        })
    }

    fn encode(&self) -> [u8; RECEIPT_BYTES] {
        let mut raw = [0_u8; RECEIPT_BYTES];
        raw[..8].copy_from_slice(MAGIC);
        raw[8..8 + RECORD_BYTES].copy_from_slice(&self.terminal.encode());
        let mut offset = 8 + RECORD_BYTES;
        for member in self.expected {
            raw[offset..offset + 4].copy_from_slice(&member.length.to_be_bytes());
            raw[offset + 4..offset + 36].copy_from_slice(&member.digest);
            offset += 36;
        }
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&raw[..BODY_BYTES]);
        raw[BODY_BYTES..].copy_from_slice(&checksum.finalize());
        raw
    }

    fn decode(raw: &[u8]) -> Result<Self, RetirementError> {
        if raw.len() != RECEIPT_BYTES || &raw[..8] != MAGIC {
            return Err(RetirementError::ManualRecovery);
        }
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&raw[..BODY_BYTES]);
        if raw[BODY_BYTES..] != checksum.finalize()[..] {
            return Err(RetirementError::ManualRecovery);
        }
        let terminal = DecisionRecord::decode(&raw[8..8 + RECORD_BYTES])
            .map_err(|_| RetirementError::ManualRecovery)?;
        if terminal.phase() == DecisionPhase::Intent {
            return Err(RetirementError::ManualRecovery);
        }
        let mut expected = [MemberBinding {
            length: 0,
            digest: [0; 32],
        }; 2];
        let mut offset = 8 + RECORD_BYTES;
        for (index, limit) in [MAX_PRIVATE_STORE_BYTES, MAX_TEMPLATE_BYTES]
            .into_iter()
            .enumerate()
        {
            let length = u32::from_be_bytes(raw[offset..offset + 4].try_into().unwrap());
            let digest: [u8; 32] = raw[offset + 4..offset + 36].try_into().unwrap();
            if length == 0 || length as usize > limit || digest == [0; 32] {
                return Err(RetirementError::ManualRecovery);
            }
            expected[index] = MemberBinding { length, digest };
            offset += 36;
        }
        Ok(Self { terminal, expected })
    }

    pub(crate) fn matches_live(&self, config: &Path, uid: u32) -> Result<bool, RetirementError> {
        let directory =
            open_private_directory(config, uid).map_err(|_| RetirementError::ManualRecovery)?;
        let before = directory
            .metadata()
            .map_err(|_| RetirementError::ManualRecovery)?;
        for (index, name) in LIVE.into_iter().enumerate() {
            let limit = if index == 0 {
                MAX_PRIVATE_STORE_BYTES
            } else {
                MAX_TEMPLATE_BYTES
            };
            let bytes = read_member(&directory, name, uid, limit)
                .map_err(|_| RetirementError::ManualRecovery)?;
            if !self.expected[index].matches(&bytes) {
                return Ok(false);
            }
        }
        Ok(same_directory(
            &before,
            &open_private_directory(config, uid)
                .map_err(|_| RetirementError::ManualRecovery)?
                .metadata()
                .map_err(|_| RetirementError::ManualRecovery)?,
        ))
    }
}

fn read_receipt_member(
    paths: &CutoverPaths,
    uid: u32,
) -> Result<(RetirementReceipt, Metadata), RetirementError> {
    let directory = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| RetirementError::ManualRecovery)?;
    let before = directory
        .metadata()
        .map_err(|_| RetirementError::ManualRecovery)?;
    let mut file = File::from(
        openat(
            &directory,
            Path::new(RECEIPT_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| RetirementError::ManualRecovery)?,
    );
    let metadata = file
        .metadata()
        .map_err(|_| RetirementError::ManualRecovery)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() != RECEIPT_BYTES as u64
    {
        return Err(RetirementError::ManualRecovery);
    }
    let mut bytes = Vec::with_capacity(RECEIPT_BYTES);
    Read::by_ref(&mut file)
        .take(RECEIPT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| RetirementError::ManualRecovery)?;
    let reopened = File::from(
        openat(
            &directory,
            Path::new(RECEIPT_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| RetirementError::ManualRecovery)?,
    );
    if !same_member(
        &metadata,
        &file
            .metadata()
            .map_err(|_| RetirementError::ManualRecovery)?,
    ) || !same_member(
        &metadata,
        &reopened
            .metadata()
            .map_err(|_| RetirementError::ManualRecovery)?,
    ) || !same_directory(
        &before,
        &open_private_directory(&paths.state_directory, uid)
            .map_err(|_| RetirementError::ManualRecovery)?
            .metadata()
            .map_err(|_| RetirementError::ManualRecovery)?,
    ) {
        return Err(RetirementError::ManualRecovery);
    }
    Ok((RetirementReceipt::decode(&bytes)?, metadata))
}

/// Independently inspect terminal evidence after the original stage/journal
/// may be gone. This does not authorize their deletion or clear the fence.
#[allow(dead_code)]
pub(crate) fn inspect_retirement_receipt(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<RetirementReceipt, RetirementError> {
    if !lock.authorizes(paths, uid) {
        return Err(RetirementError::Admission);
    }
    let (receipt, before) = read_receipt_member(paths, uid)?;
    let marker = read_marker_existing(paths, uid).map_err(|_| RetirementError::Admission)?;
    let desired =
        read_desired_for_decision(paths, uid, lock).map_err(|_| RetirementError::ManualRecovery)?;
    if marker.phase() != OwnershipPhase::Rust
        || marker.generation() != generation
        || !receipt
            .terminal
            .matches_owner_desired(generation, desired.as_ref().map(|value| value.as_slice()))
        || !receipt.matches_live(config, uid)?
    {
        return Err(RetirementError::ManualRecovery);
    }
    let (again, after) = read_receipt_member(paths, uid)?;
    if !same_member(&before, &after)
        || receipt.encode() != again.encode()
        || read_marker_existing(paths, uid).map_err(|_| RetirementError::Admission)? != marker
        || read_desired_for_decision(paths, uid, lock)
            .map_err(|_| RetirementError::ManualRecovery)?
            != desired
        || !receipt.matches_live(config, uid)?
    {
        return Err(RetirementError::ManualRecovery);
    }
    Ok(receipt)
}

fn write_receipt(
    paths: &CutoverPaths,
    uid: u32,
    bytes: &[u8; RECEIPT_BYTES],
) -> Result<Metadata, RetirementError> {
    let directory = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| RetirementError::ManualRecovery)?;
    let before = directory
        .metadata()
        .map_err(|_| RetirementError::ManualRecovery)?;
    let mut file = File::from(
        openat(
            &directory,
            Path::new(RECEIPT_MEMBER),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| RetirementError::Ambiguous)?,
    );
    let identity = file.metadata().map_err(|_| RetirementError::Ambiguous)?;
    if !identity.is_file()
        || identity.uid() != uid
        || identity.mode() & 0o7777 != 0o600
        || identity.nlink() != 1
    {
        return Err(RetirementError::Ambiguous);
    }
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| RetirementError::Ambiguous)?;
    let durable_identity = file.metadata().map_err(|_| RetirementError::Ambiguous)?;
    if durable_identity.len() != RECEIPT_BYTES as u64
        || durable_identity.uid() != uid
        || durable_identity.mode() & 0o7777 != 0o600
        || durable_identity.nlink() != 1
    {
        return Err(RetirementError::Ambiguous);
    }
    directory
        .sync_all()
        .map_err(|_| RetirementError::Ambiguous)?;
    let current = File::from(
        openat(
            &directory,
            Path::new(RECEIPT_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| RetirementError::Ambiguous)?,
    );
    if !same_member(
        &durable_identity,
        &file.metadata().map_err(|_| RetirementError::Ambiguous)?,
    ) || !same_member(
        &durable_identity,
        &current.metadata().map_err(|_| RetirementError::Ambiguous)?,
    ) || !same_directory(
        &before,
        &open_private_directory(&paths.state_directory, uid)
            .map_err(|_| RetirementError::Ambiguous)?
            .metadata()
            .map_err(|_| RetirementError::Ambiguous)?,
    ) {
        return Err(RetirementError::Ambiguous);
    }
    Ok(durable_identity)
}

/// Re-establish file and directory durability before any later cleanup effect.
/// The returned inode identity is a binding, not permission to unlink.
#[allow(dead_code)]
pub(crate) fn durable_retirement_receipt(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<(RetirementReceipt, Metadata), RetirementError> {
    let (receipt, before) = read_receipt_member(paths, uid)?;
    let directory = open_private_directory(&paths.state_directory, uid)
        .map_err(|_| RetirementError::ManualRecovery)?;
    let parent = directory
        .metadata()
        .map_err(|_| RetirementError::ManualRecovery)?;
    let file = File::from(
        openat(
            &directory,
            Path::new(RECEIPT_MEMBER),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| RetirementError::ManualRecovery)?,
    );
    if !same_member(
        &before,
        &file
            .metadata()
            .map_err(|_| RetirementError::ManualRecovery)?,
    ) {
        return Err(RetirementError::ManualRecovery);
    }
    file.sync_all().map_err(|_| RetirementError::Ambiguous)?;
    directory
        .sync_all()
        .map_err(|_| RetirementError::Ambiguous)?;
    let (again, after) = read_receipt_member(paths, uid)?;
    if !same_member(&before, &after)
        || receipt.encode() != again.encode()
        || !same_directory(
            &parent,
            &open_private_directory(&paths.state_directory, uid)
                .map_err(|_| RetirementError::ManualRecovery)?
                .metadata()
                .map_err(|_| RetirementError::ManualRecovery)?,
        )
    {
        return Err(RetirementError::ManualRecovery);
    }
    let inspected = inspect_retirement_receipt(config, paths, uid, generation, lock)?;
    if inspected.encode() != receipt.encode() {
        return Err(RetirementError::ManualRecovery);
    }
    Ok((inspected, after))
}

/// Write only a durable, still-fenced receipt for a verified terminal pair.
/// A later separately reviewed procedure may use it as fixed-artifact cleanup
/// evidence. There is intentionally no product or cleanup caller here.
#[allow(dead_code)]
pub(crate) fn publish_retirement_receipt(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    mut gate: impl FnMut() -> bool,
) -> Result<PendingOutcome, RetirementError> {
    if !gate() || !lock.authorizes(paths, uid) {
        return Err(RetirementError::Admission);
    }
    // Retirement evidence is never a way to finish an undecided restore.
    // Intent-only recovery belongs to the earlier executor and its separate
    // authorization path; this publication must be read/sync-only for live.
    if inspect_decision_journal(paths, uid, lock)
        .map_err(|_| RetirementError::ManualRecovery)?
        .active()
        .phase()
        == DecisionPhase::Intent
    {
        return Err(RetirementError::ManualRecovery);
    }
    let outcome = verify_terminal_staged_pair(config, paths, uid, generation, lock, &mut gate)
        .map_err(|_| RetirementError::ManualRecovery)?;
    let stage = read_staged_pair(&paths.state_directory, uid)
        .map_err(|_| RetirementError::ManualRecovery)?;
    let chain =
        inspect_decision_journal(paths, uid, lock).map_err(|_| RetirementError::ManualRecovery)?;
    let desired =
        read_desired_for_decision(paths, uid, lock).map_err(|_| RetirementError::ManualRecovery)?;
    let terminal = chain.active();
    let receipt = RetirementReceipt::from_terminal(terminal, &stage)?;
    if !terminal.matches_current_bindings(
        generation,
        desired.as_ref().map(|value| value.as_slice()),
        stage.identity(),
    ) || !receipt.matches_live(config, uid)?
        || !gate()
    {
        return Err(RetirementError::ManualRecovery);
    }
    let written = write_receipt(paths, uid, &receipt.encode())?;
    let inspected = inspect_retirement_receipt(config, paths, uid, generation, lock)?;
    let (_, reopened) = read_receipt_member(paths, uid)?;
    if !same_member(&written, &reopened)
        || inspected.encode() != receipt.encode()
        || !gate()
        || read_staged_pair(&paths.state_directory, uid)
            .map_err(|_| RetirementError::ManualRecovery)?
            .identity()
            .digest()
            != stage.identity().digest()
        || inspect_decision_journal(paths, uid, lock)
            .map_err(|_| RetirementError::ManualRecovery)?
            .active()
            .encode()
            != terminal.encode()
    {
        return Err(RetirementError::ManualRecovery);
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutover::MigrationLock;
    use crate::desired::DesiredState;
    use crate::restore_decision_candidate::TerminalChoice;
    use crate::restore_executor_candidate::execute_staged_pair;
    use crate::restore_staging_candidate::stage_private_pair;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;

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
            let home = std::env::var_os("HOME").expect("test needs home");
            let root = crate::test_temp::directory_under(Path::new(&home), "retirement").unwrap();
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
            Self::member(&config.join(LIVE[0]), OLD_STORE);
            Self::member(&config.join(LIVE[1]), OLD_TEMPLATE);
            stage_private_pair(
                &paths.state_directory,
                uid,
                OLD_STORE,
                OLD_TEMPLATE,
                NEW_STORE,
                NEW_TEMPLATE,
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

        fn lock(&self) -> MigrationLock {
            MigrationLock::acquire(&self.paths, self.uid).unwrap()
        }

        fn commit(&self, lock: &MigrationLock) {
            assert_eq!(
                execute_staged_pair(
                    &self.config,
                    &self.paths,
                    self.uid,
                    2,
                    lock,
                    [21; 16],
                    || true,
                ),
                Ok(PendingOutcome::Committed)
            );
        }

        fn write_journal(&self, terminal: bool) {
            let stage = read_staged_pair(&self.paths.state_directory, self.uid).unwrap();
            let desired = fs::read(self.paths.state_directory.join("desired.json")).unwrap();
            let intent =
                DecisionRecord::intent(2, Some(&desired), stage.identity(), [22; 16]).unwrap();
            Self::member(
                &self.paths.state_directory.join("restore-decision.intent"),
                &intent.encode(),
            );
            if terminal {
                Self::member(
                    &self.paths.state_directory.join("restore-decision.terminal"),
                    &intent.terminal(TerminalChoice::Abort).unwrap().encode(),
                );
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn committed_receipt_is_readable_without_stage_or_original_journal() {
        let fixture = Fixture::new();
        let lock = fixture.lock();
        fixture.commit(&lock);
        assert_eq!(
            publish_retirement_receipt(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Ok(PendingOutcome::Committed)
        );
        let receipt =
            inspect_retirement_receipt(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                .unwrap();
        assert_eq!(receipt.terminal.phase(), DecisionPhase::Committed);
        assert_eq!(
            fs::metadata(fixture.paths.state_directory.join(RECEIPT_MEMBER))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        fs::remove_dir_all(fixture.paths.state_directory.join("restore-pair.pending")).unwrap();
        fs::remove_file(
            fixture
                .paths
                .state_directory
                .join("restore-decision.intent"),
        )
        .unwrap();
        fs::remove_file(
            fixture
                .paths
                .state_directory
                .join("restore-decision.terminal"),
        )
        .unwrap();
        assert!(crate::pending_private_transaction::pending_at(
            &fixture.paths.state_directory
        ));
        assert!(
            inspect_retirement_receipt(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                .is_ok()
        );
    }

    #[test]
    fn aborted_terminal_can_publish_an_independent_old_pair_receipt() {
        let fixture = Fixture::new();
        fixture.write_journal(true);
        let lock = fixture.lock();
        assert_eq!(
            publish_retirement_receipt(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Ok(PendingOutcome::Aborted)
        );
        assert_eq!(
            inspect_retirement_receipt(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                .unwrap()
                .terminal
                .phase(),
            DecisionPhase::Aborted
        );
        assert_eq!(fs::read(fixture.config.join(LIVE[0])).unwrap(), OLD_STORE);
    }

    #[test]
    fn undecided_intent_cannot_use_receipt_publication_to_roll_back() {
        let fixture = Fixture::new();
        fixture.write_journal(false);
        let lock = fixture.lock();
        assert_eq!(
            publish_retirement_receipt(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &lock,
                || true
            ),
            Err(RetirementError::ManualRecovery)
        );
        assert!(!fixture.paths.state_directory.join(RECEIPT_MEMBER).exists());
        assert_eq!(fs::read(fixture.config.join(LIVE[0])).unwrap(), OLD_STORE);
        assert_eq!(
            fs::read(fixture.config.join(LIVE[1])).unwrap(),
            OLD_TEMPLATE
        );
    }

    #[test]
    fn receipt_refuses_torn_duplicate_and_changed_owner_desired_or_live() {
        for change in 0..4 {
            let fixture = Fixture::new();
            let lock = fixture.lock();
            fixture.commit(&lock);
            assert_eq!(
                publish_retirement_receipt(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true
                ),
                Ok(PendingOutcome::Committed)
            );
            assert_eq!(
                publish_retirement_receipt(
                    &fixture.config,
                    &fixture.paths,
                    fixture.uid,
                    2,
                    &lock,
                    || true
                ),
                Err(RetirementError::Ambiguous)
            );
            match change {
                0 => Fixture::member(&fixture.paths.state_directory.join(RECEIPT_MEMBER), b"torn"),
                1 => Fixture::member(
                    &fixture.paths.ownership_marker,
                    br#"{"schemaVersion":1,"generation":3,"phase":"rust"}"#,
                ),
                2 => Fixture::member(
                    &fixture.paths.state_directory.join("desired.json"),
                    &serde_json::to_vec(&DesiredState {
                        generation: 1,
                        ..DesiredState::default()
                    })
                    .unwrap(),
                ),
                _ => Fixture::member(&fixture.config.join(LIVE[1]), b"diverged template"),
            }
            assert!(
                inspect_retirement_receipt(&fixture.config, &fixture.paths, fixture.uid, 2, &lock)
                    .is_err()
            );
            assert!(crate::pending_private_transaction::pending_at(
                &fixture.paths.state_directory
            ));
        }
    }
}
