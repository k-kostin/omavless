use super::*;
use crate::package_group_candidate::PackageGroup;
use nix::{
    fcntl::{AtFlags, OFlag, open, openat},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::{
        socket::{
            AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket, sockopt,
        },
        stat::{Mode, fstatat},
    },
};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsFd, AsRawFd},
        unix::{
            fs::{FileTypeExt, MetadataExt},
            net::UnixStream,
        },
    },
};

pub(super) struct Linux;
pub(super) struct Endpoint {
    group: PackageGroup,
    run: File,
    parent: File,
    leaf: File,
}
const DIR: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
fn safe_root(file: &File) -> Result<()> {
    let m = file.metadata().map_err(|_| REFUSE)?;
    if m.is_dir() && (m.uid(), m.gid()) == (0, 0) && m.mode() & 0o022 == 0 {
        Ok(())
    } else {
        Err(REFUSE)
    }
}
fn named(parent: &File, name: &str, held: &File) -> Result<()> {
    let a = fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(|_| REFUSE)?;
    let b = held.metadata().map_err(|_| REFUSE)?;
    if (
        a.st_dev, a.st_ino, a.st_mode, a.st_uid, a.st_gid, a.st_nlink,
    ) == (b.dev(), b.ino(), b.mode(), b.uid(), b.gid(), b.nlink())
    {
        Ok(())
    } else {
        Err(REFUSE)
    }
}
impl Endpoint {
    fn validate(&self) -> Result<()> {
        self.group.validate().map_err(|_| REFUSE)?;
        safe_root(&self.run)?;
        let current = File::from(open("/run", DIR, Mode::empty()).map_err(|_| REFUSE)?);
        let a = current.metadata().map_err(|_| REFUSE)?;
        let b = self.run.metadata().map_err(|_| REFUSE)?;
        if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
            return Err(REFUSE);
        }
        named(&self.run, "omavless-netguard", &self.parent)?;
        let p = self.parent.metadata().map_err(|_| REFUSE)?;
        if !p.is_dir() || (p.uid(), p.gid()) != (0, self.group.gid()) || p.mode() & 0o7777 != 0o750
        {
            return Err(REFUSE);
        }
        named(&self.parent, "control.sock", &self.leaf)?;
        let l = self.leaf.metadata().map_err(|_| REFUSE)?;
        if !l.file_type().is_socket()
            || l.nlink() != 1
            || (l.uid(), l.gid()) != (0, self.group.gid())
            || l.mode() & 0o7777 != 0o660
        {
            return Err(REFUSE);
        }
        Ok(())
    }
}
impl Backend for Linux {
    type Endpoint = Endpoint;
    type Socket = UnixStream;
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn admit(&mut self) -> Result<Endpoint> {
        // Builder-local read-only FDs can drop on Err; no network operation yet.
        let group = PackageGroup::open_fixed().map_err(|_| REFUSE)?;
        let run = File::from(open("/run", DIR, Mode::empty()).map_err(|_| REFUSE)?);
        safe_root(&run)?;
        let parent =
            File::from(openat(&run, "omavless-netguard", DIR, Mode::empty()).map_err(|_| REFUSE)?);
        let leaf = File::from(
            openat(
                &parent,
                "control.sock",
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| REFUSE)?,
        );
        let endpoint = Endpoint {
            group,
            run,
            parent,
            leaf,
        };
        endpoint.validate()?;
        Ok(endpoint)
    }
    fn recheck(&mut self, e: &Endpoint) -> Result<()> {
        e.validate()
    }
    fn create(&mut self) -> Result<UnixStream> {
        socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            None,
        )
        .map(UnixStream::from)
        .map_err(|_| REFUSE)
    }
    fn connect(&mut self, s: &UnixStream) -> Result<Connect> {
        match connect(s.as_raw_fd(), &UnixAddr::new(PATH).map_err(|_| REFUSE)?) {
            Ok(()) => Ok(Connect::Complete),
            Err(nix::errno::Errno::EINPROGRESS) => Ok(Connect::Pending),
            Err(_) => Err(REFUSE), // including UNIX backlog EAGAIN and EINTR: no retry
        }
    }
    fn connected(&mut self, s: &UnixStream) -> Result<()> {
        if getsockopt(s, sockopt::SocketError).map_err(|_| REFUSE)? != 0 || s.peer_addr().is_err() {
            return Err(REFUSE);
        }
        Ok(())
    }
    fn wait(&mut self, s: &UnixStream, interest: Interest, remaining: Duration) -> Result<()> {
        let events = match interest {
            Interest::Read => PollFlags::POLLIN,
            Interest::Write => PollFlags::POLLOUT,
        };
        let ms = remaining.as_nanos().div_ceil(1_000_000);
        let timeout =
            PollTimeout::try_from(i32::try_from(ms).map_err(|_| REFUSE)?).map_err(|_| REFUSE)?;
        let mut fds = [PollFd::new(s.as_fd(), events)];
        if poll(&mut fds, timeout).map_err(|_| REFUSE)? != 1 {
            return Err(REFUSE);
        }
        let got = fds[0].revents().ok_or(REFUSE)?;
        if got.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) || !got.contains(events) {
            return Err(REFUSE);
        }
        Ok(()) // readable buffered bytes can coexist with HUP; actual EOF refuses
    }
    fn peer(&mut self, s: &UnixStream) -> Result<Peer> {
        let p = getsockopt(s, sockopt::PeerCredentials).map_err(|_| REFUSE)?;
        Ok(Peer {
            pid: p.pid(),
            uid: p.uid(),
            gid: p.gid(),
        })
    }
    fn write(&mut self, s: &UnixStream, b: &[u8]) -> Result<Progress> {
        match (&*s).write(b) {
            Ok(n) => Ok(Progress::Bytes(n)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Progress::WouldBlock),
            Err(_) => Err(REFUSE),
        }
    }
    fn read(&mut self, s: &UnixStream, b: &mut [u8]) -> Result<Progress> {
        match (&*s).read(b) {
            Ok(n) => Ok(Progress::Bytes(n)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Progress::WouldBlock),
            Err(_) => Err(REFUSE),
        }
    }
}
