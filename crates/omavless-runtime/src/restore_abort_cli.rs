// SPDX-License-Identifier: MIT

//! Explicit stopped-runtime rollback only. Success does not retire the fence.

use crate::RuntimePaths;
use crate::backup_source_candidate::open_private_directory;
use crate::restore_staging_candidate::{same_directory, same_member};
use nix::fcntl::{Flock, FlockArg, OFlag, openat};
use nix::sys::stat::Mode;
use nix::unistd::Uid;
use serde::{Deserialize, Deserializer};
use std::cell::Cell;
use std::ffi::OsString;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use zeroize::Zeroizing;

const MAX_INPUT: usize = 32 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    RuntimeNotStoppedOrUnsafe,
    RecoveryRefused,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Invalid private restore-abort input",
            Self::RuntimeNotStoppedOrUnsafe => {
                "Restore abort requires an existing safe, stopped runtime"
            }
            Self::RecoveryRefused => {
                "Restore abort refused; preserve recovery artifacts and do not retry automatically"
            }
        })
    }
}

// Deliberately neither Debug nor Clone. This clears retained buffers, not every
// temporary allocation made internally by the JSON parser.
struct PrivateText(Zeroizing<String>);

impl<'de> Deserialize<'de> for PrivateText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|text| Self(Zeroizing::new(text)))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: u8,
    archive: PrivateText,
    passphrase: PrivateText,
}

fn parse(input: impl Read) -> Result<Request, Error> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::InvalidInput)?;
    if bytes.len() > MAX_INPUT {
        return Err(Error::InvalidInput);
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidInput)?;
    let archive = request.archive.0.as_str();
    if request.schema != 1
        || archive.len() > 4096
        || archive.contains('\0')
        || !Path::new(archive).is_absolute()
        || Path::new(archive)
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || !(12..=1024).contains(&request.passphrase.0.len())
    {
        return Err(Error::InvalidInput);
    }
    Ok(request)
}

pub fn arguments_admitted(arguments: &[OsString]) -> bool {
    arguments == ["restore", "abort", "--confirm-rollback"]
}

struct StoppedRuntime {
    paths: RuntimePaths,
    uid: u32,
    directory: File,
    directory_identity: Metadata,
    lock: Flock<File>,
    identity: Metadata,
    refused: Cell<bool>,
}

fn open_lock(directory: &File) -> Result<File, Error> {
    openat(
        directory,
        crate::OWNER_LOCK_NAME,
        OFlag::O_RDWR | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| Error::RuntimeNotStoppedOrUnsafe)
}

impl StoppedRuntime {
    fn acquire(paths: RuntimePaths, uid: u32) -> Result<Self, Error> {
        let refuse = || Error::RuntimeNotStoppedOrUnsafe;
        let directory = open_private_directory(&paths.directory, uid).map_err(|_| refuse())?;
        let directory_identity = directory.metadata().map_err(|_| refuse())?;
        let file = open_lock(&directory)?;
        let identity = file.metadata().map_err(|_| refuse())?;
        if !identity.is_file()
            || identity.uid() != uid
            || identity.mode() & 0o7777 != 0o600
            || identity.nlink() != 1
            || identity.len() != 0
        {
            return Err(refuse());
        }
        let lock = Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|_| refuse())?;
        let held = Self {
            paths,
            uid,
            directory,
            directory_identity,
            lock,
            identity,
            refused: Cell::new(false),
        };
        if !held.recheck() {
            return Err(refuse());
        }
        Ok(held)
    }

    fn recheck(&self) -> bool {
        if self.refused.get() {
            return false;
        }
        let valid = (|| {
            let current = open_private_directory(&self.paths.directory, self.uid).ok()?;
            let named = open_lock(&current).ok()?;
            Some(
                same_directory(&self.directory_identity, &self.directory.metadata().ok()?)
                    && same_directory(&self.directory_identity, &current.metadata().ok()?)
                    && same_member(&self.identity, &self.lock.metadata().ok()?)
                    && same_member(&self.identity, &named.metadata().ok()?),
            )
        })()
        .unwrap_or(false);
        self.refused.set(!valid);
        valid
    }
}

