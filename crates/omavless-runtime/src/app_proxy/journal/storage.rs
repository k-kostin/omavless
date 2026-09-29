// SPDX-License-Identifier: MIT

use super::{Error, MAX_JOURNAL_BYTES};
use nix::errno::Errno;
use nix::fcntl::{AtFlags, Flock, FlockArg, OFlag, RenameFlags, open, openat, renameat2};
use nix::sys::stat::{Mode, fstatat};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

pub(super) const RECORD: &str = "app-proxy-journal.json";
pub(super) const STAGING: &str = ".app-proxy-journal.pending";
const LOCK: &str = ".app-proxy-journal.lock";

pub(super) struct Storage {
    path: PathBuf,
    directory: File,
    lock: Flock<File>,
    uid: u32,
    #[cfg(test)]
    pub(super) fail_at: std::cell::Cell<Option<Checkpoint>>,
    #[cfg(test)]
    pub(super) crash_at: std::cell::Cell<Option<Checkpoint>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Checkpoint {
    Created,
    Written,
    FileSynced,
    Renamed,
    DirectorySynced,
}

impl Storage {
    pub(super) fn acquire(path: &Path) -> Result<Self, Error> {
        let uid = nix::unistd::geteuid().as_raw();
        let directory = open_directory(path, uid)?;
        let lock = File::from(
            openat(
                &directory,
                LOCK,
                OFlag::O_RDWR
                    | OFlag::O_CREAT
                    | OFlag::O_NOFOLLOW
                    | OFlag::O_NONBLOCK
                    | OFlag::O_CLOEXEC,
                Mode::from_bits_truncate(0o600),
            )
            .map_err(|_| Error::UnsafePath)?,
        );
        private_file(&lock.metadata().map_err(|_| Error::UnsafePath)?, uid)?;
        let lock = Flock::lock(lock, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
            if error == Errno::EAGAIN {
                Error::Busy
            } else {
                Error::UnsafePath
            }
        })?;
        let storage = Self {
            path: path.to_path_buf(),
            directory,
            lock,
            uid,
            #[cfg(test)]
            fail_at: std::cell::Cell::new(None),
            #[cfg(test)]
            crash_at: std::cell::Cell::new(None),
        };
        storage.revalidate()?;
        storage.no_staging()?;
        Ok(storage)
    }

    fn revalidate(&self) -> Result<(), Error> {
        let current = open_directory(&self.path, self.uid)?;
        let expected = self.directory.metadata().map_err(|_| Error::UnsafePath)?;
        if !same(
            &expected,
            &current.metadata().map_err(|_| Error::UnsafePath)?,
        ) {
            return Err(Error::ForeignChange);
        }
        let lock = self.lock.metadata().map_err(|_| Error::UnsafePath)?;
        private_file(&lock, self.uid)?;
        self.same_named(LOCK, &lock)?;
        Ok(())
    }

    fn same_named(&self, name: &str, expected: &Metadata) -> Result<(), Error> {
        let current = fstatat(&self.directory, name, AtFlags::AT_SYMLINK_NOFOLLOW)
            .map_err(|_| Error::UnsafePath)?;
        if current.st_dev != expected.dev()
            || current.st_ino != expected.ino()
            || current.st_mode != expected.mode()
            || current.st_uid != self.uid
            || current.st_nlink != 1
        {
            return Err(Error::ForeignChange);
        }
        Ok(())
    }

    fn no_staging(&self) -> Result<(), Error> {
        match fstatat(&self.directory, STAGING, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Err(Errno::ENOENT) => Ok(()),
            Ok(_) => Err(Error::Interrupted),
            Err(_) => Err(Error::UnsafePath),
        }
    }

