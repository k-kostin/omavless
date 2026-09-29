//! Inactive receipt persistence prerequisite, not kernel ownership authority.
//! No provisioning, recovery, network effect or production caller. A receipt
//! remains a claim to be independently checked, even after durable storage.
use crate::receipt::{MAX_RECEIPT_BYTES, Receipt, ReceiptRead, decode};
use crate::root_state::{RootStateStore, StateError};
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, openat, renameat};
use nix::sys::stat::{Mode, fstatat};
use nix::unistd::{UnlinkatFlags, unlinkat};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;

const RECEIPT: &str = "table-receipt-v1.json";
const STAGE: &str = ".table-receipt-v1.json.next";
const PENDING: &str = ".table-receipt-v1.pending";
const READ: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_CLOEXEC);
const CREATE: OFlag = OFlag::O_WRONLY
    .union(OFlag::O_CREAT)
    .union(OFlag::O_EXCL)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

/// Owns the existing private-directory lock. No path, leaf name or root identity
/// can be supplied by a production caller. Never deletes a receipt or marker.
pub struct ReceiptStore {
    root: RootStateStore,
    poisoned: bool,
}

impl ReceiptStore {
    pub fn open_fixed(enrolled_uid: u32) -> Result<Self, StateError> {
        Ok(Self {
            root: RootStateStore::open_fixed(enrolled_uid)?,
            poisoned: false,
        })
    }

    /// Missing is only a safely observed missing file, not absent nft state.
    pub fn read(&self) -> ReceiptRead {
        if self.poisoned {
            return ReceiptRead::UnsafeOrUncertain;
        }
        self.read_checked(false)
            .unwrap_or(ReceiptRead::UnsafeOrUncertain)
    }

