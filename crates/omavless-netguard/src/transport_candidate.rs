//! Inactive one-exchange Unix transport. This module does not bind a socket
//! or supply a production kernel port. The shared-lock candidate composes its
//! peer and framing checks with synthetic transactions only.

use crate::enrollment::EnrollmentBinding;
use crate::protocol::{MAX_FRAME_BYTES, Request, Response, decode_request, encode_response};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

const EXCHANGE_BUDGET: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransportError {
    Unauthorized,
    EnrollmentChanged,
    InvalidFrame,
    Unavailable,
}

fn read_exact_before(
    stream: &UnixStream,
    bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), TransportError> {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or(TransportError::Unavailable)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| TransportError::Unavailable)?;
        match (&*stream).read(&mut bytes[cursor..]) {
            Ok(0) => return Err(TransportError::InvalidFrame),
            Ok(n) => cursor += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TransportError::Unavailable),
        }
    }
    Ok(())
}

fn write_all_before(
    stream: &UnixStream,
    bytes: &[u8],
    deadline: Instant,
) -> Result<(), TransportError> {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or(TransportError::Unavailable)?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(|_| TransportError::Unavailable)?;
        match (&*stream).write(&bytes[cursor..]) {
            Ok(0) => return Err(TransportError::Unavailable),
            Ok(n) => cursor += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TransportError::Unavailable),
        }
    }
    Ok(())
}

/// UID comes from SO_PEERCRED, never the request. Verify it before reading
/// even one byte; a wrong peer must not hold the service on a slow frame.
#[allow(dead_code)]
pub(crate) fn receive_request(
    stream: &UnixStream,
    enrollment: &EnrollmentBinding,
) -> Result<Request, TransportError> {
    receive_with_budget(stream, enrollment, EXCHANGE_BUDGET)
}

fn receive_with_budget(
    stream: &UnixStream,
    enrollment: &EnrollmentBinding,
    budget: Duration,
) -> Result<Request, TransportError> {
    receive_with_hook(stream, enrollment, budget, || {})
}

fn receive_with_hook(
    stream: &UnixStream,
    enrollment: &EnrollmentBinding,
    budget: Duration,
    after_read: impl FnOnce(),
) -> Result<Request, TransportError> {
    enrollment
        .validate()
        .map_err(|_| TransportError::EnrollmentChanged)?;
    let peer = getsockopt(stream, PeerCredentials).map_err(|_| TransportError::Unavailable)?;
    if peer.uid() != enrollment.uid() {
        return Err(TransportError::Unauthorized);
    }
    let deadline = Instant::now()
        .checked_add(budget)
        .ok_or(TransportError::Unavailable)?;
    let mut prefix = [0_u8; 4];
    read_exact_before(stream, &mut prefix, deadline)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(TransportError::InvalidFrame);
    }
    let mut frame = vec![0_u8; length];
    read_exact_before(stream, &mut frame, deadline)?;
    let request = decode_request(&frame).map_err(|_| TransportError::InvalidFrame)?;
    after_read();
    if Instant::now() >= deadline {
        return Err(TransportError::Unavailable);
    }
    enrollment
        .validate()
        .map_err(|_| TransportError::EnrollmentChanged)?;
    Ok(request)
}

