// SPDX-License-Identifier: MIT

//! Opt-in installed-dconf experiment. Every writer is a separately spawned
//! process with a private bus, profile and config/runtime directories. Never
//! construct a default backend in the parent test process.

use super::*;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const CHILD_TEST: &str = "gio_host::persistent_tests::private_dconf_child";
const ROOT_ENV: &str = "OMAVLESS_TEST_DCONF_ROOT";
const ROLE_ENV: &str = "OMAVLESS_TEST_DCONF_ROLE";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct OwnedChild(Child);

impl OwnedChild {
    fn finish(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().expect("child status") {
                assert!(status.success(), "isolated child failed");
                return;
            }
            assert!(Instant::now() < deadline, "isolated child timeout");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn signal(&mut self, signal: Signal) {
        let pid = Pid::from_raw(self.0.id().try_into().unwrap());
        kill(pid, signal).unwrap();
        if signal == Signal::SIGSTOP {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match waitpid(pid, Some(WaitPidFlag::WUNTRACED | WaitPidFlag::WNOHANG)).unwrap() {
                    WaitStatus::Stopped(_, Signal::SIGSTOP) => break,
                    WaitStatus::StillAlive => {}
                    _ => panic!("owned service failed before stop"),
                }
                assert!(Instant::now() < deadline, "owned service stop timeout");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // These handles refer only to fixture-spawned children, never discovery
        // by name/PID. SIGKILL also reaps an intentionally stopped test service.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    root: PathBuf,
    bus: Option<OwnedChild>,
    service: Option<OwnedChild>,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "s1-dconf-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(root.as_os_str().len() < 80, "use a short private TMPDIR");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let mut fixture = Self {
            root,
            bus: None,
            service: None,
        };
        for name in ["config", "runtime", "cache", "data"] {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(fixture.root.join(name))
                .unwrap();
        }
        private_file(&fixture.root.join("profile"), b"user-db:user\n");
        // No standard config include or service directory: the private bus
        // cannot auto-start a desktop/systemd service from the real session.
        let address_xml = fixture.address().replace('&', "&amp;").replace('<', "&lt;");
        private_file(&fixture.root.join("bus.conf"), format!(r#"<busconfig>
<type>session</type><listen>{address_xml}</listen><auth>EXTERNAL</auth>
<policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy>
</busconfig>"#).as_bytes());
        let mut daemon = fixture.command("/usr/bin/dbus-daemon");
        daemon.arg("--nofork").arg("--nopidfile").arg(format!(
            "--config-file={}",
            fixture.root.join("bus.conf").display()
        ));
        fixture.bus = Some(OwnedChild(daemon.spawn().expect("private bus dependency")));
        fixture.wait_for(|| fixture.root.join("bus").exists());
        fixture.service = Some(OwnedChild(
            fixture
                .command("/usr/lib/dconf-service")
                .spawn()
                .expect("installed dconf service dependency"),
        ));
        let socket = gio::Socket::new(
            gio::SocketFamily::Unix,
            gio::SocketType::Stream,
            gio::SocketProtocol::Default,
        )
        .unwrap();
        socket.set_timeout(2);
        SocketExt::connect(
            &socket,
            &gio::UnixSocketAddress::new(&fixture.root.join("bus")),
            None::<&gio::Cancellable>,
        )
        .unwrap();
        let bus = crate::local_bus::authenticate(&socket).unwrap();
        bus.set_exit_on_close(false);
        fixture.wait_for(|| {
            bus.call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                Some(&("ca.desrt.dconf",).to_variant()),
                None,
                gio::DBusCallFlags::NO_AUTO_START,
                500,
                None::<&gio::Cancellable>,
            )
            .ok()
            .and_then(|reply| reply.get::<(bool,)>())
            .is_some_and(|(owned,)| owned)
        });
        // Do not flush: only the fixture's read-only readiness connection.
        drop(bus);
        fixture
    }

    fn address(&self) -> String {
        format!("unix:path={}", self.root.join("bus").display())
    }

    fn command(&self, executable: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(executable);
        command
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_CACHE_HOME", self.root.join("cache"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("DCONF_PROFILE", self.root.join("profile"))
            .env("GSETTINGS_BACKEND", "dconf")
            .env("DBUS_SESSION_BUS_ADDRESS", self.address())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    fn child(&self, role: &str) -> OwnedChild {
        let mut command = self.command(std::env::current_exe().unwrap());
        command
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env(ROOT_ENV, &self.root)
            .env(ROLE_ENV, role);
        OwnedChild(command.spawn().unwrap())
    }

    fn run(&self, role: &str) {
        self.child(role).finish();
    }

    fn wait_for(&self, predicate: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !predicate() {
            assert!(
                Instant::now() < deadline,
                "private fixture readiness timeout"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.service.take());
        drop(self.bus.take());
        // Exact exclusively-created fixture root; no inherited user paths.
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn private_file(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

fn snapshot_bytes(schemas: &[gio::SettingsSchema]) -> Vec<u8> {
    read_desktop(schemas)
        .unwrap()
        .encode()
        .unwrap()
        .bytes()
        .unwrap()
        .to_vec()
}

fn setting(schemas: &[gio::SettingsSchema], key: DesktopKey) -> gio::Settings {
    let (schema, _, _) = key.schema_key_type();
    gio::Settings::new_full(
        schemas.iter().find(|s| s.id() == schema).unwrap(),
        None::<&gio::SettingsBackend>,
        None,
    )
}

fn variant(value: &DesktopValue) -> glib::Variant {
    match value {
        DesktopValue::String(value) => value.to_variant(),
        DesktopValue::Bool(value) => value.to_variant(),
        DesktopValue::Int(value) => value.to_variant(),
        DesktopValue::Strings(value) => value.to_variant(),
    }
}

#[test]
fn private_dconf_child() {
    let Some(root) = std::env::var_os(ROOT_ENV) else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(root.is_absolute() && root.is_dir());
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("s1-dconf-")
    );
    let meta = fs::symlink_metadata(&root).unwrap();
    assert!(
        meta.is_dir()
            && meta.uid() == nix::unistd::geteuid().as_raw()
            && meta.mode() & 0o777 == 0o700
    );
    assert_eq!(fs::read(root.join("profile")).unwrap(), b"user-db:user\n");
    for (name, suffix) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_RUNTIME_DIR", "runtime"),
        ("DCONF_PROFILE", "profile"),
    ] {
        assert_eq!(
            std::env::var_os(name).as_deref(),
            Some(root.join(suffix).as_os_str())
        );
    }
    assert_eq!(
        std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        format!("unix:path={}", root.join("bus").display())
    );
    let source = gio::SettingsSchemaSource::default().unwrap();
    let schemas = validate_schemas(&source).unwrap();
    match std::env::var(ROLE_ENV).unwrap().as_str() {
        "baseline" => private_file(&root.join("baseline"), &snapshot_bytes(&schemas)),
        "seed" => {
            for (i, key) in DesktopKey::ALL.into_iter().enumerate() {
                if i % 3 == 0 {
                    continue;
                }
                let s = setting(&schemas, key);
                let (_, name, signature) = key.schema_key_type();
                let value = if i % 3 == 1 && signature == "s" && key != DesktopKey::Mode {
                    "".to_variant()
                } else {
                    s.default_value(name).unwrap()
                };
                s.set_value(name, &value).unwrap();
            }
            gio::Settings::sync();
        }
        "capture" => private_file(&root.join("original"), &snapshot_bytes(&schemas)),
        "apply" => {
            for key in DesktopKey::ALL {
                let s = setting(&schemas, key);
                let (_, name, signature) = key.schema_key_type();
                let value = match signature {
                    "s" if key == DesktopKey::Mode => "manual".to_variant(),
                    "s" => "synthetic.invalid".to_variant(),
                    "b" => (!s.boolean(name)).to_variant(),
                    "i" => 32123_i32.to_variant(),
                    "as" => vec!["synthetic.invalid"].to_variant(),
                    _ => unreachable!(),
                };
                s.set_value(name, &value).unwrap();
            }
            gio::Settings::sync();
        }
        "restore" => {
            let bytes = fs::read(root.join("original")).unwrap();
            let original = DesktopSnapshot::decode(
                &omavless_runtime::app_proxy::Snapshot::new(Some(bytes)).unwrap(),
            )
            .unwrap();
            for entry in original.entries().iter().rev() {
                let s = setting(&schemas, entry.key);
                let (_, name, _) = entry.key.schema_key_type();
                match &entry.user {
                    Override::Absent => s.reset(name),
                    Override::Present(value) => s.set_value(name, &variant(value)).unwrap(),
                }
            }
            gio::Settings::sync();
        }
        "assert-original" => assert!(
            snapshot_bytes(&schemas) == fs::read(root.join("original")).unwrap(),
            "exact layered restore"
        ),
        "assert-changed" => assert!(
            snapshot_bytes(&schemas) != fs::read(root.join("original")).unwrap(),
            "write persisted"
        ),
        "delayed-write" => {
            let s = setting(&schemas, DesktopKey::Mode);
            s.set_value("mode", &"manual".to_variant()).unwrap();
            assert_eq!(s.string("mode"), "manual");
            // This is deliberately optimistic local state, saved privately
            // only so a fresh process can check the *complete* intended
            // snapshot after this writer finishes.
            private_file(&root.join("intended"), &snapshot_bytes(&schemas));
            private_file(&root.join("local-visible"), b"pending");
            gio::Settings::sync();
            private_file(&root.join("sync-returned"), b"settled-or-failed");
        }
        "assert-disk-original" => assert!(
            snapshot_bytes(&schemas) == fs::read(root.join("baseline")).unwrap(),
            "independent read must remain original"
        ),
        "assert-disk-manual" => {
            assert_eq!(setting(&schemas, DesktopKey::Mode).string("mode"), "manual");
            assert!(root.join("config/dconf/user").is_file());
        }
        "compare-independent" => {
            let intended = DesktopSnapshot::decode(
                &omavless_runtime::app_proxy::Snapshot::new(Some(
                    fs::read(root.join("intended")).unwrap(),
                ))
                .unwrap(),
            )
            .unwrap();
            let persisted = read_desktop(&schemas).unwrap();
            let result = crate::runner::compare_desktop(&intended, &persisted);
            let status = match result {
                crate::DesktopReadback::Matches => b"matches".as_slice(),
                crate::DesktopReadback::Differs => b"differs".as_slice(),
            };
            private_file(&root.join("readback"), status);
        }
        _ => panic!("unknown fixture role"),
    }
}

#[test]
#[ignore = "requires dbus-daemon, dconf-service and installed schemas; writes only private fixture database"]
fn installed_dconf_exact_persistent_restore() {
    let fixture = Fixture::new();
    fixture.run("seed");
    fixture.run("capture");
    fixture.run("apply");
    fixture.run("assert-changed");
    fixture.run("restore");
    fixture.run("assert-original");
    assert!(fixture.root.join("config/dconf/user").is_file());
}

#[test]
#[ignore = "requires private installed dconf fixture; stops only its own service child"]
fn installed_dconf_local_readback_is_not_commit_and_sync_is_not_success() {
    for fail in [false, true] {
        let mut fixture = Fixture::new();
        fixture.run("baseline");
        fixture.service.as_mut().unwrap().signal(Signal::SIGSTOP);
        let mut writer = fixture.child("delayed-write");
        fixture.wait_for(|| fixture.root.join("local-visible").exists());
        assert!(!fixture.root.join("sync-returned").exists());
        fixture.run("assert-disk-original");
        if fail {
            drop(fixture.service.take());
        } else {
            fixture.service.as_mut().unwrap().signal(Signal::SIGCONT);
        }
        writer.finish();
        assert!(fixture.root.join("sync-returned").exists());
        fixture.run("compare-independent");
        assert_eq!(
            fs::read(fixture.root.join("readback")).unwrap(),
            if fail {
                b"differs".as_slice()
            } else {
                b"matches".as_slice()
            }
        );
        fixture.run(if fail {
            "assert-disk-original"
        } else {
            "assert-disk-manual"
        });
    }
}
