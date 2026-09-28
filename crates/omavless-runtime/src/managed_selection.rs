// SPDX-License-Identifier: MIT

//! Ordinary-user opt-in to the reviewed pair. This never performs enrollment,
//! starts a service, changes DNS or launches a core. The daemon must be stopped.

use crate::core_readiness::ConfigReadiness;
use crate::desired::{DesiredPaths, RoutingMode, read_desired};
use crate::managed_pair::{ManagedPair, SELECTION_BYTES, SELECTOR};
use crate::native_host::private_directory;
use nix::fcntl::{RenameFlags, renameat2};
use nix::unistd::Uid;
use serde_json::{Value, json};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const STAGING: &str = ".managed-dns-selection.candidate";
const MAX_TEMPLATE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Refused,
    OutcomeUnknown,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Refused => {
                "DNS pair selection refused; keep the VPN disconnected and inspect setup"
            }
            Self::OutcomeUnknown => {
                "DNS pair selection outcome unknown; inspect status before retrying"
            }
        })
    }
}

impl std::error::Error for Error {}

fn config_directory() -> Result<(PathBuf, u32), Error> {
    let uid = Uid::current().as_raw();
    if uid == 0 || Uid::effective().as_raw() != uid {
        return Err(Error::Refused);
    }
    let home = env::var_os("OMAVLESS_HOME")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or(Error::Refused)?;
    if !home.is_absolute() || home.as_os_str().as_encoded_bytes().len() > 4096 {
        return Err(Error::Refused);
    }
    let directory = home.join(".config/omavless");
    if !private_directory(&directory, uid) {
        return Err(Error::Refused);
    }
    Ok((directory, uid))
}

fn fixed_unit(properties: &[&str], user: bool) -> Result<Value, Error> {
    let mut command = Command::new("/usr/bin/timeout");
    command.args(["5s", "/usr/bin/systemctl"]);
    command.arg(if user { "--user" } else { "--system" });
    command.args([
        "show",
        if user {
            "omavless-runtime.service"
        } else {
            "omavless-dns-broker.service"
        },
        "--no-pager",
    ]);
    for key in properties {
        command.arg(format!("--property={key}"));
    }
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|_| Error::Refused)?;
    if !output.status.success() || output.stdout.len() > 4096 || !output.stderr.is_empty() {
        return Err(Error::Refused);
    }
    let text = std::str::from_utf8(&output.stdout).map_err(|_| Error::Refused)?;
    parse_unit(text, properties)
}

fn parse_unit(text: &str, properties: &[&str]) -> Result<Value, Error> {
    let mut values = serde_json::Map::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or(Error::Refused)?;
        if !properties.contains(&key) || values.insert(key.to_owned(), json!(value)).is_some() {
            return Err(Error::Refused);
        }
    }
    if values.len() != properties.len() {
        return Err(Error::Refused);
    }
    Ok(Value::Object(values))
}

fn stopped_runtime() -> bool {
    fixed_unit(&["LoadState", "ActiveState", "SubState", "MainPID"], true).is_ok_and(|value| {
        value["LoadState"] == "loaded"
            && value["ActiveState"] == "inactive"
            && value["SubState"] == "dead"
            && value["MainPID"] == "0"
    })
}

fn idle_broker() -> bool {
    fixed_unit(
        &[
            "LoadState",
            "ActiveState",
            "SubState",
            "NFileDescriptorStore",
        ],
        false,
    )
    .is_ok_and(|value| {
        value["LoadState"] == "loaded"
            && value["ActiveState"] == "active"
            && value["SubState"] == "running"
            && value["NFileDescriptorStore"] == "0"
    })
}

fn absent(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(Error::Refused),
    }
}

fn managed_template(directory: &Path, uid: u32) -> bool {
    let path = directory.join("route-template.yaml");
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return false;
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > MAX_TEMPLATE_BYTES
    {
        return false;
    }
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_TEMPLATE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.is_empty()
        || bytes.len() as u64 > MAX_TEMPLATE_BYTES
    {
        return false;
    }
    let Ok(template) = String::from_utf8(bytes) else {
        return false;
    };
    ConfigReadiness::from_generated_config(RoutingMode::Rule, String::new(), &template)
        .is_some_and(|policy| policy.managed_dns())
}

