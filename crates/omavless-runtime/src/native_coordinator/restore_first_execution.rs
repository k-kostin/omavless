// SPDX-License-Identifier: MIT
//! Private first-cycle owner composition. No dispatch, recovery or history grant.

use super::*;
use crate::backup_source_candidate::open_private_directory;
use crate::restore_executor_candidate::{PendingOutcome, execute_staged_pair};
use crate::restore_staging_candidate::{
    CreatedStage, same_directory, same_member, stage_owned_checked,
};
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

#[cfg(feature = "t4-manager-actor-service")]
#[path = "restore_retained_execution.rs"]
mod retained;
#[cfg(feature = "t4-manager-actor-service")]
pub(crate) use retained::HeldExecutionSlot;
#[cfg(feature = "t4-manager-actor-service")]
#[path = "restore_native_recovery.rs"]
mod native_recovery;
#[cfg(all(test, feature = "t4-manager-actor-service"))]
pub(crate) use native_recovery::FreshRecovery;
#[cfg(feature = "t4-manager-actor-service")]
pub(crate) use native_recovery::{
    NativeCompletedOff, NativeMutationLease, NativeRecoveryOrigin, NativeSteadyCompletion,
};

#[cfg(feature = "t4-manager-actor-service")]
pub(crate) struct NativeSessionOrigin<'a, H> {
    session: Session<'a, H>,
}
#[cfg(feature = "t4-manager-actor-service")]
impl<H: LifecycleHost> NativeSessionOrigin<'_, H> {
    pub(crate) fn uid(&self) -> u32 {
        self.session.owner.uid()
    }
    pub(crate) fn generation(&self) -> u64 {
        self.session.readiness.owner_generation
    }
    pub(crate) fn desired_bytes(&self) -> Result<zeroize::Zeroizing<Vec<u8>>, FirstError> {
        use std::os::unix::fs::FileExt;
        let pin = self
            .session
            .boundary
            .members
            .get(1)
            .and_then(|pin| pin.held.as_ref())
            .ok_or(FirstError::Admission)?;
        let length = usize::try_from(pin.1.len()).map_err(|_| FirstError::Admission)?;
        if length > 65536 {
            return Err(FirstError::Admission);
        }
        let mut bytes = zeroize::Zeroizing::new(vec![0; length + 1]);
        let mut done = 0;
        while done < bytes.len() {
            let n = pin
                .0
                .read_at(&mut bytes[done..], done as u64)
                .map_err(|_| FirstError::Admission)?;
            if n == 0 {
                break;
            }
            done += n;
        }
        if done != length {
            return Err(FirstError::Admission);
        }
        bytes.truncate(length);
        self.session.boundary.native_recheck(self.uid(), true)?;
        Ok(bytes)
    }
    pub(crate) fn directory(&self, index: usize) -> Result<&File, FirstError> {
        self.session
            .boundary
            .directories
            .get(index)
            .map(|(_, file, _)| file)
            .ok_or(FirstError::Admission)
    }
    pub(crate) fn member(&self, index: usize) -> Result<Option<&File>, FirstError> {
        Ok(self
            .session
            .boundary
            .members
            .get(index)
            .ok_or(FirstError::Admission)?
            .held
            .as_ref()
            .map(|(file, _)| file))
    }
    pub(crate) fn live(&self, index: usize) -> Result<&File, FirstError> {
        self.session
            .boundary
            .live
            .get(index)
            .and_then(|pin| pin.held.as_ref())
            .map(|(file, _)| file)
            .ok_or(FirstError::Admission)
    }
    pub(crate) fn check(
        &mut self,
        view: crate::manager_actor_service::NativeStageView<'_>,
    ) -> Result<(), FirstError> {
        if self
            .session
            .owner
            .batch
            .as_ref()
            .map(|batch| batch.instance.as_str())
            != self.session.instance.as_deref()
        {
            return Err(FirstError::Admission);
        }
        self.session
            .boundary
            .native_recheck(self.uid(), !view.live_changed())?;
        let readiness = self
            .session
            .owner
            .restore_readiness_with_native_stage(self.session.lock, view)
            .map_err(|_| FirstError::Admission)?;
        if readiness != self.session.readiness {
            return Err(FirstError::Admission);
        }
        self.session
            .boundary
            .native_recheck(self.uid(), !view.live_changed())?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum FirstOutcome {
    CommittedStillFenced,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FirstError {
    Prepare,
    Admission,
    /// Effects may have occurred; no automatic recovery or retry.
    StillFenced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Checkpoint {
    Staging,
    Execution,
}

fn absent(path: &Path) -> bool {
    matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

pub(super) fn pending_allowed(
    paths: &DesiredPaths,
    uid: u32,
    created: Option<&CreatedStage>,
) -> bool {
    let Some(created) = created else {
        return !crate::pending_private_transaction::pending(paths);
    };
    created.recheck(&paths.directory, uid).is_ok()
        && !crate::routing_preset::pending(paths)
        && [
            "restore-finalization.pending",
            crate::restore_closure_model::CLOSURE_MEMBER,
            crate::restore_closure_model::NEXT_CLOSURE_MEMBER,
            crate::restore_disposition_ticket_model::TICKET_MEMBER,
            crate::restore_disposition_complete_model::COMPLETE_MEMBER,
            crate::restore_successor_handoff_model::SUCCESSOR_MEMBER,
        ]
        .iter()
        .all(|name| absent(&paths.directory.join(name)))
}

struct PinnedMember {
    directory: usize,
    name: std::ffi::OsString,
    held: Option<(File, Metadata)>,
}

/// Original owner/desired/login and directory identities, never re-baselined.
struct Boundary {
    directories: Vec<(PathBuf, File, Metadata)>,
    members: Vec<PinnedMember>,
    live: Vec<PinnedMember>,
    #[cfg(feature = "t4-manager-actor-service")]
    capture_prefix: Vec<File>,
    #[cfg(feature = "t4-manager-actor-service")]
    probe: std::cell::RefCell<Option<File>>,
}

impl Boundary {
    #[cfg(feature = "t4-manager-actor-service")]
    fn reserve_installed() -> Result<Self, FirstError> {
        let mut boundary = Self {
            directories: Vec::new(),
            members: Vec::new(),
            live: Vec::new(),
            capture_prefix: Vec::new(),
            probe: std::cell::RefCell::new(None),
        };
        boundary
            .directories
            .try_reserve_exact(3)
            .map_err(|_| FirstError::Admission)?;
        boundary
            .members
            .try_reserve_exact(3)
            .map_err(|_| FirstError::Admission)?;
        boundary
            .live
            .try_reserve_exact(2)
            .map_err(|_| FirstError::Admission)?;
        boundary
            .capture_prefix
            .try_reserve_exact(1)
            .map_err(|_| FirstError::Admission)?;
        Ok(boundary)
    }
    fn member(&self, directory: usize, name: &std::ffi::OsStr) -> Result<PinnedMember, FirstError> {
        let held = match openat(
            &self.directories[directory].1,
            Path::new(name),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => {
                let file = File::from(fd);
                let meta = file.metadata().map_err(|_| FirstError::Admission)?;
                if !meta.is_file() || meta.mode() & 0o7777 != 0o600 || meta.nlink() != 1 {
                    return Err(FirstError::Admission);
                }
                Some((file, meta))
            }
            Err(nix::errno::Errno::ENOENT) => None,
            Err(_) => return Err(FirstError::Admission),
        };
        Ok(PinnedMember {
            directory,
            name: name.to_owned(),
            held,
        })
    }

    fn capture<H: LifecycleHost>(owner: &OfflineNativeCoordinator<H>) -> Result<Self, FirstError> {
        let paths = owner.transaction.cutover_paths();
        let store = owner.transaction.store_path();
        let config = store.parent().ok_or(FirstError::Admission)?;
        if store.file_name() != Some("profiles.json".as_ref())
            || owner.transaction.desired_paths().directory != paths.state_directory
            || owner.transaction.desired_paths().file != paths.state_directory.join("desired.json")
            || paths.ownership_marker != paths.state_directory.join("ownership.json")
        {
            return Err(FirstError::Admission);
        }
        let mut this = Self {
            directories: Vec::new(),
            members: Vec::new(),
            live: Vec::new(),
            #[cfg(feature = "t4-manager-actor-service")]
            capture_prefix: Vec::new(),
            #[cfg(feature = "t4-manager-actor-service")]
            probe: std::cell::RefCell::new(None),
        };
        for path in [config, &paths.state_directory, &paths.runtime_base] {
            let held =
                open_private_directory(path, owner.uid()).map_err(|_| FirstError::Admission)?;
            let meta = held.metadata().map_err(|_| FirstError::Admission)?;
            this.directories.push((path.to_owned(), held, meta));
        }
        for (dir, name) in [
            (1, "ownership.json"),
            (1, "desired.json"),
            (2, "omavless-login.receipt"),
        ] {
            this.members.push(this.member(dir, name.as_ref())?);
        }
        for name in ["profiles.json", "route-template.yaml"] {
            this.live.push(this.member(0, name.as_ref())?);
        }
        if this.members.iter().chain(&this.live).any(|pin| {
            pin.held
                .as_ref()
                .is_some_and(|(_, meta)| meta.uid() != owner.uid())
        }) {
            return Err(FirstError::Admission);
        }
        Ok(this)
    }

    #[cfg(feature = "t4-manager-actor-service")]
    fn capture_installed<H: LifecycleHost>(
        owner: &OfflineNativeCoordinator<H>,
        installed: &mut Option<Self>,
    ) -> Result<(), FirstError> {
        if owner.transaction.desired_paths().directory
            != owner.transaction.cutover_paths().state_directory
            || owner.transaction.desired_paths().file
                != owner
                    .transaction
                    .cutover_paths()
                    .state_directory
                    .join("desired.json")
        {
            return Err(FirstError::Admission);
        }
        Self::capture_native_paths(
            owner.transaction.cutover_paths(),
            owner.transaction.store_path(),
            owner.uid(),
            installed,
        )
    }

    #[cfg(feature = "t4-manager-actor-service")]
    fn capture_native_paths(
        paths: &crate::cutover::CutoverPaths,
        store: &Path,
        uid: u32,
        installed: &mut Option<Self>,
    ) -> Result<(), FirstError> {
        let config = store.parent().ok_or(FirstError::Admission)?;
        if store.file_name() != Some("profiles.json".as_ref())
            || paths.ownership_marker != paths.state_directory.join("ownership.json")
        {
            return Err(FirstError::Admission);
        }
        if installed.is_none() {
            *installed = Some(Self::reserve_installed()?);
        }
        let this = installed.as_mut().ok_or(FirstError::Admission)?;
        if !this.directories.is_empty()
            || !this.members.is_empty()
            || !this.live.is_empty()
            || !this.capture_prefix.is_empty()
            || this.probe.borrow().is_some()
            || this.directories.capacity() < 3
            || this.members.capacity() < 3
            || this.live.capacity() < 2
            || this.capture_prefix.capacity() < 1
        {
            return Err(FirstError::Admission);
        }
        for path in [config, &paths.state_directory, &paths.runtime_base] {
            let file = open_private_directory(path, uid).map_err(|_| FirstError::Admission)?;
            this.capture_prefix.push(file);
            let metadata = this
                .capture_prefix
                .last()
                .ok_or(FirstError::Admission)?
                .metadata()
                .map_err(|_| FirstError::Admission)?;
            this.directories.push((
                path.to_owned(),
                this.capture_prefix.pop().ok_or(FirstError::Admission)?,
                metadata,
            ));
        }
        for (directory, name, live) in [
            (1, "ownership.json", false),
            (1, "desired.json", false),
            (2, "omavless-login.receipt", false),
            (0, "profiles.json", true),
            (0, "route-template.yaml", true),
        ] {
            let owned_name: std::ffi::OsString = name.into(); // before this acquisition
            let held = match openat(
                &this.directories[directory].1,
                Path::new(name),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            ) {
                Ok(fd) => {
                    this.capture_prefix.push(File::from(fd));
                    let metadata = this
                        .capture_prefix
                        .last()
                        .ok_or(FirstError::Admission)?
                        .metadata()
                        .map_err(|_| FirstError::Admission)?;
                    if !metadata.is_file()
                        || metadata.uid() != uid
                        || metadata.mode() & 0o7777 != 0o600
                        || metadata.nlink() != 1
                    {
                        return Err(FirstError::Admission);
                    }
                    Some((
                        this.capture_prefix.pop().ok_or(FirstError::Admission)?,
                        metadata,
                    ))
                }
                Err(nix::errno::Errno::ENOENT) => None,
                Err(_) => return Err(FirstError::Admission),
            };
            let pin = PinnedMember {
                directory,
                name: owned_name,
                held,
            };
            if live {
                this.live.push(pin);
            } else {
                this.members.push(pin);
            }
        }
        Ok(())
    }

    #[cfg(feature = "t4-manager-actor-service")]
    fn native_recheck(&self, uid: u32, live: bool) -> Result<(), FirstError> {
        use nix::fcntl::AtFlags;
        use nix::sys::stat::fstatat;
        // At most one reported directory probe. Install before postchecks;
        // refuse reuse after uncertainty. Constructor-internal descriptors
        // remain under the unchanged trusted backend boundary.
        let mut probe = self.probe.borrow_mut();
        if probe.is_some() {
            return Err(FirstError::Admission);
        }
        for (path, held, before) in &self.directories {
            *probe = Some(open_private_directory(path, uid).map_err(|_| FirstError::Admission)?);
            let current = probe.as_ref().ok_or(FirstError::Admission)?;
            let held_metadata = held.metadata().map_err(|_| FirstError::Admission)?;
            let current_metadata = current.metadata().map_err(|_| FirstError::Admission)?;
            if !same_directory(before, &held_metadata)
                || !same_directory(before, &current_metadata)
                || before.gid() != held_metadata.gid()
                || before.gid() != current_metadata.gid()
            {
                return Err(FirstError::Admission);
            }
            *probe = None; // only the fully positive probe is released
        }
        for pin in self.members.iter().chain(self.live.iter().filter(|_| live)) {
            let named = fstatat(
                &self.directories[pin.directory].1,
                Path::new(&pin.name),
                AtFlags::AT_SYMLINK_NOFOLLOW,
            );
            match (&pin.held, named) {
                (None, Err(nix::errno::Errno::ENOENT)) => {}
                (Some((held, before)), Ok(named)) => {
                    let after = held.metadata().map_err(|_| FirstError::Admission)?;
                    let mut attributes = [0; 1];
                    if !same_member(before, &after)
                        || before.gid() != after.gid()
                        || rustix::fs::flistxattr(held, &mut attributes)
                            .map_err(|_| FirstError::Admission)?
                            != 0
                        || (
                            before.dev(),
                            before.ino(),
                            before.mode(),
                            before.uid(),
                            before.gid(),
                            before.nlink(),
                            before.len(),
                            before.mtime(),
                            before.mtime_nsec(),
                            before.ctime(),
                            before.ctime_nsec(),
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
                        return Err(FirstError::Admission);
                    }
                }
                _ => return Err(FirstError::Admission),
            }
        }
        Ok(())
    }

    fn recheck(&self, uid: u32, live: bool) -> Result<(), FirstError> {
        for (path, held, before) in &self.directories {
            let current = open_private_directory(path, uid).map_err(|_| FirstError::Admission)?;
            if !same_directory(before, &held.metadata().map_err(|_| FirstError::Admission)?)
                || !same_directory(
                    before,
                    &current.metadata().map_err(|_| FirstError::Admission)?,
                )
            {
                return Err(FirstError::Admission);
            }
        }
        for pin in self.members.iter().chain(self.live.iter().filter(|_| live)) {
            let current = self.member(pin.directory, &pin.name)?;
            match (&pin.held, &current.held) {
                (None, None) => {}
                (Some((held, before)), Some((_, after)))
                    if same_member(before, after)
                        && same_member(
                            before,
                            &held.metadata().map_err(|_| FirstError::Admission)?,
                        ) => {}
                _ => return Err(FirstError::Admission),
            }
        }
        Ok(())
    }
}

/// The exclusive mutable borrow is the exact in-process owner identity. No
/// session or borrowed authority escapes, including on failure.
struct Session<'a, H> {
    owner: &'a mut OfflineNativeCoordinator<H>,
    lock: &'a MigrationLock,
    boundary: &'a Boundary,
    readiness: RestoreReadiness,
    instance: Option<String>,
}

impl<H: LifecycleHost> Session<'_, H> {
    fn check(&mut self, stage: &CreatedStage, live: bool) -> Result<(), FirstError> {
        if self
            .owner
            .batch
            .as_ref()
            .map(|batch| batch.instance.as_str())
            != self.instance.as_deref()
        {
            return Err(FirstError::Admission);
        }
        self.boundary.recheck(self.owner.uid(), live)?;
        let current = self
            .owner
            .restore_readiness_with_created_stage(self.lock, Some(stage))
            .map_err(|_| FirstError::Admission)?;
        if current != self.readiness {
            return Err(FirstError::Admission);
        }
        self.boundary.recheck(self.owner.uid(), live)?;
        stage
            .recheck(
                &self.owner.transaction.cutover_paths().state_directory,
                self.owner.uid(),
            )
            .map_err(|_| FirstError::Admission)
    }
}

impl<H: LifecycleHost> OfflineNativeCoordinator<H> {
    /// Deliberately private and unregistered. Fresh operation only: an existing
    /// pending stage, journal or historical fence cannot enter this path.
    #[allow(dead_code)]
    fn execute_first_restore(
        &mut self,
        source: &Path,
        passphrase: &[u8],
    ) -> Result<FirstOutcome, FirstError> {
        self.execute_first_restore_body(source, passphrase, |_, _| true)
    }

    fn execute_first_restore_body(
        &mut self,
        source: &Path,
        passphrase: &[u8],
        mut checkpoint: impl FnMut(&mut Self, Checkpoint) -> bool,
    ) -> Result<FirstOutcome, FirstError> {
        if self.retained_restore_busy() {
            return Err(FirstError::StillFenced);
        }
        let incoming =
            open_existing(source, self.uid(), passphrase).map_err(|_| FirstError::Prepare)?;
        let lock = self
            .transaction
            .acquire_lock()
            .map_err(|_| FirstError::Admission)?;
        let instance = self.batch.as_ref().map(|batch| batch.instance.clone());
        let boundary = Boundary::capture(self)?;
        let prepared = self
            .prepare_restore_locked(incoming, &lock)
            .map_err(|_| FirstError::Admission)?;
        crate::login_transaction::check_startup_receipt(
            self.transaction.cutover_paths(),
            self.uid(),
            &lock,
            Some(prepared.readiness.owner_generation),
        )
        .map_err(|_| FirstError::Admission)?;
        boundary.recheck(self.uid(), true)?;
        if self.batch.as_ref().map(|batch| batch.instance.as_str()) != instance.as_deref() {
            return Err(FirstError::Admission);
        }
        let paths = self.transaction.cutover_paths().clone();
        let uid = self.uid();
        let config = self
            .transaction
            .store_path()
            .parent()
            .ok_or(FirstError::Admission)?
            .to_owned();
        if crate::restore_executor_candidate::NEW_SLOT
            .iter()
            .chain(crate::restore_executor_candidate::OLD_SLOT.iter())
            .any(|name| !absent(&config.join(name)))
        {
            return Err(FirstError::Admission);
        }
        let mut transaction = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut transaction))
            .map_err(|_| FirstError::Admission)?;
        if transaction == [0; 16] {
            return Err(FirstError::Admission);
        }
        self.invalidate_connection_close();
        let generation = prepared.readiness.owner_generation;
        let mut session = Session {
            owner: self,
            lock: &lock,
            boundary: &boundary,
            readiness: prepared.readiness,
            instance,
        };
        // The original authenticated object and old pair remain alive until
        // after the executor's final check. Never authenticate again mid-write.
        let result = (|| {
            let stage = stage_owned_checked(
                &paths.state_directory,
                uid,
                [
                    prepared.original_store(),
                    prepared.original_template(),
                    prepared.incoming_store(),
                    prepared.incoming_template(),
                ],
                |stage| {
                    checkpoint(session.owner, Checkpoint::Staging)
                        && absent(&paths.state_directory.join("restore-decision.intent"))
                        && absent(&paths.state_directory.join("restore-decision.terminal"))
                        && session.check(stage, true).is_ok()
                },
            )
            .map_err(|_| FirstError::StillFenced)?;
            session.check(&stage, true)?;
            // Hooks can only withdraw success in the lower executor. Test-only
            // fault injection is carried by the owner observation callback.
            let outcome =
                execute_staged_pair(&config, &paths, uid, generation, &lock, transaction, || {
                    checkpoint(session.owner, Checkpoint::Execution)
                        && session.check(&stage, false).is_ok()
                })
                .map_err(|_| FirstError::StillFenced)?;
            session.check(&stage, false)?;
            if outcome != PendingOutcome::Committed {
                return Err(FirstError::StillFenced);
            }
            Ok(FirstOutcome::CommittedStillFenced)
        })();
        // Even a verified terminal remains fenced. Never return a reusable
        // normally mutable owner after changing its private backing store.
        session.owner.transaction.block();
        session.owner.invalidate_connection_close();
        result.map_err(|_| FirstError::StillFenced)
    }
}

#[cfg(test)]
#[path = "restore_first_execution_tests.rs"]
mod tests;
