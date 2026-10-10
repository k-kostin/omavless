// SPDX-License-Identifier: MIT
//! Explicitly ignored process-loss acceptance. Run only a reviewed frozen ELF.
//! No environment-triggered fault or worker exists in a production build.
use super::*;
use crate::desired::{DesiredState, write_desired};
use crate::restore_executor_candidate::{EffectStep, execute_with_hook};
use crate::restore_successor_publication_candidate::tests::{OLD, backup};
use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::FileExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

#[path = "restore_abort_process_support_tests.rs"]
mod support;
use support::{OwnedProcess, Quarantine};

const PASS: &[u8] = b"synthetic first Abort process fixture";
const WORKER: &str = "production_owner::first_abort::process_reentry::process_worker";
const TOKEN: &str = "OVABORT-CHECKPOINT-v1";
const CASES: [&str; 5] = ["linked", "mixed", "empty", "full", "final"];

fn unprivileged_status(status: &str, home: &str) -> bool {
    if status.len() > 65536 || home != "/home/kdk_vm" {
        return false;
    }
    let keys = [
        "Uid", "Gid", "Groups", "CapInh", "CapPrm", "CapEff", "CapAmb",
    ];
    keys.iter().all(|key| {
        let mut values = status.lines().filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name == *key).then_some(value.trim())
        });
        let Some(value) = values.next() else {
            return false;
        };
        values.next().is_none()
            && match *key {
                "Uid" | "Gid" => value.split_whitespace().collect::<Vec<_>>() == ["1000"; 4],
                "Groups" => value.is_empty(),
                _ => value.len() == 16 && value.bytes().all(|byte| byte == b'0'),
            }
    })
}

fn require_unprivileged_process() {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    assert!(unprivileged_status(
        &status,
        &std::env::var("HOME").unwrap()
    ));
}

#[test]
fn process_credentials_reject_saved_root_groups_capabilities_and_missing_fields() {
    let good = "Uid:\t1000\t1000\t1000\t1000\nGid:\t1000\t1000\t1000\t1000\nGroups:\t\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapAmb:\t0000000000000000\n";
    assert!(unprivileged_status(good, "/home/kdk_vm"));
    assert!(!unprivileged_status(good, "/root"));
    for bad in [
        good.replacen("1000\t1000\t1000\t1000", "1000\t1000\t0\t1000", 1),
        good.replace("Groups:\t", "Groups:\t1000"),
        good.replace("CapEff:\t0000000000000000", "CapEff:\t0000000000000001"),
        good.replace("CapPrm:\t0000000000000000", "CapPrm:\t0000000000000001"),
        good.replace("CapInh:\t0000000000000000", "CapInh:\t0000000000000001"),
        good.replace("CapAmb:\t0000000000000000", "CapAmb:\t0000000000000001"),
        good.replace("CapAmb:", "Missing:"),
        format!("{good}Uid:\t1000\t1000\t1000\t1000\n"),
    ] {
        assert!(!unprivileged_status(&bad, "/home/kdk_vm"));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Point {
    Gate,
    Final,
}

type Hook = Box<dyn FnMut(Point)>;
thread_local! { static HOOK: RefCell<Option<Hook>> = RefCell::new(None); }

pub(super) fn checkpoint(point: Point) {
    HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().as_mut() {
            hook(point);
        }
    });
}

#[test]
fn checkpoint_is_thread_local_inert_and_has_no_runtime_environment_dispatch() {
    checkpoint(Point::Gate);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let copy = seen.clone();
    HOOK.with(|hook| *hook.borrow_mut() = Some(Box::new(move |_| copy.set(copy.get() + 1))));
    checkpoint(Point::Gate);
    std::thread::spawn(|| checkpoint(Point::Final))
        .join()
        .unwrap();
    assert_eq!(seen.get(), 1);
    HOOK.with(|hook| *hook.borrow_mut() = None);
    checkpoint(Point::Final);
    assert_eq!(seen.get(), 1);
}

