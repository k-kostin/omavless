// SPDX-License-Identifier: MIT
//! Root execution selects immutable, independently authenticated bytes, never
//! /proc/<unprivileged PID>/exe or a user-writable executable path. The fixed
//! utility vocabulary below is internal: no CLI/IPC path or command is accepted.
//! Every original sudo child settles under the retained migration/owner context.
use super::{Blocker, Result, UpgradeContext, same};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

const PREFIX: &str = "/run/omavless-prepare.";
const STAT: &str = "%u:%g:%f:%h:%s:%d:%i\n";

fn unknown<T>() -> Result<T> {
    Err(Blocker::MaintenanceOutcomeUnknown)
}
fn parent() -> Result<()> {
    // /run is protected rather than a writable temporary parent. Both named
    // ancestors must be actual root-owned directories, not symlinks.
    for path in ["/", "/run"] {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return unknown();
        }
    }
    Ok(())
}
fn stage_path(output: &[u8]) -> Result<PathBuf> {
    let value = std::str::from_utf8(output).map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    let suffix = value
        .strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix('\n'))
        .ok_or(Blocker::MaintenanceOutcomeUnknown)?;
    if suffix.len() != 8 || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return unknown();
    }
    Ok(PathBuf::from(format!("{PREFIX}{suffix}")))
}
#[derive(Debug, PartialEq, Eq)]
struct Identity {
    uid: u32,
    gid: u32,
    mode: u32,
    links: u64,
    size: u64,
    device: u64,
    inode: u64,
}
impl Identity {
    fn read(output: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(output).map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        let text = text
            .strip_suffix('\n')
            .ok_or(Blocker::MaintenanceOutcomeUnknown)?;
        let values = text.split(':').collect::<Vec<_>>();
        if values.len() != 7 || values.iter().any(|value| value.is_empty()) {
            return unknown();
        }
        let decimal = |index: usize| -> Result<u64> {
            if !values[index].bytes().all(|byte| byte.is_ascii_digit()) {
                return unknown();
            }
            values[index]
                .parse()
                .map_err(|_| Blocker::MaintenanceOutcomeUnknown)
        };
        if !values[2].bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return unknown();
        }
        Ok(Self {
            uid: decimal(0)?
                .try_into()
                .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?,
            gid: decimal(1)?
                .try_into()
                .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?,
            mode: u32::from_str_radix(values[2], 16)
                .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?,
            links: decimal(3)?,
            size: decimal(4)?,
            device: decimal(5)?,
            inode: decimal(6)?,
        })
    }
    fn directory(&self) -> Result<()> {
        if self.uid != 0
            || self.gid != 0
            || self.mode != 0o40700
            || self.links != 2
            || self.inode == 0
        {
            return unknown();
        }
        Ok(())
    }
    fn executable(&self, size: u64) -> Result<()> {
        if self.uid != 0
            || self.gid != 0
            || self.mode != 0o100500
            || self.links != 1
            || self.size != size
            || size == 0
            || self.inode == 0
        {
            return unknown();
        }
        Ok(())
    }
    fn same_directory(&self, current: &Self) -> Result<()> {
        current.directory()?;
        // Adding the one file legitimately changes directory size, not identity.
        if self.uid != current.uid
            || self.gid != current.gid
            || self.mode != current.mode
            || self.links != current.links
            || self.device != current.device
            || self.inode != current.inode
        {
            return unknown();
        }
        Ok(())
    }
}
fn sudo(tool: &Path) -> Command {
    let mut command = Command::new("/usr/bin/sudo");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .args(["/usr/bin/env", "-i", "PATH=/usr/bin:/bin", "LC_ALL=C"])
        .arg(tool);
    command
}
fn settled(
    context: &mut UpgradeContext,
    cancel: &AtomicBool,
    tty: &File,
    mut command: Command,
    input: Option<File>,
    capture: bool,
) -> Result<Vec<u8>> {
    command.stdin(Stdio::from(match input {
        Some(file) => file,
        None => tty
            .try_clone()
            .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?,
    }));
    if capture {
        command.stdout(Stdio::piped());
    }
    let mut output = None;
    super::package_phase::consume(
        context,
        cancel,
        || {
            let mut child = command
                .spawn()
                .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
            output = child.stdout.take();
            Ok(child)
        },
        |context| context.recheck(),
    )
    .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
    let mut bytes = Vec::new();
    if let Some(output) = output {
        // Only fixed mktemp/stat/sha256sum produce captured output (< PIPE_BUF).
        // Never buffer an arbitrary privileged process or kill an effect on timeout.
        output
            .take(4097)
            .read_to_end(&mut bytes)
            .map_err(|_| Blocker::MaintenanceOutcomeUnknown)?;
        if bytes.len() > 4096 {
            return unknown();
        }
    }
    Ok(bytes)
}
fn stat(
    context: &mut UpgradeContext,
    cancel: &AtomicBool,
    tty: &File,
    path: &Path,
) -> Result<Identity> {
    let mut command = sudo(Path::new("/usr/bin/stat"));
    // stat without -L returns symlink metadata; the exact file type is checked.
    command.args(["--printf", STAT, "--"]).arg(path);
    Identity::read(&settled(context, cancel, tty, command, None, true)?)
}
fn digest(output: &[u8], path: &Path, expected: &str) -> Result<()> {
    let expected = format!("{expected}  {}\n", path.display());
    if output != expected.as_bytes() {
        return unknown();
    }
    Ok(())
}
fn source(expected: &str) -> Result<(File, fs::Metadata)> {
    let mut file = File::open("/proc/self/exe").map_err(|_| Blocker::HelperIdentityUnverified)?;
    let identity = file
        .metadata()
        .map_err(|_| Blocker::HelperIdentityUnverified)?;
    if !identity.is_file()
        || identity.len() == 0
        || identity.len() > 512 * 1024 * 1024
        || identity.mode() & 0o022 != 0
    {
        return Err(Blocker::HelperIdentityUnverified);
    }
    let mut hash = Sha256::new();
    let count = std::io::copy(&mut (&mut file).take(512 * 1024 * 1024 + 1), &mut hash)
        .map_err(|_| Blocker::HelperIdentityUnverified)?;
    if count != identity.len() {
        return Err(Blocker::HelperIdentityUnverified);
    }
    if format!("{:x}", hash.finalize()) != expected
        || !same(
            &identity,
            &file
                .metadata()
                .map_err(|_| Blocker::HelperIdentityUnverified)?,
        )
    {
        return Err(Blocker::HelperIdentityUnverified);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| Blocker::HelperIdentityUnverified)?;
    Ok((file, identity))
}

