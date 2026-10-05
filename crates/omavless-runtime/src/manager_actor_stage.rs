// SPDX-License-Identifier: MIT
//! A real fixed developer stage/journal, NOT product Restore authority.
//! All lower reported Files remain in the original actor's pre-reserved ledger.
//! No existing unsafe top-level stager/journal helper, error cleanup or retry.

use super::protocol::Kind;
use super::retained_io::{ChildPlan, FileIo, IO_SLOTS, Slot};
use super::{Unavailable, emit_actor, tick};
use crate::restore_abort_cli::stopped_owner::actor_canonical::Canonical;
use crate::restore_abort_cli::stopped_owner::actor_capture::Retained;
use crate::restore_decision_candidate::{
    DecisionChain, DecisionPhase, DecisionRecord, TerminalChoice,
};
use crate::restore_staging_candidate::{
    LivePairClass, MEMBERS, PENDING_DIRECTORY, READY_MEMBER, class_from_matches,
    planned_stage_identity, ready_bytes, same_directory, same_member,
};
use nix::fcntl::{AtFlags, OFlag, renameat};
use nix::sys::stat::{Mode, fstatat, mkdirat};
use rustix::fs::{RawDir, SeekFrom, flistxattr, seek};
use sha2::{Digest, Sha256};
use std::fs::Metadata;
use std::io::Write;
use std::mem::MaybeUninit;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::time::Instant;

const TRANSACTION: &str = "authenticated-transaction";
const LIVE: [(Slot, &str); 2] = [
    (Slot::OldStore, "profiles.json"),
    (Slot::OldTemplate, "route-template.yaml"),
];
const STAGED: [Slot; 5] = [
    Slot::StageOldStore,
    Slot::StageOldTemplate,
    Slot::StageNewStore,
    Slot::StageNewTemplate,
    Slot::StageReady,
];
const CREATED_DIRECTORIES: [Slot; 4] = [
    Slot::Transaction,
    Slot::Config,
    Slot::State,
    Slot::StageDirectory,
];
pub(super) const ORIGIN_FENCES: usize =
    2 * CREATED_DIRECTORIES.len() + 2 * (LIVE.len() + MEMBERS.len() + 1 + 2);
pub(super) const COMMIT_ORIGIN_FENCES: usize = ORIGIN_FENCES + 8;
pub(super) const COMMITTED_PHASE: &[u8] = b"t4_actor_fixture_pair_committed\n";
const REPLACEMENTS: [(Slot, &str); 2] = [
    (Slot::ReplacementStore, "restore-store.new"),
    (Slot::ReplacementTemplate, "restore-template.new"),
];
pub(super) const SUCCESS_PHASES: [&[u8]; 6] = [
    b"t4_actor_before_fixture_stage\n",
    b"t4_actor_before_fixture_transaction_directory\n",
    b"t4_actor_fixture_ready_inspected\n",
    b"t4_actor_fixture_intent_written\n",
    b"t4_actor_fixture_terminal_written\n",
    b"t4_actor_fixture_stage_recorded\n",
];
const INTENT: &str = "restore-decision.intent";
const TERMINAL: &str = "restore-decision.terminal";
const EPOCH_MEMBERS: [&str; 5] = [
    "actor",
    "first.stdout",
    "first.stderr",
    "reserved",
    "channel",
];

