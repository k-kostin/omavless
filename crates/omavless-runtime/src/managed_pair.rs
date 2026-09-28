// SPDX-License-Identifier: MIT

//! Explicit, durable user selection of the separately enrolled DNS pair.
//! The selector contains no credentials and never grants broker authority.
//! Root enrollment remains a separate requirement; no legacy fallback follows
//! a present but invalid selector or a missing/replaced package component.

use crate::lifecycle::HostStepError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) const CORE: &str = "/usr/lib/omavless-dns-experimental/mihomo";
const BROKER: &str = "/usr/lib/omavless/omavless-dns-broker";
const RECEIPT: &str = "/usr/share/omavless-dns-experimental/source-receipt.json";
pub(crate) const SELECTOR: &str = "managed-dns-selection";
pub(crate) const SELECTION_BYTES: &[u8] = b"managed-dns-source-pair-v1\n";
const MIHOMO_COMMIT: &str = "ab405bad5beeeac8b003bb01f60f134f6df54471";
const SING_TUN_COMMIT: &str = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754";
const CORE_PATCH: &str = "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37";
const TUN_PATCH: &str = "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab";

#[derive(Deserialize)]
struct Receipt {
    schema: u32,
    architecture: String,
    mihomo_commit: String,
    sing_tun_commit: String,
    patch_sha256: std::collections::HashMap<String, String>,
    go_build_tags: String,
    go_dependency_mode: String,
    sha256: std::collections::HashMap<String, String>,
}

pub(crate) struct ManagedPair {
    selector_path: PathBuf,
    selector_owner: u32,
    core_path: PathBuf,
    broker_path: PathBuf,
    receipt_path: PathBuf,
    owner: u32,
    receipt_sha: [u8; 32],
    core_sha: [u8; 32],
    broker_sha: [u8; 32],
}

struct PackageHashes {
    receipt: [u8; 32],
    core: [u8; 32],
    broker: [u8; 32],
}

fn safe_file(path: &Path, owner: u32, mode: u32, max_bytes: u64) -> Result<File, HostStepError> {
    let parent = fs::symlink_metadata(path.parent().ok_or(HostStepError::Prepare)?)
        .map_err(|_| HostStepError::Prepare)?;
    if !parent.is_dir()
        || parent.file_type().is_symlink()
        || parent.uid() != owner
        || parent.permissions().mode() & 0o022 != 0
    {
        return Err(HostStepError::Prepare);
    }
    let before = fs::symlink_metadata(path).map_err(|_| HostStepError::Prepare)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.uid() != owner
        || before.nlink() != 1
        || before.permissions().mode() & 0o7777 != mode
        || before.len() == 0
        || before.len() > max_bytes
    {
        return Err(HostStepError::Prepare);
    }
    let file = File::open(path).map_err(|_| HostStepError::Prepare)?;
    let after = file.metadata().map_err(|_| HostStepError::Prepare)?;
    if after.dev() != before.dev() || after.ino() != before.ino() {
        return Err(HostStepError::Prepare);
    }
    Ok(file)
}

fn bounded_bytes(
    path: &Path,
    owner: u32,
    mode: u32,
    max_bytes: u64,
) -> Result<Vec<u8>, HostStepError> {
    let mut bytes = Vec::new();
    safe_file(path, owner, mode, max_bytes)?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| HostStepError::Prepare)?;
    if bytes.len() as u64 > max_bytes {
        return Err(HostStepError::Prepare);
    }
    Ok(bytes)
}

fn sha256_file(path: &Path, owner: u32, max_bytes: u64) -> Result<[u8; 32], HostStepError> {
    let mut file = safe_file(path, owner, 0o755, max_bytes)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut block = [0_u8; 65536];
    loop {
        let read = file.read(&mut block).map_err(|_| HostStepError::Prepare)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > max_bytes {
            return Err(HostStepError::Prepare);
        }
        hasher.update(&block[..read]);
    }
    Ok(hasher.finalize().into())
}