fn private_create(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

fn nonce() -> String {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn safe_ancestry(path: &Path, uid: u32) -> bool {
    path.is_absolute() && path.ancestors().all(|ancestor| {
        let Ok(m) = fs::symlink_metadata(ancestor) else { return false; };
        m.is_dir() && !m.file_type().is_symlink() && (m.uid() == 0 || m.uid() == uid)
            && m.mode() & 0o6022 == 0
            && matches!(fs::symlink_metadata(ancestor.join(".git")), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    })
}

fn fixture_paths(root: &Path, uid: u32) -> (PathBuf, CutoverPaths, DesiredPaths) {
    (
        root.join("home/.config/omavless"),
        CutoverPaths::below(&root.join("runtime"), &root.join("state"), uid),
        DesiredPaths::below(&root.join("state")),
    )
}

fn fixture(elf: &Path, case: &str) -> PathBuf {
    let uid = Uid::current().as_raw();
    let base = PathBuf::from(format!("/run/user/{uid}"));
    assert!(
        safe_ancestry(&base, uid),
        "unsafe or Git-owned fixture ancestry"
    );
    let root = base.join(format!("ov-abort-process-{}", nonce()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    for name in [
        "home",
        "home/.config",
        "home/.config/omavless",
        "home/.local",
        "home/.local/bin",
        "runtime",
        "runtime/omavless",
        "state",
        "state/omavless",
    ] {
        fs::create_dir(root.join(name)).unwrap();
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    // Resolution-only synthetic executable, not a Mihomo package/authority claim.
    // ObservationOnlyNativeHost never executes this or admits a managed pair.
    symlink(elf, root.join("home/.local/bin/mihomo")).unwrap();
    private_create(
        &root.join("fixture.json"),
        format!("{{\"schema\":1,\"case\":\"{case}\"}}\n").as_bytes(),
    );
    let archive =
        omavless_domain::private_backup::seal(backup().store(), backup().template(), PASS).unwrap();
    private_create(&root.join("archive.ovb"), &archive);
    let opened =
        crate::backup_destination_candidate::open_existing(&root.join("archive.ovb"), uid, PASS)
            .unwrap();
    let (config, paths, desired) = fixture_paths(&root, uid);
    private_create(
        &paths.ownership_marker,
        br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
    );
    write_desired(&desired, uid, &DesiredState::default()).unwrap();
    for (name, bytes) in ["profiles.json", "route-template.yaml"]
        .into_iter()
        .zip(OLD)
    {
        private_create(&config.join(name), bytes);
    }
    let incoming = opened.restore_store_off().unwrap();
    crate::restore_staging_candidate::stage_private_pair(
        &paths.state_directory,
        uid,
        OLD[0],
        OLD[1],
        &incoming,
        opened.template(),
    )
    .unwrap();
    let lock = MigrationLock::acquire(&paths, uid).unwrap();
    assert!(
        execute_with_hook(
            &config,
            &paths,
            uid,
            2,
            &lock,
            [93; 16],
            || true,
            |step| step != EffectStep::Renamed(1)
        )
        .is_err()
    );
    drop(lock);
    root
}

fn validate_root(root: &Path) {
    let uid = Uid::current().as_raw();
    assert_eq!(root.parent(), Some(Path::new(&format!("/run/user/{uid}"))));
    let name = root.file_name().unwrap().to_str().unwrap();
    let suffix = name.strip_prefix("ov-abort-process-").unwrap();
    assert!(suffix.len() == 32 && suffix.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(safe_ancestry(root, uid));
    let metadata = fs::symlink_metadata(root).unwrap();
    assert_eq!(metadata.uid(), uid);
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    let file = fs::symlink_metadata(root.join("fixture.json")).unwrap();
    assert!(
        file.is_file()
            && file.uid() == uid
            && file.nlink() == 1
            && file.mode() & 0o7777 == 0o600
            && file.len() <= 128
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("fixture.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema"], 1);
    assert!(CASES.contains(&manifest["case"].as_str().unwrap()));
}

fn reached(root: &Path, case: &str, point: Point) -> bool {
    let uid = Uid::current().as_raw();
    let (config, paths, _) = fixture_paths(root, uid);
    let terminal = paths.state_directory.join("restore-decision.terminal");
    let length = fs::metadata(&terminal).ok().map(|m| m.len());
    match case {
        "linked" => point == Point::Gate && config.join(".restore-profiles.old").exists(),
        "mixed" => {
            point == Point::Gate
                && fs::read(config.join("profiles.json")).ok().as_deref() == Some(OLD[0])
                && fs::read(config.join("route-template.yaml")).is_ok_and(|bytes| bytes != OLD[1])
        }
        "empty" => point == Point::Gate && length == Some(0),
        "full" => {
            point == Point::Gate
                && length == Some(crate::restore_decision_candidate::RECORD_BYTES as u64)
        }
        "final" => point == Point::Final,
        _ => false,
    }
}

#[test]
#[ignore = "private subprocess worker; only reviewed frozen-ELF harness"]
fn process_worker() {
    require_unprivileged_process();
    let _frozen = FrozenElf::capture();
    let root = PathBuf::from(
        std::env::var_os("OMAVLESS_ABORT_PROCESS_ROOT").expect("explicit fixture required"),
    );
    validate_root(&root);
    let mode = std::env::var("OMAVLESS_ABORT_PROCESS_MODE").unwrap();
    assert!(CASES.contains(&mode.as_str()) || mode == "recover" || mode == "refuse");
    let mut passphrase = Zeroizing::new(Vec::new());
    std::io::stdin()
        .take(129)
        .read_to_end(&mut passphrase)
        .unwrap();
    assert!(passphrase.len() <= 128);
    let callbacks = std::rc::Rc::new(std::cell::Cell::new(0));
    if mode == "refuse" {
        let count = callbacks.clone();
        HOOK.with(|hook| *hook.borrow_mut() = Some(Box::new(move |_| count.set(count.get() + 1))));
    }
    if CASES.contains(&mode.as_str()) {
        let source_root = root.clone();
        HOOK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |point| {
                if reached(&source_root, &mode, point) {
                    println!("{TOKEN}");
                    std::io::stdout().flush().unwrap();
                    loop {
                        std::thread::park();
                    }
                }
            }))
        });
    }
    // The exact private constructor: fresh archive auth, current fixed paths,
    // actual ObservationOnlyNativeHost and existing lease. No synthetic host.
    let result = current(&root.join("archive.ovb"), &passphrase);
    if std::env::var("OMAVLESS_ABORT_PROCESS_MODE").unwrap() == "refuse" {
        assert!(result.is_err());
        assert_eq!(
            callbacks.get(),
            0,
            "empty terminal must refuse before observer/checkpoint"
        );
    } else {
        assert_eq!(result, Ok(Outcome::AbortedStillFenced));
    }
}

fn launch(elf: &FrozenElf, root: &Path, mode: &str, quarantine: &Quarantine) -> OwnedProcess {
    assert!(!quarantine.get(), "no commands after uncertainty");
    elf.recheck();
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(format!("stderr-{mode}-{}.log", nonce())))
        .unwrap();
    let child = Command::new(&elf.path)
        .args([
            "--exact",
            WORKER,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("OMAVLESS_ABORT_PROCESS_ROOT", root)
        .env("OMAVLESS_ABORT_PROCESS_MODE", mode)
        .env("OMAVLESS_HOME", root.join("home"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .env_remove("OMAVLESS_MIHOMO")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log)
        .spawn()
        .unwrap();
    let mut child = OwnedProcess::new(child, quarantine);
    elf.recheck();
    child.stdin().write_all(PASS).unwrap();
    child
}

struct FrozenElf {
    path: PathBuf,
    file: File,
    metadata: Metadata,
    parent: File,
    parent_metadata: Metadata,
    executed: File,
    expected: String,
}

fn normalized_host_build_path(value: &std::ffi::OsStr) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    let bytes = value.as_encoded_bytes();
    if !path.is_absolute()
        || bytes.len() > 4096
        || bytes.contains(&0)
        || bytes.windows(2).any(|pair| pair == b"//")
        || bytes.ends_with(b"/")
        || bytes
            .split(|b| *b == b'/')
            .any(|part| part == b"." || part == b"..")
    {
        return None;
    }
    Some(path)
}

#[test]
fn original_host_build_path_is_lexical_not_a_fabricated_guest_directory() {
    assert_eq!(
        normalized_host_build_path("/host/private/cargo-build".as_ref()),
        Some(PathBuf::from("/host/private/cargo-build"))
    );
    for path in [
        "relative", "/a/../b", "/a/./b", "/a//b", "/a/", "/", "/a\0b",
    ] {
        assert!(normalized_host_build_path(path.as_ref()).is_none());
    }
}

impl FrozenElf {
    fn capture() -> Self {
        let elf = fs::canonicalize(
            std::env::var_os("OMAVLESS_ABORT_FROZEN_ELF").expect("explicit frozen ELF"),
        )
        .unwrap();
        assert_eq!(
            fs::canonicalize(std::env::current_exe().unwrap()).unwrap(),
            elf
        );
        assert!(!elf.components().any(|c| c.as_os_str() == "target"));
        // Host's canonical build path is attested by the separately reviewed
        // sealed-copy receipt, not a claim that a guest build directory exists.
        let build_target = normalized_host_build_path(
            &std::env::var_os("OMAVLESS_ABORT_BUILD_TARGET")
                .expect("explicit original host build provenance"),
        )
        .unwrap();
        assert!(
            !elf.starts_with(build_target),
            "worker executable must be frozen outside Cargo"
        );
        let metadata = fs::symlink_metadata(&elf).unwrap();
        for (name, actual) in [
            ("OMAVLESS_ABORT_EXPECTED_DEVICE", metadata.dev()),
            ("OMAVLESS_ABORT_EXPECTED_INODE", metadata.ino()),
        ] {
            let expected: u64 = std::env::var(name).unwrap().parse().unwrap();
            assert_eq!(actual, expected, "original outer-guard executable identity");
        }
        assert!(
            metadata.is_file()
                && metadata.uid() == Uid::current().as_raw()
                && metadata.nlink() == 1
                && metadata.mode() & 0o7777 == 0o500
        );
        let expected = std::env::var("OMAVLESS_ABORT_FROZEN_SHA256").unwrap();
        assert!(metadata.len() > 4 && metadata.len() <= 1024 * 1024 * 1024);
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
            .open(&elf)
            .unwrap();
        assert!(same_member(&metadata, &file.metadata().unwrap()));
        let parent =
            open_private_directory(elf.parent().unwrap(), Uid::current().as_raw()).unwrap();
        let parent_metadata = parent.metadata().unwrap();
        // This descriptor names the actual executed inode, not a same-byte pathname.
        let executed = File::open("/proc/self/exe").unwrap();
        let frozen = Self {
            path: elf,
            file,
            metadata,
            parent,
            parent_metadata,
            executed,
            expected,
        };
        frozen.recheck();
        frozen
    }
    fn recheck(&self) {
        let check = || {
            let current = OpenOptions::new()
                .read(true)
                .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
                .open(&self.path)
                .unwrap();
            let parent =
                open_private_directory(self.path.parent().unwrap(), Uid::current().as_raw())
                    .unwrap();
            assert!(same_directory(
                &self.parent_metadata,
                &parent.metadata().unwrap()
            ));
            assert!(same_directory(
                &self.parent_metadata,
                &self.parent.metadata().unwrap()
            ));
            for file in [&self.file, &self.executed, &current] {
                let metadata = file.metadata().unwrap();
                assert!(same_member(&self.metadata, &metadata));
                assert_eq!(self.metadata.gid(), metadata.gid());
            }
        };
        check();
        let mut digest = Sha256::new();
        let mut offset = 0;
        let mut buffer = [0; 65536];
        while offset < self.metadata.len() {
            let count = self.file.read_at(&mut buffer, offset).unwrap();
            assert!(count > 0 && offset + count as u64 <= self.metadata.len());
            if offset == 0 {
                assert!(buffer[..count].starts_with(b"\x7fELF"));
            }
            digest.update(&buffer[..count]);
            offset += count as u64;
        }
        assert_eq!(self.file.read_at(&mut buffer[..1], offset).unwrap(), 0);
        assert_eq!(format!("{:x}", digest.finalize()), self.expected);
        check();
    }
}

#[test]
fn frozen_original_fd_refuses_same_bytes_new_inode_or_executed_identity() {
    for fault in 0..4 {
        let root = tempfile::Builder::new()
            .prefix("ov-abort-elf-unit-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.path().join("frozen");
        let bytes = b"\x7fELFsynthetic source-only bytes, never executed";
        private_create(&path, bytes);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
        let file = File::open(&path).unwrap();
        let parent = open_private_directory(root.path(), Uid::current().as_raw()).unwrap();
        let mut frozen = FrozenElf {
            path: path.clone(),
            metadata: file.metadata().unwrap(),
            executed: file.try_clone().unwrap(),
            file,
            parent_metadata: parent.metadata().unwrap(),
            parent,
            expected: format!("{:x}", Sha256::digest(bytes)),
        };
        frozen.recheck();
        let replacement = root.path().join("replacement");
        private_create(&replacement, bytes);
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o500)).unwrap();
        match fault {
            0 => fs::rename(replacement, &path).unwrap(),
            1 => frozen.executed = File::open(replacement).unwrap(),
            2 => fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap(),
            3 => frozen.expected = "0".repeat(64),
            _ => unreachable!(),
        }
        assert!(std::panic::catch_unwind(|| frozen.recheck()).is_err());
    }
}

fn success(status: WaitStatus) -> bool {
    matches!(status, WaitStatus::Exited(_, 0))
}

#[test]
#[ignore = "requires root-reviewed frozen executable; process loss, not power loss"]
fn fixed_current_process_loss_and_fresh_reentry() {
    require_unprivileged_process();
    let elf = FrozenElf::capture();
    eprintln!(
        "matrix-executed-identity={}:{}",
        elf.metadata.dev(),
        elf.metadata.ino()
    );
    let quarantine = std::rc::Rc::new(std::cell::Cell::new(false));
    for case in CASES {
        assert!(!quarantine.get());
        let root = fixture(&elf.path, case);
        eprintln!("private-fixture-root={}", root.display());
        let archive_before = fs::metadata(root.join("archive.ovb")).unwrap();
        let archive_bytes = fs::read(root.join("archive.ovb")).unwrap();
        let mut child = launch(&elf, &root, case, &quarantine);
        let stdout = child.stdout();
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            // libtest headings precede the bounded fixture token.
            for line in BufReader::new(stdout.take(16384)).lines().take(16) {
                let line = line.unwrap();
                if line.contains(TOKEN) {
                    let _ = tx.send(());
                    return;
                }
                assert!(line.len() <= 1024);
            }
        });
        let reached = rx.recv_timeout(Duration::from_secs(45)).is_ok();
        let disk_matches = reached
            && self::reached(
                &root,
                case,
                if case == "final" {
                    Point::Final
                } else {
                    Point::Gate
                },
            );
        if !reached || !disk_matches {
            quarantine.set(true);
            panic!("checkpoint uncertainty; preserve child and artifacts without further calls");
        }
        // Only the exact child we spawned is signalled; preserve every artifact.
        child
            .kill_live()
            .expect("fresh nonreaped live ownership required before signal");
        let status = child
            .finish()
            .expect("exact observed and reaped raw status required");
        reader.join().unwrap();
        assert!(
            reached
                && disk_matches
                && matches!(status, WaitStatus::Signaled(_, Signal::SIGKILL, false)),
            "checkpoint not reached or wrong death: {case}"
        );
        let (_, paths, _) = fixture_paths(&root, Uid::current().as_raw());
        let terminal = paths.state_directory.join("restore-decision.terminal");
        let terminal_before = fs::metadata(&terminal).ok();
        let recover = launch(
            &elf,
            &root,
            if case == "empty" { "refuse" } else { "recover" },
            &quarantine,
        )
        .finish()
        .unwrap();
        assert!(success(recover), "fresh recovery failed: {case}");
        assert!(same_member(
            &archive_before,
            &fs::metadata(root.join("archive.ovb")).unwrap()
        ));
        assert_eq!(archive_bytes, fs::read(root.join("archive.ovb")).unwrap());
        if case == "empty" {
            let before = terminal_before.unwrap();
            assert_eq!(before.len(), 0);
            assert!(same_member(&before, &fs::metadata(&terminal).unwrap()));
        } else {
            if let Some(before) = terminal_before {
                assert!(same_member(&before, &fs::metadata(&terminal).unwrap()));
            }
            let config = root.join("home/.config/omavless");
            assert_eq!(fs::read(config.join("profiles.json")).unwrap(), OLD[0]);
            assert_eq!(
                fs::read(config.join("route-template.yaml")).unwrap(),
                OLD[1]
            );
            let before = fs::metadata(&terminal).unwrap();
            assert!(success(
                launch(&elf, &root, "recover", &quarantine)
                    .finish()
                    .unwrap()
            ));
            assert!(same_member(&before, &fs::metadata(&terminal).unwrap()));
        }
        let lock = MigrationLock::acquire_existing(&paths, Uid::current().as_raw()).unwrap();
        assert!(check_startup_receipt(&paths, Uid::current().as_raw(), &lock, Some(2)).is_err());
        assert!(crate::pending_private_transaction::pending_at(
            &paths.state_directory
        ));
        private_create(
            &root.join("result.json"),
            format!(
                "{{\"schema\":1,\"case\":\"{case}\",\"signal\":9,\"reentry\":\"{}\"}}\n",
                if case == "empty" {
                    "refused-preserved"
                } else {
                    "aborted-still-fenced"
                }
            )
            .as_bytes(),
        );
        // No automatic cleanup; parent/root may review the private case roots.
    }
    elf.recheck();
    require_unprivileged_process();
    eprintln!(
        "matrix-completed-identity={}:{}",
        elf.metadata.dev(),
        elf.metadata.ino()
    );
}
