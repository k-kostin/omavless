// SPDX-License-Identifier: MIT

//! Fixed-argv, parent-owned Mihomo child supervision for R5.

use crate::core_readiness::ConfigReadiness;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
use nix::unistd::{Pid, getpgid};
use omavless_mihomo::{ErrorKind, ReadOnlyEndpoint, controller_get};
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
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
            .process_group(0)
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
        let pid = self.pid().ok_or(CoreError::StopFailed)?;
        let pid = i32::try_from(pid).map_err(|_| CoreError::StopFailed)?;
        // Do not reap on observation. The waitable leader pins its process-group
        // ID until stop has drained its helpers, including post-exit helpers.
        match waitid(
            Id::Pid(Pid::from_raw(pid)),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
        ) {
            Ok(WaitStatus::StillAlive) => Ok(true),
            Ok(WaitStatus::Exited(..) | WaitStatus::Signaled(..)) => Ok(false),
            _ => Err(CoreError::StopFailed),
        }
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
            Ok(response) => Ok(response.has_live_version()),
            Err(error) if error.kind() == ErrorKind::ControllerUnavailable => Ok(false),
            Err(_) => Err(CoreError::ReadinessTimedOut),
        }
    }

    pub fn wait_ready(&mut self, timeout: Duration) -> Result<(), CoreError> {
        self.wait_for(timeout, None)
    }

    pub(crate) fn wait_configured(
        &mut self,
        timeout: Duration,
        expected: &ConfigReadiness,
    ) -> Result<(), CoreError> {
        self.wait_for(timeout, Some(expected))
    }

    pub(crate) fn configured_ready(&self, timeout: Duration, expected: &ConfigReadiness) -> bool {
        !timeout.is_zero()
            && timeout <= Duration::from_secs(5)
            && expected.ready(&self.controller_socket, Instant::now() + timeout)
    }

    fn wait_for(
        &mut self,
        timeout: Duration,
        expected: Option<&ConfigReadiness>,
    ) -> Result<(), CoreError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(CoreError::InvalidArgument);
        }
        let deadline = Instant::now() + timeout;
        loop {
            if !self.running()? {
                return Err(CoreError::ExitedBeforeReady);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|value| !value.is_zero())
                .ok_or(CoreError::ReadinessTimedOut)?;
            let budget = remaining.min(Duration::from_millis(250));
            let ready = match expected {
                Some(expected) => {
                    let attempt_deadline = Instant::now() + budget;
                    if expected.ready(&self.controller_socket, attempt_deadline) {
                        true
                    } else {
                        // Startup-only correction; configured_ready remains a
                        // read-only observation and cannot change selectors.
                        self.running()?
                            && self.pid().is_some_and(|pid| {
                                expected.restore_selection(
                                    &self.controller_socket,
                                    pid,
                                    attempt_deadline,
                                ) && expected.ready(&self.controller_socket, attempt_deadline)
                            })
                    }
                }
                None => self.controller_ready(budget).unwrap_or(false),
            };
            if ready && Instant::now() < deadline {
                return if self.running()? {
                    Ok(())
                } else {
                    Err(CoreError::ExitedBeforeReady)
                };
            }
            thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
        }
    }

    pub fn stop(&mut self, timeout: Duration) -> Result<StopOutcome, CoreError> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(CoreError::InvalidArgument);
        }
        // Establish that this is still our unreaped child before signalling.
        // No path in OwnedCore reaps before group cleanup succeeds.
        let _ = self.running()?;
        let pid = i32::try_from(self.pid().ok_or(CoreError::StopFailed)?)
            .map_err(|_| CoreError::StopFailed)?;
        if pid <= 1
            || getpgid(Some(Pid::from_raw(pid))).map_err(|_| CoreError::StopFailed)?
                != Pid::from_raw(pid)
        {
            return Err(CoreError::StopFailed);
        }
        kill(Pid::from_raw(-pid), Signal::SIGTERM).map_err(|_| CoreError::StopFailed)?;
        let deadline = Instant::now() + timeout;
        let grace_deadline = Instant::now() + timeout.mul_f32(0.8);
        let mut forced = false;
        let mut empty_observations = 0;
        while Instant::now() < deadline {
            if !self.running()?
                && !crate::core_group::group_has_live_members(Path::new("/proc"), pid)
                    .map_err(|_| CoreError::StopFailed)?
            {
                empty_observations += 1;
                if empty_observations >= 2 {
                    self.child
                        .as_mut()
                        .ok_or(CoreError::StopFailed)?
                        .wait()
                        .map_err(|_| CoreError::StopFailed)?;
                    self.child.take();
                    return Ok(StopOutcome { graceful: !forced });
                }
            } else {
                empty_observations = 0;
            }
            if Instant::now() >= grace_deadline {
                // The leader remains unreaped, so even after its exit this
                // cannot address a recycled PGID. Repeat for late-forked helpers.
                kill(Pid::from_raw(-pid), Signal::SIGKILL).map_err(|_| CoreError::StopFailed)?;
                forced = true;
            }
            thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
        }
        // Keep the identity pinned for retry/manual recovery, never pretend
        // the tree is gone merely because the direct child exited.
        Err(CoreError::StopFailed)
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
        let staged = root.join(".fake-core.staged");
        fs::write(&staged, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(staged, &path).unwrap();
        // Some overlay-backed test runners briefly report ETXTBSY when an
        // executable is spawned immediately after publication. Production
        // starts an already-installed Mihomo binary, so keep this bounded
        // settling delay confined to the ephemeral test fixture.
        thread::sleep(Duration::from_millis(20));
        path
    }

    fn config(root: &Path) -> PathBuf {
        let path = root.join("config.yaml");
        fs::write(&path, "mode: rule\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }

    #[test]
    fn helper_resources_are_drained_even_after_leader_exit_or_term_spawn() {
        use nix::fcntl::{Flock, FlockArg};
        struct Unrelated(Child);
        impl Drop for Unrelated {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut unrelated = Unrelated(Command::new("/usr/bin/sleep").arg("60").spawn().unwrap());
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_file(self.0.join("keep-running"));
            }
        }
        for (spawn, helper, early) in [
            ("startup", "exit-term", false),
            ("startup", "ignore-term", false),
            ("term", "ignore-term", false),
            ("startup", "ignore-term", true),
        ] {
            let root = root("inherited-resource");
            let cleanup = Cleanup(root.clone());
            let fixture = root.join("fake-core");
            fs::write(
                &fixture,
                include_str!("../../../tools/owned_core_helper_fixture.py"),
            )
            .unwrap();
            fs::set_permissions(&fixture, fs::Permissions::from_mode(0o700)).unwrap();
            let config = root.join("config.json");
            fs::write(&config, format!(r#"{{"spawn":"{spawn}","helper":"{helper}","detach":false,"exitAfterReady":{early}}}"#)).unwrap();
            let mut core =
                OwnedCore::spawn(&fixture, &root, &config, &root.join("unused.sock")).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while !root.join("core-ready").exists() {
                assert!(Instant::now() < deadline, "fixture readiness");
                thread::sleep(POLL_INTERVAL);
            }
            if early {
                while core.running().unwrap() {
                    assert!(Instant::now() < deadline, "early exit");
                    thread::sleep(POLL_INTERVAL);
                }
                // Repeated observations must leave the zombie/PGID pinned.
                assert!(!core.running().unwrap());
            }
            let resource = fs::File::open(root.join("resource")).unwrap();
            assert!(Flock::lock(resource, FlockArg::LockExclusiveNonblock).is_err());
            core.stop(Duration::from_secs(1)).unwrap();
            let resource = fs::File::open(root.join("resource")).unwrap();
            let released = Flock::lock(resource, FlockArg::LockExclusiveNonblock);
            assert!(
                released.is_ok(),
                "stop returned with an inherited resource held"
            );
            assert!(core.pid().is_none());
            assert!(unrelated.0.try_wait().unwrap().is_none());
            drop(released);
            drop(core);
            drop(cleanup);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn lost_waitable_child_identity_refuses_group_signal() {
        let root = root("reaped-identity");
        let executable = script(&root, "exit 0");
        let config = config(&root);
        let mut core =
            OwnedCore::spawn(&executable, &root, &config, &root.join("unused.sock")).unwrap();
        core.child.as_mut().unwrap().wait().unwrap();
        assert_eq!(
            core.stop(Duration::from_secs(1)),
            Err(CoreError::StopFailed)
        );
        core.child.take();
        fs::remove_dir_all(root).unwrap();
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
        assert_eq!(
            owned.wait_configured(
                Duration::from_secs(1),
                &ConfigReadiness::new(crate::desired::RoutingMode::Global, "Synthetic".into())
            ),
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
    fn live_controller_is_not_configured_until_mode_and_selectors_converge() {
        use crate::desired::RoutingMode;
        use serde_json::json;
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;
        use std::sync::{
            Arc,
            atomic::{AtomicU8, Ordering},
        };

        let root = root("configuration-barrier");
        let socket = root.join("controller.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let state = Arc::new(AtomicU8::new(0));
        let serving = Arc::clone(&state);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline && serving.load(Ordering::SeqCst) != 255 {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .unwrap();
                let mut request = [0_u8; 512];
                let Ok(count) = stream.read(&mut request) else {
                    continue;
                };
                let request = std::str::from_utf8(&request[..count]).unwrap();
                let stage = serving.load(Ordering::SeqCst);
                let payload = if request.starts_with("GET /version ") {
                    json!({"version":"synthetic"})
                } else if request.starts_with("GET /configs ") {
                    json!({"mode": if stage == 0 {"direct"} else {"global"}})
                } else if request.starts_with("GET /rules ") {
                    json!({"rules":[]})
                } else if request.starts_with("GET /providers/rules ") {
                    json!({"providers":{}})
                } else if request.starts_with("GET /proxies ") {
                    json!({"proxies": {
                        "Synthetic": {"type":"Vless"},
                        "PROXY": {"type":"Selector", "now":"Synthetic", "all":["Synthetic"]},
                        "GLOBAL": {"type":"Selector", "now":if stage == 2 {"DIRECT"} else {"PROXY"}, "all":["PROXY"]}
                    }})
                } else {
                    panic!("unexpected fixed endpoint");
                };
                let body = payload.to_string();
                let status = if stage == 3 {
                    "503 Unavailable"
                } else {
                    "200 OK"
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let core = script(
            &root,
            "trap 'exit 0' TERM INT\nwhile :; do sleep 0.05; done",
        );
        let mut owned = OwnedCore::spawn(&core, &root, &config(&root), &socket).unwrap();
        let expected = ConfigReadiness::new(RoutingMode::Global, "Synthetic".into());
        assert!(owned.controller_ready(Duration::from_millis(250)).unwrap());
        let started = Instant::now();
        assert_eq!(
            owned.wait_configured(Duration::from_millis(80), &expected),
            Err(CoreError::ReadinessTimedOut)
        );
        assert!(started.elapsed() < Duration::from_millis(700));
        let changing = Arc::clone(&state);
        let delayed = thread::spawn(move || {
            thread::sleep(Duration::from_millis(60));
            changing.store(1, Ordering::SeqCst);
        });
        owned
            .wait_configured(Duration::from_secs(1), &expected)
            .unwrap();
        delayed.join().unwrap();
        state.store(2, Ordering::SeqCst);
        assert!(!owned.configured_ready(Duration::from_millis(250), &expected));
        state.store(3, Ordering::SeqCst);
        assert!(!owned.configured_ready(Duration::from_millis(250), &expected));
        assert!(owned.stop(Duration::from_secs(2)).unwrap().graceful);
        assert!(owned.pid().is_none());
        state.store(255, Ordering::SeqCst);
        worker.join().unwrap();
        fs::remove_dir_all(root).unwrap();
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
