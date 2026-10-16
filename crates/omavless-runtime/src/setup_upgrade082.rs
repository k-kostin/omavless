// SPDX-License-Identifier: MIT
//! Retained stopped-owner context for SINGLE-PROCESS predecessor UPDATE.
//! First-migration and optional VM consumers are explicitly feature selected.
//! DNS is a NEW operation. No serialized ticket, service/core start or persistent
//! all-user exclusion is introduced; frontend delivery authentication is separate.
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::cutover_transaction::BridgeTarget;
use crate::desired::{DesiredState, MAX_DESIRED_STATE_BYTES};
use crate::{OwnerLock, RuntimePaths};
use nix::fcntl::{Flock, FlockArg};
use nix::unistd::Uid;
use omavless_domain::private_store::{
    MAX_PRIVATE_STORE_BYTES, validate_existing_store_without_repair,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocker {
    Environment,
    Predecessor,
    RuntimeActive,
    ExclusionUnavailable,
    Busy,
    Ownership,
    DesiredUnavailable,
    DesiredConnected,
    StartupRequiresOff,
    PrivateState,
    TemplateRequiresReview,
    PendingRecovery,
    StateChanged,
    HostAdmissionUnavailable,
    StagingUnavailable,
    CancelledBeforeSubmission,
    PackageOutcomeUnverified,
    PackageSubmissionUnavailable,
    UnsupportedDevelopmentGuest,
    DeliveryMetadataInvalid,
    HelperIdentityUnverified,
    TtyUnavailable,
    ForeignOwnerObserved,
    BrokerPreparationRequired,
    MaintenanceOutcomeUnknown,
}
impl std::fmt::Display for Blocker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::Environment => "environment_unverified",
            Self::Predecessor => "predecessor_unverified",
            Self::RuntimeActive => "runtime_not_stopped",
            Self::ExclusionUnavailable => "exclusion_unavailable",
            Self::Busy => "operation_busy",
            Self::Ownership => "ownership_unverified",
            Self::DesiredUnavailable => "desired_unavailable",
            Self::DesiredConnected => "desired_connected",
            Self::StartupRequiresOff => "startup_requires_off",
            Self::PrivateState => "private_state_incompatible",
            Self::TemplateRequiresReview => "template_requires_review",
            Self::PendingRecovery => "pending_recovery",
            Self::StateChanged => "state_changed",
            Self::HostAdmissionUnavailable => "host_admission_unavailable",
            Self::StagingUnavailable => "staging_unavailable",
            Self::CancelledBeforeSubmission => "cancelled_before_submission",
            Self::PackageOutcomeUnverified => "package_outcome_unverified",
            Self::PackageSubmissionUnavailable => "package_submission_unavailable",
            Self::UnsupportedDevelopmentGuest => "unsupported_development_guest",
            Self::DeliveryMetadataInvalid => "delivery_metadata_invalid",
            Self::HelperIdentityUnverified => "helper_identity_unverified",
            Self::TtyUnavailable => "tty_unavailable",
            Self::ForeignOwnerObserved => "foreign_owner_observed",
            Self::BrokerPreparationRequired => "dns_clean_stop_required",
            Self::MaintenanceOutcomeUnknown => "dns_preparation_incomplete_inspect_before_retry",
        };
        write!(f, "OmaVLESS upgrade082: {reason}")
    }
}
type Result<T> = std::result::Result<T, Blocker>;

fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

