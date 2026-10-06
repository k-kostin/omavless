// SPDX-License-Identifier: MIT
//! Close-qualified normal package paths; opt-in candidate, not distribution proof.
//! No developer receipt conversion, repair, installer or exported authority.
use super::super::{CurrentImage, ExecutableEvidence, FileIdentity, Session};
use super::{Refusal, Result, elf_architecture};
use crate::managed_pair::{RELEASE_CORE, RELEASE_RECEIPT, SELECTION_BYTES, SELECTOR};
use nix::fcntl::{AtFlags, OFlag, open, openat};
use nix::sys::stat::{Mode, fstatat};
use std::cell::Cell;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
struct DirIdentity {
    dev: u64,
    ino: u64,
    uid: u32,
    gid: u32,
    mode: u32,
}
impl DirIdentity {
    fn of(m: &Metadata, uid: u32) -> Result<Self> {
        if !m.is_dir() || ![0, uid].contains(&m.uid()) || m.mode() & 0o7022 != 0 {
            return Err(Refusal::Object);
        }
        Ok(Self {
            dev: m.dev(),
            ino: m.ino(),
            uid: m.uid(),
            gid: m.gid(),
            mode: m.mode(),
        })
    }
}
struct Directory {
    file: File,
    name: PathBuf,
    identity: DirIdentity,
}
struct Chain {
    dirs: Vec<Directory>,
    uid: u32,
}
impl Chain {
    fn open(path: &Path, uid: u32, private: bool) -> Result<Self> {
        if !path.is_absolute() || path.as_os_str().as_encoded_bytes().len() > 4096 {
            return Err(Refusal::Object);
        }
        let root = File::from(
            open(
                "/",
                OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Refusal::Object)?,
        );
        let identity = DirIdentity::of(&root.metadata().map_err(|_| Refusal::Object)?, uid)?;
        let mut dirs = vec![Directory {
            file: root,
            name: PathBuf::from("/"),
            identity,
        }];
        for part in path.components().skip(1) {
            let Component::Normal(name) = part else {
                return Err(Refusal::Object);
            };
            if dirs.len() >= 16 {
                return Err(Refusal::Object);
            }
            let file = File::from(
                openat(
                    &dirs.last().ok_or(Refusal::Object)?.file,
                    Path::new(name),
                    OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| Refusal::Object)?,
            );
            let identity = DirIdentity::of(&file.metadata().map_err(|_| Refusal::Object)?, uid)?;
            dirs.push(Directory {
                file,
                name: PathBuf::from(name),
                identity,
            });
        }
        let final_id = dirs.last().ok_or(Refusal::Object)?.identity;
        if private && (final_id.uid != uid || final_id.mode & 0o7777 != 0o700) {
            return Err(Refusal::Object);
        }
        let result = Self { dirs, uid };
        result.check()?;
        Ok(result)
    }
    fn parent(&self) -> Result<&File> {
        Ok(&self.dirs.last().ok_or(Refusal::Object)?.file)
    }
    fn check(&self) -> Result<()> {
        for (i, d) in self.dirs.iter().enumerate() {
            if DirIdentity::of(&d.file.metadata().map_err(|_| Refusal::Object)?, self.uid)?
                != d.identity
            {
                return Err(Refusal::Object);
            }
            let current = if i == 0 {
                fstatat(nix::fcntl::AT_FDCWD, "/", AtFlags::AT_SYMLINK_NOFOLLOW)
            } else {
                fstatat(
                    &self.dirs[i - 1].file,
                    &d.name,
                    AtFlags::AT_SYMLINK_NOFOLLOW,
                )
            }
            .map_err(|_| Refusal::Object)?;
            if (
                current.st_dev,
                current.st_ino,
                current.st_uid,
                current.st_gid,
                current.st_mode,
            ) != (
                d.identity.dev,
                d.identity.ino,
                d.identity.uid,
                d.identity.gid,
                d.identity.mode,
            ) {
                return Err(Refusal::Object);
            }
        }
        Ok(())
    }
}
struct Member {
    file: File,
    identity: FileIdentity,
    gid: u32,
    name: &'static str,
}
impl Member {
    fn open(parent: &File, name: &'static str, uid: u32, mode: u32, limit: u64) -> Result<Self> {
        let file = File::from(
            openat(
                parent,
                name,
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Refusal::Object)?,
        );
        let m = file.metadata().map_err(|_| Refusal::Object)?;
        if m.uid() != uid || m.mode() & 0o7777 != mode || m.len() > limit {
            return Err(Refusal::Object);
        }
        let identity = FileIdentity::capture(&m).ok_or(Refusal::Object)?;
        let result = Self {
            file,
            identity,
            gid: m.gid(),
            name,
        };
        result.check(parent)?;
        Ok(result)
    }
    fn check(&self, parent: &File) -> Result<()> {
        let held = self.file.metadata().map_err(|_| Refusal::Object)?;
        if FileIdentity::capture(&held) != Some(self.identity) || held.gid() != self.gid {
            return Err(Refusal::Object);
        }
        let m = fstatat(parent, self.name, AtFlags::AT_SYMLINK_NOFOLLOW)
            .map_err(|_| Refusal::Object)?;
        if (
            m.st_dev,
            m.st_ino,
            m.st_mode,
            m.st_uid,
            m.st_gid,
            m.st_nlink,
            m.st_size,
            m.st_ctime,
            m.st_ctime_nsec,
            m.st_mtime,
            m.st_mtime_nsec,
        ) != (
            self.identity.inode.0,
            self.identity.inode.1,
            self.identity.mode,
            self.identity.owner,
            self.gid,
            1,
            self.identity.size as i64,
            self.identity.change.0,
            self.identity.change.1,
            self.identity.modified.0,
            self.identity.modified.1,
        ) {
            return Err(Refusal::Object);
        }
        Ok(())
    }
    fn read(&mut self, limit: u64) -> Result<Vec<u8>> {
        use std::io::Seek;
        if !self.identity.matches(&self.file) {
            return Err(Refusal::Object);
        }
        self.file.rewind().map_err(|_| Refusal::Object)?;
        let mut data = Vec::new();
        (&mut self.file)
            .take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|_| Refusal::Object)?;
        if data.len() as u64 != self.identity.size
            || data.len() as u64 > limit
            || !self.identity.matches(&self.file)
        {
            return Err(Refusal::Object);
        }
        Ok(data)
    }
}
pub(in crate::conditional_close_candidate) struct Evidence {
    package: Chain,
    receipt_parent: Chain,
    selection_parent: Chain,
    core: Member,
    broker: Member,
    receipt: Member,
    selection: Member,
    original: Original,
}
struct Original {
    identity: Arc<()>,
    refused: Cell<bool>,
}
impl Original {
    fn new(identity: &Arc<()>) -> Self {
        Self {
            identity: Arc::clone(identity),
            refused: Cell::new(false),
        }
    }
    fn matches(&self, identity: &Arc<()>) -> bool {
        !self.refused.get() && Arc::ptr_eq(&self.identity, identity)
    }
    fn refuse(&self) {
        self.refused.set(true);
    }
    fn check(&self, identity: &Arc<()>) -> Result<()> {
        if !self.matches(identity) {
            self.refuse();
            return Err(Refusal::Child);
        }
        Ok(())
    }
}

