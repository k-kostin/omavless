// SPDX-License-Identifier: MIT

//! Inactive fixed-member restore staging. This writes only to a new private
//! pending directory, never to the live store/template. A surviving directory
//! is a refusal/recovery signal, not permission to finish or retry a restore.

use crate::backup_source_candidate::open_private_directory;
use crate::desired::DesiredPaths;
use nix::errno::Errno;
use nix::fcntl::{OFlag, openat};
use nix::sys::stat::{Mode, mkdirat};
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

const PENDING_DIRECTORY: &str = "restore-pair.pending";

/// Existence, inaccessible metadata and unexpected entry types all block a
/// second restore attempt. There is deliberately no automatic deletion.
pub(crate) fn staging_pending(paths: &DesiredPaths) -> bool {
    !matches!(
        std::fs::symlink_metadata(paths.directory.join(PENDING_DIRECTORY)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}
const MEMBERS: [&str; 4] = [
    "old-profiles.json",
    "old-route-template.yaml",
    "new-profiles.json",
    "new-route-template.yaml",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageError {
    InvalidPair,
    UnsafeState,
    AlreadyPending,
    /// The private pending directory may exist. Never retry or remove it
    /// automatically after a write/metadata/sync failure.
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Directory,
    Member(usize),
    Complete,
}

fn exact_directory(metadata: &Metadata, uid: u32) -> bool {
    metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o7777 == 0o700
}

fn same_directory(before: &Metadata, after: &Metadata) -> bool {
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.mode() == after.mode()
}

fn write_member(directory: &File, name: &str, uid: u32, bytes: &[u8]) -> Result<(), StageError> {
    let mut file = File::from(
        openat(
            directory,
            Path::new(name),
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )
        .map_err(|_| StageError::Ambiguous)?,
    );
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|_| StageError::Ambiguous)?;
    let metadata = file.metadata().map_err(|_| StageError::Ambiguous)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(StageError::Ambiguous);
    }
    file.write_all(bytes).map_err(|_| StageError::Ambiguous)?;
    file.sync_all().map_err(|_| StageError::Ambiguous)?;
    if file.metadata().map_err(|_| StageError::Ambiguous)?.len() != bytes.len() as u64 {
        return Err(StageError::Ambiguous);
    }
    Ok(())
}

/// Preserve exact old and authenticated new bytes under five fixed names.
/// This primitive has no product caller and does not validate restore
/// admission. The owner must hold its exclusive lease, recheck generation and
/// source facts, and later implement durable commit/rollback separately.
#[allow(dead_code)]
pub(crate) fn stage_private_pair(
    state_directory: &Path,
    uid: u32,
    old_store: &[u8],
    old_template: &[u8],
    new_store: &[u8],
    new_template: &[u8],
) -> Result<(), StageError> {
    stage_with_hook(
        state_directory,
        uid,
        [old_store, old_template, new_store, new_template],
        |_| true,
    )
}

fn stage_with_hook(
    state_directory: &Path,
    uid: u32,
    members: [&[u8]; 4],
    mut proceed: impl FnMut(Step) -> bool,
) -> Result<(), StageError> {
    for (index, bytes) in members.iter().enumerate() {
        let limit = if index % 2 == 0 {
            MAX_PRIVATE_STORE_BYTES
        } else {
            MAX_TEMPLATE_BYTES
        };
        if bytes.is_empty() || bytes.len() > limit {
            return Err(StageError::InvalidPair);
        }
    }
    let parent =
        open_private_directory(state_directory, uid).map_err(|_| StageError::UnsafeState)?;
    let parent_before = parent.metadata().map_err(|_| StageError::UnsafeState)?;
    match mkdirat(&parent, PENDING_DIRECTORY, Mode::S_IRWXU) {
        Ok(()) => {}
        Err(Errno::EEXIST) => return Err(StageError::AlreadyPending),
        Err(_) => return Err(StageError::UnsafeState),
    }
    // Once the fixed name exists, every failure is ambiguous. Keep the
    // directory, even if it is partial, so nothing retries over it blindly.
    parent.sync_all().map_err(|_| StageError::Ambiguous)?;
    if !proceed(Step::Directory) {
        return Err(StageError::Ambiguous);
    }
    let directory = File::from(
        openat(
            &parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| StageError::Ambiguous)?,
    );
    let directory_before = directory.metadata().map_err(|_| StageError::Ambiguous)?;
    if !exact_directory(&directory_before, uid) {
        return Err(StageError::Ambiguous);
    }
    for (index, bytes) in members.iter().enumerate() {
        write_member(&directory, MEMBERS[index], uid, bytes)?;
        if !proceed(Step::Member(index)) {
            return Err(StageError::Ambiguous);
        }
    }
    directory.sync_all().map_err(|_| StageError::Ambiguous)?;
    if !proceed(Step::Complete) {
        return Err(StageError::Ambiguous);
    }
    let current = File::from(
        openat(
            &parent,
            Path::new(PENDING_DIRECTORY),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| StageError::Ambiguous)?,
    );
    if !same_directory(
        &directory_before,
        &current.metadata().map_err(|_| StageError::Ambiguous)?,
    ) || !same_directory(
        &parent_before,
        &open_private_directory(state_directory, uid)
            .map_err(|_| StageError::Ambiguous)?
            .metadata()
            .map_err(|_| StageError::Ambiguous)?,
    ) {
        return Err(StageError::Ambiguous);
    }
    parent.sync_all().map_err(|_| StageError::Ambiguous)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn root() -> (std::path::PathBuf, u32) {
        let home = std::env::var_os("HOME").expect("restore-stage test needs home");
        let root = crate::test_temp::directory_under(Path::new(&home), "restore-stage").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    const PAIR: [&[u8]; 4] = [b"old store", b"old template", b"new store", b"new template"];

    #[test]
    fn stages_only_fixed_private_members_and_refuses_second_attempt() {
        let (root, uid) = root();
        assert_eq!(
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
            Ok(())
        );
        let staged = root.join(PENDING_DIRECTORY);
        assert_eq!(
            fs::symlink_metadata(&staged).unwrap().mode() & 0o7777,
            0o700
        );
        let mut entries = fs::read_dir(&staged)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        entries.sort();
        let mut expected = MEMBERS.map(std::ffi::OsString::from).to_vec();
        expected.sort();
        assert_eq!(entries, expected);
        for (name, bytes) in MEMBERS.iter().zip(PAIR) {
            let file = staged.join(name);
            let metadata = fs::symlink_metadata(&file).unwrap();
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
            assert!(fs::read(file).unwrap() == bytes);
        }
        assert_eq!(
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
            Err(StageError::AlreadyPending)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_staging_is_ambiguous_and_never_auto_removed() {
        for stop in [
            Step::Directory,
            Step::Member(0),
            Step::Member(2),
            Step::Complete,
        ] {
            let (root, uid) = root();
            assert_eq!(
                stage_with_hook(&root, uid, PAIR, |step| step != stop),
                Err(StageError::Ambiguous)
            );
            assert!(root.join(PENDING_DIRECTORY).is_dir());
            assert_eq!(
                stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
                Err(StageError::AlreadyPending)
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn unsafe_or_existing_targets_and_invalid_sizes_have_no_new_members() {
        let (root, uid) = root();
        assert_eq!(
            stage_private_pair(&root, uid, b"", PAIR[1], PAIR[2], PAIR[3]),
            Err(StageError::InvalidPair)
        );
        assert_eq!(
            stage_private_pair(
                &root,
                uid.wrapping_add(1),
                PAIR[0],
                PAIR[1],
                PAIR[2],
                PAIR[3]
            ),
            Err(StageError::UnsafeState)
        );
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        symlink(&target, root.join(PENDING_DIRECTORY)).unwrap();
        assert_eq!(
            stage_private_pair(&root, uid, PAIR[0], PAIR[1], PAIR[2], PAIR[3]),
            Err(StageError::AlreadyPending)
        );
        assert!(fs::read_dir(target).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
