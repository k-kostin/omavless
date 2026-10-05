// SPDX-License-Identifier: MIT
//! Opt-in developer service, never normal daemon/recovery authority.
//!
//! Post-exec private channel, original unreaped child and original pidfd.
//! All manager descriptors remain inside one actor. Loss revokes availability;
//! there is no fatal-descriptor-survival guarantee or reconnect API.

#[path = "manager_actor_protocol.rs"]
mod protocol;

use crate::restore_abort_cli::stopped_owner::actor_capture::Retained;
use nix::fcntl::{OFlag, open};
use nix::sys::resource::{Resource, getrlimit, setrlimit};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use nix::sys::stat::Mode;
use nix::unistd::{getgid, getppid, getuid};
use protocol::{Context, FRAME_BYTES, Frame, Kind, Phase};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// One administrator-owned VM epoch, not a caller-selected fixture. No method
// here deletes either name, retries a reservation, or resets the epoch.
const EPOCH: &str = "/run/omavless-t4-actor-development";
const SENTINEL: &str = "/run/omavless-t4-actor-development/reserved";
const CHANNEL: &str = "/run/omavless-t4-actor-development/channel";
const ACTOR_NOFILE: u64 = 64;
const WHOLE_SECONDS: u64 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unavailable;

fn tick(until: Instant) -> Result<(), Unavailable> {
    if Instant::now() < until {
        Ok(())
    } else {
        Err(Unavailable)
    }
}

fn emit(label: &'static [u8], until: Instant) -> Result<(), Unavailable> {
    tick(until)?;
    let mut output = std::io::stdout().lock();
    output.write_all(label).map_err(|_| Unavailable)?;
    output.flush().map_err(|_| Unavailable)?;
    tick(until)
}

fn io_frame<T: Read + Write>(
    stream: &mut T,
    outgoing: Option<Frame>,
    until: Instant,
) -> Result<Option<Frame>, Unavailable> {
    // Ordinary stream I/O only. Never recvmsg/SCM_RIGHTS, never forwarded FDs.
    let mut bytes = match &outgoing {
        Some(frame) => frame.encode()?,
        None => [0; FRAME_BYTES],
    };
    let mut done = 0;
    while done < FRAME_BYTES {
        tick(until)?;
        let result = if outgoing.is_some() {
            stream.write(&bytes[done..])
        } else {
            stream.read(&mut bytes[done..])
        };
        match result {
            Ok(0) => return Err(Unavailable),
            Ok(size) => done += size,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(_) => return Err(Unavailable),
        }
        tick(until)?;
    }
    if outgoing.is_some() {
        Ok(None)
    } else {
        Frame::decode(&bytes).map(Some)
    }
}

fn receive(stream: &mut UnixStream, until: Instant) -> Result<Frame, Unavailable> {
    io_frame(stream, None, until)?.ok_or(Unavailable)
}

fn peer(stream: &UnixStream, pid: u32) -> Result<(), Unavailable> {
    let credentials = getsockopt(stream, PeerCredentials).map_err(|_| Unavailable)?;
    peer_identity(credentials.pid(), credentials.uid(), credentials.gid(), pid)
}

fn peer_identity(found: i32, uid: u32, gid: u32, original: u32) -> Result<(), Unavailable> {
    if found != i32::try_from(original).map_err(|_| Unavailable)? || uid != 0 || gid != 0 {
        Err(Unavailable)
    } else {
        Ok(())
    }
}

fn alive(pidfd: &OwnedFd) -> Result<(), Unavailable> {
    let mut fds = [PollFd::new(pidfd, PollFlags::IN)];
    let count = poll(
        &mut fds,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| Unavailable)?;
    if count == 0 && fds[0].revents().is_empty() {
        Ok(())
    } else {
        Err(Unavailable)
    }
}

fn reaping_prerequisite(bytes: &[u8]) -> Result<(), Unavailable> {
    // Standalone fresh Linux exec clears sa_flags (including SA_NOCLDWAIT).
    // Do not call this API from a runtime with signal handlers/background reaper.
    let text = std::str::from_utf8(bytes).map_err(|_| Unavailable)?;
    let mut ignored = None;
    let mut caught = None;
    let mut threads = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("SigIgn:") {
            if ignored.is_some() {
                return Err(Unavailable);
            }
            ignored = Some(u64::from_str_radix(value.trim(), 16).map_err(|_| Unavailable)?);
        } else if let Some(value) = line.strip_prefix("SigCgt:") {
            if caught.is_some() {
                return Err(Unavailable);
            }
            caught = Some(u64::from_str_radix(value.trim(), 16).map_err(|_| Unavailable)?);
        } else if let Some(value) = line.strip_prefix("Threads:") {
            if threads.is_some() {
                return Err(Unavailable);
            }
            threads = Some(value.trim().parse::<u32>().map_err(|_| Unavailable)?);
        }
    }
    let child_bit = 1_u64 << (nix::sys::signal::Signal::SIGCHLD as u32 - 1);
    if threads != Some(1)
        || ignored.ok_or(Unavailable)? & child_bit != 0
        || caught.ok_or(Unavailable)? & child_bit != 0
    {
        return Err(Unavailable);
    }
    Ok(())
}

