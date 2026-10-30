// SPDX-License-Identifier: MIT
//! Fixed-name, bounded private statistics checkpoint. Only the subordinate
//! statistics writer may call this module; none of this I/O belongs under the
//! general dispatcher or a lifecycle lease.
use super::{MAX_BYTES, State};
use omavless_store::{PrivateCreateOutcome, atomic_create_private, atomic_replace_private};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub const NAME: &str = "traffic-statistics.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unsafe,
    Unavailable,
    Invalid,
    Changed,
    CommitUnknown,
}

// No Debug/Display of private state, original bytes or machine paths.
#[derive(Clone)]
pub struct Snapshot {
    original: Option<Vec<u8>>,
    state: State,
}

impl Snapshot {
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn absent(&self) -> bool {
        self.original.is_none()
    }
}

pub struct Store {
    parent: PathBuf,
    uid: u32,
    identity: (u64, u64),
    #[cfg(test)]
    lose_reply_after_replace: bool,
}

pub(super) struct WriterLock {
    _file: nix::fcntl::Flock<std::fs::File>,
}

impl Store {
    /// Existing private directory inode is the same-runtime writer lock. No
    /// disabled-default file or lock path is created. A detached old writer
    /// retains this lock until its last possible write has settled.
    pub(super) fn lock_writer(&self) -> Result<WriterLock, Error> {
        self.validate_parent()?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
            .open(&self.parent)
            .map_err(|_| Error::Unsafe)?;
        self.validate_held_parent(&file)?;
        let file = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock)
            .map_err(|_| Error::Unavailable)?;
        self.validate_held_parent(&file)?;
        self.validate_parent()?;
        Ok(WriterLock { _file: file })
    }

    fn validate_held_parent(&self, file: &std::fs::File) -> Result<(), Error> {
        let m = file.metadata().map_err(|_| Error::Unsafe)?;
        if !m.is_dir()
            || m.uid() != self.uid
            || m.mode() & 0o077 != 0
            || (m.dev(), m.ino()) != self.identity
        {
            return Err(Error::Unsafe);
        }
        Ok(())
    }

    /// Application-owned configuration directory, never a client argument.
    /// Does not create either the directory or the default statistics file.
    pub fn new(parent: &Path, uid: u32) -> Result<Self, Error> {
        let metadata = fs::symlink_metadata(parent).map_err(|_| Error::Unsafe)?;
        let store = Self {
            parent: parent.to_owned(),
            uid,
            identity: (metadata.dev(), metadata.ino()),
            #[cfg(test)]
            lose_reply_after_replace: false,
        };
        store.validate_parent()?;
        Ok(store)
    }

    fn validate_parent(&self) -> Result<(), Error> {
        if !self.parent.is_absolute()
            || fs::canonicalize(&self.parent).map_err(|_| Error::Unsafe)? != self.parent
        {
            return Err(Error::Unsafe);
        }
        for ancestor in self.parent.ancestors() {
            let m = fs::symlink_metadata(ancestor).map_err(|_| Error::Unsafe)?;
            if !m.is_dir()
                || m.file_type().is_symlink()
                || ![0, self.uid].contains(&m.uid())
                || m.mode() & 0o022 != 0
            {
                return Err(Error::Unsafe);
            }
        }
        let m = fs::symlink_metadata(&self.parent).map_err(|_| Error::Unsafe)?;
        if m.uid() != self.uid || m.mode() & 0o077 != 0 || (m.dev(), m.ino()) != self.identity {
            return Err(Error::Unsafe);
        }
        Ok(())
    }

    pub fn read(&self) -> Result<Snapshot, Error> {
        self.validate_parent()?;
        let path = self.parent.join(NAME);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Snapshot {
                    original: None,
                    state: State::default(),
                });
            }
            Err(_) => return Err(Error::Unavailable),
            Ok(m) if !private_file(&m, self.uid) => return Err(Error::Unsafe),
            Ok(_) => {}
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| Error::Unavailable)?;
        let before = file.metadata().map_err(|_| Error::Unavailable)?;
        if !private_file(&before, self.uid) {
            return Err(Error::Unsafe);
        }
        if before.len() > MAX_BYTES as u64 {
            return Err(Error::Invalid);
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Unavailable)?;
        let after = file.metadata().map_err(|_| Error::Unavailable)?;
        let named = fs::symlink_metadata(&path).map_err(|_| Error::Changed)?;
        if !same(&before, &after) || !same(&after, &named) || !private_file(&named, self.uid) {
            return Err(Error::Changed);
        }
        self.validate_parent()?;
        let state = State::decode(&bytes).map_err(|_| Error::Invalid)?;
        Ok(Snapshot {
            original: Some(bytes),
            state,
        })
    }

    /// Expected bytes are an additional guard under the one statistics writer,
    /// not a claim of filesystem CAS against another independently authorized
    /// writer. A result after a possible rename is Unknown, never blind retry.
    pub fn replace(&self, expected: &Snapshot, candidate: &State) -> Result<Snapshot, Error> {
        if self.read()?.original != expected.original {
            return Err(Error::Changed);
        }
        if candidate == &expected.state {
            return Ok(expected.clone());
        }
        let bytes = candidate.encode().map_err(|_| Error::Invalid)?;
        let path = self.parent.join(NAME);
        if expected.absent() {
            match atomic_create_private(&path, &bytes, self.uid)
                .map_err(|_| Error::CommitUnknown)?
            {
                PrivateCreateOutcome::AlreadyExists => return Err(Error::Changed),
                PrivateCreateOutcome::Created => {}
            }
        } else {
            atomic_replace_private(&path, &bytes, self.uid).map_err(|_| Error::CommitUnknown)?;
        }
        // Inert fault control for the logical lost-result boundary after the
        // REAL full-state atomic replacement. Never compiled into delivery.
        #[cfg(test)]
        if self.lose_reply_after_replace {
            return Err(Error::CommitUnknown);
        }
        let observed = self.read().map_err(|_| Error::CommitUnknown)?;
        if observed.original.as_deref() != Some(bytes.as_slice()) {
            return Err(Error::CommitUnknown);
        }
        Ok(observed)
    }
}

