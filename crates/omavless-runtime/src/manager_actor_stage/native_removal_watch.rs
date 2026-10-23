// SPDX-License-Identifier: MIT
//! Exact persistent unload-watch comparison data, never removal authority.
use super::*;

const NAME: &str = "plugin-removal.lock";
// Native recovery never imports an actor-manager image. This distinct role
// stays inside the existing 36-role budget; it is not a recycled scratch or
// current-store role (ManagerLib). It remains retained on refusal/unwind.
const ROLE: Slot = Slot::ManagerImage;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RemovalWatch {
    Unselected,
    Absent,
    Retained,
}

impl NativeEngine {
    pub(super) fn capture_recovery_removal_watch(
        &mut self,
        until: Instant,
    ) -> Result<(), FirstError> {
        if !self.singleton_locked || self.recovery_removal_watch != RemovalWatch::Unselected {
            return Err(FirstError::Admission);
        }
        let root = self
            .lower
            .io
            .original(Slot::Root)
            .map_err(|_| FirstError::Admission)?;
        match fstatat(root, NAME, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Err(nix::errno::Errno::ENOENT) => {
                self.recovery_removal_watch = RemovalWatch::Absent;
            }
            Ok(_) => {
                self.lower
                    .io
                    .child(
                        ChildPlan {
                            parent: Slot::Root,
                            slot: ROLE,
                            name: NAME,
                            flags: OFlag::O_RDONLY | OFlag::O_NONBLOCK,
                            mode: Mode::empty(),
                        },
                        || tick(until),
                        |_| Ok(()),
                    )
                    .map_err(|_| FirstError::Admission)?;
                self.shape(ROLE, false, until)
                    .map_err(|_| FirstError::Admission)?;
                if self
                    .lower
                    .io
                    .original(ROLE)
                    .map_err(|_| FirstError::Admission)?
                    .metadata()
                    .map_err(|_| FirstError::Admission)?
                    .len()
                    != 0
                {
                    return Err(FirstError::Admission);
                }
                self.lower
                    .binding(Slot::Root, ROLE, NAME, false, until)
                    .map_err(|_| FirstError::Admission)?;
                self.expected[ROLE as usize] = Some((0, Sha256::digest([]).into()));
                self.recovery_removal_watch = RemovalWatch::Retained;
            }
            Err(_) => return Err(FirstError::Admission),
        }
        self.check_recovery_removal_watch(until)
            .map_err(|_| FirstError::Admission)
    }

    fn check_recovery_removal_watch(&mut self, until: Instant) -> Result<(), Unavailable> {
        tick(until)?;
        if self.unlinked[ROLE as usize] {
            return Err(Unavailable);
        }
        match self.recovery_removal_watch {
            RemovalWatch::Unselected => Err(Unavailable),
            RemovalWatch::Absent => {
                if self.lower.original[ROLE as usize].is_some()
                    || self.lower.io.original(ROLE).is_ok()
                    || self.expected[ROLE as usize].is_some()
                    || !matches!(
                        fstatat(
                            self.lower.io.original(Slot::Root)?,
                            NAME,
                            AtFlags::AT_SYMLINK_NOFOLLOW
                        ),
                        Err(nix::errno::Errno::ENOENT)
                    )
                {
                    return Err(Unavailable);
                }
                tick(until)
            }
            RemovalWatch::Retained => {
                if self.expected[ROLE as usize] != Some((0, Sha256::digest([]).into())) {
                    return Err(Unavailable);
                }
                self.lower.binding(Slot::Root, ROLE, NAME, false, until)?;
                let original = self.lower.original[ROLE as usize]
                    .as_ref()
                    .ok_or(Unavailable)?;
                self.lower.io.perform(
                    ROLE,
                    || tick(until),
                    |file| {
                        let current = file.metadata().map_err(|_| Unavailable)?;
                        let mut attributes = [0; 1];
                        if !same_member(original, &current)
                            || original.gid() != current.gid()
                            || current.len() != 0
                            || current.nlink() != 1
                            || flistxattr(file, &mut attributes).map_err(|_| Unavailable)? != 0
                        {
                            return Err(Unavailable);
                        }
                        Ok(())
                    },
                )
            }
        }
    }