pub fn status_current() -> Result<Value, Error> {
    let (directory, uid) = config_directory()?;
    let selected = ManagedPair::detect(&directory, uid)
        .map_err(|_| Error::Refused)?
        .is_some();
    Ok(json!({"schemaVersion":1,"selected":selected,"scope":"local_pair_only"}))
}

pub fn select_current() -> Result<Value, Error> {
    let (directory, uid) = config_directory()?;
    if !stopped_runtime()
        || !idle_broker()
        || !matches!(fs::symlink_metadata("/sys/class/net/Meta"),
                     Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        || !managed_template(&directory, uid)
    {
        return Err(Error::Refused);
    }
    let desired_paths = DesiredPaths::current().map_err(|_| Error::Refused)?;
    let desired = read_desired(&desired_paths, uid).map_err(|_| Error::Refused)?;
    if desired.connected {
        return Err(Error::Refused);
    }
    ManagedPair::validate_package().map_err(|_| Error::Refused)?;
    if !stopped_runtime()
        || !idle_broker()
        || !matches!(fs::symlink_metadata("/sys/class/net/Meta"),
                     Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        return Err(Error::Refused);
    }
    select_at(&directory, uid)?;
    Ok(
        json!({"schemaVersion":1,"selected":true,"restartRequired":true,
              "scope":"local_pair_only"}),
    )
}

fn select_at(directory: &Path, uid: u32) -> Result<(), Error> {
    if !private_directory(directory, uid) {
        return Err(Error::Refused);
    }
    let selector = directory.join(SELECTOR);
    let staging = directory.join(STAGING);
    if !absent(&selector)? || !absent(&staging)? {
        return Err(Error::Refused);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&staging)
        .map_err(|_| Error::Refused)?;
    file.write_all(SELECTION_BYTES)
        .map_err(|_| Error::Refused)?;
    file.sync_all().map_err(|_| Error::Refused)?;
    let metadata = file.metadata().map_err(|_| Error::Refused)?;
    if metadata.uid() != uid
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o7777 != 0o600
    {
        return Err(Error::Refused);
    }
    let parent = File::open(directory).map_err(|_| Error::Refused)?;
    renameat2(
        &parent,
        Path::new(STAGING),
        &parent,
        Path::new(SELECTOR),
        RenameFlags::RENAME_NOREPLACE,
    )
    .map_err(|_| Error::Refused)?;
    parent.sync_all().map_err(|_| Error::OutcomeUnknown)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_projection_rejects_missing_duplicate_and_unknown_fields() {
        let keys = ["ActiveState", "MainPID"];
        assert!(parse_unit("ActiveState=inactive\nMainPID=0\n", &keys).is_ok());
        for invalid in [
            "ActiveState=inactive\n",
            "ActiveState=inactive\nMainPID=0\nOther=x\n",
            "ActiveState=inactive\nActiveState=inactive\nMainPID=0\n",
            "ActiveState=inactive\nMainPID\n",
        ] {
            assert_eq!(parse_unit(invalid, &keys), Err(Error::Refused));
        }
    }

    #[test]
    fn template_gate_requires_exact_managed_flags_and_private_regular_file() {
        let root = tempfile::tempdir().unwrap();
        let uid = Uid::current().as_raw();
        let path = root.path().join("route-template.yaml");
        let template = "tun:\n  enable: true\n  device: Meta\n  disable-system-dns: true\n  omavless-dns-broker: true\n";
        fs::write(&path, template).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(managed_template(root.path(), uid));
        fs::write(&path, template.replace("  omavless-dns-broker: true\n", "")).unwrap();
        assert!(!managed_template(root.path(), uid));
        fs::write(&path, template).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!managed_template(root.path(), uid));
    }

    #[test]
    fn fixed_private_selection_refuses_replacement_and_links() {
        let root = tempfile::tempdir().unwrap();
        let uid = Uid::current().as_raw();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(select_at(root.path(), uid), Ok(()));
        assert_eq!(
            fs::read(root.path().join(SELECTOR)).unwrap(),
            SELECTION_BYTES
        );
        assert_eq!(
            fs::metadata(root.path().join(SELECTOR))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        assert_eq!(select_at(root.path(), uid), Err(Error::Refused));
        fs::remove_file(root.path().join(SELECTOR)).unwrap();
        std::os::unix::fs::symlink("missing", root.path().join(SELECTOR)).unwrap();
        assert_eq!(select_at(root.path(), uid), Err(Error::Refused));
    }
}
