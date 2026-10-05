// SPDX-License-Identifier: MIT
//! Opt-in developer service. No install/start/enrollment action is performed.
//! Entry accepts only fixed serve/recover operations, never shell commands,
//! arbitrary paths, caller namespaces, handles or serialized authority tokens.
use crate::authority_composition::AuthoritySession;
use crate::launch_acquisition::acquire_fixed_service;
use crate::listener_publisher_candidate::publish_fixed_managed;
use crate::locked_state::LockedState;
use crate::package_group_candidate::PackageGroup;
use crate::protocol::{MAX_FRAME_BYTES, Response, decode_response};
use crate::session_owner_candidate::SessionProgress;
use crate::startup_trace::{self as trace, Event, Phase};
use crate::transport_candidate::RECOVER_FRAME;
use nix::fcntl::{OFlag, open, openat};
use nix::sys::socket::{
    AddressFamily, Backlog, SockFlag, SockType, UnixAddr, bind, listen, socket,
};
use nix::sys::stat::{FchmodatFlags, Mode, fchmodat, mkdirat};
use nix::unistd::geteuid;
use std::fs::File;
use std::io::{IsTerminal, Read, Write};
use std::mem::ManuallyDrop;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

const RECOVERY_PATH: &str = "/run/omavless-netguard/admin/recovery.sock";
const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
const SOCKET_ENTRY: OFlag = OFlag::O_PATH
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);
type Result<T> = std::result::Result<T, ()>;
fn require(value: bool) -> Result<()> {
    if value { Ok(()) } else { Err(()) }
}
fn same(a: &File, b: &File) -> Result<()> {
    let a = a.metadata().map_err(|_| ())?;
    let b = b.metadata().map_err(|_| ())?;
    require((a.dev(), a.ino()) == (b.dev(), b.ino()))
}
fn private(file: &File, directory: bool) -> Result<()> {
    let m = file.metadata().map_err(|_| ())?;
    require(
        (m.uid(), m.gid()) == (0, 0)
            && m.mode() & 0o7777 == if directory { 0o700 } else { 0o600 }
            && if directory {
                m.is_dir()
            } else {
                m.file_type().is_socket() && m.nlink() == 1
            },
    )
}

/// A NEW root-only admin subdirectory; the enrolled-user control socket's
/// common parent remains traversable. No existing admin/socket is adopted or
/// unlinked. Retained pinned entries are checked before and after each client.
struct RecoveryListener {
    listener: UnixListener,
    run: File,
    parent: File,
    admin: File,
    leaf: File,
    group: PackageGroup,
}
impl RecoveryListener {
    fn publish() -> Result<Self> {
        let group = PackageGroup::open_fixed().map_err(|_| ())?;
        group.validate().map_err(|_| ())?;
        let run = File::from(open("/run", DIRECTORY, Mode::empty()).map_err(|_| ())?);
        let m = run.metadata().map_err(|_| ())?;
        require(m.is_dir() && (m.uid(), m.gid()) == (0, 0) && m.mode() & 0o022 == 0)?;
        let parent = File::from(
            openat(&run, "omavless-netguard", DIRECTORY, Mode::empty()).map_err(|_| ())?,
        );
        let m = parent.metadata().map_err(|_| ())?;
        require(
            m.is_dir() && (m.uid(), m.gid()) == (0, group.gid()) && m.mode() & 0o7777 == 0o750,
        )?;
        mkdirat(&parent, "admin", Mode::from_bits_truncate(0o700)).map_err(|_| ())?;
        let admin = File::from(openat(&parent, "admin", DIRECTORY, Mode::empty()).map_err(|_| ())?);
        private(&admin, true)?;
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            None,
        )
        .map_err(|_| ())?;
        bind(
            fd.as_raw_fd(),
            &UnixAddr::new(RECOVERY_PATH).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        listen(&fd, Backlog::new(1).map_err(|_| ())?).map_err(|_| ())?;
        let listener = UnixListener::from(fd);
        let leaf = File::from(
            openat(&admin, "recovery.sock", SOCKET_ENTRY, Mode::empty()).map_err(|_| ())?,
        );
        let m = leaf.metadata().map_err(|_| ())?;
        require(m.file_type().is_socket() && (m.uid(), m.gid()) == (0, 0) && m.nlink() == 1)?;
        fchmodat(
            &admin,
            "recovery.sock",
            Mode::from_bits_truncate(0o600),
            FchmodatFlags::NoFollowSymlink,
        )
        .map_err(|_| ())?;
        // No chmod/chown/replacement after publication; old or ambiguous
        // artifacts are left intact, never treated as this invocation's own.
        let held = Self {
            listener,
            run,
            parent,
            admin,
            leaf,
            group,
        };
        held.validate()?;
        Ok(held)
    }
    fn validate(&self) -> Result<()> {
        self.group.validate().map_err(|_| ())?;
        let m = self.run.metadata().map_err(|_| ())?;
        require(m.is_dir() && (m.uid(), m.gid()) == (0, 0) && m.mode() & 0o022 == 0)?;
        let current_run = File::from(open("/run", DIRECTORY, Mode::empty()).map_err(|_| ())?);
        same(&self.run, &current_run)?;
        let parent = File::from(
            openat(&self.run, "omavless-netguard", DIRECTORY, Mode::empty()).map_err(|_| ())?,
        );
        same(&self.parent, &parent)?;
        let m = self.parent.metadata().map_err(|_| ())?;
        require(
            m.is_dir() && (m.uid(), m.gid()) == (0, self.group.gid()) && m.mode() & 0o7777 == 0o750,
        )?;
        private(&self.admin, true)?;
        let admin =
            File::from(openat(&self.parent, "admin", DIRECTORY, Mode::empty()).map_err(|_| ())?);
        same(&self.admin, &admin)?;
        private(&self.leaf, false)?;
        let leaf = File::from(
            openat(&self.admin, "recovery.sock", SOCKET_ENTRY, Mode::empty()).map_err(|_| ())?,
        );
        same(&self.leaf, &leaf)?;
        require(
            self.listener.local_addr().map_err(|_| ())?.as_pathname()
                == Some(std::path::Path::new(RECOVERY_PATH)),
        )
    }
    fn accept_one(&self) -> Result<Option<UnixStream>> {
        self.validate()?;
        match self.listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).map_err(|_| ())?;
                self.validate()?;
                Ok(Some(stream))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(_) => Err(()),
        }
    }
}

