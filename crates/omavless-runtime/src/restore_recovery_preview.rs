// SPDX-License-Identifier: MIT
//! Authenticated recovery presentation DATA, never an engine reservation.
//! No manager query, Stop, sync, record publication, unlink or owner admission.
//! Confirm must authenticate again and compare under its OWN existing lease.

use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::restore_cleanup_candidate::{PrivateMember, read_optional};
use crate::restore_closure_model::{CLOSURE_MEMBER, ClosureRecord, NEXT_CLOSURE_MEMBER};
use crate::restore_decision_candidate::{DecisionPhase, RecoveryReview};
use crate::restore_disposition_complete_model::{COMPLETE_BYTES, COMPLETE_MEMBER, CompleteRecord};
use crate::restore_disposition_ticket_model::{TICKET_BYTES, TICKET_MEMBER, Ticket};
use crate::restore_executor_candidate::{NEW_SLOT, OLD_SLOT};
use crate::restore_journal_candidate::{inspect_decision_journal, read_desired_for_decision};
use crate::restore_retirement_candidate::{RECEIPT_BYTES, RECEIPT_MEMBER};
use crate::restore_staging_candidate::{
    PENDING_DIRECTORY, classify_live_pair_bound, read_staged_pair,
};
use crate::restore_successor_handoff_model::SUCCESSOR_MEMBER;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use omavless_domain::private_backup::OpenedBackup;
use rustix::fs::{RawDir, SeekFrom, flistxattr, seek};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[cfg(test)]
#[path = "restore_recovery_preview_tests.rs"]
mod tests;

