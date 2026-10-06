// SPDX-License-Identifier: MIT
use super::*;

#[cfg(feature = "netguard-native-scenario")]
#[test]
fn startup_diagnostic_observes_actual_receipt_and_original_fences() {
    use super::diagnostic::{Io, Reason};
    let fixture = Fixture::new(false);
    fixture.run(&mut Host::default()).unwrap();
    let original = fs::read(&fixture.paths.receipt).unwrap();
    let lock = MigrationLock::acquire(&fixture.paths.cutover, fixture.paths.uid).unwrap();
    let check = |generation, expected: Reason| {
        let result =
            check_startup_receipt(&fixture.paths.cutover, fixture.paths.uid, &lock, generation);
        assert_eq!(result.is_ok(), expected == Reason::NoFailure);
        assert_eq!(diagnostic::last(), (expected, Io::None));
    };
    check(Some(2), Reason::NoFailure);
    // Desired generations are deliberately not ownership receipt generations.
    let changed = DesiredState {
        generation: 81,
        ..DesiredState::default()
    };
    crate::desired::write_desired(&fixture.paths.desired, fixture.paths.uid, &changed).unwrap();
    check(Some(2), Reason::NoFailure);
    assert_eq!(fs::read(&fixture.paths.receipt).unwrap(), original);
    check(Some(3), Reason::Identity);
    check(None, Reason::Identity);
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    value["phase"] = serde_json::json!("pending");
    fixture.put(&fixture.paths.receipt, &serde_json::to_vec(&value).unwrap());
    check(Some(2), Reason::Identity);
    fixture.put(&fixture.paths.receipt, b"{");
    check(Some(2), Reason::Decode);
    value["schemaVersion"] = serde_json::json!(2);
    fixture.put(&fixture.paths.receipt, &serde_json::to_vec(&value).unwrap());
    check(Some(2), Reason::Schema);
    fixture.put(&fixture.paths.receipt, &[0xff]);
    check(Some(2), Reason::StoreUtf8);
    fixture.put(&fixture.paths.receipt, &original);
    fs::set_permissions(&fixture.paths.receipt, fs::Permissions::from_mode(0o644)).unwrap();
    check(Some(2), Reason::MetadataUnsafe);
    let pending = fixture
        .paths
        .cutover
        .state_directory
        .join("restore-decision.intent");
    fixture.put(&pending, b"synthetic");
    // Existing short circuit: pending wins over the still-invalid receipt mode.
    check(Some(2), Reason::Pending);
    assert!(
        check_startup_receipt(
            &fixture.paths.cutover,
            fixture.paths.uid + 1,
            &lock,
            Some(2)
        )
        .is_err()
    );
    assert_eq!(diagnostic::last(), (Reason::StartupLock, Io::None));
    fs::remove_file(pending).unwrap();
    fs::set_permissions(&fixture.paths.receipt, fs::Permissions::from_mode(0o600)).unwrap();
    check(Some(2), Reason::NoFailure);
    fs::remove_file(&fixture.paths.receipt).unwrap();
    check(Some(2), Reason::NoFailure); // Preserve the original startup semantics.
}

