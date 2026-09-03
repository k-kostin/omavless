// SPDX-License-Identifier: MIT

//! Fixed-purpose package host adapter for the native lifecycle executor.
//!
//! This module contains no IPC surface of its own. The production owner composes
//! private store/config preparation, Mihomo validation, parent-owned startup,
//! private-controller readiness, process/TUN observation, and cleanup behind
//! [`LifecycleHost`](crate::lifecycle::LifecycleHost).

use crate::core::OwnedCore;
use crate::desired::{DesiredState, OwnedObservation};
use crate::lifecycle::{HostStepError, LifecycleHost};
use omavless_domain::config::MAX_TEMPLATE_BYTES;
use omavless_domain::private_store::parse_private_store;
use omavless_mihomo::observation::{processes_named, tun_interface_count};
use omavless_mihomo::validate_config;
use omavless_store::{atomic_replace_private, read_private_utf8};
use std::env;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(20);
const START_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const OBSERVATION_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_PATH_BYTES: usize = 4096;

/// Stable host paths resolved by package policy, never by an IPC request.
/// This type intentionally has no `Debug` implementation.
pub struct NativeHostPaths {
    pub core: PathBuf,
    pub data_directory: PathBuf,
    pub config_directory: PathBuf,
    pub runtime_directory: PathBuf,
    pub proc_root: PathBuf,
    pub sys_class_net: PathBuf,
    pub store: PathBuf,
    pub template: PathBuf,
    pub active_config: PathBuf,
    pub staged_config: PathBuf,
    pub controller_socket: PathBuf,
}

impl NativeHostPaths {
    #[must_use]
    pub fn new(
        core: PathBuf,
        data_directory: PathBuf,
        config_directory: PathBuf,
        runtime_directory: PathBuf,
        proc_root: PathBuf,
        sys_class_net: PathBuf,
    ) -> Self {
        Self {
            core,
            data_directory,
            store: config_directory.join("profiles.json"),
            template: config_directory.join("route-template.yaml"),
            active_config: config_directory.join("config.yaml"),
            staged_config: config_directory.join(".config.candidate.yaml"),
            controller_socket: runtime_directory.join("mihomo.sock"),
            config_directory,
            runtime_directory,
            proc_root,
            sys_class_net,
        }
    }

    /// Resolve the fixed package-owned host paths for the current user.
    ///
    /// The optional `OMAVLESS_HOME` override exists only for isolated package
    /// acceptance tests. No path is accepted from IPC, and the Mihomo/proc/sys
    /// entry points remain fixed by package policy.
    pub fn current(runtime_directory: &Path) -> Result<Self, HostStepError> {
        let home = env::var_os("OMAVLESS_HOME")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or(HostStepError::Prepare)?;
        if !valid_absolute(&home) || !valid_absolute(runtime_directory) {
            return Err(HostStepError::Prepare);
        }
        let config = home.join(".config/omavless");
        Ok(Self::new(
            resolve_core(&home, env::var_os("OMAVLESS_MIHOMO"), env::var_os("PATH"))?,
            config.clone(),
            config,
            runtime_directory.to_path_buf(),
            PathBuf::from("/proc"),
            PathBuf::from("/sys/class/net"),
        ))
    }
}

/// Resolve the same stable Mihomo entry-point classes accepted by the current
/// plugin: an explicit absolute override, the current user's local binary, or
/// an absolute PATH entry. The resolved target is canonicalized before it is
/// retained so a later lifecycle step never executes a relative path or a
/// caller-controlled shell lookup.
fn resolve_core(
    home: &Path,
    override_path: Option<std::ffi::OsString>,
    search_path: Option<std::ffi::OsString>,
) -> Result<PathBuf, HostStepError> {
    let mut candidates = Vec::new();
    if let Some(value) = override_path {
        let candidate = PathBuf::from(value);
        if !valid_absolute(&candidate) {
            return Err(HostStepError::Prepare);
        }
        candidates.push(candidate);
    }
    candidates.push(home.join(".local/bin/mihomo"));
    if let Some(value) = search_path {
        for directory in env::split_paths(&value) {
            if valid_absolute(&directory) {
                candidates.push(directory.join("mihomo"));
            }
        }
    }
    for candidate in candidates {
        let Ok(canonical) = fs::canonicalize(candidate) else {
            continue;
        };
        if valid_absolute(&canonical) && executable(&canonical) {
            return Ok(canonical);
        }
    }
    Err(HostStepError::Prepare)
}