/// Closed proposed procedure, not a user-selectable executor step or grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Old,
    Mixed,
    Committed,
    FinalOld,
    FinalNew,
    NewTicket,
    NewComplete,
}
impl Choice {
    pub fn keeps_new(self) -> bool {
        matches!(
            self,
            Self::Committed | Self::FinalNew | Self::NewTicket | Self::NewComplete
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unavailable,
}
type Result<T> = std::result::Result<T, Error>;
const REFUSE: Error = Error::Unavailable;

/// No private content or reusable authority. Equal hashes do not authorize I/O.
/// No public constructor/decoder: a worker returns this observation to its UI.
pub struct Preview {
    pub(crate) choice: Choice,
    pub(crate) ciphertext: [u8; 32],
    pub(crate) graph: [u8; 32],
    profiles: usize,
    subscriptions: usize,
}
/// One private local worker. Its displayed observation cannot be supplied by
/// IPC or edited by the UI. Confirm consumes the exact stored nonce/binding.
#[cfg(feature = "tui")]
#[derive(Default)]
pub struct Consumer {
    preview: Option<(u64, Preview)>,
    submitted: bool,
}
#[cfg(feature = "tui")]
impl Consumer {
    pub fn execute(
        &mut self,
        request: omavless_tui::private_restore::Request,
    ) -> omavless_tui::private_restore::Completion {
        use omavless_tui::private_restore::RecoveryResult;
        let result = if !crate::product_scope::restore_enabled() {
            RecoveryResult::Denied
        } else if request.remaining().is_none() || self.submitted {
            RecoveryResult::Unknown
        } else if let Some((nonce, previewing, archive, passphrase)) = request.recovery_input() {
            if previewing {
                self.preview = None;
                match preview_current(Path::new(archive), passphrase.as_bytes()) {
                    Ok(observed) => {
                        let (profiles, subscriptions) = observed.counts();
                        let keep_new = observed.choice().keeps_new();
                        self.preview = Some((nonce, observed));
                        RecoveryResult::Preview {
                            profiles: profiles as u64,
                            subscriptions: subscriptions as u64,
                            keep_new,
                        }
                    }
                    Err(_) => RecoveryResult::Denied,
                }
            } else if let Some((expected, observed)) = self.preview.take() {
                if nonce != expected {
                    RecoveryResult::Denied
                } else {
                    self.submitted = true;
                    match crate::restore_old_cli::recover_previewed(archive, passphrase, observed) {
                        Ok(()) => RecoveryResult::Completed,
                        Err(crate::restore_old_cli::Error::AvailabilityUnverified) => {
                            RecoveryResult::AvailabilityUnverified
                        }
                        Err(
                            crate::restore_old_cli::Error::Disabled
                            | crate::restore_old_cli::Error::InvalidInput,
                        ) => RecoveryResult::Denied,
                        Err(crate::restore_old_cli::Error::RecoveryRefused) => {
                            RecoveryResult::Unknown
                        }
                    }
                }
            } else {
                RecoveryResult::Denied
            }
        } else {
            RecoveryResult::Denied
        };
        request.settle_recovery(result)
    }
}

impl Preview {
    pub fn choice(&self) -> Choice {
        self.choice
    }
    pub fn counts(&self) -> (usize, usize) {
        (self.profiles, self.subscriptions)
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.choice == other.choice
            && self.ciphertext == other.ciphertext
            && self.graph == other.graph
            && self.counts() == other.counts()
    }
}

const HISTORY: [&str; 8] = [
    "restore-disposition.history",
    "restore-disposition.history.1",
    "restore-disposition.history.2",
    "restore-disposition.history.3",
    "restore-disposition.history.4",
    "restore-disposition.history.5",
    "restore-disposition.history.6",
    "restore-disposition.history.7",
];
const RECORDS: [(&str, usize); 8] = [
    (
        "restore-decision.intent",
        crate::restore_decision_candidate::RECORD_BYTES,
    ),
    (
        "restore-decision.terminal",
        crate::restore_decision_candidate::RECORD_BYTES,
    ),
    (CLOSURE_MEMBER, crate::restore_closure_model::RECORD_BYTES),
    (
        NEXT_CLOSURE_MEMBER,
        crate::restore_closure_model::RECORD_BYTES,
    ),
    (RECEIPT_MEMBER, RECEIPT_BYTES),
    (
        SUCCESSOR_MEMBER,
        crate::restore_successor_handoff_model::RECORD_BYTES,
    ),
    (TICKET_MEMBER, TICKET_BYTES),
    (COMPLETE_MEMBER, COMPLETE_BYTES),
];

fn stamp(hash: &mut Sha256, m: &Metadata) {
    for n in [
        m.dev(),
        m.ino(),
        m.mode() as u64,
        m.uid() as u64,
        m.gid() as u64,
        m.nlink(),
        m.len(),
        m.mtime() as u64,
        m.mtime_nsec() as u64,
        m.ctime() as u64,
        m.ctime_nsec() as u64,
    ] {
        hash.update(n.to_le_bytes());
    }
}
fn part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn member(hash: &mut Sha256, value: &Option<PrivateMember>) {
    hash.update([u8::from(value.is_some())]);
    if let Some((bytes, m)) = value {
        stamp(hash, m);
        part(hash, bytes);
    }
}
fn names(directory: &File) -> Result<Vec<Vec<u8>>> {
    seek(directory, SeekFrom::Start(0)).map_err(|_| REFUSE)?;
    let mut buffer = [std::mem::MaybeUninit::uninit(); 8192];
    let mut entries = RawDir::new(directory, &mut buffer);
    let mut names = Vec::new();
    for _ in 0..135 {
        let Some(item) = entries.next() else {
            names.sort();
            return Ok(names);
        };
        let entry = item.map_err(|_| REFUSE)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() == 128 || name.is_empty() || name.len() > 255 {
            return Err(REFUSE);
        }
        names.push(name.to_vec());
    }
    Err(REFUSE)
}
fn directory(hash: &mut Sha256, path: &Path, uid: u32) -> Result<(File, Vec<Vec<u8>>)> {
    let file = open_private_directory(path, uid).map_err(|_| REFUSE)?;
    let metadata = file.metadata().map_err(|_| REFUSE)?;
    let mut attrs = [0; 1];
    if metadata.gid() != nix::unistd::getgid().as_raw()
        || flistxattr(&file, &mut attrs).map_err(|_| REFUSE)? != 0
    {
        return Err(REFUSE);
    }
    stamp(hash, &metadata);
    let entries = names(&file)?;
    hash.update((entries.len() as u64).to_le_bytes());
    for name in &entries {
        part(hash, name);
    }
    Ok((file, entries))
}
fn read(directory: &File, name: &str, uid: u32, limit: usize) -> Result<Option<PrivateMember>> {
    let value = read_optional(directory, name, uid, limit).map_err(|_| REFUSE)?;
    if let Some((_, meta)) = &value {
        let file = File::from(
            openat(
                directory,
                name,
                OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let mut attrs = [0; 1];
        if meta.gid() != nix::unistd::getgid().as_raw()
            || !crate::restore_staging_candidate::same_member(
                meta,
                &file.metadata().map_err(|_| REFUSE)?,
            )
            || flistxattr(&file, &mut attrs).map_err(|_| REFUSE)? != 0
        {
            return Err(REFUSE);
        }
    }
    Ok(value)
}

fn snapshot(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    lock: &MigrationLock,
    backup: &OpenedBackup,
) -> Result<(Choice, [u8; 32])> {
    if !lock.authorizes(paths, uid) {
        return Err(REFUSE);
    }
    let marker = read_marker_existing(paths, uid).map_err(|_| REFUSE)?;
    if marker.phase() != OwnershipPhase::Rust {
        return Err(REFUSE);
    }
    let desired = read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)?;
    let mut hash = Sha256::new();
    hash.update(b"omavless-recovery-preview-data-v1\0");
    let (state, state_names) = directory(&mut hash, &paths.state_directory, uid)?;
    let (config_dir, config_names) = directory(&mut hash, config, uid)?;
    let (runtime, _) = directory(&mut hash, &paths.runtime_base, uid)?;
    for name in &state_names {
        if (name.starts_with(b"restore-") || name.starts_with(b".restore-"))
            && name != PENDING_DIRECTORY.as_bytes()
            && !RECORDS.iter().any(|(known, _)| known.as_bytes() == name)
            && !HISTORY.iter().any(|known| known.as_bytes() == name)
        {
            return Err(REFUSE);
        }
    }
    for name in &config_names {
        if (name.starts_with(b".restore-") || name.starts_with(b"restore-"))
            && !NEW_SLOT
                .into_iter()
                .chain(OLD_SLOT)
                .any(|known| known.as_bytes() == name)
        {
            return Err(REFUSE);
        }
    }
    for (directory, name, limit) in [
        (&state, "ownership.json", 1024),
        (&state, "desired.json", 65536),
        (&runtime, "omavless-login.receipt", 1024),
    ] {
        member(&mut hash, &read(directory, name, uid, limit)?);
    }
    let mut records = Vec::new();
    for (name, limit) in RECORDS {
        let value = read(&state, name, uid, limit)?;
        member(&mut hash, &value);
        records.push(value);
    }
    if records[3].is_some()
        || records[5].is_some()
        || read(&state, "routing-preset.pending.json", uid, 65536)?.is_some()
    {
        return Err(REFUSE);
    }
    let mut history_gap = false;
    let mut history_count = 0;
    for name in HISTORY {
        let value = read(&state, name, uid, COMPLETE_BYTES)?;
        member(&mut hash, &value);
        if let Some((bytes, _)) = value {
            if history_gap || CompleteRecord::decode(&bytes).is_none() {
                return Err(REFUSE);
            }
            history_count += 1;
        } else {
            history_gap = true;
        }
    }
    if history_count == HISTORY.len() {
        return Err(REFUSE);
    }
    let live_store = read(
        &config_dir,
        "profiles.json",
        uid,
        omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
    )?
    .ok_or(REFUSE)?;
    let live_template = read(
        &config_dir,
        "route-template.yaml",
        uid,
        omavless_domain::config::MAX_TEMPLATE_BYTES,
    )?
    .ok_or(REFUSE)?;
    for (bytes, meta) in [&live_store, &live_template] {
        stamp(&mut hash, meta);
        part(&mut hash, bytes);
    }
    let mut slots = Vec::new();
    for (i, name) in OLD_SLOT.into_iter().chain(NEW_SLOT).enumerate() {
        let value = read(
            &config_dir,
            name,
            uid,
            if i % 2 == 0 {
                omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES
            } else {
                omavless_domain::config::MAX_TEMPLATE_BYTES
            },
        )?;
        member(&mut hash, &value);
        slots.push(value);
    }
    let new = backup.restore_store_off().map_err(|_| REFUSE)?;
    let stage_present = state_names
        .iter()
        .any(|name| name == PENDING_DIRECTORY.as_bytes());
    let choice = if stage_present {
        // No existing-only record is silently used as a terminal writer grant.
        if records[2..].iter().any(Option::is_some) {
            return Err(REFUSE);
        }
        let stage = read_staged_pair(&paths.state_directory, uid).map_err(|_| REFUSE)?;
        let (stage_dir, stage_names) = directory(
            &mut hash,
            &paths.state_directory.join(PENDING_DIRECTORY),
            uid,
        )?;
        let stage_members = [
            (
                "old-profiles.json",
                omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
            ),
            (
                "old-route-template.yaml",
                omavless_domain::config::MAX_TEMPLATE_BYTES,
            ),
            (
                "new-profiles.json",
                omavless_domain::private_store::MAX_PRIVATE_STORE_BYTES,
            ),
            (
                "new-route-template.yaml",
                omavless_domain::config::MAX_TEMPLATE_BYTES,
            ),
            (
                crate::restore_staging_candidate::READY_MEMBER,
                crate::restore_staging_candidate::READY_BYTES,
            ),
        ];
        if stage_names.len() != stage_members.len()
            || stage_names.iter().any(|name| {
                !stage_members
                    .iter()
                    .any(|(known, _)| known.as_bytes() == name)
            })
        {
            return Err(REFUSE);
        }
        for (name, limit) in stage_members {
            let observed = read(&stage_dir, name, uid, limit)?.ok_or(REFUSE)?;
            stamp(&mut hash, &observed.1);
            part(&mut hash, &observed.0);
        }
        if stage.new_store() != new.as_slice() || stage.new_template() != backup.template() {
            return Err(REFUSE);
        }
        for bytes in [
            stage.old_store(),
            stage.old_template(),
            stage.new_store(),
            stage.new_template(),
        ] {
            part(&mut hash, bytes);
        }
        let chain = inspect_decision_journal(paths, uid, lock).map_err(|_| REFUSE)?;
        if !chain.active().matches_stage_identity(stage.identity()) {
            return Err(REFUSE);
        }
        let live = classify_live_pair_bound(config, paths, uid, marker.generation(), lock)
            .map_err(|_| REFUSE)?;
        match chain.review(
            marker.generation(),
            desired.as_ref().map(|b| b.as_slice()),
            &live,
        ) {
            RecoveryReview::OldRollbackCandidate => {
                if live.class() == crate::restore_staging_candidate::LivePairClass::Old
                    || live.class() == crate::restore_staging_candidate::LivePairClass::Identical
                {
                    Choice::Old
                } else {
                    Choice::Mixed
                }
            }
            RecoveryReview::VerifyAbortedCandidate => Choice::Old,
            RecoveryReview::VerifyCommittedCandidate => Choice::Committed,
            RecoveryReview::ManualRecovery => return Err(REFUSE),
        }
    } else {
        if records[0].is_some()
            || records[1].is_some()
            || records[4].is_some()
            || slots.iter().any(Option::is_some)
        {
            return Err(REFUSE);
        }
        let complete = records[7]
            .as_ref()
            .map(|(raw, _)| CompleteRecord::decode(raw).ok_or(REFUSE))
            .transpose()?;
        let ticket = records[6]
            .as_ref()
            .map(|(raw, _)| Ticket::decode(raw).ok_or(REFUSE))
            .transpose()?;
        let closure = records[2]
            .as_ref()
            .map(|(raw, _)| ClosureRecord::decode(raw).map_err(|_| REFUSE))
            .transpose()?;
        let canonical = if let Some(complete) = &complete {
            let expected_ticket = complete.ticket_data();
            if ticket.as_ref().is_some_and(|t| !complete.matches_ticket(t))
                || closure
                    .as_ref()
                    .is_some_and(|c| c.encode() != expected_ticket.closure_data().encode())
                || (closure.is_some() && ticket.is_none())
            {
                return Err(REFUSE);
            }
            expected_ticket.closure_data()
        } else {
            closure.as_ref().ok_or(REFUSE)?
        };
        if !canonical
            .receipt()
            .terminal()
            .matches_owner_desired(marker.generation(), desired.as_ref().map(|b| b.as_slice()))
            || !crate::restore_executor_candidate::successor::rotation::final_review::output_binding(
                canonical.receipt(),
                [&live_store.0, &live_template.0],
                [&new, backup.template()],
            )
        {
            return Err(REFUSE);
        }
        if let Some(ticket) = ticket
            .as_ref()
            .or_else(|| complete.as_ref().map(|c| c.ticket_data()))
        {
            if canonical.receipt().terminal().phase() != DecisionPhase::Committed
                || !ticket.matches(
                    canonical,
                    uid,
                    marker.generation(),
                    desired.as_ref().map(|b| b.as_slice()),
                )
            {
                return Err(REFUSE);
            }
            if complete.is_some() {
                Choice::NewComplete
            } else {
                Choice::NewTicket
            }
        } else {
            match canonical.receipt().terminal().phase() {
                DecisionPhase::Committed => Choice::FinalNew,
                DecisionPhase::Aborted => Choice::FinalOld,
                DecisionPhase::Intent => return Err(REFUSE),
            }
        }
    };
    if !lock.authorizes(paths, uid)
        || read_marker_existing(paths, uid).ok() != Some(marker)
        || read_desired_for_decision(paths, uid, lock).map_err(|_| REFUSE)? != desired
    {
        return Err(REFUSE);
    }
    Ok((choice, hash.finalize().into()))
}

/// Caller already authenticated the SAME guarded ciphertext. This temporary
/// lease is existing-only and dropped by Preview; it is not FreshRecovery.
pub(crate) fn inspect_authenticated(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    ciphertext: [u8; 32],
) -> Result<Preview> {
    inspect_with_hook(config, paths, uid, lock, backup, ciphertext, || {})
}

fn inspect_with_hook(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    lock: &MigrationLock,
    backup: &OpenedBackup,
    ciphertext: [u8; 32],
    between: impl FnOnce(),
) -> Result<Preview> {
    let first = snapshot(config, paths, uid, lock, backup)?;
    between();
    let second = snapshot(config, paths, uid, lock, backup)?;
    if first != second {
        return Err(REFUSE);
    }
    Ok(Preview {
        choice: first.0,
        graph: first.1,
        ciphertext,
        profiles: backup.profile_count(),
        subscriptions: backup.subscription_count(),
    })
}

pub fn preview_current(archive: &Path, passphrase: &[u8]) -> Result<Preview> {
    if !crate::product_scope::restore_enabled() {
        return Err(REFUSE);
    }
    let uid = nix::unistd::getuid();
    if uid.is_root()
        || uid != nix::unistd::geteuid()
        || nix::unistd::getgid() != nix::unistd::getegid()
    {
        return Err(REFUSE);
    }
    let runtime = crate::RuntimePaths::current().map_err(|_| REFUSE)?;
    let paths = CutoverPaths::current(uid.as_raw()).map_err(|_| REFUSE)?;
    let host =
        crate::native_host::NativeHostPaths::current(&runtime.directory).map_err(|_| REFUSE)?;
    let (backup, digest) = crate::backup_destination_candidate::open_existing_with_digest(
        archive,
        uid.as_raw(),
        passphrase,
    )
    .map_err(|_| REFUSE)?;
    let lock = MigrationLock::acquire_existing(&paths, uid.as_raw()).map_err(|_| REFUSE)?;
    inspect_authenticated(
        &host.config_directory,
        &paths,
        uid.as_raw(),
        &lock,
        &backup,
        digest,
    )
}
