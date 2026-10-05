use super::*;
use crate::protocol::{Health, Protection, decode_response, encode_request};
use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;

fn peer_uid() -> u32 {
    let (_client, server) = UnixStream::pair().unwrap();
    getsockopt(&server, PeerCredentials).unwrap().uid()
}

fn bound_state(fixture: &Fixture, uid: u32) -> LockedState {
    let config = fixture.0.join("omavless-netguard/enrollment-v1.json");
    fs::write(&config, format!("{{\"version\":1,\"enrolled_uid\":{uid}}}")).unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    let parent = File::open(&fixture.0).unwrap();
    let metadata = parent.metadata().unwrap();
    let binding = EnrollmentBinding::open_test_parent(parent, (metadata.uid(), metadata.gid()))
        .unwrap();
    let root = RootStateStore::open_test_parent(
        File::open(&fixture.0).unwrap(),
        (metadata.uid(), metadata.gid()),
        uid,
    )
    .unwrap();
    let mut state = LockedState::from_root(root);
    state.enrollment = Some(binding);
    state
}

fn send(client: &mut UnixStream, request: Request) {
    let frame = encode_request(request).unwrap();
    client.write_all(&(frame.len() as u32).to_be_bytes()).unwrap();
    client.write_all(&frame).unwrap();
}

fn receive(client: &mut UnixStream) -> Response {
    let mut prefix = [0_u8; 4];
    client.read_exact(&mut prefix).unwrap();
    let length = u32::from_be_bytes(prefix) as usize;
    assert!(length <= crate::protocol::MAX_FRAME_BYTES);
    let mut frame = vec![0; length];
    client.read_exact(&mut frame).unwrap();
    decode_response(&frame).unwrap()
}

#[test]
fn one_enrolled_exchange_commits_under_one_lock_and_closes_after_one_reply() {
    let f = Fixture::new();
    let mut state = bound_state(&f, peer_uid());
    let mut kernel = Kernel::new(&f);
    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, ARM);
    send(&mut client, DISARM); // A pipelined second frame must not dispatch.
    assert_eq!(state.exchange_once(server, NS, &mut kernel), Ok(()));
    assert_eq!(
        receive(&mut client),
        Response::Status {
            policy_version: crate::protocol::POLICY_VERSION,
            protection: Protection::Armed { generation: 7 },
            health: Health::Verified,
        }
    );
    let mut tail = [0_u8; 1];
    // Linux may reset a stream closed with unread pipelined input.
    match client.read(&mut tail) {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        other => panic!("unexpected second reply: {other:?}"),
    }
    assert_eq!(kernel.effects, 1);
    assert_eq!(record(&state).state(), ReceiptState::Live { handle: 5 });

    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, DISARM);
    assert_eq!(state.exchange_once(server, NS, &mut kernel), Ok(()));
    assert!(matches!(
        receive(&mut client),
        Response::Status {
            protection: Protection::Disarmed { closed_generation: Some(7) },
            health: Health::Verified,
            ..
        }
    ));
    assert_eq!(kernel.effects, 2);
    assert_eq!(record(&state).state(), ReceiptState::Retired);
    let before = f.bytes();
    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, DISARM);
    state.exchange_once(server, NS, &mut kernel).unwrap();
    assert!(matches!(receive(&mut client), Response::Status { .. }));
    assert_eq!(kernel.effects, 2);
    assert_eq!(f.bytes(), before);
}

#[test]
fn missing_enrollment_unauthorized_and_bad_frames_never_dispatch() {
    let f = Fixture::new();
    let mut state = f.state();
    let mut kernel = Kernel::new(&f);
    let (_client, server) = UnixStream::pair().unwrap();
    assert_eq!(
        state.exchange_once(server, NS, &mut kernel),
        Err(ExchangeError::NoEnrollment)
    );
    drop(state);
    let mut state = bound_state(&f, peer_uid() + 1);
    let (_client, server) = UnixStream::pair().unwrap();
    assert_eq!(
        state.exchange_once(server, NS, &mut kernel),
        Err(ExchangeError::Receive(TransportError::Unauthorized))
    );
    drop(state);
    let mut state = bound_state(&f, peer_uid());
    for bytes in [
        0_u32.to_be_bytes().to_vec(),
        (crate::protocol::MAX_FRAME_BYTES as u32 + 1)
            .to_be_bytes()
            .to_vec(),
        [4_u32.to_be_bytes().as_slice(), b"junk"].concat(),
    ] {
        let (mut client, server) = UnixStream::pair().unwrap();
        client.write_all(&bytes).unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        assert_eq!(
            state.exchange_once(server, NS, &mut kernel),
            Err(ExchangeError::Receive(TransportError::InvalidFrame))
        );
    }
    let (_client, server) = UnixStream::pair().unwrap();
    assert_eq!(
        state.exchange_once(server, NS, &mut kernel),
        Err(ExchangeError::Receive(TransportError::Unavailable))
    );
    assert_eq!(kernel.effects, 0);
    assert_eq!(kernel.observes, 0);
    assert_eq!(f.bytes(), (None, None));
}

