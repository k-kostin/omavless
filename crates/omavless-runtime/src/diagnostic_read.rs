// SPDX-License-Identifier: MIT
//! Fixed read-only controller work. No owner/migration lease crosses I/O.
use nix::sys::socket::{
    AddressFamily, SockFlag, SockType, UnixAddr, connect, getsockopt, socket,
    sockopt::PeerCredentials,
};
use omavless_control_protocol::StableErrorCode;
use omavless_mihomo::{MAX_CONTROLLER_RESPONSE_BYTES, ReadOnlyEndpoint, parse_controller_response};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) const METHODS: &[&str] = &[
    "diagnostics.summary",
    "diagnostics.rules",
    "diagnostics.providers",
];
pub(crate) const DEADLINE: Duration = Duration::from_secs(3);

pub(crate) fn collect(
    directory: &Path,
    uid: u32,
    method: &str,
    private: &[String],
) -> Result<Value, StableErrorCode> {
    let deadline = Instant::now() + DEADLINE;
    let path = directory.join("mihomo.sock");
    let mut result = json!({"version":1});
    for (field, endpoint) in [
        ("rules", ReadOnlyEndpoint::Rules),
        ("providers", ReadOnlyEndpoint::RuleProviders),
    ] {
        if method != "diagnostics.summary" && method != format!("diagnostics.{field}") {
            continue;
        }
        let payload = read(&path, uid, endpoint, deadline)?;
        result[field] = if field == "rules" {
            omavless_mihomo::diagnostics::rules_projection_before(&payload, private, Some(deadline))
        } else {
            omavless_mihomo::diagnostics::providers_projection_before(
                &payload,
                private,
                Some(deadline),
            )
        }
        .map_err(|_| StableErrorCode::CoreRejected)?;
    }
    remaining(deadline)?;
    Ok(result)
}

fn remaining(deadline: Instant) -> Result<Duration, StableErrorCode> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or(StableErrorCode::CoreRejected)
}

fn read(
    path: &Path,
    uid: u32,
    endpoint: ReadOnlyEndpoint,
    deadline: Instant,
) -> Result<Value, StableErrorCode> {
    read_owned(path, uid, endpoint, deadline, None)
}

