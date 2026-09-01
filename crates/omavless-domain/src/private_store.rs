// SPDX-License-Identifier: MIT

//! Read-only validation and projection of the current private Python store.
//!
//! Values in this module include reusable credentials. They deliberately do
//! not implement `Debug` or serialization. Callers may publish only
//! [`StoreProjection`], which contains counts and booleans.

use crate::config::{ConfigError, assemble_runtime_config};
use crate::import::valid_subscription_url;
use crate::routing::{CustomRule, RoutingError};
use crate::store::{
    CURRENT_STORE_VERSION, MAX_PROFILES, MAX_SUBSCRIPTIONS, ProfileState, StartupState, StoreError,
    StoreStateInput, SubscriptionState, normalize_store_state, valid_record_id,
};
use omavless_profile::Protocol;
use omavless_profile::canonical::{CanonicalError, CanonicalProfile, parse_canonical};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;

const MAX_NAME_CHARS: usize = 80;
pub const MAX_PRIVATE_STORE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateStoreError {
    InvalidJson,
    TooLarge,
    InvalidShape,
    InvalidName,
    InvalidSubscriptionUrl,
    DuplicateSubscriptionUrl,
    InvalidTimestamp,
    ProtocolMismatch,
    Store(StoreError),
    Profile(CanonicalError),
    Routing(RoutingError),
    Config(ConfigError),
    ProfileNotFound,
}

impl PrivateStoreError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::TooLarge => "too_large",
            Self::InvalidShape => "invalid_shape",
            Self::InvalidName => "invalid_name",
            Self::InvalidSubscriptionUrl => "invalid_subscription_url",
            Self::DuplicateSubscriptionUrl => "duplicate_subscription_url",
            Self::InvalidTimestamp => "invalid_timestamp",
            Self::ProtocolMismatch => "protocol_mismatch",
            Self::Store(error) => error.code(),
            Self::Profile(error) => error.code(),
            Self::Routing(error) => error.code(),
            Self::Config(_) => "invalid_runtime_config",
            Self::ProfileNotFound => "profile_not_found",
        }
    }
}

impl fmt::Display for PrivateStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidJson => "Private profile store is not valid JSON",
            Self::TooLarge => "Private profile store is too large",
            Self::InvalidShape => "Private profile store has an invalid format",
            Self::InvalidName => "Private profile store has an invalid record name",
            Self::InvalidSubscriptionUrl => "Private profile store has an invalid subscription URL",
            Self::DuplicateSubscriptionUrl => {
                "Private profile store has a duplicate subscription URL"
            }
            Self::InvalidTimestamp => "Private profile store has an invalid timestamp",
            Self::ProtocolMismatch => "Stored profile protocol does not match its link",
            Self::Store(error) => return error.fmt(formatter),
            Self::Profile(error) => return error.fmt(formatter),
            Self::Routing(error) => return error.fmt(formatter),
            Self::Config(error) => return error.fmt(formatter),
            Self::ProfileNotFound => "Requested profile record was not found",
        })
    }
}

impl std::error::Error for PrivateStoreError {}

struct PrivateProfile {
    id: String,
    name: String,
    canonical: CanonicalProfile,
}

struct PrivateSubscription {
    _name: String,
    _url: String,
}

/// Credential-free facts which may safely cross the future control boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreProjection {
    pub version: u8,
    pub profile_count: usize,
    pub subscription_count: usize,
    pub vless_count: usize,
    pub trojan_count: usize,
    pub hysteria2_count: usize,
    pub tuic_count: usize,
    pub active_present: bool,
    pub last_present: bool,
    pub routing_preset: String,
    pub custom_rule_count: usize,
    pub startup_configured: bool,
    pub onboarding_complete: bool,
}

