// SPDX-License-Identifier: MIT
//! Fixed read vocabulary; the CLI adapter supplies the existing authenticated IPC.
use crate::model::{ReadError, Snapshot, opaque};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read {
    Hello,
    Capabilities,
    Snapshot,
    Observation,
    Traffic,
    Diagnostics,
}

impl Read {
    pub fn method(self) -> &'static str {
        match self {
            Self::Hello => "system.hello",
            Self::Capabilities => "capabilities.get",
            Self::Snapshot => "ui.snapshot",
            Self::Observation => "runtime.observation",
            Self::Traffic => "runtime.traffic",
            Self::Diagnostics => "diagnostics.summary",
        }
    }
    pub fn params(self) -> Value {
        if self == Self::Hello {
            json!({"versions":[1]})
        } else {
            json!({})
        }
    }
}

pub fn load(
    read: &mut impl FnMut(Read) -> Result<Value, ReadError>,
) -> Result<Snapshot, ReadError> {
    load_page(read, crate::inspection::Page::Profiles)
}

pub fn load_page(
    read: &mut impl FnMut(Read) -> Result<Value, ReadError>,
    page: crate::inspection::Page,
) -> Result<Snapshot, ReadError> {
    use crate::inspection::{Diagnostics, Page, Traffic};
    fn success(value: Value) -> Result<Value, ReadError> {
        // Production transport already validates bounded framing, envelope and ID.
        // Do not forward remote error strings to either the terminal or stderr.
        if value["ok"] != true {
            return Err(ReadError::Unavailable);
        }
        Ok(value)
    }
    let hello = success(read(Read::Hello)?)?;
    let h = &hello["result"];
    let instance = h["instanceId"]
        .as_str()
        .filter(|s| opaque(s))
        .ok_or(ReadError::Invalid)?;
    if h["version"] != 1 || h["runtimeOwnership"] != true {
        return Err(ReadError::Incompatible);
    }
    let capabilities = success(read(Read::Capabilities)?)?;
    let methods = capabilities["result"]["methods"]
        .as_array()
        .filter(|v| v.len() <= 128)
        .ok_or(ReadError::Invalid)?;
    if capabilities["result"]["runtimeOwnership"] != true
        || !["ui.snapshot", "runtime.observation"]
            .iter()
            .all(|m| methods.iter().any(|v| v == m))
    {
        return Err(ReadError::Incompatible);
    }
    let meta = success(read(Read::Snapshot)?)?;
    let method = match page {
        Page::Traffic if methods.iter().any(|m| m == "runtime.traffic") => Some(Read::Traffic),
        Page::Diagnostics if methods.iter().any(|m| m == "diagnostics.summary") => {
            Some(Read::Diagnostics)
        }
        _ => None,
    };
    // Extra reads are bracketed by metadata and observation from the same owner.
    // Failure means unavailable, never zero; unrelated connection facts survive.
    let extra = method.and_then(|m| read(m).ok());
    let observed = success(read(Read::Observation)?)?;
    let mut snapshot = Snapshot::parse(&meta, &observed, instance)?;
    snapshot.actions_available = methods.iter().any(|m| m == "plugin.action");
    snapshot.inspection_available = (
        methods.iter().any(|m| m == "runtime.traffic"),
        methods.iter().any(|m| m == "diagnostics.summary"),
    );
    if let Some(value) = extra {
        if value["ok"] == true && value["revision"].as_u64() != Some(snapshot.revision) {
            return Err(ReadError::Changed);
        }
        if method == Some(Read::Traffic) {
            snapshot.traffic = Traffic::parse(&value);
        } else {
            snapshot.diagnostics = Diagnostics::parse(&value);
        }
    }
    Ok(snapshot)
}
