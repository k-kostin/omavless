use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};
use tempfile::TempDir;

const BOOT: &str = "00000000-0000-4000-8000-000000000001";

fn fixture() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    root
}
fn open(root: &TempDir) -> Result<Journal, Error> {
    Journal::open_at(
        root.path(),
        BOOT.to_owned(),
        rustix::process::geteuid().as_raw(),
    )
}
fn write(root: &TempDir, bytes: &[u8]) {
    let path = root.path().join(RECORD);
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn normal_order_persists_each_crash_boundary() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    assert_eq!(journal.phase(), None);
    journal.begin(42).unwrap();
    assert_eq!(journal.read().unwrap().unwrap().phase, Phase::Applying);
    journal.applied_verified().unwrap();
    assert_eq!(journal.read().unwrap().unwrap().phase, Phase::Active);
    journal.begin_release().unwrap();
    assert_eq!(journal.read().unwrap().unwrap().phase, Phase::Releasing);
    journal.cleanup_verified().unwrap();
    assert_eq!(
        journal.read().unwrap().unwrap().phase,
        Phase::CleanupVerified
    );
    journal.finish_after_store_empty().unwrap();
    drop(journal);
    assert_eq!(open(&root).unwrap().phase(), None);
    assert!(!root.path().join(STAGING).exists());
}

#[test]
fn pending_and_quarantine_cannot_be_reset_or_reacquired() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    journal.begin(42).unwrap();
    assert_eq!(journal.finish_after_store_empty(), Err(Error::InvalidState));
    assert_eq!(journal.begin(43), Err(Error::InvalidState));
    journal.quarantine().unwrap();
    drop(journal);
    let mut reopened = open(&root).unwrap();
    assert_eq!(reopened.phase(), Some(Phase::Quarantined));
    assert_eq!(reopened.interface_index(), Some(42));
    assert!(reopened.requires_recovery());
    assert_eq!(reopened.applied_verified(), Err(Error::RecoveryRequired));
    assert_eq!(reopened.begin_release(), Err(Error::RecoveryRequired));
    assert_eq!(reopened.cleanup_verified(), Err(Error::RecoveryRequired));
    assert_eq!(
        reopened.finish_after_store_empty(),
        Err(Error::RecoveryRequired)
    );
}

#[test]
fn simultaneous_journal_owner_is_refused() {
    let root = fixture();
    let journal = open(&root).unwrap();
    assert_eq!(open(&root).unwrap_err(), Error::Refused);
    drop(journal);
    assert!(open(&root).is_ok());
}

#[test]
fn every_persisted_phase_requires_reconciliation_after_process_restart() {
    for phase in [
        Phase::Applying,
        Phase::Active,
        Phase::Releasing,
        Phase::CleanupVerified,
        Phase::Quarantined,
    ] {
        let root = fixture();
        let mut journal = open(&root).unwrap();
        journal.begin(42).unwrap();
        journal
            .persist(Record {
                schema: 1,
                boot: BOOT.into(),
                phase,
                index: 42,
            })
            .unwrap();
        drop(journal);
        let mut reopened = open(&root).unwrap();
        assert_eq!(reopened.phase(), Some(phase));
        assert!(reopened.requires_recovery());
        assert_eq!(reopened.begin(43), Err(Error::RecoveryRequired));
        assert_eq!(reopened.begin_release(), Err(Error::RecoveryRequired));
        assert_eq!(
            reopened.finish_after_store_empty(),
            Err(Error::RecoveryRequired)
        );
    }
}

#[test]
fn known_settled_partial_failure_has_cleanup_order() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    journal.begin(42).unwrap();
    journal.begin_release().unwrap();
    journal.cleanup_verified().unwrap();
    journal.finish_after_store_empty().unwrap();
}

