// SPDX-License-Identifier: MIT
//! Synthetic actual native IO + fresh authenticated Preview, not installed
//! current-owner admission. LocalGate and login remain synthetic observations.
use super::*;
use nix::sys::signal::Signal;
use nix::sys::wait::WaitStatus;
use std::cell::Cell;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

// One shared guard, also used by the first-Abort suite; no parent signal here.
use crate::restore_abort_process_support_tests::{OwnedProcess, Quarantine};

const ROOT_ENV: &str = "OMAVLESS_NATIVE_HISTORY_TEST_ROOT";
const CUT_ENV: &str = "OMAVLESS_NATIVE_HISTORY_TEST_CUT";
const STABLE: [&str; 7] = [
    "c/profiles.json",
    "c/route-template.yaml",
    "s/ownership.json",
    "s/desired.json",
    "s/restore-disposition.history",
    "r/omavless-login.receipt",
    "r/omavless/owner.lock",
];

fn worker_name() -> String {
    format!("{}::worker", module_path!().split_once("::").unwrap().1)
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Stamp {
    identity: [u64; 7],
    times: [i64; 4],
}
fn stamp(m: &Metadata) -> Stamp {
    Stamp {
        identity: [
            m.dev(),
            m.ino(),
            u64::from(m.mode()),
            u64::from(m.uid()),
            u64::from(m.gid()),
            m.nlink(),
            m.len(),
        ],
        times: [m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()],
    }
}
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    relative: String,
    before: Stamp,
    bytes: Vec<u8>,
}
fn record(root: &Path, relative: &str) -> Record {
    let path = root.join(relative);
    let held = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(&path)
        .unwrap();
    let metadata = held.metadata().unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.uid(), nix::unistd::getuid().as_raw());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    assert!(metadata.len() <= 1024 * 1024);
    let mut bytes = vec![0; metadata.len() as usize];
    held.read_exact_at(&mut bytes, 0).unwrap();
    assert_eq!(held.read_at(&mut [0; 1], metadata.len()).unwrap(), 0);
    assert_eq!(stamp(&metadata), stamp(&held.metadata().unwrap()));
    assert_eq!(
        stamp(&metadata),
        stamp(&fs::symlink_metadata(path).unwrap())
    );
    Record {
        relative: relative.to_owned(),
        before: stamp(&metadata),
        bytes,
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Baseline {
    root: PathBuf,
    root_identity: [u64; 7],
    stable: Vec<Record>,
    complete: Record,
}
fn paths(root: &Path) -> crate::cutover::CutoverPaths {
    let runtime = root.join("r");
    crate::cutover::CutoverPaths {
        runtime_base: runtime.clone(),
        operation_lock: runtime.join(format!("omavless.{}.lock", nix::unistd::getuid().as_raw())),
        state_directory: root.join("s"),
        ownership_marker: root.join("s/ownership.json"),
    }
}
fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}
fn catalogue(root: &Path) -> Vec<(String, Stamp, Vec<String>)> {
    ["c", "s", "r", "r/omavless"]
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            (
                name.to_owned(),
                stamp(&fs::symlink_metadata(&path).unwrap()),
                names(&path),
            )
        })
        .collect()
}
fn complete_relative() -> String {
    format!(
        "s/{}",
        crate::restore_disposition_complete_model::COMPLETE_MEMBER
    )
}

