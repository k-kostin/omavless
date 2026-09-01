// SPDX-License-Identifier: MIT

//! Credential-free store relationship and migration semantics.
//!
//! Protocol adapters validate private names, URIs and subscription URLs before
//! this boundary. This module owns only record relationships, convenience
//! pointers, startup intent, routing-preset defaults and boolean state. It
//! performs no I/O and never needs reusable connection data.

use std::collections::BTreeSet;
use std::fmt;

pub const CURRENT_STORE_VERSION: u8 = 3;
pub const MAX_PROFILES: usize = 256;
pub const MAX_SUBSCRIPTIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    UnsupportedVersion,
    TooManyProfiles,
    TooManySubscriptions,
    InvalidProfileId,
    DuplicateProfileId,
    InvalidSubscriptionId,
    DuplicateSubscriptionId,
    IncompleteSubscriptionMetadata,
    UnknownSubscription,
    InvalidSubscriptionKey,
    InvalidRoutingPreset,
    InvalidStartupTarget,
    InvalidStartupMode,
}

impl StoreError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::TooManyProfiles => "too_many_profiles",
            Self::TooManySubscriptions => "too_many_subscriptions",
            Self::InvalidProfileId => "invalid_profile_id",
            Self::DuplicateProfileId => "duplicate_profile_id",
            Self::InvalidSubscriptionId => "invalid_subscription_id",
            Self::DuplicateSubscriptionId => "duplicate_subscription_id",
            Self::IncompleteSubscriptionMetadata => "incomplete_subscription_metadata",
            Self::UnknownSubscription => "unknown_subscription",
            Self::InvalidSubscriptionKey => "invalid_subscription_key",
            Self::InvalidRoutingPreset => "invalid_routing_preset",
            Self::InvalidStartupTarget => "invalid_startup_target",
            Self::InvalidStartupMode => "invalid_startup_mode",
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "Profile store version is unsupported",
            Self::TooManyProfiles => "Profile store has too many profiles",
            Self::TooManySubscriptions => "Profile store has too many subscriptions",
            Self::InvalidProfileId => "Profile record ID is invalid",
            Self::DuplicateProfileId => "Profile record ID is duplicated",
            Self::InvalidSubscriptionId => "Subscription record ID is invalid",
            Self::DuplicateSubscriptionId => "Subscription record ID is duplicated",
            Self::IncompleteSubscriptionMetadata => "Profile subscription metadata is incomplete",
            Self::UnknownSubscription => "Profile references an unknown subscription",
            Self::InvalidSubscriptionKey => "Profile subscription key is invalid",
            Self::InvalidRoutingPreset => "Routing preset is invalid",
            Self::InvalidStartupTarget => "Startup target is invalid",
            Self::InvalidStartupMode => "Startup routing mode is invalid",
        })
    }
}

