//! Inactive durable intent adapter. This is NOT a kernel ownership receipt.
//! The caller must hold this store for the complete observation/transaction and
//! acknowledge a persist effect only after `persist` succeeds. No provisioning,
//! deletion, automatic repair, nft execution or production hookup is provided.
use crate::transaction::Marker;
use nix::fcntl::{Flock, FlockArg, OFlag, open, openat, renameat};
use nix::sys::stat::{Mode, fstatat};
use nix::{errno::Errno, fcntl::AtFlags};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;

const DIR: &str = "omavless-netguard";
const STATE: &str = "armed-v1.json";
const STAGE: &str = ".armed-v1.json.next";
const LIMIT: u64 = 4096;
const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateError {
    UnsafeOrUnreadable,
    Busy,
    InvalidTransition,
    UncertainWrite,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    policy_version: u32,
    enrolled_uid: u32,
    generation: u64,
    armed: bool,
    flags: u32,
}

fn decode(bytes: &[u8], uid: u32) -> Marker {
    if bytes.len() > LIMIT as usize
        || uid == 0
        || bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{')
    {
        return Marker::Invalid;
    }
    let Ok(record) = serde_json::from_slice::<Record>(bytes) else {
        return Marker::Invalid;
    };
    if record.version != 1
        || record.policy_version != 1
        || record.flags != 0
        || record.enrolled_uid != uid
    {
        return Marker::Invalid;
    }
    if record.armed {
        Marker::Armed(record.generation)
    } else {
        Marker::Closed(record.generation)
    }
}

fn encode(marker: Marker, uid: u32) -> Result<Vec<u8>, StateError> {
    let (generation, armed) = match marker {
        Marker::Armed(n) => (n, true),
        Marker::Closed(n) => (n, false),
        _ => return Err(StateError::InvalidTransition),
    };
    serde_json::to_vec(&Record {
        version: 1,
        policy_version: 1,
        enrolled_uid: uid,
        generation,
        armed,
        flags: 0,
    })
    .map_err(|_| StateError::InvalidTransition)
}

fn transition(old: Marker, new: Marker) -> bool {
    match (old, new) {
        (Marker::Missing, Marker::Armed(_)) => true,
        (Marker::Armed(a), Marker::Armed(b) | Marker::Closed(b)) => a == b,
        (Marker::Closed(a), Marker::Armed(b)) => b > a,
        (Marker::Closed(a), Marker::Closed(b)) => a == b,
        _ => false,
    }
}

pub struct RootStateStore {
    parent: File,
    dir: Flock<File>,
    owner: (u32, u32),
    enrolled_uid: u32,
    poisoned: bool,
}

impl RootStateStore {
    /// Shared inactive receipt adapter uses the same pinned-directory lock.
    pub(crate) fn receipt_directory(&self) -> Result<(&File, (u32, u32), u32), StateError> {
        if self.poisoned {
            return Err(StateError::UnsafeOrUnreadable);
        }
        self.bound()?;
        Ok((&self.dir, self.owner, self.enrolled_uid))
    }

    /// Inactive fixed-path writer. Only the enrolled LockedState entry may
    /// open it; external callers cannot nominate a UID for root state.
    pub(crate) fn open_fixed(enrolled_uid: u32) -> Result<Self, StateError> {
        let mut parent = File::from(
            open("/", DIRECTORY, Mode::empty()).map_err(|_| StateError::UnsafeOrUnreadable)?,
        );
        check_ancestor(&parent)?;
        for name in ["var", "lib"] {
            parent = File::from(
                openat(&parent, name, DIRECTORY, Mode::empty())
                    .map_err(|_| StateError::UnsafeOrUnreadable)?,
            );
            check_ancestor(&parent)?;
        }
        Self::open_under(parent, (0, 0), enrolled_uid)
    }

    #[cfg(test)]
    pub(crate) fn open_test_parent(
        parent: File,
        owner: (u32, u32),
        enrolled_uid: u32,
    ) -> Result<Self, StateError> {
        Self::open_under(parent, owner, enrolled_uid)
    }

    fn open_under(parent: File, owner: (u32, u32), enrolled_uid: u32) -> Result<Self, StateError> {
        if enrolled_uid == 0 {
            return Err(StateError::UnsafeOrUnreadable);
        }
        let dir = File::from(
            openat(&parent, DIR, DIRECTORY, Mode::empty())
                .map_err(|_| StateError::UnsafeOrUnreadable)?,
        );
        check(&dir, owner, true)?;
        let dir = Flock::lock(dir, FlockArg::LockExclusiveNonblock).map_err(|(_, e)| {
            if e == Errno::EWOULDBLOCK {
                StateError::Busy
            } else {
                StateError::UnsafeOrUnreadable
            }
        })?;
        Ok(Self {
            parent,
            dir,
            owner,
            enrolled_uid,
            poisoned: false,
        })
    }

    fn bound(&self) -> Result<(), StateError> {
        check(&self.dir, self.owner, true)?;
        let current = File::from(
            openat(&self.parent, DIR, DIRECTORY, Mode::empty())
                .map_err(|_| StateError::UnsafeOrUnreadable)?,
        );
        check(&current, self.owner, true)?;
        same(&self.dir, &current)
    }

    fn pending(&self) -> Result<bool, StateError> {
        match fstatat(&*self.dir, STAGE, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(Errno::ENOENT) => Ok(false),
            Err(_) => Err(StateError::UnsafeOrUnreadable),
        }
    }

    /// Missing is only a storage observation, never proof of absent rules.
    /// All unsafe/unreadable/unsupported state maps to Invalid, not Missing.
    pub fn marker(&self) -> Marker {
        if self.poisoned {
            return Marker::Invalid;
        }
        self.read().unwrap_or(Marker::Invalid)
    }

    /// Coordinator-only admission distinguishes a decoded invalid document
    /// from a failed/unsafe storage read. The latter cannot authorize even an
    /// emergency table mutation from a possibly lost directory lock.
    pub(crate) fn checked_marker(&self) -> Result<Marker, StateError> {
        if self.poisoned {
            return Err(StateError::UnsafeOrUnreadable);
        }
        self.read()
    }

    fn read(&self) -> Result<Marker, StateError> {
        self.bound()?;
        if self.pending()? {
            return Err(StateError::UnsafeOrUnreadable);
        }
        let flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
        let fd = match openat(&*self.dir, STATE, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::ENOENT) => return Ok(Marker::Missing),
            Err(_) => return Err(StateError::UnsafeOrUnreadable),
        };
        let mut file = File::from(fd);
        check(&file, self.owner, false)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| StateError::UnsafeOrUnreadable)?;
        check(&file, self.owner, false)?;
        let current = File::from(
            openat(&*self.dir, STATE, flags, Mode::empty())
                .map_err(|_| StateError::UnsafeOrUnreadable)?,
        );
        same(&file, &current)?;
        self.bound()?;
        Ok(decode(&bytes, self.enrolled_uid))
    }

    pub fn persist(&mut self, expected: Marker, next: Marker) -> Result<(), StateError> {
        self.persist_inner(expected, next, |_| Ok(()))
    }

    pub(crate) fn persist_inner(
        &mut self,
        expected: Marker,
        next: Marker,
        mut checkpoint: impl FnMut(u8) -> Result<(), StateError>,
    ) -> Result<(), StateError> {
        if self.marker() != expected || !transition(expected, next) {
            return Err(StateError::InvalidTransition);
        }
        let bytes = encode(next, self.enrolled_uid)?;
        let result = (|| {
            checkpoint(0)?;
            let mut stage = File::from(
                openat(
                    &*self.dir,
                    STAGE,
                    OFlag::O_WRONLY
                        | OFlag::O_CREAT
                        | OFlag::O_EXCL
                        | OFlag::O_NOFOLLOW
                        | OFlag::O_NONBLOCK
                        | OFlag::O_CLOEXEC,
                    Mode::S_IRUSR | Mode::S_IWUSR,
                )
                .map_err(|_| StateError::UncertainWrite)?,
            );
            check(&stage, self.owner, false)?;
            checkpoint(1)?;
            stage
                .write_all(&bytes)
                .map_err(|_| StateError::UncertainWrite)?;
            checkpoint(2)?;
            stage.sync_all().map_err(|_| StateError::UncertainWrite)?;
            checkpoint(3)?;
            self.bound()?;
            renameat(&*self.dir, STAGE, &*self.dir, STATE)
                .map_err(|_| StateError::UncertainWrite)?;
            checkpoint(4)?;
            self.dir
                .sync_all()
                .map_err(|_| StateError::UncertainWrite)?;
            checkpoint(5)?;
            if self.read()? != next {
                return Err(StateError::UncertainWrite);
            }
            Ok(())
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}

fn check_ancestor(file: &File) -> Result<(), StateError> {
    let m = file
        .metadata()
        .map_err(|_| StateError::UnsafeOrUnreadable)?;
    if !m.is_dir() || m.uid() != 0 || m.gid() != 0 || m.mode() & 0o022 != 0 {
        return Err(StateError::UnsafeOrUnreadable);
    }
    Ok(())
}

fn check(file: &File, owner: (u32, u32), directory: bool) -> Result<(), StateError> {
    let m = file
        .metadata()
        .map_err(|_| StateError::UnsafeOrUnreadable)?;
    let correct_type = if directory {
        m.is_dir()
    } else {
        m.is_file() && m.nlink() == 1 && m.len() <= LIMIT
    };
    if !correct_type
        || (m.uid(), m.gid()) != owner
        || m.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
    {
        return Err(StateError::UnsafeOrUnreadable);
    }
    Ok(())
}

fn same(a: &File, b: &File) -> Result<(), StateError> {
    let a = a.metadata().map_err(|_| StateError::UnsafeOrUnreadable)?;
    let b = b.metadata().map_err(|_| StateError::UnsafeOrUnreadable)?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Err(StateError::UnsafeOrUnreadable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        policy::Policy,
        protocol::{Mode as GuardMode, Request},
        transaction::{self, Effect, Observation, Table},
    };
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omavless-root-state-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path.join(DIR))
                .unwrap();
            Self(path)
        }
        fn open(&self) -> Result<RootStateStore, StateError> {
            let parent = File::open(&self.0).unwrap();
            let m = parent.metadata().unwrap();
            RootStateStore::open_under(parent, (m.uid(), m.gid()), 1001)
        }
        fn put(&self, name: &str, data: &[u8]) {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.0.join(DIR).join(name))
                .unwrap();
            f.write_all(data).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn strict_flat_schema_rejects_ambiguity_and_newer_state() {
        let valid = encode(Marker::Armed(u64::MAX), 1001).unwrap();
        assert_eq!(decode(&valid, 1001), Marker::Armed(u64::MAX));
        assert_eq!(decode(&valid, 1002), Marker::Invalid);
        for input in [
            "[1,1,1001,1,true,0]",
            "{}",
            "null",
            "{\"version\":1,\"version\":1}",
            "{\"version\":2,\"policy_version\":1,\"enrolled_uid\":1001,\"generation\":1,\"armed\":true,\"flags\":0}",
        ] {
            assert_eq!(decode(input.as_bytes(), 1001), Marker::Invalid);
        }
        let text = String::from_utf8(encode(Marker::Closed(7), 1001).unwrap()).unwrap();
        for changed in [
            text.replace("\"policy_version\":1", "\"policy_version\":2"),
            text.replace("\"flags\":0", "\"flags\":1"),
            text.replace("\"generation\":7", "\"generation\":-1"),
            text.replace("\"generation\":7", "\"generation\":7.0"),
            text.replace("\"flags\":0", "\"flags\":0,\"extra\":true"),
            text.replace("\"armed\":false", "\"armed\":false,\"armed\":true"),
        ] {
            assert_eq!(decode(changed.as_bytes(), 1001), Marker::Invalid);
        }
        assert_eq!(
            decode(&vec![b' '; LIMIT as usize + 1], 1001),
            Marker::Invalid
        );
    }

    #[test]
    fn roundtrip_closed_fence_and_exclusive_lock_survive_reopen() {
        let f = Fixture::new();
        let mut store = f.open().unwrap();
        assert!(matches!(f.open(), Err(StateError::Busy)));
        assert_eq!(store.marker(), Marker::Missing);
        store.persist(Marker::Missing, Marker::Armed(0)).unwrap();
        store.persist(Marker::Armed(0), Marker::Closed(0)).unwrap();
        drop(store);
        let mut store = f.open().unwrap();
        assert_eq!(store.marker(), Marker::Closed(0));
        assert_eq!(
            store.persist(Marker::Closed(0), Marker::Armed(0)),
            Err(StateError::InvalidTransition)
        );
        store
            .persist(Marker::Closed(0), Marker::Armed(u64::MAX))
            .unwrap();
        store
            .persist(Marker::Armed(u64::MAX), Marker::Closed(u64::MAX))
            .unwrap();
        assert_eq!(
            store.persist(Marker::Closed(u64::MAX), Marker::Armed(0)),
            Err(StateError::InvalidTransition)
        );
    }

    #[test]
    fn unsafe_files_and_pending_intent_never_become_missing() {
        for kind in 0..8 {
            let f = Fixture::new();
            let path = f.0.join(DIR).join(STATE);
            match kind {
                0 => symlink("missing", &path).unwrap(),
                1 => fs::create_dir(&path).unwrap(),
                2 => {
                    f.put(STATE, b"{}");
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
                }
                3 => {
                    f.put(STATE, b"{}");
                    fs::hard_link(&path, f.0.join("alias")).unwrap();
                }
                4 => f.put(STATE, &vec![b'x'; LIMIT as usize + 1]),
                5 => f.put(STAGE, b""),
                6 => symlink("missing", f.0.join(DIR).join(STAGE)).unwrap(),
                _ => nix::unistd::mkfifo(&path, Mode::S_IRUSR | Mode::S_IWUSR).unwrap(),
            }
            assert_eq!(f.open().unwrap().marker(), Marker::Invalid);
        }
    }

    #[test]
    fn directory_modes_symlinks_rebinding_and_wrong_owner_refuse() {
        let f = Fixture::new();
        let store = f.open().unwrap();
        let m = store.dir.metadata().unwrap();
        assert_eq!(
            check(&store.dir, (m.uid().wrapping_add(1), m.gid()), true),
            Err(StateError::UnsafeOrUnreadable)
        );
        fs::set_permissions(f.0.join(DIR), fs::Permissions::from_mode(0o750)).unwrap();
        assert_eq!(store.marker(), Marker::Invalid);
        fs::set_permissions(f.0.join(DIR), fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(f.0.join(DIR), f.0.join("old")).unwrap();
        fs::create_dir(f.0.join(DIR)).unwrap();
        fs::set_permissions(f.0.join(DIR), fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(store.marker(), Marker::Invalid);
        fs::remove_dir(f.0.join(DIR)).unwrap();
        symlink("old", f.0.join(DIR)).unwrap();
        assert!(matches!(f.open(), Err(StateError::UnsafeOrUnreadable)));
    }

    #[test]
    fn crash_checkpoints_preserve_intent_and_never_acknowledge_uncertain_write() {
        for point in 0..=5 {
            let f = Fixture::new();
            let mut store = f.open().unwrap();
            store.persist(Marker::Missing, Marker::Armed(9)).unwrap();
            assert!(
                store
                    .persist_inner(Marker::Armed(9), Marker::Closed(9), |stage| {
                        if stage == point {
                            Err(StateError::UncertainWrite)
                        } else {
                            Ok(())
                        }
                    })
                    .is_err()
            );
            assert_eq!(store.marker(), Marker::Invalid);
            drop(store);
            let reopened = f.open().unwrap();
            let expected = match point {
                0 => Marker::Armed(9),
                1..=3 => Marker::Invalid,
                _ => Marker::Closed(9),
            };
            assert_eq!(reopened.marker(), expected);
            assert_eq!(f.0.join(DIR).join(STAGE).exists(), (1..=3).contains(&point));
        }
    }

    #[test]
    fn invalid_state_requests_protection_but_never_claims_foreign_table() {
        let mut tx = transaction::reconcile(Observation {
            marker: Marker::Invalid,
            table: Table::Absent,
        })
        .unwrap();
        assert_eq!(
            tx.next_effect(),
            Some(Effect::CreateTableAtomic(Policy::Emergency))
        );
        assert!(tx.response().is_err());
        tx.acknowledge(Effect::CreateTableAtomic(Policy::Emergency), true)
            .unwrap();
        assert_eq!(
            tx.next_effect(),
            Some(Effect::VerifyTable(Policy::Emergency))
        );
        assert!(
            transaction::reconcile(Observation {
                marker: Marker::Invalid,
                table: Table::Foreign
            })
            .is_err()
        );
        assert!(
            transaction::reconcile(Observation {
                marker: Marker::Missing,
                table: Table::Foreign
            })
            .is_err()
        );
        let stale = transaction::reconcile(Observation {
            marker: Marker::Missing,
            table: Table::OwnedVerified(Policy::FullVpn),
        })
        .unwrap();
        assert_eq!(stale.next_effect(), Some(Effect::DeleteOwnedTableAtomic));
        assert!(stale.response().is_err());
    }

    #[test]
    fn transaction_requires_durable_closed_before_delete_and_arm_before_success() {
        let observation = Observation {
            marker: Marker::Armed(8),
            table: Table::OwnedVerified(Policy::FullVpn),
        };
        let mut tx = transaction::plan(Request::Disarm { generation: 8 }, observation).unwrap();
        assert_eq!(tx.next_effect(), Some(Effect::PersistClosedDurably(8)));
        assert!(
            tx.acknowledge(Effect::PersistClosedDurably(8), false)
                .is_err()
        );
        assert_eq!(tx.next_effect(), None);
        assert!(tx.response().is_err());
        let mut arm = transaction::plan(
            Request::Arm {
                generation: 9,
                mode: GuardMode::Full,
            },
            Observation {
                marker: Marker::Closed(8),
                table: Table::Absent,
            },
        )
        .unwrap();
        for effect in [
            Effect::CreateTableAtomic(Policy::FullVpn),
            Effect::VerifyTable(Policy::FullVpn),
        ] {
            arm.acknowledge(effect, true).unwrap();
        }
        assert!(arm.response().is_err());
        assert_eq!(arm.next_effect(), Some(Effect::PersistArmedDurably(9)));
        arm.acknowledge(Effect::PersistArmedDurably(9), true)
            .unwrap();
        assert!(arm.response().is_ok());
    }
}
