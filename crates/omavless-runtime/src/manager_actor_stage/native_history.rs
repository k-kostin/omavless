// SPDX-License-Identifier: MIT
//! Bounded immutable audit custody, never an admission or decoder grant.
use super::*;
use nix::fcntl::openat;

pub(super) const CAPACITY: usize = 8;
pub(super) const NAMES: [&str; CAPACITY] = [
    "restore-disposition.history",
    "restore-disposition.history.1",
    "restore-disposition.history.2",
    "restore-disposition.history.3",
    "restore-disposition.history.4",
    "restore-disposition.history.5",
    "restore-disposition.history.6",
    "restore-disposition.history.7",
];
pub(super) fn catalogue_name_allowed(name: &[u8]) -> bool {
    !name.starts_with(b"restore-disposition.history")
        || NAMES.iter().any(|allowed| allowed.as_bytes() == name)
}
type Stamp = (u64, u64, u32, u32, u32, u64, u64, (i64, i64), (i64, i64));
fn stamp(m: &Metadata) -> Stamp {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.nlink(),
        m.len(),
        (m.mtime(), m.mtime_nsec()),
        (m.ctime(), m.ctime_nsec()),
    )
}
fn named_stamp(state: &File, name: &str) -> Result<Stamp, Unavailable> {
    let m = fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(|_| Unavailable)?;
    Ok(lookup_stamp(&m))
}
fn lookup_stamp(m: &nix::sys::stat::FileStat) -> Stamp {
    (
        m.st_dev,
        m.st_ino,
        m.st_mode,
        m.st_uid,
        m.st_gid,
        crate::file_link_count::link_count_u64(m.st_nlink),
        m.st_size as u64,
        (m.st_mtime, m.st_mtime_nsec),
        (m.st_ctime, m.st_ctime_nsec),
    )
}
fn absent(state: &File, name: &str) -> Result<(), Unavailable> {
    if matches!(
        fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW),
        Err(nix::errno::Errno::ENOENT)
    ) {
        Ok(())
    } else {
        Err(Unavailable)
    }
}
fn bytes(
    file: &File,
    until: Instant,
) -> Result<[u8; crate::restore_disposition_complete_model::COMPLETE_BYTES], Unavailable> {
    let mut raw = [0; crate::restore_disposition_complete_model::COMPLETE_BYTES];
    tick(until)?;
    file.read_exact_at(&mut raw, 0).map_err(|_| Unavailable)?;
    tick(until)?;
    let mut extra = [0];
    if file
        .read_at(&mut extra, raw.len() as u64)
        .map_err(|_| Unavailable)?
        != 0
    {
        return Err(Unavailable);
    }
    tick(until)?;
    Ok(raw)
}