impl std::error::Error for StoreError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileState {
    pub id: String,
    pub subscription_id: String,
    pub subscription_key: String,
    pub missing: bool,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionState {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupState {
    pub enabled: bool,
    pub target: String,
    pub profile_id: String,
    pub mode: String,
}

impl Default for StartupState {
    fn default() -> Self {
        Self {
            enabled: false,
            target: "last".to_owned(),
            profile_id: String::new(),
            mode: "rule".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreStateInput {
    pub version: u8,
    pub profiles: Vec<ProfileState>,
    pub subscriptions: Vec<SubscriptionState>,
    pub active_id: String,
    pub last_id: String,
    /// `None` represents a pre-routing-preset store.
    pub routing_preset: Option<String>,
    /// `None` represents a store written before startup settings existed.
    pub startup: Option<StartupState>,
    pub startup_configured: Option<bool>,
    pub onboarding_complete: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedStoreState {
    pub version: u8,
    pub profiles: Vec<ProfileState>,
    pub subscriptions: Vec<SubscriptionState>,
    pub active_id: String,
    pub last_id: String,
    pub routing_preset: String,
    pub startup: StartupState,
    pub startup_configured: bool,
    pub onboarding_complete: bool,
}

fn python_uuid_compatible(value: &str) -> bool {
    let value = value.strip_prefix("urn:uuid:").unwrap_or(value);
    let value = if value.starts_with('{') {
        let Some(inner) = value
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        else {
            return false;
        };
        inner
    } else {
        value
    };
    let mut digits = 0_usize;
    for byte in value.bytes() {
        if byte == b'-' {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
        digits += 1;
    }
    digits == 32
}

fn valid_subscription_key(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn normalize_store_state(
    mut input: StoreStateInput,
) -> Result<NormalizedStoreState, StoreError> {
    if !(1..=CURRENT_STORE_VERSION).contains(&input.version) {
        return Err(StoreError::UnsupportedVersion);
    }
    if input.profiles.len() > MAX_PROFILES {
        return Err(StoreError::TooManyProfiles);
    }
    if input.subscriptions.len() > MAX_SUBSCRIPTIONS {
        return Err(StoreError::TooManySubscriptions);
    }
    let mut subscription_ids = BTreeSet::new();
    for subscription in &input.subscriptions {
        if !python_uuid_compatible(&subscription.id) {
            return Err(StoreError::InvalidSubscriptionId);
        }
        if !subscription_ids.insert(subscription.id.as_str()) {
            return Err(StoreError::DuplicateSubscriptionId);
        }
    }
    let mut profile_ids = BTreeSet::new();
    for profile in &input.profiles {
        if !python_uuid_compatible(&profile.id) {
            return Err(StoreError::InvalidProfileId);
        }
        if !profile_ids.insert(profile.id.as_str()) {
            return Err(StoreError::DuplicateProfileId);
        }
        if profile.subscription_id.is_empty() != profile.subscription_key.is_empty() {
            return Err(StoreError::IncompleteSubscriptionMetadata);
        }
        if !profile.subscription_id.is_empty()
            && !subscription_ids.contains(profile.subscription_id.as_str())
        {
            return Err(StoreError::UnknownSubscription);
        }
        if !profile.subscription_key.is_empty()
            && !valid_subscription_key(&profile.subscription_key)
        {
            return Err(StoreError::InvalidSubscriptionKey);
        }
    }
    if !input.active_id.is_empty() && !profile_ids.contains(input.active_id.as_str()) {
        input.active_id.clear();
    }
    if !input.last_id.is_empty() && !profile_ids.contains(input.last_id.as_str()) {
        input.last_id.clear();
    }
    let routing_preset = input.routing_preset.unwrap_or_else(|| {
        if input.profiles.is_empty() {
            String::new()
        } else {
            "roscomvpn-default".to_owned()
        }
    });
    if !matches!(
        routing_preset.as_str(),
        "" | "roscomvpn-default" | "china-cn-direct" | "iran-ir-direct" | "custom"
    ) {
        return Err(StoreError::InvalidRoutingPreset);
    }
    let startup_was_missing = input.startup.is_none();
    let mut startup = input.startup.unwrap_or_default();
    if !matches!(startup.target.as_str(), "last" | "profile") {
        return Err(StoreError::InvalidStartupTarget);
    }
    if !matches!(startup.mode.as_str(), "rule" | "global") {
        return Err(StoreError::InvalidStartupMode);
    }
    if !startup.profile_id.is_empty() && !profile_ids.contains(startup.profile_id.as_str()) {
        startup.enabled = false;
        startup.profile_id.clear();
    }
    if startup.target == "profile" && startup.profile_id.is_empty() {
        startup.enabled = false;
    }
    if startup.target == "last" && input.profiles.is_empty() {
        startup.enabled = false;
    }
    let startup_configured = input.startup_configured.unwrap_or(!startup_was_missing);
    let onboarding_complete = input
        .onboarding_complete
        .unwrap_or(!input.profiles.is_empty());
    Ok(NormalizedStoreState {
        version: CURRENT_STORE_VERSION,
        profiles: input.profiles,
        subscriptions: input.subscriptions,
        active_id: input.active_id,
        last_id: input.last_id,
        routing_preset,
        startup,
        startup_configured,
        onboarding_complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "00000000-0000-0000-0000-000000000001";
    const SUBSCRIPTION: &str = "10000000-0000-0000-0000-000000000001";

    fn input() -> StoreStateInput {
        StoreStateInput {
            version: 3,
            profiles: vec![ProfileState {
                id: PROFILE.to_owned(),
                subscription_id: SUBSCRIPTION.to_owned(),
                subscription_key: "a".repeat(64),
                missing: false,
                favorite: true,
            }],
            subscriptions: vec![SubscriptionState {
                id: SUBSCRIPTION.to_owned(),
            }],
            active_id: PROFILE.to_owned(),
            last_id: PROFILE.to_owned(),
            routing_preset: Some("custom".to_owned()),
            startup: Some(StartupState {
                enabled: true,
                target: "profile".to_owned(),
                profile_id: PROFILE.to_owned(),
                mode: "global".to_owned(),
            }),
            startup_configured: Some(true),
            onboarding_complete: Some(true),
        }
    }

    #[test]
    fn preserves_valid_relationships_and_private_free_state() {
        let normalized = normalize_store_state(input()).expect("valid state");
        assert_eq!(normalized.version, 3);
        assert_eq!(normalized.active_id, PROFILE);
        assert!(normalized.profiles[0].favorite);
        assert!(normalized.startup.enabled);
    }

    #[test]
    fn legacy_defaults_and_stale_pointers_match_current_semantics() {
        let mut legacy = input();
        legacy.version = 1;
        legacy.routing_preset = None;
        legacy.startup = None;
        legacy.startup_configured = None;
        legacy.onboarding_complete = None;
        legacy.active_id = "20000000-0000-0000-0000-000000000001".to_owned();
        let normalized = normalize_store_state(legacy).expect("legacy state");
        assert_eq!(normalized.active_id, "");
        assert_eq!(normalized.routing_preset, "roscomvpn-default");
        assert!(!normalized.startup_configured);
        assert!(normalized.onboarding_complete);
    }

    #[test]
    fn relationship_errors_are_fixed_and_safe() {
        let mut broken = input();
        broken.profiles[0].subscription_key = "private-key".to_owned();
        let error = normalize_store_state(broken).expect_err("invalid key");
        assert_eq!(error, StoreError::InvalidSubscriptionKey);
        assert!(!error.to_string().contains("private-key"));
    }
}
