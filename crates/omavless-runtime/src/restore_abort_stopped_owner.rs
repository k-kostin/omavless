// SPDX-License-Identifier: MIT
//! CLI-private, conservative stopped-owner observation. Not owner authority.

use nix::fcntl::{FcntlArg, OFlag, fcntl, open, openat};
use nix::sys::stat::Mode;
use nix::sys::statfs::{PROC_SUPER_MAGIC, fstatfs};
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid, waitpid};
use nix::unistd::Pid;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, ()>;

// Fixed, development-only progress. Historical Abort never enables this path.
// Diagnostic output does not authorize effects or alter operation results.
macro_rules! recovery_checkpoint {
    ($progress:expr, $phase:ident) => {{
        #[cfg(feature = "t4-manager-actor-service")]
        recovery_diagnostic(
            $progress,
            concat!("restore_owner_", stringify!($phase), "\n").as_bytes(),
        );
    }};
}

#[cfg(feature = "t4-manager-actor-service")]
fn recovery_diagnostic(progress: RecoveryProgress, label: &'static [u8]) {
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    bounded_recovery_diagnostic(progress.enabled(), &COUNT, label, |bytes| {
        // One raw write, no formatting/locks/retry/panic on a broken stderr.
        // The CLI's existing stderr destination is used, never a selected path.
        nix::unistd::write(std::io::stderr(), bytes).map_or(-1, |written| written as isize)
    });
}

#[cfg(feature = "t4-manager-actor-service")]
fn bounded_recovery_diagnostic(
    enabled: bool,
    count: &std::sync::atomic::AtomicUsize,
    label: &[u8],
    write: impl FnOnce(&[u8]) -> isize,
) {
    use std::sync::atomic::Ordering;
    if !enabled || label.len() > 96 {
        return;
    }
    if count
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            (value < 256).then_some(value + 1)
        })
        .is_ok()
    {
        let _ = write(label);
    }
}

#[path = "restore_recovery_manager_identity.rs"]
mod recovery_manager_identity;
use recovery_manager_identity::ManagerIdentity;
#[cfg(any(test, feature = "t4-manager-actor-service"))]
use recovery_manager_identity::RecoveryManagerRecord;
#[path = "restore_recovery_cgroup.rs"]
mod recovery_cgroup;

// The ordinary binary has no trace code, flag, sink or runtime override.
macro_rules! checkpoint {
    ($phase:ident) => {
        #[cfg(test)]
        super::diagnostic::before(super::diagnostic::Phase::$phase)?;
    };
}

// Each body keeps its original evaluation point and short-circuit order.
// Ordinary builds contain neither the latch nor evaluation of its enum.
macro_rules! capture_step {
    ($step:ident, $body:expr) => {{
        #[cfg(test)]
        crate::restore_abort_cli::diagnostic::manager_capture_before(
            crate::restore_abort_cli::diagnostic::ManagerCaptureStep::$step,
        )?;
        $body
    }};
}

#[derive(Clone, Copy)]
enum SelfInvocation {
    Recovery,
    #[cfg(feature = "t4-manager-actor-service")]
    OldRecovery,
    #[cfg(feature = "t4-manager-actor-service")]
    ConsumerRecovery,
    #[cfg(feature = "t4-manager-actor-service")]
    ClosureCut,
    #[cfg(feature = "t4-manager-actor-service")]
    CompleteOnlyCut,
    #[cfg(feature = "t4-manager-actor-service")]
    HistorySyncedLoss,
    #[cfg(test)]
    Diagnostic,
}

/// Presentation only, never a query, custody or recovery admission selector.
#[cfg(feature = "t4-manager-actor-service")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecoveryProgress {
    DiagnosticCli,
    ConsumerSilent,
}

#[cfg(feature = "t4-manager-actor-service")]
impl RecoveryProgress {
    pub(crate) fn enabled(self) -> bool {
        self == Self::DiagnosticCli
    }
}

impl SelfInvocation {
    #[cfg(feature = "t4-manager-actor-service")]
    fn progress(self) -> RecoveryProgress {
        if matches!(
            self,
            Self::OldRecovery | Self::ClosureCut | Self::CompleteOnlyCut | Self::HistorySyncedLoss
        ) {
            RecoveryProgress::DiagnosticCli
        } else {
            RecoveryProgress::ConsumerSilent
        }
    }
    #[cfg(feature = "t4-manager-actor-service")]
    fn lost_owner_recovery(self) -> bool {
        matches!(
            self,
            Self::OldRecovery
                | Self::ConsumerRecovery
                | Self::ClosureCut
                | Self::CompleteOnlyCut
                | Self::HistorySyncedLoss
        )
    }

    fn validate(self, command: &[u8]) -> Result<()> {
        match self {
            Self::Recovery => recovery_self(command),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::OldRecovery => old_recovery_self(command),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::ConsumerRecovery => consumer_recovery_self(command),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::ClosureCut => closure_cut_self(command),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::CompleteOnlyCut => complete_only_cut_self(command),
            #[cfg(feature = "t4-manager-actor-service")]
            Self::HistorySyncedLoss => history_synced_loss_self(command),
            #[cfg(test)]
            Self::Diagnostic => super::diagnostic::exact_self(&arguments(command)?),
        }
    }
}

#[cfg(test)]
#[path = "restore_abort_cached_owner_tests.rs"]
mod cached_owner_tests;
#[cfg(test)]
#[path = "restore_abort_retained_parent_prototype.rs"]
mod retained_parent_prototype;

#[cfg(feature = "t4-manager-actor-service")]
#[path = "manager_actor_capture.rs"]
pub(crate) mod actor_capture;

#[cfg(feature = "t4-manager-actor-service")]
#[path = "manager_actor_canonical.rs"]
pub(crate) mod actor_canonical;

// Ordinary builds evaluate only the original expression. The alternate arm
// is private to an inactive test prototype, not a runtime permission fallback.
macro_rules! retained_original {
    ($bundle:expr, $field:ident, $ordinary:expr) => {{
        #[cfg(test)]
        {
            if let Some(bundle) = $bundle.as_mut() {
                bundle.$field.take().ok_or(())
            } else {
                $ordinary
            }
        }
        #[cfg(not(test))]
        {
            $ordinary
        }
    }};
}
const MAX_PIDS: usize = 4096;
const MAX_STATUS: usize = 64 * 1024;
const MAX_COMMAND: usize = 128 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;

struct Budget {
    until: Instant,
    remaining: usize,
}

impl Budget {
    fn new() -> Self {
        Self {
            until: Instant::now() + Duration::from_secs(2),
            remaining: MAX_TOTAL,
        }
    }
    fn check(&self) -> Result<()> {
        if Instant::now() < self.until {
            Ok(())
        } else {
            Err(())
        }
    }
    fn charge(&mut self, size: usize) -> Result<()> {
        self.check()?;
        self.remaining = self.remaining.checked_sub(size).ok_or(())?;
        Ok(())
    }
}

fn identity(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
}

