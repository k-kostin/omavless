// SPDX-License-Identifier: MIT

//! Private, credential-free desired connection state and pure restart
//! reconciliation. Lifecycle execution remains a later R5 checkpoint.

use omavless_store::{StoreIoError, atomic_replace_private, read_private_utf8};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const DESIRED_SCHEMA_VERSION: u8 = 1;
pub const MAX_GENERATION: u64 = i64::MAX as u64;
pub const MAX_PROFILE_ID_BYTES: usize = 64;
pub const MAX_DESIRED_STATE_BYTES: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingMode {
    Rule,
    Global,
    Direct,
}

impl RoutingMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Global => "global",
            Self::Direct => "direct",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredState {
    pub schema_version: u8,
    pub generation: u64,
    pub connected: bool,
    pub profile_id: String,
    pub mode: RoutingMode,
}

impl Default for DesiredState {
    fn default() -> Self {
        Self {
            schema_version: DESIRED_SCHEMA_VERSION,
            generation: 0,
            connected: false,
            profile_id: String::new(),
            mode: RoutingMode::Rule,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredError {
    UnsafeStateDirectory,
    InvalidState,
    TooLarge,
    Io,
}

impl fmt::Display for DesiredError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeStateDirectory => "OmaVLESS state directory is unsafe",
            Self::InvalidState => "OmaVLESS desired state is invalid",
            Self::TooLarge => "OmaVLESS desired state is too large",
            Self::Io => "OmaVLESS desired state I/O failed",
        })
    }
}

impl std::error::Error for DesiredError {}

impl From<StoreIoError> for DesiredError {
    fn from(value: StoreIoError) -> Self {
        match value {
            StoreIoError::UnsafePath | StoreIoError::WrongOwner => Self::UnsafeStateDirectory,
            StoreIoError::TooLarge => Self::TooLarge,
            StoreIoError::InvalidUtf8 => Self::InvalidState,
            StoreIoError::Io => Self::Io,
        }
    }
}

pub type Result<T> = std::result::Result<T, DesiredError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredPaths {
    pub directory: PathBuf,
    pub file: PathBuf,
}

impl DesiredPaths {
    #[must_use]
    pub fn below(base: &Path) -> Self {
        let directory = base.join("omavless");
        Self {
            file: directory.join("desired.json"),
            directory,
        }
    }

    pub fn current() -> Result<Self> {
        let base = match env::var_os("XDG_STATE_HOME") {
            Some(value) => PathBuf::from(value),
            None => {
                let home = env::var_os("HOME").ok_or(DesiredError::UnsafeStateDirectory)?;
                PathBuf::from(home).join(".local/state")
            }
        };
        if !base.is_absolute() {
            return Err(DesiredError::UnsafeStateDirectory);
        }
        Ok(Self::below(&base))
    }
}

fn visible_ascii(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROFILE_ID_BYTES
        && value.bytes().all(|byte| (33..=126).contains(&byte))
}

impl DesiredState {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DESIRED_SCHEMA_VERSION
            || self.generation > MAX_GENERATION
            || (self.connected && !visible_ascii(&self.profile_id))
            || (!self.connected && !self.profile_id.is_empty())
        {
            return Err(DesiredError::InvalidState);
        }
        Ok(())
    }
}

fn prepare_directory(path: &Path, uid: u32) -> Result<()> {
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).map_err(|_| DesiredError::Io)?;
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| DesiredError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != uid {
        return Err(DesiredError::UnsafeStateDirectory);
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|_| DesiredError::Io)
}

pub fn read_desired(paths: &DesiredPaths, uid: u32) -> Result<DesiredState> {
    prepare_directory(&paths.directory, uid)?;
    match fs::symlink_metadata(&paths.file) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DesiredState::default());
        }
        Ok(metadata) if metadata.len() > MAX_DESIRED_STATE_BYTES => {
            return Err(DesiredError::TooLarge);
        }
        Ok(_) => {}
        Err(_) => return Err(DesiredError::Io),
    }
    let raw = read_private_utf8(&paths.file, uid)?;
    let state: DesiredState = serde_json::from_str(&raw).map_err(|_| DesiredError::InvalidState)?;
    state.validate()?;
    Ok(state)
}