    fn read_checked(&self, publishing: bool) -> Result<ReceiptRead, StateError> {
        let (dir, owner, uid) = self.root.receipt_directory()?;
        if exists(dir, STAGE)? || (!publishing && exists(dir, PENDING)?) {
            return Err(StateError::UnsafeOrUnreadable);
        }
        let file = match openat(dir, RECEIPT, READ, Mode::empty()) {
            Ok(fd) => File::from(fd),
            Err(Errno::ENOENT) => {
                // ENOENT from a dangling symlink must not become Missing.
                if exists(dir, RECEIPT)? {
                    return Err(StateError::UnsafeOrUnreadable);
                }
                self.root.receipt_directory()?;
                return Ok(ReceiptRead::Missing);
            }
            Err(_) => return Err(StateError::UnsafeOrUnreadable),
        };
        check(&file, owner)?;
        let mut bytes = Vec::new();
        (&file)
            .take((MAX_RECEIPT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| StateError::UnsafeOrUnreadable)?;
        check(&file, owner)?;
        bound_leaf(dir, RECEIPT, &file, owner)?;
        self.root.receipt_directory()?;
        let receipt = decode(&bytes).map_err(|_| StateError::UnsafeOrUnreadable)?;
        if receipt.enrolled_uid() != uid || receipt.encode().ok().as_deref() != Some(&bytes) {
            return Err(StateError::UnsafeOrUnreadable);
        }
        Ok(ReceiptRead::Durable(receipt))
    }

    /// Storage-only compare-and-publish. Does NOT approve lifecycle transitions,
    /// prove namespace/table identity, acknowledge an effect or change a fence.
    /// Caller must separately establish authority before ever reaching a future
    /// production integration. Unsafe remnants require explicit external review.
    pub fn publish(&mut self, expected: ReceiptRead, next: Receipt) -> Result<(), StateError> {
        self.publish_with(expected, next, |_| Ok(()))
    }

    fn publish_with(
        &mut self,
        expected: ReceiptRead,
        next: Receipt,
        mut checkpoint: impl FnMut(u8) -> Result<(), StateError>,
    ) -> Result<(), StateError> {
        if expected == ReceiptRead::UnsafeOrUncertain || self.read() != expected {
            return Err(StateError::InvalidTransition);
        }
        let (_, _, uid) = self.root.receipt_directory()?;
        if next.enrolled_uid() != uid {
            return Err(StateError::InvalidTransition);
        }
        let bytes = next.encode().map_err(|_| StateError::InvalidTransition)?;
        let result = (|| {
            checkpoint(0)?;
            let (dir, owner, _) = self.root.receipt_directory()?;
            // A durable guard must precede any modification of the receipt.
            // It survives rename, unlike the stage file itself.
            let guard = File::from(
                openat(dir, PENDING, CREATE, Mode::S_IRUSR | Mode::S_IWUSR)
                    .map_err(|_| StateError::UncertainWrite)?,
            );
            check(&guard, owner)?;
            checkpoint(1)?;
            guard.sync_all().map_err(|_| StateError::UncertainWrite)?;
            checkpoint(2)?;
            dir.sync_all().map_err(|_| StateError::UncertainWrite)?;
            checkpoint(3)?;
            let mut stage = File::from(
                openat(dir, STAGE, CREATE, Mode::S_IRUSR | Mode::S_IWUSR)
                    .map_err(|_| StateError::UncertainWrite)?,
            );
            check(&stage, owner)?;
            checkpoint(4)?;
            stage
                .write_all(&bytes)
                .map_err(|_| StateError::UncertainWrite)?;
            checkpoint(5)?;
            stage.sync_all().map_err(|_| StateError::UncertainWrite)?;
            checkpoint(6)?;
            self.root.receipt_directory()?;
            bound_leaf(dir, STAGE, &stage, owner)?;
            bound_leaf(dir, PENDING, &guard, owner)?;
            renameat(dir, STAGE, dir, RECEIPT).map_err(|_| StateError::UncertainWrite)?;
            checkpoint(7)?;
            dir.sync_all().map_err(|_| StateError::UncertainWrite)?;
            checkpoint(8)?;
            if self.read_checked(true)? != ReceiptRead::Durable(next) {
                return Err(StateError::UncertainWrite);
            }
            self.root.receipt_directory()?;
            bound_leaf(dir, PENDING, &guard, owner)?;
            // Only our O_EXCL-created, still-bound guard is removed. Never a
            // leftover guard, foreign table, receipt, generation marker or stage.
            unlinkat(dir, PENDING, UnlinkatFlags::NoRemoveDir)
                .map_err(|_| StateError::UncertainWrite)?;
            checkpoint(9)?;
            dir.sync_all().map_err(|_| StateError::UncertainWrite)?;
            checkpoint(10)?;
            if self.read_checked(false)? != ReceiptRead::Durable(next) {
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

fn exists(dir: &File, name: &str) -> Result<bool, StateError> {
    match fstatat(dir, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(Errno::ENOENT) => Ok(false),
        Err(_) => Err(StateError::UnsafeOrUnreadable),
    }
}

fn check(file: &File, owner: (u32, u32)) -> Result<(), StateError> {
    let m = file
        .metadata()
        .map_err(|_| StateError::UnsafeOrUnreadable)?;
    if !m.is_file()
        || m.nlink() != 1
        || m.len() > MAX_RECEIPT_BYTES as u64
        || (m.uid(), m.gid()) != owner
        || m.mode() & 0o7777 != 0o600
    {
        return Err(StateError::UnsafeOrUnreadable);
    }
    Ok(())
}

fn bound_leaf(dir: &File, name: &str, pinned: &File, owner: (u32, u32)) -> Result<(), StateError> {
    check(pinned, owner)?;
    let current = File::from(
        openat(dir, name, READ, Mode::empty()).map_err(|_| StateError::UnsafeOrUnreadable)?,
    );
    check(&current, owner)?;
    let a = pinned
        .metadata()
        .map_err(|_| StateError::UnsafeOrUnreadable)?;
    let b = current
        .metadata()
        .map_err(|_| StateError::UnsafeOrUnreadable)?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Err(StateError::UnsafeOrUnreadable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "omavless-receipt-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path.join("omavless-netguard"))
                .unwrap();
            Self(path)
        }
        fn leaf(&self, name: &str) -> PathBuf {
            self.0.join("omavless-netguard").join(name)
        }
        fn root(&self) -> Result<RootStateStore, StateError> {
            let m = fs::metadata(&self.0).unwrap();
            RootStateStore::open_test_parent(File::open(&self.0).unwrap(), (m.uid(), m.gid()), 1000)
        }
        fn store(&self) -> ReceiptStore {
            ReceiptStore {
                root: self.root().unwrap(),
                poisoned: false,
            }
        }
        fn write(&self, name: &str, bytes: &[u8]) {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.leaf(name))
                .unwrap()
                .write_all(bytes)
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn receipt(operation: u64, uid: u32) -> Receipt {
        let boot = [1_u8; 16];
        let epoch = [2_u8; 16];
        decode(
            serde_json::to_string(&serde_json::json!({
                "version":1,"enrolled_uid":uid,"boot":boot,"host_netns_epoch":epoch,
                "netns_device":4,"netns_inode":5,"operation":operation,
                "phase":"live","table_handle":7
            }))
            .unwrap()
            .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn publish_roundtrip_cas_and_retained_marker() {
        let f = Fixture::new();
        f.write("armed-v1.json", b"untouched generation fence");
        let mut s = f.store();
        assert_eq!(s.read(), ReceiptRead::Missing);
        let a = receipt(1, 1000);
        let b = receipt(2, 1000);
        s.publish(ReceiptRead::Missing, a).unwrap();
        assert_eq!(s.read(), ReceiptRead::Durable(a));
        assert_eq!(
            s.publish(ReceiptRead::Missing, b),
            Err(StateError::InvalidTransition)
        );
        s.publish(ReceiptRead::Durable(a), b).unwrap();
        assert!(!f.leaf(STAGE).exists());
        assert!(!f.leaf(PENDING).exists());
        assert_eq!(
            fs::read(f.leaf("armed-v1.json")).unwrap(),
            b"untouched generation fence"
        );
        drop(s);
        assert_eq!(f.store().read(), ReceiptRead::Durable(b));
    }

    #[test]
    fn all_publication_boundaries_fail_closed_and_reopen_conservatively() {
        for initial in [false, true] {
            for failure in 0..=10 {
                let f = Fixture::new();
                let mut s = f.store();
                let a = receipt(1, 1000);
                if initial {
                    s.publish(ReceiptRead::Missing, a).unwrap();
                }
                let old = s.read();
                let next = receipt(2, 1000);
                assert!(
                    s.publish_with(old, next, |point| {
                        if point == failure {
                            Err(StateError::UncertainWrite)
                        } else {
                            Ok(())
                        }
                    })
                    .is_err()
                );
                assert_eq!(s.read(), ReceiptRead::UnsafeOrUncertain);
                assert!(s.publish(old, next).is_err());
                drop(s);
                let observed = f.store().read();
                assert_eq!(
                    observed,
                    match failure {
                        0 => old,                                // no mutation
                        1..=8 => ReceiptRead::UnsafeOrUncertain, // guard survives rename
                        9..=10 => ReceiptRead::Durable(next), // receipt dirsync preceded guard removal
                        _ => unreachable!(),
                    },
                    "checkpoint {failure}"
                );
            }
        }
    }

    #[test]
    fn canonical_document_and_enrollment_required() {
        let a = receipt(1, 1000);
        let mut whitespace = a.encode().unwrap();
        whitespace.push(b'\n');
        let mut duplicate = a.encode().unwrap();
        duplicate.splice(1..1, b"\"version\":1,".iter().copied());
        for bytes in [
            b"{}".to_vec(),
            b"[]".to_vec(),
            whitespace,
            duplicate,
            receipt(1, 1001).encode().unwrap(),
            vec![b'x'; MAX_RECEIPT_BYTES + 1],
        ] {
            let f = Fixture::new();
            f.write(RECEIPT, &bytes);
            let mut s = f.store();
            assert_eq!(s.read(), ReceiptRead::UnsafeOrUncertain);
            assert!(s.publish(ReceiptRead::Missing, a).is_err());
            assert_eq!(fs::read(f.leaf(RECEIPT)).unwrap(), bytes);
        }
        let f = Fixture::new();
        let mut s = f.store();
        assert!(s.publish(ReceiptRead::Missing, receipt(1, 1001)).is_err());
        assert_eq!(s.read(), ReceiptRead::Missing);
    }

    #[test]
    fn stage_and_guard_leftovers_are_never_repaired() {
        for name in [STAGE, PENDING] {
            let f = Fixture::new();
            f.write(name, b"foreign or incomplete");
            let mut s = f.store();
            assert_eq!(s.read(), ReceiptRead::UnsafeOrUncertain);
            assert!(s.publish(ReceiptRead::Missing, receipt(1, 1000)).is_err());
            assert_eq!(fs::read(f.leaf(name)).unwrap(), b"foreign or incomplete");
        }
    }

    #[test]
    fn shared_lock_and_directory_rebinding_are_refused() {
        let f = Fixture::new();
        let s = f.store();
        assert!(matches!(f.root(), Err(StateError::Busy)));
        let original = f.0.join("omavless-netguard");
        fs::rename(&original, f.0.join("old")).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&original).unwrap();
        assert_eq!(s.read(), ReceiptRead::UnsafeOrUncertain);
    }

    #[test]
    fn replaced_stage_or_guard_is_preserved_and_never_published() {
        for name in [STAGE, PENDING] {
            let f = Fixture::new();
            let mut s = f.store();
            let a = receipt(1, 1000);
            s.publish(ReceiptRead::Missing, a).unwrap();
            assert!(
                s.publish_with(ReceiptRead::Durable(a), receipt(2, 1000), |point| {
                    if point == 6 {
                        fs::rename(f.leaf(name), f.0.join("pinned-original")).unwrap();
                        f.write(name, b"foreign sentinel");
                    }
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(s.read(), ReceiptRead::UnsafeOrUncertain);
            assert_eq!(fs::read(f.leaf(name)).unwrap(), b"foreign sentinel");
            assert_eq!(fs::read(f.leaf(RECEIPT)).unwrap(), a.encode().unwrap());
            drop(s);
            assert_eq!(f.store().read(), ReceiptRead::UnsafeOrUncertain);
        }
    }

    #[test]
    fn partial_stage_is_never_taken_as_completed_receipt() {
        let f = Fixture::new();
        let mut s = f.store();
        assert!(
            s.publish_with(ReceiptRead::Missing, receipt(1, 1000), |point| {
                if point == 4 {
                    OpenOptions::new()
                        .write(true)
                        .open(f.leaf(STAGE))
                        .unwrap()
                        .write_all(b"{\"version\":")
                        .unwrap();
                    return Err(StateError::UncertainWrite);
                }
                Ok(())
            })
            .is_err()
        );
        assert!(!f.leaf(RECEIPT).exists());
        drop(s);
        assert_eq!(f.store().read(), ReceiptRead::UnsafeOrUncertain);
        assert_eq!(fs::read(f.leaf(STAGE)).unwrap(), b"{\"version\":");
    }

    #[test]
    fn unsafe_leaf_types_modes_and_owner_are_refused() {
        for case in 0..6 {
            let f = Fixture::new();
            match case {
                0 => symlink("missing", f.leaf(RECEIPT)).unwrap(),
                1 => {
                    f.write(RECEIPT, &receipt(1, 1000).encode().unwrap());
                    fs::set_permissions(f.leaf(RECEIPT), fs::Permissions::from_mode(0o644))
                        .unwrap();
                }
                2 => {
                    f.write(RECEIPT, &receipt(1, 1000).encode().unwrap());
                    fs::hard_link(f.leaf(RECEIPT), f.0.join("hardlink")).unwrap();
                }
                3 => nix::unistd::mkfifo(&f.leaf(RECEIPT), Mode::S_IRUSR | Mode::S_IWUSR).unwrap(),
                4 => fs::create_dir(f.leaf(RECEIPT)).unwrap(),
                5 => {
                    f.write(RECEIPT, &receipt(1, 1000).encode().unwrap());
                    let file = File::open(f.leaf(RECEIPT)).unwrap();
                    let m = file.metadata().unwrap();
                    assert!(check(&file, (m.uid().wrapping_add(1), m.gid())).is_err());
                    continue;
                }
                _ => unreachable!(),
            }
            assert_eq!(f.store().read(), ReceiptRead::UnsafeOrUncertain);
        }
    }
}