#[test]
fn out_of_order_completion_never_records_success() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    assert_eq!(journal.applied_verified(), Err(Error::InvalidState));
    assert_eq!(journal.begin_release(), Err(Error::InvalidState));
    assert_eq!(journal.cleanup_verified(), Err(Error::InvalidState));
    for index in [0, i32::MAX as u32 + 1, u32::MAX] {
        assert_eq!(journal.begin(index), Err(Error::InvalidState));
    }
    assert!(root.path().read_dir().unwrap().next().is_none());
}

#[test]
fn foreign_boot_corrupt_duplicate_unknown_and_oversized_records_refuse() {
    let root = fixture();
    let valid = format!(r#"{{"schema":1,"boot":"{BOOT}","phase":"active","index":42}}"#);
    for invalid in [
        valid.replace(BOOT, "00000000-0000-4000-8000-000000000002"),
        valid.replace("\"schema\":1", "\"schema\":2"),
        valid.replace("\"index\":42", "\"index\":42,\"index\":43"),
        valid.replace("\"index\":42", "\"index\":42,\"private\":\"sentinel\""),
        valid.replace("active", "ready"),
        valid.replace(":42", ":true"),
        valid.replace(":42", ":0"),
        "[]".into(),
        "{}".into(),
        "private sentinel".repeat(100),
        valid.clone() + " {}",
    ] {
        write(&root, invalid.as_bytes());
        assert_eq!(open(&root).unwrap_err(), Error::RecoveryRequired);
    }
    write(&root, b"\xff");
    assert_eq!(open(&root).unwrap_err(), Error::RecoveryRequired);
}

#[test]
fn directory_permissions_owner_and_symlink_refuse() {
    let root = fixture();
    assert_eq!(
        Journal::open_at(
            root.path(),
            BOOT.into(),
            rustix::process::geteuid().as_raw() + 1
        )
        .unwrap_err(),
        Error::Refused
    );
    let link_root = fixture();
    symlink(root.path(), link_root.path().join("alias")).unwrap();
    assert_eq!(
        Journal::open_at(
            &link_root.path().join("alias"),
            BOOT.into(),
            rustix::process::geteuid().as_raw()
        )
        .unwrap_err(),
        Error::Refused
    );
    for mode in [0o777, 0o770, 0o755, 0o1700] {
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(open(&root).unwrap_err(), Error::Refused);
    }
}

#[test]
fn symlink_hardlink_mode_and_nonregular_record_refuse() {
    let outside = fixture();
    let sentinel = outside.path().join("sentinel");
    std::fs::write(&sentinel, b"untouched").unwrap();
    for kind in ["symlink", "hardlink", "directory", "fifo", "mode"] {
        let root = fixture();
        let path = root.path().join(RECORD);
        match kind {
            "symlink" => symlink(&sentinel, &path).unwrap(),
            "hardlink" => std::fs::hard_link(&sentinel, &path).unwrap(),
            "directory" => std::fs::create_dir(&path).unwrap(),
            "fifo" => fs::mknodat(
                fs::CWD,
                &path,
                fs::FileType::Fifo,
                Mode::RUSR | Mode::WUSR,
                0,
            )
            .unwrap(),
            _ => {
                write(&root, b"{}");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            }
        }
        assert_eq!(open(&root).unwrap_err(), Error::RecoveryRequired);
    }
    assert_eq!(std::fs::read(sentinel).unwrap(), b"untouched");
}

#[test]
fn uncertain_staging_file_is_preserved_not_auto_removed() {
    let root = fixture();
    std::fs::write(root.path().join(STAGING), b"partial").unwrap();
    assert_eq!(open(&root).unwrap_err(), Error::RecoveryRequired);
    assert_eq!(
        std::fs::read(root.path().join(STAGING)).unwrap(),
        b"partial"
    );
}

#[test]
fn failed_write_poisoning_prevents_followup_mutation() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    std::fs::write(root.path().join(STAGING), b"existing").unwrap();
    assert_eq!(journal.begin(42), Err(Error::Unavailable));
    assert_eq!(journal.begin(43), Err(Error::RecoveryRequired));
    assert_eq!(journal.applied_verified(), Err(Error::RecoveryRequired));
    assert!(!root.path().join(RECORD).exists());
}

