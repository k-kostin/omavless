// SPDX-License-Identifier: MIT

use omavless_runtime::core::OwnedCore;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[path = "../../../tests/support/temp.rs"]
mod test_temp;

fn configured_core() -> Option<PathBuf> {
    env::var_os("OMAVLESS_TEST_MIHOMO").map(PathBuf::from)
}

fn root() -> PathBuf {
    test_temp::directory_under(&test_temp::home_parent().unwrap(), "owned-controller").unwrap()
}

fn write_config(root: &Path, socket: &Path) -> PathBuf {
    let config = root.join("config.yaml");
    fs::write(
        &config,
        format!(
            "mixed-port: 0\nallow-lan: false\nmode: rule\nlog-level: silent\nexternal-controller-unix: {}\nproxies: []\nproxy-groups: []\nrules: []\n",
            socket.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    config
}

#[test]
fn installed_mihomo_is_parent_owned_ready_and_gracefully_reaped_without_tun() {
    let Some(core) = configured_core() else {
        return;
    };
    let root = root();
    let socket = root.join("controller.sock");
    let config = write_config(&root, &socket);
    let mut owned = OwnedCore::spawn(&core, &root, &config, &socket).unwrap();
    owned.wait_ready(Duration::from_secs(10)).unwrap();
    assert!(owned.running().unwrap());
    assert!(owned.stop(Duration::from_secs(5)).unwrap().graceful);
    assert!(owned.pid().is_none());
    fs::remove_dir_all(root).unwrap();
}
