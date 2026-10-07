// SPDX-License-Identifier: MIT
//! Explicit packaged user-service start. No enable, connect, preference reset,
//! invented receipt, shell, privileged operation or production Python fallback.
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::desired::{DesiredPaths, read_desired_snapshot};
use crate::{OwnerLock, RuntimePaths};
use nix::unistd::Uid;
use omavless_domain::private_store::parse_private_store;
use omavless_store::read_private_utf8;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    Current,
    PrepareLogin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;
impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OmaVLESS start was not verified. Check application state; do not retry an uncertain start.")
    }
}
impl std::error::Error for Refused {}
type Result<T> = std::result::Result<T, Refused>;
trait Host {
    fn admit(&mut self) -> Result<Admission>;
    fn prepare_login(&mut self) -> Result<()>;
    fn start_runtime(&mut self) -> Result<()>;
    fn verify_ready(&mut self) -> Result<()>;
}
fn execute(host: &mut impl Host) -> Result<()> {
    if host.admit()? == Admission::PrepareLogin {
        host.prepare_login()?;
        if host.admit()? != Admission::Current {
            return Err(Refused);
        }
    }
    host.start_runtime()?;
    host.verify_ready()
}
fn classify(consumed: bool, configured: bool, enabled: bool) -> Result<Admission> {
    if consumed {
        Ok(Admission::Current)
    } else if configured && !enabled {
        Ok(Admission::PrepareLogin)
    } else {
        Err(Refused)
    }
}
fn systemctl() -> Command {
    let uid = Uid::current().as_raw();
    let mut command = Command::new("/usr/bin/systemctl");
    command
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("LC_ALL", "C")
        .env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"));
    command
}
fn stopped() -> Result<()> {
    let mut command = systemctl();
    command.args([
        "--user",
        "show",
        "omavless-runtime.service",
        "--no-pager",
        "--property=LoadState,ActiveState,SubState,MainPID,Job",
    ]);
    let output =
        crate::production_observation::bounded_fixed_query(command, Duration::from_secs(2))
            .map_err(|_| Refused)?;
    stopped_response(&output)
}
fn stopped_response(output: &str) -> Result<()> {
    let mut lines = output.lines().collect::<Vec<_>>();
    lines.sort_unstable();
    if lines
        == [
            "ActiveState=inactive",
            "Job=",
            "LoadState=loaded",
            "MainPID=0",
            "SubState=dead",
        ]
    {
        Ok(())
    } else {
        Err(Refused)
    }
}
fn launch(login: bool) -> Result<()> {
    let mut command = systemctl();
    command.args([
        "--user",
        "start",
        if login {
            "omavless-login-prepare.service"
        } else {
            "omavless-runtime.service"
        },
    ]);
    crate::production_observation::bounded_fixed_query(command, Duration::from_secs(15))
        .map(|_| ())
        .map_err(|_| Refused)
}
struct Installed {
    apply: bool,
}
impl Host for Installed {
    fn admit(&mut self) -> Result<Admission> {
        let uid = Uid::current();
        if uid.is_root() || uid != Uid::effective() {
            return Err(Refused);
        }
        crate::login_activation::package().map_err(|_| Refused)?;
        stopped()?;
        let paths = RuntimePaths::current().map_err(|_| Refused)?;
        match fs::symlink_metadata(&paths.socket) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            _ => return Err(Refused),
        }
        // Existing daemon lock is borrowed read-only, never created/repaired
        // by this probe. An absent RuntimeDirectory is handled by login's unit.
        let _owner = match fs::symlink_metadata(&paths.directory) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Ok(_) => {
                crate::validate_client_directory(&paths.directory, uid.as_raw())
                    .map_err(|_| Refused)?;
                let before = fs::symlink_metadata(&paths.owner_lock).map_err(|_| Refused)?;
                if !before.is_file()
                    || before.uid() != uid.as_raw()
                    || before.mode() & 0o7777 != 0o600
                    || before.nlink() != 1
                {
                    return Err(Refused);
                }
                let file = fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
                    .open(&paths.owner_lock)
                    .map_err(|_| Refused)?;
                let after = file.metadata().map_err(|_| Refused)?;
                if before.dev() != after.dev() || before.ino() != after.ino() {
                    return Err(Refused);
                }
                let held =
                    nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock)
                        .map_err(|_| Refused)?;
                Some(OwnerLock { _file: held })
            }
            _ => return Err(Refused),
        };
        let cutover = CutoverPaths::current(uid.as_raw()).map_err(|_| Refused)?;
        // A frontend probe never creates or repairs a migration lock. Cold
        // absence provides advisory DATA only; Start re-earns the real lease.
        let lock = if self.apply {
            Some(MigrationLock::acquire(&cutover, uid.as_raw()).map_err(|_| Refused)?)
        } else {
            match fs::symlink_metadata(&cutover.operation_lock) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Ok(_) => Some(
                    MigrationLock::acquire_existing(&cutover, uid.as_raw()).map_err(|_| Refused)?,
                ),
                _ => return Err(Refused),
            }
        };
        let marker = read_marker_existing(&cutover, uid.as_raw()).map_err(|_| Refused)?;
        if marker.phase() != OwnershipPhase::Rust {
            return Err(Refused);
        }
        let desired_paths = DesiredPaths::current().map_err(|_| Refused)?;
        if !desired_paths.file.exists() {
            return Err(Refused);
        }
        let desired = read_desired_snapshot(&desired_paths, uid.as_raw()).map_err(|_| Refused)?;
        if desired.connected {
            return Err(Refused);
        }
        let epoch = crate::login_activation::manager_epoch().map_err(|_| Refused)?;
        let consumed = if let Some(lock) = &lock {
            crate::login_transaction::manual_start_receipt(
                &cutover,
                uid.as_raw(),
                lock,
                marker.generation(),
                &epoch,
            )
            .map_err(|_| Refused)?
        } else {
            match fs::symlink_metadata(cutover.runtime_base.join("omavless-login.receipt")) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                _ => return Err(Refused),
            }
            if crate::pending_private_transaction::pending_at(&cutover.state_directory) {
                return Err(Refused);
            }
            false
        };
        let home = std::env::var_os("HOME").ok_or(Refused)?;
        let raw = read_private_utf8(
            &std::path::PathBuf::from(home).join(".config/omavless/profiles.json"),
            uid.as_raw(),
        )
        .map_err(|_| Refused)?;
        let store = parse_private_store(&raw).map_err(|_| Refused)?;
        let result = classify(
            consumed,
            store.startup_is_configured(),
            store.startup_preferences().enabled,
        )?;
        stopped()?;
        // Leases end with this function, before either child service starts.
        Ok(result)
    }
    fn prepare_login(&mut self) -> Result<()> {
        launch(true)
    }
    fn start_runtime(&mut self) -> Result<()> {
        launch(false)
    }
    fn verify_ready(&mut self) -> Result<()> {
        let paths = RuntimePaths::current().map_err(|_| Refused)?;
        let end = Instant::now() + Duration::from_secs(8);
        while Instant::now() < end {
            // Bind the authenticated runtime to the fixed unit and installed
            // executable, not merely another same-user socket responder.
            if !ready_peer(&paths) {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            if let Ok(hello) =
                crate::call(&paths, "system.hello", serde_json::json!({"versions":[1]}))
                && hello["ok"] == true
                && hello["result"]["runtimeOwnership"] == true
                && let Ok(status) = crate::call(&paths, "status.get", serde_json::json!({}))
                && status["ok"] == true
                && status["result"]["runtimeOwnership"] == true
                && status["result"]["desired"] == "disconnected"
                && status["result"]["actual"] == "disconnected"
                && status["result"]["transition"].is_null()
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(Refused)
    }
}
use std::os::unix::fs::OpenOptionsExt;
fn ready_peer(paths: &RuntimePaths) -> bool {
    use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
    use std::os::unix::net::UnixStream;
    let uid = Uid::current().as_raw();
    if crate::validate_client_directory(&paths.directory, uid).is_err() {
        return false;
    }
    let Ok(socket) = fs::symlink_metadata(&paths.socket) else {
        return false;
    };
    use std::os::unix::fs::FileTypeExt;
    if !socket.file_type().is_socket() || socket.uid() != uid || socket.mode() & 0o7777 != 0o600 {
        return false;
    }
    let Ok(stream) = UnixStream::connect(&paths.socket) else {
        return false;
    };
    let Ok(peer) = getsockopt(&stream, PeerCredentials) else {
        return false;
    };
    let mut query = systemctl();
    query.args([
        "--user",
        "show",
        "omavless-runtime.service",
        "--no-pager",
        "--property=ActiveState,MainPID",
    ]);
    let Ok(raw) = crate::production_observation::bounded_fixed_query(query, Duration::from_secs(2))
    else {
        return false;
    };
    let mut lines = raw.lines().collect::<Vec<_>>();
    lines.sort_unstable();
    if peer.uid() != uid
        || peer.pid() <= 0
        || lines
            != [
                "ActiveState=active".to_owned(),
                format!("MainPID={}", peer.pid()),
            ]
    {
        return false;
    }
    let (Ok(running), Ok(current)) = (
        fs::metadata(format!("/proc/{}/exe", peer.pid())),
        fs::metadata("/proc/self/exe"),
    ) else {
        return false;
    };
    running.dev() == current.dev() && running.ino() == current.ino()
}
pub fn can_start() -> bool {
    Installed { apply: false }.admit().is_ok()
}
pub fn start() -> Result<()> {
    execute(&mut Installed { apply: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stopped_query_rejects_pending_jobs_ambiguous_and_live_states() {
        let off = "LoadState=loaded\nActiveState=inactive\nSubState=dead\nJob=\nMainPID=0\n";
        assert!(stopped_response(off).is_ok());
        for altered in [
            off.replace("Job=", "Job=55"),
            off.replace("inactive", "activating"),
            off.replace("inactive", "active"),
            off.replace("dead", "failed"),
            off.replace("loaded", "not-found"),
            off.replace("MainPID=0", "MainPID=99"),
            format!("{off}MainPID=0\n"),
        ] {
            assert!(stopped_response(&altered).is_err());
        }
    }
    struct Fake {
        admissions: Vec<Result<Admission>>,
        calls: Vec<&'static str>,
        fail: bool,
    }
    impl Host for Fake {
        fn admit(&mut self) -> Result<Admission> {
            self.calls.push("admit");
            self.admissions.remove(0)
        }
        fn prepare_login(&mut self) -> Result<()> {
            self.calls.push("login");
            if self.fail { Err(Refused) } else { Ok(()) }
        }
        fn start_runtime(&mut self) -> Result<()> {
            self.calls.push("runtime");
            if self.fail { Err(Refused) } else { Ok(()) }
        }
        fn verify_ready(&mut self) -> Result<()> {
            self.calls.push("verify");
            Ok(())
        }
    }
    #[test]
    fn only_real_consumed_receipt_allows_enabled_preferences() {
        assert_eq!(classify(true, true, true), Ok(Admission::Current));
        assert_eq!(classify(false, true, false), Ok(Admission::PrepareLogin));
        for (configured, enabled) in [(false, false), (false, true), (true, true)] {
            assert_eq!(classify(false, configured, enabled), Err(Refused));
        }
    }
    #[test]
    fn stages_recheck_before_runtime_and_never_retry_or_enable() {
        let mut current = Fake {
            admissions: vec![Ok(Admission::Current)],
            calls: vec![],
            fail: false,
        };
        assert!(execute(&mut current).is_ok());
        assert_eq!(current.calls, ["admit", "runtime", "verify"]);
        let mut first = Fake {
            admissions: vec![Ok(Admission::PrepareLogin), Ok(Admission::Current)],
            calls: vec![],
            fail: false,
        };
        assert!(execute(&mut first).is_ok());
        assert_eq!(
            first.calls,
            ["admit", "login", "admit", "runtime", "verify"]
        );
        for next in [Err(Refused), Ok(Admission::PrepareLogin)] {
            let mut changed = Fake {
                admissions: vec![Ok(Admission::PrepareLogin), next],
                calls: vec![],
                fail: false,
            };
            assert!(execute(&mut changed).is_err());
            assert_eq!(changed.calls, ["admit", "login", "admit"]);
        }
        for initial in [Ok(Admission::Current), Ok(Admission::PrepareLogin)] {
            let mut unknown = Fake {
                admissions: vec![initial],
                calls: vec![],
                fail: true,
            };
            assert!(execute(&mut unknown).is_err());
            assert!(!unknown.calls.contains(&"verify"));
        }
    }
}
