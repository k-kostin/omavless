// SPDX-License-Identifier: MIT
//! Unregistered, offline login-validation primitive. Never proves TUN permission.
use crate::desired::DesiredState;
use omavless_domain::private_store::PrivateStore;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const RESOURCE_LIMIT: usize = 8 * 1024 * 1024;
const TOTAL_LIMIT: usize = 64 * 1024 * 1024;
const CORE_LIMIT: usize = 128 * 1024 * 1024;
const BUNDLES: [&str; 3] = [
    include_str!("../../../templates/default.yaml"),
    include_str!("../../../templates/china.yaml"),
    include_str!("../../../templates/iran.yaml"),
];

/// Fixed classifications only: never retain a path, parser fragment or stderr.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    UnsupportedTemplate,
    UnsafeInput,
    ResourceUnavailable,
    UnsupportedCore,
    SandboxRejected,
    Timeout,
    Cleanup,
}

/// Private, non-formatable immutable inputs. No store/template reread on execute.
pub struct ValidationSnapshot {
    config: String,
    resources: Vec<(String, Vec<u8>)>,
}

fn manifest(template: &str) -> Result<Vec<String>, ValidationError> {
    // This is exact trusted-source recognition, NOT a YAML security parser.
    let bundle = BUNDLES
        .iter()
        .find(|bundle| {
            ["rule", "global", "direct"].iter().any(|mode| {
                template == bundle.replace("\nmode: rule\n", &format!("\nmode: {mode}\n"))
            })
        })
        .ok_or(ValidationError::UnsupportedTemplate)?;
    let mut paths = Vec::new();
    for line in bundle.lines() {
        if let Some(name) = line.strip_prefix("    path: ./ruleset/") {
            if name.is_empty()
                || name.len() > 80
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
                || !name.ends_with(".mrs")
                || name.contains("..")
                || paths.iter().any(|v| v == name)
            {
                return Err(ValidationError::UnsupportedTemplate);
            }
            paths.push(name.to_owned());
        }
    }
    if paths.is_empty() || paths.len() > 32 {
        return Err(ValidationError::UnsupportedTemplate);
    }
    Ok(paths)
}

fn read_bounded(
    path: &Path,
    uid: u32,
    limit: usize,
    root_allowed: bool,
) -> Result<Vec<u8>, ValidationError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| ValidationError::UnsafeInput)?;
    let before = file.metadata().map_err(|_| ValidationError::UnsafeInput)?;
    if !before.is_file()
        || (before.uid() != uid && !(root_allowed && before.uid() == 0))
        || before.mode() & 0o022 != 0
        || before.len() == 0
        || before.len() > limit as u64
    {
        return Err(ValidationError::UnsafeInput);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ValidationError::UnsafeInput)?;
    let after = file.metadata().map_err(|_| ValidationError::UnsafeInput)?;
    if bytes.len() > limit
        || before.len() != bytes.len() as u64
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(ValidationError::UnsafeInput);
    }
    Ok(bytes)
}

