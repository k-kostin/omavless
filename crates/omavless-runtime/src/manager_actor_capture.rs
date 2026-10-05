// SPDX-License-Identifier: MIT
//! Private retained capture. No descriptor/proof leaves its actor owner.

use super::*;
use nix::fcntl::AtFlags;
use nix::sys::stat::{FileStat, fstatat};
use std::io::{Read, Write};
use std::os::unix::fs::FileExt;

pub(crate) const CAPTURE_FDS: usize = 17;
pub(crate) const HELD_FDS: usize = 51;

fn phase(label: &'static [u8], budget: &Budget) -> Result<()> {
    budget.check()?;
    let mut output = std::io::stderr().lock();
    output.write_all(label).map_err(|_| ())?;
    output.flush().map_err(|_| ())?;
    budget.check()
}

fn capacity_admission(
    held: usize,
    refused: &mut bool,
    until: Instant,
    emit: impl FnOnce(&Budget) -> Result<()>,
) -> Result<()> {
    if *refused {
        return Err(());
    }
    if held.checked_add(CAPTURE_FDS).is_none_or(|n| n > HELD_FDS) {
        // The unchanged capacity predicate, with no acquisition or count output.
        *refused = true;
        let mut budget = Budget::new();
        budget.until = budget.until.min(until);
        emit(&budget)?;
        return Err(());
    }
    Ok(())
}

pub(crate) struct Retained {
    files: Vec<File>,
    observations: Vec<(usize, Snapshot, Snapshot)>,
    refused: bool,
}

struct Snapshot {
    directory: usize,
    stat: usize,
    status_file: usize,
    command_file: usize,
    comm_file: usize,
    executable: usize,
    pid_namespace: usize,
    user_namespace: usize,
    directory_identity: Metadata,
    executable_identity: Metadata,
    start: u64,
    status: Status,
    command: Zeroizing<Vec<u8>>,
    comm: Zeroizing<Vec<u8>>,
    executable_name: Zeroizing<Vec<u8>>,
}

impl Retained {
    pub(crate) fn new() -> Result<Self> {
        let mut files = Vec::new();
        files.try_reserve_exact(HELD_FDS).map_err(|_| ())?;
        let mut observations = Vec::new();
        observations.try_reserve_exact(3).map_err(|_| ())?;
        Ok(Self {
            files,
            observations,
            refused: false,
        })
    }

    // Reserved before any open. The positive return is retained immediately,
    // BEFORE checking deadline/shape. An unreported backend partial is not ours.
    fn keep(&mut self, file: File) -> usize {
        let index = self.files.len();
        self.files.push(file);
        index
    }

