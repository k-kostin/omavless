// SPDX-License-Identifier: MIT
//! Fixed developer traffic only. No observer receipt, mark or Arm authority.
//! Every nonterminal Drop retains the original child AND its input/capture FDs.
use super::{HeldDirectory, HeldFile, MAX_CORE, same};
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
use nix::unistd::Pid;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PROGRAM: &str = "/usr/lib/omavless-netguard/development-native-tests";
const SELECTOR: &str = "native_host::protected_preparation::interval::tests::fixed_native_traffic";
const CAPTURE: &str = ".k1-native-interval";
const BOUND: u64 = 4096;
const BUDGET: Duration = Duration::from_secs(30);
const SUCCESS: &[u8] = b"K1_NATIVE_TRAFFIC_ZERO\n";
const TOTAL_FDS: usize = 256;
const PEAK_INTERVAL_FDS: usize = 24;

/// Software role reservation against the current whole process inventory. It
/// neither raises rlimits nor promises future kernel FD allocation cannot fail.
/// The caller already owns the exclusive idle native graph; any extra drift or
/// allocation failure refuses with the acquired prefix retained.
struct Capacity {
    ceiling: usize,
}
impl Capacity {
    fn from_inventory(count: usize, soft: u64) -> Result<Self, ()> {
        let ceiling = count.checked_add(PEAK_INTERVAL_FDS).ok_or(())?;
        if ceiling > TOTAL_FDS || ceiling as u64 > soft {
            return Err(());
        }
        Ok(Self { ceiling })
    }
    fn reserve() -> Result<Self, ()> {
        let (soft, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| ())?;
        Self::from_inventory(Self::inventory()?, soft)
    }
    fn inventory() -> Result<usize, ()> {
        let mut count = 0;
        for entry in fs::read_dir("/proc/self/fd").map_err(|_| ())? {
            entry.map_err(|_| ())?;
            count += 1;
            if count > TOTAL_FDS {
                return Err(());
            }
        }
        // Includes the directory iterator itself, conservatively.
        Ok(count)
    }
    fn check(&self) -> Result<(), ()> {
        let (soft, _) = nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE)
            .map_err(|_| ())?;
        if Self::inventory()? > self.ceiling || soft < self.ceiling as u64 {
            return Err(());
        }
        Ok(())
    }
}

struct Capture {
    file: File,
    original: Metadata,
    path: PathBuf,
}
struct AcquiredFile(Option<File>);
impl Drop for AcquiredFile {
    fn drop(&mut self) {
        if let Some(file) = self.0.take() {
            std::mem::forget(file);
        }
    }
}

/// Prepared before the data-directory original is captured. Its one original
/// moves into the interval; every failed/abandoned prefix stays retained.
pub(super) struct Scratch {
    original: Option<(HeldDirectory, PathBuf)>,
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            std::mem::forget(original);
        }
    }
}
impl Scratch {
    pub(super) fn prepare(parent: &Path, uid: u32) -> Result<Self, ()> {
        let path = parent.join(CAPTURE);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| ())?;
        // The published directory is never removed on any failure. Retain an
        // opened descriptor even when metadata/named-original checks fail.
        let mut prefix = AcquiredFile(Some(
            OpenOptions::new()
                .read(true)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY)
                .open(&path)
                .map_err(|_| ())?,
        ));
        let metadata = prefix.0.as_ref().ok_or(())?.metadata().map_err(|_| ())?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
            return Err(());
        }
        let owner = Self {
            original: Some((
                HeldDirectory {
                    file: prefix.0.take().ok_or(())?,
                    metadata,
                },
                path,
            )),
        };
        owner.recheck()?;
        Ok(owner)
    }
    pub(super) fn recheck(&self) -> Result<(), ()> {
        let (directory, path) = self.original.as_ref().ok_or(())?;
        directory.recheck(path).map_err(|_| ())
    }
}
impl Capture {
    fn create(path: PathBuf, uid: u32) -> Result<Self, ()> {
        let mut prefix = AcquiredFile(Some(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(nix::libc::O_NOFOLLOW)
                .open(&path)
                .map_err(|_| ())?,
        ));
        let file = prefix.0.as_ref().ok_or(())?;
        let original = file.metadata().map_err(|_| ())?;
        if !original.is_file()
            || original.uid() != uid
            || original.nlink() != 1
            || original.mode() & 0o7777 != 0o600
            || original.len() != 0
        {
            return Err(());
        }
        Ok(Self {
            file: prefix.0.take().ok_or(())?,
            original,
            path,
        })
    }
    fn metadata(&self) -> Result<Metadata, ()> {
        let current = self.file.metadata().map_err(|_| ())?;
        let named = fs::symlink_metadata(&self.path).map_err(|_| ())?;
        for m in [&current, &named] {
            if m.dev() != self.original.dev()
                || m.ino() != self.original.ino()
                || m.uid() != self.original.uid()
                || m.gid() != self.original.gid()
                || m.mode() != self.original.mode()
                || m.nlink() != 1
                || m.len() > BOUND
            {
                return Err(());
            }
        }
        Ok(current)
    }
    fn whole(&self) -> Result<Vec<u8>, ()> {
        let before = self.metadata()?;
        let mut raw = vec![0; BOUND as usize + 1];
        let count = self.file.read_at(&mut raw, 0).map_err(|_| ())?;
        raw.truncate(count);
        if count as u64 != before.len()
            || !same(&before, &self.file.metadata().map_err(|_| ())?)
            || !same(&before, &fs::symlink_metadata(&self.path).map_err(|_| ())?)
        {
            return Err(());
        }
        Ok(raw)
    }
}

