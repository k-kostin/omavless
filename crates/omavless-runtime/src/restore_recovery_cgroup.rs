// SPDX-License-Identifier: MIT
//! Fixed-unit recursive quiescence for cooperative Restore only. The root
//! system manager attests user@UID; its delegated app/service subtree is NOT
//! authority against hostile same-UID cgroup migration. Shared writer locks,
//! unit/job checks, manager lifetime, NativeOff and listener checks remain
//! separate prerequisites. Historical Abort keeps its process inventory.
use super::*;
use nix::errno::Errno;
use nix::sys::statfs::CGROUP2_SUPER_MAGIC;

const UNIT: &str = "omavless-runtime.service";
const EVENTS_CAP: usize = 1024;

pub(super) fn manager_group(uid: u32) -> String {
    format!("/user.slice/user-{uid}.slice/user@{uid}.service")
}
pub(super) fn runtime_group(uid: u32) -> String {
    format!("{}/app.slice/{UNIT}", manager_group(uid))
}

struct Anchor {
    path: std::path::PathBuf,
    file: File,
    metadata: Metadata,
}
impl Anchor {
    fn new(path: std::path::PathBuf, file: File, uid: u32) -> Result<Self> {
        let metadata = file.metadata().map_err(|_| ())?;
        if !metadata.is_dir()
            || (metadata.uid() != 0 && metadata.uid() != uid)
            || metadata.mode() & 0o022 != 0
        {
            return Err(());
        }
        Ok(Self {
            path,
            file,
            metadata,
        })
    }
    fn recheck(&self) -> Result<()> {
        if !identity(&self.metadata, &self.file.metadata().map_err(|_| ())?)
            || !identity(
                &self.metadata,
                &fs::symlink_metadata(&self.path).map_err(|_| ())?,
            )
        {
            return Err(());
        }
        Ok(())
    }
}

fn optional_directory(parent: &File, leaf: &str) -> Result<Option<File>> {
    match openat(
        parent,
        leaf,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => Ok(Some(File::from(fd))),
        Err(Errno::ENOENT) => Ok(None),
        Err(_) => Err(()),
    }
}

fn cgroup2(file: &File) -> Result<()> {
    if fstatfs(file).map_err(|_| ())?.filesystem_type() != CGROUP2_SUPER_MAGIC {
        return Err(());
    }
    Ok(())
}

fn empty_events(bytes: &[u8]) -> Result<()> {
    if bytes.len() > EVENTS_CAP || !bytes.ends_with(b"\n") {
        return Err(());
    }
    let mut populated = false;
    let mut frozen = false;
    for line in bytes[..bytes.len() - 1].split(|b| *b == b'\n') {
        match line {
            b"populated 0" if !populated => populated = true,
            b"frozen 0" if !frozen => frozen = true,
            _ => return Err(()),
        }
    }
    if !populated || !frozen {
        return Err(());
    }
    Ok(())
}

