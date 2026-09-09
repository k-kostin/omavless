// SPDX-License-Identifier: MIT
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::process::{Command, Output};

#[path = "../../../tests/support/temp.rs"]
mod test_temp;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(test_temp::directory("plugin-target").unwrap())
    }
    fn state(&self) -> PathBuf {
        self.0.join("state/omavless")
    }
    fn setup(&self) {
        fs::create_dir_all(self.state()).unwrap();
        fs::set_permissions(self.state(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.state().join(name), bytes).unwrap();
        fs::set_permissions(self.state().join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn marker(&self, phase: &str, generation: u64) {
        self.write(
            "ownership.json",
            serde_json::json!({"schemaVersion":1,"generation":generation,"phase":phase})
                .to_string()
                .as_bytes(),
        );
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omavless"));
        command
            .env("HOME", &self.0)
            .env("OMAVLESS_HOME", &self.0)
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_RUNTIME_DIR", self.0.join("runtime-do-not-create"));
        command
    }
    fn run(&self) -> Output {
        self.command().args(["plugin", "target"]).output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn absent_legacy_never_creates_state_runtime_or_lock_files() {
    let f = Fixture::new();
    let output = f.run();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"legacy\n");
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
    f.setup();
    let before = fs::metadata(f.state()).unwrap().permissions().mode();
    assert_eq!(f.run().stdout, b"legacy\n");
    assert_eq!(fs::read_dir(f.state()).unwrap().count(), 0);
    assert_eq!(
        fs::metadata(f.state()).unwrap().permissions().mode(),
        before
    );
    assert!(!f.0.join("runtime-do-not-create").exists());
}

#[test]
fn exact_committed_targets_and_matching_selector_only() {
    for (phase, selector) in [("rust", "rust:1\n"), ("legacy", "legacy:1\n")] {
        let f = Fixture::new();
        f.setup();
        f.marker(phase, 2);
        f.write("frontend-bridge.target", selector.as_bytes());
        let marker = fs::read(f.state().join("ownership.json")).unwrap();
        let output = f.run();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{phase}\n").as_bytes());
        assert!(output.stderr.is_empty());
        assert_eq!(fs::read(f.state().join("ownership.json")).unwrap(), marker);
        assert_eq!(
            fs::read(f.state().join("frontend-bridge.target")).unwrap(),
            selector.as_bytes()
        );
        assert_eq!(fs::read_dir(f.state()).unwrap().count(), 2);
    }
}

#[test]
fn stale_preparing_missing_rust_selector_and_malformed_state_refuse() {
    for (phase, generation, selector) in [
        ("rust", 3, Some("rust:1\n")),
        ("cutoverPreparing", 1, Some("rust:1\n")),
        ("cutoverPreparing", 1, Some("legacy:1\n")),
        ("rollbackPreparing", 3, Some("rust:1\n")),
        ("rust", 2, None),
        ("legacy", 0, Some("rust:1\n")),
        ("rust", 2, Some("private-malformed")),
    ] {
        let f = Fixture::new();
        f.setup();
        f.marker(phase, generation);
        if let Some(selector) = selector {
            f.write("frontend-bridge.target", selector.as_bytes());
        }
        let output = f.run();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-malformed"));
    }
    let f = Fixture::new();
    f.setup();
    f.write("ownership.json", b"{private-malformed}");
    let output = f.run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-malformed"));
    f.write(
        "ownership.json",
        br#"{"schemaVersion":2,"generation":0,"phase":"legacy"}"#,
    );
    assert!(!f.run().status.success());
}

#[test]
fn unsafe_hierarchy_files_and_extra_arguments_never_repair_or_echo() {
    for kind in [
        "parent-symlink",
        "leaf-symlink",
        "leaf-mode",
        "selector-mode",
        "marker-symlink",
    ] {
        let f = Fixture::new();
        if kind == "parent-symlink" {
            symlink("missing", f.0.join("state")).unwrap();
        } else if kind == "leaf-symlink" {
            fs::create_dir(f.0.join("state")).unwrap();
            symlink("missing", f.state()).unwrap();
        } else {
            f.setup();
            f.marker("rust", 2);
            f.write("frontend-bridge.target", b"rust:1\n");
            match kind {
                "leaf-mode" => {
                    fs::set_permissions(f.state(), fs::Permissions::from_mode(0o755)).unwrap()
                }
                "selector-mode" => fs::set_permissions(
                    f.state().join("frontend-bridge.target"),
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap(),
                _ => {
                    fs::remove_file(f.state().join("ownership.json")).unwrap();
                    symlink("missing", f.state().join("ownership.json")).unwrap();
                }
            }
        }
        let output = f.run();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let f = Fixture::new();
    let output = f
        .command()
        .args(["plugin", "target", "private-token"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-token"));
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}