fn valid_absolute(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    path.is_absolute() && !bytes.is_empty() && bytes.len() <= MAX_PATH_BYTES && !bytes.contains(&0)
}

fn private_directory(path: &Path, uid: u32) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_dir()
            && metadata.uid() == uid
            && metadata.permissions().mode() & 0o077 == 0
    })
}

fn ordinary_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_dir())
}

fn executable(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
    })
}

fn remove_owned_file(path: &Path, uid: u32, socket: bool) -> Result<(), HostStepError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(HostStepError::Cleanup),
    };
    let expected_type = if socket {
        metadata.file_type().is_socket()
    } else {
        metadata.is_file() && !metadata.file_type().is_symlink()
    };
    if !expected_type || metadata.uid() != uid {
        return Err(HostStepError::Cleanup);
    }
    fs::remove_file(path).map_err(|_| HostStepError::Cleanup)
}

/// Contains private profile identity and potentially an owned child. It must
/// never be formatted or serialized.
pub struct NativeLifecycleHost {
    paths: NativeHostPaths,
    uid: u32,
    core: Option<OwnedCore>,
    profile_id: Option<String>,
    previous_config: Option<Option<Vec<u8>>>,
    active_install_attempted: bool,
}

impl NativeLifecycleHost {
    pub fn new(paths: NativeHostPaths, uid: u32) -> Result<Self, HostStepError> {
        let all_paths_valid = [
            &paths.core,
            &paths.data_directory,
            &paths.config_directory,
            &paths.runtime_directory,
            &paths.proc_root,
            &paths.sys_class_net,
            &paths.store,
            &paths.template,
            &paths.active_config,
            &paths.staged_config,
            &paths.controller_socket,
        ]
        .into_iter()
        .all(|path| valid_absolute(path));
        if !all_paths_valid
            || !executable(&paths.core)
            || !private_directory(&paths.data_directory, uid)
            || !private_directory(&paths.config_directory, uid)
            || !private_directory(&paths.runtime_directory, uid)
            || !ordinary_directory(&paths.proc_root)
            || !ordinary_directory(&paths.sys_class_net)
        {
            return Err(HostStepError::Prepare);
        }
        Ok(Self {
            paths,
            uid,
            core: None,
            profile_id: None,
            previous_config: None,
            active_install_attempted: false,
        })
    }

    #[must_use]
    pub fn core_pid(&self) -> Option<u32> {
        self.core.as_ref().and_then(OwnedCore::pid)
    }

    fn remove_controller(&self) -> Result<(), HostStepError> {
        remove_owned_file(&self.paths.controller_socket, self.uid, true)
    }

    fn read_previous_config(&self) -> Result<Option<Vec<u8>>, HostStepError> {
        match fs::symlink_metadata(&self.paths.active_config) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(HostStepError::Prepare),
            Ok(_) => read_private_utf8(&self.paths.active_config, self.uid)
                .map(String::into_bytes)
                .map(Some)
                .map_err(|_| HostStepError::Prepare),
        }
    }

    fn restore_previous_config(&mut self) -> Result<(), HostStepError> {
        let Some(previous) = self.previous_config.as_ref() else {
            self.active_install_attempted = false;
            return Ok(());
        };
        match previous {
            Some(payload) => {
                atomic_replace_private(&self.paths.active_config, payload, self.uid)
                    .map_err(|_| HostStepError::Cleanup)?;
            }
            None => remove_owned_file(&self.paths.active_config, self.uid, false)?,
        }
        self.previous_config = None;
        self.active_install_attempted = false;
        Ok(())
    }

    fn visible_core_count(&self, own_pid: Option<u32>, own_running: bool) -> u8 {
        let named = processes_named(&self.paths.proc_root, "mihomo");
        let mut count = named.len();
        if own_running && own_pid.is_some_and(|pid| !named.contains(&pid)) {
            count = count.saturating_add(1);
        }
        u8::try_from(count).unwrap_or(u8::MAX)
    }
}