impl ValidationSnapshot {
    /// Accept exactly a checked-in bundle (including its canonical mode change).
    /// Every named provider cache is required; unknown/custom policy fails closed.
    pub fn capture(
        data: &Path,
        uid: u32,
        desired: &DesiredState,
        store: &PrivateStore,
        template: &str,
    ) -> Result<Self, ValidationError> {
        let names = manifest(template)?;
        desired
            .validate()
            .map_err(|_| ValidationError::UnsafeInput)?;
        if !desired.connected
            || !data.is_absolute()
            || !crate::native_host::private_directory(data, uid)
        {
            return Err(ValidationError::UnsafeInput);
        }
        let directory = data.join("ruleset");
        let meta =
            fs::symlink_metadata(&directory).map_err(|_| ValidationError::ResourceUnavailable)?;
        if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o022 != 0 {
            return Err(ValidationError::ResourceUnavailable);
        }
        // Hold the directory inode and resolve children through that FD. A
        // concurrent directory replacement cannot redirect resource acquisition.
        let held = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW)
            .open(&directory)
            .map_err(|_| ValidationError::ResourceUnavailable)?;
        use std::os::fd::AsRawFd;
        let held_meta = held
            .metadata()
            .map_err(|_| ValidationError::ResourceUnavailable)?;
        if held_meta.dev() != meta.dev() || held_meta.ino() != meta.ino() {
            return Err(ValidationError::ResourceUnavailable);
        }
        let mut resources = Vec::new();
        let mut total = 0;
        for name in names {
            let bytes = read_bounded(
                &PathBuf::from(format!("/proc/self/fd/{}", held.as_raw_fd())).join(&name),
                uid,
                RESOURCE_LIMIT,
                false,
            )
            .map_err(|_| ValidationError::ResourceUnavailable)?;
            total += bytes.len();
            if total > TOTAL_LIMIT {
                return Err(ValidationError::ResourceUnavailable);
            }
            resources.push((name, bytes));
        }
        let config = store
            .prepare_config_mode(
                &desired.profile_id,
                template,
                "/work/mihomo.sock",
                desired.mode.as_str(),
            )
            .map_err(|_| ValidationError::UnsafeInput)?;
        Ok(Self { config, resources })
    }

    /// Fixed offline command only; no socket registration or lifecycle effects.
    /// The caller must independently prove permissions, empty host and login fences.
    pub fn validate(
        &self,
        core: &Path,
        scratch_parent: &Path,
        uid: u32,
    ) -> Result<(), ValidationError> {
        if !core.is_absolute() {
            return Err(ValidationError::UnsupportedCore);
        }
        let bytes = read_bounded(core, uid, CORE_LIMIT, true)?;
        if !static_elf(&bytes) {
            return Err(ValidationError::UnsupportedCore);
        }
        let mut scratch = Scratch::new(scratch_parent, uid)?;
        let result = (|| {
            scratch.add("core", &bytes, 0o500)?;
            scratch.add("config.yaml", self.config.as_bytes(), 0o400)?;
            for (name, bytes) in &self.resources {
                scratch.add(&format!("ruleset/{name}"), bytes, 0o400)?;
            }
            scratch.verify()?;
            let mut command = sandbox_command(&scratch.path);
            run(&mut command, Duration::from_secs(3))
        })();
        scratch.cleanup()?;
        result
    }
}

fn static_elf(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || &bytes[..6] != b"\x7fELF\x02\x01" {
        return false;
    }
    let offset = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let size = u16::from_le_bytes(bytes[54..56].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as usize;
    if size != 56 || count == 0 || count > 128 {
        return false;
    }
    let Ok(offset) = usize::try_from(offset) else {
        return false;
    };
    let Some(end) = offset.checked_add(size * count) else {
        return false;
    };
    if end > bytes.len() {
        return false;
    }
    // Dynamic interpreter loading is outside this first host contract.
    !bytes[offset..end]
        .chunks_exact(size)
        .any(|entry| entry[..4] == 3u32.to_le_bytes())
}

fn sandbox_command(scratch: &Path) -> Command {
    let mut command = Command::new("/usr/bin/bwrap");
    command
        .env_clear()
        .args([
            "--unshare-all",
            "--unshare-user",
            "--disable-userns",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--clearenv",
            "--ro-bind",
        ])
        .arg(scratch)
        .arg("/input")
        .args(["--size", "16777216", "--tmpfs", "/work", "--ro-bind"])
        .arg(scratch.join("ruleset"))
        .arg("/work/ruleset")
        .args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--remount-ro",
            "/",
            "--chdir",
            "/work",
            "--setenv",
            "HOME",
            "/work",
            "/input/core",
            "-t",
            "-d",
            "/work",
            "-f",
            "/input/config.yaml",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn run(command: &mut Command, timeout: Duration) -> Result<(), ValidationError> {
    let mut child = command
        .spawn()
        .map_err(|_| ValidationError::SandboxRejected)?;
    let deadline = Instant::now() + timeout;
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(ValidationError::SandboxRejected)
                };
            }
            Err(_) => break Err(ValidationError::SandboxRejected),
            Ok(None) if Instant::now() >= deadline => break Err(ValidationError::Timeout),
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    // bwrap's mandatory PID namespace/die-with-parent kills sandbox descendants.
    let _ = child.kill();
    let _ = child.wait();
    result
}