    pub(super) fn check_recovery_catalogue(
        &mut self,
        socket_absent: bool,
        until: Instant,
    ) -> Result<(), Unavailable> {
        self.check_recovery_removal_watch(until)?;
        let expected: &[&str] = match (self.recovery_removal_watch, socket_absent) {
            (RemovalWatch::Absent, true) => &[crate::OWNER_LOCK_NAME],
            (RemovalWatch::Absent, false) => &[crate::OWNER_LOCK_NAME, crate::SOCKET_NAME],
            (RemovalWatch::Retained, true) => &[crate::OWNER_LOCK_NAME, NAME],
            (RemovalWatch::Retained, false) => &[crate::OWNER_LOCK_NAME, NAME, crate::SOCKET_NAME],
            (RemovalWatch::Unselected, _) => return Err(Unavailable),
        };
        self.lower.catalogue_inner(Slot::Root, expected, until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    #[test]
    fn actual_singleton_capture_preserves_only_the_exact_original_watch_shape() {
        for case in [
            "absent",
            "persistent",
            "socket",
            "held-watch",
            "extra",
            "public",
            "nonempty",
            "symlink",
            "hardlink",
            "directory",
            "replaced",
            "removed",
            "grew",
            "late-watch",
            "late-extra",
            "deadline",
            "unwind",
        ] {
            let root = crate::test_temp::directory_under(
                &crate::test_temp::home_parent().unwrap(),
                "watch",
            )
            .unwrap();
            let paths = crate::RuntimePaths::below(&root);
            let server = crate::RuntimeServer::bind(paths.clone()).unwrap();
            let named = paths.directory.join(NAME);
            let watch = if matches!(case, "absent" | "late-watch") {
                None
            } else if case == "directory" {
                fs::create_dir(&named).unwrap();
                None
            } else if case == "symlink" {
                let target = root.join("synthetic-target");
                fs::write(&target, b"").unwrap();
                symlink(target, &named).unwrap();
                None
            } else {
                let file = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&named)
                    .unwrap();
                match case {
                    "public" => {
                        fs::set_permissions(&named, fs::Permissions::from_mode(0o644)).unwrap()
                    }
                    "nonempty" => fs::write(&named, b"not empty").unwrap(),
                    "hardlink" => fs::hard_link(&named, root.join("second-name")).unwrap(),
                    "held-watch" => rustix::fs::flock(
                        &file,
                        rustix::fs::FlockOperation::NonBlockingLockExclusive,
                    )
                    .unwrap(),
                    _ => {}
                }
                Some(file)
            };
            let before = fs::symlink_metadata(&named).ok();
            if case == "extra" {
                fs::write(paths.directory.join("foreign.lock"), b"").unwrap();
            }
            drop(server);
            let socket = if case == "socket" {
                let socket = UnixListener::bind(&paths.socket).unwrap();
                fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
                Some(socket)
            } else {
                None
            };
            let mut engine = NativeEngine::reserve();
            engine.uid = Some(nix::unistd::getuid().as_raw());
            engine.gid = Some(nix::unistd::getgid().as_raw());
            engine.recovery = true;
            engine.lower.io.native_admit().unwrap();
            let until = Instant::now() + std::time::Duration::from_secs(10);
            engine
                .clone_file(Slot::Run, &File::open(&root).unwrap(), true, until)
                .unwrap();
            let capture = engine.capture_recovery_singleton(until);
            if matches!(
                case,
                "extra" | "public" | "nonempty" | "symlink" | "hardlink" | "directory"
            ) {
                assert!(capture.is_err(), "unsafe capture: {case}");
                assert!(engine.lower.io.test_retains_original(Slot::Root));
                assert!(engine.lower.io.test_retains_original(Slot::Lock));
                if matches!(
                    case,
                    "extra" | "public" | "nonempty" | "hardlink" | "directory"
                ) {
                    assert!(
                        engine.lower.io.test_retains_original(ROLE),
                        "lost reported FD: {case}"
                    );
                }
                let after = fs::symlink_metadata(&named).unwrap();
                assert!(
                    same_member(before.as_ref().unwrap(), &after),
                    "changed refused target: {case}"
                );
            } else {
                capture.unwrap_or_else(|_| panic!("valid capture refused: {case}"));
                engine.check_bindings(until).unwrap();
                engine.check_bindings(until).unwrap();
                engine.check_original_bytes(until).unwrap();
                if !matches!(case, "absent" | "late-watch") {
                    assert!(engine.lower.io.test_retains_original(ROLE));
                    assert!(same_member(
                        before.as_ref().unwrap(),
                        &watch.as_ref().unwrap().metadata().unwrap()
                    ));
                }
                match case {
                    "replaced" => {
                        fs::rename(&named, root.join("retained-original")).unwrap();
                        fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .mode(0o600)
                            .open(&named)
                            .unwrap();
                    }
                    "removed" => fs::remove_file(&named).unwrap(),
                    "grew" => fs::write(&named, b"changed").unwrap(),
                    "late-watch" => {
                        fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .mode(0o600)
                            .open(&named)
                            .unwrap();
                    }
                    "late-extra" => fs::write(paths.directory.join("foreign.lock"), b"").unwrap(),
                    "unwind" => {
                        assert!(
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                engine.check_bindings(until).unwrap();
                                panic!("synthetic caller unwind");
                            }))
                            .is_err()
                        );
                        assert!(engine.lower.io.test_retains_original(ROLE));
                        assert!(same_member(
                            before.as_ref().unwrap(),
                            &fs::symlink_metadata(&named).unwrap()
                        ));
                    }
                    _ => {}
                }
                let check_until = if case == "deadline" {
                    Instant::now()
                } else {
                    until
                };
                let checked = engine.check_recovery_singleton(check_until);
                assert_eq!(
                    checked.is_err(),
                    matches!(
                        case,
                        "replaced" | "removed" | "grew" | "late-watch" | "late-extra" | "deadline"
                    ),
                    "late check: {case}"
                );
                if case == "replaced" || case == "removed" {
                    assert!(engine.lower.io.test_retains_original(ROLE));
                }
            }
            drop((engine, socket, watch));
            fs::remove_dir_all(root).unwrap(); // only these completed synthetic objects
        }
    }
}
