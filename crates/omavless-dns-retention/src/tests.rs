// SPDX-License-Identifier: MIT
use super::*;
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SocketAddrUnix,
    SocketFlags, SocketType,
};
use std::{
    fs,
    io::{IoSliceMut, Write},
    os::unix::{fs::FileTypeExt, net::UnixDatagram},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use tempfile::TempDir;

const BUS_NAME: &str = "org.freedesktop.systemd1";

#[test]
fn expired_operation_does_not_poll_bus_or_send_fdstore() {
    let polled = std::cell::Cell::new(false);
    assert!(
        complete(Instant::now(), async {
            polled.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!polled.get());
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture
            .retention
            .set_deadline(Instant::now() + Duration::from_secs(31)),
        Err(Error::PolicyMismatch)
    );
    fixture.retention.set_deadline(Instant::now()).unwrap();
    assert!(fixture.retention.retain(Fixture::file()).is_err());
    assert!(fixture.state.lock().unwrap().notifications.is_empty());
    assert!(fixture.retention.proof.is_some());
    assert!(fixture.retention.state == State::Quarantined);
    assert!(
        Retention::from_admitted_parts_with_deadline(
            fixture.retention.connection.clone(),
            &fixture.retention.manager_owner,
            fixture.retention.notify.try_clone().unwrap(),
            Instant::now(),
        )
        .is_err()
    );
}

#[test]
fn expired_removal_keeps_original_store_and_cannot_be_retried() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    let count = fixture.state.lock().unwrap().notifications.len();
    fixture.retention.set_deadline(Instant::now()).unwrap();
    assert!(fixture.retention.release_after_proven_boundary().is_err());
    fixture
        .retention
        .set_deadline(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::RecoveryRequired)
    );
    let state = fixture.state.lock().unwrap();
    assert_eq!(state.notifications.len(), count);
    assert!(state.stored.is_some());
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Fault {
    None,
    DiscardInsert,
    WrongInode,
    WrongName,
    InvalidPath,
    ExtraEntry,
    IgnoreRemove,
    BarrierByte,
    BarrierTimeout,
}

struct ManagerState {
    capacity: u32,
    preserve: String,
    access: String,
    main_pid: u32,
    stored: Option<OwnedFd>,
    notifications: Vec<Vec<u8>>,
    barriers: usize,
    fault: Fault,
}
impl Default for ManagerState {
    fn default() -> Self {
        Self {
            capacity: 1,
            preserve: "yes".into(),
            access: "main".into(),
            main_pid: std::process::id(),
            stored: None,
            notifications: vec![],
            barriers: 0,
            fault: Fault::None,
        }
    }
}

// Independent manager projection: no calls to the implementation's Identity.
fn projected(fd: &OwnedFd) -> Entry {
    let st = rustix::fs::fstat(fd).unwrap();
    let flags = rustix::fs::fcntl_getfl(fd).unwrap().bits() & !rustix::fs::OFlags::LARGEFILE.bits();
    (
        FD_NAME.into(),
        st.st_mode,
        rustix::fs::major(st.st_dev),
        rustix::fs::minor(st.st_dev),
        st.st_ino,
        rustix::fs::major(st.st_rdev),
        rustix::fs::minor(st.st_rdev),
        "/synthetic/ordinary-file".into(),
        flags,
    )
}

struct MockManager(Arc<Mutex<ManagerState>>);
#[zbus::interface(name = "org.freedesktop.systemd1.Service")]
impl MockManager {
    #[zbus(property)]
    fn file_descriptor_store_max(&self) -> u32 {
        self.0.lock().unwrap().capacity
    }
    #[zbus(property, name = "NFileDescriptorStore")]
    fn n_file_descriptor_store(&self) -> u32 {
        u32::from(self.0.lock().unwrap().stored.is_some())
    }
    #[zbus(property)]
    fn file_descriptor_store_preserve(&self) -> String {
        self.0.lock().unwrap().preserve.clone()
    }
    #[zbus(property)]
    fn notify_access(&self) -> String {
        self.0.lock().unwrap().access.clone()
    }
    #[zbus(property, name = "MainPID")]
    fn main_pid(&self) -> u32 {
        self.0.lock().unwrap().main_pid
    }
    fn dump_file_descriptor_store(&self) -> Vec<Entry> {
        let state = self.0.lock().unwrap();
        let Some(fd) = &state.stored else {
            return vec![];
        };
        let mut entry = projected(fd);
        match state.fault {
            Fault::WrongInode => entry.4 += 1,
            Fault::WrongName => entry.0 = "foreign-slot".into(),
            Fault::InvalidPath => entry.7 = "private\nmaterial".into(),
            _ => (),
        }
        if state.fault == Fault::ExtraEntry {
            vec![entry.clone(), entry]
        } else {
            vec![entry]
        }
    }
}

struct Bus {
    directory: TempDir,
    child: Child,
}
impl Bus {
    fn start() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("omavless-retention-bus-")
            .tempdir()
            .unwrap();
        let socket = directory.path().join("bus");
        let child = Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--nopidfile",
                "--address",
                &format!("unix:path={}", socket.display()),
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("private dbus-daemon fixture required");
        let mut bus = Self { directory, child };
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if fs::symlink_metadata(&socket).is_ok_and(|m| m.file_type().is_socket()) {
                return bus;
            }
            assert!(
                bus.child.try_wait().unwrap().is_none(),
                "private bus exited"
            );
            assert!(Instant::now() < deadline, "private bus startup timeout");
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn address(&self) -> String {
        format!("unix:path={}", self.directory.path().join("bus").display())
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Fixture {
    retention: Retention,
    state: Arc<Mutex<ManagerState>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    server: Option<Connection>,
    bus: Bus,
}
impl Fixture {
    fn new() -> Self {
        let bus = Bus::start();
        let state = Arc::new(Mutex::new(ManagerState::default()));
        let server = zbus::blocking::connection::Builder::address(bus.address().as_str())
            .unwrap()
            .max_queued(8)
            .serve_at(SERVICE_PATH, MockManager(state.clone()))
            .unwrap()
            .name(BUS_NAME)
            .unwrap()
            .build()
            .unwrap();
        let connection = zbus::blocking::connection::Builder::address(bus.address().as_str())
            .unwrap()
            .max_queued(8)
            .build()
            .unwrap();
        let owner = server.unique_name().unwrap().to_string();
        let notify_path = bus.directory.path().join("notify");
        let receiver = UnixDatagram::bind(&notify_path).unwrap();
        receiver.set_nonblocking(true).unwrap();
        let sender = net::socket_with(
            AddressFamily::UNIX,
            SocketType::DGRAM,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        net::connect(&sender, &SocketAddrUnix::new(&notify_path).unwrap()).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_state = state.clone();
        let worker_stop = stop.clone();
        let worker = thread::spawn(move || {
            let mut delayed_barriers: Vec<OwnedFd> = vec![];
            while !worker_stop.load(Ordering::Acquire) {
                let mut polls = [PollFd::new(&receiver, PollFlags::IN)];
                if poll(
                    &mut polls,
                    Some(&Timespec {
                        tv_sec: 0,
                        tv_nsec: 20_000_000,
                    }),
                )
                .unwrap()
                    == 0
                {
                    continue;
                }
                let mut bytes = [0u8; 256];
                let mut storage =
                    [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(1))];
                let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
                let message = net::recvmsg(
                    &receiver,
                    &mut [IoSliceMut::new(&mut bytes)],
                    &mut ancillary,
                    RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
                )
                .unwrap();
                assert!(
                    !message
                        .flags
                        .intersects(net::ReturnFlags::TRUNC | net::ReturnFlags::CTRUNC)
                );
                let mut fds = vec![];
                for record in ancillary.drain() {
                    if let RecvAncillaryMessage::ScmRights(rights) = record {
                        fds.extend(rights);
                    } else {
                        panic!("unexpected fixture ancillary");
                    }
                }
                let data = &bytes[..message.bytes];
                let mut state = worker_state.lock().unwrap();
                state.notifications.push(data.to_vec());
                match data {
                    STORE => {
                        assert_eq!(fds.len(), 1);
                        if state.fault == Fault::DiscardInsert {
                            state.capacity = 0;
                        }
                        if state.capacity == 1 && state.stored.is_none() {
                            state.stored = fds.pop();
                        }
                    }
                    b"BARRIER=1" => {
                        assert_eq!(fds.len(), 1);
                        state.barriers += 1;
                        if state.fault == Fault::BarrierByte {
                            rustix::io::write(&fds[0], b"x").unwrap();
                        }
                        if state.fault == Fault::BarrierTimeout {
                            delayed_barriers.extend(fds);
                        }
                        // Dropping pipe writer is real notification-barrier completion.
                    }
                    REMOVE => {
                        assert!(fds.is_empty());
                        if state.fault != Fault::IgnoreRemove {
                            state.stored = None;
                        }
                    }
                    _ => panic!("non-fixed notification"),
                }
            }
        });
        Self {
            retention: Retention {
                connection,
                manager_owner: owner,
                notify: sender,
                timeout: Duration::from_secs(2),
                deadline: None,
                state: State::Fresh,
                proof: None,
                identity: None,
            },
            state,
            stop,
            worker: Some(worker),
            server: Some(server),
            bus,
        }
    }
    fn file() -> OwnedFd {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"synthetic-retained-object").unwrap();
        file.into()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let joined = self.worker.take().unwrap().join();
        if !thread::panicking() {
            assert!(joined.is_ok(), "notification fixture failed");
        }
    }
}

#[test]
fn real_rights_barriers_and_typed_metadata_confirm_one_object() {
    let mut fixture = Fixture::new();
    let proof = Fixture::file();
    let original = fstat(&proof).unwrap();
    fixture.retention.retain(proof).unwrap();
    fixture.retention.verify().unwrap();
    {
        let state = fixture.state.lock().unwrap();
        let stored = state.stored.as_ref().unwrap();
        assert_eq!(fstat(stored).unwrap().st_ino, original.st_ino);
        assert!(
            rustix::io::fcntl_getfd(stored)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let mut bytes = [0; 32];
        assert_eq!(rustix::io::pread(stored, &mut bytes, 0).unwrap(), 25);
        assert_eq!(&bytes[..25], b"synthetic-retained-object");
    }
    fixture.retention.release_after_proven_boundary().unwrap();
    assert!(fixture.retention.proof.is_none());
    let state = fixture.state.lock().unwrap();
    assert!(state.stored.is_none());
    assert_eq!(state.barriers, 2);
    assert_eq!(
        state.notifications,
        [STORE, b"BARRIER=1", REMOVE, b"BARRIER=1"]
    );
}

#[test]
fn barrier_success_without_capacity_never_confirms_retention() {
    let mut fixture = Fixture::new();
    fixture.state.lock().unwrap().fault = Fault::DiscardInsert;
    assert_eq!(
        fixture.retention.retain(Fixture::file()),
        Err(Error::PolicyMismatch)
    );
    assert_eq!(fixture.state.lock().unwrap().barriers, 1);
    assert!(fixture.state.lock().unwrap().stored.is_none());
    assert!(fixture.retention.proof.is_some());
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::RecoveryRequired)
    );
}

#[test]
fn preexisting_slot_even_same_metadata_is_not_adopted() {
    let mut fixture = Fixture::new();
    let proof = Fixture::file();
    fixture.state.lock().unwrap().stored = Some(proof.try_clone().unwrap());
    assert_eq!(fixture.retention.retain(proof), Err(Error::ExistingStore));
    assert!(fixture.state.lock().unwrap().notifications.is_empty());
    assert!(fixture.state.lock().unwrap().stored.is_some());
}

#[test]
fn wrong_metadata_or_multiple_entries_require_quarantine() {
    for fault in [
        Fault::WrongInode,
        Fault::WrongName,
        Fault::InvalidPath,
        Fault::ExtraEntry,
    ] {
        let mut fixture = Fixture::new();
        fixture.state.lock().unwrap().fault = fault;
        assert!(fixture.retention.retain(Fixture::file()).is_err());
        assert!(fixture.retention.proof.is_some());
        assert_eq!(fixture.retention.verify(), Err(Error::RecoveryRequired));
        assert_eq!(
            fixture.retention.release_after_proven_boundary(),
            Err(Error::RecoveryRequired)
        );
        assert_eq!(fixture.state.lock().unwrap().notifications.len(), 2);
    }
}

#[test]
fn policy_requires_main_pid_main_only_exact_capacity_and_preserve_yes() {
    for changed in 0..4 {
        let mut fixture = Fixture::new();
        {
            let mut state = fixture.state.lock().unwrap();
            match changed {
                0 => state.capacity = 0,
                1 => state.preserve = "restart".into(),
                2 => state.access = "all".into(),
                _ => state.main_pid += 1,
            }
        }
        assert_eq!(
            fixture.retention.retain(Fixture::file()),
            Err(Error::PolicyMismatch)
        );
        assert!(fixture.state.lock().unwrap().notifications.is_empty());
    }
}

#[test]
fn timed_out_or_nonempty_barrier_is_not_acknowledgement() {
    for fault in [Fault::BarrierByte, Fault::BarrierTimeout] {
        let mut fixture = Fixture::new();
        fixture.state.lock().unwrap().fault = fault;
        fixture.retention.timeout = Duration::from_millis(80);
        assert_eq!(
            fixture.retention.retain(Fixture::file()),
            Err(Error::OutcomeUnknown)
        );
        assert!(fixture.retention.proof.is_some());
        assert!(fixture.state.lock().unwrap().stored.is_some());
    }
}

#[test]
fn unknown_removal_is_not_retried_or_reported_released() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    fixture.state.lock().unwrap().fault = Fault::IgnoreRemove;
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::OutcomeUnknown)
    );
    assert!(fixture.retention.proof.is_some());
    let count = fixture.state.lock().unwrap().notifications.len();
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(fixture.state.lock().unwrap().notifications.len(), count);
}

