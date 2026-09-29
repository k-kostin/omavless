// SPDX-License-Identifier: MIT

use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let name = format!(
            "omavless-s1-bus-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn directory(&self) -> File {
        File::from(open(&self.0, directory_flags(), Mode::empty()).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn pins_socket_inode_before_connection_and_checks_kernel_peer() {
    let fixture = Fixture::new();
    let path = fixture.0.join("bus");
    let original = UnixListener::bind(&path).unwrap();
    original.set_nonblocking(true).unwrap();
    let uid = nix::unistd::geteuid().as_raw();
    let pinned = pin_endpoint(&fixture.directory(), uid).unwrap();
    std::fs::rename(&path, fixture.0.join("old-bus")).unwrap();
    let replacement = UnixListener::bind(&path).unwrap();
    replacement.set_nonblocking(true).unwrap();
    let socket = connect_endpoint(&pinned).unwrap();
    assert!(original.accept().is_ok());
    assert!(replacement.accept().is_err());
    let (pid, start) = peer_identity(&socket, uid).unwrap();
    assert_eq!(pid, std::process::id());
    assert!(start > 0);
    assert_eq!(
        peer_identity(&socket, uid.wrapping_add(1)).err(),
        Some(Error::IdentityUnverified)
    );
}

#[test]
fn unsafe_endpoint_and_directory_refuse() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    let uid = nix::unistd::geteuid().as_raw();
    validate_directory(&directory, uid, true).unwrap();
    std::fs::write(fixture.0.join("bus"), b"not socket").unwrap();
    assert_eq!(
        pin_endpoint(&directory, uid).err(),
        Some(Error::IdentityUnverified)
    );
    std::fs::remove_file(fixture.0.join("bus")).unwrap();
    let _listener = UnixListener::bind(fixture.0.join("actual")).unwrap();
    symlink("actual", fixture.0.join("bus")).unwrap();
    assert_eq!(
        pin_endpoint(&directory, uid).err(),
        Some(Error::IdentityUnverified)
    );
    std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        validate_directory(&directory, uid, true),
        Err(Error::IdentityUnverified)
    );
}

#[test]
fn ambient_bus_redirection_refuses_without_environment_mutation() {
    use std::ffi::OsStr;
    let runtime = "/run/user/1234";
    let xdg = Some(OsStr::new(runtime));
    assert!(validate_environment(runtime, xdg, None).is_ok());
    assert!(
        validate_environment(
            runtime,
            xdg,
            Some(OsStr::new("unix:path=/run/user/1234/bus"))
        )
        .is_ok()
    );
    for address in [
        "",
        "autolaunch:",
        "unix:path=/tmp/bus",
        "unix:abstract=fixture",
        "tcp:host=localhost,port=123",
        "unix:path=/run/user/1234/bus;unix:path=/tmp/fallback",
    ] {
        assert_eq!(
            validate_environment(runtime, xdg, Some(OsStr::new(address))),
            Err(Error::IdentityUnverified)
        );
    }
    assert_eq!(
        validate_environment(runtime, None, None),
        Err(Error::IdentityUnverified)
    );
    assert_eq!(
        validate_environment(runtime, Some(OsStr::new("/tmp/other")), None),
        Err(Error::IdentityUnverified)
    );
}

fn credentials(entries: &[(&str, glib::Variant)]) -> glib::Variant {
    let entries: Vec<_> = entries
        .iter()
        .map(|(name, value)| {
            glib::Variant::from_dict_entry(&name.to_variant(), &value.to_variant())
        })
        .collect();
    let dictionary = glib::Variant::array_from_iter_with_type(
        glib::VariantTy::new("{sv}").unwrap(),
        entries.iter(),
    );
    glib::Variant::tuple_from_iter([dictionary])
}

#[test]
fn typed_credentials_reject_foreign_missing_wrong_duplicate_and_oversized() {
    let valid = [
        ("UnixUserID", 1234_u32.to_variant()),
        ("ProcessID", 5678_u32.to_variant()),
    ];
    assert_eq!(parse_credentials(&credentials(&valid), 1234), Ok(5678));
    assert_eq!(
        parse_credentials(&credentials(&valid), 1235),
        Err(Error::IdentityUnverified)
    );
    for entries in [
        vec![],
        vec![valid[0].clone()],
        vec![valid[1].clone()],
        vec![valid[0].clone(), ("ProcessID", 0_u32.to_variant())],
        vec![valid[0].clone(), ("ProcessID", "5678".to_variant())],
        vec![valid[0].clone(), valid[0].clone(), valid[1].clone()],
        vec![
            valid[0].clone(),
            valid[1].clone(),
            ("private", "secret-shaped".repeat(1000).to_variant()),
        ],
    ] {
        assert_eq!(
            parse_credentials(&credentials(&entries), 1234),
            Err(Error::IdentityUnverified)
        );
    }
    assert_eq!(
        parse_credentials(&("wrong type",).to_variant(), 1234),
        Err(Error::IdentityUnverified)
    );
}