fn private_file(m: &fs::Metadata, uid: u32) -> bool {
    m.is_file()
        && !m.file_type().is_symlink()
        && m.uid() == uid
        && m.mode() & 0o7777 == 0o600
        && m.nlink() == 1
}

fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::TrafficCounters;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn fixture() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir_in(crate::test_temp::home_parent().unwrap()).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let store = Store::new(dir.path(), nix::unistd::getuid().as_raw()).unwrap();
        (dir, store)
    }

    #[test]
    fn writable_ancestor_refuses_without_creating_or_repairing_statistics() {
        let (root, _) = fixture();
        let shared = root.path().join("shared");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o777)).unwrap();
        let private = tempfile::tempdir_in(&shared).unwrap();
        fs::set_permissions(private.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let uid = nix::unistd::getuid().as_raw();
        assert!(matches!(
            Store::new(private.path(), uid),
            Err(Error::Unsafe)
        ));
        assert!(!private.path().join(NAME).exists());
        assert_eq!(fs::metadata(&shared).unwrap().mode() & 0o777, 0o777);
        // Repair only this test-owned ancestor to prove the refusal control
        // distinguishes ancestry from the unchanged private leaf and payload.
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            Store::new(private.path(), uid)
                .unwrap()
                .read()
                .unwrap()
                .absent()
        );
    }

    #[test]
    fn writer_lock_checks_the_actual_open_directory_inode_and_excludes_a_second_writer() {
        let (_dir, store) = fixture();
        let (other, _) = fixture();
        let wrong = std::fs::File::open(other.path()).unwrap();
        assert_eq!(store.validate_held_parent(&wrong), Err(Error::Unsafe));
        let held = store.lock_writer().unwrap();
        assert!(matches!(store.lock_writer(), Err(Error::Unavailable)));
        drop(held);
        assert!(store.lock_writer().is_ok());
    }

    #[test]
    fn missing_default_and_noop_remain_absent_while_real_checkpoint_is_private_and_durable() {
        let (dir, store) = fixture();
        let absent = store.read().unwrap();
        assert!(absent.absent());
        assert!(store.replace(&absent, absent.state()).unwrap().absent());
        assert!(!dir.path().join(NAME).exists());
        let mut enabled = absent.state().clone();
        enabled.configure(true, 100).unwrap();
        let stored = store.replace(&absent, &enabled).unwrap();
        assert!(!stored.absent());
        assert!(private_file(
            &fs::metadata(dir.path().join(NAME)).unwrap(),
            nix::unistd::getuid().as_raw()
        ));
        assert_eq!(stored.state().encode().unwrap(), enabled.encode().unwrap());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn lost_result_after_actual_atomic_replace_is_unknown_and_restart_reads_the_real_outcome() {
        let (dir, mut store) = fixture();
        let before = store.read().unwrap();
        let mut enabled = before.state().clone();
        enabled.configure(true, 100).unwrap();
        store.lose_reply_after_replace = true;
        assert!(matches!(
            store.replace(&before, &enabled),
            Err(Error::CommitUnknown)
        ));
        // The error is not permission to repeat the write or restore old bytes.
        let durable = fs::read(dir.path().join(NAME)).unwrap();
        assert_eq!(durable, enabled.encode().unwrap());
        let fresh = Store::new(dir.path(), nix::unistd::getuid().as_raw())
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(fresh.state().encode().unwrap(), durable);
        assert!(!fresh.absent());
    }

    #[test]
    fn stale_expected_snapshot_never_replaces_newer_control_or_checkpoint() {
        let (dir, store) = fixture();
        let absent = store.read().unwrap();
        let mut enabled = absent.state().clone();
        enabled.configure(true, 100).unwrap();
        let first = store.replace(&absent, &enabled).unwrap();
        let mut reset = first.state().clone();
        reset.reset(200).unwrap();
        let second = store.replace(&first, &reset).unwrap();
        assert!(matches!(
            store.replace(&first, &enabled),
            Err(Error::Changed)
        ));
        assert_eq!(
            fs::read(dir.path().join(NAME)).unwrap(),
            second.original.unwrap()
        );
    }

    #[test]
    fn unknown_or_malformed_or_oversized_state_is_left_untouched() {
        let (dir, store) = fixture();
        for body in [
            b"{\"schemaVersion\":2}".to_vec(),
            b"{\"private\":true}".to_vec(),
            vec![b'x'; MAX_BYTES + 1],
        ] {
            fs::write(dir.path().join(NAME), &body).unwrap();
            fs::set_permissions(dir.path().join(NAME), fs::Permissions::from_mode(0o600)).unwrap();
            assert!(matches!(store.read(), Err(Error::Invalid)));
            assert_eq!(fs::read(dir.path().join(NAME)).unwrap(), body);
        }
    }

    #[test]
    fn symlink_hardlink_permissions_and_parent_redirection_refuse_without_writes() {
        let (dir, store) = fixture();
        let target = dir.path().join("unrelated");
        fs::write(&target, b"preserve").unwrap();
        symlink(&target, dir.path().join(NAME)).unwrap();
        assert!(matches!(store.read(), Err(Error::Unsafe)));
        assert_eq!(fs::read(&target).unwrap(), b"preserve");
        fs::remove_file(dir.path().join(NAME)).unwrap();
        fs::hard_link(&target, dir.path().join(NAME)).unwrap();
        assert!(matches!(store.read(), Err(Error::Unsafe)));
        fs::remove_file(dir.path().join(NAME)).unwrap();
        fs::write(dir.path().join(NAME), b"preserve").unwrap();
        fs::set_permissions(dir.path().join(NAME), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(store.read(), Err(Error::Unsafe)));
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(store.read(), Err(Error::Unsafe)));
    }

    #[test]
    fn both_actual_full_file_outcomes_restart_without_reapplying_the_old_delta() {
        for commit_new in [false, true] {
            let (_dir, store) = fixture();
            let absent = store.read().unwrap();
            let mut state = absent.state().clone();
            state.configure(true, 100).unwrap();
            state
                .observe(&TrafficCounters {
                    identity: "a".repeat(64),
                    rx_bytes: 5000,
                    tx_bytes: 0,
                    sampled_at_ms: 1,
                })
                .unwrap();
            let old = store.replace(&absent, &state).unwrap();
            state
                .observe(&TrafficCounters {
                    identity: "a".repeat(64),
                    rx_bytes: 5500,
                    tx_bytes: 0,
                    sampled_at_ms: 2,
                })
                .unwrap();
            if commit_new {
                store.replace(&old, &state).unwrap();
            }
            let actual = store.read().unwrap();
            let mut restart = actual.state().clone();
            restart.new_runtime().unwrap();
            restart
                .observe(&TrafficCounters {
                    identity: "a".repeat(64),
                    rx_bytes: 5500,
                    tx_bytes: 0,
                    sampled_at_ms: 0,
                })
                .unwrap();
            assert_eq!(
                restart.projection()["receivedBytes"],
                if commit_new { "500" } else { "0" }
            );
            // This checks both real serialized disk outcomes, not a claimed
            // installed crash, fsync failure, or lost IPC response witness.
        }
    }
}
