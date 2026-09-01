// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;

#[test]
fn packaged_user_unit_has_fixed_hardened_native_entrypoint() {
    let unit = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/systemd/omavless-runtime.service");
    let text = fs::read_to_string(unit).unwrap();
    assert!(text.contains("\nExecStart=/usr/bin/omavless daemon\n"));
    assert!(text.contains("\nConditionPathIsExecutable=/usr/bin/omavless\n"));
    assert!(text.contains("\nRuntimeDirectory=omavless\n"));
    assert!(text.contains("\nRuntimeDirectoryMode=0700\n"));
    assert!(text.contains("\nUMask=0077\n"));
    assert!(text.contains("\nNoNewPrivileges=yes\n"));
    assert!(text.contains("\nProtectSystem=strict\n"));
    assert!(!text.contains("python"));
    assert!(!text.contains("/bin/sh"));
    assert!(!text.contains("sudo"));
    assert!(!text.contains("pkexec"));
}
