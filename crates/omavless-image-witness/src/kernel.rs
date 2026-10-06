// SPDX-License-Identifier: MIT
//! Fixed kernel/object admission. No PID/path from a frame, no signal/getfd.
use crate::{Error, Result, status};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{self, AtFlags, Mode, OFlags},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata},
    io::Read,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
    time::Instant,
};

pub(crate) const CORE: &str = "/usr/lib/omavless-dns/mihomo";
pub(crate) const CLIENT: &str = "/usr/lib/omavless-image/development-runtime-tests";
const ENROLLMENT: &str = "/var/lib/omavless-image/development-enrollment-v1";
const PIDFS: i64 = 0x50494446;
const PROCFS: i64 = 0x9fa0;
const NSFS: i64 = 0x6e736673;

pub(crate) fn tick(until: Instant) -> Result<()> {
    if Instant::now() >= until {
        Err(Error::Expired)
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    size: u64,
    links: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
impl Identity {
    fn of(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            uid: m.uid(),
            gid: m.gid(),
            mode: m.mode(),
            size: m.len(),
            links: m.nlink(),
            mtime: m.mtime(),
            mtime_ns: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_ns: m.ctime_nsec(),
        }
    }
}
struct Chain {
    files: Vec<File>,
    paths: Vec<PathBuf>,
    ids: Vec<(u64, u64, u32, u32, u32)>,
}
impl Chain {
    fn capture(parent: &Path, until: Instant) -> Result<Self> {
        tick(until)?;
        let root = File::from(
            fs::open(
                "/",
                OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let mut result = Self {
            files: vec![root],
            paths: vec![PathBuf::from("/")],
            ids: vec![],
        };
        let mut path = PathBuf::from("/");
        for component in parent.components().skip(1) {
            tick(until)?;
            let std::path::Component::Normal(name) = component else {
                return Err(Error::Refused);
            };
            if result.files.len() >= 16 {
                return Err(Error::Refused);
            }
            let file = File::from(
                fs::openat(
                    result.files.last().ok_or(Error::Refused)?,
                    name,
                    OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| Error::Unavailable)?,
            );
            path.push(name);
            result.files.push(file);
            result.paths.push(path.clone());
        }
        for file in &result.files {
            let m = file.metadata().map_err(|_| Error::Unavailable)?;
            if !m.is_dir() || m.uid() != 0 || m.mode() & 0o7022 != 0 {
                return Err(Error::Refused);
            }
            result
                .ids
                .push((m.dev(), m.ino(), m.uid(), m.gid(), m.mode()));
        }
        result.check(until)?;
        Ok(result)
    }
    fn check(&self, until: Instant) -> Result<()> {
        for ((file, path), id) in self.files.iter().zip(&self.paths).zip(&self.ids) {
            tick(until)?;
            for m in [file.metadata(), std::fs::symlink_metadata(path)] {
                let m = m.map_err(|_| Error::Unavailable)?;
                if !m.is_dir() || (m.dev(), m.ino(), m.uid(), m.gid(), m.mode()) != *id {
                    return Err(Error::Refused);
                }
            }
        }
        tick(until)
    }
}
struct Fixed {
    file: File,
    path: &'static str,
    id: Identity,
    chain: Chain,
}
impl Fixed {
    fn open(path: &'static str, mode: u32, limit: u64, until: Instant) -> Result<Self> {
        let p = Path::new(path);
        let chain = Chain::capture(p.parent().ok_or(Error::Refused)?, until)?;
        let file = File::from(
            fs::openat(
                chain.files.last().ok_or(Error::Refused)?,
                p.file_name().ok_or(Error::Refused)?,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let m = file.metadata().map_err(|_| Error::Unavailable)?;
        if !m.is_file()
            || m.uid() != 0
            || m.nlink() != 1
            || m.mode() & 0o7777 != mode
            || m.len() == 0
            || m.len() > limit
        {
            return Err(Error::Refused);
        }
        let result = Self {
            file,
            path,
            id: Identity::of(&m),
            chain,
        };
        result.check(until)?;
        Ok(result)
    }
    fn check(&self, until: Instant) -> Result<()> {
        self.chain.check(until)?;
        if Identity::of(&self.file.metadata().map_err(|_| Error::Unavailable)?) != self.id
            || Identity::of(&std::fs::symlink_metadata(self.path).map_err(|_| Error::Unavailable)?)
                != self.id
        {
            return Err(Error::Refused);
        }
        tick(until)
    }
    fn hash(&mut self, until: Instant) -> Result<[u8; 32]> {
        use std::io::Seek;
        self.check(until)?;
        self.file.rewind().map_err(|_| Error::Unavailable)?;
        let mut digest = Sha256::new();
        let mut count = 0;
        let mut buf = [0; 65536];
        loop {
            tick(until)?;
            let n = self.file.read(&mut buf).map_err(|_| Error::Unavailable)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > self.id.size {
                return Err(Error::Refused);
            }
            digest.update(&buf[..n]);
        }
        if count != self.id.size {
            return Err(Error::Refused);
        }
        self.check(until)?;
        Ok(digest.finalize().into())
    }
    fn matches(&self, file: &File, until: Instant) -> Result<()> {
        self.check(until)?;
        if Identity::of(&file.metadata().map_err(|_| Error::Unavailable)?) != self.id {
            return Err(Error::Refused);
        }
        tick(until)
    }
}
fn bytes(file: &File, limit: usize, until: Instant) -> Result<Vec<u8>> {
    use std::os::unix::fs::FileExt;
    tick(until)?;
    let mut raw = vec![0; limit + 1];
    let n = file.read_at(&mut raw, 0).map_err(|_| Error::Unavailable)?;
    if n > limit {
        return Err(Error::Refused);
    }
    raw.truncate(n);
    tick(until)?;
    Ok(raw)
}
fn hex(s: &str) -> Result<[u8; 32]> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || s.bytes().all(|b| b == b'0')
    {
        return Err(Error::Refused);
    }
    let mut result = [0; 32];
    for (out, pair) in result.iter_mut().zip(s.as_bytes().as_chunks::<2>().0) {
        let d = |v: u8| if v <= b'9' { v - b'0' } else { v - b'a' + 10 };
        *out = d(pair[0]) * 16 + d(pair[1]);
    }
    Ok(result)
}
pub(crate) struct Roots {
    core: Fixed,
    client: Fixed,
    enrollment: Fixed,
    pub uid: u32,
}
impl Roots {
    pub(crate) fn capture(until: Instant) -> Result<Self> {
        let enrollment = Fixed::open(ENROLLMENT, 0o600, 512, until)?;
        let raw = bytes(&enrollment.file, 512, until)?;
        let text = std::str::from_utf8(&raw).map_err(|_| Error::Refused)?;
        let fields: Vec<_> = text.split('\n').collect();
        if fields.len() != 5
            || fields[0] != "omavless-development-current-image-v1"
            || fields[1] != "1000"
            || !fields[4].is_empty()
        {
            return Err(Error::Refused);
        }
        let mut core = Fixed::open(CORE, 0o755, 128 * 1024 * 1024, until)?;
        let mut client = Fixed::open(CLIENT, 0o755, 128 * 1024 * 1024, until)?;
        if core.hash(until)? != hex(fields[2])? || client.hash(until)? != hex(fields[3])? {
            return Err(Error::Refused);
        }
        let result = Self {
            core,
            client,
            enrollment,
            uid: 1000,
        };
        result.check(until)?;
        Ok(result)
    }
    fn check(&self, until: Instant) -> Result<()> {
        self.core.check(until)?;
        self.client.check(until)?;
        self.enrollment.check(until)
    }
}
fn unready(count: usize, flags: PollFlags) -> bool {
    count == 0 && flags.is_empty()
}
fn belongs(peer: status::Status, child: status::Status, uid: u32) -> bool {
    peer.uids == [uid; 4] && child.uids == [uid; 4] && child.parent == peer.pid
}
pub(crate) fn alive(fd: &OwnedFd, until: Instant) -> Result<()> {
    tick(until)?;
    let mut row = [PollFd::new(fd, PollFlags::IN)];
    let count = poll(
        &mut row,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| Error::Unavailable)?;
    if !unready(count, row[0].revents()) {
        return Err(Error::Refused);
    }
    tick(until)
}
pub(crate) fn kernel_pid(fd: &OwnedFd, until: Instant) -> Result<u32> {
    tick(until)?;
    if fs::fstatfs(fd).map_err(|_| Error::Unavailable)?.f_type as i64 != PIDFS {
        return Err(Error::Refused);
    }
    // PIDFD_THREAD is O_EXCL. No thread-only witness is admitted.
    if fs::fcntl_getfl(fd)
        .map_err(|_| Error::Unavailable)?
        .contains(OFlags::EXCL)
    {
        return Err(Error::Refused);
    }
    alive(fd, until)?;
    let file = File::from(
        fs::open(
            format!("/proc/self/fdinfo/{}", fd.as_raw_fd()),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Error::Unavailable)?,
    );
    if fs::fstatfs(&file).map_err(|_| Error::Unavailable)?.f_type as i64 != PROCFS {
        return Err(Error::Refused);
    }
    let pid = status::pidfd(&bytes(&file, status::LIMIT, until)?)?;
    alive(fd, until)?;
    Ok(pid)
}
struct Row {
    file: File,
    pid: u32,
    id: (u64, u64),
}
impl Row {
    fn open(pid: u32, until: Instant) -> Result<Self> {
        tick(until)?;
        let file = File::from(
            fs::open(
                format!("/proc/{pid}"),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        if fs::fstatfs(&file).map_err(|_| Error::Unavailable)?.f_type as i64 != PROCFS {
            return Err(Error::Refused);
        }
        let m = file.metadata().map_err(|_| Error::Unavailable)?;
        let result = Self {
            file,
            pid,
            id: (m.dev(), m.ino()),
        };
        result.check(until)?;
        Ok(result)
    }
    fn check(&self, until: Instant) -> Result<()> {
        tick(until)?;
        let m = self.file.metadata().map_err(|_| Error::Unavailable)?;
        let n = fs::statat(
            rustix::fs::CWD,
            format!("/proc/{}", self.pid),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| Error::Unavailable)?;
        if !m.is_dir() || (m.dev(), m.ino()) != self.id || (n.st_dev, n.st_ino) != self.id {
            return Err(Error::Refused);
        }
        tick(until)
    }
    fn read(&self, name: &str, until: Instant) -> Result<Vec<u8>> {
        self.check(until)?;
        let file = File::from(
            fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let raw = bytes(&file, status::LIMIT, until)?;
        self.check(until)?;
        Ok(raw)
    }
    fn facts(&self, until: Instant) -> Result<status::Status> {
        let facts = status::process(&self.read("status", until)?, &self.read("stat", until)?)?;
        if facts.pid != self.pid {
            return Err(Error::Refused);
        }
        Ok(facts)
    }
    fn image(&self, until: Instant) -> Result<File> {
        self.check(until)?;
        // Follow only this kernel-controlled exe link for the bound pidfd row.
        let image = File::from(
            fs::openat(
                &self.file,
                "exe",
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        self.check(until)?;
        Ok(image)
    }
    fn namespace(&self, name: &str, until: Instant) -> Result<File> {
        self.check(until)?;
        let fd = File::from(
            fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        if fs::fstatfs(&fd).map_err(|_| Error::Unavailable)?.f_type as i64 != NSFS {
            return Err(Error::Refused);
        }
        self.check(until)?;
        Ok(fd)
    }
}
/// Holds the exact kernel peer/child objects and original rows; no Clone/Debug.
pub(crate) struct Binding {
    roots: Roots,
    peer: OwnedFd,
    child: OwnedFd,
    peer_row: Row,
    child_row: Row,
    peer_facts: status::Status,
    child_facts: status::Status,
    namespaces: Vec<(File, File, File)>,
    poisoned: bool,
}
impl Binding {
    pub(crate) fn bind(
        roots: Roots,
        peer: OwnedFd,
        child: OwnedFd,
        until: Instant,
    ) -> Result<Self> {
        let peer_pid = kernel_pid(&peer, until)?;
        let child_pid = kernel_pid(&child, until)?;
        let peer_row = Row::open(peer_pid, until)?;
        let child_row = Row::open(child_pid, until)?;
        let peer_facts = peer_row.facts(until)?;
        let child_facts = child_row.facts(until)?;
        if !belongs(peer_facts, child_facts, roots.uid) {
            return Err(Error::Refused);
        }
        let own = Row::open(rustix::process::getpid().as_raw_pid() as u32, until)?;
        // This ties procfs's PID view to the actual helper PID namespace.
        own.facts(until)?;
        let mut namespaces = vec![];
        for name in ["ns/pid", "ns/user"] {
            let h = own.namespace(name, until)?;
            let p = peer_row.namespace(name, until)?;
            let c = child_row.namespace(name, until)?;
            for f in [&p, &c] {
                let m = f.metadata().map_err(|_| Error::Unavailable)?;
                let n = h.metadata().map_err(|_| Error::Unavailable)?;
                if (m.dev(), m.ino()) != (n.dev(), n.ino()) {
                    return Err(Error::Refused);
                }
            }
            namespaces.push((h, p, c));
        }
        let mut result = Self {
            roots,
            peer,
            child,
            peer_row,
            child_row,
            peer_facts,
            child_facts,
            namespaces,
            poisoned: false,
        };
        result.observe(until)?;
        Ok(result)
    }
    fn check(&self, until: Instant) -> Result<()> {
        if self.poisoned {
            return Err(Error::Refused);
        }
        self.roots.check(until)?;
        if kernel_pid(&self.peer, until)? != self.peer_facts.pid
            || kernel_pid(&self.child, until)? != self.child_facts.pid
            || self.peer_row.facts(until)? != self.peer_facts
            || self.child_row.facts(until)? != self.child_facts
        {
            return Err(Error::Refused);
        }
        for (n, (h, p, c)) in ["ns/pid", "ns/user"].into_iter().zip(&self.namespaces) {
            let fresh_p = self.peer_row.namespace(n, until)?;
            let fresh_c = self.child_row.namespace(n, until)?;
            for f in [p, c, &fresh_p, &fresh_c] {
                let m = f.metadata().map_err(|_| Error::Unavailable)?;
                let d = h.metadata().map_err(|_| Error::Unavailable)?;
                if (m.dev(), m.ino()) != (d.dev(), d.ino()) {
                    return Err(Error::Refused);
                }
            }
        }
        self.roots
            .client
            .matches(&self.peer_row.image(until)?, until)?;
        self.roots
            .core
            .matches(&self.child_row.image(until)?, until)?;
        tick(until)
    }
    pub(crate) fn observe(&mut self, until: Instant) -> Result<File> {
        let result = (|| {
            self.check(until)?;
            let image = self.child_row.image(until)?;
            self.roots.core.matches(&image, until)?;
            self.check(until)?;
            self.roots.core.matches(&image, until)?;
            Ok(image)
        })();
        if result.is_err() {
            self.poisoned = true
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_parent_and_all_four_uids_are_not_copied_authority() {
        let peer = status::Status {
            pid: 42,
            parent: 1,
            uids: [1000; 4],
            start: 10,
        };
        let child = status::Status {
            pid: 43,
            parent: 42,
            uids: [1000; 4],
            start: 11,
        };
        assert!(belongs(peer, child, 1000));
        let mut wrong = child;
        wrong.parent = 1;
        assert!(!belongs(peer, wrong, 1000));
        for n in 0..4 {
            let mut wrong = child;
            wrong.uids[n] = 0;
            assert!(!belongs(peer, wrong, 1000));
        }
        let mut other = peer;
        other.uids[3] = 0;
        assert!(!belongs(other, child, 1000));
    }
    #[test]
    fn exit_error_hup_and_invalid_descriptor_are_never_live() {
        assert!(unready(0, PollFlags::empty()));
        assert!(!unready(1, PollFlags::empty()));
        for flag in [
            PollFlags::IN,
            PollFlags::HUP,
            PollFlags::ERR,
            PollFlags::NVAL,
        ] {
            assert!(!unready(0, flag));
            assert!(!unready(1, flag));
        }
    }
    #[test]
    fn full_source_identity_rejects_every_drift_coordinate() {
        let original = Identity {
            dev: 1,
            ino: 2,
            uid: 0,
            gid: 0,
            mode: 0o100755,
            size: 100,
            links: 1,
            mtime: 4,
            mtime_ns: 5,
            ctime: 6,
            ctime_ns: 7,
        };
        let variants = [
            Identity { dev: 9, ..original },
            Identity { ino: 9, ..original },
            Identity { uid: 9, ..original },
            Identity { gid: 9, ..original },
            Identity {
                mode: 9,
                ..original
            },
            Identity {
                size: 9,
                ..original
            },
            Identity {
                links: 9,
                ..original
            },
            Identity {
                mtime: 9,
                ..original
            },
            Identity {
                mtime_ns: 9,
                ..original
            },
            Identity {
                ctime: 9,
                ..original
            },
            Identity {
                ctime_ns: 9,
                ..original
            },
        ];
        for changed in variants {
            assert!(changed != original);
        }
    }
}