/// Validated private store. Never derive `Debug`, `Clone`, or serialization.
pub struct PrivateStore {
    version: u8,
    profiles: Vec<PrivateProfile>,
    subscriptions: Vec<PrivateSubscription>,
    active_id: String,
    last_id: String,
    routing_preset: String,
    custom_rules: Vec<CustomRule>,
    _rules_updated_at: u64,
    startup_configured: bool,
    onboarding_complete: bool,
}

fn object(value: &Value) -> Result<&Map<String, Value>, PrivateStoreError> {
    value.as_object().ok_or(PrivateStoreError::InvalidShape)
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], PrivateStoreError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(PrivateStoreError::InvalidShape)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, PrivateStoreError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(PrivateStoreError::InvalidShape)
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    default: &'a str,
) -> Result<&'a str, PrivateStoreError> {
    object
        .get(key)
        .map(|value| value.as_str().ok_or(PrivateStoreError::InvalidShape))
        .unwrap_or(Ok(default))
}

fn optional_bool(
    object: &Map<String, Value>,
    key: &str,
    default: bool,
) -> Result<bool, PrivateStoreError> {
    object
        .get(key)
        .map(|value| value.as_bool().ok_or(PrivateStoreError::InvalidShape))
        .unwrap_or(Ok(default))
}

fn optional_u64(
    object: &Map<String, Value>,
    key: &str,
    default: u64,
) -> Result<u64, PrivateStoreError> {
    object
        .get(key)
        .map(|value| value.as_u64().ok_or(PrivateStoreError::InvalidTimestamp))
        .unwrap_or(Ok(default))
}

fn canonical_name(value: &str) -> bool {
    let cleaned = value
        .chars()
        .filter(|character| !matches!(*character as u32, 0..=31 | 127))
        .collect::<String>();
    let cleaned = cleaned.trim();
    !cleaned.is_empty() && cleaned.chars().count() <= MAX_NAME_CHARS && cleaned == value
}

fn protocol(value: &str) -> Option<Protocol> {
    match value {
        "vless" => Some(Protocol::Vless),
        "trojan" => Some(Protocol::Trojan),
        "hysteria2" => Some(Protocol::Hysteria2),
        "tuic" => Some(Protocol::Tuic),
        _ => None,
    }
}