fn park() -> ! {
    // No watchdog exit, retry, query, teardown or automatic disarm on handled
    // uncertainty. Fatal process death remains an externally tested boundary.
    loop {
        std::thread::park();
    }
}

fn serve() -> Result<()> {
    let creator = acquire_fixed_service().map_err(|_| ())?;
    let mut state = ManuallyDrop::new(trace::step(Phase::StateOpen, || {
        LockedState::open_fixed().map_err(|_| ())
    })?);
    state.seal_cold_state();
    let listener = trace::step(Phase::ControlPublished, || {
        publish_fixed_managed().map_err(|_| ())
    })?;
    let recovery = ManuallyDrop::new(trace::step(
        Phase::RecoveryPublished,
        RecoveryListener::publish,
    )?);
    let mut session = ManuallyDrop::new(trace::step(Phase::AuthorityAssembled, || {
        AuthoritySession::from_admitted(listener, ManuallyDrop::into_inner(state), creator)
            .map_err(|_| ())
    })?);
    trace::finish();
    loop {
        if let Some(stream) = recovery.accept_one()? {
            if session.recover_one(stream) != SessionProgress::Served {
                park();
            }
            recovery.validate()?;
        }
        match session.poll_one() {
            SessionProgress::Idle => std::thread::sleep(Duration::from_millis(250)),
            SessionProgress::Served => (),
            _ => park(),
        }
    }
}

fn recover_console() -> Result<()> {
    // TTY is only the administrator UX guard. Server SO_PEERCRED=root is the
    // authority; this client cannot supply an enrolled UID or generation.
    require(
        geteuid().as_raw() == 0
            && std::io::stdin().is_terminal()
            && std::io::stdout().is_terminal(),
    )?;
    let mut stream = UnixStream::connect(RECOVERY_PATH).map_err(|_| ())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| ())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| ())?;
    stream
        .write_all(&(RECOVER_FRAME.len() as u32).to_be_bytes())
        .map_err(|_| ())?;
    stream.write_all(RECOVER_FRAME).map_err(|_| ())?;
    let mut prefix = [0; 4];
    stream.read_exact(&mut prefix).map_err(|_| ())?;
    let length = u32::from_be_bytes(prefix) as usize;
    require(length > 0 && length <= MAX_FRAME_BYTES)?;
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).map_err(|_| ())?;
    let response = decode_response(&bytes).map_err(|_| ())?;
    require(matches!(
        response,
        Response::Status {
            protection: crate::protocol::Protection::Disarmed {
                closed_generation: Some(_)
            },
            health: crate::protocol::Health::Verified,
            ..
        }
    ))?;
    // Distinct administrative result, never a core-cleanup/normal-disarm proof.
    println!("K1_ROOT_RECOVERY_DIRECT_CONNECTIVITY_RESTORED");
    Ok(())
}

/// The only public service entry. Wrong CLI is inert and returns 2. Serve
/// catches unwind then parks; it cannot drop the held graph or retry startup.
pub fn entry() -> i32 {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 2 {
        return 2;
    }
    if args[1] == "recover" {
        return if recover_console().is_ok() { 0 } else { 2 };
    }
    if args[1] != "serve" {
        return 2;
    }
    trace::start();
    let _ = std::panic::catch_unwind(serve);
    trace::emit(Phase::Enter, Event::Refused, None);
    park()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_recovery_is_separate_literal_bounded_operation() {
        assert_eq!(RECOVERY_PATH, "/run/omavless-netguard/admin/recovery.sock");
        assert_eq!(RECOVER_FRAME, b"K1_ROOT_RECOVER_V1\n");
        assert!(RECOVER_FRAME.len() < 64);
    }
}