impl LifecycleHost for NativeLifecycleHost {
    fn observe(&mut self, desired: &DesiredState) -> Result<OwnedObservation, HostStepError> {
        let (own_pid, own_running, controller_ready) = match self.core.as_mut() {
            Some(core) => {
                let running = core.running().map_err(|_| HostStepError::Observation)?;
                let ready = if running {
                    core.controller_ready(OBSERVATION_TIMEOUT)
                        .map_err(|_| HostStepError::Observation)?
                } else {
                    false
                };
                (core.pid(), running, ready)
            }
            None => (None, false, false),
        };
        Ok(OwnedObservation {
            service_active: own_running,
            controller_ready,
            core_count: self.visible_core_count(own_pid, own_running),
            tun_count: tun_interface_count(&self.paths.sys_class_net),
            active_profile_matches: own_running
                && self.profile_id.as_deref() == Some(desired.profile_id.as_str()),
        })
    }

    fn prepare(&mut self, desired: &DesiredState) -> Result<(), HostStepError> {
        if self.core.is_some() || self.profile_id.is_some() {
            return Err(HostStepError::Prepare);
        }
        remove_owned_file(&self.paths.staged_config, self.uid, false)?;
        self.previous_config = Some(self.read_previous_config()?);
        self.active_install_attempted = false;
        let store_text =
            read_private_utf8(&self.paths.store, self.uid).map_err(|_| HostStepError::Prepare)?;
        let template = read_private_utf8(&self.paths.template, self.uid)
            .map_err(|_| HostStepError::Prepare)?;
        if template.len() > MAX_TEMPLATE_BYTES {
            return Err(HostStepError::Prepare);
        }
        let store = parse_private_store(&store_text).map_err(|_| HostStepError::Prepare)?;
        let controller = self
            .paths
            .controller_socket
            .to_str()
            .ok_or(HostStepError::Prepare)?;
        let config = store
            .prepare_config_mode(
                &desired.profile_id,
                &template,
                controller,
                desired.mode.as_str(),
            )
            .map_err(|_| HostStepError::Prepare)?;
        atomic_replace_private(&self.paths.staged_config, config.as_bytes(), self.uid)
            .map_err(|_| HostStepError::Prepare)?;
        if validate_config(
            &self.paths.core,
            &self.paths.data_directory,
            &self.paths.staged_config,
            VALIDATION_TIMEOUT,
        )
        .is_err()
        {
            let _ = remove_owned_file(&self.paths.staged_config, self.uid, false);
            return Err(HostStepError::Prepare);
        }
        self.profile_id = Some(desired.profile_id.clone());
        Ok(())
    }

    fn start_prepared(&mut self) -> Result<(), HostStepError> {
        if self.core.is_some() || self.profile_id.is_none() {
            return Err(HostStepError::Start);
        }
        self.remove_controller()?;
        let mut core = OwnedCore::spawn(
            &self.paths.core,
            &self.paths.data_directory,
            &self.paths.staged_config,
            &self.paths.controller_socket,
        )
        .map_err(|_| HostStepError::Start)?;
        let ready = core.wait_ready(START_TIMEOUT);
        self.core = Some(core);
        ready.map_err(|_| HostStepError::Start)
    }

    fn commit_prepared(&mut self) -> Result<(), HostStepError> {
        if self.core.is_none() || self.profile_id.is_none() {
            return Err(HostStepError::Commit);
        }
        let config = read_private_utf8(&self.paths.staged_config, self.uid)
            .map_err(|_| HostStepError::Commit)?;
        remove_owned_file(&self.paths.staged_config, self.uid, false)
            .map_err(|_| HostStepError::Commit)?;
        self.active_install_attempted = true;
        if atomic_replace_private(&self.paths.active_config, config.as_bytes(), self.uid).is_err() {
            let _ = self.restore_previous_config();
            return Err(HostStepError::Commit);
        }
        self.previous_config = None;
        self.active_install_attempted = false;
        Ok(())
    }