#[test]
fn explicit_uncertainty_blocks_removal_and_readback_promotion() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    fixture.retention.quarantine();
    assert_eq!(fixture.retention.verify(), Err(Error::RecoveryRequired));
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::RecoveryRequired)
    );
    assert!(fixture.state.lock().unwrap().stored.is_some());
    assert_eq!(fixture.state.lock().unwrap().notifications.len(), 2);
}

#[test]
fn manager_owner_replacement_is_not_adopted() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    fixture.server.take().unwrap().close().unwrap();
    let replacement = zbus::blocking::connection::Builder::address(fixture.bus.address().as_str())
        .unwrap()
        .serve_at(SERVICE_PATH, MockManager(fixture.state.clone()))
        .unwrap()
        .name(BUS_NAME)
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(
        replacement.unique_name().unwrap().as_str(),
        fixture.retention.manager_owner
    );
    assert_eq!(fixture.retention.verify(), Err(Error::ManagerUnavailable));
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::RecoveryRequired)
    );
}

#[test]
fn store_disappearance_does_not_allow_automatic_reinsertion() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    fixture.state.lock().unwrap().stored = None;
    assert_eq!(fixture.retention.verify(), Err(Error::RetentionUnconfirmed));
    assert_eq!(
        fixture.retention.retain(Fixture::file()),
        Err(Error::InvalidState)
    );
    assert!(fixture.retention.proof.is_some());
}

