// SPDX-License-Identifier: MIT

//! Private, effect-free S1 host snapshot codecs. No host APIs or IPC callers.
//!
//! Adapters must capture every allowlisted field through typed APIs. These are
//! canonical journal bytes, not GVariant text, shell assignments, or a proof
//! that a host observation is current. The future adapter owns that proof.

use super::Snapshot;
use serde::{Deserialize, Serialize};
use std::fmt;

const MAX_BYTES: usize = 16 * 1024;
const MAX_STRING: usize = 1024;
const MAX_BYPASS_ENTRIES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidSnapshot,
    IncompleteSnapshot,
    UnsupportedActivation,
    NotWritable,
    InconsistentReadback,
}

/// Only the prospective, reviewed GSettings schema fields. Unknown keys and
/// other desktop configuration trees are never admitted into a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopKey {
    Mode,
    AutoconfigUrl,
    IgnoreHosts,
    UseSameProxy,
    HttpEnabled,
    HttpHost,
    HttpPort,
    HttpUseAuthentication,
    HttpAuthenticationUser,
    HttpAuthenticationPassword,
    HttpsHost,
    HttpsPort,
    FtpHost,
    FtpPort,
    SocksHost,
    SocksPort,
}

impl DesktopKey {
    pub const ALL: [Self; 16] = [
        Self::Mode,
        Self::AutoconfigUrl,
        Self::IgnoreHosts,
        Self::UseSameProxy,
        Self::HttpEnabled,
        Self::HttpHost,
        Self::HttpPort,
        Self::HttpUseAuthentication,
        Self::HttpAuthenticationUser,
        Self::HttpAuthenticationPassword,
        Self::HttpsHost,
        Self::HttpsPort,
        Self::FtpHost,
        Self::FtpPort,
        Self::SocksHost,
        Self::SocksPort,
    ];