    fn stop_owned(&mut self) -> Result<(), HostStepError> {
        if let Some(mut core) = self.core.take()
            && core.stop(STOP_TIMEOUT).is_err()
        {
            self.core = Some(core);
            return Err(HostStepError::Stop);
        }
        self.remove_controller().map_err(|_| HostStepError::Stop)?;
        self.profile_id = None;
        Ok(())
    }

    fn discard_prepared(&mut self) -> Result<(), HostStepError> {
        remove_owned_file(&self.paths.staged_config, self.uid, false)?;
        if self.active_install_attempted {
            self.restore_previous_config()?;
        } else {
            self.previous_config = None;
        }
        if self.core.is_none() {
            self.profile_id = None;
        }
        Ok(())
    }
}

impl Drop for NativeLifecycleHost {
    fn drop(&mut self) {
        if let Some(mut core) = self.core.take() {
            let _ = core.stop(STOP_TIMEOUT);
        }
        let _ = self.remove_controller();
        let _ = remove_owned_file(&self.paths.staged_config, self.uid, false);
        if self.active_install_attempted {
            let _ = self.restore_previous_config();
        }
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
        let root = std::env::temp_dir().join(format!(
            "omavless-native-host-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn executable_at(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn core_discovery_preserves_override_local_and_absolute_path_contract() {
        let (root, _uid) = root("core-discovery");
        let home = root.join("home");
        let local = home.join(".local/bin/mihomo");
        let path_core = root.join("bin/mihomo");
        let override_core = root.join("override/mihomo");
        executable_at(&local);
        executable_at(&path_core);
        executable_at(&override_core);

        assert_eq!(
            resolve_core(
                &home,
                Some(override_core.clone().into_os_string()),
                Some(root.join("bin").into_os_string()),
            )
            .unwrap(),
            override_core
        );
        assert_eq!(
            resolve_core(&home, None, Some(root.join("bin").into_os_string())).unwrap(),
            local
        );
        fs::remove_file(&local).unwrap();
        assert_eq!(
            resolve_core(&home, None, Some(root.join("bin").into_os_string())).unwrap(),
            path_core
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_explicit_override_and_relative_path_fail_closed() {
        let (root, _uid) = root("core-discovery-invalid");
        let home = root.join("home");
        let local = home.join(".local/bin/mihomo");
        executable_at(&local);
        assert_eq!(
            resolve_core(
                &home,
                Some(std::ffi::OsString::from("relative/mihomo")),
                None,
            ),
            Err(HostStepError::Prepare)
        );
        fs::remove_file(local).unwrap();
        assert_eq!(
            resolve_core(
                &home,
                None,
                Some(std::ffi::OsString::from("relative:also-relative")),
            ),
            Err(HostStepError::Prepare)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_directories_and_symlinked_executable_without_path_leaks() {
        let (root, uid) = root("unsafe");
        for name in ["data", "config", "runtime", "proc", "sys"] {
            fs::create_dir(root.join(name)).unwrap();
        }
        let real_core = root.join("real-core");
        fs::write(&real_core, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&real_core, fs::Permissions::from_mode(0o700)).unwrap();
        let core = root.join("private.example-password");
        symlink(real_core, &core).unwrap();
        let paths = NativeHostPaths::new(
            core,
            root.join("data"),
            root.join("config"),
            root.join("runtime"),
            root.join("proc"),
            root.join("sys"),
        );
        let error = match NativeLifecycleHost::new(paths, uid) {
            Ok(_) => panic!("unsafe host paths accepted"),
            Err(error) => error,
        };
        let public = format!("{error:?} {error}");
        assert!(!public.contains("private.example"));
        assert!(!public.contains("password"));
        fs::remove_dir_all(root).unwrap();
    }
}
