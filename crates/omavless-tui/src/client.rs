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
}

impl Read {
    pub fn method(self) -> &'static str {
        match self {
            Self::Hello => "system.hello",
            Self::Capabilities => "capabilities.get",
            Self::Snapshot => "ui.snapshot",
            Self::Observation => "runtime.observation",
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
    let observed = success(read(Read::Observation)?)?;
    let mut snapshot = Snapshot::parse(&meta, &observed, instance)?;
    snapshot.actions_available = methods.iter().any(|m| m == "plugin.action");
    Ok(snapshot)
}
