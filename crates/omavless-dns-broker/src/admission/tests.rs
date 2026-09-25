use super::*;
use std::{
    fs as stdfs,
    os::unix::{
        fs::{FileTypeExt, PermissionsExt, symlink},
        net::UnixDatagram,
    },
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};
use tempfile::TempDir;

struct State {
    pid: u32,
    access: String,
    capacity: u32,
    preserve: String,
    directory: String,
}

#[test]
fn expired_operation_never_polls_bus_or_notifies() {
    let polled = Cell::new(false);
    assert!(
        complete_until(Instant::now(), async {
            polled.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!polled.get());
    let fixture = Fixture::new();
    let context = fixture.context().unwrap();
    assert_eq!(
        context.set_deadline(Instant::now() + Duration::from_secs(31)),
        Err(Error::Refused)
    );
    context.set_deadline(Instant::now()).unwrap();
    assert_eq!(context.recheck(), Err(Error::BusUnavailable));
    assert_eq!(context.notify_ready(), Err(Error::BusUnavailable));
    assert_eq!(
        fixture.notify.recv(&mut [0_u8; 64]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
impl Default for State {
    fn default() -> Self {
        Self {
            pid: std::process::id(),
            access: "main".into(),
            capacity: 1,
            preserve: "yes".into(),
            directory: "yes".into(),
        }
    }
}
struct Manager(Arc<Mutex<State>>);
#[zbus::interface(name = "org.freedesktop.systemd1.Service")]
impl Manager {
    #[zbus(property, name = "MainPID")]
    fn main_pid(&self) -> u32 {
        self.0.lock().unwrap().pid
    }
    #[zbus(property)]
    fn notify_access(&self) -> String {
        self.0.lock().unwrap().access.clone()
    }
    #[zbus(property)]
    fn file_descriptor_store_max(&self) -> u32 {
        self.0.lock().unwrap().capacity
    }
    #[zbus(property)]
    fn file_descriptor_store_preserve(&self) -> String {
        self.0.lock().unwrap().preserve.clone()
    }
    #[zbus(property)]
    fn runtime_directory_preserve(&self) -> String {
        self.0.lock().unwrap().directory.clone()
    }
}

struct Bus {
    root: TempDir,
    child: Child,
}
impl Bus {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("omavless-admission-")
            .tempdir()
            .unwrap();
        let address = format!("unix:path={}", root.path().join("bus").display());
        let child = Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--nopidfile",
                "--address",
                &address,
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("private dbus-daemon fixture required");
        let mut fixture = Self { root, child };
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if stdfs::symlink_metadata(fixture.root.path().join("bus"))
                .is_ok_and(|m| m.file_type().is_socket())
            {
                return fixture;
            }
            assert!(
                fixture.child.try_wait().unwrap().is_none(),
                "private bus stopped"
            );
            assert!(Instant::now() < deadline, "private bus startup deadline");
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn address(&self) -> String {
        format!("unix:path={}", self.root.path().join("bus").display())
    }
    fn connection(&self) -> Connection {
        zbus::blocking::connection::Builder::address(self.address().as_str())
            .unwrap()
            .max_queued(8)
            .build()
            .unwrap()
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Fixture {
    state: Arc<Mutex<State>>,
    manager: Option<Connection>,
    resolved: Option<Connection>,
    notify: UnixDatagram,
    bus: Bus,
}
impl Fixture {
    fn new() -> Self {
        let bus = Bus::new();
        let state = Arc::new(Mutex::new(State::default()));
        let manager = zbus::blocking::connection::Builder::address(bus.address().as_str())
            .unwrap()
            .serve_at(UNIT, Manager(state.clone()))
            .unwrap()
            .name(MANAGER)
            .unwrap()
            .build()
            .unwrap();
        let resolved = zbus::blocking::connection::Builder::address(bus.address().as_str())
            .unwrap()
            .name(RESOLVED)
            .unwrap()
            .build()
            .unwrap();
        let notify = UnixDatagram::bind(bus.root.path().join("notify")).unwrap();
        notify.set_nonblocking(true).unwrap();
        Self {
            state,
            manager: Some(manager),
            resolved: Some(resolved),
            notify,
            bus,
        }
    }
    fn context(&self) -> Result<RootContext, Error> {
        RootContext::authenticate(
            self.bus.connection(),
            connect_notify(&self.bus.root.path().join("notify"))?,
            1000,
            rustix::process::geteuid().as_raw(),
            Duration::from_secs(2),
        )
    }
}

fn enrollment(root: &TempDir, value: &[u8]) -> std::path::PathBuf {
    let path = root.path().join("enrollment.json");
    stdfs::write(&path, value).unwrap();
    stdfs::set_permissions(&path, stdfs::Permissions::from_mode(0o600)).unwrap();
    path
}

#[test]
fn fixed_service_properties_and_pinned_owners_admit_without_notification() {
    let fixture = Fixture::new();
    let context = fixture.context().unwrap();
    context.recheck().unwrap();
    assert_eq!(context.enrolled_uid(), 1000);
    assert_eq!(
        context.manager_owner(),
        fixture
            .manager
            .as_ref()
            .unwrap()
            .unique_name()
            .unwrap()
            .as_str()
    );
    assert_eq!(
        context.resolved_owner(),
        fixture
            .resolved
            .as_ref()
            .unwrap()
            .unique_name()
            .unwrap()
            .as_str()
    );
    assert!(context.connection().unique_name().is_some());
    assert!(context.duplicate_notify().is_ok());
    let mut byte = [0; 1];
    assert_eq!(
        fixture.notify.recv(&mut byte).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn every_required_service_property_is_enforced() {
    for field in 0..5 {
        let fixture = Fixture::new();
        {
            let mut state = fixture.state.lock().unwrap();
            match field {
                0 => state.pid += 1,
                1 => state.access = "all".into(),
                2 => state.capacity = 2,
                3 => state.preserve = "restart".into(),
                _ => state.directory = "no".into(),
            }
        }
        assert_eq!(fixture.context().unwrap_err(), Error::ServiceMismatch);
    }
}

#[test]
fn manager_uid_is_checked_but_resolved_is_not_required_to_be_root() {
    let fixture = Fixture::new();
    let notify = connect_notify(&fixture.bus.root.path().join("notify")).unwrap();
    assert_eq!(
        RootContext::authenticate(
            fixture.bus.connection(),
            notify,
            1000,
            rustix::process::geteuid().as_raw().wrapping_add(1),
            Duration::from_secs(2)
        )
        .unwrap_err(),
        Error::ServiceMismatch
    );
    // Ordinary test user owns both mock names; production separately trusts the
    // root-administered system bus to restrict resolve1 name ownership.
    fixture.context().unwrap();
}

#[test]
fn resolved_owner_replacement_refuses_instead_of_rebinding() {
    let mut fixture = Fixture::new();
    let context = fixture.context().unwrap();
    fixture.resolved.take().unwrap().close().unwrap();
    let replacement = zbus::blocking::connection::Builder::address(fixture.bus.address().as_str())
        .unwrap()
        .name(RESOLVED)
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(
        replacement.unique_name().unwrap().as_str(),
        context.resolved_owner()
    );
    assert_eq!(context.recheck(), Err(Error::ServiceMismatch));
}

#[test]
fn manager_owner_replacement_refuses_instead_of_rebinding() {
    let mut fixture = Fixture::new();
    let context = fixture.context().unwrap();
    fixture.manager.take().unwrap().close().unwrap();
    let replacement = zbus::blocking::connection::Builder::address(fixture.bus.address().as_str())
        .unwrap()
        .serve_at(UNIT, Manager(fixture.state.clone()))
        .unwrap()
        .name(MANAGER)
        .unwrap()
        .build()
        .unwrap();
    assert_ne!(
        replacement.unique_name().unwrap().as_str(),
        context.manager_owner()
    );
    assert_eq!(context.recheck(), Err(Error::ServiceMismatch));
}

#[test]
fn enrollment_is_exact_versioned_bounded_and_credential_free() {
    let root = tempfile::tempdir().unwrap();
    let uid = rustix::process::geteuid().as_raw();
    let valid = br#"{"schema":1,"uid":1000,"policy":"meta-ipv4-v1"}"#;
    assert_eq!(read_enrollment(&enrollment(&root, valid), uid), Ok(1000));
    for invalid in [
        r#"{"schema":2,"uid":1000,"policy":"meta-ipv4-v1"}"#.to_string(),
        r#"{"schema":1,"uid":0,"policy":"meta-ipv4-v1"}"#.into(),
        r#"{"schema":1,"uid":4294967295,"policy":"meta-ipv4-v1"}"#.into(),
        r#"{"schema":1,"uid":1000,"policy":"arbitrary"}"#.into(),
        r#"{"schema":1,"uid":1000,"uid":1001,"policy":"meta-ipv4-v1"}"#.into(),
        r#"{"schema":1,"uid":1000,"policy":"meta-ipv4-v1","command":"private"}"#.into(),
        r#"{"schema":1,"uid":true,"policy":"meta-ipv4-v1"}"#.into(),
        "[]".into(),
        "{}".into(),
        "x".repeat(257),
        String::from_utf8(valid.to_vec()).unwrap() + " {}",
    ] {
        assert_eq!(
            read_enrollment(&enrollment(&root, invalid.as_bytes()), uid),
            Err(Error::InvalidEnrollment)
        );
    }
    assert_eq!(
        read_enrollment(&enrollment(&root, b"\xff"), uid),
        Err(Error::InvalidEnrollment)
    );
}

#[test]
fn enrollment_owner_mode_symlink_hardlink_and_nonregular_refuse() {
    let root = tempfile::tempdir().unwrap();
    let owner = rustix::process::geteuid().as_raw();
    let path = enrollment(&root, br#"{"schema":1,"uid":1000,"policy":"meta-ipv4-v1"}"#);
    assert_eq!(
        read_enrollment(&path, owner.wrapping_add(1)),
        Err(Error::InvalidEnrollment)
    );
    stdfs::set_permissions(&path, stdfs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(read_enrollment(&path, owner), Err(Error::InvalidEnrollment));
    stdfs::set_permissions(&path, stdfs::Permissions::from_mode(0o600)).unwrap();
    let alias = root.path().join("alias");
    symlink(&path, &alias).unwrap();
    assert_eq!(
        read_enrollment(&alias, owner),
        Err(Error::InvalidEnrollment)
    );
    stdfs::hard_link(&path, root.path().join("hardlink")).unwrap();
    assert_eq!(read_enrollment(&path, owner), Err(Error::InvalidEnrollment));
    assert_eq!(
        read_enrollment(root.path(), owner),
        Err(Error::InvalidEnrollment)
    );
    let fifo = root.path().join("fifo");
    fs::mknodat(
        fs::CWD,
        &fifo,
        fs::FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        0,
    )
    .unwrap();
    assert_eq!(read_enrollment(&fifo, owner), Err(Error::InvalidEnrollment));
}

#[test]
fn protected_paths_refuse_wrong_owner_writable_or_symlink_targets() {
    let root = tempfile::tempdir().unwrap();
    let owner = rustix::process::geteuid().as_raw();
    protected_directory(root.path(), owner).unwrap();
    assert_eq!(
        protected_directory(root.path(), owner.wrapping_add(1)),
        Err(Error::Refused)
    );
    let alias = root.path().join("alias");
    symlink(root.path(), &alias).unwrap();
    assert_eq!(protected_directory(&alias, owner), Err(Error::Refused));
    stdfs::set_permissions(root.path(), stdfs::Permissions::from_mode(0o770)).unwrap();
    assert_eq!(protected_directory(root.path(), owner), Err(Error::Refused));
    let notify = root.path().join("notify");
    let _socket = UnixDatagram::bind(&notify).unwrap();
    protected_socket(&notify, owner).unwrap();
    assert_eq!(
        protected_socket(&notify, owner.wrapping_add(1)),
        Err(Error::Refused)
    );
    let socket_alias = root.path().join("socket-alias");
    symlink(&notify, &socket_alias).unwrap();
    assert_eq!(protected_socket(&socket_alias, owner), Err(Error::Refused));
    assert_eq!(protected_socket(root.path(), owner), Err(Error::Refused));
}

#[test]
fn initial_root_guard_and_public_errors_are_safe() {
    assert_eq!(require_root(0), Ok(()));
    for uid in [1, 1000, u32::MAX] {
        assert_eq!(require_root(uid), Err(Error::Refused));
    }
    let fixture = Fixture::new();
    let context = fixture.context().unwrap();
    assert_eq!(format!("{context:?}"), "RootContext { .. }");
    for error in [
        Error::Refused,
        Error::InvalidEnrollment,
        Error::BusUnavailable,
        Error::InvalidReply,
        Error::ServiceMismatch,
    ] {
        assert!(error.to_string().len() < 80);
        assert!(error.to_string().is_ascii());
        assert!(!error.to_string().contains('/'));
    }
}

#[test]
fn reply_sender_signature_size_and_descriptors_are_checked() {
    let call = Message::method_call(DBUS_PATH, "GetNameOwner")
        .unwrap()
        .build(&(MANAGER,))
        .unwrap();
    let reply = Message::method_return(&call.header())
        .unwrap()
        .sender(":8.9")
        .unwrap()
        .build(&true)
        .unwrap();
    assert_eq!(sender(&reply, DBUS), Err(Error::InvalidReply));
    assert_eq!(bounded::<String>(&reply), Err(Error::InvalidReply));
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
    assert_eq!(bounded::<String>(&with_fd), Err(Error::InvalidReply));
}
