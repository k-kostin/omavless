// SPDX-License-Identifier: MIT

//! Process-crash fixture, compiled only by the parent test-only module.
//! The trusted, private temporary directory is not a production path adapter.

use super::*;
use crate::cutover::{CutoverPaths, MigrationLock};
use crate::network_transition_plan::OwnedState;
use omavless_store::{atomic_replace_private, read_private_utf8};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const FENCE: Fence = Fence {
    boot: [1; 16],
    owner_instance: [2; 16],
    owner_generation: 7,
    desired_revision: 12,
    network_epoch: 5,
};
const READY: Receipt = Receipt {
    schema: 1,
    fence: FENCE,
    phase: Phase::Ready,
};
const HINT: Hint = Hint {
    owner_generation: 7,
    desired_revision: 12,
    network_epoch: 5,
    last_hint_tick: 100,
};

#[derive(Clone, Copy)]
enum Stop {
    None,
    Error(u8),
    Crash(u8),
}

fn checkpoint(root: &Path, stop: Stop, boundary: u8) -> Result<(), Refused> {
    match stop {
        Stop::Error(n) if n == boundary => Err(Refused),
        Stop::Crash(n) if n == boundary => {
            // Parent kills this actual process after seeing the rendezvous.
            // No unwinding/destructors or cooperative recovery run in the child.
            fs::write(root.join("checkpoint"), [boundary]).unwrap();
            loop {
                std::thread::park();
            }
        }
        _ => Ok(()),
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ov-t4-receipt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        let fixture = Self(root);
        // Deliberate fixture seeding, never an initializer on missing load.
        atomic_replace_private(
            &fixture.0.join("receipt.json"),
            &serde_json::to_vec(&READY).unwrap(),
            nix::unistd::Uid::current().as_raw(),
        )
        .unwrap();
        fixture
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct Files {
    root: PathBuf,
    paths: CutoverPaths,
    lease: MigrationLock,
    stop: Stop,
}

impl Files {
    fn open(root: &Path, stop: Stop) -> Result<Self, Refused> {
        let uid = nix::unistd::Uid::current().as_raw();
        let metadata = fs::symlink_metadata(root).map_err(|_| Refused)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.permissions().mode() & 0o777 != 0o700
        {
            return Err(Refused);
        }
        let paths = CutoverPaths::below(root, root, uid);
        let lease = MigrationLock::acquire(&paths, uid).map_err(|_| Refused)?;
        Ok(Self {
            root: root.to_owned(),
            paths,
            lease,
            stop,
        })
    }

    fn read(&self) -> Result<Receipt, Refused> {
        let uid = nix::unistd::Uid::current().as_raw();
        if !self.lease.authorizes(&self.paths, uid) {
            return Err(Refused);
        }
        let path = self.root.join("receipt.json");
        let metadata = fs::symlink_metadata(&path).map_err(|_| Refused)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.len() > 1024
        {
            return Err(Refused);
        }
        decode(
            read_private_utf8(&path, uid)
                .map_err(|_| Refused)?
                .as_bytes(),
        )
    }
}

impl Journal for Files {
    fn load(&mut self) -> Result<Receipt, Refused> {
        self.read()
    }

    fn replace_synced(&mut self, expected: Receipt, next: Receipt) -> Result<(), Refused> {
        if self.read()? != expected {
            return Err(Refused);
        }
        let boundary = if next.phase == Phase::Reserved { 1 } else { 5 };
        checkpoint(&self.root, self.stop, boundary)?;
        atomic_replace_private(
            &self.root.join("receipt.json"),
            &serde_json::to_vec(&next).map_err(|_| Refused)?,
            nix::unistd::Uid::current().as_raw(),
        )
        .map_err(|_| Refused)?;
        checkpoint(&self.root, self.stop, boundary + 1)?;
        if self.read()? != next {
            return Err(Refused);
        }
        Ok(())
    }
}

struct Host {
    root: PathBuf,
    stop: Stop,
    fence: Fence,
}

impl Host {
    fn new(root: &Path, stop: Stop, fence: Fence) -> Self {
        Self {
            root: root.to_owned(),
            stop,
            fence,
        }
    }
}

impl Observation for Host {
    fn current(&mut self) -> Result<(Fence, Current), Refused> {
        Ok((
            self.fence,
            Current {
                owner_generation: self.fence.owner_generation,
                desired_revision: self.fence.desired_revision,
                network_epoch: self.fence.network_epoch,
                now_tick: 103,
                desired_connected: true,
                mutation_idle: true,
                owned: OwnedState::ProvenEmpty,
                attempt: Attempt::OutcomeUnknown,
                recovery_safety_proven: true,
            },
        ))
    }

    fn synthetic_effect(&mut self) -> Result<(), Refused> {
        checkpoint(&self.root, self.stop, 3)?;
        // One durable synthetic counter byte; no core, controller or network.
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(self.root.join("effects"))
            .map_err(|_| Refused)?;
        file.write_all(b"x").map_err(|_| Refused)?;
        file.sync_all().map_err(|_| Refused)?;
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|_| Refused)?;
        checkpoint(&self.root, self.stop, 4)
    }
}

fn effects(root: &Path) -> usize {
    match fs::read(root.join("effects")) {
        Ok(bytes) => {
            assert!(bytes.iter().all(|b| *b == b'x'));
            bytes.len()
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => panic!("synthetic effect evidence unavailable"),
    }
}

fn restarted_refuses(root: &Path, expected_effects: usize) {
    let mut files = Files::open(root, Stop::None).unwrap();
    let before = fs::read(root.join("receipt.json")).unwrap();
    let next = Fence {
        owner_instance: [3; 16],
        ..FENCE
    };
    let mut host = Host::new(root, Stop::None, next);
    assert_eq!(
        Admission::new(next)
            .unwrap()
            .attempt(HINT, &mut files, &mut host),
        Err(Refused)
    );
    assert_eq!(fs::read(root.join("receipt.json")).unwrap(), before);
    assert_eq!(effects(root), expected_effects);
}

#[test]
fn real_files_success_and_lock_contention() {
    let fixture = Fixture::new();
    let mut files = Files::open(&fixture.0, Stop::None).unwrap();
    assert!(Files::open(&fixture.0, Stop::None).is_err());
    let mut host = Host::new(&fixture.0, Stop::None, FENCE);
    let mut admission = Admission::new(FENCE).unwrap();
    assert_eq!(admission.attempt(HINT, &mut files, &mut host), Ok(()));
    assert_eq!(files.read().unwrap().phase, Phase::Finished);
    assert_eq!(admission.attempt(HINT, &mut files, &mut host), Err(Refused));
    assert_eq!(effects(&fixture.0), 1);
    drop(files);
    restarted_refuses(&fixture.0, 1);
}

#[test]
fn real_file_errors_preserve_owner_latch_and_restart_refusal() {
    for boundary in 1..=6 {
        let fixture = Fixture::new();
        let mut files = Files::open(&fixture.0, Stop::Error(boundary)).unwrap();
        let mut host = Host::new(&fixture.0, Stop::Error(boundary), FENCE);
        let mut admission = Admission::new(FENCE).unwrap();
        assert_eq!(admission.attempt(HINT, &mut files, &mut host), Err(Refused));
        assert_eq!(effects(&fixture.0), usize::from(boundary >= 4));
        files.stop = Stop::None;
        host.stop = Stop::None;
        assert_eq!(admission.attempt(HINT, &mut files, &mut host), Err(Refused));
        drop(files);
        restarted_refuses(&fixture.0, usize::from(boundary >= 4));
    }
}

#[test]
#[ignore = "subprocess fixture; invoked by process_kill_at_receipt_boundaries"]
fn crash_child() {
    let root = PathBuf::from(std::env::var_os("OMAVLESS_T4_RECEIPT_FIXTURE").unwrap());
    let boundary = std::env::var("OMAVLESS_T4_RECEIPT_BOUNDARY")
        .unwrap()
        .parse::<u8>()
        .unwrap();
    assert!((1..=6).contains(&boundary));
    let stop = Stop::Crash(boundary);
    let mut files = Files::open(&root, stop).unwrap();
    let mut host = Host::new(&root, stop, FENCE);
    Admission::new(FENCE)
        .unwrap()
        .attempt(HINT, &mut files, &mut host)
        .unwrap();
    panic!("crash boundary was not reached");
}

#[test]
fn missing_invalid_or_unsafe_receipt_is_not_initialized_or_repaired() {
    for case in 0..7 {
        let fixture = Fixture::new();
        let path = fixture.0.join("receipt.json");
        match case {
            0 => fs::remove_file(&path).unwrap(),
            1 => fs::write(&path, b"{}").unwrap(),
            2 => fs::write(&path, vec![b' '; 1025]).unwrap(),
            3 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            4 => {
                fs::rename(&path, fixture.0.join("foreign")).unwrap();
                std::os::unix::fs::symlink("foreign", &path).unwrap();
            }
            5 => fs::write(
                &path,
                serde_json::to_vec(&Receipt { schema: 2, ..READY }).unwrap(),
            )
            .unwrap(),
            6 => fs::write(
                &path,
                serde_json::to_vec(&Receipt {
                    fence: Fence {
                        owner_instance: [4; 16],
                        ..FENCE
                    },
                    ..READY
                })
                .unwrap(),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let before = fs::read(&path).ok();
        let mode = fs::symlink_metadata(&path)
            .ok()
            .map(|m| m.permissions().mode());
        let mut files = Files::open(&fixture.0, Stop::None).unwrap();
        let mut host = Host::new(&fixture.0, Stop::None, FENCE);
        assert_eq!(
            Admission::new(FENCE)
                .unwrap()
                .attempt(HINT, &mut files, &mut host),
            Err(Refused)
        );
        assert_eq!(fs::read(&path).ok(), before);
        assert_eq!(
            fs::symlink_metadata(&path)
                .ok()
                .map(|m| m.permissions().mode()),
            mode
        );
        assert_eq!(effects(&fixture.0), 0);
    }
}

#[test]
fn process_kill_at_receipt_boundaries() {
    use std::os::unix::process::ExitStatusExt;
    for boundary in 1..=6 {
        let fixture = Fixture::new();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "network_recovery_receipt::file_tests::crash_child",
                "--ignored",
            ])
            .env("OMAVLESS_T4_RECEIPT_FIXTURE", &fixture.0)
            .env("OMAVLESS_T4_RECEIPT_BOUNDARY", boundary.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let reached = loop {
            if fs::read(fixture.0.join("checkpoint")).ok() == Some(vec![boundary]) {
                break true;
            }
            if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                break false;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        // Always reap even if the rendezvous failed; no leftover test process.
        let _ = child.kill();
        let status = child.wait().unwrap();
        assert!(reached, "child did not reach synthetic boundary {boundary}");
        assert_eq!(status.signal(), Some(9));
        let files = Files::open(&fixture.0, Stop::None).unwrap();
        assert_eq!(
            files.read().unwrap().phase,
            match boundary {
                1 => Phase::Ready,
                6 => Phase::Finished,
                _ => Phase::Reserved,
            }
        );
        drop(files);
        restarted_refuses(&fixture.0, usize::from(boundary >= 4));
    }
}