    /// Fixed schema/key/signature, for a future typed adapter. The deprecated
    /// HTTP enabled key is preserved, never interpreted as proxy readiness.
    pub fn schema_key_type(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Mode => ("org.gnome.system.proxy", "mode", "s"),
            Self::AutoconfigUrl => ("org.gnome.system.proxy", "autoconfig-url", "s"),
            Self::IgnoreHosts => ("org.gnome.system.proxy", "ignore-hosts", "as"),
            Self::UseSameProxy => ("org.gnome.system.proxy", "use-same-proxy", "b"),
            Self::HttpEnabled => ("org.gnome.system.proxy.http", "enabled", "b"),
            Self::HttpHost => ("org.gnome.system.proxy.http", "host", "s"),
            Self::HttpPort => ("org.gnome.system.proxy.http", "port", "i"),
            Self::HttpUseAuthentication => {
                ("org.gnome.system.proxy.http", "use-authentication", "b")
            }
            Self::HttpAuthenticationUser => {
                ("org.gnome.system.proxy.http", "authentication-user", "s")
            }
            Self::HttpAuthenticationPassword => (
                "org.gnome.system.proxy.http",
                "authentication-password",
                "s",
            ),
            Self::HttpsHost => ("org.gnome.system.proxy.https", "host", "s"),
            Self::HttpsPort => ("org.gnome.system.proxy.https", "port", "i"),
            Self::FtpHost => ("org.gnome.system.proxy.ftp", "host", "s"),
            Self::FtpPort => ("org.gnome.system.proxy.ftp", "port", "i"),
            Self::SocksHost => ("org.gnome.system.proxy.socks", "host", "s"),
            Self::SocksPort => ("org.gnome.system.proxy.socks", "port", "i"),
        }
    }

    fn accepts(self, value: &DesktopValue) -> bool {
        match (self, value) {
            (Self::Mode, DesktopValue::String(value)) => {
                matches!(value.as_str(), "none" | "manual" | "auto")
            }
            (_, DesktopValue::String(value)) => {
                self.schema_key_type().2 == "s" && valid_string(value)
            }
            (_, DesktopValue::Bool(_)) => self.schema_key_type().2 == "b",
            (_, DesktopValue::Int(value)) => {
                self.schema_key_type().2 == "i" && (0..=65535).contains(value)
            }
            (Self::IgnoreHosts, DesktopValue::Strings(values)) => {
                values.len() <= MAX_BYPASS_ENTRIES && values.iter().all(|v| valid_string(v))
            }
            _ => false,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DesktopValue {
    String(String),
    Bool(bool),
    Int(i32),
    Strings(Vec<String>),
}

/// Explicit absence is a required tagged field, never a missing JSON property.
/// An explicit value equal to the default remains an override.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Override {
    Absent,
    Present(DesktopValue),
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopEntry {
    pub key: DesktopKey,
    pub effective: DesktopValue,
    pub default: DesktopValue,
    pub user: Override,
    pub writable: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DesktopSnapshot(Vec<DesktopEntry>);

impl DesktopSnapshot {
    pub fn capture(mut entries: Vec<DesktopEntry>) -> Result<Self, Error> {
        if entries.len() != DesktopKey::ALL.len() {
            return Err(Error::IncompleteSnapshot);
        }
        entries.sort_by_key(|entry| entry.key);
        for (entry, key) in entries.iter().zip(DesktopKey::ALL) {
            if entry.key != key {
                return Err(Error::IncompleteSnapshot);
            }
            if !key.accepts(&entry.effective)
                || !key.accepts(&entry.default)
                || matches!(&entry.user, Override::Present(value) if !key.accepts(value))
            {
                return Err(Error::InvalidSnapshot);
            }
        }
        let snapshot = Self(entries);
        snapshot.encode()?;
        Ok(snapshot)
    }

    /// Private values for the fixed host adapter only; never diagnostics.
    pub fn entries(&self) -> &[DesktopEntry] {
        &self.0
    }

    /// Refuse known locks/inconsistent layered reads before a future write.
    /// This does not establish schema availability, temporal freshness, or CAS.
    pub fn admit_writes(&self) -> Result<(), Error> {
        for entry in &self.0 {
            if !entry.writable {
                return Err(Error::NotWritable);
            }
            let expected = match &entry.user {
                Override::Absent => &entry.default,
                Override::Present(value) => value,
            };
            if &entry.effective != expected {
                return Err(Error::InconsistentReadback);
            }
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Snapshot, Error> {
        encode(&Document::Desktop {
            version: 1,
            entries: self.0.clone(),
        })
    }

    pub fn decode(snapshot: &Snapshot) -> Result<Self, Error> {
        match decode(snapshot)? {
            Document::Desktop {
                version: 1,
                entries,
            } => Self::capture(entries),
            _ => Err(Error::InvalidSnapshot),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EnvironmentKey {
    #[serde(rename = "http_proxy")]
    Http,
    #[serde(rename = "HTTP_PROXY")]
    HttpUpper,
    #[serde(rename = "https_proxy")]
    Https,
    #[serde(rename = "HTTPS_PROXY")]
    HttpsUpper,
    #[serde(rename = "ftp_proxy")]
    Ftp,
    #[serde(rename = "FTP_PROXY")]
    FtpUpper,
    #[serde(rename = "all_proxy")]
    All,
    #[serde(rename = "ALL_PROXY")]
    AllUpper,
    #[serde(rename = "no_proxy")]
    No,
    #[serde(rename = "NO_PROXY")]
    NoUpper,
}

impl EnvironmentKey {
    pub const ALL: [Self; 10] = [
        Self::Http,
        Self::HttpUpper,
        Self::Https,
        Self::HttpsUpper,
        Self::Ftp,
        Self::FtpUpper,
        Self::All,
        Self::AllUpper,
        Self::No,
        Self::NoUpper,
    ];
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EnvironmentValue {
    Absent,
    Present(String),
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentEntry {
    pub key: EnvironmentKey,
    pub value: EnvironmentValue,
}

/// Caller-supplied classification, never a discovered fact or capability token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationEnvironment {
    SystemdBroker,
    SeparateDbusDaemon,
    Unknown,
}

/// Fixed model projection. When captured from Manager.Environment, values are
/// EFFECTIVE: they do not identify original mutable client-layer overrides.
/// Presence/empty tagging preserves this projection, not layer provenance or
/// authority to restore the installed manager's exact original state.
#[derive(Clone, PartialEq, Eq)]
pub struct EnvironmentSnapshot(Vec<EnvironmentEntry>);

impl EnvironmentSnapshot {
    pub fn capture(mut entries: Vec<EnvironmentEntry>) -> Result<Self, Error> {
        if entries.len() != EnvironmentKey::ALL.len() {
            return Err(Error::IncompleteSnapshot);
        }
        entries.sort_by_key(|entry| entry.key);
        for (entry, key) in entries.iter().zip(EnvironmentKey::ALL) {
            if entry.key != key {
                return Err(Error::IncompleteSnapshot);
            }
            if matches!(&entry.value, EnvironmentValue::Present(value) if !valid_string(value)) {
                return Err(Error::InvalidSnapshot);
            }
        }
        let snapshot = Self(entries);
        snapshot.encode()?;
        Ok(snapshot)
    }

    pub fn entries(&self) -> &[EnvironmentEntry] {
        &self.0
    }

    /// Model activation-class validation only. The caller-supplied enum does
    /// not establish actual manager capture permission, layer provenance or
    /// host write authority; the normal observer's write admission refuses.
    pub fn admit_writes(&self, activation: ActivationEnvironment) -> Result<(), Error> {
        match activation {
            ActivationEnvironment::SystemdBroker => Ok(()),
            _ => Err(Error::UnsupportedActivation),
        }
    }

    pub fn encode(&self) -> Result<Snapshot, Error> {
        encode(&Document::UserManager {
            version: 1,
            entries: self.0.clone(),
        })
    }

    pub fn decode(snapshot: &Snapshot) -> Result<Self, Error> {
        match decode(snapshot)? {
            Document::UserManager {
                version: 1,
                entries,
            } => Self::capture(entries),
            _ => Err(Error::InvalidSnapshot),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "surface", rename_all = "snake_case", deny_unknown_fields)]
enum Document {
    Desktop {
        version: u8,
        entries: Vec<DesktopEntry>,
    },
    UserManager {
        version: u8,
        entries: Vec<EnvironmentEntry>,
    },
}

fn valid_string(value: &str) -> bool {
    value.len() <= MAX_STRING && !value.contains('\0')
}

fn encode(document: &Document) -> Result<Snapshot, Error> {
    let bytes = serde_json::to_vec(document).map_err(|_| Error::InvalidSnapshot)?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::InvalidSnapshot);
    }
    Snapshot::new(Some(bytes)).map_err(|_| Error::InvalidSnapshot)
}

fn decode(snapshot: &Snapshot) -> Result<Document, Error> {
    let bytes = snapshot.bytes().ok_or(Error::IncompleteSnapshot)?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::InvalidSnapshot);
    }
    serde_json::from_slice(bytes).map_err(|_| Error::InvalidSnapshot)
}

macro_rules! private_debug {
    ($($ty:ty),+ $(,)?) => {
        $(impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($ty), "([private])"))
            }
        })+
    };
}

private_debug!(
    DesktopValue,
    Override,
    DesktopEntry,
    DesktopSnapshot,
    EnvironmentValue,
    EnvironmentEntry,
    EnvironmentSnapshot
);

#[cfg(test)]
mod tests;
