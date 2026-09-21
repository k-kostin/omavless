// SPDX-License-Identifier: MIT
//! Fixed packaged login integration. No client-supplied epoch, unit or command.
use crate::cutover::{CutoverPaths, MigrationLock, OwnershipPhase, read_marker_existing};
use crate::desired::DesiredState;
use crate::isolated_validation::ValidationSnapshot;
use crate::login_transaction::{LoginHostError, LoginPaths, LoginReadiness, consume_login};
use crate::native_host::NativeHostPaths;
use crate::production_observation::{ProductionOwnershipObserver, bounded_fixed_query};
use nix::unistd::{Uid, getppid};
use omavless_domain::private_store::PrivateStore;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const LOGIN_UNIT: &str = "omavless-login-prepare.service";
const RUNTIME_UNIT: &str = "omavless-runtime.service";
const BINARY: &str = "/usr/bin/omavless";
const MANAGER_PROPERTIES: &str = "Id,LoadState,ActiveState,MainPID,InvocationID,User,FragmentPath";
const MANAGER_UNIT_PATH: &str = "/usr/lib/systemd/system/user@.service";
const LOGIN_BYTES: &[u8] =
    include_bytes!("../../../packaging/systemd/omavless-login-prepare.service");
const RUNTIME_BYTES: &[u8] = include_bytes!("../../../packaging/systemd/omavless-runtime.service");

/// These fixed errors deliberately contain no account paths, epochs or input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invocation,
    Package,
    Ownership,
    Validation,
    Recovery,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invocation => "OmaVLESS login must run through its packaged user service",
            Self::Package => "OmaVLESS login package integration is unavailable",
            Self::Ownership => "OmaVLESS login ownership could not be verified",
            Self::Validation => {
                "OmaVLESS login validation failed; review startup settings and cached rules"
            }
            Self::Recovery => "OmaVLESS login requires manual recovery",
        })
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

fn fields(text: &str) -> Result<BTreeMap<&str, &str>> {
    if text.len() > 8192 || !text.ends_with('\n') {
        return Err(Error::Invocation);
    }
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(Error::Invocation)?;
        if fields.insert(key, value).is_some() {
            return Err(Error::Invocation);
        }
    }
    Ok(fields)
}
fn epoch_valid(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && value.bytes().any(|b| b != b'0')
}

fn query(system: bool, unit: &str, properties: &str) -> Result<String> {
    let uid = Uid::current().as_raw();
    // All callers below supply only literals or this fixed same-user unit.
    if unit != LOGIN_UNIT && unit != RUNTIME_UNIT && unit != format!("user@{uid}.service") {
        return Err(Error::Invocation);
    }
    let mut command = Command::new("/usr/bin/systemctl");
    // Do not inherit remote/bus overrides or shell configuration.
    command
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("LC_ALL", "C")
        .env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"))
        .args([
            if system { "--system" } else { "--user" },
            "show",
            unit,
            "--no-pager",
        ])
        .arg(format!("--property={properties}"));
    bounded_fixed_query(command, Duration::from_secs(2)).map_err(|_| Error::Invocation)
}

fn validate_manager(text: &str, uid: u32) -> Result<(u32, String)> {
    let f = fields(text)?;
    let pid: u32 = f
        .get("MainPID")
        .ok_or(Error::Invocation)?
        .parse()
        .map_err(|_| Error::Invocation)?;
    let epoch = *f.get("InvocationID").ok_or(Error::Invocation)?;
    if f.len() != 7
        || f.get("Id") != Some(&format!("user@{uid}.service").as_str())
        || f.get("User") != Some(&uid.to_string().as_str())
        || f.get("FragmentPath") != Some(&MANAGER_UNIT_PATH)
        || f.get("LoadState") != Some(&"loaded")
        || !matches!(f.get("ActiveState"), Some(&"active" | &"activating"))
        || pid == 0
        || !epoch_valid(epoch)
    {
        return Err(Error::Invocation);
    }
    Ok((pid, epoch.to_owned()))
}

