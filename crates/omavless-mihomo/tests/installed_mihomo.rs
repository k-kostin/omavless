// SPDX-License-Identifier: MIT

use omavless_mihomo::{ErrorKind, ReadOnlyEndpoint, controller_get, validate_config};
use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn configured_core() -> Option<PathBuf> {
    env::var_os("OMAVLESS_TEST_MIHOMO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn temporary_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "omavless-r4-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("temporary root");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("private root");
    path
}

fn write_config(root: &Path, socket: &Path) -> PathBuf {
    let path = root.join("config.yaml");
    let config = format!(
        "mixed-port: 0\nallow-lan: false\nmode: rule\nlog-level: silent\nexternal-controller-unix: {}\nproxies: []\nproxy-groups: []\nrules: []\n",
        socket.display()
    );
    fs::write(&path, config).expect("write config");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("private config");
    path
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

#[test]
fn installed_mihomo_validates_and_serves_unix_only_controller() {
    let Some(core) = configured_core() else {
        eprintln!("set OMAVLESS_TEST_MIHOMO for the opt-in core integration test");
        return;
    };
    let root = temporary_root("installed");
    let socket = root.join("controller.sock");
    let config = write_config(&root, &socket);

    validate_config(&core, &root, &config, Duration::from_secs(15))
        .expect("installed Mihomo should validate the bounded config");
    let child = Command::new(&core)
        .arg("-d")
        .arg(&root)
        .arg("-f")
        .arg(&config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start isolated Mihomo");
    let mut child = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    let version = loop {
        if child.0.try_wait().expect("Mihomo status").is_some() {
            panic!("isolated Mihomo stopped before its Unix controller became ready");
        }
        match controller_get(
            &socket,
            ReadOnlyEndpoint::Version,
            Duration::from_secs(1),
            16 * 1024,
        ) {
            Ok(response) => break response,
            Err(error)
                if error.kind() == ErrorKind::ControllerUnavailable
                    && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("private Unix controller check failed: {error}"),
        }
    };
    assert_eq!(version.status, 200);
    assert!(version.payload.is_object());
    drop(child);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => panic!("cleanup failed: {error}"),
    }
}