/// Input must come from private stdin, never command arguments or environment.
/// No service stop/start, socket removal, lock creation or fence retirement.
pub fn abort_from_private_input(input: impl Read) -> Result<(), Error> {
    let request = parse(input)?;
    let uid = Uid::current();
    if uid.is_root() || uid != Uid::effective() {
        return Err(Error::RuntimeNotStoppedOrUnsafe);
    }
    let paths = RuntimePaths::current().map_err(|_| Error::RuntimeNotStoppedOrUnsafe)?;
    let stopped = StoppedRuntime::acquire(paths, uid.as_raw())?;
    crate::production_owner::abort_first_restore_current(
        Path::new(request.archive.0.as_str()),
        request.passphrase.0.as_bytes(),
        || stopped.recheck(),
    )
    .map_err(|_| Error::RecoveryRefused)?;
    if !stopped.recheck() {
        return Err(Error::RecoveryRefused);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};

    fn valid() -> Vec<u8> {
        br#"{"schema":1,"archive":"/private/archive","passphrase":"twelve-bytes-secret"}"#.to_vec()
    }

    #[test]
    fn private_document_is_strict_and_bounded() {
        assert!(parse(valid().as_slice()).is_ok());
        for bad in [
            r#"{"schema":true,"archive":"/a","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"schema":1,"archive":"/a","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a","archive":"/b","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a","passphrase":"twelve-bytes-secret","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a","passphrase":"twelve-bytes-secret","extra":0}"#,
            r#"{"schema":1,"archive":"relative","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a/../b","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a\u0000","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a","passphrase":false}"#,
            r#"{"schema":1,"archive":"/a","passphrase":"short"}"#,
            r#"{"schema":2,"archive":"/a","passphrase":"twelve-bytes-secret"}"#,
            r#"{"schema":1,"archive":"/a","passphrase":"twelve-bytes-secret"} {}"#,
            "[]",
            "null",
            "",
        ] {
            assert!(parse(bad.as_bytes()).is_err());
        }
        for (archive, passphrase) in [
            ("/a".to_owned(), "x".repeat(1025)),
            (format!("/{}", "a".repeat(4096)), "x".repeat(12)),
        ] {
            let input = serde_json::json!({"schema":1,"archive":archive,"passphrase":passphrase})
                .to_string();
            assert!(parse(input.as_bytes()).is_err());
        }
        let mut oversized = valid();
        oversized.resize(MAX_INPUT + 1, b' ');
        assert!(parse(oversized.as_slice()).is_err());
        struct Failed;
        impl Read for Failed {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("private I/O detail"))
            }
        }
        assert!(matches!(parse(Failed), Err(Error::InvalidInput)));
        assert!(
            !Error::InvalidInput
                .to_string()
                .contains("private I/O detail")
        );
    }

    #[test]
    fn explicit_exact_arguments_only() {
        let args = |words: &[&str]| words.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(arguments_admitted(&args(&[
            "restore",
            "abort",
            "--confirm-rollback"
        ])));
        for words in [
            &["restore", "abort"][..],
            &["restore", "abort", "--yes"],
            &["restore", "abort", "--confirm-rollback", "secret"],
            &["restore", "--confirm-rollback", "abort"],
        ] {
            assert!(!arguments_admitted(&args(words)));
        }
    }

    fn fixture() -> (tempfile::TempDir, RuntimePaths, u32) {
        let root = tempfile::Builder::new()
            .prefix("ov-abort-lock-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let paths = RuntimePaths::below(root.path());
        fs::create_dir(&paths.directory).unwrap();
        fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&paths.owner_lock)
            .unwrap();
        (root, paths, Uid::current().as_raw())
    }

    #[test]
    fn existing_lock_excludes_normal_owner_and_preserves_socket() {
        let (_root, paths, uid) = fixture();
        fs::write(&paths.socket, b"retained socket placeholder").unwrap();
        let held = StoppedRuntime::acquire(paths.clone(), uid).unwrap();
        assert!(held.recheck());
        assert!(StoppedRuntime::acquire(paths.clone(), uid).is_err());
        assert!(crate::OwnerLock::acquire(&paths.owner_lock, uid).is_err());
        drop(held);
        let owner = crate::OwnerLock::acquire(&paths.owner_lock, uid).unwrap();
        assert!(StoppedRuntime::acquire(paths.clone(), uid).is_err());
        drop(owner);
        assert_eq!(
            fs::read(&paths.socket).unwrap(),
            b"retained socket placeholder"
        );
    }

    #[test]
    fn existing_lock_refuses_missing_unsafe_or_nonregular_without_repair() {
        for case in 0..6 {
            let (root, paths, uid) = fixture();
            match case {
                0 => fs::remove_file(&paths.owner_lock).unwrap(),
                1 => fs::set_permissions(&paths.owner_lock, fs::Permissions::from_mode(0o644))
                    .unwrap(),
                2 => fs::write(&paths.owner_lock, b"not empty").unwrap(),
                3 => fs::hard_link(&paths.owner_lock, root.path().join("alias")).unwrap(),
                4 => {
                    fs::remove_file(&paths.owner_lock).unwrap();
                    symlink("missing", &paths.owner_lock).unwrap();
                }
                _ => {
                    fs::remove_file(&paths.owner_lock).unwrap();
                    fs::create_dir(&paths.owner_lock).unwrap();
                }
            }
            assert!(StoppedRuntime::acquire(paths.clone(), uid).is_err());
            if case == 0 {
                assert!(!paths.owner_lock.exists());
            }
            if case == 1 {
                assert_eq!(
                    fs::metadata(&paths.owner_lock).unwrap().mode() & 0o777,
                    0o644
                );
            }
        }
    }

    #[test]
    fn retained_lock_and_parent_replacement_poison_admission() {
        for replace_parent in [false, true] {
            let (root, paths, uid) = fixture();
            let held = StoppedRuntime::acquire(paths.clone(), uid).unwrap();
            let saved = root.path().join("retained-original");
            if replace_parent {
                fs::rename(&paths.directory, &saved).unwrap();
                fs::create_dir(&paths.directory).unwrap();
                fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
            } else {
                fs::rename(&paths.owner_lock, &saved).unwrap();
            }
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&paths.owner_lock)
                .unwrap();
            assert!(!held.recheck());
            if replace_parent {
                fs::remove_file(&paths.owner_lock).unwrap();
                fs::remove_dir(&paths.directory).unwrap();
                fs::rename(saved, &paths.directory).unwrap();
            } else {
                fs::remove_file(&paths.owner_lock).unwrap();
                fs::rename(saved, &paths.owner_lock).unwrap();
            }
            assert!(
                !held.recheck(),
                "a failed identity observation is permanent"
            );
        }
    }
}
