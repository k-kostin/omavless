// SPDX-License-Identifier: MIT
use super::*;
use std::os::fd::AsRawFd;

fn pair() -> (Client, Session) {
    let (left, right) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    let uid = rustix::process::geteuid().as_raw();
    let client = Client {
        channel: Endpoint::new(left, uid).unwrap(),
        state: State::Idle,
    };
    let session = Session {
        channel: Endpoint::new(right, uid).unwrap(),
        state: State::Idle,
        proof: None,
    };
    (client, session)
}

fn raw_send(endpoint: &Endpoint, bytes: &[u8], descriptors: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(20))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !descriptors.is_empty() {
        assert!(control.push(SendAncillaryMessage::ScmRights(descriptors)));
    }
    net::sendmsg(
        &endpoint.fd,
        &[IoSlice::new(bytes)],
        &mut control,
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )
    .unwrap();
}

fn copies_of(file: &tempfile::NamedTempFile) -> usize {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| fs::read_link(entry.path()).is_ok_and(|p| p == file.path()))
        .count()
}

#[test]
fn golden_frames_match_the_shared_core_channel_corpus() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("../cases.json")).unwrap();
    assert_eq!(cases["frameBytes"], FRAME_BYTES);
    for (kind, key) in [(1, "requests"), (2, "responses")] {
        for case in cases[key].as_array().unwrap() {
            let bytes: Vec<u8> = serde_json::from_value(case["bytes"].clone()).unwrap();
            assert_eq!(frame(kind, bytes[6]).to_vec(), bytes);
            assert_eq!(
                decode(bytes.try_into().unwrap(), kind).unwrap(),
                case["bytes"][6]
            );
        }
    }
}

#[test]
fn acquire_and_verified_release_are_distinct_explicit_states() {
    let (mut client, mut session) = pair();
    let file = tempfile::NamedTempFile::new().unwrap();
    client.acquire(file.as_fd()).unwrap();
    let proof = session.receive_acquire().unwrap();
    assert!(
        rustix::io::fcntl_getfd(proof)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_ne!(proof.as_raw_fd(), file.as_raw_fd());
    assert_eq!(copies_of(&file), 2);
    session.reply(Response::Applying).unwrap();
    assert_eq!(client.receive().unwrap(), Response::Applying);
    assert_eq!(client.release(), Err(Error::InvalidState));
    session.reply(Response::Ready).unwrap();
    assert_eq!(client.receive().unwrap(), Response::Ready);
    client.release().unwrap();
    session.receive_release().unwrap();
    session.reply(Response::Releasing).unwrap();
    assert_eq!(client.receive().unwrap(), Response::Releasing);
    session.reply(Response::Released).unwrap();
    assert_eq!(client.receive().unwrap(), Response::Released);
    assert_eq!(client.acquire(file.as_fd()), Err(Error::InvalidState));
    assert!(session.proof().is_some());
    drop(session);
    assert_eq!(copies_of(&file), 1);
}

#[test]
fn malformed_frames_and_extra_rights_close_every_delivered_descriptor() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let mut bad_frames = vec![
        vec![],
        vec![0; 8],
        frame(1, 1).repeat(2),
        frame(1, 99).to_vec(),
        frame(1, 2).to_vec(),
    ];
    for byte in [0, 4, 5, 7] {
        let mut bytes = frame(1, 1);
        bytes[byte] ^= 0xff;
        bad_frames.push(bytes.to_vec());
    }
    for bytes in bad_frames {
        let (client, mut session) = pair();
        raw_send(&client.channel, &bytes, &[file.as_fd()]);
        assert!(session.receive_acquire().is_err());
        assert!(session.proof().is_none());
        assert_eq!(copies_of(&file), 1);
    }
    for count in [0, 2, 3, 20] {
        let (client, mut session) = pair();
        raw_send(&client.channel, &frame(1, 1), &vec![file.as_fd(); count]);
        assert!(session.receive_acquire().is_err());
        assert_eq!(copies_of(&file), 1);
    }
}

