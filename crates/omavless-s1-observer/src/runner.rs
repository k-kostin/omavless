// SPDX-License-Identifier: MIT

//! Inactive, fixed-purpose parent for the private read-only helper. No runtime
//! crate imports this module, and no path or argument comes from an IPC client.

use crate::{MAX_FRAME, Observation};
use nix::fcntl::{FcntlArg, OFlag, fcntl, open, openat};
use nix::poll::{PollFd, PollFlags, poll};
use nix::sys::signal::{Signal, killpg};
use nix::sys::stat::Mode;
use nix::unistd::Pid;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// Packaging this helper is a future explicit decision. The path is never
// supplied by a caller, a setting, or the process environment.
const HELPER_PATH: &str = "/usr/lib/omavless/omavless-s1-observer";
const HELPER_ARG: &str = "--private-observe-v1";
const TOTAL_DEADLINE: Duration = Duration::from_secs(15);
const MAX_STDERR: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerError {
    HelperUnavailable,
    UnsafeIdentity,
    LaunchFailed,
    TimedOut,
    OutputTooLarge,
    ChildFailed,
    InvalidResponse,
    ReadFailed,
}

/// Read-only, opt-in candidate. Successful output remains unverified and can
/// never authorize proxy setting writes through `Observation::admit_writes`.
pub fn observe_via_fixed_runner() -> Result<Observation, RunnerError> {
    let executable = pin_root_executable(Path::new(HELPER_PATH))?;
    run_pinned(&executable, &[HELPER_ARG], TOTAL_DEADLINE)
}

fn root_owned_nonwritable(file: &File, directory: bool) -> Result<(), RunnerError> {
    let meta = file.metadata().map_err(|_| RunnerError::UnsafeIdentity)?;
    if meta.uid() != 0
        || meta.mode() & 0o022 != 0
        || if directory {
            !meta.is_dir()
        } else {
            !meta.is_file()
                || meta.mode() & 0o111 == 0
                || meta.mode() & 0o6000 != 0
                || meta.nlink() != 1
        }
    {
        return Err(RunnerError::UnsafeIdentity);
    }
    Ok(())
}

// Open each absolute path component relative to its already-verified parent.
// O_NOFOLLOW rejects symlinks and the final open file descriptor pins the inode
// across replacement of the pathname before exec.
fn pin_root_executable(path: &Path) -> Result<File, RunnerError> {
    if !path.is_absolute() {
        return Err(RunnerError::UnsafeIdentity);
    }
    let root = open(
        Path::new("/"),
        OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| RunnerError::HelperUnavailable)?;
    let mut parent = File::from(root);
    root_owned_nonwritable(&parent, true)?;
    let mut names = path.components().skip(1).peekable();
    if names.peek().is_none() {
        return Err(RunnerError::UnsafeIdentity);
    }
    while let Some(component) = names.next() {
        let Component::Normal(name) = component else {
            return Err(RunnerError::UnsafeIdentity);
        };
        let final_component = names.peek().is_none();
        let flags = if final_component {
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC
        } else {
            OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC
        };
        let opened = openat(&parent, Path::new(name), flags, Mode::empty()).map_err(|_| {
            if final_component {
                RunnerError::HelperUnavailable
            } else {
                RunnerError::UnsafeIdentity
            }
        })?;
        let mut next = File::from(opened);
        root_owned_nonwritable(&next, !final_component)?;
        if final_component {
            let mut magic = [0; 4];
            next.read_exact(&mut magic)
                .map_err(|_| RunnerError::UnsafeIdentity)?;
            if &magic != b"\x7fELF" {
                return Err(RunnerError::UnsafeIdentity);
            }
            return Ok(next);
        }
        parent = next;
    }
    Err(RunnerError::UnsafeIdentity)
}

fn nonblocking(file: &impl AsFd) -> Result<(), RunnerError> {
    let flags = fcntl(file, FcntlArg::F_GETFL).map_err(|_| RunnerError::ReadFailed)?;
    fcntl(
        file,
        FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
    )
    .map_err(|_| RunnerError::ReadFailed)?;
    Ok(())
}

fn drain(reader: &mut impl Read, buffer: &mut Vec<u8>, limit: usize) -> Result<bool, RunnerError> {
    let mut chunk = [0_u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                if count > limit.saturating_sub(buffer.len()) {
                    return Err(RunnerError::OutputTooLarge);
                }
                buffer.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(RunnerError::ReadFailed),
        }
    }
}

