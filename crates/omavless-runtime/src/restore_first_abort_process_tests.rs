// SPDX-License-Identifier: MIT
//! Explicitly ignored process-loss acceptance. Run only a reviewed frozen ELF.
//! No environment-triggered fault or worker exists in a production build.
use super::*;
use crate::desired::{DesiredState, write_desired};
use crate::restore_executor_candidate::{EffectStep, execute_with_hook};
use crate::restore_successor_publication_candidate::tests::{OLD, backup};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PASS: &[u8] = b"synthetic first Abort process fixture";
const WORKER: &str = "production_owner::first_abort::process_reentry::process_worker";
const TOKEN: &str = "OVABORT-CHECKPOINT-v1";
const CASES: [&str; 5] = ["linked", "mixed", "empty", "full", "final"];

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
    frozen_elf();
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

fn launch(elf: &Path, root: &Path, mode: &str) -> Child {
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(format!("stderr-{mode}-{}.log", nonce())))
        .unwrap();
    let mut child = Command::new(elf)
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
    child.stdin.take().unwrap().write_all(PASS).unwrap();
    child
}

fn frozen_elf() -> PathBuf {
    let elf = fs::canonicalize(
        std::env::var_os("OMAVLESS_ABORT_FROZEN_ELF").expect("explicit frozen ELF"),
    )
    .unwrap();
    assert_eq!(
        fs::canonicalize(std::env::current_exe().unwrap()).unwrap(),
        elf
    );
    assert!(!elf.components().any(|c| c.as_os_str() == "target"));
    let build_target = fs::canonicalize(
        std::env::var_os("OMAVLESS_ABORT_BUILD_TARGET").expect("explicit build target provenance"),
    )
    .unwrap();
    assert!(
        !elf.starts_with(build_target),
        "worker executable must be frozen outside Cargo"
    );
    let metadata = fs::symlink_metadata(&elf).unwrap();
    assert!(
        metadata.is_file()
            && metadata.uid() == Uid::current().as_raw()
            && metadata.nlink() == 1
            && metadata.mode() & 0o7777 == 0o700
    );
    let expected = std::env::var("OMAVLESS_ABORT_FROZEN_SHA256").unwrap();
    assert!(metadata.len() > 4 && metadata.len() <= 1024 * 1024 * 1024);
    let mut file = File::open(&elf).unwrap();
    let mut magic = [0; 4];
    file.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"\x7fELF");
    let mut digest = Sha256::new();
    digest.update(magic);
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    assert_eq!(format!("{:x}", digest.finalize()), expected);
    elf
}

fn wait_bounded(mut child: Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("re-entry exceeded bound; exact child killed, artifacts preserved");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "requires root-reviewed frozen executable; process loss, not power loss"]
fn fixed_current_process_loss_and_fresh_reentry() {
    let elf = frozen_elf();
    for case in CASES {
        let root = fixture(&elf, case);
        eprintln!("private-fixture-root={}", root.display());
        let archive_before = fs::metadata(root.join("archive.ovb")).unwrap();
        let archive_bytes = fs::read(root.join("archive.ovb")).unwrap();
        let mut child = launch(&elf, &root, case);
        let stdout = child.stdout.take().unwrap();
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
        // Only the exact child we spawned is signalled; preserve every artifact.
        let killed = child.kill();
        let status = child.wait().unwrap();
        reader.join().unwrap();
        assert!(
            reached && disk_matches && killed.is_ok() && status.signal() == Some(9),
            "checkpoint not reached or wrong death: {case}"
        );
        let (_, paths, _) = fixture_paths(&root, Uid::current().as_raw());
        let terminal = paths.state_directory.join("restore-decision.terminal");
        let terminal_before = fs::metadata(&terminal).ok();
        let recover = wait_bounded(launch(
            &elf,
            &root,
            if case == "empty" { "refuse" } else { "recover" },
        ));
        assert!(recover.success(), "fresh recovery failed: {case}");
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
            assert!(wait_bounded(launch(&elf, &root, "recover")).success());
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
}