// Captures are private and keep their original no-follow descriptors alive.
// No Debug/Clone/Serialize or externally constructible capture is available.
struct Captured {
    path: PathBuf,
    file: File,
    identity: fs::Metadata,
    bytes: Vec<u8>,
}
impl Captured {
    fn read(path: &Path, uid: u32, mode: u32, maximum: usize) -> Result<Self> {
        let before = fs::symlink_metadata(path).map_err(|_| Blocker::PrivateState)?;
        if !before.is_file()
            || before.uid() != uid
            || before.mode() & 0o7777 != mode
            || before.nlink() != 1
            || before.len() == 0
            || before.len() > maximum as u64
        {
            return Err(Blocker::PrivateState);
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Blocker::PrivateState)?;
        let identity = file.metadata().map_err(|_| Blocker::PrivateState)?;
        if !same(&before, &identity) {
            return Err(Blocker::StateChanged);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Blocker::PrivateState)?;
        if bytes.len() > maximum {
            return Err(Blocker::PrivateState);
        }
        let mut captured = Self {
            path: path.to_owned(),
            file,
            identity,
            bytes,
        };
        captured.recheck()?;
        Ok(captured)
    }
    fn recheck(&mut self) -> Result<()> {
        let named = fs::symlink_metadata(&self.path).map_err(|_| Blocker::StateChanged)?;
        let held = self.file.metadata().map_err(|_| Blocker::StateChanged)?;
        if !same(&self.identity, &named) || !same(&self.identity, &held) {
            return Err(Blocker::StateChanged);
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| Blocker::StateChanged)?;
        let mut current = Vec::new();
        (&mut self.file)
            .take(self.bytes.len() as u64 + 1)
            .read_to_end(&mut current)
            .map_err(|_| Blocker::StateChanged)?;
        if current != self.bytes
            || !same(
                &self.identity,
                &self.file.metadata().map_err(|_| Blocker::StateChanged)?,
            )
        {
            return Err(Blocker::StateChanged);
        }
        Ok(())
    }
}
struct Directory {
    path: PathBuf,
    file: File,
    identity: fs::Metadata,
}
fn same_directory(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.is_dir()
        && b.is_dir()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.mode() == b.mode()
}
impl Directory {
    fn pin(path: &Path, uid: u32) -> Result<Self> {
        if !path.is_absolute() || fs::canonicalize(path).ok().as_deref() != Some(path) {
            return Err(Blocker::Environment);
        }
        let identity = fs::symlink_metadata(path).map_err(|_| Blocker::Environment)?;
        if !identity.is_dir() || identity.uid() != uid || identity.mode() & 0o7777 != 0o700 {
            return Err(Blocker::Environment);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| Blocker::Environment)?;
        if !same_directory(
            &identity,
            &file.metadata().map_err(|_| Blocker::Environment)?,
        ) {
            return Err(Blocker::StateChanged);
        }
        Ok(Self {
            path: path.to_owned(),
            file,
            identity,
        })
    }
    fn recheck(&self) -> Result<()> {
        if !same_directory(
            &self.identity,
            &fs::symlink_metadata(&self.path).map_err(|_| Blocker::StateChanged)?,
        ) || !same_directory(
            &self.identity,
            &self.file.metadata().map_err(|_| Blocker::StateChanged)?,
        ) {
            return Err(Blocker::StateChanged);
        }
        Ok(())
    }
}

struct Scope {
    runtime: RuntimePaths,
    cutover: CutoverPaths,
    config: PathBuf,
}
impl Scope {
    fn current(uid: u32) -> Result<Self> {
        if uid == 0
            || Uid::effective().as_raw() != uid
            || std::env::var_os("OMAVLESS_HOME").is_some()
        {
            return Err(Blocker::Environment);
        }
        let home = PathBuf::from(std::env::var_os("HOME").ok_or(Blocker::Environment)?);
        let runtime = PathBuf::from(format!("/run/user/{uid}"));
        let state = home.join(".local/state");
        let config_base = home.join(".config");
        let cache_base = home.join(".cache");
        for (name, expected) in [
            ("XDG_RUNTIME_DIR", &runtime),
            ("XDG_STATE_HOME", &state),
            ("XDG_CONFIG_HOME", &config_base),
            ("XDG_CACHE_HOME", &cache_base),
        ] {
            if std::env::var_os(name).is_some_and(|value| Path::new(&value) != expected.as_path()) {
                return Err(Blocker::Environment);
            }
        }
        if !home.is_absolute()
            || fs::canonicalize(&home).ok().as_ref() != Some(&home)
            || fs::symlink_metadata(&home)
                .map_err(|_| Blocker::Environment)?
                .uid()
                != uid
        {
            return Err(Blocker::Environment);
        }
        Ok(Self {
            runtime: RuntimePaths::below(&runtime),
            cutover: CutoverPaths::below(&runtime, &state, uid),
            config: home.join(".config/omavless"),
        })
    }
}

