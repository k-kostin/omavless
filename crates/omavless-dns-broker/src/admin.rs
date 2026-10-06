// SPDX-License-Identifier: MIT

//! One-time, fixed-target root enrollment. This is not a broker IPC method.
//! The ordinary user cannot name a path, network setting, link, or command.

use rustix::fs::{self as rfs, RenameFlags};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

const DIRECTORY: &str = "/etc/omavless-dns";
#[cfg(feature = "release-package")]
const ENROLLMENT: &str = "release-enrollment.json";
#[cfg(not(feature = "release-package"))]
const ENROLLMENT: &str = "enrollment.json";
const STAGING: &str = ".enrollment.pending";
#[cfg(feature = "k1-managed-device")]
const MANAGED_TUN: &str = "/sys/class/net/omavless0";
#[cfg(not(feature = "k1-managed-device"))]
const MANAGED_TUN: &str = "/sys/class/net/Meta";
#[cfg(feature = "release-package")]
const GUARD: &str = "/usr/lib/omavless-dns/package-guard";
#[cfg(not(feature = "release-package"))]
const GUARD: &str = "/usr/lib/omavless-dns-experimental/package-guard";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Enroll(u32),
    Revoke,
}

impl Action {
    #[must_use]
    pub fn enroll(raw: &std::ffi::OsStr) -> Option<Self> {
        let text = raw.to_str()?;
        if text.len() > 10 || text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let uid = text.parse::<u32>().ok()?;
        (uid != 0 && uid != u32::MAX).then_some(Self::Enroll(uid))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Refused,
    OutcomeUnknown,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Refused => "DNS enrollment refused; inspect the empty-state and package gate",
            Self::OutcomeUnknown => "DNS enrollment outcome is unknown; inspect before retrying",
        })
    }
}

impl std::error::Error for Error {}

/// Requires a separately authorized, fixed root invocation. Never start or
/// stop a service, clean a socket, reset DNS, or infer consent from install.
pub fn run(action: Action) -> Result<(), Error> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Refused);
    }
    let guard = || {
        Command::new(GUARD)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    run_at(action, Path::new(DIRECTORY), 0, guard, || {
        matches!(
            fs::symlink_metadata(MANAGED_TUN),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        )
    })
}

fn safe_directory(path: &Path, owner: u32, exact_mode: Option<u32>) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::Refused)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o022 != 0
        || exact_mode.is_some_and(|mode| metadata.permissions().mode() & 0o7777 != mode)
    {
        return Err(Error::Refused);
    }
    Ok(())
}

fn absent(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(Error::Refused),
    }
}

fn safe_enrollment(path: &Path, owner: u32) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::Refused)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != 0o600
        || metadata.len() == 0
        || metadata.len() > 256
    {
        return Err(Error::Refused);
    }
    Ok(())
}

fn run_at(
    action: Action,
    directory: &Path,
    owner: u32,
    guard: impl FnOnce() -> bool,
    no_managed_tun: impl FnOnce() -> bool,
) -> Result<(), Error> {
    let parent = directory.parent().ok_or(Error::Refused)?;
    safe_directory(parent, owner, None)?;
    if !guard() || !no_managed_tun() {
        return Err(Error::Refused);
    }
    match action {
        Action::Enroll(uid) => enroll_at(directory, owner, uid),
        Action::Revoke => revoke_at(directory, owner),
    }
}

fn enroll_at(directory: &Path, owner: u32, uid: u32) -> Result<(), Error> {
    if uid == 0 || uid == u32::MAX {
        return Err(Error::Refused);
    }
    if absent(directory)? {
        DirBuilder::new()
            .mode(0o700)
            .create(directory)
            .map_err(|_| Error::Refused)?;
    }
    safe_directory(directory, owner, Some(0o700))?;
    let target = directory.join(ENROLLMENT);
    let staging = directory.join(STAGING);
    if !absent(&target)? || !absent(&staging)? {
        return Err(Error::Refused);
    }
    let value = format!(
        "{{\"schema\":1,\"uid\":{uid},\"policy\":\"{}\"}}\n",
        crate::ENROLLMENT_POLICY
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&staging)
        .map_err(|_| Error::Refused)?;
    file.write_all(value.as_bytes())
        .map_err(|_| Error::Refused)?;
    file.sync_all().map_err(|_| Error::Refused)?;
    safe_enrollment(&staging, owner)?;
    rfs::renameat_with(
        rfs::CWD,
        &staging,
        rfs::CWD,
        &target,
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| Error::Refused)?;
    File::open(directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::OutcomeUnknown)?;
    Ok(())
}