struct ChildGuard {
    child: Child,
    reaped: bool,
    complete: bool,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        // The child has its own process group. A descendant holding a pipe
        // cannot keep this runner waiting beyond the deadline.
        let _ = killpg(Pid::from_raw(self.child.id() as i32), Signal::SIGKILL);
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn run_pinned(
    executable: &File,
    args: &[&str],
    deadline: Duration,
) -> Result<Observation, RunnerError> {
    // Linux resolves this procfs handle to the pinned descriptor before
    // CLOEXEC closes the child's copy. No mutable pathname is executed.
    let mut command = Command::new(format!("/proc/self/fd/{}", executable.as_raw_fd()));
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.process_group(0);
    // A root-owned ELF is not an identity pin if its loader can be redirected
    // through inherited dynamic-linker knobs.
    for name in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "LD_AUDIT",
        "LD_DEBUG",
        "LD_ORIGIN_PATH",
        "LD_PROFILE",
    ] {
        command.env_remove(name);
    }
    let child = command.spawn().map_err(|_| RunnerError::LaunchFailed)?;
    let mut guard = ChildGuard {
        child,
        reaped: false,
        complete: false,
    };
    let mut stdout = guard.child.stdout.take().ok_or(RunnerError::LaunchFailed)?;
    let mut stderr = guard.child.stderr.take().ok_or(RunnerError::LaunchFailed)?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let start = Instant::now();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    loop {
        if !stdout_eof {
            stdout_eof = drain(&mut stdout, &mut out, MAX_FRAME)?;
        }
        if !stderr_eof {
            stderr_eof = drain(&mut stderr, &mut err, MAX_STDERR)?;
        }
        // Defer wait/reap until both pipes close. If a descendant keeps a
        // descriptor open, the unreaped group leader pins the group ID until
        // timeout cleanup signals it; no unrelated reused group is targeted.
        if stdout_eof
            && stderr_eof
            && let Some(status) = guard
                .child
                .try_wait()
                .map_err(|_| RunnerError::ReadFailed)?
        {
            guard.reaped = true;
            guard.complete = true;
            if !status.success() {
                return Err(RunnerError::ChildFailed);
            }
            return Observation::decode_private_frame(&out)
                .map_err(|_| RunnerError::InvalidResponse);
        }
        let remaining = deadline
            .checked_sub(start.elapsed())
            .ok_or(RunnerError::TimedOut)?;
        let timeout = remaining.min(Duration::from_millis(50)).as_millis() as u16;
        let mut fds = Vec::with_capacity(2);
        if !stdout_eof {
            fds.push(PollFd::new(stdout.as_fd(), PollFlags::POLLIN));
        }
        if !stderr_eof {
            fds.push(PollFd::new(stderr.as_fd(), PollFlags::POLLIN));
        }
        if fds.is_empty() {
            std::thread::sleep(remaining.min(Duration::from_millis(10)));
        } else {
            poll(&mut fds, timeout).map_err(|_| RunnerError::ReadFailed)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        directory: std::path::PathBuf,
        binary: std::path::PathBuf,
        frame: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let base = std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let directory = base.join(format!(
                "omavless-s1-runner-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .unwrap();
            let binary = directory.join("synthetic-observer");
            let source =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake_observer.rs");
            assert!(
                Command::new("rustc")
                    .arg("--edition=2024")
                    .arg(source)
                    .arg("-o")
                    .arg(&binary)
                    .env("TMPDIR", &directory)
                    .status()
                    .unwrap()
                    .success()
            );
            let frame = directory.join("synthetic-frame");
            std::fs::write(
                &frame,
                crate::tests::synthetic_observation()
                    .encode_private_frame()
                    .unwrap(),
            )
            .unwrap();
            Self {
                directory,
                binary,
                frame,
            }
        }

        fn run(&self, mode: &str, deadline: Duration) -> Result<Observation, RunnerError> {
            let mut binary = File::open(&self.binary).unwrap();
            let mut magic = [0_u8; 4];
            binary.read_exact(&mut magic).unwrap();
            assert_eq!(&magic, b"\x7fELF");
            run_pinned(&binary, &[mode, self.frame.to_str().unwrap()], deadline)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn fake_child_matrix_is_bounded_private_and_reaped() {
        let fixture = Fixture::new();
        let decoded = fixture.run("valid", Duration::from_secs(2)).unwrap();
        assert!(matches!(
            decoded.provenance(),
            crate::Provenance::Unverified
        ));
        assert_eq!(
            decoded.admit_writes(),
            Err(crate::Error::IdentityUnverified)
        );
        assert_eq!(
            fixture.run("malformed", Duration::from_secs(2)).err(),
            Some(RunnerError::InvalidResponse)
        );
        assert_eq!(
            fixture.run("failed", Duration::from_secs(2)).err(),
            Some(RunnerError::ChildFailed)
        );
        for mode in ["oversized-stdout", "oversized-stderr"] {
            assert_eq!(
                fixture.run(mode, Duration::from_secs(2)).err(),
                Some(RunnerError::OutputTooLarge)
            );
        }
        for mode in ["sleep", "pipe-holder"] {
            let began = Instant::now();
            assert_eq!(
                fixture.run(mode, Duration::from_millis(100)).err(),
                Some(RunnerError::TimedOut)
            );
            assert!(began.elapsed() < Duration::from_secs(1));
        }
    }

    #[test]
    fn identity_and_symlink_refuse_before_any_execution() {
        let fixture = Fixture::new();
        assert_eq!(
            pin_root_executable(&fixture.binary).err(),
            Some(RunnerError::UnsafeIdentity)
        );
        let link = fixture.directory.join("link");
        symlink(&fixture.binary, &link).unwrap();
        assert!(pin_root_executable(&link).is_err());
        assert_eq!(
            pin_root_executable(Path::new("relative-helper")).err(),
            Some(RunnerError::UnsafeIdentity)
        );
    }
}
