// SPDX-License-Identifier: MIT
use crate::{core::ReadLifetime, core_selector};
use std::io::{Read, Write};
use std::os::unix::{
    fs::PermissionsExt,
    net::{UnixListener, UnixStream},
};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[test]
fn saturated_socket_write_and_revocation_never_wait_for_controller_read() {
    let lifetime = ReadLifetime::default();
    let (mut writer, _silent_reader) = UnixStream::pair().unwrap();
    let start = Instant::now();
    while lifetime.write(&mut writer, &[0u8; 8192]).is_some() {
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    let start = Instant::now();
    lifetime.revoke();
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(lifetime.write(&mut writer, b"GET /configs").is_none());
    assert!(!lifetime.live());
}

#[test]
fn actual_retained_http_read_is_revocable_without_waiting_for_the_reply() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = dir.path().join("s");
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    let lifetime = Arc::new(ReadLifetime::default());
    let (received, at_read) = mpsc::sync_channel(1);
    let (release, pause) = mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 1024];
        let n = stream.read(&mut request).unwrap();
        assert!(request[..n].starts_with(b"GET /configs HTTP/1.0\r\n"));
        received.send(()).unwrap();
        pause.recv_timeout(Duration::from_secs(2)).unwrap();
        let _ = stream.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}");
    });
    let held = Arc::clone(&lifetime);
    let reader = std::thread::spawn(move || {
        core_selector::read_retained(
            &socket,
            std::process::id(),
            omavless_mihomo::ReadOnlyEndpoint::Configs,
            Instant::now() + Duration::from_millis(500),
            &held,
        )
    });
    at_read.recv_timeout(Duration::from_secs(2)).unwrap();
    let start = Instant::now();
    lifetime.revoke();
    assert!(start.elapsed() < Duration::from_millis(100));
    release.send(()).unwrap();
    assert!(reader.join().unwrap().is_none());
    server.join().unwrap();
}
