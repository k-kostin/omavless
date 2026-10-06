// SPDX-License-Identifier: MIT
//! New source-boundary fixtures. Compile only until primary + independent GO.
use super::*;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zbus::{Connection, connection::Builder};

async fn fixture_bound<T>(future: impl std::future::Future<Output = zbus::Result<T>>) -> T {
    within(Instant::now() + Duration::from_secs(2), async {
        future.await.map_err(|_| Lost::Unavailable)
    })
    .await
    .expect("bounded private fixture operation")
}
static FIXTURE_UNKNOWN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shutdown {
    Active,
    Completed,
    Unknown,
}

struct Login {
    sleeping: Arc<AtomicBool>,
    race: bool,
    stall: bool,
}
#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Login {
    #[zbus(property)]
    async fn preparing_for_sleep(&self, #[zbus(connection)] connection: &Connection) -> bool {
        if self.stall {
            async_io::Timer::after(Duration::from_secs(3)).await;
        }
        if self.race {
            fixture_bound(connection.emit_signal(
                None::<&str>,
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                "PrepareForSleep",
                &true,
            ))
            .await;
        }
        self.sleeping.load(Ordering::Acquire)
    }
}

struct PrivateBus {
    directory: Option<tempfile::TempDir>,
    socket: PathBuf,
    child: Option<Child>,
    shutdown: Shutdown,
}
impl PrivateBus {
    fn new() -> Self {
        assert!(
            !FIXTURE_UNKNOWN.load(Ordering::Acquire),
            "unresolved original fixture stops later acquisition"
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        let directory = tempfile::Builder::new()
            .prefix("ov-events-")
            .tempdir()
            .unwrap();
        let runtime = directory.path().join("bus");
        fs::DirBuilder::new().mode(0o700).create(&runtime).unwrap();
        let socket = runtime.join("socket");
        let config = directory.path().join("bus.conf");
        let xml = format!(
            "<busconfig><type>session</type><auth>EXTERNAL</auth><listen>unix:path={}</listen><policy context=\"default\"><allow own=\"*\"/><allow send_destination=\"*\"/><allow receive_sender=\"*\"/></policy></busconfig>",
            socket.display()
        );
        omavless_store::atomic_replace_private(
            &config,
            xml.as_bytes(),
            nix::unistd::Uid::current().as_raw(),
        )
        .unwrap();
        let child = Command::new("/usr/bin/dbus-daemon")
            .env_clear()
            .arg("--nofork")
            .arg("--nopidfile")
            .arg("--config-file")
            .arg(&config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let bus = Self {
            directory: Some(directory),
            socket,
            child: Some(child),
            shutdown: Shutdown::Active,
        };
        while !fs::symlink_metadata(&bus.socket).is_ok_and(|m| m.file_type().is_socket()) {
            assert!(Instant::now() < deadline, "owned private bus unavailable");
            std::thread::sleep(Duration::from_millis(2));
        }
        bus
    }
    fn connection(&self) -> Connection {
        async_io::block_on(async {
            fixture_bound(
                Builder::address(format!("unix:path={}", self.socket.display()).as_str())
                    .unwrap()
                    .build(),
            )
            .await
        })
    }
    fn login(&self, sleeping: bool, race: bool, stall: bool) -> Connection {
        async_io::block_on(async {
            fixture_bound(
                Builder::address(format!("unix:path={}", self.socket.display()).as_str())
                    .unwrap()
                    .name("org.freedesktop.login1")
                    .unwrap()
                    .serve_at(
                        "/org/freedesktop/login1",
                        Login {
                            sleeping: Arc::new(AtomicBool::new(sleeping)),
                            race,
                            stall,
                        },
                    )
                    .unwrap()
                    .build(),
            )
            .await
        })
    }
    fn stop(&mut self) -> Result<(), Shutdown> {
        match self.shutdown {
            Shutdown::Completed => return Ok(()),
            Shutdown::Unknown => return Err(Shutdown::Unknown),
            Shutdown::Active => {}
        }
        // Consume cleanup eligibility before the first operation that can fail.
        // Any unknown result forbids another kill/reap attempt on Drop.
        self.shutdown = Shutdown::Unknown;
        let child = self.child.as_mut().unwrap();
        match child.try_wait() {
            Ok(Some(_)) => {
                self.shutdown = Shutdown::Completed;
                return Ok(());
            }
            Ok(None) => {}
            Err(_) => {
                FIXTURE_UNKNOWN.store(true, Ordering::Release);
                return Err(Shutdown::Unknown);
            }
        }
        if child.kill().is_err() {
            FIXTURE_UNKNOWN.store(true, Ordering::Release);
            return Err(Shutdown::Unknown);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.shutdown = Shutdown::Completed;
                    return Ok(());
                }
                Err(_) => {
                    FIXTURE_UNKNOWN.store(true, Ordering::Release);
                    return Err(Shutdown::Unknown);
                }
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                FIXTURE_UNKNOWN.store(true, Ordering::Release);
                return Err(Shutdown::Unknown);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for PrivateBus {
    fn drop(&mut self) {
        if self.stop().is_err() {
            // Retain the ORIGINAL handle together with its owned directory,
            // rather than retrying signals or invoking TempDir cleanup. This
            // test process cannot claim custody survives its fatal exit.
            let _original = Box::leak(Box::new((self.child.take(), self.directory.take())));
        }
    }
}
fn signal(connection: &Connection, sleeping: bool) {
    async_io::block_on(fixture_bound(connection.emit_signal(
        None::<&str>,
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        "PrepareForSleep",
        &sleeping,
    )));
}
fn next(source: &mut HostEventSource) -> Result<Emission, Lost> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(event) = source.next()? {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err(Lost::Deadline);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn private_bus_original_owner_signals_and_pause_have_typed_contiguous_sequence() {
    let bus = PrivateBus::new();
    let login = bus.login(false, false, false);
    let mut source = HostEventSource::fixture(&bus.socket).unwrap();
    assert_eq!(source.sleep_state(), SleepState::Awake);
    assert!(source.next().unwrap().is_none()); // Initial false is NOT Resume.
    source.quiescent().unwrap().recheck().unwrap();
    signal(&login, true);
    let suspend = next(&mut source).unwrap();
    assert_eq!((suspend.sequence, suspend.event), (1, Event::Suspend));
    assert_eq!(source.sleep_state(), SleepState::Suspended);
    assert!(matches!(source.quiescent(), Err(NotCurrent::Suspended)));
    signal(&login, false);
    let resume = next(&mut source).unwrap();
    assert_eq!((resume.sequence, resume.event), (2, Event::Resume));
    source.quiescent().unwrap().recheck().unwrap();
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn initial_observed_suspend_never_becomes_empty_transport_permission() {
    let bus = PrivateBus::new();
    let _login = bus.login(true, false, false);
    let mut source = HostEventSource::fixture(&bus.socket).unwrap();
    assert_eq!(source.next().unwrap().unwrap().event, Event::Suspend);
    assert!(matches!(source.quiescent(), Err(NotCurrent::Suspended)));
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn same_uid_spoofed_sender_cannot_supply_logind_hint_or_rearm_source() {
    let bus = PrivateBus::new();
    let login = bus.login(false, false, false);
    let mut source = HostEventSource::fixture(&bus.socket).unwrap();
    let spoof = bus.connection();
    signal(&spoof, true);
    assert_eq!(next(&mut source).err(), Some(Lost::Authentication));
    signal(&login, false);
    assert_eq!(source.next().err(), Some(Lost::Authentication));
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn owner_replacement_and_bus_loss_are_terminal_without_reconnect() {
    for replacement in [false, true] {
        let mut bus = PrivateBus::new();
        let login = bus.login(false, false, false);
        let mut source = HostEventSource::fixture(&bus.socket).unwrap();
        if replacement {
            async_io::block_on(fixture_bound(login.release_name("org.freedesktop.login1")));
            let _new = bus.login(false, false, false);
            assert!(source.quiescent().is_err());
        } else {
            bus.stop().unwrap();
            assert!(source.quiescent().is_err());
        }
        assert!(source.readable().is_err());
        assert!(source.next().is_err());
    }
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn subscribing_before_initial_property_detects_race_and_whole_startup_deadline() {
    for stalled in [false, true] {
        let bus = PrivateBus::new();
        let _login = bus.login(false, !stalled, stalled);
        let started = Instant::now();
        let result = HostEventSource::fixture(&bus.socket);
        assert_eq!(
            result.err(),
            Some(if stalled {
                Lost::Deadline
            } else {
                Lost::InitializationRace
            })
        );
        assert!(started.elapsed() < Duration::from_millis(2400));
    }
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn original_queue_stress_and_bad_signal_body_terminalize_without_public_payload() {
    for malformed in [false, true] {
        let bus = PrivateBus::new();
        let login = bus.login(false, false, false);
        let mut source = HostEventSource::fixture(&bus.socket).unwrap();
        if malformed {
            async_io::block_on(fixture_bound(login.emit_signal(
                None::<&str>,
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                "PrepareForSleep",
                &"not-a-boolean",
            )));
        } else {
            for n in 0..64 {
                signal(&login, n % 2 == 0);
            }
        }
        assert!(next(&mut source).is_err());
        assert!(source.next().is_err());
    }
}

fn header(big: bool, fields: u32, body: u32) -> [u8; 16] {
    let mut h = [0; 16];
    h[0] = if big { b'B' } else { b'l' };
    h[1] = 4;
    h[3] = 1;
    for (at, n) in [(4, body), (8, 1), (12, fields)] {
        let bytes = if big {
            n.to_be_bytes()
        } else {
            n.to_le_bytes()
        };
        h[at..at + 4].copy_from_slice(&bytes);
    }
    h
}
fn binary() -> bounded_bus::Framing {
    let mut state = bounded_bus::Framing::new();
    state.written(b"BEGIN\r\n").unwrap();
    state
}
#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn final_header_admission_bounds_both_endians_padding_and_no_allocation_bypass() {
    for big in [false, true] {
        let mut state = binary();
        let h = header(big, 1, (MAX_WIRE_BYTES - 24) as u32);
        state.received_bytes(&h[..15]).unwrap();
        assert!(state.partial());
        state.received_bytes(&h[15..]).unwrap(); // 16 + fields1 + pad7 + body =8192.
        let mut exceeded = binary();
        let h = header(big, 1, (MAX_WIRE_BYTES - 23) as u32);
        exceeded.received_bytes(&h[..15]).unwrap();
        assert_eq!(exceeded.received_bytes(&h[15..]), Err(Lost::Overflow));
        assert_eq!(exceeded.loss(), Err(Lost::Overflow));
        let mut giant = binary();
        assert_eq!(
            giant.received_bytes(&header(big, u32::MAX, u32::MAX)),
            Err(Lost::Overflow)
        );
    }
}
#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn auth_partial_begin_line_bytes_eof_and_header_version_type_are_terminally_bounded() {
    let mut state = bounded_bus::Framing::new();
    state.written(b"BE").unwrap();
    state.written(b"GIN\r").unwrap();
    state.received_bytes(b"O").unwrap(); // still one-byte AUTH mode, not binary.
    state.written(b"\n").unwrap();
    state.received_bytes(&header(false, 0, 0)).unwrap();
    assert_eq!(state.received_bytes(&[]), Err(Lost::Unavailable));
    for offset in [0, 1, 2, 3, 8] {
        let mut bad = binary();
        let mut h = header(false, 0, 0);
        h[offset] = if offset == 8 { 0 } else { 255 };
        assert_eq!(bad.received_bytes(&h), Err(Lost::InvalidFrame));
    }
    let mut line = bounded_bus::Framing::new();
    assert_eq!(line.received_bytes(&[b'x'; 129]), Err(Lost::Overflow));
    let mut total = bounded_bus::Framing::new();
    for _ in 0..10 {
        let _ = total.received_bytes(&[b'\n'; 128]);
    }
    assert_eq!(total.loss(), Err(Lost::Overflow));
}
#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn frame_accounting_does_not_reset_while_partial_or_undrained() {
    let mut state = binary();
    state.received_bytes(&header(false, 0, 0)).unwrap();
    assert!(state.pending());
    assert_eq!(
        state.reset_after_drain(MAX_POLL_BYTES),
        Err(Lost::InvalidFrame)
    );
    let mut clean = binary();
    clean.received_bytes(&header(false, 0, 0)).unwrap();
    clean.retire_one().unwrap();
    clean.reset_after_drain(MAX_POLL_BYTES).unwrap();
    assert!(!clean.pending());
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn exact_signature_and_complete_typed_body_refuse_trailing_wire_bytes() {
    use zbus::zvariant::Signature;
    assert_eq!(
        logind::fixture_exact_bool(vec![1, 0, 0, 0], &Signature::Bool),
        Ok(true)
    );
    assert_eq!(
        logind::fixture_exact_bool(vec![1, 0, 0, 0, 0, 0, 0, 0], &Signature::Bool),
        Err(Lost::InvalidFrame)
    );
    assert_eq!(
        logind::fixture_exact_bool(vec![1, 0, 0, 0], &Signature::U32),
        Err(Lost::InvalidFrame)
    );
    assert!(zbus::names::UniqueName::try_from(":not-a-unique-name").is_err());
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn completed_original_bus_shutdown_is_idempotent_without_another_signal() {
    let mut bus = PrivateBus::new();
    bus.stop().unwrap();
    assert_eq!(bus.shutdown, Shutdown::Completed);
    bus.stop().unwrap();
    assert_eq!(bus.shutdown, Shutdown::Completed);
}
#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn unexpected_owned_descriptors_are_closed_during_auth_and_binary_partial_read() {
    use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};
    use std::io::IoSlice;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use zbus::connection::socket::{ReadHalf, Socket};
    for binary_phase in [false, true] {
        let (a, b) = UnixStream::pair().unwrap();
        let (socket, monitor) = bounded_bus::BoundedSocket::new(async_io::Async::new(a).unwrap());
        if binary_phase {
            monitor.state().unwrap().written(b"BEGIN\r\n").unwrap();
        }
        let file = tempfile::tempfile().unwrap();
        let fds = [file.as_raw_fd()];
        sendmsg::<()>(
            b.as_raw_fd(),
            &[IoSlice::new(b"x")],
            &[ControlMessage::ScmRights(&fds)],
            MsgFlags::empty(),
            None,
        )
        .unwrap();
        let mut split = socket.split();
        let mut byte = [0; 16];
        assert!(
            async_io::block_on(within(Instant::now() + Duration::from_secs(2), async {
                split
                    .read_mut()
                    .recvmsg(&mut byte)
                    .await
                    .map_err(|_| Lost::Unavailable)
            }))
            .is_err()
        );
        assert_eq!(
            monitor.state().unwrap().loss(),
            Err(Lost::UnexpectedDescriptors)
        );
    }
}
fn nl(kind: u16, body: &[u8], pid: u32, seq: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&((16 + body.len()) as u32).to_ne_bytes());
    bytes.extend_from_slice(&kind.to_ne_bytes());
    bytes.extend_from_slice(&0u16.to_ne_bytes());
    bytes.extend_from_slice(&seq.to_ne_bytes());
    bytes.extend_from_slice(&pid.to_ne_bytes());
    bytes.extend_from_slice(body);
    bytes
}
#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn kernel_recv_metadata_not_payload_pid_or_sequence_authenticates_route_datagram() {
    for (pid, seq) in [(0, 0), (777, u32::MAX), (123, 2)] {
        let data = nl(16, &[0; 16], pid, seq);
        assert_eq!(route::synthetic(&data, 0, false), Ok(1));
        assert_eq!(
            route::synthetic(&data, 10, false),
            Err(Lost::Authentication)
        );
        assert_eq!(route::synthetic(&data, 0, true), Err(Lost::Overflow));
    }
}

#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn only_typed_emitted_hints_advance_sequence_and_invalid_route_revokes_original_source() {
    let bus = PrivateBus::new();
    let login = bus.login(false, false, false);
    let mut source = HostEventSource::fixture(&bus.socket).unwrap();
    let mut burst = Vec::new();
    for _ in 0..20 {
        burst.extend(nl(16, &[0; 16], 888, 0));
    }
    source.synthetic_route(&burst, 0, false).unwrap();
    let changed = source.next().unwrap().unwrap();
    assert_eq!(
        (changed.sequence, changed.event),
        (1, Event::NetworkChanged)
    );
    source
        .synthetic_route(&nl(1, &[], 999, u32::MAX), 0, false)
        .unwrap();
    assert!(source.next().unwrap().is_none());
    signal(&login, true);
    let suspend = next(&mut source).unwrap();
    assert_eq!((suspend.sequence, suspend.event), (2, Event::Suspend));
    source
        .synthetic_route(&nl(16, &[0; 15], 0, 0), 0, false)
        .unwrap_err();
    signal(&login, false);
    assert_eq!(source.next().err(), Some(Lost::InvalidFrame));
}
#[test]
#[ignore = "new source boundary: primary and independent review before execution"]
fn route_overrun_malformed_header_attributes_and_invalid_after_valid_never_emit() {
    for bytes in [
        nl(4, &[], 0, 0),
        nl(16, &[0; 15], 0, 0),
        vec![0; 15],
        nl(16, &[0; 20], 0, 0),
    ] {
        assert!(route::synthetic(&bytes, 0, false).is_err());
    }
    let mut both = nl(16, &[0; 16], 900, 17);
    both.extend(nl(16, &[0; 15], 0, 0));
    assert_eq!(route::synthetic(&both, 0, false), Err(Lost::InvalidFrame));
    let mut burst = Vec::new();
    for _ in 0..20 {
        burst.extend(nl(16, &[0; 16], 0, 0));
    }
    assert_eq!(route::synthetic(&burst, 0, false), Ok(1)); // every header validated, one hint.
}
