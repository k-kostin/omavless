// SPDX-License-Identifier: MIT

//! Test-only allocation that never reuses existing files or directories.

use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ATTEMPTS: usize = 128;
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Caller owns cleanup after its threads/children finish. No clock is needed.
/// Short names leave space for the Linux Unix-socket path limit.
pub fn directory(label: &str) -> io::Result<PathBuf> {
    allocate(&std::env::temp_dir(), label, std::process::id(), || {
        NEXT.fetch_add(1, Ordering::Relaxed)
    })
}

/// For tests whose production path policy deliberately refuses world-writable
/// ancestors such as /tmp. The caller must select a trusted private or
/// non-writable parent; the allocator keeps the same collision guarantees.
// This shared fixture module is included by crates that do not need this path.
#[allow(dead_code)]
pub fn directory_under(parent: &Path, label: &str) -> io::Result<PathBuf> {
    // Private-path tests historically selected HOME directly to avoid /tmp.
    // Keep that trusted ancestry, but never scatter fixtures in HOME's root.
    let scoped_parent = match std::env::var_os("HOME") {
        Some(home) if parent == Path::new(&home) => Some(private_parent(parent)?),
        _ => None,
    };
    allocate(
        scoped_parent.as_deref().unwrap_or(parent),
        label,
        std::process::id(),
        || NEXT.fetch_add(1, Ordering::Relaxed),
    )
}

/// A short HOME-backed parent for private-path tempfile fixtures. Existing
/// directories are checked, never chmod'ed; symlinks/writable parents refuse.
#[allow(dead_code)]
pub fn home_parent() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Test HOME missing"))?;
    private_parent(Path::new(&home))
}

fn private_parent(home: &Path) -> io::Result<PathBuf> {
    let metadata = fs::symlink_metadata(home)?;
    if !metadata.is_dir() || metadata.mode() & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Unsafe test HOME",
        ));
    }
    let uid = metadata.uid();
    let mut parent = home.to_path_buf();
    for component in [".cache", "ovt"] {
        parent.push(component);
        match fs::DirBuilder::new().mode(0o700).create(&parent) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(&parent)?;
        if !metadata.is_dir()
            || metadata.uid() != uid
            || metadata.mode() & 0o022 != 0
            || (component == "ovt" && metadata.mode() & 0o777 != 0o700)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unsafe private test parent",
            ));
        }
    }
    Ok(parent)
}

