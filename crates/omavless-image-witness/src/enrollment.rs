// SPDX-License-Identifier: MIT
//! Explicit root administration, not IPC, service activation or effect proof.
use crate::{Error, Result, class::Class, kernel};
use rustix::fs::{self, AtFlags, Mode, OFlags};
use std::{
    fs::File,
    io::Write,
    os::unix::fs::MetadataExt,
    time::{Duration, Instant},
};

const DIRECTORY: &str = "/var/lib/omavless-image-product";
const RECORD: &str = "runtime.enrollment";
type DirectoryId = (u64, u64, u32, u32, u32);
fn directory_id(file: &File, mode: Option<u32>) -> Result<DirectoryId> {
    let m = file.metadata().map_err(|_| Error::Unavailable)?;
    if !m.is_dir()
        || m.uid() != 0
        || m.gid() != 0
        || m.mode() & 0o7022 != 0
        || mode.is_some_and(|expected| m.mode() & 0o7777 != expected)
    {
        return Err(Error::Refused);
    }
    Ok((m.dev(), m.ino(), m.uid(), m.gid(), m.mode()))
}
fn named_directory(path: &str, file: &File, id: DirectoryId) -> Result<()> {
    let m = std::fs::symlink_metadata(path).map_err(|_| Error::Unavailable)?;
    if directory_id(file, None)? != id
        || !m.is_dir()
        || (m.dev(), m.ino(), m.uid(), m.gid(), m.mode()) != id
    {
        return Err(Error::Refused);
    }
    Ok(())
}
fn absent_at(directory: &File, name: &str) -> Result<()> {
    match fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        _ => Err(Error::Refused),
    }
}
pub(crate) fn uid(text: &str) -> Result<u32> {
    let uid = text.parse::<u32>().map_err(|_| Error::Refused)?;
    if text != uid.to_string() || !Class::Product.admits_uid(uid) {
        return Err(Error::Refused);
    }
    Ok(uid)
}