/// Parse and fully validate one existing v1-v3 store without mutating it.
pub fn parse_private_store(input: &str) -> Result<PrivateStore, PrivateStoreError> {
    if input.len() > MAX_PRIVATE_STORE_BYTES {
        return Err(PrivateStoreError::TooLarge);
    }
    let value: Value = serde_json::from_str(input).map_err(|_| PrivateStoreError::InvalidJson)?;
    let root = object(&value)?;
    let version = root
        .get("version")
        .map(|value| value.as_u64().ok_or(PrivateStoreError::InvalidShape))
        .unwrap_or(Ok(1))?;
    let version = u8::try_from(version).map_err(|_| PrivateStoreError::InvalidShape)?;
    if !(1..=CURRENT_STORE_VERSION).contains(&version) {
        return Err(PrivateStoreError::Store(StoreError::UnsupportedVersion));
    }

    let subscription_values = root
        .get("subscriptions")
        .map(|value| {
            value
                .as_array()
                .map(Vec::as_slice)
                .ok_or(PrivateStoreError::InvalidShape)
        })
        .unwrap_or(Ok(&[]))?;
    if subscription_values.len() > MAX_SUBSCRIPTIONS {
        return Err(PrivateStoreError::Store(StoreError::TooManySubscriptions));
    }
    let mut subscription_states = Vec::with_capacity(subscription_values.len());
    let mut subscriptions = Vec::with_capacity(subscription_values.len());
    let mut subscription_urls = BTreeSet::new();
    for value in subscription_values {
        let record = object(value)?;
        let id = required_string(record, "id")?;
        let name = required_string(record, "name")?;
        let url = required_string(record, "url")?;
        if !canonical_name(name) {
            return Err(PrivateStoreError::InvalidName);
        }
        if !valid_subscription_url(url) {
            return Err(PrivateStoreError::InvalidSubscriptionUrl);
        }
        if !subscription_urls.insert(url) {
            return Err(PrivateStoreError::DuplicateSubscriptionUrl);
        }
        optional_u64(record, "updatedAt", 0)?;
        subscription_states.push(SubscriptionState { id: id.to_owned() });
        subscriptions.push(PrivateSubscription {
            _name: name.to_owned(),
            _url: url.to_owned(),
        });
    }

    let profile_values = array(root, "profiles")?;
    if profile_values.len() > MAX_PROFILES {
        return Err(PrivateStoreError::Store(StoreError::TooManyProfiles));
    }
    let mut profile_states = Vec::with_capacity(profile_values.len());
    let mut profiles = Vec::with_capacity(profile_values.len());
    for value in profile_values {
        let record = object(value)?;
        let id = required_string(record, "id")?;
        let name = required_string(record, "name")?;
        let uri = required_string(record, "uri")?;
        if !canonical_name(name) {
            return Err(PrivateStoreError::InvalidName);
        }
        let canonical = parse_canonical(uri).map_err(PrivateStoreError::Profile)?;
        let stored_protocol = match record.get("protocol") {
            Some(value) => value.as_str().ok_or(PrivateStoreError::InvalidShape)?,
            None if version < 3 => canonical.protocol().as_str(),
            None => return Err(PrivateStoreError::InvalidShape),
        };
        if protocol(stored_protocol) != Some(canonical.protocol()) {
            return Err(PrivateStoreError::ProtocolMismatch);
        }
        let subscription_id = optional_string(record, "subscriptionId", "")?;
        let subscription_key = optional_string(record, "subscriptionKey", "")?;
        let missing = optional_bool(record, "missing", false)?;
        let favorite = optional_bool(record, "favorite", false)?;
        profile_states.push(ProfileState {
            id: id.to_owned(),
            subscription_id: subscription_id.to_owned(),
            subscription_key: subscription_key.to_owned(),
            missing,
            favorite,
        });
        profiles.push(PrivateProfile {
            id: id.to_owned(),
            name: name.to_owned(),
            canonical,
        });
    }

    let mut custom_rules = Vec::new();
    let mut custom_rule_pairs = BTreeSet::new();
    if let Some(value) = root.get("customRules") {
        let values = value.as_array().ok_or(PrivateStoreError::InvalidShape)?;
        if values.len() > crate::routing::MAX_CUSTOM_RULES {
            return Err(PrivateStoreError::Routing(RoutingError::TooManyRules));
        }
        for value in values {
            let record = object(value)?;
            let id = required_string(record, "id")?;
            if !valid_record_id(id) {
                return Err(PrivateStoreError::InvalidShape);
            }
            let kind = required_string(record, "kind")?;
            let action = required_string(record, "action")?;
            let value = required_string(record, "value")?;
            let rule =
                CustomRule::parse(kind, action, value).map_err(PrivateStoreError::Routing)?;
            if rule.value != value {
                return Err(PrivateStoreError::InvalidShape);
            }
            if !custom_rule_pairs.insert((kind, value)) {
                return Err(PrivateStoreError::Routing(RoutingError::DuplicateRule));
            }
            custom_rules.push(rule);
        }
    }
    let rules_updated_at = optional_u64(root, "rulesUpdatedAt", 0)?;

    let startup = root
        .get("startup")
        .map(object)
        .transpose()?
        .map(|startup| {
            Ok(StartupState {
                enabled: startup
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .ok_or(PrivateStoreError::InvalidShape)?,
                target: required_string(startup, "target")?.to_owned(),
                profile_id: optional_string(startup, "profileId", "")?.to_owned(),
                mode: required_string(startup, "mode")?.to_owned(),
            })
        })
        .transpose()?;
    let startup_configured = root
        .get("startupConfigured")
        .map(|value| value.as_bool().ok_or(PrivateStoreError::InvalidShape))
        .transpose()?;
    let onboarding_complete = root
        .get("onboardingComplete")
        .map(|value| value.as_bool().ok_or(PrivateStoreError::InvalidShape))
        .transpose()?;
    let input = StoreStateInput {
        version,
        profiles: profile_states,
        subscriptions: subscription_states,
        active_id: optional_string(root, "activeId", "")?.to_owned(),
        last_id: optional_string(root, "lastId", "")?.to_owned(),
        routing_preset: root
            .get("routingPreset")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or(PrivateStoreError::InvalidShape)
            })
            .transpose()?,
        startup,
        startup_configured,
        onboarding_complete,
    };
    let normalized = normalize_store_state(input).map_err(PrivateStoreError::Store)?;
    Ok(PrivateStore {
        version: normalized.version,
        profiles,
        subscriptions,
        active_id: normalized.active_id,
        last_id: normalized.last_id,
        routing_preset: normalized.routing_preset,
        custom_rules,
        _rules_updated_at: rules_updated_at,
        startup_configured: normalized.startup_configured,
        onboarding_complete: normalized.onboarding_complete,
    })
}

