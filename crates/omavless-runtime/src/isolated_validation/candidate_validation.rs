// SPDX-License-Identifier: MIT
//! Private captured-byte staging and original-child completion model.
//! No production caller, public execution API or installed-launcher admission.
// These private building blocks intentionally remain unreachable in production
// until a separate launcher/kernel qualification and effect recipe are reviewed.
#![allow(dead_code)]

use super::Scratch;
use super::candidate_capture::CandidateValidationSnapshot;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, DirBuilder, File, Metadata, OpenOptions};
use std::io;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const EXECUTION_BUDGET: Duration = Duration::from_secs(3);
const LAUNCHER_GRACE: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(5);

// Private synthetic-test seam, not an alternate constructor/admission API.
trait CapturedInputs {
    fn core(&self) -> &[u8];
    fn config(&self) -> &[u8];
    fn resources(&self) -> &[(String, Vec<u8>)];
    #[cfg(test)]
    fn after_staged_anchor(&self, _scratch: &Scratch) {}
}
impl CapturedInputs for CandidateValidationSnapshot {
    fn core(&self) -> &[u8] {
        self.expose_captured_core_bytes()
    }
    fn config(&self) -> &[u8] {
        self.expose_private_config()
    }
    fn resources(&self) -> &[(String, Vec<u8>)] {
        self.expose_private_resources()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Staging,
    TreeProof,
    Spawn,
    Wait,
    Kill,
    Timeout,
    LateCompletion,
    Cleanup,
}

// Initialization owns each acquired anchor before the next fallible operation.
// Unlike old Scratch::initialize, an uncertain partial tree loses no held FD.
struct PartialScratch {
    path: PathBuf,
    directory: Option<File>,
    rules: Option<File>,
}
struct Scope<I> {
    input: I,
    uid: u32,
    partial: Option<PartialScratch>,
    scratch: Option<Scratch>,
}
struct Retained<I, C> {
    scope: Scope<I>,
    child: Option<C>,
}
// Armed BEFORE any new resource acquisition. Moving this owner between stage,
// supervision, completed model and failure never disarms it. Unwind and consumer
// Drop retain the payload; only proved exact cleanup permits explicit disarm.
struct Armed<I, C>(ManuallyDrop<Box<Retained<I, C>>>);
impl<I, C> Armed<I, C> {
    fn new(input: I, uid: u32) -> Self {
        Self(ManuallyDrop::new(Box::new(Retained {
            scope: Scope {
                input,
                uid,
                partial: None,
                scratch: None,
            },
            child: None,
        })))
    }
    fn disarm_after_cleanup(self) {
        drop(ManuallyDrop::into_inner(self.0));
    }
}
impl<I, C> Deref for Armed<I, C> {
    type Target = Retained<I, C>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<I, C> DerefMut for Armed<I, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<I, C> Deref for Retained<I, C> {
    type Target = Scope<I>;
    fn deref(&self) -> &Self::Target {
        &self.scope
    }
}
impl<I, C> DerefMut for Retained<I, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scope
    }
}
#[must_use = "retained scope is not cleanup or retry authority"]
struct Failure<I, C> {
    reason: Refusal,
    // Deliberate bounded-per-attempt retention, including after consumer Drop.
    // No registry, Drop kill/cleanup, automatic retry or live-handle loss.
    retained: Armed<I, C>,
}
impl<I, C> fmt::Debug for Failure<I, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CandidateValidationRetained")
            .field(&self.reason)
            .finish()
    }
}
fn retain<I, C>(reason: Refusal, retained: Armed<I, C>) -> Failure<I, C> {
    Failure { reason, retained }
}