enum StageOwner<'a> {
    Classic(&'a mut Retained),
    Canonical(&'a mut Canonical),
    CanonicalCommit(&'a mut Canonical),
}
impl StageOwner<'_> {
    fn admit(&mut self, io: &mut FileIo, until: Instant) -> Result<(), Unavailable> {
        match self {
            Self::Classic(owner) => io.admit(owner, until),
            Self::Canonical(owner) => io.admit_canonical(owner, until),
            Self::CanonicalCommit(owner) => io.admit_canonical_commit(owner, until),
        }
    }
    fn fence(&mut self, until: Instant) -> Result<(), Unavailable> {
        match self {
            Self::Classic(owner) => owner.transaction_fence(until).map_err(|_| Unavailable),
            Self::Canonical(owner) | Self::CanonicalCommit(owner) => owner.stage_origin(until),
        }
    }
    fn final_fence(&mut self, until: Instant) -> Result<(), Unavailable> {
        match self {
            Self::Classic(owner) => owner.transaction_fence(until).map_err(|_| Unavailable),
            Self::Canonical(owner) | Self::CanonicalCommit(owner) => owner.complete_stage(until),
        }
    }
    fn revoke(&mut self) {
        if let Self::Canonical(owner) | Self::CanonicalCommit(owner) = self {
            owner.revoke();
        }
    }
    fn canonical(&self) -> bool {
        matches!(self, Self::Canonical(_) | Self::CanonicalCommit(_))
    }
    fn commits(&self) -> bool {
        matches!(self, Self::CanonicalCommit(_))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LiveRole {
    Old,
    ReplacementReady,
    Renamed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CommitStep {
    Write(usize),
    Rename(usize),
}
fn pair_steps(
    mut step: impl FnMut(CommitStep) -> Result<(), Unavailable>,
) -> Result<(), Unavailable> {
    // One fixed order, no retry branch. The actual owned-I/O implementation
    // below and fault-cut controls use this same continuation boundary.
    for operation in [
        CommitStep::Write(0),
        CommitStep::Write(1),
        CommitStep::Rename(0),
        CommitStep::Rename(1),
    ] {
        step(operation)?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RenameShape {
    identity: (u64, u64, u32, u32, u32),
    content: (u64, i64, i64),
    ctime: (i64, i64),
    links: u64,
}
impl RenameShape {
    fn of(metadata: &Metadata) -> Self {
        Self {
            identity: (
                metadata.dev(),
                metadata.ino(),
                metadata.uid(),
                metadata.gid(),
                metadata.mode(),
            ),
            content: (metadata.len(), metadata.mtime(), metadata.mtime_nsec()),
            ctime: (metadata.ctime(), metadata.ctime_nsec()),
            links: metadata.nlink(),
        }
    }
    fn own_successor(self, after: Self, links: u64) -> bool {
        self.identity == after.identity
            && self.content == after.content
            && self.links == 1
            && after.links == links
            && after.ctime >= self.ctime
    }
}

// An own rename may change ONLY ctime and link count of the held originals.
// This check is reached once, directly after that exact positive rename, never
// as a generic refresh exemption or a way to adopt an externally changed file.
fn same_after_own_rename(before: &Metadata, after: &Metadata, links: u64) -> bool {
    RenameShape::of(before).own_successor(RenameShape::of(after), links)
}

#[derive(Clone, Copy)]
enum StageCut {
    Admission,
    Origin,
    Directory,
    MemberWrite,
    MemberVerify,
    Catalogue,
    Journal,
    FinalOwner,
}
impl StageCut {
    fn label(self) -> &'static [u8] {
        match self {
            Self::Admission => b"t4_actor_stage_admission_refused\n",
            Self::Origin => b"t4_actor_stage_origin_refused\n",
            Self::Directory => b"t4_actor_stage_directory_refused\n",
            Self::MemberWrite => b"t4_actor_stage_member_write_refused\n",
            Self::MemberVerify => b"t4_actor_stage_member_verify_refused\n",
            Self::Catalogue => b"t4_actor_stage_catalogue_refused\n",
            Self::Journal => b"t4_actor_stage_journal_refused\n",
            Self::FinalOwner => b"t4_actor_stage_final_owner_refused\n",
        }
    }
}
#[derive(Default)]
struct Diagnostic {
    enabled: bool,
    attempted: bool,
    first: Option<StageCut>,
}
impl Diagnostic {
    fn result<T>(
        &mut self,
        original: Result<T, Unavailable>,
        emit: impl FnOnce(&'static [u8]) -> Result<(), Unavailable>,
    ) -> Result<T, Unavailable> {
        if self.enabled && original.is_err() && !self.attempted {
            self.attempted = true; // BEFORE output; preserve original Err on failed emission
            if let Some(cut) = self.first {
                let _ = emit(cut.label());
            }
        }
        original
    }
}

struct Catalogue<'a> {
    expected: &'a [&'a str],
    seen: [bool; 8],
}

impl<'a> Catalogue<'a> {
    fn new(expected: &'a [&'a str]) -> Result<Self, Unavailable> {
        if expected.len() > 6
            || expected
                .iter()
                .any(|name| name.is_empty() || *name == "." || *name == "..")
        {
            return Err(Unavailable);
        }
        for (index, name) in expected.iter().enumerate() {
            if expected[..index].contains(name) {
                return Err(Unavailable);
            }
        }
        Ok(Self {
            expected,
            seen: [false; 8],
        })
    }
    fn accept(&mut self, name: &[u8]) -> Result<(), Unavailable> {
        let index = if name == b"." {
            0
        } else if name == b".." {
            1
        } else {
            self.expected
                .iter()
                .position(|expected| name == expected.as_bytes())
                .map(|index| index + 2)
                .ok_or(Unavailable)?
        };
        if self.seen[index] {
            return Err(Unavailable);
        }
        self.seen[index] = true;
        Ok(())
    }
    fn finish(&self) -> Result<(), Unavailable> {
        if self.seen[..self.expected.len() + 2]
            .iter()
            .all(|value| *value)
        {
            Ok(())
        } else {
            Err(Unavailable)
        }
    }
}

pub(super) struct Stage {
    io: FileIo,
    original: [Option<Metadata>; IO_SLOTS],
    // One fixed scratch buffer before READY, not a growing directory listing.
    directory_buffer: [MaybeUninit<u8>; 8192],
    consumed: bool,
    completed: bool,
    diagnostic: Diagnostic,
    live: [LiveRole; 2],
}

impl Stage {
    pub fn reserve() -> Result<Self, Unavailable> {
        Ok(Self {
            io: FileIo::reserve()?,
            original: std::array::from_fn(|_| None),
            directory_buffer: [MaybeUninit::uninit(); 8192],
            consumed: false,
            completed: false,
            diagnostic: Diagnostic::default(),
            live: [LiveRole::Old; 2],
        })
    }

    pub fn reserve_canonical() -> Self {
        Self {
            io: FileIo::reserve_canonical(),
            original: std::array::from_fn(|_| None),
            directory_buffer: [MaybeUninit::uninit(); 8192],
            consumed: false,
            completed: false,
            diagnostic: Diagnostic {
                enabled: true,
                attempted: false,
                first: None,
            },
            live: [LiveRole::Old; 2],
        }
    }
    pub fn revoke(&mut self) {
        self.completed = false;
        self.consumed = true;
        self.io.revoke();
    }

    fn cut<T>(
        &mut self,
        category: StageCut,
        result: Result<T, Unavailable>,
        until: Instant,
    ) -> Result<T, Unavailable> {
        let _ = until; // the original operation's deadline is used at final emission
        if result.is_err() {
            self.io.revoke();
            self.completed = false;
            if self.diagnostic.first.is_none() {
                self.diagnostic.first = Some(category);
            }
        }
        result
    }

    pub fn permit_request(&mut self, kind: Kind) -> Result<(), Unavailable> {
        // Private admission uses one classic17-FD owner OR the complete
        // canonical original owner. Neither plan admits another acquisition.
        // Once consumed, no request may append another capture or transaction.
        // Only the separately valid normal Halt follows a completed stage.
        if self.consumed && (!self.completed || kind != Kind::Halt) {
            self.completed = false;
            self.io.revoke();
            return Err(Unavailable);
        }
        Ok(())
    }

    fn capture_shape(
        &mut self,
        slot: Slot,
        directory: bool,
        exact_mode: Option<u32>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.io.perform(
            slot,
            || tick(until),
            |file| {
                self.original[slot as usize] = Some(file.metadata().map_err(|_| Unavailable)?);
                Ok(()) // keep the positive metadata return before the post-tick
            },
        )?;
        let original = self.original[slot as usize].as_ref().ok_or(Unavailable)?;
        if original.uid() != 0
            || original.gid() != 0
            || (directory && !original.is_dir())
            || (!directory && (!original.is_file() || original.nlink() != 1))
            || exact_mode.is_some_and(|mode| original.mode() & 0o7777 != mode)
            || (exact_mode.is_none() && original.mode() & 0o022 != 0)
        {
            return Err(Unavailable);
        }
        self.io.perform(
            slot,
            || tick(until),
            |file| {
                let mut buffer = [0_u8; 1];
                if flistxattr(file, &mut buffer).map_err(|_| Unavailable)? != 0 {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )
    }

    fn binding(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        directory: bool,
        until: Instant,
    ) -> Result<(), Unavailable> {
        let original = self.original[slot as usize].as_ref().ok_or(Unavailable)?;
        self.io.perform(
            slot,
            || tick(until),
            |file| {
                let held = file.metadata().map_err(|_| Unavailable)?;
                if !(if directory {
                    same_directory(original, &held) && original.gid() == held.gid()
                } else {
                    same_member(original, &held) && original.gid() == held.gid()
                }) {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )?;
        self.io.perform(
            parent,
            || tick(until),
            |file| {
                let named =
                    fstatat(file, name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(|_| Unavailable)?;
                if original.dev() != named.st_dev
                    || original.ino() != named.st_ino
                    || original.mode() != named.st_mode
                    || original.uid() != named.st_uid
                    || original.gid() != named.st_gid
                    || (!directory
                        && (original.nlink() != named.st_nlink
                            || original.len() != named.st_size as u64
                            || original.mtime() != named.st_mtime
                            || original.mtime_nsec() != named.st_mtime_nsec
                            || original.ctime() != named.st_ctime
                            || original.ctime_nsec() != named.st_ctime_nsec))
                {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )?;
        self.io.perform(
            slot,
            || tick(until),
            |file| {
                let mut buffer = [0_u8; 1];
                if flistxattr(file, &mut buffer).map_err(|_| Unavailable)? != 0 {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )
    }

    fn hierarchy(&mut self, until: Instant) -> Result<(), Unavailable> {
        if let Some(original) = self.original[Slot::Root as usize].as_ref() {
            self.io.perform(
                Slot::Root,
                || tick(until),
                |file| {
                    if !same_directory(original, &file.metadata().map_err(|_| Unavailable)?) {
                        return Err(Unavailable);
                    }
                    Ok(())
                },
            )?;
            self.io.perform(
                Slot::Root,
                || tick(until),
                |_| {
                    let current = std::fs::symlink_metadata("/").map_err(|_| Unavailable)?;
                    if !same_directory(original, &current) || original.gid() != current.gid() {
                        return Err(Unavailable);
                    }
                    Ok(())
                },
            )?;
            self.io.perform(
                Slot::Root,
                || tick(until),
                |file| {
                    let mut buffer = [0_u8; 1];
                    if flistxattr(file, &mut buffer).map_err(|_| Unavailable)? != 0 {
                        return Err(Unavailable);
                    }
                    Ok(())
                },
            )?;
        }
        for (parent, slot, name) in [
            (Slot::Root, Slot::Run, "run"),
            (Slot::Run, Slot::Epoch, "omavless-t4-actor-development"),
            (Slot::Epoch, Slot::Transaction, TRANSACTION),
            (Slot::Transaction, Slot::Config, "config"),
            (Slot::Transaction, Slot::State, "state"),
            (Slot::State, Slot::StageDirectory, PENDING_DIRECTORY),
        ] {
            if self.original[slot as usize].is_some() {
                self.binding(parent, slot, name, true, until)?;
            }
        }
        Ok(())
    }

    fn directory(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        create: bool,
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        let result = self.directory_inner(parent, slot, name, create, held, until);
        self.cut(StageCut::Directory, result, until)
    }

    fn directory_inner(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        create: bool,
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if create {
            let result = held.fence(until);
            self.cut(StageCut::Origin, result, until)?;
            self.hierarchy(until)?;
            self.io.perform(
                parent,
                || tick(until),
                |file| mkdirat(file, name, Mode::S_IRWXU).map_err(|_| Unavailable),
            )?;
            self.io.perform(
                parent,
                || tick(until),
                |file| file.sync_all().map_err(|_| Unavailable),
            )?;
            let result = held.fence(until);
            self.cut(StageCut::Origin, result, until)?;
            self.hierarchy(until)?;
        }
        self.io.child(
            ChildPlan {
                parent,
                slot,
                name,
                flags: OFlag::O_RDONLY | OFlag::O_DIRECTORY,
                mode: Mode::empty(),
            },
            || tick(until),
            |_| Ok(()),
        )?;
        self.capture_shape(
            slot,
            true,
            if create || matches!(slot, Slot::Epoch) {
                Some(0o700)
            } else {
                None
            },
            until,
        )?;
        self.binding(parent, slot, name, true, until)
    }

    fn write_member(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        bytes: &[u8],
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        let result = self.write_member_inner(parent, slot, name, bytes, held, until);
        self.cut(
            if matches!(slot, Slot::Intent | Slot::Terminal) {
                StageCut::Journal
            } else {
                StageCut::MemberWrite
            },
            result,
            until,
        )
    }

    fn write_member_inner(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        bytes: &[u8],
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if bytes.is_empty() {
            return Err(Unavailable);
        }
        let result = held.fence(until);
        self.cut(StageCut::Origin, result, until)?;
        self.hierarchy(until)?;
        self.io.child(
            ChildPlan {
                parent,
                slot,
                name,
                flags: OFlag::O_RDWR | OFlag::O_NONBLOCK | OFlag::O_CREAT | OFlag::O_EXCL,
                mode: Mode::S_IRUSR | Mode::S_IWUSR,
            },
            || tick(until),
            |_| Ok(()),
        )?;
        self.capture_shape(slot, false, Some(0o600), until)?;
        let mut done = 0;
        while done < bytes.len() {
            self.io.perform(
                slot,
                || tick(until),
                |mut file| {
                    let written = file.write(&bytes[done..]).map_err(|_| Unavailable)?;
                    if written == 0 || written > bytes.len() - done {
                        return Err(Unavailable);
                    }
                    done += written;
                    Ok(())
                },
            )?;
        }
        self.io.perform(
            slot,
            || tick(until),
            |file| file.sync_all().map_err(|_| Unavailable),
        )?;
        self.capture_shape(slot, false, Some(0o600), until)?;
        if self.original[slot as usize]
            .as_ref()
            .ok_or(Unavailable)?
            .len()
            != bytes.len() as u64
        {
            return Err(Unavailable);
        }
        self.io.perform(
            parent,
            || tick(until),
            |file| file.sync_all().map_err(|_| Unavailable),
        )?;
        self.verify_member(parent, slot, name, bytes, until)?;
        let result = held.fence(until);
        self.cut(StageCut::Origin, result, until)
    }

    fn verify_member(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        expected: &[u8],
        until: Instant,
    ) -> Result<(), Unavailable> {
        let result = self.verify_member_inner(parent, slot, name, expected, until);
        self.cut(StageCut::MemberVerify, result, until)
    }

    fn verify_member_inner(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        expected: &[u8],
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.binding(parent, slot, name, false, until)?;
        self.verify_bytes(slot, expected, until)?;
        self.binding(parent, slot, name, false, until)
    }

    fn verify_bytes(
        &mut self,
        slot: Slot,
        expected: &[u8],
        until: Instant,
    ) -> Result<(), Unavailable> {
        let mut offset = 0;
        let mut digest = Sha256::new();
        loop {
            let mut buffer = [0; 4096];
            let mut read = 0;
            self.io.perform(
                slot,
                || tick(until),
                |file| {
                    let remaining = expected
                        .len()
                        .checked_add(1)
                        .and_then(|n| n.checked_sub(offset))
                        .ok_or(Unavailable)?;
                    if remaining == 0 {
                        return Err(Unavailable);
                    }
                    read = file
                        .read_at(
                            &mut buffer[..remaining.min(4096)],
                            u64::try_from(offset).map_err(|_| Unavailable)?,
                        )
                        .map_err(|_| Unavailable)?;
                    if read > remaining.min(4096) {
                        return Err(Unavailable);
                    }
                    Ok(())
                },
            )?;
            if read == 0 {
                break;
            }
            offset = offset.checked_add(read).ok_or(Unavailable)?;
            if offset > expected.len() {
                return Err(Unavailable);
            }
            digest.update(&buffer[..read]);
        }
        if offset != expected.len() || digest.finalize()[..] != Sha256::digest(expected)[..] {
            return Err(Unavailable);
        }
        Ok(())
    }

    fn verify_unlinked_old(
        &mut self,
        slot: Slot,
        expected: &[u8],
        until: Instant,
    ) -> Result<(), Unavailable> {
        for read_bytes in [true, false] {
            let original = self.original[slot as usize].as_ref().ok_or(Unavailable)?;
            if original.nlink() != 0 {
                return Err(Unavailable);
            }
            self.io.perform(
                slot,
                || tick(until),
                |file| {
                    let current = file.metadata().map_err(|_| Unavailable)?;
                    if !same_member(original, &current) || original.gid() != current.gid() {
                        return Err(Unavailable);
                    }
                    let mut xattrs = [0; 1];
                    if flistxattr(file, &mut xattrs).map_err(|_| Unavailable)? != 0 {
                        return Err(Unavailable);
                    }
                    Ok(())
                },
            )?;
            if read_bytes {
                self.verify_bytes(slot, expected, until)?;
            }
        }
        Ok(())
    }

    fn commit_sources(
        &mut self,
        members: [&[u8]; 4],
        intent: &[u8],
        until: Instant,
    ) -> Result<LivePairClass, Unavailable> {
        self.hierarchy(until)?;
        for (index, name) in MEMBERS.into_iter().enumerate() {
            self.verify_member(
                Slot::StageDirectory,
                STAGED[index],
                name,
                members[index],
                until,
            )?;
        }
        self.verify_member(
            Slot::StageDirectory,
            Slot::StageReady,
            READY_MEMBER,
            &ready_bytes(members),
            until,
        )?;
        self.verify_member(Slot::State, Slot::Intent, INTENT, intent, until)?;
        let mut config_names = [""; 4];
        let mut count = 2;
        config_names[0] = LIVE[0].1;
        config_names[1] = LIVE[1].1;
        let mut current_is_new = [false; 2];
        for index in 0..2 {
            let (old, live_name) = LIVE[index];
            let (new, temporary_name) = REPLACEMENTS[index];
            match self.live[index] {
                LiveRole::Old => {
                    self.verify_member(Slot::Config, old, live_name, members[index], until)?
                }
                LiveRole::ReplacementReady => {
                    self.verify_member(Slot::Config, old, live_name, members[index], until)?;
                    self.verify_member(
                        Slot::Config,
                        new,
                        temporary_name,
                        members[index + 2],
                        until,
                    )?;
                    config_names[count] = temporary_name;
                    count += 1;
                }
                LiveRole::Renamed => {
                    self.verify_unlinked_old(old, members[index], until)?;
                    self.verify_member(Slot::Config, new, live_name, members[index + 2], until)?;
                    current_is_new[index] = true;
                }
            }
        }
        self.catalogue(Slot::Config, &config_names[..count], until)?;
        self.catalogue(
            Slot::StageDirectory,
            &[MEMBERS[0], MEMBERS[1], MEMBERS[2], MEMBERS[3], READY_MEMBER],
            until,
        )?;
        self.catalogue(Slot::State, &[PENDING_DIRECTORY, INTENT], until)?;
        self.catalogue(Slot::Transaction, &["config", "state"], until)?;
        self.catalogue(
            Slot::Epoch,
            &[
                EPOCH_MEMBERS[0],
                EPOCH_MEMBERS[1],
                EPOCH_MEMBERS[2],
                EPOCH_MEMBERS[3],
                EPOCH_MEMBERS[4],
                TRANSACTION,
            ],
            until,
        )?;
        let actual = [
            members[usize::from(current_is_new[0]) * 2],
            members[1 + usize::from(current_is_new[1]) * 2],
        ];
        // The byte comparisons are backed by the held/name-bound exact reads
        // above, not an imported detached class or invented VerifiedLivePair.
        Ok(class_from_matches(
            actual[0] == members[0],
            actual[1] == members[1],
            actual[0] == members[2],
            actual[1] == members[3],
        ))
    }

    fn advance_own_rename(&mut self, index: usize, until: Instant) -> Result<(), Unavailable> {
        if self.live[index] != LiveRole::Renamed {
            return Err(Unavailable);
        }
        for (slot, links) in [(LIVE[index].0, 0), (REPLACEMENTS[index].0, 1)] {
            self.io.perform(
                slot,
                || tick(until),
                |file| {
                    let after = file.metadata().map_err(|_| Unavailable)?;
                    let before = self.original[slot as usize].as_ref().ok_or(Unavailable)?;
                    if !same_after_own_rename(before, &after, links) {
                        return Err(Unavailable);
                    }
                    // This is the only mutation-specific advancement. It cannot
                    // run from a sweep, after Err, or for a second own rename.
                    self.original[slot as usize] = Some(after);
                    Ok(())
                },
            )?;
        }
        Ok(())
    }

    fn commit_pair(
        &mut self,
        members: [&[u8]; 4],
        intent: &[u8],
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        let result = (|| {
            if !held.commits() || self.live != [LiveRole::Old; 2] {
                return Err(Unavailable);
            }
            if !matches!(
                self.commit_sources(members, intent, until)?,
                LivePairClass::Old | LivePairClass::Identical
            ) {
                return Err(Unavailable);
            }
            pair_steps(|operation| {
                let index = match operation {
                    CommitStep::Write(index) | CommitStep::Rename(index) => index,
                };
                if matches!(operation, CommitStep::Write(_)) {
                    if self.live[index] != LiveRole::Old {
                        return Err(Unavailable);
                    }
                    let (slot, name) = REPLACEMENTS[index];
                    self.write_member(Slot::Config, slot, name, members[index + 2], held, until)?;
                    self.live[index] = LiveRole::ReplacementReady;
                    self.commit_sources(members, intent, until)?;
                    return Ok(());
                }
                let result = held.fence(until);
                self.cut(StageCut::Origin, result, until)?;
                self.commit_sources(members, intent, until)?;
                if self.live[index] != LiveRole::ReplacementReady {
                    return Err(Unavailable);
                }
                self.io.perform(
                    Slot::Config,
                    || tick(until),
                    |directory| {
                        renameat(directory, REPLACEMENTS[index].1, directory, LIVE[index].1)
                            .map_err(|_| Unavailable)?;
                        self.live[index] = LiveRole::Renamed; // BEFORE post-tick
                        Ok(())
                    },
                )?;
                self.advance_own_rename(index, until)?;
                self.io.perform(
                    REPLACEMENTS[index].0,
                    || tick(until),
                    |file| file.sync_all().map_err(|_| Unavailable),
                )?;
                self.io.perform(
                    Slot::Config,
                    || tick(until),
                    |file| file.sync_all().map_err(|_| Unavailable),
                )?;
                self.commit_sources(members, intent, until)?;
                let result = held.fence(until);
                self.cut(StageCut::Origin, result, until)?;
                Ok(())
            })?;
            if self.live != [LiveRole::Renamed; 2]
                || !matches!(
                    self.commit_sources(members, intent, until)?,
                    LivePairClass::New | LivePairClass::Identical
                )
            {
                return Err(Unavailable);
            }
            Ok(())
        })();
        self.cut(StageCut::MemberWrite, result, until)
    }

    fn catalogue(
        &mut self,
        slot: Slot,
        expected: &[&str],
        until: Instant,
    ) -> Result<(), Unavailable> {
        let result = self.catalogue_inner(slot, expected, until);
        self.cut(StageCut::Catalogue, result, until)
    }

    fn catalogue_inner(
        &mut self,
        slot: Slot,
        expected: &[&str],
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.io.perform(
            slot,
            || tick(until),
            |file| {
                if seek(file, SeekFrom::Start(0)).map_err(|_| Unavailable)? != 0 {
                    return Err(Unavailable);
                }
                Ok(())
            },
        )?;
        self.io.perform(
            slot,
            || tick(until),
            |file| {
                let mut iterator = RawDir::new(file, &mut self.directory_buffer);
                let mut catalogue = Catalogue::new(expected)?;
                for _ in 0..expected.len() + 3 {
                    tick(until)?;
                    let entry = iterator.next();
                    tick(until)?;
                    let Some(entry) = entry else {
                        return catalogue.finish();
                    };
                    let entry = entry.map_err(|_| Unavailable)?;
                    let name = entry.file_name().to_bytes();
                    catalogue.accept(name)?;
                }
                Err(Unavailable)
            },
        )
    }

    pub fn record(
        &mut self,
        members: [&[u8]; 4],
        nonce: &[u8; 32],
        held: &mut Retained,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.record_owned(members, nonce, &mut StageOwner::Classic(held), until)
    }

    pub fn record_canonical(
        &mut self,
        members: [&[u8]; 4],
        nonce: &[u8; 32],
        held: &mut Canonical,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.record_owned(members, nonce, &mut StageOwner::Canonical(held), until)
    }

    pub fn commit_canonical(
        &mut self,
        members: [&[u8]; 4],
        nonce: &[u8; 32],
        held: &mut Canonical,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.record_owned(
            members,
            nonce,
            &mut StageOwner::CanonicalCommit(held),
            until,
        )
    }

    fn record_owned(
        &mut self,
        members: [&[u8]; 4],
        nonce: &[u8; 32],
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if self.consumed || held.canonical() != self.diagnostic.enabled {
            self.revoke();
            held.revoke();
            return Err(Unavailable);
        }
        self.consumed = true; // before admission, manager checks, or any effect
        let result = self.record_inner(members, nonce, held, until);
        if result.is_err() {
            self.io.revoke();
            held.revoke();
        } else {
            self.completed = true;
        }
        // Both original owners are sealed before this one diagnostic attempt.
        self.diagnostic
            .result(result, |label| emit_actor(label, until))
    }

    fn record_inner(
        &mut self,
        members: [&[u8]; 4],
        nonce: &[u8; 32],
        held: &mut StageOwner<'_>,
        until: Instant,
    ) -> Result<(), Unavailable> {
        tick(until)?;
        let planned = planned_stage_identity(members).map_err(|_| Unavailable)?;
        let ready = ready_bytes(members);
        let transaction_id: [u8; 16] = Sha256::digest(nonce)[..16]
            .try_into()
            .map_err(|_| Unavailable)?;
        // These are developer-fixture bindings, never a product owner/lease.
        let intent = DecisionRecord::intent(1, None, &planned, transaction_id)
            .map_err(|_| Unavailable)?
            .encode();
        let terminal = DecisionRecord::decode(&intent)
            .map_err(|_| Unavailable)?
            .terminal(if held.commits() {
                TerminalChoice::Commit
            } else {
                TerminalChoice::Abort
            })
            .map_err(|_| Unavailable)?
            .encode();
        tick(until)?;
        let result = held.admit(&mut self.io, until);
        self.cut(StageCut::Admission, result, until)?;
        self.io.root(|| tick(until), |_| Ok(()))?;
        self.capture_shape(Slot::Root, true, None, until)?;
        self.directory(Slot::Root, Slot::Run, "run", false, held, until)?;
        self.directory(
            Slot::Run,
            Slot::Epoch,
            "omavless-t4-actor-development",
            false,
            held,
            until,
        )?;
        self.catalogue(Slot::Epoch, &EPOCH_MEMBERS, until)?;
        emit_actor(SUCCESS_PHASES[1], until)?;
        self.directory(
            Slot::Epoch,
            Slot::Transaction,
            TRANSACTION,
            true,
            held,
            until,
        )?;
        self.directory(Slot::Transaction, Slot::Config, "config", true, held, until)?;
        self.directory(Slot::Transaction, Slot::State, "state", true, held, until)?;
        for (index, (slot, name)) in LIVE.into_iter().enumerate() {
            self.write_member(Slot::Config, slot, name, members[index], held, until)?;
        }
        self.directory(
            Slot::State,
            Slot::StageDirectory,
            PENDING_DIRECTORY,
            true,
            held,
            until,
        )?;
        for (index, name) in MEMBERS.into_iter().enumerate() {
            self.write_member(
                Slot::StageDirectory,
                STAGED[index],
                name,
                members[index],
                held,
                until,
            )?;
        }
        self.write_member(
            Slot::StageDirectory,
            Slot::StageReady,
            READY_MEMBER,
            &ready,
            held,
            until,
        )?;
        self.catalogue(
            Slot::StageDirectory,
            &[MEMBERS[0], MEMBERS[1], MEMBERS[2], MEMBERS[3], READY_MEMBER],
            until,
        )?;
        emit_actor(SUCCESS_PHASES[2], until)?;
        self.write_member(Slot::State, Slot::Intent, INTENT, &intent, held, until)?;
        emit_actor(SUCCESS_PHASES[3], until)?;
        if held.commits() {
            self.commit_pair(members, &intent, held, until)?;
        }
        self.write_member(
            Slot::State,
            Slot::Terminal,
            TERMINAL,
            &terminal,
            held,
            until,
        )?;
        emit_actor(SUCCESS_PHASES[4], until)?;
        // Full positive reinspection. No terminal marker/prefix supplies success.
        for (index, name) in MEMBERS.into_iter().enumerate() {
            self.verify_member(
                Slot::StageDirectory,
                STAGED[index],
                name,
                members[index],
                until,
            )?;
        }
        self.verify_member(
            Slot::StageDirectory,
            Slot::StageReady,
            READY_MEMBER,
            &ready,
            until,
        )?;
        for (index, (slot, name)) in LIVE.into_iter().enumerate() {
            let (slot, expected) = if held.commits() {
                if self.live[index] != LiveRole::Renamed {
                    return Err(Unavailable);
                }
                (REPLACEMENTS[index].0, members[index + 2])
            } else {
                (slot, members[index])
            };
            self.verify_member(Slot::Config, slot, name, expected, until)?;
            if held.commits() {
                self.verify_unlinked_old(LIVE[index].0, members[index], until)?;
            }
        }
        self.verify_member(Slot::State, Slot::Intent, INTENT, &intent, until)?;
        self.verify_member(Slot::State, Slot::Terminal, TERMINAL, &terminal, until)?;
        let chain = DecisionChain::decode(&intent, Some(&terminal)).map_err(|_| Unavailable)?;
        if !chain.active().matches_current_bindings(1, None, &planned)
            || chain.active().phase()
                != if held.commits() {
                    DecisionPhase::Committed
                } else {
                    DecisionPhase::Aborted
                }
        {
            return Err(Unavailable);
        }
        self.catalogue(
            Slot::StageDirectory,
            &[MEMBERS[0], MEMBERS[1], MEMBERS[2], MEMBERS[3], READY_MEMBER],
            until,
        )?;
        self.catalogue(Slot::Config, &[LIVE[0].1, LIVE[1].1], until)?;
        self.catalogue(Slot::State, &[PENDING_DIRECTORY, INTENT, TERMINAL], until)?;
        self.catalogue(Slot::Transaction, &["config", "state"], until)?;
        self.catalogue(
            Slot::Epoch,
            &[
                EPOCH_MEMBERS[0],
                EPOCH_MEMBERS[1],
                EPOCH_MEMBERS[2],
                EPOCH_MEMBERS[3],
                EPOCH_MEMBERS[4],
                TRANSACTION,
            ],
            until,
        )?;
        for (parent, slot, name) in [
            (Slot::Root, Slot::Run, "run"),
            (Slot::Run, Slot::Epoch, "omavless-t4-actor-development"),
            (Slot::Epoch, Slot::Transaction, TRANSACTION),
            (Slot::Transaction, Slot::Config, "config"),
            (Slot::Transaction, Slot::State, "state"),
            (Slot::State, Slot::StageDirectory, PENDING_DIRECTORY),
        ] {
            self.binding(parent, slot, name, true, until)?;
        }
        let result = held.final_fence(until);
        self.cut(StageCut::FinalOwner, result, until)
    }

    pub fn finish(&mut self) -> Result<(), Unavailable> {
        if self.consumed && !self.completed {
            self.io.revoke();
            return Err(Unavailable);
        }
        self.io.finish()
    }
}

#[cfg(test)]
pub(super) fn test_failure_projection(
    original: Result<(), Unavailable>,
    frames: &mut Vec<&'static [u8]>,
) -> Result<(), Unavailable> {
    let mut diagnostic = Diagnostic {
        enabled: true,
        attempted: false,
        first: Some(StageCut::Admission),
    };
    // Called after the outer operation has revoked both owners; no native IO.
    let result = diagnostic.result(original, |label| {
        frames.push(label);
        Err(Unavailable)
    });
    assert!(
        diagnostic
            .result::<()>(Err(Unavailable), |_| panic!("secondary stage error"))
            .is_err()
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUTS: [StageCut; 8] = [
        StageCut::Admission,
        StageCut::Origin,
        StageCut::Directory,
        StageCut::MemberWrite,
        StageCut::MemberVerify,
        StageCut::Catalogue,
        StageCut::Journal,
        StageCut::FinalOwner,
    ];

    #[test]
    fn commit_closed_trace_and_slot_plan_preserve_capture_and_fd_caps() {
        let bytes =
            crate::restore_abort_cli::stopped_owner::actor_canonical::auth_success_trace_bytes()
                + SUCCESS_PHASES[..5]
                    .iter()
                    .map(|label| label.len())
                    .sum::<usize>()
                + COMMITTED_PHASE.len()
                + crate::restore_abort_cli::stopped_owner::actor_canonical::STAGE_OWNER_PHASES
                    .iter()
                    .map(|label| label.len())
                    .sum::<usize>();
        let (categories, longest_inventory) =
            crate::restore_abort_cli::stopped_owner::actor_canonical::inventory_trace_limits();
        let longest_stage = CUTS.iter().map(|cut| cut.label().len()).max().unwrap();
        assert_eq!(bytes, 4001);
        assert_eq!(bytes + longest_inventory + longest_stage, 4083);
        assert!(bytes + longest_inventory + longest_stage <= 4096);
        assert_eq!(106 + 6 + 2 + 2, 116);
        assert_eq!(17 + categories + 3 + 6 + 2 + CUTS.len(), 68);
        let mut transaction_labels: std::collections::BTreeSet<_> =
            SUCCESS_PHASES[..5].iter().copied().collect();
        assert!(transaction_labels.insert(COMMITTED_PHASE));
        assert_eq!(transaction_labels.len(), 6);
        assert!(transaction_labels.insert(SUCCESS_PHASES[5]));
        assert_eq!(transaction_labels.len(), 7); // feature union is 69, each packet 68
        assert_eq!(COMMIT_ORIGIN_FENCES, 34);
        assert_eq!(ORIGIN_FENCES, 26);
        assert_eq!(REPLACEMENTS.len(), 2);
        assert_ne!(REPLACEMENTS[0].0 as usize, LIVE[0].0 as usize);
        assert_ne!(REPLACEMENTS[1].0 as usize, LIVE[1].0 as usize);
        assert_eq!(IO_SLOTS, 36);
        assert_eq!(8192 + 41 + IO_SLOTS + 8, 8277);
    }

    #[test]
    fn each_commit_operation_refusal_stops_before_any_later_step_or_terminal() {
        let expected = [
            CommitStep::Write(0),
            CommitStep::Write(1),
            CommitStep::Rename(0),
            CommitStep::Rename(1),
        ];
        for cut in 0..4 {
            let mut visited = Vec::new();
            let result = pair_steps(|step| {
                visited.push(step);
                if visited.len() == cut + 1 {
                    Err(Unavailable)
                } else {
                    Ok(())
                }
            });
            let mut terminal = false;
            if result.is_ok() {
                terminal = true;
            }
            assert!(result.is_err());
            assert!(!terminal);
            assert_eq!(visited, expected[..=cut]);
        }
        let mut visited = Vec::new();
        pair_steps(|step| {
            visited.push(step);
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, expected);
    }

    #[test]
    fn only_own_rename_ctime_and_exact_link_transition_can_advance_originals() {
        let before = RenameShape {
            identity: (1, 2, 0, 0, 0o100600),
            content: (5, 6, 7),
            ctime: (8, 9),
            links: 1,
        };
        for links in [0, 1] {
            let after = RenameShape {
                links,
                ctime: (8, 10),
                ..before
            };
            assert!(before.own_successor(after, links));
            let mut bad = after;
            bad.identity.0 += 1;
            assert!(!before.own_successor(bad, links));
            bad = after;
            bad.identity.1 += 1;
            assert!(!before.own_successor(bad, links));
            bad = after;
            bad.identity.2 = 1000;
            assert!(!before.own_successor(bad, links));
            bad = after;
            bad.identity.3 = 1000;
            assert!(!before.own_successor(bad, links));
            bad = after;
            bad.identity.4 = 0o100644;
            assert!(!before.own_successor(bad, links));
            for field in 0..3 {
                bad = after;
                match field {
                    0 => bad.content.0 += 1,
                    1 => bad.content.1 += 1,
                    _ => bad.content.2 += 1,
                }
                assert!(!before.own_successor(bad, links));
            }
            bad = after;
            bad.ctime = (7, 999);
            assert!(!before.own_successor(bad, links));
            bad = after;
            bad.links = 2;
            assert!(!before.own_successor(bad, links));
        }
        assert!(!RenameShape { links: 0, ..before }.own_successor(before, 1));
    }

    #[test]
    fn commit_missing_or_expired_canonical_owner_never_opens_a_replacement() {
        let mut stage = Stage::reserve_canonical();
        let mut owner = Canonical::reserve().unwrap();
        assert!(
            stage
                .commit_canonical(
                    [b"old", b"old-template", b"new", b"new-template"],
                    &[1; 32],
                    &mut owner,
                    Instant::now() - std::time::Duration::from_secs(1)
                )
                .is_err()
        );
        assert!(stage.original.iter().all(Option::is_none));
        assert!(stage.finish().is_err());
        assert!(stage.permit_request(Kind::Halt).is_err());
    }

    #[test]
    fn source_shaped_stage_output_and_descriptor_plan_stay_inside_fixed_caps() {
        let success_bytes =
            crate::restore_abort_cli::stopped_owner::actor_canonical::auth_success_trace_bytes()
                + SUCCESS_PHASES
                    .iter()
                    .map(|label| label.len())
                    .sum::<usize>()
                + crate::restore_abort_cli::stopped_owner::actor_canonical::STAGE_OWNER_PHASES
                    .iter()
                    .map(|label| label.len())
                    .sum::<usize>();
        assert_eq!(success_bytes, 4001);
        let (inventory_categories, longest_inventory) =
            crate::restore_abort_cli::stopped_owner::actor_canonical::inventory_trace_limits();
        assert_eq!(inventory_categories, 32);
        assert_eq!(longest_inventory, 45);
        let longest_stage = CUTS.iter().map(|cut| cut.label().len()).max().unwrap();
        assert_eq!(longest_stage, 37);
        assert_eq!(success_bytes + longest_inventory + longest_stage, 4083);
        assert!(success_bytes + longest_inventory + longest_stage <= 4096);
        assert_eq!(106 + SUCCESS_PHASES.len() + 2 + 2, 116);
        assert_eq!(
            17 + inventory_categories + 3 + SUCCESS_PHASES.len() + 2 + CUTS.len(),
            68
        );
        assert_eq!(1 + ORIGIN_FENCES + 1, 28);
        assert_eq!(CREATED_DIRECTORIES.len(), 4);
        assert_eq!(LIVE.len() + MEMBERS.len() + 1 + 2, 9);
        assert_eq!(8192 + 41 + IO_SLOTS + 8, 8277);
        let labels: std::collections::BTreeSet<_> = CUTS.iter().map(|cut| cut.label()).collect();
        assert_eq!(labels.len(), CUTS.len());
    }

    #[test]
    fn first_stage_failure_preserves_result_after_sealing_even_if_output_fails() {
        for cut in CUTS {
            let mut diagnostic = Diagnostic {
                enabled: true,
                first: Some(cut),
                attempted: false,
            };
            let mut frames = Vec::new();
            let result: Result<(), Unavailable> = Err(Unavailable);
            assert!(
                diagnostic
                    .result(result, |label| {
                        frames.push(label);
                        Err(Unavailable)
                    })
                    .is_err()
            );
            assert!(diagnostic.attempted);
            diagnostic.first = Some(StageCut::FinalOwner); // nested category cannot emit again
            assert!(
                diagnostic
                    .result::<()>(Err(Unavailable), |_| panic!("second error label"))
                    .is_err()
            );
            assert_eq!(frames, vec![cut.label()]);
        }
        let mut stage = Stage::reserve_canonical();
        assert!(
            stage
                .cut::<()>(StageCut::Origin, Err(Unavailable), Instant::now())
                .is_err()
        );
        assert!(
            stage
                .cut::<()>(StageCut::Directory, Err(Unavailable), Instant::now())
                .is_err()
        );
        assert!(matches!(stage.diagnostic.first, Some(StageCut::Origin)));
        assert!(stage.finish().is_err());
    }

    #[test]
    fn canonical_expiry_and_mode_mismatch_do_not_open_or_enable_later_halt() {
        let mut stage = Stage::reserve_canonical();
        let mut owner = Canonical::reserve().unwrap();
        let expired = Instant::now() - std::time::Duration::from_secs(1);
        assert!(
            stage
                .record_canonical(
                    [b"old", b"template", b"new", b"template"],
                    &[1; 32],
                    &mut owner,
                    expired
                )
                .is_err()
        );
        assert!(stage.original.iter().all(Option::is_none));
        assert!(stage.finish().is_err());
        assert!(owner.finish().is_err());
        assert!(owner.observe(expired).is_err());
        let mut stage = Stage::reserve().unwrap();
        let mut owner = Canonical::reserve().unwrap();
        assert!(
            stage
                .record_canonical(
                    [b"old", b"template", b"new", b"template"],
                    &[1; 32],
                    &mut owner,
                    expired
                )
                .is_err()
        );
        assert!(stage.finish().is_err());
        assert!(owner.finish().is_err());
        assert!(stage.original.iter().all(Option::is_none));
    }

    #[test]
    fn consumed_stage_allows_only_halt_and_refusal_seals_before_next_io() {
        for kind in [
            Kind::ObserveManager,
            Kind::AuthenticateBackup,
            Kind::StageAuthenticatedBackup,
            Kind::Challenge,
            Kind::Ready,
            Kind::Completed,
            Kind::Closed,
            Kind::Rejected,
            Kind::BackupAuthenticated,
            Kind::StageRecorded,
            Kind::ObserveStopped,
            Kind::StoppedObserved,
        ] {
            let mut stage = Stage::reserve().unwrap();
            // Memory-only completed-mode fixture, no Files or real stage run.
            stage.consumed = true;
            stage.completed = true;
            stage.permit_request(Kind::Halt).unwrap();
            assert!(stage.permit_request(kind).is_err());
            assert!(stage.original.iter().all(Option::is_none));
            assert!(stage.permit_request(Kind::Halt).is_err());
            assert!(stage.finish().is_err());
            assert!(
                stage
                    .io
                    .root(
                        || panic!("refused request reached deadline/IO"),
                        |_| panic!("refused request reached shape")
                    )
                    .is_err()
            );
        }
        let mut stage = Stage::reserve().unwrap();
        for kind in [
            Kind::ObserveManager,
            Kind::AuthenticateBackup,
            Kind::StageAuthenticatedBackup,
            Kind::Halt,
        ] {
            stage.permit_request(kind).unwrap();
        }
        stage.consumed = true;
        assert!(stage.permit_request(Kind::Halt).is_err());
        assert!(stage.finish().is_err());
    }

    #[test]
    fn real_catalogue_classifier_requires_all_dots_and_exact_names() {
        let expected = ["old", "new", "ready"];
        for shift in 0..5 {
            let names = [".", "..", "old", "new", "ready"];
            let mut catalogue = Catalogue::new(&expected).unwrap();
            for index in 0..5 {
                catalogue
                    .accept(names[(index + shift) % 5].as_bytes())
                    .unwrap();
            }
            catalogue.finish().unwrap();
            assert!(catalogue.accept(b"foreign").is_err());
            assert!(catalogue.accept(b"old").is_err());
        }
        for missing in 0..5 {
            let mut catalogue = Catalogue::new(&expected).unwrap();
            for (index, name) in [".", "..", "old", "new", "ready"].into_iter().enumerate() {
                if index != missing {
                    catalogue.accept(name.as_bytes()).unwrap();
                }
            }
            assert!(catalogue.finish().is_err());
        }
        for invalid in [
            &["."][..],
            &["", "ready"][..],
            &["ready", "ready"][..],
            &["1", "2", "3", "4", "5", "6", "7"][..],
        ] {
            assert!(Catalogue::new(invalid).is_err());
        }
        Catalogue::new(&["1", "2", "3", "4", "5", "6"]).unwrap();
    }

    #[test]
    fn actual_empty_owner_refuses_before_any_directory_and_consumes_stage() {
        let mut stage = Stage::reserve().unwrap();
        let mut held = Retained::new().unwrap();
        let until = Instant::now() + std::time::Duration::from_secs(1);
        assert!(
            stage
                .record(
                    [b"old", b"old template", b"new", b"new template"],
                    &[1; 32],
                    &mut held,
                    until
                )
                .is_err()
        );
        assert!(stage.consumed && !stage.completed);
        assert!(stage.original.iter().all(Option::is_none));
        assert!(
            stage
                .record(
                    [b"old", b"old template", b"new", b"new template"],
                    &[1; 32],
                    &mut held,
                    until
                )
                .is_err()
        );
        assert!(stage.finish().is_err());
    }

    #[test]
    fn early_invalid_plan_or_expiry_cannot_enable_halt_or_retry() {
        for expired in [false, true] {
            let mut stage = Stage::reserve().unwrap();
            let mut held = Retained::new().unwrap();
            let until = if expired {
                Instant::now() - std::time::Duration::from_secs(1)
            } else {
                Instant::now() + std::time::Duration::from_secs(1)
            };
            assert!(
                stage
                    .record(
                        [b"", b"template", b"new", b"template"],
                        &[1; 32],
                        &mut held,
                        until
                    )
                    .is_err()
            );
            assert!(stage.original.iter().all(Option::is_none));
            assert!(stage.finish().is_err());
        }
        Stage::reserve().unwrap().finish().unwrap(); // unused non-transaction Halt
    }

    #[test]
    fn active_fixture_file_plan_is_37_without_iterator_duplicates() {
        // root/run/epoch/transaction/config/state/pending + 2oldlive+5stage+2records
        let actual_lower = 7 + 2 + 5 + 2;
        assert_eq!(actual_lower, 16);
        assert_eq!(4 + 17 + actual_lower, 37);
        assert_eq!(STAGED.len(), 5);
        assert_eq!(EPOCH_MEMBERS.len() + 1 + 2 + 1, 9); // max nexts including EOF
    }
}