/// Fixed root-record creation only; no arbitrary path/hash or caller UID.
/// Every partial/error result leaves its exact prefix untouched, never retries
/// or removes a record. Explicit root reconciliation is required on uncertainty.
pub fn enroll_product(text: &str) -> Result<()> {
    if rustix::process::getuid().as_raw() != 0
        || rustix::process::geteuid().as_raw() != 0
        || rustix::process::getgid().as_raw() != 0
        || rustix::process::getegid().as_raw() != 0
    {
        return Err(Error::Refused);
    }
    let uid = uid(text)?;
    rustix::process::setrlimit(
        rustix::process::Resource::Nofile,
        rustix::process::Rlimit {
            current: Some(64),
            maximum: Some(64),
        },
    )
    .map_err(|_| Error::Unavailable)?;
    let until = Instant::now() + Duration::from_secs(5);
    // Source inputs fully validated before any enrollment mutation.
    let inputs = kernel::EnrollmentInputs::capture(uid, until)?;
    match std::fs::symlink_metadata(Class::Product.socket()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        _ => return Err(Error::Refused),
    }
    let mut parents = Vec::new();
    for path in ["/", "/var", "/var/lib"] {
        let file = File::from(
            fs::open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Unavailable)?,
        );
        let id = directory_id(&file, None)?;
        named_directory(path, &file, id)?;
        parents.push((path, file, id));
    }
    for (path, file, id) in &parents {
        named_directory(path, file, *id)?;
    }
    let parent = &parents.last().ok_or(Error::Refused)?.1;
    inputs.check(until)?;
    // Existing directories are usable only when empty, root0700 and unchanged;
    // record existence always refuses, including apparently matching content.
    match fs::mkdirat(parent, "omavless-image-product", Mode::RWXU) {
        Ok(()) => (),
        Err(rustix::io::Errno::EXIST) => (),
        Err(_) => return Err(Error::Unavailable),
    }
    let directory = File::from(
        fs::openat(
            parent,
            "omavless-image-product",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| Error::Unavailable)?,
    );
    let id = directory_id(&directory, Some(0o700))?;
    named_directory(DIRECTORY, &directory, id)?;
    if std::fs::read_dir(DIRECTORY)
        .map_err(|_| Error::Unavailable)?
        .next()
        .is_some()
    {
        return Err(Error::Refused);
    }
    // A fresh record may never be introduced under an existing provider socket.
    match std::fs::symlink_metadata(Class::Product.socket()) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        _ => return Err(Error::Refused),
    }
    absent_at(&directory, RECORD)?;
    inputs.check(until)?;
    named_directory(DIRECTORY, &directory, id)?;
    let mut record = File::from(
        fs::openat(
            &directory,
            RECORD,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| Error::Unavailable)?,
    );
    let shape = record.metadata().map_err(|_| Error::Unavailable)?;
    if !shape.is_file()
        || shape.uid() != 0
        || shape.gid() != 0
        || shape.nlink() != 1
        || shape.mode() & 0o7777 != 0o600
    {
        return Err(Error::Refused);
    }
    record
        .write_all(inputs.payload())
        .map_err(|_| Error::Unavailable)?;
    record.sync_all().map_err(|_| Error::Unavailable)?;
    use std::os::unix::fs::FileExt;
    let mut raw = [0; 513];
    let n = record
        .read_at(&mut raw, 0)
        .map_err(|_| Error::Unavailable)?;
    let held = record.metadata().map_err(|_| Error::Unavailable)?;
    let named = fs::statat(&directory, RECORD, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| Error::Unavailable)?;
    if n != inputs.payload().len()
        || &raw[..n] != inputs.payload()
        || held.len() != n as u64
        || held.dev() != shape.dev()
        || held.ino() != shape.ino()
        || held.uid() != 0
        || held.gid() != 0
        || held.nlink() != 1
        || held.mode() & 0o7777 != 0o600
        || named.st_dev != held.dev()
        || named.st_ino != held.ino()
        || named.st_mode != held.mode()
        || named.st_uid != 0
        || named.st_gid != 0
        || named.st_nlink != 1
        || named.st_size != held.len() as i64
    {
        return Err(Error::Refused);
    }
    directory.sync_all().map_err(|_| Error::Unavailable)?;
    parent.sync_all().map_err(|_| Error::Unavailable)?;
    for (path, file, id) in &parents {
        named_directory(path, file, *id)?;
    }
    named_directory(DIRECTORY, &directory, id)?;
    inputs.check(until)?;
    let final_held = record.metadata().map_err(|_| Error::Unavailable)?;
    let final_named = fs::statat(&directory, RECORD, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| Error::Unavailable)?;
    if final_held.dev() != held.dev()
        || final_held.ino() != held.ino()
        || final_held.uid() != held.uid()
        || final_held.gid() != held.gid()
        || final_held.mode() != held.mode()
        || final_held.nlink() != held.nlink()
        || final_held.len() != held.len()
        || final_held.mtime() != held.mtime()
        || final_held.mtime_nsec() != held.mtime_nsec()
        || final_held.ctime() != held.ctime()
        || final_held.ctime_nsec() != held.ctime_nsec()
        || final_named.st_dev != final_held.dev()
        || final_named.st_ino != final_held.ino()
        || final_named.st_uid != 0
        || final_named.st_gid != 0
        || final_named.st_mode != final_held.mode()
        || final_named.st_nlink != 1
        || final_named.st_size != final_held.len() as i64
    {
        return Err(Error::Refused);
    }
    kernel::tick(until)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_selection_is_canonical_explicit_nonroot_not_caller_identity() {
        assert_eq!(uid("1001"), Ok(1001));
        for value in [
            "0",
            "4294967295",
            "4294967296",
            "01001",
            "+1001",
            "1001 ",
            "1001\n1001",
            "/path",
        ] {
            assert!(uid(value).is_err());
        }
    }
}
