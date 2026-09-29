// SPDX-License-Identifier: MIT

use super::storage::{Checkpoint, RECORD, STAGING};
use super::*;
use crate::app_proxy::codec::{
    DesktopEntry, DesktopKey, DesktopValue, EnvironmentEntry, EnvironmentKey, EnvironmentValue,
    Override,
};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs, path::PathBuf, process::Command};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omavless-s1-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binding() -> Binding {
    Binding {
        owner_instance: [1; 16],
        owner_generation: 7,
        boot: [2; 16],
        session: [3; 16],
        uid: nix::unistd::geteuid().as_raw(),
    }
}

fn pair(intended: bool) -> [Snapshot; 2] {
    let desktop = DesktopKey::ALL
        .into_iter()
        .map(|key| {
            let default = match key.schema_key_type().2 {
                "s" if key == DesktopKey::Mode => DesktopValue::String("none".into()),
                "s" => DesktopValue::String(String::new()),
                "i" => DesktopValue::Int(0),
                "b" => DesktopValue::Bool(false),
                "as" => DesktopValue::Strings(Vec::new()),
                _ => unreachable!(),
            };
            let effective = if intended && key == DesktopKey::Mode {
                DesktopValue::String("manual".into())
            } else {
                default.clone()
            };
            let user = if intended && key == DesktopKey::Mode {
                Override::Present(effective.clone())
            } else {
                Override::Absent
            };
            DesktopEntry {
                key,
                effective,
                default,
                user,
                writable: true,
            }
        })
        .collect();
    let manager = EnvironmentKey::ALL
        .into_iter()
        .map(|key| EnvironmentEntry {
            key,
            value: if intended && key == EnvironmentKey::Http {
                EnvironmentValue::Present("http://127.0.0.1:17890".into())
            } else {
                EnvironmentValue::Absent
            },
        })
        .collect();
    [
        DesktopSnapshot::capture(desktop).unwrap().encode().unwrap(),
        EnvironmentSnapshot::capture(manager)
            .unwrap()
            .encode()
            .unwrap(),
    ]
}

fn apply(journal: &mut Journal, current: &mut [Snapshot; 2]) {
    while let Some(effect) = journal.begin_next(binding(), current).unwrap() {
        // Prove intent was readable and durable before executing even a fake effect.
        let disk = journal.storage.read().unwrap().unwrap();
        let (_, lease) = decode(&disk).unwrap();
        assert_eq!(lease.pending, Some(effect.surface));
        current[effect.surface.index()] = effect.replacement;
        journal.confirm(binding(), current).unwrap();
    }
}

#[test]
fn apply_restore_and_reopen_preserve_exact_initial_pair() {
    let root = Temp::new();
    let original = pair(false);
    let mut current = original.clone();
    let mut journal = Journal::create(&root.0, binding(), original.clone(), pair(true)).unwrap();
    apply(&mut journal, &mut current);
    assert_eq!(journal.phase(), Phase::Active);
    drop(journal);
    let mut journal = Journal::open(&root.0, binding()).unwrap();
    assert_eq!(
        journal.begin_next(binding(), &current),
        Err(Error::RecoveryRequired)
    );
    journal.begin_restore(binding(), &current).unwrap();
    apply(&mut journal, &mut current);
    assert_eq!(journal.phase(), Phase::Released);
    assert_eq!(current, original);
    drop(journal);
    let journal = Journal::open(&root.0, binding()).unwrap();
    assert_eq!(journal.phase(), Phase::Released);
    drop(journal);
    assert!(matches!(
        Journal::create(&root.0, binding(), pair(false), pair(true)),
        Err(Error::AlreadyExists)
    ));
}

