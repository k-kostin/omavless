// SPDX-License-Identifier: MIT

//! Read-only production-store and config-preparation preflight for R5.

use crate::cutover::{CutoverError, CutoverPaths, MigrationLock};
use crate::{RuntimeError, RuntimePaths};
use nix::unistd::Uid;
use omavless_domain::config::MAX_TEMPLATE_BYTES;
use omavless_domain::private_store::{PrivateStoreError, StoreProjection, parse_private_store};
use omavless_store::{StoreIoError, read_private_utf8};
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePreflightError {
    UnsafeHome,
    StoreIo(StoreIoError),
    TemplateIo(StoreIoError),
    TemplateTooLarge,
    Store(PrivateStoreError),
    Runtime(RuntimeError),
    Cutover(CutoverError),
}

impl fmt::Display for StorePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeHome => "OmaVLESS private home path is unsafe",
            Self::StoreIo(error) | Self::TemplateIo(error) => return error.fmt(formatter),
            Self::TemplateTooLarge => "Routing template is too large",
            Self::Store(error) => return error.fmt(formatter),
            Self::Runtime(error) => return error.fmt(formatter),
            Self::Cutover(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for StorePreflightError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePreflight {
    pub projection: StoreProjection,
    pub config_ready: bool,
}

/// Private prepared config plus its credential-free projection. This type
/// intentionally has no `Debug`, clone, or serialization implementation.
pub struct PreparedStore {
    pub projection: StoreProjection,
    pub config: String,
}

/// Credential-free compatibility classification, not a cutover authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreCompatibility {
    Compatible,
    Unavailable,
    Incompatible,
}

impl StoreCompatibility {
    #[must_use]
    pub fn public_json(self) -> serde_json::Value {
        let (code, message, recovery) = match self {
            Self::Compatible => (
                "compatible",
                "Private store passed native validation",
                "none",
            ),
            Self::Unavailable => (
                "store_unavailable",
                "Private store is missing, unsafe or unreadable; restore a private same-user store before retrying",
                "restore_private_store",
            ),
            Self::Incompatible => (
                "store_requires_repair",
                "Private store failed native validation; keep legacy ownership and repair or export through the current plugin where possible, or restore a valid backup",
                "legacy_repair_or_export",
            ),
        };
        serde_json::json!({"schemaVersion":1,"compatible":self == Self::Compatible,
            "code":code,"message":message,"recovery":recovery})
    }
}

/// The production cutover guard and diagnostic use the same strict boundary.
/// The caller holds the migration lock; raw parser errors never escape here.
pub(crate) fn inspect_store_compatibility(path: &Path, uid: u32) -> StoreCompatibility {
    if crate::private_store_transaction::validate_store_path(path, uid).is_err() {
        return StoreCompatibility::Unavailable;
    }
    match read_private_utf8(path, uid) {
        Ok(input) if parse_private_store(&input).is_ok() => StoreCompatibility::Compatible,
        Ok(_) | Err(StoreIoError::InvalidUtf8 | StoreIoError::TooLarge) => {
            StoreCompatibility::Incompatible
        }
        Err(_) => StoreCompatibility::Unavailable,
    }
}

pub fn current_store_compatibility() -> Result<StoreCompatibility, StorePreflightError> {
    let uid = Uid::current().as_raw();
    let paths = CutoverPaths::current(uid).map_err(StorePreflightError::Cutover)?;
    let _lock = MigrationLock::acquire(&paths, uid).map_err(StorePreflightError::Cutover)?;
    Ok(inspect_store_compatibility(
        &private_config_directory()?.join("profiles.json"),
        uid,
    ))
}

fn private_config_directory() -> Result<PathBuf, StorePreflightError> {
    let home = env::var_os("OMAVLESS_HOME")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or(StorePreflightError::UnsafeHome)?;
    if !home.is_absolute() {
        return Err(StorePreflightError::UnsafeHome);
    }
    Ok(home.join(".config/omavless"))
}

pub fn prepare_current_store() -> Result<PreparedStore, StorePreflightError> {
    let uid = Uid::current().as_raw();
    let directory = private_config_directory()?;
    let store_text = read_private_utf8(&directory.join("profiles.json"), uid)
        .map_err(StorePreflightError::StoreIo)?;
    let store = parse_private_store(&store_text).map_err(StorePreflightError::Store)?;
    let template = read_private_utf8(&directory.join("route-template.yaml"), uid)
        .map_err(StorePreflightError::TemplateIo)?;
    if template.len() > MAX_TEMPLATE_BYTES {
        return Err(StorePreflightError::TemplateTooLarge);
    }
    let runtime = RuntimePaths::current().map_err(StorePreflightError::Runtime)?;
    let controller = runtime.directory.join("mihomo.sock");
    let controller = controller.to_str().ok_or(StorePreflightError::Runtime(
        RuntimeError::UnsafeRuntimeDirectory,
    ))?;
    let config = store
        .prepare_last_config(&template, controller)
        .map_err(StorePreflightError::Store)?;
    Ok(PreparedStore {
        projection: store.projection(),
        config,
    })
}

pub fn current_store_preflight() -> Result<StorePreflight, StorePreflightError> {
    let prepared = prepare_current_store()?;
    Ok(StorePreflight {
        projection: prepared.projection,
        config_ready: true,
    })
}