pub(super) fn prepare(
    context: &mut UpgradeContext,
    cancel: &AtomicBool,
    tty: &File,
    expected: &str,
) -> Result<()> {
    parent()?;
    let (file, original) = source(expected)?;
    let mut host = Production {
        context,
        cancel,
        tty,
        expected,
        source: Some(file),
        size: original.len(),
        directory: None,
        directory_identity: None,
        file_identity: None,
    };
    chain(|step| host.step(step)).map_err(|_| Blocker::MaintenanceOutcomeUnknown)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Create,
    Copy,
    Protect,
    Verify,
    Execute,
    VerifyOutcome,
    Unlink,
    Rmdir,
}
const STEPS: [Step; 8] = [
    Step::Create,
    Step::Copy,
    Step::Protect,
    Step::Verify,
    Step::Execute,
    Step::VerifyOutcome,
    Step::Unlink,
    Step::Rmdir,
];
fn chain(mut step: impl FnMut(Step) -> Result<()>) -> Result<()> {
    // Private fault seam used by the real implementation: every original child
    // and its verification must settle before the dependent step is submitted.
    for phase in STEPS {
        step(phase)?;
    }
    Ok(())
}
struct Production<'a> {
    context: &'a mut UpgradeContext,
    cancel: &'a AtomicBool,
    tty: &'a File,
    expected: &'a str,
    source: Option<File>,
    size: u64,
    directory: Option<PathBuf>,
    directory_identity: Option<Identity>,
    file_identity: Option<Identity>,
}
impl Production<'_> {
    fn stage(&self) -> Result<(PathBuf, PathBuf)> {
        let directory = self
            .directory
            .as_ref()
            .ok_or(Blocker::MaintenanceOutcomeUnknown)?
            .clone();
        Ok((directory.clone(), directory.join("helper")))
    }
    fn recheck_stage(&mut self) -> Result<()> {
        parent()?;
        let (directory, helper) = self.stage()?;
        self.directory_identity
            .as_ref()
            .ok_or(Blocker::MaintenanceOutcomeUnknown)?
            .same_directory(&stat(self.context, self.cancel, self.tty, &directory)?)?;
        if self.file_identity.as_ref() != Some(&stat(self.context, self.cancel, self.tty, &helper)?)
        {
            return unknown();
        }
        Ok(())
    }
    fn step(&mut self, phase: Step) -> Result<()> {
        match phase {
            Step::Create => {
                // A fresh root-private path prevents a same-UID writer from replacing the
                // helper while sudo awaits consent, even if this parent dies or execs.
                let mut create = sudo(Path::new("/usr/bin/mktemp"));
                create.args(["-d", "-p", "/run", "omavless-prepare.XXXXXXXX"]);
                let directory = stage_path(&settled(
                    self.context,
                    self.cancel,
                    self.tty,
                    create,
                    None,
                    true,
                )?)?;
                let dir_identity = stat(self.context, self.cancel, self.tty, &directory)?;
                dir_identity.directory()?;
                parent()?;
                self.directory = Some(directory);
                self.directory_identity = Some(dir_identity);
            }
            Step::Copy => {
                let (_, helper) = self.stage()?;
                let mut copy = sudo(Path::new("/usr/bin/tee"));
                copy.arg("--").arg(&helper).stdout(Stdio::null());
                // Binary stdin is the original held descriptor, not a reopened pathname;
                // sudo's authentication uses its controlling TTY, never this binary stream.
                let file = self
                    .source
                    .take()
                    .ok_or(Blocker::MaintenanceOutcomeUnknown)?;
                settled(self.context, self.cancel, self.tty, copy, Some(file), false)?;
            }
            Step::Protect => {
                let (_, helper) = self.stage()?;
                let mut permissions = sudo(Path::new("/usr/bin/chmod"));
                permissions.args(["0500", "--"]).arg(&helper);
                settled(
                    self.context,
                    self.cancel,
                    self.tty,
                    permissions,
                    None,
                    false,
                )?;
            }
            Step::Verify => {
                let (_, helper) = self.stage()?;
                let file_identity = stat(self.context, self.cancel, self.tty, &helper)?;
                file_identity.executable(self.size)?;
                let mut hash = sudo(Path::new("/usr/bin/sha256sum"));
                hash.arg("--").arg(&helper);
                digest(
                    &settled(self.context, self.cancel, self.tty, hash, None, true)?,
                    &helper,
                    self.expected,
                )?;
                self.file_identity = Some(file_identity);
                self.recheck_stage()?;
            }
            Step::Execute => {
                self.recheck_stage()?;
                let (_, helper) = self.stage()?;
                let mut execute = sudo(&helper);
                execute
                    .args(["setup", "prepare-pair-root"])
                    .arg(self.context.uid.to_string());
                settled(self.context, self.cancel, self.tty, execute, None, false)?;
            }
            Step::VerifyOutcome => {
                self.context.recheck()?;
                super::package_phase::require_broker_prepared()?;
            }
            Step::Unlink => {
                // There is deliberately no Drop cleanup: failure/unknown/fatal loss retains
                // staging and never submits another effect. After settled success, remove
                // only our verified helper and its newly created directory, never recursively.
                self.recheck_stage()?;
                let (_, helper) = self.stage()?;
                let mut remove = sudo(Path::new("/usr/bin/unlink"));
                remove.arg("--").arg(&helper);
                settled(self.context, self.cancel, self.tty, remove, None, false)?;
            }
            Step::Rmdir => {
                parent()?;
                let (directory, _) = self.stage()?;
                self.directory_identity
                    .as_ref()
                    .ok_or(Blocker::MaintenanceOutcomeUnknown)?
                    .same_directory(&stat(self.context, self.cancel, self.tty, &directory)?)?;
                let mut remove = sudo(Path::new("/usr/bin/rmdir"));
                remove.arg("--").arg(&directory);
                settled(self.context, self.cancel, self.tty, remove, None, false)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_original_failure_blocks_exec_and_all_dependent_cleanup_without_retry() {
        for (position, failed) in STEPS.into_iter().enumerate() {
            let mut trace = Vec::new();
            assert_eq!(
                chain(|step| {
                    trace.push(step);
                    if step == failed { unknown() } else { Ok(()) }
                }),
                Err(Blocker::MaintenanceOutcomeUnknown)
            );
            assert_eq!(trace, STEPS[..=position]);
        }
        let mut trace = Vec::new();
        chain(|step| {
            trace.push(step);
            Ok(())
        })
        .unwrap();
        assert_eq!(trace, STEPS);
    }
    #[test]
    fn binary_child_reads_original_descriptor_even_after_parent_handle_and_path_are_gone() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture");
        fs::write(&path, b"owned inert binary stdin fixture").unwrap();
        let original = File::open(&path).unwrap();
        let mut child = Command::new("/usr/bin/cat")
            .stdin(Stdio::from(original))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        fs::remove_file(path).unwrap();
        assert!(child.wait().unwrap().success());
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"owned inert binary stdin fixture");
    }
    #[test]
    fn closed_stage_path_rejects_extra_lines_traversal_and_arbitrary_paths() {
        assert_eq!(
            stage_path(b"/run/omavless-prepare.Ab12Cd34\n").unwrap(),
            PathBuf::from("/run/omavless-prepare.Ab12Cd34")
        );
        for bad in [
            b"/tmp/omavless-prepare.Ab12Cd34\n".as_slice(),
            b"/run/omavless-prepare.Ab12Cd34\nextra\n",
            b"/run/omavless-prepare.../evil/\n",
            b"/run/omavless-prepare.Ab12Cd34",
            b"/run/omavless-prepare.Ab12Cd34\r\n",
            b"/run/omavless-prepare.Ab12Cd345\n",
        ] {
            assert_eq!(stage_path(bad), Err(Blocker::MaintenanceOutcomeUnknown));
        }
    }
    #[test]
    fn root_identity_rejects_link_mutable_user_file_and_replacement() {
        let good = Identity::read(b"0:0:8140:1:123:8:99\n").unwrap(); // regular0500
        assert!(good.executable(123).is_ok());
        for bad in [
            "1000:0:8140:1:123:8:99\n",
            "0:1000:8140:1:123:8:99\n",
            "0:0:a140:1:123:8:99\n",
            "0:0:81ff:1:123:8:99\n",
            "0:0:8140:2:123:8:99\n",
            "0:0:8140:1:124:8:99\n",
            "0:0:8140:1:123:8:0\n",
        ] {
            assert!(
                Identity::read(bad.as_bytes())
                    .unwrap()
                    .executable(123)
                    .is_err()
            );
        }
        let dir = Identity::read(b"0:0:41c0:2:40:8:99\n").unwrap();
        assert!(
            dir.same_directory(&Identity::read(b"0:0:41c0:2:60:8:99\n").unwrap())
                .is_ok()
        );
        assert!(
            dir.same_directory(&Identity::read(b"0:0:41c0:2:40:8:100\n").unwrap())
                .is_err()
        );
        for bad in [
            "0:0:41ff:2:40:8:99\n",
            "0:0:a1c0:2:40:8:99\n",
            "0:0:41c0:2:40:8:99\nsecond\n",
            "0:0:41c0:2:40:8\n",
        ] {
            assert!(
                Identity::read(bad.as_bytes())
                    .and_then(|value| value.directory())
                    .is_err()
            );
        }
    }
    #[test]
    fn staged_hash_is_bound_to_exact_file_and_authenticated_digest() {
        let path = Path::new("/run/omavless-prepare.Ab12Cd34/helper");
        let hash = "a".repeat(64);
        assert!(
            digest(
                format!("{hash}  {}\n", path.display()).as_bytes(),
                path,
                &hash
            )
            .is_ok()
        );
        assert!(
            digest(
                format!("{}  {}\n", "b".repeat(64), path.display()).as_bytes(),
                path,
                &hash
            )
            .is_err()
        );
        assert!(digest(format!("{hash}  /tmp/helper\n").as_bytes(), path, &hash).is_err());
    }
}
