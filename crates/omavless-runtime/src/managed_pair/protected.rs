// SPDX-License-Identifier: MIT
//! Original protected-only package binding. No family-to-permit conversion.
use super::*;
use std::fs::Metadata;
use std::os::unix::fs::FileExt;

const CORE: &str = "897ada648fe975718ac1b7318702def5b826a9901797a0d13cdd333a012b9fcb";
const BROKER: &str = "4bbba825bc39209cd821af18f7a776825bf762c46a05e43f4354aca283009c70";
const RECEIPT: &str = "c6e283d9c8b4c2fa59163e6a363b93788a36b210f33d5d9dee2d8b5eec5bd8f2";
const CAPS: [u8; 20] = [
    1, 0, 0, 2, 0, 0x34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

struct Original {
    file: File,
    metadata: Metadata,
    path: PathBuf,
    digest: [u8; 32],
    limit: u64,
}
impl Original {
    fn capture(path: &Path, owner: u32, mode: u32, limit: u64) -> Result<Self, HostStepError> {
        let file = safe_file(path, owner, mode, limit)?;
        let metadata = file.metadata().map_err(|_| HostStepError::Prepare)?;
        let digest = hash(&file, metadata.len(), limit)?;
        let held = Self {
            file,
            metadata,
            path: path.into(),
            digest,
            limit,
        };
        held.recheck()?;
        Ok(held)
    }
    fn recheck(&self) -> Result<(), HostStepError> {
        for m in [self.file.metadata(), fs::symlink_metadata(&self.path)] {
            if !same(&self.metadata, &m.map_err(|_| HostStepError::Prepare)?) {
                return Err(HostStepError::Prepare);
            }
        }
        if hash(&self.file, self.metadata.len(), self.limit)? != self.digest {
            return Err(HostStepError::Prepare);
        }
        for m in [self.file.metadata(), fs::symlink_metadata(&self.path)] {
            if !same(&self.metadata, &m.map_err(|_| HostStepError::Prepare)?) {
                return Err(HostStepError::Prepare);
            }
        }
        Ok(())
    }
}
fn hash(file: &File, len: u64, limit: u64) -> Result<[u8; 32], HostStepError> {
    if len == 0 || len > limit {
        return Err(HostStepError::Prepare);
    }
    let mut hash = Sha256::new();
    let mut block = [0; 65536];
    let mut offset = 0;
    while offset < len {
        let take = ((len - offset) as usize).min(block.len());
        file.read_exact_at(&mut block[..take], offset)
            .map_err(|_| HostStepError::Prepare)?;
        hash.update(&block[..take]);
        offset += take as u64;
    }
    Ok(hash.finalize().into())
}
fn capabilities(core: &File) -> Result<(), HostStepError> {
    let mut bytes = [0; 21];
    let length = rustix::fs::fgetxattr(core, "security.capability", &mut bytes[..])
        .map_err(|_| HostStepError::Prepare)?;
    if length != CAPS.len() || bytes[..length] != CAPS {
        return Err(HostStepError::Prepare);
    }
    Ok(())
}

/// Three persistent originals. Core original is already retained by Bound.
/// No Debug/Clone/deserializer; identity alone is never coverage authority.
pub(crate) struct ProtectedPackage {
    originals: [Original; 3],
    identity: ProtectedPairIdentity,
}
impl ManagedPair {
    pub(crate) fn capture_protected(&self, core: &File) -> Result<ProtectedPackage, HostStepError> {
        self.verify_protected()?;
        let identity = self.protected_identity();
        exact(identity)?;
        let originals = [
            Original::capture(&self.selector_path, self.selector_owner, 0o600, 64)?,
            Original::capture(&self.broker_path, self.owner, 0o755, 32 * 1024 * 1024)?,
            Original::capture(&self.receipt_path, self.owner, 0o644, 8192)?,
        ];
        if originals[0].digest != <[u8; 32]>::from(Sha256::digest(SELECTION_BYTES))
            || originals[1].digest != identity.broker
            || originals[2].digest != identity.receipt
        {
            return Err(HostStepError::Prepare);
        }
        let held = ProtectedPackage {
            originals,
            identity,
        };
        held.recheck(self, core)?;
        Ok(held)
    }
}
fn exact(identity: ProtectedPairIdentity) -> Result<(), HostStepError> {
    if identity.core != decoded_sha(CORE)?
        || identity.broker != decoded_sha(BROKER)?
        || identity.receipt != decoded_sha(RECEIPT)?
    {
        return Err(HostStepError::Prepare);
    }
    Ok(())
}
impl ProtectedPackage {
    pub(crate) fn identity(&self) -> ProtectedPairIdentity {
        self.identity
    }
    pub(crate) fn recheck(&self, pair: &ManagedPair, core: &File) -> Result<(), HostStepError> {
        pair.verify_protected()?;
        exact(pair.protected_identity())?;
        if pair.protected_identity() != self.identity {
            return Err(HostStepError::Prepare);
        }
        for (held, path) in
            self.originals
                .iter()
                .zip([&pair.selector_path, &pair.broker_path, &pair.receipt_path])
        {
            if held.path != *path {
                return Err(HostStepError::Prepare);
            }
            held.recheck()?;
        }
        let before = core.metadata().map_err(|_| HostStepError::Prepare)?;
        if !same(
            &before,
            &fs::symlink_metadata(&pair.core_path).map_err(|_| HostStepError::Prepare)?,
        ) || hash(core, before.len(), 128 * 1024 * 1024)? != self.identity.core
        {
            return Err(HostStepError::Prepare);
        }
        capabilities(core)?;
        for m in [core.metadata(), fs::symlink_metadata(&pair.core_path)] {
            if !same(&before, &m.map_err(|_| HostStepError::Prepare)?) {
                return Err(HostStepError::Prepare);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(path: &Path, raw: &[u8], mode: u32) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    #[test]
    fn protected_exact_triple_changes_refuse_without_family_promotion() {
        let identity = ProtectedPairIdentity {
            core: decoded_sha(CORE).unwrap(),
            broker: decoded_sha(BROKER).unwrap(),
            receipt: decoded_sha(RECEIPT).unwrap(),
        };
        assert!(exact(identity).is_ok()); // consistency only, not a Coverage
        for member in 0..3 {
            let mut changed = identity;
            match member {
                0 => changed.core[0] ^= 1,
                1 => changed.broker[0] ^= 1,
                _ => changed.receipt[0] ^= 1,
            }
            assert!(exact(changed).is_err());
        }
    }
    #[test]
    fn protected_original_rejects_same_bytes_new_inode_each_role() {
        for mode in [0o600, 0o644, 0o755] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("original");
            bytes(&path, b"inert fixture bytes", mode);
            let held = Original::capture(&path, nix::unistd::getuid().as_raw(), mode, 64).unwrap();
            held.recheck().unwrap();
            fs::rename(&path, root.path().join("displaced")).unwrap();
            bytes(&path, b"inert fixture bytes", mode);
            assert!(held.recheck().is_err());
        }
    }
    #[test]
    fn protected_original_mutation_mode_and_symlink_refuse() {
        for cut in 0..3 {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("original");
            bytes(&path, b"inert fixture bytes", 0o600);
            let held = Original::capture(&path, nix::unistd::getuid().as_raw(), 0o600, 64).unwrap();
            match cut {
                0 => bytes(&path, b"other fixture bytes", 0o600),
                1 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
                _ => {
                    let old = root.path().join("old");
                    fs::rename(&path, &old).unwrap();
                    std::os::unix::fs::symlink(old, &path).unwrap();
                }
            }
            assert!(held.recheck().is_err());
        }
    }
    #[test]
    fn protected_missing_capabilities_refuse_without_setting_any() {
        let file = tempfile::tempfile().unwrap();
        assert!(capabilities(&file).is_err());
    }
}
