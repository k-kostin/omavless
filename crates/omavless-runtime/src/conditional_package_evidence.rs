// SPDX-License-Identifier: MIT
//! Provisional research evidence, NOT an adopted package or effect permit.
//! Root statements of build provenance are checked, not independently proved.
//! No production caller, installation, repair, execution or controller write.
#![allow(dead_code)]

use super::{ExecutableEvidence, FileIdentity, Session};
use nix::fcntl::{OFlag, open, openat};
use nix::sys::stat::Mode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata};
use std::io::{Read, Seek};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const FIXTURE_DIRECTORY: &str = "/var/lib/omavless-close-research-fixture";
const CORE: &str = "mihomo";
const RECEIPT: &str = "source-receipt.json";
const MAX_CORE: u64 = 128 * 1024 * 1024;
const MAX_RECEIPT: u64 = 8192;
const BUDGET: Duration = Duration::from_secs(3);
const SOURCE: &str = "8d6877c49d400aa723e01032c62a95d6559fecc3";
const DNS_SOURCE: &str = "c4e800425243c1b02165f82153e4bf418fe465e6";
const MIHOMO: &str = "ab405bad5beeeac8b003bb01f60f134f6df54471";
const SING_TUN: &str = "b50ae28a1409c7bce8e96e6c6966cf57d8ace754";
const CLOSE_PATCH: &str = "0858827e1af00c3ed3196f021b0dbc76ce34a8de7aa7130d7085614d149acc8f";
const DNS_PATCH: &str = "d5ebe9d6b37f6b76599fc3c2dd25adbfb774ca0121beeb79c5768a9a08d7ff37";
const TUN_PATCH: &str = "2556c82aafbeb598a817d43042cf2069c6f209433c7a506395df581b4e31e2ab";

// A distinct opt-in developer evidence class. The old source-only reader above
// is not upgraded into a permit and its receipt schema is unchanged.
#[cfg(feature = "developer-conditional-close")]
#[path = "conditional_developer_pair.rs"]
pub(super) mod developer_pair;
#[cfg(feature = "developer-conditional-close")]
#[path = "conditional_release_pair.rs"]
pub(super) mod release_pair;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Object,
    Receipt,
    Architecture,
    Child,
    Expired,
}
type Result<T> = std::result::Result<T, Refusal>;

// Struct decoding rejects duplicate fields, including escaped spellings. No
// Value/map collapse, extension bag, path, shell command or caller grant.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    architecture: String,
    abi: u32,
    pair: String,
    source: String,
    dns_source: String,
    mihomo_commit: String,
    sing_tun_commit: String,
    conditional_patch_sha256: String,
    dns_patch_sha256: String,
    tun_patch_sha256: String,
    build: Build,
    core_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Build {
    go_version: String,
    tags: String,
    cgo: bool,
    buildvcs: bool,
    dependency_mode: String,
    goos: String,
    goarch: String,
}