/// One bounded response, with no parser diagnostics or request bytes echoed.
#[allow(dead_code)]
pub(crate) fn send_response(
    stream: &UnixStream,
    enrollment: &EnrollmentBinding,
    response: Response,
) -> Result<(), TransportError> {
    enrollment
        .validate()
        .map_err(|_| TransportError::EnrollmentChanged)?;
    let peer = getsockopt(stream, PeerCredentials).map_err(|_| TransportError::Unavailable)?;
    if peer.uid() != enrollment.uid() {
        return Err(TransportError::Unauthorized);
    }
    let frame = encode_response(response).map_err(|_| TransportError::InvalidFrame)?;
    let length = u32::try_from(frame.len()).map_err(|_| TransportError::InvalidFrame)?;
    let deadline = Instant::now()
        .checked_add(EXCHANGE_BUDGET)
        .ok_or(TransportError::Unavailable)?;
    write_all_before(stream, &length.to_be_bytes(), deadline)?;
    write_all_before(stream, &frame, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ErrorCode, Mode, decode_response, encode_request};
    use std::fs::{self, File};
    use std::net::Shutdown;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct EnrollmentFixture {
        root: PathBuf,
        binding: EnrollmentBinding,
    }

    impl EnrollmentFixture {
        fn new(uid: u32) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "omavless-netguard-transport-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            let directory = root.join("omavless-netguard");
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .unwrap();
            let enrollment = directory.join("enrollment-v1.json");
            fs::write(
                &enrollment,
                format!("{{\"version\":1,\"enrolled_uid\":{uid}}}"),
            )
            .unwrap();
            fs::set_permissions(&enrollment, fs::Permissions::from_mode(0o600)).unwrap();
            let parent = File::open(&root).unwrap();
            let metadata = parent.metadata().unwrap();
            let binding =
                EnrollmentBinding::open_test_parent(parent, (metadata.uid(), metadata.gid()))
                    .unwrap();
            Self { root, binding }
        }

        fn enrollment_path(&self) -> PathBuf {
            self.root.join("omavless-netguard/enrollment-v1.json")
        }
    }

    impl Drop for EnrollmentFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn current_uid(stream: &UnixStream) -> u32 {
        getsockopt(stream, PeerCredentials).unwrap().uid()
    }

    #[test]
    fn accepts_only_kernel_authenticated_peer_and_exact_request() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let enrollment = EnrollmentFixture::new(current_uid(&server));
        let frame = encode_request(Request::Arm {
            generation: 7,
            mode: Mode::Full,
        })
        .unwrap();
        client
            .write_all(&(frame.len() as u32).to_be_bytes())
            .unwrap();
        client.write_all(&frame).unwrap();
        assert_eq!(
            receive_request(&server, &enrollment.binding),
            Ok(Request::Arm {
                generation: 7,
                mode: Mode::Full
            })
        );
        let other = EnrollmentFixture::new(current_uid(&server).wrapping_add(1));
        assert_eq!(
            receive_request(&server, &other.binding),
            Err(TransportError::Unauthorized)
        );
    }

    #[test]
    fn refuses_invalid_sizes_truncation_and_untrusted_fields() {
        let forged = br#"{"version":1,"payload":{"operation":"status","uid":7}}"#;
        for frame in [
            (0_u32.to_be_bytes().to_vec(), Vec::new()),
            (
                (MAX_FRAME_BYTES as u32 + 1).to_be_bytes().to_vec(),
                Vec::new(),
            ),
            (10_u32.to_be_bytes().to_vec(), b"short".to_vec()),
            (
                (forged.len() as u32).to_be_bytes().to_vec(),
                forged.to_vec(),
            ),
        ] {
            let (mut client, server) = UnixStream::pair().unwrap();
            let enrollment = EnrollmentFixture::new(current_uid(&server));
            client.write_all(&frame.0).unwrap();
            client.write_all(&frame.1).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
            assert_eq!(
                receive_request(&server, &enrollment.binding),
                Err(TransportError::InvalidFrame)
            );
        }
    }

    #[test]
    fn total_deadline_rejects_silent_peer_without_unbounded_read() {
        let (_client, server) = UnixStream::pair().unwrap();
        let enrollment = EnrollmentFixture::new(current_uid(&server));
        let start = Instant::now();
        assert_eq!(
            receive_with_budget(&server, &enrollment.binding, Duration::from_millis(80)),
            Err(TransportError::Unavailable)
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn replaced_enrollment_cannot_authorize_a_new_request() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let enrollment = EnrollmentFixture::new(current_uid(&server));
        let frame = encode_request(Request::Status {}).unwrap();
        client
            .write_all(&(frame.len() as u32).to_be_bytes())
            .unwrap();
        client.write_all(&frame).unwrap();
        let file = enrollment.enrollment_path();
        let replaced = file.with_extension("old");
        fs::rename(&file, replaced).unwrap();
        fs::write(
            &file,
            format!(
                "{{\"version\":1,\"enrolled_uid\":{}}}",
                current_uid(&server)
            ),
        )
        .unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            receive_request(&server, &enrollment.binding),
            Err(TransportError::EnrollmentChanged)
        );
    }

    #[test]
    fn enrollment_rebinding_after_read_refuses_before_dispatch() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let enrollment = EnrollmentFixture::new(current_uid(&server));
        let frame = encode_request(Request::Status {}).unwrap();
        client
            .write_all(&(frame.len() as u32).to_be_bytes())
            .unwrap();
        client.write_all(&frame).unwrap();
        assert_eq!(
            receive_with_hook(&server, &enrollment.binding, EXCHANGE_BUDGET, || {
                let file = enrollment.enrollment_path();
                fs::rename(&file, file.with_extension("old")).unwrap();
                fs::write(
                    &file,
                    format!(
                        "{{\"version\":1,\"enrolled_uid\":{}}}",
                        current_uid(&server)
                    ),
                )
                .unwrap();
                fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
            }),
            Err(TransportError::EnrollmentChanged)
        );
    }

    #[test]
    fn response_is_one_bounded_fixed_code_frame() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let enrollment = EnrollmentFixture::new(current_uid(&server));
        send_response(
            &server,
            &enrollment.binding,
            Response::Error {
                code: ErrorCode::Unauthorized,
            },
        )
        .unwrap();
        let mut prefix = [0_u8; 4];
        client.read_exact(&mut prefix).unwrap();
        let length = u32::from_be_bytes(prefix) as usize;
        assert!(length <= MAX_FRAME_BYTES);
        let mut frame = vec![0_u8; length];
        client.read_exact(&mut frame).unwrap();
        assert_eq!(
            decode_response(&frame),
            Ok(Response::Error {
                code: ErrorCode::Unauthorized
            })
        );
        let other = EnrollmentFixture::new(current_uid(&server).wrapping_add(1));
        assert_eq!(
            send_response(
                &server,
                &other.binding,
                Response::Error {
                    code: ErrorCode::Unauthorized
                }
            ),
            Err(TransportError::Unauthorized)
        );
    }
}
