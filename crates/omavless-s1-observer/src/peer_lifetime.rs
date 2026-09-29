// SPDX-License-Identifier: MIT
//! Synthetic-only socket-peer lifetime contract. Not AUTH-writer provenance.

use crate::Error;
use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::sys::socket::{UnixAddr, getpeername, getsockopt, sockopt::PeerPidfd};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};

// No PID access, Debug, serialization, signals, exports or conversion to permit.
struct UnverifiedPeerLifetime(OwnedFd);
impl UnverifiedPeerLifetime {
    fn capture(socket: &impl AsFd) -> Result<Self, Error> {
        // A listening socket can itself expose sk_peer_pid; require a connected
        // Unix endpoint before interpreting this as the connection peer.
        getpeername::<UnixAddr>(socket.as_fd().as_raw_fd())
            .map_err(|_| Error::IdentityUnverified)?;
        // Kernel returns a handle to sk_peer_pid. Do not reopen a numerical PID.
        let fd = getsockopt(socket, PeerPidfd).map_err(|_| Error::IdentityUnverified)?;
        let flags = fcntl(&fd, FcntlArg::F_GETFD).map_err(|_| Error::IdentityUnverified)?;
        if flags & FdFlag::FD_CLOEXEC.bits() == 0 {
            return Err(Error::IdentityUnverified);
        }
        let pin = Self(fd);
        pin.require_alive_now()?;
        Ok(pin)
    }
    fn require_alive_now(&self) -> Result<(), Error> {
        let mut descriptors = [PollFd::new(self.0.as_fd(), PollFlags::POLLIN)];
        let count =
            poll(&mut descriptors, PollTimeout::ZERO).map_err(|_| Error::IdentityUnverified)?;
        match (count, descriptors[0].revents()) {
            (0, Some(events)) if events.is_empty() => Ok(()),
            _ => Err(Error::IdentityUnverified),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    struct Fixture {
        path: std::path::PathBuf,
        child: Option<Child>,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "omavless-s1-pidfd-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self { path, child: None }
        }
        fn spawn(&mut self, inherited: Option<OwnedFd>) -> UnixStream {
            let (mut control, child_control) = UnixStream::pair().unwrap();
            control
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            control
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "peer_lifetime::tests::child_fixture",
                    "--nocapture",
                ])
                .env("OMAVLESS_SYNTHETIC_PEER_CHILD", self.path.join("socket"))
                .stdin(Stdio::from(OwnedFd::from(child_control)))
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if let Some(fd) = inherited {
                command
                    .env("OMAVLESS_SYNTHETIC_INHERITED_LISTENER", "1")
                    .stderr(Stdio::from(fd));
            }
            self.child = Some(command.spawn().unwrap());
            let mut ready = [0];
            control.read_exact(&mut ready).unwrap();
            assert_eq!(ready, [b'R']);
            control
        }
        fn finish(&mut self, mut control: UnixStream) {
            control.write_all(b"Q").unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                    assert!(status.success());
                    self.child.take();
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "synthetic child exceeded deadline"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn child_fixture() {
        let Some(path) = std::env::var_os("OMAVLESS_SYNTHETIC_PEER_CHILD") else {
            return;
        };
        let mut control = File::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
        let inherited = std::env::var_os("OMAVLESS_SYNTHETIC_INHERITED_LISTENER").is_some();
        let listener = if inherited {
            UnixListener::from(std::io::stderr().as_fd().try_clone_to_owned().unwrap())
        } else {
            UnixListener::bind(path).unwrap()
        };
        control.write_all(b"R").unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(b"W").unwrap();
        let mut quit = [0];
        control.read_exact(&mut quit).unwrap();
        assert_eq!(quit, [b'Q']);
    }

    #[test]
    fn peer_handle_tracks_process_exit_not_socket_close_or_numeric_reopen() {
        let mut fixture = Fixture::new();
        let control = fixture.spawn(None);
        let mut connection = UnixStream::connect(fixture.path.join("socket")).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reply = [0];
        connection.read_exact(&mut reply).unwrap();
        assert_eq!(reply, [b'W']);
        let pin = UnverifiedPeerLifetime::capture(&connection).unwrap();
        assert!(pin.require_alive_now().is_ok());
        drop(connection);
        assert!(pin.require_alive_now().is_ok());
        fixture.finish(control);
        assert_eq!(pin.require_alive_now(), Err(Error::IdentityUnverified));
        // The retained descriptor still names the dead peer; no numerical PID
        // lookup or replacement process can silently refresh it.
        assert_eq!(pin.require_alive_now(), Err(Error::IdentityUnverified));
    }

    #[test]
    fn inherited_listener_pin_is_not_the_process_writing_the_response() {
        let mut fixture = Fixture::new();
        let listener = UnixListener::bind(fixture.path.join("socket")).unwrap();
        let control = fixture.spawn(Some(listener.as_fd().try_clone_to_owned().unwrap()));
        let mut connection = UnixStream::connect(fixture.path.join("socket")).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reply = [0];
        connection.read_exact(&mut reply).unwrap();
        assert_eq!(reply, [b'W']);
        let pin = UnverifiedPeerLifetime::capture(&connection).unwrap();
        fixture.finish(control);
        // Child writer is gone, but listener creator (this test process) lives.
        // This counterexample prevents treating SO_PEERPIDFD as AUTH provenance.
        assert!(pin.require_alive_now().is_ok());
    }

    #[test]
    fn non_socket_and_unconnected_socket_refuse_no_fallback() {
        let file = File::open("/dev/null").unwrap();
        assert!(UnverifiedPeerLifetime::capture(&file).is_err());
        let fixture = Fixture::new();
        let listener = UnixListener::bind(fixture.path.join("socket")).unwrap();
        assert!(UnverifiedPeerLifetime::capture(&listener).is_err());
    }
}
