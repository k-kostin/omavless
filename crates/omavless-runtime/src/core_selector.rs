// SPDX-License-Identifier: MIT
//! Fixed startup selection and authenticated read-only configured observation.
//! No IPC forwarding or caller-selected HTTP operation.
//! The expected profile is private; neither requests nor responses are formatted
//! in public errors. Observation can use only the fixed GET helper, never repair.

use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use nix::unistd::Uid;
use omavless_mihomo::{MAX_CONTROLLER_RESPONSE_BYTES, parse_controller_response};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

enum Request<'a> {
    Configs,
    Proxies,
    Rules,
    Providers,
    Profile(&'a str),
    Global,
}

fn exchange(path: &Path, pid: u32, request: Request<'_>, deadline: Instant) -> Option<Value> {
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    };
    remaining()?;
    let uid = Uid::current().as_raw();
    let directory = fs::symlink_metadata(path.parent()?).ok()?;
    let before = fs::symlink_metadata(path).ok()?;
    if !directory.is_dir()
        || directory.file_type().is_symlink()
        || directory.uid() != uid
        || directory.mode() & 0o7777 != 0o700
        || !before.file_type().is_socket()
        || before.uid() != uid
        // Mihomo creates its socket as 0666. The mandatory private 0700
        // parent is the access boundary; exact same-UID child credentials
        // below authenticate the server. This is not control.sock's policy.
        || !matches!(before.mode() & 0o7777, 0o600 | 0o666)
    {
        return None;
    }
    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        None,
    )
    .ok()?;
    connect(fd.as_raw_fd(), &UnixAddr::new(path).ok()?).ok()?;
    let mut stream = UnixStream::from(fd);
    let peer = getsockopt(&stream, PeerCredentials).ok()?;
    let after = fs::symlink_metadata(path).ok()?;
    // Check the owned child on every connection before sending private bytes.
    if peer.uid() != uid
        || u32::try_from(peer.pid()).ok()? != pid
        || before.dev() != after.dev()
        || before.ino() != after.ino()
    {
        return None;
    }
    let (method, endpoint, body) = match request {
        Request::Configs => ("GET", "/configs", String::new()),
        Request::Proxies => ("GET", "/proxies", String::new()),
        Request::Rules => ("GET", "/rules", String::new()),
        Request::Providers => ("GET", "/providers/rules", String::new()),
        Request::Profile(name) => {
            if name.is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
                return None;
            }
            ("PUT", "/proxies/PROXY", json!({"name":name}).to_string())
        }
        Request::Global => (
            "PUT",
            "/proxies/GLOBAL",
            json!({"name":"PROXY"}).to_string(),
        ),
    };
    let request = format!(
        "{method} {endpoint} HTTP/1.0\r\nHost: localhost\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.set_nonblocking(false).ok()?;
    let mut pending = request.as_bytes();
    while !pending.is_empty() {
        stream.set_write_timeout(Some(remaining()?)).ok()?;
        let count = stream.write(pending).ok()?;
        if count == 0 {
            return None;
        }
        pending = &pending[count..];
    }
    let mut response = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        stream.set_read_timeout(Some(remaining()?)).ok()?;
        let count = stream.read(&mut chunk).ok()?;
        if count == 0 {
            break;
        }
        if response.len().saturating_add(count) > MAX_CONTROLLER_RESPONSE_BYTES {
            return None;
        }
        response.extend_from_slice(&chunk[..count]);
    }
    remaining()?;
    let response = parse_controller_response(&response).ok()?;
    (response.status == if method == "PUT" { 204 } else { 200 }).then_some(response.payload)
}

fn selector<'a>(proxies: &'a Value, group: &str, target: &str) -> Option<&'a str> {
    let group = &proxies["proxies"][group];
    if group["type"] != "Selector"
        || !group["all"]
            .as_array()?
            .iter()
            .any(|member| member.as_str() == Some(target))
    {
        return None;
    }
    group["now"].as_str()
}

/// Fixed read-only configured-admission endpoints using the same exact-peer
/// transport as startup selection. No mutation can be selected by this API.
pub(crate) fn read_configuration(
    path: &Path,
    pid: u32,
    endpoint: omavless_mihomo::ReadOnlyEndpoint,
    deadline: Instant,
) -> Option<Value> {
    use omavless_mihomo::ReadOnlyEndpoint;
    let request = match endpoint {
        ReadOnlyEndpoint::Configs => Request::Configs,
        ReadOnlyEndpoint::Proxies => Request::Proxies,
        ReadOnlyEndpoint::Rules => Request::Rules,
        ReadOnlyEndpoint::RuleProviders => Request::Providers,
        _ => return None,
    };
    exchange(path, pid, request, deadline)
}

