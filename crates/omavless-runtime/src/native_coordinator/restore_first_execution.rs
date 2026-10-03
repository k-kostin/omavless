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

#[derive(Debug, PartialEq, Eq)]
enum FirstOutcome {
    CommittedStillFenced,
}

#[derive(Debug, PartialEq, Eq)]
enum FirstError {
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
}

impl Boundary {
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
    boundary: Boundary,
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
            boundary,
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
