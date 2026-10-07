// SPDX-License-Identifier: MIT
use super::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const EPOCH: &str = "11111111111111111111111111111111";
const NEXT: &str = "22222222222222222222222222222222";
struct Fixture {
    root: PathBuf,
    paths: CutoverPaths,
    uid: u32,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::var_os("HOME").unwrap();
        let root = crate::test_temp::directory_under(Path::new(&home), "epoch-proof").unwrap();
        let uid = nix::unistd::Uid::current().as_raw();
        let runtime = root.join("runtime");
        let paths = CutoverPaths::below(&runtime, &root.join("state"), uid);
        for path in [&root, &runtime, &root.join("state"), &paths.state_directory] {
            fs::create_dir_all(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self::put(
            &paths.ownership_marker,
            br#"{"schemaVersion":1,"generation":2,"phase":"rust"}"#,
        );
        let f = Self { root, paths, uid };
        f.receipt(EPOCH);
        f
    }
    fn put(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn receipt(&self, epoch: &str) {
        let hash = format!(
            "{:x}",
            Sha256::digest([b"omavless-login-epoch-v1\0".as_slice(), epoch.as_bytes()].concat())
        );
        Self::put(&self.paths.runtime_base.join(MEMBER), serde_json::json!({"schemaVersion":1,"epochHash":hash,"ownershipGeneration":2,"phase":"consumed"}).to_string().as_bytes());
    }
    fn proof<'a>(&'a self, lock: &'a MigrationLock) -> Result<CurrentEpochProof<'a>> {
        CurrentEpochProof::synthetic(&self.paths, self.uid, 2, lock, |_| Ok(EPOCH.into()))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn exact_consumed_identity_is_not_normal_pending_permission() {
    let f = Fixture::new();
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    let receipt = fs::read(f.paths.runtime_base.join(MEMBER)).unwrap();
    Fixture::put(
        &f.paths.state_directory.join("restore-disposition.complete"),
        b"synthetic fence",
    );
    let mut proof = f.proof(&lock).unwrap();
    proof.recheck(&f.paths, f.uid, 2, &lock).unwrap();
    assert!(
        crate::login_transaction::check_current_receipt(&f.paths, f.uid, &lock, 2, EPOCH).is_err()
    );
    assert!(
        crate::login_transaction::check_startup_receipt(&f.paths, f.uid, &lock, Some(2)).is_err()
    );
    assert_eq!(
        fs::read(f.paths.runtime_base.join(MEMBER)).unwrap(),
        receipt
    );
    // A source-tree executable and arbitrary synthetic paths cannot claim the
    // package-fixed production source, even when receipt contents match.
    assert!(CurrentEpochProof::capture(&f.paths, f.uid, 2, &lock).is_err());
}

#[test]
fn missing_old_pending_duplicate_unknown_and_wrong_generation_refuse() {
    for variant in 0..8 {
        let f = Fixture::new();
        let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
        let path = f.paths.runtime_base.join(MEMBER);
        match variant {
            0 => fs::remove_file(&path).unwrap(),
            1 => f.receipt(NEXT),
            2..=4 => {
                let mut json: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                match variant {
                    2 => json["phase"] = "pending".into(),
                    3 => json["ownershipGeneration"] = 3.into(),
                    _ => json["extra"] = true.into(),
                }
                Fixture::put(&path, &serde_json::to_vec(&json).unwrap());
            }
            5 => Fixture::put(&path, br#"{"phase":"consumed","phase":"consumed"}"#),
            6 => Fixture::put(&path, &vec![b' '; LIMIT + 1]),
            _ => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
        }
        assert!(f.proof(&lock).is_err(), "variant={variant}");
    }
}

#[test]
fn system_source_failures_and_epoch_change_during_acquisition_refuse() {
    let f = Fixture::new();
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    for fail_at in 0..6 {
        let mut calls = 0;
        assert!(
            CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
                let fail = calls == fail_at;
                calls += 1;
                if fail {
                    Err(Error::Invocation)
                } else {
                    Ok(EPOCH.into())
                }
            })
            .is_err()
        );
    }
    let mut epochs = 0;
    assert!(
        CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |is_epoch| {
            if is_epoch {
                epochs += 1;
            }
            Ok(if epochs > 1 { NEXT } else { EPOCH }.into())
        })
        .is_err()
    );
}

#[test]
fn original_receipt_and_runtime_descriptors_reject_same_byte_replacement() {
    for variant in 0..5 {
        let f = Fixture::new();
        let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
        let mut proof = f.proof(&lock).unwrap();
        let path = f.paths.runtime_base.join(MEMBER);
        let raw = fs::read(&path).unwrap();
        match variant {
            0 => {
                fs::rename(&path, path.with_extension("old")).unwrap();
                Fixture::put(&path, &raw);
            }
            1 => {
                fs::rename(&path, path.with_extension("old")).unwrap();
                symlink(path.with_extension("old"), &path).unwrap();
            }
            2 => fs::hard_link(&path, path.with_extension("extra")).unwrap(),
            3 => {
                fs::rename(&f.paths.runtime_base, f.root.join("old-runtime")).unwrap();
                fs::create_dir(&f.paths.runtime_base).unwrap();
                fs::set_permissions(&f.paths.runtime_base, fs::Permissions::from_mode(0o700))
                    .unwrap();
                Fixture::put(&path, &raw);
            }
            _ => f.receipt(NEXT),
        }
        assert!(proof.recheck(&f.paths, f.uid, 2, &lock).is_err());
    }
}

#[test]
fn receipt_injection_in_last_source_callback_and_later_epoch_drift_refuse() {
    let f = Fixture::new();
    let lock = MigrationLock::acquire(&f.paths, f.uid).unwrap();
    let changed = Arc::new(AtomicBool::new(false));
    let source = Arc::clone(&changed);
    let mut proof = CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
        Ok(if source.load(Ordering::SeqCst) {
            NEXT
        } else {
            EPOCH
        }
        .into())
    })
    .unwrap();
    changed.store(true, Ordering::SeqCst);
    assert!(proof.recheck(&f.paths, f.uid, 2, &lock).is_err());
    drop(proof);
    let path = f.paths.runtime_base.join(MEMBER);
    let mut call = 0;
    assert!(
        CurrentEpochProof::synthetic(&f.paths, f.uid, 2, &lock, move |_| {
            call += 1;
            if call == 6 {
                let raw = fs::read(&path).unwrap();
                fs::rename(&path, path.with_extension("old")).unwrap();
                Fixture::put(&path, &raw);
            }
            Ok(EPOCH.into())
        })
        .is_err()
    );
}
