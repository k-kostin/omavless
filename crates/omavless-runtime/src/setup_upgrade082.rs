// SPDX-License-Identifier: MIT
//! Before-effect context for a future SINGLE-PROCESS predecessor upgrade.
//! No package/service/core mutation, child spawning for effects, serialized
//! ticket, or public success path. The app package lacks an all-user upgrade
//! guard and this candidate has no reviewed effect consumer yet.
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
        };
        write!(f, "OmaVLESS upgrade082 refused before effects: {reason}")
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
        for (name, expected) in [("XDG_RUNTIME_DIR", &runtime), ("XDG_STATE_HOME", &state)] {
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

fn units() -> Result<[String; 2]> {
    let mut observations = Vec::new();
    for name in ["omavless-runtime.service", "omavless-login-prepare.service"] {
        let mut command = Command::new("/usr/bin/systemctl");
        command.env_clear().env("PATH", "/usr/bin").env("LC_ALL", "C")
            .env("XDG_RUNTIME_DIR", format!("/run/user/{}", Uid::current().as_raw()))
            .args(["--user", "show", name, "--no-pager",
                "--property=LoadState,ActiveState,SubState,MainPID,Job,FragmentPath,DropInPaths,UnitFileState"]);
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
    if values.len() != 8
        || !settled
        || values.get("LoadState") != Some(&"loaded")
        || values.get("MainPID") != Some(&"0")
        || values.get("Job") != Some(&"")
        || values.get("DropInPaths") != Some(&"")
        || values.get("FragmentPath").copied()
            != Some(format!("/usr/lib/systemd/user/{name}").as_str())
        || values.get("UnitFileState") != Some(&if runtime { "disabled" } else { "static" })
    {
        return Err(Blocker::RuntimeActive);
    }
    Ok(())
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
}
impl UpgradeContext {
    fn acquire(scope: Scope, uid: u32, observed_units: [String; 2]) -> Result<Self> {
        let directories = [
            &scope.runtime.directory,
            &scope.cutover.runtime_base,
            &scope.cutover.state_directory,
            &scope.config,
        ]
        .into_iter()
        .map(|path| Directory::pin(path, uid))
        .collect::<Result<Vec<_>>>()?;
        if !absent(&scope.runtime.socket) || !absent(&scope.runtime.directory.join("mihomo.sock")) {
            return Err(Blocker::RuntimeActive);
        }
        let path = &scope.runtime.owner_lock;
        let identity = fs::symlink_metadata(path).map_err(|_| Blocker::ExclusionUnavailable)?;
        if !identity.is_file()
            || identity.uid() != uid
            || identity.mode() & 0o7777 != 0o600
            || identity.nlink() != 1
        {
            return Err(Blocker::ExclusionUnavailable);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Blocker::ExclusionUnavailable)?;
        if !same(
            &identity,
            &file.metadata().map_err(|_| Blocker::ExclusionUnavailable)?,
        ) {
            return Err(Blocker::StateChanged);
        }
        let held =
            Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_, _)| Blocker::Busy)?;
        let owner = OwnerLock { _file: held };
        let migration = MigrationLock::acquire_existing(&scope.cutover, uid).map_err(|error| {
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
        let template =
            std::str::from_utf8(&captured[4].bytes).map_err(|_| Blocker::TemplateRequiresReview)?;
        if crate::managed_template::for_host(template, true).is_none()
            || (!crate::managed_template::is_bundled_default_mode_variant(template)
                && !crate::core_readiness::ConfigReadiness::from_generated_config(
                    desired.mode,
                    String::new(),
                    template,
                )
                .is_some_and(|policy| policy.managed_dns()))
        {
            return Err(Blocker::TemplateRequiresReview);
        }
        if crate::managed_template::is_bundled_default_mode_variant(template)
            && !absent(&scope.config.join("route-template.pre-managed-dns.yaml"))
        {
            return Err(Blocker::TemplateRequiresReview);
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
        };
        context.recheck_private()?;
        Ok(context)
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
        let mut entries =
            fs::read_dir(&self.scope.cutover.state_directory).map_err(|_| Blocker::PrivateState)?;
        for entry in entries.by_ref().take(4) {
            let entry = entry.map_err(|_| Blocker::PrivateState)?;
            if !["ownership.json", "frontend-bridge.target", "desired.json"]
                .iter()
                .any(|name| entry.file_name() == *name)
            {
                return Err(Blocker::PrivateState);
            }
        }
        if entries.next().is_some() {
            return Err(Blocker::PrivateState);
        }
        for capture in &mut self.captured {
            capture.recheck()?;
        }
        Ok(())
    }
    pub(crate) fn recheck(&mut self) -> Result<()> {
        self.recheck_private()?;
        if units()? != self.units {
            return Err(Blocker::StateChanged);
        }
        self.recheck_private()
    }
}

/// Internal precursor only. Not a callable updater or public readiness report.
/// The future coordinator must keep this ORIGINAL value through all effects.
pub(crate) fn acquire_current() -> Result<UpgradeContext> {
    let uid = Uid::current().as_raw();
    let scope = Scope::current(uid)?;
    predecessor()?;
    let mut context = UpgradeContext::acquire(scope, uid, units()?)?;
    context.recheck()?;
    predecessor()?;
    // The private captures/exclusions above are implemented. Full strict orphan
    // core/TUN absence, authenticated manager roots/loaded unit execution and
    // staged-candidate authenticity are still separate admission work. Never
    // return this precursor as a production effect capability meanwhile.
    Err(Blocker::HostAdmissionUnavailable)
}

#[cfg(test)]
mod tests;
