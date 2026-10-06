// SPDX-License-Identifier: MIT
//! Opt-in developer service, never normal daemon/recovery authority.
//!
//! Post-exec private channel, original unreaped child and original pidfd.
//! All manager descriptors remain inside one actor. Loss revokes availability;
//! there is no fatal-descriptor-survival guarantee or reconnect API.

#[path = "manager_actor_protocol.rs"]
mod protocol;
#[path = "manager_actor_io.rs"]
mod retained_io;
#[path = "manager_actor_stage.rs"]
mod stage;
#[path = "manager_actor_transfer.rs"]
mod transfer;
pub(crate) const CANONICAL_STAGE_ORIGIN_FENCES: usize = stage::ORIGIN_FENCES;
pub(crate) const CANONICAL_COMMIT_ORIGIN_FENCES: usize = stage::COMMIT_ORIGIN_FENCES;
pub(crate) const CANONICAL_INSPECT_ORIGIN_FENCES: usize = stage::INSPECT_ORIGIN_FENCES;
pub(crate) const CANONICAL_MIXED_ORIGIN_FENCES: usize = stage::MIXED_ORIGIN_FENCES;

use crate::restore_abort_cli::stopped_owner::actor_canonical::{self, Canonical, ObserverRole};
use crate::restore_abort_cli::stopped_owner::actor_capture::Retained;
use nix::fcntl::{OFlag, open};
use nix::sys::resource::{Resource, getrlimit, setrlimit};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use nix::sys::stat::Mode;
use nix::unistd::{getgid, getppid, getuid};
use protocol::{Context, FRAME_BYTES, Frame, Kind, Phase};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use sha2::Digest;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// One administrator-owned VM epoch, not a caller-selected fixture. No method
// here deletes either name, retries a reservation, or resets the epoch.
const EPOCH: &str = "/run/omavless-t4-actor-development";
const SENTINEL: &str = "/run/omavless-t4-actor-development/reserved";
const CHANNEL: &str = "/run/omavless-t4-actor-development/channel";
pub(crate) const ACTOR_NOFILE: u64 = 64;
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

fn emit_actor(label: &'static [u8], until: Instant) -> Result<(), Unavailable> {
    tick(until)?;
    let mut output = std::io::stderr().lock();
    output.write_all(label).map_err(|_| Unavailable)?;
    output.flush().map_err(|_| Unavailable)?;
    tick(until)
}

fn io_frame<T: Read + Write>(
    stream: &mut T,
    outgoing: Option<Frame>,
    until: Instant,
) -> Result<Option<Frame>, Unavailable> {
    io_frame_width(stream, outgoing, FRAME_BYTES, until)
}

