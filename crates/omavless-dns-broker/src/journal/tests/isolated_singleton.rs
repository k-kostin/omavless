//! Isolate close/reacquire from unrelated parallel fork-before-exec children.
//! This does not identify the historical failure or weaken production flock.
use super::*;
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid, waitpid};
use nix::unistd::Pid;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WORKER: &str = "journal::tests::isolated_singleton::singleton_worker";
const ROOT_ENV: &str = "OMAVLESS_JOURNAL_SINGLETON_TEST_ROOT";

fn same(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    (
        left.dev(),
        left.ino(),
        left.uid(),
        left.gid(),
        left.mode(),
        left.nlink(),
    ) == (
        right.dev(),
        right.ino(),
        right.uid(),
        right.gid(),
        right.mode(),
        right.nlink(),
    )
}

fn prepare_journal_root(root: &std::path::Path) {
    let journal_root = root.join("journal");
    std::fs::create_dir(&journal_root).unwrap();
    std::fs::set_permissions(&journal_root, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn terminal(status: WaitStatus, pid: Pid) -> Result<Option<WaitStatus>, &'static str> {
    match status {
        WaitStatus::StillAlive => Ok(None),
        WaitStatus::Exited(actual, _) | WaitStatus::Signaled(actual, _, _) if actual == pid => {
            Ok(Some(status))
        }
        _ => Err("unknown child status: preserve, no signal/reap/retry"),
    }
}

#[allow(clippy::zombie_processes)] // Only exact raw WNOWAIT followed by matching waitpid.
pub(super) fn run() {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    let root = tempfile::Builder::new()
        .prefix("ov-journal-singleton-")
        .tempdir_in(&home)
        .unwrap()
        .keep();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    // Declared child-directory creation legitimately changes parent nlink on
    // filesystems that count subdirectories. Finish it BEFORE the strict pin.
    prepare_journal_root(&root);
    let directory = open_directory(&root, rustix::process::geteuid().as_raw(), true).unwrap();
    let original = std::fs::symlink_metadata(&root).unwrap();
    let executable = File::open("/proc/self/exe").unwrap();
    let exe = executable.metadata().unwrap();
    let executable_path = std::env::current_exe().unwrap();
    assert!(executable_path.starts_with(&home));
    assert!(exe.is_file() && exe.uid() == rustix::process::geteuid().as_raw());
    assert!(same(
        &exe,
        &std::fs::symlink_metadata(&executable_path).unwrap()
    ));
    let log = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(OFlags::NOFOLLOW.bits() as i32)
        .open(root.join("worker.log"))
        .unwrap();
    let child = Command::new(format!("/proc/self/fd/{}", executable.as_raw_fd()))
        .args([
            "--exact",
            WORKER,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("HOME", &home)
        .env(ROOT_ENV, &root)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let pid = Pid::from_raw(i32::try_from(child.id()).unwrap());
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            Instant::now() < deadline,
            "unknown timeout: preserve worker and artifacts"
        );
        let observed = terminal(
            waitid(
                Id::Pid(pid),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT | WaitPidFlag::WNOHANG,
            )
            .unwrap(),
            pid,
        )
        .unwrap();
        if let Some(observed) = observed {
            let reaped = waitpid(pid, Some(WaitPidFlag::WNOHANG)).unwrap();
            assert_eq!(reaped, observed, "uncertain completion: preserve artifacts");
            assert_eq!(
                observed,
                WaitStatus::Exited(pid, 0),
                "known failure: retained private worker.log"
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(same(&exe, &executable.metadata().unwrap()));
    assert!(same(
        &exe,
        &std::fs::symlink_metadata(executable_path).unwrap()
    ));
    assert!(same(&original, &std::fs::symlink_metadata(&root).unwrap()));
    assert_eq!(fs::fstat(&directory).unwrap().st_ino, original.ino());
    // Only known success permits removal of this exact synthetic test root.
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "internal single-thread singleton lifecycle worker; parent owns exact child"]
fn singleton_worker() {
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        [
            "--exact",
            WORKER,
            "--ignored",
            "--nocapture",
            "--test-threads=1"
        ]
    );
    let root = std::path::PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    let journal_root = root.join("journal");
    let owner = rustix::process::geteuid().as_raw();
    let existing = std::fs::symlink_metadata(&journal_root).unwrap();
    assert!(
        existing.is_dir()
            && existing.uid() == owner
            && existing.gid() == rustix::process::getegid().as_raw()
            && existing.mode() & 0o7777 == 0o700
    );
    let open = || Journal::open_at(&journal_root, BOOT.to_owned(), owner);
    let journal = open().unwrap();
    assert_eq!(open().unwrap_err(), Error::Refused);
    drop(journal);
    let reopened = open();
    if let Err(error) = &reopened {
        // Diagnostic-only independent attempt, never a retry/admission fallback.
        eprintln!("singleton reopen error={error:?}");
        match open_directory(&journal_root, owner, true) {
            Ok(fd) => {
                let diagnostic = fs::flock(&fd, fs::FlockOperation::NonBlockingLockExclusive);
                eprintln!(
                    "independent flock errno={:?}",
                    diagnostic.err().map(|errno| errno.raw_os_error())
                );
            }
            Err(error) => eprintln!("independent directory open error={error:?}"),
        }
    }
    assert!(
        reopened.is_ok(),
        "singleton lifecycle failed; typed diagnostic retained"
    );
}

#[test]
fn predeclared_child_directory_keeps_strict_parent_pin_through_journal_use() {
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    prepare_journal_root(root.path());
    let original = std::fs::symlink_metadata(root.path()).unwrap();
    let directory = open_directory(root.path(), rustix::process::geteuid().as_raw(), true).unwrap();
    let journal = Journal::open_at(
        &root.path().join("journal"),
        BOOT.to_owned(),
        rustix::process::geteuid().as_raw(),
    )
    .unwrap();
    drop(journal);
    assert!(same(
        &original,
        &std::fs::symlink_metadata(root.path()).unwrap()
    ));
    assert_eq!(fs::fstat(&directory).unwrap().st_ino, original.ino());
}

#[test]
fn unknown_and_wrong_pid_never_authorize_reap() {
    let pid = Pid::from_raw(7);
    assert_eq!(terminal(WaitStatus::StillAlive, pid), Ok(None));
    assert!(terminal(WaitStatus::Exited(Pid::from_raw(8), 0), pid).is_err());
    assert!(terminal(WaitStatus::Continued(pid), pid).is_err());
    assert_eq!(
        terminal(WaitStatus::Exited(pid, 7), pid),
        Ok(Some(WaitStatus::Exited(pid, 7)))
    );
}
