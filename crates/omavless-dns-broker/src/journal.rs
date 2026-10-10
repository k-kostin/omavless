//! Root-private intent ordering, not proof of completed DNS or FD-store removal.
use rustix::fs::{self, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::File,
    io::{Read, Write},
    os::fd::OwnedFd,
    path::Path,
};

const DIRECTORY: &str = "/run/omavless-dns/private";
const RECORD: &str = "lease.json";
const STAGING: &str = ".lease.pending";
const MAX_BYTES: u64 = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Refused,
    InvalidState,
    RecoveryRequired,
    Unavailable,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Refused => "DNS journal ownership is unavailable.",
            Self::InvalidState => "DNS journal transition is invalid.",
            Self::RecoveryRequired => "DNS journal requires recovery before new work.",
            Self::Unavailable => "DNS journal persistence could not be verified.",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Applying,
    Active,
    Releasing,
    CleanupVerified,
    Quarantined,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u8,
    boot: String,
    phase: Phase,
    index: u32,
}

/// Contains no provider/profile/config values. Debug deliberately hides identity.
pub struct Journal {
    directory: OwnedFd,
    boot: String,
    owner: u32,
    record: Option<Record>,
    poisoned: bool,
}
impl fmt::Debug for Journal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Journal")
            .field("phase", &self.phase())
            .finish_non_exhaustive()
    }
}

impl Journal {
    /// Fixed root-owned directory only. This does not enroll/install/create it.
    /// Unit identity, verified FD-store state and lease admission are separate.
    pub fn open_root() -> Result<Self, Error> {
        if rustix::process::geteuid().as_raw() != 0 {
            return Err(Error::Refused);
        }
        for directory in ["/run", "/run/omavless-dns"] {
            let fd = open_directory(Path::new(directory), 0, false)?;
            drop(fd);
        }
        let file = File::open("/proc/sys/kernel/random/boot_id").map_err(|_| Error::Unavailable)?;
        let mut boot = String::new();
        file.take(38)
            .read_to_string(&mut boot)
            .map_err(|_| Error::Unavailable)?;
        if !boot.ends_with('\n') {
            return Err(Error::Refused);
        }
        boot.pop();
        Self::open_at(Path::new(DIRECTORY), boot, 0)
    }

    fn open_at(path: &Path, boot: String, owner: u32) -> Result<Self, Error> {
        if !valid_boot(&boot) {
            return Err(Error::Refused);
        }
        let directory = open_directory(path, owner, true)?;
        fs::flock(&directory, fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| Error::Refused)?;
        match fs::statat(&directory, STAGING, fs::AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => (),
            _ => return Err(Error::RecoveryRequired),
        }
        let mut journal = Self {
            directory,
            boot,
            owner,
            record: None,
            poisoned: false,
        };
        journal.record = journal.read()?;
        // A previous process's transaction is never resumed by this slice.
        // Reading apparently Active/CleanupVerified state is not authority to
        // proceed without retained-object and old-operation reconciliation.
        journal.poisoned = journal.record.is_some();
        Ok(journal)
    }

    pub fn phase(&self) -> Option<Phase> {
        self.record.as_ref().map(|r| r.phase)
    }

    pub fn requires_recovery(&self) -> bool {
        self.poisoned || self.phase() == Some(Phase::Quarantined)
    }

    /// Internal linkage only; never expose in ordinary public diagnostics.
    pub fn interface_index(&self) -> Option<u32> {
        self.record.as_ref().map(|r| r.index)
    }