// The only real entry consumes the same opaque carrier. No core/config/resource
// path, prerendered payload/manifest, supplied digest or eligibility bool exists.
fn stage(
    input: CandidateValidationSnapshot,
    parent: &Path,
    uid: u32,
) -> Result<Armed<CandidateValidationSnapshot, Child>, Failure<CandidateValidationSnapshot, Child>>
{
    stage_owned(input, parent, uid)
}
fn stage_owned<I: CapturedInputs, C>(
    input: I,
    parent: &Path,
    uid: u32,
) -> Result<Armed<I, C>, Failure<I, C>> {
    let mut owner = Armed::new(input, uid);
    let result = (|| {
        let scope = &mut owner.scope;
        if !parent.is_absolute() || !crate::native_host::private_directory(parent, uid) {
            return Err(Refusal::Staging);
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = parent.join(format!(
            ".candidate-validation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Record the intended scope before create. A create error/panic grants
        // no ownership/cleanup authority, but cannot lose its bounded pathname.
        scope.partial = Some(PartialScratch {
            path,
            directory: None,
            rules: None,
        });
        let partial = scope.partial.as_mut().unwrap();
        DirBuilder::new()
            .mode(0o700)
            .create(&partial.path)
            .map_err(|_| Refusal::Staging)?;
        partial.directory = Some(open_directory(&partial.path)?);
        DirBuilder::new()
            .mode(0o700)
            .create(partial.path.join("ruleset"))
            .map_err(|_| Refusal::Staging)?;
        partial.rules = Some(open_directory(&partial.path.join("ruleset"))?);
        // Clone/validate while all anchors remain in the armed owner. The
        // following checked Some-to-Scratch moves contain no callbacks or
        // fallible operations, and ownership never leaves the armed payload.
        let partial = scope.partial.as_mut().unwrap();
        let path = partial.path.clone();
        if partial.directory.is_none() || partial.rules.is_none() {
            return Err(Refusal::Staging);
        }
        scope.scratch = Some(Scratch {
            path,
            directory: partial.directory.take().unwrap(),
            rules: partial.rules.take().unwrap(),
            files: Vec::new(),
        });
        scope.partial = None;
        #[cfg(test)]
        scope
            .input
            .after_staged_anchor(scope.scratch.as_ref().unwrap());
        let scratch = scope.scratch.as_mut().unwrap();
        scratch
            .add("core", scope.input.core(), 0o500)
            .map_err(|_| Refusal::Staging)?;
        #[cfg(test)]
        scope.input.after_staged_anchor(scratch);
        scratch
            .add("config.yaml", scope.input.config(), 0o400)
            .map_err(|_| Refusal::Staging)?;
        for (name, bytes) in scope.input.resources() {
            scratch
                .add(&format!("ruleset/{name}"), bytes, 0o400)
                .map_err(|_| Refusal::Staging)?;
        }
        scope.prove()
    })();
    match result {
        Ok(()) => Ok(owner),
        Err(reason) => Err(retain(reason, owner)),
    }
}
fn open_directory(path: &Path) -> Result<File, Refusal> {
    OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| Refusal::Staging)
}
fn stable(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn file_proof(path: &Path, held: &File, bytes: &[u8], uid: u32, mode: u32) -> Result<(), Refusal> {
    let before = held.metadata().map_err(|_| Refusal::TreeProof)?;
    if !before.is_file()
        || before.uid() != uid
        || before.mode() & 0o7777 != mode
        || before.nlink() != 1
        || before.len() != bytes.len() as u64
        || bytes.is_empty()
        || !Scratch::same(path, held)
    {
        return Err(Refusal::TreeProof);
    }
    let mut buffer = [0u8; 65536];
    let mut offset = 0;
    while offset < bytes.len() {
        let size = buffer.len().min(bytes.len() - offset);
        let got = held
            .read_at(&mut buffer[..size], offset as u64)
            .map_err(|_| Refusal::TreeProof)?;
        if got == 0 || buffer[..got] != bytes[offset..offset + got] {
            return Err(Refusal::TreeProof);
        }
        offset += got;
    }
    let after = held.metadata().map_err(|_| Refusal::TreeProof)?;
    if !stable(&before, &after) || !Scratch::same(path, held) {
        return Err(Refusal::TreeProof);
    }
    Ok(())
}
fn directory_proof(
    path: &Path,
    held: &File,
    uid: u32,
    names: BTreeSet<OsString>,
) -> Result<(), Refusal> {
    let before = held.metadata().map_err(|_| Refusal::TreeProof)?;
    if !before.is_dir()
        || before.uid() != uid
        || before.mode() & 0o7777 != 0o700
        || !Scratch::same(path, held)
    {
        return Err(Refusal::TreeProof);
    }
    let mut remaining = names;
    let anchored = PathBuf::from(format!("/proc/self/fd/{}", held.as_raw_fd()));
    for entry in fs::read_dir(anchored).map_err(|_| Refusal::TreeProof)? {
        let entry = entry.map_err(|_| Refusal::TreeProof)?;
        if !remaining.remove(&entry.file_name()) {
            return Err(Refusal::TreeProof);
        }
    }
    let after = held.metadata().map_err(|_| Refusal::TreeProof)?;
    if !remaining.is_empty() || !stable(&before, &after) || !Scratch::same(path, held) {
        return Err(Refusal::TreeProof);
    }
    Ok(())
}
impl<I: CapturedInputs> Scope<I> {
    fn prove(&self) -> Result<(), Refusal> {
        let scratch = self.scratch.as_ref().ok_or(Refusal::TreeProof)?;
        if self.partial.is_some() || scratch.files.len() != 2 + self.input.resources().len() {
            return Err(Refusal::TreeProof);
        }
        let roots: BTreeSet<_> = ["core", "config.yaml", "ruleset"]
            .map(OsString::from)
            .into_iter()
            .collect();
        directory_proof(&scratch.path, &scratch.directory, self.uid, roots.clone())?;
        let resources: BTreeSet<_> = self
            .input
            .resources()
            .iter()
            .map(|(name, _)| OsString::from(name))
            .collect();
        directory_proof(
            &scratch.path.join("ruleset"),
            &scratch.rules,
            self.uid,
            resources.clone(),
        )?;
        for ((path, held), (name, bytes, mode)) in scratch.files.iter().zip(
            [
                ("core".to_owned(), self.input.core(), 0o500),
                ("config.yaml".to_owned(), self.input.config(), 0o400),
            ]
            .into_iter()
            .chain(
                self.input
                    .resources()
                    .iter()
                    .map(|(name, bytes)| (format!("ruleset/{name}"), bytes.as_slice(), 0o400)),
            ),
        ) {
            if *path != scratch.path.join(name) {
                return Err(Refusal::TreeProof);
            }
            file_proof(path, held, bytes, self.uid, mode)?;
        }
        // File scans can take longer than enumeration. Recheck held directory
        // paths/membership at the end as well; this still is not atomic against
        // an actively malicious same-UID writer.
        directory_proof(&scratch.path, &scratch.directory, self.uid, roots)?;
        directory_proof(
            &scratch.path.join("ruleset"),
            &scratch.rules,
            self.uid,
            resources,
        )?;
        Ok(())
    }
}

fn candidate_command(scratch: &Path) -> Command {
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
            "--as-pid-1",
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

trait OriginalChild {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
}
impl OriginalChild for Child {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Child::try_wait(self)
    }
    fn kill(&mut self) -> io::Result<()> {
        Child::kill(self)
    }
}
trait Clock {
    fn elapsed(&self) -> Duration;
    fn pause(&mut self);
}
struct Monotonic(Instant);
impl Clock for Monotonic {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
    fn pause(&mut self) {
        std::thread::sleep(POLL);
    }
}
// This is NOT a namespace/validation receipt. Even a private normal-zero model
// needs installed launcher/kernel qualification before a live success route.
struct CompletedModel<I, C> {
    retained: Armed<I, C>,
}
// Private source-model finalization only, never a live validation grant. There
// is no production launch caller: installed completion assumptions stay closed.
fn finish_model<I: CapturedInputs, C>(
    mut completed: CompletedModel<I, C>,
) -> Result<(), Failure<I, C>> {
    if completed.retained.scope.prove().is_err() {
        return Err(retain(Refusal::TreeProof, completed.retained));
    }
    if completed
        .retained
        .scope
        .scratch
        .as_mut()
        .unwrap()
        .cleanup()
        .is_err()
    {
        return Err(retain(Refusal::Cleanup, completed.retained));
    }
    completed.retained.disarm_after_cleanup();
    Ok(())
}
fn supervise<I: CapturedInputs, C: OriginalChild, T: Clock>(
    mut owner: Armed<I, C>,
    mut clock: T,
    launch: impl FnOnce(&Path) -> io::Result<C>,
) -> Result<CompletedModel<I, C>, Failure<I, C>> {
    // Clock origin precedes launch. Staging/spawn/fs calls are not hard-bounded
    // by these polling budgets; no whole-call wall-clock bound is promised.
    if owner.scope.prove().is_err() {
        return Err(retain(Refusal::TreeProof, owner));
    }
    if clock.elapsed() >= EXECUTION_BUDGET {
        return Err(retain(Refusal::Timeout, owner));
    }
    // Store the returned handle immediately, before clock/proof/wait callbacks.
    owner.child = match launch(&owner.scope.scratch.as_ref().unwrap().path) {
        Ok(child) => Some(child),
        Err(_) => return Err(retain(Refusal::Spawn, owner)),
    };
    loop {
        if clock.elapsed() >= EXECUTION_BUDGET {
            return timeout(owner, clock);
        }
        let observed = owner.child.as_mut().unwrap().try_wait();
        let late = clock.elapsed() >= EXECUTION_BUDGET;
        match observed {
            Ok(Some(status)) => {
                if late {
                    return Err(retain(Refusal::LateCompletion, owner));
                }
                if !status.success() {
                    return Err(retain(Refusal::Wait, owner));
                }
                if owner.scope.prove().is_err() {
                    return Err(retain(Refusal::TreeProof, owner));
                }
                // Post-tree proof is also charged to the execution budget.
                if clock.elapsed() >= EXECUTION_BUDGET {
                    return Err(retain(Refusal::LateCompletion, owner));
                }
                return Ok(CompletedModel { retained: owner });
            }
            Err(_) => return Err(retain(Refusal::Wait, owner)),
            Ok(None) if late => return timeout(owner, clock),
            Ok(None) => clock.pause(),
        }
    }
}
fn timeout<I, C: OriginalChild, T: Clock>(
    mut owner: Armed<I, C>,
    mut clock: T,
) -> Result<CompletedModel<I, C>, Failure<I, C>> {
    let grace_start = clock.elapsed();
    if owner.child.as_mut().unwrap().kill().is_err() {
        return Err(retain(Refusal::Kill, owner));
    }
    while clock.elapsed().saturating_sub(grace_start) < LAUNCHER_GRACE {
        let observed = owner.child.as_mut().unwrap().try_wait();
        let elapsed = clock.elapsed().saturating_sub(grace_start);
        match observed {
            Ok(Some(_)) => break,
            Err(_) => return Err(retain(Refusal::Wait, owner)),
            Ok(None) if elapsed >= LAUNCHER_GRACE => break,
            Ok(None) => clock.pause(),
        }
    }
    // Reaping the killed launcher proves no namespace completion. Retain even
    // when this grace observed zero; never manufacture validation or cleanup.
    Err(retain(Refusal::Timeout, owner))
}

#[cfg(test)]
mod tests;
