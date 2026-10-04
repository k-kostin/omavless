// SPDX-License-Identifier: MIT
//! Actual cached RuntimeServer negative. No manager-query fixture or absence claim.

use super::*;
use nix::unistd::Uid;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const WORKER: &str = "restore_abort_cli::stopped_owner::cached_owner_tests::cached_owner_worker";
const ROOT_ENV: &str = "OMAVLESS_ABORT_CACHED_OWNER_TEST_ROOT";

fn hash(file: &mut File) -> [u8; 32] {
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 32768];
    loop {
        let size = file.read(&mut buffer).unwrap();
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    digest.finalize().into()
}

fn freeze(root: &Path) -> File {
    let uid = Uid::current().as_raw();
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let source_path = std::env::current_exe().unwrap().canonicalize().unwrap();
    assert!(
        source_path.starts_with(&home),
        "source executable must remain in HOME"
    );
    for ancestor in source_path.parent().unwrap().ancestors() {
        let metadata = fs::symlink_metadata(ancestor).unwrap();
        assert!(
            metadata.is_dir() && [0, uid].contains(&metadata.uid()) && metadata.mode() & 0o022 == 0
        );
    }
    let mut source = File::open("/proc/self/exe").unwrap();
    let before = source.metadata().unwrap();
    assert!(
        before.is_file()
            && before.uid() == uid
            && before.nlink() == 1
            && before.mode() & 0o6022 == 0
            && before.len() > 0
            && before.len() <= 512 * 1024 * 1024
    );
    assert!(executable_identity(
        &before,
        &fs::symlink_metadata(&source_path).unwrap()
    ));
    let source_hash = hash(&mut source);
    source.seek(SeekFrom::Start(0)).unwrap();
    let frozen_path = root.join("omavless-cached-owner");
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o500)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(&frozen_path)
        .unwrap();
    assert_eq!(
        std::io::copy(&mut (&mut source).take(before.len() + 1), &mut writer).unwrap(),
        before.len()
    );
    writer.sync_all().unwrap();
    drop(writer);
    let mut frozen = OpenOptions::new()
        .read(true)
        .custom_flags(OFlag::O_NOFOLLOW.bits())
        .open(&frozen_path)
        .unwrap();
    let sealed = frozen.metadata().unwrap();
    assert!(
        sealed.is_file()
            && sealed.uid() == uid
            && sealed.nlink() == 1
            && sealed.mode() & 0o7777 == 0o500
    );
    assert_eq!(hash(&mut frozen), source_hash);
    assert_eq!(hash(&mut source), source_hash);
    assert!(executable_identity(&before, &source.metadata().unwrap()));
    assert!(executable_identity(
        &before,
        &fs::symlink_metadata(source_path).unwrap()
    ));
    assert!(executable_identity(
        &sealed,
        &fs::symlink_metadata(frozen_path).unwrap()
    ));
    frozen
}

fn alive(pid: Pid) {
    assert_eq!(
        waitid(
            Id::Pid(pid),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT
        )
        .unwrap(),
        WaitStatus::StillAlive,
        "unknown/non-live child: preserve artifacts, no retry or signal"
    );
}

fn token(stdout: &mut impl Read, pid: Pid, expected: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    while bytes.len() < expected.len() {
        assert!(
            Instant::now() < deadline,
            "unknown timeout: preserve, no cleanup"
        );
        alive(pid);
        let mut byte = [0_u8; 1];
        match stdout.read(&mut byte) {
            Ok(1) => bytes.push(byte[0]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1))
            }
            _ => panic!("unknown worker response: preserve, no followup"),
        }
    }
    assert!(
        bytes == expected,
        "malformed worker response: preserve, no followup"
    );
}

#[test]
#[ignore = "internal fixed cached-owner child, only the owning source test starts it"]
fn cached_owner_worker() {
    let root = std::path::PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    let paths = crate::RuntimePaths::below(&root);
    let server = crate::RuntimeServer::bind(paths.clone()).unwrap();
    std::io::stdout().write_all(b"ready\n").unwrap();
    std::io::stdout().flush().unwrap();
    let mut request = [0_u8; 1];
    std::io::stdin().read_exact(&mut request).unwrap();
    assert_eq!(request, [b'v']);
    let cached = server._owner._file.metadata().unwrap();
    assert_eq!(cached.nlink(), 0);
    assert_ne!(cached.ino(), fs::metadata(&paths.owner_lock).unwrap().ino());
    assert!(!paths.socket.exists());
    assert_eq!(
        server.listener.local_addr().unwrap().as_pathname(),
        Some(paths.socket.as_path())
    );
    std::io::stdout().write_all(b"retained\n").unwrap();
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_exact(&mut request).unwrap();
    assert_eq!(request, [b'f']);
    drop(server);
}