fn startup() -> Result<(), Unavailable> {
    if getuid().as_raw() != 0
        || getgid().as_raw() != 0
        || [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "LD_AUDIT",
            "LD_DEBUG",
            "LD_PROFILE",
        ]
        .iter()
        .any(|key| std::env::var_os(key).is_some())
    {
        return Err(Unavailable);
    }
    let mut bytes = Vec::new();
    File::open("/proc/self/status")
        .map_err(|_| Unavailable)?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|_| Unavailable)?;
    if bytes.len() > 65536 {
        return Err(Unavailable);
    }
    reaping_prerequisite(&bytes)?;
    // The only preexisting FDs are standard streams plus the live directory
    // iterator's FD. No preexisting descriptor bypasses the application budget.
    let mut count = 0;
    for entry in std::fs::read_dir("/proc/self/fd").map_err(|_| Unavailable)? {
        let entry = entry.map_err(|_| Unavailable)?;
        let name = entry.file_name();
        let number = name
            .to_str()
            .ok_or(Unavailable)?
            .parse::<u32>()
            .map_err(|_| Unavailable)?;
        if number > 3 || count >= 4 {
            return Err(Unavailable);
        }
        count += 1;
    }
    if count != 4 {
        return Err(Unavailable);
    }
    Ok(())
}

fn epoch() -> Result<(), Unavailable> {
    // Trusted-admin developer path, pre-created root:root0700 by ROOT recipe.
    // No caller path, chmod, recursive creation, follow-symlink admission.
    for path in ["/", "/run", EPOCH] {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| Unavailable)?;
        if !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.gid() != 0
            || metadata.mode() & 0o022 != 0
            || (path == EPOCH && metadata.mode() & 0o7777 != 0o700)
        {
            return Err(Unavailable);
        }
    }
    Ok(())
}

