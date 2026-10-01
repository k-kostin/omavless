use crate::listener_admission::{AdmittedListener, ListenerError};
use crate::session_owner_candidate::{SessionOwner, SessionProgress};
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, symlink};
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

fn session_owner(f: &Fixture, uid: u32, kernel: Kernel) -> (SessionOwner<Kernel>, PathBuf) {
    let path = f.0.join("session.sock");
    let listener = UnixListener::bind(&path).unwrap();
    (
        SessionOwner::from_prebound(listener, bound_state(f, uid), kernel, NS).unwrap(),
        path,
    )
}

fn client(path: &PathBuf, request: Request) -> UnixStream {
    let mut client = UnixStream::connect(path).unwrap();
    send(&mut client, request);
    client
}

fn admitted_listener(f: &Fixture) -> (AdmittedListener, PathBuf) {
    let parent = f.0.join("run");
    fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
    let directory = parent.join("omavless-netguard");
    fs::DirBuilder::new().mode(0o750).create(&directory).unwrap();
    let path = directory.join("control.sock");
    let listener = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
    let meta = fs::metadata(&parent).unwrap();
    (
        AdmittedListener::open_test_parent(
            listener,
            File::open(&parent).unwrap(),
            &path,
            (meta.uid(), meta.gid()),
            meta.gid(),
        )
        .unwrap(),
        path,
    )
}

#[test]
fn session_owner_serializes_clients_under_one_lock_without_replay() {
    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid(), Kernel::new(&f));
    assert_eq!(owner.poll_one(), SessionProgress::Idle);
    assert!(matches!(open_fixture(&f.0), Err(StateError::Busy)));

    let mut arm = client(&path, ARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(
        receive(&mut arm),
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            health: Health::Verified,
            ..
        }
    ));
    assert_eq!(owner.test_kernel().effects, 1);
    assert_eq!(record(owner.test_state()).state(), ReceiptState::Live { handle: 5 });

    let mut status = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(
        receive(&mut status),
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            ..
        }
    ));
    assert_eq!(owner.test_kernel().effects, 1);

    let mut disarm = client(&path, DISARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(
        receive(&mut disarm),
        Response::Status {
            protection: Protection::Disarmed {},
            health: Health::Verified,
            ..
        }
    ));
    assert_eq!(owner.test_kernel().effects, 2);
    let before = f.bytes();
    let mut duplicate = client(&path, DISARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(receive(&mut duplicate), Response::Status { .. }));
    assert_eq!(owner.test_kernel().effects, 2);
    assert_eq!(f.bytes(), before);
}

#[test]
fn malformed_and_stalled_clients_do_not_prevent_a_later_valid_client() {
    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid(), Kernel::new(&f));
    for bytes in [
        0_u32.to_be_bytes().to_vec(),
        (crate::protocol::MAX_FRAME_BYTES as u32 + 1)
            .to_be_bytes()
            .to_vec(),
        [8_u32.to_be_bytes().as_slice(), b"short"].concat(),
    ] {
        let mut peer = UnixStream::connect(&path).unwrap();
        peer.write_all(&bytes).unwrap();
        peer.shutdown(Shutdown::Write).unwrap();
        assert_eq!(
            owner.poll_one(),
            SessionProgress::Refused(ExchangeError::Receive(TransportError::InvalidFrame))
        );
    }
    let stalled = UnixStream::connect(&path).unwrap();
    let started = Instant::now();
    assert_eq!(
        owner.poll_one(),
        SessionProgress::Refused(ExchangeError::Receive(TransportError::Unavailable))
    );
    assert!(started.elapsed() >= Duration::from_secs(1));
    assert!(started.elapsed() < Duration::from_secs(4));
    drop(stalled);
    assert_eq!(owner.test_kernel().effects, 0);
    assert_eq!(f.bytes(), (None, None));

    let mut valid = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(receive(&mut valid), Response::Status { .. }));
    assert_eq!(owner.test_kernel().effects, 0);
}

#[test]
fn uncertain_accept_error_returns_once_and_does_not_poison_the_owner() {
    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid(), Kernel::new(&f));
    let mut called = 0;
    assert_eq!(
        owner.test_poll_with(|_| {
            called += 1;
            Err(std::io::Error::from(ErrorKind::Other))
        }),
        SessionProgress::AcceptUnavailable
    );
    assert_eq!(called, 1);
    assert_eq!(owner.poll_one(), SessionProgress::Idle);
    let mut valid = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(receive(&mut valid), Response::Status { .. }));
}

