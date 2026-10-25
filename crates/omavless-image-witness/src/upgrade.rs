// SPDX-License-Identifier: MIT
//! Explicit fixed-namespace root image transition, never effect IPC.
//!
//! Preparation does not change the filesystem. A prepared value owns its original
//! five-second deadline and, where one already exists, the exclusive lock. Commit
//! consumes that value and never cleans up or retries a partially published prefix.
use crate::{Error, Result};
use rustix::fs::{self, AtFlags, FlockOperation, Mode, OFlags, RenameFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, Metadata},
    io::Write,
    os::unix::fs::{FileExt, MetadataExt},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

const DIRECTORY: &str = "/var/lib/omavless-image-product";
const RECORD: &str = "runtime.enrollment";
const LOCK: &str = "maintenance.lock";
const HISTORY: &str = "upgrade-history";
const PENDING: &str = "upgrade.pending";
const SCHEMA: &str = "omavless-product-current-image-v1";
const MAX_HISTORY: usize = 128;
const MAX_IMAGE: u64 = 128 * 1024 * 1024;
const BUDGET: Duration = Duration::from_secs(5);

fn tick(until: Instant) -> Result<()> {
    crate::kernel::tick(until)
}
fn digest(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}
fn encode(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02x}")).collect()
}
fn decode(text: &str, limit: usize) -> Result<Vec<u8>> {
    if text.len() > limit * 2 || !text.len().is_multiple_of(2) {
        return Err(Error::Refused);
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |c| match c {
                b'0'..=b'9' => Ok(c - b'0'),
                b'a'..=b'f' => Ok(c - b'a' + 10),
                _ => Err(Error::Refused),
            };
            Ok(digit(pair[0])? * 16 + digit(pair[1])?)
        })
        .collect()
}
fn hash_arg(text: &str) -> Result<[u8; 32]> {
    let bytes: [u8; 32] = decode(text, 32)?.try_into().map_err(|_| Error::Refused)?;
    if bytes == [0; 32] {
        return Err(Error::Refused);
    }
    Ok(bytes)
}
fn root() -> Result<()> {
    if rustix::process::getuid().as_raw() != 0
        || rustix::process::geteuid().as_raw() != 0
        || rustix::process::getgid().as_raw() != 0
        || rustix::process::getegid().as_raw() != 0
    {
        return Err(Error::Refused);
    }
    Ok(())
}

/// Pure, canonical request identity; constructing this value performs no I/O.
#[derive(Clone, Debug)]
pub struct UpgradeRequest {
    uid: u32,
    predecessor: [u8; 32],
    core: [u8; 32],
    runtime: [u8; 32],
    operation: String,
}
impl UpgradeRequest {
    pub fn parse(uid: &str, old: &str, core: &str, runtime: &str) -> Result<Self> {
        let uid = crate::enrollment::uid(uid)?;
        let predecessor = hash_arg(old)?;
        let core = hash_arg(core)?;
        let runtime = hash_arg(runtime)?;
        let identity = format!(
            "omavless-product-upgrade-operation-v2\n{uid}\n{}\n{}\n{}\n",
            encode(&predecessor),
            encode(&core),
            encode(&runtime)
        );
        Ok(Self {
            uid,
            predecessor,
            core,
            runtime,
            operation: encode(&digest(identity.as_bytes())),
        })
    }
    pub fn operation_id(&self) -> &str {
        &self.operation
    }
    fn payload(&self) -> Vec<u8> {
        format!(
            "{SCHEMA}\n{}\n{}\n{}\n",
            self.uid,
            encode(&self.core),
            encode(&self.runtime)
        )
        .into_bytes()
    }
}

/// Observation, not replay permission or a replacement for the original exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inspection {
    /// No publication claim or bootstrap origin for this operation is recorded.
    NotRecorded,
    /// A retained prefix fences admission. No automatic repair is provided.
    Incomplete,
    /// Current committed predicates match; missing original durability ACKs
    /// and the original command's error remain missing/error respectively.
    ObservedCommitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Id {
    dev: u64,
    ino: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
    size: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
impl Id {
    fn of(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            uid: m.uid(),
            gid: m.gid(),
            mode: m.mode(),
            links: m.nlink(),
            size: m.len(),
            mtime: m.mtime(),
            mtime_ns: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_ns: m.ctime_nsec(),
        }
    }
    fn renamed_from(self, before: Self) -> bool {
        Self {
            ctime: before.ctime,
            ctime_ns: before.ctime_ns,
            ..self
        } == before
    }
    fn regular(self, owner: (u32, u32), mode: u32, limit: u64) -> Result<()> {
        if self.uid != owner.0
            || self.gid != owner.1
            || self.links != 1
            || self.mode != (0o100000 | mode)
            || self.size > limit
        {
            return Err(Error::Refused);
        }
        Ok(())
    }
}