struct Scratch {
    path: PathBuf,
    directory: File,
    rules: File,
    files: Vec<(PathBuf, File)>,
}
impl Scratch {
    fn new(parent: &Path, uid: u32) -> Result<Self, ValidationError> {
        Self::initialize(parent, uid, |path| {
            DirBuilder::new().mode(0o700).create(path.join("ruleset"))?;
            OpenOptions::new()
                .read(true)
                .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW)
                .open(path.join("ruleset"))
        })
    }
    fn initialize(
        parent: &Path,
        uid: u32,
        initialize_rules: impl FnOnce(&Path) -> std::io::Result<File>,
    ) -> Result<Self, ValidationError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        if !parent.is_absolute() || !crate::native_host::private_directory(parent, uid) {
            return Err(ValidationError::UnsafeInput);
        }
        let path = parent.join(format!(
            ".login-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| ValidationError::UnsafeInput)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| ValidationError::Cleanup)?;
        let rules = match initialize_rules(&path) {
            Ok(rules) => rules,
            Err(_) => {
                // Empty, exactly owned initialization can be undone. Anything
                // unexpected/partially initialized is retained, not recursively
                // removed on an error path without held inode proof.
                return Err(
                    if Self::same(&path, &directory) && fs::remove_dir(&path).is_ok() {
                        ValidationError::UnsafeInput
                    } else {
                        ValidationError::Cleanup
                    },
                );
            }
        };
        Ok(Self {
            path,
            directory,
            rules,
            files: Vec::new(),
        })
    }
    fn add(&mut self, name: &str, bytes: &[u8], mode: u32) -> Result<(), ValidationError> {
        let path = self.path.join(name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| ValidationError::UnsafeInput)?;
        self.files.push((path, file));
        let (path, held) = self.files.last_mut().unwrap();
        held.write_all(bytes)
            .map_err(|_| ValidationError::UnsafeInput)?;
        let read_only = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&*path)
            .map_err(|_| ValidationError::UnsafeInput)?;
        let original = held.metadata().map_err(|_| ValidationError::UnsafeInput)?;
        let replacement = read_only
            .metadata()
            .map_err(|_| ValidationError::UnsafeInput)?;
        if original.dev() != replacement.dev() || original.ino() != replacement.ino() {
            return Err(ValidationError::UnsafeInput);
        }
        // Keep an inode anchor, not a writable executable descriptor (ETXTBSY).
        *held = read_only;
        Ok(())
    }
    fn same(path: &Path, held: &File) -> bool {
        let (Ok(a), Ok(b)) = (fs::symlink_metadata(path), held.metadata()) else {
            return false;
        };
        a.dev() == b.dev()
            && a.ino() == b.ino()
            && a.mode() == b.mode()
            && a.uid() == b.uid()
            && !a.file_type().is_symlink()
    }
    fn verify(&self) -> Result<(), ValidationError> {
        if !Self::same(&self.path, &self.directory)
            || !Self::same(&self.path.join("ruleset"), &self.rules)
            || self.files.iter().any(|(p, f)| !Self::same(p, f))
        {
            return Err(ValidationError::Cleanup);
        }
        Ok(())
    }
    fn cleanup(&mut self) -> Result<(), ValidationError> {
        self.verify()?;
        for (path, _) in &self.files {
            fs::remove_file(path).map_err(|_| ValidationError::Cleanup)?;
        }
        fs::remove_dir(self.path.join("ruleset")).map_err(|_| ValidationError::Cleanup)?;
        fs::remove_dir(&self.path).map_err(|_| ValidationError::Cleanup)
    }
}

#[cfg(test)]
#[path = "isolated_validation_tests.rs"]
mod tests;
