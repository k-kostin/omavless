// SPDX-License-Identifier: MIT
//! Inactive current-manager/consumed-receipt proof. No receipt publication,
//! pending exception or normal-owner authority. Missing/old epochs refuse.
use super::{Error, Result, manager_epoch, package};
use crate::backup_source_candidate::open_private_directory;
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::login_transaction::validate_consumed_receipt_identity;
use crate::restore_cleanup_candidate::read_optional;
use crate::restore_staging_candidate::{same_directory, same_member};
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::Mode;
use std::fs::{File, Metadata};
use std::path::Path;
use zeroize::Zeroizing;

const MEMBER: &str = "omavless-login.receipt";
const LIMIT: usize = 1024;

enum Source {
    System,
    #[cfg(test)]
    Synthetic(Box<dyn FnMut(bool) -> Result<String>>),
}
impl Source {
    fn package(&mut self) -> Result<()> {
        match self {
            Self::System => package(),
            #[cfg(test)]
            Self::Synthetic(source) => source(false).map(|_| ()),
        }
    }
    fn epoch(&mut self) -> Result<String> {
        match self {
            Self::System => manager_epoch(),
            #[cfg(test)]
            Self::Synthetic(source) => source(true),
        }
    }
}

struct PinnedReceipt {
    directory: File,
    directory_metadata: Metadata,
    file: File,
    metadata: Metadata,
    bytes: Zeroizing<Vec<u8>>,
}
impl PinnedReceipt {
    fn read(paths: &CutoverPaths, uid: u32) -> Result<Self> {
        let directory =
            open_private_directory(&paths.runtime_base, uid).map_err(|_| Error::Recovery)?;
        let directory_metadata = directory.metadata().map_err(|_| Error::Recovery)?;
        let (bytes, metadata) = read_optional(&directory, MEMBER, uid, LIMIT)
            .map_err(|_| Error::Recovery)?
            .ok_or(Error::Recovery)?;
        let file = File::from(
            openat(
                &directory,
                Path::new(MEMBER),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Recovery)?,
        );
        let pinned = Self {
            directory,
            directory_metadata,
            file,
            metadata,
            bytes,
        };
        pinned.check(paths, uid)?;
        Ok(pinned)
    }
    fn check(&self, paths: &CutoverPaths, uid: u32) -> Result<()> {
        let directory =
            open_private_directory(&paths.runtime_base, uid).map_err(|_| Error::Recovery)?;
        for observed in [self.directory.metadata(), directory.metadata()] {
            if !same_directory(
                &self.directory_metadata,
                &observed.map_err(|_| Error::Recovery)?,
            ) {
                return Err(Error::Recovery);
            }
        }
        if !same_member(
            &self.metadata,
            &self.file.metadata().map_err(|_| Error::Recovery)?,
        ) {
            return Err(Error::Recovery);
        }
        let (bytes, metadata) = read_optional(&directory, MEMBER, uid, LIMIT)
            .map_err(|_| Error::Recovery)?
            .ok_or(Error::Recovery)?;
        if bytes != self.bytes || !same_member(&self.metadata, &metadata) {
            return Err(Error::Recovery);
        }
        Ok(())
    }
}

/// Original receipt descriptor and continuous migration lease outlive every
/// use. No Clone/Debug/serialization, caller epoch or production injected source.
pub(crate) struct CurrentEpochProof<'a> {
    paths: &'a CutoverPaths,
    lock: &'a MigrationLock,
    uid: u32,
    generation: u64,
    epoch: Zeroizing<String>,
    receipt: PinnedReceipt,
    source: Source,
}
#[allow(dead_code)]
impl<'a> CurrentEpochProof<'a> {
    pub(crate) fn capture(
        paths: &'a CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &'a MigrationLock,
    ) -> Result<Self> {
        // Shape-valid supplied paths alone are not product provenance.
        if uid != nix::unistd::Uid::current().as_raw()
            || CutoverPaths::current(uid).map_err(|_| Error::Recovery)? != *paths
        {
            return Err(Error::Recovery);
        }
        Self::capture_with(paths, uid, generation, lock, Source::System)
    }
    fn capture_with(
        paths: &'a CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &'a MigrationLock,
        mut source: Source,
    ) -> Result<Self> {
        if !lock.authorizes(paths, uid) {
            return Err(Error::Recovery);
        }
        source.package()?;
        let epoch = Zeroizing::new(source.epoch()?);
        if !super::epoch_valid(&epoch) {
            return Err(Error::Invocation);
        }
        let receipt = PinnedReceipt::read(paths, uid)?;
        validate_consumed_receipt_identity(&receipt.bytes, generation, &epoch)
            .map_err(|_| Error::Recovery)?;
        let mut proof = Self {
            paths,
            lock,
            uid,
            generation,
            epoch,
            receipt,
            source,
        };
        proof.recheck(paths, uid, generation, lock)?;
        Ok(proof)
    }
    pub(crate) fn recheck(
        &mut self,
        paths: &CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &MigrationLock,
    ) -> Result<()> {
        if paths != self.paths
            || uid != self.uid
            || generation != self.generation
            || !std::ptr::eq(lock, self.lock)
            || !lock.authorizes(paths, uid)
        {
            return Err(Error::Recovery);
        }
        let marker = read_marker_existing(paths, uid).map_err(|_| Error::Recovery)?;
        if marker.phase() != OwnershipPhase::Rust || marker.generation() != generation {
            return Err(Error::Recovery);
        }
        self.receipt.check(paths, uid)?;
        self.source.package()?;
        if self.source.epoch()? != *self.epoch {
            return Err(Error::Recovery);
        }
        self.receipt.check(paths, uid)?;
        if self.source.epoch()? != *self.epoch {
            return Err(Error::Recovery);
        }
        self.source.package()?;
        self.receipt.check(paths, uid)?;
        if !lock.authorizes(paths, uid) || read_marker_existing(paths, uid).ok() != Some(marker) {
            return Err(Error::Recovery);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn synthetic(
        paths: &'a CutoverPaths,
        uid: u32,
        generation: u64,
        lock: &'a MigrationLock,
        source: impl FnMut(bool) -> Result<String> + 'static,
    ) -> Result<Self> {
        Self::capture_with(
            paths,
            uid,
            generation,
            lock,
            Source::Synthetic(Box::new(source)),
        )
    }
}

#[cfg(test)]
#[path = "login_epoch_candidate_tests.rs"]
mod tests;
