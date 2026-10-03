// SPDX-License-Identifier: MIT
// Real executable, private synthetic roots, no usable helper PATH or host bus.
use nix::unistd::Uid;
use omavless_runtime::cutover::{CutoverPaths, MigrationLock};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    runtime: PathBuf,
    state: PathBuf,
}
fn mkdir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "omavless-fresh-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        mkdir(&root);
        let home = root.join("home");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for path in [&home, &runtime, &state] {
            mkdir(path);
        }
        Self {
            root,
            home,
            runtime,
            state,
        }
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_omavless"));
        cmd.env_clear()
            .env("HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("XDG_STATE_HOME", &self.state)
            .env("PATH", "/nonexistent")
            .stdin(Stdio::null());
        cmd
    }
    fn run(&self) -> Output {
        self.command()
            .args(["setup", "initialize"])
            .output()
            .unwrap()
    }
    fn config(&self) -> PathBuf {
        self.home.join(".config/omavless")
    }
    fn store(&self) -> PathBuf {
        self.config().join("profiles.json")
    }
    fn template(&self) -> PathBuf {
        self.config().join("route-template.yaml")
    }
    fn config_directory(&self) {
        mkdir(&self.home.join(".config"));
        mkdir(&self.config());
    }
    fn assert_no_owner(&self) {
        assert!(!self.runtime.join("omavless").exists());
        assert!(!self.state.join("omavless").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn accepted(output: Output, created: u64) {
    assert!(output.status.success(), "synthetic setup failed");
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"schemaVersion":1,"outcome":"prepared","createdFiles":created,
        "ownershipActivated":false,"startupEnabled":false,"onboardingComplete":false})
    );
}
fn refused(output: Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.len() < 180);
    assert!(!text.contains("synthetic-private"));
    assert!(!text.contains("/tmp/"));
}
fn private_write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn initialize_creates_exact_private_defaults_without_python_or_runtime_and_retries_noop() {
    let f = Fixture::new();
    accepted(f.run(), 2);
    f.assert_no_owner();
    let store = fs::read(f.store()).unwrap();
    let value: Value = serde_json::from_slice(&store).unwrap();
    assert_eq!(value["profiles"], serde_json::json!([]));
    assert_eq!(value["subscriptions"], serde_json::json!([]));
    assert_eq!(value["startup"]["enabled"], false);
    assert_eq!(value["startupConfigured"], true);
    assert_eq!(value["onboardingComplete"], false);
    assert_eq!(value["activeId"], "");
    assert_eq!(
        fs::read(f.template()).unwrap(),
        include_bytes!("../../../templates/default.yaml")
    );
    for path in [f.store(), f.template()] {
        let m = fs::symlink_metadata(path).unwrap();
        assert!(m.is_file());
        assert_eq!(m.mode() & 0o777, 0o600);
        assert_eq!(m.uid(), Uid::current().as_raw());
    }
    assert_eq!(fs::metadata(f.config()).unwrap().mode() & 0o777, 0o700);
    let inode = fs::metadata(f.store()).unwrap().ino();
    accepted(f.run(), 0);
    assert_eq!(fs::read(f.store()).unwrap(), store);
    assert_eq!(fs::metadata(f.store()).unwrap().ino(), inode);
    f.assert_no_owner();
}

#[test]
fn default_state_parents_are_create_only_and_custom_roots_require_host_preparation() {
    let f = Fixture::new();
    let run = || {
        f.command()
            .env_remove("XDG_STATE_HOME")
            .args(["setup", "initialize"])
            .output()
            .unwrap()
    };
    accepted(run(), 2);
    for path in [f.home.join(".local"), f.home.join(".local/state")] {
        let m = fs::symlink_metadata(path).unwrap();
        assert!(m.is_dir());
        assert_eq!(m.mode() & 0o777, 0o700);
        assert_eq!(m.uid(), Uid::current().as_raw());
    }
    assert!(!f.home.join(".local/state/omavless").exists());
    accepted(run(), 0);
    fs::set_permissions(f.home.join(".local"), fs::Permissions::from_mode(0o777)).unwrap();
    refused(run());
    assert_eq!(
        fs::metadata(f.home.join(".local")).unwrap().mode() & 0o777,
        0o777
    );

    let f = Fixture::new();
    refused(
        f.command()
            .env("XDG_STATE_HOME", f.root.join("missing/custom"))
            .args(["setup", "initialize"])
            .output()
            .unwrap(),
    );
    assert!(!f.config().exists());
    assert!(!f.root.join("missing").exists());
    symlink(&f.state, f.home.join(".local")).unwrap();
    refused(
        f.command()
            .env_remove("XDG_STATE_HOME")
            .args(["setup", "initialize"])
            .output()
            .unwrap(),
    );
    assert!(!f.state.join("state").exists());
}

