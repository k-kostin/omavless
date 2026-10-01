// SPDX-License-Identifier: MIT

//! Inactive versioned private storage codec. No filesystem, runtime, or IPC
//! integration is provided. These bytes contain reusable credentials; callers
//! must keep them in private storage and never send them to ordinary diagnostics.

use super::{IpPrefix, WireGuardProfile, parse_wireguard_config, unique_json};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_PRIVATE_RECORD_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateRecordError {
    TooLarge,
    InvalidRecord,
    UnsupportedVersion,
}

impl fmt::Display for PrivateRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Private WireGuard record exceeds its bound",
            Self::InvalidRecord => "Private WireGuard record is invalid",
            Self::UnsupportedVersion => "Private WireGuard record version is unsupported",
        })
    }
}

impl std::error::Error for PrivateRecordError {}

/// Deliberately not serializable, cloneable, or printable as credential bytes.
pub struct PrivateWireGuardRecord(Vec<u8>);

impl fmt::Debug for PrivateWireGuardRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateWireGuardRecord([REDACTED])")
    }
}

impl PrivateWireGuardRecord {
    /// Explicit private release for a future same-user persistence boundary.
    #[must_use]
    pub fn expose_private_bytes(&self) -> &[u8] {
        &self.0
    }
}

// Maps use exactly the native parser's lower-case field vocabulary. Storing
// source text, guest metadata, or a cached flavor would create competing truth.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    schema_version: u32,
    interface: BTreeMap<String, String>,
    peer: BTreeMap<String, String>,
}

/// Revalidates every stored value through the original strict config validator.
/// JSON errors are always collapsed so paths, fragments, and credentials cannot
/// escape through an error. Duplicate keys are rejected at every object level.
pub fn parse_private_wireguard_record(
    bytes: &[u8],
) -> Result<WireGuardProfile, PrivateRecordError> {
    if bytes.len() > MAX_PRIVATE_RECORD_BYTES {
        return Err(PrivateRecordError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| PrivateRecordError::InvalidRecord)?;
    let value = unique_json(text).map_err(|_| PrivateRecordError::InvalidRecord)?;
    let record: Record =
        serde_json::from_value(value).map_err(|_| PrivateRecordError::InvalidRecord)?;
    if record.schema_version != 1 {
        return Err(PrivateRecordError::UnsupportedVersion);
    }
    let mut config = String::new();
    for (section, fields, allowed) in [
        (
            "Interface",
            record.interface,
            super::INTERFACE_FIELDS.as_slice(),
        ),
        ("Peer", record.peer, super::PEER_FIELDS.as_slice()),
    ] {
        config.push_str(&format!("[{section}]\n"));
        for (name, value) in fields {
            // Check before constructing native lines: a stored string must not
            // inject a new field/section or exploit native whitespace aliases.
            if !allowed.contains(&name.as_str())
                || value.bytes().any(|byte| byte.is_ascii_control())
                || value.trim() != value
            {
                return Err(PrivateRecordError::InvalidRecord);
            }
            config.push_str(&format!("{name} = {value}\n"));
        }
    }
    parse_wireguard_config(&config).map_err(|_| PrivateRecordError::InvalidRecord)
}

impl WireGuardProfile {
    /// Encodes canonical credentials, never the original guest container or
    /// administrator metadata. This does not persist or activate the profile.
    pub fn private_record(&self) -> Result<PrivateWireGuardRecord, PrivateRecordError> {
        let mut interface = BTreeMap::from([
            ("privatekey".to_owned(), self.private_key.clone()),
            (
                "address".to_owned(),
                self.addresses
                    .iter()
                    .map(IpPrefix::canonical)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        ]);
        if !self.dns.is_empty() {
            interface.insert("dns".to_owned(), self.dns.join(","));
        }
        if let Some(mtu) = self.mtu {
            interface.insert("mtu".to_owned(), mtu.to_string());
        }
        if let Some(awg) = &self.awg {
            interface.extend(awg.values.clone());
        }
        let endpoint = if self.endpoint.host.contains(':') {
            format!("[{}]:{}", self.endpoint.host, self.endpoint.port)
        } else {
            format!("{}:{}", self.endpoint.host, self.endpoint.port)
        };
        let mut peer = BTreeMap::from([
            ("publickey".to_owned(), self.public_key.clone()),
            ("endpoint".to_owned(), endpoint),
            (
                "allowedips".to_owned(),
                self.allowed_ips
                    .iter()
                    .map(IpPrefix::canonical)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        ]);
        if !self.preshared_key.is_empty() {
            peer.insert("presharedkey".to_owned(), self.preshared_key.clone());
        }
        if let Some(keepalive) = &self.persistent_keepalive {
            peer.insert("persistentkeepalive".to_owned(), keepalive.canonical());
        }
        let bytes = serde_json::to_vec(&Record {
            schema_version: 1,
            interface,
            peer,
        })
        .map_err(|_| PrivateRecordError::InvalidRecord)?;
        // Canonical spelling can grow near a native line bound. Never publish
        // an envelope which the same strict loader cannot restore losslessly.
        let restored = parse_private_wireguard_record(&bytes)?;
        if restored.subscription_identity() != self.subscription_identity() {
            return Err(PrivateRecordError::InvalidRecord);
        }
        Ok(PrivateWireGuardRecord(bytes))
    }
}
