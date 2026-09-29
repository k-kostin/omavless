// SPDX-License-Identifier: MIT
use super::*;
use std::collections::VecDeque;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;

#[test]
fn redirection_and_wire_debug_refuse_without_mutating_environment() {
    use std::ffi::OsStr;
    assert!(validate_ambient(None, None).is_ok());
    assert!(
        validate_ambient(
            Some(OsStr::new("unix:path=/run/dbus/system_bus_socket")),
            None
        )
        .is_ok()
    );
    for value in [
        "",
        "tcp:host=localhost",
        "unix:path=/tmp/fake",
        "unix:path=/run/dbus/system_bus_socket;unix:path=/tmp/fake",
    ] {
        assert!(validate_ambient(Some(OsStr::new(value)), None).is_err());
    }
    for value in ["", "all", "message"] {
        assert!(validate_ambient(None, Some(OsStr::new(value))).is_err());
    }
}

struct Fake {
    pids: VecDeque<Result<u32, Error>>,
    starts: VecDeque<u64>,
    peer: (u32, u32),
    alive: VecDeque<Result<(), Error>>,
    checks: VecDeque<Result<(), Error>>,
    calls: Vec<&'static str>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            pids: [Ok(42), Ok(42)].into(),
            starts: [100, 100].into(),
            peer: (42, 1000),
            alive: [Ok(()), Ok(())].into(),
            checks: [Ok(()), Ok(())].into(),
            calls: vec![],
        }
    }
}
impl Facts for Fake {
    fn main_pid(&mut self) -> Result<u32, Error> {
        self.calls.push("scalar");
        self.pids.pop_front().unwrap()
    }
    fn lifetime(&mut self, pid: u32) -> Result<Lifetime, Error> {
        self.calls.push("lifetime");
        Ok(Lifetime {
            pid,
            start: self.starts.pop_front().unwrap(),
        })
    }
    fn private_peer(&mut self) -> Result<(u32, u32), Error> {
        self.calls.push("peer");
        Ok(self.peer)
    }
    fn peer_alive_now(&mut self) -> Result<(), Error> {
        self.calls.push("pidfd");
        self.alive.pop_front().unwrap()
    }
    fn revalidate_endpoints(&mut self) -> Result<(), Error> {
        self.calls.push("endpoint");
        self.checks.pop_front().unwrap()
    }
}

#[test]
fn observes_only_scalar_peer_lifetimes_and_endpoints() {
    let mut fake = Fake::default();
    assert_eq!(observe(&mut fake, 1000), Ok(()));
    assert_eq!(
        fake.calls,
        [
            "scalar", "lifetime", "peer", "pidfd", "endpoint", "scalar", "lifetime", "endpoint",
            "pidfd"
        ]
    );
}

#[test]
fn missing_zero_malformed_and_unavailable_scalar_refuse() {
    for reply in [
        ().to_variant(),
        (0_u32.to_variant(),).to_variant(),
        (u32::MAX.to_variant(),).to_variant(),
        ("42".to_variant(),).to_variant(),
        ("secret-shaped".repeat(100).to_variant(),).to_variant(),
        (42_u32,).to_variant(),
    ] {
        assert_eq!(scalar_pid(&reply), Err(Error::IdentityUnverified));
    }
    assert_eq!(scalar_pid(&(42_u32.to_variant(),).to_variant()), Ok(42));
    for error in [Error::ManagerUnavailable, Error::IdentityUnverified] {
        let mut fake = Fake {
            pids: [Err(error)].into(),
            ..Fake::default()
        };
        assert_eq!(observe(&mut fake, 1000), Err(error));
        assert_eq!(fake.calls, ["scalar"]);
    }
    let mut fake = Fake {
        pids: [Ok(0)].into(),
        ..Fake::default()
    };
    assert_eq!(observe(&mut fake, 1000), Err(Error::IdentityUnverified));
}

#[test]
fn counterfeit_peer_restarts_and_endpoint_changes_refuse() {
    for peer in [(43, 1000), (42, 1001), (0, 1000)] {
        let mut fake = Fake {
            peer,
            ..Fake::default()
        };
        assert_eq!(observe(&mut fake, 1000), Err(Error::IdentityUnverified));
    }
    let mut fake = Fake {
        pids: [Ok(42), Ok(43)].into(),
        ..Fake::default()
    };
    assert_eq!(observe(&mut fake, 1000), Err(Error::OwnerChanged));
    let mut fake = Fake {
        starts: [100, 101].into(),
        ..Fake::default()
    };
    assert_eq!(observe(&mut fake, 1000), Err(Error::OwnerChanged));
    for checks in [
        [Err(Error::OwnerChanged), Ok(())],
        [Ok(()), Err(Error::OwnerChanged)],
    ] {
        let mut fake = Fake {
            checks: checks.into(),
            ..Fake::default()
        };
        assert_eq!(observe(&mut fake, 1000), Err(Error::OwnerChanged));
    }
    for alive in [
        [Err(Error::IdentityUnverified), Ok(())],
        [Ok(()), Err(Error::IdentityUnverified)],
    ] {
        let mut fake = Fake {
            alive: alive.into(),
            ..Fake::default()
        };
        assert_eq!(observe(&mut fake, 1000), Err(Error::IdentityUnverified));
    }
}

#[test]
fn held_endpoint_rejects_symlinks_wrong_owners_and_replacements() {
    let directory =
        std::env::temp_dir().join(format!("omavless-s1-manager-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let held = File::from(open(&directory, local_bus::directory_flags(), Mode::empty()).unwrap());
    let uid = nix::unistd::geteuid().as_raw();
    let listener = UnixListener::bind(directory.join("private")).unwrap();
    let pinned = socket_file(&held, "private", uid).unwrap();
    assert!(socket_file(&held, "private", uid.wrapping_add(1)).is_err());
    symlink("private", directory.join("linked")).unwrap();
    assert!(socket_file(&held, "linked", uid).is_err());
    symlink(".", directory.join("linked-dir")).unwrap();
    assert!(child_directory(&held, "linked-dir", uid).is_err());
    std::fs::create_dir(directory.join("unsafe")).unwrap();
    std::fs::set_permissions(
        directory.join("unsafe"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(child_directory(&held, "unsafe", uid).is_err());
    std::fs::rename(directory.join("private"), directory.join("old")).unwrap();
    let replacement = UnixListener::bind(directory.join("private")).unwrap();
    assert!(
        inode(&pinned).unwrap() != inode(&socket_file(&held, "private", uid).unwrap()).unwrap()
    );
    let socket = local_bus::connect_endpoint(&pinned).unwrap();
    let (_accepted, _) = listener.accept().unwrap();
    let credentials = socket.credentials().unwrap();
    assert_eq!(credentials.unix_pid().unwrap(), std::process::id() as i32);
    assert_eq!(credentials.unix_user().unwrap(), uid);
    let pin = UnverifiedManagerPin::capture(&socket).unwrap();
    assert_eq!(pin.require_alive_now(), Ok(()));
    assert!(UnverifiedManagerPin::capture(&listener).is_err());
    // This is listener identity only, not proof of who serves future bytes.
    drop(socket);
    drop(replacement);
    drop(listener);
    std::fs::remove_dir_all(directory).unwrap();
}