impl PrivateStore {
    #[must_use]
    pub fn projection(&self) -> StoreProjection {
        let mut counts = [0_usize; 4];
        for profile in &self.profiles {
            let index = match profile.canonical.protocol() {
                Protocol::Vless => 0,
                Protocol::Trojan => 1,
                Protocol::Hysteria2 => 2,
                Protocol::Tuic => 3,
            };
            counts[index] += 1;
        }
        StoreProjection {
            version: self.version,
            profile_count: self.profiles.len(),
            subscription_count: self.subscriptions.len(),
            vless_count: counts[0],
            trojan_count: counts[1],
            hysteria2_count: counts[2],
            tuic_count: counts[3],
            active_present: !self.active_id.is_empty(),
            last_present: !self.last_id.is_empty(),
            routing_preset: self.routing_preset.clone(),
            custom_rule_count: self.custom_rules.len(),
            startup_configured: self.startup_configured,
            onboarding_complete: self.onboarding_complete,
        }
    }

    /// Render one internal profile into a complete private Mihomo config.
    pub fn prepare_config(
        &self,
        profile_id: &str,
        template: &str,
        controller_socket: &str,
    ) -> Result<String, PrivateStoreError> {
        let profile = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(PrivateStoreError::ProfileNotFound)?;
        let rendered = profile.canonical.render_mihomo_proxy(&profile.name, None);
        assemble_runtime_config(template, &rendered, controller_socket, &self.custom_rules)
            .map_err(PrivateStoreError::Config)
    }

    /// Prepare the last-selected profile without exposing its opaque ID.
    pub fn prepare_last_config(
        &self,
        template: &str,
        controller_socket: &str,
    ) -> Result<String, PrivateStoreError> {
        if self.last_id.is_empty() {
            return Err(PrivateStoreError::ProfileNotFound);
        }
        self.prepare_config(&self.last_id, template, controller_socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
    const SUBSCRIPTION_ID: &str = "10000000-0000-0000-0000-000000000001";

    fn store(uri: &str, protocol: &str) -> String {
        format!(
            r#"{{
              "version": 3,
              "activeId": "{PROFILE_ID}",
              "lastId": "{PROFILE_ID}",
              "profiles": [{{
                "id": "{PROFILE_ID}", "name": "Synthetic",
                "uri": {uri:?}, "protocol": "{protocol}",
                "subscriptionId": "{SUBSCRIPTION_ID}",
                "subscriptionKey": "{}", "missing": false, "favorite": true
              }}],
              "subscriptions": [{{
                "id": "{SUBSCRIPTION_ID}", "name": "Synthetic source",
                "url": "https://example.invalid/sub", "updatedAt": 1
              }}],
              "routingPreset": "custom",
              "customRules": [{{
                "id": "20000000-0000-0000-0000-000000000001",
                "kind": "domain", "value": "example.invalid", "action": "proxy"
              }}],
              "rulesUpdatedAt": 1,
              "startupConfigured": true,
              "startup": {{"enabled": true, "target": "profile", "profileId": "{PROFILE_ID}", "mode": "global"}},
              "onboardingComplete": true
            }}"#,
            "a".repeat(64)
        )
    }