impl Evidence {
    pub(in crate::conditional_close_candidate) fn belongs_to(&self, identity: &Arc<()>) -> bool {
        self.original.matches(identity)
    }
    pub(in crate::conditional_close_candidate) fn capture(
        session: &mut Session,
        config: &Path,
    ) -> Result<Option<Self>> {
        let flight = session.image_proof_flight().map_err(|_| Refusal::Child)?;
        session
            .check_with_flight(&flight)
            .map_err(|_| Refusal::Child)?;
        let until = session
            .deadline
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(3));
        if Instant::now() >= until {
            return Err(Refusal::Expired);
        }
        let package = Chain::open(
            Path::new(RELEASE_CORE).parent().ok_or(Refusal::Object)?,
            0,
            false,
        )?;
        let receipt_parent = Chain::open(
            Path::new(RELEASE_RECEIPT).parent().ok_or(Refusal::Object)?,
            0,
            false,
        )?;
        let mut receipt = Member::open(
            receipt_parent.parent()?,
            "source-receipt.json",
            0,
            0o644,
            8192,
        )?;
        let raw = receipt.read(8192)?;
        // Old DNS/TUN receipt is intentionally non-authorizing. No bool,
        // developer token/schema or passive research receipt is promoted.
        #[derive(serde::Deserialize)]
        struct Header {
            schema: serde_json::Value,
        }
        let header: Header = serde_json::from_slice(&raw).map_err(|_| Refusal::Receipt)?;
        if header.schema == serde_json::json!(1) {
            return Ok(None);
        }
        let hashes = crate::managed_close_receipt::decode(&raw, std::env::consts::ARCH)
            .map_err(|_| Refusal::Receipt)?;
        let mut core = Member::open(package.parent()?, "mihomo", 0, 0o755, 128 * 1024 * 1024)?;
        let mut broker = Member::open(
            package.parent()?,
            "omavless-dns-broker",
            0,
            0o755,
            32 * 1024 * 1024,
        )?;
        elf_architecture(&mut core.file, std::env::consts::ARCH)?;
        elf_architecture(&mut broker.file, std::env::consts::ARCH)?;
        // Reuse only hashes of the SAME already-retained source/image object,
        // never copied attestation facts or a second full core hash per row.
        let image = session.executable.as_ref().ok_or(Refusal::Child)?;
        if image.image_identity != core.identity
            || image.source_identity != core.identity
            || image.digests != Some((hashes.core, hashes.core))
        {
            return Err(Refusal::Child);
        }
        if ExecutableEvidence::hash(&mut broker.file, broker.identity, until) != Some(hashes.broker)
        {
            return Err(Refusal::Object);
        }
        let selection_parent = Chain::open(config, session.binding.uid, true)?;
        let mut selection = Member::open(
            selection_parent.parent()?,
            SELECTOR,
            session.binding.uid,
            0o600,
            64,
        )?;
        if selection.read(64)? != SELECTION_BYTES {
            return Err(Refusal::Object);
        }
        let result = Self {
            package,
            receipt_parent,
            selection_parent,
            core,
            broker,
            receipt,
            selection,
            original: Original::new(&session.identity),
        };
        result.check(
            &session.identity,
            session.executable.as_ref().ok_or(Refusal::Child)?,
            session.binding.pid,
            flight.current.as_ref(),
        )?;
        session
            .check_with_flight(&flight)
            .map_err(|_| Refusal::Child)?;
        if Instant::now() >= until {
            return Err(Refusal::Expired);
        }
        Ok(Some(result))
    }
    pub(in crate::conditional_close_candidate) fn check(
        &self,
        original: &Arc<()>,
        image: &ExecutableEvidence,
        pid: u32,
        current: Option<&CurrentImage>,
    ) -> Result<()> {
        let result = (|| {
            self.original.check(original)?;
            self.package.check()?;
            self.receipt_parent.check()?;
            self.selection_parent.check()?;
            self.core.check(self.package.parent()?)?;
            self.broker.check(self.package.parent()?)?;
            self.receipt.check(self.receipt_parent.parent()?)?;
            self.selection.check(self.selection_parent.parent()?)?;
            if image.source_path != Path::new(RELEASE_CORE)
                || image.image_identity != self.core.identity
                || image.source_identity != self.core.identity
                || !self.core.identity.matches(&image.image)
                || !self.core.identity.matches(&image.source)
                || !image.check_current(pid, original, current)
                || image.image.metadata().map_err(|_| Refusal::Child)?.gid() != self.core.gid
                || image.source.metadata().map_err(|_| Refusal::Child)?.gid() != self.core.gid
            {
                return Err(Refusal::Child);
            }
            Ok(())
        })();
        if result.is_err() {
            self.original.refuse();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[test]
    fn qualification_binding_never_revives_after_observed_drift_or_foreign_session() {
        let identity = Arc::new(());
        let other = Arc::new(());
        let original = Original::new(&identity);
        assert!(original.matches(&identity));
        assert!(original.check(&other).is_err());
        assert!(!original.matches(&identity));
        assert!(!original.matches(&other));
    }
    #[test]
    fn retained_member_detects_same_byte_replacement_mode_link_and_content_drift() {
        let root = crate::test_temp::directory("q-pair").unwrap();
        let uid = nix::unistd::getuid().as_raw();
        let chain = Chain::open(&root, uid, true).unwrap();
        let path = root.join("member");
        fs::write(&path, b"public fixture").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let member = Member::open(chain.parent().unwrap(), "member", uid, 0o600, 64).unwrap();
        member.check(chain.parent().unwrap()).unwrap();
        let replacement = root.join("replacement");
        fs::write(&replacement, b"public fixture").unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert!(member.check(chain.parent().unwrap()).is_err());
        let current = Member::open(chain.parent().unwrap(), "member", uid, 0o600, 64).unwrap();
        fs::hard_link(&path, root.join("linked")).unwrap();
        assert!(current.check(chain.parent().unwrap()).is_err());
        fs::remove_file(root.join("linked")).unwrap();
        let current = Member::open(chain.parent().unwrap(), "member", uid, 0o600, 64).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(current.check(chain.parent().unwrap()).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let current = Member::open(chain.parent().unwrap(), "member", uid, 0o600, 64).unwrap();
        fs::write(&path, b"changed bytes!").unwrap();
        assert!(current.check(chain.parent().unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn secure_chain_and_member_refuse_links_wrong_owner_or_unknown_kind() {
        let root = crate::test_temp::directory("q-path").unwrap();
        let uid = nix::unistd::getuid().as_raw();
        let chain = Chain::open(&root, uid, true).unwrap();
        fs::write(root.join("member"), b"public").unwrap();
        fs::set_permissions(root.join("member"), fs::Permissions::from_mode(0o600)).unwrap();
        symlink("member", root.join("link")).unwrap();
        assert!(Member::open(chain.parent().unwrap(), "link", uid, 0o600, 64).is_err());
        assert!(
            Member::open(
                chain.parent().unwrap(),
                "member",
                uid.wrapping_add(1),
                0o600,
                64
            )
            .is_err()
        );
        assert!(Member::open(chain.parent().unwrap(), "member", uid, 0o600, 2).is_err());
        assert!(Member::open(chain.parent().unwrap(), ".", uid, 0o600, 64).is_err());
        let old = root.with_extension("displaced");
        fs::rename(&root, &old).unwrap();
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(chain.check().is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(old).unwrap();
    }
}