#[test]
fn lost_effect_outcome_accepts_only_original_or_intended() {
    for applied in [false, true] {
        let root = Temp::new();
        let original = pair(false);
        let mut current = original.clone();
        let mut journal =
            Journal::create(&root.0, binding(), original.clone(), pair(true)).unwrap();
        let effect = journal.begin_next(binding(), &current).unwrap().unwrap();
        if applied {
            current[0] = effect.replacement;
        }
        drop(journal); // Crash before confirmation.
        let mut journal = Journal::open(&root.0, binding()).unwrap();
        assert_eq!(
            journal.confirm(binding(), &current),
            Err(Error::RecoveryRequired)
        );
        let mut foreign = current.clone();
        foreign[0] = Snapshot::new(Some(b"unrelated".to_vec())).unwrap();
        assert_eq!(
            journal.begin_restore(binding(), &foreign),
            Err(Error::Planner(crate::app_proxy::Error::ForeignChange))
        );
        journal.begin_restore(binding(), &current).unwrap();
        apply(&mut journal, &mut current);
        assert_eq!(current, original);
    }
}

#[test]
fn unknown_persistence_poisoned_before_any_effect_is_released() {
    for point in [
        Checkpoint::Created,
        Checkpoint::Written,
        Checkpoint::FileSynced,
        Checkpoint::Renamed,
        Checkpoint::DirectorySynced,
    ] {
        let root = Temp::new();
        let mut journal = Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap();
        journal.storage.fail_at.set(Some(point));
        assert_eq!(
            journal.begin_next(binding(), &pair(false)),
            Err(Error::OutcomeUnknown)
        );
        assert_eq!(
            journal.begin_next(binding(), &pair(false)),
            Err(Error::RecoveryRequired)
        );
        drop(journal);
        if matches!(
            point,
            Checkpoint::Created | Checkpoint::Written | Checkpoint::FileSynced
        ) {
            assert!(matches!(
                Journal::open(&root.0, binding()),
                Err(Error::Interrupted)
            ));
        } else {
            let mut reopened = Journal::open(&root.0, binding()).unwrap();
            reopened.begin_restore(binding(), &pair(false)).unwrap();
            assert!(
                reopened
                    .begin_next(binding(), &pair(false))
                    .unwrap()
                    .is_none()
            );
            assert_eq!(reopened.phase(), Phase::Released);
        }
    }
}

#[test]
fn crash_fixture_worker() {
    let Ok(path) = std::env::var("OMAVLESS_S1_CRASH_DIRECTORY") else {
        return;
    };
    let point = match std::env::var("OMAVLESS_S1_CRASH_POINT").unwrap().as_str() {
        "created" => Checkpoint::Created,
        "written" => Checkpoint::Written,
        "synced" => Checkpoint::FileSynced,
        "renamed" => Checkpoint::Renamed,
        "directory" => Checkpoint::DirectorySynced,
        _ => panic!("invalid test stage"),
    };
    let mut journal =
        Journal::create(Path::new(&path), binding(), pair(false), pair(true)).unwrap();
    journal.storage.crash_at.set(Some(point));
    let _ = journal.begin_next(binding(), &pair(false));
    panic!("crash checkpoint not reached");
}

