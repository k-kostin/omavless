//! Inactive, read-only binding to the administrator's fixed enrollment file.
//! This authenticates the local configuration bytes, not a peer, namespace,
//! nft table, or protected connection. Provisioning is a separate root action.
use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, os::unix::fs::MetadataExt};

const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const LEAF: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_CLOEXEC);
const DIR: &str = "omavless-netguard";
const FILE: &str = "enrollment-v1.json";
const LIMIT: u64 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnrollmentError {
    UnsafeOrUnreadable,
}
const REFUSE: EnrollmentError = EnrollmentError::UnsafeOrUnreadable;
type Result<T> = std::result::Result<T, EnrollmentError>;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    enrolled_uid: u32,
}

fn decode(bytes: &[u8]) -> Result<u32> {
    if bytes.len() > LIMIT as usize || bytes.first() != Some(&b'{') {
        return Err(REFUSE);
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| REFUSE)?;
    if record.version != 1
        || record.enrolled_uid == 0
        || serde_json::to_vec(&record).map_err(|_| REFUSE)? != bytes
    {
        return Err(REFUSE);
    }
    Ok(record.enrolled_uid)
}

fn ancestor(file: &File) -> Result<()> {
    let m = file.metadata().map_err(|_| REFUSE)?;
    if !m.is_dir() || (m.uid(), m.gid()) != (0, 0) || m.mode() & 0o022 != 0 {
        return Err(REFUSE);
    }
    Ok(())
}

fn check(file: &File, owner: (u32, u32), directory: bool) -> Result<()> {
    let m = file.metadata().map_err(|_| REFUSE)?;
    let kind = if directory {
        m.is_dir()
    } else {
        m.is_file() && m.nlink() == 1 && m.len() <= LIMIT
    };
    if !kind
        || (m.uid(), m.gid()) != owner
        || m.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
    {
        return Err(REFUSE);
    }
    Ok(())
}

fn same(left: &File, right: &File) -> Result<()> {
    let a = left.metadata().map_err(|_| REFUSE)?;
    let b = right.metadata().map_err(|_| REFUSE)?;
    if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
        return Err(REFUSE);
    }
    Ok(())
}

/// Pins the enrolled UID and file identity for the lifetime of one request.
/// Only the root package may create or replace the configuration. The caller
/// must validate again before and after every external effect.
pub(crate) struct EnrollmentBinding {
    etc: File,
    directory: File,
    file: File,
    owner: (u32, u32),
    uid: u32,
}

impl EnrollmentBinding {
    pub(crate) fn open_fixed() -> Result<Self> {
        let root = File::from(open("/", DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        ancestor(&root)?;
        let etc = File::from(openat(&root, "etc", DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        ancestor(&etc)?;
        Self::open_under(etc, (0, 0))
    }

    #[cfg(test)]
    pub(crate) fn open_test_parent(etc: File, owner: (u32, u32)) -> Result<Self> {
        Self::open_under(etc, owner)
    }

    fn open_under(etc: File, owner: (u32, u32)) -> Result<Self> {
        let directory =
            File::from(openat(&etc, DIR, DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        check(&directory, owner, true)?;
        let mut file =
            File::from(openat(&directory, FILE, LEAF, Mode::empty()).map_err(|_| REFUSE)?);
        check(&file, owner, false)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| REFUSE)?;
        let uid = decode(&bytes)?;
        let binding = Self {
            etc,
            directory,
            file,
            owner,
            uid,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub(crate) fn uid(&self) -> u32 {
        self.uid
    }

    pub(crate) fn validate(&self) -> Result<()> {
        check(&self.directory, self.owner, true)?;
        check(&self.file, self.owner, false)?;
        let directory =
            File::from(openat(&self.etc, DIR, DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        same(&self.directory, &directory)?;
        let mut file =
            File::from(openat(&self.directory, FILE, LEAF, Mode::empty()).map_err(|_| REFUSE)?);
        same(&self.file, &file)?;
        check(&file, self.owner, false)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| REFUSE)?;
        if decode(&bytes)? != self.uid {
            return Err(REFUSE);
        }
        same(&self.file, &file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::Write,
        os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink},
    };

    #[test]
    fn exact_enrollment_and_rebinding_are_checked() {
        let temp = std::env::temp_dir().join(format!("omavless-enrollment-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&temp).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(temp.join(DIR))
            .unwrap();
        let path = temp.join(DIR).join(FILE);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        file.write_all(b"{\"version\":1,\"enrolled_uid\":1001}")
            .unwrap();
        drop(file);
        let parent = File::open(&temp).unwrap();
        let m = parent.metadata().unwrap();
        let owner = (m.uid(), m.gid());
        let binding = EnrollmentBinding::open_under(parent, owner).unwrap();
        assert_eq!(binding.uid(), 1001);
        assert_eq!(binding.validate(), Ok(()));
        fs::rename(&path, temp.join(DIR).join("old")).unwrap();
        symlink("old", &path).unwrap();
        assert_eq!(binding.validate(), Err(REFUSE));
        fs::remove_file(&path).unwrap();
        fs::rename(temp.join(DIR).join("old"), &path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(binding.validate(), Err(REFUSE));
        drop(binding);
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn schema_has_no_peer_selected_fields() {
        assert_eq!(decode(b"{\"version\":1,\"enrolled_uid\":1001}"), Ok(1001));
        for bytes in [
            b"{\"version\":1,\"enrolled_uid\":0}".as_slice(),
            b"{\"version\":1,\"enrolled_uid\":1001,\"uid\":1002}",
            b"{\"version\":1,\"enrolled_uid\":1001,\"enrolled_uid\":1002}",
            b"{\"version\":2,\"enrolled_uid\":1001}",
            b" {\"version\":1,\"enrolled_uid\":1001}",
            b"{\"enrolled_uid\":1001,\"version\":1}",
        ] {
            assert_eq!(decode(bytes), Err(REFUSE));
        }
    }
}
