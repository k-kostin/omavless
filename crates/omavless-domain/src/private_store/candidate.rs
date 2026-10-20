// SPDX-License-Identifier: MIT

//! Compatibility names for the former pure candidate API. Validation and every
//! mutation now delegate to the single typed PrivateStore engine. These APIs
//! remain pure byte preparation, not owner publication or feature activation.

use super::*;
use omavless_profile::wireguard::WireGuardProfile;

pub enum CandidateProfileInput {
    Uri(String),
    WireGuard(WireGuardProfile),
}
impl CandidateProfileInput {
    fn private_input(self) -> Result<String, PrivateStoreError> {
        match self {
            Self::Uri(input) => Ok(input),
            Self::WireGuard(profile) => {
                PrivateProfileCredential::WireGuard(profile).private_input()
            }
        }
    }
}

/// Historical API wrapper; never a second store, parser or mutation engine.
pub struct CandidatePrivateStore(PrivateStore);
impl CandidatePrivateStore {
    pub fn selected_wireguard_flavor(
        &self,
        id: &str,
    ) -> Result<Option<omavless_profile::wireguard::WireGuardFlavor>, PrivateStoreError> {
        self.0.selected_wireguard_flavor(id)
    }
    pub fn prepare_private_config_mode(
        &self,
        id: &str,
        template: &str,
        socket: &str,
        mode: &str,
    ) -> Result<CandidateRuntimeConfig, PrivateStoreError> {
        let profile = self
            .0
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or(PrivateStoreError::ProfileNotFound)?;
        Ok(CandidateRuntimeConfig {
            config: self.0.prepare_config_mode(id, template, socket, mode)?,
            name: profile.name.clone(),
        })
    }
    pub fn with_profile(
        self,
        id: &str,
        name: &str,
        input: CandidateProfileInput,
    ) -> Result<Self, PrivateStoreError> {
        let payload = apply_profile_import(
            &self.0.document.to_string(),
            id,
            name,
            &input.private_input()?,
        )?;
        parse_candidate_private_store(
            std::str::from_utf8(payload.payload()).map_err(|_| PrivateStoreError::InvalidJson)?,
        )
    }
    pub fn with_wireguard(
        self,
        id: &str,
        name: &str,
        profile: WireGuardProfile,
    ) -> Result<Self, PrivateStoreError> {
        self.with_profile(id, name, CandidateProfileInput::WireGuard(profile))
    }
    fn mutate(self, mutation: ProfileMutation) -> Result<(Self, bool), PrivateStoreError> {
        let result = apply_profile_mutation(&self.0.document.to_string(), mutation)?;
        Ok((
            parse_candidate_private_store(
                std::str::from_utf8(result.payload())
                    .map_err(|_| PrivateStoreError::InvalidJson)?,
            )?,
            result.changed,
        ))
    }
    pub fn replace_standalone(
        self,
        id: &str,
        name: &str,
        input: CandidateProfileInput,
    ) -> Result<(Self, bool), PrivateStoreError> {
        self.mutate(ProfileMutation::Replace {
            profile_id: id.into(),
            new_name: name.into(),
            new_input: input.private_input()?,
        })
    }
    pub fn delete_standalone(self, id: &str) -> Result<Self, PrivateStoreError> {
        self.mutate(ProfileMutation::Delete {
            profile_id: id.into(),
        })
        .map(|(store, _)| store)
    }
    pub fn rename_standalone(
        self,
        id: &str,
        name: &str,
    ) -> Result<(Self, bool), PrivateStoreError> {
        self.mutate(ProfileMutation::Rename {
            profile_id: id.into(),
            new_name: name.into(),
        })
    }
    pub fn set_favorite(self, id: &str, enabled: bool) -> Result<(Self, bool), PrivateStoreError> {
        self.mutate(ProfileMutation::Favorite {
            profile_id: id.into(),
            enabled,
        })
    }
    pub fn export_private_credential(
        &self,
        id: &str,
    ) -> Result<CandidateProfileExport, PrivateStoreError> {
        let profile = self
            .0
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or(PrivateStoreError::ProfileNotFound)?;
        match &profile.credential {
            PrivateProfileCredential::Uri { .. } => profile.credential.private_export(),
            PrivateProfileCredential::WireGuard(profile) => Ok(CandidateProfileExport {
                format: CandidateExportFormat::WireGuardRecord,
                bytes: profile
                    .private_record()
                    .map_err(|_| PrivateStoreError::InvalidShape)?
                    .expose_private_bytes()
                    .to_vec(),
            }),
        }
    }
    pub fn export_private_native_credential(
        &self,
        id: &str,
    ) -> Result<CandidateProfileExport, PrivateStoreError> {
        self.0.profile_credential_export(id)
    }
    pub fn private_edit_input(
        &self,
        id: &str,
    ) -> Result<CandidateProfileEditInput, PrivateStoreError> {
        let edit = self.0.profile_edit_input(id)?;
        Ok(CandidateProfileEditInput {
            name: edit.private_name().into(),
            credential: self.export_private_native_credential(id)?,
        })
    }
    #[must_use]
    pub fn profile_counts(&self) -> (usize, usize, usize) {
        let counts = self.0.projection();
        let wg = counts.wireguard_count + counts.amneziawg_count;
        (counts.profile_count - wg, wg, counts.subscription_count)
    }
    #[must_use]
    pub fn pointer_presence(&self) -> (bool, bool, bool) {
        (
            !self.0.active_id.is_empty(),
            !self.0.last_id.is_empty(),
            self.0.startup.enabled,
        )
    }
    #[must_use]
    pub fn references_active_profile(&self, id: &str) -> bool {
        !id.is_empty() && self.0.active_id == id
    }
    pub fn into_private_bytes(self) -> Result<Vec<u8>, PrivateStoreError> {
        self.0.private_payload()
    }
}

