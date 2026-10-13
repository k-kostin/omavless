// SPDX-License-Identifier: MIT
//! Linux-only source-stage reader for a selected OWNED public fixture.
//! No script path arguments, ambient theme loader, writes or watcher API.
//! NOT registered in either candidate. Host integration/review remains pending.
use crate::theme_source::{MAX_THEME_BYTES, Prepared, Status};
use std::os::unix::{
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawFd,
};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

// Linux asm-generic/fcntl.h; x86_64/AArch64. No unsafe syscall/new dependency.
const O_NONBLOCK: i32 = 0x800;
const O_DIRECTORY: i32 = 0x10000;
const O_NOFOLLOW: i32 = 0x20000;
const MAX_LINKS: usize = 8;
const MAX_DEPTH: usize = 8;
const MAX_STEPS: usize = 64;

fn unavailable() -> Prepared {
    Prepared::fallback(Status::Unavailable)
}
fn fd_path(file: &File, component: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd())).join(component)
}
fn open(path: &Path, directory: bool) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW | if directory { O_DIRECTORY } else { 0 })
        .open(path)
}

pub struct OwnedFixture {
    root: File,
    uid: u32,
}
impl OwnedFixture {
    pub fn has_trial_manifest(&self) -> bool {
        let Ok(mut file) = open(&fd_path(&self.root, "gpui-shell.json"), false) else {
            return false;
        };
        let Ok(metadata) = file.metadata() else {
            return false;
        };
        let expected = include_bytes!("shell/gpui-shell.json");
        if !metadata.is_file()
            || metadata.uid() != self.uid
            || metadata.mode() & 0o022 != 0
            || metadata.len() > expected.len() as u64
        {
            return false;
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take((expected.len() + 1) as u64)
            .read_to_end(&mut bytes)
            .is_ok()
            && bytes == expected
    }
    // Trusted constructor only. Future host supplies a fresh synthetic HOME,
    // not a caller-provided script path. Capture/recheck the actual directory.
    pub fn open_root(root: &Path) -> std::io::Result<Self> {
        let uid = fs::metadata("/proc/self")?.uid();
        let before = fs::symlink_metadata(root)?;
        if !before.is_dir() || before.uid() != uid || before.mode() & 0o777 != 0o700 {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let file = open(root, true)?;
        let after = file.metadata()?;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        Ok(Self { root: file, uid })
    }

    pub fn sample(&self) -> Prepared {
        // Directory handles, not canonicalize-then-open strings, anchor each
        // next component. O_NOFOLLOW rejects a symlink substituted after the
        // link read; the retained borrowed parent cannot turn into another FD.
        let Ok(root) = self.root.try_clone() else {
            return unavailable();
        };
        let mut stack = vec![root];
        let mut pending = VecDeque::from([
            "current".to_owned(),
            "theme".to_owned(),
            "colors.toml".to_owned(),
        ]);
        let mut links = 0;
        for _ in 0..MAX_STEPS {
            let Some(component) = pending.pop_front() else {
                return unavailable();
            };
            if component == "." {
                continue;
            }
            if component == ".." {
                if stack.len() <= 1 {
                    return unavailable();
                }
                stack.pop();
                continue;
            }
            if component.len() > 64
                || !component
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
            {
                return unavailable();
            }
            let candidate = fd_path(stack.last().unwrap(), &component);
            match fs::read_link(&candidate) {
                Ok(target) => {
                    links += 1;
                    if links > MAX_LINKS || target.as_os_str().len() > 256 || target.is_absolute() {
                        return unavailable();
                    }
                    let mut parts = Vec::new();
                    for part in target.components() {
                        parts.push(match part {
                            Component::CurDir => ".".into(),
                            Component::ParentDir => "..".into(),
                            Component::Normal(part) => match part.to_str() {
                                Some(part) => part.to_owned(),
                                None => return unavailable(),
                            },
                            _ => return unavailable(),
                        });
                    }
                    if parts.is_empty() {
                        return unavailable();
                    }
                    for part in parts.into_iter().rev() {
                        pending.push_front(part);
                    }
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Prepared::fallback(Status::Missing);
                }
                Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => (),
                Err(_) => return unavailable(),
            }
            let final_file = pending.is_empty();
            let mut file = match open(&candidate, !final_file) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Prepared::fallback(Status::Missing);
                }
                Err(_) => return unavailable(),
            };
            let Ok(metadata) = file.metadata() else {
                return unavailable();
            };
            if metadata.uid() != self.uid || metadata.mode() & 0o022 != 0 {
                return unavailable();
            }
            if final_file {
                if !metadata.is_file() {
                    return unavailable();
                }
                if metadata.len() > MAX_THEME_BYTES as u64 {
                    return Prepared::fallback(Status::Oversize);
                }
                let mut bytes = Vec::new();
                if file
                    .by_ref()
                    .take((MAX_THEME_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .is_err()
                {
                    return unavailable();
                }
                return Prepared::from_bytes(Some(&bytes));
            }
            if !metadata.is_dir() || stack.len() >= MAX_DEPTH {
                return unavailable();
            }
            stack.push(file);
        }
        unavailable()
    }
}

pub struct Sampler {
    fixture: OwnedFixture,
    busy: AtomicBool,
    sequence: AtomicU64,
}
pub struct Permit {
    sampler: Arc<Sampler>,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.sampler.busy.store(false, Ordering::Release);
    }
}
impl Sampler {
    pub fn new(fixture: OwnedFixture) -> Arc<Self> {
        Arc::new(Self {
            fixture,
            busy: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
        })
    }
    pub fn reserve(self: &Arc<Self>) -> Option<Permit> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        Some(Permit {
            sampler: Arc::clone(self),
        })
    }
}
impl Permit {
    pub fn sample(self) -> Option<(u64, Prepared)> {
        let sequence = self
            .sampler
            .sequence
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < crate::theme_source::MAX_THEME_SEQUENCE).then_some(current + 1)
            })
            .ok()?
            + 1;
        Some((sequence, self.sampler.fixture.sample()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, symlink};
    fn fresh(label: &str) -> PathBuf {
        let parent = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cache/ovt");
        let metadata = fs::symlink_metadata(&parent).unwrap();
        assert!(metadata.is_dir() && metadata.mode() & 0o777 == 0o700);
        assert_eq!(metadata.uid(), fs::metadata("/proc/self").unwrap().uid());
        // Existing shared test-allocator pattern: short create-only attempts,
        // no permission repair and never reuse an existing scope.
        for index in 0..128 {
            let root = parent.join(format!("g1-{label}-{}-{index}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&root) {
                Ok(()) => return root,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(_) => panic!("cannot allocate public fixture"),
            }
        }
        panic!("public fixture allocation bound")
    }
    #[test]
    fn owned_replacement_and_escape_refusal() {
        let root = fresh("theme");
        let public_outside = fresh("other");
        let source = include_str!("shell/palette.js")
            .split_once("export const LIGHT = `")
            .unwrap()
            .1
            .split_once('`')
            .unwrap()
            .0;
        for directory in [&root, &public_outside] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(directory.join("light"))
                .unwrap();
            fs::write(directory.join("light/colors.toml"), source).unwrap();
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(root.join("current"))
            .unwrap();
        symlink("../light", root.join("current/theme")).unwrap();
        let fixture = OwnedFixture::open_root(&root).unwrap();
        assert!(!fixture.has_trial_manifest());
        fs::write(
            root.join("gpui-shell.json"),
            include_bytes!("shell/gpui-shell.json"),
        )
        .unwrap();
        assert!(fixture.has_trial_manifest());
        assert_eq!(fixture.sample().palette.mode(), "light");
        fs::remove_file(root.join("current/theme")).unwrap();
        symlink(public_outside.join("light"), root.join("current/theme")).unwrap();
        assert_eq!(fixture.sample().status, Status::Unavailable);
        fs::remove_file(root.join("current/theme")).unwrap();
        symlink("../../escape", root.join("current/theme")).unwrap();
        assert_eq!(fixture.sample().status, Status::Unavailable);
        fs::remove_file(root.join("current/theme")).unwrap();
        symlink("theme", root.join("current/theme")).unwrap();
        assert_eq!(fixture.sample().status, Status::Unavailable); // bounded loop
        fs::remove_file(root.join("current/theme")).unwrap();
        symlink("../light", root.join("current/theme")).unwrap();
        fs::rename(root.join("current"), root.join("old")).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(root.join("current"))
            .unwrap();
        symlink("../light", root.join("current/theme")).unwrap();
        assert_eq!(fixture.sample().palette.mode(), "light");
        fs::write(root.join("light/colors.toml"), "broken").unwrap();
        assert_eq!(fixture.sample().status, Status::Malformed);
        fs::write(
            root.join("light/colors.toml"),
            vec![b' '; MAX_THEME_BYTES + 1],
        )
        .unwrap();
        assert_eq!(fixture.sample().status, Status::Oversize);
        fs::remove_file(root.join("light/colors.toml")).unwrap();
        assert_eq!(fixture.sample().status, Status::Missing);
        fs::write(root.join("light/colors.toml"), source).unwrap();
        let sampler = Sampler::new(fixture);
        let permit = sampler.reserve().unwrap();
        assert!(sampler.reserve().is_none());
        assert_eq!(permit.sample().unwrap().0, 1);
        assert_eq!(sampler.reserve().unwrap().sample().unwrap().0, 2);
        // Socket/FIFO/etc have no read path. Opening must never await a writer.
        fs::remove_file(root.join("light/colors.toml")).unwrap();
        let socket =
            std::os::unix::net::UnixListener::bind(root.join("light/colors.toml")).unwrap();
        assert_eq!(
            sampler.reserve().unwrap().sample().unwrap().1.status,
            Status::Unavailable
        );
        drop(socket);
        assert!(root.as_os_str().len() < 256);
        drop(sampler);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(public_outside).unwrap();
    }
}