#[test]
fn worker() {
    let Some(value) = std::env::var_os(CUT_ENV) else {
        return;
    };
    let value = value.to_str().expect("closed test selector");
    let selected = match value {
        "before-rename" => Some(NativeStep::HistoryRenamePrepared),
        "after-rename" => Some(NativeStep::HistoryRenamed),
        "after-sync" => Some(NativeStep::HistorySynced),
        "normal" => None,
        _ => panic!("closed test selector"),
    };
    let control = PathBuf::from(std::env::var_os(ROOT_ENV).expect("test control root"));
    let parent = crate::test_temp::home_parent().unwrap();
    assert_eq!(control.parent(), Some(parent.as_path()));
    assert!(
        control
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("ovt-history-loss-")
    );
    let metadata = fs::symlink_metadata(&control).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), nix::unistd::getuid().as_raw());
    assert_eq!(metadata.mode() & 0o777, 0o700);
    assert!(names(&control).is_empty());
    let _watchdog = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(30));
        std::process::exit(124); // failure only, never a checkpoint result
    });
    let backup = crate::restore_executor_candidate::successor::tests::second();
    let new = backup.restore_store_off().unwrap();
    let mut f = Fixture::from_members(
        "authenticated-preview",
        true,
        [MEMBERS[0], MEMBERS[1], &new, backup.template()],
    )
    .with_complete(NewCompletePresence::CompleteOnly);
    let _past = prior_audits(&f.state, 1);
    let p = paths(&f.root);
    let uid = nix::unistd::getuid().as_raw();
    let lease = crate::cutover::MigrationLock::acquire(&p, uid).unwrap();
    assert_eq!(
        crate::restore_recovery_preview::inspect_authenticated(
            &f.config, &p, uid, &lease, backup, [42; 32]
        )
        .unwrap()
        .choice(),
        crate::restore_recovery_preview::Choice::NewComplete
    );
    let baseline = Baseline {
        root: f.root.clone(),
        root_identity: stamp(&fs::symlink_metadata(&f.root).unwrap()).identity,
        stable: STABLE
            .into_iter()
            .map(|name| record(&f.root, name))
            .collect(),
        complete: record(&f.root, &complete_relative()),
    };
    drop(write(
        &control.join("baseline.json"),
        &serde_json::to_vec(&baseline).unwrap(),
    ));
    f.capture("positive").unwrap();
    f.finish_with(&mut |event| {
        if selected == Some(event) {
            let witness = match event {
                NativeStep::HistoryRenamePrepared => b"before-rename".as_slice(),
                NativeStep::HistoryRenamed => b"after-rename".as_slice(),
                NativeStep::HistorySynced => b"after-sync".as_slice(),
                _ => panic!("unexpected selected checkpoint"),
            };
            drop(write(&control.join("reached"), witness));
            nix::sys::signal::kill(nix::unistd::Pid::this(), Signal::SIGKILL).unwrap();
            panic!("self-termination returned");
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(
        selected, None,
        "selected checkpoint must terminate original child"
    );
    assert!(f.engine.disposition_ready());
    drop(write(&control.join("reached"), b"normal"));
    drop(lease);
    drop(f); // parent owns this wholly synthetic root after original settlement
}

#[test]
fn native_loss_before_history_rename_preserves_same_complete_and_fresh_preview() {
    verify_case("before-rename");
}

#[test]
fn native_history_normal_completion_is_data_not_recovery_authority() {
    verify_case("normal");
}

#[test]
fn native_loss_after_history_sync_retains_data_but_not_recovery_authority() {
    verify_case("after-sync");
}

#[test]
fn native_loss_after_history_rename_retains_data_but_not_recovery_authority() {
    verify_case("after-rename");
}

#[test]
fn native_history_worker_default_and_unknown_selector_create_no_data() {
    let quarantine: Quarantine = Rc::new(Cell::new(false));
    for selected in [None, Some("not-a-cut")] {
        let control = crate::test_temp::directory_under(
            &crate::test_temp::home_parent().unwrap(),
            "history-loss",
        )
        .unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg(worker_name())
            .env(ROOT_ENV, &control)
            .env_remove(CUT_ENV)
            .stdin(Stdio::null());
        if let Some(selected) = selected {
            command.env(CUT_ENV, selected);
        }
        let child = command.spawn().unwrap();
        let pid = nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap());
        let mut child = OwnedProcess::new(child, &quarantine);
        let status = child
            .finish()
            .expect("unsettled original worker; retain fixture");
        assert_eq!(
            status,
            WaitStatus::Exited(pid, if selected.is_none() { 0 } else { 101 })
        );
        assert!(!quarantine.get());
        assert!(names(&control).is_empty());
        fs::remove_dir(control).unwrap();
    }
}

fn verify_case(selected: &str) {
    let parent = crate::test_temp::home_parent().unwrap();
    let control = crate::test_temp::directory_under(&parent, "history-loss").unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(worker_name())
        .arg("--nocapture")
        .env(ROOT_ENV, &control)
        .env(CUT_ENV, selected)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let pid = nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap());
    let quarantine: Quarantine = Rc::new(Cell::new(false));
    let mut child = OwnedProcess::new(child, &quarantine);
    let status = child
        .finish()
        .expect("unsettled original worker; retain fixture");
    if selected == "normal" {
        assert_eq!(status, WaitStatus::Exited(pid, 0));
    } else {
        assert_eq!(status, WaitStatus::Signaled(pid, Signal::SIGKILL, false));
    }
    assert!(!quarantine.get());
    assert_eq!(
        fs::read(control.join("reached")).unwrap(),
        selected.as_bytes()
    );
    let raw = fs::read(control.join("baseline.json")).unwrap();
    assert!(raw.len() < 1024 * 1024);
    let baseline: Baseline = serde_json::from_slice(&raw).unwrap();
    assert_eq!(baseline.root.parent(), Some(parent.as_path()));
    assert!(
        baseline
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(&format!("ovt-final-old-{}-", pid.as_raw()))
    );
    assert_eq!(
        stamp(&fs::symlink_metadata(&baseline.root).unwrap()).identity,
        baseline.root_identity
    );
    assert_eq!(
        baseline
            .stable
            .iter()
            .map(|r| r.relative.as_str())
            .collect::<Vec<_>>(),
        STABLE
    );
    assert_eq!(baseline.complete.relative, complete_relative());
    for original in &baseline.stable {
        assert_eq!(&record(&baseline.root, &original.relative), original);
    }
    let before_rename = selected == "before-rename";
    let observed_name = if before_rename {
        complete_relative()
    } else {
        format!("s/{}", native_history::NAMES[1])
    };
    let observed = record(&baseline.root, &observed_name);
    assert_eq!(observed.before.identity, baseline.complete.before.identity);
    assert_eq!(observed.bytes, baseline.complete.bytes);
    if before_rename {
        assert_eq!(observed.before, baseline.complete.before);
    }
    let absent = if before_rename {
        format!("s/{}", native_history::NAMES[1])
    } else {
        complete_relative()
    };
    assert!(matches!(fs::symlink_metadata(baseline.root.join(absent)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound));
    let mut expected = vec![
        "desired.json".to_owned(),
        "ownership.json".to_owned(),
        native_history::NAMES[0].to_owned(),
        Path::new(&observed_name)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned(),
    ];
    expected.sort();
    assert_eq!(names(&baseline.root.join("s")), expected);
    assert_eq!(
        crate::pending_private_transaction::pending_at(&baseline.root.join("s")),
        before_rename
    );
    let p = paths(&baseline.root);
    let lease = crate::cutover::MigrationLock::acquire(&p, nix::unistd::getuid().as_raw()).unwrap();
    let before = catalogue(&baseline.root);
    let preview = crate::restore_recovery_preview::inspect_authenticated(
        &baseline.root.join("c"),
        &p,
        nix::unistd::getuid().as_raw(),
        &lease,
        crate::restore_executor_candidate::successor::tests::second(),
        [42; 32],
    );
    if before_rename {
        assert_eq!(
            preview.unwrap().choice(),
            crate::restore_recovery_preview::Choice::NewComplete
        );
    } else {
        assert!(matches!(
            preview,
            Err(crate::restore_recovery_preview::Error::Unavailable)
        ));
    }
    assert_eq!(catalogue(&baseline.root), before);
    for original in &baseline.stable {
        assert_eq!(&record(&baseline.root, &original.relative), original);
    }
    assert_eq!(record(&baseline.root, &observed_name), observed);
    drop(lease);
    fs::remove_dir_all(&baseline.root).unwrap(); // verified completed synthetic fixture only
    fs::remove_dir_all(control).unwrap();
}