    pub(super) fn read(&self) -> Result<Option<Vec<u8>>, Error> {
        self.revalidate()?;
        let file = match openat(
            &self.directory,
            RECORD,
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(Errno::ENOENT) => return Ok(None),
            Err(_) => return Err(Error::UnsafePath),
        };
        let metadata = file.metadata().map_err(|_| Error::UnsafePath)?;
        private_file(&metadata, self.uid)?;
        if metadata.len() > MAX_JOURNAL_BYTES as u64 {
            return Err(Error::Invalid);
        }
        let mut bytes = Vec::new();
        (&file)
            .take((MAX_JOURNAL_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Invalid)?;
        let after = file.metadata().map_err(|_| Error::UnsafePath)?;
        if bytes.len() > MAX_JOURNAL_BYTES
            || metadata.len() != bytes.len() as u64
            || metadata.len() != after.len()
            || metadata.mtime_nsec() != after.mtime_nsec()
            || metadata.mtime() != after.mtime()
            || metadata.ctime() != after.ctime()
            || metadata.ctime_nsec() != after.ctime_nsec()
        {
            return Err(Error::ForeignChange);
        }
        self.same_named(RECORD, &metadata)?;
        Ok(Some(bytes))
    }

    pub(super) fn replace(&self, expected: Option<&[u8]>, payload: &[u8]) -> Result<(), Error> {
        if payload.len() > MAX_JOURNAL_BYTES {
            return Err(Error::Invalid);
        }
        self.revalidate()?;
        self.no_staging()?;
        if self.read()?.as_deref() != expected {
            return Err(Error::ForeignChange);
        }
        let mut staging = File::from(
            openat(
                &self.directory,
                STAGING,
                OFlag::O_WRONLY
                    | OFlag::O_CREAT
                    | OFlag::O_EXCL
                    | OFlag::O_NOFOLLOW
                    | OFlag::O_NONBLOCK
                    | OFlag::O_CLOEXEC,
                Mode::from_bits_truncate(0o600),
            )
            .map_err(|_| Error::Interrupted)?,
        );
        private_file(
            &staging.metadata().map_err(|_| Error::OutcomeUnknown)?,
            self.uid,
        )?;
        self.checkpoint(Checkpoint::Created)?;
        staging
            .write_all(payload)
            .map_err(|_| Error::OutcomeUnknown)?;
        self.checkpoint(Checkpoint::Written)?;
        staging.sync_all().map_err(|_| Error::OutcomeUnknown)?;
        self.checkpoint(Checkpoint::FileSynced)?;
        self.revalidate()?;
        if self.read()?.as_deref() != expected {
            return Err(Error::ForeignChange);
        }
        self.same_named(
            STAGING,
            &staging.metadata().map_err(|_| Error::OutcomeUnknown)?,
        )?;
        let flags = if expected.is_none() {
            RenameFlags::RENAME_NOREPLACE
        } else {
            RenameFlags::empty()
        };
        renameat2(&self.directory, STAGING, &self.directory, RECORD, flags)
            .map_err(|_| Error::OutcomeUnknown)?;
        self.checkpoint(Checkpoint::Renamed)?;
        self.directory
            .sync_all()
            .map_err(|_| Error::OutcomeUnknown)?;
        self.checkpoint(Checkpoint::DirectorySynced)?;
        self.revalidate()?;
        if self.read()?.as_deref() != Some(payload) {
            return Err(Error::ForeignChange);
        }
        Ok(())
    }

    fn checkpoint(&self, stage: Checkpoint) -> Result<(), Error> {
        #[cfg(test)]
        if self.crash_at.get() == Some(stage) {
            std::process::exit(71);
        }
        #[cfg(test)]
        if self.fail_at.get() == Some(stage) {
            return Err(Error::OutcomeUnknown);
        }
        let _ = stage;
        Ok(())
    }
}

fn private_file(metadata: &Metadata, uid: u32) -> Result<(), Error> {
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

fn open_directory(path: &Path, uid: u32) -> Result<File, Error> {
    if !path.is_absolute() {
        return Err(Error::UnsafePath);
    }
    let flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let mut directory =
        File::from(open(Path::new("/"), flags, Mode::empty()).map_err(|_| Error::UnsafePath)?);
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = File::from(
                    openat(&directory, Path::new(name), flags, Mode::empty())
                        .map_err(|_| Error::UnsafePath)?,
                );
            }
            _ => return Err(Error::UnsafePath),
        }
    }
    let metadata = directory.metadata().map_err(|_| Error::UnsafePath)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(Error::UnsafePath);
    }
    Ok(directory)
}