#[test]
fn unauthorized_peer_and_missing_enrollment_never_dispatch() {
    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid() + 1, Kernel::new(&f));
    let _unauthorized = client(&path, ARM);
    assert_eq!(
        owner.poll_one(),
        SessionProgress::Refused(ExchangeError::Receive(TransportError::Unauthorized))
    );
    assert_eq!(owner.test_kernel().effects, 0);
    assert_eq!(f.bytes(), (None, None));
    drop(owner);

    let listener = UnixListener::bind(f.0.join("without-enrollment.sock")).unwrap();
    let mut owner = SessionOwner::from_prebound(listener, f.state(), Kernel::new(&f), NS).unwrap();
    let _client = client(&f.0.join("without-enrollment.sock"), Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
    assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
    assert_eq!(owner.test_kernel().effects, 0);
}

#[test]
fn enrollment_rebinding_seals_session_without_disarming_or_replaying() {
    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid(), Kernel::new(&f));
    let mut arm = client(&path, ARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(receive(&mut arm), Response::Status { .. }));
    let before = f.bytes();
    kernel_rebind(&f);
    let _next = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
    assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
    assert_eq!(owner.test_kernel().effects, 1);
    assert_eq!(f.bytes(), before);

    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid(), Kernel::new(&f));
    let _status = client(&path, Request::Status {});
    assert_eq!(
        owner.test_poll_with(|listener| {
            kernel_rebind(&f);
            listener.accept().map(|(stream, _)| stream)
        }),
        SessionProgress::Refused(ExchangeError::Receive(TransportError::EnrollmentChanged))
    );
    assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
    assert_eq!(owner.test_kernel().effects, 0);

    let f = Fixture::new();
    let mut kernel = Kernel::new(&f);
    kernel.rebind_enrollment_on_effect = true;
    let (mut owner, path) = session_owner(&f, peer_uid(), kernel);
    let _arm = client(&path, ARM);
    assert_eq!(
        owner.poll_one(),
        SessionProgress::Refused(ExchangeError::ReplyDeliveryUnknown(
            TransportError::EnrollmentChanged
        ))
    );
    assert_eq!(owner.poll_one(), SessionProgress::AuthorityLost);
    assert_eq!(owner.test_kernel().effects, 1);
    assert_eq!(record(owner.test_state()).state(), ReceiptState::PendingCreate);
}

#[test]
fn lost_reply_and_poisoned_transaction_preserve_effect_history() {
    let f = Fixture::new();
    let (mut owner, path) = session_owner(&f, peer_uid(), Kernel::new(&f));
    let lost = client(&path, ARM);
    drop(lost);
    assert_eq!(
        owner.poll_one(),
        SessionProgress::Refused(ExchangeError::ReplyDeliveryUnknown(
            TransportError::Unavailable
        ))
    );
    assert_eq!(owner.test_kernel().effects, 1);
    assert_eq!(record(owner.test_state()).state(), ReceiptState::Live { handle: 5 });
    let mut status = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(
        receive(&mut status),
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            ..
        }
    ));
    assert_eq!(owner.test_kernel().effects, 1);

    let f = Fixture::new();
    let mut kernel = Kernel::new(&f);
    kernel.fail_after_effect = true;
    let (mut owner, path) = session_owner(&f, peer_uid(), kernel);
    let mut arm = client(&path, ARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert_eq!(
        receive(&mut arm),
        Response::Error {
            code: ErrorCode::ManualRecoveryRequired
        }
    );
    assert_eq!(record(owner.test_state()).state(), ReceiptState::PendingCreate);
    let mut status = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert_eq!(
        receive(&mut status),
        Response::Error {
            code: ErrorCode::ManualRecoveryRequired
        }
    );
    assert_eq!(owner.test_kernel().effects, 1);
}

#[test]
fn dropping_armed_listener_preserves_durable_evidence_for_restart() {
    let f = Fixture::new();
    let uid = peer_uid();
    let (mut owner, path) = session_owner(&f, uid, Kernel::new(&f));
    let mut arm = client(&path, ARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(receive(&mut arm), Response::Status { .. }));
    let before = f.bytes();
    drop(owner);
    assert_eq!(f.bytes(), before);

    let metadata = fs::metadata(&f.0).unwrap();
    let binding = EnrollmentBinding::open_test_parent(
        File::open(&f.0).unwrap(),
        (metadata.uid(), metadata.gid()),
    )
    .unwrap();
    let root = RootStateStore::open_test_parent(
        File::open(&f.0).unwrap(),
        (metadata.uid(), metadata.gid()),
        uid,
    )
    .unwrap();
    let mut state = LockedState::from_root(root);
    state.enrollment = Some(binding);
    let mut kernel = Kernel::new(&f);
    kernel.current = LIVE;
    let listener = UnixListener::bind(f.0.join("restarted.sock")).unwrap();
    let mut restarted = SessionOwner::from_prebound(listener, state, kernel, NS).unwrap();
    let mut status = client(&f.0.join("restarted.sock"), Request::Status {});
    assert_eq!(restarted.poll_one(), SessionProgress::Served);
    assert!(matches!(
        receive(&mut status),
        Response::Status {
            protection: Protection::Armed { generation: 7 },
            ..
        }
    ));
    assert_eq!(f.bytes(), before);
    assert_eq!(restarted.test_kernel().effects, 0);
}