fn executable_identity(a: &Metadata, b: &Metadata) -> bool {
    identity(a, b)
        && a.len() == b.len()
        && a.nlink() == b.nlink()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

struct TrustedExecutable {
    file: File,
    metadata: Metadata,
    parents: Vec<(File, Metadata)>,
    path: &'static str,
}

impl TrustedExecutable {
    fn capture(path: &'static str) -> Result<Self> {
        use std::path::Component;
        let mut parent = File::from(
            open(
                Path::new("/"),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        let components: Vec<_> = Path::new(path).components().collect();
        if components.first() != Some(&Component::RootDir) || components.len() < 2 {
            return Err(());
        }
        let mut parents = Vec::new();
        for component in &components[1..components.len() - 1] {
            let metadata = parent.metadata().map_err(|_| ())?;
            if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                return Err(());
            }
            let Component::Normal(name) = component else {
                return Err(());
            };
            let next = directory(&parent, name.to_str().ok_or(())?)?;
            parents.push((parent, metadata));
            parent = next;
        }
        let metadata = parent.metadata().map_err(|_| ())?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(());
        }
        let Component::Normal(name) = components.last().ok_or(())? else {
            return Err(());
        };
        let file = File::from(
            openat(
                &parent,
                Path::new(name),
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        parents.push((parent, metadata));
        let metadata = file.metadata().map_err(|_| ())?;
        if !metadata.is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || metadata.mode() & 0o111 == 0
            || metadata.nlink() == 0
        {
            return Err(());
        }
        Ok(Self {
            file,
            metadata,
            parents,
            path,
        })
    }

    fn recheck(&self) -> Result<()> {
        let current = Self::capture(self.path)?;
        if !executable_identity(&self.metadata, &self.file.metadata().map_err(|_| ())?)
            || !executable_identity(&self.metadata, &current.metadata)
            || self.parents.len() != current.parents.len()
        {
            return Err(());
        }
        for ((held, metadata), (_, named)) in self.parents.iter().zip(&current.parents) {
            if !identity(metadata, &held.metadata().map_err(|_| ())?) || !identity(metadata, named)
            {
                return Err(());
            }
        }
        Ok(())
    }

    fn exec_path(&self) -> String {
        // Linux resolves this original executable FD before exec's CLOEXEC
        // close. No caller-selected program/FD or inherited private evidence.
        format!("/proc/self/fd/{}", self.file.as_raw_fd())
    }
}

fn directory(parent: &File, name: &str) -> Result<File> {
    openat(
        parent,
        name,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| ())
}

fn proc_bytes(
    parent: &File,
    name: &str,
    cap: usize,
    budget: &mut Budget,
) -> Result<Zeroizing<Vec<u8>>> {
    budget.check()?;
    let file = File::from(
        openat(
            parent,
            name,
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?,
    );
    if !file.metadata().map_err(|_| ())?.is_file()
        || fstatfs(&file).map_err(|_| ())?.filesystem_type() != PROC_SUPER_MAGIC
    {
        return Err(());
    }
    let mut bytes = Zeroizing::new(Vec::new());
    file.take((cap + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    budget.charge(bytes.len())?;
    if bytes.len() > cap {
        return Err(());
    }
    Ok(bytes)
}

fn decimal(text: &[u8]) -> Result<u32> {
    if text.is_empty() || !text.iter().all(u8::is_ascii_digit) {
        return Err(());
    }
    std::str::from_utf8(text)
        .map_err(|_| ())?
        .parse()
        .map_err(|_| ())
}

#[derive(PartialEq, Eq)]
struct Status {
    uids: [u32; 4],
    namespace_pids: Vec<u32>,
}

fn status(bytes: &[u8], pid: u32) -> Result<Status> {
    let mut uids = None;
    let mut own_pid = None;
    let mut namespace_pids = None;
    for line in bytes.split(|b| *b == b'\n') {
        if let Some(values) = line.strip_prefix(b"Uid:") {
            if uids.is_some() {
                return Err(());
            }
            let values: Vec<_> = values
                .split(u8::is_ascii_whitespace)
                .filter(|v| !v.is_empty())
                .map(decimal)
                .collect::<Result<_>>()?;
            uids = Some(<[u32; 4]>::try_from(values).map_err(|_| ())?);
        } else if let Some(value) = line.strip_prefix(b"Pid:") {
            if own_pid.is_some() {
                return Err(());
            }
            let values: Vec<_> = value
                .split(u8::is_ascii_whitespace)
                .filter(|v| !v.is_empty())
                .collect();
            if values.len() != 1 {
                return Err(());
            }
            own_pid = Some(decimal(values[0])?);
        } else if let Some(values) = line.strip_prefix(b"NSpid:") {
            if namespace_pids.is_some() {
                return Err(());
            }
            namespace_pids = Some(
                values
                    .split(u8::is_ascii_whitespace)
                    .filter(|v| !v.is_empty())
                    .map(decimal)
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    let namespace_pids = namespace_pids.ok_or(())?;
    if own_pid != Some(pid) || namespace_pids.first() != Some(&pid) || namespace_pids.len() > 32 {
        return Err(());
    }
    Ok(Status {
        uids: uids.ok_or(())?,
        namespace_pids,
    })
}

fn start_time(bytes: &[u8], pid: u32) -> Result<u64> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if !text.ends_with('\n') {
        return Err(());
    }
    let (first, _) = text.split_once(" (").ok_or(())?;
    if decimal(first.as_bytes())? != pid {
        return Err(());
    }
    let (_, fields) = text.rsplit_once(") ").ok_or(())?;
    let fields: Vec<_> = fields.split_ascii_whitespace().collect();
    if fields.len() != 50
        || fields[0].len() != 1
        || fields[1..]
            .iter()
            .any(|field| field.parse::<i128>().is_err())
    {
        return Err(());
    }
    let start = fields.get(19).ok_or(())?;
    if !start.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    if start == 0 {
        return Err(());
    }
    Ok(start)
}

fn arguments(bytes: &[u8]) -> Result<Vec<&[u8]>> {
    let body = bytes.strip_suffix(&[0]).ok_or(())?;
    let args: Vec<_> = body.split(|b| *b == 0).collect();
    if args.first().is_none_or(|arg| arg.is_empty()) {
        return Err(());
    }
    Ok(args)
}

fn recovery_self(command: &[u8]) -> Result<()> {
    let args = arguments(command)?;
    if args.len() != 4 || args[1..] != [b"restore".as_slice(), b"abort", b"--confirm-rollback"] {
        return Err(());
    }
    Ok(())
}

#[cfg(feature = "t4-manager-actor-service")]
fn old_recovery_self(command: &[u8]) -> Result<()> {
    let args = arguments(command)?;
    if args.len() != 4 || args[1] != b"restore" {
        return Err(());
    }
    match (args[2], args[3]) {
        (b"recover-old" | b"recover-mixed", b"--confirm-rollback")
        | (b"recover-committed", b"--confirm-completion")
        | (
            b"recover-final-old"
            | b"recover-final-new"
            | b"recover-ticket-new"
            | b"recover-complete-new",
            b"--confirm-disposition",
        ) => Ok(()),
        _ => Err(()),
    }
}

#[cfg(feature = "t4-manager-actor-service")]
fn consumer_recovery_self(command: &[u8]) -> Result<()> {
    let args = arguments(command)?;
    if args.len() != 2 || args[1] != b"tui" {
        return Err(());
    }
    Ok(())
}

#[cfg(feature = "t4-manager-actor-service")]
fn closure_cut_self(command: &[u8]) -> Result<()> {
    let args = arguments(command)?;
    if args.len() != 4
        || !(args[1..]
            == [
                b"developer".as_slice(),
                b"cut-new-complete-after-closure",
                b"--confirm-private-closure-cut",
            ]
            || args[1..]
                == [
                    b"developer".as_slice(),
                    b"cut-new-complete-after-closure-with-history",
                    b"--confirm-private-history-closure-cut",
                ])
    {
        return Err(());
    }
    Ok(())
}

#[cfg(feature = "t4-manager-actor-service")]
fn complete_only_cut_self(command: &[u8]) -> Result<()> {
    let args = arguments(command)?;
    if args.len() != 4
        || args[1..]
            != [
                b"developer".as_slice(),
                b"cut-new-complete-only-with-history",
                b"--confirm-private-history-complete-only-cut",
            ]
    {
        return Err(());
    }
    Ok(())
}

#[cfg(feature = "t4-manager-actor-service")]
fn history_synced_loss_self(command: &[u8]) -> Result<()> {
    let args = arguments(command)?;
    if args.len() != 4
        || args[1..]
            != [
                b"developer".as_slice(),
                b"lose-new-complete-at-history-synced",
                b"--confirm-private-history-synced-process-loss",
            ]
    {
        return Err(());
    }
    Ok(())
}

fn same_namespace(left: &File, right: &File) -> Result<()> {
    if identity(
        &left.metadata().map_err(|_| ())?,
        &right.metadata().map_err(|_| ())?,
    ) {
        Ok(())
    } else {
        Err(())
    }
}

fn owner_name(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_suffix(b" (deleted)").unwrap_or(bytes);
    let name = bytes.rsplit(|b| *b == b'/').next().unwrap_or(bytes);
    name.starts_with(b"omavless") || name.starts_with(b"OmaVLESS")
}

fn daemon_candidate(command: &[u8], executable: &[u8], comm: &[u8]) -> Result<bool> {
    let args = arguments(command)?;
    Ok(args.iter().any(|arg| *arg == b"daemon")
        || owner_name(args[0])
        || owner_name(executable)
        || owner_name(comm.strip_suffix(b"\n").unwrap_or(comm)))
}

fn magic_file(parent: &File, name: &str) -> Result<File> {
    // These are explicit procfs magic-link reads, not caller-selected paths.
    openat(
        parent,
        name,
        OFlag::O_PATH | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        // Reuse this exact failed syscall result: no diagnostic probe or retry.
        #[cfg(test)]
        crate::restore_abort_cli::diagnostic::manager_executable_open_error(error);
        #[cfg(not(test))]
        let _ = error;
    })
}

fn proc_link(parent: &File, name: &str) -> Result<Zeroizing<Vec<u8>>> {
    use std::os::unix::ffi::OsStringExt;
    let value =
        fs::read_link(format!("/proc/self/fd/{}/{name}", parent.as_raw_fd())).map_err(|_| ())?;
    let bytes = Zeroizing::new(value.into_os_string().into_vec());
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(());
    }
    Ok(bytes)
}

struct Process {
    pid: u32,
    directory: File,
    directory_identity: Metadata,
    start: u64,
    status: Status,
    command: Zeroizing<Vec<u8>>,
    comm: Zeroizing<Vec<u8>>,
    executable: File,
    executable_identity: Metadata,
    executable_name: Zeroizing<Vec<u8>>,
    #[cfg(test)]
    retained_parent: Option<std::rc::Rc<retained_parent_prototype::LocalParent>>,
}

impl Process {
    fn capture(root: &File, pid: u32, budget: &mut Budget) -> Result<Self> {
        Self::capture_inner(
            root,
            pid,
            budget,
            #[cfg(test)]
            None,
        )
    }

    fn capture_inner(
        root: &File,
        pid: u32,
        budget: &mut Budget,
        #[cfg(test)] retained_parent: Option<std::rc::Rc<retained_parent_prototype::LocalParent>>,
    ) -> Result<Self> {
        let directory = capture_step!(DirectoryOpen, directory(root, &pid.to_string()))?;
        let directory_identity =
            capture_step!(DirectoryMetadata, directory.metadata()).map_err(|_| ())?;
        let raw = capture_step!(StatRead, proc_bytes(&directory, "stat", MAX_STATUS, budget))?;
        let start = capture_step!(StatParse, start_time(&raw, pid))?;
        drop(raw);
        let raw = capture_step!(
            StatusRead,
            proc_bytes(&directory, "status", MAX_STATUS, budget)
        )?;
        let status = capture_step!(StatusParse, status(&raw, pid))?;
        drop(raw);
        let command = capture_step!(
            CommandRead,
            proc_bytes(&directory, "cmdline", MAX_COMMAND, budget)
        )?;
        capture_step!(CommandParse, arguments(&command))?;
        let comm = capture_step!(CommRead, proc_bytes(&directory, "comm", 4096, budget))?;
        #[cfg(test)]
        let mut forwarded = retained_parent
            .as_ref()
            .map(|parent| parent.consult(root, pid, &directory, start, budget))
            .transpose()?;
        let executable = capture_step!(
            ExecutableOpen,
            retained_original!(forwarded, executable, magic_file(&directory, "exe"))
        )?;
        let executable_identity =
            capture_step!(ExecutableMetadata, executable.metadata()).map_err(|_| ())?;
        if !capture_step!(ExecutableType, executable_identity.is_file()) {
            return Err(());
        }
        let executable_name = capture_step!(
            ExecutableLink,
            retained_original!(forwarded, executable_name, proc_link(&directory, "exe"))
        )?;
        let process = Self {
            pid,
            directory,
            directory_identity,
            start,
            status,
            command,
            comm,
            executable,
            executable_identity,
            executable_name,
            #[cfg(test)]
            retained_parent,
        };
        process.recheck(root, budget)?;
        Ok(process)
    }

    fn recheck(&self, root: &File, budget: &mut Budget) -> Result<()> {
        capture_step!(RecheckBudget, budget.check())?;
        let current = capture_step!(RecheckDirectory, directory(root, &self.pid.to_string()))?;
        #[cfg(test)]
        let mut forwarded = self
            .retained_parent
            .as_ref()
            .map(|parent| parent.consult(root, self.pid, &current, self.start, budget))
            .transpose()?;
        let executable = capture_step!(
            RecheckExecutable,
            retained_original!(forwarded, executable, magic_file(&current, "exe"))
        )?;
        if !identity(
            &self.directory_identity,
            &capture_step!(RecheckNamedDirectoryMetadata, current.metadata()).map_err(|_| ())?,
        ) || !identity(
            &self.directory_identity,
            &capture_step!(RecheckHeldDirectoryMetadata, self.directory.metadata())
                .map_err(|_| ())?,
        ) || !executable_identity(
            &self.executable_identity,
            &capture_step!(RecheckHeldExecutableMetadata, self.executable.metadata())
                .map_err(|_| ())?,
        ) || !executable_identity(
            &self.executable_identity,
            &capture_step!(RecheckNamedExecutableMetadata, executable.metadata())
                .map_err(|_| ())?,
        ) || self.start != {
            let raw = capture_step!(
                RecheckStatRead,
                proc_bytes(&current, "stat", MAX_STATUS, budget)
            )?;
            capture_step!(RecheckStatParse, start_time(&raw, self.pid))?
        } || self.status != {
            let raw = capture_step!(
                RecheckStatusRead,
                proc_bytes(&current, "status", MAX_STATUS, budget)
            )?;
            capture_step!(RecheckStatusParse, status(&raw, self.pid))?
        } || self.command
            != capture_step!(
                RecheckCommandRead,
                proc_bytes(&current, "cmdline", MAX_COMMAND, budget)
            )?
            || self.comm
                != capture_step!(RecheckCommRead, proc_bytes(&current, "comm", 4096, budget))?
            || self.executable_name
                != capture_step!(
                    RecheckExecutableLink,
                    retained_original!(forwarded, executable_name, proc_link(&current, "exe"))
                )?
        {
            return Err(());
        }
        Ok(())
    }

    #[cfg(test)]
    fn current_namespace(&self, root: &File, name: &str, budget: &mut Budget) -> Result<File> {
        if let Some(parent) = &self.retained_parent {
            if !matches!(name, "ns/pid" | "ns/user") {
                parent.refuse();
                return Err(());
            }
            let mut bundle = parent.consult(root, self.pid, &self.directory, self.start, budget)?;
            match name {
                "ns/pid" => bundle.pid_namespace.take().ok_or(()),
                "ns/user" => bundle.user_namespace.take().ok_or(()),
                _ => Err(()),
            }
        } else {
            magic_file(&self.directory, name)
        }
    }
}

fn pids(root: &File, budget: &Budget) -> Result<BTreeSet<u32>> {
    let mut result = BTreeSet::new();
    for entry in fs::read_dir(format!("/proc/self/fd/{}", root.as_raw_fd())).map_err(|_| ())? {
        budget.check()?;
        let entry = entry.map_err(|_| ())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(());
        };
        if name.bytes().all(|b| b.is_ascii_digit()) {
            let pid = decimal(name.as_bytes())?;
            if pid == 0 || !result.insert(pid) || result.len() > MAX_PIDS {
                return Err(());
            }
        }
    }
    if result.is_empty() {
        return Err(());
    }
    Ok(result)
}

fn mount_id(bytes: &[u8]) -> Result<u32> {
    let mut result = None;
    for line in bytes.split(|b| *b == b'\n') {
        if let Some(value) = line.strip_prefix(b"mnt_id:") {
            if result.is_some() {
                return Err(());
            }
            let values: Vec<_> = value
                .split(u8::is_ascii_whitespace)
                .filter(|v| !v.is_empty())
                .collect();
            if values.len() != 1 {
                return Err(());
            }
            result = Some(decimal(values[0])?);
        }
    }
    result.filter(|id| *id != 0).ok_or(())
}

fn visible_proc_mount(bytes: &[u8], id: u32) -> Result<()> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if !text.ends_with('\n') {
        return Err(());
    }
    let mut found = false;
    for line in text.lines() {
        let (left, right) = line.split_once(" - ").ok_or(())?;
        let left: Vec<_> = left.split_ascii_whitespace().collect();
        let right: Vec<_> = right.split_ascii_whitespace().collect();
        if left.len() < 6 || right.len() != 3 {
            return Err(());
        }
        if decimal(left[0].as_bytes())? != id {
            continue;
        }
        if found || left[3] != "/" || left[4] != "/proc" || right[0] != "proc" {
            return Err(());
        }
        found = true;
        // Do not infer visibility from the proc mount type alone. hidepid can
        // hide a nondumpable same-UID process before status can classify it.
        for option in left[5].split(',').chain(right[2].split(',')) {
            if !matches!(
                option,
                "rw" | "ro"
                    | "nosuid"
                    | "nodev"
                    | "noexec"
                    | "relatime"
                    | "strictatime"
                    | "noatime"
                    | "nodiratime"
                    | "lazytime"
                    | "hidepid=0"
            ) {
                return Err(());
            }
        }
    }
    if found { Ok(()) } else { Err(()) }
}

fn proc_visibility(root: &File, myself: &Process, budget: &mut Budget) -> Result<()> {
    let fdinfo = directory(&myself.directory, "fdinfo")?;
    let id = mount_id(&proc_bytes(
        &fdinfo,
        &root.as_raw_fd().to_string(),
        MAX_STATUS,
        budget,
    )?)?;
    visible_proc_mount(
        &proc_bytes(&myself.directory, "mountinfo", MAX_STATUS, budget)?,
        id,
    )
}

#[derive(Debug, PartialEq, Eq)]
enum InventoryError {
    Unknown,
    KnownOwner(u32),
}

impl From<()> for InventoryError {
    fn from(_: ()) -> Self {
        Self::Unknown
    }
}

fn inventory(
    root: &File,
    uid: u32,
    myself: &Process,
    budget: &mut Budget,
) -> std::result::Result<(), InventoryError> {
    let before = pids(root, budget)?;
    let order = before.iter().copied().collect();
    inspect_inventory(root, uid, myself, budget, &before, order)
}

fn inspect_inventory(
    root: &File,
    uid: u32,
    myself: &Process,
    budget: &mut Budget,
    before: &BTreeSet<u32>,
    order: Vec<u32>,
) -> std::result::Result<(), InventoryError> {
    inspect_inventory_inner(
        root,
        uid,
        myself,
        budget,
        before,
        order,
        #[cfg(test)]
        None,
    )
}

fn inspect_inventory_inner(
    root: &File,
    uid: u32,
    myself: &Process,
    budget: &mut Budget,
    before: &BTreeSet<u32>,
    order: Vec<u32>,
    #[cfg(test)] retained_parent: Option<&std::rc::Rc<retained_parent_prototype::LocalParent>>,
) -> std::result::Result<(), InventoryError> {
    if before.is_empty()
        || before.len() > MAX_PIDS
        || order.len() != before.len()
        || order.iter().copied().collect::<BTreeSet<_>>() != *before
    {
        return Err(InventoryError::Unknown);
    }
    let mut retained = Vec::new();
    let mut classified = Vec::new();
    for pid in &order {
        let directory = directory(root, &pid.to_string())?;
        let initial = status(&proc_bytes(&directory, "status", MAX_STATUS, budget)?, *pid)?;
        let start = start_time(&proc_bytes(&directory, "stat", MAX_STATUS, budget)?, *pid)?;
        let metadata = directory.metadata().map_err(|_| ())?;
        if initial.uids.contains(&uid) {
            // Only the exact retained original manager row can consult its
            // parent. Every other same-UID row retains ordinary strict capture.
            #[cfg(test)]
            let process = retained_parent_prototype::capture_inventory_row(
                root,
                *pid,
                start,
                &metadata,
                budget,
                retained_parent,
            )?;
            #[cfg(not(test))]
            let process = Process::capture(root, *pid, budget)?;
            if process.status != initial {
                return Err(InventoryError::Unknown);
            }
            if *pid == myself.pid {
                myself.recheck(root, budget)?;
                if process.start != myself.start
                    || !executable_identity(
                        &process.executable_identity,
                        &myself.executable_identity,
                    )
                {
                    return Err(InventoryError::Unknown);
                }
            } else if daemon_candidate(&process.command, &process.executable_name, &process.comm)? {
                return Err(InventoryError::KnownOwner(*pid));
            }
            retained.push(process);
        }
        // Do not silently classify a changing real/effective/saved/fs UID.
        if status(&proc_bytes(&directory, "status", MAX_STATUS, budget)?, *pid)? != initial {
            return Err(InventoryError::Unknown);
        }
        classified.push((*pid, directory, metadata, initial, start));
    }
    if pids(root, budget)? != *before {
        return Err(InventoryError::Unknown);
    }
    for process in retained {
        process.recheck(root, budget)?;
    }
    for (pid, held, metadata, initial, start) in classified {
        let current = directory(root, &pid.to_string())?;
        if !identity(&metadata, &held.metadata().map_err(|_| ())?)
            || !identity(&metadata, &current.metadata().map_err(|_| ())?)
            || status(&proc_bytes(&current, "status", MAX_STATUS, budget)?, pid)? != initial
            || start_time(&proc_bytes(&current, "stat", MAX_STATUS, budget)?, pid)? != start
        {
            return Err(InventoryError::Unknown);
        }
    }
    if pids(root, budget)? != *before {
        return Err(InventoryError::Unknown);
    }
    budget.check().map_err(Into::into)
}

fn service_record(bytes: &[u8], manager: bool) -> Result<u32> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    let mut active = None;
    let mut sub = None;
    let mut main = None;
    let mut control = None;
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(())?;
        match key {
            "ActiveState" if active.is_none() => active = Some(value),
            "SubState" if sub.is_none() => sub = Some(value),
            "MainPID" if main.is_none() => main = Some(decimal(value.as_bytes())?),
            "ControlPID" if control.is_none() => control = Some(decimal(value.as_bytes())?),
            _ => return Err(()),
        }
    }
    let pid = main.ok_or(())?;
    if control != Some(0)
        || (manager && (active != Some("active") || sub != Some("running") || pid == 0))
        || (!manager && (active != Some("inactive") || sub != Some("dead") || pid != 0))
    {
        return Err(());
    }
    Ok(pid)
}

fn terminal(status: WaitStatus, pid: Pid) -> Result<Option<WaitStatus>> {
    match status {
        WaitStatus::StillAlive => Ok(None),
        WaitStatus::Exited(found, _) | WaitStatus::Signaled(found, _, _) if found == pid => {
            Ok(Some(status))
        }
        _ => Err(()),
    }
}

fn known_reap(
    status: WaitStatus,
    pid: Pid,
    reap: impl FnOnce() -> Result<WaitStatus>,
) -> Result<()> {
    if terminal(status, pid)?.is_none() || reap()? != status {
        return Err(());
    }
    Ok(())
}

fn checked_once(refused: &Cell<bool>, observe: impl FnOnce() -> Result<()>) -> bool {
    if refused.get() {
        return false;
    }
    let result = observe();
    refused.set(result.is_err());
    result.is_ok()
}

fn query(uid: u32, unit: &str, system: bool, budget: &mut Budget) -> Result<Zeroizing<Vec<u8>>> {
    budget.check()?;
    let tool = TrustedExecutable::capture("/usr/bin/systemctl")?;
    tool.recheck()?;
    let mut child = Command::new(tool.exec_path())
        .arg0("/usr/bin/systemctl")
        .env_clear()
        .env("LC_ALL", "C")
        .env("SYSTEMD_COLORS", "0")
        .env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path=/run/user/{uid}/bus"),
        )
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            "unix:path=/run/dbus/system_bus_socket",
        )
        .args([
            if system { "--system" } else { "--user" },
            "--no-pager",
            "--no-ask-password",
            "show",
            unit,
            "--property=ActiveState",
            "--property=SubState",
            "--property=MainPID",
            "--property=ControlPID",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ())?;
    let pid = Pid::from_raw(i32::try_from(child.id()).map_err(|_| ())?);
    let mut stdout = child.stdout.take().ok_or(())?;
    // Child Drop never signals or waits. All outcomes below are raw, owned,
    // exact waits. Unknown/timeout stops this observer, with no cleanup retry.
    fcntl(&stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| ())?;
    let mut output = Zeroizing::new(Vec::new());
    let mut eof = false;
    let mut observed = None;
    loop {
        budget.check()?;
        if !eof {
            let mut bytes = [0_u8; 4096];
            match stdout.read(&mut bytes) {
                Ok(0) => eof = true,
                Ok(size) => {
                    budget.charge(size)?;
                    output.extend_from_slice(&bytes[..size]);
                    if output.len() > MAX_STATUS {
                        return Err(());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return Err(()),
            }
        }
        if observed.is_none() {
            observed = terminal(
                waitid(
                    Id::Pid(pid),
                    WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
                )
                .map_err(|_| ())?,
                pid,
            )?;
        }
        if let Some(status) = observed
            && (eof || status != WaitStatus::Exited(pid, 0))
        {
            known_reap(status, pid, || {
                waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(|_| ())
            })?;
            if status != WaitStatus::Exited(pid, 0) || !eof {
                return Err(());
            }
            tool.recheck()?;
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn listener_paths(uid: u32, socket: &Path) -> Result<[String; 2]> {
    let socket = socket.to_str().ok_or(())?;
    if socket.len() > 4096
        || !Path::new(socket).is_absolute()
        || !socket.ends_with("/omavless/control.sock")
        || socket.bytes().any(|b| b.is_ascii_control())
    {
        return Err(());
    }
    Ok([
        socket.to_owned(),
        format!("/run/user/{uid}/omavless/control.sock"),
    ])
}

// Private bounded reuse for the separately admitted recover-old path. Abort
// keeps its original command/record parser, and never selects these commands.
#[allow(dead_code)]
#[derive(Clone, Copy)]
enum FixedRecoveryCommand {
    Manager,
    Legacy,
    Runtime,
    StopRuntime,
    StartRuntime,
}

impl FixedRecoveryCommand {
    fn arguments(self, uid: u32) -> Vec<String> {
        let (system, unit) = match self {
            Self::Manager => (true, format!("user@{uid}.service")),
            Self::Legacy => (false, "omavless.service".to_owned()),
            Self::Runtime | Self::StopRuntime | Self::StartRuntime => {
                (false, "omavless-runtime.service".to_owned())
            }
        };
        let mut args = vec![
            if system { "--system" } else { "--user" }.to_owned(),
            "--no-pager".to_owned(),
            "--no-ask-password".to_owned(),
            match self {
                Self::StopRuntime => "stop",
                Self::StartRuntime => "start",
                _ => "show",
            }
            .to_owned(),
            unit,
        ];
        if !matches!(self, Self::StopRuntime | Self::StartRuntime) {
            for property in ["ActiveState", "SubState", "MainPID", "ControlPID", "Job"] {
                args.push(format!("--property={property}"));
            }
            if !system {
                for property in ["LoadState", "NeedDaemonReload"] {
                    args.push(format!("--property={property}"));
                }
            }
            if system {
                for property in ["Id", "User", "InvocationID", "LoadState", "ControlGroup"] {
                    args.push(format!("--property={property}"));
                }
            }
            if matches!(self, Self::Runtime) {
                for property in [
                    "RuntimeDirectoryPreserve",
                    "FragmentPath",
                    "DropInPaths",
                    "Slice",
                    "ControlGroup",
                    "Delegate",
                    "KillMode",
                ] {
                    args.push(format!("--property={property}"));
                }
            }
        }
        args
    }
}

#[derive(PartialEq, Eq)]
struct RuntimeUnitConfiguration {
    fragment: String,
    dropins: String,
}

fn recovery_fields(bytes: &[u8]) -> Result<std::collections::BTreeMap<&str, &str>> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if bytes.len() > MAX_STATUS || !text.ends_with('\n') {
        return Err(());
    }
    let mut fields = std::collections::BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(())?;
        if fields.insert(key, value).is_some() || value.bytes().any(|b| b.is_ascii_control()) {
            return Err(());
        }
    }
    Ok(fields)
}

#[cfg(test)]
fn recovery_manager_record(bytes: &[u8]) -> Result<u32> {
    let mut fields = recovery_fields(bytes)?;
    if fields.remove("Job") != Some("") || fields.len() != 4 {
        return Err(());
    }
    let mut original = Zeroizing::new(Vec::new());
    for (key, value) in fields {
        original.extend_from_slice(key.as_bytes());
        original.push(b'=');
        original.extend_from_slice(value.as_bytes());
        original.push(b'\n');
    }
    service_record(&original, true)
}

fn recovery_unit_record(
    bytes: &[u8],
    runtime: bool,
    off: bool,
    uid: u32,
) -> Result<Option<RuntimeUnitConfiguration>> {
    let mut fields = recovery_fields(bytes)?;
    let load = fields.remove("LoadState").ok_or(())?;
    if (load != "loaded" && (runtime || load != "not-found"))
        || fields.remove("NeedDaemonReload") != Some("no")
        || (off && fields.get("Job") != Some(&""))
    {
        return Err(());
    }
    let config = if runtime {
        if fields.remove("RuntimeDirectoryPreserve") != Some("yes") {
            return Err(());
        }
        let fragment = fields.remove("FragmentPath").ok_or(())?;
        let dropins = fields.remove("DropInPaths").ok_or(())?;
        let cgroup = fields.remove("ControlGroup").ok_or(())?;
        if fragment != "/usr/lib/systemd/user/omavless-runtime.service"
            || !dropins.is_empty()
            || fields.remove("Slice") != Some("app.slice")
            || fields.remove("Delegate") != Some("no")
            || fields.remove("KillMode") != Some("control-group")
            || (!cgroup.is_empty() && cgroup != recovery_cgroup::runtime_group(uid))
        {
            return Err(());
        }
        Some(RuntimeUnitConfiguration {
            fragment: fragment.to_owned(),
            dropins: dropins.to_owned(),
        })
    } else {
        None
    };
    fields.remove("Job").ok_or(())?;
    if fields.len() != 4 {
        return Err(());
    }
    if off {
        let mut original = Zeroizing::new(Vec::new());
        for (key, value) in fields {
            original.extend_from_slice(key.as_bytes());
            original.push(b'=');
            original.extend_from_slice(value.as_bytes());
            original.push(b'\n');
        }
        service_record(&original, false)?;
    } else {
        // Lost-owner qualification can cancel a queued automatic restart, but
        // cannot Stop a live main/control process or a currently starting unit.
        if decimal(fields.remove("MainPID").ok_or(())?.as_bytes())? != 0
            || decimal(fields.remove("ControlPID").ok_or(())?.as_bytes())? != 0
            || !matches!(
                (fields.remove("ActiveState"), fields.remove("SubState")),
                (Some("inactive"), Some("dead"))
                    | (Some("failed"), Some("failed"))
                    | (Some("activating"), Some("auto-restart"))
            )
            || !fields.is_empty()
        {
            return Err(());
        }
    }
    Ok(config)
}

#[allow(dead_code)]
struct OldRecoveryQueries {
    tool: TrustedExecutable,
    // Insert before any fallible child observation. Unknown outcome keeps the
    // exact Child/stdout here; no kill, reap retry or second command is allowed.
    child: RefCell<Option<Child>>,
    configuration: RefCell<Option<RuntimeUnitConfiguration>>,
    refused: Cell<bool>,
}

#[allow(dead_code)]
impl OldRecoveryQueries {
    fn run(
        &self,
        command: FixedRecoveryCommand,
        uid: u32,
        budget: &mut Budget,
    ) -> Result<Zeroizing<Vec<u8>>> {
        if self.refused.get() || self.child.borrow().is_some() {
            return Err(());
        }
        let result = self.run_inner(command, uid, budget);
        if result.is_err() {
            self.refused.set(true);
        }
        result
    }

    fn run_inner(
        &self,
        command: FixedRecoveryCommand,
        uid: u32,
        budget: &mut Budget,
    ) -> Result<Zeroizing<Vec<u8>>> {
        budget.check()?;
        self.tool.recheck()?;
        let child = Command::new(self.tool.exec_path())
            .arg0("/usr/bin/systemctl")
            .env_clear()
            .env("LC_ALL", "C")
            .env("SYSTEMD_COLORS", "0")
            .env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"))
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path=/run/user/{uid}/bus"),
            )
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                "unix:path=/run/dbus/system_bus_socket",
            )
            .args(command.arguments(uid))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ())?;
        *self.child.borrow_mut() = Some(child);
        let mut held = self.child.borrow_mut();
        let child = held.as_mut().ok_or(())?;
        let pid = Pid::from_raw(i32::try_from(child.id()).map_err(|_| ())?);
        let stdout = child.stdout.as_mut().ok_or(())?;
        fcntl(&*stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).map_err(|_| ())?;
        let mut output = Zeroizing::new(Vec::new());
        let mut eof = false;
        let mut observed = None;
        loop {
            budget.check()?;
            if !eof {
                let mut bytes = [0_u8; 4096];
                match stdout.read(&mut bytes) {
                    Ok(0) => eof = true,
                    Ok(size) => {
                        budget.charge(size)?;
                        output.extend_from_slice(&bytes[..size]);
                        if output.len() > MAX_STATUS {
                            return Err(());
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => return Err(()),
                }
            }
            if observed.is_none() {
                observed = terminal(
                    waitid(
                        Id::Pid(pid),
                        WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
                    )
                    .map_err(|_| ())?,
                    pid,
                )?;
            }
            if let Some(status) = observed
                && (eof || status != WaitStatus::Exited(pid, 0))
            {
                known_reap(status, pid, || {
                    waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(|_| ())
                })?;
                // Only this proven terminal child can be released.
                held.take();
                if status != WaitStatus::Exited(pid, 0) || !eof {
                    return Err(());
                }
                self.tool.recheck()?;
                return Ok(output);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn check_unit(&self, bytes: &[u8], runtime: bool, off: bool, uid: u32) -> Result<()> {
        let configuration = recovery_unit_record(bytes, runtime, off, uid)?;
        if runtime {
            let mut original = self.configuration.borrow_mut();
            if let Some(original) = original.as_ref() {
                if configuration.as_ref() != Some(original) {
                    return Err(());
                }
            } else {
                *original = configuration;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "t4-manager-actor-service")]
static RECOVERED_START_RESERVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(any(test, feature = "t4-manager-actor-service"))]
const RECOVERED_START_TIMEOUT: Duration = Duration::from_secs(15);

#[cfg(any(test, feature = "t4-manager-actor-service"))]
fn reserve_recovered_start(slot: &std::sync::atomic::AtomicBool) -> Result<()> {
    slot.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    )
    .map(|_| ())
    .map_err(|_| ())
}

#[cfg(any(test, feature = "t4-manager-actor-service"))]
#[derive(Default)]
struct RecoveredStartLifetime {
    attempted: bool,
    completed: bool,
    poisoned: bool,
}

#[cfg(any(test, feature = "t4-manager-actor-service"))]
impl RecoveredStartLifetime {
    fn begin(&mut self) -> Result<()> {
        if self.attempted || self.poisoned {
            self.poisoned = true;
            return Err(());
        }
        self.attempted = true;
        Ok(())
    }

    fn finish(&mut self, succeeded: bool) {
        self.completed = succeeded;
        self.poisoned |= !succeeded;
    }

    fn retain_on_drop(&self) -> bool {
        self.poisoned || (self.attempted && !self.completed)
    }
}

/// Separate availability action after the parent's positive consuming release.
/// Neither Clone nor a general service helper. One reservation per process bounds
/// leaked uncertain command custody, and no Drop signals, retries or waits.
#[cfg(feature = "t4-manager-actor-service")]
pub(crate) struct RecoveredRuntimeStart {
    uid: u32,
    queries: Option<Box<OldRecoveryQueries>>,
    lifetime: RecoveredStartLifetime,
}

#[cfg(feature = "t4-manager-actor-service")]
impl RecoveredRuntimeStart {
    pub(crate) fn reserve(uid: u32) -> Result<Self> {
        // Consume the global slot before path/tool acquisition. Never reset it,
        // even if reservation subsequently refuses without spawning a child.
        reserve_recovered_start(&RECOVERED_START_RESERVED)?;
        if uid != nix::unistd::getuid().as_raw() || uid != nix::unistd::geteuid().as_raw() {
            return Err(());
        }
        Ok(Self {
            uid,
            queries: Some(Box::new(OldRecoveryQueries {
                tool: TrustedExecutable::capture("/usr/bin/systemctl")?,
                child: RefCell::new(None),
                configuration: RefCell::new(None),
                refused: Cell::new(false),
            })),
            lifetime: RecoveredStartLifetime::default(),
        })
    }

    pub(crate) fn start_once(&mut self) -> Result<()> {
        self.lifetime.begin()?;
        let result = (|| {
            if self.uid != nix::unistd::getuid().as_raw()
                || self.uid != nix::unistd::geteuid().as_raw()
            {
                return Err(());
            }
            let queries = self.queries.as_ref().ok_or(())?;
            let mut budget = Budget {
                until: Instant::now() + RECOVERED_START_TIMEOUT,
                remaining: MAX_TOTAL,
            };
            let output = queries.run(FixedRecoveryCommand::StartRuntime, self.uid, &mut budget)?;
            if !output.is_empty() {
                return Err(());
            }
            Ok(())
        })();
        self.lifetime.finish(result.is_ok());
        result
    }
}

#[cfg(feature = "t4-manager-actor-service")]
impl Drop for RecoveredRuntimeStart {
    fn drop(&mut self) {
        if self.lifetime.retain_on_drop()
            && let Some(original) = self.queries.take()
        {
            // Retain this original trusted tool and exact Child/stdout without
            // guessed reaping or signalling. Fatal process loss is unavailable,
            // not descriptor survival. The global slot prevents replacement.
            let _ = Box::leak(original);
        }
    }
}

/// No singleton acquisition, socket read/retirement, journal or Start action.
/// The coordinator keeps this in its original recovery custody on uncertainty.
#[cfg(feature = "t4-manager-actor-service")]
pub(crate) struct OldRecoveryOwner {
    owner: Option<StoppedOwner>,
    queries: OldRecoveryQueries,
    uid: u32,
    socket: std::path::PathBuf,
    attempted: bool,
    qualified: bool,
    invocation: SelfInvocation,
}

#[cfg(feature = "t4-manager-actor-service")]
impl OldRecoveryOwner {
    pub(crate) fn prepare(uid: u32, socket: &Path) -> Result<Self> {
        Self::prepare_inner(uid, socket, SelfInvocation::OldRecovery)
    }

    pub(crate) fn prepare_consumer(uid: u32, socket: &Path) -> Result<Self> {
        Self::prepare_inner(uid, socket, SelfInvocation::ConsumerRecovery)
    }

    pub(crate) fn prepare_closure_cut(uid: u32, socket: &Path) -> Result<Self> {
        Self::prepare_inner(uid, socket, SelfInvocation::ClosureCut)
    }

    pub(crate) fn prepare_complete_only_cut(uid: u32, socket: &Path) -> Result<Self> {
        Self::prepare_inner(uid, socket, SelfInvocation::CompleteOnlyCut)
    }

    pub(crate) fn prepare_history_synced_loss(uid: u32, socket: &Path) -> Result<Self> {
        Self::prepare_inner(uid, socket, SelfInvocation::HistorySyncedLoss)
    }

    fn prepare_inner(uid: u32, socket: &Path, invocation: SelfInvocation) -> Result<Self> {
        // Reserve original command custody without a process query/capture.
        // The caller installs this guard before qualification can spawn a child.
        recovery_checkpoint!(invocation.progress(), Prepare);
        listener_paths(uid, socket)?;
        Ok(Self {
            owner: None,
            queries: OldRecoveryQueries {
                tool: TrustedExecutable::capture("/usr/bin/systemctl")?,
                child: RefCell::new(None),
                configuration: RefCell::new(None),
                refused: Cell::new(false),
            },
            uid,
            socket: socket.to_owned(),
            attempted: false,
            qualified: false,
            invocation,
        })
    }

    pub(crate) fn stop_and_qualify(&mut self) -> Result<()> {
        recovery_checkpoint!(self.invocation.progress(), QualifyEntry);
        if self.attempted || self.queries.refused.get() {
            if let Some(owner) = &self.owner {
                owner.refused.set(true);
            }
            return Err(());
        }
        self.attempted = true;
        let result = (|| {
            // Qualification already holds the SAME migration and singleton
            // locks. Initial manager query uses original retained Queries, so
            // capture failure cannot drop an unknown local Child.
            recovery_checkpoint!(self.invocation.progress(), CaptureEntry);
            self.owner = Some(StoppedOwner::capture_inner(
                self.uid,
                &self.socket,
                self.invocation,
                Some(&self.queries),
                #[cfg(test)]
                None,
            )?);
            let owner = self.owner.as_ref().ok_or(())?;
            qualify_lost_owner(
                // Reuse the complete original observation BEFORE Stop: all daemon
                // candidates, canonical listeners and live legacy operations must
                // be absent. Only runtime failed/queued-restart state is permitted.
                || {
                    recovery_checkpoint!(self.invocation.progress(), BeforeStopObservation);
                    owner.observe_inner(Some(&self.queries), false)
                },
                || {
                    recovery_checkpoint!(self.invocation.progress(), StopRuntime);
                    let mut budget = Budget::new();
                    let stopped = self.queries.run(
                        FixedRecoveryCommand::StopRuntime,
                        owner.uid,
                        &mut budget,
                    )?;
                    if stopped.is_empty() { Ok(()) } else { Err(()) }
                },
                || {
                    recovery_checkpoint!(self.invocation.progress(), AfterStopObservation);
                    owner.observe_inner(Some(&self.queries), true)
                },
            )
        })();
        if let Some(owner) = &self.owner {
            owner.refused.set(result.is_err());
        }
        self.qualified = result.is_ok();
        if self.qualified {
            recovery_checkpoint!(self.invocation.progress(), Qualified);
        } else {
            recovery_checkpoint!(self.invocation.progress(), QualificationRefused);
        }
        result
    }

    pub(crate) fn recheck(&self) -> Result<()> {
        let owner = self.owner.as_ref().ok_or(())?;
        if !self.qualified
            || self.queries.refused.get()
            || !checked_once(&owner.refused, || {
                owner.observe_inner(Some(&self.queries), true)
            })
        {
            return Err(());
        }
        Ok(())
    }
}

#[cfg(any(test, feature = "t4-manager-actor-service"))]
fn qualify_lost_owner(
    before: impl FnOnce() -> Result<()>,
    stop: impl FnOnce() -> Result<()>,
    after: impl FnOnce() -> Result<()>,
) -> Result<()> {
    before()?;
    stop()?;
    after()
}

fn no_listener(bytes: &[u8], own: &[String; 2]) -> Result<()> {
    std::str::from_utf8(bytes).map_err(|_| ())?;
    let body = bytes.strip_suffix(b"\n").ok_or(())?;
    let mut lines = body.split(|b| *b == b'\n');
    if lines.next() != Some(b"Num       RefCount Protocol Flags    Type St Inode Path".as_slice()) {
        return Err(());
    }
    for line in lines {
        let mut cursor = 0;
        let mut fields = Vec::new();
        for index in 0..7 {
            let start = cursor;
            if index == 6 {
                // unix_seq_show uses width-five decimal: leading spaces belong to the
                // inode field, not to the following pathname separator.
                while line.get(cursor) == Some(&b' ') {
                    cursor += 1;
                }
            }
            while cursor < line.len() && line[cursor] != b' ' {
                cursor += 1;
            }
            if start == cursor {
                return Err(());
            }
            fields.push(std::str::from_utf8(&line[start..cursor]).map_err(|_| ())?);
            if index < 6 {
                if line.get(cursor) != Some(&b' ') {
                    return Err(());
                }
                cursor += 1;
            }
        }
        if fields[0]
            .strip_suffix(':')
            .is_none_or(|n| n.is_empty() || u64::from_str_radix(n, 16).is_err())
            || fields[1..6]
                .iter()
                .any(|field| u64::from_str_radix(field, 16).is_err())
            || !fields[6]
                .trim_start_matches(' ')
                .parse::<u64>()
                .is_ok_and(|inode| fields[6] == format!("{inode:5}"))
        {
            return Err(());
        }
        if cursor < line.len() {
            // Only the one kernel separator is removed. Spaces inside or after
            // a pathname are data; never normalize a foreign listener into ours.
            if line[cursor] != b' ' {
                return Err(());
            }
            let pathname = &line[cursor + 1..];
            if own.iter().any(|name| pathname == name.as_bytes()) {
                return Err(());
            }
        }
    }
    Ok(())
}

pub(crate) struct StoppedOwner {
    #[cfg(feature = "t4-manager-actor-service")]
    progress: RecoveryProgress,
    root: File,
    root_identity: Metadata,
    uid: u32,
    myself: Process,
    manager: ManagerIdentity,
    recovery_cgroup: Option<recovery_cgroup::RecoveryCgroup>,
    namespaces: Vec<(&'static str, File, Metadata)>,
    listeners: [String; 2],
    refused: Cell<bool>,
}

impl StoppedOwner {
    pub(super) fn capture(uid: u32, socket: &Path) -> Result<Self> {
        Self::capture_inner(
            uid,
            socket,
            SelfInvocation::Recovery,
            None,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn capture_for_diagnostic(uid: u32, socket: &Path) -> Result<Self> {
        Self::capture_inner(uid, socket, SelfInvocation::Diagnostic, None, None)
    }

    fn capture_inner(
        uid: u32,
        socket: &Path,
        invocation: SelfInvocation,
        old_queries: Option<&OldRecoveryQueries>,
        #[cfg(test)] retained_parent: Option<std::rc::Rc<retained_parent_prototype::LocalParent>>,
    ) -> Result<Self> {
        // The plain historical Abort build cannot enter development recovery.
        #[cfg(feature = "t4-manager-actor-service")]
        let progress = invocation.progress();
        #[cfg(not(any(test, feature = "t4-manager-actor-service")))]
        if old_queries.is_some() {
            return Err(());
        }
        #[cfg(feature = "t4-manager-actor-service")]
        if invocation.lost_owner_recovery() != old_queries.is_some() {
            return Err(());
        }
        recovery_checkpoint!(progress, CaptureProcRoot);
        checkpoint!(ProcRoot);
        let listeners = listener_paths(uid, socket)?;
        let root = File::from(
            open(
                Path::new("/proc"),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        if fstatfs(&root).map_err(|_| ())?.filesystem_type() != PROC_SUPER_MAGIC {
            return Err(());
        }
        let root_identity = root.metadata().map_err(|_| ())?;
        let mut budget = Budget::new();
        checkpoint!(SelfProcess);
        recovery_checkpoint!(progress, CaptureSelfProcess);
        let myself = Process::capture(&root, std::process::id(), &mut budget)?;
        checkpoint!(SelfArguments);
        recovery_checkpoint!(progress, CaptureSelfArguments);
        invocation.validate(&myself.command)?;
        if myself.status.uids != [uid; 4] || myself.status.namespace_pids != [myself.pid] {
            return Err(());
        }
        checkpoint!(ProcVisibility);
        recovery_checkpoint!(progress, CaptureProcVisibility);
        proc_visibility(&root, &myself, &mut budget)?;
        let mut namespaces = Vec::new();
        checkpoint!(NamespaceHandles);
        recovery_checkpoint!(progress, CaptureNamespaces);
        for name in ["ns/pid", "ns/user", "ns/mnt", "ns/net"] {
            let file = magic_file(&myself.directory, name)?;
            let metadata = file.metadata().map_err(|_| ())?;
            namespaces.push((name, file, metadata));
        }
        checkpoint!(ManagerQuery);
        recovery_checkpoint!(progress, CaptureManagerQuery);
        let reply = match old_queries {
            Some(queries) => queries.run(FixedRecoveryCommand::Manager, uid, &mut budget)?,
            None => query(uid, &format!("user@{uid}.service"), true, &mut budget)?,
        };
        checkpoint!(ManagerRecord);
        recovery_checkpoint!(progress, CaptureManagerRecord);
        let manager = if old_queries.is_some() {
            #[cfg(not(any(test, feature = "t4-manager-actor-service")))]
            return Err(());
            #[cfg(any(test, feature = "t4-manager-actor-service"))]
            {
                let record = RecoveryManagerRecord::parse(&reply, uid)?;
                recovery_checkpoint!(progress, CaptureManagerIdentity);
                let manager = recovery_manager_identity::RecoveryManagerIdentity::capture(
                    &root,
                    record,
                    &mut budget,
                    #[cfg(feature = "t4-manager-actor-service")]
                    progress,
                )?;
                // The first observe_inner below brackets capture with the second
                // fixed system snapshot AFTER this graph is in OldRecoveryOwner.
                // An uncertain query therefore retains both original authorities.
                ManagerIdentity::Recovery(Box::new(manager))
            }
        } else {
            let pid = service_record(&reply, true)?;
            checkpoint!(ManagerProcess);
            let manager = Process::capture_inner(
                &root,
                pid,
                &mut budget,
                #[cfg(test)]
                retained_parent,
            )?;
            checkpoint!(ManagerExecutable);
            let manager_executable = TrustedExecutable::capture("/usr/lib/systemd/systemd")?;
            checkpoint!(ManagerIdentity);
            if !executable_identity(&manager_executable.metadata, &manager.executable_identity)
                || manager.status.uids != [uid; 4]
                || manager.status.namespace_pids != [pid]
                || !arguments(&manager.command)?.contains(&b"--user".as_slice())
            {
                return Err(());
            }
            ManagerIdentity::Historical {
                process: Box::new(manager),
                executable: Box::new(manager_executable),
            }
        };
        let recovery_cgroup = if old_queries.is_some() {
            recovery_checkpoint!(progress, CaptureCgroupHierarchy);
            Some(recovery_cgroup::RecoveryCgroup::capture(uid, &mut budget)?)
        } else {
            None
        };
        budget.check()?;
        let observer = Self {
            #[cfg(feature = "t4-manager-actor-service")]
            progress,
            root,
            root_identity,
            uid,
            myself,
            manager,
            recovery_cgroup,
            namespaces,
            listeners,
            refused: Cell::new(false),
        };
        #[cfg(feature = "t4-manager-actor-service")]
        if invocation.lost_owner_recovery() {
            // Original manager/namespace capture is inside already installed
            // recovery custody and after engine singleton exclusivity. This
            // capture path itself never issues Stop.
            recovery_checkpoint!(progress, CaptureFinalNamespaces);
            observer.namespace_boundary(&mut budget)?;
            recovery_checkpoint!(progress, CaptureFinalManager);
            observer.manager.recheck(&observer.root, &mut budget)?;
            recovery_checkpoint!(progress, CaptureFinalSelf);
            observer.myself.recheck(&observer.root, &mut budget)?;
            recovery_checkpoint!(progress, CaptureComplete);
            return Ok(observer);
        }
        if !observer.recheck() {
            return Err(());
        }
        Ok(observer)
    }

    pub(super) fn recheck(&self) -> bool {
        checked_once(&self.refused, || self.observe())
    }

    fn namespace_boundary(&self, _budget: &mut Budget) -> Result<()> {
        if !identity(&self.root_identity, &fs::metadata("/proc").map_err(|_| ())?)
            || !identity(&self.root_identity, &self.root.metadata().map_err(|_| ())?)
        {
            return Err(());
        }
        for (name, file, metadata) in &self.namespaces {
            if !identity(metadata, &file.metadata().map_err(|_| ())?)
                || !identity(
                    metadata,
                    &magic_file(&self.myself.directory, name)?
                        .metadata()
                        .map_err(|_| ())?,
                )
            {
                return Err(());
            }
        }
        if let Some(manager) = self.manager.historical_process() {
            for name in ["ns/pid", "ns/user"] {
                let manager_namespace = {
                    #[cfg(test)]
                    {
                        manager.current_namespace(&self.root, name, _budget)?
                    }
                    #[cfg(not(test))]
                    {
                        magic_file(&manager.directory, name)?
                    }
                };
                same_namespace(
                    &manager_namespace,
                    &magic_file(&self.myself.directory, name)?,
                )?;
            }
        }
        Ok(())
    }

    fn observe(&self) -> Result<()> {
        self.observe_inner(None, true)
    }

    fn inventory(&self, budget: &mut Budget) -> Result<()> {
        if self.manager.is_recovery() {
            // Cooperative supported writers obey the SAME retained locks.
            // This is recursive fixed-unit quiescence, not hostile same-UID
            // exclusion or a census of unrelated user/PAM/desktop processes.
            self.recovery_cgroup
                .as_ref()
                .ok_or(())?
                .require_empty(budget)
        } else {
            inventory(&self.root, self.uid, &self.myself, budget).map_err(|_| ())
        }
    }

    fn observe_inner(
        &self,
        #[allow(unused_variables)] old: Option<&OldRecoveryQueries>,
        runtime_off: bool,
    ) -> Result<()> {
        let mut budget = Budget::new();
        recovery_checkpoint!(self.progress, ObserveNamespaces);
        checkpoint!(NamespaceBoundary);
        self.namespace_boundary(&mut budget)?;
        checkpoint!(ProcVisibility);
        recovery_checkpoint!(self.progress, ObserveProcVisibility);
        proc_visibility(&self.root, &self.myself, &mut budget)?;
        checkpoint!(ManagerRecheck);
        recovery_checkpoint!(self.progress, ObserveManager);
        self.manager.recheck(&self.root, &mut budget)?;
        checkpoint!(ManagerQuery);
        recovery_checkpoint!(self.progress, ObserveManagerQuery);
        let reply = match old {
            Some(queries) => queries.run(FixedRecoveryCommand::Manager, self.uid, &mut budget)?,
            None => query(
                self.uid,
                &format!("user@{}.service", self.uid),
                true,
                &mut budget,
            )?,
        };
        checkpoint!(ManagerRecord);
        recovery_checkpoint!(self.progress, ObserveManagerRecord);
        let pid = if old.is_some() {
            #[cfg(not(any(test, feature = "t4-manager-actor-service")))]
            return Err(());
            #[cfg(any(test, feature = "t4-manager-actor-service"))]
            {
                let record = RecoveryManagerRecord::parse(&reply, self.uid)?;
                self.manager.recovery().ok_or(())?.matches_record(&record)?;
                record.pid
            }
        } else {
            service_record(&reply, true)?
        };
        if pid != self.manager.pid() {
            return Err(());
        }
        checkpoint!(ManagerIdentityRecheck);
        self.manager.recheck_executable()?;
        for unit in ["omavless.service", "omavless-runtime.service"] {
            if unit == "omavless.service" {
                recovery_checkpoint!(self.progress, ObserveLegacyQuery);
                checkpoint!(LegacyUnitQuery);
            } else {
                recovery_checkpoint!(self.progress, ObserveRuntimeQuery);
                checkpoint!(RuntimeUnitQuery);
            }
            let reply = match old {
                Some(queries) => queries.run(
                    if unit == "omavless.service" {
                        FixedRecoveryCommand::Legacy
                    } else {
                        FixedRecoveryCommand::Runtime
                    },
                    self.uid,
                    &mut budget,
                )?,
                None => query(self.uid, unit, false, &mut budget)?,
            };
            if unit == "omavless.service" {
                recovery_checkpoint!(self.progress, ObserveLegacyRecord);
                checkpoint!(LegacyUnitRecord);
            } else {
                recovery_checkpoint!(self.progress, ObserveRuntimeRecord);
                checkpoint!(RuntimeUnitRecord);
            }
            if let Some(queries) = old {
                let runtime = unit == "omavless-runtime.service";
                queries.check_unit(&reply, runtime, !runtime || runtime_off, self.uid)?;
            } else {
                service_record(&reply, false)?;
            }
        }
        checkpoint!(Inventory);
        recovery_checkpoint!(self.progress, ObserveInventory);
        #[cfg(test)]
        if let ManagerIdentity::Historical {
            process: manager, ..
        } = &self.manager
            && let Some(parent) = &manager.retained_parent
        {
            let before = pids(&self.root, &budget)?;
            let order = before.iter().copied().collect();
            inspect_inventory_inner(
                &self.root,
                self.uid,
                &self.myself,
                &mut budget,
                &before,
                order,
                Some(parent),
            )
            .map_err(|_| ())?;
        } else {
            self.inventory(&mut budget)?;
        }
        #[cfg(not(test))]
        self.inventory(&mut budget)?;
        checkpoint!(UnixTable);
        recovery_checkpoint!(self.progress, ObserveUnixTable);
        let net = directory(&self.myself.directory, "net")?;
        let table = proc_bytes(&net, "unix", 4 * 1024 * 1024, &mut budget)?;
        checkpoint!(UnixParser);
        recovery_checkpoint!(self.progress, ObserveListeners);
        no_listener(&table, &self.listeners)?;
        checkpoint!(FinalManager);
        recovery_checkpoint!(self.progress, ObserveFinalManager);
        self.manager.recheck(&self.root, &mut budget)?;
        checkpoint!(FinalSelf);
        recovery_checkpoint!(self.progress, ObserveFinalSelf);
        self.myself.recheck(&self.root, &mut budget)?;
        checkpoint!(FinalNamespaces);
        recovery_checkpoint!(self.progress, ObserveFinalNamespaces);
        self.namespace_boundary(&mut budget)?;
        checkpoint!(FinalProcVisibility);
        recovery_checkpoint!(self.progress, ObserveFinalProcVisibility);
        proc_visibility(&self.root, &self.myself, &mut budget)?;
        checkpoint!(FinalBudget);
        recovery_checkpoint!(self.progress, ObserveFinalBudget);
        budget.check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::signal::Signal;
    use std::io::Write;

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn recovery_progress_is_gated_bounded_and_ignores_broken_output() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let count = AtomicUsize::new(0);
        bounded_recovery_diagnostic(false, &count, b"fixed\n", |_| panic!("disabled"));
        bounded_recovery_diagnostic(true, &count, &[b'x'; 97], |_| panic!("oversized"));
        assert_eq!(count.load(Ordering::Relaxed), 0);
        let writes = Cell::new(0);
        for _ in 0..300 {
            bounded_recovery_diagnostic(true, &count, b"restore_owner_fixed\n", |bytes| {
                assert_eq!(bytes, b"restore_owner_fixed\n");
                writes.set(writes.get() + 1);
                -1 // Broken/interrupted writes do not retry or change control flow.
            });
        }
        assert_eq!(writes.get(), 256);
        assert_eq!(count.load(Ordering::Relaxed), 256);
    }

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn consumer_recovery_progress_never_reaches_original_writer() {
        let count = std::sync::atomic::AtomicUsize::new(0);
        bounded_recovery_diagnostic(
            SelfInvocation::ConsumerRecovery.progress().enabled(),
            &count,
            b"restore_owner_CaptureEntry\n",
            |_| panic!("consumer terminal writer must not run"),
        );
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert!(SelfInvocation::OldRecovery.progress().enabled());
        assert!(!SelfInvocation::Recovery.progress().enabled());
        assert!(!SelfInvocation::Diagnostic.progress().enabled());
    }

    #[test]
    fn recovered_start_command_and_lifetime_are_fixed_and_once_only() {
        assert_eq!(
            FixedRecoveryCommand::StartRuntime.arguments(1000),
            [
                "--user",
                "--no-pager",
                "--no-ask-password",
                "start",
                "omavless-runtime.service",
            ]
        );
        assert_eq!(RECOVERED_START_TIMEOUT, Duration::from_secs(15));
        let slot = std::sync::atomic::AtomicBool::new(false);
        assert!(reserve_recovered_start(&slot).is_ok());
        assert!(reserve_recovered_start(&slot).is_err());

        let mut failed = RecoveredStartLifetime::default();
        assert!(!failed.retain_on_drop());
        assert!(failed.begin().is_ok());
        assert!(failed.retain_on_drop());
        failed.finish(false);
        assert!(failed.retain_on_drop());
        assert!(failed.begin().is_err());
        assert!(failed.retain_on_drop());

        let mut positive = RecoveredStartLifetime::default();
        assert!(positive.begin().is_ok());
        positive.finish(true);
        assert!(!positive.retain_on_drop());
        assert!(positive.begin().is_err());
        assert!(positive.retain_on_drop());
    }

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn old_recovery_self_is_exact_and_abort_is_unchanged() {
        assert!(
            old_recovery_self(b"binary\0restore\0recover-complete-new\0--confirm-disposition\0")
                .is_ok()
        );
        assert!(
            old_recovery_self(b"binary\0restore\0recover-ticket-new\0--confirm-disposition\0")
                .is_ok()
        );
        assert!(
            old_recovery_self(b"binary\0restore\0recover-final-new\0--confirm-disposition\0")
                .is_ok()
        );
        assert!(
            old_recovery_self(b"binary\0restore\0recover-final-old\0--confirm-disposition\0")
                .is_ok()
        );
        assert!(old_recovery_self(b"binary\0restore\0recover-old\0--confirm-rollback\0").is_ok());
        assert!(old_recovery_self(b"binary\0restore\0recover-mixed\0--confirm-rollback\0").is_ok());
        assert!(
            old_recovery_self(b"binary\0restore\0recover-committed\0--confirm-completion\0")
                .is_ok()
        );
        for command in [
            b"binary\0restore\0abort\0--confirm-rollback\0".as_slice(),
            b"binary\0restore\0recover-old\0".as_slice(),
            b"binary\0restore\0recover-old\0--confirm-rollback\0extra\0".as_slice(),
            b"binary\0daemon\0".as_slice(),
            b"binary\0restore\0recover-committed\0--confirm-rollback\0".as_slice(),
            b"binary\0restore\0recover-mixed\0--confirm-completion\0".as_slice(),
            b"binary\0restore\0recover-new\0--confirm-completion\0".as_slice(),
            b"binary\0restore\0recover-final-old\0--confirm-rollback\0".as_slice(),
            b"binary\0restore\0recover-final-old\0--confirm-completion\0".as_slice(),
            b"binary\0restore\0recover-final-old\0--confirm-disposition\0step\0".as_slice(),
            b"binary\0restore\0recover-final-new\0--confirm-completion\0".as_slice(),
            b"binary\0restore\0recover-final-new\0--confirm-disposition\0step\0".as_slice(),
            b"binary\0restore\0recover-ticket-new\0--confirm-completion\0".as_slice(),
            b"binary\0restore\0recover-ticket-new\0--confirm-disposition\0step\0".as_slice(),
            b"binary\0restore\0recover-complete-new\0--confirm-completion\0".as_slice(),
            b"binary\0restore\0recover-complete-new\0--confirm-disposition\0step\0".as_slice(),
        ] {
            assert!(old_recovery_self(command).is_err());
        }
        assert!(recovery_self(b"binary\0restore\0abort\0--confirm-rollback\0").is_ok());
        assert!(recovery_self(b"binary\0restore\0recover-old\0--confirm-rollback\0").is_err());
    }

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn consumer_recovery_self_accepts_only_ordinary_tui_and_keeps_cli_guards() {
        let ordinary = b"/usr/bin/omavless\0tui\0";
        assert!(SelfInvocation::ConsumerRecovery.validate(ordinary).is_ok());
        assert!(SelfInvocation::ConsumerRecovery.lost_owner_recovery());
        assert!(SelfInvocation::OldRecovery.lost_owner_recovery());
        assert!(!SelfInvocation::Recovery.lost_owner_recovery());
        assert!(SelfInvocation::OldRecovery.validate(ordinary).is_err());
        assert!(SelfInvocation::Recovery.validate(ordinary).is_err());
        for command in [
            b"binary\0tui\0--developer-private-restore\0".as_slice(),
            b"binary\0tui\0--available\0".as_slice(),
            b"binary\0tui\0extra\0".as_slice(),
            b"binary\0tui\0\0".as_slice(),
            b"binary\0tui".as_slice(),
            b"\0tui\0".as_slice(),
            b"binary\0daemon\0".as_slice(),
            b"binary\0restore\0abort\0--confirm-rollback\0".as_slice(),
            b"binary\0restore\0recover-old\0--confirm-rollback\0".as_slice(),
        ] {
            assert!(SelfInvocation::ConsumerRecovery.validate(command).is_err());
        }
    }

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn closure_cut_self_is_distinct_fixed_and_uses_full_lost_owner_custody() {
        let command = b"/usr/bin/omavless\0developer\0cut-new-complete-after-closure\0--confirm-private-closure-cut\0";
        let invocation = SelfInvocation::ClosureCut;
        assert!(invocation.validate(command).is_ok());
        assert!(invocation.lost_owner_recovery());
        assert!(invocation.progress().enabled());
        let history_command = b"/usr/bin/omavless\0developer\0cut-new-complete-after-closure-with-history\0--confirm-private-history-closure-cut\0";
        assert!(invocation.validate(history_command).is_ok());
        for other in [
            SelfInvocation::Recovery,
            SelfInvocation::OldRecovery,
            SelfInvocation::ConsumerRecovery,
        ] {
            assert!(other.validate(command).is_err());
            assert!(other.validate(history_command).is_err());
        }
        for wrong in [
            b"binary\0restore\0recover-complete-new\0--confirm-disposition\0".as_slice(),
            b"binary\0restore\0abort\0--confirm-rollback\0".as_slice(),
            b"binary\0tui\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure\0--confirm-disposition\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure\0--confirm-private-closure-cut\0extra\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure\0--confirm-private-closure-cut".as_slice(),
            b"\0developer\0cut-new-complete-after-closure\0--confirm-private-closure-cut\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure-with-history\0--confirm-private-closure-cut\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure-with-history\0--confirm-private-history-closure-cut\0extra\0".as_slice(),
            b"binary\0developer\0cut-new-complete-after-closure-with-history\0--confirm-private-history-closure-cut".as_slice(),
        ] { assert!(invocation.validate(wrong).is_err()); }
    }

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn complete_only_cut_self_is_exact_distinct_and_keeps_lost_owner_custody() {
        let command = b"/usr/bin/omavless\0developer\0cut-new-complete-only-with-history\0--confirm-private-history-complete-only-cut\0";
        let invocation = SelfInvocation::CompleteOnlyCut;
        assert!(invocation.validate(command).is_ok());
        assert!(invocation.lost_owner_recovery() && invocation.progress().enabled());
        for other in [
            SelfInvocation::ClosureCut,
            SelfInvocation::Recovery,
            SelfInvocation::OldRecovery,
            SelfInvocation::ConsumerRecovery,
        ] {
            assert!(other.validate(command).is_err());
        }
        for wrong in [
            b"binary\0developer\0cut-new-complete-after-closure-with-history\0--confirm-private-history-closure-cut\0".as_slice(),
            b"binary\0developer\0cut-new-complete-only-with-history\0--confirm-private-history-closure-cut\0".as_slice(),
            b"binary\0developer\0cut-new-complete-only-with-history\0--confirm-private-history-complete-only-cut\0extra\0".as_slice(),
            b"binary\0developer\0cut-new-complete-only-with-history\0--confirm-private-history-complete-only-cut".as_slice(),
            b"\0developer\0cut-new-complete-only-with-history\0--confirm-private-history-complete-only-cut\0".as_slice(),
            b"binary\0tui\0".as_slice(),
        ] { assert!(invocation.validate(wrong).is_err()); }
    }

    #[cfg(feature = "t4-manager-actor-service")]
    #[test]
    fn history_synced_loss_self_is_exact_distinct_and_keeps_lost_owner_custody() {
        let command = b"/usr/bin/omavless\0developer\0lose-new-complete-at-history-synced\0--confirm-private-history-synced-process-loss\0";
        let invocation = SelfInvocation::HistorySyncedLoss;
        assert!(invocation.validate(command).is_ok());
        assert!(invocation.lost_owner_recovery() && invocation.progress().enabled());
        for other in [
            SelfInvocation::CompleteOnlyCut,
            SelfInvocation::ClosureCut,
            SelfInvocation::Recovery,
            SelfInvocation::OldRecovery,
            SelfInvocation::ConsumerRecovery,
        ] {
            assert!(other.validate(command).is_err());
        }
        for wrong in [
            b"binary\0developer\0cut-new-complete-only-with-history\0--confirm-private-history-complete-only-cut\0".as_slice(),
            b"binary\0developer\0lose-new-complete-at-history-synced\0--confirm-private-history-complete-only-cut\0".as_slice(),
            b"binary\0developer\0lose-new-complete-at-history-synced\0--confirm-private-history-synced-process-loss\0extra\0".as_slice(),
            b"binary\0developer\0lose-new-complete-at-history-synced\0--confirm-private-history-synced-process-loss".as_slice(),
            b"\0developer\0lose-new-complete-at-history-synced\0--confirm-private-history-synced-process-loss\0".as_slice(),
            b"binary\0tui\0".as_slice(),
        ] { assert!(invocation.validate(wrong).is_err()); }
    }

    #[test]
    fn old_recovery_unit_requires_current_preserved_runtime_and_no_job() {
        let off = b"LoadState=loaded\nNeedDaemonReload=no\nActiveState=inactive\nSubState=dead\nMainPID=0\nControlPID=0\nJob=\nRuntimeDirectoryPreserve=yes\nFragmentPath=/usr/lib/systemd/user/omavless-runtime.service\nDropInPaths=\nSlice=app.slice\nControlGroup=\nDelegate=no\nKillMode=control-group\n";
        assert!(recovery_unit_record(off, true, true, 1001).is_ok());
        let text = std::str::from_utf8(off).unwrap();
        for changed in [
            text.replace("Job=\n", "Job=42\n"),
            text.replace("Preserve=yes", "Preserve=no"),
            text.replace("Reload=no", "Reload=yes"),
            text.replace("ControlPID=0", "ControlPID=1"),
            text.replace("MainPID=0", "MainPID=1"),
            text.replace("Slice=app.slice", "Slice=other.slice"),
            text.replace("Delegate=no", "Delegate=yes"),
            text.replace("KillMode=control-group", "KillMode=process"),
            text.replace("DropInPaths=\n", "DropInPaths=/some.conf\n"),
            text.replace("ControlGroup=\n", "ControlGroup=/foreign\n"),
            text.replace("inactive", "active"),
            text.replace("loaded", "not-found"),
            text.replace(
                "/usr/lib/systemd/user/omavless-runtime.service",
                "/foreign.service",
            ),
            text.replace("Job=\n", ""),
            format!("{text}Job=\n"),
            format!("{text}Unknown=value\n"),
        ] {
            assert!(recovery_unit_record(changed.as_bytes(), true, true, 1001).is_err());
        }
        let group = text.replace(
            "ControlGroup=\n",
            &format!("ControlGroup={}\n", recovery_cgroup::runtime_group(1001)),
        );
        assert!(recovery_unit_record(group.as_bytes(), true, true, 1001).is_ok());
        assert!(recovery_unit_record(group.as_bytes(), true, true, 1002).is_err());
        let queued = text
            .replace("Job=\n", "Job=42\n")
            .replace("inactive", "failed")
            .replace("SubState=dead", "SubState=failed");
        assert!(recovery_unit_record(queued.as_bytes(), true, false, 1001).is_ok());
        assert!(recovery_unit_record(queued.as_bytes(), true, true, 1001).is_err());
        let restarting = queued
            .replace("ActiveState=failed", "ActiveState=activating")
            .replace("SubState=failed", "SubState=auto-restart");
        assert!(recovery_unit_record(restarting.as_bytes(), true, false, 1001).is_ok());
        for live in [
            queued.replace("MainPID=0", "MainPID=42"),
            queued.replace("ControlPID=0", "ControlPID=42"),
            queued
                .replace("ActiveState=failed", "ActiveState=active")
                .replace("SubState=failed", "SubState=running"),
            restarting.replace("SubState=auto-restart", "SubState=start"),
        ] {
            assert!(recovery_unit_record(live.as_bytes(), true, false, 1001).is_err());
        }
        let legacy = b"LoadState=not-found\nNeedDaemonReload=no\nActiveState=inactive\nSubState=dead\nMainPID=0\nControlPID=0\nJob=\n";
        assert!(recovery_unit_record(legacy, false, true, 1001).is_ok());
        assert!(recovery_unit_record(legacy, true, true, 1001).is_err());
        let legacy_queued = std::str::from_utf8(legacy)
            .unwrap()
            .replace("Job=\n", "Job=42\n");
        assert!(recovery_unit_record(legacy_queued.as_bytes(), false, true, 1001).is_err());
    }

    #[test]
    fn old_recovery_lost_owner_inventory_must_finish_before_fixed_stop() {
        let calls = RefCell::new(Vec::new());
        assert!(
            qualify_lost_owner(
                || {
                    calls.borrow_mut().push("before");
                    Err(())
                },
                || {
                    calls.borrow_mut().push("stop");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("after");
                    Ok(())
                },
            )
            .is_err()
        );
        assert_eq!(*calls.borrow(), ["before"]);
        calls.borrow_mut().clear();
        assert!(
            qualify_lost_owner(
                || {
                    calls.borrow_mut().push("before");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("stop");
                    Err(())
                },
                || {
                    calls.borrow_mut().push("after");
                    Ok(())
                },
            )
            .is_err()
        );
        assert_eq!(*calls.borrow(), ["before", "stop"]);
        calls.borrow_mut().clear();
        assert!(
            qualify_lost_owner(
                || {
                    calls.borrow_mut().push("before");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("stop");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("after");
                    Ok(())
                },
            )
            .is_ok()
        );
        assert_eq!(*calls.borrow(), ["before", "stop", "after"]);
    }

    #[test]
    fn old_recovery_manager_requires_no_queued_job_and_exact_original_shape() {
        let manager = b"ActiveState=active\nSubState=running\nMainPID=42\nControlPID=0\nJob=\n";
        assert_eq!(recovery_manager_record(manager), Ok(42));
        for bytes in [
            b"ActiveState=active\nSubState=running\nMainPID=42\nControlPID=0\nJob=5\n".as_slice(),
            b"ActiveState=active\nSubState=running\nMainPID=42\nControlPID=0\n".as_slice(),
            b"ActiveState=active\nSubState=running\nMainPID=42\nControlPID=0\nJob=\nJob=\n"
                .as_slice(),
            b"ActiveState=active\nSubState=running\nMainPID=0\nControlPID=0\nJob=\n".as_slice(),
        ] {
            assert!(recovery_manager_record(bytes).is_err());
        }
    }

    #[test]
    fn old_recovery_commands_are_fixed_and_have_no_start_or_shell() {
        let stop = FixedRecoveryCommand::StopRuntime.arguments(1000);
        assert_eq!(
            stop,
            [
                "--user",
                "--no-pager",
                "--no-ask-password",
                "stop",
                "omavless-runtime.service"
            ]
        );
        for command in [
            FixedRecoveryCommand::Manager,
            FixedRecoveryCommand::Legacy,
            FixedRecoveryCommand::Runtime,
        ] {
            let arguments = command.arguments(1000);
            assert!(arguments.iter().any(|arg| arg == "show"));
            assert!(
                !arguments
                    .iter()
                    .any(|arg| ["stop", "start", "restart", "sh", "bash"].contains(&arg.as_str()))
            );
            assert!(arguments.iter().any(|arg| arg == "--property=Job"));
        }
    }

    fn proc_root() -> File {
        File::open("/proc").unwrap()
    }

    #[test]
    fn capture_step_keeps_original_short_circuit_and_failure_order() -> Result<()> {
        // Actual production wrapper, synthetic bodies: no proc or process IO.
        let calls = Cell::new(0);
        let stopped = capture_step!(RecheckNamedDirectoryMetadata, {
            calls.set(calls.get() + 1);
            true
        }) || capture_step!(RecheckHeldDirectoryMetadata, {
            panic!("short-circuited later predicate")
        });
        assert!(stopped);
        assert_eq!(calls.get(), 1);
        let result = (|| {
            capture_step!(StatRead, {
                calls.set(calls.get() + 1);
                Err::<(), ()>(())
            })?;
            capture_step!(StatParse, {
                calls.set(99);
                Ok::<(), ()>(())
            })?;
            Ok::<(), ()>(())
        })();
        assert!(result.is_err());
        assert_eq!(calls.get(), 2);
        Ok(())
    }

    #[test]
    fn uid_inventory_uses_all_four_ids_not_proc_directory_owner() {
        for ids in ["1000 0 0 0", "0 1000 0 0", "0 0 1000 0", "0 0 0 1000"] {
            let raw = format!("Name:\tordinary\nPid:\t42\nUid:\t{ids}\nNSpid:\t42 1\n");
            let state = status(raw.as_bytes(), 42).unwrap();
            assert!(state.uids.contains(&1000));
            assert_eq!(state.namespace_pids, [42, 1]);
        }
        for raw in [
            "Pid: 42\nUid: 1000 1000 1000\nNSpid: 42\n",
            "Pid: 42\nUid: 1000 1000 1000 true\nNSpid: 42\n",
            "Pid: 42\nUid: 1000 1000 1000 1000\nUid: 0 0 0 0\nNSpid: 42\n",
            "Pid: 43\nUid: 1000 1000 1000 1000\nNSpid: 42\n",
            "Pid: 42\nUid: 1000 1000 1000 1000\nNSpid: 43\n",
            "Pid: 42\nUid: 1000 1000 1000 1000\n",
        ] {
            assert!(status(raw.as_bytes(), 42).is_err());
        }
    }

    #[test]
    fn normal_renamed_deleted_and_mixed_version_daemons_refuse_without_netns_filter() {
        // Classification deliberately receives no network namespace: a daemon
        // in another netns is not filtered out before argv/exe inspection.
        for (argv, executable, comm) in [
            (
                b"/usr/bin/omavless\0daemon\0".as_slice(),
                b"/usr/bin/omavless".as_slice(),
                b"omavless\n".as_slice(),
            ),
            (
                b"/renamed/version\0daemon\0",
                b"/removed/older-build (deleted)",
                b"renamed\n",
            ),
            (b"alias\0daemon\0", b"/arbitrary/next-version", b"alias\n"),
            (
                b"alias\0",
                b"/usr/bin/omavless-runtime (deleted)",
                b"alias\n",
            ),
            (b"omavless-old\0", b"/renamed/file", b"renamed\n"),
            (b"alias\0", b"/renamed/file", b"omavless_runtim\n"),
        ] {
            assert_eq!(daemon_candidate(argv, executable, comm), Ok(true));
        }
        assert_eq!(
            daemon_candidate(b"/usr/bin/editor\0file\0", b"/usr/bin/editor", b"editor\n"),
            Ok(false)
        );
        for argv in [b"".as_slice(), b"\0", b"alias", b"\0argument\0"] {
            assert!(daemon_candidate(argv, b"/usr/bin/editor", b"editor\n").is_err());
        }
    }

    #[test]
    fn actual_original_proc_descriptor_detects_starttime_exec_and_argv_drift() {
        let root = proc_root();
        for change in 0..3 {
            let mut process =
                Process::capture(&root, std::process::id(), &mut Budget::new()).unwrap();
            process.recheck(&root, &mut Budget::new()).unwrap();
            match change {
                0 => process.start += 1,
                1 => process.executable_identity = root.metadata().unwrap(),
                _ => process.command.push(b'X'),
            }
            assert!(process.recheck(&root, &mut Budget::new()).is_err());
        }
    }

    #[test]
    fn original_held_executable_metadata_cannot_adopt_same_bytes_new_inode() {
        let mut old = tempfile::tempfile().unwrap();
        let mut replacement = tempfile::tempfile().unwrap();
        old.write_all(b"same bytes").unwrap();
        replacement.write_all(b"same bytes").unwrap();
        assert!(!executable_identity(
            &old.metadata().unwrap(),
            &replacement.metadata().unwrap()
        ));
        assert!(
            !identity(&old.metadata().unwrap(), &replacement.metadata().unwrap()),
            "namespace identity uses device/inode, not a label"
        );
    }

    #[test]
    fn only_exact_recovery_self_and_matching_namespace_can_be_exempted() {
        assert!(recovery_self(b"binary\0restore\0abort\0--confirm-rollback\0").is_ok());
        for command in [
            b"binary\0daemon\0".as_slice(),
            b"binary\0",
            b"binary\0restore\0abort\0",
            b"binary\0restore\0abort\0--confirm-rollback\0extra\0",
        ] {
            assert!(recovery_self(command).is_err());
        }
        let pid = File::open("/proc/self/ns/pid").unwrap();
        let net = File::open("/proc/self/ns/net").unwrap();
        assert!(same_namespace(&pid, &pid.try_clone().unwrap()).is_ok());
        assert!(same_namespace(&pid, &net).is_err());
    }

    #[test]
    fn bounds_deadline_and_proc_type_are_fail_closed() {
        let root = proc_root();
        let process = directory(&root, &std::process::id().to_string()).unwrap();
        assert!(proc_bytes(&process, "status", 1, &mut Budget::new()).is_err());
        let mut budget = Budget::new();
        budget.remaining = 0;
        assert!(proc_bytes(&process, "status", MAX_STATUS, &mut budget).is_err());
        budget.until = Instant::now() - Duration::from_secs(1);
        assert!(budget.check().is_err());
        assert!(proc_bytes(&process, "fd", MAX_STATUS, &mut Budget::new()).is_err());
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("status"), b"pretend procfs").unwrap();
        assert!(
            proc_bytes(
                &File::open(temp.path()).unwrap(),
                "status",
                MAX_STATUS,
                &mut Budget::new()
            )
            .is_err()
        );
    }

    #[test]
    fn selected_proc_mount_rejects_hidden_partial_or_unbound_inventory() {
        let id = mount_id(b"pos:\t0\nflags:\t0100000\nmnt_id:\t42\n").unwrap();
        assert_eq!(id, 42);
        for bytes in [
            b"mnt_id: 42\nmnt_id: 42\n".as_slice(),
            b"mnt_id: true\n",
            b"other: 42\n",
            b"mnt_id: 0\n",
        ] {
            assert!(mount_id(bytes).is_err());
        }
        let normal = "42 1 0:1 / /proc rw,nosuid,nodev,noexec,relatime - proc proc rw\n";
        assert!(visible_proc_mount(normal.as_bytes(), 42).is_ok());
        for option in [
            "hidepid=1",
            "hidepid=2",
            "hidepid=invisible",
            "subset=pid",
            "gid=1000",
            "unknown",
        ] {
            let raw = format!("42 1 0:1 / /proc rw - proc proc rw,{option}\n");
            assert!(visible_proc_mount(raw.as_bytes(), 42).is_err());
        }
        assert!(visible_proc_mount(normal.as_bytes(), 43).is_err());
        assert!(visible_proc_mount(format!("{normal}{normal}").as_bytes(), 42).is_err());
        assert!(
            visible_proc_mount(normal.replace("/ /proc", "/hidden /proc").as_bytes(), 42).is_err()
        );
        assert!(visible_proc_mount(normal.replace("- proc", "- tmpfs").as_bytes(), 42).is_err());
        assert!(visible_proc_mount(normal.trim_end().as_bytes(), 42).is_err());
    }

    #[test]
    fn fixed_service_shape_rejects_duplicates_types_missing_and_nonempty_state() {
        let inactive = b"ActiveState=inactive\nSubState=dead\nMainPID=0\nControlPID=0\n";
        let active = b"ActiveState=active\nSubState=running\nMainPID=42\nControlPID=0\n";
        assert_eq!(service_record(inactive, false), Ok(0));
        assert_eq!(service_record(active, true), Ok(42));
        assert!(service_record(active, false).is_err());
        assert!(service_record(inactive, true).is_err());
        for tail in ["MainPID=0\n", "Extra=0\n", "\n", "ControlPID=false\n"] {
            let mut raw = inactive.to_vec();
            raw.extend_from_slice(tail.as_bytes());
            assert!(service_record(&raw, false).is_err());
        }
        for bad in [
            "ActiveState=inactive\nSubState=dead\nMainPID=false\nControlPID=0\n",
            "ActiveState=inactive\nSubState=dead\nMainPID=0\nControlPID=1\n",
            "ActiveState=inactive\nMainPID=0\nControlPID=0\n",
            "ActiveState=inactive\nSubState=dead\nMainPID=4294967296\nControlPID=0\n",
        ] {
            assert!(service_record(bad.as_bytes(), false).is_err());
        }
    }

    #[test]
    fn raw_wait_only_reaps_exact_known_terminal_and_never_promotes_unknown() {
        let pid = Pid::from_raw(42);
        for observed in [
            WaitStatus::StillAlive,
            WaitStatus::Exited(Pid::from_raw(43), 0),
            WaitStatus::Stopped(pid, Signal::SIGSTOP),
            WaitStatus::Continued(pid),
        ] {
            let called = Cell::new(false);
            assert!(
                known_reap(observed, pid, || {
                    called.set(true);
                    Ok(observed)
                })
                .is_err()
            );
            assert!(!called.get());
        }
        let exited = WaitStatus::Exited(pid, 0);
        assert!(known_reap(exited, pid, || Err(())).is_err());
        assert!(known_reap(exited, pid, || Ok(WaitStatus::Exited(pid, 1))).is_err());
        assert!(known_reap(exited, pid, || Ok(exited)).is_ok());
        assert_eq!(
            terminal(WaitStatus::Signaled(pid, Signal::SIGKILL, false), pid),
            Ok(Some(WaitStatus::Signaled(pid, Signal::SIGKILL, false)))
        );
    }

    #[test]
    fn first_uncertainty_blocks_all_followup_observation_or_query() {
        let refused = Cell::new(false);
        assert!(!checked_once(&refused, || Err(())));
        assert!(!checked_once(&refused, || panic!(
            "no retry, later query or invented recovery"
        )));
        let refused = Cell::new(false);
        assert!(checked_once(&refused, || Ok(())));
        assert!(!checked_once(&refused, || Err(())));
        assert!(!checked_once(&refused, || panic!(
            "failure cannot be healed"
        )));
    }

    #[test]
    fn kernel_listener_includes_unlinked_path_and_refuses_ambiguous_table() {
        const HEADER: &str = "Num       RefCount Protocol Flags    Type St Inode Path\n";
        let own = listener_paths(1001, Path::new("/removed/omavless/control.sock")).unwrap();
        assert!(
            no_listener(
                format!(
                    "{HEADER}0000: 00000002 00000000 00010000 0001 01    42 /unrelated/socket\n"
                )
                .as_bytes(),
                &own
            )
            .is_ok()
        );
        // There is deliberately no pathname-existence check.
        assert!(no_listener(format!("{HEADER}0000: 00000002 00000000 00010000 0001 01    42 /removed/omavless/control.sock\n").as_bytes(), &own).is_err());
        assert!(no_listener(b"malformed\n", &own).is_err());
        assert!(no_listener(format!("{HEADER}missing fields\n").as_bytes(), &own).is_err());
    }

    #[test]
    fn listener_scope_preserves_exact_encoded_paths_and_other_uids() {
        use std::os::unix::ffi::OsStrExt;
        const HEADER: &str = "Num       RefCount Protocol Flags    Type St Inode Path\n";
        let own = listener_paths(1001, Path::new("/private path/omavless/control.sock")).unwrap();
        for name in [&own[0], &own[1]] {
            let row = format!("{HEADER}0000: 00000002 00000000 00010000 0001 01    42 {name}\n");
            assert!(no_listener(row.as_bytes(), &own).is_err());
        }
        for name in [
            "/run/user/1000/omavless/control.sock",
            "/run/user/10010/omavless/control.sock",
            "/run/user/1001/omavless/control.sock.extra",
            "/prefix/run/user/1001/omavless/control.sock",
            "/run/user/1001/omavless/control.sock ",
            "/different /private path/omavless/control.sock",
            "/private  path/omavless/control.sock",
            "/private\tpath/omavless/control.sock",
        ] {
            let row = format!("{HEADER}0000: 00000002 00000000 00010000 0001 01    42 {name}\n");
            assert!(no_listener(row.as_bytes(), &own).is_ok());
        }
        for invalid in [
            "relative/omavless/control.sock",
            "/private\npath/omavless/control.sock",
            "/private/omavless/control.sock ",
            "/private/other.sock",
        ] {
            assert!(listener_paths(1001, Path::new(invalid)).is_err());
        }
        let oversized = format!("/{} /omavless/control.sock", "x".repeat(4096));
        assert!(listener_paths(1001, Path::new(&oversized)).is_err());
        assert!(
            listener_paths(
                1001,
                Path::new(std::ffi::OsStr::from_bytes(
                    b"/invalid\xff/omavless/control.sock"
                ))
            )
            .is_err()
        );
        assert!(
            no_listener(
                format!("{HEADER}0000:  00000002 00000000 00010000 0001 01    42 /unrelated\n")
                    .as_bytes(),
                &own
            )
            .is_err()
        );
    }

    #[test]
    fn listener_inode_kernel_width_preserves_exact_path_bytes() {
        const HEADER: &str = "Num       RefCount Protocol Flags    Type St Inode Path\n";
        const PREFIX: &str = "0000000000000000: 00000002 00000000 00010000 0001 01 ";
        let own = listener_paths(1001, Path::new("/private path/omavless/control.sock")).unwrap();
        for inode in [0_u64, 42, 9999, 10000, u64::MAX] {
            for suffix in [
                "",
                " /unrelated",
                "  /private path/omavless/control.sock",
                " /private path/omavless/control.sock ",
            ] {
                assert!(
                    no_listener(
                        format!("{HEADER}{PREFIX}{inode:5}{suffix}\n").as_bytes(),
                        &own
                    )
                    .is_ok()
                );
            }
            for path in &own {
                assert!(
                    no_listener(
                        format!("{HEADER}{PREFIX}{inode:5} {path}\n").as_bytes(),
                        &own
                    )
                    .is_err()
                );
            }
        }
        for inode in [
            "0",
            "42",
            "9999",
            "     0",
            "  42",
            "    42",
            " 0000",
            "00042",
            "010000",
            "+0042",
            "\t  42",
            "   4\t",
            "18446744073709551616",
            "     ",
        ] {
            assert!(
                no_listener(
                    format!("{HEADER}{PREFIX}{inode} /unrelated\n").as_bytes(),
                    &own
                )
                .is_err(),
                "malformed inode field"
            );
        }
        for prefix in [
            "0000:  00000002 00000000 00010000 0001 01 ",
            "0000:\t00000002 00000000 00010000 0001 01 ",
            "0000: 00000002 00000000 00010000 0001 01\t",
            "0000: 00000002 00000000 00010000 0001 GG ",
        ] {
            assert!(
                no_listener(
                    format!("{HEADER}{prefix}   42 /unrelated\n").as_bytes(),
                    &own
                )
                .is_err()
            );
        }
    }

    #[test]
    // Exact waitid(WNOWAIT) + waitpid below deliberately replace Child::wait.
    #[allow(clippy::zombie_processes)]
    fn original_cloexec_elf_fd_executes_without_path_fallback() {
        // Some distributions provide `true` as a symlink to a multicall ELF.
        // Keep capture's no-symlink/root-owned boundary; select a regular ELF
        // required by this project's test runner, without startup-file effects.
        let tool = TrustedExecutable::capture("/usr/bin/bash").unwrap();
        let child = Command::new(tool.exec_path())
            .arg0("/usr/bin/bash")
            .args(["--noprofile", "--norc", "-c", "exit 0"])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = Pid::from_raw(i32::try_from(child.id()).unwrap());
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            assert!(
                Instant::now() < until,
                "quarantine: no timeout kill or reap"
            );
            let observed = terminal(
                waitid(
                    Id::Pid(pid),
                    WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
                )
                .unwrap(),
                pid,
            )
            .unwrap();
            if let Some(status) = observed {
                known_reap(status, pid, || {
                    waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(|_| ())
                })
                .unwrap();
                assert_eq!(status, WaitStatus::Exited(pid, 0));
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        tool.recheck().unwrap();
        assert!(
            fcntl(&tool.file, FcntlArg::F_GETFD).unwrap() & nix::fcntl::FdFlag::FD_CLOEXEC.bits()
                != 0
        );
    }

    #[test]
    fn pid_count_cap_and_root_owned_executable_type_are_real_boundaries() {
        let root = tempfile::tempdir().unwrap();
        for pid in 1..=MAX_PIDS + 1 {
            File::create(root.path().join(pid.to_string())).unwrap();
        }
        let held = File::open(root.path()).unwrap();
        assert!(pids(&held, &Budget::new()).is_err());
        fs::remove_file(root.path().join((MAX_PIDS + 1).to_string())).unwrap();
        assert_eq!(pids(&held, &Budget::new()).unwrap().len(), MAX_PIDS);
        assert!(TrustedExecutable::capture("/proc/self/exe").is_err());
        assert!(TrustedExecutable::capture("/dev/null").is_err());
        assert!(TrustedExecutable::capture("/tmp/not-a-trusted-tool").is_err());
    }
}