    /// Must finish before any descriptor insertion/DNS dispatch. Missing journal
    /// alone is NOT empty FD-store proof: caller must independently establish it.
    pub fn begin(&mut self, index: u32) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::RecoveryRequired);
        }
        if self.record.is_some() || index == 0 || index > i32::MAX as u32 {
            return Err(Error::InvalidState);
        }
        self.persist(Record {
            schema: 1,
            boot: self.boot.clone(),
            phase: Phase::Applying,
            index,
        })
    }

    /// Call only after ALL writes and readback are settled for the held object.
    pub fn applied_verified(&mut self) -> Result<(), Error> {
        self.transition(&[Phase::Applying], Phase::Active)
    }

    /// Applying→Releasing is only for known-settled partial failure. Timeout or
    /// unknown completion instead requires quarantine; no method infers this.
    pub fn begin_release(&mut self) -> Result<(), Error> {
        self.transition(&[Phase::Applying, Phase::Active], Phase::Releasing)
    }

    /// Record only proven cleanup; it is NOT the cleanup operation itself.
    pub fn cleanup_verified(&mut self) -> Result<(), Error> {
        self.transition(&[Phase::Releasing], Phase::CleanupVerified)
    }

    pub fn quarantine(&mut self) -> Result<(), Error> {
        self.transition(
            &[
                Phase::Applying,
                Phase::Active,
                Phase::Releasing,
                Phase::CleanupVerified,
            ],
            Phase::Quarantined,
        )
    }

    /// Only after typed manager readback proves FD removal. This method does not
    /// remove descriptors and deliberately cannot clear an unknown transaction.
    pub fn finish_after_store_empty(&mut self) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::RecoveryRequired);
        }
        if self.phase() != Some(Phase::CleanupVerified) {
            return Err(Error::InvalidState);
        }
        if fs::unlinkat(&self.directory, RECORD, fs::AtFlags::empty()).is_err()
            || fs::fsync(&self.directory).is_err()
        {
            self.poisoned = true;
            return Err(Error::Unavailable);
        }
        self.record = None;
        Ok(())
    }

    fn transition(&mut self, allowed: &[Phase], phase: Phase) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::RecoveryRequired);
        }
        let mut record = self.record.clone().ok_or(Error::InvalidState)?;
        if !allowed.contains(&record.phase) {
            return Err(Error::InvalidState);
        }
        record.phase = phase;
        self.persist(record)
    }

    fn read(&self) -> Result<Option<Record>, Error> {
        let fd = match fs::openat(
            &self.directory,
            RECORD,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Error::RecoveryRequired),
        };
        let metadata = fs::fstat(&fd).map_err(|_| Error::Unavailable)?;
        if fs::FileType::from_raw_mode(metadata.st_mode) != fs::FileType::RegularFile
            || metadata.st_uid != self.owner
            || metadata.st_mode & 0o7777 != 0o600
            || metadata.st_nlink != 1
            || metadata.st_size < 1
            || metadata.st_size as u64 > MAX_BYTES
        {
            return Err(Error::RecoveryRequired);
        }
        let mut bytes = Vec::new();
        File::from(fd)
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Unavailable)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::RecoveryRequired);
        }
        let record: Record = serde_json::from_slice(&bytes).map_err(|_| Error::RecoveryRequired)?;
        if record.schema != 1
            || record.boot != self.boot
            || record.index == 0
            || record.index > i32::MAX as u32
        {
            return Err(Error::RecoveryRequired);
        }
        Ok(Some(record))
    }

    fn persist(&mut self, record: Record) -> Result<(), Error> {
        let result = (|| {
            let mut bytes = serde_json::to_vec(&record).map_err(|_| Error::Unavailable)?;
            bytes.push(b'\n');
            if bytes.len() as u64 > MAX_BYTES {
                return Err(Error::Unavailable);
            }
            let fd = fs::openat(
                &self.directory,
                STAGING,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| Error::Unavailable)?;
            let mut file = File::from(fd);
            file.write_all(&bytes).map_err(|_| Error::Unavailable)?;
            file.sync_all().map_err(|_| Error::Unavailable)?;
            fs::renameat(&self.directory, STAGING, &self.directory, RECORD)
                .map_err(|_| Error::Unavailable)?;
            fs::fsync(&self.directory).map_err(|_| Error::Unavailable)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result?;
        self.record = Some(record);
        Ok(())
    }
}

fn open_directory(path: &Path, owner: u32, private: bool) -> Result<OwnedFd, Error> {
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| Error::Refused)?;
    let metadata = fs::fstat(&fd).map_err(|_| Error::Refused)?;
    if metadata.st_uid != owner
        || metadata.st_mode & 0o022 != 0
        || (private && metadata.st_mode & 0o7777 != 0o700)
    {
        return Err(Error::Refused);
    }
    Ok(fd)
}

fn valid_boot(boot: &str) -> bool {
    boot.len() == 36
        && boot.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

#[cfg(test)]
mod tests;
