// SPDX-License-Identifier: MIT

//! Fixed-argv, parent-owned Mihomo child supervision for R5.

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use omavless_mihomo::{ErrorKind, ReadOnlyEndpoint, controller_get};
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_PATH_BYTES: usize = 4096;
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    InvalidArgument,
    SpawnFailed,
    ExitedBeforeReady,
    ReadinessTimedOut,
    StopFailed,
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidArgument => "Mihomo supervisor input is invalid",
            Self::SpawnFailed => "Mihomo child could not be started",
            Self::ExitedBeforeReady => "Mihomo child exited before becoming ready",
            Self::ReadinessTimedOut => "Mihomo child did not become ready in time",
            Self::StopFailed => "Mihomo child could not be stopped",
        })
    }
}

impl std::error::Error for CoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopOutcome {
    pub graceful: bool,
}

fn valid_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    path.is_absolute() && !bytes.is_empty() && bytes.len() <= MAX_PATH_BYTES && !bytes.contains(&0)
}

fn executable(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

pub struct OwnedCore {
    child: Option<Child>,
    controller_socket: PathBuf,
}

impl OwnedCore {
    pub fn spawn(
        core: &Path,
        data_directory: &Path,
        config: &Path,
        controller_socket: &Path,
    ) -> Result<Self, CoreError> {
        if !valid_path(core)
            || !executable(core)
            || !valid_path(data_directory)
            || !fs::metadata(data_directory).is_ok_and(|metadata| metadata.is_dir())
            || !valid_path(config)
            || !fs::metadata(config).is_ok_and(|metadata| metadata.is_file())
            || !valid_path(controller_socket)
        {
            return Err(CoreError::InvalidArgument);
        }
        let child = Command::new(core)
            .arg("-d")
            .arg(data_directory)
            .arg("-f")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| CoreError::SpawnFailed)?;
        Ok(Self {
            child: Some(child),
            controller_socket: controller_socket.to_owned(),
        })
    }

    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    pub fn running(&mut self) -> Result<bool, CoreError> {
        self.child
            .as_mut()
            .ok_or(CoreError::StopFailed)?
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|_| CoreError::StopFailed)
    }

    pub fn controller_ready(&self, timeout: Duration) -> Result<bool, CoreError> {
        if timeout.is_zero() || timeout > Duration::from_secs(5) {
            return Err(CoreError::InvalidArgument);
        }
        match controller_get(
            &self.controller_socket,
            ReadOnlyEndpoint::Version,
            timeout,
            16 * 1024,
        ) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == ErrorKind::ControllerUnavailable => Ok(false),
            Err(_) => Err(CoreError::ReadinessTimedOut),
        }
    }

    pub fn wait_ready(&mut self, timeout: Duration) -> Result<(), CoreError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(CoreError::InvalidArgument);
        }
        let deadline = Instant::now() + timeout;
        loop {
            if !self.running()? {
                return Err(CoreError::ExitedBeforeReady);
            }
            match controller_get(
                &self.controller_socket,
                ReadOnlyEndpoint::Version,
                Duration::from_millis(250),
                16 * 1024,
            ) {
                Ok(_) => return Ok(()),
                Err(error)
                    if error.kind() == ErrorKind::ControllerUnavailable
                        && Instant::now() < deadline =>
                {
                    thread::sleep(POLL_INTERVAL);
                }
                Err(error) if error.kind() == ErrorKind::ControllerUnavailable => {
                    return Err(CoreError::ReadinessTimedOut);
                }
                Err(_) => return Err(CoreError::ReadinessTimedOut),
            }
        }
    }

    pub fn stop(&mut self, timeout: Duration) -> Result<StopOutcome, CoreError> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(CoreError::InvalidArgument);
        }
        let child = self.child.as_mut().ok_or(CoreError::StopFailed)?;
        if child
            .try_wait()
            .map_err(|_| CoreError::StopFailed)?
            .is_some()
        {
            self.child.take();
            return Ok(StopOutcome { graceful: true });
        }
        let pid = i32::try_from(child.id()).map_err(|_| CoreError::StopFailed)?;
        kill(Pid::from_raw(pid), Signal::SIGTERM).map_err(|_| CoreError::StopFailed)?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if child
                .try_wait()
                .map_err(|_| CoreError::StopFailed)?
                .is_some()
            {
                self.child.take();
                return Ok(StopOutcome { graceful: true });
            }
            thread::sleep(POLL_INTERVAL);
        }
        child.kill().map_err(|_| CoreError::StopFailed)?;
        child.wait().map_err(|_| CoreError::StopFailed)?;
        self.child.take();
        Ok(StopOutcome { graceful: false })
    }
}

impl Drop for OwnedCore {
    fn drop(&mut self) {
        if self.child.is_some() {
            let _ = self.stop(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omavless-core-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        root
    }

    fn script(root: &Path, body: &str) -> PathBuf {
        let path = root.join("fake-core");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn config(root: &Path) -> PathBuf {
        let path = root.join("config.yaml");
        fs::write(&path, "mode: rule\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }

    #[test]
    fn fixed_argv_child_is_owned_and_stops_gracefully() {
        let root = root("graceful");
        let core = script(
            &root,
            "trap 'exit 0' TERM INT\nwhile :; do sleep 0.05; done",
        );
        let config = config(&root);
        let mut owned = OwnedCore::spawn(&core, &root, &config, &root.join("controller.sock"))
            .expect("spawn fixed child");
        assert!(owned.pid().is_some());
        assert!(owned.running().unwrap());
        assert!(owned.stop(Duration::from_secs(2)).unwrap().graceful);
        assert!(owned.pid().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn early_exit_and_readiness_timeout_are_bounded() {
        let early_root = root("early");
        let early = script(&early_root, "exit 7");
        let early_config = config(&early_root);
        let mut owned = OwnedCore::spawn(
            &early,
            &early_root,
            &early_config,
            &early_root.join("controller.sock"),
        )
        .unwrap();
        assert_eq!(
            owned.wait_ready(Duration::from_secs(1)),
            Err(CoreError::ExitedBeforeReady)
        );
        drop(owned);
        fs::remove_dir_all(early_root).unwrap();

        let timeout_root = root("timeout");
        let hanging = script(
            &timeout_root,
            "trap 'exit 0' TERM INT\nwhile :; do sleep 0.05; done",
        );
        let timeout_config = config(&timeout_root);
        let mut owned = OwnedCore::spawn(
            &hanging,
            &timeout_root,
            &timeout_config,
            &timeout_root.join("controller.sock"),
        )
        .unwrap();
        assert_eq!(
            owned.wait_ready(Duration::from_millis(80)),
            Err(CoreError::ReadinessTimedOut)
        );
        assert!(owned.stop(Duration::from_secs(2)).unwrap().graceful);
        match fs::remove_dir_all(timeout_root) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("cleanup failed: {error}"),
        }
    }

    #[test]
    fn errors_never_include_private_paths() {
        let marker = Path::new("/private.example/password");
        let error = match OwnedCore::spawn(marker, marker, marker, marker) {
            Ok(_) => panic!("invalid private paths were accepted"),
            Err(error) => error,
        };
        let output = format!("{error:?} {error}");
        assert!(!output.contains("private.example"));
        assert!(!output.contains("password"));
    }
}