#[test]
fn credentials_are_kernel_checked_not_payload_assertions() {
    let (left, _right) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    assert!(matches!(
        Endpoint::new(left, rustix::process::geteuid().as_raw() + 1),
        Err(Error::PeerRejected)
    ));
    let (client, mut session) = pair();
    let file = tempfile::tempfile().unwrap();
    raw_send(&client.channel, &frame(1, 1), &[file.as_fd()]);
    // Compare the real incoming kernel credential with an intentionally stale
    // admitted peer identity; no fake serialized identity is being accepted.
    session.channel.peer.uid =
        rustix::process::Uid::from_raw(rustix::process::geteuid().as_raw() + 1);
    assert!(matches!(
        session.receive_acquire(),
        Err(Error::PeerRejected)
    ));
}

#[test]
fn foreign_ancillary_timestamp_cannot_hide_behind_supported_rights() {
    let (client, mut session) = pair();
    let file = tempfile::NamedTempFile::new().unwrap();
    nix::sys::socket::setsockopt(
        &session.channel.fd,
        nix::sys::socket::sockopt::ReceiveTimestamp,
        &true,
    )
    .unwrap();
    raw_send(&client.channel, &frame(1, 1), &[file.as_fd()]);
    assert!(session.receive_acquire().is_err());
    assert_eq!(copies_of(&file), 1);
}

#[test]
fn replies_never_transfer_descriptors_and_unknown_codes_fail() {
    for code in [0, 7, 255] {
        let (mut client, session) = pair();
        client.state = State::Acquiring;
        raw_send(&session.channel, &frame(2, code), &[]);
        assert_eq!(client.receive(), Err(Error::InvalidFrame));
    }
    let file = tempfile::NamedTempFile::new().unwrap();
    let (mut client, session) = pair();
    client.state = State::Acquiring;
    raw_send(&session.channel, &frame(2, 1), &[file.as_fd()]);
    assert!(client.receive().is_err());
    assert_eq!(copies_of(&file), 1);
}

#[test]
fn io_loss_retains_proof_and_never_means_released() {
    let (mut client, mut session) = pair();
    let file = tempfile::NamedTempFile::new().unwrap();
    client.acquire(file.as_fd()).unwrap();
    session.receive_acquire().unwrap();
    drop(client);
    assert_eq!(session.reply(Response::Applying), Err(Error::ChannelLost));
    assert_eq!(copies_of(&file), 2);
    assert!(session.proof().is_some());
    drop(session);
    assert_eq!(copies_of(&file), 1);
    let (mut client, session) = pair();
    client.state = State::Applying;
    drop(session);
    assert_eq!(client.receive(), Err(Error::ChannelLost));
}

#[test]
fn read_timeout_is_finite_and_terminal_for_the_client() {
    let (mut client, _session) = pair();
    client.channel.timeout = Duration::from_millis(25);
    client.state = State::Acquiring;
    let began = Instant::now();
    assert_eq!(client.receive(), Err(Error::Timeout));
    assert!(began.elapsed() < Duration::from_secs(1));
    assert_eq!(client.release(), Err(Error::InvalidState));
}

#[test]
fn progress_cannot_be_replayed_to_extend_the_transaction() {
    let (mut client, mut session) = pair();
    let file = tempfile::tempfile().unwrap();
    client.acquire(file.as_fd()).unwrap();
    session.receive_acquire().unwrap();
    assert_eq!(session.reply(Response::Ready), Err(Error::InvalidState));
    session.reply(Response::Applying).unwrap();
    client.receive().unwrap();
    assert_eq!(session.reply(Response::Applying), Err(Error::InvalidState));
    raw_send(&session.channel, &frame(2, 1), &[]);
    assert_eq!(client.receive(), Err(Error::InvalidState));
}

#[test]
fn fixed_refusal_and_recovery_do_not_drop_the_proof() {
    for response in [Response::Rejected, Response::RecoveryRequired] {
        let (mut client, mut session) = pair();
        let file = tempfile::tempfile().unwrap();
        client.acquire(file.as_fd()).unwrap();
        session.receive_acquire().unwrap();
        session.reply(response).unwrap();
        assert_eq!(client.receive().unwrap(), response);
        assert!(session.proof().is_some());
        assert_eq!(client.release(), Err(Error::InvalidState));
    }
}

