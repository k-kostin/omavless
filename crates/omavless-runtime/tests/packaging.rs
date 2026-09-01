// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;

#[test]
fn packaged_user_unit_has_fixed_hardened_native_entrypoint() {
    let unit = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/systemd/omavless-runtime.service");
    let text = fs::read_to_string(unit).unwrap();
    assert!(text.contains("\nExecStart=/usr/bin/omavless daemon\n"));
    assert!(text.contains("\nConditionFileIsExecutable=/usr/bin/omavless\n"));
    assert!(text.contains("\nRuntimeDirectory=omavless\n"));
    assert!(text.contains("\nRuntimeDirectoryMode=0700\n"));
    assert!(text.contains("\nUMask=0077\n"));
    assert!(text.contains("\nNoNewPrivileges=yes\n"));
    assert!(text.contains("\nProtectSystem=strict\n"));
    for directive in [
        "ConfigurationDirectory=omavless",
        "ConfigurationDirectoryMode=0700",
        "StateDirectory=omavless",
        "StateDirectoryMode=0700",
        "CacheDirectory=omavless",
        "CacheDirectoryMode=0700",
    ] {
        assert!(text.lines().any(|line| line == directive));
    }
    assert!(!text.contains("python"));
    assert!(!text.contains("/bin/sh"));
    assert!(!text.contains("sudo"));
    assert!(!text.contains("pkexec"));
}
