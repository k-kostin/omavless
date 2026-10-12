// SPDX-License-Identifier: MIT
//! Recovery-only manager authority: system-manager attestation + original
//! pidfd and authenticated private peer. This does NOT claim executable or
//! namespace descriptor access for the nondumpable user manager. Historical
//! Abort keeps its direct Process/ELF/namespace qualification.

use super::*;
use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, sockopt};
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileTypeExt;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RecoveryManagerRecord {
    pub(super) pid: u32,
    uid: u32,
    invocation: String,
}

impl RecoveryManagerRecord {
    pub(super) fn parse(bytes: &[u8], uid: u32) -> Result<Self> {
        let mut fields = recovery_fields(bytes)?;
        let id = format!("user@{uid}.service");
        if fields.remove("Id") != Some(id.as_str())
            || fields.remove("LoadState") != Some("loaded")
            || fields.remove("ActiveState") != Some("active")
            || fields.remove("SubState") != Some("running")
            || fields.remove("ControlPID") != Some("0")
            || fields.remove("Job") != Some("")
            || fields.remove("ControlGroup")
                != Some(super::recovery_cgroup::manager_group(uid).as_str())
            || decimal(fields.remove("User").ok_or(())?.as_bytes())? != uid
        {
            return Err(());
        }
        let pid = decimal(fields.remove("MainPID").ok_or(())?.as_bytes())?;
        let invocation = fields.remove("InvocationID").ok_or(())?;
        if pid == 0
            || pid > i32::MAX as u32
            || invocation.len() != 32
            || !invocation
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || invocation.bytes().all(|b| b == b'0')
            || !fields.is_empty()
        {
            return Err(());
        }
        Ok(Self {
            pid,
            uid,
            invocation: invocation.to_owned(),
        })
    }
}

pub(super) enum ManagerIdentity {
    Historical {
        process: Process,
        executable: TrustedExecutable,
    },
    Recovery(RecoveryManagerIdentity),
}

impl ManagerIdentity {
    pub(super) fn pid(&self) -> u32 {
        match self {
            Self::Historical { process, .. } => process.pid,
            Self::Recovery(v) => v.record.pid,
        }
    }
    pub(super) fn recheck(&self, root: &File, budget: &mut Budget) -> Result<()> {
        match self {
            Self::Historical { process, .. } => process.recheck(root, budget),
            Self::Recovery(v) => v.recheck(root, budget),
        }
    }
    pub(super) fn recheck_executable(&self) -> Result<()> {
        match self {
            Self::Historical { executable, .. } => executable.recheck(),
            Self::Recovery(_) => Ok(()),
        }
    }
    pub(super) fn recovery(&self) -> Option<&RecoveryManagerIdentity> {
        match self {
            Self::Recovery(v) => Some(v),
            _ => None,
        }
    }
}

pub(super) struct RecoveryManagerIdentity {
    record: RecoveryManagerRecord,
    directory: File,
    metadata: Metadata,
    start: u64,
    status: Status,
    pidfd: File,
    // Hold each canonical directory and the original socket inode; no reconnect.
    parents: Vec<(String, File, Metadata)>,
    socket: File,
    socket_metadata: Metadata,
    peer: OwnedFd,
}

fn alive(pidfd: &File) -> Result<()> {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let mut fds = [PollFd::new(pidfd, PollFlags::IN)];
    let n = poll(
        &mut fds,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| ())?;
    if n != 0 || !fds[0].revents().is_empty() {
        return Err(());
    }
    Ok(())
}

fn peer_matches(peer: &OwnedFd, record: &RecoveryManagerRecord) -> Result<()> {
    let credentials = getsockopt(peer, sockopt::PeerCredentials).map_err(|_| ())?;
    if credentials.pid() != record.pid as i32 || credentials.uid() != record.uid {
        return Err(());
    }
    // A dead/disconnected held connection is not replaced by another connection.
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let mut fds = [PollFd::new(peer, PollFlags::IN)];
    let n = poll(
        &mut fds,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| ())?;
    if n != 0 || !fds[0].revents().is_empty() {
        return Err(());
    }
    Ok(())
}