#[test]
fn real_private_listener_accepts_expected_uid_and_cleans_its_socket() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("channel");
    let uid = rustix::process::geteuid().as_raw();
    let listener = Listener::bind(&path, uid).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    let mut client = Client::connect_peer(&path, uid).unwrap();
    let mut session = listener.accept().unwrap();
    let file = tempfile::tempfile().unwrap();
    client.acquire(file.as_fd()).unwrap();
    session.receive_acquire().unwrap();
    session.reply(Response::Rejected).unwrap();
    assert_eq!(client.receive().unwrap(), Response::Rejected);
    drop(listener);
    assert!(!path.exists());
}

#[test]
fn root_peer_required_by_public_client_and_foreign_uid_refused_by_listener() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("channel");
    let uid = rustix::process::geteuid().as_raw();
    let listener = Listener::bind(&path, uid + 1).unwrap();
    if uid != 0 {
        assert!(matches!(Client::connect(&path), Err(Error::PeerRejected)));
    } else {
        let _client = Client::connect(&path).unwrap();
    }
    assert!(matches!(listener.accept(), Err(Error::PeerRejected)));
}

#[test]
fn listener_refuses_unsafe_directory_or_existing_target_without_removal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("channel");
    fs::write(&path, b"keep").unwrap();
    assert!(Listener::bind(&path, 0).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o777)).unwrap();
    assert!(Listener::bind(&directory.path().join("other"), 0).is_err());
}

#[test]
fn replaced_socket_path_is_not_removed_by_listener_drop() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("channel");
    let listener = Listener::bind(&path, rustix::process::geteuid().as_raw()).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"replacement").unwrap();
    drop(listener);
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
}

#[test]
fn saturated_peer_has_a_finite_send_deadline() {
    let (mut client, _session) = pair();
    client.channel.timeout = Duration::from_millis(25);
    let mut full = false;
    for _ in 0..4096 {
        let result = net::send(
            &client.channel.fd,
            &frame(1, 2),
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        );
        if result == Err(rustix::io::Errno::AGAIN) {
            full = true;
            break;
        }
        assert_eq!(result.unwrap(), FRAME_BYTES);
    }
    assert!(full);
    let start = Instant::now();
    assert_eq!(client.channel.send(&frame(1, 2), None), Err(Error::Timeout));
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn stream_and_datagram_endpoints_are_not_silently_accepted() {
    for kind in [SocketType::STREAM, SocketType::DGRAM] {
        let (left, _right) =
            net::socketpair(AddressFamily::UNIX, kind, SocketFlags::CLOEXEC, None).unwrap();
        assert!(matches!(
            Endpoint::new(left, rustix::process::geteuid().as_raw()),
            Err(Error::PeerRejected)
        ));
    }
}

#[test]
fn unsolicited_ready_replay_release_and_early_calls_refuse() {
    let (mut client, session) = pair();
    assert_eq!(client.receive(), Err(Error::InvalidState));
    let proof = tempfile::tempfile().unwrap();
    client.acquire(proof.as_fd()).unwrap();
    raw_send(&session.channel, &frame(2, Response::Ready as u8), &[]);
    assert_eq!(client.receive(), Err(Error::InvalidState));
    assert_eq!(client.acquire(proof.as_fd()), Err(Error::InvalidState));
    assert_eq!(client.receive(), Err(Error::InvalidState));
}

#[test]
fn healthy_ready_lease_survives_bounded_idle_polls() {
    let (mut client, mut session) = pair();
    let file = tempfile::tempfile().unwrap();
    client.acquire(file.as_fd()).unwrap();
    session.receive_acquire().unwrap();
    session.reply(Response::Applying).unwrap();
    client.receive().unwrap();
    session.reply(Response::Ready).unwrap();
    client.receive().unwrap();
    client.channel.timeout = Duration::from_millis(10);
    session.channel.timeout = Duration::from_millis(10);
    assert_eq!(session.receive_release(), Err(Error::Idle));
    assert_eq!(client.receive(), Err(Error::Idle));
    client.release().unwrap();
    session.receive_release().unwrap();
    session.reply(Response::Releasing).unwrap();
    client.receive().unwrap();
    session.reply(Response::Released).unwrap();
    assert_eq!(client.receive(), Ok(Response::Released));
}