#[test]
fn state_and_errors_do_not_expose_metadata() {
    let fixture = Fixture::new();
    assert_eq!(format!("{:?}", fixture.retention), "Retention { .. }");
    for error in [
        Error::InvalidState,
        Error::InvalidDescriptor,
        Error::ManagerUnavailable,
        Error::InvalidReply,
        Error::PolicyMismatch,
        Error::ExistingStore,
        Error::RetentionUnconfirmed,
        Error::OutcomeUnknown,
        Error::RecoveryRequired,
    ] {
        assert!(error.to_string().len() <= 80);
        assert!(error.to_string().is_ascii());
        assert!(!error.to_string().contains('/'));
    }
}

#[test]
fn local_drop_never_sends_removal_and_manager_keeps_reference() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    let state = fixture.state.clone();
    drop(fixture);
    let state = state.lock().unwrap();
    assert!(state.stored.is_some());
    assert_eq!(state.notifications, [STORE, b"BARRIER=1"]);
}

#[test]
fn uncertain_barrier_after_remove_still_keeps_local_object() {
    let mut fixture = Fixture::new();
    fixture.retention.retain(Fixture::file()).unwrap();
    fixture.state.lock().unwrap().fault = Fault::BarrierTimeout;
    fixture.retention.timeout = Duration::from_millis(80);
    assert_eq!(
        fixture.retention.release_after_proven_boundary(),
        Err(Error::OutcomeUnknown)
    );
    assert!(fixture.retention.proof.is_some());
    assert!(fixture.state.lock().unwrap().stored.is_none());
    assert_eq!(fixture.retention.verify(), Err(Error::RecoveryRequired));
}

