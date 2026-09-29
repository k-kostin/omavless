// SPDX-License-Identifier: MIT

use super::*;
use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};

const OK: &[u8] = b"OK 0123456789abcdef0123456789abcdef\r\n";

#[test]
fn fragmented_auth_is_bounded_exact_and_sender_stable() {
    let sender = Sender { pid: 123, uid: 456 };
    let mut frame = AuthFrame::new();
    for (index, byte) in OK.iter().enumerate() {
        assert_eq!(
            frame.append(&[*byte], sender, 456),
            Ok(index == OK.len() - 1)
        );
    }
    for bytes in [
        b"REJECTED EXTERNAL\r\n".as_slice(),
        b"OK short\r\n",
        b"OK 0123456789abcdef0123456789abcdeg\r\n",
        b"OK 0123456789abcdef0123456789abcdef\r\ntrailing",
        &vec![b'x'; MAX_AUTH_FRAME],
        &vec![b'x'; MAX_AUTH_FRAME + 1],
        b"",
    ] {
        assert!(AuthFrame::new().append(bytes, sender, 456).is_err());
    }
    let mut changed = AuthFrame::new();
    assert_eq!(changed.append(&OK[..3], sender, 456), Ok(false));
    assert!(
        changed
            .append(&OK[3..], Sender { pid: 124, ..sender }, 456)
            .is_err()
    );
    assert!(
        AuthFrame::new()
            .append(OK, Sender { pid: 0, ..sender }, 456)
            .is_err()
    );
    assert!(AuthFrame::new().append(OK, sender, 457).is_err());
}

#[test]
fn ancillary_requires_one_credential_and_no_truncation_or_rights() {
    let credential: gio::SocketControlMessage = gio::UnixCredentialsMessage::new().upcast();
    assert!(parse_ancillary(std::slice::from_ref(&credential), 0).is_ok());
    assert!(parse_ancillary(&[], 0).is_err());
    assert!(parse_ancillary(&[credential.clone(), credential.clone()], 0).is_err());
    for flags in [nix::libc::MSG_TRUNC, nix::libc::MSG_CTRUNC] {
        assert!(parse_ancillary(std::slice::from_ref(&credential), flags).is_err());
    }
}

fn read_request(stream: &mut UnixStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut byte = [0];
    while !request.ends_with(b"\r\n") {
        stream.read_exact(&mut byte).unwrap();
        request.push(byte[0]);
        assert!(request.len() < 64);
    }
    assert!(request.starts_with(b"\0AUTH EXTERNAL "));
}

#[test]
fn real_socket_credentials_observe_the_writer_without_begin_or_hello() {
    let (mut server, client) = UnixStream::pair().unwrap();
    let worker = std::thread::spawn(move || {
        read_request(&mut server);
        server.write_all(OK).unwrap();
        let mut extra = [0; 1];
        assert_eq!(server.read(&mut extra).unwrap(), 0);
    });
    let socket = gio::Socket::from_fd(client.into()).unwrap();
    let uid = nix::unistd::geteuid().as_raw();
    let sender = observe_auth(&socket, uid, Instant::now() + Duration::from_secs(2)).unwrap();
    assert!(
        sender
            == Sender {
                pid: std::process::id(),
                uid
            }
    );
    socket.close().unwrap();
    worker.join().unwrap();
}

#[test]
fn silent_and_dripping_auth_peers_cannot_extend_the_total_deadline() {
    for drip in [false, true] {
        let (mut server, client) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            read_request(&mut server);
            if drip {
                for byte in OK {
                    if server.write_all(&[*byte]).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
            } else {
                std::thread::sleep(Duration::from_millis(180));
            }
        });
        let socket = gio::Socket::from_fd(client.into()).unwrap();
        let start = Instant::now();
        assert!(
            observe_auth(
                &socket,
                nix::unistd::geteuid().as_raw(),
                start + Duration::from_millis(75)
            )
            .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        socket.close().unwrap();
        worker.join().unwrap();
    }
}

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "omavless-s1-auth-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fixture_child() {
    let Some(role) = std::env::var_os("OMAVLESS_S1_AUTH_TEST_ROLE") else {
        return;
    };
    let listener = UnixListener::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
    let (mut stream, _) = listener.accept().unwrap();
    read_request(&mut stream);
    if role == "relay" {
        let path = std::env::var_os("OMAVLESS_S1_AUTH_TEST_UPSTREAM").unwrap();
        let mut upstream = UnixStream::connect(path).unwrap();
        let mut reply = [0; 37];
        upstream.read_exact(&mut reply).unwrap();
        stream.write_all(&reply).unwrap();
    } else {
        assert_eq!(role, "activation");
        stream.write_all(OK).unwrap();
    }
}

#[test]
fn inherited_listener_and_relay_report_actual_sender_not_listener_or_upstream() {
    for relay in [false, true] {
        let fixture = Fixture::new();
        let front_path = fixture.0.join("front");
        let listener = UnixListener::bind(&front_path).unwrap();
        let upstream_path = fixture.0.join("upstream");
        let upstream = UnixListener::bind(&upstream_path).unwrap();
        let upstream_worker = relay.then(|| {
            std::thread::spawn(move || {
                let (mut stream, _) = upstream.accept().unwrap();
                stream.write_all(OK).unwrap();
            })
        });
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "auth_sender::tests::fixture_child",
                "--test-threads=1",
            ])
            .env(
                "OMAVLESS_S1_AUTH_TEST_ROLE",
                if relay { "relay" } else { "activation" },
            )
            .env("OMAVLESS_S1_AUTH_TEST_UPSTREAM", &upstream_path)
            .stdin(Stdio::from(listener.as_fd().try_clone_to_owned().unwrap()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let client = UnixStream::connect(&front_path).unwrap();
        let socket = gio::Socket::from_fd(client.into()).unwrap();
        let peer = socket.credentials().unwrap().unix_pid().unwrap();
        let sender = observe_auth(
            &socket,
            nix::unistd::geteuid().as_raw(),
            Instant::now() + Duration::from_secs(3),
        )
        .unwrap();
        assert!(i64::from(peer) == i64::from(std::process::id()));
        assert!(sender.pid == child.id());
        assert!(i64::from(peer) != i64::from(sender.pid));
        socket.close().unwrap();
        assert!(child.wait().unwrap().success());
        if let Some(worker) = upstream_worker {
            worker.join().unwrap();
        }
    }
}

#[test]
fn fd_payload_is_refused_and_received_descriptors_are_closed() {
    let (server, client) = UnixStream::pair().unwrap();
    let (reader, writer) = nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC).unwrap();
    nix::fcntl::fcntl(
        &reader,
        nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
    )
    .unwrap();
    let worker = std::thread::spawn(move || {
        let mut request_stream = server.try_clone().unwrap();
        read_request(&mut request_stream);
        use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};
        sendmsg::<()>(
            server.as_raw_fd(),
            &[std::io::IoSlice::new(OK)],
            &[ControlMessage::ScmRights(&[writer.as_raw_fd()])],
            MsgFlags::empty(),
            None,
        )
        .unwrap();
        drop(writer);
    });
    let socket = gio::Socket::from_fd(client.into()).unwrap();
    assert!(
        observe_auth(
            &socket,
            nix::unistd::geteuid().as_raw(),
            Instant::now() + Duration::from_secs(2)
        )
        .is_err()
    );
    socket.close().unwrap();
    worker.join().unwrap();
    // EOF proves that GIO released the received duplicate, not only that the
    // parser rejected its presence. A leaked writer would return EAGAIN.
    assert_eq!(nix::unistd::read(&reader, &mut [0; 1]).unwrap(), 0);
}