/// Epoch comes from the root system manager, never from a caller's variable.
pub(crate) fn manager_epoch() -> Result<String> {
    let uid = Uid::current().as_raw();
    let (pid, epoch) = validate_manager(
        &query(true, &format!("user@{uid}.service"), MANAGER_PROPERTIES)?,
        uid,
    )?;
    let process = fs::metadata(format!("/proc/{pid}")).map_err(|_| Error::Invocation)?;
    // The root manager authenticates its fixed User/MainPID assignment. Reading
    // /proc/PID/exe can be denied for the non-dumpable systemd user manager;
    // requiring ptrace permission would reject normal protected installations.
    installed_file(Path::new(MANAGER_UNIT_PATH), None).map_err(|_| Error::Invocation)?;
    if process.uid() != uid {
        return Err(Error::Invocation);
    }
    Ok(epoch)
}

fn validate_invocation(text: &str, condition: bool, pid: u32, invocation: &str) -> Result<()> {
    let f = fields(text)?;
    let pid_key = if condition { "ControlPID" } else { "MainPID" };
    if f.len() != 7
        || f.get("Id") != Some(&LOGIN_UNIT)
        || f.get("LoadState") != Some(&"loaded")
        || f.get("ActiveState") != Some(&"activating")
        || f.get("SubState") != Some(&if condition { "condition" } else { "start" })
        || !epoch_valid(invocation)
        || f.get("InvocationID") != Some(&invocation)
        || f.get(pid_key).and_then(|v| v.parse::<u32>().ok()) != Some(pid)
    {
        return Err(Error::Invocation);
    }
    Ok(())
}

