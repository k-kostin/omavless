// SPDX-License-Identifier: MIT
//! Fixed trusted-admin canonical observation, NOT product StoppedOwner authority.
//! All originals remain inside the actor; Result errors revoke and retain them.
//! No unwind/fatal custody, atomic process snapshot or hostile-admin guarantee.

use super::*;
use crate::manager_actor_service::Unavailable;
use nix::fcntl::AtFlags;
use nix::sys::stat::{FileStat, fstatat};
use rustix::fs::{RawDir, SeekFrom, seek};
use rustix::process::{Pid as RustPid, PidfdFlags, pidfd_open};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::process::{Child, ChildStdout};

#[path = "manager_actor_inventory_candidate.rs"]
mod inventory;
use inventory::{Class, Owner};

pub(crate) const NOFILE: u64 = 8320;
const FIXED: usize = 120;
// This path reports at most34 fixed Files, plus manager pidfd1 + actor base4
// + active query stdout/pidfd2 =41. Future lower36 remains RESERVED, not
// implemented canonical Restore permission. Fixed120 leaves43 spare roles;
// std spawn's unreported internal/partial/null/pipe acquisitions are an ordinary
// backend boundary, NOT a proven all-FD constructor bound or custody claim.
const FIXED_FILES: usize = 34;
const UID: u32 = 1000; // trusted developer packet origin, never client-selected
const DIRECTORY_ENTRIES: usize = 16384;
const LISTENERS: [&str; 2] = [
    "/run/user/1000/omavless/control.sock",
    // Fixed RuntimePaths::below(/run/user/1000) equals the canonical fallback.
    // No imported alternate runtime path or arbitrary client pathname.
    "/run/user/1000/omavless/control.sock",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueryPhase {
    Before,
    BeforeSpawn,
    Spawned,
    Eof,
    OriginalZero,
    Parsed,
    Completed,
}
const QUERY_PHASES: [QueryPhase; 7] = [
    QueryPhase::Before,
    QueryPhase::BeforeSpawn,
    QueryPhase::Spawned,
    QueryPhase::Eof,
    QueryPhase::OriginalZero,
    QueryPhase::Parsed,
    QueryPhase::Completed,
];
impl QueryPhase {
    fn label(self) -> &'static [u8] {
        match self {
            Self::Before => b"t4_actor_before_canonical_query\n",
            Self::BeforeSpawn => b"t4_actor_before_canonical_query_spawn\n",
            Self::Spawned => b"t4_actor_canonical_query_spawned\n",
            Self::Eof => b"t4_actor_canonical_query_stdout_eof\n",
            Self::OriginalZero => b"t4_actor_canonical_query_original_zero\n",
            Self::Parsed => b"t4_actor_canonical_query_parsed\n",
            Self::Completed => b"t4_actor_canonical_query_completed\n",
        }
    }
}
#[derive(Default)]
struct QueryProgress {
    next: usize,
    refused: bool,
}
impl QueryProgress {
    fn emit(
        &mut self,
        next: QueryPhase,
        output: impl FnOnce(&'static [u8]) -> Result<()>,
    ) -> Result<()> {
        if self.refused || QUERY_PHASES.get(self.next) != Some(&next) {
            self.refused = true;
            return Err(());
        }
        // Consume BEFORE output; emission refusal cannot repeat a milestone.
        self.next += 1;
        let result = output(next.label());
        if result.is_err() {
            self.refused = true;
        }
        result
    }
}

fn query_credentials(system: bool) -> Option<(u32, u32)> {
    if system { None } else { Some((UID, UID)) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImageOpenErrno {
    NoEntry,
    Access,
    Permission,
    ProcessLimit,
    SystemLimit,
    Other,
}
impl ImageOpenErrno {
    fn original(error: nix::errno::Errno) -> Self {
        use nix::errno::Errno;
        match error {
            Errno::ENOENT => Self::NoEntry,
            Errno::EACCES => Self::Access,
            Errno::EPERM => Self::Permission,
            Errno::EMFILE => Self::ProcessLimit,
            Errno::ENFILE => Self::SystemLimit,
            _ => Self::Other,
        }
    }
    fn label(self) -> &'static [u8] {
        match self {
            Self::NoEntry => b"t4_actor_inventory_image_open_enoent_refused\n",
            Self::Access => b"t4_actor_inventory_image_open_eacces_refused\n",
            Self::Permission => b"t4_actor_inventory_image_open_eperm_refused\n",
            Self::ProcessLimit => b"t4_actor_inventory_image_open_emfile_refused\n",
            Self::SystemLimit => b"t4_actor_inventory_image_open_enfile_refused\n",
            Self::Other => b"t4_actor_inventory_image_open_other_refused\n",
        }
    }
}

fn canonical_image_file(directory: &File) -> nix::Result<File> {
    // Same single original magic-link open as magic_file; no probe or retry.
    // Keep its typed error here instead of the shared helper's unit erasure.
    openat(
        directory,
        "exe",
        OFlag::O_PATH | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
}

fn record_image_open<T>(
    original: nix::Result<T>,
    reported: &mut Option<ImageOpenErrno>,
) -> std::result::Result<T, Unavailable> {
    original.map_err(|error| {
        // Only this original error is classified. No output/budget/IO inside
        // the acquisition callback; Owner revokes before the outer cut emits.
        *reported = Some(ImageOpenErrno::original(error));
        Unavailable
    })
}

fn image_open_failure(reported: Option<ImageOpenErrno>) -> InventoryFailure {
    reported.map_or(
        InventoryFailure::ImageOpen,
        InventoryFailure::ImageOpenErrno,
    )
}

#[derive(Clone, Copy)]
enum InventoryFailure {
    CatalogueSeek,
    CatalogueNext,
    CatalogueName,
    CatalogueLimit,
    CatalogueSet,
    OwnerAdmit,
    RowDirectory,
    StatusRead,
    StatusParse,
    StatRead,
    StatParse,
    Classify,
    ImageOpen,
    ImageOpenErrno(ImageOpenErrno),
    ImageShape,
    CommandRead,
    CommandParse,
    CommRead,
    CommParse,
    LinkRead,
    Daemon,
    RowCurrent,
    RowComplete,
    SweepSet,
}
impl InventoryFailure {
    fn label(self) -> &'static [u8] {
        match self {
            Self::CatalogueSeek => b"t4_actor_inventory_catalogue_seek_refused\n",
            Self::CatalogueNext => b"t4_actor_inventory_catalogue_next_refused\n",
            Self::CatalogueName => b"t4_actor_inventory_catalogue_name_refused\n",
            Self::CatalogueLimit => b"t4_actor_inventory_catalogue_limit_refused\n",
            Self::CatalogueSet => b"t4_actor_inventory_catalogue_set_refused\n",
            Self::OwnerAdmit => b"t4_actor_inventory_owner_admit_refused\n",
            Self::RowDirectory => b"t4_actor_inventory_row_directory_refused\n",
            Self::StatusRead => b"t4_actor_inventory_status_read_refused\n",
            Self::StatusParse => b"t4_actor_inventory_status_parse_refused\n",
            Self::StatRead => b"t4_actor_inventory_stat_read_refused\n",
            Self::StatParse => b"t4_actor_inventory_stat_parse_refused\n",
            Self::Classify => b"t4_actor_inventory_classify_refused\n",
            Self::ImageOpen => b"t4_actor_inventory_image_open_refused\n",
            Self::ImageOpenErrno(error) => error.label(),
            Self::ImageShape => b"t4_actor_inventory_image_shape_refused\n",
            Self::CommandRead => b"t4_actor_inventory_command_read_refused\n",
            Self::CommandParse => b"t4_actor_inventory_command_parse_refused\n",
            Self::CommRead => b"t4_actor_inventory_comm_read_refused\n",
            Self::CommParse => b"t4_actor_inventory_comm_parse_refused\n",
            Self::LinkRead => b"t4_actor_inventory_link_read_refused\n",
            Self::Daemon => b"t4_actor_inventory_daemon_refused\n",
            Self::RowCurrent => b"t4_actor_inventory_row_current_refused\n",
            Self::RowComplete => b"t4_actor_inventory_row_complete_refused\n",
            Self::SweepSet => b"t4_actor_inventory_sweep_set_refused\n",
        }
    }
}
#[derive(Default)]
struct InventoryDiagnostic {
    attempted: bool,
}
impl InventoryDiagnostic {
    fn result<T>(
        &mut self,
        cut: InventoryFailure,
        original: Result<T>,
        emit: impl FnOnce(&'static [u8]) -> Result<()>,
    ) -> Result<T> {
        if original.is_err() && !self.attempted {
            self.attempted = true; // BEFORE output; never retry/secondary log
            let _ = emit(cut.label());
        }
        // The operation ALREADY refused. Diagnostic failure cannot turn its
        // original Err into success, a retry or an alternative effect.
        original
    }
    fn cut<T>(&mut self, cut: InventoryFailure, original: Result<T>, budget: &Budget) -> Result<T> {
        self.result(cut, original, |label| phase(label, budget))
    }
}

#[derive(Clone, Copy)]
enum TextRole {
    Status,
    Stat,
    Command,
    Comm,
}
impl TextRole {
    fn name(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Stat => "stat",
            Self::Command => "cmdline",
            Self::Comm => "comm",
        }
    }
    fn read_failure(self) -> InventoryFailure {
        match self {
            Self::Status => InventoryFailure::StatusRead,
            Self::Stat => InventoryFailure::StatRead,
            Self::Command => InventoryFailure::CommandRead,
            Self::Comm => InventoryFailure::CommRead,
        }
    }
    fn parse_failure(self) -> InventoryFailure {
        match self {
            Self::Status => InventoryFailure::StatusParse,
            Self::Stat => InventoryFailure::StatParse,
            Self::Command => InventoryFailure::CommandParse,
            Self::Comm => InventoryFailure::CommParse,
        }
    }
}

fn phase(label: &'static [u8], budget: &Budget) -> Result<()> {
    use std::io::Write;
    budget.check()?;
    let mut output = std::io::stderr().lock();
    output.write_all(label).map_err(|_| ())?;
    output.flush().map_err(|_| ())?;
    budget.check()
}

fn available<T>(value: Result<T>) -> std::result::Result<T, Unavailable> {
    value.map_err(|_| Unavailable)
}
fn live(fd: &OwnedFd, budget: &Budget) -> Result<()> {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    budget.check()?;
    let mut fds = [PollFd::new(fd, PollFlags::IN)];
    if poll(
        &mut fds,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|_| ())?
        != 0
        || !fds[0].revents().is_empty()
    {
        return Err(());
    }
    budget.check()
}
fn named_matches(held: &Metadata, named: &FileStat, image: bool) -> bool {
    held.dev() == named.st_dev
        && held.ino() == named.st_ino
        && held.mode() == named.st_mode
        && held.uid() == named.st_uid
        && held.gid() == named.st_gid
        && (!image
            || (held.len() == named.st_size as u64
                && held.nlink() == named.st_nlink
                && held.mtime() == named.st_mtime
                && held.mtime_nsec() == named.st_mtime_nsec
                && held.ctime() == named.st_ctime
                && held.ctime_nsec() == named.st_ctime_nsec))
}
fn binding(
    parent: &File,
    name: &str,
    file: &File,
    initial: &Metadata,
    follow: bool,
    image: bool,
    budget: &Budget,
) -> Result<()> {
    budget.check()?;
    let held = file.metadata().map_err(|_| ())?;
    budget.check()?;
    let current = fstatat(
        parent,
        name,
        if follow {
            AtFlags::empty()
        } else {
            AtFlags::AT_SYMLINK_NOFOLLOW
        },
    )
    .map_err(|_| ())?;
    budget.check()?;
    if !(if image {
        executable_identity(initial, &held)
    } else {
        identity(initial, &held)
    }) || !named_matches(initial, &current, image)
    {
        return Err(());
    }
    Ok(())
}
fn read_original(
    file: &File,
    cap: usize,
    bytes: &mut Zeroizing<Vec<u8>>,
    budget: &mut Budget,
) -> Result<()> {
    bytes.clear();
    budget.check()?;
    let metadata = file.metadata().map_err(|_| ())?;
    budget.check()?;
    if bytes.capacity() < cap + 1 || !metadata.is_file() {
        return Err(());
    }
    let filesystem = fstatfs(file).map_err(|_| ())?;
    budget.check()?;
    if filesystem.filesystem_type() != PROC_SUPER_MAGIC {
        return Err(());
    }
    loop {
        budget.check()?;
        let mut chunk = [0; 4096];
        let left = (cap + 1).checked_sub(bytes.len()).ok_or(())?;
        if left == 0 {
            return Err(());
        }
        let size = file
            .read_at(&mut chunk[..left.min(4096)], bytes.len() as u64)
            .map_err(|_| ())?;
        bytes.extend_from_slice(&chunk[..size]); // bounded reserved buffer
        budget.charge(size)?;
        if size == 0 {
            return Ok(());
        }
        if bytes.len() > cap {
            return Err(());
        }
    }
}
fn link_original(directory: &File, bytes: &mut Zeroizing<Vec<u8>>, budget: &Budget) -> Result<()> {
    budget.check()?;
    let mut buffer = [0_u8; 4097];
    // One fixed magic-link syscall, no allocation-sized pathname traversal.
    let size = rustix::fs::readlinkat_raw(directory, "exe", &mut buffer[..]).map_err(|_| ())?;
    if size == 0 || size >= buffer.len() {
        return Err(());
    }
    bytes.clear();
    bytes.extend_from_slice(&buffer[..size]);
    budget.check()
}

struct Installed {
    parent: usize,
    name: &'static str,
    index: usize,
    metadata: Metadata,
    image: bool,
}
struct Named {
    parent: usize,
    name: String,
    index: usize,
    metadata: Metadata,
    follow: bool,
    image: bool,
}
struct Snapshot {
    pid: u32,
    directory: usize,
    stat: usize,
    status: usize,
    command: usize,
    comm: usize,
    image: usize,
    namespaces: [usize; 2],
    metadata: Metadata,
    image_metadata: Metadata,
    start: u64,
    identity: Status,
    command_bytes: Zeroizing<Vec<u8>>,
    comm_bytes: Zeroizing<Vec<u8>>,
    image_name: Zeroizing<Vec<u8>>,
}
struct Facts {
    metadata: Metadata,
    start: u64,
    identity: Status,
    image_metadata: Option<Metadata>,
    command: Zeroizing<Vec<u8>>,
    comm: Zeroizing<Vec<u8>>,
    image_name: Zeroizing<Vec<u8>>,
}
struct Query {
    child: Child,
    stdout: Option<ChildStdout>,
    pidfd: Option<OwnedFd>,
    output: Zeroizing<Vec<u8>>,
    eof: bool,
    positive: bool,
}

// Only EOF + the ORIGINAL unreaped child's zero status admits a wait. Nonzero
// and unexpected status refuse without reap/signal/retry. No PID-name adoption.
fn positive_query(eof: bool, status: WaitStatus, pid: Pid) -> Result<bool> {
    match status {
        WaitStatus::StillAlive => Ok(false),
        WaitStatus::Exited(found, 0) if found == pid => Ok(eof),
        _ => Err(()),
    }
}

pub(crate) struct Canonical {
    files: Vec<File>,
    installed: Vec<Installed>,
    root: Option<usize>,
    named: Vec<Named>,
    roots: Vec<(usize, &'static str, Metadata)>,
    myself: Option<Snapshot>,
    manager: Option<Snapshot>,
    manager_pidfd: Option<OwnedFd>,
    tool: Option<usize>,
    systemd: Option<usize>,
    observer_ns: [Option<usize>; 4],
    fdinfo: Option<usize>,
    mountinfo: Option<usize>,
    net: Option<usize>,
    unix: Option<usize>,
    query: Option<Query>,
    inventory_diagnostic: InventoryDiagnostic,
    rows: Owner<File>,
    facts: Vec<Facts>,
    names: Vec<u32>,
    buffer: Zeroizing<Vec<u8>>,
    link: Zeroizing<Vec<u8>>,
    directory_buffer: [MaybeUninit<u8>; 8192],
    consumed: bool,
    completed: bool,
    refused: bool,
}
impl Canonical {
    pub(crate) fn reserve() -> std::result::Result<Self, Unavailable> {
        let mut files = Vec::new();
        files
            .try_reserve_exact(FIXED_FILES)
            .map_err(|_| Unavailable)?;
        let mut installed = Vec::new();
        installed.try_reserve_exact(7).map_err(|_| Unavailable)?;
        let mut named = Vec::new();
        named
            .try_reserve_exact(FIXED_FILES)
            .map_err(|_| Unavailable)?;
        let mut roots = Vec::new();
        roots.try_reserve_exact(2).map_err(|_| Unavailable)?;
        let mut facts = Vec::new();
        facts.try_reserve_exact(MAX_PIDS).map_err(|_| Unavailable)?;
        let mut names = Vec::new();
        names.try_reserve_exact(MAX_PIDS).map_err(|_| Unavailable)?;
        let mut buffer = Zeroizing::new(Vec::new());
        buffer
            .try_reserve_exact(4 * 1024 * 1024 + 1)
            .map_err(|_| Unavailable)?;
        let mut link = Zeroizing::new(Vec::new());
        link.try_reserve_exact(4097).map_err(|_| Unavailable)?;
        Ok(Self {
            files,
            installed,
            named,
            roots,
            root: None,
            myself: None,
            manager: None,
            manager_pidfd: None,
            tool: None,
            systemd: None,
            observer_ns: [None; 4],
            fdinfo: None,
            mountinfo: None,
            net: None,
            unix: None,
            query: None,
            inventory_diagnostic: InventoryDiagnostic::default(),
            rows: Owner::reserve_before_ready(FIXED, NOFILE as usize)?,
            facts,
            names,
            buffer,
            link,
            directory_buffer: [MaybeUninit::uninit(); 8192],
            consumed: false,
            completed: false,
            refused: false,
        })
    }
    fn keep(&mut self, file: File) -> usize {
        let index = self.files.len();
        self.files.push(file);
        index
    }
    fn open(
        &mut self,
        parent: Option<usize>,
        name: &str,
        flags: OFlag,
        budget: &Budget,
    ) -> Result<usize> {
        budget.check()?;
        if self.files.len() >= FIXED_FILES {
            return Err(());
        } // charge BEFORE open
        let file = match parent {
            Some(index) => openat(
                &self.files[index],
                name,
                flags | OFlag::O_CLOEXEC,
                Mode::empty(),
            ),
            None => open(name, flags | OFlag::O_CLOEXEC, Mode::empty()),
        }
        .map(File::from)
        .map_err(|_| ())?;
        let index = self.keep(file); // positive reported original BEFORE postchecks
        budget.check()?;
        if let Some(parent) = parent {
            let metadata = self.files[index].metadata().map_err(|_| ())?;
            self.named.push(Named {
                parent,
                name: name.to_owned(),
                index,
                metadata,
                follow: !flags.contains(OFlag::O_NOFOLLOW),
                image: name == "exe",
            });
        }
        budget.check()?;
        Ok(index)
    }
    fn dir(&mut self, parent: Option<usize>, name: &str, budget: &Budget) -> Result<usize> {
        self.open(
            parent,
            name,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW,
            budget,
        )
    }
    fn bytes_file(
        &mut self,
        parent: usize,
        name: &str,
        cap: usize,
        budget: &mut Budget,
    ) -> Result<usize> {
        let index = self.open(
            Some(parent),
            name,
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW,
            budget,
        )?;
        read_original(&self.files[index], cap, &mut self.buffer, budget)?;
        Ok(index)
    }
    fn install(
        &mut self,
        parent: usize,
        name: &'static str,
        image: bool,
        budget: &Budget,
    ) -> Result<usize> {
        let flags = if image {
            OFlag::O_PATH | OFlag::O_NOFOLLOW
        } else {
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW
        };
        let index = self.open(Some(parent), name, flags, budget)?;
        let metadata = self.files[index].metadata().map_err(|_| ())?;
        if metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || (image
                && (!metadata.is_file() || metadata.mode() & 0o111 == 0 || metadata.nlink() == 0))
            || (!image && !metadata.is_dir())
        {
            return Err(());
        }
        self.installed.push(Installed {
            parent,
            name,
            index,
            metadata,
            image,
        });
        budget.check()?;
        Ok(index)
    }
    fn installed_current(&self, budget: &Budget) -> Result<()> {
        let root = self.files.first().ok_or(())?;
        let metadata = root.metadata().map_err(|_| ())?;
        budget.check()?;
        if metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || !identity(&metadata, &fs::symlink_metadata("/").map_err(|_| ())?)
        {
            return Err(());
        }
        for item in &self.installed {
            binding(
                &self.files[item.parent],
                item.name,
                &self.files[item.index],
                &item.metadata,
                false,
                item.image,
                budget,
            )?;
        }
        Ok(())
    }
    fn snapshot(&mut self, pid: u32, budget: &mut Budget) -> Result<Snapshot> {
        let directory = self.dir(self.root, &pid.to_string(), budget)?;
        let metadata = self.files[directory].metadata().map_err(|_| ())?;
        let stat = self.bytes_file(directory, "stat", MAX_STATUS, budget)?;
        let start = start_time(&self.buffer, pid)?;
        let status_file = self.bytes_file(directory, "status", MAX_STATUS, budget)?;
        let identity = status(&self.buffer, pid)?;
        let command = self.bytes_file(directory, "cmdline", MAX_COMMAND, budget)?;
        arguments(&self.buffer)?;
        let command_bytes = Zeroizing::new(self.buffer.to_vec());
        let comm = self.bytes_file(directory, "comm", 4096, budget)?;
        if self.buffer.is_empty() || !self.buffer.ends_with(b"\n") {
            return Err(());
        }
        let comm_bytes = Zeroizing::new(self.buffer.to_vec());
        let image = self.open(Some(directory), "exe", OFlag::O_PATH, budget)?;
        let image_metadata = self.files[image].metadata().map_err(|_| ())?;
        if !image_metadata.is_file() || image_metadata.nlink() == 0 {
            return Err(());
        }
        link_original(&self.files[directory], &mut self.link, budget)?;
        let image_name = Zeroizing::new(self.link.to_vec());
        let namespaces = [
            self.open(Some(directory), "ns/pid", OFlag::O_PATH, budget)?,
            self.open(Some(directory), "ns/user", OFlag::O_PATH, budget)?,
        ];
        Ok(Snapshot {
            pid,
            directory,
            stat,
            status: status_file,
            command,
            comm,
            image,
            namespaces,
            metadata,
            image_metadata,
            start,
            identity,
            command_bytes,
            comm_bytes,
            image_name,
        })
    }
    fn snapshot_current(&mut self, manager: bool, budget: &mut Budget) -> Result<()> {
        let snapshot = if manager {
            self.manager.as_ref()
        } else {
            self.myself.as_ref()
        }
        .ok_or(())?;
        let root = self.root.ok_or(())?;
        binding(
            &self.files[root],
            &snapshot.pid.to_string(),
            &self.files[snapshot.directory],
            &snapshot.metadata,
            false,
            false,
            budget,
        )?;
        for (name, index, cap) in [
            ("stat", snapshot.stat, MAX_STATUS),
            ("status", snapshot.status, MAX_STATUS),
            ("cmdline", snapshot.command, MAX_COMMAND),
            ("comm", snapshot.comm, 4096),
        ] {
            let metadata = self.files[index].metadata().map_err(|_| ())?;
            binding(
                &self.files[snapshot.directory],
                name,
                &self.files[index],
                &metadata,
                false,
                false,
                budget,
            )?;
            read_original(&self.files[index], cap, &mut self.buffer, budget)?;
            let matches = match name {
                "stat" => start_time(&self.buffer, snapshot.pid)? == snapshot.start,
                "status" => status(&self.buffer, snapshot.pid)? == snapshot.identity,
                "cmdline" => self.buffer == snapshot.command_bytes,
                "comm" => self.buffer == snapshot.comm_bytes,
                _ => false,
            };
            if !matches {
                return Err(());
            }
        }
        binding(
            &self.files[snapshot.directory],
            "exe",
            &self.files[snapshot.image],
            &snapshot.image_metadata,
            true,
            true,
            budget,
        )?;
        link_original(&self.files[snapshot.directory], &mut self.link, budget)?;
        if self.link != snapshot.image_name {
            return Err(());
        }
        for (name, index) in ["ns/pid", "ns/user"].into_iter().zip(snapshot.namespaces) {
            let initial = self.files[index].metadata().map_err(|_| ())?;
            binding(
                &self.files[snapshot.directory],
                name,
                &self.files[index],
                &initial,
                true,
                false,
                budget,
            )?;
        }
        if manager {
            live(self.manager_pidfd.as_ref().ok_or(())?, budget)?;
        }
        budget.check()
    }
    fn boundaries(&mut self, budget: &mut Budget) -> Result<()> {
        self.installed_current(budget)?;
        for (index, name, metadata) in &self.roots {
            budget.check()?;
            if !identity(metadata, &self.files[*index].metadata().map_err(|_| ())?)
                || !identity(metadata, &fs::symlink_metadata(name).map_err(|_| ())?)
            {
                return Err(());
            }
        }
        for original in &self.named {
            binding(
                &self.files[original.parent],
                &original.name,
                &self.files[original.index],
                &original.metadata,
                original.follow,
                original.image,
                budget,
            )?;
        }
        let root = self.root.ok_or(())?;
        let original = self.files[root].metadata().map_err(|_| ())?;
        budget.check()?;
        if !identity(&original, &fs::symlink_metadata("/proc").map_err(|_| ())?)
            || fstatfs(&self.files[root])
                .map_err(|_| ())?
                .filesystem_type()
                != PROC_SUPER_MAGIC
        {
            return Err(());
        }
        self.snapshot_current(false, budget)?;
        self.snapshot_current(true, budget)?;
        let myself = self.myself.as_ref().ok_or(())?;
        let manager = self.manager.as_ref().ok_or(())?;
        for (name, slot) in ["ns/pid", "ns/user", "ns/mnt", "ns/net"]
            .into_iter()
            .zip(self.observer_ns)
        {
            let index = slot.ok_or(())?;
            let initial = self.files[index].metadata().map_err(|_| ())?;
            binding(
                &self.files[myself.directory],
                name,
                &self.files[index],
                &initial,
                true,
                false,
                budget,
            )?;
        }
        for (left, right) in myself.namespaces.into_iter().zip(manager.namespaces) {
            same_namespace(&self.files[left], &self.files[right])?;
        }
        let fdinfo = self.fdinfo.ok_or(())?;
        let index = self.open(
            Some(fdinfo),
            &self.files[root].as_raw_fd().to_string(),
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW,
            budget,
        )?;
        // Fixed visibility observations occur only twice: these originals do
        // not recycle on error, and are counted within34 fixed slots.
        read_original(&self.files[index], MAX_STATUS, &mut self.buffer, budget)?;
        let id = mount_id(&self.buffer)?;
        read_original(
            &self.files[self.mountinfo.ok_or(())?],
            MAX_STATUS,
            &mut self.buffer,
            budget,
        )?;
        visible_proc_mount(&self.buffer, id)?;
        budget.check()
    }
    fn query(&mut self, unit: &'static str, system: bool, budget: &mut Budget) -> Result<u32> {
        if self.query.is_some() {
            return Err(());
        }
        let mut progress = QueryProgress::default();
        progress.emit(QueryPhase::Before, |label| phase(label, budget))?;
        self.installed_current(budget)?;
        let mut output = Zeroizing::new(Vec::new());
        output.try_reserve_exact(MAX_STATUS + 1).map_err(|_| ())?;
        budget.check()?;
        let mut command = Command::new(format!(
            "/proc/self/fd/{}",
            self.files[self.tool.ok_or(())?].as_raw_fd()
        ));
        command
            .arg0("/usr/bin/systemctl")
            .env_clear()
            .env("LC_ALL", "C")
            .env("SYSTEMD_COLORS", "0")
            .env("XDG_RUNTIME_DIR", "/run/user/1000")
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus")
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
            .stderr(Stdio::null());
        if let Some((uid, gid)) = query_credentials(system) {
            // Fixed user-manager children only. The actor cleared and verified
            // its own supplementary groups before READY, so std's ignored
            // setgroups EPERM cannot preserve an inherited nonempty group set.
            command.gid(gid).uid(uid);
        }
        progress.emit(QueryPhase::BeforeSpawn, |label| phase(label, budget))?;
        let child = command.spawn().map_err(|_| ())?;
        self.query = Some(Query {
            child,
            stdout: None,
            pidfd: None,
            output,
            eof: false,
            positive: false,
        });
        let query = self.query.as_mut().ok_or(())?;
        query.stdout = query.child.stdout.take(); // immediate transfer, BEFORE gate
        budget.check()?;
        let pid = Pid::from_raw(i32::try_from(query.child.id()).map_err(|_| ())?);
        query.pidfd = Some(
            pidfd_open(
                RustPid::from_raw(pid.as_raw()).ok_or(())?,
                PidfdFlags::NONBLOCK,
            )
            .map_err(|_| ())?,
        );
        budget.check()?;
        fcntl(
            query.stdout.as_ref().ok_or(())?,
            FcntlArg::F_SETFL(OFlag::O_NONBLOCK),
        )
        .map_err(|_| ())?;
        budget.check()?;
        progress.emit(QueryPhase::Spawned, |label| phase(label, budget))?;
        loop {
            budget.check()?;
            if !query.eof {
                let mut chunk = [0; 4096];
                let left = (MAX_STATUS + 1).checked_sub(query.output.len()).ok_or(())?;
                if left == 0 {
                    return Err(());
                }
                match query
                    .stdout
                    .as_mut()
                    .ok_or(())?
                    .read(&mut chunk[..left.min(4096)])
                {
                    Ok(0) => {
                        query.eof = true;
                        progress.emit(QueryPhase::Eof, |label| phase(label, budget))?;
                    }
                    Ok(size) => {
                        query.output.extend_from_slice(&chunk[..size]);
                        budget.charge(size)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => return Err(()),
                }
                if query.output.len() > MAX_STATUS {
                    return Err(());
                }
            }
            budget.check()?;
            let status = waitid(
                Id::Pid(pid),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
            )
            .map_err(|_| ())?;
            budget.check()?;
            if positive_query(query.eof, status, pid)? {
                progress.emit(QueryPhase::OriginalZero, |label| phase(label, budget))?;
                // No concurrent reaper; exact original positive WNOWAIT first.
                if query.child.wait().map_err(|_| ())?.code() != Some(0) {
                    return Err(());
                }
                budget.check()?;
                query.positive = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let value = service_record(&self.query.as_ref().ok_or(())?.output, system)?;
        progress.emit(QueryPhase::Parsed, |label| phase(label, budget))?;
        self.installed_current(budget)?;
        budget.check()?;
        progress.emit(QueryPhase::Completed, |label| phase(label, budget))?;
        // Only fully parsed, EOF/original0 query originals release/recycle.
        if !self.query.as_ref().ok_or(())?.positive {
            return Err(());
        }
        self.query = None;
        Ok(value)
    }
    fn units(&mut self, original: u32, budget: &mut Budget) -> Result<()> {
        if self.query("user@1000.service", true, budget)? != original {
            return Err(());
        }
        for unit in ["omavless.service", "omavless-runtime.service"] {
            self.query(unit, false, budget)?;
        }
        Ok(())
    }
    fn catalogue(&mut self, budget: &Budget) -> Result<()> {
        self.names.clear();
        let root = &self.files[self.root.ok_or(())?];
        budget.check()?;
        let position = self.inventory_diagnostic.cut(
            InventoryFailure::CatalogueSeek,
            seek(root, SeekFrom::Start(0)).map_err(|_| ()),
            budget,
        )?;
        self.inventory_diagnostic.cut(
            InventoryFailure::CatalogueSeek,
            if position == 0 { Ok(()) } else { Err(()) },
            budget,
        )?;
        budget.check()?;
        let mut entries = RawDir::new(root, &mut self.directory_buffer);
        for _ in 0..DIRECTORY_ENTRIES {
            budget.check()?;
            let next = entries.next();
            budget.check()?;
            let Some(entry) = next else {
                self.names.sort_unstable();
                if self.names.is_empty() || self.names.windows(2).any(|p| p[0] == p[1]) {
                    return self.inventory_diagnostic.cut(
                        InventoryFailure::CatalogueSet,
                        Err(()),
                        budget,
                    );
                }
                return budget.check();
            };
            let entry = self.inventory_diagnostic.cut(
                InventoryFailure::CatalogueNext,
                entry.map_err(|_| ()),
                budget,
            )?;
            let name = entry.file_name().to_bytes();
            if name.iter().all(u8::is_ascii_digit) {
                let pid = self.inventory_diagnostic.cut(
                    InventoryFailure::CatalogueName,
                    decimal(name),
                    budget,
                )?;
                self.inventory_diagnostic.cut(
                    InventoryFailure::CatalogueName,
                    if pid == 0 || name != pid.to_string().as_bytes() {
                        Err(())
                    } else {
                        Ok(())
                    },
                    budget,
                )?;
                self.inventory_diagnostic.cut(
                    InventoryFailure::CatalogueLimit,
                    if self.names.len() >= MAX_PIDS {
                        Err(())
                    } else {
                        Ok(())
                    },
                    budget,
                )?;
                self.names.push(pid);
            }
        }
        // No unbounded next, materialization or silently truncated pass.
        self.inventory_diagnostic
            .cut(InventoryFailure::CatalogueLimit, Err(()), budget)
    }
    fn row_text<R>(
        &mut self,
        role: TextRole,
        cap: usize,
        budget: &mut Budget,
        parse: impl FnOnce(&[u8]) -> Result<R>,
    ) -> Result<R> {
        let until = budget.until;
        let name = role.name();
        let initial = (|| {
            available_to_unit(self.rows.scratch_acquire(
                0,
                || available(budget.check()),
                |directory| {
                    available(
                        openat(
                            directory,
                            name,
                            OFlag::O_RDONLY
                                | OFlag::O_NONBLOCK
                                | OFlag::O_NOFOLLOW
                                | OFlag::O_CLOEXEC,
                            Mode::empty(),
                        )
                        .map(File::from)
                        .map_err(|_| ()),
                    )
                },
            ))?;
            let metadata = self
                .rows
                .scratch_original(0)
                .map_err(|_| ())?
                .metadata()
                .map_err(|_| ())?;
            binding(
                self.rows.row_originals().map_err(|_| ())?.0,
                name,
                self.rows.scratch_original(0).map_err(|_| ())?,
                &metadata,
                false,
                false,
                budget,
            )?;
            Ok(metadata)
        })();
        let metadata = self
            .inventory_diagnostic
            .cut(role.read_failure(), initial, budget)?;
        let mut result = None;
        let buffer = &mut self.buffer;
        let diagnostic = &mut self.inventory_diagnostic;
        let performed = available_to_unit(self.rows.scratch_perform(
            0,
            || time_gate(until),
            |file| {
                available(diagnostic.cut(
                    role.read_failure(),
                    read_original(file, cap, buffer, budget),
                    budget,
                ))?;
                result = Some(available(diagnostic.cut(
                    role.parse_failure(),
                    parse(buffer),
                    budget,
                ))?);
                Ok(())
            },
        ));
        self.inventory_diagnostic
            .cut(role.read_failure(), performed, budget)?;
        let completed = (|| {
            binding(
                self.rows.row_originals().map_err(|_| ())?.0,
                name,
                self.rows.scratch_original(0).map_err(|_| ())?,
                &metadata,
                false,
                false,
                budget,
            )?;
            available_to_unit(self.rows.scratch_release(0, || time_gate(until)))?;
            result.ok_or(())
        })();
        self.inventory_diagnostic
            .cut(role.read_failure(), completed, budget)
    }
    fn acquire_row(&mut self, pid: u32, budget: &mut Budget) -> Result<()> {
        let root = &self.files[self.root.ok_or(())?];
        self.inventory_diagnostic.cut(
            InventoryFailure::RowDirectory,
            available_to_unit(self.rows.directory(
                pid,
                || available(budget.check()),
                || available(directory(root, &pid.to_string())),
            )),
            budget,
        )?;
        let identity = self.row_text(TextRole::Status, MAX_STATUS, budget, |bytes| {
            status(bytes, pid)
        })?;
        let start = self.row_text(TextRole::Stat, MAX_STATUS, budget, |bytes| {
            start_time(bytes, pid)
        })?;
        let metadata = self
            .rows
            .row_originals()
            .map_err(|_| ())?
            .0
            .metadata()
            .map_err(|_| ());
        let metadata =
            self.inventory_diagnostic
                .cut(InventoryFailure::RowDirectory, metadata, budget)?;
        let class = if identity.uids.contains(&UID) {
            Class::SameUid
        } else {
            Class::OtherUid
        };
        self.facts.push(Facts {
            metadata,
            start,
            identity,
            image_metadata: None,
            command: Zeroizing::new(Vec::new()),
            comm: Zeroizing::new(Vec::new()),
            image_name: Zeroizing::new(Vec::new()),
        });
        self.inventory_diagnostic.cut(
            InventoryFailure::Classify,
            available_to_unit(
                self.rows
                    .classify(|| available(budget.check()), |_| Ok(class)),
            ),
            budget,
        )?;
        if class == Class::SameUid {
            let mut original_error = None;
            let acquired = self.rows.executable(
                || available(budget.check()),
                |directory| record_image_open(canonical_image_file(directory), &mut original_error),
            );
            // Owner already revoked on any error; a positive File is installed
            // before its unchanged postgate. Gate/charge refusal stays generic.
            self.inventory_diagnostic.cut(
                image_open_failure(original_error),
                available_to_unit(acquired),
                budget,
            )?;
            let image_metadata = self
                .rows
                .row_originals()
                .map_err(|_| ())?
                .1
                .ok_or(())?
                .metadata()
                .map_err(|_| ());
            self.facts.last_mut().ok_or(())?.image_metadata = Some(self.inventory_diagnostic.cut(
                InventoryFailure::ImageShape,
                image_metadata,
                budget,
            )?);
            if !self
                .facts
                .last()
                .ok_or(())?
                .image_metadata
                .as_ref()
                .ok_or(())?
                .is_file()
            {
                return self.inventory_diagnostic.cut(
                    InventoryFailure::ImageShape,
                    Err(()),
                    budget,
                );
            }
            let command = self.row_text(TextRole::Command, MAX_COMMAND, budget, |bytes| {
                arguments(bytes)?;
                Ok(Zeroizing::new(bytes.to_vec()))
            })?;
            self.facts.last_mut().ok_or(())?.command = command;
            let comm = self.row_text(TextRole::Comm, 4096, budget, |bytes| {
                if bytes.is_empty() || !bytes.ends_with(b"\n") {
                    return Err(());
                }
                Ok(Zeroizing::new(bytes.to_vec()))
            })?;
            self.facts.last_mut().ok_or(())?.comm = comm;
            let linked = link_original(
                self.rows.row_originals().map_err(|_| ())?.0,
                &mut self.link,
                budget,
            );
            self.inventory_diagnostic
                .cut(InventoryFailure::LinkRead, linked, budget)?;
            let facts = self.facts.last_mut().ok_or(())?;
            facts.image_name = Zeroizing::new(self.link.to_vec());
            let candidate = daemon_candidate(&facts.command, &facts.image_name, &facts.comm);
            let candidate =
                self.inventory_diagnostic
                    .cut(InventoryFailure::Daemon, candidate, budget)?;
            self.inventory_diagnostic.cut(
                InventoryFailure::Daemon,
                if candidate { Err(()) } else { Ok(()) },
                budget,
            )?;
        }
        let current = self.row_current(pid, self.facts.len() - 1, budget);
        self.inventory_diagnostic
            .cut(InventoryFailure::RowCurrent, current, budget)?;
        self.inventory_diagnostic.cut(
            InventoryFailure::RowComplete,
            available_to_unit(
                self.rows
                    .complete_row(|| available(budget.check()), |_, _, _| Ok(())),
            ),
            budget,
        )
    }
    fn row_current(&mut self, pid: u32, index: usize, budget: &mut Budget) -> Result<()> {
        let identity = self.row_text(TextRole::Status, MAX_STATUS, budget, |bytes| {
            status(bytes, pid)
        })?;
        let start = self.row_text(TextRole::Stat, MAX_STATUS, budget, |bytes| {
            start_time(bytes, pid)
        })?;
        let facts = self.facts.get(index).ok_or(())?;
        if identity != facts.identity || start != facts.start {
            return Err(());
        }
        let (directory, image) = self.rows.row_originals().map_err(|_| ())?;
        binding(
            &self.files[self.root.ok_or(())?],
            &pid.to_string(),
            directory,
            &facts.metadata,
            false,
            false,
            budget,
        )?;
        if let Some(image) = image {
            binding(
                directory,
                "exe",
                image,
                facts.image_metadata.as_ref().ok_or(())?,
                true,
                true,
                budget,
            )?;
            link_original(directory, &mut self.link, budget)?;
            if self.link != facts.image_name {
                return Err(());
            }
            let command = self.row_text(TextRole::Command, MAX_COMMAND, budget, |bytes| {
                Ok(Zeroizing::new(bytes.to_vec()))
            })?;
            let comm = self.row_text(TextRole::Comm, 4096, budget, |bytes| {
                Ok(Zeroizing::new(bytes.to_vec()))
            })?;
            let facts = self.facts.get(index).ok_or(())?;
            if command != facts.command || comm != facts.comm {
                return Err(());
            }
        }
        budget.check()
    }
    fn observe_inner(&mut self, until: Instant) -> Result<()> {
        let mut budget = Budget::new();
        budget.until = budget.until.min(until);
        phase(b"t4_actor_before_canonical_installation\n", &budget)?;
        let root = self.dir(None, "/", &budget)?;
        let root_metadata = self.files[root].metadata().map_err(|_| ())?;
        if root_metadata.uid() != 0 || root_metadata.mode() & 0o022 != 0 {
            return Err(());
        }
        self.roots.push((root, "/", root_metadata));
        let usr = self.install(root, "usr", false, &budget)?;
        let bin = self.install(usr, "bin", false, &budget)?;
        let lib = self.install(usr, "lib", false, &budget)?;
        let systemd_dir = self.install(lib, "systemd", false, &budget)?;
        self.tool = Some(self.install(bin, "systemctl", true, &budget)?);
        self.systemd = Some(self.install(systemd_dir, "systemd", true, &budget)?);
        phase(b"t4_actor_before_canonical_observer\n", &budget)?;
        self.root = Some(self.dir(None, "/proc", &budget)?);
        let proc_root = self.root.ok_or(())?;
        self.roots.push((
            proc_root,
            "/proc",
            self.files[proc_root].metadata().map_err(|_| ())?,
        ));
        if fstatfs(&self.files[proc_root])
            .map_err(|_| ())?
            .filesystem_type()
            != PROC_SUPER_MAGIC
        {
            return Err(());
        }
        self.myself = Some(self.snapshot(std::process::id(), &mut budget)?);
        let myself = self.myself.as_ref().ok_or(())?;
        let args = arguments(&myself.command_bytes)?;
        if myself.identity.uids != [0; 4]
            || myself.identity.namespace_pids != [myself.pid]
            || args.len() != 2
            || args[1] != b"--actor-canonical"
        {
            return Err(());
        }
        let directory = myself.directory;
        for (slot, name) in ["ns/pid", "ns/user", "ns/mnt", "ns/net"]
            .into_iter()
            .enumerate()
        {
            self.observer_ns[slot] =
                Some(self.open(Some(directory), name, OFlag::O_PATH, &budget)?);
        }
        self.fdinfo = Some(self.dir(Some(directory), "fdinfo", &budget)?);
        self.mountinfo = Some(self.bytes_file(directory, "mountinfo", MAX_STATUS, &mut budget)?);
        self.net = Some(self.dir(Some(directory), "net", &budget)?);
        self.unix =
            Some(self.bytes_file(self.net.ok_or(())?, "unix", 4 * 1024 * 1024, &mut budget)?);
        let pid = self.query("user@1000.service", true, &mut budget)?;
        phase(b"t4_actor_before_canonical_manager\n", &budget)?;
        self.manager = Some(self.snapshot(pid, &mut budget)?);
        let manager = self.manager.as_ref().ok_or(())?;
        if manager.identity.uids != [UID; 4]
            || manager.identity.namespace_pids != [pid]
            || !arguments(&manager.command_bytes)?.contains(&b"--user".as_slice())
            || !executable_identity(
                &manager.image_metadata,
                &self.files[self.systemd.ok_or(())?]
                    .metadata()
                    .map_err(|_| ())?,
            )
        {
            return Err(());
        }
        budget.check()?;
        self.manager_pidfd = Some(
            pidfd_open(
                RustPid::from_raw(i32::try_from(pid).map_err(|_| ())?).ok_or(())?,
                PidfdFlags::NONBLOCK,
            )
            .map_err(|_| ())?,
        );
        budget.check()?;
        self.boundaries(&mut budget)?;
        phase(b"t4_actor_before_canonical_initial_units\n", &budget)?;
        self.units(pid, &mut budget)?; // fully EOF/original0 before catalogue
        phase(b"t4_actor_before_canonical_inventory\n", &budget)?;
        self.catalogue(&budget)?;
        self.inventory_diagnostic.cut(
            InventoryFailure::OwnerAdmit,
            available_to_unit(self.rows.admit_catalogue(&self.names)),
            &budget,
        )?;
        let count = self.names.len();
        for index in 0..count {
            self.acquire_row(self.names[index], &mut budget)?;
        }
        self.catalogue(&budget)?;
        self.inventory_diagnostic.cut(
            InventoryFailure::SweepSet,
            available_to_unit(self.rows.begin_sweep(&self.names)),
            &budget,
        )?;
        // Final queries complete BEFORE final originals and final PID set.
        phase(b"t4_actor_before_canonical_final_units\n", &budget)?;
        self.units(pid, &mut budget)?;
        phase(b"t4_actor_before_canonical_final_boundaries\n", &budget)?;
        self.boundaries(&mut budget)?;
        read_original(
            &self.files[self.unix.ok_or(())?],
            4 * 1024 * 1024,
            &mut self.buffer,
            &mut budget,
        )?;
        no_listener(&self.buffer, &LISTENERS.map(str::to_owned))?;
        phase(b"t4_actor_before_canonical_final_sweep\n", &budget)?;
        for index in 0..count {
            let pid = self.names[index];
            let current = self.row_current(pid, index, &mut budget);
            self.inventory_diagnostic
                .cut(InventoryFailure::RowCurrent, current, &budget)?;
            self.inventory_diagnostic.cut(
                InventoryFailure::RowComplete,
                available_to_unit(self.rows.recheck_row(
                    pid,
                    || available(budget.check()),
                    |_, _, _| Ok(()),
                )),
                &budget,
            )?;
        }
        self.catalogue(&budget)?;
        self.inventory_diagnostic.cut(
            InventoryFailure::SweepSet,
            available_to_unit(
                self.rows
                    .complete_inventory(&self.names, || available(budget.check())),
            ),
            &budget,
        )?;
        phase(b"t4_actor_canonical_inventory_completed\n", &budget)?;
        budget.check()
    }
    pub(crate) fn observe(&mut self, until: Instant) -> std::result::Result<(), Unavailable> {
        if self.refused || self.consumed {
            self.refused = true;
            return Err(Unavailable);
        }
        self.consumed = true;
        let result = available(self.observe_inner(until));
        if result.is_err() {
            self.refused = true;
            let _ = self.rows.refuse();
        } else {
            self.completed = true;
        }
        result
    }
    pub(crate) fn finish(&mut self) -> std::result::Result<(), Unavailable> {
        if self.refused || !self.completed {
            self.refused = true;
            return Err(Unavailable);
        }
        self.rows.finish()?;
        self.files.clear();
        self.manager_pidfd = None;
        self.completed = false;
        self.refused = true;
        Ok(())
    }
}
fn available_to_unit<T>(value: std::result::Result<T, Unavailable>) -> Result<T> {
    value.map_err(|_| ())
}
fn time_gate(until: Instant) -> std::result::Result<(), Unavailable> {
    if Instant::now() < until {
        Ok(())
    } else {
        Err(Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const INVENTORY_FAILURES: [InventoryFailure; 29] = [
        InventoryFailure::CatalogueSeek,
        InventoryFailure::CatalogueNext,
        InventoryFailure::CatalogueName,
        InventoryFailure::CatalogueLimit,
        InventoryFailure::CatalogueSet,
        InventoryFailure::OwnerAdmit,
        InventoryFailure::RowDirectory,
        InventoryFailure::StatusRead,
        InventoryFailure::StatusParse,
        InventoryFailure::StatRead,
        InventoryFailure::StatParse,
        InventoryFailure::Classify,
        InventoryFailure::ImageOpen,
        InventoryFailure::ImageOpenErrno(ImageOpenErrno::NoEntry),
        InventoryFailure::ImageOpenErrno(ImageOpenErrno::Access),
        InventoryFailure::ImageOpenErrno(ImageOpenErrno::Permission),
        InventoryFailure::ImageOpenErrno(ImageOpenErrno::ProcessLimit),
        InventoryFailure::ImageOpenErrno(ImageOpenErrno::SystemLimit),
        InventoryFailure::ImageOpenErrno(ImageOpenErrno::Other),
        InventoryFailure::ImageShape,
        InventoryFailure::CommandRead,
        InventoryFailure::CommandParse,
        InventoryFailure::CommRead,
        InventoryFailure::CommParse,
        InventoryFailure::LinkRead,
        InventoryFailure::Daemon,
        InventoryFailure::RowCurrent,
        InventoryFailure::RowComplete,
        InventoryFailure::SweepSet,
    ];
    #[test]
    fn image_open_errno_classifies_only_the_original_finite_result() {
        use nix::errno::Errno;
        for (error, expected, label) in [
            (
                Errno::ENOENT,
                ImageOpenErrno::NoEntry,
                b"t4_actor_inventory_image_open_enoent_refused\n".as_slice(),
            ),
            (
                Errno::EACCES,
                ImageOpenErrno::Access,
                b"t4_actor_inventory_image_open_eacces_refused\n",
            ),
            (
                Errno::EPERM,
                ImageOpenErrno::Permission,
                b"t4_actor_inventory_image_open_eperm_refused\n",
            ),
            (
                Errno::EMFILE,
                ImageOpenErrno::ProcessLimit,
                b"t4_actor_inventory_image_open_emfile_refused\n",
            ),
            (
                Errno::ENFILE,
                ImageOpenErrno::SystemLimit,
                b"t4_actor_inventory_image_open_enfile_refused\n",
            ),
            (
                Errno::EINTR,
                ImageOpenErrno::Other,
                b"t4_actor_inventory_image_open_other_refused\n",
            ),
            (
                Errno::EIO,
                ImageOpenErrno::Other,
                b"t4_actor_inventory_image_open_other_refused\n",
            ),
            (
                Errno::ELOOP,
                ImageOpenErrno::Other,
                b"t4_actor_inventory_image_open_other_refused\n",
            ),
        ] {
            let mut reported = None;
            assert!(record_image_open::<usize>(Err(error), &mut reported).is_err());
            assert_eq!(reported, Some(expected));
            assert_eq!(image_open_failure(reported).label(), label);
        }
        let mut reported = None;
        assert_eq!(record_image_open(Ok(42), &mut reported), Ok(42));
        assert_eq!(reported, None);
        assert_eq!(
            image_open_failure(None).label(),
            InventoryFailure::ImageOpen.label()
        );
    }
    #[test]
    fn image_open_errno_owner_revokes_before_one_diagnostic_with_no_followup() {
        use std::cell::Cell;
        use std::rc::Rc;
        struct Handle(Rc<Cell<usize>>);
        impl Drop for Handle {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve_before_ready(FIXED, NOFILE as usize).unwrap();
        owner.admit_catalogue(&[2]).unwrap();
        owner
            .directory(2, || Ok(()), || Ok(Handle(drops.clone())))
            .unwrap();
        owner.classify(|| Ok(()), |_| Ok(Class::SameUid)).unwrap();
        let mut reported = None;
        let mut opens = 0;
        let original = owner.executable(
            || Ok(()),
            |_| {
                opens += 1;
                record_image_open(Err(nix::errno::Errno::ENOENT), &mut reported)
            },
        );
        assert_eq!(opens, 1);
        let mut diagnostic = InventoryDiagnostic::default();
        let mut emits = 0;
        assert!(
            diagnostic
                .result(
                    image_open_failure(reported),
                    available_to_unit(original),
                    |_| {
                        emits += 1;
                        assert!(
                            owner
                                .directory(2, || panic!("revoked gate"), || panic!("revoked open"))
                                .is_err()
                        );
                        assert!(
                            owner
                                .executable(
                                    || panic!("revoked image gate"),
                                    |_| panic!("retry image")
                                )
                                .is_err()
                        );
                        assert!(owner.finish().is_err());
                        assert_eq!(drops.get(), 0);
                        Err(())
                    }
                )
                .is_err()
        );
        assert_eq!(emits, 1);
        assert!(
            diagnostic
                .result::<()>(image_open_failure(reported), Err(()), |_| panic!(
                    "secondary label"
                ))
                .is_err()
        );
        assert_eq!(drops.get(), 0); // Live memory owner only; no fatal custody.
    }
    #[test]
    fn image_open_positive_then_late_keeps_original_and_has_no_errno_label() {
        use std::cell::Cell;
        use std::rc::Rc;
        struct Handle(Rc<Cell<usize>>);
        impl Drop for Handle {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve_before_ready(FIXED, NOFILE as usize).unwrap();
        owner.admit_catalogue(&[2]).unwrap();
        owner
            .directory(2, || Ok(()), || Ok(Handle(drops.clone())))
            .unwrap();
        owner.classify(|| Ok(()), |_| Ok(Class::SameUid)).unwrap();
        let mut reported = None;
        let mut gates = 0;
        let mut opens = 0;
        let original = owner.executable(
            || {
                gates += 1;
                if gates == 1 { Ok(()) } else { Err(Unavailable) }
            },
            |_| {
                opens += 1;
                record_image_open(Ok(Handle(drops.clone())), &mut reported)
            },
        );
        assert!(original.is_err());
        assert_eq!((gates, opens), (2, 1));
        assert_eq!(reported, None);
        assert_eq!(
            image_open_failure(reported).label(),
            InventoryFailure::ImageOpen.label()
        );
        assert_eq!(drops.get(), 0);
        assert!(
            owner
                .executable(|| panic!("post-late gate"), |_| panic!("retry"))
                .is_err()
        );
        assert!(owner.finish().is_err());
        assert_eq!(drops.get(), 0);
    }
    #[test]
    fn image_open_before_callback_refusal_stays_generic_and_never_opens() {
        let mut owner = Owner::reserve_before_ready(FIXED, NOFILE as usize).unwrap();
        owner.admit_catalogue(&[2]).unwrap();
        owner.directory(2, || Ok(()), || Ok(42)).unwrap();
        owner.classify(|| Ok(()), |_| Ok(Class::SameUid)).unwrap();
        let mut reported = None;
        let original = owner.executable(
            || Err(Unavailable),
            |_| {
                reported = Some(ImageOpenErrno::NoEntry);
                panic!("callback after pre-gate refusal")
            },
        );
        assert!(original.is_err());
        assert_eq!(reported, None);
        assert_eq!(
            image_open_failure(reported).label(),
            InventoryFailure::ImageOpen.label()
        );
        assert!(owner.finish().is_err());
    }
    #[test]
    fn inventory_failure_is_one_attempt_preserving_the_original_result() {
        for cut in INVENTORY_FAILURES {
            let mut diagnostic = InventoryDiagnostic::default();
            assert_eq!(
                diagnostic.result(cut, Ok(42), |_| panic!("success has no diagnostic")),
                Ok(42)
            );
            assert!(!diagnostic.attempted);
            let mut labels = Vec::new();
            let original: Result<usize> = Err(());
            assert_eq!(
                diagnostic.result(cut, original, |label| {
                    labels.push(label);
                    Err(())
                }),
                original
            );
            assert_eq!(labels, [cut.label()]);
            assert!(diagnostic.attempted);
            for later in INVENTORY_FAILURES {
                assert_eq!(
                    diagnostic.result::<()>(later, Err(()), |_| panic!("secondary diagnostic")),
                    Err(())
                );
            }
        }
    }
    #[test]
    fn inventory_failure_vocabulary_and_whole_bound_are_finite() {
        let labels: Vec<_> = INVENTORY_FAILURES
            .into_iter()
            .map(InventoryFailure::label)
            .collect();
        for (index, label) in labels.iter().enumerate() {
            assert!(label.starts_with(b"t4_actor_inventory_"));
            assert!(label.ends_with(b"\n"));
            assert!(!labels[..index].contains(label));
        }
        assert_eq!(labels.len(), 29);
        assert_eq!(17 + labels.len(), 46);
        assert_eq!(9 + 7 * QUERY_PHASES.len() + 1 + 1, 60);
        let longest = labels
            .iter()
            .map(|label| label.len())
            .chain(QUERY_PHASES.into_iter().map(|step| step.label().len()))
            .chain([b"t4_actor_before_canonical_final_boundaries\n".len()])
            .max()
            .unwrap();
        assert!(longest * 60 <= 4096);
    }
    #[test]
    fn same_inventory_prefix_has_distinct_possible_refusals_not_a_cause() {
        let mut prefix = vec![
            b"t4_actor_before_canonical_installation\n".as_slice(),
            b"t4_actor_before_canonical_observer\n",
        ];
        prefix.extend(QUERY_PHASES.into_iter().map(QueryPhase::label));
        prefix.extend([
            b"t4_actor_before_canonical_manager\n".as_slice(),
            b"t4_actor_before_canonical_initial_units\n",
        ]);
        for _ in 0..3 {
            prefix.extend(QUERY_PHASES.into_iter().map(QueryPhase::label));
        }
        prefix.push(b"t4_actor_before_canonical_inventory\n");
        assert_eq!(prefix.len(), 33);
        for cut in [
            InventoryFailure::CatalogueNext,
            InventoryFailure::StatParse,
            InventoryFailure::Daemon,
        ] {
            let mut frames = prefix.clone();
            let mut diagnostic = InventoryDiagnostic::default();
            assert!(
                diagnostic
                    .result::<()>(cut, Err(()), |label| {
                        frames.push(label);
                        Ok(())
                    })
                    .is_err()
            );
            assert_eq!(&frames[..33], prefix.as_slice());
            assert_eq!(frames.len(), 34);
            assert_eq!(frames[33], cut.label());
        }
    }
    #[test]
    fn expired_inventory_diagnostic_preserves_error_without_output_or_retry() {
        let mut diagnostic = InventoryDiagnostic::default();
        let mut budget = Budget::new();
        budget.until = Instant::now() - Duration::from_secs(1);
        let mut writes = 0;
        assert!(
            diagnostic
                .result::<()>(InventoryFailure::StatRead, Err(()), |_| {
                    budget.check()?;
                    writes += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(writes, 0);
        assert!(
            diagnostic
                .result::<()>(InventoryFailure::RowCurrent, Err(()), |_| panic!("retry"))
                .is_err()
        );
    }
    #[test]
    fn parse_refusal_retains_row_and_scratch_and_forbids_downstream_io() {
        use std::cell::Cell;
        use std::rc::Rc;
        struct Handle(Rc<Cell<usize>>);
        impl Drop for Handle {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut owner = Owner::reserve_before_ready(FIXED, NOFILE as usize).unwrap();
        owner.admit_catalogue(&[2]).unwrap();
        owner
            .directory(2, || Ok(()), || Ok(Handle(drops.clone())))
            .unwrap();
        owner
            .scratch_acquire(0, || Ok(()), |_| Ok(Handle(drops.clone())))
            .unwrap();
        let mut diagnostic = InventoryDiagnostic::default();
        let refused = owner.scratch_perform(
            0,
            || Ok(()),
            |_| {
                available(
                    diagnostic.result::<()>(InventoryFailure::StatusParse, Err(()), |_| Ok(())),
                )
            },
        );
        assert!(refused.is_err());
        assert_eq!(drops.get(), 0);
        assert!(
            owner
                .directory(
                    2,
                    || panic!("gate after refusal"),
                    || panic!("open after refusal")
                )
                .is_err()
        );
        assert!(
            owner
                .scratch_release(0, || panic!("release after refusal"))
                .is_err()
        );
        assert!(owner.finish().is_err());
        assert_eq!(drops.get(), 0); // Owner still alive; no unwind/fatal claim.
    }
    #[test]
    fn zero_start_producer_counterexample_is_still_rejected_not_relaxed() {
        let mut fields = ["0"; 50];
        fields[0] = "S";
        let bytes = format!("2 (synthetic) {}\n", fields.join(" "));
        assert!(start_time(bytes.as_bytes(), 2).is_err());
        fields[19] = "1";
        let bytes = format!("2 (synthetic) {}\n", fields.join(" "));
        assert_eq!(start_time(bytes.as_bytes(), 2), Ok(1));
        assert_eq!(TextRole::Status.name(), "status");
        assert_eq!(TextRole::Stat.name(), "stat");
        assert_eq!(TextRole::Command.name(), "cmdline");
        assert_eq!(TextRole::Comm.name(), "comm");
    }
    #[test]
    fn only_fixed_user_queries_change_child_credentials() {
        let scopes = [true, true, false, false, true, false, false];
        assert_eq!(scopes.into_iter().filter(|system| !system).count(), 4);
        assert_eq!(query_credentials(true), None);
        assert_eq!(query_credentials(false), Some((1000, 1000)));
    }
    #[test]
    fn finite_query_progress_has_seven_unique_closed_frames() {
        let mut progress = QueryProgress::default();
        let mut seen = Vec::new();
        for step in QUERY_PHASES {
            progress
                .emit(step, |label| {
                    seen.push(label);
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(seen.len(), 7);
        for (index, label) in seen.iter().enumerate() {
            assert!(label.starts_with(b"t4_actor_"));
            assert!(label.ends_with(b"\n"));
            assert!(!seen[..index].contains(label));
        }
        assert_eq!(9 + 7 * QUERY_PHASES.len() + 1, 59);
        assert!(
            progress
                .emit(QueryPhase::Before, |_| panic!("eighth frame"))
                .is_err()
        );
    }
    #[test]
    fn query_progress_order_or_emission_refusal_is_terminal() {
        for cut in 0..QUERY_PHASES.len() {
            let mut progress = QueryProgress::default();
            let mut emitted = 0;
            for step in QUERY_PHASES[..cut].iter().copied() {
                progress
                    .emit(step, |_| {
                        emitted += 1;
                        Ok(())
                    })
                    .unwrap();
            }
            assert!(
                progress
                    .emit(QUERY_PHASES[cut], |_| {
                        emitted += 1;
                        Err(())
                    })
                    .is_err()
            );
            for later in QUERY_PHASES {
                assert!(
                    progress
                        .emit(later, |_| panic!("after output refusal"))
                        .is_err()
                );
            }
            assert_eq!(emitted, cut + 1);
        }
        let mut progress = QueryProgress::default();
        assert!(
            progress
                .emit(QueryPhase::Eof, |_| panic!("out of order"))
                .is_err()
        );
        assert!(
            progress
                .emit(QueryPhase::Before, |_| panic!("after order refusal"))
                .is_err()
        );
    }
    #[test]
    fn expired_query_phase_refuses_without_output_or_reentry() {
        let mut budget = Budget::new();
        budget.until = Instant::now() - Duration::from_secs(1);
        let mut progress = QueryProgress::default();
        let mut writes = 0;
        assert!(
            progress
                .emit(QueryPhase::Before, |_| {
                    budget.check()?;
                    writes += 1;
                    budget.check()
                })
                .is_err()
        );
        assert_eq!(writes, 0);
        assert!(
            progress
                .emit(QueryPhase::BeforeSpawn, |_| panic!("after expired phase"))
                .is_err()
        );
    }
    #[test]
    fn query_admits_only_original_zero_and_eof() {
        let pid = Pid::from_raw(42);
        assert_eq!(
            positive_query(false, WaitStatus::Exited(pid, 0), pid),
            Ok(false)
        );
        assert_eq!(
            positive_query(true, WaitStatus::Exited(pid, 0), pid),
            Ok(true)
        );
        assert_eq!(positive_query(true, WaitStatus::StillAlive, pid), Ok(false));
        for status in [
            WaitStatus::Exited(pid, 2),
            WaitStatus::Exited(Pid::from_raw(43), 0),
            WaitStatus::Signaled(pid, nix::sys::signal::Signal::SIGTERM, false),
        ] {
            assert!(positive_query(true, status, pid).is_err());
        }
    }
    #[test]
    fn reserve_and_refused_reentry_have_no_backend() {
        let mut held = Canonical::reserve().unwrap();
        held.refused = true;
        assert!(
            held.observe(Instant::now() + Duration::from_secs(2))
                .is_err()
        );
        assert!(held.finish().is_err());
        assert!(held.files.is_empty());
        assert!(held.query.is_none());
        assert_eq!(NOFILE as usize, MAX_PIDS * 2 + FIXED + 8);
        assert_eq!(FIXED_FILES + 1 + 4 + 2, 41);
        assert_eq!(FIXED - (FIXED_FILES + 1 + 4 + 36 + 2), 43);
    }
    #[test]
    fn expired_operation_refuses_before_original_acquisition() {
        let mut held = Canonical::reserve().unwrap();
        let expired = Instant::now() - Duration::from_secs(1);
        assert!(held.observe(expired).is_err());
        assert!(held.files.is_empty());
        assert!(held.query.is_none());
        assert!(
            held.observe(Instant::now() + Duration::from_secs(2))
                .is_err()
        );
        assert!(held.finish().is_err());
        assert!(held.files.is_empty());
    }
}