pub struct CandidateRuntimeConfig {
    config: String,
    name: String,
}
impl CandidateRuntimeConfig {
    pub fn expose_private_bytes(&self) -> &[u8] {
        self.config.as_bytes()
    }
    pub fn private_name(&self) -> &str {
        &self.name
    }
}
impl fmt::Debug for CandidateRuntimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CandidateRuntimeConfig([REDACTED])")
    }
}

pub fn apply_candidate_compatibility_pointer_update(
    input: &str,
    target: CompatibilityPointerTarget,
) -> Result<PrivatePointerMutation, PrivateStoreError> {
    parse_candidate_private_store(input)?;
    apply_compatibility_pointer_update(input, target)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateExportFormat {
    Uri,
    WireGuardRecord,
    WireGuardConfig,
}
pub struct CandidateProfileExport {
    pub(super) format: CandidateExportFormat,
    pub(super) bytes: Vec<u8>,
}
pub struct CandidateProfileEditInput {
    name: String,
    credential: CandidateProfileExport,
}
impl CandidateProfileEditInput {
    #[must_use]
    pub fn private_name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn private_credential(&self) -> &CandidateProfileExport {
        &self.credential
    }
}
impl CandidateProfileExport {
    #[must_use]
    pub const fn format(&self) -> CandidateExportFormat {
        self.format
    }
    #[must_use]
    pub fn expose_private_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn migrate_legacy_store_candidate(
    input: &str,
) -> Result<CandidatePrivateStore, PrivateStoreError> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(PrivateStoreError::TooLarge);
    }
    parse_unique_private_json(input).map_err(|_| PrivateStoreError::InvalidJson)?;
    let mut store = parse_private_store(input)?;
    if store.version == 4 {
        return Err(PrivateStoreError::Store(StoreError::UnsupportedVersion));
    }
    store.document["version"] = Value::from(4);
    parse_candidate_private_store(&store.document.to_string())
}
pub fn parse_candidate_private_store(
    input: &str,
) -> Result<CandidatePrivateStore, PrivateStoreError> {
    let store = parse_private_store(input)?;
    if store.version != 4 {
        return Err(PrivateStoreError::Store(StoreError::UnsupportedVersion));
    }
    Ok(CandidatePrivateStore(store))
}