/// Entire capacity is installed in the SAME NativeEngine before any capture.
/// Reported opens are stored before tick/metadata/bytes checks; no failed prefix
/// is evicted, retried or reused. No prior audit file is written/renamed/unlinked.
pub(super) struct History {
    files: [Option<File>; CAPACITY],
    metadata: [Option<Metadata>; CAPACITY],
    digests: [Option<[u8; 32]>; CAPACITY],
    attempted: bool,
    ready: bool,
    selected: Option<usize>,
    #[cfg(test)]
    fail_after_open: Option<usize>,
}
impl History {
    pub(super) fn reserve() -> Self {
        Self {
            files: std::array::from_fn(|_| None),
            metadata: std::array::from_fn(|_| None),
            digests: [None; CAPACITY],
            attempted: false,
            ready: false,
            selected: None,
            #[cfg(test)]
            fail_after_open: None,
        }
    }
    pub(super) fn attempted(&self) -> bool {
        self.attempted
    }
    pub(super) fn target(&self) -> Result<&'static str, Unavailable> {
        if !self.ready {
            return Err(Unavailable);
        }
        self.selected.map(|index| NAMES[index]).ok_or(Unavailable)
    }
    pub(super) fn capture(
        &mut self,
        state: &File,
        uid: u32,
        gid: u32,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if self.attempted {
            return Err(Unavailable);
        }
        self.attempted = true;
        for (index, name) in NAMES.into_iter().enumerate() {
            tick(until)?;
            match fstatat(state, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
                Err(nix::errno::Errno::ENOENT) => {
                    if self.selected.is_none() {
                        self.selected = Some(index);
                    }
                }
                Ok(named) if self.selected.is_none() => {
                    let reported = openat(
                        state,
                        name,
                        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| Unavailable)?;
                    self.files[index] = Some(File::from(reported)); // BEFORE all postchecks
                    #[cfg(test)]
                    if self.fail_after_open == Some(index) {
                        return Err(Unavailable);
                    }
                    tick(until)?;
                    let file = self.files[index].as_ref().ok_or(Unavailable)?;
                    self.metadata[index] = Some(file.metadata().map_err(|_| Unavailable)?);
                    let m = self.metadata[index].as_ref().ok_or(Unavailable)?;
                    let mut attributes = [0; 1];
                    if stamp(m) != lookup_stamp(&named)
                        || !m.is_file()
                        || m.mode() & 0o7777 != 0o600
                        || m.nlink() != 1
                        || m.uid() != uid
                        || m.gid() != gid
                        || m.len()
                            != crate::restore_disposition_complete_model::COMPLETE_BYTES as u64
                        || flistxattr(file, &mut attributes).map_err(|_| Unavailable)? != 0
                    {
                        return Err(Unavailable);
                    }
                    let raw = bytes(file, until)?;
                    // Integrity data only. No generation/login/lease/owner is
                    // produced from an older record, and its bytes are unchanged.
                    if crate::restore_disposition_complete_model::CompleteRecord::decode(&raw)
                        .is_none()
                        || stamp(m) != stamp(&file.metadata().map_err(|_| Unavailable)?)
                        || stamp(m) != named_stamp(state, name)?
                    {
                        return Err(Unavailable);
                    }
                    self.digests[index] = Some(Sha256::digest(raw).into());
                    tick(until)?;
                }
                _ => return Err(Unavailable), // sparse, wrong type/error, never repair
            }
        }
        self.selected.ok_or(Unavailable)?; // exhausted originals remain retained
        self.check_inner(state, false, until)?;
        self.ready = true;
        Ok(())
    }
    pub(super) fn check(
        &self,
        state: &File,
        published: bool,
        until: Instant,
    ) -> Result<(), Unavailable> {
        if !self.attempted {
            return Ok(());
        } // before closed capture, never selected authority
        if !self.ready {
            return Err(Unavailable);
        }
        self.check_inner(state, published, until)
    }
    fn check_inner(
        &self,
        state: &File,
        published: bool,
        until: Instant,
    ) -> Result<(), Unavailable> {
        let selected = self.selected.ok_or(Unavailable)?;
        for (index, name) in NAMES.into_iter().enumerate() {
            tick(until)?;
            if index < selected {
                let file = self.files[index].as_ref().ok_or(Unavailable)?;
                let original = self.metadata[index].as_ref().ok_or(Unavailable)?;
                let mut attributes = [0; 1];
                if stamp(original) != stamp(&file.metadata().map_err(|_| Unavailable)?)
                    || stamp(original) != named_stamp(state, name)?
                    || flistxattr(file, &mut attributes).map_err(|_| Unavailable)? != 0
                    || Some(Sha256::digest(bytes(file, until)?).into()) != self.digests[index]
                    || stamp(original) != stamp(&file.metadata().map_err(|_| Unavailable)?)
                    || stamp(original) != named_stamp(state, name)?
                {
                    return Err(Unavailable);
                }
            } else if index != selected || !published {
                absent(state, name)?;
            }
            tick(until)?;
        }
        // Published target is checked by the SAME Complete/Scratch1 role in
        // NativeEngine; this never recaptures it as an older audit original.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
    fn audit() -> [u8; crate::restore_disposition_complete_model::COMPLETE_BYTES] {
        let plan = planned_stage_identity([b"old", b"template", b"new", b"template"]).unwrap();
        let terminal = DecisionRecord::intent(7, None, &plan, [3; 16])
            .unwrap()
            .terminal(TerminalChoice::Abort)
            .unwrap();
        let receipt = crate::restore_retirement_candidate::RetirementReceipt::synthetic(
            &terminal,
            b"old",
            b"template",
        );
        let closure =
            crate::restore_closure_model::ClosureRecord::from_verified_receipt(&receipt).unwrap();
        let ticket = crate::restore_disposition_ticket_model::Ticket::from_bound_closure(
            &closure,
            nix::unistd::getuid().as_raw(),
            7,
            None,
        )
        .unwrap();
        crate::restore_disposition_complete_model::CompleteRecord::from_ticket(&ticket)
            .unwrap()
            .encode()
    }
    fn write(root: &std::path::Path, name: &str, raw: &[u8]) {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join(name))
            .unwrap();
        f.write_all(raw).unwrap();
        f.sync_all().unwrap();
    }
    #[test]
    fn native_history_all_eight_prefixes_preserve_originals_and_exhaustion() {
        let raw = audit();
        for count in 0..=CAPACITY {
            let root = crate::test_temp::directory("native-history-prefix").unwrap();
            for name in &NAMES[..count] {
                write(&root, name, &raw);
            }
            let before: Vec<_> = NAMES[..count]
                .iter()
                .map(|name| stamp(&fs::symlink_metadata(root.join(name)).unwrap()))
                .collect();
            let state = File::open(&root).unwrap();
            let mut history = History::reserve();
            let until = Instant::now() + std::time::Duration::from_secs(5);
            let result = history.capture(
                &state,
                nix::unistd::getuid().as_raw(),
                nix::unistd::getgid().as_raw(),
                until,
            );
            assert_eq!(result.is_ok(), count < CAPACITY);
            assert_eq!(history.files.iter().filter(|f| f.is_some()).count(), count);
            if count < CAPACITY {
                assert_eq!(history.target().unwrap(), NAMES[count]);
                history.check(&state, false, until).unwrap();
                write(&root, "test-owned-complete", &raw);
                let incoming = File::open(root.join("test-owned-complete")).unwrap();
                let ino = incoming.metadata().unwrap().ino();
                nix::fcntl::renameat2(
                    &state,
                    "test-owned-complete",
                    &state,
                    history.target().unwrap(),
                    nix::fcntl::RenameFlags::RENAME_NOREPLACE,
                )
                .unwrap();
                state.sync_all().unwrap();
                assert_eq!(fs::metadata(root.join(NAMES[count])).unwrap().ino(), ino);
                history.check(&state, true, until).unwrap();
            } else {
                assert!(history.target().is_err() && history.check(&state, false, until).is_err());
            }
            assert!(
                history
                    .capture(
                        &state,
                        nix::unistd::getuid().as_raw(),
                        nix::unistd::getgid().as_raw(),
                        until
                    )
                    .is_err()
            );
            for (index, name) in NAMES[..count].iter().enumerate() {
                assert_eq!(fs::read(root.join(name)).unwrap(), raw);
                assert_eq!(
                    stamp(&fs::symlink_metadata(root.join(name)).unwrap()),
                    before[index]
                );
            }
            drop((history, state));
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn native_history_sparse_corrupt_type_mode_deadline_and_collision_refuse() {
        let raw = audit();
        for case in 0..8 {
            let root = crate::test_temp::directory("native-history-refusal").unwrap();
            match case {
                0 => write(&root, NAMES[1], &raw), // hole before later audit
                1 => {
                    let mut bad = raw;
                    bad[0] ^= 1;
                    write(&root, NAMES[0], &bad);
                }
                2 => write(&root, NAMES[0], &[raw.as_slice(), b"x"].concat()),
                3 => {
                    write(&root, NAMES[0], &raw);
                    fs::set_permissions(root.join(NAMES[0]), fs::Permissions::from_mode(0o644))
                        .unwrap();
                }
                4 => symlink("missing", root.join(NAMES[0])).unwrap(),
                5 => {
                    fs::create_dir(root.join(NAMES[0])).unwrap();
                }
                6 | 7 => write(&root, NAMES[0], &raw),
                _ => unreachable!(),
            }
            let state = File::open(&root).unwrap();
            let mut h = History::reserve();
            let until = if case == 6 {
                Instant::now() - std::time::Duration::from_secs(1)
            } else {
                Instant::now() + std::time::Duration::from_secs(5)
            };
            let result = h.capture(
                &state,
                nix::unistd::getuid().as_raw(),
                nix::unistd::getgid().as_raw(),
                until,
            );
            if case == 7 {
                result.unwrap();
                write(&root, NAMES[1], &raw);
                assert!(h.check(&state, false, until).is_err());
                write(&root, "test-owned-complete", &raw);
                assert_eq!(
                    nix::fcntl::renameat2(
                        &state,
                        "test-owned-complete",
                        &state,
                        NAMES[1],
                        nix::fcntl::RenameFlags::RENAME_NOREPLACE
                    ),
                    Err(nix::errno::Errno::EEXIST)
                );
            } else {
                assert!(result.is_err() && h.target().is_err());
                if matches!(case, 1 | 2 | 3 | 5) {
                    assert!(h.files[0].is_some());
                }
            }
            drop((h, state));
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn native_history_drift_of_each_original_or_any_absence_is_not_waived() {
        let raw = audit();
        for case in 0..5 {
            let root = crate::test_temp::directory("native-history-drift").unwrap();
            write(&root, NAMES[0], &raw);
            write(&root, NAMES[1], &raw);
            let state = File::open(&root).unwrap();
            let mut h = History::reserve();
            let until = Instant::now() + std::time::Duration::from_secs(5);
            h.capture(
                &state,
                nix::unistd::getuid().as_raw(),
                nix::unistd::getgid().as_raw(),
                until,
            )
            .unwrap();
            match case {
                0 => {
                    fs::rename(root.join(NAMES[0]), root.join("displaced")).unwrap();
                    write(&root, NAMES[0], &raw);
                }
                1 => {
                    fs::OpenOptions::new()
                        .write(true)
                        .open(root.join(NAMES[1]))
                        .unwrap()
                        .write_all(b"x")
                        .unwrap();
                }
                2 => fs::set_permissions(root.join(NAMES[1]), fs::Permissions::from_mode(0o644))
                    .unwrap(),
                3 => symlink("missing", root.join(NAMES[3])).unwrap(),
                4 => fs::hard_link(root.join(NAMES[0]), root.join("extra-link")).unwrap(),
                _ => unreachable!(),
            }
            assert!(h.check(&state, false, until).is_err());
            assert!(h.files[0].is_some() && h.files[1].is_some());
            drop((h, state));
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn native_history_every_reported_open_is_retained_before_postcheck_failure() {
        let raw = audit();
        for cut in 0..CAPACITY {
            let root = crate::test_temp::directory("native-history-cut").unwrap();
            for name in NAMES {
                write(&root, name, &raw);
            }
            let originals: Vec<_> = NAMES
                .iter()
                .map(|name| stamp(&fs::symlink_metadata(root.join(name)).unwrap()))
                .collect();
            let state = File::open(&root).unwrap();
            let mut h = History::reserve();
            h.fail_after_open = Some(cut);
            let until = Instant::now() + std::time::Duration::from_secs(5);
            assert!(
                h.capture(
                    &state,
                    nix::unistd::getuid().as_raw(),
                    nix::unistd::getgid().as_raw(),
                    until
                )
                .is_err()
            );
            assert_eq!(h.files.iter().filter(|f| f.is_some()).count(), cut + 1);
            assert!(h.target().is_err() && h.check(&state, false, until).is_err());
            h.fail_after_open = None; // repairing the test fault does not rearm capture
            assert!(
                h.capture(
                    &state,
                    nix::unistd::getuid().as_raw(),
                    nix::unistd::getgid().as_raw(),
                    until
                )
                .is_err()
            );
            for (index, name) in NAMES.iter().enumerate() {
                assert_eq!(fs::read(root.join(name)).unwrap(), raw);
                assert_eq!(
                    stamp(&fs::symlink_metadata(root.join(name)).unwrap()),
                    originals[index]
                );
            }
            drop((h, state));
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn native_history_vocabulary_is_fixed_not_an_arbitrary_numbered_path() {
        assert_eq!(CAPACITY, 8);
        for name in NAMES {
            assert!(catalogue_name_allowed(name.as_bytes()));
        }
        assert!(catalogue_name_allowed(b"desired-state.json"));
        for name in [
            b"restore-disposition.history.8".as_slice(),
            b"restore-disposition.history.01",
            b"restore-disposition.history.-1",
            b"restore-disposition.history.next",
            b"restore-disposition.history/child",
        ] {
            assert!(!catalogue_name_allowed(name));
        }
    }
}