#[test]
fn metadata_fields_are_checked_but_not_misrepresented_as_open_description_identity() {
    let fd = Fixture::file();
    let identity = Identity::capture(&fd).unwrap();
    let original = projected(&fd);
    assert!(identity.matches(&original));
    for field in 0..8 {
        let mut entry = original.clone();
        match field {
            0 => entry.0.push('x'),
            1 => entry.1 ^= 1,
            2 => entry.2 += 1,
            3 => entry.3 += 1,
            4 => entry.4 += 1,
            5 => entry.5 += 1,
            6 => entry.6 += 1,
            _ => entry.8 ^= 1,
        }
        assert!(!identity.matches(&entry));
    }
    // Duplication has identical metadata, which is intentionally not claimed
    // to distinguish two independent opens of a shared character-device inode.
    assert!(identity.matches(&projected(&fd.try_clone().unwrap())));
}

#[test]
fn invalid_body_fd_oversize_or_wrong_sender_never_becomes_metadata() {
    let fixture = Fixture::new();
    let call = Message::method_call(SERVICE_PATH, "DumpFileDescriptorStore")
        .unwrap()
        .build(&())
        .unwrap();
    let wrong = Message::method_return(&call.header())
        .unwrap()
        .build(&true)
        .unwrap();
    assert_eq!(bounded::<Vec<Entry>>(&wrong), Err(Error::InvalidReply));
    assert_eq!(
        fixture.retention.verify_sender(&wrong),
        Err(Error::ManagerUnavailable)
    );
    let wrong_sender = Message::method_return(&call.header())
        .unwrap()
        .sender(":9.999")
        .unwrap()
        .build(&())
        .unwrap();
    assert_eq!(
        fixture.retention.verify_sender(&wrong_sender),
        Err(Error::ManagerUnavailable)
    );
    let oversized = Message::method_return(&call.header())
        .unwrap()
        .build(&vec![0u8; MAX_REPLY])
        .unwrap();
    assert_eq!(bounded::<Vec<u8>>(&oversized), Err(Error::InvalidReply));
    let file = tempfile::tempfile().unwrap();
    let descriptor = zbus::zvariant::Fd::from(&file);
    let with_fd = Message::method_return(&call.header())
        .unwrap()
        .build(&descriptor)
        .unwrap();
    assert_eq!(bounded::<Vec<Entry>>(&with_fd), Err(Error::InvalidReply));
}