fn decode_receipt(bytes: &[u8], architecture: &str) -> Result<[u8; 32]> {
    if bytes.is_empty() || bytes.len() > MAX_RECEIPT as usize {
        return Err(Refusal::Receipt);
    }
    let r: Receipt = serde_json::from_slice(bytes).map_err(|_| Refusal::Receipt)?;
    let goarch = match architecture {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(Refusal::Architecture),
    };
    if r.schema != "omavless-close-research-only-v1"
        || r.architecture != architecture
        || r.abi != 1
        || r.pair != "managed-dns-source-composition-only"
        || r.source != SOURCE
        || r.dns_source != DNS_SOURCE
        || r.mihomo_commit != MIHOMO
        || r.sing_tun_commit != SING_TUN
        || r.conditional_patch_sha256 != CLOSE_PATCH
        || r.dns_patch_sha256 != DNS_PATCH
        || r.tun_patch_sha256 != TUN_PATCH
        || r.build.go_version != "go1.27.0-X:nodwarf5"
        || r.build.tags != "with_gvisor"
        || r.build.cgo
        || r.build.buildvcs
        || r.build.dependency_mode != "vendor"
        || r.build.goos != "linux"
        || r.build.goarch != goarch
    {
        return Err(Refusal::Receipt);
    }
    let raw = r.core_sha256.as_bytes();
    if raw.len() != 64
        || !raw
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return Err(Refusal::Receipt);
    }
    let mut hash = [0; 32];
    for (out, pair) in hash.iter_mut().zip(raw.as_chunks::<2>().0) {
        let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        *out = digit(pair[0]) * 16 + digit(pair[1]);
    }
    if hash == [0; 32] {
        return Err(Refusal::Receipt);
    }
    Ok(hash)
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
}
impl DirectoryIdentity {
    fn capture(m: &Metadata) -> Result<Self> {
        if !m.is_dir() || m.uid() != 0 || m.mode() & 0o7022 != 0 {
            return Err(Refusal::Object);
        }
        Ok(Self {
            dev: m.dev(),
            ino: m.ino(),
            mode: m.mode(),
            uid: m.uid(),
            gid: m.gid(),
        })
    }
}
struct Directory {
    file: File,
    identity: DirectoryIdentity,
    name: PathBuf,
}
struct Objects {
    directories: Vec<Directory>,
    receipt: File,
    receipt_identity: FileIdentity,
    core: File,
    core_identity: FileIdentity,
}
fn directory(file: File, name: PathBuf) -> Result<Directory> {
    let identity = DirectoryIdentity::capture(&file.metadata().map_err(|_| Refusal::Object)?)?;
    Ok(Directory {
        file,
        identity,
        name,
    })
}
fn open_directory(parent: &File, name: &Path) -> Result<File> {
    openat(
        parent,
        name,
        OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| Refusal::Object)
}
fn member(parent: &File, name: &str, mode: u32, limit: u64) -> Result<(File, FileIdentity)> {
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
    if m.uid() != 0 || m.mode() & 0o7777 != mode || m.len() > limit {
        return Err(Refusal::Object);
    }
    let identity = FileIdentity::capture(&m).ok_or(Refusal::Object)?;
    Ok((file, identity))
}
impl Objects {
    fn open() -> Result<Self> {
        Self::open_at(Path::new(FIXTURE_DIRECTORY))
    }
    // Private test routing changes only location, never accepted owner/modes.
    fn open_at(path: &Path) -> Result<Self> {
        let mut parts = path.components();
        if parts.next() != Some(Component::RootDir) {
            return Err(Refusal::Object);
        }
        let root = File::from(
            open(
                Path::new("/"),
                OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Refusal::Object)?,
        );
        let mut directories = vec![directory(root, PathBuf::from("/"))?];
        for part in parts {
            let Component::Normal(name) = part else {
                return Err(Refusal::Object);
            };
            if directories.len() >= 16 {
                return Err(Refusal::Object);
            }
            let file = open_directory(
                &directories.last().ok_or(Refusal::Object)?.file,
                Path::new(name),
            )?;
            directories.push(directory(file, PathBuf::from(name))?);
        }
        let parent = &directories.last().ok_or(Refusal::Object)?.file;
        let (receipt, receipt_identity) = member(parent, RECEIPT, 0o644, MAX_RECEIPT)?;
        let (core, core_identity) = member(parent, CORE, 0o755, MAX_CORE)?;
        let objects = Self {
            directories,
            receipt,
            receipt_identity,
            core,
            core_identity,
        };
        objects.recheck()?;
        Ok(objects)
    }
    fn recheck(&self) -> Result<()> {
        for (index, held) in self.directories.iter().enumerate() {
            let current = if index == 0 {
                File::from(
                    open(
                        Path::new("/"),
                        OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| Refusal::Object)?,
                )
            } else {
                open_directory(&self.directories[index - 1].file, &held.name)?
            };
            if DirectoryIdentity::capture(&held.file.metadata().map_err(|_| Refusal::Object)?)?
                != held.identity
                || DirectoryIdentity::capture(&current.metadata().map_err(|_| Refusal::Object)?)?
                    != held.identity
            {
                return Err(Refusal::Object);
            }
        }
        let parent = &self.directories.last().ok_or(Refusal::Object)?.file;
        let (_, receipt) = member(parent, RECEIPT, 0o644, MAX_RECEIPT)?;
        let (_, core) = member(parent, CORE, 0o755, MAX_CORE)?;
        if receipt != self.receipt_identity
            || core != self.core_identity
            || !receipt.matches(&self.receipt)
            || !core.matches(&self.core)
        {
            return Err(Refusal::Object);
        }
        Ok(())
    }
}