/// Only the fixed standalone developer binary calls this entry. Not a library
/// launch API for the daemon; exclusive child reaping is a process prerequisite.
pub fn supervisor_entry() -> Result<(), Unavailable> {
    let until = Instant::now() + Duration::from_secs(WHOLE_SECONDS);
    emit(b"t4_service_before_startup\n", until)?;
    startup()?;
    epoch()?;
    setrlimit(Resource::RLIMIT_NOFILE, ACTOR_NOFILE, ACTOR_NOFILE).map_err(|_| Unavailable)?;
    if getrlimit(Resource::RLIMIT_NOFILE).map_err(|_| Unavailable)? != (ACTOR_NOFILE, ACTOR_NOFILE)
    {
        return Err(Unavailable);
    }
    emit(b"t4_service_before_reservation\n", until)?;
    let mut sentinel = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(SENTINEL)
        .map_err(|_| Unavailable)?;
    sentinel
        .write_all(b"t4-original-epoch-reserved-v1\n")
        .map_err(|_| Unavailable)?;
    sentinel.sync_all().map_err(|_| Unavailable)?;
    tick(until)?;
    let listener = UnixListener::bind(CHANNEL).map_err(|_| Unavailable)?;
    listener.set_nonblocking(true).map_err(|_| Unavailable)?;
    let mut nonce = [0; 32];
    let mut entropy = File::from(
        open(
            "/dev/urandom",
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Unavailable)?,
    );
    entropy.read_exact(&mut nonce).map_err(|_| Unavailable)?;
    let mut context = Context::new(nonce)?;
    emit(b"t4_service_before_spawn\n", until)?;
    // Fresh exec, no threads/waiters, no SIGCHLD handler or ignored disposition.
    // std spawn's internally unreported partial acquisitions are not attested.
    let child = Command::new(std::env::current_exe().map_err(|_| Unavailable)?)
        .arg("--actor")
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|_| Unavailable)?;
    let mut owner = Supervisor {
        child,
        pidfd: None,
        stream: None,
        _listener: listener,
        _sentinel: sentinel,
        _entropy: entropy,
    };
    let pid = Pid::from_raw(i32::try_from(owner.child.id()).map_err(|_| Unavailable)?)
        .ok_or(Unavailable)?;
    owner.pidfd = Some(pidfd_open(pid, PidfdFlags::NONBLOCK).map_err(|_| Unavailable)?);
    tick(until)?;
    emit(b"t4_service_before_accept\n", until)?;
    loop {
        tick(until)?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        match owner._listener.accept() {
            Ok((stream, _)) => {
                owner.stream = Some(stream);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(_) => return Err(Unavailable),
        }
    }
    let stream = owner.stream.as_mut().ok_or(Unavailable)?;
    peer(stream, owner.child.id())?;
    stream.set_nonblocking(true).map_err(|_| Unavailable)?;
    io_frame(
        stream,
        Some(Frame {
            kind: Kind::Challenge,
            sequence: 0,
            nonce,
        }),
        until,
    )?;
    context.ready(receive(stream, until)?)?;
    alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
    emit(b"t4_service_ready\n", until)?;
    let request = context.begin(Kind::ObserveManager)?;
    io_frame(stream, Some(request), until)?;
    context.completed(receive(stream, until)?, Kind::Completed)?;
    alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
    emit(b"t4_service_manager_identity_checked\n", until)?;
    io_frame(stream, Some(context.begin(Kind::Halt)?), until)?;
    context.completed(receive(stream, until)?, Kind::Closed)?;
    // Only positively completed normal Halt may wait/reap. No failure path
    // queries/waits/signals; Child Drop does not wait or kill the original.
    loop {
        tick(until)?;
        let mut fds = [PollFd::new(
            owner.pidfd.as_ref().ok_or(Unavailable)?,
            PollFlags::IN,
        )];
        let count = poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )
        .map_err(|_| Unavailable)?;
        if count == 1 && fds[0].revents() == PollFlags::IN {
            break;
        }
        if count != 0 || !fds[0].revents().is_empty() {
            return Err(Unavailable);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    if !owner.child.wait().map_err(|_| Unavailable)?.success() {
        return Err(Unavailable);
    }
    emit(b"t4_service_completed\n", until)
}

struct Supervisor {
    child: Child,
    pidfd: Option<OwnedFd>,
    stream: Option<UnixStream>,
    _listener: UnixListener,
    _sentinel: File,
    _entropy: File,
}

fn quarantine(_held: Retained, _channel: UnixStream) -> ! {
    // Never query/read/reply/evict after uncertainty. While alive, the owner
    // retains its recorded originals. Fatal process loss has no custody claim.
    loop {
        std::thread::park();
    }
}

pub fn actor_entry() -> Result<(), Unavailable> {
    startup()?;
    epoch()?;
    setrlimit(Resource::RLIMIT_NOFILE, ACTOR_NOFILE, ACTOR_NOFILE).map_err(|_| Unavailable)?;
    if getrlimit(Resource::RLIMIT_NOFILE).map_err(|_| Unavailable)? != (ACTOR_NOFILE, ACTOR_NOFILE)
    {
        return Err(Unavailable);
    }
    let mut channel = UnixStream::connect(CHANNEL).map_err(|_| Unavailable)?;
    peer(
        &channel,
        u32::try_from(getppid().as_raw()).map_err(|_| Unavailable)?,
    )?;
    channel.set_nonblocking(true).map_err(|_| Unavailable)?;
    let until = Instant::now() + Duration::from_secs(5);
    let challenge = receive(&mut channel, until)?;
    if challenge.kind != Kind::Challenge || challenge.sequence != 0 {
        return Err(Unavailable);
    }
    let mut context = Context::new(challenge.nonce)?;
    let mut held = Retained::new().map_err(|_| Unavailable)?;
    io_frame(
        &mut channel,
        Some(Frame {
            kind: Kind::Ready,
            sequence: 0,
            nonce: context.nonce,
        }),
        until,
    )?;
    context.phase = Phase::Live;
    loop {
        let until = Instant::now() + Duration::from_secs(5);
        let result = actor_operation(&mut channel, &mut context, &mut held, until);
        match result {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(_) => {
                context.revoke();
                quarantine(held, channel);
            }
        }
    }
}

fn actor_operation(
    channel: &mut UnixStream,
    context: &mut Context,
    held: &mut Retained,
    until: Instant,
) -> Result<bool, Unavailable> {
    let request = receive(channel, until)?;
    let kind = request.kind;
    let expected = context.begin(kind)?;
    if request.sequence != expected.sequence || request.nonce != expected.nonce {
        context.revoke();
        return Err(Unavailable);
    }
    match kind {
        Kind::ObserveManager => {
            held.observe().map_err(|_| Unavailable)?;
            tick(until)?;
            io_frame(
                channel,
                Some(Frame {
                    kind: Kind::Completed,
                    sequence: context.sequence,
                    nonce: context.nonce,
                }),
                until,
            )?;
            context.phase = Phase::Live;
            Ok(false)
        }
        Kind::Halt => {
            // Cleanup occurs only on a separate valid request while Live, after
            // prior operations completed. No uncertain owner can enter here.
            // Release recorded originals before replying. Ordinary File close
            // backend/fatal loss is not a per-FD kernel absence attestation.
            held.finish().map_err(|_| Unavailable)?;
            io_frame(
                channel,
                Some(Frame {
                    kind: Kind::Closed,
                    sequence: context.sequence,
                    nonce: context.nonce,
                }),
                until,
            )?;
            context.phase = Phase::Closed;
            Ok(true)
        }
        _ => {
            context.revoke();
            Err(Unavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_child_reaping_prerequisites_refuse_ignored_caught_parallel_or_missing() {
        let clean = b"Threads:\t1\nSigIgn:\t0000000000000000\nSigCgt:\t0000000000000000\n";
        assert!(reaping_prerequisite(clean).is_ok());
        let bit = 1_u64 << (nix::sys::signal::Signal::SIGCHLD as u32 - 1);
        for raw in [
            format!("Threads:1\nSigIgn:{bit:016x}\nSigCgt:0\n"),
            format!("Threads:1\nSigIgn:0\nSigCgt:{bit:016x}\n"),
            "Threads:2\nSigIgn:0\nSigCgt:0\n".to_owned(),
            "Threads:1\nSigIgn:0\n".to_owned(),
            "Threads:1\nSigIgn:0\nSigIgn:0\nSigCgt:0\n".to_owned(),
        ] {
            assert!(reaping_prerequisite(raw.as_bytes()).is_err());
        }
    }
    #[test]
    fn fixed_epoch_and_fd_budget_cannot_be_client_selected() {
        assert_eq!(SENTINEL, format!("{EPOCH}/reserved"));
        assert_eq!(CHANNEL, format!("{EPOCH}/channel"));
        assert!(
            crate::restore_abort_cli::stopped_owner::actor_capture::HELD_FDS + 4
                < ACTOR_NOFILE as usize
        );
        assert_eq!(WHOLE_SECONDS, 15);
    }

    struct Memory {
        input: Vec<u8>,
        output: Vec<u8>,
        position: usize,
        fail_at: usize,
    }
    impl Read for Memory {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.position >= self.fail_at {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            let size = buffer
                .len()
                .min(3)
                .min(self.input.len() - self.position)
                .min(self.fail_at - self.position);
            buffer[..size].copy_from_slice(&self.input[self.position..self.position + size]);
            self.position += size;
            Ok(size)
        }
    }
    impl Write for Memory {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.output.len() >= self.fail_at {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            let size = bytes.len().min(3).min(self.fail_at - self.output.len());
            self.output.extend_from_slice(&bytes[..size]);
            Ok(size)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn kernel_peer_identity_must_match_the_original_child_not_a_claim() {
        assert!(peer_identity(41, 0, 0, 41).is_ok());
        for (pid, uid, gid, original) in [
            (42, 0, 0, 41),
            (41, 1, 0, 41),
            (41, 0, 1, 41),
            (-1, 0, 0, 41),
            (41, 0, 0, u32::MAX),
        ] {
            assert!(peer_identity(pid, uid, gid, original).is_err());
        }
    }

    #[test]
    fn every_partial_stream_cut_refuses_without_completing_or_resending() {
        let raw = Frame {
            kind: Kind::Ready,
            sequence: 0,
            nonce: [1; 32],
        }
        .encode()
        .unwrap();
        for cut in 0..FRAME_BYTES {
            let mut memory = Memory {
                input: raw.to_vec(),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            assert!(io_frame(&mut memory, None, Instant::now() + Duration::from_secs(1)).is_err());
            assert_eq!(memory.position, cut);
            assert!(memory.output.is_empty());
            let mut memory = Memory {
                input: Vec::new(),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            assert!(
                io_frame(
                    &mut memory,
                    Some(Frame {
                        kind: Kind::Ready,
                        sequence: 0,
                        nonce: [1; 32]
                    }),
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(memory.output, raw[..cut]);
        }
    }

    #[test]
    fn complete_short_continuations_work_and_expiry_has_no_first_effect() {
        let raw = Frame {
            kind: Kind::Ready,
            sequence: 0,
            nonce: [1; 32],
        }
        .encode()
        .unwrap();
        let mut memory = Memory {
            input: raw.to_vec(),
            output: Vec::new(),
            position: 0,
            fail_at: FRAME_BYTES,
        };
        assert_eq!(
            io_frame(&mut memory, None, Instant::now() + Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .kind,
            Kind::Ready
        );
        let mut memory = Memory {
            input: raw.to_vec(),
            output: Vec::new(),
            position: 0,
            fail_at: FRAME_BYTES,
        };
        assert!(io_frame(&mut memory, None, Instant::now() - Duration::from_secs(1)).is_err());
        assert_eq!(memory.position, 0);
        assert!(memory.output.is_empty());
    }
}