pub fn write_desired(paths: &DesiredPaths, uid: u32, state: &DesiredState) -> Result<()> {
    state.validate()?;
    prepare_directory(&paths.directory, uid)?;
    let mut payload = serde_json::to_vec(state).map_err(|_| DesiredError::InvalidState)?;
    payload.push(b'\n');
    if payload.len() as u64 > MAX_DESIRED_STATE_BYTES {
        return Err(DesiredError::TooLarge);
    }
    atomic_replace_private(&paths.file, &payload, uid).map_err(Into::into)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedObservation {
    pub service_active: bool,
    pub controller_ready: bool,
    pub core_count: u8,
    pub tun_count: u8,
    pub active_profile_matches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileAction {
    SettledDisconnected,
    AdoptConnected,
    RecoverConnected,
    StopOwned,
    ManualRecoveryRequired,
}

#[must_use]
pub const fn reconcile(desired: &DesiredState, observed: OwnedObservation) -> ReconcileAction {
    let empty = !observed.service_active
        && !observed.controller_ready
        && observed.core_count == 0
        && observed.tun_count == 0;
    let healthy = observed.service_active
        && observed.controller_ready
        && observed.core_count == 1
        && observed.tun_count == 1
        && observed.active_profile_matches;
    if desired.connected {
        if healthy {
            ReconcileAction::AdoptConnected
        } else if empty {
            ReconcileAction::RecoverConnected
        } else {
            ReconcileAction::ManualRecoveryRequired
        }
    } else if empty {
        ReconcileAction::SettledDisconnected
    } else if healthy {
        ReconcileAction::StopOwned
    } else {
        ReconcileAction::ManualRecoveryRequired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(label: &str) -> (PathBuf, u32) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            env::temp_dir().join(format!("ovr-state-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    #[test]
    fn absent_state_defaults_disconnected_and_round_trip_is_private() {
        let (root, uid) = root("roundtrip");
        let paths = DesiredPaths::below(&root);
        assert_eq!(read_desired(&paths, uid).unwrap(), DesiredState::default());
        let state = DesiredState {
            schema_version: 1,
            generation: 4,
            connected: true,
            profile_id: "opaque-record-id".to_owned(),
            mode: RoutingMode::Global,
        };
        write_desired(&paths, uid, &state).unwrap();
        assert_eq!(read_desired(&paths, uid).unwrap(), state);
        assert_eq!(
            fs::metadata(&paths.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&paths.file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_unknown_duplicate_and_symlinked_state_fail_closed() {
        let (root, uid) = root("invalid");
        let paths = DesiredPaths::below(&root);
        fs::create_dir(&paths.directory).unwrap();
        for payload in [
            r#"{"schemaVersion":2,"generation":0,"connected":false,"profileId":"","mode":"rule"}"#,
            r#"{"schemaVersion":1,"generation":0,"connected":false,"profileId":"secret","mode":"rule"}"#,
            r#"{"schemaVersion":1,"generation":0,"connected":false,"connected":true,"profileId":"id","mode":"rule"}"#,
            r#"{"schemaVersion":1,"generation":0,"connected":false,"profileId":"","mode":"rule","extra":1}"#,
        ] {
            fs::write(&paths.file, payload).unwrap();
            assert_eq!(read_desired(&paths, uid), Err(DesiredError::InvalidState));
        }
        fs::remove_file(&paths.file).unwrap();
        let destination = root.join("destination");
        fs::write(&destination, "private.example/password").unwrap();
        symlink(&destination, &paths.file).unwrap();
        let error = read_desired(&paths, uid).unwrap_err().to_string();
        assert!(!error.contains("private.example"));
        assert!(!error.contains("password"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_and_broken_symlinked_state_do_not_become_defaults() {
        let (root, uid) = root("bounded");
        let paths = DesiredPaths::below(&root);
        fs::create_dir(&paths.directory).unwrap();
        fs::write(
            &paths.file,
            vec![b'x'; MAX_DESIRED_STATE_BYTES as usize + 1],
        )
        .unwrap();
        assert_eq!(read_desired(&paths, uid), Err(DesiredError::TooLarge));
        fs::remove_file(&paths.file).unwrap();
        symlink(root.join("missing"), &paths.file).unwrap();
        assert_eq!(
            read_desired(&paths, uid),
            Err(DesiredError::UnsafeStateDirectory)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reconciliation_is_fail_closed_and_follows_desired_state() {
        let disconnected = DesiredState::default();
        let connected = DesiredState {
            connected: true,
            profile_id: "id".to_owned(),
            ..DesiredState::default()
        };
        let empty = OwnedObservation {
            service_active: false,
            controller_ready: false,
            core_count: 0,
            tun_count: 0,
            active_profile_matches: false,
        };
        let healthy = OwnedObservation {
            service_active: true,
            controller_ready: true,
            core_count: 1,
            tun_count: 1,
            active_profile_matches: true,
        };
        assert_eq!(
            reconcile(&disconnected, empty),
            ReconcileAction::SettledDisconnected
        );
        assert_eq!(
            reconcile(&disconnected, healthy),
            ReconcileAction::StopOwned
        );
        assert_eq!(
            reconcile(&connected, empty),
            ReconcileAction::RecoverConnected
        );
        assert_eq!(
            reconcile(&connected, healthy),
            ReconcileAction::AdoptConnected
        );
        assert_eq!(
            reconcile(
                &connected,
                OwnedObservation {
                    core_count: 2,
                    ..healthy
                }
            ),
            ReconcileAction::ManualRecoveryRequired
        );
        assert_eq!(
            reconcile(
                &disconnected,
                OwnedObservation {
                    tun_count: 1,
                    ..empty
                }
            ),
            ReconcileAction::ManualRecoveryRequired
        );
    }
}