#[test]
fn actual_process_exit_at_each_durable_boundary_requires_recovery() {
    for point in ["created", "written", "synced", "renamed", "directory"] {
        let root = Temp::new();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app_proxy::journal::tests::crash_fixture_worker"])
            .env("OMAVLESS_S1_CRASH_DIRECTORY", &root.0)
            .env("OMAVLESS_S1_CRASH_POINT", point)
            .output()
            .unwrap()
            .status;
        assert_eq!(status.code(), Some(71));
        if matches!(point, "created" | "written" | "synced") {
            assert!(matches!(
                Journal::open(&root.0, binding()),
                Err(Error::Interrupted)
            ));
        } else {
            let mut journal = Journal::open(&root.0, binding()).unwrap();
            assert_eq!(
                journal.begin_next(binding(), &pair(false)),
                Err(Error::RecoveryRequired)
            );
            journal.begin_restore(binding(), &pair(false)).unwrap();
            assert!(
                journal
                    .begin_next(binding(), &pair(false))
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn exact_owner_boot_session_uid_and_writer_lock_required() {
    let root = Temp::new();
    let journal = Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::Busy)
    ));
    drop(journal);
    for which in 0..5 {
        let mut stale = binding();
        match which {
            0 => stale.owner_generation += 1,
            1 => stale.owner_instance[0] += 1,
            2 => stale.boot[0] += 1,
            3 => stale.session[0] += 1,
            _ => stale.uid ^= 1,
        }
        assert!(matches!(
            Journal::open(&root.0, stale),
            Err(Error::BindingMismatch)
        ));
    }
    let mut journal = Journal::open(&root.0, binding()).unwrap();
    let mut stale = binding();
    stale.owner_generation += 1;
    assert_eq!(
        journal.begin_restore(stale, &pair(false)),
        Err(Error::BindingMismatch)
    );
}

#[test]
fn unsafe_paths_hardlinks_modes_fifo_and_staging_are_refused() {
    let root = Temp::new();
    let journal = Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap();
    drop(journal);
    fs::set_permissions(root.0.join(RECORD), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::UnsafePath)
    ));
    fs::set_permissions(root.0.join(RECORD), fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(root.0.join(RECORD), root.0.join("other")).unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::UnsafePath)
    ));
    fs::remove_file(root.0.join("other")).unwrap();
    fs::rename(root.0.join(RECORD), root.0.join("saved")).unwrap();
    symlink("saved", root.0.join(RECORD)).unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::UnsafePath)
    ));
    fs::remove_file(root.0.join(RECORD)).unwrap();
    nix::unistd::mkfifo(
        &root.0.join(RECORD),
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )
    .unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::UnsafePath)
    ));
    fs::remove_file(root.0.join(RECORD)).unwrap();
    fs::rename(root.0.join("saved"), root.0.join(RECORD)).unwrap();
    symlink("/does-not-exist", root.0.join(STAGING)).unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::Interrupted)
    ));
    fs::remove_file(root.0.join(STAGING)).unwrap();
    fs::set_permissions(&root.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::UnsafePath)
    ));
    fs::set_permissions(&root.0, fs::Permissions::from_mode(0o700)).unwrap();
    let outer = Temp::new();
    symlink(&root.0, outer.0.join("link")).unwrap();
    assert!(matches!(
        Journal::open(&outer.0.join("link"), binding()),
        Err(Error::UnsafePath)
    ));
    assert!(matches!(
        Journal::open(Path::new("relative"), binding()),
        Err(Error::UnsafePath)
    ));
}

#[test]
fn foreign_file_replacement_and_directory_rename_poison_handle() {
    let root = Temp::new();
    let mut journal = Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap();
    fs::write(root.0.join(RECORD), b"foreign").unwrap();
    assert_eq!(
        journal.begin_next(binding(), &pair(false)),
        Err(Error::ForeignChange)
    );
    assert_eq!(fs::read(root.0.join(RECORD)).unwrap(), b"foreign");
    assert_eq!(
        journal.begin_restore(binding(), &pair(false)),
        Err(Error::RecoveryRequired)
    );
    drop(journal);
    let root = Temp::new();
    let mut journal = Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap();
    let outer = Temp::new();
    fs::rename(&root.0, outer.0.join("moved")).unwrap();
    fs::DirBuilder::new().mode(0o700).create(&root.0).unwrap();
    assert_eq!(
        journal.begin_next(binding(), &pair(false)),
        Err(Error::ForeignChange)
    );
    assert!(!root.0.join(RECORD).exists());
}