fn invocation(condition: bool) -> Result<String> {
    let uid = Uid::current().as_raw();
    let manager = query(true, &format!("user@{uid}.service"), MANAGER_PROPERTIES)?;
    let (manager_pid, epoch) = validate_manager(&manager, uid)?;
    if getppid().as_raw() as u32 != manager_pid
        || std::env::var("MANAGERPID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            != Some(manager_pid)
        || std::env::var("SYSTEMD_EXEC_PID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            != Some(std::process::id())
    {
        return Err(Error::Invocation);
    }
    let own_invocation = std::env::var("INVOCATION_ID").map_err(|_| Error::Invocation)?;
    validate_invocation(
        &query(
            false,
            LOGIN_UNIT,
            "Id,LoadState,ActiveState,SubState,MainPID,ControlPID,InvocationID",
        )?,
        condition,
        std::process::id(),
        &own_invocation,
    )?;
    if manager_epoch()? != epoch {
        return Err(Error::Invocation);
    }
    Ok(epoch)
}

fn installed_file(path: &Path, contents: Option<&[u8]>) -> Result<()> {
    let m = fs::symlink_metadata(path).map_err(|_| Error::Package)?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 {
        return Err(Error::Package);
    }
    if let Some(bytes) = contents
        && (m.len() != bytes.len() as u64 || fs::read(path).map_err(|_| Error::Package)? != bytes)
    {
        return Err(Error::Package);
    }
    Ok(())
}
fn package() -> Result<()> {
    if std::env::var_os("OMAVLESS_HOME").is_some() {
        return Err(Error::Package);
    }
    installed_file(Path::new(BINARY), None)?;
    let expected = fs::metadata(BINARY).map_err(|_| Error::Package)?;
    let running = fs::metadata("/proc/self/exe").map_err(|_| Error::Package)?;
    if expected.dev() != running.dev() || expected.ino() != running.ino() {
        return Err(Error::Package);
    }
    for (name, bytes) in [(LOGIN_UNIT, LOGIN_BYTES), (RUNTIME_UNIT, RUNTIME_BYTES)] {
        let path = format!("/usr/lib/systemd/user/{name}");
        installed_file(Path::new(&path), Some(bytes))?;
        let text = query(false, name, "Id,FragmentPath,DropInPaths,NeedDaemonReload")?;
        let f = fields(&text)?;
        if f.len() != 4
            || f.get("Id") != Some(&name)
            || f.get("FragmentPath") != Some(&path.as_str())
            || f.get("DropInPaths") != Some(&"")
            || f.get("NeedDaemonReload") != Some(&"no")
        {
            return Err(Error::Package);
        }
    }
    crate::cutover_activation::environment::with_current(|| ()).map_err(|_| Error::Package)?;
    Ok(())
}

fn phase_requires_login(phase: OwnershipPhase) -> Result<bool> {
    match phase {
        OwnershipPhase::Rust => Ok(true),
        OwnershipPhase::Legacy | OwnershipPhase::CutoverPreparing => Ok(false),
        _ => Err(Error::Ownership),
    }
}

/// False is the only intentional ExecCondition skip: validated non-native phase.
pub fn condition() -> Result<bool> {
    package()?;
    let _ = invocation(true)?;
    let uid = Uid::current().as_raw();
    let paths = CutoverPaths::current(uid).map_err(|_| Error::Ownership)?;
    let _lock = MigrationLock::acquire(&paths, uid).map_err(|_| Error::Ownership)?;
    if !crate::native_host::private_directory(&paths.state_directory, uid) {
        return Err(Error::Ownership);
    }
    let marker = read_marker_existing(&paths, uid).map_err(|_| Error::Ownership)?;
    if marker.phase() == OwnershipPhase::CutoverPreparing {
        crate::cutover::TransitionBootstrap::from_preparing(&marker)
            .map_err(|_| Error::Ownership)?;
    }
    phase_requires_login(marker.phase())
}

struct Host {
    runtime: std::path::PathBuf,
    epoch: String,
}
impl LoginReadiness for Host {
    fn verify_empty(&mut self) -> std::result::Result<(), LoginHostError> {
        if invocation(false).map_err(|_| LoginHostError)? != self.epoch {
            return Err(LoginHostError);
        }
        ProductionOwnershipObserver::current()
            .and_then(|observer| observer.verify_native_empty())
            .map_err(|_| LoginHostError)
    }
    fn validate_candidate(
        &mut self,
        desired: &DesiredState,
        store: &PrivateStore,
        template: &str,
    ) -> std::result::Result<(), LoginHostError> {
        // Resolve only for enabled login; disabled login needs no installed core.
        let paths = NativeHostPaths::current(&self.runtime).map_err(|_| LoginHostError)?;
        let status = fs::read_to_string("/proc/self/status").map_err(|_| LoginHostError)?;
        if !status.lines().any(|line| {
            line.strip_prefix("NoNewPrivs:")
                .is_some_and(|value| value.trim() == "0")
        }) || !crate::startup_validation::tun_capabilities(
            Path::new("/usr/bin/getcap"),
            &paths.core,
        ) {
            return Err(LoginHostError);
        }
        ValidationSnapshot::capture(
            &paths.data_directory,
            Uid::current().as_raw(),
            desired,
            store,
            template,
        )
        .and_then(|snapshot| {
            snapshot.validate(
                &paths.core,
                &paths.runtime_directory,
                Uid::current().as_raw(),
            )
        })
        .map_err(|_| LoginHostError)
    }
}

pub fn prepare() -> Result<()> {
    package()?;
    let epoch = invocation(false)?;
    let uid = Uid::current().as_raw();
    let cutover = CutoverPaths::current(uid).map_err(|_| Error::Ownership)?;
    let generation = {
        let _lock = MigrationLock::acquire(&cutover, uid).map_err(|_| Error::Ownership)?;
        let marker = read_marker_existing(&cutover, uid).map_err(|_| Error::Ownership)?;
        if marker.phase() != OwnershipPhase::Rust {
            return Err(Error::Ownership);
        }
        marker.generation()
    };
    let runtime = crate::RuntimePaths::current().map_err(|_| Error::Validation)?;
    let home = std::env::var_os("HOME").ok_or(Error::Validation)?;
    let state_base = cutover.state_directory.parent().ok_or(Error::Validation)?;
    let paths = LoginPaths::below(Path::new(&home), &cutover.runtime_base, state_base, uid)
        .map_err(|_| Error::Validation)?;
    let mut host = Host {
        runtime: runtime.directory,
        epoch,
    };
    consume_login(&paths, generation, &host.epoch.clone(), &mut host).map_err(
        |error| match error {
            crate::login_transaction::LoginTransactionError::ManualRecoveryRequired
            | crate::login_transaction::LoginTransactionError::EpochMismatch => Error::Recovery,
            _ => Error::Validation,
        },
    )?;
    Ok(())
}

/// Called under the existing migration lease, before production reconciliation.
pub(crate) fn require_current_receipt(
    paths: &CutoverPaths,
    uid: u32,
    lock: &MigrationLock,
    generation: u64,
) -> Result<()> {
    package()?;
    let epoch = manager_epoch()?;
    crate::login_transaction::check_current_receipt(paths, uid, lock, generation, &epoch)
        .map_err(|_| Error::Recovery)
}

pub(crate) fn startup_configuration_available() -> bool {
    query(false, RUNTIME_UNIT, "UnitFileState")
        .and_then(|text| Ok(fields(&text)?.get("UnitFileState") == Some(&"enabled")))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    const EPOCH: &str = "1234567890abcdef1234567890abcdef";
    #[test]
    #[ignore = "read-only installed user-manager identity check"]
    fn installed_manager_epoch_readonly_optin() {
        assert!(manager_epoch().is_ok());
        // A test executable in a terminal is never the packaged login unit.
        assert!(invocation(false).is_err());
    }
    #[test]
    fn manager_identity_is_exact_and_not_an_oneshot_epoch() {
        let good = format!(
            "Id=user@1000.service\nUser=1000\nFragmentPath={MANAGER_UNIT_PATH}\nLoadState=loaded\nActiveState=active\nMainPID=42\nInvocationID={EPOCH}\n"
        );
        assert_eq!(
            validate_manager(&good, 1000).unwrap(),
            (42, EPOCH.to_owned())
        );
        assert!(validate_manager(&good.replace("active\n", "activating\n"), 1000).is_ok());
        for bad in [
            good.replace("1000", "1001"),
            good.replace("MainPID=42", "MainPID=0"),
            good.replace("User=1000", "User=0"),
            good.replace(MANAGER_UNIT_PATH, "/tmp/user@.service"),
            good.replace("active\n", "deactivating\n"),
            good.replace(EPOCH, "00000000000000000000000000000000"),
            format!("{good}InvocationID={EPOCH}\n"),
            good.trim().to_owned(),
        ] {
            assert_eq!(validate_manager(&bad, 1000), Err(Error::Invocation));
        }
    }
    #[test]
    fn actual_unit_process_required_for_both_fixed_commands() {
        for condition in [true, false] {
            let text = format!(
                "Id={LOGIN_UNIT}\nLoadState=loaded\nActiveState=activating\nSubState={}\nMainPID={}\nControlPID={}\nInvocationID={EPOCH}\n",
                if condition { "condition" } else { "start" },
                if condition { 0 } else { 42 },
                if condition { 42 } else { 0 }
            );
            assert_eq!(validate_invocation(&text, condition, 42, EPOCH), Ok(()));
            assert_eq!(
                validate_invocation(&text, condition, 43, EPOCH),
                Err(Error::Invocation)
            );
            assert_eq!(
                validate_invocation(&text, !condition, 42, EPOCH),
                Err(Error::Invocation)
            );
            assert_eq!(
                validate_invocation(
                    &text.replace(LOGIN_UNIT, RUNTIME_UNIT),
                    condition,
                    42,
                    EPOCH
                ),
                Err(Error::Invocation)
            );
            assert_eq!(
                validate_invocation(&text, condition, 42, "not-an-epoch"),
                Err(Error::Invocation)
            );
        }
    }
    #[test]
    fn only_explicit_non_native_phases_may_skip_login() {
        assert_eq!(phase_requires_login(OwnershipPhase::Rust), Ok(true));
        assert_eq!(phase_requires_login(OwnershipPhase::Legacy), Ok(false));
        assert_eq!(
            phase_requires_login(OwnershipPhase::CutoverPreparing),
            Ok(false)
        );
        assert_eq!(
            phase_requires_login(OwnershipPhase::RollbackPreparing),
            Err(Error::Ownership)
        );
    }
}