#[test]
fn production_receipt_requires_current_epoch_and_preserves_manual_disconnect() {
    let fixture = Fixture::new(false);
    fixture.put(
        &fixture.paths.desired.file,
        &serde_json::to_vec(&DesiredState::default()).unwrap(),
    );
    let check = |epoch: &str, generation| {
        let lock = MigrationLock::acquire(&fixture.paths.cutover, fixture.paths.uid).unwrap();
        check_current_receipt(
            &fixture.paths.cutover,
            fixture.paths.uid,
            &lock,
            generation,
            epoch,
        )
    };
    assert_eq!(
        check("synthetic-epoch", 2),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    fixture.run(&mut Host::default()).unwrap();
    let desired = fs::read(&fixture.paths.desired.file).unwrap();
    let receipt = fs::read(&fixture.paths.receipt).unwrap();
    assert_eq!(check("synthetic-epoch", 2), Ok(()));
    assert_eq!(
        check("another-manager", 2),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    assert_eq!(
        check("synthetic-epoch", 4),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    assert_eq!(
        fixture.run(&mut Host::default()),
        Ok(LoginTransactionOutcome::AlreadyConsumed)
    );
    assert_eq!(fs::read(&fixture.paths.desired.file).unwrap(), desired);
    assert_eq!(fs::read(&fixture.paths.receipt).unwrap(), receipt);
    let mut value: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
    value["phase"] = serde_json::json!("pending");
    fixture.put(&fixture.paths.receipt, &serde_json::to_vec(&value).unwrap());
    assert_eq!(
        check("synthetic-epoch", 2),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
}

#[test]
fn startup_receipt_requires_exact_migration_lease_even_when_absent() {
    let first = Fixture::new(false);
    let other = Fixture::new(false);
    let lock = MigrationLock::acquire(&first.paths.cutover, first.paths.uid).unwrap();
    assert_eq!(
        check_startup_receipt(&first.paths.cutover, first.paths.uid, &lock, Some(2)),
        Ok(())
    );
    assert_eq!(
        check_startup_receipt(&other.paths.cutover, other.paths.uid, &lock, Some(2)),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    assert_eq!(
        check_startup_receipt(&first.paths.cutover, first.paths.uid + 1, &lock, Some(2)),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
}

#[test]
fn private_transaction_pending_blocks_login_and_both_startup_receipt_proofs() {
    for name in ["routing-preset.pending.json", "restore-pair.pending"] {
        let fixture = Fixture::new(true);
        let path = fixture.paths.desired.directory.join(name);
        let before = fs::read(&fixture.paths.store).unwrap();
        if name == "restore-pair.pending" {
            fs::create_dir(&path).unwrap();
        } else {
            fs::write(&path, b"synthetic incomplete pending marker").unwrap();
        }
        let mut host = Host::default();
        assert_eq!(
            fixture.run(&mut host),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
        assert_eq!((host.empty, host.validated), (0, 0));
        assert!(!fixture.paths.receipt.exists());
        assert!(!fixture.paths.desired.file.exists());
        assert_eq!(fs::read(&fixture.paths.store).unwrap(), before);
        let lock = MigrationLock::acquire(&fixture.paths.cutover, fixture.paths.uid).unwrap();
        assert_eq!(
            check_startup_receipt(&fixture.paths.cutover, fixture.paths.uid, &lock, Some(2)),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
        assert_eq!(
            check_current_receipt(
                &fixture.paths.cutover,
                fixture.paths.uid,
                &lock,
                2,
                "synthetic-epoch",
            ),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
        drop(lock);
        if name == "restore-pair.pending" {
            fs::remove_dir(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        fixture.run(&mut Host::default()).unwrap();
        fs::create_dir(&path).unwrap();
        let lock = MigrationLock::acquire(&fixture.paths.cutover, fixture.paths.uid).unwrap();
        assert_eq!(
            check_startup_receipt(&fixture.paths.cutover, fixture.paths.uid, &lock, Some(2)),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
        assert_eq!(
            check_current_receipt(
                &fixture.paths.cutover,
                fixture.paths.uid,
                &lock,
                2,
                "synthetic-epoch",
            ),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
        drop(lock);
        assert_eq!(
            fixture.run(&mut Host::default()),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
    }
}
use serde_json::json;
use std::os::unix::fs::{PermissionsExt, symlink};

struct Fixture {
    root: PathBuf,
    paths: LoginPaths,
}
impl Fixture {
    fn new(enabled: bool) -> Self {
        let root = crate::test_temp::directory("login-transaction").unwrap();
        let home = root.join("home");
        let runtime = root.join("runtime");
        let state = root.join("state");
        for directory in [
            home.join(".config/omavless"),
            runtime.clone(),
            runtime.join("omavless"),
            state.join("omavless"),
        ] {
            fs::create_dir_all(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let paths = LoginPaths::below(&home, &runtime, &state, Uid::current().as_raw()).unwrap();
        let fixture = Self { root, paths };
        fixture.put(
            &fixture.paths.cutover.ownership_marker,
            br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
        );
        fixture.put(&fixture.paths.template, b"mode: rule\n");
        let id = "00000000-0000-4000-8000-000000000001";
        let store = json!({"version":3,"activeId":"","lastId":id,
            "profiles":[{"id":id,"name":"Synthetic","protocol":"vless",
            "uri":"vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp",
            "subscriptionId":"","subscriptionKey":"","favorite":false,"missing":false}],
            "subscriptions":[],"routingPreset":"custom","customRules":[],"startupConfigured":true,
            "startup":{"enabled":enabled,"target":"last","profileId":"","mode":"global"}});
        fixture.put(&fixture.paths.store, store.to_string().as_bytes());
        fixture
    }
    fn put(&self, path: &Path, bytes: &[u8]) {
        atomic_replace_private(path, bytes, self.paths.uid).unwrap();
    }
    fn run(&self, host: &mut Host) -> Result<LoginTransactionOutcome> {
        consume_login(&self.paths, 2, "synthetic-epoch", host)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[derive(Default)]
struct Host {
    empty: usize,
    validated: usize,
    reject_empty_at: usize,
    reject_candidate: bool,
    tamper: Option<(PathBuf, Vec<u8>)>,
    pending_after_validation: Option<PathBuf>,
}
impl LoginReadiness for Host {
    fn verify_empty(&mut self) -> std::result::Result<(), LoginHostError> {
        self.empty += 1;
        if self.empty == self.reject_empty_at {
            Err(LoginHostError)
        } else {
            Ok(())
        }
    }
    fn validate_candidate(
        &mut self,
        _: &DesiredState,
        _: &PrivateStore,
        _: &str,
    ) -> std::result::Result<(), LoginHostError> {
        self.validated += 1;
        if let Some((path, bytes)) = &self.tamper {
            atomic_replace_private(path, bytes, Uid::current().as_raw()).unwrap();
        }
        if let Some(path) = &self.pending_after_validation {
            fs::create_dir(path).unwrap();
        }
        if self.reject_candidate {
            Err(LoginHostError)
        } else {
            Ok(())
        }
    }
}

#[test]
fn pending_appearing_during_host_validation_refuses_before_login_receipt() {
    let fixture = Fixture::new(true);
    let mut host = Host {
        pending_after_validation: Some(
            fixture.paths.desired.directory.join("restore-pair.pending"),
        ),
        ..Host::default()
    };
    assert_eq!(
        fixture.run(&mut host),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    assert_eq!((host.empty, host.validated), (2, 1));
    assert!(!fixture.paths.receipt.exists());
    assert!(!fixture.paths.desired.file.exists());
}

#[test]
fn pending_after_login_publication_preserves_receipt_for_manual_recovery() {
    for stage in [Publication::Pending, Publication::Desired] {
        let fixture = Fixture::new(true);
        let publisher = Publisher {
            create_pending_after: Some((
                stage,
                fixture.paths.desired.directory.join("restore-pair.pending"),
            )),
            ..Publisher::default()
        };
        assert_eq!(
            consume(
                &fixture.paths,
                2,
                "synthetic-epoch",
                &mut Host::default(),
                &publisher,
            ),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
        assert!(receipt(&fixture.paths).unwrap().unwrap().phase == Phase::Pending);
        assert_eq!(
            fixture.paths.desired.file.exists(),
            stage == Publication::Desired
        );
        assert_eq!(
            fixture.run(&mut Host::default()),
            Err(LoginTransactionError::ManualRecoveryRequired)
        );
    }
}

#[test]
fn real_files_commit_and_private_receipt() {
    let f = Fixture::new(true);
    let original = fs::read(&f.paths.store).unwrap();
    let mut host = Host::default();
    assert_eq!(
        f.run(&mut host),
        Ok(LoginTransactionOutcome::Consumed {
            desired_changed: true
        })
    );
    assert_eq!((host.empty, host.validated), (2, 1));
    let desired =
        desired_from_snapshot(Some(&fs::read_to_string(&f.paths.desired.file).unwrap())).unwrap();
    assert!(desired.connected);
    assert_eq!(desired.generation, 1);
    assert_eq!(fs::read(&f.paths.store).unwrap(), original);
    let raw = fs::read_to_string(&f.paths.receipt).unwrap();
    assert!(!raw.contains("synthetic-epoch"));
    assert!(!raw.contains("vless://"));
    assert!(receipt(&f.paths).unwrap().unwrap().phase == Phase::Consumed);
    assert_eq!(
        fs::metadata(&f.paths.receipt).unwrap().mode() & 0o777,
        0o600
    );
}

#[test]
fn consumed_preserves_explicit_disconnect_and_rejects_other_epoch_or_owner() {
    let f = Fixture::new(true);
    f.run(&mut Host::default()).unwrap();
    let desired = DesiredState {
        generation: 2,
        ..DesiredState::default()
    };
    let bytes = serde_json::to_vec(&desired).unwrap();
    f.put(&f.paths.desired.file, &bytes);
    let mut host = Host::default();
    assert_eq!(
        f.run(&mut host),
        Ok(LoginTransactionOutcome::AlreadyConsumed)
    );
    assert_eq!((host.empty, host.validated), (0, 0));
    assert_eq!(fs::read(&f.paths.desired.file).unwrap(), bytes);
    assert_eq!(
        consume_login(&f.paths, 2, "other", &mut host),
        Err(LoginTransactionError::EpochMismatch)
    );
    f.put(
        &f.paths.cutover.ownership_marker,
        br#"{"schemaVersion":1,"generation":4,"phase":"rust"}"#,
    );
    assert_eq!(
        consume_login(&f.paths, 4, "synthetic-epoch", &mut host),
        Err(LoginTransactionError::EpochMismatch)
    );
}

#[test]
fn disabled_empty_store_consumes_without_template_or_desired_write() {
    let f = Fixture::new(false);
    fs::remove_file(&f.paths.template).unwrap();
    f.put(&f.paths.store, br#"{"version":3,"profiles":[],"subscriptions":[],"startupConfigured":true,"startup":{"enabled":false,"target":"last","profileId":"","mode":"rule"}}"#);
    let mut host = Host::default();
    assert_eq!(
        f.run(&mut host),
        Ok(LoginTransactionOutcome::Consumed {
            desired_changed: false
        })
    );
    assert!(!f.paths.desired.file.exists());
    assert_eq!((host.empty, host.validated), (2, 0));
}

#[test]
fn both_real_locks_refuse_without_effects() {
    let f = Fixture::new(true);
    let owner = OwnerLock::acquire(&f.paths.runtime.owner_lock, f.paths.uid).unwrap();
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::Busy)
    );
    drop(owner);
    let migration = MigrationLock::acquire(&f.paths.cutover, f.paths.uid).unwrap();
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::Busy)
    );
    drop(migration);
    assert!(!f.paths.receipt.exists());
    assert!(!f.paths.desired.file.exists());
}

#[test]
fn ownership_and_host_validation_fail_before_journal() {
    for phase in ["legacy", "cutoverPreparing", "rollbackPreparing"] {
        let f = Fixture::new(true);
        f.put(
            &f.paths.cutover.ownership_marker,
            json!({"schemaVersion":1,"generation":2,"phase":phase})
                .to_string()
                .as_bytes(),
        );
        assert_eq!(
            f.run(&mut Host::default()),
            Err(LoginTransactionError::OwnershipUnavailable)
        );
        assert!(!f.paths.receipt.exists());
    }
    for host in [
        Host {
            reject_empty_at: 1,
            ..Host::default()
        },
        Host {
            reject_empty_at: 2,
            ..Host::default()
        },
        Host {
            reject_candidate: true,
            ..Host::default()
        },
    ] {
        let f = Fixture::new(true);
        assert_eq!(
            f.run(&mut { host }),
            Err(LoginTransactionError::ValidationRejected)
        );
        assert!(!f.paths.receipt.exists());
        assert!(!f.paths.desired.file.exists());
    }
    let f = Fixture::new(true);
    assert_eq!(
        consume_login(&f.paths, 4, "synthetic-epoch", &mut Host::default()),
        Err(LoginTransactionError::OwnershipUnavailable)
    );
}

#[test]
fn validation_snapshot_tamper_is_fenced() {
    for field in 0..3 {
        let f = Fixture::new(true);
        let path = [&f.paths.store, &f.paths.template, &f.paths.desired.file][field].clone();
        let bytes = if field == 2 {
            serde_json::to_vec(&DesiredState::default()).unwrap()
        } else {
            b"changed".to_vec()
        };
        let mut host = Host {
            tamper: Some((path, bytes)),
            ..Host::default()
        };
        assert_eq!(
            f.run(&mut host),
            Err(LoginTransactionError::SnapshotChanged)
        );
        assert!(!f.paths.receipt.exists());
    }
}

#[test]
fn publication_fault_matrix_distinguishes_ack_loss_without_replay() {
    for stage in [
        Publication::Pending,
        Publication::Desired,
        Publication::Consumed,
    ] {
        for after in [false, true] {
            let f = Fixture::new(true);
            let publisher = Publisher {
                fault: Some((stage, after)),
                ..Publisher::default()
            };
            assert_eq!(
                consume(
                    &f.paths,
                    2,
                    "synthetic-epoch",
                    &mut Host::default(),
                    &publisher
                ),
                Err(LoginTransactionError::ManualRecoveryRequired)
            );
            let before = fs::read(&f.paths.desired.file).ok();
            let result = f.run(&mut Host::default());
            if stage == Publication::Pending && !after {
                assert_eq!(
                    result,
                    Ok(LoginTransactionOutcome::Consumed {
                        desired_changed: true
                    })
                );
            } else if stage == Publication::Consumed && after {
                assert_eq!(result, Ok(LoginTransactionOutcome::AlreadyConsumed));
                assert_eq!(fs::read(&f.paths.desired.file).ok(), before);
            } else {
                assert_eq!(result, Err(LoginTransactionError::ManualRecoveryRequired));
                assert_eq!(fs::read(&f.paths.desired.file).ok(), before);
            }
        }
    }
}

#[test]
fn no_op_post_pending_desired_change_is_not_blessed() {
    let f = Fixture::new(false);
    let publisher = Publisher {
        tamper: Some((
            Publication::Pending,
            f.paths.desired.file.clone(),
            serde_json::to_vec(&DesiredState::default()).unwrap(),
        )),
        ..Publisher::default()
    };
    assert_eq!(
        consume(
            &f.paths,
            2,
            "synthetic-epoch",
            &mut Host::default(),
            &publisher
        ),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    assert!(receipt(&f.paths).unwrap().unwrap().phase == Phase::Pending);
}

#[test]
fn corrupt_unsafe_or_pending_receipt_blocks() {
    for raw in ["{}".to_owned(), "x".repeat(1025), "{\"schemaVersion\":1,\"schemaVersion\":1}".to_owned(), json!({"schemaVersion":1,"epochHash":"a".repeat(64),"ownershipGeneration":2,"phase":"pending"}).to_string(), json!({"schemaVersion":1,"epochHash":"a".repeat(64),"ownershipGeneration":2,"phase":"consumed","extra":true}).to_string()] {
        let f = Fixture::new(true);
        f.put(&f.paths.receipt,raw.as_bytes());
        assert_eq!(f.run(&mut Host::default()),Err(LoginTransactionError::ManualRecoveryRequired));
        assert!(!f.paths.desired.file.exists());
    }
    let f = Fixture::new(true);
    symlink(&f.paths.store, &f.paths.receipt).unwrap();
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    let f = Fixture::new(true);
    f.run(&mut Host::default()).unwrap();
    let valid = fs::read_to_string(&f.paths.receipt).unwrap();
    let duplicate = valid.replacen(
        "\"schemaVersion\":1",
        "\"schemaVersion\":1,\"schemaVersion\":1",
        1,
    );
    f.put(&f.paths.receipt, duplicate.as_bytes());
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    f.put(&f.paths.receipt, valid.as_bytes());
    fs::set_permissions(&f.paths.receipt, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
}

#[test]
fn enabled_noop_is_validated_and_consumed_without_desired_rewrite() {
    let f = Fixture::new(true);
    f.run(&mut Host::default()).unwrap();
    fs::remove_file(&f.paths.receipt).unwrap(); // Synthetic fresh trigger, test only.
    let original = fs::read(&f.paths.desired.file).unwrap();
    let inode = fs::metadata(&f.paths.desired.file).unwrap().ino();
    let mut host = Host::default();
    assert_eq!(
        f.run(&mut host),
        Ok(LoginTransactionOutcome::Consumed {
            desired_changed: false
        })
    );
    assert_eq!((host.empty, host.validated), (2, 1));
    assert_eq!(fs::read(&f.paths.desired.file).unwrap(), original);
    assert_eq!(fs::metadata(&f.paths.desired.file).unwrap().ino(), inode);
}

#[test]
fn changed_pending_receipt_after_desired_is_never_marked_consumed() {
    let f = Fixture::new(true);
    let publisher = Publisher {
        tamper: Some((Publication::Desired, f.paths.receipt.clone(),
            json!({"schemaVersion":1,"epochHash":"b".repeat(64),"ownershipGeneration":2,"phase":"pending"}).to_string().into_bytes())),
        ..Publisher::default()
    };
    assert_eq!(
        consume(
            &f.paths,
            2,
            "synthetic-epoch",
            &mut Host::default(),
            &publisher
        ),
        Err(LoginTransactionError::ManualRecoveryRequired)
    );
    assert!(receipt(&f.paths).unwrap().unwrap().phase == Phase::Pending);
}

#[test]
fn invalid_desired_modes_inputs_and_missing_template_refuse() {
    let f = Fixture::new(true);
    for epoch in ["".to_owned(), "x".repeat(129), "русский".to_owned()] {
        assert_eq!(
            consume_login(&f.paths, 2, &epoch, &mut Host::default()),
            Err(LoginTransactionError::InvalidInput)
        );
    }
    f.put(&f.paths.desired.file, b"private malformed input");
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::InvalidState)
    );
    fs::remove_file(&f.paths.desired.file).unwrap();
    fs::set_permissions(&f.paths.template, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::InvalidState)
    );
    fs::remove_file(&f.paths.template).unwrap();
    assert_eq!(
        f.run(&mut Host::default()),
        Err(LoginTransactionError::ValidationRejected)
    );
    assert!(!f.paths.receipt.exists());
}

#[test]
fn public_errors_are_fixed_bounded_english() {
    for error in [
        LoginTransactionError::InvalidInput,
        LoginTransactionError::Busy,
        LoginTransactionError::InvalidState,
        LoginTransactionError::OwnershipUnavailable,
        LoginTransactionError::EpochMismatch,
        LoginTransactionError::ValidationRejected,
        LoginTransactionError::SnapshotChanged,
        LoginTransactionError::ManualRecoveryRequired,
    ] {
        let text = error.to_string();
        assert!(text.is_ascii() && text.len() < 100);
        assert!(!text.contains("private malformed input"));
        assert!(!text.contains("vless://"));
        assert!(!text.contains("synthetic-epoch"));
    }
}
