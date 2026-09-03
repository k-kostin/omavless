// SPDX-License-Identifier: MIT

//! Create-only bootstrap for the canonical private profile store.
//!
//! This module has no CLI, IPC, daemon, or UI registration. It accepts no
//! client path or payload: callers construct the fixed
//! `~/.config/omavless/profiles.json` location from a trusted home directory,
//! and the payload is the exact credential-free Python `empty_store()`
//! representation.

use crate::cutover::{CutoverError, CutoverPaths, MigrationLock};
use omavless_domain::private_store::parse_private_store;
use omavless_store::{
    PrivateCreateOutcome, StoreIoError, atomic_create_private, read_private_utf8,
};
use std::env;
use std::fmt;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const PRIVATE_STORE_NAME: &str = "profiles.json";

const EMPTY_STORE_PAYLOAD: &[u8] = br#"{
  "version": 3,
  "activeId": "",
  "lastId": "",
  "profiles": [],
  "subscriptions": [],
  "routingPreset": "",
  "customRules": [],
  "rulesUpdatedAt": 0,
  "startupConfigured": true,
  "startup": {
    "enabled": false,
    "target": "last",
    "profileId": "",
    "mode": "rule"
  },
  "onboardingComplete": false
}
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateStoreBootstrapPaths {
    config_directory: PathBuf,
    store: PathBuf,
}

impl PrivateStoreBootstrapPaths {
    #[must_use]
    pub fn below_home(home: &Path) -> Self {
        let config_directory = home.join(".config/omavless");
        Self {
            store: config_directory.join(PRIVATE_STORE_NAME),
            config_directory,
        }
    }

    pub fn current() -> Result<Self, PrivateStoreBootstrapError> {
        let home = env::var_os("OMAVLESS_HOME")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or(PrivateStoreBootstrapError::UnsafeHome)?;
        if !home.is_absolute() {
            return Err(PrivateStoreBootstrapError::UnsafeHome);
        }
        Ok(Self::below_home(&home))
    }

    #[must_use]
    pub fn store_path(&self) -> &Path {
        &self.store
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateStoreBootstrapOutcome {
    Created,
    AlreadyExists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateStoreBootstrapError {
    UnsafeHome,
    UnsafeConfigDirectory,
    UnsafeStore,
    InvalidStore,
    Busy,
    LockIo,
    Io,
}

impl fmt::Display for PrivateStoreBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeHome => "OmaVLESS private home path is unsafe",
            Self::UnsafeConfigDirectory => "OmaVLESS private config directory is unsafe",
            Self::UnsafeStore => "OmaVLESS private store path is unsafe",
            Self::InvalidStore => "OmaVLESS private store is invalid",
            Self::Busy => "Another OmaVLESS operation owns the migration lock",
            Self::LockIo => "OmaVLESS migration lock failed",
            Self::Io => "OmaVLESS private store bootstrap failed",
        })
    }
}

impl std::error::Error for PrivateStoreBootstrapError {}

fn validate_config_directory(path: &Path, uid: u32) -> Result<(), PrivateStoreBootstrapError> {
    if !path.is_absolute() {
        return Err(PrivateStoreBootstrapError::UnsafeConfigDirectory);
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| PrivateStoreBootstrapError::UnsafeConfigDirectory)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(PrivateStoreBootstrapError::UnsafeConfigDirectory);
    }
    Ok(())
}

fn validate_existing_store(path: &Path, uid: u32) -> Result<(), PrivateStoreBootstrapError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PrivateStoreBootstrapError::Io)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(PrivateStoreBootstrapError::UnsafeStore);
    }
    let text = read_private_utf8(path, uid).map_err(|error| match error {
        StoreIoError::UnsafePath | StoreIoError::WrongOwner => {
            PrivateStoreBootstrapError::UnsafeStore
        }
        StoreIoError::TooLarge | StoreIoError::InvalidUtf8 => {
            PrivateStoreBootstrapError::InvalidStore
        }
        StoreIoError::Io => PrivateStoreBootstrapError::Io,
    })?;
    parse_private_store(&text)
        .map(|_| ())
        .map_err(|_| PrivateStoreBootstrapError::InvalidStore)
}