#[test]
fn enrollment_replacement_after_parse_and_during_effect_never_delivers_success() {
    for at_effect in [false, true] {
        let f = Fixture::new();
        let mut state = bound_state(&f, peer_uid());
        let mut kernel = Kernel::new(&f);
        kernel.rebind_enrollment_on_effect = at_effect;
        let (mut client, server) = UnixStream::pair().unwrap();
        send(&mut client, ARM);
        let result = state.exchange_with(server, NS, &mut kernel, |point| {
            if !at_effect && point == ExchangePoint::Received {
                kernel_rebind(&f);
            }
        });
        assert_eq!(
            result,
            Err(ExchangeError::ReplyDeliveryUnknown(
                TransportError::EnrollmentChanged
            ))
        );
        assert_eq!(kernel.effects, usize::from(at_effect));
        assert_eq!(
            f.bytes().1.as_deref().map(receipt::decode).transpose().unwrap().map(|r| r.state()),
            at_effect.then_some(ReceiptState::PendingCreate)
        );
        let mut prefix = [0_u8; 4];
        assert_eq!(client.read(&mut prefix).unwrap(), 0);
    }
}

fn kernel_rebind(fixture: &Fixture) {
    let config = fixture.0.join("omavless-netguard/enrollment-v1.json");
    fs::rename(&config, config.with_extension("old")).unwrap();
    fs::copy(config.with_extension("old"), &config).unwrap();
}

#[test]
fn lost_socket_reply_after_commit_keeps_terminal_state_without_retry() {
    let f = Fixture::new();
    let mut state = bound_state(&f, peer_uid());
    let mut kernel = Kernel::new(&f);
    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, ARM);
    drop(client);
    assert_eq!(
        state.exchange_once(server, NS, &mut kernel),
        Err(ExchangeError::ReplyDeliveryUnknown(
            TransportError::Unavailable
        ))
    );
    assert_eq!(record(&state).state(), ReceiptState::Live { handle: 5 });
    assert_eq!(kernel.effects, 1);
    let before = f.bytes();
    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, Request::Status {});
    state.exchange_once(server, NS, &mut kernel).unwrap();
    assert!(matches!(
        receive(&mut client),
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            ..
        }
    ));
    assert_eq!(f.bytes(), before);
    assert_eq!(kernel.effects, 1);
}

#[test]
fn enrollment_replacement_before_response_preserves_commit_but_refuses_delivery() {
    let f = Fixture::new();
    let mut state = bound_state(&f, peer_uid());
    let mut kernel = Kernel::new(&f);
    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, ARM);
    assert_eq!(
        state.exchange_with(server, NS, &mut kernel, |point| {
            if point == ExchangePoint::BeforeResponse {
                kernel_rebind(&f);
            }
        }),
        Err(ExchangeError::ReplyDeliveryUnknown(
            TransportError::EnrollmentChanged
        ))
    );
    assert_eq!(kernel.effects, 1);
    assert_eq!(record(&state).state(), ReceiptState::Live { handle: 5 });
    let mut prefix = [0_u8; 4];
    assert_eq!(client.read(&mut prefix).unwrap(), 0);
    drop(state);
    let mut reopened = bound_state(&f, peer_uid());
    let (mut client, server) = UnixStream::pair().unwrap();
    send(&mut client, Request::Status {});
    reopened.exchange_once(server, NS, &mut kernel).unwrap();
    assert!(matches!(
        receive(&mut client),
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            ..
        }
    ));
    assert_eq!(kernel.effects, 1);
}
