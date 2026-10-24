// SPDX-License-Identifier: MIT
//! Real inherited descriptors, synthetic Read only; never constructs an owner.
use super::*;
use crate::restore_abort_process_support_tests::{OwnedProcess, Quarantine};
use nix::fcntl::OFlag;
use nix::sys::wait::WaitStatus;
use std::cell::Cell;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

const CASE: &str = "OMAVLESS_HISTORY_CHANNEL_WORKER";
fn worker_name() -> String {
    format!("{}::worker", module_path!().split_once("::").unwrap().1)
}

#[test]
fn worker() {
    let Some(case) = std::env::var_os(CASE) else {
        return;
    };
    let case = case.to_str().expect("closed channel selector");
    assert!(["ready", "blocking", "broken", "full", "missing", "file"].contains(&case));
    if case == "missing" {
        nix::unistd::close(2).unwrap();
    }
    let _watchdog = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(30));
        std::process::exit(124);
    });
    struct SentinelRead<'a>(&'a Cell<bool>);
    impl Read for SentinelRead<'_> {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            self.0.set(true);
            Err(std::io::Error::other("synthetic read boundary"))
        }
    }
    let read = Cell::new(false);
    let result = lose_new_complete_at_history_synced_from_private_input(SentinelRead(&read));
    if crate::product_scope::backup_only() {
        assert_eq!(result, Err(Error::Disabled));
        assert!(!read.get());
    } else if case == "ready" {
        assert_eq!(result, Err(Error::InvalidInput));
        assert!(read.get());
    } else {
        assert_eq!(result, Err(Error::RecoveryRefused));
        assert!(!read.get());
    }
    let developer_read = Cell::new(false);
    let args = [
        "developer",
        "lose-new-complete-at-history-synced",
        "--confirm-private-history-synced-process-loss",
    ]
    .map(OsString::from);
    assert_eq!(
        crate::developer_current_restore::from_private_input(&args, SentinelRead(&developer_read)),
        Err(if crate::product_scope::backup_only() {
            "developer_restore_disabled"
        } else {
            "developer_history_synced_process_loss_unknown_or_refused_do_not_repeat"
        })
    );
    assert_eq!(
        developer_read.get(),
        !crate::product_scope::backup_only() && case == "ready"
    );
}

#[test]
fn ready_nonblocking_witness_pipe_admits_only_the_bounded_input_parser() {
    verify_channel("ready");
}

#[test]
fn unsuitable_witness_channels_refuse_before_private_input_or_owner_effects() {
    for case in ["blocking", "broken", "full", "missing", "file"] {
        verify_channel(case);
    }
}

fn verify_channel(case: &str) {
    let flags = if case == "blocking" {
        OFlag::O_CLOEXEC
    } else {
        OFlag::O_NONBLOCK | OFlag::O_CLOEXEC
    };
    let (reader, writer) = nix::unistd::pipe2(flags).unwrap();
    if case == "full" {
        let mut full = false;
        for _ in 0..4096 {
            match nix::unistd::write(&writer, &[0; 4096]) {
                Ok(4096) => (),
                Err(nix::errno::Errno::EAGAIN) => {
                    full = true;
                    break;
                }
                _ => panic!("synthetic pipe fill refused"),
            }
        }
        assert!(full, "bounded fixture must reach full pipe");
    }
    let reader = if case == "broken" {
        drop(reader);
        None
    } else {
        Some(reader)
    };
    let file_root = (case == "file").then(|| {
        crate::test_temp::directory_under(
            &crate::test_temp::home_parent().unwrap(),
            "history-channel",
        )
        .unwrap()
    });
    let channel: std::os::fd::OwnedFd = if let Some(root) = &file_root {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("witness"))
            .unwrap();
        drop(writer);
        file.into()
    } else {
        writer
    };
    // Deliberately shared OFD: read-only admission must not change the parent's
    // flags, even when the child inherited a duplicate of this same description.
    let held_channel = nix::unistd::dup(&channel).unwrap();
    let original_flags = nix::fcntl::fcntl(&held_channel, nix::fcntl::FcntlArg::F_GETFL).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(worker_name())
        .arg("--test-threads=1")
        .env(CASE, case)
        .stdin(Stdio::null())
        .stderr(Stdio::from(channel));
    let child = command.spawn().unwrap();
    drop(command); // retain only the explicit read-only OFD comparison anchor
    let pid = nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap());
    let quarantine: Quarantine = Rc::new(Cell::new(false));
    let mut child = OwnedProcess::new(child, &quarantine);
    assert_eq!(child.finish(), Ok(WaitStatus::Exited(pid, 0)));
    assert!(!quarantine.get());
    assert_eq!(
        nix::fcntl::fcntl(&held_channel, nix::fcntl::FcntlArg::F_GETFL).unwrap(),
        original_flags
    );
    drop(held_channel);
    drop(reader); // only after original child settlement; no signal/retry
    if let Some(root) = file_root {
        assert_eq!(std::fs::metadata(root.join("witness")).unwrap().len(), 0);
        std::fs::remove_file(root.join("witness")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