fn map_lock_error(error: CutoverError) -> PrivateStoreBootstrapError {
    match error {
        CutoverError::Busy => PrivateStoreBootstrapError::Busy,
        CutoverError::UnsafeRuntimeDirectory => PrivateStoreBootstrapError::LockIo,
        CutoverError::UnsafeStateDirectory
        | CutoverError::InvalidMarker
        | CutoverError::MarkerTooLarge
        | CutoverError::InvalidTransition
        | CutoverError::PreconditionsFailed
        | CutoverError::Io => PrivateStoreBootstrapError::LockIo,
    }
}

/// Create the canonical empty v3 store under the shared migration lock.
///
/// An existing target is never replaced. It must itself be a complete,
/// private, valid v1-v3 store before `AlreadyExists` is returned.
pub fn bootstrap_private_store(
    paths: &PrivateStoreBootstrapPaths,
    cutover_paths: &CutoverPaths,
    uid: u32,
) -> Result<PrivateStoreBootstrapOutcome, PrivateStoreBootstrapError> {
    validate_config_directory(&paths.config_directory, uid)?;
    let _lock = MigrationLock::acquire(cutover_paths, uid).map_err(map_lock_error)?;
    // Repeat after acquiring the lease so a cooperating migration cannot have
    // changed the directory between admission and publication.
    validate_config_directory(&paths.config_directory, uid)?;

    match fs::symlink_metadata(&paths.store) {
        Ok(_) => {
            validate_existing_store(&paths.store, uid)?;
            return Ok(PrivateStoreBootstrapOutcome::AlreadyExists);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(PrivateStoreBootstrapError::Io),
    }

    match atomic_create_private(&paths.store, EMPTY_STORE_PAYLOAD, uid).map_err(
        |error| match error {
            StoreIoError::UnsafePath | StoreIoError::WrongOwner => {
                PrivateStoreBootstrapError::UnsafeStore
            }
            StoreIoError::TooLarge | StoreIoError::InvalidUtf8 | StoreIoError::Io => {
                PrivateStoreBootstrapError::Io
            }
        },
    )? {
        PrivateCreateOutcome::AlreadyExists => {
            validate_existing_store(&paths.store, uid)?;
            Ok(PrivateStoreBootstrapOutcome::AlreadyExists)
        }
        PrivateCreateOutcome::Created => {
            validate_existing_store(&paths.store, uid)?;
            let reread =
                read_private_utf8(&paths.store, uid).map_err(|_| PrivateStoreBootstrapError::Io)?;
            if reread.as_bytes() != EMPTY_STORE_PAYLOAD {
                return Err(PrivateStoreBootstrapError::InvalidStore);
            }
            Ok(PrivateStoreBootstrapOutcome::Created)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRoot {
        root: PathBuf,
        home: PathBuf,
        runtime: PathBuf,
        state: PathBuf,
        uid: u32,
    }

    impl TestRoot {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = env::temp_dir().join(format!(
                "omavless-store-bootstrap-{}-{unique}",
                std::process::id()
            ));
            let home = root.join("home");
            let config = home.join(".config/omavless");
            let runtime = root.join("runtime");
            let state = root.join("state");
            for directory in [
                &root,
                &home,
                &home.join(".config"),
                &config,
                &runtime,
                &state,
            ] {
                fs::create_dir(directory).unwrap();
            }
            fs::set_permissions(&config, fs::Permissions::from_mode(0o700)).unwrap();
            fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
            let uid = fs::metadata(&root).unwrap().uid();
            Self {
                root,
                home,
                runtime,
                state,
                uid,
            }
        }

        fn paths(&self) -> (PrivateStoreBootstrapPaths, CutoverPaths) {
            (
                PrivateStoreBootstrapPaths::below_home(&self.home),
                CutoverPaths::below(&self.runtime, &self.state, self.uid),
            )
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn creates_exact_private_current_store_and_then_reports_existing() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid).unwrap(),
            PrivateStoreBootstrapOutcome::Created
        );
        assert_eq!(fs::read(paths.store_path()).unwrap(), EMPTY_STORE_PAYLOAD);
        assert_eq!(
            fs::metadata(paths.store_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid).unwrap(),
            PrivateStoreBootstrapOutcome::AlreadyExists
        );
    }

    #[test]
    fn payload_matches_python_empty_store_exactly() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = Command::new("python3")
            .arg(repository.join("tools/private_store_bootstrap_parity.py"))
            .current_dir(&repository)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, EMPTY_STORE_PAYLOAD);
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn malformed_existing_store_fails_closed_without_reset() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        let original = b"{not-json}\n";
        fs::write(paths.store_path(), original).unwrap();
        fs::set_permissions(paths.store_path(), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Err(PrivateStoreBootstrapError::InvalidStore)
        );
        assert_eq!(fs::read(paths.store_path()).unwrap(), original);
    }

    #[test]
    fn valid_legacy_store_is_reported_existing_without_normalization() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        let original = b"{\"profiles\":[]}\n";
        fs::write(paths.store_path(), original).unwrap();
        fs::set_permissions(paths.store_path(), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Ok(PrivateStoreBootstrapOutcome::AlreadyExists)
        );
        assert_eq!(fs::read(paths.store_path()).unwrap(), original);
    }

    #[test]
    fn unsafe_existing_mode_fails_closed_without_repair() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        fs::write(paths.store_path(), EMPTY_STORE_PAYLOAD).unwrap();
        fs::set_permissions(paths.store_path(), fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Err(PrivateStoreBootstrapError::UnsafeStore)
        );
        assert_eq!(
            fs::metadata(paths.store_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
    }

    #[test]
    fn symlink_target_fails_closed_without_touching_destination() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        let destination = fixture.root.join("destination");
        fs::write(&destination, b"unchanged").unwrap();
        symlink(&destination, paths.store_path()).unwrap();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Err(PrivateStoreBootstrapError::UnsafeStore)
        );
        assert_eq!(fs::read(destination).unwrap(), b"unchanged");
    }

    #[test]
    fn non_private_or_wrong_owner_parent_fails_before_creation() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        fs::set_permissions(&paths.config_directory, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Err(PrivateStoreBootstrapError::UnsafeConfigDirectory)
        );
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid.wrapping_add(1)),
            Err(PrivateStoreBootstrapError::UnsafeConfigDirectory)
        );
        assert!(!paths.store_path().exists());
    }

    #[test]
    fn symlinked_parent_is_rejected() {
        let fixture = TestRoot::new();
        let config = fixture.home.join(".config/omavless");
        fs::remove_dir(&config).unwrap();
        let destination = fixture.root.join("other-config");
        fs::create_dir(&destination).unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();
        symlink(&destination, &config).unwrap();
        let (paths, cutover) = fixture.paths();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Err(PrivateStoreBootstrapError::UnsafeConfigDirectory)
        );
        assert!(!destination.join(PRIVATE_STORE_NAME).exists());
    }

    #[test]
    fn shared_migration_lock_is_mandatory() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        let _lock = MigrationLock::acquire(&cutover, fixture.uid).unwrap();
        assert_eq!(
            bootstrap_private_store(&paths, &cutover, fixture.uid),
            Err(PrivateStoreBootstrapError::Busy)
        );
        assert!(!paths.store_path().exists());
    }

    #[test]
    fn errors_never_include_paths_or_payload_fragments() {
        let fixture = TestRoot::new();
        let (paths, cutover) = fixture.paths();
        let private_fragment = "private.example.invalid";
        fs::write(
            paths.store_path(),
            format!("{{\"password\":\"{private_fragment}\"}}\n"),
        )
        .unwrap();
        fs::set_permissions(paths.store_path(), fs::Permissions::from_mode(0o600)).unwrap();
        let error = bootstrap_private_store(&paths, &cutover, fixture.uid).unwrap_err();
        let public = error.to_string();
        assert!(!public.contains(private_fragment));
        assert!(!public.contains("password"));
        assert!(!public.contains(paths.store_path().to_string_lossy().as_ref()));
    }
}