#[test]
fn initialize_resumes_only_exact_partial_initial_payloads() {
    for missing_store in [true, false] {
        let f = Fixture::new();
        accepted(f.run(), 2);
        fs::remove_file(if missing_store {
            f.store()
        } else {
            f.template()
        })
        .unwrap();
        accepted(f.run(), 1);
        f.assert_no_owner();
    }
}

#[test]
fn initialize_rejects_existing_modified_or_corrupt_data_without_replacing_anything() {
    for target_store in [true, false] {
        for content in [b"synthetic-private://password".as_slice(), b"{}", b"\xff"] {
            let f = Fixture::new();
            f.config_directory();
            let target = if target_store {
                f.store()
            } else {
                f.template()
            };
            private_write(&target, content);
            refused(f.run());
            assert_eq!(fs::read(&target).unwrap(), content);
            assert_eq!(fs::read_dir(f.config()).unwrap().count(), 1);
            f.assert_no_owner();
        }
    }
    let f = Fixture::new();
    accepted(f.run(), 2);
    let original = fs::read_to_string(f.store()).unwrap();
    // Even a valid user's changed preferences are not initialization input.
    let changed = original.replace(
        "\"onboardingComplete\": false",
        "\"onboardingComplete\": true",
    );
    private_write(&f.store(), changed.as_bytes());
    refused(f.run());
    assert_eq!(fs::read_to_string(f.store()).unwrap(), changed);
}

#[test]
fn initialize_refuses_ownership_receipts_unknown_members_and_runtime_publication() {
    for member in [
        "ownership.json",
        "frontend-bridge.target",
        "desired.json",
        "login-receipt.json",
        "unknown",
    ] {
        let f = Fixture::new();
        mkdir(&f.state.join("omavless"));
        let file = f.state.join("omavless").join(member);
        private_write(&file, b"synthetic-private");
        refused(f.run());
        assert_eq!(fs::read(file).unwrap(), b"synthetic-private");
        assert!(!f.config().exists());
    }
    let f = Fixture::new();
    mkdir(&f.runtime.join("omavless"));
    refused(f.run());
    assert!(!f.config().exists());
    let f = Fixture::new();
    f.config_directory();
    private_write(&f.config().join("config.yaml"), b"synthetic-private");
    refused(f.run());
    assert!(!f.store().exists());
}

