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
            if let Some(hello) = ready_reply(&paths, "system.hello", end)
                && let Some(status) = ready_reply(&paths, "status.get", end)
                && ready_pair(&hello, &status)
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(Refused)
    }
}
use std::os::unix::fs::OpenOptionsExt;
fn ready_reply(paths: &RuntimePaths, method: &str, end: Instant) -> Option<serde_json::Value> {
    let stream = ready_peer(paths, end)?;
    let remaining = end.checked_duration_since(Instant::now())?;
    if remaining.is_zero() {
        return None;
    }
    crate::call_stream_with_timeout(
        stream,
        Uid::current().as_raw(),
        method,
        if method == "system.hello" {
            serde_json::json!({"versions":[1]})
        } else {
            serde_json::json!({})
        },
        remaining,
    )
    .ok()
}
fn ready_pair(hello: &serde_json::Value, status: &serde_json::Value) -> bool {
    let instance = hello["result"]["instanceId"].as_str();
    hello["ok"] == true
        && hello["result"]["runtimeOwnership"] == true
        && instance.is_some_and(|value| !value.is_empty())
        && status["ok"] == true
        && status["result"]["instanceId"].as_str() == instance
        && status["result"]["runtimeOwnership"] == true
        && status["result"]["desired"] == "disconnected"
        && status["result"]["actual"] == "disconnected"
        && status["result"]["transition"].is_null()
}
fn connect_ready(path: &std::path::Path) -> Option<std::os::unix::net::UnixStream> {
    use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr, connect, socket};
    use std::os::fd::AsRawFd;
    // Backlog saturation refuses immediately; only read-only readiness polls
    // may repeat. No launch is repeated after an uncertain result.
    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        None,
    )
    .ok()?;
    connect(fd.as_raw_fd(), &UnixAddr::new(path).ok()?).ok()?;
    let stream = std::os::unix::net::UnixStream::from(fd);
    stream.set_nonblocking(false).ok()?;
    Some(stream)
}
fn ready_peer(paths: &RuntimePaths, end: Instant) -> Option<std::os::unix::net::UnixStream> {
    use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
    if Instant::now() >= end {
        return None;
    }
    let uid = Uid::current().as_raw();
    if crate::validate_client_directory(&paths.directory, uid).is_err() {
        return None;
    }
    let Ok(socket) = fs::symlink_metadata(&paths.socket) else {
        return None;
    };
    use std::os::unix::fs::FileTypeExt;
    if !socket.file_type().is_socket() || socket.uid() != uid || socket.mode() & 0o7777 != 0o600 {
        return None;
    }
    let stream = connect_ready(&paths.socket)?;
    let Ok(peer) = getsockopt(&stream, PeerCredentials) else {
        return None;
    };
    let mut query = systemctl();
    query.args([
        "--user",
        "show",
        "omavless-runtime.service",
        "--no-pager",
        "--property=ActiveState,MainPID",
    ]);
    let remaining = end.checked_duration_since(Instant::now())?;
    if remaining.is_zero() {
        return None;
    }
    let Ok(raw) = crate::production_observation::bounded_fixed_query(
        query,
        remaining.min(Duration::from_secs(2)),
    ) else {
        return None;
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
        return None;
    }
    let (Ok(running), Ok(current)) = (
        fs::metadata(format!("/proc/{}/exe", peer.pid())),
        fs::metadata("/proc/self/exe"),
    ) else {
        return None;
    };
    (running.dev() == current.dev() && running.ino() == current.ino()).then_some(stream)
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
    fn ready_replies_require_same_nonempty_instance_and_owned_off() {
        let hello = serde_json::json!({"ok":true,"result":{"instanceId":"test-owner", "runtimeOwnership":true}});
        let status = serde_json::json!({"ok":true,"result":{"instanceId":"test-owner", "runtimeOwnership":true,
            "desired":"disconnected", "actual":"disconnected", "transition":null}});
        assert!(ready_pair(&hello, &status));
        for (key, value) in [
            ("instanceId", serde_json::json!("successor")),
            ("runtimeOwnership", serde_json::json!(false)),
            ("desired", serde_json::json!("connected")),
            ("actual", serde_json::json!("unknown")),
            ("transition", serde_json::json!({"phase":"connecting"})),
        ] {
            let mut altered = status.clone();
            altered["result"][key] = value;
            assert!(!ready_pair(&hello, &altered));
        }
        for empty in [serde_json::Value::Null, serde_json::json!("")] {
            let mut h = hello.clone();
            let mut s = status.clone();
            h["result"]["instanceId"] = empty.clone();
            s["result"]["instanceId"] = empty;
            assert!(!ready_pair(&h, &s));
        }
    }
    #[test]
    fn saturated_backlog_refuses_without_blocking() {
        use nix::sys::socket::{
            AddressFamily, Backlog, SockFlag, SockType, UnixAddr, bind, listen, socket,
        };
        use std::os::fd::AsRawFd;
        let root = crate::test_temp::directory("relaunch-backlog").unwrap();
        let path = root.join("s");
        let fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .unwrap();
        bind(fd.as_raw_fd(), &UnixAddr::new(&path).unwrap()).unwrap();
        listen(&fd, Backlog::new(1).unwrap()).unwrap();
        let start = Instant::now();
        let mut retained = vec![];
        let mut refused = false;
        for _ in 0..8 {
            if let Some(stream) = connect_ready(&path) {
                retained.push(stream);
            } else {
                refused = true;
                break;
            }
        }
        assert!(refused);
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(retained);
        drop(fd);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn retained_stream_does_not_reconnect_to_replaced_path() {
        use omavless_control_protocol::{
            FrameKind, decode_request, encode_response, read_unary_frame, write_unary_frame,
        };
        use std::os::unix::net::UnixListener;
        let root = crate::test_temp::directory("relaunch-retained").unwrap();
        let path = root.join("s");
        let listener = UnixListener::bind(&path).unwrap();
        let stream = connect_ready(&path).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        fs::remove_file(&path).unwrap();
        let successor = UnixListener::bind(&path).unwrap();
        successor.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            let request =
                decode_request(&read_unary_frame(&mut peer, FrameKind::Request).unwrap()).unwrap();
            let response = omavless_control_protocol::success_response(
                request["id"].as_str().unwrap(),
                0,
                serde_json::json!({"instanceId":"original"}),
            )
            .unwrap();
            write_unary_frame(
                &mut peer,
                &encode_response(&response).unwrap(),
                FrameKind::Response,
            )
            .unwrap();
        });
        let reply = crate::call_stream_with_timeout(
            stream,
            Uid::current().as_raw(),
            "status.get",
            serde_json::json!({}),
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(reply["result"]["instanceId"], "original");
        assert!(successor.accept().is_err());
        server.join().unwrap();
        drop(successor);
        drop(listener);
        fs::remove_dir_all(root).unwrap();
    }
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
