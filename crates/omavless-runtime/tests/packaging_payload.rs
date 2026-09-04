// SPDX-License-Identifier: MIT

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "omavless-package-{label}-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn binary(&self) -> PathBuf {
        let binary = self.root.join("prebuilt-omavless");
        fs::write(&binary, b"synthetic native executable\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        binary
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stager() -> PathBuf {
    repository_root().join("packaging/arch/stage-payload.sh")
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn files_below(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() {
            files_below(root, &path, output);
        } else {
            output.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
}

#[test]
fn staged_arch_payload_has_exact_tree_content_and_modes() {
    let fixture = Fixture::new("exact");
    let destdir = fixture.root.join("pkgroot");
    fs::create_dir(&destdir).unwrap();
    let binary = fixture.binary();

    let status = Command::new(stager())
        .arg(&destdir)
        .arg(&binary)
        .status()
        .unwrap();
    assert!(status.success());

    let expected = [
        "usr/bin/omavless",
        "usr/lib/systemd/user/omavless-runtime.service",
        "usr/share/doc/omavless/README.md",
        "usr/share/licenses/omavless/LICENSE",
        "usr/share/licenses/omavless/THIRD_PARTY_NOTICES.md",
    ];
    let mut actual = Vec::new();
    files_below(&destdir, &destdir, &mut actual);
    actual.sort();
    assert_eq!(
        actual,
        expected.map(PathBuf::from),
        "payload must not acquire undeclared files"
    );

    assert_eq!(
        fs::read(destdir.join(expected[0])).unwrap(),
        fs::read(&binary).unwrap()
    );
    assert_eq!(
        fs::read(destdir.join(expected[1])).unwrap(),
        fs::read(repository_root().join("packaging/systemd/omavless-runtime.service")).unwrap()
    );
    assert_eq!(
        fs::read(destdir.join(expected[2])).unwrap(),
        fs::read(repository_root().join("packaging/arch/README.md")).unwrap()
    );
    assert_eq!(
        fs::read(destdir.join(expected[3])).unwrap(),
        fs::read(repository_root().join("LICENSE")).unwrap()
    );
    assert_eq!(
        fs::read(destdir.join(expected[4])).unwrap(),
        fs::read(repository_root().join("THIRD_PARTY_NOTICES.md")).unwrap()
    );

    assert_eq!(mode(&destdir.join(expected[0])), 0o755);
    for path in &expected[1..] {
        assert_eq!(mode(&destdir.join(path)), 0o644);
    }
    for path in [
        "usr",
        "usr/bin",
        "usr/lib",
        "usr/lib/systemd",
        "usr/lib/systemd/user",
        "usr/share",
        "usr/share/doc",
        "usr/share/doc/omavless",
        "usr/share/licenses",
        "usr/share/licenses/omavless",
    ] {
        assert_eq!(mode(&destdir.join(path)), 0o755);
    }
}

#[test]
fn staging_rejects_relative_and_symlinked_destinations_without_escape() {
    let fixture = Fixture::new("unsafe");
    let binary = fixture.binary();

    let relative = Command::new(stager())
        .arg("relative-package-root")
        .arg(&binary)
        .status()
        .unwrap();
    assert!(!relative.success());

    let escaped = fixture.root.join("escaped");
    fs::create_dir(&escaped).unwrap();
    let linked_root = fixture.root.join("linked-root");
    symlink(&escaped, &linked_root).unwrap();
    let linked = Command::new(stager())
        .arg(&linked_root)
        .arg(&binary)
        .status()
        .unwrap();
    assert!(!linked.success());
    assert!(fs::read_dir(&escaped).unwrap().next().is_none());

    let destdir = fixture.root.join("file-link-root");
    let binary_dir = destdir.join("usr/bin");
    fs::create_dir_all(&binary_dir).unwrap();
    let escaped_file = escaped.join("outside-binary");
    fs::write(&escaped_file, b"unchanged\n").unwrap();
    symlink(&escaped_file, binary_dir.join("omavless")).unwrap();
    let file_link = Command::new(stager())
        .arg(&destdir)
        .arg(&binary)
        .status()
        .unwrap();
    assert!(!file_link.success());
    assert_eq!(fs::read(&escaped_file).unwrap(), b"unchanged\n");
}

#[test]
fn staging_boundary_contains_no_activation_or_generic_command_path() {
    let script = fs::read_to_string(stager()).unwrap();
    for forbidden in [
        "systemctl",
        "pacman",
        "sudo",
        "pkexec",
        "ownership.json",
        ".config/omavless",
        "eval ",
        "bash -c",
        "sh -c",
    ] {
        assert!(
            !script.contains(forbidden),
            "stager contains forbidden behavior: {forbidden}"
        );
    }
}