pub(crate) fn read_owned(
    path: &Path,
    uid: u32,
    endpoint: ReadOnlyEndpoint,
    deadline: Instant,
    expected_pid: Option<u32>,
) -> Result<Value, StableErrorCode> {
    let unavailable = StableErrorCode::CapabilityUnavailable;
    let parent = path.parent().ok_or(unavailable)?;
    let directory = fs::symlink_metadata(parent).map_err(|_| unavailable)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| unavailable)?;
    if !directory.is_dir()
        || directory.file_type().is_symlink()
        || directory.uid() != uid
        || directory.mode() & 0o7777 != 0o700
        || !metadata.file_type().is_socket()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(unavailable);
    }
    // A full Unix accept backlog must never turn connect into an unbounded wait.
    let fd = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
        None,
    )
    .map_err(|_| unavailable)?;
    connect(
        fd.as_raw_fd(),
        &UnixAddr::new(path).map_err(|_| unavailable)?,
    )
    .map_err(|_| unavailable)?;
    let mut stream = UnixStream::from(fd);
    let peer = getsockopt(&stream, PeerCredentials).map_err(|_| unavailable)?;
    if peer.uid() != uid
        || expected_pid.is_some_and(|pid| u32::try_from(peer.pid()).ok() != Some(pid))
    {
        return Err(unavailable);
    }
    stream.set_nonblocking(false).map_err(|_| unavailable)?;
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|_| unavailable)?;
    // HTTP/1.0 + close gives a bounded EOF-delimited body even when Mihomo's
    // Go handler would stream a large HTTP/1.1 body with chunked encoding.
    // The existing strict parser intentionally rejects ambiguous transfer coding.
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        endpoint.path()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| unavailable)?;
    let mut response = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| unavailable)?;
        let read = stream
            .read(&mut chunk)
            .map_err(|_| StableErrorCode::CoreRejected)?;
        if read == 0 {
            break;
        }
        if response.len() + read > MAX_CONTROLLER_RESPONSE_BYTES {
            return Err(StableErrorCode::CoreRejected);
        }
        response.extend_from_slice(&chunk[..read]);
    }
    let response =
        parse_controller_response(&response).map_err(|_| StableErrorCode::CoreRejected)?;
    if response.status != 200 {
        return Err(StableErrorCode::CoreRejected);
    }
    Ok(response.payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn installed_mihomo_serves_bounded_diagnostics_without_tcp_or_tun() {
        let Some(core) = std::env::var_os("OMAVLESS_TEST_MIHOMO").map(std::path::PathBuf::from)
        else {
            return;
        };
        for inline_provider in [false, true] {
            installed_case(&core, inline_provider);
        }
    }

    fn installed_case(core: &Path, inline_provider: bool) {
        let directory = std::env::temp_dir().join(format!(
            "ov-diag-core-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = directory.join("mihomo.sock");
        let config = directory.join("config.yaml");
        let rules = (0..80)
            .map(|i| format!("- DOMAIN,sample-{i}.example.invalid,DIRECT\n"))
            .collect::<String>();
        let provider = if inline_provider {
            "rule-providers:\n  synthetic:\n    type: inline\n    behavior: domain\n    payload:\n      - fixture.example.invalid\n"
        } else {
            ""
        };
        let provider_rule = if inline_provider {
            "- RULE-SET,synthetic,DIRECT\n"
        } else {
            ""
        };
        fs::write(&config,format!("mixed-port: 0\nport: 0\nsocks-port: 0\nallow-lan: false\nmode: rule\nlog-level: silent\nexternal-controller-unix: {}\ntun:\n  enable: false\ndns:\n  enable: false\nproxies: []\nproxy-groups: []\n{}rules:\n{}{}- MATCH,DIRECT\n",socket.display(),provider,rules,provider_rule)).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
        omavless_mihomo::validate_config(core, &directory, &config, Duration::from_secs(15))
            .unwrap();
        // Installed file capabilities make Linux hide /proc/<pid>/fd even
        // from the parent. A byte-identical non-capability-bearing copy is
        // sufficient for this explicitly no-TUN test and preserves OS policy.
        let unprivileged_core = directory.join("mihomo");
        fs::copy(core, &unprivileged_core).unwrap();
        fs::set_permissions(&unprivileged_core, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(fs::read(core).unwrap() == fs::read(&unprivileged_core).unwrap());
        let mut owned =
            crate::core::OwnedCore::spawn(&unprivileged_core, &directory, &config, &socket)
                .unwrap();
        owned.wait_ready(Duration::from_secs(10)).unwrap();
        // Match the packaged service's UMask=0077 without mutating the
        // multithreaded test process's global umask.
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        // /version readiness precedes config initialization on Mihomo 1.19.30:
        // /providers/rules can transiently contain null, even with an inline
        // provider. Wait for this fixture's actual loaded data, not a sleep or
        // a production schema relaxation which would invent an empty result.
        let loaded_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let providers = read(
                &socket,
                uid,
                ReadOnlyEndpoint::RuleProviders,
                loaded_deadline,
            )
            .unwrap();
            let rules = read(&socket, uid, ReadOnlyEndpoint::Rules, loaded_deadline).unwrap();
            if providers
                .get("providers")
                .and_then(Value::as_object)
                .is_some_and(|rows| rows.len() == usize::from(inline_provider))
                && rules
                    .get("rules")
                    .and_then(Value::as_array)
                    .is_some_and(|rows| rows.len() == 81 + usize::from(inline_provider))
            {
                break;
            }
            assert!(
                Instant::now() < loaded_deadline,
                "synthetic core configuration did not become observable"
            );
            thread::sleep(Duration::from_millis(20));
        }
        for method in METHODS {
            let result = collect(&directory, uid, method, &[]).unwrap();
            assert_eq!(result["version"], 1);
            assert!(result.to_string().len() < 225 * 1024);
            if *method != "diagnostics.providers" {
                assert_eq!(result["rules"]["total"], 81 + usize::from(inline_provider));
            }
            if *method != "diagnostics.rules" {
                assert_eq!(result["providers"]["total"], usize::from(inline_provider));
            }
        }
        // Check only this owned child's socket inodes against listening TCP
        // sockets; no process args or another user's network data is printed.
        let descriptors = fs::read_dir(format!("/proc/{}/fd", owned.pid().unwrap())).unwrap();
        let targets: Vec<String> = descriptors
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read_link(entry.path()).ok())
            .filter_map(|path| path.to_str().map(str::to_owned))
            .collect();
        assert!(
            !targets.iter().any(|target| target == "/dev/net/tun"),
            "isolated core unexpectedly owns a TUN descriptor"
        );
        let sockets: Vec<String> = targets
            .into_iter()
            .filter_map(|link| {
                link.strip_prefix("socket:[")
                    .and_then(|s| s.strip_suffix(']'))
                    .map(str::to_owned)
            })
            .collect();
        for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
            for line in fs::read_to_string(table).unwrap().lines().skip(1) {
                let fields: Vec<_> = line.split_whitespace().collect();
                assert!(
                    !(fields.get(3) == Some(&"0A")
                        && fields
                            .get(9)
                            .is_some_and(|inode| sockets.iter().any(|own| own == inode))),
                    "isolated core unexpectedly owns a TCP listener"
                );
            }
        }
        owned.stop(Duration::from_secs(5)).unwrap();
        assert!(owned.pid().is_none());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn private_controller_checks_modes_peer_fixed_path_and_global_deadline() {
        let directory = std::env::temp_dir().join(format!(
            "ov-diag-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("mihomo.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        for mode in [0o644, 0o666] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                read(
                    &path,
                    uid,
                    ReadOnlyEndpoint::Rules,
                    Instant::now() + DEADLINE
                )
                .is_err()
            );
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            read(
                &path,
                uid.saturating_add(1),
                ReadOnlyEndpoint::Rules,
                Instant::now() + DEADLINE
            )
            .is_err()
        );
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 1024];
            let size = stream.read(&mut buffer).unwrap();
            assert!(buffer[..size].starts_with(b"GET /rules HTTP/1.0\r\n"));
            for _ in 0..20 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        let started = Instant::now();
        assert!(
            read(
                &path,
                uid,
                ReadOnlyEndpoint::Rules,
                started + Duration::from_millis(60)
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_millis(300));
        worker.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }
}