fn allocate(
    parent: &Path,
    label: &str,
    pid: u32,
    mut next: impl FnMut() -> u64,
) -> io::Result<PathBuf> {
    if label.is_empty()
        || label.len() > 24
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid test directory label",
        ));
    }
    for _ in 0..ATTEMPTS {
        let path = parent.join(format!("ovt-{label}-{pid}-{:x}", next()));
        match fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Test directory allocation exhausted",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn explicit_home_allocation_uses_scoped_parent_not_home_root() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let child = directory_under(&home, "home-scoped").unwrap();
        assert_eq!(child.parent(), Some(home.join(".cache/ovt").as_path()));
        fs::remove_dir(child).unwrap();
    }

    #[test]
    fn home_fixtures_are_nested_and_existing_permissions_are_preserved() {
        let home = directory("home-parent").unwrap();
        fs::DirBuilder::new()
            .mode(0o755)
            .create(home.join(".cache"))
            .unwrap();
        fs::set_permissions(home.join(".cache"), fs::Permissions::from_mode(0o755)).unwrap();
        let parent = private_parent(&home).unwrap();
        assert_eq!(parent, home.join(".cache/ovt"));
        assert_eq!(fs::metadata(&parent).unwrap().mode() & 0o777, 0o700);
        assert_eq!(
            fs::metadata(home.join(".cache")).unwrap().mode() & 0o777,
            0o755
        );
        let child = directory_under(&parent, "restore").unwrap();
        assert_eq!(child.parent(), Some(parent.as_path()));
        assert_eq!(private_parent(&home).unwrap(), parent);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn home_fixture_parent_refuses_symlinks_and_writable_nodes_without_repair() {
        for cut in [
            "home-link",
            "cache-link",
            "cache-writable",
            "ovt-link",
            "ovt-public",
        ] {
            let scope = directory("home-refusal").unwrap();
            let home = scope.join("home");
            let target = scope.join("target");
            fs::DirBuilder::new().mode(0o700).create(&target).unwrap();
            fs::write(target.join("sentinel"), b"unchanged").unwrap();
            if cut == "home-link" {
                symlink(&target, &home).unwrap();
            } else {
                fs::DirBuilder::new().mode(0o700).create(&home).unwrap();
                if cut == "cache-link" {
                    symlink(&target, home.join(".cache")).unwrap();
                } else {
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(home.join(".cache"))
                        .unwrap();
                    match cut {
                        "cache-writable" => fs::set_permissions(
                            home.join(".cache"),
                            fs::Permissions::from_mode(0o777),
                        )
                        .unwrap(),
                        "ovt-link" => symlink(&target, home.join(".cache/ovt")).unwrap(),
                        "ovt-public" => {
                            fs::DirBuilder::new()
                                .mode(0o755)
                                .create(home.join(".cache/ovt"))
                                .unwrap();
                            fs::set_permissions(
                                home.join(".cache/ovt"),
                                fs::Permissions::from_mode(0o755),
                            )
                            .unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            assert_eq!(
                private_parent(&home).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
            assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"unchanged");
            if cut == "cache-writable" {
                assert_eq!(
                    fs::metadata(home.join(".cache")).unwrap().mode() & 0o777,
                    0o777
                );
            }
            fs::remove_dir_all(scope).unwrap();
        }
    }

    #[test]
    fn parallel_allocations_are_unique_private_and_independent() {
        let workers: Vec<_> = (0..32)
            .map(|_| {
                std::thread::spawn(|| {
                    let root = directory("parallel").unwrap();
                    assert_eq!(
                        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                        0o700
                    );
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(root.join("owned"))
                        .unwrap();
                    root
                })
            })
            .collect();
        let roots: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(roots.iter().collect::<BTreeSet<_>>().len(), 32);
        for root in roots {
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn collisions_and_symlinks_are_skipped_without_touching_targets() {
        let root = directory("collision").unwrap();
        let original = root.join("protected");
        fs::write(&original, b"unchanged").unwrap();
        let occupied = root.join("ovt-case-7-0");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("sentinel"), b"unchanged").unwrap();
        symlink(&original, root.join("ovt-case-7-1")).unwrap();
        symlink(root.join("missing"), root.join("ovt-case-7-2")).unwrap();
        let mut sequence = 0;
        let allocated = allocate(&root, "case", 7, || {
            let value = sequence;
            sequence += 1;
            value
        })
        .unwrap();
        assert_eq!(allocated, root.join("ovt-case-7-3"));
        fs::write(allocated.join("candidate"), b"different").unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"unchanged");
        assert_eq!(fs::read(occupied.join("sentinel")).unwrap(), b"unchanged");
        assert!(!root.join("missing").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collisions_are_bounded_and_labels_cannot_escape_parent() {
        let root = directory("bounds").unwrap();
        fs::create_dir(root.join("ovt-case-7-0")).unwrap();
        let mut calls = 0;
        let result = allocate(&root, "case", 7, || {
            calls += 1;
            0
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(calls, ATTEMPTS);
        for label in [
            "",
            "../escape",
            "/absolute",
            "space label",
            "abcdefghijklmnopqrstuvwxyz",
        ] {
            assert_eq!(
                allocate(&root, label, 7, || panic!(
                    "invalid label reached allocator"
                ))
                .unwrap_err()
                .kind(),
                io::ErrorKind::InvalidInput
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