fn private_socket_mode(mode: u32) -> bool {
    // systemd binds its private AF_UNIX socket under umask 0077, producing
    // 0700. The owner execute bit grants no additional socket-connect access.
    // Accept the equally private 0600 spelling, not group/other or special bits.
    matches!(mode & 0o7777, 0o600 | 0o700)
}

impl RecoveryManagerIdentity {
    pub(super) fn capture(
        root: &File,
        record: RecoveryManagerRecord,
        budget: &mut Budget,
    ) -> Result<Self> {
        use rustix::process::{Pid as RustPid, PidfdFlags, pidfd_open};
        recovery_checkpoint!(true, ManagerProcDirectory);
        let pid = record.pid;
        let directory = directory(root, &pid.to_string())?;
        let metadata = directory.metadata().map_err(|_| ())?;
        recovery_checkpoint!(true, ManagerProcStat);
        let start = start_time(&proc_bytes(&directory, "stat", MAX_STATUS, budget)?, pid)?;
        recovery_checkpoint!(true, ManagerProcStatus);
        let status = status(&proc_bytes(&directory, "status", MAX_STATUS, budget)?, pid)?;
        if status.uids != [record.uid; 4] || status.namespace_pids != [pid] {
            return Err(());
        }
        recovery_checkpoint!(true, ManagerPidfd);
        let pidfd = File::from(
            pidfd_open(
                RustPid::from_raw(pid as i32).ok_or(())?,
                PidfdFlags::NONBLOCK,
            )
            .map_err(|_| ())?,
        );
        alive(&pidfd)?;
        recovery_checkpoint!(true, ManagerCanonicalDirectories);
        let mut parents = Vec::new();
        for (path, owner) in [
            ("/run".to_owned(), 0),
            ("/run/user".to_owned(), 0),
            (format!("/run/user/{}", record.uid), record.uid),
            (format!("/run/user/{}/systemd", record.uid), record.uid),
        ] {
            let file = if let Some((_, parent, _)) = parents.last() {
                super::directory(
                    parent,
                    Path::new(&path)
                        .file_name()
                        .and_then(|v| v.to_str())
                        .ok_or(())?,
                )?
            } else {
                File::from(
                    open(
                        Path::new(&path),
                        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| ())?,
                )
            };
            let m = file.metadata().map_err(|_| ())?;
            if !m.is_dir() || m.uid() != owner || m.mode() & 0o022 != 0 {
                return Err(());
            }
            parents.push((path, file, m));
        }
        recovery_checkpoint!(true, ManagerPrivateSocket);
        let parent = &parents.last().ok_or(())?.1;
        let socket = File::from(
            openat(
                parent,
                "private",
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        let socket_metadata = socket.metadata().map_err(|_| ())?;
        if !socket_metadata.file_type().is_socket()
            || socket_metadata.uid() != record.uid
            || !private_socket_mode(socket_metadata.mode())
            || socket_metadata.nlink() != 1
        {
            return Err(());
        }
        recovery_checkpoint!(true, ManagerPeerAllocation);
        let peer = nix::sys::socket::socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            None,
        )
        .map_err(|_| ())?;
        let path = format!("/proc/self/fd/{}/private", parent.as_raw_fd());
        // AF_UNIX nonblocking connect either finishes or refuses. No pending
        // connection is retried or used as evidence.
        recovery_checkpoint!(true, ManagerPeerConnect);
        connect(
            peer.as_raw_fd(),
            &UnixAddr::new(Path::new(&path)).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        recovery_checkpoint!(true, ManagerPeerCredentials);
        peer_matches(&peer, &record)?;
        let result = Self {
            record,
            directory,
            metadata,
            start,
            status,
            pidfd,
            parents,
            socket,
            socket_metadata,
            peer,
        };
        recovery_checkpoint!(true, ManagerCaptureRecheck);
        result.recheck(root, budget)?;
        Ok(result)
    }

    pub(super) fn matches_record(&self, record: &RecoveryManagerRecord) -> Result<()> {
        if &self.record != record {
            return Err(());
        }
        Ok(())
    }

    pub(super) fn recheck(&self, root: &File, budget: &mut Budget) -> Result<()> {
        budget.check()?;
        alive(&self.pidfd)?;
        let named = directory(root, &self.record.pid.to_string())?;
        if !identity(&self.metadata, &named.metadata().map_err(|_| ())?)
            || !identity(&self.metadata, &self.directory.metadata().map_err(|_| ())?)
            || start_time(
                &proc_bytes(&named, "stat", MAX_STATUS, budget)?,
                self.record.pid,
            )? != self.start
            || status(
                &proc_bytes(&named, "status", MAX_STATUS, budget)?,
                self.record.pid,
            )? != self.status
        {
            return Err(());
        }
        for (path, held, metadata) in &self.parents {
            if !identity(metadata, &held.metadata().map_err(|_| ())?)
                || !identity(metadata, &fs::symlink_metadata(path).map_err(|_| ())?)
            {
                return Err(());
            }
        }
        let parent = &self.parents.last().ok_or(())?.1;
        let named = File::from(
            openat(
                parent,
                "private",
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        if !identity(&self.socket_metadata, &named.metadata().map_err(|_| ())?)
            || !identity(
                &self.socket_metadata,
                &self.socket.metadata().map_err(|_| ())?,
            )
        {
            return Err(());
        }
        peer_matches(&self.peer, &self.record)?;
        alive(&self.pidfd)?;
        budget.check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};

    fn record() -> String {
        "Id=user@1001.service\nUser=1001\nLoadState=loaded\nActiveState=active\nSubState=running\nMainPID=42\nControlPID=0\nJob=\nControlGroup=/user.slice/user-1001.slice/user@1001.service\nInvocationID=1234567890abcdef1234567890abcdef\n".to_owned()
    }

    #[test]
    fn recovery_manager_attestation_is_closed_and_exact() {
        let source = record();
        assert_eq!(
            RecoveryManagerRecord::parse(source.as_bytes(), 1001)
                .unwrap()
                .pid,
            42
        );
        for (before, after) in [
            ("user@1001.service", "user@1002.service"),
            ("User=1001", "User=1002"),
            (
                "ControlGroup=/user.slice/user-1001.slice/user@1001.service",
                "ControlGroup=/foreign",
            ),
            ("LoadState=loaded", "LoadState=not-found"),
            ("ActiveState=active", "ActiveState=activating"),
            ("SubState=running", "SubState=dead"),
            ("MainPID=42", "MainPID=0"),
            ("MainPID=42", "MainPID=2147483648"),
            ("ControlPID=0", "ControlPID=1"),
            ("Job=\n", "Job=9\n"),
            (
                "1234567890abcdef1234567890abcdef",
                "00000000000000000000000000000000",
            ),
            ("1234567890abcdef1234567890abcdef", "123"),
            (
                "1234567890abcdef1234567890abcdef",
                "1234567890ABCDEF1234567890ABCDEF",
            ),
        ] {
            assert!(
                RecoveryManagerRecord::parse(source.replace(before, after).as_bytes(), 1001)
                    .is_err()
            );
        }
        for extra in [
            "Other=1\n",
            "Id=user@1001.service\n",
            "InvocationID=1234567890abcdef1234567890abcdef\n",
        ] {
            assert!(
                RecoveryManagerRecord::parse(format!("{source}{extra}").as_bytes(), 1001).is_err()
            );
        }
        for line in source.lines() {
            assert!(
                RecoveryManagerRecord::parse(
                    source.replace(&format!("{line}\n"), "").as_bytes(),
                    1001
                )
                .is_err()
            );
        }
    }

    // Actual descriptors but no service, package, VM or network mutation.
    struct Fixture {
        _temp: tempfile::TempDir,
        _listener: UnixListener,
        _accepted: UnixStream,
        root: File,
        identity: RecoveryManagerIdentity,
    }
    fn fixture() -> Fixture {
        fixture_with_mode(0o600)
    }

    fn fixture_with_mode(mode: u32) -> Fixture {
        use rustix::process::{Pid as RustPid, PidfdFlags, pidfd_open};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("private");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let peer = UnixStream::connect(&path).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        let root = File::open("/proc").unwrap();
        let pid = std::process::id();
        let proc = directory(&root, &pid.to_string()).unwrap();
        let mut budget = Budget::new();
        let state = status(
            &proc_bytes(&proc, "status", MAX_STATUS, &mut budget).unwrap(),
            pid,
        )
        .unwrap();
        let start = start_time(
            &proc_bytes(&proc, "stat", MAX_STATUS, &mut budget).unwrap(),
            pid,
        )
        .unwrap();
        let parent = File::open(temp.path()).unwrap();
        let metadata = parent.metadata().unwrap();
        let socket = File::from(
            openat(
                &parent,
                "private",
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        let socket_metadata = socket.metadata().unwrap();
        let identity = RecoveryManagerIdentity {
            record: RecoveryManagerRecord {
                pid,
                uid: state.uids[0],
                invocation: "1234567890abcdef1234567890abcdef".to_owned(),
            },
            metadata: proc.metadata().unwrap(),
            directory: proc,
            start,
            status: state,
            pidfd: File::from(
                pidfd_open(RustPid::from_raw(pid as i32).unwrap(), PidfdFlags::NONBLOCK).unwrap(),
            ),
            parents: vec![(temp.path().to_str().unwrap().to_owned(), parent, metadata)],
            socket,
            socket_metadata,
            peer: peer.into(),
        };
        Fixture {
            _temp: temp,
            _listener: listener,
            _accepted: accepted,
            root,
            identity,
        }
    }

    #[test]
    fn recovery_manager_private_socket_modes_are_owner_only_and_exact() {
        for permissions in 0..=0o7777 {
            assert_eq!(
                private_socket_mode(0o140000 | permissions),
                permissions == 0o600 || permissions == 0o700,
            );
        }
        for mode in [0o600, 0o700] {
            let fixture = fixture_with_mode(mode);
            assert!(private_socket_mode(fixture.identity.socket_metadata.mode()));
            assert!(
                fixture
                    .identity
                    .recheck(&fixture.root, &mut Budget::new())
                    .is_ok()
            );
            // Even another accepted spelling cannot replace the captured mode.
            fs::set_permissions(
                fixture._temp.path().join("private"),
                fs::Permissions::from_mode(if mode == 0o600 { 0o700 } else { 0o600 }),
            )
            .unwrap();
            assert!(
                fixture
                    .identity
                    .recheck(&fixture.root, &mut Budget::new())
                    .is_err()
            );
        }
    }

    #[test]
    fn recovery_manager_original_lifetime_and_uid_are_exact() {
        let mut fixture = fixture();
        let m = &mut fixture.identity;
        assert!(m.recheck(&fixture.root, &mut Budget::new()).is_ok());
        m.status.uids = [m.record.uid + 1; 4];
        assert!(m.recheck(&fixture.root, &mut Budget::new()).is_err());
        m.status.uids = [m.record.uid; 4];
        m.start += 1;
        assert!(m.recheck(&fixture.root, &mut Budget::new()).is_err());
    }

    #[test]
    fn recovery_manager_replaced_unit_invocation_or_peer_refuses() {
        let mut fixture = fixture();
        let m = &mut fixture.identity;
        let different = RecoveryManagerRecord {
            pid: m.record.pid,
            uid: m.record.uid,
            invocation: "abcdef1234567890abcdef1234567890".to_owned(),
        };
        assert!(m.matches_record(&different).is_err());
        m.record.pid += 1;
        assert!(peer_matches(&m.peer, &m.record).is_err());
        m.record.pid -= 1;
        m.record.uid += 1;
        assert!(peer_matches(&m.peer, &m.record).is_err());
    }

    #[test]
    fn recovery_manager_socket_substitution_never_reconnects() {
        let fixture = fixture();
        let path = fixture._temp.path().join("private");
        fs::remove_file(&path).unwrap();
        let _replacement = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            fixture
                .identity
                .recheck(&fixture.root, &mut Budget::new())
                .is_err()
        );
    }

    #[test]
    fn recovery_manager_closed_original_peer_and_exited_pidfd_refuse() {
        use rustix::process::{Pid as RustPid, PidfdFlags, pidfd_open};
        let fixture = fixture();
        fixture
            ._accepted
            .shutdown(std::net::Shutdown::Both)
            .unwrap();
        assert!(peer_matches(&fixture.identity.peer, &fixture.identity.record).is_err());
        let mut child = Command::new("/usr/bin/true").spawn().unwrap();
        let fd = File::from(
            pidfd_open(
                RustPid::from_raw(child.id() as i32).unwrap(),
                PidfdFlags::NONBLOCK,
            )
            .unwrap(),
        );
        child.wait().unwrap();
        assert!(alive(&fd).is_err());
    }
}