fn revoke_at(directory: &Path, owner: u32) -> Result<(), Error> {
    safe_directory(directory, owner, Some(0o700))?;
    let target = directory.join(ENROLLMENT);
    let staging = directory.join(STAGING);
    if !absent(&staging)? {
        return Err(Error::Refused);
    }
    safe_enrollment(&target, owner)?;
    fs::remove_file(target).map_err(|_| Error::Refused)?;
    File::open(directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::OutcomeUnknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn fixture() -> (tempfile::TempDir, PathBuf, u32) {
        let root = tempfile::tempdir().unwrap();
        let owner = rustix::process::geteuid().as_raw();
        let directory = root.path().join("omavless-dns");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        (root, directory, owner)
    }

    #[test]
    fn action_vocabulary_is_fixed_numeric_and_nonroot_rejected() {
        for invalid in [
            "",
            "0",
            "4294967295",
            "4294967296",
            "-1",
            "+1000",
            "1x",
            "١٠٠٠",
        ] {
            assert!(Action::enroll(OsStr::new(invalid)).is_none());
        }
        assert_eq!(
            Action::enroll(OsStr::new("1000")),
            Some(Action::Enroll(1000))
        );
        if !rustix::process::geteuid().is_root() {
            assert_eq!(run(Action::Enroll(1000)), Err(Error::Refused));
        }
    }

    #[test]
    fn enroll_once_then_revoke_only_after_empty_gate() {
        let (_root, directory, owner) = fixture();
        assert!(run_at(Action::Enroll(1000), &directory, owner, || true, || true).is_ok());
        let target = directory.join(ENROLLMENT);
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            format!(
                "{{\"schema\":1,\"uid\":1000,\"policy\":\"{}\"}}\n",
                crate::ENROLLMENT_POLICY
            )
        );
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        assert_eq!(
            run_at(Action::Enroll(1001), &directory, owner, || true, || true),
            Err(Error::Refused)
        );
        assert_eq!(
            run_at(Action::Revoke, &directory, owner, || true, || true),
            Ok(())
        );
        assert!(!target.exists());
    }

    #[test]
    fn failed_gates_never_create_or_remove_enrollment() {
        let (_root, directory, owner) = fixture();
        for (guard, no_tun) in [(false, true), (true, false)] {
            assert_eq!(
                run_at(Action::Enroll(1000), &directory, owner, || guard, || no_tun),
                Err(Error::Refused)
            );
            assert!(!directory.exists());
        }
        run_at(Action::Enroll(1000), &directory, owner, || true, || true).unwrap();
        for (guard, no_tun) in [(false, true), (true, false)] {
            assert_eq!(
                run_at(Action::Revoke, &directory, owner, || guard, || no_tun),
                Err(Error::Refused)
            );
            assert!(directory.join(ENROLLMENT).exists());
        }
    }

    #[test]
    fn unsafe_directory_staging_link_and_file_refuse() {
        let (_root, directory, owner) = fixture();
        symlink("missing", &directory).unwrap();
        assert_eq!(
            run_at(Action::Enroll(1000), &directory, owner, || true, || true),
            Err(Error::Refused)
        );
        fs::remove_file(&directory).unwrap();
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            run_at(Action::Enroll(1000), &directory, owner, || true, || true),
            Err(Error::Refused)
        );
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        symlink("missing", directory.join(STAGING)).unwrap();
        assert_eq!(
            run_at(Action::Enroll(1000), &directory, owner, || true, || true),
            Err(Error::Refused)
        );
        fs::remove_file(directory.join(STAGING)).unwrap();
        run_at(Action::Enroll(1000), &directory, owner, || true, || true).unwrap();
        fs::set_permissions(
            directory.join(ENROLLMENT),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(
            run_at(Action::Revoke, &directory, owner, || true, || true),
            Err(Error::Refused)
        );
    }
}
