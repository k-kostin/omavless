// SPDX-License-Identifier: MIT
//! Nonescaping genuine-native-session pair engine. No actor or public issuer.
use super::*;
use crate::lifecycle::LifecycleHost;
use crate::native_coordinator::NativeSessionOrigin;
use crate::native_coordinator::{NativeFirstError as FirstError, PreparedRestorePair};
use std::fs::File;
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
}
impl NativeCatalogue {
    fn empty() -> Self {
        Self {
            names: [[0; 255]; CATALOGUE_LIMIT],
            lengths: [0; CATALOGUE_LIMIT],
            count: 0,
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
}
#[derive(Clone, Copy)]
pub(crate) struct NativeStageView<'a> {
    engine: &'a NativeEngine,
}
impl NativeStageView<'_> {
    pub(crate) fn stage_present(self) -> bool {
        self.engine.stage_name.is_some()
    }
    pub(crate) fn live_changed(self) -> bool {
        self.engine.lower.live.contains(&LiveRole::Renamed)
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
        ].iter().all(|name| matches!(std::fs::symlink_metadata(paths.directory.join(name)), Err(error) if error.kind() == std::io::ErrorKind::NotFound))
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
        }
    }
    fn view(&self) -> NativeStageView<'_> {
        NativeStageView { engine: self }
    }
    fn gate<H: LifecycleHost>(
        &mut self,
        origin: &mut NativeSessionOrigin<'_, H>,
        until: Instant,
    ) -> Result<(), FirstError> {
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
                                || (seen[..original.count].iter().all(|b| *b)
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
                        if seen[i] {
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
        for index in 0..2 {
            let (old, name) = LIVE[index];
            if self.lower.live[index] == LiveRole::Renamed {
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
                self.lower.binding(
                    Slot::Config,
                    NATIVE_REPLACEMENTS[index].0,
                    name,
                    false,
                    until,
                )?;
            } else {
                self.lower.binding(Slot::Config, old, name, false, until)?;
            }
        }
        let mut extra_config = [""; 2];
        let mut count = 0;
        for (index, (slot, name)) in NATIVE_REPLACEMENTS.into_iter().enumerate() {
            if self.lower.original[slot as usize].is_some()
                && self.lower.live[index] != LiveRole::Renamed
            {
                self.lower.binding(Slot::Config, slot, name, false, until)?;
                extra_config[count] = name;
                count += 1;
            }
        }
        self.scan_catalogue(Slot::Config, None, &extra_config[..count], until)?;
        let mut extra_state = [""; 3];
        let mut count = 0;
        if self.stage_name.is_some() {
            self.lower.binding(
                Slot::State,
                Slot::StageDirectory,
                PENDING_DIRECTORY,
                true,
                until,
            )?;
            extra_state[count] = PENDING_DIRECTORY;
            count += 1;
            let mut expected = [""; 5];
            let mut staged_count = 0;
            for (slot, name) in STAGED
                .into_iter()
                .zip(MEMBERS.into_iter().chain(std::iter::once(READY_MEMBER)))
            {
                if self.lower.original[slot as usize].is_some() {
                    self.lower
                        .binding(Slot::StageDirectory, slot, name, false, until)?;
                    expected[staged_count] = name;
                    staged_count += 1;
                }
            }
            self.lower
                .catalogue_inner(Slot::StageDirectory, &expected[..staged_count], until)?;
        }
        for (slot, name) in [(Slot::Intent, INTENT), (Slot::Terminal, TERMINAL)] {
            if self.lower.original[slot as usize].is_some() {
                self.lower.binding(Slot::State, slot, name, false, until)?;
                extra_state[count] = name;
                count += 1;
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
    fn write<H: LifecycleHost>(
        &mut self,
        parent: Slot,
        slot: Slot,
        name: &'static str,
        bytes: &[u8],
        origin: &mut NativeSessionOrigin<'_, H>,
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