    #[test]
    fn current_store_projects_only_safe_counts_and_prepares_config() {
        let input = store(
            "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp#Private",
            "vless",
        );
        let store = parse_private_store(&input).unwrap();
        assert_eq!(
            store.projection(),
            StoreProjection {
                version: 3,
                profile_count: 1,
                subscription_count: 1,
                vless_count: 1,
                trojan_count: 0,
                hysteria2_count: 0,
                tuic_count: 0,
                active_present: true,
                last_present: true,
                routing_preset: "custom".into(),
                custom_rule_count: 1,
                startup_configured: true,
                onboarding_complete: true,
            }
        );
        let config = store
            .prepare_config(
                PROFILE_ID,
                "proxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,DIRECT\n",
                "/run/user/1000/omavless/mihomo.sock",
            )
            .unwrap();
        assert!(config.contains("external-controller-unix"));
        assert!(config.contains("DOMAIN,example.invalid,PROXY"));
        assert!(!config.contains("external-controller:"));
    }

    #[test]
    fn legacy_defaults_and_all_existing_protocols_are_supported() {
        let cases = [
            (
                "trojan://password@203.0.113.2:443?security=tls&sni=cdn.example.invalid",
                "trojan",
            ),
            (
                "hy2://auth@203.0.113.3:443?sni=cdn.example.invalid",
                "hysteria2",
            ),
            (
                "tuic://22222222-2222-4222-8222-222222222222:password@203.0.113.4:443?sni=cdn.example.invalid",
                "tuic",
            ),
        ];
        for (uri, protocol) in cases {
            assert_eq!(
                parse_private_store(&store(uri, protocol))
                    .unwrap()
                    .projection()
                    .profile_count,
                1
            );
        }
        let legacy = format!(
            r#"{{"profiles":[{{"id":"{PROFILE_ID}","name":"Legacy","uri":"vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp"}}]}}"#
        );
        let projection = parse_private_store(&legacy).unwrap().projection();
        assert_eq!(projection.version, 3);
        assert_eq!(projection.routing_preset, "roscomvpn-default");
        assert!(!projection.startup_configured);
    }

    #[test]
    fn errors_and_debug_never_echo_private_values() {
        let private = "trojan://secret@private.example:443";
        for input in [
            private.to_owned(),
            store(private, "vless"),
            store("trojan://secret@bad", "trojan"),
        ] {
            let error = match parse_private_store(&input) {
                Ok(_) => panic!("private invalid store was accepted"),
                Err(error) => error,
            };
            let output = format!("{error:?} {error}");
            assert!(!output.contains("secret"));
            assert!(!output.contains("private.example"));
        }
    }

    #[test]
    fn malformed_names_urls_relationships_rules_and_timestamps_fail_closed() {
        let base = store(
            "vless://11111111-1111-4111-8111-111111111111@203.0.113.1:443?security=none&type=tcp",
            "vless",
        );
        let replacements = [
            ("\"Synthetic\"", "\" Synthetic \""),
            ("https://example.invalid/sub", "http://example.invalid/sub"),
            (&"a".repeat(64), "not-a-key"),
            ("\"example.invalid\"", "\"EXAMPLE.INVALID\""),
            ("\"updatedAt\": 1", "\"updatedAt\": -1"),
        ];
        for (from, to) in replacements {
            assert!(parse_private_store(&base.replacen(from, to, 1)).is_err());
        }
    }
}
