// SPDX-License-Identifier: MIT

use omavless_domain::config::assemble_runtime_config;
use omavless_mihomo::{ErrorKind, ReadOnlyEndpoint, controller_get, validate_config};
use omavless_profile::canonical::parse_canonical;
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

#[test]
fn installed_mihomo_accepts_every_canonical_profile_renderer() {
    let Some(core) = configured_core() else {
        eprintln!("set OMAVLESS_TEST_MIHOMO for the opt-in core integration test");
        return;
    };
    let root = temporary_root("renderers");
    let socket = root.join("controller.sock");
    let profiles = [
        "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Synthetic",
        "trojan://synthetic-password@203.0.113.2:443?security=tls&sni=cdn.example.invalid#Synthetic",
        "hy2://synthetic-auth@203.0.113.3:443?sni=cdn.example.invalid#Synthetic",
        "tuic://22222222-2222-4222-8222-222222222222:synthetic-password@203.0.113.4:443?sni=cdn.example.invalid#Synthetic",
    ];
    let rendered = profiles
        .iter()
        .enumerate()
        .map(|(index, input)| {
            parse_canonical(input)
                .expect("synthetic canonical profile")
                .render_mihomo_proxy(&format!("Synthetic {index}"), None)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let template = "mixed-port: 0\nallow-lan: false\nmode: rule\nlog-level: silent\nproxies:\n{{OMAVLESS_PROXY}}\nproxy-groups:\n- name: PROXY\n  type: select\n  proxies:\n  - Synthetic 0\n  - Synthetic 1\n  - Synthetic 2\n  - Synthetic 3\nrules:\n- MATCH,PROXY\n";
    let config = assemble_runtime_config(template, &rendered, socket.to_str().unwrap(), &[])
        .expect("runtime config");
    let config_path = root.join("profiles.yaml");
    fs::write(&config_path, config).expect("write private config");
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600)).expect("private config");

    validate_config(&core, &root, &config_path, Duration::from_secs(15))
        .expect("installed Mihomo should accept every canonical renderer");
    fs::remove_dir_all(root).expect("cleanup renderer config");
}