    fn open_at(
        &mut self,
        parent: usize,
        name: &str,
        flags: OFlag,
        budget: &Budget,
    ) -> Result<usize> {
        budget.check()?;
        let owned = openat(
            &self.files[parent],
            name,
            flags | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?;
        let index = self.keep(File::from(owned));
        budget.check()?;
        Ok(index)
    }

    fn bytes(
        &mut self,
        parent: usize,
        name: &str,
        cap: usize,
        budget: &mut Budget,
    ) -> Result<(usize, Zeroizing<Vec<u8>>)> {
        // Allocate before acquisition; bounded continuation reads do not grow.
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.try_reserve_exact(cap + 1).map_err(|_| ())?;
        let index = self.open_at(
            parent,
            name,
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW,
            budget,
        )?;
        if !self.files[index].metadata().map_err(|_| ())?.is_file()
            || fstatfs(&self.files[index])
                .map_err(|_| ())?
                .filesystem_type()
                != PROC_SUPER_MAGIC
        {
            return Err(());
        }
        loop {
            budget.check()?;
            let mut buffer = [0; 4096];
            let left = (cap + 1).checked_sub(bytes.len()).ok_or(())?;
            if left == 0 {
                return Err(());
            }
            let size = (&self.files[index])
                .read(&mut buffer[..left.min(4096)])
                .map_err(|_| ())?;
            budget.charge(size)?;
            if size == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..size]);
            if bytes.len() > cap {
                return Err(());
            }
        }
        Ok((index, bytes))
    }

    fn snapshot(&mut self, root: usize, budget: &mut Budget) -> Result<Snapshot> {
        phase(b"t4_actor_before_directory\n", budget)?;
        let directory = self.open_at(
            root,
            "1",
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW,
            budget,
        )?;
        let directory_identity = self.files[directory].metadata().map_err(|_| ())?;
        phase(b"t4_actor_before_stat\n", budget)?;
        let (stat, stat_bytes) = self.bytes(directory, "stat", MAX_STATUS, budget)?;
        let start = start_time(&stat_bytes, 1)?;
        phase(b"t4_actor_before_status\n", budget)?;
        let (status_file, status_bytes) = self.bytes(directory, "status", MAX_STATUS, budget)?;
        let status = status(&status_bytes, 1)?;
        // First developer scope is root-owned visible PID1, not a canonical
        // systemd/bus/service origin or whole-inventory StoppedOwner admission.
        if status.uids != [0; 4] || status.namespace_pids != [1] {
            return Err(());
        }
        phase(b"t4_actor_before_command\n", budget)?;
        let (command_file, command) = self.bytes(directory, "cmdline", MAX_COMMAND, budget)?;
        arguments(&command)?;
        phase(b"t4_actor_before_comm\n", budget)?;
        let (comm_file, comm) = self.bytes(directory, "comm", 4096, budget)?;
        if comm.is_empty() || !comm.ends_with(b"\n") {
            return Err(());
        }
        phase(b"t4_actor_before_executable\n", budget)?;
        let executable = self.open_at(directory, "exe", OFlag::O_PATH, budget)?;
        let executable_identity = self.files[executable].metadata().map_err(|_| ())?;
        if !executable_identity.is_file() || executable_identity.nlink() == 0 {
            return Err(());
        }
        phase(b"t4_actor_before_executable_name\n", budget)?;
        let executable_name = proc_link(&self.files[directory], "exe")?;
        phase(b"t4_actor_before_pid_namespace\n", budget)?;
        let pid_namespace = self.open_at(directory, "ns/pid", OFlag::O_PATH, budget)?;
        phase(b"t4_actor_before_user_namespace\n", budget)?;
        let user_namespace = self.open_at(directory, "ns/user", OFlag::O_PATH, budget)?;
        budget.check()?;
        Ok(Snapshot {
            directory,
            stat,
            status_file,
            command_file,
            comm_file,
            executable,
            pid_namespace,
            user_namespace,
            directory_identity,
            executable_identity,
            start,
            status,
            command,
            comm,
            executable_name,
        })
    }

    fn held_current(&self, snapshot: &Snapshot) -> Result<()> {
        if !identity(
            &snapshot.directory_identity,
            &self.files[snapshot.directory].metadata().map_err(|_| ())?,
        ) || !executable_identity(
            &snapshot.executable_identity,
            &self.files[snapshot.executable].metadata().map_err(|_| ())?,
        ) {
            return Err(());
        }
        Ok(())
    }

    fn observe_inner(&mut self, until: Instant) -> Result<()> {
        let mut budget = Budget::new();
        budget.until = budget.until.min(until);
        phase(b"t4_actor_before_proc_root\n", &budget)?;
        let root = self.keep(File::from(
            open(
                "/proc",
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        ));
        budget.check()?;
        if fstatfs(&self.files[root])
            .map_err(|_| ())?
            .filesystem_type()
            != PROC_SUPER_MAGIC
        {
            return Err(());
        }
        phase(b"t4_actor_before_first_snapshot\n", &budget)?;
        let before = self.snapshot(root, &mut budget)?;
        phase(b"t4_actor_before_second_snapshot\n", &budget)?;
        let after = self.snapshot(root, &mut budget)?;
        phase(b"t4_actor_before_final_comparison\n", &budget)?;
        self.held_current(&before)?;
        self.held_current(&after)?;
        if !identity(&before.directory_identity, &after.directory_identity)
            || !executable_identity(&before.executable_identity, &after.executable_identity)
            || before.start != after.start
            || before.status != after.status
            || before.command != after.command
            || before.comm != after.comm
            || before.executable_name != after.executable_name
        {
            return Err(());
        }
        same_namespace(
            &self.files[before.pid_namespace],
            &self.files[after.pid_namespace],
        )?;
        same_namespace(
            &self.files[before.user_namespace],
            &self.files[after.user_namespace],
        )?;
        // Retain original comparison inputs and their actual FD indices too;
        // future checks cannot reconstruct a new lease from copied caller data.
        self.observations.push((root, before, after));
        phase(b"t4_actor_identity_checked\n", &budget)
    }

    fn named_current(
        &self,
        parent: usize,
        name: &str,
        index: usize,
        follow: bool,
        executable: bool,
        budget: &Budget,
    ) -> Result<()> {
        budget.check()?;
        let original = self.files[index].metadata().map_err(|_| ())?;
        budget.check()?;
        let named = fstatat(
            &self.files[parent],
            name,
            if follow {
                AtFlags::empty()
            } else {
                AtFlags::AT_SYMLINK_NOFOLLOW
            },
        )
        .map_err(|_| ())?;
        budget.check()?;
        if !named_matches(&original, &named, executable) {
            return Err(());
        }
        let after = self.files[index].metadata().map_err(|_| ())?;
        budget.check()?;
        if !(if executable {
            executable_identity(&original, &after)
        } else {
            identity(&original, &after)
        }) {
            return Err(());
        }
        Ok(())
    }

    fn original_bytes(
        &self,
        snapshot: &Snapshot,
        name: &str,
        index: usize,
        cap: usize,
        budget: &mut Budget,
    ) -> Result<Zeroizing<Vec<u8>>> {
        self.named_current(snapshot.directory, name, index, false, false, budget)?;
        if !self.files[index].metadata().map_err(|_| ())?.is_file()
            || fstatfs(&self.files[index])
                .map_err(|_| ())?
                .filesystem_type()
                != PROC_SUPER_MAGIC
        {
            return Err(());
        }
        // Explicit offset zero refreshes seq_file reads. No seek-shared cursor,
        // new open, duplicate FD or detached manager/proof capability.
        let bytes = bounded_read_at(
            cap,
            || budget.check(),
            |buffer, offset| self.files[index].read_at(buffer, offset).map_err(|_| ()),
        )?;
        budget.charge(bytes.len())?;
        self.named_current(snapshot.directory, name, index, false, false, budget)?;
        Ok(bytes)
    }

    fn recheck_snapshot(
        &self,
        root: usize,
        snapshot: &Snapshot,
        budget: &mut Budget,
    ) -> Result<()> {
        self.held_current(snapshot)?;
        self.named_current(root, "1", snapshot.directory, false, false, budget)?;
        if start_time(
            &self.original_bytes(snapshot, "stat", snapshot.stat, MAX_STATUS, budget)?,
            1,
        )? != snapshot.start
            || status(
                &self.original_bytes(
                    snapshot,
                    "status",
                    snapshot.status_file,
                    MAX_STATUS,
                    budget,
                )?,
                1,
            )? != snapshot.status
            || self.original_bytes(
                snapshot,
                "cmdline",
                snapshot.command_file,
                MAX_COMMAND,
                budget,
            )? != snapshot.command
            || self.original_bytes(snapshot, "comm", snapshot.comm_file, 4096, budget)?
                != snapshot.comm
        {
            return Err(());
        }
        self.named_current(
            snapshot.directory,
            "exe",
            snapshot.executable,
            true,
            true,
            budget,
        )?;
        budget.check()?;
        if proc_link(&self.files[snapshot.directory], "exe")? != snapshot.executable_name {
            return Err(());
        }
        budget.check()?;
        self.named_current(
            snapshot.directory,
            "ns/pid",
            snapshot.pid_namespace,
            true,
            false,
            budget,
        )?;
        self.named_current(
            snapshot.directory,
            "ns/user",
            snapshot.user_namespace,
            true,
            false,
            budget,
        )?;
        self.held_current(snapshot)?;
        self.named_current(root, "1", snapshot.directory, false, false, budget)
    }

    /// Re-read BOTH original snapshots inside this same owner borrow after
    /// crypto and before a reply/effect. Fixed PID1 remains developer origin,
    /// never a canonical user-manager/StoppedOwner or future restore authority.
    pub(crate) fn recheck_original(&mut self, until: Instant) -> Result<()> {
        if self.refused {
            return Err(());
        }
        let result = (|| {
            let mut budget = Budget::new();
            budget.until = budget.until.min(until);
            let (root, before, after) = self.observations.last().ok_or(())?;
            budget.check()?;
            if fstatfs(&self.files[*root])
                .map_err(|_| ())?
                .filesystem_type()
                != PROC_SUPER_MAGIC
            {
                return Err(());
            }
            self.recheck_snapshot(*root, before, &mut budget)?;
            self.recheck_snapshot(*root, after, &mut budget)?;
            phase(b"t4_actor_original_manager_rechecked\n", &budget)
        })();
        if result.is_err() {
            self.refused = true;
        }
        result
    }

    /// Single exclusive original owner borrow includes acquisition, recheck,
    /// comparison and proof consumption. Result has no live authority payload.
    pub(crate) fn observe(&mut self, until: Instant) -> Result<()> {
        capacity_admission(self.files.len(), &mut self.refused, until, |budget| {
            phase(b"t4_actor_capacity_refused\n", budget)
        })?;
        let result = self.observe_inner(until);
        if result.is_err() {
            self.refused = true;
        }
        result
    }

    /// Normal completed protocol teardown only. The caller cannot reach this
    /// from a pending/refused owner. File Drop uses the ordinary close backend;
    /// no per-FD kernel absence proof or uncertain-close adoption is claimed.
    pub(crate) fn finish(&mut self) -> Result<()> {
        if self.refused {
            return Err(());
        }
        self.refused = true;
        self.files.clear();
        self.observations.clear();
        Ok(())
    }
}

