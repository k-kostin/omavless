// SPDX-License-Identifier: MIT
//! A real fixed developer stage/journal, NOT product Restore authority.
//! All lower reported Files remain in the original actor's pre-reserved ledger.
//! No existing unsafe top-level stager/journal helper, error cleanup or retry.

use super::retained_io::{ChildPlan, FileIo, IO_SLOTS, Slot};
use super::{Unavailable, emit_actor, tick};
use crate::restore_abort_cli::stopped_owner::actor_capture::Retained;
use crate::restore_decision_candidate::{DecisionChain, DecisionRecord, TerminalChoice};
use crate::restore_staging_candidate::{
    MEMBERS, PENDING_DIRECTORY, READY_MEMBER, planned_stage_identity, ready_bytes, same_directory,
    same_member,
};
use nix::fcntl::{AtFlags, OFlag};
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
const INTENT: &str = "restore-decision.intent";
const TERMINAL: &str = "restore-decision.terminal";
const EPOCH_MEMBERS: [&str; 5] = [
    "actor",
    "first.stdout",
    "first.stderr",
    "reserved",
    "channel",
];

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
}

impl Stage {
    pub fn reserve() -> Result<Self, Unavailable> {
        Ok(Self {
            io: FileIo::reserve()?,
            original: std::array::from_fn(|_| None),
            directory_buffer: [MaybeUninit::uninit(); 8192],
            consumed: false,
            completed: false,
        })
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
        held: &mut Retained,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if create {
            held.transaction_fence(until).map_err(|_| Unavailable)?;
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
            held.transaction_fence(until).map_err(|_| Unavailable)?;
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
        held: &mut Retained,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if bytes.is_empty() {
            return Err(Unavailable);
        }
        held.transaction_fence(until).map_err(|_| Unavailable)?;
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
        held.transaction_fence(until).map_err(|_| Unavailable)
    }

    fn verify_member(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        expected: &[u8],
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.binding(parent, slot, name, false, until)?;
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
        self.binding(parent, slot, name, false, until)
    }

    fn catalogue(
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
        if self.consumed {
            self.io.revoke();
            return Err(Unavailable);
        }
        self.consumed = true; // before admission, manager checks, or any effect
        let result = self.record_inner(members, nonce, held, until);
        if result.is_err() {
            self.io.revoke();
        } else {
            self.completed = true;
        }
        result
    }

    fn record_inner(
        &mut self,
        members: [&[u8]; 4],
        nonce: &[u8; 32],
        held: &mut Retained,
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
            .terminal(TerminalChoice::Abort)
            .map_err(|_| Unavailable)?
            .encode();
        tick(until)?;
        self.io.admit(held, until)?;
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
        emit_actor(b"t4_actor_before_fixture_transaction_directory\n", until)?;
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
        emit_actor(b"t4_actor_fixture_ready_inspected\n", until)?;
        self.write_member(Slot::State, Slot::Intent, INTENT, &intent, held, until)?;
        emit_actor(b"t4_actor_fixture_intent_written\n", until)?;
        self.write_member(
            Slot::State,
            Slot::Terminal,
            TERMINAL,
            &terminal,
            held,
            until,
        )?;
        emit_actor(b"t4_actor_fixture_terminal_written\n", until)?;
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
            self.verify_member(Slot::Config, slot, name, members[index], until)?;
        }
        self.verify_member(Slot::State, Slot::Intent, INTENT, &intent, until)?;
        self.verify_member(Slot::State, Slot::Terminal, TERMINAL, &terminal, until)?;
        let chain = DecisionChain::decode(&intent, Some(&terminal)).map_err(|_| Unavailable)?;
        if !chain.active().matches_current_bindings(1, None, &planned) {
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
        held.transaction_fence(until).map_err(|_| Unavailable)
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
mod tests {
    use super::*;

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