fn absent(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

fn predecessor_at(
    receipt: &Path,
    binary: &Path,
    owner: u32,
    arch: &str,
    digest: &str,
) -> Result<()> {
    let mut receipt =
        Captured::read(receipt, owner, 0o644, 4096).map_err(|_| Blocker::Predecessor)?;
    let expected = format!(
        "schemaVersion=3\nsourceCommit=22e23e64c49b8110088b6be3f063b5e641ba0853\nbinarySha256={digest}\narchitecture={arch}\nprovenance=caller-supplied-prebuilt\nproductVersion=0.8.2\n"
    );
    if receipt.bytes != expected.as_bytes() {
        return Err(Blocker::Predecessor);
    }
    let before = fs::symlink_metadata(binary).map_err(|_| Blocker::Predecessor)?;
    if !before.is_file()
        || before.uid() != owner
        || before.mode() & 0o7777 != 0o755
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > 256 * 1024 * 1024
    {
        return Err(Blocker::Predecessor);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(binary)
        .map_err(|_| Blocker::Predecessor)?;
    if !same(&before, &file.metadata().map_err(|_| Blocker::Predecessor)?) {
        return Err(Blocker::Predecessor);
    }
    let mut sha = Sha256::new();
    let mut total = 0;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|_| Blocker::Predecessor)?;
        if count == 0 {
            break;
        }
        total += count;
        if total > 256 * 1024 * 1024 {
            return Err(Blocker::Predecessor);
        }
        sha.update(&buffer[..count]);
    }
    if format!("{:x}", sha.finalize()) != digest
        || !same(&before, &file.metadata().map_err(|_| Blocker::Predecessor)?)
        || !same(
            &before,
            &fs::symlink_metadata(binary).map_err(|_| Blocker::Predecessor)?,
        )
    {
        return Err(Blocker::Predecessor);
    }
    receipt.recheck().map_err(|_| Blocker::Predecessor)
}

fn predecessor() -> Result<()> {
    let arch = std::env::consts::ARCH;
    let digest = match arch {
        "x86_64" => "3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1",
        "aarch64" => "62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f",
        _ => return Err(Blocker::Predecessor),
    };
    // Public recorded hashes: v0.8.2 NATIVE_082_ARTIFACTS_2026-09-21.md.
    // Byte provenance only, not pacman DB/all-user replacement admission.
    predecessor_at(
        Path::new("/usr/share/doc/omavless/build-identity.txt"),
        Path::new("/usr/bin/omavless"),
        0,
        arch,
        digest,
    )
}

