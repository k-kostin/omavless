// SPDX-License-Identifier: MIT

//! The single typed credential model shared by private store operations.

use super::*;
use omavless_profile::import::{ImportFamily, ImportedProfile, parse_import};
use omavless_profile::wireguard::WireGuardProfile;
use omavless_profile::wireguard::private_record::parse_private_wireguard_record;

/// Explicit private material. Only redacted Debug; no display or serialization.
pub enum PrivateProfileCredential {
    Uri {
        original_uri: String,
        canonical: CanonicalProfile,
    },
    WireGuard(WireGuardProfile),
}

impl fmt::Debug for PrivateProfileCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateProfileCredential")
            .field("family", &self.family())
            .finish_non_exhaustive()
    }
}

impl PrivateProfileCredential {
    pub fn parse_input(input: &str) -> Result<Self, PrivateStoreError> {
        if input.len() > crate::import::MAX_IMPORT_BYTES {
            return Err(PrivateStoreError::TooLarge);
        }
        if matches!(
            crate::import::classify_import(input, &[]),
            Ok(crate::import::ImportKind::Profile(_))
        ) {
            return Ok(Self::Uri {
                original_uri: input.trim().into(),
                canonical: parse_canonical(input.trim()).map_err(PrivateStoreError::Profile)?,
            });
        }
        match parse_import(input).map_err(|_| PrivateStoreError::InvalidShape)? {
            ImportedProfile::Uri(_) => Err(PrivateStoreError::InvalidShape),
            ImportedProfile::WireGuard(profile) => {
                profile
                    .private_record()
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                profile
                    .private_config()
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                Ok(Self::WireGuard(profile))
            }
        }
    }

    pub(super) fn parse_row(
        row: &Map<String, Value>,
        version: u8,
    ) -> Result<Self, PrivateStoreError> {
        if version < 4 {
            // Legacy URI-only compatibility keeps opaque extensions exactly as
            // before. Promotion to v4 still revalidates the complete graph.
            let uri = required_string(row, "uri")?;
            let canonical = parse_canonical(uri).map_err(PrivateStoreError::Profile)?;
            let stored = match row.get("protocol") {
                Some(value) => value.as_str().ok_or(PrivateStoreError::InvalidShape)?,
                None if version < 3 => canonical.protocol().as_str(),
                None => return Err(PrivateStoreError::InvalidShape),
            };
            if protocol(stored) != Some(canonical.protocol()) {
                return Err(PrivateStoreError::ProtocolMismatch);
            }
            return Ok(Self::Uri {
                original_uri: uri.into(),
                canonical,
            });
        }
        match (row.get("uri"), row.get("wireguard")) {
            (Some(uri), None) => {
                let uri = uri.as_str().ok_or(PrivateStoreError::InvalidShape)?;
                let canonical = parse_canonical(uri).map_err(PrivateStoreError::Profile)?;
                let stored = match row.get("protocol") {
                    Some(value) => value.as_str().ok_or(PrivateStoreError::InvalidShape)?,
                    None if version < 3 => canonical.protocol().as_str(),
                    None => return Err(PrivateStoreError::InvalidShape),
                };
                if protocol(stored) != Some(canonical.protocol()) {
                    return Err(PrivateStoreError::ProtocolMismatch);
                }
                Ok(Self::Uri {
                    original_uri: uri.into(),
                    canonical,
                })
            }
            (None, Some(record)) if version == 4 => {
                if row.keys().any(|key| {
                    !["id", "name", "protocol", "wireguard", "favorite"].contains(&key.as_str())
                }) {
                    return Err(PrivateStoreError::InvalidShape);
                }
                let profile = parse_private_wireguard_record(record.to_string().as_bytes())
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                if required_string(row, "protocol")? != profile.facts().flavor.protocol_name() {
                    return Err(PrivateStoreError::ProtocolMismatch);
                }
                profile
                    .private_record()
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                profile
                    .private_config()
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                Ok(Self::WireGuard(profile))
            }
            _ => Err(PrivateStoreError::InvalidShape),
        }
    }

    #[must_use]
    pub fn family(&self) -> ImportFamily {
        match self {
            Self::Uri { canonical, .. } => match canonical.protocol() {
                Protocol::Vless => ImportFamily::Vless,
                Protocol::Trojan => ImportFamily::Trojan,
                Protocol::Hysteria2 => ImportFamily::Hysteria2,
                Protocol::Tuic => ImportFamily::Tuic,
            },
            Self::WireGuard(profile) => {
                if profile.facts().flavor.protocol_name() == "wireguard" {
                    ImportFamily::WireGuard
                } else {
                    ImportFamily::AmneziaWg
                }
            }
        }
    }