#[test]
fn public_debug_errors_do_not_reveal_identity_or_input() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    journal.begin(9876543).unwrap();
    let debug = format!("{journal:?}");
    assert!(!debug.contains(BOOT) && !debug.contains("9876543"));
    assert!(!debug.contains(&root.path().display().to_string()));
    for error in [
        Error::Refused,
        Error::InvalidState,
        Error::RecoveryRequired,
        Error::Unavailable,
    ] {
        assert!(error.to_string().len() < 96);
        assert!(!error.to_string().contains("sentinel"));
    }
}

#[test]
fn boot_identifier_is_exact_and_bounded() {
    assert!(valid_boot(BOOT));
    for invalid in [
        "",
        "../boot",
        "00000000-0000-4000-8000-00000000000G",
        "000000000000-4000-8000-000000000001",
    ] {
        assert!(!valid_boot(invalid));
    }
}

#[test]
fn same_process_quarantine_blocks_every_followup_mutation() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    journal.begin(42).unwrap();
    journal.quarantine().unwrap();
    let saved = std::fs::read(root.path().join(RECORD)).unwrap();
    assert!(journal.requires_recovery());
    assert!(journal.begin(43).is_err());
    assert!(journal.applied_verified().is_err());
    assert!(journal.begin_release().is_err());
    assert!(journal.cleanup_verified().is_err());
    assert!(journal.finish_after_store_empty().is_err());
    assert_eq!(std::fs::read(root.path().join(RECORD)).unwrap(), saved);
    assert!(!root.path().join(STAGING).exists());
}

#[test]
fn failed_transition_preserves_previous_record_and_uncertain_staging() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    journal.begin(42).unwrap();
    let saved = std::fs::read(root.path().join(RECORD)).unwrap();
    std::fs::write(root.path().join(STAGING), b"uncertain-staging").unwrap();
    assert_eq!(journal.applied_verified(), Err(Error::Unavailable));
    assert!(journal.requires_recovery());
    assert_eq!(journal.begin_release(), Err(Error::RecoveryRequired));
    assert_eq!(journal.phase(), Some(Phase::Applying));
    assert_eq!(std::fs::read(root.path().join(RECORD)).unwrap(), saved);
    drop(journal);
    assert_eq!(open(&root).unwrap_err(), Error::RecoveryRequired);
    assert_eq!(
        std::fs::read(root.path().join(STAGING)).unwrap(),
        b"uncertain-staging"
    );
}

#[test]
fn failed_final_unlink_poisoning_never_claims_empty_or_reacquires() {
    let root = fixture();
    let mut journal = open(&root).unwrap();
    journal.begin(42).unwrap();
    journal.begin_release().unwrap();
    journal.cleanup_verified().unwrap();
    let record = root.path().join(RECORD);
    let saved_path = root.path().join("saved-synthetic-record");
    let saved = std::fs::read(&record).unwrap();
    // Deterministic unlink failure even when tests run as root. This is fault
    // injection in a private fixture, not an ordinary-user production attack.
    std::fs::rename(&record, &saved_path).unwrap();
    std::fs::create_dir(&record).unwrap();
    assert_eq!(journal.finish_after_store_empty(), Err(Error::Unavailable));
    assert!(journal.requires_recovery());
    assert_eq!(journal.phase(), Some(Phase::CleanupVerified));
    assert_eq!(
        journal.finish_after_store_empty(),
        Err(Error::RecoveryRequired)
    );
    assert_eq!(journal.begin(43), Err(Error::RecoveryRequired));
    assert!(record.is_dir());
    assert_eq!(std::fs::read(saved_path).unwrap(), saved);
}