#[test]
fn initialize_refuses_symlinks_and_unsafe_modes_without_chmod() {
    for mode in [0o755, 0o777] {
        let f = Fixture::new();
        f.config_directory();
        fs::set_permissions(f.config(), fs::Permissions::from_mode(mode)).unwrap();
        refused(f.run());
        assert_eq!(fs::metadata(f.config()).unwrap().mode() & 0o777, mode);
        assert!(!f.store().exists());
    }
    for target_store in [true, false] {
        let f = Fixture::new();
        f.config_directory();
        let target = if target_store {
            f.store()
        } else {
            f.template()
        };
        symlink(f.root.join("missing"), &target).unwrap();
        refused(f.run());
        assert!(
            fs::symlink_metadata(target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    let f = Fixture::new();
    symlink(&f.state, f.home.join(".config")).unwrap();
    refused(f.run());
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
    let f = Fixture::new();
    accepted(f.run(), 2);
    fs::set_permissions(f.store(), fs::Permissions::from_mode(0o644)).unwrap();
    refused(f.run());
    assert_eq!(fs::metadata(f.store()).unwrap().mode() & 0o777, 0o644);
}

#[test]
fn initialize_refuses_lock_contention_before_config_creation() {
    let f = Fixture::new();
    let uid = Uid::current().as_raw();
    let paths = CutoverPaths::below(&f.runtime, &f.state, uid);
    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    refused(f.run());
    assert!(!f.config().exists());
    drop(lock);
    accepted(f.run(), 2);
}

#[test]
fn initialize_invalid_command_environment_and_private_input_do_not_leak_or_write() {
    let f = Fixture::new();
    for args in [
        vec!["setup"],
        vec!["setup", "initialize", "synthetic-private"],
        vec!["setup", "--force"],
    ] {
        refused(f.command().args(args).output().unwrap());
        assert!(!f.config().exists());
    }
    for (key, value) in [
        ("HOME", "relative"),
        ("OMAVLESS_HOME", "/synthetic-private"),
        ("XDG_RUNTIME_DIR", ""),
        ("XDG_STATE_HOME", "relative"),
        ("HOME", "/tmp/../synthetic-private"),
    ] {
        refused(
            f.command()
                .env(key, value)
                .args(["setup", "initialize"])
                .output()
                .unwrap(),
        );
        assert!(!f.config().exists());
    }
}

#[test]
fn initialize_refuses_fifo_and_oversized_file_without_reading_or_replacing() {
    let f = Fixture::new();
    f.config_directory();
    nix::unistd::mkfifo(&f.store(), nix::sys::stat::Mode::S_IRUSR).unwrap();
    refused(f.run());
    assert!(!f.template().exists());
    let f = Fixture::new();
    f.config_directory();
    let file = fs::File::create(f.store()).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .unwrap();
    file.set_len(10 * 1024 * 1024).unwrap();
    refused(f.run());
    assert_eq!(file.metadata().unwrap().len(), 10 * 1024 * 1024);
    assert!(!f.template().exists());
}

#[test]
fn concurrent_initializers_never_replace_or_publish_partial_success() {
    let f = Fixture::new();
    let mut children = Vec::new();
    for _ in 0..2 {
        children.push(
            f.command()
                .args(["setup", "initialize"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    // Reap both owned subprocesses before asserting on either result: an
    // assertion failure must not drop a still-running sibling into cleanup.
    let outputs: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    let mut created = 0;
    for output in outputs {
        if output.status.success() {
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            let count = value["createdFiles"].as_u64().unwrap();
            assert!(count <= 2);
            created += count;
            accepted(output, count);
        } else {
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            // If both saw an absent lease, the exclusive-create loser fails
            // closed before flock. Never retry/reopen/repair that new inode.
            assert!(
                output.stderr == b"Another OmaVLESS operation owns the migration lock\n"
                    || output.stderr == b"Native setup paths or permissions are unsafe\n"
            );
        }
    }
    assert_eq!(created, 2);
    let store = fs::read(f.store()).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&store).unwrap(),
        serde_json::json!({"version":3,"activeId":"","lastId":"","profiles":[],
            "subscriptions":[],"routingPreset":"","customRules":[],"rulesUpdatedAt":0,
            "startupConfigured":true,"startup":{"enabled":false,"target":"last",
                "profileId":"","mode":"rule"},"onboardingComplete":false})
    );
    assert_eq!(
        fs::read(f.template()).unwrap(),
        include_bytes!("../../../templates/default.yaml")
    );
    assert_eq!(fs::metadata(f.config()).unwrap().mode() & 0o7777, 0o700);
    let members: Vec<_> = [f.store(), f.template()]
        .into_iter()
        .map(|path| {
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file());
            assert!(!metadata.file_type().is_symlink());
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.uid(), Uid::current().as_raw());
            assert_eq!(metadata.nlink(), 1);
            (path, metadata)
        })
        .collect();
    accepted(f.run(), 0);
    for (path, before) in members {
        let after = fs::symlink_metadata(&path).unwrap();
        assert_eq!(after.dev(), before.dev());
        assert_eq!(after.ino(), before.ino());
        assert_eq!(after.uid(), before.uid());
        assert_eq!(after.mode(), before.mode());
        assert_eq!(after.nlink(), before.nlink());
        assert_eq!(after.len(), before.len());
        assert_eq!(after.mtime(), before.mtime());
        assert_eq!(after.mtime_nsec(), before.mtime_nsec());
        assert_eq!(after.ctime(), before.ctime());
        assert_eq!(after.ctime_nsec(), before.ctime_nsec());
    }
    assert_eq!(fs::read(f.store()).unwrap(), store);
    assert_eq!(
        fs::read(f.template()).unwrap(),
        include_bytes!("../../../templates/default.yaml")
    );
    f.assert_no_owner();
}
