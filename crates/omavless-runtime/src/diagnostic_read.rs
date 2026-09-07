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
const DEADLINE: Duration = Duration::from_secs(3);

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
            omavless_mihomo::diagnostics::rules_projection(&payload, private)
        } else {
            omavless_mihomo::diagnostics::providers_projection(&payload, private)
        }
        .map_err(|_| StableErrorCode::CoreRejected)?;
    }
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
    if getsockopt(&stream, PeerCredentials)
        .map_err(|_| unavailable)?
        .uid()
        != uid
    {
        return Err(unavailable);
    }
    stream.set_nonblocking(false).map_err(|_| unavailable)?;
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|_| unavailable)?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
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
            assert!(buffer[..size].starts_with(b"GET /rules HTTP/1.1\r\n"));
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