fn decoded_sha(raw: &str) -> Result<[u8; 32], HostStepError> {
    let mut value = [0_u8; 32];
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(HostStepError::Prepare);
    }
    for (index, pair) in raw.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| HostStepError::Prepare)?;
        value[index] = u8::from_str_radix(text, 16).map_err(|_| HostStepError::Prepare)?;
    }
    Ok(value)
}

impl ManagedPair {
    pub(crate) fn detect(config_directory: &Path, uid: u32) -> Result<Option<Self>, HostStepError> {
        Self::detect_at(
            &config_directory.join(SELECTOR),
            Path::new(CORE),
            Path::new(BROKER),
            Path::new(RECEIPT),
            uid,
            0,
        )
    }

    fn detect_at(
        selector: &Path,
        core: &Path,
        broker: &Path,
        receipt: &Path,
        uid: u32,
        package_owner: u32,
    ) -> Result<Option<Self>, HostStepError> {
        match fs::symlink_metadata(selector) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(HostStepError::Prepare),
            Ok(_) => (),
        }
        if bounded_bytes(selector, uid, 0o600, 64)? != SELECTION_BYTES {
            return Err(HostStepError::Prepare);
        }
        let hashes = Self::validate_package_at(core, broker, receipt, package_owner)?;
        let selection = Self {
            selector_path: selector.to_path_buf(),
            selector_owner: uid,
            core_path: core.to_path_buf(),
            broker_path: broker.to_path_buf(),
            receipt_path: receipt.to_path_buf(),
            owner: package_owner,
            receipt_sha: hashes.receipt,
            core_sha: hashes.core,
            broker_sha: hashes.broker,
        };
        selection.verify()?;
        Ok(Some(selection))
    }

    fn validate_package_at(
        core: &Path,
        broker: &Path,
        receipt: &Path,
        package_owner: u32,
    ) -> Result<PackageHashes, HostStepError> {
        let bytes = bounded_bytes(receipt, package_owner, 0o644, 8192)?;
        let value: Receipt = serde_json::from_slice(&bytes).map_err(|_| HostStepError::Prepare)?;
        if value.schema != 1
            || value.architecture != std::env::consts::ARCH
            || value.mihomo_commit != MIHOMO_COMMIT
            || value.sing_tun_commit != SING_TUN_COMMIT
            || value.go_build_tags != "with_gvisor"
            || value.go_dependency_mode != "vendor"
            || value.patch_sha256.len() != 2
            || value
                .patch_sha256
                .get("mihomo-dns-broker.patch")
                .map(String::as_str)
                != Some(CORE_PATCH)
            || value
                .patch_sha256
                .get("sing-tun-descriptor.patch")
                .map(String::as_str)
                != Some(TUN_PATCH)
        {
            return Err(HostStepError::Prepare);
        }
        let core_sha = decoded_sha(value.sha256.get("mihomo").ok_or(HostStepError::Prepare)?)?;
        let broker_sha = decoded_sha(
            value
                .sha256
                .get("omavless-dns-broker")
                .ok_or(HostStepError::Prepare)?,
        )?;
        if sha256_file(core, package_owner, 128 * 1024 * 1024)? != core_sha
            || sha256_file(broker, package_owner, 32 * 1024 * 1024)? != broker_sha
        {
            return Err(HostStepError::Prepare);
        }
        Ok(PackageHashes {
            receipt: Sha256::digest(&bytes).into(),
            core: core_sha,
            broker: broker_sha,
        })
    }

    pub(crate) fn validate_package() -> Result<(), HostStepError> {
        Self::validate_package_at(Path::new(CORE), Path::new(BROKER), Path::new(RECEIPT), 0)?;
        Ok(())
    }

    pub(crate) fn verify(&self) -> Result<(), HostStepError> {
        // A running daemon must not keep using an opted-in core after the
        // user's selection has been removed or replaced. Selection changes
        // still require a disconnected restart to take effect.
        if bounded_bytes(&self.selector_path, self.selector_owner, 0o600, 64)? != SELECTION_BYTES
            || sha256_file(&self.core_path, self.owner, 128 * 1024 * 1024)? != self.core_sha
            || sha256_file(&self.broker_path, self.owner, 32 * 1024 * 1024)? != self.broker_sha
        {
            return Err(HostStepError::Prepare);
        }
        // Refuse a replaced receipt even if the old binaries remain present.
        let bytes = bounded_bytes(&self.receipt_path, self.owner, 0o644, 8192)?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.receipt_sha {
            return Err(HostStepError::Prepare);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;

    fn write_mode(path: &Path, bytes: &[u8], mode: u32) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn absent_selector_is_not_opt_in_and_valid_pair_is_pinned() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let package = root.path().join("package");
        fs::create_dir(&config).unwrap();
        fs::create_dir(&package).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o700)).unwrap();
        let selector = config.join(SELECTOR);
        let core = package.join("mihomo");
        let broker = package.join("broker");
        let receipt = package.join("receipt.json");
        let uid = nix::unistd::getuid().as_raw();
        assert!(
            ManagedPair::detect_at(&selector, &core, &broker, &receipt, uid, uid)
                .unwrap()
                .is_none()
        );
        write_mode(&selector, SELECTION_BYTES, 0o600);
        write_mode(&core, b"synthetic core", 0o755);
        write_mode(&broker, b"synthetic broker", 0o755);
        let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        let document = json!({
            "schema": 1, "architecture": std::env::consts::ARCH,
            "mihomo_commit": MIHOMO_COMMIT, "sing_tun_commit": SING_TUN_COMMIT,
            "patch_sha256": {
                "mihomo-dns-broker.patch": CORE_PATCH,
                "sing-tun-descriptor.patch": TUN_PATCH,
            },
            "go_build_tags": "with_gvisor", "go_dependency_mode": "vendor",
            "sha256": {"mihomo": hash(b"synthetic core"),
                       "omavless-dns-broker": hash(b"synthetic broker")},
        });
        write_mode(
            &receipt,
            serde_json::to_string(&document).unwrap().as_bytes(),
            0o644,
        );
        let pair = ManagedPair::detect_at(&selector, &core, &broker, &receipt, uid, uid)
            .unwrap()
            .unwrap();
        assert!(pair.verify().is_ok());
        write_mode(&selector, b"changed selection", 0o600);
        assert!(pair.verify().is_err());
        write_mode(&selector, SELECTION_BYTES, 0o600);
        fs::remove_file(&selector).unwrap();
        assert!(pair.verify().is_err());
        write_mode(&selector, SELECTION_BYTES, 0o600);
        write_mode(&core, b"changed core", 0o755);
        assert!(pair.verify().is_err());
        write_mode(&core, b"synthetic core", 0o755);
        write_mode(&receipt, b"{}", 0o644);
        assert!(pair.verify().is_err());
        write_mode(
            &receipt,
            serde_json::to_string(&document).unwrap().as_bytes(),
            0o644,
        );
        fs::remove_file(&broker).unwrap();
        symlink(&core, &broker).unwrap();
        assert!(pair.verify().is_err());
    }

    #[test]
    fn malformed_or_linked_selection_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        fs::create_dir(&config).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o700)).unwrap();
        let selector = config.join(SELECTOR);
        let uid = nix::unistd::getuid().as_raw();
        let package = root.path().join("missing");
        write_mode(&selector, b"other", 0o600);
        assert!(ManagedPair::detect_at(&selector, &package, &package, &package, uid, uid).is_err());
        write_mode(&selector, SELECTION_BYTES, 0o644);
        assert!(ManagedPair::detect_at(&selector, &package, &package, &package, uid, uid).is_err());
        fs::remove_file(&selector).unwrap();
        symlink("missing", &selector).unwrap();
        assert!(ManagedPair::detect_at(&selector, &package, &package, &package, uid, uid).is_err());
    }
}
