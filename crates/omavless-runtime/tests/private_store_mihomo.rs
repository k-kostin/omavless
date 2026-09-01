// SPDX-License-Identifier: MIT

use omavless_mihomo::validate_config;
use omavless_runtime::store_preflight::prepare_current_store;
use std::env;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn private_current_store_prepares_a_config_accepted_by_installed_mihomo() {
    if env::var_os("OMAVLESS_TEST_PRIVATE_STORE").is_none() {
        return;
    }
    let Some(core) = env::var_os("OMAVLESS_TEST_MIHOMO").map(PathBuf::from) else {
        panic!("OMAVLESS_TEST_MIHOMO is required for private-store validation");
    };
    let prepared = prepare_current_store().expect("private store preparation");
    assert!(prepared.projection.profile_count > 0);

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "omavless-private-config-{}-{unique}",
        std::process::id()
    ));
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(&root).unwrap();
    let config = root.join("config.yaml");
    fs::write(&config, prepared.config.as_bytes()).unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();

    let home = env::var_os("OMAVLESS_HOME")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .expect("private home");
    let data = home.join(".config/omavless");
    validate_config(&core, &data, &config, Duration::from_secs(30))
        .expect("installed Mihomo accepts private prepared config");
    fs::remove_dir_all(root).unwrap();
}