struct Resources {
    capacity: Capacity,
    program: Option<HeldFile>,
    directory: Option<HeldDirectory>,
    path: PathBuf,
    stdout: Option<Capture>,
    stderr: Option<Capture>,
    child: Option<Child>,
    image: Option<File>,
    deadline: Instant,
    reaped: bool,
}
pub(crate) struct Interval {
    original: Option<Resources>,
    completed: bool,
}
impl Drop for Interval {
    fn drop(&mut self) {
        if !self.completed
            && let Some(original) = self.original.take()
        {
            std::mem::forget(original);
        }
    }
}
impl Interval {
    pub(super) fn begin(mut scratch: Scratch, uid: u32) -> Result<Self, ()> {
        if uid != 1000
            || nix::unistd::getuid().as_raw() != uid
            || nix::unistd::geteuid().as_raw() != uid
        {
            return Err(());
        }
        // Preinstall the whole holder BEFORE acquisition/spawn. No later error
        // or unwind silently drops a partial original prefix.
        let mut owner = Self {
            original: Some(Resources {
                capacity: Capacity::reserve()?,
                program: None,
                directory: None,
                path: PathBuf::new(),
                stdout: None,
                stderr: None,
                child: None,
                image: None,
                deadline: Instant::now() + BUDGET,
                reaped: false,
            }),
            completed: false,
        };
        let r = owner.original.as_mut().ok_or(())?;
        scratch.recheck()?;
        let (directory, path) = scratch.original.take().ok_or(())?;
        r.directory = Some(directory);
        r.path = path;
        for path in ["/usr", "/usr/lib", "/usr/lib/omavless-netguard"] {
            let m = fs::symlink_metadata(path).map_err(|_| ())?;
            if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
                return Err(());
            }
        }
        r.program =
            Some(HeldFile::capture(Path::new(PROGRAM), 0, 0o755, MAX_CORE).map_err(|_| ())?);
        r.stdout = Some(Capture::create(r.path.join("traffic.out"), uid)?);
        r.stderr = Some(Capture::create(r.path.join("traffic.err"), uid)?);
        r.capacity.check()?;
        // Actual image and zero privilege are checked while the original child
        // is blocked on stdin, BEFORE the one traffic release byte.
        r.child = Some(
            Command::new(PROGRAM)
                .env_clear()
                .env("LANG", "C")
                .args([
                    "--exact",
                    SELECTOR,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                    "--quiet",
                ])
                .stdin(Stdio::piped())
                .stdout(
                    r.stdout
                        .as_ref()
                        .ok_or(())?
                        .file
                        .try_clone()
                        .map_err(|_| ())?,
                )
                .stderr(
                    r.stderr
                        .as_ref()
                        .ok_or(())?
                        .file
                        .try_clone()
                        .map_err(|_| ())?,
                )
                .spawn()
                .map_err(|_| ())?,
        );
        let child = r.child.as_mut().ok_or(())?;
        let pid = child.id();
        let image = File::open(format!("/proc/{pid}/exe")).map_err(|_| ())?;
        r.image = Some(image);
        if !same(
            &r.program.as_ref().ok_or(())?.metadata,
            &r.image.as_ref().ok_or(())?.metadata().map_err(|_| ())?,
        ) {
            return Err(());
        }
        let mut status = String::new();
        File::open(format!("/proc/{pid}/status"))
            .map_err(|_| ())?
            .take(16385)
            .read_to_string(&mut status)
            .map_err(|_| ())?;
        if !capless_child(&status, std::process::id(), uid) {
            return Err(());
        }
        r.program
            .as_ref()
            .ok_or(())?
            .recheck(Path::new(PROGRAM))
            .map_err(|_| ())?;
        r.capacity.check()?;
        if Instant::now() >= r.deadline || original_status(child)? != WaitStatus::StillAlive {
            return Err(());
        }
        child
            .stdin
            .as_mut()
            .ok_or(())?
            .write_all(b"G")
            .map_err(|_| ())?;
        // Keep the original write end until known completion too.
        Ok(owner)
    }
    pub(super) fn complete(&mut self) -> Result<(), ()> {
        if self.completed {
            return Err(());
        }
        let r = self.original.as_mut().ok_or(())?;
        if r.reaped {
            return Err(());
        }
        loop {
            if Instant::now() >= r.deadline {
                return Err(());
            }
            r.stdout.as_ref().ok_or(())?.metadata()?;
            r.stderr.as_ref().ok_or(())?.metadata()?;
            r.capacity.check()?;
            let status = original_status(r.child.as_ref().ok_or(())?)?;
            if Instant::now() >= r.deadline {
                return Err(());
            }
            match status {
                WaitStatus::StillAlive => std::thread::sleep(Duration::from_millis(10)),
                WaitStatus::Exited(pid, 0)
                    if pid.as_raw() as u32 == r.child.as_ref().ok_or(())?.id() =>
                {
                    break;
                }
                _ => return Err(()),
            }
        }
        if r.stdout.as_ref().ok_or(())?.whole()? != b"\nrunning 1 test\n"
            || r.stderr.as_ref().ok_or(())?.whole()? != SUCCESS
        {
            return Err(());
        }
        r.program
            .as_ref()
            .ok_or(())?
            .recheck(Path::new(PROGRAM))
            .map_err(|_| ())?;
        let directory = r.directory.as_ref().ok_or(())?;
        let current = directory.file.metadata().map_err(|_| ())?;
        let named = fs::symlink_metadata(&r.path).map_err(|_| ())?;
        if current.dev() != directory.metadata.dev()
            || current.ino() != directory.metadata.ino()
            || current.gid() != directory.metadata.gid()
            || current.nlink() != directory.metadata.nlink()
            || current.uid() != 1000
            || current.mode() & 0o7777 != 0o700
            || !same(&current, &named)
            || Instant::now() >= r.deadline
        {
            return Err(());
        }
        r.capacity.check()?;
        if !r
            .child
            .as_mut()
            .ok_or(())?
            .wait()
            .map_err(|_| ())?
            .success()
        {
            return Err(());
        }
        r.reaped = true;
        if Instant::now() >= r.deadline {
            return Err(());
        }
        self.completed = true;
        Ok(())
    }
}
use std::os::unix::fs::DirBuilderExt;
fn original_status(child: &Child) -> Result<WaitStatus, ()> {
    let pid = i32::try_from(child.id()).map_err(|_| ())?;
    waitid(
        Id::Pid(Pid::from_raw(pid)),
        WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
    )
    .map_err(|_| ())
}
fn capless_child(status: &str, parent: u32, uid: u32) -> bool {
    if status.len() > 16384 {
        return false;
    }
    let field = |name: &str| {
        let mut values = status.lines().filter_map(|line| line.strip_prefix(name));
        let first = values.next();
        if values.next().is_some() {
            None
        } else {
            first.map(str::trim)
        }
    };
    field("PPid:") == Some(parent.to_string().as_str())
        && field("Uid:")
            .is_some_and(|s| s.split_whitespace().collect::<Vec<_>>() == vec![uid.to_string(); 4])
        && field("CapPrm:") == Some("0000000000000000")
        && field("CapEff:") == Some("0000000000000000")
        && field("CapAmb:") == Some("0000000000000000")
}

#[cfg(test)]
mod tests;
