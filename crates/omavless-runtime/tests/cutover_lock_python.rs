// SPDX-License-Identifier: MIT

use omavless_runtime::cutover::{CutoverPaths, MigrationLock};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const PYTHON_TRY_LOCK: &str = r#"
import fcntl
import sys

with open(sys.argv[1], "a", encoding="utf-8") as handle:
    try:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        raise SystemExit(0)
raise SystemExit(1)
"#;

#[test]
fn rust_cutover_lock_interoperates_with_legacy_python_flock() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "omavless-cutover-python-{}-{nonce}",
        std::process::id()
    ));
    let runtime = root.join("runtime");
    let state = root.join("state");
    fs::create_dir_all(&runtime).unwrap();
    fs::create_dir(&state).unwrap();
    for path in [&root, &runtime, &state] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let uid = fs::metadata(&root).unwrap().uid();
    let paths = CutoverPaths::below(&runtime, &state, uid);

    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    let blocked = Command::new("python3")
        .arg("-c")
        .arg(PYTHON_TRY_LOCK)
        .arg(&paths.operation_lock)
        .status()
        .unwrap();
    assert!(blocked.success(), "legacy Python did not observe Rust lock");

    drop(lock);
    let available = Command::new("python3")
        .arg("-c")
        .arg(PYTHON_TRY_LOCK)
        .arg(&paths.operation_lock)
        .status()
        .unwrap();
    assert!(
        !available.success(),
        "released migration lock remained busy"
    );
    fs::remove_dir_all(root).unwrap();
}