#[test]
fn proc_starttime_handles_parentheses_and_rejects_partial_wrong_or_zero() {
    let mut fields = vec!["S"; 20];
    fields[19] = "54321";
    let stat = format!("123 (test ) tricky name) {}", fields.join(" "));
    assert_eq!(parse_start(stat.as_bytes(), 123), Ok(54321));
    assert_eq!(
        parse_start(stat.as_bytes(), 124),
        Err(Error::IdentityUnverified)
    );
    assert_eq!(
        parse_start(b"123 (partial) S", 123),
        Err(Error::IdentityUnverified)
    );
    fields[19] = "0";
    assert_eq!(
        parse_start(format!("123 (zero) {}", fields.join(" ")).as_bytes(), 123),
        Err(Error::IdentityUnverified)
    );
}

#[test]
fn fake_dbus_identity_changes_and_failures_never_establish_authority() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (server_stream, client_stream) = std::os::unix::net::UnixStream::pair().unwrap();
    let state = Arc::new(AtomicUsize::new(0));
    let server_state = state.clone();
    let (stop, stopped) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let context = glib::MainContext::new();
        context.with_thread_default(|| {
            let socket = gio::Socket::from_fd(server_stream.into()).unwrap();
            let stream = socket.connection_factory_create_connection();
            let bus = gio::DBusConnection::new_sync(
                &stream, Some(&gio::dbus_generate_guid()),
                gio::DBusConnectionFlags::AUTHENTICATION_SERVER | gio::DBusConnectionFlags::DELAY_MESSAGE_PROCESSING,
                None::<&gio::DBusAuthObserver>, None::<&gio::Cancellable>,
            ).unwrap();
            bus.set_exit_on_close(false);
            let xml = "<node><interface name='org.freedesktop.DBus'><method name='Hello'><arg type='s' direction='out'/></method><method name='GetId'><arg type='s' direction='out'/></method><method name='GetNameOwner'><arg type='s' direction='in'/><arg type='s' direction='out'/></method><method name='GetConnectionCredentials'><arg type='s' direction='in'/><arg type='a{sv}' direction='out'/></method></interface></node>";
            let interface = gio::DBusNodeInfo::for_xml(xml).unwrap().lookup_interface(DBUS).unwrap();
            let _registration = bus.register_object(DBUS_PATH, &interface).method_call(move |_, _, _, _, method, _, invocation| {
                if method != "Hello" { assert!(invocation.message().flags().contains(gio::DBusMessageFlags::NO_AUTO_START)); }
                let mode = server_state.load(Ordering::SeqCst);
                if mode == 4 {
                    invocation.return_dbus_error("org.freedesktop.DBus.Error.Failed", "synthetic private failure");
                    return;
                }
                let reply = match method {
                    "Hello" => (":1.1",).to_variant(),
                    "GetId" => (if mode == 1 { "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" } else { "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },).to_variant(),
                    "GetNameOwner" => (if mode == 2 { ":1.23" } else { ":1.22" },).to_variant(),
                    "GetConnectionCredentials" => credentials(&[
                        ("UnixUserID", if mode == 3 { nix::unistd::geteuid().as_raw().wrapping_add(1) } else { nix::unistd::geteuid().as_raw() }.to_variant()),
                        ("ProcessID", std::process::id().to_variant()),
                    ]),
                    _ => panic!("unexpected method"),
                };
                invocation.return_value(Some(&reply));
            }).build().unwrap();
            bus.start_message_processing();
            while stopped.try_recv().is_err() {
                while context.pending() { context.iteration(false); }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let _ = bus.close_sync(None::<&gio::Cancellable>);
        }).unwrap();
    });
    let socket = gio::Socket::from_fd(client_stream.into()).unwrap();
    let bus = authenticate(&socket).unwrap();
    let uid = nix::unistd::geteuid().as_raw();
    let first = capture(&bus, &socket, uid).unwrap();
    assert!(capture(&bus, &socket, uid).unwrap() == first);
    for mode in [1, 2] {
        state.store(mode, Ordering::SeqCst);
        assert!(capture(&bus, &socket, uid).unwrap() != first);
    }
    state.store(3, Ordering::SeqCst);
    assert!(matches!(
        capture(&bus, &socket, uid),
        Err(Error::IdentityUnverified)
    ));
    state.store(4, Ordering::SeqCst);
    assert!(matches!(
        capture(&bus, &socket, uid),
        Err(Error::ManagerUnavailable)
    ));
    stop.send(()).unwrap();
    server.join().unwrap();
    assert!(capture(&bus, &socket, uid).is_err());
    // Even the deliberately spoofed same-UID manager above never creates an
    // authority object. Successful private frames still refuse all writes.
    assert_eq!(
        crate::tests::synthetic_observation().admit_writes(),
        Err(Error::IdentityUnverified)
    );
}

#[test]
fn silent_authentication_peer_is_cancelled() {
    let (_silent, client) = std::os::unix::net::UnixStream::pair().unwrap();
    let socket = gio::Socket::from_fd(client.into()).unwrap();
    let start = std::time::Instant::now();
    assert!(matches!(
        authenticate(&socket),
        Err(Error::ManagerUnavailable)
    ));
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}