fn io_frame_width<T: Read + Write>(
    stream: &mut T,
    outgoing: Option<Frame>,
    width: usize,
    until: Instant,
) -> Result<Option<Frame>, Unavailable> {
    if width == 0 || width > FRAME_BYTES || (outgoing.is_none() && width != FRAME_BYTES) {
        return Err(Unavailable);
    }
    // Ordinary stream I/O only. Never recvmsg/SCM_RIGHTS, never forwarded FDs.
    let mut bytes = match &outgoing {
        Some(frame) => frame.encode()?,
        None => [0; FRAME_BYTES],
    };
    let mut done = 0;
    while done < width {
        tick(until)?;
        let result = if outgoing.is_some() {
            stream.write(&bytes[done..width])
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestShape {
    Exact,
    WrongNonce,
    Partial,
}

// A failed exchange consumes the ORIGINAL outstanding capability, including
// decode/EOF/deadline/short I/O. Reentry consults Context before any next write.
fn exchange<T: Read + Write>(
    stream: &mut T,
    context: &mut Context,
    kind: Kind,
    shape: RequestShape,
    until: Instant,
) -> Result<(), Unavailable> {
    let mut request = context.begin(kind)?;
    let result = (|| {
        if shape == RequestShape::WrongNonce {
            request.nonce[0] ^= 1;
            if request.nonce == [0; 32] {
                request.nonce[1] = 1;
            }
        }
        let width = if shape == RequestShape::Partial {
            FRAME_BYTES / 2
        } else {
            FRAME_BYTES
        };
        io_frame_width(stream, Some(request), width, until)?;
        let reply = io_frame(stream, None, until)?.ok_or(Unavailable)?;
        context.completed(reply, kind.completion()?)?;
        if shape != RequestShape::Exact {
            // Even an unexpected authenticated reply to a deliberately wrong
            // request is not an admitted completed operation or test success.
            return Err(Unavailable);
        }
        Ok(())
    })();
    if result.is_err() {
        context.revoke();
    }
    result
}

fn exchange_backup<T: Read + Write>(
    stream: &mut T,
    context: &mut Context,
    archive: &[u8],
    passphrase: &[u8],
    until: Instant,
) -> Result<(), Unavailable> {
    exchange_private(
        stream,
        context,
        Kind::AuthenticateBackup,
        archive,
        passphrase,
        until,
    )
}

fn exchange_private<T: Read + Write>(
    stream: &mut T,
    context: &mut Context,
    kind: Kind,
    archive: &[u8],
    passphrase: &[u8],
    until: Instant,
) -> Result<(), Unavailable> {
    if !matches!(
        kind,
        Kind::AuthenticateBackup | Kind::StageAuthenticatedBackup
    ) {
        context.revoke();
        return Err(Unavailable);
    }
    let request = context.begin(kind)?;
    let result = (|| {
        io_frame(stream, Some(request), until)?;
        transfer::send(stream, archive, passphrase, until)?;
        let reply = io_frame(stream, None, until)?.ok_or(Unavailable)?;
        context.completed(reply, kind.completion()?)
    })();
    if result.is_err() {
        context.revoke();
    }
    result
}

// This scenario transfers only public synthetic data in memory. It registers
// no backup/restore product operation, user input, passphrase argv or env key.
const SYNTHETIC_PASSPHRASE: &[u8] = b"synthetic transfer passphrase";
const SYNTHETIC_STORE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
const SYNTHETIC_TEMPLATE: &[u8] = include_bytes!("../../../templates/default.yaml");
// Distinct public OLD members, confined to the interruption scenario. Both
// members must differ from authenticated NEW to produce a genuine Mixed class.
const INTERRUPTED_OLD_STORE: &[u8] = br#"
{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
const INTERRUPTED_OLD_TEMPLATE: &[u8] = b"# public interrupted OLD template\nmode: global\n";

fn synthetic_backup(until: Instant) -> Result<zeroize::Zeroizing<Vec<u8>>, Unavailable> {
    tick(until)?;
    let archive = zeroize::Zeroizing::new(
        omavless_domain::private_backup::seal(
            SYNTHETIC_STORE,
            SYNTHETIC_TEMPLATE,
            SYNTHETIC_PASSPHRASE,
        )
        .map_err(|_| Unavailable)?,
    );
    tick(until)?;
    Ok(archive)
}

/// Closed trusted-admin developer scenarios, never request-selected targets,
/// paths, commands, reset tokens or normal product operation authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeveloperScenario {
    Single,
    CapacityThree,
    CapacityFourth,
    WrongNonceAfterFirst,
    PartialAfterFirst,
    DisconnectAfterFirst,
    AuthenticateBackup,
    StageAuthenticatedBackup,
    CanonicalStopped,
    CanonicalAuthenticate,
    CanonicalStage,
    CanonicalCommit,
    CanonicalInterruptedInspection,
}

impl DeveloperScenario {
    fn observations(self) -> usize {
        match self {
            Self::CapacityThree => 3,
            Self::CapacityFourth => 4,
            Self::AuthenticateBackup
            | Self::StageAuthenticatedBackup
            | Self::CanonicalStopped
            | Self::CanonicalAuthenticate
            | Self::CanonicalStage
            | Self::CanonicalCommit => 0,
            Self::CanonicalInterruptedInspection => 0,
            _ => 1,
        }
    }

    fn request_shape(self) -> Option<RequestShape> {
        match self {
            Self::WrongNonceAfterFirst => Some(RequestShape::WrongNonce),
            Self::PartialAfterFirst => Some(RequestShape::Partial),
            _ => None,
        }
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
    supervisor_scenario(DeveloperScenario::Single)
}

/// Same standalone/reaping prerequisites and SAME aggregate epoch reservation.
/// Any uncertain scenario leaves the sentinel and original live actor alone.
pub fn supervisor_scenario(scenario: DeveloperScenario) -> Result<(), Unavailable> {
    if scenario == DeveloperScenario::CanonicalInterruptedInspection {
        return interrupted_inspection_scenario();
    }
    let until = Instant::now() + Duration::from_secs(WHOLE_SECONDS);
    emit(b"t4_service_before_startup\n", until)?;
    startup()?;
    epoch()?;
    let canonical_mode = matches!(
        scenario,
        DeveloperScenario::CanonicalStopped
            | DeveloperScenario::CanonicalAuthenticate
            | DeveloperScenario::CanonicalStage
            | DeveloperScenario::CanonicalCommit
    );
    let nofile = if canonical_mode {
        actor_canonical::NOFILE
    } else {
        ACTOR_NOFILE
    };
    // The fixed explicit canonical mode must inherit its separately reviewed
    // ceiling; the classic actor's64 hard ceiling is never raised implicitly.
    setrlimit(Resource::RLIMIT_NOFILE, nofile, nofile).map_err(|_| Unavailable)?;
    if getrlimit(Resource::RLIMIT_NOFILE).map_err(|_| Unavailable)? != (nofile, nofile) {
        return Err(Unavailable);
    }
    // Fixed synthetic crypto is prepared BEFORE reservation/launch. It shares
    // the whole budget but cannot consume the actor's READY/read deadline while
    // the actor waits. No real user input or archive path is introduced.
    let backup = if matches!(
        scenario,
        DeveloperScenario::AuthenticateBackup
            | DeveloperScenario::StageAuthenticatedBackup
            | DeveloperScenario::CanonicalAuthenticate
            | DeveloperScenario::CanonicalStage
            | DeveloperScenario::CanonicalCommit
    ) {
        Some(synthetic_backup(until)?)
    } else {
        None
    };
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
        .arg(if canonical_mode {
            "--actor-canonical"
        } else {
            "--actor"
        })
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
    for _ in 0..scenario.observations() {
        exchange(
            stream,
            &mut context,
            Kind::ObserveManager,
            RequestShape::Exact,
            until,
        )?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        emit(b"t4_service_manager_identity_checked\n", until)?;
    }
    if scenario == DeveloperScenario::AuthenticateBackup {
        emit(b"t4_service_before_backup_transfer\n", until)?;
        exchange_backup(
            stream,
            &mut context,
            backup.as_ref().ok_or(Unavailable)?,
            SYNTHETIC_PASSPHRASE,
            until,
        )?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        emit(b"t4_service_backup_authenticated\n", until)?;
    }
    if scenario == DeveloperScenario::StageAuthenticatedBackup {
        emit(b"t4_service_before_fixture_stage\n", until)?;
        exchange_private(
            stream,
            &mut context,
            Kind::StageAuthenticatedBackup,
            backup.as_ref().ok_or(Unavailable)?,
            SYNTHETIC_PASSPHRASE,
            until,
        )?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        emit(b"t4_service_fixture_stage_recorded\n", until)?;
    }
    if canonical_mode {
        exchange(
            stream,
            &mut context,
            Kind::ObserveStopped,
            RequestShape::Exact,
            until,
        )?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        emit(b"t4_service_canonical_stopped_observed\n", until)?;
    }
    if matches!(
        scenario,
        DeveloperScenario::CanonicalAuthenticate
            | DeveloperScenario::CanonicalStage
            | DeveloperScenario::CanonicalCommit
    ) {
        emit(b"t4_service_before_backup_transfer\n", until)?;
        exchange_backup(
            stream,
            &mut context,
            backup.as_ref().ok_or(Unavailable)?,
            SYNTHETIC_PASSPHRASE,
            until,
        )?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        emit(b"t4_service_backup_authenticated\n", until)?;
    }
    if matches!(
        scenario,
        DeveloperScenario::CanonicalStage | DeveloperScenario::CanonicalCommit
    ) {
        emit(b"t4_service_before_fixture_stage\n", until)?;
        exchange(
            stream,
            &mut context,
            if scenario == DeveloperScenario::CanonicalCommit {
                Kind::CommitAuthenticatedBackup
            } else {
                Kind::StageAuthenticatedBackup
            },
            RequestShape::Exact,
            until,
        )?;
        alive(owner.pidfd.as_ref().ok_or(Unavailable)?)?;
        emit(
            if scenario == DeveloperScenario::CanonicalCommit {
                b"t4_service_fixture_pair_committed\n"
            } else {
                b"t4_service_fixture_stage_recorded\n"
            },
            until,
        )?;
    }
    if scenario == DeveloperScenario::CapacityFourth {
        // Unexpected fourth completion cannot silently turn a refusal scenario
        // into normal Halt/success, even if a future capacity regression exists.
        context.revoke();
        return Err(Unavailable);
    }
    if let Some(shape) = scenario.request_shape() {
        emit(b"t4_service_before_fault_request\n", until)?;
        exchange(stream, &mut context, Kind::ObserveManager, shape, until)?;
        // An unexpected positive reply to a deliberately invalid request is
        // not a scenario PASS and cannot enable Halt or another acquisition.
        context.revoke();
        return Err(Unavailable);
    }
    if scenario == DeveloperScenario::DisconnectAfterFirst {
        emit(b"t4_service_before_channel_disconnect\n", until)?;
        context.begin(Kind::ObserveManager)?;
        context.revoke();
        // Intentional developer channel-loss cut, no actor signal or query.
        stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(|_| Unavailable)?;
        return Err(Unavailable);
    }
    exchange(stream, &mut context, Kind::Halt, RequestShape::Exact, until)?;
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

struct PairSupervisor {
    // Fixed reservation BEFORE writer effects, never a growing actor list.
    children: [Option<Child>; 2],
    pidfds: [Option<OwnedFd>; 2],
    streams: [Option<UnixStream>; 2],
    attempted: [bool; 2],
    listener: UnixListener,
    _sentinel: File,
    _entropy: File,
}

fn settle_original(
    id: u32,
    wanted: i32,
    mut gate: impl FnMut() -> Result<(), Unavailable>,
    observe: impl FnOnce() -> Result<nix::sys::wait::WaitStatus, Unavailable>,
    reap: impl FnOnce() -> Result<Option<i32>, Unavailable>,
) -> Result<(), Unavailable> {
    gate()?;
    let observed = observe()?;
    gate()?;
    match observed {
        nix::sys::wait::WaitStatus::Exited(pid, code)
            if code == wanted && u32::try_from(pid.as_raw()).ok() == Some(id) => {}
        _ => return Err(Unavailable),
    }
    if reap()? != Some(wanted) {
        return Err(Unavailable);
    }
    gate()
}
impl PairSupervisor {
    fn spawn(
        &mut self,
        index: usize,
        argument: &'static str,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if index >= 2 || self.attempted[index] {
            return Err(Unavailable);
        }
        self.attempted[index] = true;
        tick(until)?;
        let child = Command::new(std::env::current_exe().map_err(|_| Unavailable)?)
            .arg(argument)
            .env_clear()
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| Unavailable)?;
        self.children[index] = Some(child); // BEFORE every reported-child postcheck
        let pid = Pid::from_raw(
            i32::try_from(self.children[index].as_ref().ok_or(Unavailable)?.id())
                .map_err(|_| Unavailable)?,
        )
        .ok_or(Unavailable)?;
        self.pidfds[index] = Some(pidfd_open(pid, PidfdFlags::NONBLOCK).map_err(|_| Unavailable)?);
        loop {
            tick(until)?;
            alive(self.pidfds[index].as_ref().ok_or(Unavailable)?)?;
            match self.listener.accept() {
                Ok((stream, _)) => {
                    self.streams[index] = Some(stream);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(_) => return Err(Unavailable),
            }
        }
        let stream = self.streams[index].as_mut().ok_or(Unavailable)?;
        peer(
            stream,
            self.children[index].as_ref().ok_or(Unavailable)?.id(),
        )?;
        stream.set_nonblocking(true).map_err(|_| Unavailable)?;
        tick(until)
    }
    fn ready(
        &mut self,
        index: usize,
        nonce: [u8; 32],
        until: Instant,
    ) -> Result<Context, Unavailable> {
        let mut context = Context::new(nonce)?;
        let stream = self.streams[index].as_mut().ok_or(Unavailable)?;
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
        alive(self.pidfds[index].as_ref().ok_or(Unavailable)?)?;
        Ok(context)
    }
    fn wait_original(
        &mut self,
        index: usize,
        wanted: i32,
        until: Instant,
    ) -> Result<(), Unavailable> {
        loop {
            tick(until)?;
            let mut pollfd = [PollFd::new(
                self.pidfds[index].as_ref().ok_or(Unavailable)?,
                PollFlags::IN,
            )];
            let count = poll(
                &mut pollfd,
                Some(&Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                }),
            )
            .map_err(|_| Unavailable)?;
            if count == 1 && pollfd[0].revents() == PollFlags::IN {
                break;
            }
            if count != 0 || !pollfd[0].revents().is_empty() {
                return Err(Unavailable);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let id = self.children[index].as_ref().ok_or(Unavailable)?.id();
        settle_original(
            id,
            wanted,
            || tick(until),
            || {
                nix::sys::wait::waitid(
                    nix::sys::wait::Id::PIDFd(
                        self.pidfds[index].as_ref().ok_or(Unavailable)?.as_fd(),
                    ),
                    nix::sys::wait::WaitPidFlag::WEXITED
                        | nix::sys::wait::WaitPidFlag::WNOHANG
                        | nix::sys::wait::WaitPidFlag::WNOWAIT,
                )
                .map_err(|_| Unavailable)
            },
            || {
                self.children[index]
                    .as_mut()
                    .ok_or(Unavailable)?
                    .try_wait()
                    .map_err(|_| Unavailable)?
                    .map(|status| status.code())
                    .ok_or(Unavailable)
            },
        )
    }
}

fn interrupted_inspection_scenario() -> Result<(), Unavailable> {
    const WHOLE: u64 = 45;
    const {
        assert!(2 * actor_canonical::NOFILE == 16640);
    }
    let until = Instant::now() + Duration::from_secs(WHOLE);
    startup()?;
    epoch()?;
    setrlimit(
        Resource::RLIMIT_NOFILE,
        actor_canonical::NOFILE,
        actor_canonical::NOFILE,
    )
    .map_err(|_| Unavailable)?;
    let archive = synthetic_backup(until)?;
    // Two owner/ledger/channel ceilings are reserved in this closed plan before
    // effects. Each child additionally reserves actual buffers before READY.
    let mut nonces = [[0; 32]; 2];
    let sentinel = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(SENTINEL)
        .map_err(|_| Unavailable)?;
    sentinel.sync_all().map_err(|_| Unavailable)?;
    let listener = UnixListener::bind(CHANNEL).map_err(|_| Unavailable)?;
    listener.set_nonblocking(true).map_err(|_| Unavailable)?;
    let mut entropy = File::from(
        open(
            "/dev/urandom",
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Unavailable)?,
    );
    for nonce in &mut nonces {
        entropy.read_exact(nonce).map_err(|_| Unavailable)?;
    }
    if nonces[0] == [0; 32] || nonces[1] == [0; 32] || nonces[0] == nonces[1] {
        return Err(Unavailable);
    }
    let mut owner = PairSupervisor {
        children: [None, None],
        pidfds: [None, None],
        streams: [None, None],
        attempted: [false, false],
        listener,
        _sentinel: sentinel,
        _entropy: entropy,
    };
    emit(b"t4_service_pair_capacity_reserved\n", until)?;
    owner.spawn(0, "--actor-mixed-writer", until)?;
    let mut writer = owner.ready(0, nonces[0], until)?;
    emit(b"t4_service_mixed_writer_ready\n", until)?;
    let stream = owner.streams[0].as_mut().ok_or(Unavailable)?;
    exchange(
        stream,
        &mut writer,
        Kind::ObserveStopped,
        RequestShape::Exact,
        until,
    )?;
    exchange_backup(stream, &mut writer, &archive, SYNTHETIC_PASSPHRASE, until)?;
    let request = writer.begin(Kind::InterruptMixed)?;
    io_frame(stream, Some(request), until)?;
    // Only the predeclared original86 admits another actor. No expected wire
    // completion, compensation or generic nonzero-as-known-effect inference.
    owner.wait_original(0, 86, until)?;
    writer.revoke();
    emit(b"t4_service_original_writer_86_reaped\n", until)?;
    owner.spawn(1, "--actor-inspector", until)?;
    let mut inspector = owner.ready(1, nonces[1], until)?;
    emit(b"t4_service_fresh_inspector_ready\n", until)?;
    let stream = owner.streams[1].as_mut().ok_or(Unavailable)?;
    exchange(
        stream,
        &mut inspector,
        Kind::ObserveStopped,
        RequestShape::Exact,
        until,
    )?;
    exchange_backup(
        stream,
        &mut inspector,
        &archive,
        SYNTHETIC_PASSPHRASE,
        until,
    )?;
    let request = inspector.begin(Kind::InspectInterrupted)?;
    io_frame(stream, Some(request), until)?;
    let transaction: [u8; 16] = sha2::Sha256::digest(nonces[0])[..16]
        .try_into()
        .map_err(|_| Unavailable)?;
    let mut done = 0;
    while done < transaction.len() {
        tick(until)?;
        match stream.write(&transaction[done..]) {
            Ok(0) => return Err(Unavailable),
            Ok(size) => done += size,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(_) => return Err(Unavailable),
        }
        tick(until)?;
    }
    inspector.completed(receive(stream, until)?, Kind::InterruptedInspected)?;
    alive(owner.pidfds[1].as_ref().ok_or(Unavailable)?)?;
    emit(b"t4_service_intent_mixed_candidate_inspected\n", until)?;
    exchange(
        stream,
        &mut inspector,
        Kind::Halt,
        RequestShape::Exact,
        until,
    )?;
    owner.wait_original(1, 0, until)?;
    emit(b"t4_service_interrupted_inspection_completed\n", until)
}

/// Separate fixed-admin developer mode. Never selected by the classic actor
/// wire and never changes that mode's64-FD envelope or PID1 predicates.
pub fn actor_canonical_entry() -> Result<(), Unavailable> {
    canonical_actor(ActorRole::Normal)
}
pub fn actor_mixed_writer_entry() -> Result<(), Unavailable> {
    canonical_actor(ActorRole::MixedWriter)
}
pub fn actor_inspector_entry() -> Result<(), Unavailable> {
    canonical_actor(ActorRole::Inspector)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActorRole {
    Normal,
    MixedWriter,
    Inspector,
}
impl ActorRole {
    fn observer(self) -> ObserverRole {
        match self {
            Self::Normal => ObserverRole::Canonical,
            Self::MixedWriter => ObserverRole::MixedWriter,
            Self::Inspector => ObserverRole::Inspector,
        }
    }
}
fn canonical_actor(role: ActorRole) -> Result<(), Unavailable> {
    startup()?;
    epoch()?;
    canonical_groups(
        || nix::unistd::setgroups(&[]).map_err(|_| Unavailable),
        || {
            nix::unistd::getgroups()
                .map(|groups| groups.is_empty())
                .map_err(|_| Unavailable)
        },
    )?;
    setrlimit(
        Resource::RLIMIT_NOFILE,
        actor_canonical::NOFILE,
        actor_canonical::NOFILE,
    )
    .map_err(|_| Unavailable)?;
    if getrlimit(Resource::RLIMIT_NOFILE).map_err(|_| Unavailable)?
        != (actor_canonical::NOFILE, actor_canonical::NOFILE)
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
    let mut canonical = match role {
        ActorRole::Normal => Canonical::reserve(),
        ActorRole::MixedWriter | ActorRole::Inspector => Canonical::reserve_role(role.observer()),
    }?; // BEFORE READY/proc/query; normal constructor remains unchanged
    let mut transfer = transfer::Transfer::new()?; // finite private slot BEFORE READY
    let mut stage = stage::Stage::reserve_canonical(); // all36 lower roles BEFORE READY
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
        let result = (|| {
            let request = receive(&mut channel, until)?;
            let kind = request.kind;
            let expected = context.begin(kind)?;
            if request.sequence != expected.sequence || request.nonce != expected.nonce {
                return Err(Unavailable);
            }
            match role {
                ActorRole::Normal
                    if matches!(kind, Kind::InterruptMixed | Kind::InspectInterrupted) =>
                {
                    return Err(Unavailable);
                }
                ActorRole::MixedWriter
                    if !matches!(
                        kind,
                        Kind::ObserveStopped | Kind::AuthenticateBackup | Kind::InterruptMixed
                    ) =>
                {
                    return Err(Unavailable);
                }
                ActorRole::Inspector
                    if !matches!(
                        kind,
                        Kind::ObserveStopped
                            | Kind::AuthenticateBackup
                            | Kind::InspectInterrupted
                            | Kind::Halt
                    ) =>
                {
                    return Err(Unavailable);
                }
                _ => (),
            }
            stage.permit_request(kind)?; // BEFORE every acquisition/normalization match
            let reply = match kind {
                Kind::ObserveStopped => {
                    canonical.observe(until)?;
                    emit_actor(b"t4_actor_canonical_stopped_observed\n", until)?;
                    Kind::StoppedObserved
                }
                Kind::AuthenticateBackup => {
                    transfer.receive_fenced(&mut channel, until, |phase| match phase {
                        transfer::AuthenticationFence::BeforeInput => {
                            canonical.begin_authentication(until)
                        }
                        transfer::AuthenticationFence::AfterCrypto => {
                            canonical.complete_authentication(until)
                        }
                    })?;
                    emit_actor(b"t4_actor_backup_authenticated\n", until)?;
                    Kind::BackupAuthenticated
                }
                Kind::StageAuthenticatedBackup => {
                    emit_actor(stage::SUCCESS_PHASES[0], until)?;
                    transfer.with_restore_pair(until, |new_store, new_template| {
                        stage.record_canonical(
                            [SYNTHETIC_STORE, SYNTHETIC_TEMPLATE, new_store, new_template],
                            &context.nonce,
                            &mut canonical,
                            until,
                        )
                    })?;
                    emit_actor(stage::SUCCESS_PHASES[5], until)?;
                    Kind::StageRecorded
                }
                Kind::CommitAuthenticatedBackup => {
                    emit_actor(stage::SUCCESS_PHASES[0], until)?;
                    transfer.with_restore_pair(until, |new_store, new_template| {
                        stage.commit_canonical(
                            [SYNTHETIC_STORE, SYNTHETIC_TEMPLATE, new_store, new_template],
                            &context.nonce,
                            &mut canonical,
                            until,
                        )
                    })?;
                    emit_actor(stage::COMMITTED_PHASE, until)?;
                    Kind::PairCommitted
                }
                Kind::InterruptMixed => {
                    emit_actor(stage::SUCCESS_PHASES[0], until)?;
                    transfer.with_restore_pair(until, |new_store, new_template| {
                        stage.mixed_canonical(
                            [
                                INTERRUPTED_OLD_STORE,
                                INTERRUPTED_OLD_TEMPLATE,
                                new_store,
                                new_template,
                            ],
                            &context.nonce,
                            &mut canonical,
                            until,
                        )
                    })?;
                    emit_actor(stage::MIXED_PHASE, until)?;
                    tick(until)?;
                    // Deliberate owned process-exit cut, not an Err transition,
                    // signal, Rust Drop cleanup or claim of fatal FD custody.
                    std::process::exit(86);
                }
                Kind::InspectInterrupted => {
                    let mut transaction = [0; 16];
                    let mut done = 0;
                    while done < transaction.len() {
                        tick(until)?;
                        match channel.read(&mut transaction[done..]) {
                            Ok(0) => return Err(Unavailable),
                            Ok(size) => done += size,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(1))
                            }
                            Err(_) => return Err(Unavailable),
                        }
                        tick(until)?;
                    }
                    emit_actor(stage::INSPECTION_PHASES[0], until)?;
                    transfer.with_restore_pair(until, |new_store, new_template| {
                        let result = stage.inspect_canonical(
                            [
                                INTERRUPTED_OLD_STORE,
                                INTERRUPTED_OLD_TEMPLATE,
                                new_store,
                                new_template,
                            ],
                            transaction,
                            &mut canonical,
                            until,
                        )?;
                        if !result.mixed_intent_candidate() {
                            return Err(Unavailable);
                        }
                        Ok(())
                    })?;
                    emit_actor(stage::INSPECTION_PHASES[1], until)?;
                    Kind::InterruptedInspected
                }
                Kind::Halt => {
                    stage.finish()?;
                    canonical.finish()?;
                    transfer.finish();
                    Kind::Closed
                }
                _ => return Err(Unavailable),
            };
            io_frame(
                &mut channel,
                Some(Frame {
                    kind: reply,
                    sequence: context.sequence,
                    nonce: context.nonce,
                }),
                until,
            )?;
            context.completed(
                Frame {
                    kind: reply,
                    sequence: context.sequence,
                    nonce: context.nonce,
                },
                reply,
            )?;
            Ok(kind == Kind::Halt)
        })();
        match result {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(_) => {
                canonical_refusal(&mut context, &mut canonical, &mut stage);
                // Owner and channel remain outside the fallible closure. No
                // query, read, reply, reap or retry after uncertainty.
                let _held = canonical;
                let _private = transfer;
                let _lower = stage;
                let _channel = channel;
                loop {
                    std::thread::park();
                }
            }
        }
    }
}

fn canonical_refusal(context: &mut Context, canonical: &mut Canonical, stage: &mut stage::Stage) {
    context.revoke();
    canonical.revoke();
    stage.revoke();
}

#[cfg(test)]
pub(crate) fn stage_failure_projection(
    original: Result<(), Unavailable>,
    frames: &mut Vec<&'static [u8]>,
) -> Result<(), Unavailable> {
    stage::test_failure_projection(original, frames)
}

fn canonical_groups(
    clear: impl FnOnce() -> Result<(), Unavailable>,
    empty: impl FnOnce() -> Result<bool, Unavailable>,
) -> Result<(), Unavailable> {
    // Only this standalone actor's supplementary groups change; its root UID,
    // primary GID and namespaces do not. No host account/group database write.
    clear()?;
    if !empty()? {
        return Err(Unavailable);
    }
    Ok(())
}

fn quarantine(
    _held: Retained,
    _transfer: transfer::Transfer,
    _stage: stage::Stage,
    _channel: UnixStream,
) -> ! {
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
    let mut transfer = transfer::Transfer::new()?;
    let mut stage = stage::Stage::reserve()?; // full fixed ledger/buffer before READY
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
        let result = actor_operation(
            &mut channel,
            &mut context,
            &mut held,
            &mut transfer,
            &mut stage,
            until,
        );
        match result {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(_) => {
                context.revoke();
                quarantine(held, transfer, stage, channel);
            }
        }
    }
}

fn actor_operation(
    channel: &mut UnixStream,
    context: &mut Context,
    held: &mut Retained,
    transfer: &mut transfer::Transfer,
    stage: &mut stage::Stage,
    until: Instant,
) -> Result<bool, Unavailable> {
    let request = receive(channel, until)?;
    let kind = request.kind;
    let expected = context.begin(kind)?;
    if request.sequence != expected.sequence || request.nonce != expected.nonce {
        context.revoke();
        return Err(Unavailable);
    }
    if stage.permit_request(kind).is_err() {
        context.revoke();
        return Err(Unavailable);
    }
    match kind {
        Kind::StageAuthenticatedBackup => {
            transfer.admit(until)?;
            held.observe(until).map_err(|_| Unavailable)?;
            transfer.receive(channel, until)?;
            held.transaction_fence(until).map_err(|_| Unavailable)?;
            emit_actor(stage::SUCCESS_PHASES[0], until)?;
            transfer.with_restore_pair(until, |new_store, new_template| {
                stage.record(
                    [SYNTHETIC_STORE, SYNTHETIC_TEMPLATE, new_store, new_template],
                    &context.nonce,
                    held,
                    until,
                )
            })?;
            emit_actor(stage::SUCCESS_PHASES[5], until)?;
            let reply = Kind::StageRecorded;
            io_frame(
                channel,
                Some(Frame {
                    kind: reply,
                    sequence: context.sequence,
                    nonce: context.nonce,
                }),
                until,
            )?;
            context.completed(
                Frame {
                    kind: reply,
                    sequence: context.sequence,
                    nonce: context.nonce,
                },
                reply,
            )?;
            Ok(false)
        }
        Kind::AuthenticateBackup => {
            transfer.admit(until)?;
            held.observe(until).map_err(|_| Unavailable)?;
            transfer.receive(channel, until)?;
            held.recheck_original(until).map_err(|_| Unavailable)?;
            // Positive authenticated payload remains inside this original
            // actor borrow. A fixed acknowledgement is not restore authority.
            tick(until)?;
            {
                let mut output = std::io::stderr().lock();
                output
                    .write_all(b"t4_actor_backup_authenticated\n")
                    .map_err(|_| Unavailable)?;
                output.flush().map_err(|_| Unavailable)?;
            }
            tick(until)?;
            io_frame(
                channel,
                Some(Frame {
                    kind: Kind::BackupAuthenticated,
                    sequence: context.sequence,
                    nonce: context.nonce,
                }),
                until,
            )?;
            context.completed(
                Frame {
                    kind: Kind::BackupAuthenticated,
                    sequence: context.sequence,
                    nonce: context.nonce,
                },
                Kind::BackupAuthenticated,
            )?;
            Ok(false)
        }
        Kind::ObserveManager => {
            held.observe(until).map_err(|_| Unavailable)?;
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
            context.completed(
                Frame {
                    kind: Kind::Completed,
                    sequence: context.sequence,
                    nonce: context.nonce,
                },
                Kind::Completed,
            )?;
            Ok(false)
        }
        Kind::Halt => {
            // Cleanup occurs only on a separate valid request while Live, after
            // prior operations completed. No uncertain owner can enter here.
            // Release recorded originals before replying. Ordinary File close
            // backend/fatal loss is not a per-FD kernel absence attestation.
            stage.finish()?;
            held.finish().map_err(|_| Unavailable)?;
            transfer.finish();
            io_frame(
                channel,
                Some(Frame {
                    kind: Kind::Closed,
                    sequence: context.sequence,
                    nonce: context.nonce,
                }),
                until,
            )?;
            context.completed(
                Frame {
                    kind: Kind::Closed,
                    sequence: context.sequence,
                    nonce: context.nonce,
                },
                Kind::Closed,
            )?;
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
    fn worker_entry_role_binds_only_its_own_observer_expectation() {
        assert!(ActorRole::Normal.observer() == ObserverRole::Canonical);
        assert!(ActorRole::MixedWriter.observer() == ObserverRole::MixedWriter);
        assert!(ActorRole::Inspector.observer() == ObserverRole::Inspector);
        assert!(!include_str!("main.rs").contains("--actor-inspector"));
    }

    #[test]
    fn planned_original_86_is_classified_before_reap_and_all_late_cuts_stop() {
        use nix::sys::wait::WaitStatus;
        use nix::unistd::Pid as NixPid;
        for outcome in [
            WaitStatus::Exited(NixPid::from_raw(123), 0),
            WaitStatus::Exited(NixPid::from_raw(124), 86),
            WaitStatus::Signaled(
                NixPid::from_raw(123),
                nix::sys::signal::Signal::SIGTERM,
                false,
            ),
            WaitStatus::StillAlive,
        ] {
            let mut reaped = false;
            assert!(
                settle_original(
                    123,
                    86,
                    || Ok(()),
                    || Ok(outcome),
                    || {
                        reaped = true;
                        Ok(Some(86))
                    }
                )
                .is_err()
            );
            assert!(!reaped);
        }
        for cut in 0..5 {
            let calls = std::cell::Cell::new(0);
            let mut reaped = false;
            let result = settle_original(
                123,
                86,
                || {
                    let n = calls.get();
                    calls.set(n + 1);
                    if cut < 3 && n == cut {
                        Err(Unavailable)
                    } else {
                        Ok(())
                    }
                },
                || {
                    if cut == 3 {
                        Err(Unavailable)
                    } else {
                        Ok(WaitStatus::Exited(NixPid::from_raw(123), 86))
                    }
                },
                || {
                    reaped = true;
                    if cut == 4 {
                        Err(Unavailable)
                    } else {
                        Ok(Some(86))
                    }
                },
            );
            assert!(result.is_err());
            assert_eq!(reaped, cut == 2 || cut == 4);
        }
        assert!(
            settle_original(
                123,
                86,
                || Ok(()),
                || Ok(WaitStatus::Exited(NixPid::from_raw(123), 86)),
                || Ok(Some(86))
            )
            .is_ok()
        );
        assert!(
            settle_original(
                123,
                86,
                || Ok(()),
                || Ok(WaitStatus::Exited(NixPid::from_raw(123), 86)),
                || Ok(None)
            )
            .is_err()
        );
    }

    #[test]
    fn interrupted_fixture_changes_both_members_and_keeps_product_entry_unregistered() {
        assert_ne!(INTERRUPTED_OLD_STORE, SYNTHETIC_STORE);
        assert_ne!(INTERRUPTED_OLD_TEMPLATE, SYNTHETIC_TEMPLATE);
        assert!(serde_json::from_slice::<serde_json::Value>(INTERRUPTED_OLD_STORE).is_ok());
        assert_eq!(
            DeveloperScenario::CanonicalInterruptedInspection.observations(),
            0
        );
        assert!(
            DeveloperScenario::CanonicalInterruptedInspection
                .request_shape()
                .is_none()
        );
        assert!(!include_str!("main.rs").contains("--inspect-fixed-interrupted-transaction"));
    }

    #[test]
    fn canonical_commit_uses_one_already_authenticated_pair_and_a_distinct_reply() {
        assert_eq!(DeveloperScenario::CanonicalCommit.observations(), 0);
        assert!(DeveloperScenario::CanonicalCommit.request_shape().is_none());
        let mut context = live_context();
        for (kind, completed, sequence) in [
            (Kind::ObserveStopped, Kind::StoppedObserved, 1),
            (Kind::AuthenticateBackup, Kind::BackupAuthenticated, 2),
            (Kind::CommitAuthenticatedBackup, Kind::PairCommitted, 3),
            (Kind::Halt, Kind::Closed, 4),
        ] {
            let mut memory = Memory {
                input: reply(completed, sequence),
                output: Vec::new(),
                position: 0,
                fail_at: usize::MAX,
            };
            let until = Instant::now() + Duration::from_secs(1);
            if kind == Kind::AuthenticateBackup {
                exchange_backup(
                    &mut memory,
                    &mut context,
                    b"synthetic archive",
                    SYNTHETIC_PASSPHRASE,
                    until,
                )
                .unwrap();
            } else {
                exchange(&mut memory, &mut context, kind, RequestShape::Exact, until).unwrap();
                assert_eq!(memory.output.len(), FRAME_BYTES);
            }
        }
        assert_eq!(context.phase, Phase::Closed);
        assert!(context.begin(Kind::CommitAuthenticatedBackup).is_err());
        assert!(!include_str!("main.rs").contains("--commit-canonical-synthetic-backup"));
    }

    #[test]
    fn commit_reply_cuts_revoke_same_originals_before_halt_without_compensation() {
        for cut in 0..FRAME_BYTES {
            let mut context = live_context();
            let mut canonical = Canonical::reserve().unwrap();
            let mut stage = stage::Stage::reserve_canonical();
            let mut memory = Memory {
                input: reply(Kind::PairCommitted, 1),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            assert!(
                exchange(
                    &mut memory,
                    &mut context,
                    Kind::CommitAuthenticatedBackup,
                    RequestShape::Exact,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            canonical_refusal(&mut context, &mut canonical, &mut stage);
            let written = memory.output.len();
            assert!(context.begin(Kind::Halt).is_err());
            assert!(context.begin(Kind::StageAuthenticatedBackup).is_err());
            assert!(canonical.finish().is_err());
            assert!(stage.finish().is_err());
            assert_eq!(memory.output.len(), written);
        }
    }

    #[test]
    fn canonical_stage_reply_or_output_error_revokes_all_original_owners_before_halt() {
        for cut in 0..FRAME_BYTES {
            let mut context = live_context();
            let mut canonical = Canonical::reserve().unwrap(); // no backend originals
            let mut stage = stage::Stage::reserve_canonical();
            let mut memory = Memory {
                input: reply(Kind::StageRecorded, 1),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            let until = Instant::now() + Duration::from_secs(1);
            assert!(
                exchange(
                    &mut memory,
                    &mut context,
                    Kind::StageAuthenticatedBackup,
                    RequestShape::Exact,
                    until
                )
                .is_err()
            );
            canonical_refusal(&mut context, &mut canonical, &mut stage);
            let bytes = memory.output.len();
            assert!(context.begin(Kind::Halt).is_err());
            assert!(canonical.finish().is_err());
            assert!(stage.finish().is_err());
            assert!(stage.permit_request(Kind::Halt).is_err());
            assert_eq!(memory.output.len(), bytes);
            let mut context = live_context();
            let pending = context.begin(Kind::StageAuthenticatedBackup).unwrap();
            let mut output = Memory {
                input: Vec::new(),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            assert!(
                io_frame(
                    &mut output,
                    Some(Frame {
                        kind: Kind::StageRecorded,
                        sequence: pending.sequence,
                        nonce: pending.nonce
                    }),
                    until
                )
                .is_err()
            );
            canonical_refusal(&mut context, &mut canonical, &mut stage);
            assert!(context.begin(Kind::Halt).is_err());
            assert!(canonical.finish().is_err() && stage.finish().is_err());
        }
        assert_eq!(DeveloperScenario::CanonicalStage.observations(), 0);
        assert!(DeveloperScenario::CanonicalStage.request_shape().is_none());
    }

    #[test]
    fn canonical_sequence_stages_only_the_already_private_authenticated_pair() {
        let mut context = live_context();
        for (kind, completed, sequence) in [
            (Kind::ObserveStopped, Kind::StoppedObserved, 1),
            (Kind::AuthenticateBackup, Kind::BackupAuthenticated, 2),
            (Kind::StageAuthenticatedBackup, Kind::StageRecorded, 3),
            (Kind::Halt, Kind::Closed, 4),
        ] {
            let mut memory = Memory {
                input: reply(completed, sequence),
                output: Vec::new(),
                position: 0,
                fail_at: usize::MAX,
            };
            let until = Instant::now() + Duration::from_secs(1);
            if kind == Kind::AuthenticateBackup {
                exchange_backup(
                    &mut memory,
                    &mut context,
                    b"synthetic archive",
                    SYNTHETIC_PASSPHRASE,
                    until,
                )
                .unwrap();
            } else {
                exchange(&mut memory, &mut context, kind, RequestShape::Exact, until).unwrap();
                assert_eq!(memory.output.len(), FRAME_BYTES); // no second Stage body/FD
            }
            let sent = Frame::decode(&memory.output[..FRAME_BYTES]).unwrap();
            assert_eq!(
                (sent.kind, sent.sequence, sent.nonce),
                (kind, sequence, context.nonce)
            );
        }
        assert_eq!(context.phase, Phase::Closed);
        assert!(context.begin(Kind::StageAuthenticatedBackup).is_err());
    }

    #[test]
    fn canonical_stage_stays_out_of_the_normal_runtime_entry() {
        let normal = include_str!("main.rs");
        assert!(!normal.contains("--stage-canonical-synthetic-backup"));
        assert!(!normal.contains("actor_canonical_entry"));
        let manifest = include_str!("../Cargo.toml");
        assert!(manifest.contains("required-features = [\"t4-manager-actor-service\"]"));
        assert!(include_str!("lib.rs").contains(
            "#[cfg(feature = \"t4-manager-actor-service\")]\npub mod manager_actor_service;"
        ));
    }
    #[test]
    fn canonical_groups_clear_then_verify_before_admission() {
        let calls = std::cell::RefCell::new(Vec::new());
        canonical_groups(
            || {
                calls.borrow_mut().push("clear");
                Ok(())
            },
            || {
                calls.borrow_mut().push("empty");
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(*calls.borrow(), ["clear", "empty"]);
    }
    #[test]
    fn canonical_group_failures_never_admit_or_fall_back() {
        assert!(
            canonical_groups(|| Err(Unavailable), || panic!("read after failed clear"),).is_err()
        );
        for read in [Ok(false), Err(Unavailable)] {
            let calls = std::cell::Cell::new(0);
            assert!(
                canonical_groups(
                    || {
                        calls.set(calls.get() + 1);
                        Ok(())
                    },
                    || {
                        calls.set(calls.get() + 1);
                        read
                    },
                )
                .is_err()
            );
            assert_eq!(calls.get(), 2);
        }
    }
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

    fn live_context() -> Context {
        let mut context = Context::new([1; 32]).unwrap();
        context
            .ready(Frame {
                kind: Kind::Ready,
                sequence: 0,
                nonce: [1; 32],
            })
            .unwrap();
        context
    }

    fn reply(kind: Kind, sequence: u32) -> Vec<u8> {
        Frame {
            kind,
            sequence,
            nonce: [1; 32],
        }
        .encode()
        .unwrap()
        .to_vec()
    }

    fn assert_no_reentry(memory: &mut Memory, context: &mut Context) {
        assert_eq!(context.phase, Phase::Revoked);
        let output = memory.output.clone();
        let read = memory.position;
        assert!(
            exchange(
                memory,
                context,
                Kind::ObserveManager,
                RequestShape::Exact,
                Instant::now() + Duration::from_secs(1)
            )
            .is_err()
        );
        assert_eq!(memory.output, output);
        assert_eq!(memory.position, read);
        assert!(
            context
                .completed(
                    Frame {
                        kind: Kind::Completed,
                        sequence: 1,
                        nonce: [1; 32]
                    },
                    Kind::Completed
                )
                .is_err()
        );
    }

    #[test]
    fn every_exchange_request_or_reply_cut_permanently_consumes_capability() {
        for cut in 0..FRAME_BYTES {
            let mut context = live_context();
            let mut memory = Memory {
                input: reply(Kind::Completed, 1),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            assert!(
                exchange(
                    &mut memory,
                    &mut context,
                    Kind::ObserveManager,
                    RequestShape::Exact,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(memory.output.len(), cut);
            assert_eq!(memory.position, 0);
            assert_no_reentry(&mut memory, &mut context);

            let mut context = live_context();
            let mut memory = Memory {
                input: reply(Kind::Completed, 1)[..cut].to_vec(),
                output: Vec::new(),
                position: 0,
                fail_at: FRAME_BYTES,
            };
            assert!(
                exchange(
                    &mut memory,
                    &mut context,
                    Kind::ObserveManager,
                    RequestShape::Exact,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(memory.output.len(), FRAME_BYTES);
            assert_eq!(memory.position, cut);
            assert_no_reentry(&mut memory, &mut context);
        }
    }

    #[test]
    fn malformed_decoded_reply_and_deadline_refuse_all_downstream_effects() {
        for offset in [0, 8, 45, 63] {
            let mut raw = reply(Kind::Completed, 1);
            raw[offset] = 255;
            let mut memory = Memory {
                input: raw,
                output: Vec::new(),
                position: 0,
                fail_at: FRAME_BYTES,
            };
            let mut context = live_context();
            assert!(
                exchange(
                    &mut memory,
                    &mut context,
                    Kind::ObserveManager,
                    RequestShape::Exact,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_no_reentry(&mut memory, &mut context);
        }
        let mut memory = Memory {
            input: reply(Kind::Completed, 1),
            output: Vec::new(),
            position: 0,
            fail_at: FRAME_BYTES,
        };
        let mut context = live_context();
        assert!(
            exchange(
                &mut memory,
                &mut context,
                Kind::ObserveManager,
                RequestShape::Exact,
                Instant::now() - Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(memory.output.is_empty());
        assert_no_reentry(&mut memory, &mut context);
    }

    #[test]
    fn exact_completed_operation_then_halt_have_operation_bound_replies() {
        let mut context = live_context();
        for (kind, completed, sequence) in [
            (Kind::ObserveManager, Kind::Completed, 1),
            (Kind::Halt, Kind::Closed, 2),
        ] {
            let mut memory = Memory {
                input: reply(completed, sequence),
                output: Vec::new(),
                position: 0,
                fail_at: FRAME_BYTES,
            };
            exchange(
                &mut memory,
                &mut context,
                kind,
                RequestShape::Exact,
                Instant::now() + Duration::from_secs(1),
            )
            .unwrap();
            let sent = Frame::decode(&memory.output).unwrap();
            assert_eq!(sent.kind, kind);
            assert_eq!(sent.sequence, sequence);
        }
        assert_eq!(context.phase, Phase::Closed);
    }

    #[test]
    fn same_original_context_observe_private_auth_then_halt_are_kind_and_sequence_bound() {
        let mut context = live_context();
        for (kind, completed, sequence) in [
            (Kind::ObserveStopped, Kind::StoppedObserved, 1),
            (Kind::AuthenticateBackup, Kind::BackupAuthenticated, 2),
            (Kind::Halt, Kind::Closed, 3),
        ] {
            let mut memory = Memory {
                input: reply(completed, sequence),
                output: Vec::new(),
                position: 0,
                fail_at: usize::MAX,
            };
            let until = Instant::now() + Duration::from_secs(1);
            if kind == Kind::AuthenticateBackup {
                exchange_backup(
                    &mut memory,
                    &mut context,
                    b"public synthetic archive",
                    SYNTHETIC_PASSPHRASE,
                    until,
                )
                .unwrap();
            } else {
                exchange(&mut memory, &mut context, kind, RequestShape::Exact, until).unwrap();
            }
            let sent = Frame::decode(&memory.output[..FRAME_BYTES]).unwrap();
            assert_eq!((sent.kind, sent.sequence), (kind, sequence));
            assert_eq!(sent.nonce, context.nonce);
        }
        assert_eq!(context.phase, Phase::Closed);
        assert_eq!(DeveloperScenario::CanonicalAuthenticate.observations(), 0);
        assert_eq!(
            DeveloperScenario::CanonicalAuthenticate.request_shape(),
            None
        );
    }

    #[test]
    fn closed_developer_fault_shapes_cannot_pass_even_with_unexpected_reply() {
        for shape in [RequestShape::WrongNonce, RequestShape::Partial] {
            let mut memory = Memory {
                input: reply(Kind::Completed, 1),
                output: Vec::new(),
                position: 0,
                fail_at: FRAME_BYTES,
            };
            let mut context = live_context();
            assert!(
                exchange(
                    &mut memory,
                    &mut context,
                    Kind::ObserveManager,
                    shape,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(
                memory.output.len(),
                if shape == RequestShape::Partial {
                    32
                } else {
                    64
                }
            );
            if shape == RequestShape::WrongNonce {
                assert_ne!(Frame::decode(&memory.output).unwrap().nonce, context.nonce);
            }
            assert_no_reentry(&mut memory, &mut context);
        }
    }

    #[test]
    fn capacity_and_fault_scenarios_have_no_path_target_or_reset_parameter() {
        assert_eq!(DeveloperScenario::Single.observations(), 1);
        assert_eq!(DeveloperScenario::CapacityThree.observations(), 3);
        assert_eq!(DeveloperScenario::CapacityFourth.observations(), 4);
        assert_eq!(DeveloperScenario::AuthenticateBackup.observations(), 0);
        assert_eq!(
            DeveloperScenario::StageAuthenticatedBackup.observations(),
            0
        );
        assert_eq!(
            DeveloperScenario::WrongNonceAfterFirst.request_shape(),
            Some(RequestShape::WrongNonce)
        );
        assert_eq!(
            DeveloperScenario::PartialAfterFirst.request_shape(),
            Some(RequestShape::Partial)
        );
        assert_eq!(
            DeveloperScenario::DisconnectAfterFirst.request_shape(),
            None
        );
        assert_eq!(DeveloperScenario::DisconnectAfterFirst.observations(), 1);
    }

    #[test]
    fn private_fixture_stage_reply_and_every_prefix_consume_only_original_operation() {
        let archive = b"bad";
        let passphrase = b"synthetic transfer passphrase";
        let total = FRAME_BYTES + 16 + archive.len() + passphrase.len();
        for cut in 0..total + FRAME_BYTES {
            let write_cut = if cut < total { cut } else { usize::MAX };
            let reply_cut = if cut < total {
                FRAME_BYTES
            } else {
                cut - total
            };
            let mut memory = Memory {
                input: reply(Kind::StageRecorded, 1)[..reply_cut].to_vec(),
                output: Vec::new(),
                position: 0,
                fail_at: write_cut,
            };
            let mut context = live_context();
            assert!(
                exchange_private(
                    &mut memory,
                    &mut context,
                    Kind::StageAuthenticatedBackup,
                    archive,
                    passphrase,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_no_reentry(&mut memory, &mut context);
        }
        for completed in [
            Kind::StageRecorded,
            Kind::BackupAuthenticated,
            Kind::Completed,
            Kind::Closed,
        ] {
            let mut memory = Memory {
                input: reply(completed, 1),
                output: Vec::new(),
                position: 0,
                fail_at: usize::MAX,
            };
            let mut context = live_context();
            let result = exchange_private(
                &mut memory,
                &mut context,
                Kind::StageAuthenticatedBackup,
                archive,
                passphrase,
                Instant::now() + Duration::from_secs(1),
            );
            assert_eq!(result.is_ok(), completed == Kind::StageRecorded);
            if result.is_err() {
                assert_no_reentry(&mut memory, &mut context);
            } else {
                assert_eq!(context.phase, Phase::Live);
            }
        }
    }

    #[test]
    fn private_transfer_exchange_every_write_or_reply_prefix_consumes_context() {
        let archive = b"bad";
        let passphrase = b"synthetic transfer passphrase";
        let total = FRAME_BYTES + 16 + archive.len() + passphrase.len();
        for cut in 0..total {
            let mut memory = Memory {
                input: reply(Kind::BackupAuthenticated, 1),
                output: Vec::new(),
                position: 0,
                fail_at: cut,
            };
            let mut context = live_context();
            assert!(
                exchange_backup(
                    &mut memory,
                    &mut context,
                    archive,
                    passphrase,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(context.phase, Phase::Revoked);
            let effects = (memory.output.len(), memory.position);
            assert!(
                exchange_backup(
                    &mut memory,
                    &mut context,
                    archive,
                    passphrase,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!((memory.output.len(), memory.position), effects);
        }
        for cut in 0..FRAME_BYTES {
            let mut memory = Memory {
                input: reply(Kind::BackupAuthenticated, 1)[..cut].to_vec(),
                output: Vec::new(),
                position: 0,
                fail_at: usize::MAX,
            };
            let mut context = live_context();
            assert!(
                exchange_backup(
                    &mut memory,
                    &mut context,
                    archive,
                    passphrase,
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(context.phase, Phase::Revoked);
            assert_no_reentry(&mut memory, &mut context);
        }
    }

    #[test]
    fn private_transfer_reply_is_kind_bound_and_expiry_has_no_first_write() {
        for wrong in [Kind::Completed, Kind::Closed, Kind::Rejected] {
            let mut memory = Memory {
                input: reply(wrong, 1),
                output: Vec::new(),
                position: 0,
                fail_at: usize::MAX,
            };
            let mut context = live_context();
            assert!(
                exchange_backup(
                    &mut memory,
                    &mut context,
                    b"bad",
                    b"synthetic transfer passphrase",
                    Instant::now() + Duration::from_secs(1)
                )
                .is_err()
            );
            assert_eq!(context.phase, Phase::Revoked);
        }
        let mut memory = Memory {
            input: Vec::new(),
            output: Vec::new(),
            position: 0,
            fail_at: usize::MAX,
        };
        let mut context = live_context();
        assert!(
            exchange_backup(
                &mut memory,
                &mut context,
                b"bad",
                b"synthetic transfer passphrase",
                Instant::now() - Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(memory.output.is_empty());
        assert_eq!(context.phase, Phase::Revoked);
    }
}