// Published 0.9.8 producers only. Package-version equality cannot distinguish
// maintenance deliveries; modified/developer receipts do not earn admission.
const PUBLIC_098_SOURCES: [&str; 6] = [
    "e3d2245966c725ee11b505899b22a20721ff2ddd",
    "8d02f9d7dd1a9063a82e9dcd93cb19cf4c5d8524",
    "b65695ee0609c2a971289109975678b8da87c946",
    "4915ae4a8f4ec6cfb469ad6b38126bd70d5a1998",
    "99c556e5c966e1e1dae04f2d8babd2ca2dec9375",
    "d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b",
];

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishedPair {
    source_commit: String,
    architecture: String,
    app_sha: String,
    core_sha: String,
    broker_sha: String,
    dns_receipt_sha: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishedPairs {
    schema_version: u32,
    pairs: Vec<PublishedPair>,
}
fn published098(source: &str, arch: &str) -> Result<PublishedPair> {
    // Compiled public bytes, never a caller-selected receipt/manifest.
    let table: PublishedPairs =
        serde_json::from_str(include_str!("../../../plugin/published-pairs.json"))
            .map_err(|_| Blocker::Predecessor)?;
    if table.schema_version != 1 {
        return Err(Blocker::Predecessor);
    }
    let mut matches = table
        .pairs
        .into_iter()
        .filter(|pair| pair.source_commit == source && pair.architecture == arch);
    let pair = matches.next().ok_or(Blocker::Predecessor)?;
    if matches.next().is_some() {
        return Err(Blocker::Predecessor);
    }
    Ok(pair)
}

fn predecessor098_at(
    receipt: &Path,
    binary: &Path,
    owner: u32,
    arch: &str,
    target: Option<&str>,
) -> Result<()> {
    let mut captured =
        Captured::read(receipt, owner, 0o644, 4096).map_err(|_| Blocker::Predecessor)?;
    let text = std::str::from_utf8(&captured.bytes).map_err(|_| Blocker::Predecessor)?;
    let source = text
        .lines()
        .find_map(|line| line.strip_prefix("sourceCommit="))
        .filter(|source| PUBLIC_098_SOURCES.contains(source) || Some(*source) == target)
        .ok_or(Blocker::Predecessor)?;
    let digest = package_phase::app_identity(&captured.bytes, arch, source, "0.9.8")
        .map_err(|_| Blocker::Predecessor)?;
    let expected = if Some(source) == target {
        package_phase::running_digest()?
    } else {
        published098(source, arch)?.app_sha
    };
    if expected != digest {
        return Err(Blocker::Predecessor);
    }
    package_phase::Archive::open(binary.to_owned(), owner, &digest, 0o755)
        .map_err(|_| Blocker::Predecessor)?;
    captured.recheck().map_err(|_| Blocker::Predecessor)
}

fn predecessor098(target: &str) -> Result<()> {
    predecessor098_at(
        Path::new("/usr/share/doc/omavless/build-identity.txt"),
        Path::new("/usr/bin/omavless"),
        0,
        std::env::consts::ARCH,
        Some(target),
    )
}

fn units() -> Result<[String; 2]> {
    let mut observations = Vec::new();
    for name in ["omavless-runtime.service", "omavless-login-prepare.service"] {
        let mut command = Command::new("/usr/bin/systemctl");
        command.env_clear().env("PATH", "/usr/bin").env("LC_ALL", "C")
            .env("XDG_RUNTIME_DIR", format!("/run/user/{}", Uid::current().as_raw()))
            .args(["--user", "show", name, "--no-pager",
                "--property=LoadState,ActiveState,SubState,MainPID,Job,FragmentPath,DropInPaths,UnitFileState,NeedDaemonReload,ExecStart"]);
        let raw =
            crate::production_observation::bounded_fixed_query(command, Duration::from_secs(2))
                .map_err(|_| Blocker::RuntimeActive)?;
        validate_unit(&raw, name)?;
        observations.push(raw);
    }
    observations.try_into().map_err(|_| Blocker::RuntimeActive)
}
fn validate_unit(raw: &str, name: &str) -> Result<()> {
    let mut values = std::collections::BTreeMap::new();
    for line in raw.lines() {
        let (key, value) = line.split_once('=').ok_or(Blocker::RuntimeActive)?;
        if values.insert(key, value).is_some() {
            return Err(Blocker::RuntimeActive);
        }
    }
    let runtime = name == "omavless-runtime.service";
    let settled = (values.get("ActiveState") == Some(&"inactive")
        && values.get("SubState") == Some(&"dead"))
        || (!runtime
            && values.get("ActiveState") == Some(&"active")
            && values.get("SubState") == Some(&"exited"));
    if values.len() != 10
        || !settled
        || values.get("LoadState") != Some(&"loaded")
        || values.get("MainPID") != Some(&"0")
        || values.get("Job") != Some(&"")
        || values.get("DropInPaths") != Some(&"")
        || values.get("FragmentPath").copied()
            != Some(format!("/usr/lib/systemd/user/{name}").as_str())
        || values.get("UnitFileState") != Some(&if runtime { "disabled" } else { "static" })
        || values.get("NeedDaemonReload") != Some(&"no")
        || !values
            .get("ExecStart")
            .is_some_and(|raw| exec_matches(raw, runtime))
    {
        return Err(Blocker::RuntimeActive);
    }
    Ok(())
}

fn exec_matches(raw: &str, runtime: bool) -> bool {
    let Some(inner) = raw
        .trim()
        .strip_prefix('{')
        .and_then(|raw| raw.strip_suffix('}'))
    else {
        return false;
    };
    let mut fields = std::collections::BTreeMap::new();
    for item in inner.split(';') {
        let Some((key, value)) = item.trim().split_once('=') else {
            return false;
        };
        if fields.insert(key, value).is_some() {
            return false;
        }
    }
    fields.get("path") == Some(&"/usr/bin/omavless")
        && fields.get("argv[]")
            == Some(&if runtime {
                "/usr/bin/omavless daemon"
            } else {
                "/usr/bin/omavless login-prepare"
            })
        && fields.get("ignore_errors") == Some(&"no")
}

// A future reviewed effect consumer must own this value for its ENTIRE lifetime.
// It cannot acquire authority from a preceding CLI's exit status or JSON.
pub(crate) struct UpgradeContext {
    scope: Scope,
    uid: u32,
    owner: OwnerLock,
    owner_identity: fs::Metadata,
    migration: MigrationLock,
    directories: Vec<Directory>,
    captured: Vec<Captured>,
    units: [String; 2],
    login_receipt_absent: bool,
    absent_members: Vec<PathBuf>,
}
impl UpgradeContext {
    fn acquire(
        scope: Scope,
        uid: u32,
        observed_units: [String; 2],
        managed_pair_update: bool,
    ) -> Result<Self> {
        // /run is boot-local. These descriptors exclude this original update,
        // not prove that a historical daemon once created a lock pathname.
        // Persistent ownership/configuration is never initialized or repaired.
        let mut directories = [
            &scope.cutover.runtime_base,
            &scope.cutover.state_directory,
            &scope.config,
        ]
        .into_iter()
        .map(|path| Directory::pin(path, uid))
        .collect::<Result<Vec<_>>>()?;
        precheck_stopped_owner(&scope, uid)?;
        if scope.runtime.directory != scope.cutover.runtime_base.join("omavless")
            || scope.runtime.owner_lock != scope.runtime.directory.join(crate::OWNER_LOCK_NAME)
        {
            return Err(Blocker::Environment);
        }
        if absent(&scope.runtime.directory) {
            nix::sys::stat::mkdirat(
                &directories[0].file,
                "omavless",
                nix::sys::stat::Mode::from_bits_truncate(0o700),
            )
            .map_err(|_| Blocker::StateChanged)?;
        }
        directories[0].recheck()?;
        directories.push(Directory::pin(&scope.runtime.directory, uid)?);
        if !absent(&scope.runtime.socket) || !absent(&scope.runtime.directory.join("mihomo.sock")) {
            return Err(Blocker::RuntimeActive);
        }
        let path = &scope.runtime.owner_lock;
        let prior = match fs::symlink_metadata(path) {
            Ok(identity) => Some(identity),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(Blocker::ExclusionUnavailable),
        };
        if prior.as_ref().is_some_and(|identity| {
            !identity.is_file()
                || identity.uid() != uid
                || identity.mode() & 0o7777 != 0o600
                || identity.nlink() != 1
        }) {
            return Err(Blocker::ExclusionUnavailable);
        }
        let mut flags = nix::fcntl::OFlag::O_RDWR
            | nix::fcntl::OFlag::O_NOFOLLOW
            | nix::fcntl::OFlag::O_NONBLOCK
            | nix::fcntl::OFlag::O_CLOEXEC;
        if prior.is_none() {
            flags |= nix::fcntl::OFlag::O_CREAT | nix::fcntl::OFlag::O_EXCL;
        }
        let file = File::from(
            nix::fcntl::openat(
                &directories[3].file,
                crate::OWNER_LOCK_NAME,
                flags,
                nix::sys::stat::Mode::from_bits_truncate(0o600),
            )
            .map_err(|_| Blocker::ExclusionUnavailable)?,
        );
        let identity = file.metadata().map_err(|_| Blocker::ExclusionUnavailable)?;
        if !identity.is_file()
            || identity.uid() != uid
            || identity.mode() & 0o7777 != 0o600
            || identity.nlink() != 1
            || prior
                .as_ref()
                .is_some_and(|before| !same(before, &identity))
            || !same(
                &identity,
                &fs::symlink_metadata(path).map_err(|_| Blocker::StateChanged)?,
            )
        {
            return Err(Blocker::ExclusionUnavailable);
        }
        let held =
            Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_, _)| Blocker::Busy)?;
        let owner = OwnerLock { _file: held };
        // Never use the legacy permission-repairing normal acquisition for
        // an existing inode. Exclusive fresh creation cannot adopt a winner.
        let migration = if absent(&scope.cutover.operation_lock) {
            MigrationLock::acquire_absent(&scope.cutover, uid)
        } else {
            MigrationLock::acquire_existing(&scope.cutover, uid)
        }
        .map_err(|error| {
            if error == crate::cutover::CutoverError::Busy {
                Blocker::Busy
            } else {
                Blocker::ExclusionUnavailable
            }
        })?;
        let mut captured = Vec::new();
        for (path, maximum, blocker) in [
            (
                scope.cutover.ownership_marker.clone(),
                1024,
                Blocker::Ownership,
            ),
            (
                scope
                    .cutover
                    .state_directory
                    .join(crate::frontend_bridge::FRONTEND_BRIDGE_TARGET_NAME),
                64,
                Blocker::Ownership,
            ),
            (
                scope.cutover.state_directory.join("desired.json"),
                MAX_DESIRED_STATE_BYTES as usize,
                Blocker::DesiredUnavailable,
            ),
            (
                scope.config.join("profiles.json"),
                MAX_PRIVATE_STORE_BYTES,
                Blocker::PrivateState,
            ),
            (
                scope.config.join("route-template.yaml"),
                1024 * 1024,
                Blocker::TemplateRequiresReview,
            ),
        ] {
            captured.push(Captured::read(&path, uid, 0o600, maximum).map_err(|_| blocker)?);
        }
        let marker = read_marker_existing(&scope.cutover, uid).map_err(|_| Blocker::Ownership)?;
        if marker.phase() != OwnershipPhase::Rust
            || crate::frontend_bridge::read_committed_plugin_target(&scope.cutover, uid)
                .map_err(|_| Blocker::Ownership)?
                != BridgeTarget::Rust
        {
            return Err(Blocker::Ownership);
        }
        let desired: DesiredState =
            serde_json::from_slice(&captured[2].bytes).map_err(|_| Blocker::DesiredUnavailable)?;
        desired
            .validate()
            .map_err(|_| Blocker::DesiredUnavailable)?;
        if desired.connected {
            return Err(Blocker::DesiredConnected);
        }
        let raw_store: serde_json::Value =
            serde_json::from_slice(&captured[3].bytes).map_err(|_| Blocker::PrivateState)?;
        if raw_store["startupConfigured"] != true || raw_store["startup"]["enabled"] != false {
            return Err(Blocker::StartupRequiresOff);
        }
        let store = validate_existing_store_without_repair(
            std::str::from_utf8(&captured[3].bytes).map_err(|_| Blocker::PrivateState)?,
        )
        .map_err(|_| Blocker::PrivateState)?;
        if !store.startup_is_configured() || store.startup_preferences().enabled {
            return Err(Blocker::StartupRequiresOff);
        }
        crate::login_transaction::check_startup_receipt(
            &scope.cutover,
            uid,
            &migration,
            Some(marker.generation()),
        )
        .map_err(|_| Blocker::PendingRecovery)?;
        let receipt = scope.cutover.runtime_base.join("omavless-login.receipt");
        let login_receipt_absent = absent(&receipt);
        if !login_receipt_absent {
            captured.push(
                Captured::read(&receipt, uid, 0o600, 1024).map_err(|_| Blocker::PendingRecovery)?,
            );
        }
        let mut context = Self {
            scope,
            uid,
            owner,
            owner_identity: identity,
            migration,
            directories,
            captured,
            units: observed_units,
            login_receipt_absent,
            absent_members: Vec::new(),
        };
        if managed_pair_update {
            // A completed managed setup may retain its original backup while
            // a later ordinary preset transaction saves a portable template.
            // Capture selector/backup under these same original leases BEFORE
            // distinguishing that state from an unfinished first conversion.
            context.capture_managed_members()?;
        }
        context.validate_template(managed_pair_update, desired.mode)?;
        context.recheck_private()?;
        Ok(context)
    }
    fn validate_template(
        &self,
        managed_pair_update: bool,
        mode: crate::desired::RoutingMode,
    ) -> Result<()> {
        let template = std::str::from_utf8(&self.captured[4].bytes)
            .map_err(|_| Blocker::TemplateRequiresReview)?;
        let selector = self.scope.config.join(crate::managed_pair::SELECTOR);
        let selected = managed_pair_update
            && self.captured.iter().any(|capture| {
                capture.path == selector && capture.bytes == crate::managed_pair::SELECTION_BYTES
            });
        if crate::managed_template::for_host(template, true).is_none()
            || (!selected
                && !crate::managed_template::is_bundled_default_mode_variant(template)
                && !crate::core_readiness::ConfigReadiness::from_generated_config(
                    mode,
                    String::new(),
                    template,
                )
                .is_some_and(|policy| policy.managed_dns()))
            || (!selected
                && crate::managed_template::is_bundled_default_mode_variant(template)
                && !absent(
                    &self
                        .scope
                        .config
                        .join("route-template.pre-managed-dns.yaml"),
                ))
        {
            return Err(Blocker::TemplateRequiresReview);
        }
        // Package-only update preserves all captured bytes. Selected-host
        // rendering, not another prepare-template/backup write, adds DNS flags.
        Ok(())
    }
    fn recheck_private(&mut self) -> Result<()> {
        for directory in &self.directories {
            directory.recheck()?;
        }
        if !same(
            &self.owner_identity,
            &fs::symlink_metadata(&self.scope.runtime.owner_lock)
                .map_err(|_| Blocker::StateChanged)?,
        ) || !same(
            &self.owner_identity,
            &self
                .owner
                ._file
                .metadata()
                .map_err(|_| Blocker::StateChanged)?,
        ) || !self.migration.authorizes(&self.scope.cutover, self.uid)
        {
            return Err(Blocker::StateChanged);
        }
        if crate::pending_private_transaction::pending_at(&self.scope.cutover.state_directory)
            || !absent(&self.scope.config.join(".config.candidate.yaml"))
            || !absent(
                &self
                    .scope
                    .config
                    .join(".route-template.managed-dns.candidate"),
            )
        {
            return Err(Blocker::PendingRecovery);
        }
        if !absent(&self.scope.runtime.socket)
            || !absent(&self.scope.runtime.directory.join("mihomo.sock"))
            || absent(
                &self
                    .scope
                    .cutover
                    .runtime_base
                    .join("omavless-login.receipt"),
            ) != self.login_receipt_absent
        {
            return Err(Blocker::StateChanged);
        }
        // This package-only operation neither consumes nor migrates unrelated
        // siblings (e.g. retained support notes). Authority remains the pinned
        // directory, exact captured canonical members and reserved transaction
        // absence fence above, repeated while both original leases are held.
        // Do not enumerate, traverse, delete or normalize ancillary entries.
        for capture in &mut self.captured {
            capture.recheck()?;
        }
        if self.absent_members.iter().any(|path| !absent(path)) {
            return Err(Blocker::StateChanged);
        }
        Ok(())
    }

    fn capture_managed_members(&mut self) -> Result<()> {
        for (name, maximum) in [
            (crate::managed_pair::SELECTOR, 64),
            ("route-template.pre-managed-dns.yaml", 1024 * 1024),
        ] {
            let path = self.scope.config.join(name);
            if absent(&path) {
                self.absent_members.push(path);
            } else {
                let capture = Captured::read(&path, self.uid, 0o600, maximum)?;
                if name == crate::managed_pair::SELECTOR
                    && capture.bytes != crate::managed_pair::SELECTION_BYTES
                {
                    return Err(Blocker::PrivateState);
                }
                self.captured.push(capture);
            }
        }
        let candidate = self.scope.config.join(".managed-dns-selection.candidate");
        if !absent(&candidate) {
            return Err(Blocker::PendingRecovery);
        }
        self.absent_members.push(candidate);
        self.recheck_private()
    }
    pub(crate) fn recheck(&mut self) -> Result<()> {
        self.recheck_private()?;
        if units()? != self.units {
            return Err(Blocker::StateChanged);
        }
        self.recheck_private()
    }
}