/// One bounded attempt; caller retains the single startup deadline and child
/// exit checks. Retry only inside startup, never from observation/adoption.
pub(crate) fn restore_global(path: &Path, pid: u32, profile: &str, deadline: Instant) -> bool {
    let Some(config) = exchange(path, pid, Request::Configs, deadline) else {
        return false;
    };
    if config["mode"] != "global" {
        return false;
    }
    let Some(proxies) = exchange(path, pid, Request::Proxies, deadline) else {
        return false;
    };
    // Require the whole expected topology before the first effect, not a
    // guessed direct GLOBAL -> profile mapping or an arbitrary group type.
    if !proxies["proxies"][profile].is_object()
        || selector(&proxies, "PROXY", profile).is_none()
        || selector(&proxies, "GLOBAL", "PROXY").is_none()
    {
        return false;
    }
    if selector(&proxies, "PROXY", profile) != Some(profile) {
        if exchange(path, pid, Request::Profile(profile), deadline).is_none() {
            return false;
        }
        let Some(verified) = exchange(path, pid, Request::Proxies, deadline) else {
            return false;
        };
        if selector(&verified, "PROXY", profile) != Some(profile) {
            return false;
        }
    }
    if selector(&proxies, "GLOBAL", "PROXY") != Some("PROXY")
        && exchange(path, pid, Request::Global, deadline).is_none()
    {
        return false;
    }
    // A 204 alone is not retained-selection proof. Full configured admission
    // (including rules/providers) runs separately before startup can succeed.
    exchange(path, pid, Request::Proxies, deadline).is_some_and(|verified| {
        selector(&verified, "PROXY", profile) == Some(profile)
            && selector(&verified, "GLOBAL", "PROXY") == Some("PROXY")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;
    use std::time::Duration;

    struct Controller {
        root: std::path::PathBuf,
        stop: Arc<AtomicBool>,
        puts: Arc<Mutex<Vec<(String, String)>>>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl Controller {
        fn new(mode: &str, target: &str, broken: &str) -> Self {
            let root = crate::test_temp::directory("selector").unwrap();
            let path = root.join("mihomo.sock");
            let listener = UnixListener::bind(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let puts = Arc::new(Mutex::new(Vec::new()));
            let stopped = Arc::clone(&stop);
            let recorded = Arc::clone(&puts);
            let mode = mode.to_owned();
            let broken = broken.to_owned();
            let mut proxies = json!({"proxies": {
                target: {"type":"Vless"},
                "PROXY": {"type":"Selector", "now":"DIRECT", "all":[target,"DIRECT"]},
                "GLOBAL": {"type":"Selector", "now":"DIRECT", "all":["PROXY","DIRECT"]}
            }});
            if broken == "membership" {
                proxies["proxies"]["GLOBAL"]["all"] = json!(["DIRECT"]);
            }
            if broken == "type" {
                proxies["proxies"]["PROXY"]["type"] = json!("URLTest");
            }
            if broken == "correct" {
                proxies["proxies"]["PROXY"]["now"] = json!(target);
                proxies["proxies"]["GLOBAL"]["now"] = json!("PROXY");
            }
            let worker = thread::spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 4096];
                    while let Ok(n) = stream.read(&mut chunk) {
                        if n == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&chunk[..n]);
                        assert!(bytes.len() < 8192);
                        if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                            let header = std::str::from_utf8(&bytes[..end]).unwrap();
                            let length: usize = header
                                .lines()
                                .find_map(|line| line.strip_prefix("Content-Length: "))
                                .unwrap_or("0")
                                .parse()
                                .unwrap();
                            if bytes.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    if bytes.is_empty() {
                        continue;
                    } // Wrong peer: no request bytes.
                    if broken == "slow" {
                        thread::sleep(Duration::from_millis(150));
                    }
                    let request = std::str::from_utf8(&bytes).unwrap();
                    let mut status = "200 OK";
                    let payload = if request.starts_with("GET /configs ") {
                        json!({"mode":mode})
                    } else if request.starts_with("GET /rules ") {
                        json!({"rules":[]})
                    } else if request.starts_with("GET /providers/rules ") {
                        json!({"providers":{}})
                    } else if request.starts_with("GET /proxies ") {
                        proxies.clone()
                    } else {
                        let selector = if request.starts_with("PUT /proxies/PROXY ") {
                            "PROXY"
                        } else if request.starts_with("PUT /proxies/GLOBAL ") {
                            "GLOBAL"
                        } else {
                            panic!("unexpected fixed request");
                        };
                        let body: Value =
                            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1)
                                .unwrap();
                        let name = body["name"].as_str().unwrap();
                        recorded
                            .lock()
                            .unwrap()
                            .push((selector.into(), name.into()));
                        if broken == "rejected" {
                            status = "400 Bad Request";
                        } else {
                            status = "204 No Content";
                            if broken != "not-retained" {
                                proxies["proxies"][selector]["now"] = json!(name);
                            }
                        }
                        Value::Null
                    };
                    let body = if status.starts_with("204") {
                        String::new()
                    } else {
                        payload.to_string()
                    };
                    let response = format!(
                        "HTTP/1.0 {status}\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            Self {
                root,
                stop,
                puts,
                worker: Some(worker),
            }
        }
        fn run(&self, target: &str) -> bool {
            restore_global(
                &self.root.join("mihomo.sock"),
                std::process::id(),
                target,
                Instant::now() + Duration::from_secs(1),
            )
        }
    }
    impl Drop for Controller {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn restores_nested_selection_and_never_repeats_already_correct_puts() {
        let target = "Synthetic \"雪\" /?";
        let controller = Controller::new("global", target, "");
        assert!(controller.run(target));
        assert_eq!(
            *controller.puts.lock().unwrap(),
            vec![
                ("PROXY".into(), target.into()),
                ("GLOBAL".into(), "PROXY".into())
            ]
        );
        assert!(controller.run(target));
        assert_eq!(controller.puts.lock().unwrap().len(), 2);
    }

    #[test]
    fn no_effect_for_wrong_mode_topology_target_peer_permissions_or_deadline() {
        for (mode, broken, target) in [
            ("rule", "", "Synthetic"),
            ("global", "membership", "Synthetic"),
            ("global", "type", "Synthetic"),
            ("global", "", "Missing"),
        ] {
            let controller = Controller::new(mode, "Synthetic", broken);
            assert!(!controller.run(target));
            assert!(controller.puts.lock().unwrap().is_empty());
        }
        let controller = Controller::new("global", "Synthetic", "");
        let socket = controller.root.join("mihomo.sock");
        assert!(!restore_global(
            &socket,
            std::process::id() + 1,
            "Synthetic",
            Instant::now() + Duration::from_secs(1)
        ));
        assert!(!restore_global(
            &socket,
            std::process::id(),
            "Synthetic",
            Instant::now()
        ));
        fs::set_permissions(&controller.root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!controller.run("Synthetic"));
        assert!(controller.puts.lock().unwrap().is_empty());
    }

    #[test]
    fn accepted_put_without_retention_and_rejection_do_not_select_global() {
        for broken in ["rejected", "not-retained"] {
            let controller = Controller::new("global", "Synthetic", broken);
            assert!(!controller.run("Synthetic"));
            assert_eq!(controller.puts.lock().unwrap().len(), 1);
            assert_eq!(controller.puts.lock().unwrap()[0].0, "PROXY");
        }
    }

    #[test]
    fn slow_controller_cannot_extend_attempt_and_observation_never_repairs() {
        let controller = Controller::new("global", "Synthetic", "slow");
        let start = Instant::now();
        assert!(!restore_global(
            &controller.root.join("mihomo.sock"),
            std::process::id(),
            "Synthetic",
            start + Duration::from_millis(30)
        ));
        assert!(start.elapsed() < Duration::from_millis(120));
        assert!(controller.puts.lock().unwrap().is_empty());
        let controller = Controller::new("global", "Synthetic", "");
        let expected = crate::core_readiness::ConfigReadiness::new(
            crate::desired::RoutingMode::Global,
            "Synthetic".into(),
        );
        assert!(!expected.ready(
            &controller.root.join("mihomo.sock"),
            Instant::now() + Duration::from_secs(1)
        ));
        assert!(controller.puts.lock().unwrap().is_empty());
    }

    #[test]
    fn successful_nested_actions_match_actual_python_d1_reference() {
        use sha2::{Digest, Sha256};
        use std::process::{Command, Stdio};
        for target in ["Synthetic", "Synthetic \"雪\" /?", "имя с пробелами"] {
            let controller = Controller::new("global", target, "");
            assert!(controller.run(target));
            let actions = serde_json::to_vec(&*controller.puts.lock().unwrap()).unwrap();
            let digest = format!("{:x}", Sha256::digest(actions));
            let mut child = Command::new("python3")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tools/global_selector_reference.py"
                ))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(target.as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "safe Python reference failed");
            let reference: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                reference["digest"] == digest,
                "selection action digest differs"
            );
            assert_eq!(reference["count"], 2);
        }
    }
}
