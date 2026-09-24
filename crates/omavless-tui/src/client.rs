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
    Connections,
    ProfileDetails(ProfileTarget),
}

/// Fixed-size private target, never included in derived debug output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ProfileTarget {
    bytes: [u8; 64],
    len: u8,
}
impl std::fmt::Debug for ProfileTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileTarget([private])")
    }
}
impl ProfileTarget {
    pub fn new(value: &str) -> Option<Self> {
        if !opaque(value) || value.len() > 64 {
            return None;
        }
        let mut bytes = [0; 64];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Some(Self {
            bytes,
            len: value.len() as u8,
        })
    }
    pub fn as_str(&self) -> &str {
        // Constructor admits ASCII only; fields are private.
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or_default()
    }
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
            Self::Connections => "runtime.connections",
            Self::ProfileDetails(_) => "profiles.details",
        }
    }
    pub fn params(self) -> Value {
        match self {
            Self::Hello => json!({"versions":[1]}),
            Self::ProfileDetails(target) => json!({"profileId": target.as_str()}),
            _ => json!({}),
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
    load_page_for(read, page, None)
}

pub fn load_page_for(
    read: &mut impl FnMut(Read) -> Result<Value, ReadError>,
    page: crate::inspection::Page,
    selected: Option<&str>,
) -> Result<Snapshot, ReadError> {
    use crate::inspection::{Capabilities, Diagnostics, Page, ProfileDetails, Traffic};
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
    let target = selected.and_then(ProfileTarget::new).filter(|target| {
        meta["result"]["profiles"]
            .as_array()
            .is_some_and(|profiles| {
                profiles.len() <= 256 && profiles.iter().any(|p| p["id"] == target.as_str())
            })
    });
    let method = match page {
        Page::Traffic if methods.iter().any(|m| m == "runtime.traffic") => Some(Read::Traffic),
        Page::Diagnostics if methods.iter().any(|m| m == "diagnostics.summary") => {
            Some(Read::Diagnostics)
        }
        Page::Details if methods.iter().any(|m| m == "profiles.details") => {
            target.map(Read::ProfileDetails)
        }
        _ => None,
    };
    // Extra reads are bracketed by metadata and observation from the same owner.
    // Failure means unavailable, never zero; unrelated connection facts survive.
    let extra = method.and_then(|m| read(m).ok());
    let connections = if page == Page::Traffic && methods.iter().any(|m| m == "runtime.connections")
    {
        read(Read::Connections).ok()
    } else {
        None
    };
    let observed = success(read(Read::Observation)?)?;
    let mut snapshot = Snapshot::parse(&meta, &observed, instance)?;
    snapshot.actions_available = methods.iter().any(|m| m == "plugin.action");
    snapshot.capabilities = Capabilities::parse(methods);
    if let Some(value) = connections {
        if value["ok"] == true
            && (value["revision"].as_u64() != Some(snapshot.revision)
                || value["result"]["instanceId"] != instance)
        {
            return Err(ReadError::Changed);
        }
        snapshot.active_connections = crate::inspection::connection_count(&value);
    }
    snapshot.inspection_available = (
        methods.iter().any(|m| m == "runtime.traffic"),
        methods.iter().any(|m| m == "diagnostics.summary"),
    );
    if let Some(value) = extra {
        if value["ok"] == true && value["revision"].as_u64() != Some(snapshot.revision) {
            return Err(ReadError::Changed);
        }
        match method {
            Some(Read::Traffic) => snapshot.traffic = Traffic::parse(&value),
            Some(Read::Diagnostics) => snapshot.diagnostics = Diagnostics::parse(&value),
            Some(Read::ProfileDetails(target)) => {
                snapshot.profile_details = ProfileDetails::parse(&value, target);
            }
            _ => {}
        }
    }
    Ok(snapshot)
}