fn named_matches(original: &Metadata, named: &FileStat, executable: bool) -> bool {
    original.dev() == named.st_dev
        && original.ino() == named.st_ino
        && original.mode() == named.st_mode
        && original.uid() == named.st_uid
        && original.gid() == named.st_gid
        && (!executable
            || (original.len() == named.st_size as u64
                && original.nlink() == named.st_nlink
                && original.mtime() == named.st_mtime
                && original.mtime_nsec() == named.st_mtime_nsec
                && original.ctime() == named.st_ctime
                && original.ctime_nsec() == named.st_ctime_nsec))
}

fn bounded_read_at(
    cap: usize,
    mut gate: impl FnMut() -> Result<()>,
    mut read: impl FnMut(&mut [u8], u64) -> Result<usize>,
) -> Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(cap.checked_add(1).ok_or(())?)
        .map_err(|_| ())?;
    loop {
        gate()?;
        let mut buffer = [0; 4096];
        let left = cap
            .checked_add(1)
            .and_then(|n| n.checked_sub(bytes.len()))
            .ok_or(())?;
        if left == 0 {
            return Err(());
        }
        let requested = left.min(buffer.len());
        let size = read(
            &mut buffer[..requested],
            u64::try_from(bytes.len()).map_err(|_| ())?,
        )?;
        if size > requested {
            return Err(());
        }
        bytes.extend_from_slice(&buffer[..size]);
        gate()?;
        if bytes.len() > cap {
            return Err(());
        }
        if size == 0 {
            return Ok(bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_reads_use_original_offset_zero_then_exact_bounded_continuations() {
        let source = b"whole fixed public source";
        let mut offsets = Vec::new();
        let got = bounded_read_at(
            source.len(),
            || Ok(()),
            |buffer, offset| {
                offsets.push(offset);
                let offset = usize::try_from(offset).unwrap();
                let take = buffer.len().min(3).min(source.len() - offset);
                buffer[..take].copy_from_slice(&source[offset..offset + take]);
                Ok(take)
            },
        )
        .unwrap();
        assert_eq!(got.as_slice(), source);
        assert_eq!(offsets.first(), Some(&0));
        assert_eq!(offsets.last(), Some(&(source.len() as u64)));
        for source in [source.as_slice(), b""] {
            let mut first = true;
            let got = bounded_read_at(
                source.len(),
                || Ok(()),
                |buffer, offset| {
                    if first {
                        assert_eq!(offset, 0);
                        first = false;
                    }
                    let offset = usize::try_from(offset).unwrap();
                    let take = buffer.len().min(source.len() - offset);
                    buffer[..take].copy_from_slice(&source[offset..offset + take]);
                    Ok(take)
                },
            )
            .unwrap();
            assert_eq!(got.as_slice(), source);
        }
    }

    #[test]
    fn fresh_reads_reject_overlong_io_error_or_late_next_without_continuation() {
        use std::cell::Cell;
        for cut in 0..4 {
            let gates = Cell::new(0);
            let reads = Cell::new(0);
            let result = bounded_read_at(
                3,
                || {
                    let step = gates.get();
                    gates.set(step + 1);
                    if (cut == 0 && step == 0) || (cut == 1 && step == 1) {
                        Err(())
                    } else {
                        Ok(())
                    }
                },
                |buffer, offset| {
                    reads.set(reads.get() + 1);
                    assert_eq!(offset, 0);
                    if cut == 2 {
                        return Err(());
                    }
                    buffer.fill(b'x');
                    Ok(buffer.len()) // cap+1 exact oversize for cut3
                },
            );
            assert!(result.is_err());
            assert_eq!(reads.get(), usize::from(cut != 0));
        }
        assert!(bounded_read_at(3, || Ok(()), |buffer, _| Ok(buffer.len() + 1)).is_err());
    }

    #[test]
    fn no_original_lease_or_revoked_owner_cannot_recheck_or_restart_capture() {
        let until = Instant::now() + Duration::from_secs(1);
        let mut owner = Retained::new().unwrap();
        assert!(owner.recheck_original(until).is_err());
        assert!(owner.refused);
        assert!(owner.recheck_original(until).is_err());
        assert!(owner.observe(until).is_err()); // refusal before any open/phase
        assert!(owner.finish().is_err());
        assert!(owner.files.is_empty());
    }
    #[test]
    fn reservation_accounts_two_full_snapshots_and_root_before_effect() {
        // root + two*(directory, stat, status, cmdline, comm, exe, pidns,userns).
        assert_eq!(CAPTURE_FDS, 1 + 2 * 8);
        assert_eq!(HELD_FDS, 3 * CAPTURE_FDS);
        for held in 0..=HELD_FDS + 1 {
            let admitted = held.checked_add(CAPTURE_FDS).is_some_and(|n| n <= HELD_FDS);
            assert_eq!(admitted, held <= HELD_FDS - CAPTURE_FDS);
        }
    }
    #[test]
    fn refused_owner_has_no_reentry_even_with_unused_capacity() {
        let mut owner = Retained::new().unwrap();
        owner.refused = true;
        let until = Instant::now() + Duration::from_secs(1);
        assert!(owner.observe(until).is_err()); // returns before first open.
        assert!(owner.observe(until).is_err());
        assert!(owner.files.is_empty());
    }

    #[test]
    fn exact_capacity_refusal_emits_once_and_output_failure_cannot_reenter() {
        use std::cell::Cell;
        for output_ok in [true, false] {
            let mut refused = false;
            let calls = Cell::new(0);
            let until = Instant::now() + Duration::from_secs(1);
            let output = |budget: &Budget| {
                calls.set(calls.get() + 1);
                budget.check()?;
                if output_ok { Ok(()) } else { Err(()) }
            };
            assert!(capacity_admission(HELD_FDS, &mut refused, until, output).is_err());
            assert!(refused);
            assert_eq!(calls.get(), 1);
            // Even a different presented count cannot bypass the original latch.
            assert!(capacity_admission(0, &mut refused, until, output).is_err());
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn capacity_pre_post_deadline_failure_is_not_completion_or_second_diagnostic() {
        for late in [false, true] {
            let mut refused = false;
            let until = if late {
                Instant::now() + Duration::from_secs(1)
            } else {
                Instant::now() - Duration::from_secs(1)
            };
            let mut wrote = false;
            assert!(
                capacity_admission(HELD_FDS, &mut refused, until, |budget| {
                    budget.check()?;
                    wrote = true;
                    let expired = Budget {
                        until: Instant::now() - Duration::from_secs(1),
                        ..Budget::new()
                    };
                    expired.check()
                })
                .is_err()
            );
            assert_eq!(wrote, late);
            assert!(refused);
            assert!(
                capacity_admission(
                    0,
                    &mut refused,
                    Instant::now() + Duration::from_secs(1),
                    |_| panic!("reentry diagnostic")
                )
                .is_err()
            );
        }
    }
}