#[test]
fn admitted_listener_refuses_wrong_address_permissions_and_package_group() {
    let f = Fixture::new();
    let (admitted, path) = admitted_listener(&f);
    assert_eq!(admitted.validate(), Ok(()));
    let parent = f.0.join("run");
    let meta = fs::metadata(&parent).unwrap();
    let wrong_path = parent.join("other.sock");
    let wrong = UnixListener::bind(&wrong_path).unwrap();
    assert!(matches!(
        AdmittedListener::open_test_parent(
            wrong,
            File::open(&parent).unwrap(),
            &path,
            (meta.uid(), meta.gid()),
            meta.gid(),
        ),
        Err(ListenerError::UnsafeOrUnavailable)
    ));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(admitted.validate(), Err(ListenerError::UnsafeOrUnavailable));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
    assert_eq!(admitted.validate(), Ok(()));
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o730)).unwrap();
    assert_eq!(admitted.validate(), Err(ListenerError::UnsafeOrUnavailable));
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(admitted.validate(), Ok(()));
    let wrong_group = meta.gid().checked_add(1).unwrap();
    let wrong = UnixListener::bind(parent.join("wrong-group.sock")).unwrap();
    assert!(matches!(
        AdmittedListener::open_test_parent(
            wrong,
            File::open(&parent).unwrap(),
            &parent.join("wrong-group.sock"),
            (meta.uid(), meta.gid()),
            wrong_group,
        ),
        Err(ListenerError::UnsafeOrUnavailable)
    ));
}

#[test]
fn socket_replacement_seals_owner_without_erasing_armed_records() {
    let f = Fixture::new();
    let (listener, path) = admitted_listener(&f);
    let mut owner =
        SessionOwner::from_admitted(listener, bound_state(&f, peer_uid()), Kernel::new(&f), NS)
            .unwrap();
    let mut arm = client(&path, ARM);
    assert_eq!(owner.poll_one(), SessionProgress::Served);
    assert!(matches!(receive(&mut arm), Response::Status { .. }));
    let before = f.bytes();
    fs::rename(&path, path.with_extension("old")).unwrap();
    let replacement = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
    let _queued = client(&path, Request::Status {});
    assert_eq!(owner.poll_one(), SessionProgress::ListenerLost);
    assert_eq!(owner.poll_one(), SessionProgress::ListenerLost);
    assert_eq!(owner.test_kernel().effects, 1);
    assert_eq!(f.bytes(), before);
    drop(replacement);
}

#[test]
fn directory_replacement_or_symlink_seals_admitted_listener() {
    let f = Fixture::new();
    let (admitted, path) = admitted_listener(&f);
    let directory = path.parent().unwrap();
    let old = f.0.join("old-runtime");
    fs::rename(directory, &old).unwrap();
    symlink(&old, directory).unwrap();
    assert_eq!(admitted.validate(), Err(ListenerError::UnsafeOrUnavailable));
    fs::remove_file(directory).unwrap();
    fs::DirBuilder::new().mode(0o750).create(directory).unwrap();
    let replacement = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
    assert_eq!(admitted.validate(), Err(ListenerError::UnsafeOrUnavailable));
    drop(replacement);
}

#[test]
fn listener_replacement_after_accept_does_not_dispatch_queued_request() {
    let f = Fixture::new();
    let (listener, path) = admitted_listener(&f);
    let mut owner =
        SessionOwner::from_admitted(listener, bound_state(&f, peer_uid()), Kernel::new(&f), NS)
            .unwrap();
    let _queued = client(&path, ARM);
    assert_eq!(
        owner.test_poll_with(|listener| {
            fs::rename(&path, path.with_extension("old")).unwrap();
            listener.accept().map(|(stream, _)| stream)
        }),
        SessionProgress::ListenerLost
    );
    assert_eq!(owner.poll_one(), SessionProgress::ListenerLost);
    assert_eq!(owner.test_kernel().effects, 0);
    assert_eq!(f.bytes(), (None, None));
}