fn precheck_stopped_owner(scope: &Scope, uid: u32) -> Result<()> {
    // Advisory prerequisite before ephemeral writes. Authority is earned again
    // from fresh descriptor-backed captures under BOTH original held flocks.
    let marker = read_marker_existing(&scope.cutover, uid).map_err(|_| Blocker::Ownership)?;
    if marker.phase() != OwnershipPhase::Rust
        || crate::frontend_bridge::read_committed_plugin_target(&scope.cutover, uid)
            .map_err(|_| Blocker::Ownership)?
            != BridgeTarget::Rust
    {
        return Err(Blocker::Ownership);
    }
    let desired = Captured::read(
        &scope.cutover.state_directory.join("desired.json"),
        uid,
        0o600,
        MAX_DESIRED_STATE_BYTES as usize,
    )
    .map_err(|_| Blocker::DesiredUnavailable)?;
    let desired: DesiredState =
        serde_json::from_slice(&desired.bytes).map_err(|_| Blocker::DesiredUnavailable)?;
    desired
        .validate()
        .map_err(|_| Blocker::DesiredUnavailable)?;
    if desired.connected {
        return Err(Blocker::DesiredConnected);
    }
    let store = Captured::read(
        &scope.config.join("profiles.json"),
        uid,
        0o600,
        MAX_PRIVATE_STORE_BYTES,
    )
    .map_err(|_| Blocker::PrivateState)?;
    let raw: serde_json::Value =
        serde_json::from_slice(&store.bytes).map_err(|_| Blocker::PrivateState)?;
    if raw["startupConfigured"] != true || raw["startup"]["enabled"] != false {
        return Err(Blocker::StartupRequiresOff);
    }
    validate_existing_store_without_repair(
        std::str::from_utf8(&store.bytes).map_err(|_| Blocker::PrivateState)?,
    )
    .map_err(|_| Blocker::PrivateState)?;
    Ok(())
}

