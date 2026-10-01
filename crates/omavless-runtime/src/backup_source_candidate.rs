// SPDX-License-Identifier: MIT

//! Inactive source-pair acquisition and authenticated sealing primitive. There
//! is no product caller, file publication, IPC or restore authority.

use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::desired::DesiredPaths;
use omavless_domain::{config::MAX_TEMPLATE_BYTES, private_store::MAX_PRIVATE_STORE_BYTES};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SnapshotError {
    Admission,
    UnsafeSource,
    SourceChanged,
    InvalidBackupInput,
    SealingUnavailable,
}

// No formatting, serialization or clone: these are reusable private bytes.
// Both buffers are cleared when the pair leaves scope, including error paths.
struct PrivateSourcePair {
    store: Zeroizing<Vec<u8>>,
    template: Zeroizing<Vec<u8>>,
}

fn stable(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn private_directory(path: &Path, uid: u32) -> Result<Metadata, SnapshotError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| SnapshotError::UnsafeSource)?;
    if !path.is_absolute()
        || !metadata.is_dir()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(SnapshotError::UnsafeSource);
    }
    Ok(metadata)
}

fn admit(
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
) -> Result<(), SnapshotError> {
    if !lock.authorizes(paths, uid) {
        return Err(SnapshotError::Admission);
    }
    private_directory(&paths.state_directory, uid)?;
    let marker = read_marker_existing(paths, uid).map_err(|_| SnapshotError::Admission)?;
    let desired = DesiredPaths {
        directory: paths.state_directory.clone(),
        file: paths.state_directory.join("desired.json"),
    };
    if marker.phase() != OwnershipPhase::Rust
        || marker.generation() != generation
        || crate::routing_preset::pending(&desired)
    {
        return Err(SnapshotError::Admission);
    }
    Ok(())
}

struct Member {
    file: File,
    path: PathBuf,
    before: Metadata,
    limit: usize,
}

impl Member {
    fn open(directory: &File, name: &str, uid: u32, limit: usize) -> Result<Self, SnapshotError> {
        let path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(name);
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| SnapshotError::UnsafeSource)?;
        let before = file.metadata().map_err(|_| SnapshotError::UnsafeSource)?;
        if !before.is_file()
            || before.uid() != uid
            || before.mode() & 0o7777 != 0o600
            || before.nlink() != 1
            || before.len() == 0
            || before.len() > limit as u64
        {
            return Err(SnapshotError::UnsafeSource);
        }
        Ok(Self {
            file,
            path,
            before,
            limit,
        })
    }

    fn read(&mut self) -> Result<Zeroizing<Vec<u8>>, SnapshotError> {
        let mut bytes = Zeroizing::new(Vec::new());
        Read::by_ref(&mut self.file)
            .take(self.limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| SnapshotError::UnsafeSource)?;
        if bytes.len() as u64 != self.before.len() || bytes.len() > self.limit {
            return Err(SnapshotError::SourceChanged);
        }
        self.verify()?;
        Ok(bytes)
    }

    fn verify(&self) -> Result<(), SnapshotError> {
        let held = self
            .file
            .metadata()
            .map_err(|_| SnapshotError::SourceChanged)?;
        let current = fs::symlink_metadata(&self.path).map_err(|_| SnapshotError::SourceChanged)?;
        if !stable(&self.before, &held) || !stable(&self.before, &current) {
            return Err(SnapshotError::SourceChanged);
        }
        Ok(())
    }
}

fn capture(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    between_reads: impl FnOnce(),
) -> Result<PrivateSourcePair, SnapshotError> {
    admit(paths, uid, generation, lock)?;
    let before = private_directory(config, uid)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_DIRECTORY)
        .open(config)
        .map_err(|_| SnapshotError::UnsafeSource)?;
    if !stable(
        &before,
        &directory
            .metadata()
            .map_err(|_| SnapshotError::UnsafeSource)?,
    ) {
        return Err(SnapshotError::SourceChanged);
    }
    // Fixed member names only; hold both descriptors before reading either.
    let mut store = Member::open(&directory, "profiles.json", uid, MAX_PRIVATE_STORE_BYTES)?;
    let mut template = Member::open(&directory, "route-template.yaml", uid, MAX_TEMPLATE_BYTES)?;
    let store_bytes = store.read()?;
    between_reads(); // Synthetic interleaving hook, absent from product builds.
    let template_bytes = template.read()?;
    store.verify()?;
    template.verify()?;
    if !stable(&before, &private_directory(config, uid)?) {
        return Err(SnapshotError::SourceChanged);
    }
    admit(paths, uid, generation, lock)?;
    Ok(PrivateSourcePair {
        store: store_bytes,
        template: template_bytes,
    })
}

