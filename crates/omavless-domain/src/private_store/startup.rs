// SPDX-License-Identifier: MIT
//! Login preferences are separate from durable current connection intent.
use super::*;

/// Private configuration intent; never include opaque selection in logs.
pub struct StartupPreferences {
    pub enabled: bool,
    pub target: String,
    pub profile_id: String,
    pub mode: String,
}

impl StartupPreferences {
    pub fn validate(&self) -> Result<(), PrivateStoreError> {
        if !matches!(self.target.as_str(), "last" | "profile")
            || !matches!(self.mode.as_str(), "rule" | "global")
            || (!self.profile_id.is_empty() && !valid_record_id(&self.profile_id))
            || (self.target == "last" && !self.profile_id.is_empty())
        {
            return Err(PrivateStoreError::InvalidShape);
        }
        Ok(())
    }
}

impl PrivateStore {
    /// Explicit private settings response; never infer old unit enablement.
    #[must_use]
    pub fn startup_preferences(&self) -> StartupPreferences {
        StartupPreferences {
            enabled: self.startup.enabled,
            target: self.startup.target.clone(),
            profile_id: self.startup.profile_id.clone(),
            mode: self.startup.mode.clone(),
        }
    }

    #[must_use]
    pub fn startup_is_configured(&self) -> bool {
        self.startup_configured
    }

    pub fn resolve_startup_selection(
        &self,
        preferences: &StartupPreferences,
    ) -> Result<String, PrivateStoreError> {
        preferences.validate()?;
        if preferences.mode == "rule" && self.routing_preset.is_empty() {
            return Err(PrivateStoreError::InvalidShape);
        }
        let selected = if preferences.target == "last" {
            if self.last_id.is_empty() {
                self.profiles
                    .first()
                    .map(|profile| profile.id.as_str())
                    .unwrap_or("")
            } else {
                &self.last_id
            }
        } else {
            &preferences.profile_id
        };
        self.profiles
            .iter()
            .find(|profile| profile.id == selected)
            .map(|profile| profile.id.clone())
            .ok_or(PrivateStoreError::ProfileNotFound)
    }
}

pub fn apply_startup_preferences(
    input: &str,
    preferences: &StartupPreferences,
) -> Result<(Vec<u8>, bool), PrivateStoreError> {
    preferences.validate()?;
    let mut store = parse_private_store(input)?;
    if preferences.enabled {
        store.resolve_startup_selection(preferences)?;
    }
    let previous = store.document.clone();
    store.document["startupConfigured"] = Value::Bool(true);
    store.document["startup"] = serde_json::json!({"enabled":preferences.enabled,
        "target":preferences.target,"profileId":preferences.profile_id,"mode":preferences.mode});
    // Revalidate complete candidate and normalize disabled stale selections.
    let result = parse_private_store(&store.document.to_string())?;
    let changed = result.document != previous;
    let mut payload =
        serde_json::to_vec(&result.document).map_err(|_| PrivateStoreError::InvalidJson)?;
    payload.push(b'\n');
    Ok((payload, changed))
}