// Only the explicitly selected Btrfs profile has a persistent-FSID contract.
// Linux fs/btrfs/super.c:btrfs_statfs derives f_fsid from the filesystem UUID
// and persistent subvolume object ID. This is not a generic statvfs guarantee
// or an installed reboot result; the actual guest qualification remains required.
const BTRFS_MAGIC: i64 = 0x9123_683e;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FsProfile {
    Btrfs,
    // Public finite source fixtures may live on another host filesystem. Their
    // actual descriptors are still observed; this is not production admission.
    #[cfg(test)]
    Fixture,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Filesystem {
    kind: i64,
    fsid: u64,
}
impl Filesystem {
    fn capture(file: &File) -> Result<Self> {
        let kind = fs::fstatfs(file).map_err(|_| Error::Unavailable)?.f_type as i64;
        // Pinned rustix's Linux implementation combines both statfs fsid words
        // into this public u64; no Debug parsing, unsafe cast or mount subprocess.
        let fsid = fs::fstatvfs(file).map_err(|_| Error::Unavailable)?.f_fsid;
        if fsid == 0 {
            return Err(Error::Refused);
        }
        Ok(Self { kind, fsid })
    }
    fn admit(self, profile: FsProfile) -> Result<()> {
        if self.fsid == 0 {
            return Err(Error::Refused);
        }
        match profile {
            FsProfile::Btrfs if self.kind != BTRFS_MAGIC => Err(Error::Refused),
            _ => Ok(()),
        }
    }
}
/// Serialized identity deliberately has NO kernel-assigned device number.
/// Live Id equality, held/named observations and same-process rename checks
/// remain exact, including st_dev. Nothing normalizes a live mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PersistentId {
    filesystem: Filesystem,
    ino: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
    size: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
impl PersistentId {
    fn of(filesystem: Filesystem, live: Id) -> Self {
        Self {
            filesystem,
            ino: live.ino,
            uid: live.uid,
            gid: live.gid,
            mode: live.mode,
            links: live.links,
            size: live.size,
            mtime: live.mtime,
            mtime_ns: live.mtime_ns,
            ctime: live.ctime,
            ctime_ns: live.ctime_ns,
        }
    }
    fn text(self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            self.filesystem.kind,
            self.filesystem.fsid,
            self.ino,
            self.uid,
            self.gid,
            self.mode,
            self.links,
            self.size,
            self.mtime,
            self.mtime_ns,
            self.ctime,
            self.ctime_ns
        )
    }
    fn parse(text: &str) -> Result<Self> {
        let f: Vec<_> = text.split(',').collect();
        if f.len() != 12 {
            return Err(Error::Refused);
        }
        let u = |i: usize| f[i].parse::<u64>().map_err(|_| Error::Refused);
        let small = |i: usize| u(i)?.try_into().map_err(|_| Error::Refused);
        let signed = |i: usize| f[i].parse::<i64>().map_err(|_| Error::Refused);
        let result = Self {
            filesystem: Filesystem {
                kind: signed(0)?,
                fsid: u(1)?,
            },
            ino: u(2)?,
            uid: small(3)?,
            gid: small(4)?,
            mode: small(5)?,
            links: u(6)?,
            size: u(7)?,
            mtime: signed(8)?,
            mtime_ns: signed(9)?,
            ctime: signed(10)?,
            ctime_ns: signed(11)?,
        };
        if result.filesystem.fsid == 0 || result.text() != text {
            return Err(Error::Refused);
        }
        Ok(result)
    }
    fn renamed_from(self, before: Self) -> bool {
        Self {
            ctime: before.ctime,
            ctime_ns: before.ctime_ns,
            ..self
        } == before
    }
    fn regular(self, owner: (u32, u32), mode: u32, limit: u64) -> Result<()> {
        if self.uid != owner.0
            || self.gid != owner.1
            || self.links != 1
            || self.mode != (0o100000 | mode)
            || self.size > limit
        {
            return Err(Error::Refused);
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Layout {
    anchor: PathBuf,
    directory: PathBuf,
    core: PathBuf,
    runtime: PathBuf,
    helper: PathBuf,
    socket: PathBuf,
    owner: (u32, u32),
    #[cfg(test)]
    fixture: bool,
    #[cfg(test)]
    fixture_profile: FsProfile,
}
impl Layout {
    fn production() -> Self {
        Self {
            anchor: PathBuf::from("/"),
            directory: PathBuf::from(DIRECTORY),
            core: PathBuf::from("/usr/lib/omavless-dns/mihomo"),
            runtime: PathBuf::from("/usr/bin/omavless"),
            helper: PathBuf::from("/usr/lib/omavless-image/omavless-image-witness"),
            socket: PathBuf::from("/run/omavless-image-product/control.sock"),
            owner: (0, 0),
            #[cfg(test)]
            fixture: false,
            #[cfg(test)]
            fixture_profile: FsProfile::Btrfs,
        }
    }
    fn profile(&self) -> FsProfile {
        #[cfg(test)]
        if self.fixture {
            return self.fixture_profile;
        }
        FsProfile::Btrfs
    }
    fn authorize(&self) -> Result<()> {
        #[cfg(test)]
        if self.fixture {
            return Ok(());
        }
        root()
    }
    fn no_socket(&self, until: Instant) -> Result<()> {
        tick(until)?;
        match std::fs::symlink_metadata(&self.socket) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => tick(until),
            _ => Err(Error::Refused),
        }
    }
}

struct Directory {
    file: File,
    path: PathBuf,
    id: (u64, u64, u32, u32, u32),
    filesystem: Filesystem,
    profile: FsProfile,
}
impl Directory {
    fn from(
        file: File,
        path: PathBuf,
        owner: (u32, u32),
        exact: Option<u32>,
        profile: FsProfile,
    ) -> Result<Self> {
        let m = file.metadata().map_err(|_| Error::Unavailable)?;
        if !m.is_dir()
            || m.uid() != owner.0
            || m.gid() != owner.1
            || m.mode() & 0o7022 != 0
            || exact.is_some_and(|mode| m.mode() & 0o7777 != mode)
        {
            return Err(Error::Refused);
        }
        let filesystem = Filesystem::capture(&file)?;
        filesystem.admit(profile)?;
        let result = Self {
            file,
            path,
            id: (m.dev(), m.ino(), m.uid(), m.gid(), m.mode()),
            filesystem,
            profile,
        };
        result.check()?;
        Ok(result)
    }
    fn check(&self) -> Result<()> {
        if Filesystem::capture(&self.file)? != self.filesystem {
            return Err(Error::Refused);
        }
        for m in [self.file.metadata(), std::fs::symlink_metadata(&self.path)] {
            let m = m.map_err(|_| Error::Unavailable)?;
            if !m.is_dir() || (m.dev(), m.ino(), m.uid(), m.gid(), m.mode()) != self.id {
                return Err(Error::Refused);
            }
        }
        Ok(())
    }
    fn child(&self, name: &str, owner: (u32, u32)) -> Result<Self> {
        self.check()?;
        let file = File::from(
            fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let result = Self::from(file, self.path.join(name), owner, Some(0o700), self.profile)?;
        if result.id.0 != self.id.0 || result.filesystem != self.filesystem {
            return Err(Error::Refused);
        }
        self.check()?;
        Ok(result)
    }
    fn sync(&self) -> Result<()> {
        self.check()?;
        self.file.sync_all().map_err(|_| Error::Unavailable)?;
        self.check()
    }
}
struct Chain {
    directories: Vec<Directory>,
}
impl Chain {
    fn capture(layout: &Layout, path: &Path, until: Instant) -> Result<Self> {
        tick(until)?;
        let suffix = path
            .strip_prefix(&layout.anchor)
            .map_err(|_| Error::Refused)?;
        let file = File::from(
            fs::open(
                &layout.anchor,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let mut directories = vec![Directory::from(
            file,
            layout.anchor.clone(),
            layout.owner,
            None,
            layout.profile(),
        )?];
        for component in suffix.components() {
            tick(until)?;
            let Component::Normal(name) = component else {
                return Err(Error::Refused);
            };
            if directories.len() >= 16 {
                return Err(Error::Refused);
            }
            let parent = directories.last().ok_or(Error::Refused)?;
            let file = File::from(
                fs::openat(
                    &parent.file,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| Error::Unavailable)?,
            );
            directories.push(Directory::from(
                file,
                parent.path.join(name),
                layout.owner,
                None,
                layout.profile(),
            )?);
        }
        let result = Self { directories };
        result.check(until)?;
        Ok(result)
    }
    fn last(&self) -> &Directory {
        self.directories.last().expect("captured anchor")
    }
    fn check(&self, until: Instant) -> Result<()> {
        for directory in &self.directories {
            tick(until)?;
            directory.check()?;
        }
        tick(until)
    }
    fn seal_prefix(&self) -> String {
        self.directories
            .iter()
            .map(|d| {
                let (_, ino, uid, gid, mode) = d.id;
                let Filesystem { kind, fsid } = d.filesystem;
                format!("{kind},{fsid},{ino},{uid},{gid},{mode}\n")
            })
            .collect()
    }
}
struct Node {
    file: File,
    path: PathBuf,
    id: Id,
    filesystem: Filesystem,
}
impl Node {
    fn open(
        directory: &Directory,
        name: &str,
        owner: (u32, u32),
        mode: u32,
        limit: u64,
    ) -> Result<Self> {
        directory.check()?;
        let file = File::from(
            fs::openat(
                &directory.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let id = Id::of(&file.metadata().map_err(|_| Error::Unavailable)?);
        id.regular(owner, mode, limit)?;
        let filesystem = Filesystem::capture(&file)?;
        filesystem.admit(directory.profile)?;
        if filesystem != directory.filesystem {
            return Err(Error::Refused);
        }
        let result = Self {
            file,
            path: directory.path.join(name),
            id,
            filesystem,
        };
        result.check()?;
        directory.check()?;
        Ok(result)
    }
    fn create(directory: &Directory, name: &str, owner: (u32, u32), raw: &[u8]) -> Result<Self> {
        directory.check()?;
        let mut file = File::from(
            fs::openat(
                &directory.file,
                name,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| Error::Unavailable)?,
        );
        Id::of(&file.metadata().map_err(|_| Error::Unavailable)?).regular(
            owner,
            0o600,
            raw.len() as u64,
        )?;
        file.write_all(raw).map_err(|_| Error::Unavailable)?;
        file.sync_all().map_err(|_| Error::Unavailable)?;
        let id = Id::of(&file.metadata().map_err(|_| Error::Unavailable)?);
        id.regular(owner, 0o600, raw.len() as u64)?;
        let filesystem = Filesystem::capture(&file)?;
        filesystem.admit(directory.profile)?;
        if filesystem != directory.filesystem {
            return Err(Error::Refused);
        }
        let result = Self {
            file,
            path: directory.path.join(name),
            id,
            filesystem,
        };
        if result.bytes(raw.len())? != raw {
            return Err(Error::Refused);
        }
        result.check()?;
        directory.check()?;
        Ok(result)
    }
    fn check(&self) -> Result<()> {
        self.check_at(&self.path, self.id)
    }
    fn check_at(&self, path: &Path, expected: Id) -> Result<()> {
        if Filesystem::capture(&self.file)? != self.filesystem {
            return Err(Error::Refused);
        }
        for m in [self.file.metadata(), std::fs::symlink_metadata(path)] {
            if Id::of(&m.map_err(|_| Error::Unavailable)?) != expected {
                return Err(Error::Refused);
            }
        }
        Ok(())
    }
    fn persistent(&self) -> PersistentId {
        PersistentId::of(self.filesystem, self.id)
    }
    fn bytes(&self, limit: usize) -> Result<Vec<u8>> {
        let mut raw = vec![0; limit + 1];
        let n = self
            .file
            .read_at(&mut raw, 0)
            .map_err(|_| Error::Unavailable)?;
        if n > limit || n as u64 != self.file.metadata().map_err(|_| Error::Unavailable)?.len() {
            return Err(Error::Refused);
        }
        raw.truncate(n);
        Ok(raw)
    }
}
fn absent(directory: &Directory, name: &str) -> Result<bool> {
    directory.check()?;
    match fs::statat(&directory.file, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(Error::Unavailable),
    }
}
fn names(directory: &Directory, limit: usize, until: Instant) -> Result<BTreeSet<String>> {
    directory.check()?;
    let mut result = BTreeSet::new();
    for entry in std::fs::read_dir(&directory.path).map_err(|_| Error::Unavailable)? {
        tick(until)?;
        let name = entry
            .map_err(|_| Error::Unavailable)?
            .file_name()
            .into_string()
            .map_err(|_| Error::Refused)?;
        if result.len() == limit || !result.insert(name) {
            return Err(Error::Refused);
        }
    }
    directory.check()?;
    Ok(result)
}
fn exact_names(directory: &Directory, expected: &[&str], until: Instant) -> Result<()> {
    let actual = names(directory, expected.len(), until)?;
    if actual != expected.iter().map(|s| (*s).to_owned()).collect() {
        return Err(Error::Refused);
    }
    Ok(())
}
fn payload(raw: &[u8]) -> Result<(u32, [u8; 32], [u8; 32])> {
    if raw.len() > 512 {
        return Err(Error::Refused);
    }
    let text = std::str::from_utf8(raw).map_err(|_| Error::Refused)?;
    let fields: Vec<_> = text.split('\n').collect();
    if fields.len() != 5 || fields[0] != SCHEMA || !fields[4].is_empty() {
        return Err(Error::Refused);
    }
    Ok((
        crate::enrollment::uid(fields[1])?,
        hash_arg(fields[2])?,
        hash_arg(fields[3])?,
    ))
}
struct Tree {
    chain: Chain,
    record: Node,
    raw: Vec<u8>,
}
impl Tree {
    fn capture(layout: &Layout, until: Instant) -> Result<Self> {
        let chain = Chain::capture(layout, &layout.directory, until)?;
        if chain.last().id.4 & 0o7777 != 0o700 {
            return Err(Error::Refused);
        }
        let record = Node::open(chain.last(), RECORD, layout.owner, 0o600, 512)?;
        let raw = record.bytes(512)?;
        payload(&raw)?;
        let result = Self { chain, record, raw };
        result.check(until)?;
        Ok(result)
    }
    fn directory(&self) -> &Directory {
        self.chain.last()
    }
    fn check(&self, until: Instant) -> Result<()> {
        self.chain.check(until)?;
        self.record.check()?;
        if self.record.bytes(512)? != self.raw {
            return Err(Error::Refused);
        }
        tick(until)
    }
    fn seal_for(&self, id: PersistentId, raw: &[u8]) -> [u8; 32] {
        digest(
            format!(
                "omavless-product-predecessor-v2\n{}{}\n{}\n",
                self.chain.seal_prefix(),
                id.text(),
                encode(raw)
            )
            .as_bytes(),
        )
    }
    fn seal(&self) -> [u8; 32] {
        self.seal_for(self.record.persistent(), &self.raw)
    }
    fn shape(&self, until: Instant) -> Result<()> {
        for name in names(self.directory(), 4, until)? {
            if ![RECORD, LOCK, HISTORY, PENDING].contains(&name.as_str()) {
                return Err(Error::Refused);
            }
        }
        Ok(())
    }
}
struct Image {
    chain: Chain,
    node: Node,
    hash: [u8; 32],
}
impl Image {
    fn capture(layout: &Layout, path: &Path, until: Instant) -> Result<Self> {
        let chain = Chain::capture(layout, path.parent().ok_or(Error::Refused)?, until)?;
        let node = Node::open(
            chain.last(),
            path.file_name()
                .and_then(|n| n.to_str())
                .ok_or(Error::Refused)?,
            layout.owner,
            0o755,
            MAX_IMAGE,
        )?;
        let mut header = [0; 64];
        if node
            .file
            .read_at(&mut header, 0)
            .map_err(|_| Error::Unavailable)?
            != 64
            || !native_header(&header)
        {
            return Err(Error::Refused);
        }
        let mut hasher = Sha256::new();
        let mut offset = 0;
        let mut bytes = [0; 65536];
        loop {
            tick(until)?;
            let n = node
                .file
                .read_at(&mut bytes, offset)
                .map_err(|_| Error::Unavailable)?;
            if n == 0 {
                break;
            }
            offset += n as u64;
            if offset > node.id.size {
                return Err(Error::Refused);
            }
            hasher.update(&bytes[..n]);
        }
        if offset != node.id.size {
            return Err(Error::Refused);
        }
        let result = Self {
            chain,
            node,
            hash: hasher.finalize().into(),
        };
        result.check(until)?;
        Ok(result)
    }
    fn check(&self, until: Instant) -> Result<()> {
        self.chain.check(until)?;
        self.node.check()?;
        tick(until)
    }
}
fn native_header(header: &[u8; 64]) -> bool {
    let machine = if cfg!(target_arch = "x86_64") {
        62
    } else if cfg!(target_arch = "aarch64") {
        183
    } else {
        0
    };
    header[..7] == [0x7f, b'E', b'L', b'F', 2, 1, 1]
        && matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        && machine != 0
        && u16::from_le_bytes([header[18], header[19]]) == machine
        && u64::from_le_bytes(header[24..32].try_into().expect("ELF entry width")) != 0
}
struct Administrator {
    image: Image,
    original: Option<File>,
}
impl Administrator {
    fn capture(layout: &Layout, until: Instant) -> Result<Self> {
        layout.authorize()?;
        let image = Image::capture(layout, &layout.helper, until)?;
        #[cfg(test)]
        if layout.fixture {
            return Ok(Self {
                image,
                original: None,
            });
        }
        // /proc/self/exe is the kernel's intentional executable link, not an
        // arbitrary caller pathname. Compare the opened object to the fixed ELF.
        let original = File::open("/proc/self/exe").map_err(|_| Error::Unavailable)?;
        if Id::of(&original.metadata().map_err(|_| Error::Unavailable)?) != image.node.id {
            return Err(Error::Refused);
        }
        Ok(Self {
            image,
            original: Some(original),
        })
    }
    fn check(&self, layout: &Layout, until: Instant) -> Result<()> {
        layout.authorize()?;
        self.image.check(until)?;
        if let Some(file) = &self.original
            && Id::of(&file.metadata().map_err(|_| Error::Unavailable)?) != self.image.node.id
        {
            return Err(Error::Refused);
        }
        tick(until)
    }
}
fn lock_payload(origin: Option<&str>) -> Vec<u8> {
    format!(
        "omavless-product-maintenance-lock-v2\n{}\n",
        origin.unwrap_or("fresh-enrollment")
    )
    .into_bytes()
}
struct Lock {
    node: Node,
    origin: Option<String>,
}
impl Lock {
    fn acquire(
        directory: &Directory,
        owner: (u32, u32),
        exclusive: bool,
        until: Instant,
    ) -> Result<Self> {
        tick(until)?;
        let node = Node::open(directory, LOCK, owner, 0o600, 128)?;
        let raw = node.bytes(128)?;
        let origin = if raw == lock_payload(None) {
            None
        } else {
            let text = std::str::from_utf8(&raw).map_err(|_| Error::Refused)?;
            let fields: Vec<_> = text.split('\n').collect();
            if fields.len() != 3 {
                return Err(Error::Refused);
            }
            hash_arg(fields[1])?;
            if raw != lock_payload(Some(fields[1])) {
                return Err(Error::Refused);
            }
            Some(fields[1].to_owned())
        };
        fs::flock(
            &node.file,
            if exclusive {
                FlockOperation::NonBlockingLockExclusive
            } else {
                FlockOperation::NonBlockingLockShared
            },
        )
        .map_err(|_| Error::Refused)?;
        node.check()?;
        tick(until)?;
        Ok(Self { node, origin })
    }
    fn check(&self, until: Instant) -> Result<()> {
        tick(until)?;
        self.node.check()?;
        tick(until)
    }
    fn history(&self, entries: &[Completed]) -> Result<()> {
        if self.origin.as_ref().is_some_and(|origin| {
            !entries
                .iter()
                .any(|entry| entry.manifest.request.operation == *origin)
        }) {
            return Err(Error::Refused);
        }
        Ok(())
    }
}

struct Manifest {
    request: UpgradeRequest,
    old: PersistentId,
    new: PersistentId,
    old_raw: Vec<u8>,
    new_raw: Vec<u8>,
}
impl Manifest {
    fn bytes(&self) -> Vec<u8> {
        format!(
            "omavless-product-upgrade-manifest-v2\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            self.request.uid,
            self.request.operation,
            encode(&self.request.predecessor),
            encode(&self.request.core),
            encode(&self.request.runtime),
            self.old.text(),
            self.new.text(),
            encode(&self.old_raw),
            encode(&self.new_raw)
        )
        .into_bytes()
    }
    fn parse(raw: &[u8], tree: &Tree, owner: (u32, u32)) -> Result<Self> {
        let text = std::str::from_utf8(raw).map_err(|_| Error::Refused)?;
        let f: Vec<_> = text.split('\n').collect();
        if f.len() != 11 || f[0] != "omavless-product-upgrade-manifest-v2" || !f[10].is_empty() {
            return Err(Error::Refused);
        }
        let request = UpgradeRequest::parse(f[1], f[3], f[4], f[5])?;
        let old = PersistentId::parse(f[6])?;
        let new = PersistentId::parse(f[7])?;
        let old_raw = decode(f[8], 512)?;
        let new_raw = decode(f[9], 512)?;
        old.regular(owner, 0o600, 512)?;
        new.regular(owner, 0o600, 512)?;
        if old.filesystem != tree.record.filesystem
            || new.filesystem != tree.record.filesystem
            || f[2] != request.operation
            || payload(&old_raw)?.0 != request.uid
            || old.size != old_raw.len() as u64
            || new.size != new_raw.len() as u64
            || new_raw != request.payload()
            || old_raw == new_raw
            || tree.seal_for(old, &old_raw) != request.predecessor
        {
            return Err(Error::Refused);
        }
        Ok(Self {
            request,
            old,
            new,
            old_raw,
            new_raw,
        })
    }
    fn claim(&self) -> Vec<u8> {
        format!(
            "omavless-product-upgrade-attempt-v2\n{}\n{}\n",
            self.request.operation,
            encode(&digest(&self.bytes()))
        )
        .into_bytes()
    }
    fn completion(&self, old: PersistentId, new: PersistentId) -> Vec<u8> {
        format!(
            "omavless-product-upgrade-complete-v2\n{}\n{}\n{}\n{}\n",
            self.request.operation,
            encode(&digest(&self.bytes())),
            old.text(),
            new.text()
        )
        .into_bytes()
    }
}
struct Completed {
    manifest: Manifest,
    old_after: PersistentId,
    new_after: PersistentId,
}
fn read_completed(
    tree: &Tree,
    history: &Directory,
    operation: &str,
    layout: &Layout,
    until: Instant,
) -> Result<Completed> {
    hash_arg(operation)?;
    let slot = history.child(operation, layout.owner)?;
    exact_names(&slot, &["manifest", "swap", "attempt", "complete"], until)?;
    let manifest_node = Node::open(&slot, "manifest", layout.owner, 0o600, 4096)?;
    let manifest = Manifest::parse(&manifest_node.bytes(4096)?, tree, layout.owner)?;
    if manifest.request.operation != operation {
        return Err(Error::Refused);
    }
    let claim = Node::open(&slot, "attempt", layout.owner, 0o600, 256)?;
    if claim.bytes(256)? != manifest.claim() {
        return Err(Error::Refused);
    }
    let complete = Node::open(&slot, "complete", layout.owner, 0o600, 1024)?;
    let raw = complete.bytes(1024)?;
    let text = std::str::from_utf8(&raw).map_err(|_| Error::Refused)?;
    let fields: Vec<_> = text.split('\n').collect();
    if fields.len() != 6 {
        return Err(Error::Refused);
    }
    let old_after = PersistentId::parse(fields[3])?;
    let new_after = PersistentId::parse(fields[4])?;
    if raw != manifest.completion(old_after, new_after)
        || !old_after.renamed_from(manifest.old)
        || !new_after.renamed_from(manifest.new)
    {
        return Err(Error::Refused);
    }
    let old = Node::open(&slot, "swap", layout.owner, 0o600, 512)?;
    if old.persistent() != old_after || old.bytes(512)? != manifest.old_raw {
        return Err(Error::Refused);
    }
    for node in [&manifest_node, &claim, &complete, &old] {
        node.check()?;
    }
    slot.check()?;
    tick(until)?;
    Ok(Completed {
        manifest,
        old_after,
        new_after,
    })
}
fn history(tree: &Tree, layout: &Layout, until: Instant) -> Result<Vec<Completed>> {
    tree.shape(until)?;
    if !absent(tree.directory(), PENDING)? {
        return Err(Error::Refused);
    }
    if absent(tree.directory(), HISTORY)? {
        return Ok(Vec::new());
    }
    let directory = tree.directory().child(HISTORY, layout.owner)?;
    let entries = names(&directory, MAX_HISTORY, until)?;
    if entries.is_empty() {
        return Err(Error::Refused);
    }
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        result.push(read_completed(tree, &directory, &entry, layout, until)?);
    }
    // Every completed generation must be on ONE unbranched chain ending at the
    // current exact record. A later owned exchange updates ctime in its own new
    // completion, while preserving each immutable OriginalBefore manifest.
    let mut terminals = 0;
    let mut beginnings = 0;
    for (index, item) in result.iter().enumerate() {
        tick(until)?;
        let successors = result
            .iter()
            .enumerate()
            .filter(|(j, next)| {
                *j != index
                    && next.manifest.old == item.new_after
                    && next.manifest.old_raw == item.manifest.new_raw
            })
            .count();
        let predecessors = result
            .iter()
            .enumerate()
            .filter(|(j, previous)| {
                *j != index
                    && previous.new_after == item.manifest.old
                    && previous.manifest.new_raw == item.manifest.old_raw
            })
            .count();
        if successors > 1 || predecessors > 1 {
            return Err(Error::Refused);
        }
        if predecessors == 0 {
            beginnings += 1;
        }
        if successors == 0 {
            terminals += 1;
            if tree.record.persistent() != item.new_after || tree.raw != item.manifest.new_raw {
                return Err(Error::Refused);
            }
        }
        if item.manifest.request.uid != payload(&tree.raw)?.0 {
            return Err(Error::Refused);
        }
        // Reading this field also documents that predecessor custody remains
        // part of each accepted entry, not only its content-hash graph edge.
        if item.old_after.ino != item.manifest.old.ino {
            return Err(Error::Refused);
        }
    }
    if terminals != 1 || beginnings != 1 {
        return Err(Error::Refused);
    }
    // Exclude a disconnected cycle alongside the valid beginning/end chain.
    let mut current = result
        .iter()
        .position(|item| item.new_after == tree.record.persistent())
        .ok_or(Error::Refused)?;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current) {
            return Err(Error::Refused);
        }
        match result.iter().position(|previous| {
            previous.new_after == result[current].manifest.old
                && previous.manifest.new_raw == result[current].manifest.old_raw
        }) {
            Some(previous) => current = previous,
            None => break,
        }
    }
    if seen.len() != result.len() {
        return Err(Error::Refused);
    }
    directory.check()?;
    tree.check(until)?;
    Ok(result)
}

/// Read-only admission before fresh enrollment's first mkdir. The caller must
/// use the SAME original deadline and call this immediately before that mkdir.
/// No full image hashes are repeated; the existing EnrollmentInputs and later
/// EnrollmentLease retain their independent image and completion checks.
pub(crate) fn preflight_enrollment_filesystems(until: Instant) -> Result<()> {
    preflight_enrollment_filesystems_at(&Layout::production(), until)
}
fn preflight_enrollment_filesystems_at(layout: &Layout, until: Instant) -> Result<()> {
    layout.authorize()?;
    let parent = Chain::capture(
        layout,
        layout.directory.parent().ok_or(Error::Refused)?,
        until,
    )?;
    let mut images = Vec::with_capacity(3);
    for path in [&layout.core, &layout.runtime, &layout.helper] {
        tick(until)?;
        let chain = Chain::capture(layout, path.parent().ok_or(Error::Refused)?, until)?;
        let node = Node::open(
            chain.last(),
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or(Error::Refused)?,
            layout.owner,
            0o755,
            MAX_IMAGE,
        )?;
        images.push((chain, node));
    }
    #[cfg(test)]
    let fixture = layout.fixture;
    #[cfg(not(test))]
    let fixture = false;
    if !fixture {
        let original = File::open("/proc/self/exe").map_err(|_| Error::Unavailable)?;
        if Id::of(&original.metadata().map_err(|_| Error::Unavailable)?)
            != images.last().ok_or(Error::Refused)?.1.id
        {
            return Err(Error::Refused);
        }
    }
    for (chain, node) in &images {
        chain.check(until)?;
        node.check()?;
    }
    parent.check(until)?;
    layout.authorize()?;
    tick(until)
}

/// Create only during already-qualified explicit fresh enrollment. This is not
/// a provider repair path; an existing lock or any foreign entry refuses.
pub(crate) struct EnrollmentLease {
    chain: Chain,
    administrator: Administrator,
    layout: Layout,
    lock: Lock,
}
impl EnrollmentLease {
    pub(crate) fn check(&self, until: Instant) -> Result<()> {
        self.administrator.check(&self.layout, until)?;
        self.chain.check(until)?;
        self.lock.check(until)?;
        exact_names(self.chain.last(), &[RECORD, LOCK], until)?;
        let tree = Tree::capture(&self.layout, until)?;
        payload(&tree.raw)?;
        self.layout.no_socket(until)?;
        tree.check(until)
    }
}
pub(crate) fn initialize_enrollment_lock(
    directory: &File,
    until: Instant,
) -> Result<EnrollmentLease> {
    root()?;
    initialize_enrollment_lock_at(Layout::production(), directory, until)
}
fn initialize_enrollment_lock_at(
    layout: Layout,
    directory: &File,
    until: Instant,
) -> Result<EnrollmentLease> {
    let administrator = Administrator::capture(&layout, until)?;
    let chain = Chain::capture(&layout, &layout.directory, until)?;
    if Id::of(&directory.metadata().map_err(|_| Error::Unavailable)?)
        != Id::of(
            &chain
                .last()
                .file
                .metadata()
                .map_err(|_| Error::Unavailable)?,
        )
    {
        return Err(Error::Refused);
    }
    if chain.last().id.4 & 0o7777 != 0o700 {
        return Err(Error::Refused);
    }
    // The fresh writer calls this BEFORE exclusive record creation. An existing
    // record belongs to upgrade's bootstrap-origin path, never this initializer.
    exact_names(chain.last(), &[], until)?;
    layout.no_socket(until)?;
    let node = Node::create(chain.last(), LOCK, layout.owner, &lock_payload(None))?;
    chain.last().sync()?;
    node.check()?;
    let lock = Lock::acquire(chain.last(), layout.owner, true, until)?;
    administrator.check(&layout, until)?;
    chain.check(until)?;
    Ok(EnrollmentLease {
        chain,
        administrator,
        layout,
        lock,
    })
}

/// Declare this before provider Roots/listener so it drops after them. Within
/// the lease, field order closes tree descriptors before the shared lock itself.
pub(crate) struct ProductLease {
    tree: Tree,
    layout: Layout,
    lock: Lock,
}
impl ProductLease {
    pub(crate) fn acquire(until: Instant) -> Result<Self> {
        let layout = Layout::production();
        layout.authorize()?;
        Self::acquire_at(layout, until)
    }
    fn acquire_at(layout: Layout, until: Instant) -> Result<Self> {
        // Lock before reading even the current enrollment. No create fallback.
        let chain = Chain::capture(&layout, &layout.directory, until)?;
        let lock = Lock::acquire(chain.last(), layout.owner, false, until)?;
        let tree = Tree::capture(&layout, until)?;
        if tree.directory().id != chain.last().id {
            return Err(Error::Refused);
        }
        let result = Self { tree, layout, lock };
        result.check(until)?;
        Ok(result)
    }
    pub(crate) fn check(&self, until: Instant) -> Result<()> {
        self.lock.check(until)?;
        self.tree.check(until)?;
        self.lock
            .history(&history(&self.tree, &self.layout, until)?)?;
        self.lock.check(until)
    }
}

pub fn enrollment_seal(uid: &str) -> Result<String> {
    let uid = crate::enrollment::uid(uid)?;
    let until = Instant::now() + BUDGET;
    let layout = Layout::production();
    let administrator = Administrator::capture(&layout, until)?;
    let result = seal_at(&layout, uid, until)?;
    administrator.check(&layout, until)?;
    Ok(result)
}
fn seal_at(layout: &Layout, uid: u32, until: Instant) -> Result<String> {
    let chain = Chain::capture(layout, &layout.directory, until)?;
    let lock = if absent(chain.last(), LOCK)? {
        None
    } else {
        Some(Lock::acquire(chain.last(), layout.owner, false, until)?)
    };
    let tree = Tree::capture(layout, until)?;
    if tree.directory().id != chain.last().id || payload(&tree.raw)?.0 != uid {
        return Err(Error::Refused);
    }
    if lock.is_none() {
        exact_names(tree.directory(), &[RECORD], until)?;
    }
    let entries = history(&tree, layout, until)?;
    if let Some(lock) = &lock {
        lock.history(&entries)?;
    }
    tree.check(until)?;
    if let Some(lock) = &lock {
        lock.check(until)?;
    }
    Ok(encode(&tree.seal()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Lock,
    Pending,
    History,
    Slot,
    Staged,
    Manifest,
    Attempt,
    Exchange,
    ExchangeSync,
    Complete,
    Unlink,
    FinalSync,
}

pub struct PreparedUpgrade {
    request: UpgradeRequest,
    layout: Layout,
    until: Instant,
    administrator: Administrator,
    tree: Tree,
    core: Image,
    runtime: Image,
    // Last: lock survives all captured files until consumed/drop completes.
    lock: Option<Lock>,
    #[cfg(test)]
    fault: Option<Cut>,
}
pub fn prepare_upgrade(request: UpgradeRequest) -> Result<PreparedUpgrade> {
    PreparedUpgrade::prepare(request, Layout::production(), Instant::now() + BUDGET)
}
impl PreparedUpgrade {
    fn prepare(request: UpgradeRequest, layout: Layout, until: Instant) -> Result<Self> {
        let administrator = Administrator::capture(&layout, until)?;
        let tree = Tree::capture(&layout, until)?;
        if tree.seal() != request.predecessor || payload(&tree.raw)?.0 != request.uid {
            return Err(Error::Refused);
        }
        let core = Image::capture(&layout, &layout.core, until)?;
        let runtime = Image::capture(&layout, &layout.runtime, until)?;
        layout.no_socket(until)?;
        if core.hash != request.core
            || runtime.hash != request.runtime
            || tree.raw == request.payload()
        {
            return Err(Error::Refused);
        }
        let lock = if absent(tree.directory(), LOCK)? {
            exact_names(tree.directory(), &[RECORD], until)?;
            None
        } else {
            Some(Lock::acquire(tree.directory(), layout.owner, true, until)?)
        };
        let result = Self {
            request,
            layout,
            until,
            administrator,
            tree,
            core,
            runtime,
            lock,
            #[cfg(test)]
            fault: None,
        };
        result.check_before()?;
        Ok(result)
    }
    pub fn operation_id(&self) -> &str {
        self.request.operation_id()
    }
    fn cut(&self, cut: Cut) -> Result<()> {
        #[cfg(test)]
        if self.fault == Some(cut) {
            return Err(Error::Unavailable);
        }
        #[cfg(not(test))]
        let _ = cut;
        tick(self.until)
    }
    fn check_inputs(&self) -> Result<()> {
        self.administrator.check(&self.layout, self.until)?;
        self.core.check(self.until)?;
        self.runtime.check(self.until)?;
        self.tree.chain.check(self.until)?;
        self.layout.no_socket(self.until)?;
        if let Some(lock) = &self.lock {
            lock.check(self.until)?;
        }
        tick(self.until)
    }
    fn check_before(&self) -> Result<()> {
        self.check_inputs()?;
        self.tree.check(self.until)?;
        if self.tree.seal() != self.request.predecessor {
            return Err(Error::Refused);
        }
        let entries = history(&self.tree, &self.layout, self.until)?;
        if let Some(lock) = &self.lock {
            lock.history(&entries)?;
        }
        if entries.len() == MAX_HISTORY
            || entries
                .iter()
                .any(|e| e.manifest.request.operation == self.request.operation)
        {
            return Err(Error::Refused);
        }
        if self.lock.is_none() {
            exact_names(self.tree.directory(), &[RECORD], self.until)?;
        }
        Ok(())
    }
    pub fn commit(mut self) -> Result<()> {
        self.check_before()?;
        let bootstrap = self.lock.is_none();
        if self.lock.is_none() {
            // The first filesystem object already records this operation ID.
            // A crash here is observable and cannot masquerade as a fresh lock.
            let node = Node::create(
                self.tree.directory(),
                LOCK,
                self.layout.owner,
                &lock_payload(Some(self.request.operation_id())),
            )?;
            self.tree.directory().sync()?;
            node.check()?;
            self.lock = Some(Lock::acquire(
                self.tree.directory(),
                self.layout.owner,
                true,
                self.until,
            )?);
            self.cut(Cut::Lock)?;
        }
        if bootstrap {
            self.check_inputs()?;
            self.tree.check(self.until)?;
            exact_names(self.tree.directory(), &[RECORD, LOCK], self.until)?;
            if self.lock.as_ref().and_then(|lock| lock.origin.as_deref())
                != Some(self.request.operation_id())
            {
                return Err(Error::Refused);
            }
        } else {
            self.check_before()?;
        }
        let pending_raw = format!("{}\n", self.request.operation);
        let pending = Node::create(
            self.tree.directory(),
            PENDING,
            self.layout.owner,
            pending_raw.as_bytes(),
        )?;
        self.tree.directory().sync()?;
        self.cut(Cut::Pending)?;
        if absent(self.tree.directory(), HISTORY)? {
            fs::mkdirat(&self.tree.directory().file, HISTORY, Mode::RWXU)
                .map_err(|_| Error::Unavailable)?;
            self.tree.directory().sync()?;
        }
        let journal = self.tree.directory().child(HISTORY, self.layout.owner)?;
        self.cut(Cut::History)?;
        fs::mkdirat(&journal.file, self.request.operation_id(), Mode::RWXU)
            .map_err(|_| Error::Unavailable)?;
        journal.sync()?;
        let slot = journal.child(self.request.operation_id(), self.layout.owner)?;
        self.cut(Cut::Slot)?;
        let next_raw = self.request.payload();
        let next = Node::create(&slot, "swap", self.layout.owner, &next_raw)?;
        slot.sync()?;
        self.cut(Cut::Staged)?;
        let manifest = Manifest {
            request: self.request.clone(),
            old: self.tree.record.persistent(),
            new: next.persistent(),
            old_raw: self.tree.raw.clone(),
            new_raw: next_raw.clone(),
        };
        let manifest_node = Node::create(&slot, "manifest", self.layout.owner, &manifest.bytes())?;
        slot.sync()?;
        self.cut(Cut::Manifest)?;
        let attempt = Node::create(&slot, "attempt", self.layout.owner, &manifest.claim())?;
        slot.sync()?;
        journal.sync()?;
        self.tree.directory().sync()?;
        self.cut(Cut::Attempt)?;
        self.check_inputs()?;
        self.tree.check(self.until)?;
        for node in [&pending, &next, &manifest_node, &attempt] {
            node.check()?;
        }
        slot.check()?;
        journal.check()?;
        tick(self.until)?;
        // The sole publication syscall. No error or inspection path reaches it.
        fs::renameat_with(
            &self.tree.directory().file,
            RECORD,
            &slot.file,
            "swap",
            RenameFlags::EXCHANGE,
        )
        .map_err(|_| Error::Unavailable)?;
        self.cut(Cut::Exchange)?;
        let old_after = Id::of(
            &self
                .tree
                .record
                .file
                .metadata()
                .map_err(|_| Error::Unavailable)?,
        );
        let new_after = Id::of(&next.file.metadata().map_err(|_| Error::Unavailable)?);
        // Full live rename checks retain st_dev. Only the persisted projection
        // omits it; filesystem identity is descriptor-observed and checked again.
        if !old_after.renamed_from(self.tree.record.id)
            || !new_after.renamed_from(next.id)
            || !PersistentId::of(self.tree.record.filesystem, old_after).renamed_from(manifest.old)
            || !PersistentId::of(next.filesystem, new_after).renamed_from(manifest.new)
            || self.tree.record.bytes(512)? != manifest.old_raw
            || next.bytes(512)? != next_raw
        {
            return Err(Error::Refused);
        }
        self.tree
            .record
            .check_at(&slot.path.join("swap"), old_after)?;
        next.check_at(&self.tree.record.path, new_after)?;
        self.tree
            .record
            .file
            .sync_all()
            .map_err(|_| Error::Unavailable)?;
        next.file.sync_all().map_err(|_| Error::Unavailable)?;
        slot.sync()?;
        self.tree.directory().sync()?;
        self.cut(Cut::ExchangeSync)?;
        let complete = Node::create(
            &slot,
            "complete",
            self.layout.owner,
            &manifest.completion(
                PersistentId::of(self.tree.record.filesystem, old_after),
                PersistentId::of(next.filesystem, new_after),
            ),
        )?;
        slot.sync()?;
        journal.sync()?;
        self.tree.directory().sync()?;
        self.cut(Cut::Complete)?;
        self.check_inputs()?;
        for node in [&pending, &manifest_node, &attempt, &complete] {
            node.check()?;
        }
        self.tree
            .record
            .check_at(&slot.path.join("swap"), old_after)?;
        next.check_at(&self.tree.record.path, new_after)?;
        slot.check()?;
        journal.check()?;
        tick(self.until)?;
        fs::unlinkat(&self.tree.directory().file, PENDING, AtFlags::empty())
            .map_err(|_| Error::Unavailable)?;
        self.cut(Cut::Unlink)?;
        self.tree.directory().sync()?;
        self.cut(Cut::FinalSync)?;
        self.check_inputs()?;
        let current = Tree::capture(&self.layout, self.until)?;
        let entries = history(&current, &self.layout, self.until)?;
        self.lock
            .as_ref()
            .ok_or(Error::Refused)?
            .history(&entries)?;
        if current.record.id != new_after || current.raw != next_raw {
            return Err(Error::Refused);
        }
        tick(self.until)
    }
}

pub fn inspect_upgrade(uid: &str, operation: &str) -> Result<Inspection> {
    let uid = crate::enrollment::uid(uid)?;
    hash_arg(operation)?;
    let layout = Layout::production();
    let until = Instant::now() + BUDGET;
    let administrator = Administrator::capture(&layout, until)?;
    let result = inspect_at(&layout, uid, operation, until)?;
    administrator.check(&layout, until)?;
    Ok(result)
}
fn inspect_at(layout: &Layout, uid: u32, operation: &str, until: Instant) -> Result<Inspection> {
    hash_arg(operation)?;
    let chain = Chain::capture(layout, &layout.directory, until)?;
    let lock = if absent(chain.last(), LOCK)? {
        None
    } else {
        Some(Lock::acquire(chain.last(), layout.owner, false, until)?)
    };
    let tree = Tree::capture(layout, until)?;
    if tree.directory().id != chain.last().id || payload(&tree.raw)?.0 != uid {
        return Err(Error::Refused);
    }
    tree.shape(until)?;
    let disposition = if lock.is_none() {
        exact_names(tree.directory(), &[RECORD], until)?;
        Inspection::NotRecorded
    } else if !absent(tree.directory(), PENDING)? {
        let pending = Node::open(tree.directory(), PENDING, layout.owner, 0o600, 65)?;
        if pending.bytes(65)? != format!("{operation}\n").as_bytes() {
            return Err(Error::Refused);
        }
        pending.check()?;
        Inspection::Incomplete
    } else {
        let entries = history(&tree, layout, until)?;
        let held = lock.as_ref().ok_or(Error::Refused)?;
        if let Some(origin) = &held.origin
            && !entries
                .iter()
                .any(|entry| entry.manifest.request.operation == *origin)
        {
            if origin != operation || !entries.is_empty() {
                return Err(Error::Refused);
            }
            Inspection::Incomplete
        } else if entries
            .iter()
            .any(|entry| entry.manifest.request.operation == operation)
        {
            Inspection::ObservedCommitted
        } else {
            Inspection::NotRecorded
        }
    };
    // Only a committed observation requires the current images; pending/absent
    // inspection must still work after package replacement or an interrupted cut.
    let images = if disposition == Inspection::ObservedCommitted {
        let (_, core_hash, runtime_hash) = payload(&tree.raw)?;
        let core = Image::capture(layout, &layout.core, until)?;
        let runtime = Image::capture(layout, &layout.runtime, until)?;
        if core.hash != core_hash || runtime.hash != runtime_hash {
            return Err(Error::Refused);
        }
        Some((core, runtime))
    } else {
        None
    };
    // One final retained-object/deadline path for EVERY successful disposition.
    // No early NotRecorded/Incomplete result bypasses lock or ancestry checks.
    if let Some((core, runtime)) = &images {
        core.check(until)?;
        runtime.check(until)?;
    }
    tree.check(until)?;
    if let Some(lock) = &lock {
        lock.check(until)?;
    } else {
        exact_names(tree.directory(), &[RECORD], until)?;
    }
    chain.check(until)?;
    tick(until)?;
    Ok(disposition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        sync::atomic::{AtomicU64, Ordering},
    };

    const ROOT_REBOOT_ENV: &str = "OMAVLESS_IMAGE_UPGRADE_REBOOT_FIXTURE";
    const ROOT_REBOOT_PREFIX: &str = "omavless-upgrade-reboot-fixture-";
    const ROOT_REBOOT_EXPECTED: &str = "reboot.expected";
    fn root_reboot_path() -> PathBuf {
        root().expect("explicit inert fixture requires actual UID/EUID/GID/EGID zero");
        let path = PathBuf::from(
            std::env::var_os(ROOT_REBOOT_ENV).expect("explicit PUBLIC fixture pathname required"),
        );
        assert_eq!(path.parent(), Some(Path::new("/var/lib")));
        let suffix = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix(ROOT_REBOOT_PREFIX)
            .unwrap();
        assert!((16..=32).contains(&suffix.len()));
        assert!(
            suffix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        path
    }
    fn root_reboot_layout(path: &Path) -> Layout {
        Layout {
            anchor: PathBuf::from("/"),
            directory: path.join("product"),
            core: path.join("images/core"),
            runtime: path.join("images/runtime"),
            helper: path.join("images/helper"),
            socket: path.join("socket/control.sock"),
            owner: (0, 0),
            fixture: true,
            fixture_profile: FsProfile::Btrfs,
        }
    }
    fn root_reboot_boot_id() -> String {
        let file = File::open("/proc/sys/kernel/random/boot_id").unwrap();
        let mut raw = [0; 64];
        let n = file.read_at(&mut raw, 0).unwrap();
        assert_eq!(n, 37);
        assert_eq!(raw[36], b'\n');
        for (index, byte) in raw[..36].iter().copied().enumerate() {
            assert!(if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            });
        }
        String::from_utf8(raw[..36].to_vec()).unwrap()
    }
    #[test]
    #[ignore = "explicit same-VM ROOT inert fixture preparation; retain objects for actual reboot"]
    fn root_reboot_prepare_public_fixture() {
        let path = root_reboot_path();
        let layout = root_reboot_layout(&path);
        // Qualify the real parent filesystem BEFORE the first create; the root
        // fixture still does not use the installed provider namespace/images.
        Chain::capture(&layout, Path::new("/var/lib"), Instant::now() + BUDGET).unwrap();
        std::fs::create_dir(&path).expect("create-only fixture; never resume or overwrite");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        for directory in [
            &layout.directory,
            layout.core.parent().unwrap(),
            layout.socket.parent().unwrap(),
        ] {
            std::fs::create_dir(directory).unwrap();
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        for (image, tag) in [(&layout.core, 1), (&layout.runtime, 2), (&layout.helper, 3)] {
            Fixture::elf(image, tag);
            File::open(image).unwrap().sync_all().unwrap();
        }
        let old_raw =
            format!("{SCHEMA}\n1000\n{}\n{}\n", "4".repeat(64), "5".repeat(64)).into_bytes();
        Fixture::write(&layout.directory.join(RECORD), &old_raw, 0o600);
        let f = Fixture {
            layout,
            cleanup: None,
        };
        let request = f.request();
        let operation = request.operation.clone();
        f.prepare(request).unwrap().commit().unwrap();
        assert_eq!(
            f.inspect(&operation).unwrap(),
            Inspection::ObservedCommitted
        );
        let until = Instant::now() + BUDGET;
        let tree = Tree::capture(&f.layout, until).unwrap();
        let entries = history(&tree, &f.layout, until).unwrap();
        assert_eq!(entries.len(), 1);
        let expected = format!(
            "omavless-product-root-reboot-fixture-v2\n{}\n{}\n{}\n{}\n{}\n{}\n",
            root_reboot_boot_id(),
            operation,
            encode(&tree.seal()),
            entries[0].old_after.text(),
            encode(&old_raw),
            tree.record.id.dev
        );
        let anchor = Chain::capture(&f.layout, &path, until).unwrap();
        Node::create(
            anchor.last(),
            ROOT_REBOOT_EXPECTED,
            (0, 0),
            expected.as_bytes(),
        )
        .unwrap();
        anchor.last().sync().unwrap();
        for directory in [path.join("images"), path.join("socket"), path.clone()] {
            File::open(directory).unwrap().sync_all().unwrap();
        }
        File::open("/var/lib").unwrap().sync_all().unwrap();
        let lease = f.lease().unwrap();
        lease.check(until).unwrap();
        drop(lease);
        println!(
            "ROOT_PUBLIC_REBOOT_FIXTURE_PREPARED; actual reboot and original inspection still required"
        );
        // Deliberately no cleanup, reboot, service action or production selector.
    }
    #[test]
    #[ignore = "explicit read-only inspection of SAME retained public ROOT fixture after actual reboot"]
    fn root_reboot_inspect_same_public_fixture() {
        let path = root_reboot_path();
        let f = Fixture {
            layout: root_reboot_layout(&path),
            cleanup: None,
        };
        let until = Instant::now() + BUDGET;
        let anchor = Chain::capture(&f.layout, &path, until).unwrap();
        let expected =
            Node::open(anchor.last(), ROOT_REBOOT_EXPECTED, (0, 0), 0o600, 4096).unwrap();
        let raw = expected.bytes(4096).unwrap();
        let text = std::str::from_utf8(&raw).unwrap();
        let fields: Vec<_> = text.split('\n').collect();
        assert_eq!(fields.len(), 8);
        assert_eq!(fields[0], "omavless-product-root-reboot-fixture-v2");
        assert!(fields[7].is_empty());
        assert_ne!(
            fields[1],
            root_reboot_boot_id(),
            "same-boot inspection is not reboot qualification"
        );
        hash_arg(fields[2]).unwrap();
        hash_arg(fields[3]).unwrap();
        let old_after = PersistentId::parse(fields[4]).unwrap();
        let old_raw = decode(fields[5], 512).unwrap();
        let prior_device = fields[6].parse::<u64>().unwrap();
        assert_eq!(prior_device.to_string(), fields[6]);
        assert_eq!(f.inspect(fields[2]).unwrap(), Inspection::ObservedCommitted);
        let tree = Tree::capture(&f.layout, until).unwrap();
        assert_eq!(encode(&tree.seal()), fields[3]);
        let entries = history(&tree, &f.layout, until).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].manifest.request.operation, fields[2]);
        assert_eq!(entries[0].old_after, old_after);
        assert_eq!(entries[0].manifest.old_raw, old_raw);
        let lease = f.lease().unwrap();
        lease.check(until).unwrap();
        drop(lease);
        expected.check().unwrap();
        anchor.check(until).unwrap();
        println!(
            "ROOT_PUBLIC_REBOOT_FIXTURE_OBSERVED_COMMITTED; device_number_changed={}",
            tree.record.id.dev != prior_device
        );
        // Read-only disposition; no new request/commit, repair, deletion or ACK
        // substitution for the original prepare exit. Parent retains originals.
    }
    #[test]
    fn fresh_filesystem_preflight_is_readonly_when_namespace_is_absent() {
        let f = Fixture::new();
        std::fs::remove_file(f.layout.directory.join(RECORD)).unwrap();
        std::fs::remove_dir(&f.layout.directory).unwrap();
        preflight_enrollment_filesystems_at(&f.layout, Instant::now() + BUDGET).unwrap();
        assert!(!f.layout.directory.exists());
    }

    #[test]
    fn persisted_identity_recaptures_device_renumbering_without_relaxing_live_identity() {
        let f = Fixture::new();
        let tree = Tree::capture(&f.layout, Instant::now() + BUDGET).unwrap();
        let current = tree.record.id;
        let earlier_boot = Id {
            dev: current.dev ^ 0x10000,
            ..current
        };
        assert_ne!(earlier_boot, current);
        // A synthetic earlier device assignment is serialized by the actual
        // production durable codec, then compared with the actual current node.
        // This is a source regression, NOT an actual reboot qualification.
        let stored = PersistentId::of(tree.record.filesystem, earlier_boot).text();
        let restored = PersistentId::parse(&stored).unwrap();
        assert_eq!(restored, tree.record.persistent());
        assert!(
            tree.record
                .check_at(&tree.record.path, earlier_boot)
                .is_err()
        );
        assert!(tree.check(Instant::now() + BUDGET).is_ok());
        let base = restored;
        for changed in [
            PersistentId {
                filesystem: Filesystem {
                    fsid: base.filesystem.fsid ^ 1,
                    ..base.filesystem
                },
                ..base
            },
            PersistentId {
                filesystem: Filesystem {
                    kind: base.filesystem.kind ^ 1,
                    ..base.filesystem
                },
                ..base
            },
            PersistentId {
                ino: base.ino ^ 1,
                ..base
            },
            PersistentId {
                uid: base.uid ^ 1,
                ..base
            },
            PersistentId {
                gid: base.gid ^ 1,
                ..base
            },
            PersistentId {
                mode: base.mode ^ 1,
                ..base
            },
            PersistentId {
                links: base.links + 1,
                ..base
            },
            PersistentId {
                size: base.size + 1,
                ..base
            },
            PersistentId {
                mtime: base.mtime + 1,
                ..base
            },
            PersistentId {
                mtime_ns: base.mtime_ns ^ 1,
                ..base
            },
            PersistentId {
                ctime: base.ctime + 1,
                ..base
            },
            PersistentId {
                ctime_ns: base.ctime_ns ^ 1,
                ..base
            },
        ] {
            assert_ne!(changed, tree.record.persistent());
            assert_ne!(changed.text(), stored);
        }
    }
    #[test]
    fn ancestry_and_record_seal_pin_fsid_but_not_ephemeral_devices() {
        let f = Fixture::new();
        let mut tree = Tree::capture(&f.layout, Instant::now() + BUDGET).unwrap();
        let current_seal = tree.seal();
        let original_device = tree.record.id.dev;
        for directory in &mut tree.chain.directories {
            directory.id.0 ^= 0x10000;
        }
        tree.record.id.dev ^= 0x10000;
        assert_eq!(tree.seal(), current_seal);
        assert!(tree.check(Instant::now() + BUDGET).is_err());
        // Restore the live observations; no file or saved journal is rewritten.
        for directory in &mut tree.chain.directories {
            directory.id.0 ^= 0x10000;
        }
        tree.record.id.dev = original_device;
        assert!(tree.check(Instant::now() + BUDGET).is_ok());
        tree.chain.directories[0].filesystem.fsid ^= 1;
        assert_ne!(tree.seal(), current_seal);
        tree.chain.directories[0].filesystem.fsid ^= 1;
        tree.record.filesystem.fsid ^= 1;
        assert_ne!(tree.seal(), current_seal);
    }
    #[test]
    fn btrfs_profile_is_explicit_and_zero_or_other_filesystems_refuse() {
        assert!(
            Filesystem {
                kind: BTRFS_MAGIC,
                fsid: 1
            }
            .admit(FsProfile::Btrfs)
            .is_ok()
        );
        for filesystem in [
            Filesystem {
                kind: BTRFS_MAGIC,
                fsid: 0,
            },
            Filesystem {
                kind: 0xef53,
                fsid: 1,
            },
            Filesystem {
                kind: 0x0102_1994,
                fsid: 1,
            },
            Filesystem { kind: 0, fsid: 1 },
        ] {
            assert_eq!(filesystem.admit(FsProfile::Btrfs), Err(Error::Refused));
        }
        assert_eq!(Layout::production().profile(), FsProfile::Btrfs);
    }
    #[test]
    fn manifest_requires_current_persistent_filesystem_and_new_domain() {
        let f = Fixture::new();
        let request = f.request();
        let operation = request.operation.clone();
        f.prepare(request).unwrap().commit().unwrap();
        let until = Instant::now() + BUDGET;
        let tree = Tree::capture(&f.layout, until).unwrap();
        let raw = std::fs::read(
            f.layout
                .directory
                .join(HISTORY)
                .join(operation)
                .join("manifest"),
        )
        .unwrap();
        let manifest = Manifest::parse(&raw, &tree, f.layout.owner).unwrap();
        assert_eq!(manifest.old.filesystem, tree.record.filesystem);
        assert_eq!(manifest.new.filesystem, tree.record.filesystem);
        let legacy = String::from_utf8(raw.clone()).unwrap().replacen(
            "omavless-product-upgrade-manifest-v2",
            "omavless-product-upgrade-manifest-v1",
            1,
        );
        assert!(Manifest::parse(legacy.as_bytes(), &tree, f.layout.owner).is_err());
        let mut foreign = Manifest::parse(&raw, &tree, f.layout.owner).unwrap();
        foreign.new.filesystem.fsid ^= 1;
        assert!(Manifest::parse(&foreign.bytes(), &tree, f.layout.owner).is_err());
        // Current bytes, owner and inode are otherwise identical; a changed
        // descriptor filesystem identity cannot satisfy the stored history.
        let mut recaptured = Tree::capture(&f.layout, until).unwrap();
        recaptured.record.filesystem.fsid ^= 1;
        assert!(Manifest::parse(&raw, &recaptured, f.layout.owner).is_err());
    }

    #[test]
    fn upgrade_request_is_canonical_and_operation_id_precedes_mutation() {
        let predecessor = "1".repeat(64);
        let core = "2".repeat(64);
        let runtime = "3".repeat(64);
        let a = UpgradeRequest::parse("1000", &predecessor, &core, &runtime).unwrap();
        let b = UpgradeRequest::parse("1000", &predecessor, &core, &runtime).unwrap();
        assert_eq!(a.operation_id(), b.operation_id());
        assert_eq!(a.operation_id().len(), 64);
        for uid in ["0", "01000", "+1000", "1000 ", "4294967295", "/path"] {
            assert!(UpgradeRequest::parse(uid, &predecessor, &core, &runtime).is_err());
        }
        for bad in [
            "0".repeat(64),
            "a".repeat(63),
            "A".repeat(64),
            "/path".into(),
        ] {
            assert!(UpgradeRequest::parse("1000", &bad, &core, &runtime).is_err());
            assert!(UpgradeRequest::parse("1000", &predecessor, &bad, &runtime).is_err());
            assert!(UpgradeRequest::parse("1000", &predecessor, &core, &bad).is_err());
        }
        let other = UpgradeRequest::parse("1001", &predecessor, &core, &runtime).unwrap();
        assert_ne!(a.operation_id(), other.operation_id());
    }

    // These are finite owned PUBLIC fixtures using the production filesystem
    // primitives. Nonroot execution is NOT actual root/self-image admission.
    // They can separately run under guest root without a production path flag.
    struct Fixture {
        layout: Layout,
        cleanup: Option<PathBuf>,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let anchor = std::env::temp_dir().join(format!(
                "ov-image-upgrade-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&anchor).unwrap();
            std::fs::set_permissions(&anchor, std::fs::Permissions::from_mode(0o700)).unwrap();
            let owner = (
                rustix::process::getuid().as_raw(),
                rustix::process::getgid().as_raw(),
            );
            let layout = Layout {
                directory: anchor.join("product"),
                core: anchor.join("images/core"),
                runtime: anchor.join("images/runtime"),
                helper: anchor.join("images/helper"),
                socket: anchor.join("socket/control.sock"),
                anchor,
                owner,
                fixture: true,
                fixture_profile: FsProfile::Fixture,
            };
            for path in [
                &layout.directory,
                layout.core.parent().unwrap(),
                layout.socket.parent().unwrap(),
            ] {
                std::fs::create_dir(path).unwrap();
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            for (path, tag) in [(&layout.core, 1), (&layout.runtime, 2), (&layout.helper, 3)] {
                Self::elf(path, tag);
            }
            let raw = format!("{SCHEMA}\n1000\n{}\n{}\n", "4".repeat(64), "5".repeat(64));
            Self::write(&layout.directory.join(RECORD), raw.as_bytes(), 0o600);
            Self {
                cleanup: Some(layout.anchor.clone()),
                layout,
            }
        }
        fn write(path: &Path, raw: &[u8], mode: u32) {
            std::fs::write(path, raw).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        fn elf(path: &Path, tag: u8) {
            let mut raw = [0u8; 128];
            raw[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
            raw[16..18].copy_from_slice(&2u16.to_le_bytes());
            raw[18..20].copy_from_slice(
                &(if cfg!(target_arch = "x86_64") {
                    62u16
                } else {
                    183u16
                })
                .to_le_bytes(),
            );
            raw[24] = 1;
            raw[127] = tag;
            Self::write(path, &raw, 0o755);
        }
        fn request(&self) -> UpgradeRequest {
            let until = Instant::now() + BUDGET;
            let tree = Tree::capture(&self.layout, until).unwrap();
            UpgradeRequest::parse(
                "1000",
                &encode(&tree.seal()),
                &encode(&digest(&std::fs::read(&self.layout.core).unwrap())),
                &encode(&digest(&std::fs::read(&self.layout.runtime).unwrap())),
            )
            .unwrap()
        }
        fn prepare(&self, request: UpgradeRequest) -> Result<PreparedUpgrade> {
            PreparedUpgrade::prepare(request, self.layout.clone(), Instant::now() + BUDGET)
        }
        fn inspect(&self, operation: &str) -> Result<Inspection> {
            inspect_at(&self.layout, 1000, operation, Instant::now() + BUDGET)
        }
        fn lease(&self) -> Result<ProductLease> {
            ProductLease::acquire_at(self.layout.clone(), Instant::now() + BUDGET)
        }
        fn install_lock(&self) {
            let tree = Tree::capture(&self.layout, Instant::now() + BUDGET).unwrap();
            Node::create(
                tree.directory(),
                LOCK,
                self.layout.owner,
                &lock_payload(None),
            )
            .unwrap();
            tree.directory().sync().unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(path) = &self.cleanup {
                std::fs::remove_dir_all(path).unwrap();
            }
        }
    }

    #[test]
    fn preparation_is_readonly_and_commit_preserves_original_inode_and_bytes() {
        let f = Fixture::new();
        let old = std::fs::read(f.layout.directory.join(RECORD)).unwrap();
        let before = Id::of(&std::fs::metadata(f.layout.directory.join(RECORD)).unwrap());
        let request = f.request();
        let operation = request.operation.clone();
        let prepared = f.prepare(request.clone()).unwrap();
        assert_eq!(std::fs::read_dir(&f.layout.directory).unwrap().count(), 1);
        assert_eq!(f.inspect(&operation).unwrap(), Inspection::NotRecorded);
        prepared.commit().unwrap();
        let retained = f
            .layout
            .directory
            .join(HISTORY)
            .join(&operation)
            .join("swap");
        let after = Id::of(&std::fs::metadata(&retained).unwrap());
        assert!(after.renamed_from(before));
        assert_eq!(std::fs::read(retained).unwrap(), old);
        assert_eq!(
            f.inspect(&operation).unwrap(),
            Inspection::ObservedCommitted
        );
        assert!(f.prepare(request).is_err());
        assert!(f.lease().is_ok());
    }
    #[test]
    fn every_publication_cut_is_retained_and_never_reexchanged() {
        for cut in [
            Cut::Lock,
            Cut::Pending,
            Cut::History,
            Cut::Slot,
            Cut::Staged,
            Cut::Manifest,
            Cut::Attempt,
            Cut::Exchange,
            Cut::ExchangeSync,
            Cut::Complete,
            Cut::Unlink,
            Cut::FinalSync,
        ] {
            let f = Fixture::new();
            let request = f.request();
            let operation = request.operation.clone();
            let before = Id::of(&std::fs::metadata(f.layout.directory.join(RECORD)).unwrap());
            let mut prepared = f.prepare(request.clone()).unwrap();
            prepared.fault = Some(cut);
            assert_eq!(prepared.commit(), Err(Error::Unavailable), "{cut:?}");
            let disposition = f.inspect(&operation).unwrap();
            let observed = matches!(cut, Cut::Unlink | Cut::FinalSync);
            assert_eq!(
                disposition,
                if observed {
                    Inspection::ObservedCommitted
                } else {
                    Inspection::Incomplete
                },
                "{cut:?}"
            );
            if !matches!(
                cut,
                Cut::Exchange | Cut::ExchangeSync | Cut::Complete | Cut::Unlink | Cut::FinalSync
            ) {
                assert_eq!(
                    Id::of(&std::fs::metadata(f.layout.directory.join(RECORD)).unwrap()),
                    before
                );
            }
            assert!(f.prepare(request).is_err(), "{cut:?}");
            assert_eq!(f.lease().is_ok(), observed, "{cut:?}");
        }
    }
    #[test]
    fn mismatched_inputs_refuse_before_creating_lock_or_history() {
        for variant in 0..5 {
            let f = Fixture::new();
            let original = f.request();
            let request = UpgradeRequest::parse(
                if variant == 0 { "1001" } else { "1000" },
                &if variant == 1 {
                    "a".repeat(64)
                } else {
                    encode(&original.predecessor)
                },
                &if variant == 2 {
                    "b".repeat(64)
                } else {
                    encode(&original.core)
                },
                &if variant == 3 {
                    "c".repeat(64)
                } else {
                    encode(&original.runtime)
                },
            )
            .unwrap();
            if variant == 4 {
                Fixture::write(&f.layout.core, b"not an ELF", 0o755);
            }
            assert!(f.prepare(request).is_err());
            assert_eq!(std::fs::read_dir(&f.layout.directory).unwrap().count(), 1);
        }
    }
    #[test]
    fn predecessor_mismatch_refuses_before_reading_the_next_image() {
        let f = Fixture::new();
        let original = f.request();
        let request = UpgradeRequest::parse(
            "1000",
            &"a".repeat(64),
            &encode(&original.core),
            &encode(&original.runtime),
        )
        .unwrap();
        std::fs::remove_file(&f.layout.core).unwrap();
        assert!(matches!(f.prepare(request), Err(Error::Refused)));
        assert_eq!(std::fs::read_dir(&f.layout.directory).unwrap().count(), 1);
    }

    #[test]
    fn fresh_enrollment_lease_excludes_service_until_publication_and_drop() {
        let f = Fixture::new();
        let record = std::fs::read(f.layout.directory.join(RECORD)).unwrap();
        std::fs::remove_file(f.layout.directory.join(RECORD)).unwrap();
        let directory = File::open(&f.layout.directory).unwrap();
        let until = Instant::now() + BUDGET;
        let lease = initialize_enrollment_lock_at(f.layout.clone(), &directory, until).unwrap();
        assert!(f.lease().is_err());
        assert!(lease.check(until).is_err());
        Fixture::write(&f.layout.directory.join(RECORD), &record, 0o600);
        lease.check(until).unwrap();
        assert!(f.lease().is_err());
        drop(lease);
        f.lease().unwrap();
    }

    #[test]
    fn fresh_enrollment_completion_refuses_foreign_entry_and_lock_replacement() {
        let f = Fixture::new();
        let record = std::fs::read(f.layout.directory.join(RECORD)).unwrap();
        std::fs::remove_file(f.layout.directory.join(RECORD)).unwrap();
        let directory = File::open(&f.layout.directory).unwrap();
        let until = Instant::now() + BUDGET;
        let lease = initialize_enrollment_lock_at(f.layout.clone(), &directory, until).unwrap();
        Fixture::write(&f.layout.directory.join(RECORD), &record, 0o600);
        Fixture::write(&f.layout.directory.join("foreign"), b"foreign", 0o600);
        assert!(lease.check(until).is_err());
        std::fs::remove_file(f.layout.directory.join("foreign")).unwrap();
        lease.check(until).unwrap();
        std::fs::rename(
            f.layout.directory.join(LOCK),
            f.layout.directory.join("old-lock"),
        )
        .unwrap();
        Fixture::write(&f.layout.directory.join(LOCK), &lock_payload(None), 0o600);
        assert!(lease.check(until).is_err());
    }

    #[test]
    fn production_administrator_entries_refuse_nonroot_before_fixed_path_access() {
        if rustix::process::getuid().as_raw() == 0 {
            return;
        }
        assert!(matches!(enrollment_seal("1000"), Err(Error::Refused)));
        let request = Fixture::new().request();
        assert!(matches!(
            prepare_upgrade(request.clone()),
            Err(Error::Refused)
        ));
        assert!(matches!(
            inspect_upgrade("1000", request.operation_id()),
            Err(Error::Refused)
        ));
    }
    #[test]
    fn unchanged_pair_wrong_modes_symlinks_and_hardlinks_refuse() {
        for variant in 0..7 {
            let f = Fixture::new();
            if variant == 0 {
                let r = f.request();
                Fixture::write(&f.layout.directory.join(RECORD), &r.payload(), 0o600);
            }
            let request = f.request();
            match variant {
                1 => std::fs::set_permissions(
                    &f.layout.directory,
                    std::fs::Permissions::from_mode(0o777),
                )
                .unwrap(),
                2 => {
                    std::fs::rename(&f.layout.core, f.layout.core.with_extension("held")).unwrap();
                    symlink(f.layout.core.with_extension("held"), &f.layout.core).unwrap();
                }
                3 => std::fs::hard_link(&f.layout.core, f.layout.core.with_extension("link"))
                    .unwrap(),
                4 => std::fs::hard_link(
                    f.layout.directory.join(RECORD),
                    f.layout.anchor.join("record-link"),
                )
                .unwrap(),
                5 => std::fs::set_permissions(
                    &f.layout.runtime,
                    std::fs::Permissions::from_mode(0o777),
                )
                .unwrap(),
                6 => Fixture::write(&f.layout.socket, b"live or foreign socket prefix", 0o600),
                _ => (),
            }
            assert!(f.prepare(request).is_err(), "variant {variant}");
        }
    }
    #[test]
    fn retained_image_record_helper_and_lock_drift_refuse() {
        for variant in 0..4 {
            let f = Fixture::new();
            f.install_lock();
            let request = f.request();
            let prepared = f.prepare(request).unwrap();
            match variant {
                0 => Fixture::elf(&f.layout.runtime, 88),
                1 => {
                    let path = f.layout.directory.join(RECORD);
                    let bytes = std::fs::read(&path).unwrap();
                    std::fs::remove_file(&path).unwrap();
                    Fixture::write(&path, &bytes, 0o600);
                }
                2 => Fixture::elf(&f.layout.helper, 89),
                3 => {
                    let path = f.layout.directory.join(LOCK);
                    std::fs::remove_file(&path).unwrap();
                    Fixture::write(&path, b"", 0o600);
                }
                _ => unreachable!(),
            }
            assert!(prepared.commit().is_err());
            assert!(!f.layout.directory.join(HISTORY).exists());
        }
    }
    #[test]
    fn same_lock_excludes_service_and_writer_without_waiting() {
        let f = Fixture::new();
        f.install_lock();
        let lease = f.lease().unwrap();
        assert!(f.prepare(f.request()).is_err());
        drop(lease);
        let prepared = f.prepare(f.request()).unwrap();
        assert!(f.lease().is_err());
        assert!(f.prepare(f.request()).is_err());
        drop(prepared);
        assert!(f.lease().is_ok());
    }
    #[test]
    fn incomplete_history_without_pending_and_foreign_entries_refuse() {
        for variant in 0..3 {
            let f = Fixture::new();
            f.install_lock();
            match variant {
                0 => std::fs::create_dir(f.layout.directory.join(HISTORY)).unwrap(),
                1 => Fixture::write(&f.layout.directory.join("foreign"), b"x", 0o600),
                2 => {
                    let mut prepared = f.prepare(f.request()).unwrap();
                    prepared.fault = Some(Cut::Attempt);
                    assert!(prepared.commit().is_err());
                    std::fs::remove_file(f.layout.directory.join(PENDING)).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(f.lease().is_err());
            assert!(f.prepare(f.request()).is_err());
        }
    }
    #[test]
    fn two_generations_preserve_chain_and_old_receipt_after_lost_reply() {
        let f = Fixture::new();
        let first = f.request();
        let first_id = first.operation.clone();
        let mut prepared = f.prepare(first.clone()).unwrap();
        prepared.fault = Some(Cut::Unlink);
        assert!(prepared.commit().is_err());
        assert_eq!(f.inspect(&first_id).unwrap(), Inspection::ObservedCommitted);
        assert!(f.prepare(first).is_err());
        Fixture::elf(&f.layout.runtime, 91);
        let second = f.request();
        let second_id = second.operation.clone();
        f.prepare(second).unwrap().commit().unwrap();
        assert_eq!(f.inspect(&first_id).unwrap(), Inspection::ObservedCommitted);
        assert_eq!(
            f.inspect(&second_id).unwrap(),
            Inspection::ObservedCommitted
        );
        assert!(f.lease().is_ok());
    }
    #[test]
    fn original_deadline_is_not_renewed_by_commit() {
        let f = Fixture::new();
        let mut prepared = f.prepare(f.request()).unwrap();
        prepared.until = Instant::now();
        assert_eq!(prepared.commit(), Err(Error::Expired));
        assert_eq!(std::fs::read_dir(&f.layout.directory).unwrap().count(), 1);
    }
    #[test]
    fn live_socket_appearing_after_preparation_refuses_without_a_prefix() {
        let f = Fixture::new();
        let prepared = f.prepare(f.request()).unwrap();
        let listener = std::os::unix::net::UnixListener::bind(&f.layout.socket).unwrap();
        assert!(prepared.commit().is_err());
        assert_eq!(std::fs::read_dir(&f.layout.directory).unwrap().count(), 1);
        drop(listener);
    }
    #[test]
    fn record_symlink_ancestor_drift_and_partial_lock_refuse() {
        for variant in 0..3 {
            let f = Fixture::new();
            let request = f.request();
            match variant {
                0 => {
                    let record = f.layout.directory.join(RECORD);
                    let held = f.layout.anchor.join("held-record");
                    std::fs::rename(&record, &held).unwrap();
                    symlink(held, record).unwrap();
                }
                1 => std::fs::set_permissions(
                    f.layout.core.parent().unwrap(),
                    std::fs::Permissions::from_mode(0o777),
                )
                .unwrap(),
                2 => Fixture::write(&f.layout.directory.join(LOCK), b"partial bootstrap", 0o600),
                _ => unreachable!(),
            }
            assert!(f.prepare(request).is_err());
            assert!(f.lease().is_err());
        }
    }
    #[test]
    fn completed_manifest_or_retained_predecessor_drift_never_admits_service() {
        for member in ["manifest", "attempt", "complete", "swap"] {
            let f = Fixture::new();
            let request = f.request();
            let operation = request.operation.clone();
            f.prepare(request).unwrap().commit().unwrap();
            let path = f
                .layout
                .directory
                .join(HISTORY)
                .join(&operation)
                .join(member);
            Fixture::write(&path, b"changed", 0o600);
            assert!(f.lease().is_err(), "{member}");
            assert!(f.inspect(&operation).is_err(), "{member}");
        }
    }
    #[test]
    fn observed_commit_requires_current_enrolled_images_to_still_match() {
        for runtime in [false, true] {
            let f = Fixture::new();
            let request = f.request();
            let operation = request.operation.clone();
            f.prepare(request).unwrap().commit().unwrap();
            assert_eq!(
                f.inspect(&operation).unwrap(),
                Inspection::ObservedCommitted
            );
            Fixture::elf(
                if runtime {
                    &f.layout.runtime
                } else {
                    &f.layout.core
                },
                199,
            );
            assert!(f.inspect(&operation).is_err());
        }
    }
    #[test]
    fn bounded_history_admits_128_and_refuses_129_without_eviction() {
        let f = Fixture::new();
        let mut first = None;
        for generation in 0..MAX_HISTORY {
            Fixture::elf(&f.layout.runtime, generation as u8 + 10);
            let request = f.request();
            if first.is_none() {
                first = Some(request.operation.clone());
            }
            f.prepare(request).unwrap().commit().unwrap();
        }
        assert_eq!(
            std::fs::read_dir(f.layout.directory.join(HISTORY))
                .unwrap()
                .count(),
            MAX_HISTORY
        );
        assert!(f.lease().is_ok());
        assert_eq!(
            f.inspect(first.as_ref().unwrap()).unwrap(),
            Inspection::ObservedCommitted
        );
        Fixture::elf(&f.layout.runtime, 250);
        assert!(f.prepare(f.request()).is_err());
        assert!(!f.layout.directory.join(PENDING).exists());
    }
}