fn observe_child(
    parent: &File,
    uid: u32,
    budget: &mut Budget,
    verify_fs: impl Fn(&File) -> Result<()>,
) -> Result<()> {
    budget.check()?;
    let Some(child) = optional_directory(parent, UNIT)? else {
        // ENOENT only, not a failed read/permission check interpreted as empty.
        return if optional_directory(parent, UNIT)?.is_none() {
            Ok(())
        } else {
            Err(())
        };
    };
    let metadata = child.metadata().map_err(|_| ())?;
    if (metadata.uid() != 0 && metadata.uid() != uid) || metadata.mode() & 0o022 != 0 {
        return Err(());
    }
    verify_fs(&child)?;
    if metadata.dev() != parent.metadata().map_err(|_| ())?.dev() {
        return Err(());
    }
    let file = File::from(
        openat(
            &child,
            "cgroup.events",
            OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?,
    );
    let events_metadata = file.metadata().map_err(|_| ())?;
    if !events_metadata.is_file() || events_metadata.dev() != metadata.dev() {
        return Err(());
    }
    verify_fs(&file)?;
    let mut bytes = Vec::new();
    (&file)
        .take((EVENTS_CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    budget.charge(bytes.len())?;
    empty_events(&bytes)?;
    let named = optional_directory(parent, UNIT)?.ok_or(())?;
    if !identity(&metadata, &named.metadata().map_err(|_| ())?)
        || !identity(&metadata, &child.metadata().map_err(|_| ())?)
    {
        return Err(());
    }
    let named_events = File::from(
        openat(
            &child,
            "cgroup.events",
            OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?,
    );
    if !identity(&events_metadata, &named_events.metadata().map_err(|_| ())?)
        || !identity(&events_metadata, &file.metadata().map_err(|_| ())?)
    {
        return Err(());
    }
    budget.check()
}

pub(super) struct RecoveryCgroup {
    anchors: Vec<Anchor>,
    app: Option<Anchor>,
    unit_parents: Vec<Anchor>,
    unit: File,
    unit_metadata: Metadata,
    uid: u32,
}

impl RecoveryCgroup {
    pub(super) fn capture(uid: u32, budget: &mut Budget) -> Result<Self> {
        budget.check()?;
        let root = File::from(
            open(
                Path::new("/"),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        let mut anchors = vec![Anchor::new("/".into(), root, 0)?];
        let components = [
            "sys".to_owned(),
            "fs".to_owned(),
            "cgroup".to_owned(),
            "user.slice".to_owned(),
            format!("user-{uid}.slice"),
            format!("user@{uid}.service"),
        ];
        let mut group_device = None;
        for (index, name) in components.iter().enumerate() {
            let parent = anchors.last().ok_or(())?;
            let file = directory(&parent.file, name)?;
            let anchor = Anchor::new(
                parent.path.join(name),
                file,
                if index == 5 { uid } else { 0 },
            )?;
            if index >= 2 {
                cgroup2(&anchor.file)?;
                if let Some(device) = group_device {
                    if anchor.metadata.dev() != device {
                        return Err(());
                    }
                } else {
                    group_device = Some(anchor.metadata.dev());
                }
            }
            anchors.push(anchor);
        }
        let parent = anchors.last().ok_or(())?;
        let app = optional_directory(&parent.file, "app.slice")?
            .map(|file| {
                cgroup2(&file)?;
                let anchor = Anchor::new(parent.path.join("app.slice"), file, uid)?;
                if anchor.metadata.dev() != parent.metadata.dev() {
                    return Err(());
                }
                Ok::<_, ()>(anchor)
            })
            .transpose()?;
        // Pin the actual root-owned packaged unit and every parent, rather than
        // trusting a user manager's FragmentPath string alone.
        let root = File::from(
            open(
                Path::new("/"),
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        let mut unit_parents = vec![Anchor::new("/".into(), root, 0)?];
        for name in ["usr", "lib", "systemd", "user"] {
            let parent = unit_parents.last().ok_or(())?;
            let file = directory(&parent.file, name)?;
            unit_parents.push(Anchor::new(parent.path.join(name), file, 0)?);
        }
        let unit = File::from(
            openat(
                &unit_parents.last().ok_or(())?.file,
                UNIT,
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        let unit_metadata = unit.metadata().map_err(|_| ())?;
        if !unit_metadata.is_file()
            || unit_metadata.uid() != 0
            || unit_metadata.mode() & 0o022 != 0
            || unit_metadata.nlink() == 0
        {
            return Err(());
        }
        let result = Self {
            anchors,
            app,
            unit_parents,
            unit,
            unit_metadata,
            uid,
        };
        result.recheck_hierarchy()?;
        budget.check()?;
        Ok(result)
    }

    fn recheck_hierarchy(&self) -> Result<()> {
        for anchor in self.anchors.iter().chain(self.unit_parents.iter()) {
            anchor.recheck()?;
        }
        let parent = self.anchors.last().ok_or(())?;
        cgroup2(&parent.file)?;
        if let Some(app) = &self.app {
            app.recheck()?;
            cgroup2(&app.file)?;
        } else if optional_directory(&parent.file, "app.slice")?.is_some() {
            return Err(());
        }
        let current = File::from(
            openat(
                &self.unit_parents.last().ok_or(())?.file,
                UNIT,
                OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| ())?,
        );
        if !executable_identity(&self.unit_metadata, &self.unit.metadata().map_err(|_| ())?)
            || !executable_identity(&self.unit_metadata, &current.metadata().map_err(|_| ())?)
        {
            return Err(());
        }
        Ok(())
    }

    pub(super) fn require_empty(&self, budget: &mut Budget) -> Result<()> {
        self.recheck_hierarchy()?;
        if let Some(app) = &self.app {
            observe_child(&app.file, self.uid, budget, cgroup2)?;
        }
        self.recheck_hierarchy()?;
        budget.check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn recovery_cgroup_events_require_recursive_unpopulated_and_unfrozen() {
        assert!(empty_events(b"populated 0\nfrozen 0\n").is_ok());
        assert!(empty_events(b"frozen 0\npopulated 0\n").is_ok());
        for bytes in [
            b"populated 1\nfrozen 0\n".as_slice(),
            b"populated 0\nfrozen 1\n",
            b"populated 0\n",
            b"populated 0\nfrozen 0",
            b"populated 0\nfrozen 0\nunknown 0\n",
            b"populated 0\npopulated 0\nfrozen 0\n",
        ] {
            assert!(empty_events(bytes).is_err());
        }
        assert!(empty_events(&vec![b'x'; EVENTS_CAP + 1]).is_err());
    }

    #[test]
    fn recovery_cgroup_fixed_paths_cannot_target_another_unit() {
        assert_eq!(
            manager_group(1001),
            "/user.slice/user-1001.slice/user@1001.service"
        );
        assert_eq!(
            runtime_group(1001),
            "/user.slice/user-1001.slice/user@1001.service/app.slice/omavless-runtime.service"
        );
    }

    #[test]
    fn recovery_cgroup_child_absence_empty_population_and_unsafe_nodes() {
        let temp = tempfile::tempdir().unwrap();
        let parent = File::open(temp.path()).unwrap();
        let uid = parent.metadata().unwrap().uid();
        let check = || observe_child(&parent, uid, &mut Budget::new(), |_| Ok(()));
        assert!(check().is_ok());
        let child = temp.path().join(UNIT);
        fs::create_dir(&child).unwrap();
        fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(check().is_err()); // Present but unreadable/missing events is not absence.
        fs::write(child.join("cgroup.events"), b"populated 0\nfrozen 0\n").unwrap();
        assert!(check().is_ok());
        fs::create_dir(child.join("descendant")).unwrap();
        fs::write(child.join("cgroup.events"), b"populated 1\nfrozen 0\n").unwrap();
        assert!(check().is_err()); // Kernel populated includes descendant membership.
        fs::write(child.join("cgroup.events"), b"populated 0\nfrozen 0\n").unwrap();
        fs::set_permissions(&child, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(check().is_err());
        fs::rename(&child, temp.path().join("other")).unwrap();
        symlink(temp.path().join("other"), &child).unwrap();
        assert!(check().is_err());
    }

    #[test]
    fn recovery_cgroup_substitution_during_observation_refuses() {
        let temp = tempfile::tempdir().unwrap();
        let child = temp.path().join(UNIT);
        fs::create_dir(&child).unwrap();
        fs::write(child.join("cgroup.events"), b"populated 0\nfrozen 0\n").unwrap();
        let parent = File::open(temp.path()).unwrap();
        let uid = parent.metadata().unwrap().uid();
        let moved = Cell::new(false);
        assert!(
            observe_child(&parent, uid, &mut Budget::new(), |_| {
                if !moved.replace(true) {
                    fs::rename(&child, temp.path().join("old")).unwrap();
                    fs::create_dir(&child).unwrap();
                    fs::write(child.join("cgroup.events"), b"populated 0\nfrozen 0\n").unwrap();
                }
                Ok(())
            })
            .is_err()
        );
        assert!(observe_child(&parent, uid, &mut Budget::new(), |_| Err(())).is_err());
    }

    #[test]
    fn recovery_cgroup_pinned_parent_refuses_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("app.slice");
        fs::create_dir(&path).unwrap();
        let file = File::open(&path).unwrap();
        let uid = file.metadata().unwrap().uid();
        let anchor = Anchor::new(path.clone(), file, uid).unwrap();
        assert!(anchor.recheck().is_ok());
        fs::rename(&path, temp.path().join("old")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(anchor.recheck().is_err());
    }
}