fn read_receipt(file: &mut File, identity: FileIdentity) -> Result<Vec<u8>> {
    if !identity.matches(file) {
        return Err(Refusal::Object);
    }
    file.rewind().map_err(|_| Refusal::Object)?;
    let mut bytes = Vec::new();
    (&mut *file)
        .take(MAX_RECEIPT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::Object)?;
    if bytes.len() as u64 != identity.size || !identity.matches(file) {
        return Err(Refusal::Object);
    }
    Ok(bytes)
}

fn elf_architecture(file: &mut File, architecture: &str) -> Result<()> {
    file.rewind().map_err(|_| Refusal::Object)?;
    let mut header = [0; 64];
    file.read_exact(&mut header)
        .map_err(|_| Refusal::Architecture)?;
    let machine = match architecture {
        "x86_64" => 62,
        "aarch64" => 183,
        _ => return Err(Refusal::Architecture),
    };
    if &header[..7] != b"\x7fELF\x02\x01\x01"
        || !matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        || u16::from_le_bytes([header[18], header[19]]) != machine
        || header[20..24] != [1, 0, 0, 0]
        || header[52..54] != [64, 0]
    {
        return Err(Refusal::Architecture);
    }
    Ok(())
}

/// No Clone/Debug/serialization and no conversion to any permit. Session Arc
/// identity and the retained waitable child are required; PID is not an API.
pub(super) struct PackageObjects {
    objects: Objects,
    session: Arc<()>,
    receipt_hash: [u8; 32],
    core_hash: [u8; 32],
    refused: bool,
}
impl PackageObjects {
    pub(super) fn capture(session: &mut Session) -> Result<Self> {
        session.check().map_err(|_| Refusal::Child)?;
        let deadline = Instant::now() + BUDGET;
        let mut objects = Objects::open()?;
        let bytes = read_receipt(&mut objects.receipt, objects.receipt_identity)?;
        let core_hash = decode_receipt(&bytes, std::env::consts::ARCH)?;
        elf_architecture(&mut objects.core, std::env::consts::ARCH)?;
        if ExecutableEvidence::hash(&mut objects.core, objects.core_identity, deadline)
            != Some(core_hash)
        {
            return Err(Refusal::Object);
        }
        let mut result = Self {
            objects,
            session: Arc::clone(&session.identity),
            receipt_hash: Sha256::digest(&bytes).into(),
            core_hash,
            refused: false,
        };
        result.check(session)?;
        if Instant::now() >= deadline {
            return Err(Refusal::Expired);
        }
        Ok(result)
    }
    pub(super) fn check(&mut self, session: &mut Session) -> Result<()> {
        let result = self.check_inner(session);
        if result.is_err() {
            self.refused = true;
        }
        result
    }
    fn check_inner(&mut self, session: &mut Session) -> Result<()> {
        if self.refused || !Arc::ptr_eq(&self.session, &session.identity) {
            return Err(Refusal::Child);
        }
        session.check().map_err(|_| Refusal::Child)?;
        self.objects.recheck()?;
        let evidence = session.executable.as_ref().ok_or(Refusal::Child)?;
        if evidence.image_identity != self.objects.core_identity
            || evidence.source_identity != self.objects.core_identity
            || evidence.source_path != Path::new(FIXTURE_DIRECTORY).join(CORE)
            || !self.objects.core_identity.matches(&evidence.image)
            || !self.objects.core_identity.matches(&evidence.source)
            || !evidence.check(session.binding.pid)
        {
            return Err(Refusal::Child);
        }
        let bytes = read_receipt(&mut self.objects.receipt, self.objects.receipt_identity)?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.receipt_hash
            || decode_receipt(&bytes, std::env::consts::ARCH)? != self.core_hash
        {
            return Err(Refusal::Receipt);
        }
        self.objects.recheck()?;
        session.check().map_err(|_| Refusal::Child)
    }
}

#[cfg(test)]
#[path = "conditional_package_evidence_tests.rs"]
mod tests;