    pub(super) fn write_fields(
        &self,
        row: &mut Map<String, Value>,
    ) -> Result<(), PrivateStoreError> {
        row.insert("protocol".into(), self.family().as_str().into());
        match self {
            Self::Uri { original_uri, .. } => {
                row.insert("uri".into(), original_uri.clone().into());
            }
            Self::WireGuard(profile) => {
                row.remove("uri");
                let record = profile
                    .private_record()
                    .map_err(|_| PrivateStoreError::InvalidShape)?;
                row.insert(
                    "wireguard".into(),
                    serde_json::from_slice(record.expose_private_bytes())
                        .map_err(|_| PrivateStoreError::InvalidShape)?,
                );
            }
        }
        Ok(())
    }

    pub fn private_export(&self) -> Result<CandidateProfileExport, PrivateStoreError> {
        match self {
            Self::Uri { original_uri, .. } => Ok(CandidateProfileExport {
                format: CandidateExportFormat::Uri,
                bytes: original_uri.as_bytes().to_vec(),
            }),
            Self::WireGuard(profile) => Ok(CandidateProfileExport {
                format: CandidateExportFormat::WireGuardConfig,
                bytes: profile
                    .private_config()
                    .map_err(|_| PrivateStoreError::InvalidShape)?
                    .expose_private_bytes()
                    .to_vec(),
            }),
        }
    }

    pub(super) fn private_input(&self) -> Result<String, PrivateStoreError> {
        String::from_utf8(self.private_export()?.bytes).map_err(|_| PrivateStoreError::InvalidShape)
    }

    pub fn subscription_identity(&self) -> String {
        match self {
            Self::Uri { canonical, .. } => canonical.subscription_identity(),
            Self::WireGuard(profile) => profile.subscription_identity(),
        }
    }

    pub fn render_mihomo_proxy(
        &self,
        name: &str,
        server_override: Option<&str>,
    ) -> Result<String, PrivateStoreError> {
        match self {
            Self::Uri { canonical, .. } => Ok(canonical.render_mihomo_proxy(name, server_override)),
            Self::WireGuard(profile) => profile
                .render_mihomo_proxy(name, server_override)
                .map_err(|_| PrivateStoreError::TypedCredentialRequired),
        }
    }

    /// Allowlisted private UI metadata, never the complete credential record.
    pub fn private_preview(&self) -> Value {
        match self {
            Self::Uri { canonical, .. } => canonical.private_preview(),
            Self::WireGuard(profile) => {
                // Validated native values are the source of truth; no second protocol parser.
                let record = profile.private_record().expect("validated WG credential");
                let value: Value = serde_json::from_slice(record.expose_private_bytes())
                    .expect("canonical WG record");
                let endpoint = value["peer"]["endpoint"]
                    .as_str()
                    .expect("required validated endpoint");
                let (host, port) = endpoint.rsplit_once(':').expect("validated endpoint port");
                let name = if self.family() == ImportFamily::WireGuard {
                    "WireGuard"
                } else {
                    "AmneziaWG"
                };
                serde_json::json!({"version":1,"protocol":self.family().as_str(),"server":host,"port":port.parse::<u16>().expect("validated endpoint port"),"transport":"udp","security":"wireguard","sni":"","flow":"","insecure":false,"advancedXhttp":false,"experimental":true,"experimentalFeatures":[name],"compatibilityNote":"","credentialHint":"••••","suggestedName":name})
            }
        }
    }

    pub(super) fn private_diagnostic_model(&self) -> Value {
        match self {
            Self::Uri { canonical, .. } => canonical.private_diagnostic_model(),
            Self::WireGuard(profile) => {
                let record = profile.private_record().expect("validated WG credential");
                serde_json::from_slice(record.expose_private_bytes()).expect("canonical WG record")
            }
        }
    }

    pub(super) fn native_private_fragments(&self) -> Vec<String> {
        let Self::WireGuard(_) = self else {
            return Vec::new();
        };
        let mut fragments = Vec::new();
        let model = self.private_diagnostic_model();
        for section in ["interface", "peer"] {
            if let Some(fields) = model[section].as_object() {
                for value in fields.values().filter_map(Value::as_str) {
                    fragments.push(value.to_owned());
                    fragments.extend(value.split(',').map(str::trim).map(str::to_owned));
                }
            }
        }
        let preview = self.private_preview();
        if let Some(host) = preview["server"].as_str() {
            fragments.push(host.to_owned());
            fragments.push(host.trim_matches(['[', ']']).to_owned());
        }
        fragments
    }

    pub(super) fn into_uri(self) -> Result<CanonicalProfile, PrivateStoreError> {
        match self {
            Self::Uri { canonical, .. } => Ok(canonical),
            Self::WireGuard(_) => Err(PrivateStoreError::TypedCredentialRequired),
        }
    }
}