/// Only a future serialized native owner may call this with its held lease and
/// exact generation. This does not publish the sealed bytes or authorize a
/// restore. No current CLI, IPC or background operation exposes it.
#[allow(dead_code)]
pub(crate) fn seal_current_pair(
    config: &Path,
    paths: &CutoverPaths,
    uid: u32,
    generation: u64,
    lock: &MigrationLock,
    passphrase: &[u8],
) -> Result<Vec<u8>, SnapshotError> {
    let pair = capture(config, paths, uid, generation, lock, || {})?;
    omavless_domain::private_backup::seal(&pair.store, &pair.template, passphrase).map_err(
        |error| match error {
            omavless_domain::private_backup::BackupError::Unavailable => {
                SnapshotError::SealingUnavailable
            }
            _ => SnapshotError::InvalidBackupInput,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    const VALID_STORE: &[u8] = br#"{"version":3,"profiles":[],"subscriptions":[],"activeId":"","lastId":"","routingPreset":"roscomvpn-default","customRules":[],"rulesUpdatedAt":0,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"},"startupConfigured":true,"onboardingComplete":false}"#;
    const VALID_TEMPLATE: &[u8] = include_bytes!("../../../templates/default.yaml");
    const PASSPHRASE: &[u8] = b"synthetic passphrase only";

    struct Fixture {
        root: PathBuf,
        config: PathBuf,
        paths: CutoverPaths,
        uid: u32,
        lock: MigrationLock,
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    impl Fixture {
        fn new() -> Self {
            let root = crate::test_temp::directory("backup-source").unwrap();
            let config = root.join("config");
            let runtime = root.join("runtime");
            let state = root.join("state");
            for directory in [&root, &config, &runtime, &state, &state.join("omavless")] {
                fs::create_dir_all(directory).unwrap();
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::metadata(&root).unwrap().uid();
            let paths = CutoverPaths::below(&runtime, &state, uid);
            let lock = MigrationLock::acquire(&paths, uid).unwrap();
            write(
                &paths.ownership_marker,
                br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
            );
            write(&config.join("profiles.json"), b"synthetic-store");
            write(&config.join("route-template.yaml"), b"synthetic-template");
            Self {
                root,
                config,
                paths,
                uid,
                lock,
            }
        }

        fn read(&self, hook: impl FnOnce()) -> Result<PrivateSourcePair, SnapshotError> {
            capture(&self.config, &self.paths, self.uid, 2, &self.lock, hook)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn backup_source_reads_only_fixed_pair_without_semantic_or_export_claim() {
        let fixture = Fixture::new();
        write(&fixture.config.join("desired.json"), b"not-portable");
        write(&fixture.config.join("private.log"), b"not-portable");
        let before = fs::read(&fixture.paths.ownership_marker).unwrap();
        let pair = fixture.read(|| {}).unwrap();
        assert!(pair.store.as_slice() == b"synthetic-store");
        assert!(pair.template.as_slice() == b"synthetic-template");
        assert!(before == fs::read(&fixture.paths.ownership_marker).unwrap());
        assert!(MigrationLock::acquire(&fixture.paths, fixture.uid).is_err());
        // Bytes are intentionally not a semantic store/template fixture.
        assert!(
            omavless_domain::private_store::parse_private_store(
                std::str::from_utf8(&pair.store).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn committed_source_pair_seals_without_publishing_plaintext_or_changing_sources() {
        let fixture = Fixture::new();
        let store_path = fixture.config.join("profiles.json");
        let template_path = fixture.config.join("route-template.yaml");
        write(&store_path, VALID_STORE);
        write(&template_path, VALID_TEMPLATE);
        let original_marker = fs::read(&fixture.paths.ownership_marker).unwrap();

        let envelope = seal_current_pair(
            &fixture.config,
            &fixture.paths,
            fixture.uid,
            2,
            &fixture.lock,
            PASSPHRASE,
        )
        .unwrap();
        assert!(envelope.len() <= omavless_domain::private_backup::MAX_BACKUP_BYTES);
        assert!(
            !envelope
                .windows(VALID_STORE.len())
                .any(|window| window == VALID_STORE)
        );
        let opened = omavless_domain::private_backup::open(&envelope, PASSPHRASE).unwrap();
        assert_eq!(opened.store(), VALID_STORE);
        assert_eq!(opened.template(), VALID_TEMPLATE);
        assert_eq!(fs::read(store_path).unwrap(), VALID_STORE);
        assert_eq!(fs::read(template_path).unwrap(), VALID_TEMPLATE);
        assert_eq!(
            fs::read(&fixture.paths.ownership_marker).unwrap(),
            original_marker
        );

        assert_eq!(
            seal_current_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &fixture.lock,
                b"short",
            ),
            Err(SnapshotError::InvalidBackupInput)
        );
        write(
            &fixture.config.join("route-template.yaml"),
            b"invalid-template",
        );
        assert_eq!(
            seal_current_pair(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &fixture.lock,
                PASSPHRASE,
            ),
            Err(SnapshotError::InvalidBackupInput)
        );
    }

    #[test]
    fn backup_source_refuses_wrong_lease_generation_and_interrupted_preset() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        assert!(
            capture(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                2,
                &other.lock,
                || {}
            )
            .is_err()
        );
        assert!(
            capture(
                &fixture.config,
                &fixture.paths,
                fixture.uid,
                3,
                &fixture.lock,
                || {}
            )
            .is_err()
        );
        let pending = fixture
            .paths
            .state_directory
            .join("routing-preset.pending.json");
        write(&pending, b"even-malformed-blocks");
        assert!(fixture.read(|| {}).is_err());
        assert!(fs::read(&pending).unwrap() == b"even-malformed-blocks");
    }

    #[test]
    fn backup_source_refuses_unsafe_members_before_reading() {
        for name in ["profiles.json", "route-template.yaml"] {
            for kind in [
                "public",
                "symlink",
                "hardlink",
                "directory",
                "missing",
                "empty",
            ] {
                let fixture = Fixture::new();
                let path = fixture.config.join(name);
                match kind {
                    "public" => {
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap()
                    }
                    "hardlink" => fs::hard_link(&path, fixture.root.join("alias")).unwrap(),
                    "empty" => write(&path, b""),
                    _ => {
                        fs::remove_file(&path).unwrap();
                        if kind == "symlink" {
                            symlink(&fixture.paths.ownership_marker, &path).unwrap();
                        } else if kind == "directory" {
                            fs::create_dir(&path).unwrap();
                        }
                    }
                }
                assert!(fixture.read(|| {}).is_err());
            }
        }
    }

    #[test]
    fn backup_source_rejects_oversize_and_wrong_owner_without_partial_pair() {
        for (name, limit) in [
            ("profiles.json", MAX_PRIVATE_STORE_BYTES),
            ("route-template.yaml", MAX_TEMPLATE_BYTES),
        ] {
            let fixture = Fixture::new();
            File::options()
                .write(true)
                .open(fixture.config.join(name))
                .unwrap()
                .set_len(limit as u64 + 1)
                .unwrap();
            assert!(fixture.read(|| {}).is_err());
        }
        let fixture = Fixture::new();
        assert!(
            capture(
                &fixture.config,
                &fixture.paths,
                fixture.uid.wrapping_add(1),
                2,
                &fixture.lock,
                || {}
            )
            .is_err()
        );
    }

    #[test]
    fn backup_source_detects_in_place_and_atomic_replacement_between_reads() {
        for name in ["profiles.json", "route-template.yaml"] {
            for atomic in [false, true] {
                let fixture = Fixture::new();
                let changed =
                    vec![b'x'; fs::metadata(fixture.config.join(name)).unwrap().len() as usize];
                assert!(
                    fixture
                        .read(|| {
                            let path = fixture.config.join(name);
                            if atomic {
                                let replacement = fixture.config.join("replacement");
                                write(&replacement, &changed);
                                fs::rename(replacement, path).unwrap();
                            } else {
                                write(&path, &changed);
                            }
                        })
                        .is_err()
                );
                assert!(fs::read(fixture.config.join(name)).unwrap() == changed);
            }
        }
    }

    #[test]
    fn backup_source_revalidates_directory_owner_and_pending_state() {
        for change in ["directory", "owner", "pending"] {
            let fixture = Fixture::new();
            assert!(
                fixture
                    .read(|| match change {
                        "directory" => {
                            fs::rename(&fixture.config, fixture.root.join("old-config")).unwrap();
                            fs::create_dir(&fixture.config).unwrap();
                            fs::set_permissions(&fixture.config, fs::Permissions::from_mode(0o700))
                                .unwrap();
                        }
                        "owner" => write(
                            &fixture.paths.ownership_marker,
                            br#"{"schemaVersion":1,"generation":3,"phase":"rollbackPreparing"}"#
                        ),
                        _ => write(
                            &fixture
                                .paths
                                .state_directory
                                .join("routing-preset.pending.json"),
                            b"pending"
                        ),
                    })
                    .is_err()
            );
        }
    }

    #[test]
    fn backup_source_rejects_unsafe_directory_and_uncommitted_markers() {
        for change in [
            "symlink",
            "public",
            "missing-marker",
            "legacy",
            "unsafe-marker",
        ] {
            let fixture = Fixture::new();
            match change {
                "symlink" => {
                    let held = fixture.root.join("held-config");
                    fs::rename(&fixture.config, &held).unwrap();
                    symlink(held, &fixture.config).unwrap();
                }
                "public" => {
                    fs::set_permissions(&fixture.config, fs::Permissions::from_mode(0o755)).unwrap()
                }
                "missing-marker" => fs::remove_file(&fixture.paths.ownership_marker).unwrap(),
                "legacy" => write(
                    &fixture.paths.ownership_marker,
                    br#"{"schemaVersion":1,"generation":2,"phase":"legacy"}"#,
                ),
                _ => fs::set_permissions(
                    &fixture.paths.ownership_marker,
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap(),
            }
            assert!(fixture.read(|| {}).is_err());
        }
    }

    #[test]
    fn backup_source_holds_lease_during_both_member_reads() {
        let fixture = Fixture::new();
        let pair = fixture
            .read(|| {
                assert!(matches!(
                    MigrationLock::acquire(&fixture.paths, fixture.uid),
                    Err(crate::cutover::CutoverError::Busy)
                ));
            })
            .unwrap();
        assert!(pair.store.as_slice() == b"synthetic-store");
        assert!(pair.template.as_slice() == b"synthetic-template");
    }
}