#[test]
// Known exact raw waitid/waitpid is intentional; never hidden Child::wait/kill.
#[allow(clippy::zombie_processes)]
fn actual_cached_owner_with_both_names_removed_is_rejected_by_complete_inventory() {
    let uid = Uid::current().as_raw();
    let root = tempfile::Builder::new()
        .prefix("ov-cached-owner-")
        .tempdir_in(std::env::var_os("HOME").unwrap())
        .unwrap()
        .keep();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    // Keep the root on every uncertain/error path. Only known child completion
    // at the very end authorizes deletion of this exact synthetic root.
    let parent = crate::backup_source_candidate::open_private_directory(&root, uid).unwrap();
    let parent_identity = parent.metadata().unwrap();
    let frozen = freeze(&root);
    let frozen_identity = frozen.metadata().unwrap();
    let mut child = Command::new(format!("/proc/self/fd/{}", frozen.as_raw_fd()))
        .arg0("omavless-cached-owner")
        .args(["--exact", WORKER, "--ignored", "--nocapture", "--quiet"])
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap())
        .env(ROOT_ENV, &root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = Pid::from_raw(i32::try_from(child.id()).unwrap());
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    fcntl(&output, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
    // libtest prints this fixed heading before entering the selected worker.
    token(&mut output, pid, b"\nrunning 1 test\n");
    token(&mut output, pid, b"ready\n");
    let paths = crate::RuntimePaths::below(&root);
    fs::remove_file(&paths.owner_lock).unwrap();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&paths.owner_lock)
        .unwrap();
    fs::remove_file(&paths.socket).unwrap();
    alive(pid);
    input.write_all(b"v").unwrap();
    token(&mut output, pid, b"retained\n");
    // This reproduces the old vulnerability: the replacement flock itself is
    // available even while the actual server retains its old cached owner.
    let replacement_lock = super::super::StoppedRuntime::acquire(paths, uid).unwrap();
    let proc = File::open("/proc").unwrap();
    let myself = Process::capture(&proc, std::process::id(), &mut Budget::new()).unwrap();
    let captured = Process::capture(&proc, child.id(), &mut Budget::new()).unwrap();
    assert!(executable_identity(
        &frozen_identity,
        &captured.executable_identity
    ));
    let mut budget = Budget::new();
    let complete = pids(&proc, &budget).unwrap();
    assert!(complete.contains(&child.id()));
    // Only traversal order differs from production. The same actual proc
    // reader validates complete-set equality/count before examining any PID.
    let order = std::iter::once(child.id())
        .chain(complete.iter().copied().filter(|p| *p != child.id()))
        .collect();
    assert_eq!(
        inspect_inventory(&proc, uid, &myself, &mut budget, &complete, order),
        Err(InventoryError::KnownOwner(child.id()))
    );
    alive(pid);
    input.write_all(b"f").unwrap();
    drop(input);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "unknown completion: preserve, no signal/reap"
        );
        let observed = terminal(
            waitid(
                Id::Pid(pid),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
            )
            .unwrap(),
            pid,
        )
        .unwrap();
        if let Some(status) = observed {
            known_reap(status, pid, || {
                waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(|_| ())
            })
            .unwrap();
            assert_eq!(status, WaitStatus::Exited(pid, 0));
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(identity(&parent_identity, &parent.metadata().unwrap()));
    assert!(identity(
        &parent_identity,
        &fs::symlink_metadata(&root).unwrap()
    ));
    assert!(executable_identity(
        &frozen_identity,
        &frozen.metadata().unwrap()
    ));
    assert!(executable_identity(
        &frozen_identity,
        &fs::symlink_metadata(root.join("omavless-cached-owner")).unwrap()
    ));
    drop(replacement_lock);
    fs::remove_dir_all(root).unwrap();
}