#[test]
fn trusted_parts_constructor_checks_policy_before_any_notification() {
    let fixture = Fixture::new();
    let mut retention = Retention::from_admitted_parts(
        fixture.retention.connection.clone(),
        &fixture.retention.manager_owner,
        fixture.retention.notify.try_clone().unwrap(),
    )
    .unwrap();
    assert!(fixture.state.lock().unwrap().notifications.is_empty());
    retention.retain(Fixture::file()).unwrap();
    retention.release_after_proven_boundary().unwrap();
    fixture.state.lock().unwrap().capacity = 0;
    assert_eq!(
        Retention::from_admitted_parts(
            fixture.retention.connection.clone(),
            &fixture.retention.manager_owner,
            fixture.retention.notify.try_clone().unwrap(),
        )
        .unwrap_err(),
        Error::PolicyMismatch
    );
}

#[test]
fn trusted_parts_constructor_rejects_non_unique_owner_and_non_datagram_fd() {
    let fixture = Fixture::new();
    assert_eq!(
        Retention::from_admitted_parts(
            fixture.retention.connection.clone(),
            "org.freedesktop.systemd1",
            fixture.retention.notify.try_clone().unwrap(),
        )
        .unwrap_err(),
        Error::PolicyMismatch
    );
    assert_eq!(
        Retention::from_admitted_parts(
            fixture.retention.connection.clone(),
            &fixture.retention.manager_owner,
            Fixture::file(),
        )
        .unwrap_err(),
        Error::PolicyMismatch
    );
    let (stream, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    assert_eq!(
        Retention::from_admitted_parts(
            fixture.retention.connection.clone(),
            &fixture.retention.manager_owner,
            stream.into(),
        )
        .unwrap_err(),
        Error::PolicyMismatch
    );
}