/// Internal admission only, never an exported advisory ticket. The concrete
/// consumer keeps this ORIGINAL value through its entire package-only phase.
pub(crate) fn acquire_current() -> Result<UpgradeContext> {
    acquire_current_for(None)
}

fn acquire_current_for(target098: Option<&str>) -> Result<UpgradeContext> {
    crate::cutover_activation::environment::with_current(|| {
        let uid = Uid::current().as_raw();
        let scope = Scope::current(uid)?;
        if let Some(target) = target098 {
            predecessor098(target)?;
        } else {
            predecessor()?;
        }
        let observed_units = units()?;
        let observer = crate::production_observation::ProductionOwnershipObserver::current()
            .map_err(|_| Blocker::HostAdmissionUnavailable)?;
        observer
            .verify_native_empty()
            .map_err(|_| Blocker::HostAdmissionUnavailable)?;
        let mut context = UpgradeContext::acquire(scope, uid, observed_units, target098.is_some())?;
        observer
            .verify_native_empty()
            .map_err(|_| Blocker::HostAdmissionUnavailable)?;
        context.recheck()?;
        if let Some(target) = target098 {
            predecessor098(target)?;
        } else {
            predecessor()?;
        }
        Ok(context)
    })
    .map_err(|_| Blocker::Environment)?
}

#[cfg(any(test, feature = "developer-upgrade082", feature = "setup-upgrade082"))]
#[path = "setup_upgrade082/delivery_metadata.rs"]
mod delivery_metadata;
#[cfg(any(test, feature = "developer-upgrade082", feature = "setup-upgrade082"))]
#[path = "setup_upgrade082/package_phase.rs"]
mod package_phase;

#[cfg(any(test, feature = "developer-upgrade082", feature = "setup-upgrade082"))]
#[path = "setup_upgrade082/terminal_copy.rs"]
mod terminal_copy;
#[cfg(feature = "setup-upgrade082")]
pub use package_phase::run_delivery_update;
#[cfg(feature = "developer-upgrade082")]
pub use package_phase::run_development_update;
#[cfg(feature = "setup-upgrade082")]
pub use package_phase::run_pair_update;
#[cfg(feature = "setup-upgrade082")]
#[path = "setup_upgrade082/maintenance.rs"]
mod maintenance;
#[cfg(feature = "setup-upgrade082")]
#[path = "setup_upgrade082/root_stage.rs"]
mod root_stage;
#[cfg(feature = "setup-upgrade082")]
pub use maintenance::run_root as prepare_pair_root;

#[cfg(test)]
mod tests;
