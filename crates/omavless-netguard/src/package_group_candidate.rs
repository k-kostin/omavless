//! Inactive, read-only identity binding for the dedicated local package group.
//! This does not create a group, add a user to it, grant an existing session
//! membership or install a service. The socket publisher can require this
//! binding instead of accepting a caller-chosen numeric GID.

use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;

const GROUP: &str = "omavless-netguard";
const LIMIT: u64 = 1_048_576;
const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const READ: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GroupError {
    UnavailableOrChanged,
}

const REFUSE: GroupError = GroupError::UnavailableOrChanged;
type Result<T> = std::result::Result<T, GroupError>;

fn safe_ancestor(file: &File, owner: (u32, u32)) -> Result<()> {
    let meta = file.metadata().map_err(|_| REFUSE)?;
    if !meta.is_dir() || (meta.uid(), meta.gid()) != owner || meta.mode() & 0o022 != 0 {
        return Err(REFUSE);
    }
    Ok(())
}

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

fn safe_group_file(meta: &Metadata, owner: (u32, u32)) -> bool {
    meta.is_file()
        && (meta.uid(), meta.gid()) == owner
        && meta.nlink() == 1
        && meta.mode() & 0o022 == 0
        && (1..=LIMIT).contains(&meta.len())
}

fn canonical_gid(text: &str) -> Option<u32> {
    if text.is_empty()
        || text.len() > 10
        || (text.len() > 1 && text.starts_with('0'))
        || !text.as_bytes().iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    text.parse().ok()
}

fn group_gid(bytes: &[u8]) -> Result<u32> {
    if bytes.is_empty() || bytes.len() > LIMIT as usize || bytes.last() != Some(&b'\n') {
        return Err(REFUSE);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| REFUSE)?;
    let mut target = None;
    let mut gids = Vec::new();
    for line in text.lines() {
        let fields = line.split(':').collect::<Vec<_>>();
        // Unrelated local group names are not OmaVLESS-controlled. Arch can
        // legitimately use mixed case, dots, trailing '$' and leading digits.
        if fields.len() != 4 || fields[0].is_empty() || fields[1].is_empty() {
            return Err(REFUSE);
        }
        let gid = canonical_gid(fields[2]).ok_or(REFUSE)?;
        if fields[0] == GROUP
            && (target.replace(gid).is_some()
                || matches!(gid, 0 | u32::MAX)
                || !matches!(fields[1], "x" | "!"))
        {
            return Err(REFUSE);
        }
        gids.push((fields[0], gid));
    }
    let gid = target.ok_or(REFUSE)?;
    if gids
        .iter()
        .any(|(name, other)| *name != GROUP && *other == gid)
    {
        return Err(REFUSE);
    }
    Ok(gid)
}

fn read_group(etc: &File, owner: (u32, u32)) -> Result<(File, Metadata, Vec<u8>)> {
    safe_ancestor(etc, owner)?;
    let mut file = File::from(openat(etc, "group", READ, Mode::empty()).map_err(|_| REFUSE)?);
    let before = file.metadata().map_err(|_| REFUSE)?;
    if !safe_group_file(&before, owner) {
        return Err(REFUSE);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| REFUSE)?;
    let current = File::from(openat(etc, "group", READ, Mode::empty()).map_err(|_| REFUSE)?);
    let after = file.metadata().map_err(|_| REFUSE)?;
    let reopened = current.metadata().map_err(|_| REFUSE)?;
    if !same(&before, &after) || !same(&before, &reopened) || bytes.len() as u64 != before.len() {
        return Err(REFUSE);
    }
    Ok((file, before, bytes))
}

pub(crate) struct PackageGroup {
    etc: File,
    file: File,
    owner: (u32, u32),
    metadata: Metadata,
    bytes: Vec<u8>,
    gid: u32,
}

impl PackageGroup {
    pub(crate) fn open_fixed() -> Result<Self> {
        let root = File::from(open("/", DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        safe_ancestor(&root, (0, 0))?;
        let etc = File::from(openat(&root, "etc", DIRECTORY, Mode::empty()).map_err(|_| REFUSE)?);
        Self::open_under(etc, (0, 0))
    }

    #[cfg(test)]
    pub(crate) fn open_test_parent(etc: File, owner: (u32, u32)) -> Result<Self> {
        Self::open_under(etc, owner)
    }

    fn open_under(etc: File, owner: (u32, u32)) -> Result<Self> {
        let (file, metadata, bytes) = read_group(&etc, owner)?;
        let gid = group_gid(&bytes)?;
        Ok(Self {
            etc,
            file,
            owner,
            metadata,
            bytes,
            gid,
        })
    }

    pub(crate) fn gid(&self) -> u32 {
        self.gid
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let (file, metadata, bytes) = read_group(&self.etc, self.owner)?;
        if !same(&self.metadata, &metadata)
            || !same(&self.metadata, &self.file.metadata().map_err(|_| REFUSE)?)
            || !same(&self.metadata, &file.metadata().map_err(|_| REFUSE)?)
            || bytes != self.bytes
            || group_gid(&bytes)? != self.gid
        {
            return Err(REFUSE);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_must_be_unique_local_nonroot_group_with_unique_gid() {
        let valid = b"root:x:0:\nomavless-netguard:x:995:kk\nother:x:996:\n";
        assert_eq!(group_gid(valid), Ok(995));
        assert_eq!(
            group_gid(
                b"Project.Team:x:1001:\nACME$:x:1002:\n2d-team:x:1003:\nomavless-netguard:x:995:\n"
            ),
            Ok(995)
        );
        for invalid in [
            b"root:x:0:\n".as_slice(),
            b"omavless-netguard:x:0:\n",
            b"omavless-netguard:x:4294967295:\n",
            b"omavless-netguard:x:0995:\n",
            b"omavless-netguard:x:995:\nomavless-netguard:x:995:\n",
            b"omavless-netguard:x:995:\nalias:x:995:\n",
            b"omavless-netguard:x:995:\0",
            b"omavless-netguard:x:995:",
        ] {
            assert_eq!(group_gid(invalid), Err(REFUSE));
        }
    }
}