#[test]
fn malformed_duplicate_noncanonical_and_impossible_records_refuse() {
    let lease = ProxyLease::prepare(binding().owner(), pair(false), pair(true));
    let encoded = encode(binding(), &lease).unwrap();
    let text = String::from_utf8(encoded.clone()).unwrap();
    for broken in [
        text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        text.replacen("\"version\":1", "\"version\":2", 1),
        text.replacen("\"version\":1", "\"version\":1,\"command\":\"bad\"", 1),
        format!("{text} trailing"),
    ] {
        assert!(matches!(decode(broken.as_bytes()), Err(Error::Invalid)));
    }
    for (key, value) in [
        ("phase", serde_json::json!("active")),
        ("pending", serde_json::json!("desktop")),
        ("attempted", serde_json::json!([false, true])),
        ("expected", serde_json::json!(["intended", "original"])),
    ] {
        let mut broken: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        broken[key] = value;
        assert!(matches!(
            decode(&serde_json::to_vec(&broken).unwrap()),
            Err(Error::Invalid)
        ));
    }
    let mut broken: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    broken["original"]["desktop"] = serde_json::json!("{}");
    assert!(matches!(
        decode(&serde_json::to_vec(&broken).unwrap()),
        Err(Error::Invalid)
    ));
    let root = Temp::new();
    assert!(matches!(
        Journal::open(&root.0, binding()),
        Err(Error::Missing)
    ));
    assert!(matches!(
        Journal::create(
            &root.0,
            binding(),
            [Snapshot::new(None).unwrap(), pair(false)[1].clone()],
            pair(true)
        ),
        Err(Error::Invalid)
    ));
    assert!(!root.0.join(RECORD).exists());
}

#[test]
fn private_debug_and_final_permissions() {
    let root = Temp::new();
    let journal = Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap();
    let debug = format!("{journal:?} {:?}", binding());
    assert!(!debug.contains("127.0.0.1"));
    assert!(!debug.contains(root.0.to_str().unwrap()));
    assert_eq!(
        fs::metadata(root.0.join(RECORD))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(!root.0.join(STAGING).exists());
}

#[test]
fn lost_confirmation_or_restore_intent_keeps_original_recoverable() {
    for during_restore in [false, true] {
        for point in [
            Checkpoint::Created,
            Checkpoint::Written,
            Checkpoint::FileSynced,
            Checkpoint::Renamed,
            Checkpoint::DirectorySynced,
        ] {
            let root = Temp::new();
            let original = pair(false);
            let mut current = original.clone();
            let mut journal =
                Journal::create(&root.0, binding(), original.clone(), pair(true)).unwrap();
            let effect = journal.begin_next(binding(), &current).unwrap().unwrap();
            current[0] = effect.replacement;
            if during_restore {
                journal.confirm(binding(), &current).unwrap();
                journal.begin_restore(binding(), &current).unwrap();
            }
            journal.storage.fail_at.set(Some(point));
            let outcome = if during_restore {
                journal.begin_next(binding(), &current).map(|_| ())
            } else {
                journal.confirm(binding(), &current)
            };
            assert_eq!(outcome, Err(Error::OutcomeUnknown));
            // Neither the old nor replacement record ever loses its baseline.
            let (_, durable) = decode(&fs::read(root.0.join(RECORD)).unwrap()).unwrap();
            assert_eq!(durable.original, original);
            drop(journal);
            if matches!(point, Checkpoint::Renamed | Checkpoint::DirectorySynced) {
                let mut journal = Journal::open(&root.0, binding()).unwrap();
                journal.begin_restore(binding(), &current).unwrap();
                apply(&mut journal, &mut current);
                assert_eq!(current, original);
            } else {
                assert!(matches!(
                    Journal::open(&root.0, binding()),
                    Err(Error::Interrupted)
                ));
            }
        }
    }
}

#[test]
fn private_but_corrupt_or_oversized_disk_record_is_never_reset() {
    for data in [
        b"not json".to_vec(),
        vec![0xff],
        vec![b'x'; MAX_JOURNAL_BYTES + 1],
    ] {
        let root = Temp::new();
        drop(Journal::create(&root.0, binding(), pair(false), pair(true)).unwrap());
        fs::write(root.0.join(RECORD), &data).unwrap();
        assert!(matches!(
            Journal::open(&root.0, binding()),
            Err(Error::Invalid)
        ));
        assert!(Journal::create(&root.0, binding(), pair(false), pair(true)).is_err());
        assert_eq!(fs::read(root.0.join(RECORD)).unwrap(), data);
    }
}
