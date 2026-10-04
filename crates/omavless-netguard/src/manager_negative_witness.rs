//! cfg(test)-only fixed negative witness. No canonical authority or namespace adoption.
//! Root retains the actual host namespace inode while the restricted writer reads
//! a pinned receipt instead of dereferencing privileged /proc/1/ns/net.
use crate::kernel_observer::private_fixture_namespace_identity;
use rustix::fs::{Mode, OFlags, openat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::Write;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};

const STAGE: &str = "/run/omavless-k1-retained-private-lifecycle";
const UNIT: &str = "omavless-k1-retained-private-lifecycle.service";
const NAME: &str = "host-negative-witness.json";
const LIMIT: usize = 2048;
type Result<T> = std::result::Result<T, ()>;

fn require(ok: bool) -> Result<()> {
    if ok { Ok(()) } else { Err(()) }
}
fn identity(file: &File) -> Result<(u64, u64)> {
    private_fixture_namespace_identity(file).map_err(|_| ())
}
fn no_xattrs(file: &File) -> Result<()> {
    let mut names = [0_u8; 1];
    require(rustix::fs::flistxattr(file, &mut names[..]).map_err(|_| ())? == 0)
}
fn meta(m: &Metadata) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.nlink(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn dir_meta(m: &Metadata) -> (u64, u64, u32, u32, u32) {
    (m.dev(), m.ino(), m.mode(), m.uid(), m.gid())
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u32,
    fixture_unit: String,
    stage_device: u64,
    stage_inode: u64,
    namespace_device: u64,
    namespace_inode: u64,
    negative_witness_only: bool,
    canonical_authority: bool,
}
impl Receipt {
    fn decode(raw: &[u8], stage: (u64, u64), namespace: (u64, u64)) -> Result<Self> {
        require(!raw.is_empty() && raw.len() <= LIMIT)?;
        let value: Self = serde_json::from_slice(raw).map_err(|_| ())?;
        require(
            value.schema == 1
                && value.fixture_unit == UNIT
                && (value.stage_device, value.stage_inode) == stage
                && (value.namespace_device, value.namespace_inode) == namespace
                && stage.0 != 0
                && stage.1 != 0
                && namespace.0 != 0
                && namespace.1 != 0
                && value.negative_witness_only
                && !value.canonical_authority,
        )?;
        Ok(value)
    }
}

struct Directory {
    file: File,
    path: PathBuf,
    initial: Metadata,
}
impl Directory {
    fn open(path: &Path, private: bool, owner: (u32, u32)) -> Result<Self> {
        let file: File = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?
        .into();
        let initial = file.metadata().map_err(|_| ())?;
        require(
            initial.is_dir()
                && (initial.uid(), initial.gid()) == owner
                && if private {
                    initial.mode() & 0o7777 == 0o700
                } else {
                    initial.mode() & 0o022 == 0
                },
        )?;
        let value = Self {
            file,
            path: path.into(),
            initial,
        };
        value.recheck()?;
        Ok(value)
    }
    fn recheck(&self) -> Result<()> {
        require(
            dir_meta(&self.initial) == dir_meta(&self.file.metadata().map_err(|_| ())?)
                && dir_meta(&self.initial)
                    == dir_meta(&std::fs::symlink_metadata(&self.path).map_err(|_| ())?),
        )?;
        no_xattrs(&self.file)
    }
}

/// Both processes retain this original receipt and all fixed ancestry FDs.
pub(crate) struct Witness {
    dirs: Vec<Directory>,
    file: File,
    initial: Metadata,
    hash: [u8; 32],
    namespace: (u64, u64),
}
impl Witness {
    fn directories() -> Result<Vec<Directory>> {
        ["/", "/run", STAGE]
            .into_iter()
            .map(|p| Directory::open(Path::new(p), p == STAGE, (0, 0)))
            .collect()
    }
    pub(crate) fn read(namespace: (u64, u64)) -> Result<Self> {
        Self::from_dirs(Self::directories()?, namespace, (0, 0))
    }
    fn from_dirs(dirs: Vec<Directory>, namespace: (u64, u64), owner: (u32, u32)) -> Result<Self> {
        let stage = dirs.last().ok_or(())?;
        let file: File = openat(
            &stage.file,
            NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| ())?
        .into();
        let initial = file.metadata().map_err(|_| ())?;
        require(
            initial.is_file()
                && initial.nlink() == 1
                && (initial.uid(), initial.gid()) == owner
                && initial.mode() & 0o7777 == 0o600
                && initial.len() > 0
                && initial.len() <= LIMIT as u64,
        )?;
        let mut value = Self {
            dirs,
            file,
            initial,
            hash: [0; 32],
            namespace,
        };
        value.hash = Sha256::digest(value.bytes()?).into();
        value.recheck(namespace)?;
        Ok(value)
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        let mut raw = vec![0; self.initial.len() as usize];
        self.file.read_exact_at(&mut raw, 0).map_err(|_| ())?;
        let mut extra = [0];
        require(
            self.file
                .read_at(&mut extra, self.initial.len())
                .map_err(|_| ())?
                == 0,
        )?;
        Ok(raw)
    }
    pub(crate) fn recheck(&self, namespace: (u64, u64)) -> Result<()> {
        require(namespace == self.namespace)?;
        for dir in &self.dirs {
            dir.recheck()?;
        }
        let stage = self.dirs.last().ok_or(())?;
        let check = || -> Result<()> {
            require(
                meta(&self.initial) == meta(&self.file.metadata().map_err(|_| ())?)
                    && meta(&self.initial)
                        == meta(&std::fs::symlink_metadata(stage.path.join(NAME)).map_err(|_| ())?),
            )?;
            no_xattrs(&self.file)
        };
        check()?;
        let raw = self.bytes()?;
        require(<[u8; 32]>::from(Sha256::digest(&raw)) == self.hash)?;
        Receipt::decode(&raw, (stage.initial.dev(), stage.initial.ino()), namespace)?;
        check()?;
        for dir in &self.dirs {
            dir.recheck()?;
        }
        Ok(())
    }
}

/// Retained by the leaked fixed adapter for the entire Ref/Start/Stop/Unref run.
pub(crate) struct RootWitness {
    host: File,
    own: File,
    receipt: Witness,
}
impl RootWitness {
    pub(crate) fn capture() -> Result<Self> {
        let host = File::open("/proc/1/ns/net").map_err(|_| ())?;
        let own = File::open("/proc/thread-self/ns/net").map_err(|_| ())?;
        let namespace = identity(&host)?;
        require(namespace == identity(&own)?)?;
        let dirs = Witness::directories()?;
        let stage = dirs.last().ok_or(())?;
        let value = Receipt {
            schema: 1,
            fixture_unit: UNIT.into(),
            stage_device: stage.initial.dev(),
            stage_inode: stage.initial.ino(),
            namespace_device: namespace.0,
            namespace_inode: namespace.1,
            negative_witness_only: true,
            canonical_authority: false,
        };
        let raw = serde_json::to_vec(&value).map_err(|_| ())?;
        Receipt::decode(&raw, (stage.initial.dev(), stage.initial.ino()), namespace)?;
        // Validate all witnesses before exclusive publication, not afterwards only.
        for dir in &dirs {
            dir.recheck()?;
        }
        require(
            identity(&host)? == identity(&File::open("/proc/1/ns/net").map_err(|_| ())?)?
                && identity(&own)?
                    == identity(&File::open("/proc/thread-self/ns/net").map_err(|_| ())?)?,
        )?;
        let mut file: File = openat(
            &stage.file,
            NAME,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(|_| ())?
        .into();
        file.write_all(&raw).map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
        stage.file.sync_all().map_err(|_| ())?;
        // Compare publication FD to newly retained read FD before releasing writer.
        let receipt = Witness::from_dirs(dirs, namespace, (0, 0))?;
        require(meta(&file.metadata().map_err(|_| ())?) == meta(&receipt.initial))?;
        let value = Self { host, own, receipt };
        value.recheck()?;
        Ok(value)
    }
    pub(crate) fn recheck(&self) -> Result<()> {
        let namespace = identity(&self.host)?;
        require(
            identity(&self.own)? == namespace
                && identity(&File::open("/proc/1/ns/net").map_err(|_| ())?)? == namespace
                && identity(&File::open("/proc/thread-self/ns/net").map_err(|_| ())?)? == namespace,
        )?;
        self.receipt.recheck(namespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn sample(stage: (u64, u64)) -> Vec<u8> {
        serde_json::to_vec(&Receipt {
            schema: 1,
            fixture_unit: UNIT.into(),
            stage_device: stage.0,
            stage_inode: stage.1,
            namespace_device: 5,
            namespace_inode: 9,
            negative_witness_only: true,
            canonical_authority: false,
        })
        .unwrap()
    }
    #[test]
    fn strict_schema_wrong_stale_missing_duplicate_short_and_authority_refuse() {
        let raw = sample((2, 3));
        assert!(Receipt::decode(&raw, (2, 3), (5, 9)).is_ok());
        for end in 0..raw.len() {
            assert!(Receipt::decode(&raw[..end], (2, 3), (5, 9)).is_err());
        }
        let text = String::from_utf8(raw.clone()).unwrap();
        for (from, to) in [
            ("\"schema\":1", "\"schema\":1,\"schema\":1"),
            ("\"schema\":1", "\"schema\":true"),
            ("\"schema\":1,", ""),
            ("\"schema\":1", "\"schema\":2"),
            ("\"schema\":1", "\"schema\":1,\"unknown\":0"),
            (UNIT, "other.service"),
            (
                "\"negative_witness_only\":true",
                "\"negative_witness_only\":false",
            ),
            (
                "\"canonical_authority\":false",
                "\"canonical_authority\":true",
            ),
            ("\"namespace_inode\":9", "\"namespace_inode\":0"),
        ] {
            assert!(Receipt::decode(text.replace(from, to).as_bytes(), (2, 3), (5, 9)).is_err());
        }
        assert!(Receipt::decode(&raw, (2, 4), (5, 9)).is_err());
        assert!(Receipt::decode(&raw, (2, 3), (5, 10)).is_err());
        assert!(Receipt::decode(&sample((0, 3)), (0, 3), (5, 9)).is_err());
        assert!(Receipt::decode(&vec![b' '; LIMIT + 1], (2, 3), (5, 9)).is_err());
    }
    #[test]
    fn original_receipt_and_stage_fds_fail_before_simulated_socket() {
        let owner = (
            nix::unistd::getuid().as_raw(),
            nix::unistd::getgid().as_raw(),
        );
        for variant in [
            "valid",
            "missing",
            "short",
            "swapped",
            "changed",
            "mode",
            "symlink",
            "hardlink",
            "stage",
            "wrong-anchor",
            "xattr",
        ] {
            let path = crate::test_temp::directory("negative-witness").unwrap();
            let directory = Directory::open(&path, true, owner).unwrap();
            let raw = sample((directory.initial.dev(), directory.initial.ino()));
            let file_path = path.join(NAME);
            if variant != "missing" {
                std::fs::write(
                    &file_path,
                    if variant == "short" {
                        &raw[..raw.len() - 1]
                    } else {
                        &raw
                    },
                )
                .unwrap();
                std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o600))
                    .unwrap();
            }
            let mut sockets = 0;
            let result = (|| -> Result<()> {
                let witness = Witness::from_dirs(vec![directory], (5, 9), owner)?;
                match variant {
                    "swapped" | "symlink" => {
                        std::fs::rename(&file_path, path.join("old")).unwrap();
                        if variant == "symlink" {
                            symlink(path.join("old"), &file_path).unwrap();
                        } else {
                            std::fs::write(&file_path, &raw).unwrap();
                            std::fs::set_permissions(
                                &file_path,
                                std::fs::Permissions::from_mode(0o600),
                            )
                            .unwrap();
                        }
                    }
                    "changed" => {
                        std::fs::write(&file_path, b"{}").unwrap();
                    }
                    "mode" => {
                        std::fs::set_permissions(
                            &file_path,
                            std::fs::Permissions::from_mode(0o400),
                        )
                        .unwrap();
                    }
                    "hardlink" => {
                        std::fs::hard_link(&file_path, path.join("alias")).unwrap();
                    }
                    "stage" => {
                        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o750))
                            .unwrap();
                    }
                    "xattr" => {
                        rustix::fs::fsetxattr(
                            &witness.file,
                            "user.k1-test",
                            b"x",
                            rustix::fs::XattrFlags::empty(),
                        )
                        .unwrap();
                    }
                    _ => {}
                }
                witness.recheck(if variant == "wrong-anchor" {
                    (5, 10)
                } else {
                    (5, 9)
                })?;
                sockets += 1; // No real socket, namespace or privileged action.
                Ok(())
            })();
            assert_eq!(result.is_ok(), variant == "valid", "{variant}");
            assert_eq!(sockets, usize::from(variant == "valid"), "{variant}");
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    #[test]
    fn ordinary_file_cannot_be_namespace_witness() {
        let path = crate::test_temp::directory("not-netns").unwrap();
        std::fs::write(path.join("file"), b"net:[9]").unwrap();
        assert!(identity(&File::open(path.join("file")).unwrap()).is_err());
        std::fs::remove_dir_all(path).unwrap();
    }
}
