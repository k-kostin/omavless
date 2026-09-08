// SPDX-License-Identifier: MIT

//! Test-only allocation that never reuses existing files or directories.

use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
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
