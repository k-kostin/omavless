// SPDX-License-Identifier: MIT
//! Detached fixed CONNECT observation; no application request or arbitrary URL.
use omavless_control_protocol::StableErrorCode;
use omavless_mihomo::ReadOnlyEndpoint;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) const DEADLINE: Duration = Duration::from_secs(3);
pub(crate) enum Plan {
    Fast(Value),
    Live(Context),
}
// Deliberately not Debug: query and private redaction fragments are UI-private.
#[derive(PartialEq, Eq)]
pub(crate) struct Context {
    pub query: String,
    pub pid: u32,
    pub private: Vec<String>,
    pub desired: crate::desired::DesiredState,
    pub store_digest: [u8; 32],
    pub config_digest: [u8; 32],
}
fn remaining(deadline: Instant) -> Result<Duration, StableErrorCode> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or(StableErrorCode::CoreRejected)
}

// Prove the *accepted connection*, not a listener which could disappear and
// be rebound before connect. No query bytes are sent until this succeeds.
fn owns_accepted(
    pid: u32,
    source: u16,
    mixed: u16,
    deadline: Instant,
) -> Result<bool, StableErrorCode> {
    let unavailable = StableErrorCode::CapabilityUnavailable;
    let mut inodes = HashSet::new();
    for (index, entry) in fs::read_dir(format!("/proc/{pid}/fd"))
        .map_err(|_| unavailable)?
        .enumerate()
    {
        remaining(deadline)?;
        if index >= 4096 {
            return Err(unavailable);
        }
        // A vanished descriptor is expected; denial is not proof of ownership.
        let entry = entry.map_err(|_| unavailable)?;
        let target = match fs::read_link(entry.path()) {
            Ok(target) => target,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(unavailable),
        };
        if let Some(inode) = target
            .to_str()
            .and_then(|s| s.strip_prefix("socket:["))
            .and_then(|s| s.strip_suffix(']'))
        {
            inodes.insert(inode.to_owned());
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(format!("/proc/{pid}/net/tcp"))
        .map_err(|_| unavailable)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable)?;
    if bytes.len() > 1024 * 1024 {
        return Err(unavailable);
    }
    let table = std::str::from_utf8(&bytes).map_err(|_| unavailable)?;
    let loopback = u32::from_ne_bytes(Ipv4Addr::LOCALHOST.octets());
    let local = format!("{loopback:08X}:{mixed:04X}");
    let remote = format!("{loopback:08X}:{source:04X}");
    for (index, line) in table.lines().skip(1).enumerate() {
        remaining(deadline)?;
        if index >= 8192 {
            return Err(unavailable);
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 10
            && fields[1] == local
            && fields[2] == remote
            && fields[3] == "01"
            && inodes.contains(fields[9])
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn collect(
    directory: &Path,
    uid: u32,
    context: &Context,
    deadline: Instant,
) -> Result<Value, StableErrorCode> {
    if omavless_domain::route_check::canonical_query(&context.query)
        .ok()
        .as_deref()
        != Some(context.query.as_str())
    {
        return Err(StableErrorCode::InvalidArgument);
    }
    let socket = directory.join("mihomo.sock");
    let read = |endpoint| {
        crate::diagnostic_read::read_owned(&socket, uid, endpoint, deadline, Some(context.pid))
    };
    let config = read(ReadOnlyEndpoint::Configs)?;
    if config.get("mode").and_then(Value::as_str) != Some("rule") {
        return Err(StableErrorCode::CapabilityUnavailable);
    }
    let port = config
        .get("mixed-port")
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p != 0)
        .ok_or(StableErrorCode::CapabilityUnavailable)?;
    let destination = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut probe = TcpStream::connect_timeout(&destination, remaining(deadline)?)
        .map_err(|_| StableErrorCode::CapabilityUnavailable)?;
    let source = probe
        .local_addr()
        .map_err(|_| StableErrorCode::CapabilityUnavailable)?
        .port();
    loop {
        if owns_accepted(context.pid, source, port, deadline)? {
            break;
        }
        std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(20)));
    }
    probe
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|_| StableErrorCode::CoreRejected)?;
    let authority = if context.query.contains(':') {
        format!("[{}]:443", context.query)
    } else {
        format!("{}:443", context.query)
    };
    probe
        .write_all(format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
        .map_err(|_| StableErrorCode::CoreRejected)?;
    loop {
        let payload = read(ReadOnlyEndpoint::Connections)?;
        if let Some(mut result) = omavless_mihomo::route_observation::exact_match(
            &payload,
            &context.query,
            source,
            port,
            &context.private,
        )
        .map_err(|_| StableErrorCode::CoreRejected)?
        {
            remaining(deadline)?;
            result["query"] = Value::String(context.query.clone());
            result["version"] = Value::from(1);
            return Ok(result);
        }
        std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(50)));
    }
    // TcpStream drop closes the held probe on every success/error/unwind.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            for _ in 0..1024 {
                let path = std::env::temp_dir().join(format!(
                    "ov-route-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(_) => panic!("cannot create isolated route test directory"),
                }
            }
            panic!("isolated route directory retry bound exhausted");
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn headers(stream: &mut impl Read) -> Vec<u8> {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(bytes.len() < 1024, "synthetic header bound");
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
        }
        bytes
    }

    #[test]
    fn proves_accepted_owned_tuple_not_an_unrelated_listener() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let source = client.local_addr().unwrap().port();
        let deadline = Instant::now() + DEADLINE;
        assert!(owns_accepted(std::process::id(), source, port, deadline).unwrap());
        assert!(
            !owns_accepted(std::process::id(), source, port.wrapping_add(1), deadline).unwrap()
        );
        assert!(owns_accepted(u32::MAX, source, port, deadline).is_err());
        assert!(owns_accepted(std::process::id(), source, port, Instant::now()).is_err());
        drop(server);
        drop(client);
    }

    #[test]
    fn noncanonical_query_is_rejected_before_any_socket_access() {
        for query in [
            "https://private.invalid/token",
            "example.com\r\nInjected: yes",
            "127.0.0.1:80",
            "fe80::1%eth0",
        ] {
            let context = Context {
                query: query.into(),
                pid: std::process::id(),
                private: vec![],
                desired: crate::desired::DesiredState::default(),
                store_digest: [0; 32],
                config_digest: [0; 32],
            };
            assert_eq!(
                collect(
                    Path::new("/no-socket-access"),
                    0,
                    &context,
                    Instant::now() + DEADLINE
                ),
                Err(StableErrorCode::InvalidArgument)
            );
        }
    }

    #[test]
    fn fixed_probe_uses_exact_tuple_and_closes_after_result() {
        let directory = Directory::new();
        let socket = directory.0.join("mihomo.sock");
        let controller = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let mixed = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = mixed.local_addr().unwrap().port();
        let (sender, receiver) = std::sync::mpsc::channel();
        let peer = std::thread::spawn(move || {
            let (mut stream, address) = mixed.accept().unwrap();
            stream.set_read_timeout(Some(DEADLINE)).unwrap();
            assert_eq!(
                headers(&mut stream),
                b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n"
            );
            sender.send(address.port()).unwrap();
            assert_eq!(
                stream.read(&mut [0; 1]).unwrap(),
                0,
                "probe must be closed before completion"
            );
        });
        let http = std::thread::spawn(move || {
            for endpoint in ["/configs", "/connections"] {
                let (mut stream, _) = controller.accept().unwrap();
                stream.set_read_timeout(Some(DEADLINE)).unwrap();
                let request = headers(&mut stream);
                assert!(
                    std::str::from_utf8(&request)
                        .unwrap()
                        .starts_with(&format!("GET {endpoint} HTTP/1.0\r\n"))
                );
                let payload = if endpoint == "/configs" {
                    serde_json::json!({"mixed-port":port,"mode":"rule"})
                } else {
                    let source = receiver.recv_timeout(DEADLINE).unwrap();
                    serde_json::json!({"connections":[{"metadata":{"sourceIP":"127.0.0.1","sourcePort":source.to_string(),"inboundIP":"127.0.0.1","inboundPort":port.to_string(),"destinationPort":"443","network":"tcp","type":"HTTPS","host":"example.com"},"rule":"Match","rulePayload":"","chains":["DIRECT"]}]})
                };
                let body = payload.to_string();
                write!(
                    stream,
                    "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let context = Context {
            query: "example.com".into(),
            pid: std::process::id(),
            private: vec![],
            desired: crate::desired::DesiredState::default(),
            store_digest: [0; 32],
            config_digest: [0; 32],
        };
        let result = collect(
            &directory.0,
            nix::unistd::Uid::current().as_raw(),
            &context,
            Instant::now() + DEADLINE,
        )
        .unwrap();
        assert_eq!(result["target"], "DIRECT");
        assert_eq!(result["version"], 1);
        peer.join().unwrap();
        http.join().unwrap();
    }

    #[test]
    fn wrong_controller_pid_receives_no_request() {
        let directory = Directory::new();
        let socket = directory.0.join("mihomo.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(DEADLINE)).unwrap();
            assert_eq!(stream.read(&mut [0; 1]).unwrap(), 0);
        });
        let context = Context {
            query: "never-sent.example".into(),
            pid: u32::MAX,
            private: vec![],
            desired: crate::desired::DesiredState::default(),
            store_digest: [0; 32],
            config_digest: [0; 32],
        };
        assert!(
            collect(
                &directory.0,
                nix::unistd::Uid::current().as_raw(),
                &context,
                Instant::now() + DEADLINE
            )
            .is_err()
        );
        server.join().unwrap();
    }

    #[test]
    fn foreign_mixed_listener_receives_zero_query_bytes() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let directory = Directory::new();
        let socket = directory.0.join("mihomo.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let mut child=Command::new("python3").arg("-c").arg("import socket\ns=socket.socket(); s.settimeout(3); s.bind(('127.0.0.1',0)); s.listen(1)\nprint(s.getsockname()[1],flush=True)\nc,_=s.accept(); c.settimeout(3)\nprint(len(c.recv(4096)),flush=True)\nc.close(); s.close()\n")
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let port: u16 = line.trim().parse().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(DEADLINE)).unwrap();
            let mut request = [0; 512];
            assert!(stream.read(&mut request).unwrap() > 0);
            let body = serde_json::json!({"mixed-port":port,"mode":"rule"}).to_string();
            write!(
                stream,
                "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let context = Context {
            query: "must-not-leak.example".into(),
            pid: std::process::id(),
            private: vec![],
            desired: crate::desired::DesiredState::default(),
            store_digest: [0; 32],
            config_digest: [0; 32],
        };
        let result = collect(
            &directory.0,
            nix::unistd::Uid::current().as_raw(),
            &context,
            Instant::now() + Duration::from_millis(300),
        );
        line.clear();
        output.read_line(&mut line).unwrap();
        let status = child.wait().unwrap();
        server.join().unwrap();
        assert!(result.is_err());
        assert!(status.success());
        assert_eq!(line.trim(), "0");
    }

    #[test]
    fn installed_mihomo_and_actual_python_observe_exact_reject_without_tun() {
        let Some(core) = std::env::var_os("OMAVLESS_TEST_MIHOMO") else {
            return;
        };
        let directory = Directory::new();
        let reserve = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = reserve.local_addr().unwrap().port();
        drop(reserve);
        let config = directory.0.join("config.yaml");
        let socket = directory.0.join("mihomo.sock");
        fs::write(&config,format!("mixed-port: {port}\nport: 0\nsocks-port: 0\nallow-lan: false\nbind-address: 127.0.0.1\nmode: rule\nlog-level: silent\nexternal-controller-unix: {}\ntun:\n  enable: false\ndns:\n  enable: false\nproxies: []\nproxy-groups: []\nrules:\n  - IP-CIDR,127.0.0.2/32,REJECT,no-resolve\n  - MATCH,REJECT\n",socket.display())).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        // Test-only, byte-identical no-TUN copy: installed file capabilities
        // deliberately make proc-fd inaccessible. Never a production workaround.
        let executable = directory.0.join("core");
        fs::copy(&core, &executable).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(fs::read(&core).unwrap() == fs::read(&executable).unwrap());
        let mut owned =
            crate::core::OwnedCore::spawn(&executable, &directory.0, &config, &socket).unwrap();
        owned.wait_ready(Duration::from_secs(5)).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        let pid = owned.pid().unwrap();
        let readiness = Instant::now() + Duration::from_secs(5);
        loop {
            let rules = crate::diagnostic_read::read_owned(
                &socket,
                uid,
                ReadOnlyEndpoint::Rules,
                readiness,
                Some(pid),
            )
            .unwrap();
            if rules
                .get("rules")
                .and_then(Value::as_array)
                .is_some_and(|rows| rows.len() == 2)
            {
                break;
            }
            assert!(
                Instant::now() < readiness,
                "synthetic rule readiness failed"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let context = Context {
            query: "127.0.0.2".into(),
            pid,
            private: vec![],
            desired: crate::desired::DesiredState::default(),
            store_digest: [0; 32],
            config_digest: [0; 32],
        };
        let result = collect(&directory.0, uid, &context, Instant::now() + DEADLINE).unwrap();
        assert_eq!(result["target"], "REJECT");
        assert_eq!(result["source"], "live");
        let python = std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/route_live_reference.py"
            ))
            .arg(&directory.0)
            .output()
            .unwrap();
        assert!(
            python.status.success(),
            "actual Python loopback live reference failed"
        );
        assert!(String::from_utf8(python.stdout).unwrap().contains("PASS"));
        owned.stop(Duration::from_secs(2)).unwrap();
    }
}
